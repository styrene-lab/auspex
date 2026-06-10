//! Reconciliation logic for OmegonAgent CRDs.
//!
//! Watches OmegonAgent resources and ensures the corresponding k8s primitives
//! (Deployment/CronJob, ConfigMap, Service) match the desired state.

use std::sync::Arc;

use k8s_openapi::api::{
    apps::v1::Deployment,
    batch::v1::{CronJob, Job},
    core::v1::{ConfigMap, Service},
};
use kube::{
    Api, Client, ResourceExt,
    api::{Patch, PatchParams},
    runtime::controller::Action,
};
use serde_json::json;
use tracing::{info, warn};

use crate::crd::{AgentMode, OmegonAgent};

const CONTROL_TLS_MOUNT_PATH: &str = "/run/omegon/control-tls";
const AUSPEX_PRIMARY_AGENT_ID: &str = "styrene.auspex-primary";

const AUSPEX_PRIMARY_AGENT_TOML: &str = r#"[agent]
id = "styrene.auspex-primary"
name = "Auspex Primary Coordinator"
version = "0.1.0"
description = "Operator-facing coordinator for Auspex-managed agent fleets, workflows, deployments, and policy-gated work routing."
domain = "orchestration"

[persona]
directive = "PERSONA.md"
badge = "coordinator"

[settings]
model = "anthropic:claude-sonnet-4-6"
thinking_level = "medium"
context_class = "squad"
max_turns = 80

[secrets]
required = ["ANTHROPIC_API_KEY"]
optional = ["GITHUB_TOKEN", "OPENAI_API_KEY", "KUBECONFIG"]
"#;

const AUSPEX_PRIMARY_PERSONA_MD: &str = r#"# Auspex Primary Coordinator

You are the operator-facing coordinator for an Auspex-managed agent fleet.

Your job is not to behave like a single coding assistant by default. Your job is to understand operator intent, normalize work into execution lanes, choose whether work should run inline or be delegated, deploy and supervise Omegon agents when policy allows, and publish concise operating state back to Auspex.

Prefer coordination over local execution when work can progress independently. Keep deployment, secret, workflow, and agent lifecycle operations explicit. Treat high-impact actions as policy-gated operations that require clear operator intent or configured approval.
"#;

/// Default sidecar images. Override via AUSPEX_STYRENED_IMAGE / AUSPEX_AETHER_IMAGE env vars.
fn styrened_image() -> String {
    std::env::var("AUSPEX_STYRENED_IMAGE")
        .unwrap_or_else(|_| "ghcr.io/styrene-lab/styrened:0.5".into())
}

fn aether_image() -> String {
    std::env::var("AUSPEX_AETHER_IMAGE").unwrap_or_else(|_| "ghcr.io/styrene-lab/aether:0.3".into())
}

/// Shared state across reconcile calls.
pub struct Context {
    pub client: Client,
    /// When set, restricts all operations to this namespace.
    pub watch_namespace: Option<String>,
}

/// Reconcile a single OmegonAgent resource.
pub async fn reconcile(agent: Arc<OmegonAgent>, ctx: Arc<Context>) -> Result<Action, kube::Error> {
    let client = &ctx.client;
    let ns = agent.namespace().unwrap_or_else(|| "default".into());
    let name = agent.name_any();

    info!(agent = %name, namespace = %ns, mode = ?agent.spec.mode, "reconciling");

    // Provision StyreneID if identity is configured.
    if agent.spec.identity.as_ref().is_some_and(|id| id.provision) {
        match crate::identity::provision_identity(client, &agent, &ns, &name).await {
            Ok(provisioned) => {
                info!(
                    agent = %name,
                    secret = %provisioned.secret_name,
                    mesh_role = %provisioned.mesh_role,
                    rns_dest = %provisioned.rns_destination_hash,
                    wg_pub = %provisioned.wireguard_pubkey,
                    control_tls_secret = provisioned
                        .control_tls
                        .as_ref()
                        .map(|tls| tls.secret_name.as_str())
                        .unwrap_or("disabled"),
                    control_tls_ca_fingerprint = provisioned
                        .control_tls
                        .as_ref()
                        .map(|tls| tls.ca_fingerprint_sha256.as_str())
                        .unwrap_or(""),
                    control_tls_server_fingerprint = provisioned
                        .control_tls
                        .as_ref()
                        .map(|tls| tls.server_fingerprint_sha256.as_str())
                        .unwrap_or(""),
                    "identity provisioned"
                );
            }
            Err(crate::identity::IdentityError::NotConfigured) => {}
            Err(e) => {
                warn!(agent = %name, error = %e, "identity provisioning failed");
            }
        }
    }

    // Ensure ConfigMap for vox.toml
    reconcile_configmap(client, &agent, &ns, &name).await?;

    // Seed the built-in primary coordinator bundle until Omegon images carry it natively.
    if agent.spec.agent == AUSPEX_PRIMARY_AGENT_ID {
        reconcile_primary_catalog_configmap(client, &agent, &ns, &name).await?;
    }

    // Ensure workload
    match agent.spec.mode {
        AgentMode::Daemon => {
            reconcile_deployment(client, &agent, &ns, &name).await?;
            reconcile_service(client, &agent, &ns, &name).await?;
        }
        AgentMode::Cronjob => {
            reconcile_cronjob(client, &agent, &ns, &name).await?;
        }
        AgentMode::Job => {
            reconcile_job(client, &agent, &ns, &name).await?;
        }
    }

    // Ensure prompt ConfigMap for job/cronjob with inline prompt
    if matches!(agent.spec.mode, AgentMode::Job | AgentMode::Cronjob) {
        reconcile_prompt_configmap(client, &agent, &ns, &name).await?;
    }

    info!(agent = %name, "reconciliation complete");
    Ok(Action::requeue(std::time::Duration::from_secs(300)))
}

