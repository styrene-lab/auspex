//! auspex-operator — Kubernetes operator for Omegon agent fleet management.
//!
//! Watches OmegonAgent CRDs and reconciles them into Deployments, CronJobs,
//! ConfigMaps, and Services. Exposes a fleet API for the Auspex UI.

mod crd;
mod external;
mod identity;
mod reconciler;

use std::{collections::BTreeMap, io::Cursor, net::SocketAddr, sync::Arc};

use auspex_core::agent_packages::{
    AgentPackageDeployRequest, OciImageAssessment, assess_oci_image_ref, builtin_agent_packages,
    find_builtin_agent_package,
};
use auspex_core::armory::{
    ArmoryClient, ArmoryDeploymentOverlay, ArmoryError, ArmoryIndex, ArmoryPlanOptions,
    DEFAULT_ARMORY_INDEX_URL, PolicySeverity, agent_package_from_armory_overlay,
    plan_armory_install,
};
use axum::{
    Json, Router,
    extract::{
        Path as AxumPath,
        ws::{Message as AxumWsMessage, WebSocket, WebSocketUpgrade},
    },
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::{get, post},
};
use futures_util::{SinkExt, StreamExt};
use k8s_openapi::api::{
    apps::v1::Deployment,
    batch::v1::{CronJob, Job},
    core::v1::{ConfigMap, Event, Secret},
};
use kube::{
    Api, Client, CustomResourceExt, ResourceExt,
    api::{ListParams, Patch, PatchParams},
    runtime::Controller,
};
use serde_json::Value;
use styrene_mqtt::{EmbeddedBrokerBuilder, EmbeddedBrokerConfig, broker::TcpListenerConfig};
use tokio_tungstenite::Connector;
use tower_http::services::ServeDir;
use tracing::{error, info, warn};

use crd::{AgentMode, ExternalAgent, OmegonAgent};
use reconciler::Context;

struct AcpProxyTarget {
    url: String,
    connector: Option<Connector>,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let _ = rustls::crypto::aws_lc_rs::default_provider().install_default();

    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "info,kube=warn".into()),
        )
        .init();

    // Print CRDs for installation: auspex-operator --crd
    if std::env::args().any(|a| a == "--crd") {
        let managed = OmegonAgent::crd();
        let external = ExternalAgent::crd();
        println!("---");
        print!("{}", serde_yaml::to_string(&managed)?);
        println!("---");
        print!("{}", serde_yaml::to_string(&external)?);
        return Ok(());
    }

    info!("auspex-operator starting");

    let client = Client::try_default().await?;

    // Namespace scoping: when AUSPEX_WATCH_NAMESPACE is set, the operator
    // only watches CRDs in that namespace. When unset, watches cluster-wide.
    // Production deployments should always set this to limit blast radius.
    let watch_namespace = std::env::var("AUSPEX_WATCH_NAMESPACE").ok();
    if let Some(ref ns) = watch_namespace {
        info!(namespace = %ns, "scoped to namespace");
    } else {
        warn!("AUSPEX_WATCH_NAMESPACE not set — watching all namespaces (cluster-wide)");
    }

    let ctx = Arc::new(Context {
        client: client.clone(),
        watch_namespace: watch_namespace.clone(),
    });

    ensure_primary_agent(&client, watch_namespace.as_deref()).await?;

    let mqtt_bind_addr: SocketAddr = std::env::var("AUSPEX_MQTT_BIND_ADDR")
        .unwrap_or_else(|_| "0.0.0.0:1883".into())
        .parse()?;
    let (_mqtt_broker, _mqtt_links) = EmbeddedBrokerBuilder::new(EmbeddedBrokerConfig {
        tcp_listener: Some(TcpListenerConfig {
            bind_addr: mqtt_bind_addr,
        }),
        ..Default::default()
    })
    .add_link("auspex-operator")
    .start()?;
    info!(bind_addr = %mqtt_bind_addr, "Aether MQTT broker listening");

    let agents: Api<OmegonAgent> = match &watch_namespace {
        Some(ns) => Api::namespaced(client.clone(), ns),
        None => Api::all(client.clone()),
    };
    let deployments: Api<Deployment> = match &watch_namespace {
        Some(ns) => Api::namespaced(client.clone(), ns),
        None => Api::all(client.clone()),
    };
    let cronjobs: Api<CronJob> = match &watch_namespace {
        Some(ns) => Api::namespaced(client.clone(), ns),
        None => Api::all(client.clone()),
    };
    let jobs: Api<Job> = match &watch_namespace {
        Some(ns) => Api::namespaced(client.clone(), ns),
        None => Api::all(client.clone()),
    };

    // Fleet API (health + instance list)
    let api_ctx = ctx.clone();
    let api_server = tokio::spawn(async move {
        // Serve the Auspex web UI from the dist directory.
        // In the container image, the WASM bundle is at /ui/dist.
        // Locally, fall back to the workspace dist directory.
        let web_ui_path = std::env::var("AUSPEX_WEB_UI_PATH").unwrap_or_else(|_| "/ui/dist".into());
        let serve_dir = ServeDir::new(&web_ui_path).append_index_html_on_directories(true);

        // Fleet API token: required for /api/* routes.
        // Set via AUSPEX_API_TOKEN env var or k8s Secret mount.
        // When unset, API is open (development mode only).
        let api_token = std::env::var("AUSPEX_API_TOKEN").ok();
        if api_token.is_none() {
            warn!("AUSPEX_API_TOKEN not set — fleet API is unauthenticated");
        }

        let api_routes = Router::new()
            .route(
                "/fleet",
                get({
                    let ctx = api_ctx.clone();
                    move || fleet_handler(ctx)
                }),
            )
            .route(
                "/fleet/{ns}/{name}/sbom",
                get({
                    let ctx = api_ctx.clone();
                    move |path: AxumPath<(String, String)>| sbom_handler(ctx, path)
                }),
            )
            .route(
                "/agents",
                post({
                    let ctx = api_ctx.clone();
                    move |body: Json<Value>| deploy_agent_handler(ctx, body)
                }),
            )
            .route("/packages", get(packages_handler))
            .route("/armory/packages", get(armory_packages_handler))
            .route(
                "/armory/packages/{kind}/{id}",
                get(|path: AxumPath<(String, String)>| armory_package_detail_handler(path)),
            )
            .route(
                "/armory/plan",
                post(|body: Json<Value>| armory_plan_handler(body)),
            )
            .route(
                "/armory/overlays",
                get({
                    let ctx = api_ctx.clone();
                    move || armory_overlays_handler(ctx)
                })
                .post({
                    let ctx = api_ctx.clone();
                    move |body: Json<Value>| upsert_armory_overlay_handler(ctx, None, body)
                }),
            )
            .route(
                "/armory/overlays/{id}",
                get({
                    let ctx = api_ctx.clone();
                    move |path: AxumPath<String>| armory_overlay_detail_handler(ctx, path)
                })
                .put({
                    let ctx = api_ctx.clone();
                    move |path: AxumPath<String>, body: Json<Value>| {
                        upsert_armory_overlay_handler(ctx, Some(path), body)
                    }
                })
                .delete({
                    let ctx = api_ctx.clone();
                    move |path: AxumPath<String>| delete_armory_overlay_handler(ctx, path)
                }),
            )
            .route(
                "/armory/preflight",
                post({
                    let ctx = api_ctx.clone();
                    move |body: Json<Value>| armory_preflight_handler(ctx, body)
                }),
            )
            .route(
                "/packages/{id}",
                get(|path: AxumPath<String>| package_detail_handler(path)),
            )
            .route(
                "/packages/{id}/deploy",
                post({
                    let ctx = api_ctx.clone();
                    move |path: AxumPath<String>, body: Option<Json<Value>>| {
                        deploy_package_handler(ctx, path, body)
                    }
                }),
            )
            .route(
                "/agents/{ns}/{name}",
                get({
                    let ctx = api_ctx.clone();
                    move |path: AxumPath<(String, String)>| agent_detail_handler(ctx, path)
                })
                .patch({
                    let ctx = api_ctx.clone();
                    move |path: AxumPath<(String, String)>, body: Json<Value>| {
                        patch_agent_handler(ctx, path, body)
                    }
                }),
            )
            .route(
                "/agents/{ns}/{name}/control-plane",
                get({
                    let ctx = api_ctx.clone();
                    move |path: AxumPath<(String, String)>| agent_control_plane_handler(ctx, path)
                }),
            )
            .route(
                "/agents/{ns}/{name}/acp",
                get({
                    let ctx = api_ctx.clone();
                    move |path: AxumPath<(String, String)>, ws: WebSocketUpgrade| {
                        agent_acp_proxy_handler(ctx, path, ws)
                    }
                }),
            )
            .route(
                "/agents/{ns}/{name}/rotate-control-tls",
                post({
                    let ctx = api_ctx.clone();
                    move |path: AxumPath<(String, String)>, body: Option<Json<Value>>| {
                        rotate_control_tls_handler(ctx, path, body)
                    }
                }),
            )
            .route(
                "/audit",
                get({
                    let ctx = api_ctx.clone();
                    move || audit_handler(ctx)
                }),
            )
            .route(
                "/secrets/grants",
                get({
                    let ctx = api_ctx.clone();
                    move || secret_grants_handler(ctx)
                }),
            );

        // Wrap API routes with bearer token validation when configured.
        let api_routes = if let Some(token) = api_token {
            let expected_value = format!("Bearer {token}");
            api_routes.layer(axum::middleware::from_fn(
                move |req: axum::extract::Request, next: axum::middleware::Next| {
                    let expected = expected_value.clone();
                    async move {
                        if request_has_valid_api_auth(&req, &expected) {
                            next.run(req).await
                        } else {
                            axum::http::Response::builder()
                                .status(401)
                                .body(axum::body::Body::from("unauthorized"))
                                .unwrap()
                        }
                    }
                },
            ))
        } else {
            api_routes
        };

        let app = Router::new()
            .route("/healthz", get(|| async { "ok" }))
            .nest("/api", api_routes)
            // Web UI: served last so API routes take precedence.
            .fallback_service(serve_dir);

        let listener = tokio::net::TcpListener::bind("0.0.0.0:8080").await.unwrap();
        info!("fleet API listening on :8080");
        axum::serve(listener, app).await.unwrap();
    });

    // Controller loop: managed agents (OmegonAgent CRDs → workloads)
    let managed_ctx = ctx.clone();
    let managed_controller = tokio::spawn(async move {
        info!("starting managed agent controller");
        Controller::new(agents, kube::runtime::watcher::Config::default())
            .owns(deployments, kube::runtime::watcher::Config::default())
            .owns(cronjobs, kube::runtime::watcher::Config::default())
            .owns(jobs, kube::runtime::watcher::Config::default())
            .run(reconciler::reconcile, reconciler::error_policy, managed_ctx)
            .for_each(|res| async move {
                match res {
                    Ok(o) => info!(agent = ?o, "reconciled"),
                    Err(e) => error!(error = %e, "reconcile failed"),
                }
            })
            .await;
    });

    // Controller loop: external agents (ExternalAgent CRDs → health probing)
    let external_agents: Api<ExternalAgent> = match &watch_namespace {
        Some(ns) => Api::namespaced(client.clone(), ns),
        None => Api::all(client.clone()),
    };
    let external_ctx = ctx.clone();
    let external_controller = tokio::spawn(async move {
        info!("starting external agent controller");
        Controller::new(external_agents, kube::runtime::watcher::Config::default())
            .run(external::reconcile, external::error_policy, external_ctx)
            .for_each(|res| async move {
                match res {
                    Ok(o) => info!(external_agent = ?o, "probed"),
                    Err(e) => error!(error = %e, "external probe failed"),
                }
            })
            .await;
    });

    // Run both controllers and the API server concurrently.
    tokio::select! {
        _ = managed_controller => error!("managed controller exited unexpectedly"),
        _ = external_controller => error!("external controller exited unexpectedly"),
        _ = api_server => error!("API server exited unexpectedly"),
    }
    Ok(())
}

