#![allow(dead_code)]

use crate::fixtures::SessionData;
use crate::instance_registry::InstanceRegistryStore;
use crate::runtime_types::{CommandTarget, InstanceRecord};

pub const HOST_CONTROL_PLANE_ROUTE_ID: &str = "host-control-plane";
pub const SESSION_DISPATCHER_ROUTE_ID: &str = "session-dispatcher";
pub const LOCAL_SHELL_ROUTE_ID: &str = "local-shell";
const STALE_AFTER_SECONDS: u64 = 300;
const ABANDON_AFTER_SECONDS: u64 = 1800;

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
    pub registry_record: Option<InstanceRecord>,
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
    registry_store: InstanceRegistryStore,
}

impl AttachedInstanceStateEngine {
    pub fn from_session_snapshot(session_key: impl Into<String>, session: &SessionData) -> Self {
        Self::from_registry_and_session(InstanceRegistryStore::default(), session_key, session)
    }

    pub fn from_registry_and_session(
        registry_store: InstanceRegistryStore,
        session_key: impl Into<String>,
        session: &SessionData,
    ) -> Self {
        let session_key = session_key.into();
        let attached_instances = reconcile_attached_instances(&registry_store, session_key.clone(), session);
        let registry_store = merge_attached_instances_into_registry(registry_store, &attached_instances);
        Self {
            attached_instances,
            session_key,
            selected_command_route_id: None,
            registry_store,
        }
    }

    pub fn attached_instances(&self) -> &[AttachedInstanceRecord] {
        &self.attached_instances
    }

    pub fn registry_store(&self) -> &InstanceRegistryStore {
        &self.registry_store
    }

    pub fn reconcile_session_snapshot(&mut self, session_key: impl Into<String>, session: &SessionData) {
        let session_key = session_key.into();
        let selected_route = self.selected_command_route_id();
        let attached_instances = reconcile_attached_instances(&self.registry_store, session_key.clone(), session);
        self.registry_store = merge_attached_instances_into_registry(self.registry_store.clone(), &attached_instances);
        self.attached_instances = attached_instances;
        self.session_key = session_key;
        self.selected_command_route_id = Some(selected_route);
    }

    pub fn replace_registry_store(&mut self, registry_store: InstanceRegistryStore, session: &SessionData) {
        let session_key = self.session_key.clone();
        let selected_route = self.selected_command_route_id();
        let attached_instances = reconcile_attached_instances(&registry_store, session_key, session);
        self.registry_store = merge_attached_instances_into_registry(registry_store, &attached_instances);
        self.attached_instances = attached_instances;
        self.selected_command_route_id = Some(selected_route);
    }

    pub fn select_command_route(&mut self, route_id: impl Into<String>) {
        self.selected_command_route_id = Some(route_id.into());
    }

    pub fn attach_instance(&mut self, instance: AttachedInstanceRecord) {
        let next_record = synthesize_instance_record(&instance);
        if let Some(position) = self
            .registry_store
            .instances
            .iter()
            .position(|record| record.identity.instance_id == next_record.identity.instance_id)
        {
            self.registry_store.instances[position] = next_record.clone();
        } else {
            self.registry_store.instances.push(next_record.clone());
        }

        let next_instance = AttachedInstanceRecord {
            registry_record: Some(next_record),
            ..instance
        };
        if let Some(position) = self
            .attached_instances
            .iter()
            .position(|existing| existing.instance_id == next_instance.instance_id)
        {
            self.attached_instances[position] = next_instance;
        } else {
            self.attached_instances.push(next_instance);
        }
    }

    pub fn detach_instance(&mut self, instance_id: &str) {
        self.attached_instances
            .retain(|instance| instance.instance_id != instance_id);
        self.registry_store.instances.retain(|record| {
            !(record.identity.instance_id == instance_id
                && record.ownership.owner_kind == crate::runtime_types::OwnerKind::AuspexSession
                && record.ownership.owner_id == self.session_key.trim_start_matches("remote:"))
        });
    }