async fn reconcile_primary_catalog_configmap(
    client: &Client,
    agent: &OmegonAgent,
    ns: &str,
    name: &str,
) -> Result<(), kube::Error> {
    let api: Api<ConfigMap> = Api::namespaced(client.clone(), ns);
    let cm_name = format!("{name}-catalog");

    let cm = json!({
        "apiVersion": "v1",
        "kind": "ConfigMap",
        "metadata": {
            "name": cm_name,
            "namespace": ns,
            "ownerReferences": [owner_ref(agent)],
        },
        "data": {
            "agent.toml": AUSPEX_PRIMARY_AGENT_TOML,
            "PERSONA.md": AUSPEX_PRIMARY_PERSONA_MD,
        }
    });

    api.patch(
        &cm_name,
        &PatchParams::apply("auspex-operator"),
        &Patch::Apply(cm),
    )
    .await?;
    Ok(())
}

/// Handle reconciliation errors.
pub fn error_policy(agent: Arc<OmegonAgent>, error: &kube::Error, _ctx: Arc<Context>) -> Action {
    warn!(
        agent = %agent.name_any(),
        error = %error,
        "reconcile error — retrying in 30s"
    );
    Action::requeue(std::time::Duration::from_secs(30))
}

async fn reconcile_configmap(
    client: &Client,
    agent: &OmegonAgent,
    ns: &str,
    name: &str,
) -> Result<(), kube::Error> {
    let api: Api<ConfigMap> = Api::namespaced(client.clone(), ns);
    let cm_name = format!("{name}-vox");

    let mut vox_toml = String::from("# Generated by auspex-operator\n");

    let has_runtime_secret = agent.spec.secrets.secret_name.is_some();

    if let Some(ref discord) = agent.spec.vox.discord {
        vox_toml.push_str("\n[discord]\n");
        vox_toml.push_str(&format!("require_mention = {}\n", discord.require_mention));
        if has_runtime_secret {
            vox_toml.push_str("bot_token_file = \"/run/omegon/secrets/discord_bot_token\"\n");
        }
        if let Some(ref gid) = discord.guild_id {
            vox_toml.push_str(&format!("guild_id = \"{gid}\"\n"));
        }
    }

    if let Some(ref slack) = agent.spec.vox.slack {
        vox_toml.push_str("\n[slack]\n");
        if has_runtime_secret {
            vox_toml.push_str("oauth_token_file = \"/run/omegon/secrets/slack_oauth_token\"\n");
            vox_toml.push_str("socket_token_file = \"/run/omegon/secrets/slack_socket_token\"\n");
        }
        if let Some(ref ws) = slack.workspace {
            vox_toml.push_str(&format!("workspace = \"{ws}\"\n"));
        }
        if let Some(ref channel) = slack.default_channel {
            vox_toml.push_str(&format!("default_channel = \"{channel}\"\n"));
        }
        vox_toml.push_str(&format!("require_mention = {}\n", slack.require_mention));
    }

    let cm = json!({
        "apiVersion": "v1",
        "kind": "ConfigMap",
        "metadata": {
            "name": cm_name,
            "namespace": ns,
            "ownerReferences": [owner_ref(agent)],
        },
        "data": {
            "vox.toml": vox_toml,
        }
    });

    api.patch(
        &cm_name,
        &PatchParams::apply("auspex-operator"),
        &Patch::Apply(cm),
    )
    .await?;
    Ok(())
}

async fn reconcile_deployment(
    client: &Client,
    agent: &OmegonAgent,
    ns: &str,
    name: &str,
) -> Result<(), kube::Error> {
    let api: Api<Deployment> = Api::namespaced(client.clone(), ns);

    let tpl = pod_spec(agent, name);
    let mut template_metadata = json!({ "labels": { "styrene.sh/agent": name } });
    if let Some(ref annotations) = tpl.annotations {
        template_metadata
            .as_object_mut()
            .unwrap()
            .insert("annotations".into(), annotations.clone());
    }

    let deploy = json!({
        "apiVersion": "apps/v1",
        "kind": "Deployment",
        "metadata": {
            "name": name,
            "namespace": ns,
            "ownerReferences": [owner_ref(agent)],
        },
        "spec": {
            "replicas": 1,
            "selector": { "matchLabels": { "styrene.sh/agent": name } },
            "template": {
                "metadata": template_metadata,
                "spec": tpl.spec,
            }
        }
    });

    api.patch(
        name,
        &PatchParams::apply("auspex-operator"),
        &Patch::Apply(deploy),
    )
    .await?;
    Ok(())
}