fn request_has_valid_api_auth(req: &axum::extract::Request, expected_bearer: &str) -> bool {
    let auth_header = req
        .headers()
        .get("authorization")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    if constant_time_eq(auth_header, expected_bearer) {
        return true;
    }

    // Browser WebSocket clients cannot set arbitrary Authorization headers.
    // Allow query-token auth only on WebSocket upgrade requests so normal API
    // calls stay on the less leak-prone bearer-header path.
    if !req
        .headers()
        .get(axum::http::header::UPGRADE)
        .and_then(|v| v.to_str().ok())
        .is_some_and(|value| value.eq_ignore_ascii_case("websocket"))
    {
        return false;
    }

    let Some(expected_token) = expected_bearer.strip_prefix("Bearer ") else {
        return false;
    };
    req.uri().query().is_some_and(|query| {
        url::form_urlencoded::parse(query.as_bytes()).any(|(key, value)| {
            matches!(key.as_ref(), "access_token" | "token")
                && constant_time_eq(value.as_ref(), expected_token)
        })
    })
}

fn constant_time_eq(actual: &str, expected: &str) -> bool {
    // Constant-time comparison to prevent timing attacks. Always iterate the
    // full expected length regardless of actual length, comparing against
    // zero-padding for short inputs so the loop duration doesn't leak length.
    let actual_bytes = actual.as_bytes();
    let expected_bytes = expected.as_bytes();
    let mut diff = (actual_bytes.len() ^ expected_bytes.len()) as u8;
    for (i, expected_byte) in expected_bytes.iter().enumerate() {
        let actual_byte = actual_bytes.get(i).copied().unwrap_or(0xff);
        diff |= actual_byte ^ expected_byte;
    }
    diff == 0
}

async fn ensure_primary_agent(
    client: &Client,
    watch_namespace: Option<&str>,
) -> anyhow::Result<()> {
    if !env_flag("AUSPEX_BOOTSTRAP_PRIMARY_AGENT", true) {
        return Ok(());
    }

    let config = PrimaryAgentBootstrapConfig::from_env(watch_namespace);

    let agents: Api<OmegonAgent> = Api::namespaced(client.clone(), &config.namespace);
    if agents.get_opt(&config.name).await?.is_some() {
        info!(namespace = %config.namespace, name = %config.name, "primary OmegonAgent already exists");
        return Ok(());
    }

    agents
        .patch(
            &config.name,
            &PatchParams::apply("auspex-operator").force(),
            &Patch::Apply(primary_agent_manifest(&config)),
        )
        .await?;
    info!(namespace = %config.namespace, name = %config.name, "bootstrapped primary OmegonAgent");
    Ok(())
}

struct PrimaryAgentBootstrapConfig {
    namespace: String,
    name: String,
    image: String,
    model: String,
    secret_name: Option<String>,
    auth_json_secret: Option<String>,
    control_tls_secret: Option<String>,
}

impl PrimaryAgentBootstrapConfig {
    fn from_env(watch_namespace: Option<&str>) -> Self {
        Self {
            namespace: std::env::var("AUSPEX_PRIMARY_AGENT_NAMESPACE")
                .ok()
                .or_else(|| watch_namespace.map(str::to_string))
                .unwrap_or_else(|| "omegon-agents".to_string()),
            name: std::env::var("AUSPEX_PRIMARY_AGENT_NAME")
                .unwrap_or_else(|_| "auspex-primary".into()),
            image: std::env::var("AUSPEX_PRIMARY_AGENT_IMAGE")
                .unwrap_or_else(|_| "ghcr.io/styrene-lab/omegon-agents:latest".into()),
            model: std::env::var("AUSPEX_PRIMARY_AGENT_MODEL")
                .unwrap_or_else(|_| "anthropic:claude-sonnet-4-6".into()),
            secret_name: std::env::var("AUSPEX_PRIMARY_AGENT_SECRET").ok(),
            auth_json_secret: std::env::var("AUSPEX_PRIMARY_AGENT_AUTH_JSON_SECRET").ok(),
            control_tls_secret: std::env::var("AUSPEX_PRIMARY_AGENT_CONTROL_TLS_SECRET").ok(),
        }
    }
}

fn primary_agent_manifest(config: &PrimaryAgentBootstrapConfig) -> Value {
    let mut secrets = serde_json::json!({});
    if let Some(secret_name) = config.secret_name.as_ref() {
        secrets["secretName"] = serde_json::json!(secret_name);
    }
    if let Some(auth_json_secret) = config.auth_json_secret.as_ref() {
        secrets["authJsonSecret"] = serde_json::json!(auth_json_secret);
    }

    let mut manifest = serde_json::json!({
        "apiVersion": "styrene.sh/v1alpha1",
        "kind": "OmegonAgent",
        "metadata": {
            "name": config.name,
            "namespace": config.namespace,
            "labels": {
                "app.kubernetes.io/part-of": "auspex",
                "styrene.sh/agent-role": "primary-driver",
            },
        },
        "spec": {
            "agent": "styrene.auspex-primary",
            "model": config.model,
            "posture": "architect",
            "role": "primary-driver",
            "mode": "daemon",
            "image": config.image,
            "terminalTool": false,
            "secrets": secrets,
            "resources": {
                "cpu": "1",
                "memory": "2Gi",
            },
        },
    });

    if let Some(secret) = config.control_tls_secret.as_ref() {
        manifest["spec"]["identity"] = serde_json::json!({
            "provision": true,
            "securityTier": "file",
            "meshRole": "operator",
            "mtls": true,
        });
        manifest["spec"]["controlPlane"] = serde_json::json!({
            "tls": {
                "enabled": true,
                "secretName": secret,
            }
        });
    }

    manifest
}

fn env_flag(name: &str, default: bool) -> bool {
    std::env::var(name)
        .ok()
        .map(|value| {
            matches!(
                value.to_ascii_lowercase().as_str(),
                "1" | "true" | "yes" | "on"
            )
        })
        .unwrap_or(default)
}

async fn fleet_handler(ctx: Arc<Context>) -> (axum::http::HeaderMap, Json<serde_json::Value>) {
    let mut headers = axum::http::HeaderMap::new();
    // Prevent excessive polling — fleet data is only as fresh as the last reconcile.
    headers.insert("cache-control", "private, max-age=5".parse().unwrap());
    let body = fleet_handler_inner(ctx).await;
    (headers, body)
}

