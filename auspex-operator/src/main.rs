//! auspex-operator — Kubernetes operator for Omegon agent fleet management.
//!
//! Watches OmegonAgent CRDs and reconciles them into Deployments, CronJobs,
//! ConfigMaps, and Services. Exposes a fleet API for the Auspex UI.

mod crd;
mod external;
mod identity;
mod reconciler;

use std::{net::SocketAddr, sync::Arc};

use axum::{
    Json, Router,
    extract::Path as AxumPath,
    routing::{get, post},
};
use futures_util::StreamExt;
use k8s_openapi::api::{
    apps::v1::Deployment,
    batch::v1::{CronJob, Job},
    core::v1::{Event, Secret},
};
use kube::{
    Api, Client, CustomResourceExt, ResourceExt,
    api::{ListParams, Patch, PatchParams},
    runtime::Controller,
};
use serde_json::Value;
use styrene_mqtt::{EmbeddedBrokerBuilder, EmbeddedBrokerConfig, broker::TcpListenerConfig};
use tower_http::services::ServeDir;
use tracing::{error, info, warn};

use crd::{AgentMode, ExternalAgent, OmegonAgent};
use reconciler::Context;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "info,kube=warn".into()),
        )
        .init();

    info!("auspex-operator starting");

    let client = Client::try_default().await?;

    // Print CRDs for installation: auspex-operator --crd
    if std::env::args().any(|a| a == "--crd") {
        let managed = OmegonAgent::crd();
        let external = ExternalAgent::crd();
        println!(
            "{}",
            serde_json::to_string_pretty(&serde_json::json!([managed, external]))?
        );
        return Ok(());
    }

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
                        let auth_header = req
                            .headers()
                            .get("authorization")
                            .and_then(|v| v.to_str().ok())
                            .unwrap_or("");
                        // Constant-time comparison to prevent timing attacks.
                        // Always iterate the full expected length regardless of
                        // header length, comparing against zero-padding for short
                        // inputs so the loop duration doesn't leak length info.
                        let header_bytes = auth_header.as_bytes();
                        let expected_bytes = expected.as_bytes();
                        let mut diff = (header_bytes.len() ^ expected_bytes.len()) as u8;
                        for i in 0..expected_bytes.len() {
                            let h = header_bytes.get(i).copied().unwrap_or(0xff);
                            diff |= h ^ expected_bytes[i];
                        }
                        if diff == 0 {
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
            control_tls_secret: std::env::var("AUSPEX_PRIMARY_AGENT_CONTROL_TLS_SECRET").ok(),
        }
    }
}

fn primary_agent_manifest(config: &PrimaryAgentBootstrapConfig) -> Value {
    let mut secrets = serde_json::json!({});
    if let Some(secret_name) = config.secret_name.as_ref() {
        secrets["secretName"] = serde_json::json!(secret_name);
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
            "secrets": secrets,
            "resources": {
                "cpu": "1",
                "memory": "2Gi",
            },
        },
    });

    if let Some(secret) = config.control_tls_secret.as_ref() {
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

    let mut fleet: Vec<serde_json::Value> = agents
        .items
        .iter()
        .map(|a| {
            let name = a.name_any();
            let namespace = a.namespace().unwrap_or_else(|| "default".into());
            serde_json::json!({
                "name": name,
                "namespace": namespace,
                "agent": a.spec.agent,
                "model": a.spec.model,
                "posture": a.spec.posture,
                "role": a.spec.role,
                "mode": a.spec.mode,
                "image": a.spec.image,
                "profile": a.spec.profile,
                "is_primary": is_primary_agent(a),
                "control_plane": managed_agent_control_plane(a),
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
            })
        })
        .collect();

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
    if let Some(ref allowed) = ctx.watch_namespace {
        if ns != *allowed {
            return Json(serde_json::json!({
                "error": format!("namespace '{ns}' is outside operator scope '{allowed}'")
            }));
        }
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
        assert_eq!(
            manifest["spec"]["secrets"]["secretName"],
            "auspex-primary-secrets"
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
}
