use crate::omegon_control::{
    DispatcherBindingSnapshot, HarnessStatusSnapshot, OmegonControlPlaneDescriptor,
    OmegonInstanceDescriptor, ProviderTelemetrySnapshot,
};

#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct ProviderTelemetryData {
    pub provider: String,
    pub source: String,
    pub requests_remaining: Option<u64>,
    pub tokens_remaining: Option<u64>,
    pub retry_after_secs: Option<u64>,
    pub request_id: Option<String>,
    pub unified_5h_utilization_pct: Option<String>,
    pub unified_7d_utilization_pct: Option<String>,
    pub codex_primary_pct: Option<u64>,
}

#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct ControlPlaneTelemetryData {
    pub startup_url: Option<String>,
    pub health_url: Option<String>,
    pub ready_url: Option<String>,
    pub auth_mode: Option<String>,
    pub base_url: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct SessionTelemetryData {
    pub provider_summary: String,
    pub lifecycle_summary: String,
    pub route_summary: String,
    pub latest_turn_summary: String,
    pub latest_provider_telemetry: Option<ProviderTelemetryData>,
    pub latest_estimated_tokens: Option<u64>,
    pub latest_actual_input_tokens: Option<u64>,
    pub latest_actual_output_tokens: Option<u64>,
    pub latest_cache_read_tokens: Option<u64>,
    pub control_plane: Option<ControlPlaneTelemetryData>,
}

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
        route_summary,
        latest_turn_summary,
        latest_provider_telemetry: latest_turn
            .provider_telemetry
            .clone()
            .map(project_provider_telemetry),
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
                    startup_url: None,
                    health_url: None,
                    ready_url: None,
                    auth_mode: None,
                    base_url: binding.observed_base_url.clone(),
                })
            }),
    }
}

pub fn project_provider_telemetry(snapshot: ProviderTelemetrySnapshot) -> ProviderTelemetryData {
    ProviderTelemetryData {
        provider: snapshot.provider,
        source: snapshot.source,
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
        startup_url: control_plane.startup_url.clone(),
        health_url: control_plane.health_url.clone(),
        ready_url: control_plane.ready_url.clone(),
        auth_mode: control_plane.auth_mode.clone(),
        base_url: control_plane.base_url.clone(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::omegon_control::{
        DelegateSummarySnapshot, OmegonInstanceIdentity, ProviderStatusSnapshot,
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
                base_url: Some("http://127.0.0.1:9999".into()),
                ..ControlPlaneTelemetryData::default()
            })
        );
    }
}