async fn fleet_handler_inner(ctx: Arc<Context>) -> Json<serde_json::Value> {
    let api: Api<OmegonAgent> = match &ctx.watch_namespace {
        Some(ns) => Api::namespaced(ctx.client.clone(), ns),
        None => Api::all(ctx.client.clone()),
    };
    let agents = match api.list(&Default::default()).await {
        Ok(list) => list,
        Err(e) => {
            return Json(serde_json::json!({ "error": e.to_string() }));
        }
    };

    let mut fleet = Vec::with_capacity(agents.items.len());
    for a in &agents.items {
        let name = a.name_any();
        let namespace = a.namespace().unwrap_or_else(|| "default".into());
        let lifecycle = managed_agent_lifecycle(&ctx.client, a).await;
        fleet.push(serde_json::json!({
            "name": name,
            "namespace": namespace,
            "agent": a.spec.agent,
            "model": a.spec.model,
            "posture": a.spec.posture,
            "role": a.spec.role,
            "mode": a.spec.mode,
            "image": a.spec.image,
            "profile": a.spec.profile,
            "package": a.labels().get("styrene.sh/agent-package"),
            "home_stack": a.labels().get("styrene.sh/home-stack"),
            "is_primary": is_primary_agent(a),
            "control_plane": managed_agent_control_plane(a),
            "lifecycle": lifecycle,
            "status": a.status.as_ref().map(|s| &s.phase),
            "sbom": a.status.as_ref().and_then(|s| s.sbom.as_ref()).map(|sb| {
                serde_json::json!({
                    "available": sb.available,
                    "format": sb.format,
                    "artifact_ref": sb.artifact_ref,
                    "image_digest": sb.image_digest,
                    "component_count": sb.component_count,
                    "vulnerability_count": sb.vulnerability_count,
                    "signature_verified": sb.signature_verified,
                })
            }),
        }));
    }

    // External agents (observed, not managed).
    let ext_api: Api<ExternalAgent> = match &ctx.watch_namespace {
        Some(ns) => Api::namespaced(ctx.client.clone(), ns),
        None => Api::all(ctx.client.clone()),
    };
    if let Ok(ext_list) = ext_api.list(&Default::default()).await {
        for a in &ext_list.items {
            fleet.push(serde_json::json!({
                "name": a.metadata.name,
                "namespace": a.metadata.namespace,
                "agent": a.status.as_ref().and_then(|s| s.agent_id.as_ref()),
                "model": a.status.as_ref().and_then(|s| s.model.as_ref()),
                "mode": "external",
                "image": null,
                "profile": null,
                "endpoint": a.spec.endpoint,
                "display_name": a.spec.display_name,
                "reachability": a.status.as_ref().map(|s| &s.reachability),
                "omegon_version": a.status.as_ref().and_then(|s| s.omegon_version.as_ref()),
                "ws_url": a.status.as_ref().and_then(|s| s.ws_url.as_ref()),
                "status": a.status.as_ref().map(|s| &s.reachability),
                "sbom": a.status.as_ref().and_then(|s| s.sbom.as_ref()).map(|sb| {
                    serde_json::json!({
                        "available": sb.available,
                        "format": sb.format,
                        "artifact_ref": sb.artifact_ref,
                        "image_digest": sb.image_digest,
                        "component_count": sb.component_count,
                        "vulnerability_count": sb.vulnerability_count,
                        "signature_verified": sb.signature_verified,
                    })
                }),
            }));
        }
    }

    Json(serde_json::json!({ "agents": fleet }))
}

async fn agent_detail_handler(
    ctx: Arc<Context>,
    AxumPath((ns, name)): AxumPath<(String, String)>,
) -> Json<Value> {
    if let Some(error) = namespace_scope_error(&ctx, &ns) {
        return Json(error);
    }

    let api: Api<OmegonAgent> = Api::namespaced(ctx.client.clone(), &ns);
    match api.get(&name).await {
        Ok(agent) => Json(managed_agent_detail(&agent)),
        Err(error) => Json(serde_json::json!({ "error": error.to_string() })),
    }
}

async fn agent_control_plane_handler(
    ctx: Arc<Context>,
    AxumPath((ns, name)): AxumPath<(String, String)>,
) -> Json<Value> {
    if let Some(error) = namespace_scope_error(&ctx, &ns) {
        return Json(error);
    }

    let api: Api<OmegonAgent> = Api::namespaced(ctx.client.clone(), &ns);
    match api.get(&name).await {
        Ok(agent) => Json(serde_json::json!({
            "name": name,
            "namespace": ns,
            "control_plane": managed_agent_control_plane(&agent),
        })),
        Err(error) => Json(serde_json::json!({ "error": error.to_string() })),
    }
}

async fn agent_acp_proxy_handler(
    ctx: Arc<Context>,
    AxumPath((ns, name)): AxumPath<(String, String)>,
    ws: WebSocketUpgrade,
) -> Response {
    match resolve_managed_agent_acp_proxy_target(&ctx, &ns, &name).await {
        Ok(target) => ws
            .on_upgrade(move |socket| proxy_acp_websocket(socket, target))
            .into_response(),
        Err(error) => error.into_response(),
    }
}

async fn resolve_managed_agent_acp_proxy_target(
    ctx: &Context,
    ns: &str,
    name: &str,
) -> Result<AcpProxyTarget, (StatusCode, String)> {
    if let Some(error) = namespace_scope_error(ctx, ns) {
        return Err((
            StatusCode::FORBIDDEN,
            error
                .get("error")
                .and_then(Value::as_str)
                .unwrap_or("namespace outside operator scope")
                .to_string(),
        ));
    }

    let api: Api<OmegonAgent> = Api::namespaced(ctx.client.clone(), ns);
    let agent = api
        .get(name)
        .await
        .map_err(|error| (StatusCode::NOT_FOUND, error.to_string()))?;
    if agent.spec.mode != AgentMode::Daemon {
        return Err((
            StatusCode::BAD_REQUEST,
            "ACP proxy is only available for daemon agents".to_string(),
        ));
    }

    let Some(target) = managed_agent_acp_url(&agent) else {
        return Err((
            StatusCode::BAD_GATEWAY,
            "agent has no ACP control-plane endpoint".to_string(),
        ));
    };

    let connector = match reconciler::resolved_control_tls(&agent, name) {
        Some(tls) => Some(control_tls_connector(ctx, ns, &tls).await?),
        None => None,
    };
    Ok(AcpProxyTarget {
        url: target,
        connector,
    })
}

async fn control_tls_connector(
    ctx: &Context,
    ns: &str,
    tls: &reconciler::ResolvedControlTls,
) -> Result<Connector, (StatusCode, String)> {
    let api: Api<Secret> = Api::namespaced(ctx.client.clone(), ns);
    let secret = api.get(&tls.secret_name).await.map_err(|error| {
        (
            StatusCode::BAD_GATEWAY,
            format!(
                "control-TLS Secret '{}' unavailable: {error}",
                tls.secret_name
            ),
        )
    })?;

    let cert_pem = secret_bytes(&secret, &tls.cert_key)?;
    let key_pem = secret_bytes(&secret, &tls.key_key)?;
    let trust_pem = match tls.client_ca_key.as_ref() {
        Some(key) => secret_bytes(&secret, key)?,
        None => cert_pem.clone(),
    };

    let root_certs = parse_pem_certs(&trust_pem, "control-TLS trust bundle")?;
    let mut roots = rustls::RootCertStore::empty();
    let (accepted, _ignored) = roots.add_parsable_certificates(root_certs);
    if accepted == 0 {
        return Err((
            StatusCode::BAD_GATEWAY,
            "control-TLS trust bundle contains no usable certificates".to_string(),
        ));
    }

    let client_certs = parse_pem_certs(&cert_pem, "control-TLS client certificate")?;
    let private_key = parse_pem_private_key(&key_pem, "control-TLS client key")?;
    let config = if tls.client_ca_key.is_some() {
        rustls::ClientConfig::builder()
            .with_root_certificates(roots)
            .with_client_auth_cert(client_certs, private_key)
            .map_err(|error| {
                (
                    StatusCode::BAD_GATEWAY,
                    format!("control-TLS client certificate rejected: {error}"),
                )
            })?
    } else {
        rustls::ClientConfig::builder()
            .with_root_certificates(roots)
            .with_no_client_auth()
    };

    Ok(Connector::Rustls(Arc::new(config)))
}

fn secret_bytes(secret: &Secret, key: &str) -> Result<Vec<u8>, (StatusCode, String)> {
    secret
        .data
        .as_ref()
        .and_then(|data| data.get(key))
        .map(|value| value.0.clone())
        .ok_or_else(|| {
            (
                StatusCode::BAD_GATEWAY,
                format!(
                    "Secret '{}' is missing required key '{key}'",
                    secret.name_any()
                ),
            )
        })
}

fn parse_pem_certs(
    pem: &[u8],
    label: &str,
) -> Result<Vec<rustls::pki_types::CertificateDer<'static>>, (StatusCode, String)> {
    rustls_pemfile::certs(&mut Cursor::new(pem))
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| {
            (
                StatusCode::BAD_GATEWAY,
                format!("{label} parse failed: {error}"),
            )
        })
}

fn parse_pem_private_key(
    pem: &[u8],
    label: &str,
) -> Result<rustls::pki_types::PrivateKeyDer<'static>, (StatusCode, String)> {
    rustls_pemfile::private_key(&mut Cursor::new(pem))
        .map_err(|error| {
            (
                StatusCode::BAD_GATEWAY,
                format!("{label} parse failed: {error}"),
            )
        })?
        .ok_or_else(|| {
            (
                StatusCode::BAD_GATEWAY,
                format!("{label} contains no private key"),
            )
        })
}

async fn proxy_acp_websocket(client_socket: WebSocket, target: AcpProxyTarget) {
    let upstream = tokio_tungstenite::connect_async_tls_with_config(
        target.url.as_str(),
        None,
        false,
        target.connector,
    )
    .await;
    let Ok((upstream_socket, _response)) = upstream else {
        warn!(target = %target.url, "ACP upstream websocket connection failed");
        return;
    };

    let (mut client_tx, mut client_rx) = client_socket.split();
    let (mut upstream_tx, mut upstream_rx) = upstream_socket.split();

    let client_to_upstream = async {
        while let Some(message) = client_rx.next().await {
            let Ok(message) = message else {
                break;
            };
            let Some(message) = axum_to_tungstenite_message(message) else {
                continue;
            };
            if upstream_tx.send(message).await.is_err() {
                break;
            }
        }
    };

    let upstream_to_client = async {
        while let Some(message) = upstream_rx.next().await {
            let Ok(message) = message else {
                break;
            };
            let Some(message) = tungstenite_to_axum_message(message) else {
                continue;
            };
            if client_tx.send(message).await.is_err() {
                break;
            }
        }
    };

    tokio::select! {
        _ = client_to_upstream => {}
        _ = upstream_to_client => {}
    }
}