    pub fn purge_stale_instances(&mut self, active_instance_ids: &[String]) {
        let active: std::collections::HashSet<&str> =
            active_instance_ids.iter().map(String::as_str).collect();
        self.attached_instances
            .retain(|instance| active.contains(instance.instance_id.as_str()));
        for record in &mut self.registry_store.instances {
            if record.ownership.owner_kind == crate::runtime_types::OwnerKind::AuspexSession
                && record.ownership.owner_id == self.session_key.trim_start_matches("remote:")
                && !active.contains(record.identity.instance_id.as_str())
                && record.identity.role == crate::runtime_types::WorkerRole::DetachedService
            {
                record.identity.status = crate::runtime_types::WorkerLifecycleState::Lost;
                record.observed.health.ready = false;
                record.observed.health.freshness =
                    Some(crate::runtime_types::InstanceFreshness::Stale);
            }
        }
        self.registry_store.instances.retain(|record| {
            record.ownership.owner_kind != crate::runtime_types::OwnerKind::AuspexSession
                || record.ownership.owner_id != self.session_key.trim_start_matches("remote:")
                || active.contains(record.identity.instance_id.as_str())
                || record.identity.role == crate::runtime_types::WorkerRole::DetachedService
        });
    }

    pub fn evaluate_lifecycle_policy(&mut self, now_epoch_seconds: u64) {
        for record in &mut self.registry_store.instances {
            let Some(last_seen_at) = record.observed.health.last_seen_at.as_deref() else {
                continue;
            };
            let Ok(last_seen_epoch) = last_seen_at.parse::<u64>() else {
                continue;
            };
            let age = now_epoch_seconds.saturating_sub(last_seen_epoch);

            if age >= ABANDON_AFTER_SECONDS {
                record.identity.status = crate::runtime_types::WorkerLifecycleState::Abandoned;
                record.observed.health.ready = false;
                record.observed.health.freshness =
                    Some(crate::runtime_types::InstanceFreshness::Abandoned);
            } else if age >= STALE_AFTER_SECONDS {
                if record.identity.status == crate::runtime_types::WorkerLifecycleState::Ready {
                    record.identity.status = crate::runtime_types::WorkerLifecycleState::Lost;
                }
                record.observed.health.ready = false;
                record.observed.health.freshness =
                    Some(crate::runtime_types::InstanceFreshness::Stale);
            } else {
                record.observed.health.freshness =
                    Some(crate::runtime_types::InstanceFreshness::Fresh);
            }
        }
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
    reconcile_attached_instances(&InstanceRegistryStore::default(), session_key, session)
}

pub fn reconcile_attached_instances(
    registry_store: &InstanceRegistryStore,
    session_key: impl Into<String>,
    session: &SessionData,
) -> Vec<AttachedInstanceRecord> {
    let session_key = session_key.into();
    let mut attached_instances = Vec::new();

    if let Some(instance) = session.instance_descriptor.as_ref() {
        let instance_id = if instance.identity.instance_id.is_empty() {
            HOST_CONTROL_PLANE_ROUTE_ID.into()
        } else {
            instance.identity.instance_id.clone()
        };
        let registry_record = registry_store
            .instances
            .iter()
            .find(|record| record.identity.instance_id == instance_id)
            .cloned();
        attached_instances.push(AttachedInstanceRecord {
            instance_id,
            route_id: HOST_CONTROL_PLANE_ROUTE_ID.into(),
            role: if instance.identity.role.is_empty() {
                "host".into()
            } else {
                instance.identity.role.clone()
            },
            profile: if instance.identity.profile.is_empty() {
                registry_record
                    .as_ref()
                    .map(|record| record.identity.profile.clone())
                    .unwrap_or_default()
            } else {
                instance.identity.profile.clone()
            },
            session_key: session_key.clone(),
            base_url: instance
                .control_plane
                .as_ref()
                .and_then(|control_plane| control_plane.base_url.clone())
                .or_else(|| registry_record.as_ref().map(|record| record.observed.control_plane.base_url.clone())),
            model: instance
                .policy
                .as_ref()
                .and_then(|policy| policy.model.clone())
                .or_else(|| registry_record.as_ref().and_then(|record| record.desired.policy.model.clone())),
            dispatcher_instance_id: None,
            registry_record,
        });
    } else if let Some(registry_record) = registry_store
        .instances
        .iter()
        .find(|record| {
            record.ownership.owner_kind == crate::runtime_types::OwnerKind::AuspexSession
                && record.ownership.owner_id == session_key.trim_start_matches("remote:")
                && record.identity.instance_id != session
                    .dispatcher_binding
                    .as_ref()
                    .map(|binding| binding.dispatcher_instance_id.as_str())
                    .unwrap_or_default()
        })
        .cloned()
    {
        attached_instances.push(AttachedInstanceRecord {
            instance_id: registry_record.identity.instance_id.clone(),
            route_id: HOST_CONTROL_PLANE_ROUTE_ID.into(),
            role: match registry_record.identity.role {
                crate::runtime_types::WorkerRole::PrimaryDriver => "primary-driver".into(),
                crate::runtime_types::WorkerRole::SupervisedChild => "supervised-child".into(),
                crate::runtime_types::WorkerRole::DetachedService => "detached-service".into(),
            },
            profile: registry_record.identity.profile.clone(),
            session_key: session_key.clone(),
            base_url: Some(registry_record.observed.control_plane.base_url.clone()),
            model: registry_record.desired.policy.model.clone(),
            dispatcher_instance_id: None,
            registry_record: Some(registry_record),
        });
    }

    if let Some(binding) = session.dispatcher_binding.as_ref() {
        let instance_id = if binding.dispatcher_instance_id.is_empty() {
            SESSION_DISPATCHER_ROUTE_ID.into()
        } else {
            binding.dispatcher_instance_id.clone()
        };
        let registry_record = registry_store
            .instances
            .iter()
            .find(|record| record.identity.instance_id == instance_id)
            .cloned();
        attached_instances.push(AttachedInstanceRecord {
            instance_id,
            route_id: SESSION_DISPATCHER_ROUTE_ID.into(),
            role: if binding.expected_role.is_empty() {
                registry_record
                    .as_ref()
                    .map(|record| match record.identity.role {
                        crate::runtime_types::WorkerRole::PrimaryDriver => "primary-driver".to_string(),
                        crate::runtime_types::WorkerRole::SupervisedChild => "supervised-child".to_string(),
                        crate::runtime_types::WorkerRole::DetachedService => "detached-service".to_string(),
                    })
                    .unwrap_or_else(|| "dispatcher".into())
            } else {
                binding.expected_role.clone()
            },
            profile: if binding.expected_profile.is_empty() {
                registry_record
                    .as_ref()
                    .map(|record| record.identity.profile.clone())
                    .unwrap_or_default()
            } else {
                binding.expected_profile.clone()
            },
            session_key,
            base_url: binding.observed_base_url.clone().or_else(|| {
                registry_record
                    .as_ref()
                    .map(|record| record.observed.control_plane.base_url.clone())
            }),
            model: binding.expected_model.clone().or_else(|| {
                registry_record
                    .as_ref()
                    .and_then(|record| record.desired.policy.model.clone())
            }),
            dispatcher_instance_id: Some(binding.dispatcher_instance_id.clone()),
            registry_record,
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

fn merge_attached_instances_into_registry(
    mut registry_store: InstanceRegistryStore,
    attached_instances: &[AttachedInstanceRecord],
) -> InstanceRegistryStore {
    let active_ids: std::collections::HashSet<&str> =
        attached_instances.iter().map(|instance| instance.instance_id.as_str()).collect();
    for instance in attached_instances {
        let next_record = synthesize_instance_record(instance);
        if let Some(position) = registry_store
            .instances
            .iter()
            .position(|record| record.identity.instance_id == next_record.identity.instance_id)
        {
            registry_store.instances[position] = next_record;
        } else {
            registry_store.instances.push(next_record);
        }
    }

    for record in &mut registry_store.instances {
        if active_ids.contains(record.identity.instance_id.as_str()) {
            record.observed.health.freshness = Some(crate::runtime_types::InstanceFreshness::Fresh);
            if record.observed.health.last_seen_at.is_none() {
                record.observed.health.last_seen_at = record.observed.health.last_heartbeat_at.clone();
            }
        }
    }
    registry_store
}

fn synthesize_instance_record(instance: &AttachedInstanceRecord) -> InstanceRecord {
    let mut record = instance.registry_record.clone().unwrap_or_else(|| InstanceRecord {
        schema_version: 1,
        identity: crate::runtime_types::WorkerIdentity {
            instance_id: instance.instance_id.clone(),
            role: infer_worker_role(instance),
            profile: instance.profile.clone(),
            status: crate::runtime_types::WorkerLifecycleState::Ready,
            created_at: String::new(),
            updated_at: String::new(),
        },
        ownership: crate::runtime_types::WorkerOwnership {
            owner_kind: crate::runtime_types::OwnerKind::AuspexSession,
            owner_id: instance.session_key.trim_start_matches("remote:").to_string(),
            parent_instance_id: None,
        },
        desired: crate::runtime_types::DesiredWorkerState {
            backend: crate::runtime_types::BackendConfig {
                kind: crate::runtime_types::BackendKind::LocalProcess,
                image: None,
                namespace: None,
                resources: None,
            },
            workspace: crate::runtime_types::WorkspaceBinding {
                cwd: String::new(),
                workspace_id: String::new(),
                branch: None,
            },
            task: None,
            policy: crate::runtime_types::PolicyOverrides::default(),
        },
        observed: crate::runtime_types::ObservedWorkerState {
            placement: crate::runtime_types::ObservedPlacement {
                placement_id: String::new(),
                host: String::new(),
                pid: None,
                namespace: None,
                pod_name: None,
                container_name: None,
            },
            control_plane: crate::runtime_types::ObservedControlPlane {
                schema_version: 0,
                omegon_version: String::new(),
                base_url: String::new(),
                startup_url: String::new(),
                health_url: String::new(),
                ready_url: String::new(),
                ws_url: String::new(),
                auth_mode: String::new(),
                token_ref: None,
                last_ready_at: None,
            },
            health: crate::runtime_types::ObservedHealth {
                ready: true,
                degraded_reason: None,
                last_heartbeat_at: None,
                last_seen_at: None,
                freshness: Some(crate::runtime_types::InstanceFreshness::Fresh),
            },
            exit: crate::runtime_types::ObservedExit {
                exited: false,
                exit_code: None,
                exit_reason: None,
                exited_at: None,
            },
        },
    });

    record.identity.instance_id = instance.instance_id.clone();
    record.identity.role = infer_worker_role(instance);
    record.identity.profile = instance.profile.clone();
    record.identity.status = crate::runtime_types::WorkerLifecycleState::Ready;
    record.ownership.owner_kind = crate::runtime_types::OwnerKind::AuspexSession;
    record.ownership.owner_id = instance.session_key.trim_start_matches("remote:").to_string();
    record.desired.policy.model = instance.model.clone();
    record.observed.health.ready = true;
    record.observed.health.freshness = Some(crate::runtime_types::InstanceFreshness::Fresh);
    if record.observed.health.last_seen_at.is_none() {
        record.observed.health.last_seen_at = record.observed.health.last_heartbeat_at.clone();
    }
    if let Some(base_url) = instance.base_url.clone() {
        record.observed.control_plane.base_url = base_url;
    }
    record
}

fn infer_worker_role(instance: &AttachedInstanceRecord) -> crate::runtime_types::WorkerRole {
    if instance.dispatcher_instance_id.is_some() {
        crate::runtime_types::WorkerRole::PrimaryDriver
    } else {
        crate::runtime_types::WorkerRole::DetachedService
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
                    registry_record: None,
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
                    registry_record: None,
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
    fn reconciles_registry_metadata_into_live_attached_instances() {
        let session = SessionData {
            dispatcher_binding: Some(DispatcherBindingData {
                session_id: "session_01HVTEST".into(),
                dispatcher_instance_id: "omg_dispatcher_01HVTEST".into(),
                expected_role: "".into(),
                expected_profile: "".into(),
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

        let store = InstanceRegistryStore {
            schema_version: 1,
            instances: vec![InstanceRecord {
                schema_version: 1,
                identity: crate::runtime_types::WorkerIdentity {
                    instance_id: "omg_dispatcher_01HVTEST".into(),
                    role: crate::runtime_types::WorkerRole::PrimaryDriver,
                    profile: "primary-interactive".into(),
                    status: crate::runtime_types::WorkerLifecycleState::Ready,
                    created_at: "2026-04-06T00:00:00Z".into(),
                    updated_at: "2026-04-06T00:00:01Z".into(),
                },
                ownership: crate::runtime_types::WorkerOwnership {
                    owner_kind: crate::runtime_types::OwnerKind::AuspexSession,
                    owner_id: "session_01HVTEST".into(),
                    parent_instance_id: None,
                },
                desired: crate::runtime_types::DesiredWorkerState {
                    backend: crate::runtime_types::BackendConfig {
                        kind: crate::runtime_types::BackendKind::LocalProcess,
                        image: None,
                        namespace: None,
                        resources: None,
                    },
                    workspace: crate::runtime_types::WorkspaceBinding {
                        cwd: "/repo".into(),
                        workspace_id: "repo:test".into(),
                        branch: Some("main".into()),
                    },
                    task: None,
                    policy: crate::runtime_types::PolicyOverrides {
                        model: Some("anthropic:claude-sonnet-4-6".into()),
                        ..Default::default()
                    },
                },
                observed: crate::runtime_types::ObservedWorkerState {
                    placement: crate::runtime_types::ObservedPlacement {
                        placement_id: "pid/8123".into(),
                        host: "desktop:local".into(),
                        pid: Some(8123),
                        namespace: None,
                        pod_name: None,
                        container_name: None,
                    },
                    control_plane: crate::runtime_types::ObservedControlPlane {
                        schema_version: 2,
                        omegon_version: "0.15.10-rc.17".into(),
                        base_url: "http://127.0.0.1:7842".into(),
                        startup_url: "http://127.0.0.1:7842/api/startup".into(),
                        health_url: "http://127.0.0.1:7842/api/healthz".into(),
                        ready_url: "http://127.0.0.1:7842/api/readyz".into(),
                        ws_url: "ws://127.0.0.1:7842/ws".into(),
                        auth_mode: "ephemeral-bearer".into(),
                        token_ref: Some("secret://auspex/instances/omg_dispatcher_01HVTEST/token".into()),
                        last_ready_at: Some("2026-04-06T00:00:02Z".into()),
                    },
                    health: crate::runtime_types::ObservedHealth {
                        ready: true,
                        degraded_reason: None,
                        last_heartbeat_at: Some("2026-04-06T00:00:03Z".into()),
                        last_seen_at: Some("2026-04-06T00:00:03Z".into()),
                        freshness: Some(crate::runtime_types::InstanceFreshness::Fresh),
                    },
                    exit: crate::runtime_types::ObservedExit {
                        exited: false,
                        exit_code: None,
                        exit_reason: None,
                        exited_at: None,
                    },
                },
            }],
        };

        let engine = AttachedInstanceStateEngine::from_registry_and_session(
            store,
            "remote:session_01HVTEST",
            &session,
        );

        let attached = engine.attached_instances();
        assert_eq!(attached.len(), 1);
        assert_eq!(attached[0].role, "primary-driver");
        assert_eq!(attached[0].profile, "primary-interactive");
        assert_eq!(attached[0].base_url.as_deref(), Some("http://127.0.0.1:7842"));
        assert_eq!(attached[0].model.as_deref(), Some("anthropic:claude-sonnet-4-6"));
        assert!(attached[0].registry_record.is_some());
    }

    #[test]
    fn state_engine_merges_reconciled_instances_back_into_registry_store() {
        let session = SessionData {
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

        let engine = AttachedInstanceStateEngine::from_registry_and_session(
            InstanceRegistryStore::default(),
            "remote:session_01HVTEST",
            &session,
        );

        assert!(engine
            .registry_store()
            .instances
            .iter()
            .any(|record| record.identity.instance_id == "omg_dispatcher_01HVTEST"));
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

    #[test]
    fn attach_instance_merges_into_registry_and_route_state() {
        let mut engine = AttachedInstanceStateEngine::default();
        engine.reconcile_session_snapshot("remote:session_01HVTEST", &SessionData::default());
        engine.attach_instance(AttachedInstanceRecord {
            instance_id: "omg_host_01HVTEST".into(),
            route_id: HOST_CONTROL_PLANE_ROUTE_ID.into(),
            role: "host".into(),
            profile: "control-plane".into(),
            session_key: "remote:session_01HVTEST".into(),
            base_url: Some("http://127.0.0.1:7842".into()),
            model: Some("openai:gpt-4.1".into()),
            dispatcher_instance_id: None,
            registry_record: None,
        });

        assert!(engine
            .attached_instances()
            .iter()
            .any(|instance| instance.instance_id == "omg_host_01HVTEST"));
        assert!(engine
            .registry_store()
            .instances
            .iter()
            .any(|record| record.identity.instance_id == "omg_host_01HVTEST"));
    }

    #[test]
    fn detach_instance_removes_session_owned_registry_record() {
        let mut engine = AttachedInstanceStateEngine::from_registry_and_session(
            InstanceRegistryStore::default(),
            "remote:session_01HVTEST",
            &SessionData::default(),
        );
        engine.attach_instance(AttachedInstanceRecord {
            instance_id: "omg_host_01HVTEST".into(),
            route_id: HOST_CONTROL_PLANE_ROUTE_ID.into(),
            role: "host".into(),
            profile: "control-plane".into(),
            session_key: "remote:session_01HVTEST".into(),
            base_url: Some("http://127.0.0.1:7842".into()),
            model: Some("openai:gpt-4.1".into()),
            dispatcher_instance_id: None,
            registry_record: None,
        });
        engine.detach_instance("omg_host_01HVTEST");

        assert!(!engine
            .attached_instances()
            .iter()
            .any(|instance| instance.instance_id == "omg_host_01HVTEST"));
        assert!(!engine
            .registry_store()
            .instances
            .iter()
            .any(|record| record.identity.instance_id == "omg_host_01HVTEST"));
    }

    #[test]
    fn purge_stale_instances_removes_non_active_session_records() {
        let mut engine = AttachedInstanceStateEngine::from_registry_and_session(
            InstanceRegistryStore::default(),
            "remote:session_01HVTEST",
            &SessionData::default(),
        );
        engine.attach_instance(AttachedInstanceRecord {
            instance_id: "omg_host_01HVTEST".into(),
            route_id: HOST_CONTROL_PLANE_ROUTE_ID.into(),
            role: "host".into(),
            profile: "control-plane".into(),
            session_key: "remote:session_01HVTEST".into(),
            base_url: Some("http://127.0.0.1:7842".into()),
            model: Some("openai:gpt-4.1".into()),
            dispatcher_instance_id: None,
            registry_record: None,
        });
        engine.attach_instance(AttachedInstanceRecord {
            instance_id: "omg_dispatcher_01HVTEST".into(),
            route_id: SESSION_DISPATCHER_ROUTE_ID.into(),
            role: "primary-driver".into(),
            profile: "primary-interactive".into(),
            session_key: "remote:session_01HVTEST".into(),
            base_url: Some("http://127.0.0.1:7842".into()),
            model: Some("anthropic:claude-sonnet-4-6".into()),
            dispatcher_instance_id: Some("omg_dispatcher_01HVTEST".into()),
            registry_record: None,
        });

        engine.purge_stale_instances(&["omg_dispatcher_01HVTEST".into()]);

        assert_eq!(engine.attached_instances().len(), 1);
        assert_eq!(engine.attached_instances()[0].instance_id, "omg_dispatcher_01HVTEST");
        assert_eq!(engine.registry_store().instances.len(), 2);
        assert_eq!(engine.registry_store().instances[0].identity.instance_id, "omg_host_01HVTEST");
        assert_eq!(engine.registry_store().instances[0].identity.status, crate::runtime_types::WorkerLifecycleState::Lost);
        assert_eq!(
            engine.registry_store().instances[0].observed.health.freshness,
            Some(crate::runtime_types::InstanceFreshness::Stale)
        );
        assert_eq!(engine.registry_store().instances[1].identity.instance_id, "omg_dispatcher_01HVTEST");
    }

    #[test]
    fn evaluate_lifecycle_policy_marks_stale_before_abandonment() {
        let mut engine = AttachedInstanceStateEngine::from_registry_and_session(
            InstanceRegistryStore::default(),
            "remote:session_01HVTEST",
            &SessionData::default(),
        );
        engine.attach_instance(AttachedInstanceRecord {
            instance_id: "omg_service_01HVTEST".into(),
            route_id: HOST_CONTROL_PLANE_ROUTE_ID.into(),
            role: "detached-service".into(),
            profile: "background-sync".into(),
            session_key: "remote:session_01HVTEST".into(),
            base_url: Some("http://127.0.0.1:9001".into()),
            model: Some("anthropic:claude-haiku".into()),
            dispatcher_instance_id: None,
            registry_record: None,
        });
        engine.registry_store.instances[0].observed.health.last_seen_at = Some("100".into());

        engine.evaluate_lifecycle_policy(100 + STALE_AFTER_SECONDS + 1);

        assert_eq!(
            engine.registry_store().instances[0].identity.status,
            crate::runtime_types::WorkerLifecycleState::Lost
        );
        assert_eq!(
            engine.registry_store().instances[0].observed.health.freshness,
            Some(crate::runtime_types::InstanceFreshness::Stale)
        );
    }

    #[test]
    fn evaluate_lifecycle_policy_marks_abandoned_after_long_expiry() {
        let mut engine = AttachedInstanceStateEngine::from_registry_and_session(
            InstanceRegistryStore::default(),
            "remote:session_01HVTEST",
            &SessionData::default(),
        );
        engine.attach_instance(AttachedInstanceRecord {
            instance_id: "omg_service_01HVTEST".into(),
            route_id: HOST_CONTROL_PLANE_ROUTE_ID.into(),
            role: "detached-service".into(),
            profile: "background-sync".into(),
            session_key: "remote:session_01HVTEST".into(),
            base_url: Some("http://127.0.0.1:9001".into()),
            model: Some("anthropic:claude-haiku".into()),
            dispatcher_instance_id: None,
            registry_record: None,
        });
        engine.registry_store.instances[0].observed.health.last_seen_at = Some("100".into());

        engine.evaluate_lifecycle_policy(100 + ABANDON_AFTER_SECONDS + 1);

        assert_eq!(
            engine.registry_store().instances[0].identity.status,
            crate::runtime_types::WorkerLifecycleState::Abandoned
        );
        assert_eq!(
            engine.registry_store().instances[0].observed.health.freshness,
            Some(crate::runtime_types::InstanceFreshness::Abandoned)
        );
    }

    #[test]
    fn purge_stale_instances_marks_detached_service_stale_before_reap() {
        let mut engine = AttachedInstanceStateEngine::from_registry_and_session(
            InstanceRegistryStore::default(),
            "remote:session_01HVTEST",
            &SessionData::default(),
        );
        engine.attach_instance(AttachedInstanceRecord {
            instance_id: "omg_service_01HVTEST".into(),
            route_id: HOST_CONTROL_PLANE_ROUTE_ID.into(),
            role: "detached-service".into(),
            profile: "background-sync".into(),
            session_key: "remote:session_01HVTEST".into(),
            base_url: Some("http://127.0.0.1:9001".into()),
            model: Some("anthropic:claude-haiku".into()),
            dispatcher_instance_id: None,
            registry_record: Some(InstanceRecord {
                schema_version: 1,
                identity: crate::runtime_types::WorkerIdentity {
                    instance_id: "omg_service_01HVTEST".into(),
                    role: crate::runtime_types::WorkerRole::DetachedService,
                    profile: "background-sync".into(),
                    status: crate::runtime_types::WorkerLifecycleState::Ready,
                    created_at: String::new(),
                    updated_at: String::new(),
                },
                ownership: crate::runtime_types::WorkerOwnership {
                    owner_kind: crate::runtime_types::OwnerKind::AuspexSession,
                    owner_id: "session_01HVTEST".into(),
                    parent_instance_id: None,
                },
                desired: crate::runtime_types::DesiredWorkerState {
                    backend: crate::runtime_types::BackendConfig {
                        kind: crate::runtime_types::BackendKind::LocalProcess,
                        image: None,
                        namespace: None,
                        resources: None,
                    },
                    workspace: crate::runtime_types::WorkspaceBinding {
                        cwd: String::new(),
                        workspace_id: String::new(),
                        branch: None,
                    },
                    task: None,
                    policy: crate::runtime_types::PolicyOverrides {
                        model: Some("anthropic:claude-haiku".into()),
                        ..Default::default()
                    },
                },
                observed: crate::runtime_types::ObservedWorkerState {
                    placement: crate::runtime_types::ObservedPlacement {
                        placement_id: String::new(),
                        host: String::new(),
                        pid: None,
                        namespace: None,
                        pod_name: None,
                        container_name: None,
                    },
                    control_plane: crate::runtime_types::ObservedControlPlane {
                        schema_version: 2,
                        omegon_version: String::new(),
                        base_url: "http://127.0.0.1:9001".into(),
                        startup_url: String::new(),
                        health_url: String::new(),
                        ready_url: String::new(),
                        ws_url: String::new(),
                        auth_mode: String::new(),
                        token_ref: None,
                        last_ready_at: None,
                    },
                    health: crate::runtime_types::ObservedHealth {
                        ready: true,
                        degraded_reason: None,
                        last_heartbeat_at: None,
                        last_seen_at: None,
                        freshness: Some(crate::runtime_types::InstanceFreshness::Fresh),
                    },
                    exit: crate::runtime_types::ObservedExit {
                        exited: false,
                        exit_code: None,
                        exit_reason: None,
                        exited_at: None,
                    },
                },
            }),
        });

        engine.purge_stale_instances(&[]);

        assert!(engine.attached_instances().is_empty());
        assert_eq!(engine.registry_store().instances.len(), 1);
        assert_eq!(
            engine.registry_store().instances[0].identity.status,
            crate::runtime_types::WorkerLifecycleState::Lost
        );
        assert_eq!(
            engine.registry_store().instances[0].observed.health.freshness,
            Some(crate::runtime_types::InstanceFreshness::Stale)
        );
    }
}
