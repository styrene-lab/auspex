//! Read-only Armory catalog discovery and deploy planning.
//!
//! Armory owns reusable package intent. Auspex may discover and plan from that
//! intent, but package metadata is not authority to mutate a runtime. This
//! module keeps the boundary explicit: fetch/list/resolve packages, produce a
//! dry-run install plan, and require a separate Auspex overlay before an
//! Armory package can become an `AgentPackage`.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::agent_packages::{AgentPackage, PackageResources};

pub const DEFAULT_ARMORY_INDEX_URL: &str = "https://armory.styrene.io/api/index.json";
pub const DEFAULT_ARMORY_SCHEMA_URL: &str = "https://armory.styrene.io/api/schema.json";

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ArmoryIndex {
    pub generated_at: String,
    #[serde(default)]
    pub registry: Option<String>,
    #[serde(default)]
    pub items: Vec<ArmoryPackage>,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

impl ArmoryIndex {
    pub fn from_json(json: &str) -> Result<Self, ArmoryError> {
        let index: Self = serde_json::from_str(json)
            .map_err(|error| ArmoryError::MalformedIndex(error.to_string()))?;
        if index.generated_at.trim().is_empty() {
            return Err(ArmoryError::MalformedIndex(
                "generatedAt must not be empty".into(),
            ));
        }
        Ok(index)
    }

    pub fn package_refs(&self) -> impl Iterator<Item = &str> {
        self.items.iter().map(|item| item.package_ref.as_str())
    }

    pub fn get(&self, package_ref_or_id: &str) -> Option<&ArmoryPackage> {
        self.items
            .iter()
            .find(|item| item.package_ref == package_ref_or_id || item.id == package_ref_or_id)
    }

    pub fn filter(&self, filter: ArmoryPackageFilter<'_>) -> Vec<&ArmoryPackage> {
        self.items
            .iter()
            .filter(|item| {
                filter.kind.is_none_or(|kind| item.kind == kind)
                    && filter
                        .distribution
                        .is_none_or(|distribution| item.distribution == distribution)
                    && filter
                        .publisher
                        .is_none_or(|publisher| item.publisher.eq_ignore_ascii_case(publisher))
                    && filter
                        .max_compatibility_tier
                        .is_none_or(|tier| item.compatibility.tier <= tier)
            })
            .collect()
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ArmoryPackageFilter<'a> {
    pub kind: Option<ArmoryPackageKind>,
    pub distribution: Option<ArmoryDistribution>,
    pub publisher: Option<&'a str>,
    pub max_compatibility_tier: Option<u8>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ArmoryPackage {
    pub kind: ArmoryPackageKind,
    pub id: String,
    pub package_ref: String,
    pub name: String,
    pub version: String,
    pub description: String,
    pub category: String,
    #[serde(default)]
    pub source_path: String,
    #[serde(default)]
    pub source_url: String,
    #[serde(default)]
    pub repository_url: String,
    #[serde(default)]
    pub homepage_url: String,
    #[serde(default)]
    pub armory_url: String,
    #[serde(default)]
    pub install_command: String,
    #[serde(default)]
    pub install_note: String,
    #[serde(default)]
    pub verify_command: String,
    #[serde(default)]
    pub oci_ref: String,
    #[serde(default)]
    pub artifact_type: String,
    #[serde(default)]
    pub payload_digest: String,
    #[serde(default)]
    pub manifest_id: String,
    #[serde(default)]
    pub license: String,
    #[serde(default)]
    pub min_omegon: String,
    #[serde(default)]
    pub min_nex: String,
    #[serde(default)]
    pub canonical_format: String,
    #[serde(default)]
    pub destructive_capabilities: Vec<String>,
    #[serde(default)]
    pub network_requirements: Vec<String>,
    #[serde(default)]
    pub publisher: String,
    #[serde(default)]
    pub official: bool,
    #[serde(default)]
    pub capabilities: Vec<String>,
    #[serde(default)]
    pub keywords: Vec<String>,
    #[serde(default)]
    pub files: Vec<String>,
    #[serde(default)]
    pub dependencies: Vec<ArmoryDependency>,
    pub distribution: ArmoryDistribution,
    #[serde(default)]
    pub compatibility: ArmoryCompatibility,
    #[serde(default)]
    pub interfaces: BTreeMap<String, ArmoryInterfaceDetails>,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ArmoryPackageKind {
    Skill,
    Persona,
    Tone,
    Profile,
    Agent,
    Extension,
    ForgeTemplate,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ArmoryDistribution {
    Oci,
    Registry,
    External,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ArmoryCompatibility {
    #[serde(default)]
    pub tier: u8,
    #[serde(default)]
    pub native: Vec<ArmoryCompatibilityMode>,
    #[serde(default)]
    pub degraded: Vec<ArmoryCompatibilityMode>,
    #[serde(default)]
    pub notes: Vec<String>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ArmoryCompatibilityMode {
    pub runtime: String,
    pub mode: String,
    #[serde(default)]
    pub install_command: String,
    #[serde(default)]
    pub entrypoints: Vec<String>,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ArmoryDependency {
    pub kind: ArmoryPackageKind,
    pub id: String,
    #[serde(default)]
    pub version: String,
    #[serde(default)]
    pub required: bool,
    #[serde(default)]
    pub enabled: bool,
    #[serde(default)]
    pub install_command: String,
    #[serde(default)]
    pub compatibility: ArmoryDependencyCompatibility,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ArmoryDependencyCompatibility {
    #[serde(default)]
    pub tier: u8,
    #[serde(default)]
    pub mode: String,
    #[serde(default)]
    pub native_only: bool,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ArmoryInterfaceDetails {
    #[serde(default)]
    pub status: String,
    #[serde(default)]
    pub install: String,
    #[serde(default)]
    pub binary: String,
    #[serde(default)]
    pub image: String,
    #[serde(default)]
    pub commands: Vec<String>,
    #[serde(default)]
    pub tools: Vec<String>,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

#[derive(Debug, thiserror::Error)]
pub enum ArmoryError {
    #[error("armory index fetch failed: {0}")]
    Fetch(String),
    #[error("armory index malformed: {0}")]
    MalformedIndex(String),
    #[error("unknown armory package '{0}'")]
    UnknownPackage(String),
    #[error("deployment overlay invalid: {0}")]
    InvalidOverlay(String),
}

#[derive(Clone, Debug)]
pub struct ArmoryClient {
    index_url: String,
    schema_url: Option<String>,
    cached: Option<ArmoryCachedIndex>,
}

#[derive(Clone, Debug)]
pub struct ArmoryCachedIndex {
    pub index: ArmoryIndex,
    pub etag: Option<String>,
    pub last_modified: Option<String>,
}

impl Default for ArmoryClient {
    fn default() -> Self {
        Self::new(DEFAULT_ARMORY_INDEX_URL)
    }
}

impl ArmoryClient {
    pub fn new(index_url: impl Into<String>) -> Self {
        Self {
            index_url: index_url.into(),
            schema_url: Some(DEFAULT_ARMORY_SCHEMA_URL.into()),
            cached: None,
        }
    }

    pub fn with_schema_url(mut self, schema_url: impl Into<Option<String>>) -> Self {
        self.schema_url = schema_url.into();
        self
    }

    pub fn index_url(&self) -> &str {
        &self.index_url
    }

    pub fn schema_url(&self) -> Option<&str> {
        self.schema_url.as_deref()
    }

    pub fn cached_index(&self) -> Option<&ArmoryCachedIndex> {
        self.cached.as_ref()
    }

    pub async fn fetch_index(&mut self) -> Result<&ArmoryIndex, ArmoryError> {
        let client = reqwest::Client::new();
        let mut request = client.get(&self.index_url);
        if let Some(cached) = self.cached.as_ref() {
            if let Some(etag) = cached.etag.as_deref() {
                request = request.header(reqwest::header::IF_NONE_MATCH, etag);
            }
            if let Some(last_modified) = cached.last_modified.as_deref() {
                request = request.header(reqwest::header::IF_MODIFIED_SINCE, last_modified);
            }
        }
        let response = request
            .send()
            .await
            .map_err(|error| ArmoryError::Fetch(error.to_string()))?;
        if response.status() == reqwest::StatusCode::NOT_MODIFIED {
            return self
                .cached
                .as_ref()
                .map(|cached| &cached.index)
                .ok_or_else(|| ArmoryError::Fetch("304 response without cached index".into()));
        }
        let response = response
            .error_for_status()
            .map_err(|error| ArmoryError::Fetch(error.to_string()))?;
        let etag = response
            .headers()
            .get(reqwest::header::ETAG)
            .and_then(|value| value.to_str().ok())
            .map(str::to_string);
        let last_modified = response
            .headers()
            .get(reqwest::header::LAST_MODIFIED)
            .and_then(|value| value.to_str().ok())
            .map(str::to_string);
        let body = response
            .text()
            .await
            .map_err(|error| ArmoryError::Fetch(error.to_string()))?;
        let index = ArmoryIndex::from_json(&body)?;
        self.cached = Some(ArmoryCachedIndex {
            index,
            etag,
            last_modified,
        });
        Ok(&self.cached.as_ref().expect("cache populated").index)
    }

    pub fn list_cached(&self) -> &[ArmoryPackage] {
        self.cached
            .as_ref()
            .map(|cached| cached.index.items.as_slice())
            .unwrap_or(&[])
    }

    pub fn get_cached(&self, package_ref_or_id: &str) -> Option<&ArmoryPackage> {
        self.cached
            .as_ref()
            .and_then(|cached| cached.index.get(package_ref_or_id))
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArmoryPlanOptions {
    #[serde(default)]
    pub include_optional: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArmoryInstallPlan {
    pub package_ref: String,
    pub package_id: String,
    pub kind: ArmoryPackageKind,
    pub distribution: ArmoryDistribution,
    pub pull_artifacts: Vec<OciArtifact>,
    pub omegon_plugins: Vec<OmegonPluginInstall>,
    pub omegon_extensions: Vec<OmegonExtensionInstall>,
    pub external_integrations: Vec<ExternalIntegration>,
    pub nex_forge_templates: Vec<NexForgeTemplate>,
    pub required_secrets: Vec<String>,
    pub optional_secrets: Vec<String>,
    pub warnings: Vec<String>,
    pub policy_gates: Vec<PolicyGate>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct OciArtifact {
    pub oci_ref: String,
    pub artifact_type: String,
    pub payload_digest: String,
    pub verify_command: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct OmegonPluginInstall {
    pub package_ref: String,
    pub entrypoints: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct OmegonExtensionInstall {
    pub id: String,
    pub version: String,
    pub install_command: String,
    pub required: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExternalIntegration {
    pub id: String,
    pub interfaces: BTreeMap<String, ArmoryInterfaceDetails>,
    pub install_command: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct NexForgeTemplate {
    pub package_ref: String,
    pub canonical_format: String,
    pub min_nex: String,
    pub destructive_capabilities: Vec<String>,
    pub network_requirements: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PolicyGate {
    pub operation: String,
    pub reason: String,
    pub severity: PolicySeverity,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum PolicySeverity {
    Info,
    ApprovalRequired,
    Blocked,
}

pub fn plan_armory_install(
    package: &ArmoryPackage,
    options: ArmoryPlanOptions,
) -> ArmoryInstallPlan {
    let mut plan = ArmoryInstallPlan {
        package_ref: package.package_ref.clone(),
        package_id: package.id.clone(),
        kind: package.kind,
        distribution: package.distribution,
        pull_artifacts: Vec::new(),
        omegon_plugins: Vec::new(),
        omegon_extensions: Vec::new(),
        external_integrations: Vec::new(),
        nex_forge_templates: Vec::new(),
        required_secrets: Vec::new(),
        optional_secrets: Vec::new(),
        warnings: Vec::new(),
        policy_gates: Vec::new(),
    };

    if package.distribution == ArmoryDistribution::Oci && !package.oci_ref.is_empty() {
        plan.pull_artifacts.push(OciArtifact {
            oci_ref: package.oci_ref.clone(),
            artifact_type: package.artifact_type.clone(),
            payload_digest: package.payload_digest.clone(),
            verify_command: package.verify_command.clone(),
        });
    }

    match (package.kind, package.distribution) {
        (ArmoryPackageKind::Agent | ArmoryPackageKind::Profile, ArmoryDistribution::Oci) => {
            plan.omegon_plugins.push(OmegonPluginInstall {
                package_ref: package.package_ref.clone(),
                entrypoints: package.files.clone(),
            });
        }
        (ArmoryPackageKind::Extension, ArmoryDistribution::Registry) => {
            plan.omegon_extensions.push(OmegonExtensionInstall {
                id: package.id.clone(),
                version: package.version.clone(),
                install_command: package.install_command.clone(),
                required: true,
            });
            plan.policy_gates.push(PolicyGate {
                operation: format!("install native extension {}", package.id),
                reason: "Native Omegon extension installs change runtime authority.".into(),
                severity: PolicySeverity::ApprovalRequired,
            });
        }
        (ArmoryPackageKind::Extension, ArmoryDistribution::External) => {
            plan.external_integrations.push(ExternalIntegration {
                id: package.id.clone(),
                interfaces: package.interfaces.clone(),
                install_command: package.install_command.clone(),
            });
            plan.policy_gates.push(PolicyGate {
                operation: format!("link external integration {}", package.id),
                reason: "External integrations are deployed or linked separately from Omegon extension installs.".into(),
                severity: PolicySeverity::ApprovalRequired,
            });
        }
        (ArmoryPackageKind::ForgeTemplate, _) => {
            plan.nex_forge_templates.push(NexForgeTemplate {
                package_ref: package.package_ref.clone(),
                canonical_format: package.canonical_format.clone(),
                min_nex: package.min_nex.clone(),
                destructive_capabilities: package.destructive_capabilities.clone(),
                network_requirements: package.network_requirements.clone(),
            });
            plan.policy_gates.push(PolicyGate {
                operation: format!("validate Nex forge template {}", package.package_ref),
                reason: "Forge templates are Nex-owned and require semantic validation before build or provisioning.".into(),
                severity: PolicySeverity::ApprovalRequired,
            });
            if !package.destructive_capabilities.is_empty() {
                plan.policy_gates.push(PolicyGate {
                    operation: format!("execute Nex forge template {}", package.package_ref),
                    reason: format!(
                        "Template declares destructive capabilities: {}",
                        package.destructive_capabilities.join(", ")
                    ),
                    severity: PolicySeverity::Blocked,
                });
            }
        }
        _ => {}
    }

    for dependency in &package.dependencies {
        if !dependency.required && !options.include_optional {
            continue;
        }
        if dependency.required {
            plan.required_secrets
                .extend(secret_names_for_dependency(dependency));
        } else {
            plan.optional_secrets
                .extend(secret_names_for_dependency(dependency));
        }

        match (dependency.kind, dependency.compatibility.native_only) {
            (ArmoryPackageKind::Extension, true) => {
                plan.omegon_extensions.push(OmegonExtensionInstall {
                    id: dependency.id.clone(),
                    version: dependency.version.clone(),
                    install_command: dependency.install_command.clone(),
                    required: dependency.required,
                });
                plan.policy_gates.push(PolicyGate {
                    operation: format!("install native extension {}", dependency.id),
                    reason: "Dependency is native-only and must not be installed without explicit policy approval.".into(),
                    severity: PolicySeverity::ApprovalRequired,
                });
            }
            (ArmoryPackageKind::ForgeTemplate, _) => {
                plan.policy_gates.push(PolicyGate {
                    operation: format!("delegate forge template dependency {}", dependency.id),
                    reason:
                        "Forge template dependency must be handled by Nex validation/build paths."
                            .into(),
                    severity: PolicySeverity::ApprovalRequired,
                });
            }
            _ => {}
        }
    }

    if plan
        .pull_artifacts
        .iter()
        .any(|artifact| !artifact.oci_ref.is_empty())
    {
        plan.policy_gates.push(PolicyGate {
            operation: "pull and verify OCI artifact".into(),
            reason: "Artifact pulls affect supply-chain state and should be audited.".into(),
            severity: PolicySeverity::ApprovalRequired,
        });
    }
    plan.required_secrets.sort();
    plan.required_secrets.dedup();
    plan.optional_secrets.sort();
    plan.optional_secrets.dedup();
    plan
}

fn secret_names_for_dependency(dependency: &ArmoryDependency) -> Vec<String> {
    dependency
        .extra
        .get("secrets")
        .and_then(Value::as_array)
        .map(|values| {
            values
                .iter()
                .filter_map(Value::as_str)
                .map(str::to_string)
                .collect()
        })
        .unwrap_or_default()
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArmoryDeploymentOverlay {
    pub id: String,
    pub armory: String,
    pub mode: String,
    pub role: String,
    pub image: String,
    pub model: String,
    #[serde(default)]
    pub posture: String,
    #[serde(default)]
    pub resources: PackageResources,
    #[serde(default)]
    pub namespace: Option<String>,
    #[serde(default)]
    pub required_secrets: Vec<String>,
    #[serde(default)]
    pub optional_secrets: Vec<String>,
    #[serde(default)]
    pub control_tls_profile: String,
    #[serde(default)]
    pub mesh_role: String,
    #[serde(default, rename = "terminalTool", alias = "terminal_tool")]
    pub terminal_tool: bool,
}

impl ArmoryDeploymentOverlay {
    pub fn validate(&self) -> Result<(), ArmoryError> {
        let required = [
            ("id", self.id.as_str()),
            ("armory", self.armory.as_str()),
            ("mode", self.mode.as_str()),
            ("role", self.role.as_str()),
            ("image", self.image.as_str()),
            ("model", self.model.as_str()),
        ];
        for (field, value) in required {
            if value.trim().is_empty() {
                return Err(ArmoryError::InvalidOverlay(format!(
                    "{field} must be provided"
                )));
            }
        }
        Ok(())
    }
}

pub fn agent_package_from_armory_overlay(
    package: &ArmoryPackage,
    overlay: &ArmoryDeploymentOverlay,
    plan: &ArmoryInstallPlan,
) -> Result<AgentPackage, ArmoryError> {
    overlay.validate()?;
    if overlay.armory != package.package_ref && overlay.armory != package.id {
        return Err(ArmoryError::InvalidOverlay(format!(
            "overlay references {}, but package is {}",
            overlay.armory, package.package_ref
        )));
    }
    if !matches!(
        package.kind,
        ArmoryPackageKind::Agent | ArmoryPackageKind::Profile
    ) {
        return Err(ArmoryError::InvalidOverlay(format!(
            "only agent/profile packages can map to AgentPackage, got {:?}",
            package.kind
        )));
    }

    let mut required_secrets = plan.required_secrets.clone();
    required_secrets.extend(overlay.required_secrets.clone());
    required_secrets.sort();
    required_secrets.dedup();
    let mut optional_secrets = plan.optional_secrets.clone();
    optional_secrets.extend(overlay.optional_secrets.clone());
    optional_secrets.sort();
    optional_secrets.dedup();

    Ok(AgentPackage {
        id: overlay.id.clone(),
        name: package.name.clone(),
        description: package.description.clone(),
        domain: package.category.clone(),
        agent: package.id.clone(),
        profile: package.package_ref.clone(),
        default_model: overlay.model.clone(),
        posture: if overlay.posture.is_empty() {
            "fabricator".into()
        } else {
            overlay.posture.clone()
        },
        role: overlay.role.clone(),
        mode: overlay.mode.clone(),
        image: overlay.image.clone(),
        labels: vec![format!("armory:{}", package.package_ref)],
        required_secrets,
        optional_secrets,
        resources: overlay.resources.clone(),
        control_tls_profile: overlay.control_tls_profile.clone(),
        mesh_role: overlay.mesh_role.clone(),
        terminal_tool: overlay.terminal_tool,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const FIXTURE: &str = r#"{
      "generatedAt": "2026-05-19T17:12:39+00:00",
      "registry": "styrene",
      "items": [
        {
          "kind": "profile",
          "id": "security-review",
          "packageRef": "profile/security-review",
          "name": "Security Review",
          "version": "1.0.0",
          "description": "Security review profile",
          "category": "security",
          "sourcePath": "profiles/security-review",
          "sourceUrl": "https://example.test/security-review",
          "repositoryUrl": "https://example.test/repo",
          "homepageUrl": "https://example.test/home",
          "armoryUrl": "https://example.test/armory",
          "installCommand": "omegon profile install",
          "installNote": "install profile",
          "verifyCommand": "cosign verify example/profile",
          "ociRef": "ghcr.io/styrene-lab/omegon-armory/profile/security-review:1.0.0",
          "artifactType": "application/vnd.styrene.omegon.profile.v1+tar",
          "payloadDigest": "sha256:profile",
          "manifestId": "security-review",
          "license": "MIT",
          "publisher": "Styrene Lab",
          "official": true,
          "capabilities": ["security"],
          "keywords": ["security"],
          "files": ["PERSONA.md"],
          "dependencies": [
            {
              "kind": "extension",
              "id": "flynt",
              "version": ">=0.3.0",
              "required": false,
              "enabled": false,
              "installCommand": "omegon extension install flynt",
              "compatibility": { "tier": 0, "mode": "extension", "nativeOnly": true }
            }
          ],
          "distribution": "oci",
          "compatibility": {
            "tier": 2,
            "native": [{ "runtime": "omegon", "mode": "profile", "installCommand": "omegon profile install" }],
            "degraded": [{ "runtime": "generic-agent", "mode": "prompt", "entrypoints": ["PERSONA.md"] }],
            "notes": []
          },
          "unexpectedFutureField": { "kept": true }
        },
        {
          "kind": "extension",
          "id": "lookout",
          "packageRef": "extension/lookout",
          "name": "Lookout",
          "version": "1.0.0",
          "description": "External monitor",
          "category": "security",
          "sourcePath": "extensions/lookout",
          "sourceUrl": "",
          "repositoryUrl": "",
          "homepageUrl": "",
          "armoryUrl": "",
          "installCommand": "lookout install",
          "installNote": "",
          "verifyCommand": "",
          "ociRef": "",
          "artifactType": "",
          "payloadDigest": "",
          "manifestId": "lookout",
          "license": "MIT",
          "publisher": "Styrene Lab",
          "official": true,
          "capabilities": [],
          "keywords": [],
          "files": [],
          "dependencies": [],
          "distribution": "external",
          "compatibility": { "tier": 1, "native": [], "degraded": [], "notes": [] },
          "interfaces": { "cli": { "status": "supported", "binary": "lookout" }, "oci": { "status": "supported", "image": "example/lookout:latest" } }
        },
        {
          "kind": "forge-template",
          "id": "minimal-workstation",
          "packageRef": "forge-template/minimal-workstation",
          "name": "Minimal Workstation",
          "version": "1.0.0",
          "description": "Nex workstation template",
          "category": "forge",
          "sourcePath": "forge/minimal-workstation",
          "sourceUrl": "",
          "repositoryUrl": "",
          "homepageUrl": "",
          "armoryUrl": "",
          "installCommand": "nex forge validate",
          "installNote": "",
          "verifyCommand": "",
          "ociRef": "",
          "artifactType": "",
          "payloadDigest": "",
          "manifestId": "minimal-workstation",
          "license": "MIT",
          "minNex": "0.6.0",
          "canonicalFormat": "pkl",
          "destructiveCapabilities": ["disk-write"],
          "networkRequirements": ["github.com"],
          "publisher": "Styrene Lab",
          "official": true,
          "capabilities": [],
          "keywords": [],
          "files": [],
          "dependencies": [],
          "distribution": "oci",
          "compatibility": { "tier": 1, "native": [], "degraded": [], "notes": [] }
        }
      ]
    }"#;

    #[test]
    fn index_parses_unknown_fields_and_resolves_refs() {
        let index = ArmoryIndex::from_json(FIXTURE).expect("index");

        assert_eq!(index.items.len(), 3);
        assert!(index.get("profile/security-review").is_some());
        assert!(index.get("security-review").is_some());
        assert!(
            index
                .get("profile/security-review")
                .expect("profile")
                .extra
                .contains_key("unexpectedFutureField")
        );
    }

    #[test]
    fn index_rejects_malformed_json() {
        let error = ArmoryIndex::from_json(r#"{"items":[]}"#).expect_err("invalid");

        assert!(matches!(error, ArmoryError::MalformedIndex(_)));
    }

    #[test]
    fn plan_profile_keeps_optional_native_dependencies_out_by_default() {
        let index = ArmoryIndex::from_json(FIXTURE).expect("index");
        let package = index.get("profile/security-review").expect("package");

        let plan = plan_armory_install(package, ArmoryPlanOptions::default());

        assert_eq!(plan.pull_artifacts.len(), 1);
        assert_eq!(plan.omegon_plugins.len(), 1);
        assert!(plan.omegon_extensions.is_empty());
        assert!(
            plan.policy_gates
                .iter()
                .any(|gate| gate.operation == "pull and verify OCI artifact")
        );
    }

    #[test]
    fn plan_external_extension_does_not_emit_native_extension_install() {
        let index = ArmoryIndex::from_json(FIXTURE).expect("index");
        let package = index.get("extension/lookout").expect("package");

        let plan = plan_armory_install(package, ArmoryPlanOptions::default());

        assert!(plan.omegon_extensions.is_empty());
        assert_eq!(plan.external_integrations[0].id, "lookout");
        assert!(plan.external_integrations[0].interfaces.contains_key("cli"));
    }

    #[test]
    fn plan_forge_template_marks_nex_metadata_and_blocks_execution() {
        let index = ArmoryIndex::from_json(FIXTURE).expect("index");
        let package = index
            .get("forge-template/minimal-workstation")
            .expect("package");

        let plan = plan_armory_install(package, ArmoryPlanOptions::default());

        assert_eq!(plan.nex_forge_templates[0].min_nex, "0.6.0");
        assert_eq!(plan.nex_forge_templates[0].canonical_format, "pkl");
        assert!(
            plan.policy_gates
                .iter()
                .any(|gate| gate.severity == PolicySeverity::Blocked)
        );
    }

    #[test]
    fn overlay_maps_armory_profile_to_agent_package_without_guessing_runtime_fields() {
        let index = ArmoryIndex::from_json(FIXTURE).expect("index");
        let package = index.get("profile/security-review").expect("package");
        let plan = plan_armory_install(package, ArmoryPlanOptions::default());
        let overlay = ArmoryDeploymentOverlay {
            id: "security-review".into(),
            armory: "profile/security-review".into(),
            mode: "job".into(),
            role: "supervised-child".into(),
            image: "ghcr.io/styrene-lab/omegon-agents:0.23".into(),
            model: "anthropic:claude-sonnet-4-6".into(),
            required_secrets: vec!["ANTHROPIC_API_KEY".into()],
            ..Default::default()
        };

        let agent_package =
            agent_package_from_armory_overlay(package, &overlay, &plan).expect("agent package");

        assert_eq!(agent_package.id, "security-review");
        assert_eq!(agent_package.profile, "profile/security-review");
        assert_eq!(agent_package.required_secrets, vec!["ANTHROPIC_API_KEY"]);
    }

    #[test]
    fn overlay_validation_rejects_missing_runtime_fields() {
        let overlay = ArmoryDeploymentOverlay {
            id: "bad".into(),
            armory: "profile/security-review".into(),
            ..Default::default()
        };

        let error = overlay.validate().expect_err("invalid overlay");

        assert!(matches!(error, ArmoryError::InvalidOverlay(_)));
    }
}