fn axum_to_tungstenite_message(
    message: AxumWsMessage,
) -> Option<tokio_tungstenite::tungstenite::Message> {
    use tokio_tungstenite::tungstenite::Message;

    match message {
        AxumWsMessage::Text(text) => Some(Message::Text(text.to_string().into())),
        AxumWsMessage::Binary(data) => Some(Message::Binary(data)),
        AxumWsMessage::Ping(data) => Some(Message::Ping(data)),
        AxumWsMessage::Pong(data) => Some(Message::Pong(data)),
        AxumWsMessage::Close(_) => Some(Message::Close(None)),
    }
}

fn tungstenite_to_axum_message(
    message: tokio_tungstenite::tungstenite::Message,
) -> Option<AxumWsMessage> {
    use tokio_tungstenite::tungstenite::Message;

    match message {
        Message::Text(text) => Some(AxumWsMessage::Text(text.to_string().into())),
        Message::Binary(data) => Some(AxumWsMessage::Binary(data)),
        Message::Ping(data) => Some(AxumWsMessage::Ping(data)),
        Message::Pong(data) => Some(AxumWsMessage::Pong(data)),
        Message::Close(_) => Some(AxumWsMessage::Close(None)),
        Message::Frame(_) => None,
    }
}

async fn deploy_agent_handler(ctx: Arc<Context>, Json(mut manifest): Json<Value>) -> Json<Value> {
    manifest["apiVersion"] = serde_json::json!("styrene.sh/v1alpha1");
    manifest["kind"] = serde_json::json!("OmegonAgent");

    let name = manifest
        .pointer("/metadata/name")
        .and_then(Value::as_str)
        .unwrap_or("")
        .trim()
        .to_string();
    if name.is_empty() {
        return Json(serde_json::json!({ "error": "metadata.name is required" }));
    }

    let ns = manifest
        .pointer("/metadata/namespace")
        .and_then(Value::as_str)
        .map(str::to_string)
        .or_else(|| ctx.watch_namespace.clone())
        .unwrap_or_else(|| "default".to_string());
    manifest["metadata"]["namespace"] = serde_json::json!(ns.clone());

    if let Some(error) = namespace_scope_error(&ctx, &ns) {
        return Json(error);
    }
    if let Err(error) = serde_json::from_value::<OmegonAgent>(manifest.clone()) {
        return Json(
            serde_json::json!({ "error": format!("invalid OmegonAgent manifest: {error}") }),
        );
    }

    let api: Api<OmegonAgent> = Api::namespaced(ctx.client.clone(), &ns);
    match api
        .patch(
            &name,
            &PatchParams::apply("auspex-webui").force(),
            &Patch::Apply(manifest),
        )
        .await
    {
        Ok(agent) => Json(serde_json::json!({ "agent": managed_agent_detail(&agent) })),
        Err(error) => Json(serde_json::json!({ "error": error.to_string() })),
    }
}

async fn packages_handler() -> Json<Value> {
    Json(serde_json::json!({
        "packages": builtin_agent_packages(),
        "source": "builtin",
        "next_source": "armory-signum",
    }))
}

async fn armory_packages_handler() -> Json<Value> {
    match fetch_armory_index().await {
        Ok(index) => Json(serde_json::json!({
            "packages": index.items,
            "source": "armory",
            "generatedAt": index.generated_at,
            "registry": index.registry,
        })),
        Err(error) => Json(serde_json::json!({ "error": error.to_string() })),
    }
}

async fn armory_package_detail_handler(
    AxumPath((kind, id)): AxumPath<(String, String)>,
) -> Json<Value> {
    let package_ref = format!("{kind}/{id}");
    match fetch_armory_index().await {
        Ok(index) => match index.get(&package_ref) {
            Some(package) => Json(serde_json::json!({
                "package": package,
                "source": "armory",
            })),
            None => Json(
                serde_json::json!({ "error": format!("unknown Armory package '{package_ref}'") }),
            ),
        },
        Err(error) => Json(serde_json::json!({ "error": error.to_string() })),
    }
}

async fn armory_plan_handler(Json(body): Json<Value>) -> Json<Value> {
    let Some(package_ref) = body
        .get("packageRef")
        .or_else(|| body.get("package_ref"))
        .and_then(|value| value.as_str())
        .filter(|value| !value.trim().is_empty())
    else {
        return Json(serde_json::json!({ "error": "packageRef is required" }));
    };
    let options = ArmoryPlanOptions {
        include_optional: body
            .get("includeOptional")
            .or_else(|| body.get("include_optional"))
            .and_then(|value| value.as_bool())
            .unwrap_or(false),
    };

    match fetch_armory_index().await {
        Ok(index) => match index.get(package_ref) {
            Some(package) => Json(serde_json::json!({
                "plan": plan_armory_install(package, options),
                "source": "armory",
            })),
            None => Json(
                serde_json::json!({ "error": format!("unknown Armory package '{package_ref}'") }),
            ),
        },
        Err(error) => Json(serde_json::json!({ "error": error.to_string() })),
    }
}

async fn armory_overlays_handler(ctx: Arc<Context>) -> Json<Value> {
    let (namespace, name) = armory_overlay_store_location(&ctx);
    match read_armory_overlay_config_data(&ctx).await {
        Ok(data) => {
            let (overlays, errors) = parse_armory_overlay_config_data(&data);
            Json(serde_json::json!({
                "overlays": overlays,
                "errors": errors,
                "source": "config-map",
                "configMap": {
                    "namespace": namespace,
                    "name": name,
                },
            }))
        }
        Err(error) => Json(serde_json::json!({ "error": error })),
    }
}

async fn armory_overlay_detail_handler(
    ctx: Arc<Context>,
    AxumPath(id): AxumPath<String>,
) -> Json<Value> {
    match read_armory_overlay_by_id(&ctx, &id).await {
        Ok(Some(overlay)) => Json(serde_json::json!({
            "overlay": overlay,
            "source": "config-map",
        })),
        Ok(None) => Json(serde_json::json!({ "error": format!("unknown Armory overlay '{id}'") })),
        Err(error) => Json(serde_json::json!({ "error": error })),
    }
}

async fn upsert_armory_overlay_handler(
    ctx: Arc<Context>,
    id: Option<AxumPath<String>>,
    Json(body): Json<Value>,
) -> Json<Value> {
    let id = id.map(|AxumPath(id)| id);
    let overlay = match armory_overlay_from_body(body, id.as_deref()) {
        Ok(overlay) => overlay,
        Err(error) => return Json(serde_json::json!({ "error": error })),
    };

    let (namespace, name) = armory_overlay_store_location(&ctx);
    match write_armory_overlay(&ctx, &overlay).await {
        Ok(()) => Json(serde_json::json!({
            "overlay": overlay,
            "source": "config-map",
            "configMap": {
                "namespace": namespace,
                "name": name,
            },
        })),
        Err(error) => Json(serde_json::json!({ "error": error })),
    }
}

async fn delete_armory_overlay_handler(
    ctx: Arc<Context>,
    AxumPath(id): AxumPath<String>,
) -> Json<Value> {
    let (namespace, name) = armory_overlay_store_location(&ctx);
    match delete_armory_overlay(&ctx, &id).await {
        Ok(deleted) => Json(serde_json::json!({
            "deleted": deleted,
            "id": id,
            "source": "config-map",
            "configMap": {
                "namespace": namespace,
                "name": name,
            },
        })),
        Err(error) => Json(serde_json::json!({ "error": error })),
    }
}

async fn armory_preflight_handler(ctx: Arc<Context>, Json(body): Json<Value>) -> Json<Value> {
    let Some(package_ref) = body
        .get("packageRef")
        .or_else(|| body.get("package_ref"))
        .and_then(|value| value.as_str())
        .filter(|value| !value.trim().is_empty())
    else {
        return Json(serde_json::json!({ "error": "packageRef is required" }));
    };
    let options = ArmoryPlanOptions {
        include_optional: body
            .get("includeOptional")
            .or_else(|| body.get("include_optional"))
            .and_then(|value| value.as_bool())
            .unwrap_or(false),
    };

    let overlay = match resolve_armory_preflight_overlay(&ctx, &body).await {
        Ok(overlay) => overlay,
        Err(error) => return Json(serde_json::json!({ "error": error })),
    };

    let index = match fetch_armory_index().await {
        Ok(index) => index,
        Err(error) => return Json(serde_json::json!({ "error": error.to_string() })),
    };
    let Some(package) = index.get(package_ref) else {
        return Json(
            serde_json::json!({ "error": format!("unknown Armory package '{package_ref}'") }),
        );
    };

    let plan = plan_armory_install(package, options);
    let blocked = plan
        .policy_gates
        .iter()
        .any(|gate| gate.severity == PolicySeverity::Blocked);
    let requires_approval = plan
        .policy_gates
        .iter()
        .any(|gate| gate.severity == PolicySeverity::ApprovalRequired);
    let agent_package = match agent_package_from_armory_overlay(package, &overlay, &plan) {
        Ok(package) => package,
        Err(error) => return Json(serde_json::json!({ "error": error.to_string() })),
    };

    let deploy_request = match body.get("deploy").cloned() {
        Some(value) => match serde_json::from_value::<AgentPackageDeployRequest>(value) {
            Ok(request) => request,
            Err(error) => {
                return Json(serde_json::json!({
                    "error": format!("invalid deploy request: {error}")
                }));
            }
        },
        None => AgentPackageDeployRequest::default(),
    };
    let mut deploy_request = deploy_request;
    if deploy_request.namespace.is_none() {
        deploy_request.namespace = overlay
            .namespace
            .clone()
            .or_else(|| ctx.watch_namespace.clone())
            .or_else(|| Some("default".into()));
    }
    if deploy_request.name.is_none() {
        deploy_request.name = Some(overlay.id.clone());
    }
    let manifest = agent_package.omegon_agent_manifest(&deploy_request);
    let namespace = manifest["metadata"]["namespace"]
        .as_str()
        .unwrap_or("default");
    let scope_error = namespace_scope_error(&ctx, namespace);
    let oci_policy = oci_preflight_policy_from_body(&body);
    let image_ref = manifest["spec"]["image"].as_str().unwrap_or_default();
    let image_assessment = assess_oci_image_ref(image_ref);
    let supply_chain = armory_supply_chain_projection(package, &image_assessment, &oci_policy);
    let oci_policy_blocked = oci_policy.blocks(&image_assessment);

    Json(serde_json::json!({
        "package": package,
        "overlay": overlay,
        "plan": plan,
        "agentPackage": agent_package,
        "deployRequest": deploy_request,
        "manifest": manifest,
        "deployable": !blocked && !oci_policy_blocked && scope_error.is_none(),
        "blocked": blocked || oci_policy_blocked,
        "requiresApproval": requires_approval,
        "scopeError": scope_error,
        "ociPolicy": {
            "mode": oci_policy.as_str(),
            "blocked": oci_policy_blocked,
            "reason": if oci_policy_blocked {
                Some("strict OCI policy requires a valid digest-pinned image")
            } else {
                None
            },
        },
        "supplyChain": supply_chain,
        "secretRequests": {
            "required": plan.required_secrets,
            "optional": plan.optional_secrets,
        },
        "source": "armory",
    }))
}

