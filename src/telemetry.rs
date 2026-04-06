use crate::fixtures::{
    ControlPlaneTelemetryData, LifecycleInstanceTelemetryData, LifecycleRollupCountsData,
    LifecycleTelemetryData, ProviderInfo, ProviderTelemetryData, SessionTelemetryData,
};
use crate::instance_registry::InstanceRegistryStore;
use crate::omegon_control::{
    DispatcherBindingSnapshot, HarnessStatusSnapshot, OmegonControlPlaneDescriptor,
    OmegonInstanceDescriptor, ProviderTelemetrySnapshot,
};
use crate::state_engine::AttachedInstanceRecord;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct LatestTurnTelemetry {
    pub provider_telemetry: Option<ProviderTelemetrySnapshot>,
    pub estimated_tokens: Option<u64>,
    pub actual_input_tokens: Option<u64>,
    pub actual_output_tokens: Option<u64>,
    pub cache_read_tokens: Option<u64>,
}

pub fn build_session_telemetry(
    harness: Option<&HarnessStatusSnapshot>,
    turns: u32,
    tool_calls: u32,
    dispatcher: Option<&DispatcherBindingSnapshot>,
    instance_descriptor: Option<&OmegonInstanceDescriptor>,
    latest_turn: &LatestTurnTelemetry,
) -> SessionTelemetryData {
    let authenticated = harness
        .map(|h| h.providers.iter().filter(|provider| provider.authenticated).count())
        .unwrap_or(0);
    let total = harness.map(|h| h.providers.len()).unwrap_or(0);
    let provider_summary = if total == 0 {
        "providers unavailable".into()
    } else {
        format!("{authenticated} / {total} authenticated")
    };

    let route_summary = dispatcher
        .map(|binding| {
            format!(
                "dispatcher {} · {}",
                if binding.dispatcher_instance_id.is_empty() {
                    "unreported"
                } else {
                    binding.dispatcher_instance_id.as_str()
                },
                binding.expected_model.as_deref().unwrap_or("model unreported")
            )
        })
        .or_else(|| {
            instance_descriptor.map(|instance| {
                format!(
                    "host {}",
                    if instance.identity.instance_id.is_empty() {
                        "unreported"
                    } else {
                        instance.identity.instance_id.as_str()
                    }
                )
            })
        })
        .unwrap_or_else(|| "route unavailable".into());

    let lifecycle_summary = harness
        .map(|h| {
            if h.active_delegates.is_empty() {
                "no active delegates".into()
            } else {
                format!("{} active delegate(s)", h.active_delegates.len())
            }
        })
        .unwrap_or_else(|| "lifecycle unavailable".into());

    let latest_turn_summary = format!("turns {turns} · tool calls {tool_calls}");

    SessionTelemetryData {
        provider_summary,
        lifecycle_summary,
        lifecycle: LifecycleTelemetryData::default(),
        route_summary,
        latest_turn_summary,
        latest_provider_telemetry: latest_turn
            .provider_telemetry
            .clone()
            .map(project_provider_telemetry),
        provider_rollups: Vec::new(),
        latest_estimated_tokens: latest_turn.estimated_tokens,
        latest_actual_input_tokens: latest_turn.actual_input_tokens,
        latest_actual_output_tokens: latest_turn.actual_output_tokens,
        latest_cache_read_tokens: latest_turn.cache_read_tokens,
        control_plane: instance_descriptor
            .and_then(|instance| {
                instance
                    .control_plane
                    .as_ref()
                    .map(project_control_plane_telemetry)
            })
            .or_else(|| {
                dispatcher.map(|binding| ControlPlaneTelemetryData {
                    route_id: Some("session-dispatcher".into()),
                    instance_id: (!binding.dispatcher_instance_id.is_empty())
                        .then(|| binding.dispatcher_instance_id.clone()),
                    role: (!binding.expected_role.is_empty()).then(|| binding.expected_role.clone()),
                    profile: (!binding.expected_profile.is_empty())
                        .then(|| binding.expected_profile.clone()),
                    startup_url: None,
                    health_url: None,
                    ready_url: None,
                    auth_mode: None,
                    base_url: binding.observed_base_url.clone(),
                })
            }),
        control_plane_rollups: Vec::new(),
    }
}

