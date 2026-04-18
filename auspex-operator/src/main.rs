//! auspex-operator — Kubernetes operator for Omegon agent fleet management.
//!
//! Watches OmegonAgent CRDs and reconciles them into Deployments, CronJobs,
//! ConfigMaps, and Services. Exposes a fleet API for the Auspex UI.

mod crd;
mod reconciler;

use std::sync::Arc;

use axum::{Json, Router, routing::get};
use k8s_openapi::api::{apps::v1::Deployment, batch::v1::CronJob};
use kube::{Api, Client, CustomResourceExt, runtime::Controller};
use futures_util::StreamExt;
use tracing::{info, error};

use crd::OmegonAgent;
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

    // Print CRD for installation: auspex-operator --crd
    if std::env::args().any(|a| a == "--crd") {
        let crd = OmegonAgent::crd();
        println!("{}", serde_json::to_string_pretty(&crd)?);
        return Ok(());
    }

    let ctx = Arc::new(Context {
        client: client.clone(),
    });

    // Watch OmegonAgent CRDs
    let agents: Api<OmegonAgent> = Api::all(client.clone());
    let deployments: Api<Deployment> = Api::all(client.clone());
    let cronjobs: Api<CronJob> = Api::all(client.clone());

    // Fleet API (health + instance list)
    let api_ctx = ctx.clone();
    let api_server = tokio::spawn(async move {
        let app = Router::new()
            .route("/healthz", get(|| async { "ok" }))
            .route("/api/fleet", get({
                let ctx = api_ctx.clone();
                move || fleet_handler(ctx)
            }));

        let listener = tokio::net::TcpListener::bind("0.0.0.0:8080").await.unwrap();
        info!("fleet API listening on :8080");
        axum::serve(listener, app).await.unwrap();
    });

    // Controller loop
    info!("starting controller");
    Controller::new(agents, kube::runtime::watcher::Config::default())
        .owns(deployments, kube::runtime::watcher::Config::default())
        .owns(cronjobs, kube::runtime::watcher::Config::default())
        .run(reconciler::reconcile, reconciler::error_policy, ctx)
        .for_each(|res| async move {
            match res {
                Ok(o) => info!(agent = ?o, "reconciled"),
                Err(e) => error!(error = %e, "reconcile failed"),
            }
        })
        .await;

    api_server.abort();
    Ok(())
}

async fn fleet_handler(ctx: Arc<Context>) -> Json<serde_json::Value> {
    let api: Api<OmegonAgent> = Api::all(ctx.client.clone());
    let agents = match api.list(&Default::default()).await {
        Ok(list) => list,
        Err(e) => {
            return Json(serde_json::json!({ "error": e.to_string() }));
        }
    };

    let fleet: Vec<serde_json::Value> = agents
        .items
        .iter()
        .map(|a| {
            serde_json::json!({
                "name": a.metadata.name,
                "namespace": a.metadata.namespace,
                "agent": a.spec.agent,
                "model": a.spec.model,
                "mode": a.spec.mode,
                "status": a.status.as_ref().map(|s| &s.phase),
            })
        })
        .collect();

    Json(serde_json::json!({ "agents": fleet }))
}