async fn fetch_armory_index() -> Result<ArmoryIndex, ArmoryError> {
    let index_url = std::env::var("AUSPEX_ARMORY_INDEX_URL")
        .unwrap_or_else(|_| DEFAULT_ARMORY_INDEX_URL.into());
    let mut client = ArmoryClient::new(index_url);
    client.fetch_index().await.cloned()
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum OciPreflightPolicy {
    Warn,
    Strict,
}

impl OciPreflightPolicy {
    fn as_str(self) -> &'static str {
        match self {
            Self::Warn => "warn",
            Self::Strict => "strict",
        }
    }

    fn blocks(self, assessment: &OciImageAssessment) -> bool {
        self == Self::Strict && !assessment.digest_pinned
    }
}

fn oci_preflight_policy_from_body(body: &Value) -> OciPreflightPolicy {
    let policy = body
        .get("ociPolicy")
        .or_else(|| body.get("oci_policy"))
        .and_then(Value::as_str)
        .map(str::to_string)
        .or_else(|| std::env::var("AUSPEX_OCI_PREFLIGHT_POLICY").ok())
        .unwrap_or_else(|| "warn".into());
    match policy.trim().to_ascii_lowercase().as_str() {
        "strict" | "enforce" | "required" => OciPreflightPolicy::Strict,
        _ => OciPreflightPolicy::Warn,
    }
}

fn armory_supply_chain_projection(
    package: &auspex_core::armory::ArmoryPackage,
    image: &OciImageAssessment,
    policy: &OciPreflightPolicy,
) -> Value {
    serde_json::json!({
        "image": image,
        "packageArtifact": {
            "ociRef": empty_as_null(&package.oci_ref),
            "artifactType": empty_as_null(&package.artifact_type),
            "payloadDigest": empty_as_null(&package.payload_digest),
            "verifyCommand": empty_as_null(&package.verify_command),
        },
        "sbom": {
            "expected": true,
            "verified": false,
            "status": "not-resolved-in-preflight",
        },
        "signature": {
            "expected": *policy == OciPreflightPolicy::Strict,
            "verified": false,
            "status": "not-resolved-in-preflight",
        },
        "provenance": {
            "expected": *policy == OciPreflightPolicy::Strict,
            "verified": false,
            "status": "not-resolved-in-preflight",
        },
    })
}

fn empty_as_null(value: &str) -> Option<&str> {
    let value = value.trim();
    if value.is_empty() { None } else { Some(value) }
}

fn armory_overlay_store_location(ctx: &Context) -> (String, String) {
    let namespace = ctx
        .watch_namespace
        .clone()
        .or_else(|| std::env::var("AUSPEX_ARMORY_OVERLAYS_NAMESPACE").ok())
        .unwrap_or_else(|| "default".into());
    let name = std::env::var("AUSPEX_ARMORY_OVERLAYS_CONFIG_MAP")
        .unwrap_or_else(|_| "auspex-armory-overlays".into());
    (namespace, name)
}

async fn read_armory_overlay_config_data(
    ctx: &Context,
) -> Result<BTreeMap<String, String>, String> {
    let (namespace, name) = armory_overlay_store_location(ctx);
    let api: Api<ConfigMap> = Api::namespaced(ctx.client.clone(), &namespace);
    let config_map = api
        .get_opt(&name)
        .await
        .map_err(|error| format!("failed to read Armory overlay ConfigMap: {error}"))?;
    Ok(config_map.and_then(|cm| cm.data).unwrap_or_default())
}

async fn read_armory_overlay_by_id(
    ctx: &Context,
    id: &str,
) -> Result<Option<ArmoryDeploymentOverlay>, String> {
    let key = armory_overlay_data_key(id)?;
    let data = read_armory_overlay_config_data(ctx).await?;
    let Some(raw) = data.get(&key) else {
        return Ok(None);
    };
    let overlay = parse_armory_overlay_json(&key, raw)?;
    Ok(Some(overlay))
}

async fn write_armory_overlay(
    ctx: &Context,
    overlay: &ArmoryDeploymentOverlay,
) -> Result<(), String> {
    overlay
        .validate()
        .map_err(|error| format!("invalid Armory overlay: {error}"))?;
    let (namespace, name) = armory_overlay_store_location(ctx);
    let mut data = read_armory_overlay_config_data(ctx).await?;
    upsert_armory_overlay_data(&mut data, overlay)?;
    apply_armory_overlay_config_map(ctx, &namespace, &name, data).await
}

async fn delete_armory_overlay(ctx: &Context, id: &str) -> Result<bool, String> {
    let (namespace, name) = armory_overlay_store_location(ctx);
    let mut data = read_armory_overlay_config_data(ctx).await?;
    let key = armory_overlay_data_key(id)?;
    let deleted = data.remove(&key).is_some();
    apply_armory_overlay_config_map(ctx, &namespace, &name, data).await?;
    Ok(deleted)
}

async fn apply_armory_overlay_config_map(
    ctx: &Context,
    namespace: &str,
    name: &str,
    data: BTreeMap<String, String>,
) -> Result<(), String> {
    let api: Api<ConfigMap> = Api::namespaced(ctx.client.clone(), namespace);
    let manifest = serde_json::json!({
        "apiVersion": "v1",
        "kind": "ConfigMap",
        "metadata": {
            "name": name,
            "namespace": namespace,
            "labels": {
                "app.kubernetes.io/part-of": "auspex",
                "auspex.styrene.sh/config": "armory-overlays",
            },
        },
        "data": data,
    });
    api.patch(
        name,
        &PatchParams::apply("auspex-webui").force(),
        &Patch::Apply(manifest),
    )
    .await
    .map(|_| ())
    .map_err(|error| format!("failed to write Armory overlay ConfigMap: {error}"))
}

async fn resolve_armory_preflight_overlay(
    ctx: &Context,
    body: &Value,
) -> Result<ArmoryDeploymentOverlay, String> {
    if let Some(value) = body.get("overlay") {
        return armory_overlay_from_body(value.clone(), None);
    }
    let Some(overlay_id) = body
        .get("overlayId")
        .or_else(|| body.get("overlay_id"))
        .and_then(Value::as_str)
        .filter(|value| !value.trim().is_empty())
    else {
        return Err("overlay or overlayId is required".into());
    };
    read_armory_overlay_by_id(ctx, overlay_id)
        .await?
        .ok_or_else(|| format!("unknown Armory overlay '{overlay_id}'"))
}

fn armory_overlay_from_body(
    body: Value,
    expected_id: Option<&str>,
) -> Result<ArmoryDeploymentOverlay, String> {
    let value = body.get("overlay").cloned().unwrap_or(body);
    let mut overlay = serde_json::from_value::<ArmoryDeploymentOverlay>(value)
        .map_err(|error| format!("invalid Armory overlay: {error}"))?;
    if overlay.id.trim().is_empty() {
        overlay.id = expected_id.unwrap_or_default().to_string();
    }
    if let Some(expected_id) = expected_id
        && overlay.id != expected_id
    {
        return Err(format!(
            "overlay id '{}' does not match route id '{expected_id}'",
            overlay.id
        ));
    }
    overlay
        .validate()
        .map_err(|error| format!("invalid Armory overlay: {error}"))?;
    armory_overlay_data_key(&overlay.id)?;
    Ok(overlay)
}

fn parse_armory_overlay_config_data(
    data: &BTreeMap<String, String>,
) -> (Vec<ArmoryDeploymentOverlay>, Vec<Value>) {
    let mut overlays = Vec::new();
    let mut errors = Vec::new();
    for (key, raw) in data {
        if !key.ends_with(".json") {
            continue;
        }
        match parse_armory_overlay_json(key, raw) {
            Ok(overlay) => overlays.push(overlay),
            Err(error) => errors.push(serde_json::json!({
                "key": key,
                "error": error,
            })),
        }
    }
    overlays.sort_by(|left, right| left.id.cmp(&right.id));
    (overlays, errors)
}

fn parse_armory_overlay_json(key: &str, raw: &str) -> Result<ArmoryDeploymentOverlay, String> {
    let overlay = serde_json::from_str::<ArmoryDeploymentOverlay>(raw)
        .map_err(|error| format!("invalid overlay entry '{key}': {error}"))?;
    overlay
        .validate()
        .map_err(|error| format!("invalid overlay entry '{key}': {error}"))?;
    armory_overlay_data_key(&overlay.id)?;
    Ok(overlay)
}

fn upsert_armory_overlay_data(
    data: &mut BTreeMap<String, String>,
    overlay: &ArmoryDeploymentOverlay,
) -> Result<(), String> {
    let key = armory_overlay_data_key(&overlay.id)?;
    let raw = serde_json::to_string_pretty(overlay)
        .map_err(|error| format!("failed to encode Armory overlay: {error}"))?;
    data.insert(key, raw);
    Ok(())
}

