#![allow(dead_code)]

use crate::fixtures::SessionData;
use crate::runtime_types::CommandTarget;

pub const HOST_CONTROL_PLANE_ROUTE_ID: &str = "host-control-plane";
pub const SESSION_DISPATCHER_ROUTE_ID: &str = "session-dispatcher";
pub const LOCAL_SHELL_ROUTE_ID: &str = "local-shell";

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AttachedInstanceRecord {
    pub instance_id: String,
    pub route_id: String,
    pub role: String,
    pub profile: String,
    pub session_key: String,
    pub base_url: Option<String>,
    pub model: Option<String>,
    pub dispatcher_instance_id: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CommandRouteProjection {
    pub route_id: String,
    pub label: String,
    pub detail: String,
    pub target: CommandTarget,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct AttachedInstanceStateEngine {
    session_key: String,
    attached_instances: Vec<AttachedInstanceRecord>,
    selected_command_route_id: Option<String>,
}

impl AttachedInstanceStateEngine {
    pub fn from_session_snapshot(session_key: impl Into<String>, session: &SessionData) -> Self {
        let session_key = session_key.into();
        Self {
            attached_instances: rebuild_attached_instances(session_key.clone(), session),
            session_key,
            selected_command_route_id: None,
        }
    }

    pub fn attached_instances(&self) -> &[AttachedInstanceRecord] {
        &self.attached_instances
    }

    pub fn select_command_route(&mut self, route_id: impl Into<String>) {
        self.selected_command_route_id = Some(route_id.into());
    }

    pub fn selected_command_route_id(&self) -> String {
        let routes = self.available_command_routes();
        if let Some(selected) = self.selected_command_route_id.as_ref()
            && routes.iter().any(|route| &route.route_id == selected)
        {
            return selected.clone();
        }

        if routes
            .iter()
            .any(|route| route.route_id == SESSION_DISPATCHER_ROUTE_ID)
        {
            SESSION_DISPATCHER_ROUTE_ID.into()
        } else {
            routes
                .first()
                .map(|route| route.route_id.clone())
                .unwrap_or_else(|| LOCAL_SHELL_ROUTE_ID.into())
        }
    }

    pub fn available_command_routes(&self) -> Vec<CommandRouteProjection> {
        let mut routes: Vec<CommandRouteProjection> = self
            .attached_instances
            .iter()
            .map(project_command_route)
            .collect();

        if routes.is_empty() {
            routes.push(CommandRouteProjection {
                route_id: LOCAL_SHELL_ROUTE_ID.into(),
                label: "Local shell".into(),
                detail: "No attached host instance reported".into(),
                target: CommandTarget {
                    session_key: self.session_key.clone(),
                    dispatcher_instance_id: None,
                },
            });
        }

        routes
    }

    pub fn current_command_target(&self) -> CommandTarget {
        let selected_route_id = self.selected_command_route_id();
        self.available_command_routes()
            .into_iter()
            .find(|route| route.route_id == selected_route_id)
            .map(|route| route.target)
            .unwrap_or(CommandTarget {
                session_key: self.session_key.clone(),
                dispatcher_instance_id: None,
            })
    }
}

pub fn rebuild_attached_instances(
    session_key: impl Into<String>,
    session: &SessionData,
) -> Vec<AttachedInstanceRecord> {
    let session_key = session_key.into();
    let mut attached_instances = Vec::new();

    if let Some(instance) = session.instance_descriptor.as_ref() {
        attached_instances.push(AttachedInstanceRecord {
            instance_id: if instance.identity.instance_id.is_empty() {
                HOST_CONTROL_PLANE_ROUTE_ID.into()
            } else {
                instance.identity.instance_id.clone()
            },
            route_id: HOST_CONTROL_PLANE_ROUTE_ID.into(),
            role: if instance.identity.role.is_empty() {
                "host".into()
            } else {
                instance.identity.role.clone()
            },
            profile: instance.identity.profile.clone(),
            session_key: session_key.clone(),
            base_url: instance
                .control_plane
                .as_ref()
                .and_then(|control_plane| control_plane.base_url.clone()),
            model: instance
                .policy
                .as_ref()
                .and_then(|policy| policy.model.clone()),
            dispatcher_instance_id: None,
        });
    }

    if let Some(binding) = session.dispatcher_binding.as_ref() {
        attached_instances.push(AttachedInstanceRecord {
            instance_id: if binding.dispatcher_instance_id.is_empty() {
                SESSION_DISPATCHER_ROUTE_ID.into()
            } else {
                binding.dispatcher_instance_id.clone()
            },
            route_id: SESSION_DISPATCHER_ROUTE_ID.into(),
            role: if binding.expected_role.is_empty() {
                "dispatcher".into()
            } else {
                binding.expected_role.clone()
            },
            profile: binding.expected_profile.clone(),
            session_key,
            base_url: binding.observed_base_url.clone(),
            model: binding.expected_model.clone(),
            dispatcher_instance_id: Some(binding.dispatcher_instance_id.clone()),
        });
    }

    attached_instances
}

pub fn project_command_route(instance: &AttachedInstanceRecord) -> CommandRouteProjection {
    CommandRouteProjection {
        route_id: instance.route_id.clone(),
        label: if instance.role.is_empty() {
            instance.instance_id.clone()
        } else {
            format!("{} · {}", instance.role, instance.instance_id)
        },
        detail: instance
            .model
            .clone()
            .or_else(|| instance.base_url.clone())
            .unwrap_or_else(|| instance.profile.clone()),
        target: CommandTarget {
            session_key: instance.session_key.clone(),
            dispatcher_instance_id: instance.dispatcher_instance_id.clone(),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fixtures::{
        DispatcherBindingData, InstanceControlPlaneData, InstanceDescriptorData,
        InstanceIdentityData, InstancePolicyData, SessionData,
    };

    #[test]
    fn rebuilds_route_state_from_session_snapshot() {
        let session = SessionData {
            instance_descriptor: Some(InstanceDescriptorData {
                identity: InstanceIdentityData {
                    instance_id: "omg_host_01HVTEST".into(),
                    role: "host".into(),
                    profile: "control-plane".into(),
                    status: "ready".into(),
                },
                control_plane: Some(InstanceControlPlaneData {
                    schema_version: 2,
                    omegon_version: Some("0.1.0".into()),
                    base_url: Some("http://127.0.0.1:7842".into()),
                    startup_url: None,
                    state_url: None,
                    health_url: None,
                    ready_url: None,
                    ws_url: None,
                    auth_mode: None,
                    token_ref: None,
                    last_ready_at: None,
                    last_verified_at: None,
                    capabilities: vec![],
                }),
                runtime: None,
                workspace: None,
                session: None,
                policy: Some(InstancePolicyData {
                    model: Some("openai:gpt-4.1".into()),
                    thinking_level: None,
                    capability_tier: None,
                }),
            }),
            dispatcher_binding: Some(DispatcherBindingData {
                session_id: "session_01HVTEST".into(),
                dispatcher_instance_id: "omg_dispatcher_01HVTEST".into(),
                expected_role: "primary-driver".into(),
                expected_profile: "primary-interactive".into(),
                expected_model: Some("anthropic:claude-sonnet-4-6".into()),
                control_plane_schema: 2,
                token_ref: None,
                observed_base_url: Some("http://127.0.0.1:7842".into()),
                last_verified_at: None,
                instance_descriptor: None,
                available_options: vec![],
                switch_state: None,
            }),
            ..SessionData::default()
        };

        let engine = AttachedInstanceStateEngine::from_session_snapshot(
            "remote:session_01HVTEST",
            &session,
        );

        assert_eq!(
            engine.attached_instances(),
            &[
                AttachedInstanceRecord {
                    instance_id: "omg_host_01HVTEST".into(),
                    route_id: HOST_CONTROL_PLANE_ROUTE_ID.into(),
                    role: "host".into(),
                    profile: "control-plane".into(),
                    session_key: "remote:session_01HVTEST".into(),
                    base_url: Some("http://127.0.0.1:7842".into()),
                    model: Some("openai:gpt-4.1".into()),
                    dispatcher_instance_id: None,
                },
                AttachedInstanceRecord {
                    instance_id: "omg_dispatcher_01HVTEST".into(),
                    route_id: SESSION_DISPATCHER_ROUTE_ID.into(),
                    role: "primary-driver".into(),
                    profile: "primary-interactive".into(),
                    session_key: "remote:session_01HVTEST".into(),
                    base_url: Some("http://127.0.0.1:7842".into()),
                    model: Some("anthropic:claude-sonnet-4-6".into()),
                    dispatcher_instance_id: Some("omg_dispatcher_01HVTEST".into()),
                },
            ]
        );

        assert_eq!(
            engine.available_command_routes(),
            vec![
                CommandRouteProjection {
                    route_id: HOST_CONTROL_PLANE_ROUTE_ID.into(),
                    label: "host · omg_host_01HVTEST".into(),
                    detail: "openai:gpt-4.1".into(),
                    target: CommandTarget {
                        session_key: "remote:session_01HVTEST".into(),
                        dispatcher_instance_id: None,
                    },
                },
                CommandRouteProjection {
                    route_id: SESSION_DISPATCHER_ROUTE_ID.into(),
                    label: "primary-driver · omg_dispatcher_01HVTEST".into(),
                    detail: "anthropic:claude-sonnet-4-6".into(),
                    target: CommandTarget {
                        session_key: "remote:session_01HVTEST".into(),
                        dispatcher_instance_id: Some("omg_dispatcher_01HVTEST".into()),
                    },
                },
            ]
        );
    }

    #[test]
    fn selects_dispatcher_route_by_default_when_present() {
        let session = SessionData {
            dispatcher_binding: Some(DispatcherBindingData {
                session_id: "session_01HVTEST".into(),
                dispatcher_instance_id: "omg_dispatcher_01HVTEST".into(),
                expected_role: "primary-driver".into(),
                expected_profile: "primary-interactive".into(),
                expected_model: Some("anthropic:claude-sonnet-4-6".into()),
                control_plane_schema: 2,
                token_ref: None,
                observed_base_url: None,
                last_verified_at: None,
                instance_descriptor: None,
                available_options: vec![],
                switch_state: None,
            }),
            ..SessionData::default()
        };

        let mut engine =
            AttachedInstanceStateEngine::from_session_snapshot("remote:session_01HVTEST", &session);

        assert_eq!(engine.selected_command_route_id(), SESSION_DISPATCHER_ROUTE_ID);
        assert_eq!(
            engine.current_command_target(),
            CommandTarget {
                session_key: "remote:session_01HVTEST".into(),
                dispatcher_instance_id: Some("omg_dispatcher_01HVTEST".into()),
            }
        );

        engine.select_command_route(HOST_CONTROL_PLANE_ROUTE_ID);
        assert_eq!(engine.selected_command_route_id(), SESSION_DISPATCHER_ROUTE_ID);
    }

    #[test]
    fn falls_back_to_local_shell_when_no_instances_are_attached() {
        let engine = AttachedInstanceStateEngine::from_session_snapshot("remote:unused", &SessionData::default());

        assert_eq!(engine.selected_command_route_id(), LOCAL_SHELL_ROUTE_ID);
        assert_eq!(
            engine.available_command_routes(),
            vec![CommandRouteProjection {
                route_id: LOCAL_SHELL_ROUTE_ID.into(),
                label: "Local shell".into(),
                detail: "No attached host instance reported".into(),
                target: CommandTarget {
                    session_key: "remote:unused".into(),
                    dispatcher_instance_id: None,
                },
            }]
        );
        assert_eq!(
            engine.current_command_target(),
            CommandTarget {
                session_key: "remote:unused".into(),
                dispatcher_instance_id: None,
            }
        );
    }

    #[test]
    fn keeps_explicit_selection_when_route_exists() {
        let session = SessionData {
            instance_descriptor: Some(InstanceDescriptorData {
                identity: InstanceIdentityData {
                    instance_id: "omg_host_01HVTEST".into(),
                    role: "host".into(),
                    profile: "control-plane".into(),
                    status: "ready".into(),
                },
                workspace: None,
                control_plane: None,
                runtime: None,
                session: None,
                policy: None,
            }),
            dispatcher_binding: Some(DispatcherBindingData {
                session_id: "session_01HVTEST".into(),
                dispatcher_instance_id: "omg_dispatcher_01HVTEST".into(),
                expected_role: "primary-driver".into(),
                expected_profile: "primary-interactive".into(),
                expected_model: None,
                control_plane_schema: 2,
                token_ref: None,
                observed_base_url: None,
                last_verified_at: None,
                instance_descriptor: None,
                available_options: vec![],
                switch_state: None,
            }),
            ..SessionData::default()
        };

        let mut engine =
            AttachedInstanceStateEngine::from_session_snapshot("remote:session_01HVTEST", &session);
        engine.select_command_route(HOST_CONTROL_PLANE_ROUTE_ID);

        assert_eq!(
            engine.selected_command_route_id(),
            HOST_CONTROL_PLANE_ROUTE_ID
        );
        assert_eq!(
            engine.current_command_target(),
            CommandTarget {
                session_key: "remote:session_01HVTEST".into(),
                dispatcher_instance_id: None,
            }
        );
    }
}