pub fn summarize_provider_inventory(providers: &[ProviderInfo]) -> String {
    if providers.is_empty() {
        return "providers unavailable".into();
    }

    let authenticated = providers.iter().filter(|provider| provider.authenticated).count();
    format!("{authenticated} / {} authenticated", providers.len())
}

pub fn aggregate_provider_rollups(
    attached_instances: &[AttachedInstanceRecord],
    providers: &[ProviderInfo],
    selected_route_id: &str,
    latest_provider_telemetry: Option<&ProviderTelemetryData>,
) -> Vec<ProviderTelemetryData> {
    attached_instances
        .iter()
        .map(|instance| ProviderTelemetryData {
            provider: latest_provider_telemetry
                .map(|telemetry| telemetry.provider.clone())
                .unwrap_or_else(|| summarize_instance_provider_label(providers)),
            source: if instance.route_id == selected_route_id {
                latest_provider_telemetry
                    .map(|telemetry| telemetry.source.clone())
                    .unwrap_or_else(|| "inventory".into())
            } else {
                "inventory".into()
            },
            route_id: Some(instance.route_id.clone()),
            instance_id: Some(instance.instance_id.clone()),
            role: Some(instance.role.clone()),
            profile: Some(instance.profile.clone()),
            model: instance.model.clone(),
            requests_remaining: if instance.route_id == selected_route_id {
                latest_provider_telemetry.and_then(|telemetry| telemetry.requests_remaining)
            } else {
                None
            },
            tokens_remaining: if instance.route_id == selected_route_id {
                latest_provider_telemetry.and_then(|telemetry| telemetry.tokens_remaining)
            } else {
                None
            },
            retry_after_secs: if instance.route_id == selected_route_id {
                latest_provider_telemetry.and_then(|telemetry| telemetry.retry_after_secs)
            } else {
                None
            },
            request_id: if instance.route_id == selected_route_id {
                latest_provider_telemetry.and_then(|telemetry| telemetry.request_id.clone())
            } else {
                None
            },
            unified_5h_utilization_pct: if instance.route_id == selected_route_id {
                latest_provider_telemetry
                    .and_then(|telemetry| telemetry.unified_5h_utilization_pct.clone())
            } else {
                None
            },
            unified_7d_utilization_pct: if instance.route_id == selected_route_id {
                latest_provider_telemetry
                    .and_then(|telemetry| telemetry.unified_7d_utilization_pct.clone())
            } else {
                None
            },
            codex_primary_pct: if instance.route_id == selected_route_id {
                latest_provider_telemetry.and_then(|telemetry| telemetry.codex_primary_pct)
            } else {
                None
            },
        })
        .collect()
}

fn summarize_instance_provider_label(providers: &[ProviderInfo]) -> String {
    match providers {
        [] => "unreported".into(),
        [provider] => provider.name.clone(),
        _ => format!("{} providers", providers.len()),
    }
}

pub fn aggregate_control_plane_rollups(
    attached_instances: &[AttachedInstanceRecord],
    selected_route_id: &str,
    selected_control_plane: Option<&ControlPlaneTelemetryData>,
) -> Vec<ControlPlaneTelemetryData> {
    attached_instances
        .iter()
        .map(|instance| ControlPlaneTelemetryData {
            route_id: Some(instance.route_id.clone()),
            instance_id: Some(instance.instance_id.clone()),
            role: Some(instance.role.clone()),
            profile: Some(instance.profile.clone()),
            startup_url: if instance.route_id == selected_route_id {
                selected_control_plane.and_then(|control| control.startup_url.clone())
            } else {
                None
            },
            health_url: if instance.route_id == selected_route_id {
                selected_control_plane.and_then(|control| control.health_url.clone())
            } else {
                None
            },
            ready_url: if instance.route_id == selected_route_id {
                selected_control_plane.and_then(|control| control.ready_url.clone())
            } else {
                None
            },
            auth_mode: if instance.route_id == selected_route_id {
                selected_control_plane.and_then(|control| control.auth_mode.clone())
            } else {
                None
            },
            base_url: instance.base_url.clone().or_else(|| {
                if instance.route_id == selected_route_id {
                    selected_control_plane.and_then(|control| control.base_url.clone())
                } else {
                    None
                }
            }),
        })
        .collect()
}

