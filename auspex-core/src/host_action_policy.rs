#![allow(dead_code)]

use serde::{Deserialize, Serialize};

/// Auspex-side policy classification for host actions proposed by Omegon or
/// extensions. This is intentionally small: execution remains host-owned, while
/// Auspex decides whether a candidate may enter an approval/audit path.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HostActionPolicyClass {
    /// Safe evidence/discovery path; no host mutation should occur.
    ReadOnlyDiscovery,
    /// Known mutating action. It must be reviewed/approved and audited before execution.
    MutatingRequiresApproval,
    /// Known action type, but this Auspex deployment cannot execute it.
    Unsupported,
    /// Unknown or explicitly disallowed action.
    Deny,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct HostActionPolicyDecision {
    pub action_type: String,
    pub class: HostActionPolicyClass,
    pub requires_approval: bool,
    pub audit_required: bool,
    pub reason: String,
}

impl HostActionPolicyDecision {
    fn new(
        action_type: impl Into<String>,
        class: HostActionPolicyClass,
        requires_approval: bool,
        audit_required: bool,
        reason: impl Into<String>,
    ) -> Self {
        Self {
            action_type: action_type.into(),
            class,
            requires_approval,
            audit_required,
            reason: reason.into(),
        }
    }
}

/// Classify a declarative HostAction candidate by action type.
///
/// Tool-level read-only capability checks such as `nex_capability` are not
/// HostActions, but the classifier accepts the symbolic name so callers can use
/// one policy seam for capability-discovery evidence and HostAction candidates.
pub fn classify_host_action(action_type: &str) -> HostActionPolicyDecision {
    match action_type {
        "nex_capability" | "nex.capability.check" | "nex.capability.resolve" => {
            HostActionPolicyDecision::new(
                action_type,
                HostActionPolicyClass::ReadOnlyDiscovery,
                false,
                false,
                "Nex capability resolution is read-only discovery evidence",
            )
        }
        "package.install@1" => HostActionPolicyDecision::new(
            action_type,
            HostActionPolicyClass::MutatingRequiresApproval,
            true,
            true,
            "package.install@1 mutates host package state and requires approval",
        ),
        "terminal.create@1" => HostActionPolicyDecision::new(
            action_type,
            HostActionPolicyClass::MutatingRequiresApproval,
            true,
            true,
            "terminal.create@1 can execute host commands and requires approval",
        ),
        known if known.starts_with("package.") || known.starts_with("terminal.") => {
            HostActionPolicyDecision::new(
                action_type,
                HostActionPolicyClass::Unsupported,
                true,
                true,
                "known HostAction domain is not supported by this Auspex policy version",
            )
        }
        _ => HostActionPolicyDecision::new(
            action_type,
            HostActionPolicyClass::Deny,
            false,
            true,
            "unknown HostAction types are denied by default",
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn package_install_requires_approval_and_audit() {
        let decision = classify_host_action("package.install@1");

        assert_eq!(
            decision.class,
            HostActionPolicyClass::MutatingRequiresApproval
        );
        assert!(decision.requires_approval);
        assert!(decision.audit_required);
    }

    #[test]
    fn nex_capability_is_read_only_discovery() {
        let decision = classify_host_action("nex_capability");

        assert_eq!(decision.class, HostActionPolicyClass::ReadOnlyDiscovery);
        assert!(!decision.requires_approval);
        assert!(!decision.audit_required);
    }

    #[test]
    fn unknown_host_actions_are_denied_by_default() {
        let decision = classify_host_action("filesystem.destroy@9");

        assert_eq!(decision.class, HostActionPolicyClass::Deny);
        assert!(!decision.requires_approval);
        assert!(decision.audit_required);
    }

    #[test]
    fn unknown_known_domains_are_unsupported_not_silently_allowed() {
        let decision = classify_host_action("package.remove@1");

        assert_eq!(decision.class, HostActionPolicyClass::Unsupported);
        assert!(decision.requires_approval);
        assert!(decision.audit_required);
    }
}
