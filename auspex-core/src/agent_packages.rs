//! Agent package catalog primitives for Auspex-managed Omegon deployments.
//!
//! This is the thin bridge between Armory/Omegon package intent and Auspex
//! runtime orchestration. Armory owns reusable agent bundles. Nex owns profile
//! to image construction. Auspex owns choosing a package and realizing it as a
//! managed runtime, such as a Kubernetes `OmegonAgent`.

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
#[cfg(not(target_arch = "wasm32"))]
use std::collections::HashMap;

#[cfg(not(target_arch = "wasm32"))]
use crate::oci_backend::{OciLaunchSpec, PullPolicy, LABEL_PACKAGE_ID};
#[cfg(not(target_arch = "wasm32"))]
use crate::runtime_types::ResourceRequirements;

/// A deployable Omegon agent package as Auspex needs to understand it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AgentPackage {
    pub id: String,
    pub name: String,
    pub description: String,
    pub domain: String,
    pub agent: String,
    pub profile: String,
    pub default_model: String,
    pub posture: String,
    pub role: String,
    pub mode: String,
    pub image: String,
    #[serde(default)]
    pub labels: Vec<String>,
    #[serde(default)]
    pub required_secrets: Vec<String>,
    #[serde(default)]
    pub optional_secrets: Vec<String>,
    #[serde(default)]
    pub resources: PackageResources,
    #[serde(default)]
    pub control_tls_profile: String,
    #[serde(default)]
    pub mesh_role: String,
    #[serde(default, rename = "terminalTool", alias = "terminal_tool")]
    pub terminal_tool: bool,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct PackageResources {
    #[serde(default)]
    pub cpu: Option<String>,
    #[serde(default)]
    pub memory: Option<String>,
}