async fn reconcile_cronjob(
    client: &Client,
    agent: &OmegonAgent,
    ns: &str,
    name: &str,
) -> Result<(), kube::Error> {
    let api: Api<CronJob> = Api::namespaced(client.clone(), ns);
    let schedule = agent.spec.schedule.as_deref().unwrap_or("0 * * * *");

    let tpl = pod_spec(agent, name);
    let mut spec = tpl.spec;
    spec.as_object_mut()
        .unwrap()
        .insert("restartPolicy".into(), json!("Never"));

    let mut template_metadata = json!({ "labels": { "styrene.sh/agent": name } });
    if let Some(ref annotations) = tpl.annotations {
        template_metadata
            .as_object_mut()
            .unwrap()
            .insert("annotations".into(), annotations.clone());
    }

    let cj = json!({
        "apiVersion": "batch/v1",
        "kind": "CronJob",
        "metadata": {
            "name": name,
            "namespace": ns,
            "ownerReferences": [owner_ref(agent)],
        },
        "spec": {
            "schedule": schedule,
            "concurrencyPolicy": "Forbid",
            "successfulJobsHistoryLimit": 3,
            "failedJobsHistoryLimit": 3,
            "jobTemplate": {
                "spec": {
                    "backoffLimit": 1,
                    "template": {
                        "metadata": template_metadata,
                        "spec": spec,
                    }
                }
            }
        }
    });

    api.patch(
        name,
        &PatchParams::apply("auspex-operator"),
        &Patch::Apply(cj),
    )
    .await?;
    Ok(())
}

async fn reconcile_job(
    client: &Client,
    agent: &OmegonAgent,
    ns: &str,
    name: &str,
) -> Result<(), kube::Error> {
    let api: Api<Job> = Api::namespaced(client.clone(), ns);

    let tpl = pod_spec(agent, name);
    let mut spec = tpl.spec;
    spec.as_object_mut()
        .unwrap()
        .insert("restartPolicy".into(), json!("Never"));

    let mut template_metadata = json!({ "labels": { "styrene.sh/agent": name } });
    if let Some(ref annotations) = tpl.annotations {
        template_metadata
            .as_object_mut()
            .unwrap()
            .insert("annotations".into(), annotations.clone());
    }

    let active_deadline = agent
        .spec
        .bounds
        .as_ref()
        .and_then(|b| b.active_deadline_seconds);

    let mut job_spec = json!({
        "backoffLimit": 0,
        "template": {
            "metadata": template_metadata,
            "spec": spec,
        }
    });

    if let Some(deadline) = active_deadline {
        job_spec
            .as_object_mut()
            .unwrap()
            .insert("activeDeadlineSeconds".into(), json!(deadline));
    }

    let job = json!({
        "apiVersion": "batch/v1",
        "kind": "Job",
        "metadata": {
            "name": name,
            "namespace": ns,
            "ownerReferences": [owner_ref(agent)],
        },
        "spec": job_spec,
    });

    api.patch(
        name,
        &PatchParams::apply("auspex-operator"),
        &Patch::Apply(job),
    )
    .await?;
    Ok(())
}

async fn reconcile_prompt_configmap(
    client: &Client,
    agent: &OmegonAgent,
    ns: &str,
    name: &str,
) -> Result<(), kube::Error> {
    let inline = agent.spec.prompt.as_ref().and_then(|p| p.inline.as_deref());

    let Some(prompt_text) = inline else {
        return Ok(());
    };

    let api: Api<ConfigMap> = Api::namespaced(client.clone(), ns);
    let cm_name = format!("{name}-prompt");

    let cm = json!({
        "apiVersion": "v1",
        "kind": "ConfigMap",
        "metadata": {
            "name": cm_name,
            "namespace": ns,
            "ownerReferences": [owner_ref(agent)],
        },
        "data": {
            "prompt.txt": prompt_text,
        }
    });

    api.patch(
        &cm_name,
        &PatchParams::apply("auspex-operator"),
        &Patch::Apply(cm),
    )
    .await?;
    Ok(())
}

async fn reconcile_service(
    client: &Client,
    agent: &OmegonAgent,
    ns: &str,
    name: &str,
) -> Result<(), kube::Error> {
    let api: Api<Service> = Api::namespaced(client.clone(), ns);

    let svc = json!({
        "apiVersion": "v1",
        "kind": "Service",
        "metadata": {
            "name": name,
            "namespace": ns,
            "ownerReferences": [owner_ref(agent)],
        },
        "spec": {
            "selector": { "styrene.sh/agent": name },
            "ports": [{
                "port": 7842,
                "targetPort": 7842,
                "protocol": "TCP",
                "name": "control",
            }],
        }
    });

    api.patch(
        name,
        &PatchParams::apply("auspex-operator"),
        &Patch::Apply(svc),
    )
    .await?;
    Ok(())
}

