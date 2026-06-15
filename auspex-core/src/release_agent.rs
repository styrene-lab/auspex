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

    let preview_ready = blockers.is_empty();

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

    #[test]
    fn release_agent_readiness_keeps_preview_clean_without_publish_target() {
        let readiness = release_agent_readiness(&ReleaseAgentReadinessInputs {
            configured_secrets: BTreeSet::from([GITHUB_TOKEN_SECRET.to_string()]),
            model_available: true,
            repo_allowlist: vec!["styrene-lab/vox".to_string()],
            publish_target: None,
            preview_approval: ReleasePreviewApproval::default(),
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
        });

        assert!(readiness.preview_ready);
        assert!(readiness.discord_publish_ready);
        assert!(readiness.blockers.is_empty());
        assert!(readiness.warnings.is_empty());
    }
}
