//! Reconciliation and health probing for ExternalAgent CRDs.
//!
//! Unlike OmegonAgent (where the operator creates workloads), ExternalAgent
//! only monitors: it probes health, fetches startup info, and updates status.
//! The operator proxies WebSocket connections to external agents so the
//! Auspex UI can steer them through a single control plane.

use std::sync::Arc;

use kube::{Api, ResourceExt, api::PatchParams};
use serde_json::json;
use tracing::{debug, info, warn};

use crate::crd::ExternalAgent;
use crate::reconciler::Context;

/// Reconcile an ExternalAgent: probe its health and update status.
pub async fn reconcile(
    agent: Arc<ExternalAgent>,
    ctx: Arc<Context>,
) -> Result<kube::runtime::controller::Action, kube::Error> {
    let name = agent.name_any();
    let ns = agent.namespace().unwrap_or_else(|| "default".into());
    let endpoint = &agent.spec.endpoint;

    info!(agent = %name, endpoint = %endpoint, "probing external agent");

    let probe = probe_external_agent(endpoint).await;

    // Build status patch.
    let status = match probe {
        Ok(info) => {
            info!(agent = %name, version = %info.omegon_version, "external agent online");
            json!({
                "status": {
                    "reachability": "Online",
                    "omegon_version": info.omegon_version,
                    "agent_id": info.agent_id,
                    "model": info.model,
                    "ws_url": info.ws_url,
                    "last_seen": timestamp_now(),
                    "last_error": null,
                }
            })
        }
        Err(err) => {
            warn!(agent = %name, error = %err, "external agent unreachable");
            // Preserve existing status fields, only update reachability and error.
            json!({
                "status": {
                    "reachability": "Unreachable",
                    "last_error": err,
                }
            })
        }
    };

    // Patch status subresource.
    let api: Api<ExternalAgent> = Api::namespaced(ctx.client.clone(), &ns);
    if let Err(e) = api
        .patch_status(
            &name,
            &PatchParams::apply("auspex-operator"),
            &kube::api::Patch::Merge(status),
        )
        .await
    {
        debug!(agent = %name, error = %e, "status patch failed (may be first reconcile)");
    }

    // Floor at 10 seconds to prevent tight reconciliation loops.
    let interval = (agent.spec.probe_interval_seconds as u64).max(10);
    Ok(kube::runtime::controller::Action::requeue(
        std::time::Duration::from_secs(interval),
    ))
}

/// Handle reconciliation errors for ExternalAgent.
pub fn error_policy(
    agent: Arc<ExternalAgent>,
    error: &kube::Error,
    _ctx: Arc<Context>,
) -> kube::runtime::controller::Action {
    warn!(
        agent = %agent.name_any(),
        error = %error,
        "external agent reconcile error — retrying in 60s"
    );
    kube::runtime::controller::Action::requeue(std::time::Duration::from_secs(60))
}

/// Info returned from probing an external agent.
struct ProbeInfo {
    omegon_version: String,
    agent_id: Option<String>,
    model: Option<String>,
    ws_url: Option<String>,
}

/// Probe an external agent's startup and health endpoints.
async fn probe_external_agent(endpoint: &str) -> Result<ProbeInfo, String> {
    // Validate the endpoint to prevent SSRF against cluster-internal services.
    validate_endpoint(endpoint)?;

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(5))
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .map_err(|e| format!("HTTP client error: {e}"))?;

    // 1. Check health.
    let health_url = format!("{endpoint}/api/readyz");
    let health_resp = client
        .get(&health_url)
        .send()
        .await
        .map_err(|e| format!("health probe failed: {e}"))?;

    if !health_resp.status().is_success() {
        return Err(format!("health probe returned {}", health_resp.status()));
    }

    // 2. Fetch startup info for version, agent, and WebSocket URL.
    let startup_url = format!("{endpoint}/api/startup");
    let startup_resp = client
        .get(&startup_url)
        .send()
        .await
        .map_err(|e| format!("startup probe failed: {e}"))?;

    if !startup_resp.status().is_success() {
        // Agent is reachable but startup endpoint may not be available.
        return Ok(ProbeInfo {
            omegon_version: "unknown".into(),
            agent_id: None,
            model: None,
            ws_url: None,
        });
    }

    let body: serde_json::Value = startup_resp
        .json()
        .await
        .map_err(|e| format!("startup parse failed: {e}"))?;

    let omegon_version = body
        .pointer("/instance_descriptor/identity/omegon_version")
        .or_else(|| body.get("omegon_version"))
        .and_then(|v| v.as_str())
        .unwrap_or("unknown")
        .to_string();

    let agent_id = body
        .pointer("/instance_descriptor/identity/agent_id")
        .or_else(|| body.get("agent_id"))
        .and_then(|v| v.as_str())
        .map(String::from);

    let model = body
        .pointer("/instance_descriptor/desired/policy/model")
        .or_else(|| body.get("model"))
        .and_then(|v| v.as_str())
        .map(String::from);

    // Derive WebSocket URL from the endpoint using proper URL parsing.
    let ws_url = url::Url::parse(endpoint).ok().map(|mut u| {
        let _ = u.set_scheme(if u.scheme() == "https" { "wss" } else { "ws" });
        u.set_path("/ws");
        u.to_string()
    });

    Ok(ProbeInfo {
        omegon_version,
        agent_id,
        model,
        ws_url,
    })
}

/// Validate that an endpoint is not pointing at cluster-internal or metadata services.
fn validate_endpoint(endpoint: &str) -> Result<(), String> {
    let url = url::Url::parse(endpoint).map_err(|e| format!("invalid endpoint URL: {e}"))?;

    // Must be http or https.
    match url.scheme() {
        "http" | "https" => {}
        other => return Err(format!("unsupported scheme: {other}")),
    }

    let host = url.host_str().unwrap_or("");

    // Block k8s internal service DNS.
    if host.ends_with(".svc")
        || host.ends_with(".svc.cluster.local")
        || host == "kubernetes"
        || host == "kubernetes.default"
    {
        return Err(format!(
            "endpoint must not target cluster-internal services: {host}"
        ));
    }

    // Block cloud metadata endpoints.
    let blocked_hosts = [
        "169.254.169.254", // AWS/GCP/Azure metadata
        "metadata.google.internal",
        "100.100.100.200", // Alibaba metadata
    ];
    if blocked_hosts.contains(&host) {
        return Err(format!("endpoint blocked (metadata service): {host}"));
    }

    // Block loopback and link-local addresses.
    if host == "localhost" || host == "0.0.0.0" {
        return Err(format!("endpoint must not target loopback: {host}"));
    }

    // Parse IP addresses for range checks.
    if let Ok(ip) = host.parse::<std::net::IpAddr>() {
        if ip.is_loopback() {
            return Err(format!("endpoint must not target loopback: {host}"));
        }
        // Block link-local (169.254.0.0/16, fe80::/10).
        match ip {
            std::net::IpAddr::V4(v4) => {
                if v4.octets()[0] == 169 && v4.octets()[1] == 254 {
                    return Err(format!("endpoint must not target link-local: {host}"));
                }
            }
            std::net::IpAddr::V6(v6) => {
                if (v6.segments()[0] & 0xffc0) == 0xfe80 {
                    return Err(format!("endpoint must not target link-local: {host}"));
                }
            }
        }
    }

    Ok(())
}

fn timestamp_now() -> String {
    // Unix epoch seconds as string. Status consumers should treat this as
    // an opaque sortable timestamp, not display it directly.
    let d = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default();
    format!("{}", d.as_secs())
}
