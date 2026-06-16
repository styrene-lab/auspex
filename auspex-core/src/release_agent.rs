//! Release-agent primitives for Styrene ecosystem release announcements.
//!
//! This module intentionally starts with a preview-only flow: GitHub release
//! metadata becomes a bounded post draft, and publication adapters can consume
//! that draft later. Keeping the first path side-effect free gives Auspex an
//! end-to-end test without needing Discord/Slack credentials.

use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
pub struct GitHubReleaseFixture {
    pub repo: String,
    pub tag: String,
    pub name: String,
    #[serde(default)]
    pub body: String,
    pub html_url: String,
    pub published_at: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct ReleasePreviewPost {
    pub repo: String,
    pub tag: String,
    pub title: String,
    pub body: String,
    pub targets: Vec<String>,
    pub source_url: String,
    pub dedupe_key: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ReleasePreviewArtifact {
    pub path: PathBuf,
    pub content: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ParsedReleasePreviewArtifact {
    pub title: String,
    pub repo: String,
    pub tag: String,
    pub source_url: String,
    pub dedupe_key: String,
    pub targets: Vec<String>,
    pub publish_state: String,
    pub approved_by: Option<String>,
    pub approved_at: Option<String>,
    pub body: String,
}

impl ParsedReleasePreviewArtifact {
    pub fn approval(&self) -> ReleasePreviewApproval {
        ReleasePreviewApproval {
            approved_by: self.approved_by.clone(),
            approved_at: self.approved_at.clone(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ReleasePreviewError {
    RepoNotAllowed { repo: String },
    NotReady { blockers: Vec<String> },
}

impl std::fmt::Display for ReleasePreviewError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::RepoNotAllowed { repo } => write!(f, "release repo is not allowlisted: {repo}"),
            Self::NotReady { blockers } => write!(
                f,
                "release-agent preview is not ready: {}",
                blockers.join(", ")
            ),
        }
    }
}

impl std::error::Error for ReleasePreviewError {}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ReleasePreviewApproval {
    pub approved_by: Option<String>,
    pub approved_at: Option<String>,
}

impl ReleasePreviewApproval {
    pub fn is_approved(&self) -> bool {
        self.approved_by
            .as_deref()
            .is_some_and(|operator| !operator.trim().is_empty())
            && self
                .approved_at
                .as_deref()
                .is_some_and(|timestamp| !timestamp.trim().is_empty())
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ReleaseAgentReadinessInputs {
    pub configured_secrets: BTreeSet<String>,
    pub model_available: bool,
    pub repo_allowlist: Vec<String>,
    pub preview_approval: ReleasePreviewApproval,
    pub publish_target: Option<ReleasePublishTarget>,
    pub execution_boundary: ReleaseAgentExecutionBoundary,
}

#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub enum ReleaseAgentExecutionBoundary {
    #[default]
    Inherited,
    Oci {
        image: Option<String>,
        substrate: Box<Option<crate::omegon_control::OmegonExecutionSubstrate>>,
    },
}

pub fn release_agent_execution_boundary_summary(
    boundary: &ReleaseAgentExecutionBoundary,
) -> String {
    match boundary {
        ReleaseAgentExecutionBoundary::Inherited => {
            "execution boundary: inherited runtime".to_string()
        }
        ReleaseAgentExecutionBoundary::Oci { image, substrate } => {
            let image = image
                .as_deref()
                .map(str::trim)
                .filter(|image| !image.is_empty());
            match substrate.as_ref() {
                Some(substrate) if substrate.is_host_native() && substrate.capabilities.has_host_runtime => {
                    format!(
                        "execution boundary: host-shim OCI via {}",
                        image.unwrap_or("<missing image>")
                    )
                }
                Some(substrate) if substrate.is_host_native() => {
                    "execution boundary: OCI requested but no host container runtime is available"
                        .to_string()
                }
                Some(substrate) if substrate.is_host_shim_oci() => {
                    "execution boundary: inherited host-shim OCI; recursive OCI launch blocked"
                        .to_string()
                }
                Some(substrate) if substrate.is_orchestrated_container() => {
                    format!(
                        "execution boundary: orchestrated container ({}) uses inherited container runtime",
                        substrate.kind
                    )
                }
                Some(substrate) => format!(
                    "execution boundary: OCI requested but substrate '{}' is not launch-capable",
                    substrate.kind
                ),
                None => {
                    "execution boundary: OCI requested but runtime substrate telemetry is unavailable"
                        .to_string()
                }
            }
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ReleasePublishTarget {
    Discord { channel_id: Option<String> },
}

impl ReleasePublishTarget {
    fn is_discord(&self) -> bool {
        matches!(self, Self::Discord { .. })
    }

    fn discord_channel_id(&self) -> Option<&str> {
        match self {
            Self::Discord { channel_id } => channel_id.as_deref(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ReleaseAgentReadiness {
    pub preview_ready: bool,
    pub discord_publish_ready: bool,
    pub execution_boundary_ready: bool,
    pub blockers: Vec<String>,
    pub warnings: Vec<String>,
}

pub const RELEASE_AGENT_ID: &str = "styrene.release-agent";
pub const GITHUB_TOKEN_SECRET: &str = "GITHUB_TOKEN";
pub const DISCORD_TOKEN_SECRET: &str = "VOX_DISCORD_BOT_TOKEN";
pub const DISCORD_CHANNEL_CONFIG: &str = "DISCORD_RELEASE_CHANNEL_ID";

pub fn generate_release_preview_post(release: &GitHubReleaseFixture) -> ReleasePreviewPost {
    let product = release
        .repo
        .rsplit('/')
        .next()
        .unwrap_or(release.repo.as_str());
    let title = format!("{} {} is out", title_case_product(product), release.tag);
    let summary = release_note_summary(&release.body);
    let body = format!("{title}\n\n{summary}\n\nRelease: {}", release.html_url);

    ReleasePreviewPost {
        repo: release.repo.clone(),
        tag: release.tag.clone(),
        title,
        body,
        targets: vec!["preview".to_string()],
        source_url: release.html_url.clone(),
        dedupe_key: format!("{}#{}", release.repo, release.tag),
    }
}

pub fn build_release_preview_artifact(
    release: &GitHubReleaseFixture,
    output_dir: impl AsRef<Path>,
) -> ReleasePreviewArtifact {
    let post = generate_release_preview_post(release);
    let filename = format!(
        "{}__{}.md",
        slug_path_segment(&release.repo),
        slug_path_segment(&release.tag)
    );
    let path = output_dir.as_ref().join(filename);
    let content = format!(
        "---\ntitle: \"{}\"\nrepo: \"{}\"\ntag: \"{}\"\nsource_url: \"{}\"\ndedupe_key: \"{}\"\ntargets: [{}]\npublish_state: preview\n---\n\n{}\n",
        escape_frontmatter_string(&post.title),
        escape_frontmatter_string(&post.repo),
        escape_frontmatter_string(&post.tag),
        escape_frontmatter_string(&post.source_url),
        escape_frontmatter_string(&post.dedupe_key),
        post.targets
            .iter()
            .map(|target| format!("\"{}\"", escape_frontmatter_string(target)))
            .collect::<Vec<_>>()
            .join(", "),
        post.body
    );

    ReleasePreviewArtifact { path, content }
}

#[cfg(not(target_arch = "wasm32"))]
pub fn write_release_preview_artifact(
    release: &GitHubReleaseFixture,
    output_dir: impl AsRef<Path>,
) -> std::io::Result<ReleasePreviewArtifact> {
    let artifact = build_release_preview_artifact(release, output_dir);
    if let Some(parent) = artifact.path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(&artifact.path, &artifact.content)?;
    Ok(artifact)
}

#[cfg(not(target_arch = "wasm32"))]
pub fn stage_release_preview_artifact(
    release: &GitHubReleaseFixture,
    output_dir: impl AsRef<Path>,
    readiness: &ReleaseAgentReadiness,
    repo_allowlist: &[String],
) -> Result<ReleasePreviewArtifact, ReleasePreviewError> {
    if !readiness.preview_ready {
        return Err(ReleasePreviewError::NotReady {
            blockers: readiness.blockers.clone(),
        });
    }
    if !repo_allowlist.iter().any(|repo| repo == &release.repo) {
        return Err(ReleasePreviewError::RepoNotAllowed {
            repo: release.repo.clone(),
        });
    }
    write_release_preview_artifact(release, output_dir).map_err(|error| {
        ReleasePreviewError::NotReady {
            blockers: vec![format!("preview artifact write failed: {error}")],
        }
    })
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ReleaseAgentPreviewRequest {
    pub release: GitHubReleaseFixture,
    pub output_dir: PathBuf,
    pub repo_allowlist: Vec<String>,
    pub configured_secrets: BTreeSet<String>,
    pub model_available: bool,
}

#[cfg(not(target_arch = "wasm32"))]
pub fn run_release_agent_preview(
    request: ReleaseAgentPreviewRequest,
) -> Result<ReleasePreviewArtifact, ReleasePreviewError> {
    let readiness = release_agent_readiness(&ReleaseAgentReadinessInputs {
        configured_secrets: request.configured_secrets,
        model_available: request.model_available,
        repo_allowlist: request.repo_allowlist.clone(),
        preview_approval: ReleasePreviewApproval::default(),
        publish_target: None,
        execution_boundary: ReleaseAgentExecutionBoundary::Inherited,
    });
    stage_release_preview_artifact(
        &request.release,
        request.output_dir,
        &readiness,
        &request.repo_allowlist,
    )
}

#[cfg(not(target_arch = "wasm32"))]
pub async fn fetch_github_release(
    repo: &str,
    tag: &str,
    token: &str,
) -> Result<GitHubReleaseFixture, String> {
    if token.trim().is_empty() {
        return Err("GITHUB_TOKEN is required".to_string());
    }
    let url = format!(
        "https://api.github.com/repos/{}/releases/tags/{}",
        repo.trim_matches('/'),
        tag
    );
    let client = reqwest::Client::builder()
        .user_agent("auspex-release-agent/0.1")
        .build()
        .map_err(|error| format!("GitHub client setup failed: {error}"))?;
    let response = client
        .get(url)
        .bearer_auth(token)
        .header("Accept", "application/vnd.github+json")
        .send()
        .await
        .map_err(|error| format!("GitHub release request failed: {error}"))?;
    if !response.status().is_success() {
        return Err(format!(
            "GitHub release request returned {}",
            response.status()
        ));
    }
    let api: GitHubReleaseApiResponse = response
        .json()
        .await
        .map_err(|error| format!("GitHub release JSON failed: {error}"))?;
    Ok(api.into_fixture(repo.to_string()))
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
struct GitHubReleaseApiResponse {
    tag_name: String,
    name: Option<String>,
    body: Option<String>,
    html_url: String,
    published_at: Option<String>,
}

impl GitHubReleaseApiResponse {
    fn into_fixture(self, repo: String) -> GitHubReleaseFixture {
        GitHubReleaseFixture {
            repo,
            tag: self.tag_name.clone(),
            name: self.name.unwrap_or(self.tag_name),
            body: self.body.unwrap_or_default(),
            html_url: self.html_url,
            published_at: self.published_at.unwrap_or_default(),
        }
    }
}

#[cfg(not(target_arch = "wasm32"))]
pub fn approve_release_preview_artifact(
    path: impl AsRef<Path>,
    approval: &ReleasePreviewApproval,
) -> Result<(), ReleasePreviewError> {
    if !approval.is_approved() {
        return Err(ReleasePreviewError::NotReady {
            blockers: vec!["approval metadata is incomplete".to_string()],
        });
    }
    let path = path.as_ref();
    let content = std::fs::read_to_string(path).map_err(|error| ReleasePreviewError::NotReady {
        blockers: vec![format!("preview artifact read failed: {error}")],
    })?;
    let approved = apply_release_preview_approval(&content, approval)?;
    std::fs::write(path, approved).map_err(|error| ReleasePreviewError::NotReady {
        blockers: vec![format!("preview artifact write failed: {error}")],
    })
}

pub fn parse_release_preview_artifact(
    content: &str,
) -> Result<ParsedReleasePreviewArtifact, ReleasePreviewError> {
    let Some(rest) = content.strip_prefix("---\n") else {
        return Err(ReleasePreviewError::NotReady {
            blockers: vec!["preview artifact is missing frontmatter".to_string()],
        });
    };
    let Some((frontmatter, body)) = rest.split_once("---\n") else {
        return Err(ReleasePreviewError::NotReady {
            blockers: vec!["preview artifact frontmatter is unterminated".to_string()],
        });
    };

    Ok(ParsedReleasePreviewArtifact {
        title: required_frontmatter_value(frontmatter, "title")?,
        repo: required_frontmatter_value(frontmatter, "repo")?,
        tag: required_frontmatter_value(frontmatter, "tag")?,
        source_url: required_frontmatter_value(frontmatter, "source_url")?,
        dedupe_key: required_frontmatter_value(frontmatter, "dedupe_key")?,
        targets: frontmatter_array(frontmatter, "targets").unwrap_or_default(),
        publish_state: required_frontmatter_value(frontmatter, "publish_state")?,
        approved_by: frontmatter_value_from_block(frontmatter, "approved_by"),
        approved_at: frontmatter_value_from_block(frontmatter, "approved_at"),
        body: body.trim_start_matches('\n').to_string(),
    })
}

pub fn serialize_release_preview_artifact(parsed: &ParsedReleasePreviewArtifact) -> String {
    let targets = parsed
        .targets
        .iter()
        .map(|target| format!("\"{}\"", escape_frontmatter_string(target)))
        .collect::<Vec<_>>()
        .join(", ");
    let mut frontmatter = format!(
        "---\ntitle: \"{}\"\nrepo: \"{}\"\ntag: \"{}\"\nsource_url: \"{}\"\ndedupe_key: \"{}\"\ntargets: [{}]\npublish_state: {}\n",
        escape_frontmatter_string(&parsed.title),
        escape_frontmatter_string(&parsed.repo),
        escape_frontmatter_string(&parsed.tag),
        escape_frontmatter_string(&parsed.source_url),
        escape_frontmatter_string(&parsed.dedupe_key),
        targets,
        parsed.publish_state,
    );
    if let Some(approved_by) = parsed.approved_by.as_deref() {
        frontmatter.push_str(&format!(
            "approved_by: \"{}\"\n",
            escape_frontmatter_string(approved_by)
        ));
    }
    if let Some(approved_at) = parsed.approved_at.as_deref() {
        frontmatter.push_str(&format!(
            "approved_at: \"{}\"\n",
            escape_frontmatter_string(approved_at)
        ));
    }
    format!("{frontmatter}---\n\n{}", parsed.body)
}

pub fn apply_release_preview_approval(
    content: &str,
    approval: &ReleasePreviewApproval,
) -> Result<String, ReleasePreviewError> {
    if !approval.is_approved() {
        return Err(ReleasePreviewError::NotReady {
            blockers: vec!["approval metadata is incomplete".to_string()],
        });
    }
    let mut parsed = parse_release_preview_artifact(content)?;
    parsed.publish_state = "approved".to_string();
    parsed.approved_by = approval.approved_by.clone();
    parsed.approved_at = approval.approved_at.clone();
    Ok(serialize_release_preview_artifact(&parsed))
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct DiscordDryRunPublish {
    pub channel_id: String,
    pub dedupe_key: String,
    pub content: String,
    pub would_post: bool,
}

pub fn build_discord_dry_run_publish(
    artifact: &ReleasePreviewArtifact,
    channel_id: &str,
    approval: &ReleasePreviewApproval,
) -> Result<DiscordDryRunPublish, ReleasePreviewError> {
    if !approval.is_approved() {
        return Err(ReleasePreviewError::NotReady {
            blockers: vec!["preview artifact is not operator-approved".to_string()],
        });
    }
    if channel_id.trim().is_empty() {
        return Err(ReleasePreviewError::NotReady {
            blockers: vec![format!("missing {DISCORD_CHANNEL_CONFIG}")],
        });
    }
    let parsed = parse_release_preview_artifact(&artifact.content)?;
    if parsed.publish_state != "approved" {
        return Err(ReleasePreviewError::NotReady {
            blockers: vec!["preview artifact is not approved".to_string()],
        });
    }
    let dedupe_key = parsed.dedupe_key;
    let body = parsed.body.trim().to_string();
    Ok(DiscordDryRunPublish {
        channel_id: channel_id.trim().to_string(),
        dedupe_key,
        content: body,
        would_post: false,
    })
}

fn required_frontmatter_value(frontmatter: &str, key: &str) -> Result<String, ReleasePreviewError> {
    frontmatter_value_from_block(frontmatter, key).ok_or_else(|| ReleasePreviewError::NotReady {
        blockers: vec![format!("preview artifact is missing {key}")],
    })
}

fn frontmatter_value_from_block(frontmatter: &str, key: &str) -> Option<String> {
    frontmatter.lines().find_map(|line| {
        let (candidate, value) = line.split_once(':')?;
        if candidate.trim() != key {
            return None;
        }
        Some(value.trim().trim_matches('"').to_string())
    })
}

fn frontmatter_array(frontmatter: &str, key: &str) -> Option<Vec<String>> {
    let raw = frontmatter_value_from_block(frontmatter, key)?;
    let raw = raw.trim().trim_start_matches('[').trim_end_matches(']');
    if raw.trim().is_empty() {
        return Some(Vec::new());
    }
    Some(
        raw.split(',')
            .map(|item| item.trim().trim_matches('"').to_string())
            .collect(),
    )
}

pub fn release_agent_readiness(inputs: &ReleaseAgentReadinessInputs) -> ReleaseAgentReadiness {
    let mut blockers = Vec::new();
    let mut warnings = Vec::new();
    let has_github = inputs.configured_secrets.contains(GITHUB_TOKEN_SECRET);
    let has_discord = inputs.configured_secrets.contains(DISCORD_TOKEN_SECRET);
    let target_is_discord = inputs
        .publish_target
        .as_ref()
        .is_some_and(ReleasePublishTarget::is_discord);
    let has_channel = inputs
        .publish_target
        .as_ref()
        .and_then(ReleasePublishTarget::discord_channel_id)
        .is_some_and(|channel| !channel.trim().is_empty());

    if !has_github {
        blockers.push(format!("missing required secret {GITHUB_TOKEN_SECRET}"));
    }
    if !inputs.model_available {
        blockers.push("model route unavailable".to_string());
    }
    if inputs.repo_allowlist.is_empty() {
        blockers.push("release repo allowlist is empty".to_string());
    }
    let execution_boundary_ready = match &inputs.execution_boundary {
        ReleaseAgentExecutionBoundary::Inherited => true,
        ReleaseAgentExecutionBoundary::Oci { image, substrate } => {
            if image.as_deref().is_none_or(|image| image.trim().is_empty()) {
                blockers.push("OCI execution boundary requires an image reference".to_string());
                false
            } else {
                match substrate.as_ref() {
                    Some(substrate)
                        if substrate.is_host_native()
                            && substrate.capabilities.has_host_runtime =>
                    {
                        true
                    }
                    Some(substrate) if substrate.is_host_native() => {
                        warnings.push(
                            "OCI execution boundary selected but no host container runtime is available"
                                .to_string(),
                        );
                        false
                    }
                    Some(substrate) if substrate.is_host_shim_oci() => {
                        blockers.push(
                            "OCI execution boundary cannot recursively launch from host-shim OCI"
                                .to_string(),
                        );
                        false
                    }
                    Some(substrate) if substrate.is_orchestrated_container() => {
                        blockers.push(
                            "host-shim OCI launch is unavailable from orchestrated container substrates"
                                .to_string(),
                        );
                        false
                    }
                    Some(substrate) => {
                        warnings.push(format!(
                            "OCI execution boundary selected but substrate '{}' is not launch-capable",
                            substrate.kind
                        ));
                        false
                    }
                    None => {
                        warnings.push(
                            "OCI execution boundary selected but attached runtime did not report substrate telemetry"
                                .to_string(),
                        );
                        false
                    }
                }
            }
        }
    };

    let preview_ready = blockers.is_empty() && execution_boundary_ready;

    if preview_ready && target_is_discord && !has_discord {
        warnings.push(format!(
            "discord publish disabled: missing {DISCORD_TOKEN_SECRET}"
        ));
    }
    if preview_ready && target_is_discord && !has_channel {
        warnings.push(format!(
            "discord publish disabled: missing {DISCORD_CHANNEL_CONFIG}"
        ));
    }

    let discord_publish_ready = preview_ready
        && target_is_discord
        && has_discord
        && has_channel
        && inputs.preview_approval.is_approved();
    if preview_ready
        && target_is_discord
        && has_discord
        && has_channel
        && !inputs.preview_approval.is_approved()
    {
        warnings.push(
            "discord publish disabled: preview artifact is not operator-approved".to_string(),
        );
    }

    ReleaseAgentReadiness {
        preview_ready,
        discord_publish_ready,
        execution_boundary_ready,
        blockers,
        warnings,
    }
}

fn release_note_summary(body: &str) -> String {
    let bullets: Vec<_> = body
        .lines()
        .map(str::trim)
        .filter(|line| line.starts_with("- ") || line.starts_with("* "))
        .take(3)
        .map(|line| format!("- {}", line.trim_start_matches(['-', '*', ' '])))
        .collect();

    if bullets.is_empty() {
        "Release notes are available in the GitHub release.".to_string()
    } else {
        bullets.join("\n")
    }
}

fn title_case_product(product: &str) -> String {
    product
        .split(['-', '_'])
        .filter(|part| !part.is_empty())
        .map(|part| {
            let mut chars = part.chars();
            match chars.next() {
                Some(first) => first.to_uppercase().chain(chars).collect::<String>(),
                None => String::new(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

fn slug_path_segment(value: &str) -> String {
    value
        .chars()
        .map(|ch| match ch {
            'a'..='z' | 'A'..='Z' | '0'..='9' => ch.to_ascii_lowercase(),
            _ => '_',
        })
        .collect::<String>()
        .trim_matches('_')
        .to_string()
}

fn escape_frontmatter_string(value: &str) -> String {
    value.replace('\\', "\\\\").replace('"', "\\\"")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn vox_release() -> GitHubReleaseFixture {
        serde_json::from_str(include_str!(
            "../../tests/fixtures/github-release-vox-v0.1.5.json"
        ))
        .unwrap()
    }
    fn host_native_substrate(
        has_host_runtime: bool,
    ) -> crate::omegon_control::OmegonExecutionSubstrate {
        crate::omegon_control::OmegonExecutionSubstrate {
            kind: "host-native".to_string(),
            capabilities: crate::omegon_control::OmegonExecutionSubstrateCapabilities {
                has_host_runtime,
                can_mount_host_paths: true,
                can_launch_sibling_containers: has_host_runtime,
                can_write_workspace: true,
                has_kubernetes_service_account: false,
            },
            ..Default::default()
        }
    }

    fn substrate(kind: &str) -> crate::omegon_control::OmegonExecutionSubstrate {
        crate::omegon_control::OmegonExecutionSubstrate {
            kind: kind.to_string(),
            capabilities: crate::omegon_control::OmegonExecutionSubstrateCapabilities {
                can_write_workspace: true,
                ..Default::default()
            },
            ..Default::default()
        }
    }

    #[test]
    fn release_preview_post_preserves_release_identity_and_summary() {
        let release = vox_release();
        let post = generate_release_preview_post(&release);

        assert_eq!(post.repo, "styrene-lab/vox");
        assert_eq!(post.tag, "v0.1.5");
        assert_eq!(post.title, "Vox v0.1.5 is out");
        assert_eq!(post.targets, ["preview"]);
        assert_eq!(post.dedupe_key, "styrene-lab/vox#v0.1.5");
        assert!(post.body.contains("Kubernetes background integration"));
        assert!(post.body.contains(&post.source_url));
        assert!(!post.body.contains("GITHUB_TOKEN"));
    }

    #[test]
    fn release_preview_artifact_has_stable_path_and_preview_frontmatter() {
        let release = vox_release();
        let artifact = build_release_preview_artifact(&release, "release-posts");

        assert_eq!(
            artifact.path,
            PathBuf::from("release-posts/styrene_lab_vox__v0_1_5.md")
        );
        assert!(artifact.content.contains("publish_state: preview"));
        assert!(
            artifact
                .content
                .contains("dedupe_key: \"styrene-lab/vox#v0.1.5\"")
        );
        assert!(artifact.content.contains("Vox v0.1.5 is out"));
        assert!(!artifact.content.contains("VOX_DISCORD_BOT_TOKEN"));
    }

    #[test]
    fn committed_release_preview_artifact_matches_generator() {
        let release = vox_release();
        let artifact = build_release_preview_artifact(&release, "release-posts");
        let committed = include_str!("../../release-posts/styrene_lab_vox__v0_1_5.md");

        assert_eq!(artifact.content, committed);
    }

    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    fn release_preview_artifact_can_be_written_end_to_end() {
        let release = vox_release();
        let output_dir =
            std::env::temp_dir().join(format!("auspex-release-agent-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&output_dir);

        let artifact = write_release_preview_artifact(&release, &output_dir).unwrap();
        let written = std::fs::read_to_string(&artifact.path).unwrap();

        assert_eq!(written, artifact.content);
        assert!(
            written.contains("Release: https://github.com/styrene-lab/vox/releases/tag/v0.1.5")
        );
        let _ = std::fs::remove_dir_all(output_dir);
    }

    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    fn release_preview_artifact_can_be_staged_end_to_end_when_ready_and_allowlisted() {
        let release = vox_release();
        let output_dir = std::env::temp_dir().join(format!(
            "auspex-release-agent-stage-test-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&output_dir);
        let readiness = ReleaseAgentReadiness {
            preview_ready: true,
            discord_publish_ready: false,
            execution_boundary_ready: true,
            blockers: Vec::new(),
            warnings: vec!["discord disabled".to_string()],
        };

        let artifact = stage_release_preview_artifact(
            &release,
            &output_dir,
            &readiness,
            &["styrene-lab/vox".to_string()],
        )
        .unwrap();

        assert!(artifact.path.exists());
        assert!(artifact.content.contains("publish_state: preview"));
        let _ = std::fs::remove_dir_all(output_dir);
    }

    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    fn release_preview_staging_rejects_non_allowlisted_repos() {
        let mut release = vox_release();
        release.repo = "other/repo".to_string();
        let readiness = ReleaseAgentReadiness {
            preview_ready: true,
            discord_publish_ready: false,
            execution_boundary_ready: true,
            blockers: Vec::new(),
            warnings: Vec::new(),
        };

        let error = stage_release_preview_artifact(
            &release,
            "release-posts",
            &readiness,
            &["styrene-lab/vox".to_string()],
        )
        .unwrap_err();

        assert_eq!(
            error,
            ReleasePreviewError::RepoNotAllowed {
                repo: "other/repo".to_string()
            }
        );
    }

    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    fn release_agent_preview_runner_stages_preview_artifact() {
        let release = vox_release();
        let output_dir = std::env::temp_dir().join(format!(
            "auspex-release-agent-runner-test-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&output_dir);

        let artifact = run_release_agent_preview(ReleaseAgentPreviewRequest {
            release,
            output_dir: output_dir.clone(),
            repo_allowlist: vec!["styrene-lab/vox".to_string()],
            configured_secrets: BTreeSet::from([GITHUB_TOKEN_SECRET.to_string()]),
            model_available: true,
        })
        .unwrap();

        assert!(artifact.path.exists());
        assert!(artifact.content.contains("publish_state: preview"));
        let _ = std::fs::remove_dir_all(output_dir);
    }

    #[test]
    fn release_preview_artifact_parses_and_roundtrips() {
        let release = vox_release();
        let artifact = build_release_preview_artifact(&release, "release-posts");

        let parsed = parse_release_preview_artifact(&artifact.content).unwrap();
        assert_eq!(parsed.repo, "styrene-lab/vox");
        assert_eq!(parsed.tag, "v0.1.5");
        assert_eq!(parsed.targets, ["preview"]);
        assert_eq!(parsed.publish_state, "preview");
        assert!(parsed.body.contains("Vox v0.1.5 is out"));
        assert_eq!(
            serialize_release_preview_artifact(&parsed),
            artifact.content
        );
    }

    #[test]
    fn release_preview_artifact_parser_requires_schema_fields() {
        let error = parse_release_preview_artifact("---\ntitle: \"x\"\n---\n\nbody").unwrap_err();

        assert_eq!(
            error,
            ReleasePreviewError::NotReady {
                blockers: vec!["preview artifact is missing repo".to_string()]
            }
        );
    }

    #[test]
    fn release_preview_approval_updates_frontmatter() {
        let release = vox_release();
        let artifact = build_release_preview_artifact(&release, "release-posts");

        let approved = apply_release_preview_approval(
            &artifact.content,
            &ReleasePreviewApproval {
                approved_by: Some("operator".to_string()),
                approved_at: Some("2026-06-15T00:00:00Z".to_string()),
            },
        )
        .unwrap();

        assert!(approved.contains("publish_state: approved"));
        assert!(approved.contains("approved_by: \"operator\""));
        assert!(approved.contains("approved_at: \"2026-06-15T00:00:00Z\""));
    }

    #[test]
    fn discord_dry_run_requires_approved_preview() {
        let release = vox_release();
        let artifact = build_release_preview_artifact(&release, "release-posts");

        let error =
            build_discord_dry_run_publish(&artifact, "123", &ReleasePreviewApproval::default())
                .unwrap_err();

        assert_eq!(
            error,
            ReleasePreviewError::NotReady {
                blockers: vec!["preview artifact is not operator-approved".to_string()]
            }
        );
    }

    #[test]
    fn discord_dry_run_extracts_body_and_dedupe_key() {
        let release = vox_release();
        let artifact = build_release_preview_artifact(&release, "release-posts");
        let approval = ReleasePreviewApproval {
            approved_by: Some("operator".to_string()),
            approved_at: Some("2026-06-15T00:00:00Z".to_string()),
        };

        let approved_content =
            apply_release_preview_approval(&artifact.content, &approval).unwrap();
        let approved_artifact = ReleasePreviewArtifact {
            path: artifact.path.clone(),
            content: approved_content,
        };
        let dry_run = build_discord_dry_run_publish(&approved_artifact, "123", &approval).unwrap();

        assert_eq!(dry_run.channel_id, "123");
        assert_eq!(dry_run.dedupe_key, "styrene-lab/vox#v0.1.5");
        assert!(!dry_run.would_post);
        assert!(dry_run.content.contains("Vox v0.1.5 is out"));
        assert!(!dry_run.content.contains("publish_state"));
    }

    #[test]
    fn release_agent_preview_allows_oci_boundary_with_image_and_runtime() {
        let readiness = release_agent_readiness(&ReleaseAgentReadinessInputs {
            configured_secrets: BTreeSet::from([GITHUB_TOKEN_SECRET.to_string()]),
            model_available: true,
            repo_allowlist: vec!["styrene-lab/vox".to_string()],
            publish_target: None,
            preview_approval: ReleasePreviewApproval::default(),
            execution_boundary: ReleaseAgentExecutionBoundary::Oci {
                image: Some("ghcr.io/styrene-lab/omegon-full:0.27.0-local".to_string()),
                substrate: Box::new(Some(host_native_substrate(true))),
            },
        });

        assert!(readiness.preview_ready);
        assert!(readiness.execution_boundary_ready);
        assert!(readiness.blockers.is_empty());
        assert!(readiness.warnings.is_empty());
    }

    #[test]
    fn release_agent_preview_blocks_oci_boundary_without_image() {
        let readiness = release_agent_readiness(&ReleaseAgentReadinessInputs {
            configured_secrets: BTreeSet::from([GITHUB_TOKEN_SECRET.to_string()]),
            model_available: true,
            repo_allowlist: vec!["styrene-lab/vox".to_string()],
            publish_target: None,
            preview_approval: ReleasePreviewApproval::default(),
            execution_boundary: ReleaseAgentExecutionBoundary::Oci {
                image: None,
                substrate: Box::new(Some(host_native_substrate(true))),
            },
        });

        assert!(!readiness.preview_ready);
        assert!(!readiness.execution_boundary_ready);
        assert!(
            readiness
                .blockers
                .iter()
                .any(|blocker| blocker.contains("image reference"))
        );
    }

    #[test]
    fn release_agent_preview_warns_for_oci_boundary_without_runtime() {
        let readiness = release_agent_readiness(&ReleaseAgentReadinessInputs {
            configured_secrets: BTreeSet::from([GITHUB_TOKEN_SECRET.to_string()]),
            model_available: true,
            repo_allowlist: vec!["styrene-lab/vox".to_string()],
            publish_target: None,
            preview_approval: ReleasePreviewApproval::default(),
            execution_boundary: ReleaseAgentExecutionBoundary::Oci {
                image: Some("ghcr.io/styrene-lab/omegon-full:0.27.0-local".to_string()),
                substrate: Box::new(Some(host_native_substrate(false))),
            },
        });

        assert!(!readiness.preview_ready);
        assert!(!readiness.execution_boundary_ready);
        assert!(
            readiness
                .warnings
                .iter()
                .any(|warning| warning.contains("container runtime"))
        );
    }

    #[test]
    fn release_agent_preview_blocks_recursive_oci_from_host_shim() {
        let readiness = release_agent_readiness(&ReleaseAgentReadinessInputs {
            configured_secrets: BTreeSet::from([GITHUB_TOKEN_SECRET.to_string()]),
            model_available: true,
            repo_allowlist: vec!["styrene-lab/vox".to_string()],
            publish_target: None,
            preview_approval: ReleasePreviewApproval::default(),
            execution_boundary: ReleaseAgentExecutionBoundary::Oci {
                image: Some("ghcr.io/styrene-lab/omegon-full:0.27.0-local".to_string()),
                substrate: Box::new(Some(substrate("host-shim-oci"))),
            },
        });

        assert!(!readiness.preview_ready);
        assert!(!readiness.execution_boundary_ready);
        assert!(
            readiness
                .blockers
                .iter()
                .any(|blocker| blocker.contains("recursively"))
        );
    }

    #[test]
    fn release_agent_preview_blocks_host_shim_oci_from_kubernetes() {
        let readiness = release_agent_readiness(&ReleaseAgentReadinessInputs {
            configured_secrets: BTreeSet::from([GITHUB_TOKEN_SECRET.to_string()]),
            model_available: true,
            repo_allowlist: vec!["styrene-lab/vox".to_string()],
            publish_target: None,
            preview_approval: ReleasePreviewApproval::default(),
            execution_boundary: ReleaseAgentExecutionBoundary::Oci {
                image: Some("ghcr.io/styrene-lab/omegon-full:0.27.0-local".to_string()),
                substrate: Box::new(Some(substrate("kubernetes"))),
            },
        });

        assert!(!readiness.preview_ready);
        assert!(!readiness.execution_boundary_ready);
        assert!(
            readiness
                .blockers
                .iter()
                .any(|blocker| blocker.contains("orchestrated container"))
        );
    }

    #[test]
    fn release_agent_preview_warns_when_substrate_telemetry_missing() {
        let readiness = release_agent_readiness(&ReleaseAgentReadinessInputs {
            configured_secrets: BTreeSet::from([GITHUB_TOKEN_SECRET.to_string()]),
            model_available: true,
            repo_allowlist: vec!["styrene-lab/vox".to_string()],
            publish_target: None,
            preview_approval: ReleasePreviewApproval::default(),
            execution_boundary: ReleaseAgentExecutionBoundary::Oci {
                image: Some("ghcr.io/styrene-lab/omegon-full:0.27.0-local".to_string()),
                substrate: Box::new(None),
            },
        });

        assert!(!readiness.preview_ready);
        assert!(!readiness.execution_boundary_ready);
        assert!(
            readiness
                .warnings
                .iter()
                .any(|warning| warning.contains("substrate telemetry"))
        );
    }

    #[test]
    fn release_agent_execution_boundary_summary_explains_substrate_decision() {
        let summary =
            release_agent_execution_boundary_summary(&ReleaseAgentExecutionBoundary::Oci {
                image: Some("ghcr.io/styrene-lab/omegon-full:0.27.0-local".to_string()),
                substrate: Box::new(Some(substrate("host-shim-oci"))),
            });

        assert!(summary.contains("recursive OCI launch blocked"));
    }

    #[test]
    fn release_agent_readiness_keeps_preview_clean_without_publish_target() {
        let readiness = release_agent_readiness(&ReleaseAgentReadinessInputs {
            configured_secrets: BTreeSet::from([GITHUB_TOKEN_SECRET.to_string()]),
            model_available: true,
            repo_allowlist: vec!["styrene-lab/vox".to_string()],
            publish_target: None,
            preview_approval: ReleasePreviewApproval::default(),
            execution_boundary: ReleaseAgentExecutionBoundary::Inherited,
        });

        assert!(readiness.preview_ready);
        assert!(!readiness.discord_publish_ready);
        assert!(readiness.blockers.is_empty());
        assert!(readiness.warnings.is_empty());
    }

    #[test]
    fn release_agent_readiness_separates_preview_from_discord_publish() {
        let readiness = release_agent_readiness(&ReleaseAgentReadinessInputs {
            configured_secrets: BTreeSet::from([GITHUB_TOKEN_SECRET.to_string()]),
            model_available: true,
            repo_allowlist: vec!["styrene-lab/vox".to_string()],
            publish_target: Some(ReleasePublishTarget::Discord { channel_id: None }),
            preview_approval: ReleasePreviewApproval::default(),
            execution_boundary: ReleaseAgentExecutionBoundary::Inherited,
        });

        assert!(readiness.preview_ready);
        assert!(!readiness.discord_publish_ready);
        assert!(readiness.blockers.is_empty());
        assert!(
            readiness
                .warnings
                .iter()
                .any(|warning| warning.contains(DISCORD_TOKEN_SECRET))
        );
    }

    #[test]
    fn release_agent_readiness_blocks_preview_without_github() {
        let readiness = release_agent_readiness(&ReleaseAgentReadinessInputs {
            configured_secrets: BTreeSet::new(),
            model_available: true,
            repo_allowlist: vec!["styrene-lab/vox".to_string()],
            publish_target: Some(ReleasePublishTarget::Discord {
                channel_id: Some("123".to_string()),
            }),
            preview_approval: ReleasePreviewApproval::default(),
            execution_boundary: ReleaseAgentExecutionBoundary::Inherited,
        });

        assert!(!readiness.preview_ready);
        assert!(!readiness.discord_publish_ready);
        assert!(
            readiness
                .blockers
                .iter()
                .any(|blocker| blocker.contains(GITHUB_TOKEN_SECRET))
        );
    }

    #[test]
    fn release_agent_readiness_requires_approval_for_discord_publish() {
        let readiness = release_agent_readiness(&ReleaseAgentReadinessInputs {
            configured_secrets: BTreeSet::from([
                GITHUB_TOKEN_SECRET.to_string(),
                DISCORD_TOKEN_SECRET.to_string(),
            ]),
            model_available: true,
            repo_allowlist: vec!["styrene-lab/vox".to_string()],
            publish_target: Some(ReleasePublishTarget::Discord {
                channel_id: Some("123".to_string()),
            }),
            preview_approval: ReleasePreviewApproval::default(),
            execution_boundary: ReleaseAgentExecutionBoundary::Inherited,
        });

        assert!(readiness.preview_ready);
        assert!(!readiness.discord_publish_ready);
        assert!(
            readiness
                .warnings
                .iter()
                .any(|warning| warning.contains("operator-approved"))
        );
    }

    #[test]
    fn release_agent_readiness_allows_discord_publish_after_approval() {
        let readiness = release_agent_readiness(&ReleaseAgentReadinessInputs {
            configured_secrets: BTreeSet::from([
                GITHUB_TOKEN_SECRET.to_string(),
                DISCORD_TOKEN_SECRET.to_string(),
            ]),
            model_available: true,
            repo_allowlist: vec!["styrene-lab/vox".to_string()],
            publish_target: Some(ReleasePublishTarget::Discord {
                channel_id: Some("123".to_string()),
            }),
            preview_approval: ReleasePreviewApproval {
                approved_by: Some("operator".to_string()),
                approved_at: Some("2026-06-15T00:00:00Z".to_string()),
            },
            execution_boundary: ReleaseAgentExecutionBoundary::Inherited,
        });

        assert!(readiness.preview_ready);
        assert!(readiness.discord_publish_ready);
        assert!(readiness.blockers.is_empty());
        assert!(readiness.warnings.is_empty());
    }
}
