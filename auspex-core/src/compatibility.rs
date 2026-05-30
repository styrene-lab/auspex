#![allow(dead_code)]

use semver::Version;
use serde::{Deserialize, Serialize};

use crate::runtime_types::ObservedControlPlane;

pub const MINIMUM_OMEGON_VERSION: &str = "0.25.0";
pub const MAXIMUM_TESTED_OMEGON_VERSION: &str = "0.25.4";
pub const WEB_STARTUP_SCHEMA_VERSION: u32 = 2;
pub const INSTANCE_DESCRIPTOR_SCHEMA_VERSION: u32 = 1;
pub const CONTROL_PLANE_PROTOCOL_VERSION: u32 = 1;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CompatibilityStatus {
    Compatible,
    Unsupported,
    Unknown,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CompatibilityIssue {
    pub code: String,
    pub detail: String,
}

impl CompatibilityIssue {
    fn new(code: impl Into<String>, detail: impl Into<String>) -> Self {
        Self { code: code.into(), detail: detail.into() }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CompatibilityAssessment {
    pub status: CompatibilityStatus,
    pub minimum_omegon_version: String,
    pub maximum_tested_omegon_version: String,
    pub expected_web_startup_schema: u32,
    pub expected_instance_descriptor_schema: u32,
    pub expected_control_plane_protocol: u32,
    pub observed_omegon_version: Option<String>,
    pub observed_schema_version: Option<u32>,
    #[serde(default)]
    pub issues: Vec<CompatibilityIssue>,
}

impl CompatibilityAssessment {
    fn compatible(observed_omegon_version: String, observed_schema_version: u32) -> Self {
        Self {
            status: CompatibilityStatus::Compatible,
            minimum_omegon_version: MINIMUM_OMEGON_VERSION.into(),
            maximum_tested_omegon_version: MAXIMUM_TESTED_OMEGON_VERSION.into(),
            expected_web_startup_schema: WEB_STARTUP_SCHEMA_VERSION,
            expected_instance_descriptor_schema: INSTANCE_DESCRIPTOR_SCHEMA_VERSION,
            expected_control_plane_protocol: CONTROL_PLANE_PROTOCOL_VERSION,
            observed_omegon_version: Some(observed_omegon_version),
            observed_schema_version: Some(observed_schema_version),
            issues: Vec::new(),
        }
    }

    fn with_issues(
        status: CompatibilityStatus,
        observed_omegon_version: Option<String>,
        observed_schema_version: Option<u32>,
        issues: Vec<CompatibilityIssue>,
    ) -> Self {
        Self {
            status,
            minimum_omegon_version: MINIMUM_OMEGON_VERSION.into(),
            maximum_tested_omegon_version: MAXIMUM_TESTED_OMEGON_VERSION.into(),
            expected_web_startup_schema: WEB_STARTUP_SCHEMA_VERSION,
            expected_instance_descriptor_schema: INSTANCE_DESCRIPTOR_SCHEMA_VERSION,
            expected_control_plane_protocol: CONTROL_PLANE_PROTOCOL_VERSION,
            observed_omegon_version,
            observed_schema_version,
            issues,
        }
    }

    pub fn is_compatible(&self) -> bool {
        self.status == CompatibilityStatus::Compatible
    }
}

/// Assess the compatibility of an observed Omegon web startup/control-plane
/// surface using the local Omegon 0.25 source contract.
///
/// `ObservedControlPlane.schema_version` historically represented the web
/// startup schema in Auspex records. Local Omegon source shows that this should
/// be checked against `WebStartupInfo.schema_version == 2`, while the embedded
/// `OmegonInstanceDescriptor` and `OmegonControlPlane` use IPC protocol `1`.
pub fn assess_observed_control_plane(
    control_plane: &ObservedControlPlane,
) -> CompatibilityAssessment {
    let mut issues = Vec::new();

    let observed_version = if control_plane.omegon_version.trim().is_empty()
        || control_plane.omegon_version == "unknown"
    {
        issues.push(CompatibilityIssue::new(
            "missing_omegon_version",
            "Omegon version is absent or unknown",
        ));
        None
    } else {
        Some(control_plane.omegon_version.clone())
    };

    if control_plane.schema_version != WEB_STARTUP_SCHEMA_VERSION {
        issues.push(CompatibilityIssue::new(
            "web_startup_schema_mismatch",
            format!(
                "expected web startup schema {}, observed {}",
                WEB_STARTUP_SCHEMA_VERSION, control_plane.schema_version
            ),
        ));
    }

    if let Some(version_text) = observed_version.as_deref() {
        match parse_omegon_version(version_text) {
            Some(version) => {
                let minimum = Version::parse(MINIMUM_OMEGON_VERSION)
                    .expect("minimum Omegon compatibility version is valid semver");
                if version < minimum {
                    issues.push(CompatibilityIssue::new(
                        "omegon_version_unsupported",
                        format!(
                            "Omegon {version_text} is older than required {MINIMUM_OMEGON_VERSION}"
                        ),
                    ));
                }
            }
            None => issues.push(CompatibilityIssue::new(
                "invalid_omegon_version",
                format!("Omegon version '{version_text}' is not valid semver"),
            )),
        }
    }

    if issues.is_empty() {
        CompatibilityAssessment::compatible(
            control_plane.omegon_version.clone(),
            control_plane.schema_version,
        )
    } else {
        let status = if observed_version.is_none() {
            CompatibilityStatus::Unknown
        } else {
            CompatibilityStatus::Unsupported
        };
        CompatibilityAssessment::with_issues(
            status,
            observed_version,
            Some(control_plane.schema_version),
            issues,
        )
    }
}

fn parse_omegon_version(version: &str) -> Option<Version> {
    let trimmed = version.trim().trim_start_matches('v');
    Version::parse(trimmed).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn observed(version: &str, schema_version: u32) -> ObservedControlPlane {
        ObservedControlPlane {
            schema_version,
            omegon_version: version.into(),
            base_url: "http://127.0.0.1:7842".into(),
            startup_url: "http://127.0.0.1:7842/api/startup".into(),
            health_url: "http://127.0.0.1:7842/api/healthz".into(),
            ready_url: "http://127.0.0.1:7842/api/readyz".into(),
            ws_url: "ws://127.0.0.1:7842/ws".into(),
            acp_url: Some("ws://127.0.0.1:7842/acp".into()),
            auth_mode: "ephemeral-bearer".into(),
            ..Default::default()
        }
    }

    #[test]
    fn omegon_0254_with_web_startup_schema_2_is_compatible() {
        let assessment = assess_observed_control_plane(&observed("0.25.4", 2));

        assert!(assessment.is_compatible());
        assert_eq!(assessment.expected_web_startup_schema, 2);
        assert_eq!(assessment.expected_instance_descriptor_schema, 1);
        assert_eq!(assessment.expected_control_plane_protocol, 1);
    }

    #[test]
    fn pre_025_is_unsupported_not_degraded() {
        let assessment = assess_observed_control_plane(&observed("0.23.0", 2));

        assert_eq!(assessment.status, CompatibilityStatus::Unsupported);
        assert!(assessment
            .issues
            .iter()
            .any(|issue| issue.code == "omegon_version_unsupported"));
    }

    #[test]
    fn wrong_web_startup_schema_is_unsupported() {
        let assessment = assess_observed_control_plane(&observed("0.25.4", 1));

        assert_eq!(assessment.status, CompatibilityStatus::Unsupported);
        assert!(assessment
            .issues
            .iter()
            .any(|issue| issue.code == "web_startup_schema_mismatch"));
    }

    #[test]
    fn missing_version_is_unknown() {
        let assessment = assess_observed_control_plane(&observed("unknown", 2));

        assert_eq!(assessment.status, CompatibilityStatus::Unknown);
        assert!(assessment
            .issues
            .iter()
            .any(|issue| issue.code == "missing_omegon_version"));
    }
}