fn armory_overlay_data_key(id: &str) -> Result<String, String> {
    let id = id.trim();
    if id.is_empty() {
        return Err("overlay id is required".into());
    }
    if id.len() > 128 {
        return Err("overlay id must be 128 characters or fewer".into());
    }
    let valid = id
        .bytes()
        .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-'));
    if !valid {
        return Err("overlay id may only contain ASCII letters, numbers, '.', '_', and '-'".into());
    }
    Ok(format!("{id}.json"))
}

async fn package_detail_handler(AxumPath(id): AxumPath<String>) -> Json<Value> {
    match find_builtin_agent_package(&id) {
        Some(package) => Json(serde_json::json!({
            "package": package,
            "source": "builtin",
        })),
        None => Json(serde_json::json!({ "error": format!("unknown package '{id}'") })),
    }
}

async fn deploy_package_handler(
    ctx: Arc<Context>,
    AxumPath(id): AxumPath<String>,
    body: Option<Json<Value>>,
) -> Json<Value> {
    let Some(package) = find_builtin_agent_package(&id) else {
        return Json(serde_json::json!({ "error": format!("unknown package '{id}'") }));
    };

    let request = match body {
        Some(Json(value)) => match serde_json::from_value::<AgentPackageDeployRequest>(value) {
            Ok(request) => request,
            Err(error) => {
                return Json(serde_json::json!({
                    "error": format!("invalid package deploy request: {error}")
                }));
            }
        },
        None => AgentPackageDeployRequest::default(),
    };

    let mut request = request;
    if request.namespace.is_none() {
        request.namespace = ctx
            .watch_namespace
            .clone()
            .or_else(|| Some("default".into()));
    }

    let manifest = package.omegon_agent_manifest(&request);
    deploy_agent_handler(ctx, Json(manifest)).await
}

async fn patch_agent_handler(
    ctx: Arc<Context>,
    AxumPath((ns, name)): AxumPath<(String, String)>,
    Json(patch_body): Json<Value>,
) -> Json<Value> {
    if let Some(error) = namespace_scope_error(&ctx, &ns) {
        return Json(error);
    }
    if let Err(error) = validate_webui_agent_patch(&patch_body) {
        return Json(serde_json::json!({ "error": error }));
    }

    let api: Api<OmegonAgent> = Api::namespaced(ctx.client.clone(), &ns);
    match api
        .patch(&name, &PatchParams::default(), &Patch::Merge(&patch_body))
        .await
    {
        Ok(agent) => Json(serde_json::json!({ "agent": managed_agent_detail(&agent) })),
        Err(error) => Json(serde_json::json!({ "error": error.to_string() })),
    }
}

async fn rotate_control_tls_handler(
    ctx: Arc<Context>,
    AxumPath((ns, name)): AxumPath<(String, String)>,
    body: Option<Json<Value>>,
) -> Json<Value> {
    if let Some(error) = namespace_scope_error(&ctx, &ns) {
        return Json(error);
    }

    let body = body
        .map(|Json(value)| value)
        .unwrap_or_else(|| serde_json::json!({}));
    let leaf_epoch = body
        .get("leafEpoch")
        .or_else(|| body.get("leaf_epoch"))
        .and_then(Value::as_str)
        .map(str::to_string)
        .unwrap_or_else(rotation_epoch);
    let ca_epoch = body
        .get("caEpoch")
        .or_else(|| body.get("ca_epoch"))
        .and_then(Value::as_str)
        .map(str::to_string);
    let profile = body
        .get("profile")
        .and_then(Value::as_str)
        .map(str::to_string);

    let mut tls_patch = serde_json::json!({
        "enabled": true,
        "leafEpoch": leaf_epoch,
    });
    if let Some(ca_epoch) = ca_epoch {
        tls_patch["caEpoch"] = serde_json::json!(ca_epoch);
    }
    if let Some(profile) = profile {
        tls_patch["profile"] = serde_json::json!(profile);
    }

    let patch_body = serde_json::json!({
        "spec": {
            "controlPlane": {
                "tls": tls_patch
            }
        }
    });

    let api: Api<OmegonAgent> = Api::namespaced(ctx.client.clone(), &ns);
    match api
        .patch(&name, &PatchParams::default(), &Patch::Merge(&patch_body))
        .await
    {
        Ok(agent) => Json(serde_json::json!({
            "agent": managed_agent_detail(&agent),
            "rotation": reconciler::resolved_control_tls(&agent, &name).map(|tls| serde_json::json!({
                "profile": tls.profile,
                "ca_epoch": tls.ca_epoch,
                "leaf_epoch": tls.leaf_epoch,
                "secret": tls.secret_name,
            })),
        })),
        Err(error) => Json(serde_json::json!({ "error": error.to_string() })),
    }
}

async fn audit_handler(ctx: Arc<Context>) -> Json<Value> {
    let api: Api<Event> = match &ctx.watch_namespace {
        Some(ns) => Api::namespaced(ctx.client.clone(), ns),
        None => Api::all(ctx.client.clone()),
    };
    let params = ListParams::default().limit(100);
    match api.list(&params).await {
        Ok(events) => {
            let entries: Vec<_> = events
                .items
                .iter()
                .filter(|event| {
                    event.involved_object.kind.as_deref() == Some("OmegonAgent")
                        || event
                            .metadata
                            .labels
                            .as_ref()
                            .is_some_and(|labels| labels.contains_key("styrene.sh/agent"))
                })
                .map(|event| {
                    serde_json::json!({
                        "namespace": event.namespace(),
                        "name": event.name_any(),
                        "type": event.type_,
                        "reason": event.reason,
                        "message": event.message,
                        "count": event.count,
                        "first_timestamp": event.first_timestamp.as_ref().map(|t| t.0.to_rfc3339()),
                        "last_timestamp": event.last_timestamp.as_ref().map(|t| t.0.to_rfc3339()),
                        "involved_object": {
                            "kind": event.involved_object.kind,
                            "namespace": event.involved_object.namespace,
                            "name": event.involved_object.name,
                        },
                    })
                })
                .collect();
            Json(serde_json::json!({ "entries": entries }))
        }
        Err(error) => Json(serde_json::json!({ "error": error.to_string() })),
    }
}

async fn secret_grants_handler(ctx: Arc<Context>) -> Json<Value> {
    let api: Api<Secret> = match &ctx.watch_namespace {
        Some(ns) => Api::namespaced(ctx.client.clone(), ns),
        None => Api::all(ctx.client.clone()),
    };
    let params = ListParams::default().limit(250);
    match api.list(&params).await {
        Ok(secrets) => {
            let grants: Vec<_> = secrets
                .items
                .iter()
                .filter_map(secret_grant_projection)
                .collect();
            Json(serde_json::json!({ "grants": grants }))
        }
        Err(error) => Json(serde_json::json!({ "error": error.to_string() })),
    }
}

fn is_primary_agent(agent: &OmegonAgent) -> bool {
    agent.spec.role == "primary-driver"
        || agent
            .labels()
            .get("styrene.sh/agent-role")
            .map(String::as_str)
            == Some("primary-driver")
}

fn managed_agent_detail(agent: &OmegonAgent) -> Value {
    serde_json::json!({
        "name": agent.name_any(),
        "namespace": agent.namespace().unwrap_or_else(|| "default".into()),
        "metadata": {
            "labels": agent.labels(),
            "annotations": agent.annotations(),
        },
        "spec": agent.spec,
        "status": agent.status,
        "is_primary": is_primary_agent(agent),
        "control_plane": managed_agent_control_plane(agent),
    })
}

async fn managed_agent_lifecycle(client: &Client, agent: &OmegonAgent) -> Value {
    let name = agent.name_any();
    let namespace = agent.namespace().unwrap_or_else(|| "default".into());
    let phase = agent
        .status
        .as_ref()
        .map(|status| status.phase.as_str())
        .filter(|phase| !phase.is_empty())
        .unwrap_or("Pending");
    let status_message = agent
        .status
        .as_ref()
        .and_then(|status| status.message.as_deref())
        .filter(|message| !message.is_empty());
    let observed_generation = agent
        .status
        .as_ref()
        .and_then(|status| status.observed_generation);
    let cr_observed = observed_generation
        .zip(agent.metadata.generation)
        .is_some_and(|(observed, generation)| observed >= generation);
    let control_plane = managed_agent_control_plane(agent);
    let acp_proxy_ready = control_plane
        .as_ref()
        .and_then(|control| control.get("acp_proxy_url"))
        .and_then(|value| value.as_str())
        .is_some_and(|url| !url.is_empty());

    let mut workload_created = false;
    let mut ready_replicas = 0;
    let mut available_replicas = 0;
    let mut workload_message = None::<String>;
    if agent.spec.mode == AgentMode::Daemon {
        let deployments: Api<Deployment> = Api::namespaced(client.clone(), &namespace);
        match deployments.get(&name).await {
            Ok(deployment) => {
                workload_created = true;
                if let Some(status) = deployment.status.as_ref() {
                    ready_replicas = status.ready_replicas.unwrap_or_default();
                    available_replicas = status.available_replicas.unwrap_or_default();
                    workload_message = deployment_condition_message(status);
                }
            }
            Err(error) => {
                workload_message = Some(error.to_string());
            }
        }
    } else {
        workload_created = !phase.eq_ignore_ascii_case("Pending");
    }

    let pod_ready = if agent.spec.mode == AgentMode::Daemon {
        ready_replicas > 0 || available_replicas > 0
    } else {
        phase.eq_ignore_ascii_case("Succeeded") || phase.eq_ignore_ascii_case("Running")
    };
    let failed = phase.eq_ignore_ascii_case("Failed");
    let summary = status_message
        .map(str::to_string)
        .or(workload_message)
        .unwrap_or_else(|| {
            if failed {
                "Agent reconciliation failed.".into()
            } else if pod_ready && acp_proxy_ready {
                "Agent workload is ready and control-plane metadata is published.".into()
            } else if workload_created {
                "Agent workload exists; waiting for runtime readiness.".into()
            } else {
                "OmegonAgent accepted; waiting for workload reconciliation.".into()
            }
        });

    serde_json::json!({
        "phase": phase,
        "summary": summary,
        "ready_replicas": ready_replicas,
        "available_replicas": available_replicas,
        "steps": [
            {
                "key": "cr",
                "label": "CR accepted",
                "state": if failed { "failed" } else { "ok" },
                "detail": if cr_observed { "Observed by operator" } else { "Stored in API server" }
            },
            {
                "key": "workload",
                "label": "Workload created",
                "state": lifecycle_step_state(workload_created, failed),
                "detail": if workload_created { "Deployment/CronJob materialized" } else { "Waiting for reconcile" }
            },
            {
                "key": "pod",
                "label": "Pod ready",
                "state": lifecycle_step_state(pod_ready, failed),
                "detail": format!("{ready_replicas} ready / {available_replicas} available")
            },
            {
                "key": "control-plane",
                "label": "ACP reachable",
                "state": lifecycle_step_state(acp_proxy_ready, failed),
                "detail": if acp_proxy_ready { "Operator ACP proxy published" } else { "Waiting for daemon control plane" }
            }
        ]
    })
}

