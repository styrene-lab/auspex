//! Auspex authorization adapter over `styrene-policy`.
//!
//! This module keeps Auspex-specific action/resource/context semantics local
//! while using the shared Styrene policy request/decision shape.

use styrene_policy::{
    ActionRef, NativePolicyEngine, PolicyContext, PolicyDecision, PolicyEngine, PolicyFollowup,
    PolicyRequest, PrincipalRef, ResourceRef,
};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum LocalOmegonAction {
    Discover,
    Probe,
    Attach,
    Command,
    Launch,
    StopOwned,
    RestartOwned,
    StopExternal,
    InstallPackage,
    InvokeHostAction,
}

impl LocalOmegonAction {
    pub fn capability(&self) -> &'static str {
        match self {
            Self::Discover => "auspex.discovery.local.read",
            Self::Probe => "auspex.instance.probe",
            Self::Attach => "auspex.instance.attach",
            Self::Command => "auspex.instance.command",
            Self::Launch => "auspex.instance.launch",
            Self::StopOwned => "auspex.instance.stop_owned",
            Self::RestartOwned => "auspex.instance.restart_owned",
            Self::StopExternal => "auspex.instance.stop_external",
            Self::InstallPackage => "auspex.package.install",
            Self::InvokeHostAction => "auspex.host_action.invoke_mutating",
        }
    }

    pub fn requires_identity(&self) -> bool {
        !matches!(self, Self::Discover | Self::Probe)
    }

    pub fn requires_approval(&self) -> bool {
        matches!(
            self,
            Self::Launch
                | Self::StopOwned
                | Self::RestartOwned
                | Self::StopExternal
                | Self::InstallPackage
                | Self::InvokeHostAction
        )
    }

    pub fn audit_required(&self) -> bool {
        !matches!(self, Self::Discover)
    }
}

pub fn local_omegon_policy_request(
    principal: PrincipalRef,
    action: LocalOmegonAction,
    resource: ResourceRef,
) -> PolicyRequest {
    let context = PolicyContext::default()
        .with("requires_identity", action.requires_identity().to_string())
        .with("approval_required", action.requires_approval().to_string())
        .with("audit_required", action.audit_required().to_string())
        .with(
            "signature_required",
            matches!(
                action,
                LocalOmegonAction::StopOwned
                    | LocalOmegonAction::RestartOwned
                    | LocalOmegonAction::StopExternal
                    | LocalOmegonAction::InstallPackage
                    | LocalOmegonAction::InvokeHostAction
            )
            .to_string(),
        );

    PolicyRequest {
        principal,
        action: ActionRef { namespace: "".into(), name: action.capability().into() },
        resource,
        context,
    }
}

pub fn authorize_local_omegon_action(
    principal: PrincipalRef,
    action: LocalOmegonAction,
    resource: ResourceRef,
) -> PolicyDecision {
    if !action.requires_identity() && principal.has_capability(action.capability()) {
        return PolicyDecision::allow().normalized();
    }
    if action.requires_identity()
        && principal.has_identity()
        && principal.has_capability(action.capability())
    {
        let mut decision = PolicyDecision::allow();
        if action.requires_approval() {
            decision = decision.with_obligation(PolicyFollowup::Approval);
        }
        if action.audit_required() {
            decision = decision.with_obligation(PolicyFollowup::Audit);
        }
        if matches!(
            action,
            LocalOmegonAction::StopOwned
                | LocalOmegonAction::RestartOwned
                | LocalOmegonAction::StopExternal
                | LocalOmegonAction::InstallPackage
                | LocalOmegonAction::InvokeHostAction
        ) {
            decision = decision.with_obligation(PolicyFollowup::Signature);
        }
        return decision.normalized();
    }
    let request = local_omegon_policy_request(principal, action, resource);
    NativePolicyEngine::default().authorize(&request)
}

pub fn runtime_resource(instance_id: impl Into<String>) -> ResourceRef {
    ResourceRef::new("auspex", "omegon-runtime", instance_id)
}

pub fn local_host_resource() -> ResourceRef {
    ResourceRef::new("auspex", "local-host", "localhost")
}

#[cfg(test)]
mod tests {
    use super::*;
    use styrene_policy::PolicyEffect;

    fn principal_with(capability: &str) -> PrincipalRef {
        PrincipalRef {
            identity_hash: Some("aaaa1111bbbb2222cccc3333dddd4444".into()),
            role: Some("operator".into()),
            capabilities: vec![capability.into()],
            can_sign: true,
        }
    }

    #[test]
    fn discovery_is_allowed_without_identity_when_capability_present() {
        let decision = authorize_local_omegon_action(
            PrincipalRef { capabilities: vec!["auspex.discovery.local.read".into()], ..PrincipalRef::anonymous() },
            LocalOmegonAction::Discover,
            local_host_resource(),
        );

        assert_eq!(decision.effect, PolicyEffect::Allow);
        assert!(decision.obligations.is_empty());
    }

    #[test]
    fn attach_is_denied_without_identity() {
        let decision = authorize_local_omegon_action(
            PrincipalRef { capabilities: vec!["auspex.instance.attach".into()], ..PrincipalRef::anonymous() },
            LocalOmegonAction::Attach,
            runtime_resource("runtime-1"),
        );

        assert_eq!(decision.effect, PolicyEffect::Deny);
        assert!(decision.reasons.iter().any(|reason| reason.code == "identity.required"));
    }

    #[test]
    fn stop_owned_requires_approval_audit_and_signature() {
        let decision = authorize_local_omegon_action(
            principal_with("auspex.instance.stop_owned"),
            LocalOmegonAction::StopOwned,
            runtime_resource("runtime-1"),
        );

        assert_eq!(decision.effect, PolicyEffect::Allow);
        assert!(decision.obligations.contains(&PolicyFollowup::Approval));
        assert!(decision.obligations.contains(&PolicyFollowup::Audit));
        assert!(decision.obligations.contains(&PolicyFollowup::Signature));
    }

    #[test]
    fn package_install_requires_explicit_capability() {
        let decision = authorize_local_omegon_action(
            principal_with("auspex.instance.command"),
            LocalOmegonAction::InstallPackage,
            runtime_resource("runtime-1"),
        );

        assert_eq!(decision.effect, PolicyEffect::Deny);
        assert!(decision.reasons.iter().any(|reason| reason.code == "capability.missing"));
    }
}