/// Runtime overrides supplied by the WebUI/API when creating an agent from a
/// package.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct AgentPackageDeployRequest {
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub namespace: Option<String>,
    #[serde(default)]
    pub image: Option<String>,
    #[serde(default)]
    pub model: Option<String>,
    #[serde(default, rename = "secretName", alias = "secret_name")]
    pub secret_name: Option<String>,
    #[serde(default, rename = "authJsonSecret", alias = "auth_json_secret")]
    pub auth_json_secret: Option<String>,
    #[serde(default)]
    pub connectors: Vec<String>,
    /// Host port used by the OCI backend when launching directly through
    /// Docker/Podman. Kubernetes deployments ignore this field.
    #[serde(default, rename = "hostPort", alias = "host_port")]
    pub host_port: Option<u16>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum OciImageReferenceKind {
    Digest,
    TagAndDigest,
    Tag,
    Untagged,
    Invalid,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OciImageAssessment {
    pub image: String,
    pub kind: OciImageReferenceKind,
    pub digest_pinned: bool,
    pub mutable_tag: bool,
    #[serde(default)]
    pub tag: Option<String>,
    #[serde(default)]
    pub digest: Option<String>,
    #[serde(default)]
    pub warnings: Vec<String>,
    #[serde(default)]
    pub errors: Vec<String>,
}

pub fn assess_oci_image_ref(image: &str) -> OciImageAssessment {
    let image = image.trim();
    let mut warnings = Vec::new();
    let mut errors = Vec::new();
    if image.is_empty() {
        errors.push("image reference is required".into());
        return OciImageAssessment {
            image: image.into(),
            kind: OciImageReferenceKind::Invalid,
            digest_pinned: false,
            mutable_tag: false,
            tag: None,
            digest: None,
            warnings,
            errors,
        };
    }
    if image.chars().any(char::is_whitespace) {
        errors.push("image reference must not contain whitespace".into());
    }

    let (name_and_tag, digest) = match image.split_once('@') {
        Some((name, digest)) => {
            if !is_supported_digest(digest) {
                errors.push("image digest must be sha256:<64 lowercase hex characters>".into());
            }
            (name, Some(digest.to_string()))
        }
        None => (image, None),
    };
    let tag = image_tag(name_and_tag);
    if digest.is_none() {
        match tag.as_deref() {
            Some("latest") => warnings.push("image uses mutable tag 'latest'".into()),
            Some(_) => warnings.push("image uses a mutable tag instead of a digest".into()),
            None => warnings.push("image has no tag or digest and will resolve implicitly".into()),
        }
    }

    let kind = if !errors.is_empty() {
        OciImageReferenceKind::Invalid
    } else {
        match (tag.is_some(), digest.is_some()) {
            (false, true) => OciImageReferenceKind::Digest,
            (true, true) => OciImageReferenceKind::TagAndDigest,
            (true, false) => OciImageReferenceKind::Tag,
            (false, false) => OciImageReferenceKind::Untagged,
        }
    };

    OciImageAssessment {
        image: image.into(),
        kind,
        digest_pinned: digest.is_some() && errors.is_empty(),
        mutable_tag: tag.is_some() && digest.is_none(),
        tag,
        digest,
        warnings,
        errors,
    }
}

fn image_tag(image_without_digest: &str) -> Option<String> {
    let last_segment = image_without_digest.rsplit('/').next().unwrap_or_default();
    last_segment
        .rsplit_once(':')
        .map(|(_, tag)| tag)
        .filter(|tag| !tag.is_empty())
        .map(str::to_string)
}

fn is_supported_digest(digest: &str) -> bool {
    let Some(hex) = digest.strip_prefix("sha256:") else {
        return false;
    };
    hex.len() == 64
        && hex
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

impl AgentPackage {
    /// Build a Kubernetes `OmegonAgent` manifest for this package.
    pub fn omegon_agent_manifest(&self, request: &AgentPackageDeployRequest) -> Value {
        let name = request.name.as_deref().unwrap_or(self.id.as_str());
        let namespace = request.namespace.as_deref().unwrap_or("omegon-agents");
        let image = request.image.as_deref().unwrap_or(self.image.as_str());
        let model = request
            .model
            .as_deref()
            .unwrap_or(self.default_model.as_str());
        let stack_label = self
            .labels
            .iter()
            .find_map(|label| label.strip_prefix("home-stack:"))
            .unwrap_or(self.domain.as_str());

        let mut secrets = json!({});
        if let Some(secret_name) = request
            .secret_name
            .as_deref()
            .filter(|value| !value.is_empty())
        {
            secrets["secretName"] = json!(secret_name);
        }
        if let Some(auth_json_secret) = request
            .auth_json_secret
            .as_deref()
            .filter(|value| !value.is_empty())
        {
            secrets["authJsonSecret"] = json!(auth_json_secret);
        }

        json!({
            "apiVersion": "styrene.sh/v1alpha1",
            "kind": "OmegonAgent",
            "metadata": {
                "name": name,
                "namespace": namespace,
                "labels": {
                    "app.kubernetes.io/part-of": "auspex-managed-agents",
                    "styrene.sh/agent-package": self.id,
                    "styrene.sh/agent-role": self.role,
                    "styrene.sh/home-stack": stack_label,
                },
                "annotations": {
                    "auspex.styrene.sh/package-name": self.name,
                    "auspex.styrene.sh/package-profile": self.profile,
                    "auspex.styrene.sh/package-agent": self.agent,
                }
            },
            "spec": {
                "agent": self.agent,
                "model": model,
                "posture": self.posture,
                "role": self.role,
                "mode": self.mode,
                "image": image,
                "profile": self.profile,
                "terminalTool": self.terminal_tool,
                "vox": {
                    "connectors": request.connectors
                        .iter()
                        .map(|connector| connector.trim())
                        .filter(|connector| !connector.is_empty())
                        .collect::<Vec<_>>()
                },
                "secrets": secrets,
                "identity": {
                    "provision": true,
                    "securityTier": "file",
                    "meshRole": if self.mesh_role.is_empty() { "monitor" } else { self.mesh_role.as_str() },
                    "mtls": true
                },
                "controlPlane": {
                    "tls": {
                        "enabled": true,
                        "profile": if self.control_tls_profile.is_empty() { self.id.as_str() } else { self.control_tls_profile.as_str() }
                    }
                },
                "resources": {
                    "cpu": self.resources.cpu.as_deref().unwrap_or("500m"),
                    "memory": self.resources.memory.as_deref().unwrap_or("768Mi")
                }
            }
        })
    }

    /// Build an OCI launch spec for the Docker/Podman backend.
    ///
    /// This is the OCI analogue of [`AgentPackage::omegon_agent_manifest`]: it
    /// preserves the package/profile/model identity, maps the selected host
    /// port to the agent control-plane port, and marks the container with
    /// Auspex discovery labels.  Secrets are intentionally not materialised
    /// here; callers should inject resolved secret values through the launch
    /// environment after policy checks.
    #[cfg(not(target_arch = "wasm32"))]
    pub fn oci_launch_spec(
        &self,
        request: &AgentPackageDeployRequest,
        host_port: u16,
    ) -> OciLaunchSpec {
        let name = request.name.as_deref().unwrap_or(self.id.as_str());
        let image = request.image.as_deref().unwrap_or(self.image.as_str());
        let model = request
            .model
            .as_deref()
            .unwrap_or(self.default_model.as_str());

        let mut extra_labels = HashMap::new();
        extra_labels.insert(LABEL_PACKAGE_ID.to_string(), self.id.clone());
        extra_labels.insert("styrene.sh/agent-role".to_string(), self.role.clone());
        extra_labels.insert("styrene.sh/agent-domain".to_string(), self.domain.clone());
        for label in &self.labels {
            if let Some((key, value)) = label.split_once(':') {
                extra_labels.insert(format!("styrene.sh/{key}"), value.to_string());
            }
        }

        let mut env = vec![
            ("OMEGON_AGENT".to_string(), self.agent.clone()),
            ("OMEGON_PROFILE".to_string(), self.profile.clone()),
            ("OMEGON_MODEL".to_string(), model.to_string()),
            ("OMEGON_POSTURE".to_string(), self.posture.clone()),
            ("OMEGON_ROLE".to_string(), self.role.clone()),
            ("OMEGON_MODE".to_string(), self.mode.clone()),
            (
                "OMEGON_TERMINAL_TOOL".to_string(),
                self.terminal_tool.to_string(),
            ),
        ];
        let connectors = request
            .connectors
            .iter()
            .map(|connector| connector.trim())
            .filter(|connector| !connector.is_empty())
            .collect::<Vec<_>>();
        if !connectors.is_empty() {
            env.push(("OMEGON_VOX_CONNECTORS".to_string(), connectors.join(",")));
        }

        OciLaunchSpec {
            image: image.to_string(),
            name: name.to_string(),
            host_port,
            env,
            extra_labels,
            resources: Some(ResourceRequirements {
                cpu: self.resources.cpu.clone(),
                memory: self.resources.memory.clone(),
            }),
            pull_policy: PullPolicy::IfNotPresent,
            registry_auth: None,
        }
    }
}

/// Built-in packages used before Armory/Signum package discovery is live.
pub fn builtin_home_packages() -> Vec<AgentPackage> {
    vec![
        AgentPackage {
            id: "home-media-operator".into(),
            name: "Home Media Operator".into(),
            description: "Long-running operator for Jellyfin, Jellyseerr, Arr services, downloaders, and media namespace health.".into(),
            domain: "ops".into(),
            agent: "styrene.home-media-operator".into(),
            profile: "styrene-lab/omegon-home-media-operator".into(),
            default_model: default_model(),
            posture: "fabricator".into(),
            role: "detached-service".into(),
            mode: "daemon".into(),
            image: default_image(),
            labels: vec!["home-stack:media".into()],
            required_secrets: vec!["ANTHROPIC_API_KEY".into()],
            optional_secrets: vec![
                "JELLYFIN_API_KEY".into(),
                "JELLYSEERR_API_KEY".into(),
                "RADARR_API_KEY".into(),
                "SONARR_API_KEY".into(),
                "PROWLARR_API_KEY".into(),
                "QBITTORRENT_USERNAME".into(),
                "QBITTORRENT_PASSWORD".into(),
                "SABNZBD_API_KEY".into(),
            ],
            resources: PackageResources {
                cpu: Some("500m".into()),
                memory: Some("768Mi".into()),
            },
            control_tls_profile: "home-media".into(),
            mesh_role: "operator".into(),
            terminal_tool: false,
        },
        AgentPackage {
            id: "home-infra-sentinel".into(),
            name: "Home Infra Sentinel".into(),
            description: "Long-running sentinel for Brutus cluster health, ingress, identity, backups, and GitOps drift.".into(),
            domain: "infra".into(),
            agent: "styrene.home-infra-sentinel".into(),
            profile: "styrene-lab/omegon-home-infra-sentinel".into(),
            default_model: default_model(),
            posture: "explorator".into(),
            role: "detached-service".into(),
            mode: "daemon".into(),
            image: default_image(),
            labels: vec!["home-stack:infra".into()],
            required_secrets: vec!["ANTHROPIC_API_KEY".into()],
            optional_secrets: vec!["GITHUB_TOKEN".into(), "VAULT_ADDR".into()],
            resources: PackageResources {
                cpu: Some("500m".into()),
                memory: Some("768Mi".into()),
            },
            control_tls_profile: "home-infra".into(),
            mesh_role: "monitor".into(),
            terminal_tool: false,
        },
        AgentPackage {
            id: "home-forge-steward".into(),
            name: "Home Forge Steward".into(),
            description: "Long-running steward for Styrene Forgejo, GitOps, image builds, package publication, and release readiness.".into(),
            domain: "ops".into(),
            agent: "styrene.home-forge-steward".into(),
            profile: "styrene-lab/omegon-home-forge-steward".into(),
            default_model: default_model(),
            posture: "fabricator".into(),
            role: "detached-service".into(),
            mode: "daemon".into(),
            image: default_image(),
            labels: vec!["home-stack:forge".into()],
            required_secrets: vec!["ANTHROPIC_API_KEY".into()],
            optional_secrets: vec![
                "GITHUB_TOKEN".into(),
                "FORGEJO_TOKEN".into(),
                "COSIGN_EXPERIMENTAL".into(),
            ],
            resources: PackageResources {
                cpu: Some("500m".into()),
                memory: Some("768Mi".into()),
            },
            control_tls_profile: "home-forge".into(),
            mesh_role: "operator".into(),
            terminal_tool: false,
        },
        AgentPackage {
            id: "home-knowledge-curator".into(),
            name: "Home Knowledge Curator".into(),
            description: "Long-running curator for home operations runbooks, incident summaries, boards, and handoff context.".into(),
            domain: "ops".into(),
            agent: "styrene.home-knowledge-curator".into(),
            profile: "styrene-lab/omegon-home-knowledge-curator".into(),
            default_model: default_model(),
            posture: "fabricator".into(),
            role: "detached-service".into(),
            mode: "daemon".into(),
            image: default_image(),
            labels: vec!["home-stack:knowledge".into()],
            required_secrets: vec!["ANTHROPIC_API_KEY".into()],
            optional_secrets: vec!["GITHUB_TOKEN".into()],
            resources: PackageResources {
                cpu: Some("350m".into()),
                memory: Some("512Mi".into()),
            },
            control_tls_profile: "home-knowledge".into(),
            mesh_role: "monitor".into(),
            terminal_tool: false,
        },
    ]
}

pub fn builtin_agent_packages() -> Vec<AgentPackage> {
    builtin_home_packages()
}

pub fn find_builtin_agent_package(id: &str) -> Option<AgentPackage> {
    builtin_agent_packages()
        .into_iter()
        .find(|package| package.id == id || package.agent == id)
}

fn default_model() -> String {
    "anthropic:claude-sonnet-4-6".into()
}

fn default_image() -> String {
    "ghcr.io/styrene-lab/omegon:0.26.5".into()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builtin_home_packages_have_distinct_agent_ids() {
        let packages = builtin_home_packages();
        let ids: std::collections::BTreeSet<_> = packages
            .iter()
            .map(|package| package.agent.as_str())
            .collect();

        assert_eq!(packages.len(), 4);
        assert_eq!(ids.len(), 4);
        assert!(ids.contains("styrene.home-media-operator"));
    }

    #[test]
    fn package_manifest_preserves_package_identity_and_secret_reference() {
        let package = find_builtin_agent_package("home-media-operator").expect("package");
        let manifest = package.omegon_agent_manifest(&AgentPackageDeployRequest {
            name: Some("media-watch".into()),
            namespace: Some("omegon-agents".into()),
            secret_name: Some("media-agent-secrets".into()),
            auth_json_secret: Some("media-auth-json".into()),
            connectors: vec!["aether".into(), "slack".into()],
            ..Default::default()
        });

        assert_eq!(manifest["metadata"]["name"], "media-watch");
        assert_eq!(
            manifest["metadata"]["labels"]["styrene.sh/agent-package"],
            "home-media-operator"
        );
        assert_eq!(manifest["spec"]["agent"], "styrene.home-media-operator");
        assert_eq!(manifest["spec"]["mode"], "daemon");
        assert_eq!(
            manifest["spec"]["secrets"]["secretName"],
            "media-agent-secrets"
        );
        assert_eq!(
            manifest["spec"]["secrets"]["authJsonSecret"],
            "media-auth-json"
        );
        assert_eq!(manifest["spec"]["vox"]["connectors"][0], "aether");
        assert_eq!(manifest["spec"]["vox"]["connectors"][1], "slack");
        assert_eq!(manifest["spec"]["controlPlane"]["tls"]["enabled"], true);
    }

    #[test]
    fn package_oci_launch_spec_preserves_runtime_identity() {
        let package = find_builtin_agent_package("home-infra-sentinel").expect("package");
        let spec = package.oci_launch_spec(
            &AgentPackageDeployRequest {
                name: Some("infra-watch".into()),
                image: Some("ghcr.io/styrene-lab/custom-agent:dev".into()),
                model: Some("anthropic:claude-haiku".into()),
                connectors: vec![" aether ".into(), "".into(), "slack".into()],
                ..Default::default()
            },
            7901,
        );

        assert_eq!(spec.name, "infra-watch");
        assert_eq!(spec.image, "ghcr.io/styrene-lab/custom-agent:dev");
        assert_eq!(spec.host_port, 7901);
        assert_eq!(
            spec.extra_labels[crate::oci_backend::LABEL_PACKAGE_ID],
            "home-infra-sentinel"
        );
        assert_eq!(
            spec.extra_labels["styrene.sh/home-stack"],
            "infra"
        );
        assert!(spec.env.contains(&(
            "OMEGON_AGENT".into(),
            "styrene.home-infra-sentinel".into()
        )));
        assert!(spec.env.contains(&(
            "OMEGON_MODEL".into(),
            "anthropic:claude-haiku".into()
        )));
        assert!(spec.env.contains(&(
            "OMEGON_VOX_CONNECTORS".into(),
            "aether,slack".into()
        )));
        assert_eq!(spec.resources.as_ref().unwrap().memory.as_deref(), Some("768Mi"));
        assert_eq!(spec.pull_policy, crate::oci_backend::PullPolicy::IfNotPresent);
    }

    #[test]
    fn oci_image_assessment_distinguishes_digest_from_mutable_tag() {
        let digest = "ghcr.io/styrene-lab/omegon-agents@sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
        let assessment = assess_oci_image_ref(digest);

        assert_eq!(assessment.kind, OciImageReferenceKind::Digest);
        assert!(assessment.digest_pinned);
        assert!(!assessment.mutable_tag);
        assert_eq!(
            assessment.digest.as_deref(),
            Some("sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa")
        );

        let tagged = assess_oci_image_ref("ghcr.io/styrene-lab/omegon:0.26.5");
        assert_eq!(tagged.kind, OciImageReferenceKind::Tag);
        assert!(!tagged.digest_pinned);
        assert!(tagged.mutable_tag);
        assert_eq!(tagged.tag.as_deref(), Some("0.26.5"));
        assert!(!tagged.warnings.is_empty());
    }

    #[test]
    fn oci_image_assessment_rejects_malformed_digest() {
        let assessment = assess_oci_image_ref("ghcr.io/example/agent@sha256:ABC");

        assert_eq!(assessment.kind, OciImageReferenceKind::Invalid);
        assert!(!assessment.errors.is_empty());
    }
}