fn pod_spec(agent: &OmegonAgent, name: &str) -> PodTemplate {
    let is_bounded = matches!(agent.spec.mode, AgentMode::Job | AgentMode::Cronjob);
    let has_aether = agent.spec.vox.connectors.iter().any(|c| c == "aether");
    let control_tls = resolved_control_tls(agent, name);
    let control_tls_args_enabled =
        omegon_runtime_flag_enabled("AUSPEX_ENABLE_OMEGON_CONTROL_TLS_ARGS");

    let mut env = vec![
        json!({"name": "VOX_CONFIG_PATH", "value": "/config/vox"}),
        json!({"name": "AETHER_WORKER_ROLE", "value": &agent.spec.role}),
        json!({
            "name": "OMEGON_TERMINAL_TOOL",
            "value": if agent.spec.terminal_tool { "1" } else { "0" },
        }),
    ];
    if let Some(ref discord) = agent.spec.vox.discord
        && let Some(ref gid) = discord.guild_id
    {
        env.push(json!({"name": "VOX_DISCORD_GUILD_ID", "value": gid}));
    }

    // When aether is enabled, set the styrened socket path and extensions dir.
    if has_aether {
        env.push(json!({"name": "STYRENED_SOCKET", "value": "/shared/styrened.sock"}));
        env.push(json!({"name": "OMEGON_EXTENSIONS_PATH", "value": "/extensions"}));
    }

    // Inject profile reference as annotation for SBOM tracking.
    if let Some(ref profile) = agent.spec.profile {
        env.push(json!({"name": "OMEGON_PROFILE", "value": profile}));
    }

    // StyreneID env vars (volumes mounted below after declaration).
    if agent.spec.identity.as_ref().is_some_and(|id| id.provision) {
        env.push(json!({
            "name": "STYRENE_IDENTITY_PATH",
            "value": "/run/styrene/identity/root-secret"
        }));
        let role = agent
            .spec
            .identity
            .as_ref()
            .map(|id| id.mesh_role.as_str())
            .unwrap_or("operator");
        env.push(json!({"name": "STYRENE_MESH_ROLE", "value": role}));
    }

    // Mount vox config at a well-known path that doesn't assume root home.
    // Omegon reads VOX_CONFIG_PATH if set, falling back to ~/.config/vox.
    let mut volume_mounts = vec![json!({
        "name": "vox-config",
        "mountPath": "/config/vox",
    })];
    let mut volumes = vec![json!({
        "name": "vox-config",
        "configMap": { "name": format!("{name}-vox") },
    })];

    volumes.push(json!({
        "name": "agent-catalog",
        "configMap": {
            "name": format!("{name}-catalog"),
            "optional": true,
        },
    }));
    volume_mounts.push(json!({
        "name": "agent-catalog",
        "mountPath": format!("/data/omegon/catalog/{}", agent.spec.agent),
        "readOnly": true,
    }));

    if has_aether {
        // Shared volume for Unix socket between agent and styrened sidecar.
        volumes.push(json!({
            "name": "shared",
            "emptyDir": { "medium": "Memory", "sizeLimit": "10Mi" },
        }));
        volume_mounts.push(json!({
            "name": "shared",
            "mountPath": "/shared",
        }));

        // Extensions volume (populated by init container).
        volumes.push(json!({
            "name": "extensions",
            "emptyDir": {},
        }));
        volume_mounts.push(json!({
            "name": "extensions",
            "mountPath": "/extensions",
            "readOnly": true,
        }));

        // styrened sidecar config: use per-agent ConfigMap name so each
        // agent can have its own mesh config. Falls back to a shared default.
        let styrened_cm = format!("{name}-styrened");
        volumes.push(json!({
            "name": "styrened-config",
            "configMap": {
                "name": styrened_cm,
                "optional": true,
            },
        }));
    }

    if let Some(ref auth_secret) = agent.spec.secrets.auth_json_secret {
        env.push(json!({
            "name": "OMEGON_AUTH_JSON_PATH",
            "value": "/config/omegon/auth.json",
        }));
        volume_mounts.push(json!({
            "name": "auth-json",
            "mountPath": "/config/omegon",
            "readOnly": true,
        }));
        volumes.push(json!({
            "name": "auth-json",
            "secret": {
                "secretName": auth_secret,
                "items": [{"key": "auth.json", "path": "auth.json"}],
            },
        }));
    }

    // StyreneID secret volume for mesh identity.
    if agent.spec.identity.as_ref().is_some_and(|id| id.provision) {
        let secret_name = format!("{name}-styrene-id");
        volumes.push(json!({
            "name": "styrene-id",
            "secret": {
                "secretName": secret_name,
                "items": [{"key": "root-secret", "path": "root-secret"}],
            },
        }));
        volume_mounts.push(json!({
            "name": "styrene-id",
            "mountPath": "/run/styrene/identity",
            "readOnly": true,
        }));
    }

    if let Some(tls) = control_tls.as_ref() {
        let mut items = vec![
            json!({"key": tls.cert_key, "path": "tls.crt"}),
            json!({"key": tls.key_key, "path": "tls.key"}),
        ];
        if let Some(client_ca_key) = tls.client_ca_key.as_ref() {
            items.push(json!({"key": client_ca_key, "path": "ca.crt"}));
        }
        volumes.push(json!({
            "name": "control-tls",
            "secret": {
                "secretName": tls.secret_name,
                "items": items,
            },
        }));
        volume_mounts.push(json!({
            "name": "control-tls",
            "mountPath": CONTROL_TLS_MOUNT_PATH,
            "readOnly": true,
        }));
    }

    if let Some(ref secret_name) = agent.spec.secrets.secret_name {
        volume_mounts.push(json!({
            "name": "runtime-secrets",
            "mountPath": "/run/omegon/secrets",
            "readOnly": true,
        }));
        volumes.push(json!({
            "name": "runtime-secrets",
            "secret": { "secretName": secret_name },
        }));
    }
    let env_from: Vec<serde_json::Value> = vec![];

    // Prompt/output mounts for bounded modes.
    if is_bounded {
        let prompt_mount_path = agent
            .spec
            .prompt
            .as_ref()
            .map(|p| p.mount_path.as_str())
            .unwrap_or("/input/prompt.txt");
        let prompt_dir = prompt_mount_path
            .rsplit_once('/')
            .map(|(d, _)| d)
            .unwrap_or("/input");

        // Prompt volume: from Secret (sensitive), ConfigMap (inline), or user-provided.
        // Priority: secret > inline (stored as ConfigMap) > config_map reference.
        let prompt_spec = agent.spec.prompt.as_ref();
        if let Some(secret_name) = prompt_spec.and_then(|p| p.secret.as_ref()) {
            volumes.push(json!({
                "name": "prompt",
                "secret": {
                    "secretName": secret_name,
                    "items": [{"key": "prompt.txt", "path": "prompt.txt"}],
                },
            }));
            volume_mounts.push(json!({
                "name": "prompt",
                "mountPath": prompt_dir,
                "readOnly": true,
            }));
        } else if prompt_spec.and_then(|p| p.inline.as_ref()).is_some() {
            volumes.push(json!({
                "name": "prompt",
                "configMap": { "name": format!("{name}-prompt") },
            }));
            volume_mounts.push(json!({
                "name": "prompt",
                "mountPath": prompt_dir,
                "readOnly": true,
            }));
        } else if let Some(cm) = prompt_spec.and_then(|p| p.config_map.as_ref()) {
            volumes.push(json!({
                "name": "prompt",
                "configMap": { "name": cm },
            }));
            volume_mounts.push(json!({
                "name": "prompt",
                "mountPath": prompt_dir,
                "readOnly": true,
            }));
        }

        // Output volume for structured results.
        volumes.push(json!({
            "name": "output",
            "emptyDir": {},
        }));
        volume_mounts.push(json!({
            "name": "output",
            "mountPath": "/output",
        }));
    }

    // Build container args: `run` for bounded modes, `serve` for daemon.
    let mut args: Vec<String> = if is_bounded {
        vec!["run".into()]
    } else {
        vec![
            "serve".into(),
            "--control-port".into(),
            "7842".into(),
            "--strict-port".into(),
        ]
    };

    if !is_bounded
        && control_tls_args_enabled
        && let Some(tls) = control_tls.as_ref()
    {
        args.extend([
            "--control-tls-cert".into(),
            format!("{CONTROL_TLS_MOUNT_PATH}/tls.crt"),
            "--control-tls-key".into(),
            format!("{CONTROL_TLS_MOUNT_PATH}/tls.key"),
        ]);
        if tls.client_ca_key.is_some() {
            args.extend([
                "--control-tls-client-ca".into(),
                format!("{CONTROL_TLS_MOUNT_PATH}/ca.crt"),
            ]);
        }
    }

    args.extend(["--agent".into(), agent.spec.agent.clone()]);
    args.extend(["--model".into(), agent.spec.model.clone()]);
    if omegon_runtime_flag_enabled("AUSPEX_ENABLE_OMEGON_POSTURE_ARG") {
        args.extend(["--posture".into(), agent.spec.posture.clone()]);
    }

    // Append resource bounds for bounded modes.
    if let Some(ref bounds) = agent.spec.bounds {
        if let Some(turns) = bounds.max_turns {
            args.extend(["--max-turns".into(), turns.to_string()]);
        }
        if let Some(timeout) = bounds.timeout {
            args.extend(["--timeout".into(), timeout.to_string()]);
        }
        if let Some(budget) = bounds.token_budget {
            args.extend(["--token-budget".into(), budget.to_string()]);
        }
        if let Some(ref ctx_class) = bounds.context_class {
            args.extend(["--context-class".into(), ctx_class.clone()]);
        }
    }

    // Prompt and output paths for bounded modes.
    if is_bounded && let Some(ref prompt) = agent.spec.prompt {
        args.extend(["--prompt-file".into(), prompt.mount_path.clone()]);
        args.extend(["--output".into(), prompt.output_path.clone()]);
    }

    // Build container list: agent + optional styrened sidecar.
    let mut agent_container = json!({
        "name": "agent",
        "image": &agent.spec.image,
        "command": ["omegon"],
        "args": args,
        "env": env,
        "envFrom": env_from,
        "volumeMounts": volume_mounts,
    });

    // Daemon mode gets health probes and exposed ports.
    if !is_bounded {
        let container = agent_container.as_object_mut().unwrap();
        container.insert("ports".into(), json!([{"containerPort": 7842}]));
        let probe_scheme = if control_tls.is_some() && control_tls_args_enabled {
            "https"
        } else {
            "http"
        };
        let probe_curl_args = |path: &str| {
            let url = format!("{probe_scheme}://127.0.0.1:7842{path}");
            if probe_scheme == "https" {
                vec!["curl".to_string(), "-kfsS".to_string(), url]
            } else {
                vec!["curl".to_string(), "-fsS".to_string(), url]
            }
        };
        container.insert(
            "livenessProbe".into(),
            json!({
                "exec": { "command": probe_curl_args("/api/healthz") },
                "initialDelaySeconds": 15,
                "periodSeconds": 30,
            }),
        );
        container.insert(
            "readinessProbe".into(),
            json!({
                "exec": { "command": probe_curl_args("/api/readyz") },
                "initialDelaySeconds": 10,
                "periodSeconds": 10,
            }),
        );
    }

    // Resource limits from spec.
    if let Some(ref res) = agent.spec.resources {
        let mut requests = serde_json::Map::new();
        let mut limits = serde_json::Map::new();
        if let Some(ref cpu) = res.cpu {
            requests.insert("cpu".into(), json!(cpu));
            limits.insert("cpu".into(), json!(cpu));
        }
        if let Some(ref mem) = res.memory {
            requests.insert("memory".into(), json!(mem));
            limits.insert("memory".into(), json!(mem));
        }
        if !requests.is_empty() {
            agent_container.as_object_mut().unwrap().insert(
                "resources".into(),
                json!({ "requests": requests, "limits": limits }),
            );
        }
    }

    let mut containers = vec![agent_container];

    // styrened sidecar: provides mesh transport via Unix socket.
    if has_aether {
        let has_identity = agent.spec.identity.as_ref().is_some_and(|id| id.provision);

        let mut styrened_env =
            vec![json!({"name": "STYRENED_SOCKET", "value": "/shared/styrened.sock"})];
        let mut styrened_mounts = vec![
            json!({"name": "shared", "mountPath": "/shared"}),
            json!({"name": "styrened-config", "mountPath": "/etc/styrene", "readOnly": true}),
        ];

        // Share the StyreneID with styrened so it can announce on RNS
        // with the same identity the operator pre-authorized.
        if has_identity {
            styrened_mounts.push(json!({
                "name": "styrene-id",
                "mountPath": "/run/styrene/identity",
                "readOnly": true,
            }));
            styrened_env.push(json!({
                "name": "STYRENE_IDENTITY_PATH",
                "value": "/run/styrene/identity/root-secret"
            }));
        }

        containers.push(json!({
            "name": "styrened",
            "image": styrened_image(),
            "env": styrened_env,
            "ports": [
                { "containerPort": 9101, "name": "metrics", "protocol": "TCP" },
            ],
            "volumeMounts": styrened_mounts,
            "resources": {
                "requests": { "cpu": "50m", "memory": "64Mi" },
                "limits": { "cpu": "200m", "memory": "128Mi" },
            },
        }));
    }

    // Init container: copy aether extension binary into shared volume.
    let init_containers = if has_aether {
        json!([{
            "name": "install-aether",
            "image": aether_image(),
            "command": ["/bin/sh", "-c",
                "mkdir -p /extensions/aether && \
                 cp /usr/local/bin/aether /extensions/aether/aether && \
                 cp /usr/local/lib/omegon/extensions/aether/manifest.toml /extensions/aether/manifest.toml"
            ],
            "volumeMounts": [
                { "name": "extensions", "mountPath": "/extensions" },
            ],
        }])
    } else {
        json!([])
    };

    // Annotations for SBOM, profile, and Vault injection.
    let mut annotations = serde_json::Map::new();
    if let Some(ref profile) = agent.spec.profile {
        annotations.insert("styrene.sh/profile".into(), json!(profile));
    }
    if let Some(ref sbom) = agent.spec.sbom
        && sbom.enabled
    {
        annotations.insert("styrene.sh/sbom-format".into(), json!(&sbom.format));
        if let Some(ref artifact) = sbom.artifact_ref {
            annotations.insert("styrene.sh/sbom-ref".into(), json!(artifact));
        }
    }

    // Vault Agent injector annotations: when vault secrets are configured,
    // annotate the pod so the Vault Agent sidecar injects secrets directly.
    // This means secrets never pass through the operator's memory or k8s Secrets.
    if let Some(ref vault) = agent.spec.secrets.vault
        && vault.agent_inject
    {
        annotations.insert("vault.hashicorp.com/agent-inject".into(), json!("true"));
        if let Some(ref role) = vault.role {
            annotations.insert("vault.hashicorp.com/role".into(), json!(role));
        }
        if let Some(ref addr) = vault.address {
            annotations.insert(
                "vault.hashicorp.com/agent-inject-status".into(),
                json!("update"),
            );
            annotations.insert("vault.hashicorp.com/agent-address".into(), json!(addr));
        }
        for mapping in &vault.secrets {
            // The annotation suffix becomes the filename under /vault/secrets/.
            // Sanitize the destination to a safe annotation key suffix.
            let suffix = mapping
                .destination
                .trim_start_matches('/')
                .replace('/', "-");

            annotations.insert(
                format!("vault.hashicorp.com/agent-inject-secret-{suffix}"),
                json!(&mapping.path),
            );
            if let Some(ref template) = mapping.template {
                annotations.insert(
                    format!("vault.hashicorp.com/agent-inject-template-{suffix}"),
                    json!(template),
                );
            }
        }
    }

    let pod_spec = json!({
        "initContainers": init_containers,
        "containers": containers,
        "volumes": volumes,
        "terminationGracePeriodSeconds": if is_bounded { 30 } else { 60 },
    });

    PodTemplate {
        spec: pod_spec,
        annotations: if annotations.is_empty() {
            None
        } else {
            Some(json!(annotations))
        },
    }
}