fn lifecycle_step_state(ready: bool, failed: bool) -> &'static str {
    if ready {
        "ok"
    } else if failed {
        "failed"
    } else {
        "pending"
    }
}

fn deployment_condition_message(
    status: &k8s_openapi::api::apps::v1::DeploymentStatus,
) -> Option<String> {
    status.conditions.as_ref().and_then(|conditions| {
        conditions
            .iter()
            .find(|condition| {
                matches!(
                    condition.type_.as_str(),
                    "Progressing" | "Available" | "ReplicaFailure"
                ) && condition.status != "True"
            })
            .and_then(|condition| {
                condition
                    .message
                    .clone()
                    .or_else(|| condition.reason.clone())
            })
    })
}

fn managed_agent_control_plane(agent: &OmegonAgent) -> Option<Value> {
    if agent.spec.mode != AgentMode::Daemon {
        return None;
    }

    let name = agent.name_any();
    let namespace = agent.namespace().unwrap_or_else(|| "default".into());
    let service = format!("{name}.{namespace}.svc");
    let tls = reconciler::resolved_control_tls(agent, &name);
    let http_scheme = if tls.is_some() { "https" } else { "http" };
    let ws_scheme = if tls.is_some() { "wss" } else { "ws" };
    let base_url = format!("{http_scheme}://{service}:7842");
    let auth_mode = if tls.as_ref().is_some_and(|t| t.client_ca_key.is_some()) {
        "mtls"
    } else if tls.is_some() {
        "cluster-internal-tls"
    } else {
        "cluster-internal"
    };

    Some(serde_json::json!({
        "schema_version": 2,
        "service": service,
        "http_base": base_url,
        "base_url": base_url,
        "startup_url": format!("{http_scheme}://{service}:7842/api/startup"),
        "state_url": format!("{http_scheme}://{service}:7842/api/state"),
        "health_url": format!("{http_scheme}://{service}:7842/api/healthz"),
        "ready_url": format!("{http_scheme}://{service}:7842/api/readyz"),
        "ws_url": format!("{ws_scheme}://{service}:7842/ws"),
        "acp_url": format!("{ws_scheme}://{service}:7842/acp"),
        "acp_proxy_url": format!("/api/agents/{namespace}/{name}/acp"),
        "auth_mode": auth_mode,
        "transport_security": if tls.is_some() { "tls" } else { "plaintext" },
        "mtls": tls.as_ref().is_some_and(|t| t.client_ca_key.is_some()),
        "tls_secret": tls.as_ref().map(|t| t.secret_name.as_str()),
        "tls_profile": tls.as_ref().map(|t| t.profile.as_str()),
        "tls_ca_epoch": tls.as_ref().map(|t| t.ca_epoch.as_str()),
        "tls_leaf_epoch": tls.as_ref().map(|t| t.leaf_epoch.as_str()),
        "tls_leaf_validity": tls.as_ref().map(|t| format!(
            "{}-{}",
            t.validity.leaf_not_before_year,
            t.validity.leaf_not_after_year
        )),
    }))
}

fn managed_agent_acp_url(agent: &OmegonAgent) -> Option<String> {
    if agent.spec.mode != AgentMode::Daemon {
        return None;
    }

    let name = agent.name_any();
    let namespace = agent.namespace().unwrap_or_else(|| "default".into());
    let service = format!("{name}.{namespace}.svc");
    let ws_scheme = if reconciler::resolved_control_tls(agent, &name).is_some() {
        "wss"
    } else {
        "ws"
    };
    Some(format!("{ws_scheme}://{service}:7842/acp"))
}

fn namespace_scope_error(ctx: &Context, ns: &str) -> Option<Value> {
    ctx.watch_namespace.as_ref().and_then(|allowed| {
        (ns != allowed).then(|| {
            serde_json::json!({
                "error": format!("namespace '{ns}' is outside operator scope '{allowed}'")
            })
        })
    })
}

fn rotation_epoch() -> String {
    let seconds = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or(0);
    format!("manual-{seconds}")
}

fn secret_grant_projection(secret: &Secret) -> Option<Value> {
    let labels = secret.metadata.labels.as_ref()?;
    let has_grant_label = labels.contains_key("styrene.sh/secret-grant")
        || labels.contains_key("styrene.sh/identity")
        || labels.contains_key("styrene.sh/control-plane-tls");
    if !has_grant_label {
        return None;
    }

    let data_keys: Vec<_> = secret
        .data
        .as_ref()
        .map(|data| data.keys().cloned().collect())
        .unwrap_or_default();

    Some(serde_json::json!({
        "name": secret.name_any(),
        "namespace": secret.namespace(),
        "type": secret.type_,
        "labels": labels,
        "annotations": secret.annotations(),
        "data_keys": data_keys,
        "redacted": true,
    }))
}

fn validate_webui_agent_patch(patch_body: &Value) -> Result<(), &'static str> {
    let object = patch_body
        .as_object()
        .ok_or("agent patch must be a JSON object")?;
    for key in object.keys() {
        match key.as_str() {
            "spec" => {}
            "metadata" => validate_patch_metadata(object.get(key).expect("key exists"))?,
            _ => return Err("agent patch may only update spec or metadata labels/annotations"),
        }
    }
    Ok(())
}

fn validate_patch_metadata(metadata: &Value) -> Result<(), &'static str> {
    let object = metadata
        .as_object()
        .ok_or("metadata patch must be a JSON object")?;
    for key in object.keys() {
        match key.as_str() {
            "labels" | "annotations" => {}
            _ => return Err("metadata patch may only update labels or annotations"),
        }
    }
    Ok(())
}

