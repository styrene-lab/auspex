//! auspex-operator — Kubernetes operator for Omegon agent fleet management.
//!
//! Watches OmegonAgent CRDs and reconciles them into Deployments, CronJobs,
//! ConfigMaps, and Services. Exposes a fleet API for the Auspex UI.

mod crd;
mod external;
mod identity;
mod mtls;
mod reconciler;

use std::sync::Arc;

use axum::{Json, Router, extract::Path as AxumPath, routing::get};
use tower_http::services::ServeDir;
use k8s_openapi::api::{apps::v1::Deployment, batch::v1::{CronJob, Job}};
use kube::{Api, Client, CustomResourceExt, runtime::Controller};
use futures_util::StreamExt;
use tracing::{info, warn, error};

use crd::{ExternalAgent, OmegonAgent};
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
        println!("{}", serde_json::to_string_pretty(&serde_json::json!([managed, external]))?);
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
        let web_ui_path = std::env::var("AUSPEX_WEB_UI_PATH")
            .unwrap_or_else(|_| "/ui/dist".into());
        let serve_dir = ServeDir::new(&web_ui_path)
            .append_index_html_on_directories(true);

        // Fleet API token: required for /api/* routes.
        // Set via AUSPEX_API_TOKEN env var or k8s Secret mount.
        // When unset, API is open (development mode only).
        let api_token = std::env::var("AUSPEX_API_TOKEN").ok();
        if api_token.is_none() {
            warn!("AUSPEX_API_TOKEN not set — fleet API is unauthenticated");
        }

        let api_routes = Router::new()
            .route("/fleet", get({
                let ctx = api_ctx.clone();
                move || fleet_handler(ctx)
            }))
            .route("/fleet/{ns}/{name}/sbom", get({
                let ctx = api_ctx.clone();
                move |path: AxumPath<(String, String)>| sbom_handler(ctx, path)
            }));

        // Wrap API routes with bearer token validation when configured.
        let api_routes = if let Some(token) = api_token {
            let expected_value = format!("Bearer {token}");
            api_routes.layer(axum::middleware::from_fn(move |req: axum::extract::Request, next: axum::middleware::Next| {
                let expected = expected_value.clone();
                async move {
                    let auth_header = req.headers()
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
            }))
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
            serde_json::json!({
                "name": a.metadata.name,
                "namespace": a.metadata.namespace,
                "agent": a.spec.agent,
                "model": a.spec.model,
                "mode": a.spec.mode,
                "image": a.spec.image,
                "profile": a.spec.profile,
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

    let sbom_status = agent
        .status
        .as_ref()
        .and_then(|s| s.sbom.as_ref());

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