pub fn aggregate_lifecycle_telemetry(
    attached_instances: &[AttachedInstanceRecord],
    registry_store: &InstanceRegistryStore,
    selected_route_id: &str,
) -> LifecycleTelemetryData {
    let instances: Vec<LifecycleInstanceTelemetryData> = attached_instances
        .iter()
        .map(|instance| project_lifecycle_instance(instance, registry_store))
        .collect();
    let selected_instance = instances
        .iter()
        .find(|instance| instance.route_id == selected_route_id)
        .cloned();

    let summary = if attached_instances.is_empty() {
        "no attached instances".into()
    } else if let Some(selected) = selected_instance.as_ref() {
        let status = selected.status.as_deref().unwrap_or("unknown");
        match selected.freshness.as_deref() {
            Some(freshness) => format!(
                "{} attached instance(s) · {} · freshness {}",
                attached_instances.len(),
                status,
                freshness
            ),
            None => format!("{} attached instance(s) · {}", attached_instances.len(), status),
        }
    } else {
        format!("{} attached instance(s) · route unavailable", attached_instances.len())
    };

    LifecycleTelemetryData {
        summary,
        attached_count: attached_instances.len(),
        selected_route_id: (!selected_route_id.is_empty()).then(|| selected_route_id.to_string()),
        selected_instance,
        counts: summarize_lifecycle_counts(&instances),
        instances,
    }
}

fn summarize_lifecycle_counts(
    instances: &[LifecycleInstanceTelemetryData],
) -> LifecycleRollupCountsData {
    let mut counts = LifecycleRollupCountsData {
        total_attached: instances.len(),
        ..Default::default()
    };

    for instance in instances {
        match instance.freshness.as_deref().unwrap_or("unknown") {
            "fresh" => counts.fresh += 1,
            "stale" => counts.stale += 1,
            "lost" => counts.lost += 1,
            "abandoned" => counts.abandoned += 1,
            "reaped" => counts.reaped += 1,
            _ => counts.unknown += 1,
        }
    }

    counts
}

pub fn project_provider_telemetry(snapshot: ProviderTelemetrySnapshot) -> ProviderTelemetryData {
    ProviderTelemetryData {
        provider: snapshot.provider,
        source: snapshot.source,
        route_id: None,
        instance_id: None,
        role: None,
        profile: None,
        model: None,
        requests_remaining: snapshot.requests_remaining,
        tokens_remaining: snapshot.tokens_remaining,
        retry_after_secs: snapshot.retry_after_secs,
        request_id: snapshot.request_id,
        unified_5h_utilization_pct: snapshot
            .unified_5h_utilization_pct
            .map(|value| format!("{value:.1}")),
        unified_7d_utilization_pct: snapshot
            .unified_7d_utilization_pct
            .map(|value| format!("{value:.1}")),
        codex_primary_pct: snapshot.codex_primary_pct,
    }
}

pub fn project_control_plane_telemetry(
    control_plane: &OmegonControlPlaneDescriptor,
) -> ControlPlaneTelemetryData {
    ControlPlaneTelemetryData {
        route_id: None,
        instance_id: None,
        role: None,
        profile: None,
        startup_url: control_plane.startup_url.clone(),
        health_url: control_plane.health_url.clone(),
        ready_url: control_plane.ready_url.clone(),
        auth_mode: control_plane.auth_mode.clone(),
        base_url: control_plane.base_url.clone(),
    }
}