fn omegon_runtime_flag_enabled(name: &str) -> bool {
    std::env::var(name).is_ok_and(|value| matches!(value.as_str(), "1" | "true" | "TRUE" | "yes"))
}

/// Pod template with spec and optional annotations for the template metadata.
pub struct PodTemplate {
    pub spec: serde_json::Value,
    pub annotations: Option<serde_json::Value>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedControlTls {
    pub secret_name: String,
    pub cert_key: String,
    pub key_key: String,
    pub client_ca_key: Option<String>,
    pub profile: String,
    pub ca_epoch: String,
    pub leaf_epoch: String,
    pub validity: crate::crd::ControlPlaneTlsValiditySpec,
}

pub fn control_plane_tls_enabled(agent: &OmegonAgent) -> bool {
    agent.spec.control_plane.tls.enabled || agent.spec.identity.as_ref().is_some_and(|id| id.mtls)
}

pub fn resolved_control_tls(agent: &OmegonAgent, name: &str) -> Option<ResolvedControlTls> {
    if !control_plane_tls_enabled(agent) {
        return None;
    }

    let tls = &agent.spec.control_plane.tls;
    Some(ResolvedControlTls {
        secret_name: tls
            .secret_name
            .clone()
            .unwrap_or_else(|| format!("{name}-control-tls")),
        cert_key: tls.cert_key.clone(),
        key_key: tls.key_key.clone(),
        client_ca_key: tls.client_ca_key.clone(),
        profile: tls.profile.clone(),
        ca_epoch: tls.ca_epoch.clone(),
        leaf_epoch: tls.leaf_epoch.clone(),
        validity: tls.validity.clone(),
    })
}

pub fn owner_ref(agent: &OmegonAgent) -> serde_json::Value {
    json!({
        "apiVersion": "styrene.sh/v1alpha1",
        "kind": "OmegonAgent",
        "name": agent.name_any(),
        "uid": agent.metadata.uid.as_deref().unwrap_or(""),
        "controller": true,
        "blockOwnerDeletion": true,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn daemon_agent(value: serde_json::Value) -> OmegonAgent {
        serde_json::from_value(value).expect("valid OmegonAgent")
    }

    fn env_value<'a>(container: &'a serde_json::Value, name: &str) -> Option<&'a str> {
        container["env"].as_array()?.iter().find_map(|env| {
            (env["name"].as_str()? == name)
                .then(|| env["value"].as_str())
                .flatten()
        })
    }

    #[test]
    fn pod_spec_mounts_tls_secret_and_passes_control_tls_args() {
        unsafe {
            std::env::set_var("AUSPEX_ENABLE_OMEGON_CONTROL_TLS_ARGS", "true");
        }
        let agent = daemon_agent(serde_json::json!({
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
        }));

        let template = pod_spec(&agent, "secure-primary");
        let agent_container = &template.spec["containers"][0];
        let args = agent_container["args"].as_array().expect("args array");
        let args: Vec<_> = args.iter().filter_map(|value| value.as_str()).collect();

        assert!(args.contains(&"--control-tls-cert"));
        assert!(args.contains(&"/run/omegon/control-tls/tls.crt"));
        assert!(args.contains(&"--control-tls-key"));
        assert!(args.contains(&"/run/omegon/control-tls/tls.key"));
        assert!(args.contains(&"--control-tls-client-ca"));
        assert!(args.contains(&"/run/omegon/control-tls/ca.crt"));
        let readiness_command = agent_container["readinessProbe"]["exec"]["command"]
            .as_array()
            .expect("readiness command");
        assert!(readiness_command.iter().any(|value| value == "-kfsS"));
        assert!(readiness_command.iter().any(|value| {
            value
                .as_str()
                .is_some_and(|item| item == "https://127.0.0.1:7842/api/readyz")
        }));
        assert_eq!(
            template.spec["volumes"]
                .as_array()
                .expect("volumes")
                .iter()
                .find(|volume| volume["name"] == "control-tls")
                .expect("control TLS volume")["secret"]["secretName"],
            "secure-primary-control-tls"
        );
        unsafe {
            std::env::remove_var("AUSPEX_ENABLE_OMEGON_CONTROL_TLS_ARGS");
        }
    }

    #[test]
    fn identity_mtls_uses_default_control_tls_secret() {
        let agent = daemon_agent(serde_json::json!({
            "apiVersion": "styrene.sh/v1alpha1",
            "kind": "OmegonAgent",
            "metadata": {
                "name": "primary-driver",
                "namespace": "omegon-agents"
            },
            "spec": {
                "agent": "styrene.primary",
                "model": "anthropic:claude-sonnet-4-6",
                "role": "primary-driver",
                "mode": "daemon",
                "identity": {
                    "provision": true,
                    "mtls": true
                }
            }
        }));

        let tls = resolved_control_tls(&agent, "primary-driver").expect("resolved TLS");

        assert_eq!(tls.secret_name, "primary-driver-control-tls");
        assert_eq!(tls.client_ca_key.as_deref(), Some("ca.crt"));
        assert_eq!(tls.profile, "default");
        assert_eq!(tls.ca_epoch, "0");
        assert_eq!(tls.leaf_epoch, "0");
        assert_eq!(tls.validity.leaf_not_after_year, 2031);
    }

    #[test]
    fn terminal_tool_policy_is_explicit_in_agent_env() {
        let disabled = daemon_agent(serde_json::json!({
            "apiVersion": "styrene.sh/v1alpha1",
            "kind": "OmegonAgent",
            "metadata": {
                "name": "headless-agent",
                "namespace": "omegon-agents"
            },
            "spec": {
                "agent": "styrene.headless",
                "model": "anthropic:claude-sonnet-4-6",
                "role": "detached-service",
                "mode": "daemon"
            }
        }));
        let enabled = daemon_agent(serde_json::json!({
            "apiVersion": "styrene.sh/v1alpha1",
            "kind": "OmegonAgent",
            "metadata": {
                "name": "interactive-agent",
                "namespace": "omegon-agents"
            },
            "spec": {
                "agent": "styrene.interactive",
                "model": "anthropic:claude-sonnet-4-6",
                "role": "primary-driver",
                "mode": "daemon",
                "terminalTool": true
            }
        }));

        let disabled_template = pod_spec(&disabled, "headless-agent");
        let enabled_template = pod_spec(&enabled, "interactive-agent");

        assert_eq!(
            env_value(
                &disabled_template.spec["containers"][0],
                "OMEGON_TERMINAL_TOOL"
            ),
            Some("0")
        );
        assert_eq!(
            env_value(
                &enabled_template.spec["containers"][0],
                "OMEGON_TERMINAL_TOOL"
            ),
            Some("1")
        );
    }

    #[test]
    fn daemon_agents_mount_optional_catalog_by_agent_id() {
        let agent = daemon_agent(serde_json::json!({
            "apiVersion": "styrene.sh/v1alpha1",
            "kind": "OmegonAgent",
            "metadata": {
                "name": "release-manager",
                "namespace": "omegon-agents"
            },
            "spec": {
                "agent": "styrene.release-manager-agent",
                "model": "openai-codex:gpt-5.5",
                "role": "detached-service",
                "mode": "daemon"
            }
        }));

        let template = pod_spec(&agent, "release-manager");
        let agent_container = &template.spec["containers"][0];

        let catalog_volume = template.spec["volumes"]
            .as_array()
            .expect("volumes")
            .iter()
            .find(|volume| volume["name"] == "agent-catalog")
            .expect("agent catalog volume");
        assert_eq!(
            catalog_volume["configMap"]["name"],
            "release-manager-catalog"
        );
        assert_eq!(catalog_volume["configMap"]["optional"], true);
        assert_eq!(
            agent_container["volumeMounts"]
                .as_array()
                .expect("volume mounts")
                .iter()
                .find(|mount| mount["name"] == "agent-catalog")
                .expect("agent catalog mount")["mountPath"],
            "/data/omegon/catalog/styrene.release-manager-agent"
        );
    }



    #[test]
    fn connector_secret_mounts_as_files_and_configures_vox_paths() {
        let agent = daemon_agent(serde_json::json!({
            "apiVersion": "styrene.sh/v1alpha1",
            "kind": "OmegonAgent",
            "metadata": {
                "name": "release-manager",
                "namespace": "omegon-agents"
            },
            "spec": {
                "agent": "styrene.release-manager-agent",
                "model": "ollama-cloud:gpt-oss:120b-cloud",
                "role": "detached-service",
                "mode": "daemon",
                "vox": {
                    "connectors": ["discord", "slack"],
                    "discord": { "requireMention": true, "guildId": "D123" },
                    "slack": {
                        "workspace": "styrene",
                        "defaultChannel": "C123",
                        "requireMention": true
                    }
                },
                "secrets": {
                    "secretName": "release-manager-public-connectors"
                }
            }
        }));

        let template = pod_spec(&agent, "release-manager");
        let agent_container = &template.spec["containers"][0];

        assert!(agent_container["envFrom"].as_array().expect("envFrom").is_empty());
        assert_eq!(
            agent_container["volumeMounts"]
                .as_array()
                .expect("volume mounts")
                .iter()
                .find(|mount| mount["name"] == "runtime-secrets")
                .expect("runtime secrets mount")["mountPath"],
            "/run/omegon/secrets"
        );
        assert_eq!(
            template.spec["volumes"]
                .as_array()
                .expect("volumes")
                .iter()
                .find(|volume| volume["name"] == "runtime-secrets")
                .expect("runtime secrets volume")["secret"]["secretName"],
            "release-manager-public-connectors"
        );
    }

    #[test]
    fn auth_json_secret_sets_explicit_runtime_path() {
        let agent = daemon_agent(serde_json::json!({
            "apiVersion": "styrene.sh/v1alpha1",
            "kind": "OmegonAgent",
            "metadata": {
                "name": "primary-driver",
                "namespace": "omegon-agents"
            },
            "spec": {
                "agent": "styrene.primary",
                "model": "openai-codex:gpt-5.4",
                "role": "primary-driver",
                "mode": "daemon",
                "secrets": {
                    "authJsonSecret": "primary-driver-openai-codex-auth"
                }
            }
        }));

        let template = pod_spec(&agent, "primary-driver");
        let agent_container = &template.spec["containers"][0];

        assert_eq!(
            agent_container["env"]
                .as_array()
                .expect("env")
                .iter()
                .find(|env| env["name"] == "OMEGON_AUTH_JSON_PATH")
                .expect("OMEGON_AUTH_JSON_PATH")["value"],
            "/config/omegon/auth.json"
        );
        assert_eq!(
            agent_container["volumeMounts"]
                .as_array()
                .expect("volume mounts")
                .iter()
                .find(|mount| mount["name"] == "auth-json")
                .expect("auth json mount")["mountPath"],
            "/config/omegon"
        );
        assert_eq!(
            template.spec["volumes"]
                .as_array()
                .expect("volumes")
                .iter()
                .find(|volume| volume["name"] == "auth-json")
                .expect("auth json volume")["secret"]["secretName"],
            "primary-driver-openai-codex-auth"
        );
    }

    #[test]
    fn control_tls_rotation_fields_resolve_from_spec() {
        let agent: OmegonAgent = serde_json::from_value(json!({
            "apiVersion": "styrene.sh/v1alpha1",
            "kind": "OmegonAgent",
            "metadata": {
                "name": "primary-driver",
                "namespace": "omegon-agents"
            },
            "spec": {
                "agent": "styrene.primary",
                "model": "anthropic:claude-sonnet-4-6",
                "mode": "daemon",
                "controlPlane": {
                    "tls": {
                        "enabled": true,
                        "profile": "prod",
                        "caEpoch": "2026h1",
                        "leafEpoch": "2026w20",
                        "validity": {
                            "caNotBeforeYear": 2026,
                            "caNotAfterYear": 2030,
                            "leafNotBeforeYear": 2026,
                            "leafNotAfterYear": 2027
                        }
                    }
                }
            }
        }))
        .expect("valid OmegonAgent");

        let tls = resolved_control_tls(&agent, "primary-driver").expect("resolved TLS");
        assert_eq!(tls.profile, "prod");
        assert_eq!(tls.ca_epoch, "2026h1");
        assert_eq!(tls.leaf_epoch, "2026w20");
        assert_eq!(tls.validity.ca_not_after_year, 2030);
        assert_eq!(tls.validity.leaf_not_after_year, 2027);
    }
}