/// Return SBOM status and artifact pointer for a specific agent.
async fn sbom_handler(
    ctx: Arc<Context>,
    AxumPath((ns, name)): AxumPath<(String, String)>,
) -> Json<serde_json::Value> {
    // Enforce namespace scoping: reject cross-namespace reads.
    if let Some(ref allowed) = ctx.watch_namespace
        && ns != *allowed
    {
        return Json(serde_json::json!({
            "error": format!("namespace '{ns}' is outside operator scope '{allowed}'")
        }));
    }

    let api: Api<OmegonAgent> = Api::namespaced(ctx.client.clone(), &ns);
    let agent = match api.get(&name).await {
        Ok(a) => a,
        Err(e) => {
            return Json(serde_json::json!({ "error": e.to_string() }));
        }
    };

    let sbom_status = agent.status.as_ref().and_then(|s| s.sbom.as_ref());

    Json(serde_json::json!({
        "name": name,
        "namespace": ns,
        "image": agent.spec.image,
        "profile": agent.spec.profile,
        "sbom": sbom_status.map(|sb| {
            serde_json::json!({
                "available": sb.available,
                "format": sb.format,
                "artifact_ref": sb.artifact_ref,
                "image_digest": sb.image_digest,
                "generated_at": sb.generated_at,
                "component_count": sb.component_count,
                "vulnerability_count": sb.vulnerability_count,
                "signature_verified": sb.signature_verified,
            })
        }),
        "sbom_spec": agent.spec.sbom.as_ref().map(|s| {
            serde_json::json!({
                "enabled": s.enabled,
                "format": s.format,
                "vulnerability_scan": s.vulnerability_scan,
            })
        }),
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn primary_agent_manifest_marks_dedicated_daemon_driver() {
        let manifest = primary_agent_manifest(&PrimaryAgentBootstrapConfig {
            namespace: "omegon-agents".into(),
            name: "auspex-primary".into(),
            image: "example.com/omegon:dev".into(),
            model: "anthropic:claude-sonnet-4-6".into(),
            secret_name: Some("auspex-primary-secrets".into()),
            auth_json_secret: Some("auspex-primary-auth-json".into()),
            control_tls_secret: None,
        });

        assert_eq!(manifest["metadata"]["name"], "auspex-primary");
        assert_eq!(
            manifest["metadata"]["labels"]["styrene.sh/agent-role"],
            "primary-driver"
        );
        assert_eq!(manifest["spec"]["agent"], "styrene.auspex-primary");
        assert_eq!(manifest["spec"]["role"], "primary-driver");
        assert_eq!(manifest["spec"]["mode"], "daemon");
        assert_eq!(manifest["spec"]["terminalTool"], false);
        assert_eq!(
            manifest["spec"]["secrets"]["secretName"],
            "auspex-primary-secrets"
        );
        assert_eq!(
            manifest["spec"]["secrets"]["authJsonSecret"],
            "auspex-primary-auth-json"
        );
    }

    #[test]
    fn api_auth_accepts_header_and_websocket_query_token_only() {
        let expected = "Bearer secret-token";
        let header_req = axum::http::Request::builder()
            .uri("/api/fleet")
            .header("authorization", expected)
            .body(axum::body::Body::empty())
            .expect("request");
        assert!(request_has_valid_api_auth(&header_req, expected));

        let ws_req = axum::http::Request::builder()
            .uri("/api/agents/ops/primary/acp?access_token=secret-token")
            .header(axum::http::header::UPGRADE, "websocket")
            .body(axum::body::Body::empty())
            .expect("request");
        assert!(request_has_valid_api_auth(&ws_req, expected));

        let plain_query_req = axum::http::Request::builder()
            .uri("/api/fleet?access_token=secret-token")
            .body(axum::body::Body::empty())
            .expect("request");
        assert!(!request_has_valid_api_auth(&plain_query_req, expected));
    }

    #[test]
    fn primary_agent_manifest_enables_identity_when_control_tls_is_requested() {
        let manifest = primary_agent_manifest(&PrimaryAgentBootstrapConfig {
            namespace: "omegon-agents".into(),
            name: "auspex-primary".into(),
            image: "example.com/omegon:dev".into(),
            model: "anthropic:claude-sonnet-4-6".into(),
            secret_name: None,
            auth_json_secret: None,
            control_tls_secret: Some("auspex-primary-control-tls".into()),
        });

        assert_eq!(manifest["spec"]["identity"]["provision"], true);
        assert_eq!(manifest["spec"]["identity"]["mtls"], true);
        assert_eq!(
            manifest["spec"]["controlPlane"]["tls"]["secretName"],
            "auspex-primary-control-tls"
        );
    }

    #[test]
    fn daemon_agents_publish_cluster_control_plane_urls() {
        let agent: OmegonAgent = serde_json::from_value(serde_json::json!({
            "apiVersion": "styrene.sh/v1alpha1",
            "kind": "OmegonAgent",
            "metadata": {
                "name": "auspex-primary",
                "namespace": "omegon-agents",
                "labels": {
                    "styrene.sh/agent-role": "primary-driver"
                }
            },
            "spec": {
                "agent": "styrene.auspex-primary",
                "model": "anthropic:claude-sonnet-4-6",
                "role": "primary-driver",
                "mode": "daemon"
            }
        }))
        .expect("valid OmegonAgent");

        let control_plane = managed_agent_control_plane(&agent).expect("daemon control plane");

        assert!(is_primary_agent(&agent));
        assert_eq!(control_plane["schema_version"], 2);
        assert_eq!(
            control_plane["base_url"],
            "http://auspex-primary.omegon-agents.svc:7842"
        );
        assert_eq!(
            control_plane["acp_url"],
            "ws://auspex-primary.omegon-agents.svc:7842/acp"
        );
        assert_eq!(
            control_plane["acp_proxy_url"],
            "/api/agents/omegon-agents/auspex-primary/acp"
        );
        assert_eq!(control_plane["transport_security"], "plaintext");
        assert_eq!(control_plane["mtls"], false);
    }

    #[test]
    fn daemon_agents_publish_wss_control_plane_when_tls_enabled() {
        let agent: OmegonAgent = serde_json::from_value(serde_json::json!({
            "apiVersion": "styrene.sh/v1alpha1",
            "kind": "OmegonAgent",
            "metadata": {
                "name": "secure-primary",
                "namespace": "omegon-agents"
            },
            "spec": {
                "agent": "styrene.secure-primary",
                "model": "anthropic:claude-sonnet-4-6",
                "role": "primary-driver",
                "mode": "daemon",
                "controlPlane": {
                    "tls": {
                        "enabled": true,
                        "secretName": "secure-primary-control-tls"
                    }
                }
            }
        }))
        .expect("valid OmegonAgent");

        let control_plane = managed_agent_control_plane(&agent).expect("daemon control plane");

        assert_eq!(
            control_plane["base_url"],
            "https://secure-primary.omegon-agents.svc:7842"
        );
        assert_eq!(
            control_plane["acp_url"],
            "wss://secure-primary.omegon-agents.svc:7842/acp"
        );
        assert_eq!(
            control_plane["acp_proxy_url"],
            "/api/agents/omegon-agents/secure-primary/acp"
        );
        assert_eq!(control_plane["auth_mode"], "mtls");
        assert_eq!(control_plane["transport_security"], "tls");
        assert_eq!(control_plane["mtls"], true);
        assert_eq!(control_plane["tls_secret"], "secure-primary-control-tls");
        assert_eq!(control_plane["tls_profile"], "default");
        assert_eq!(control_plane["tls_ca_epoch"], "0");
        assert_eq!(control_plane["tls_leaf_epoch"], "0");
        assert_eq!(control_plane["tls_leaf_validity"], "2026-2031");
    }

    #[test]
    fn managed_agent_detail_includes_webui_control_metadata() {
        let agent: OmegonAgent = serde_json::from_value(serde_json::json!({
            "apiVersion": "styrene.sh/v1alpha1",
            "kind": "OmegonAgent",
            "metadata": {
                "name": "primary",
                "namespace": "ops"
            },
            "spec": {
                "agent": "styrene.primary",
                "model": "anthropic:claude-sonnet-4-6",
                "role": "primary-driver",
                "mode": "daemon"
            }
        }))
        .expect("valid OmegonAgent");

        let detail = managed_agent_detail(&agent);

        assert_eq!(detail["name"], "primary");
        assert_eq!(detail["namespace"], "ops");
        assert_eq!(detail["is_primary"], true);
        assert_eq!(
            detail["control_plane"]["acp_url"],
            "ws://primary.ops.svc:7842/acp"
        );
        assert_eq!(
            detail["control_plane"]["acp_proxy_url"],
            "/api/agents/ops/primary/acp"
        );
    }

    #[test]
    fn secret_grant_projection_redacts_secret_values() {
        use k8s_openapi::ByteString;
        use k8s_openapi::apimachinery::pkg::apis::meta::v1::ObjectMeta;
        use std::collections::BTreeMap;

        let secret = Secret {
            metadata: ObjectMeta {
                name: Some("primary-control-tls".into()),
                namespace: Some("ops".into()),
                labels: Some(BTreeMap::from([(
                    "styrene.sh/control-plane-tls".into(),
                    "true".into(),
                )])),
                ..Default::default()
            },
            type_: Some("kubernetes.io/tls".into()),
            data: Some(BTreeMap::from([(
                "tls.key".into(),
                ByteString(b"do-not-return".to_vec()),
            )])),
            ..Default::default()
        };

        let projection = secret_grant_projection(&secret).expect("projected");

        assert_eq!(projection["name"], "primary-control-tls");
        assert_eq!(projection["redacted"], true);
        assert_eq!(projection["data_keys"], serde_json::json!(["tls.key"]));
        assert!(!projection.to_string().contains("do-not-return"));
    }

    #[test]
    fn webui_agent_patch_rejects_status_and_arbitrary_metadata() {
        assert!(
            validate_webui_agent_patch(&serde_json::json!({
                "spec": {
                    "controlPlane": {
                        "tls": {
                            "enabled": true
                        }
                    }
                },
                "metadata": {
                    "annotations": {
                        "styrene.sh/requested-by": "webui"
                    }
                }
            }))
            .is_ok()
        );

        assert!(
            validate_webui_agent_patch(&serde_json::json!({
                "status": {
                    "phase": "Ready"
                }
            }))
            .is_err()
        );

        assert!(
            validate_webui_agent_patch(&serde_json::json!({
                "metadata": {
                    "ownerReferences": []
                }
            }))
            .is_err()
        );
    }

    #[test]
    fn armory_overlay_config_data_is_validated_and_resilient() {
        let overlay = ArmoryDeploymentOverlay {
            id: "security-review".into(),
            armory: "profile/security-review".into(),
            mode: "daemon".into(),
            role: "security-reviewer".into(),
            image: "ghcr.io/styrene-lab/omegon-agents:latest".into(),
            model: "anthropic:claude-sonnet-4-6".into(),
            ..Default::default()
        };
        let mut data = BTreeMap::from([
            ("notes.txt".into(), "ignored".into()),
            ("bad.json".into(), "{".into()),
        ]);

        upsert_armory_overlay_data(&mut data, &overlay).expect("stored overlay");
        let (overlays, errors) = parse_armory_overlay_config_data(&data);

        assert_eq!(
            armory_overlay_data_key("security-review").unwrap(),
            "security-review.json"
        );
        assert_eq!(overlays, vec![overlay]);
        assert_eq!(errors.len(), 1);
        assert_eq!(errors[0]["key"], "bad.json");
    }

    #[test]
    fn armory_overlay_ids_reject_path_like_names() {
        assert!(armory_overlay_data_key("home-media").is_ok());
        assert!(armory_overlay_data_key("profile/security-review").is_err());
        assert!(armory_overlay_data_key("../escape").is_err());
        assert!(armory_overlay_data_key("").is_err());
    }

    #[test]
    fn strict_oci_preflight_policy_blocks_mutable_images() {
        let policy = oci_preflight_policy_from_body(&serde_json::json!({
            "ociPolicy": "strict"
        }));
        let tagged = assess_oci_image_ref("ghcr.io/styrene-lab/omegon-agents:latest");
        let digest = assess_oci_image_ref(
            "ghcr.io/styrene-lab/omegon-agents@sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
        );

        assert_eq!(policy.as_str(), "strict");
        assert!(policy.blocks(&tagged));
        assert!(!policy.blocks(&digest));
    }

    #[test]
    fn warn_oci_preflight_policy_reports_but_does_not_block() {
        let policy = oci_preflight_policy_from_body(&serde_json::json!({}));
        let tagged = assess_oci_image_ref("ghcr.io/styrene-lab/omegon-agents:latest");

        assert_eq!(policy.as_str(), "warn");
        assert!(!policy.blocks(&tagged));
    }
}