fn project_lifecycle_instance(
    instance: &AttachedInstanceRecord,
    registry_store: &InstanceRegistryStore,
) -> LifecycleInstanceTelemetryData {
    let registry_record = instance.registry_record.as_ref().or_else(|| {
        registry_store
            .instances
            .iter()
            .find(|record| record.identity.instance_id == instance.instance_id)
    });

    LifecycleInstanceTelemetryData {
        instance_id: instance.instance_id.clone(),
        route_id: instance.route_id.clone(),
        role: instance.role.clone(),
        profile: instance.profile.clone(),
        base_url: instance.base_url.clone(),
        status: registry_record
            .map(|record| format!("{:?}", record.identity.status).to_ascii_lowercase()),
        freshness: registry_record.and_then(|record| {
            record
                .observed
                .health
                .freshness
                .as_ref()
                .map(|freshness| format!("{:?}", freshness).to_ascii_lowercase())
        }),
        last_seen_at: registry_record.and_then(|record| record.observed.health.last_seen_at.clone()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fixtures::SessionData;
    use crate::instance_registry::InstanceRegistryStore;
    use crate::omegon_control::{
        DelegateSummarySnapshot, DispatcherBindingSnapshot, HarnessStatusSnapshot,
        OmegonControlPlaneDescriptor, OmegonInstanceDescriptor, OmegonInstanceIdentity,
        ProviderStatusSnapshot,
    };
    use crate::state_engine::{
        AttachedInstanceRecord, AttachedInstanceStateEngine, HOST_CONTROL_PLANE_ROUTE_ID,
    };

    #[test]
    fn build_session_telemetry_prefers_instance_control_plane_and_projects_turn_metrics() {
        let harness = HarnessStatusSnapshot {
            providers: vec![
                ProviderStatusSnapshot {
                    name: "Anthropic".into(),
                    authenticated: true,
                    auth_method: Some("api-key".into()),
                    model: Some("claude-sonnet".into()),
                },
                ProviderStatusSnapshot {
                    name: "OpenAI".into(),
                    authenticated: false,
                    auth_method: None,
                    model: None,
                },
            ],
            active_delegates: vec![DelegateSummarySnapshot {
                task_id: "task-1".into(),
                agent_name: "general".into(),
                status: "running".into(),
                elapsed_ms: 42,
            }],
            ..HarnessStatusSnapshot::default()
        };
        let dispatcher = DispatcherBindingSnapshot {
            dispatcher_instance_id: "dispatcher-01".into(),
            expected_model: Some("anthropic:claude-sonnet-4-6".into()),
            observed_base_url: Some("http://dispatcher.invalid".into()),
            ..DispatcherBindingSnapshot::default()
        };
        let instance = OmegonInstanceDescriptor {
            identity: OmegonInstanceIdentity {
                instance_id: "host-01".into(),
                ..OmegonInstanceIdentity::default()
            },
            control_plane: Some(OmegonControlPlaneDescriptor {
                startup_url: Some("http://127.0.0.1:7842/startup".into()),
                health_url: Some("http://127.0.0.1:7842/health".into()),
                ready_url: Some("http://127.0.0.1:7842/ready".into()),
                auth_mode: Some("ephemeral-bearer".into()),
                base_url: Some("http://127.0.0.1:7842".into()),
                ..OmegonControlPlaneDescriptor::default()
            }),
            ..OmegonInstanceDescriptor::default()
        };
        let latest_turn = LatestTurnTelemetry {
            provider_telemetry: Some(ProviderTelemetrySnapshot {
                provider: "Anthropic".into(),
                source: "headers".into(),
                unified_5h_utilization_pct: Some(12.34),
                unified_7d_utilization_pct: Some(56.78),
                requests_remaining: Some(9),
                tokens_remaining: Some(1234),
                retry_after_secs: Some(3),
                request_id: Some("req-123".into()),
                codex_primary_pct: Some(88),
                ..ProviderTelemetrySnapshot::default()
            }),
            estimated_tokens: Some(100),
            actual_input_tokens: Some(80),
            actual_output_tokens: Some(20),
            cache_read_tokens: Some(5),
        };

        let telemetry = build_session_telemetry(
            Some(&harness),
            7,
            11,
            Some(&dispatcher),
            Some(&instance),
            &latest_turn,
        );

        assert_eq!(telemetry.provider_summary, "1 / 2 authenticated");
        assert_eq!(telemetry.lifecycle_summary, "1 active delegate(s)");
        assert_eq!(telemetry.route_summary, "dispatcher dispatcher-01 · anthropic:claude-sonnet-4-6");
        assert_eq!(telemetry.latest_turn_summary, "turns 7 · tool calls 11");
        assert_eq!(telemetry.latest_estimated_tokens, Some(100));
        assert_eq!(telemetry.latest_actual_input_tokens, Some(80));
        assert_eq!(telemetry.latest_actual_output_tokens, Some(20));
        assert_eq!(telemetry.latest_cache_read_tokens, Some(5));
        assert_eq!(
            telemetry.latest_provider_telemetry,
            Some(ProviderTelemetryData {
                provider: "Anthropic".into(),
                source: "headers".into(),
                route_id: None,
                instance_id: None,
                role: None,
                profile: None,
                model: None,
                requests_remaining: Some(9),
                tokens_remaining: Some(1234),
                retry_after_secs: Some(3),
                request_id: Some("req-123".into()),
                unified_5h_utilization_pct: Some("12.3".into()),
                unified_7d_utilization_pct: Some("56.8".into()),
                codex_primary_pct: Some(88),
            })
        );
        assert_eq!(
            telemetry.control_plane,
            Some(ControlPlaneTelemetryData {
                route_id: None,
                instance_id: None,
                role: None,
                profile: None,
                startup_url: Some("http://127.0.0.1:7842/startup".into()),
                health_url: Some("http://127.0.0.1:7842/health".into()),
                ready_url: Some("http://127.0.0.1:7842/ready".into()),
                auth_mode: Some("ephemeral-bearer".into()),
                base_url: Some("http://127.0.0.1:7842".into()),
            })
        );
    }

    #[test]
    fn build_session_telemetry_falls_back_to_dispatcher_base_url_without_instance_descriptor() {
        let dispatcher = DispatcherBindingSnapshot {
            observed_base_url: Some("http://127.0.0.1:9999".into()),
            ..DispatcherBindingSnapshot::default()
        };

        let telemetry = build_session_telemetry(
            None,
            4,
            12,
            Some(&dispatcher),
            None,
            &LatestTurnTelemetry::default(),
        );

        assert_eq!(telemetry.provider_summary, "providers unavailable");
        assert_eq!(telemetry.lifecycle_summary, "lifecycle unavailable");
        assert_eq!(telemetry.latest_turn_summary, "turns 4 · tool calls 12");
        assert_eq!(
            telemetry.control_plane,
            Some(ControlPlaneTelemetryData {
                route_id: Some("session-dispatcher".into()),
                instance_id: None,
                role: None,
                profile: None,
                startup_url: None,
                health_url: None,
                ready_url: None,
                auth_mode: None,
                base_url: Some("http://127.0.0.1:9999".into()),
            })
        );
    }

    #[test]
    fn lifecycle_aggregation_reports_selected_instance_freshness() {
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
        engine.replace_registry_store(
            {
                let mut store = engine.registry_store().clone();
                store.instances[0].observed.health.last_seen_at = Some("100".into());
                store
            },
            &SessionData::default(),
        );
        engine.evaluate_lifecycle_policy(100);

        let lifecycle = aggregate_lifecycle_telemetry(
            engine.attached_instances(),
            engine.registry_store(),
            HOST_CONTROL_PLANE_ROUTE_ID,
        );

        assert_eq!(lifecycle.attached_count, 1);
        assert_eq!(lifecycle.selected_route_id.as_deref(), Some(HOST_CONTROL_PLANE_ROUTE_ID));
        assert_eq!(lifecycle.selected_instance.as_ref().unwrap().freshness.as_deref(), Some("fresh"));
    }
}
