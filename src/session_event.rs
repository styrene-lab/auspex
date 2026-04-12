use omegon_traits::IpcEventPayload;

use crate::omegon_control::{OmegonEvent, OmegonStateSnapshot, ProviderTelemetrySnapshot};

#[derive(Clone, Debug, PartialEq)]
pub enum SessionEvent {
    StateSnapshot {
        data: Box<OmegonStateSnapshot>,
    },
    MessageStart {
        role: String,
    },
    MessageDelta {
        text: String,
    },
    ThinkingDelta {
        text: String,
    },
    MessageCompleted,
    MessageAbort,
    SystemNotification {
        message: String,
    },
    HarnessChanged,
    HarnessStatusChanged {
        status: crate::omegon_control::HarnessStatusSnapshot,
    },
    StateChanged {
        sections: Vec<String>,
    },
    SessionReset,
    TurnStarted {
        turn: u32,
    },
    TurnEnded {
        turn: u32,
        estimated_tokens: Option<u64>,
        actual_input_tokens: Option<u64>,
        actual_output_tokens: Option<u64>,
        cache_read_tokens: Option<u64>,
        provider_telemetry: Option<ProviderTelemetrySnapshot>,
    },
    ToolStarted {
        id: String,
        name: String,
        args: Option<serde_json::Value>,
    },
    ToolUpdated {
        id: String,
        partial: Option<String>,
    },
    ToolEnded {
        id: String,
        name: Option<String>,
        is_error: bool,
        result: Option<String>,
    },
    AgentCompleted,
    PhaseChanged {
        phase: String,
    },
    ContextUpdated {
        tokens: u64,
        context_window: Option<u64>,
        context_class: Option<String>,
        thinking_level: Option<String>,
    },
    DecompositionStarted {
        children: Vec<String>,
    },
    DecompositionChildCompleted {
        label: String,
        success: bool,
    },
    DecompositionCompleted {
        merged: bool,
    },
}

impl From<OmegonEvent> for SessionEvent {
    fn from(value: OmegonEvent) -> Self {
        match value {
            OmegonEvent::StateSnapshot { data } => Self::StateSnapshot { data },
            OmegonEvent::MessageStart { role } => Self::MessageStart { role },
            OmegonEvent::MessageChunk { text } => Self::MessageDelta { text },
            OmegonEvent::ThinkingChunk { text } => Self::ThinkingDelta { text },
            OmegonEvent::MessageEnd => Self::MessageCompleted,
            OmegonEvent::MessageAbort => Self::MessageAbort,
            OmegonEvent::SystemNotification { message } => Self::SystemNotification { message },
            OmegonEvent::HarnessStatusChanged { status } => Self::HarnessStatusChanged { status },
            OmegonEvent::SessionReset => Self::SessionReset,
            OmegonEvent::TurnStart { turn } => Self::TurnStarted { turn },
            OmegonEvent::TurnEnd {
                turn,
                estimated_tokens,
                actual_input_tokens,
                actual_output_tokens,
                cache_read_tokens,
                provider_telemetry,
            } => Self::TurnEnded {
                turn,
                estimated_tokens,
                actual_input_tokens,
                actual_output_tokens,
                cache_read_tokens,
                provider_telemetry,
            },
            OmegonEvent::ToolStart { id, name, args } => Self::ToolStarted { id, name, args },
            OmegonEvent::ToolUpdate { id, partial } => Self::ToolUpdated {
                id,
                partial: Some(partial),
            },
            OmegonEvent::ToolEnd {
                id,
                is_error,
                result,
            } => Self::ToolEnded {
                id,
                name: None,
                is_error,
                result,
            },
            OmegonEvent::AgentEnd => Self::AgentCompleted,
            OmegonEvent::PhaseChanged { phase } => Self::PhaseChanged { phase },
            OmegonEvent::ContextUpdated {
                tokens,
                context_window,
                context_class,
                thinking_level,
            } => Self::ContextUpdated {
                tokens,
                context_window,
                context_class,
                thinking_level,
            },
            OmegonEvent::DecompositionStarted { children } => {
                Self::DecompositionStarted { children }
            }
            OmegonEvent::DecompositionChildCompleted { label, success } => {
                Self::DecompositionChildCompleted { label, success }
            }
            OmegonEvent::DecompositionCompleted { merged } => {
                Self::DecompositionCompleted { merged }
            }
        }
    }
}

impl From<IpcEventPayload> for SessionEvent {
    fn from(value: IpcEventPayload) -> Self {
        match value {
            IpcEventPayload::TurnStarted { turn } => Self::TurnStarted { turn },
            IpcEventPayload::TurnEnded {
                turn,
                estimated_tokens,
                actual_input_tokens,
                actual_output_tokens,
                cache_read_tokens,
                provider_telemetry,
                ..
            } => Self::TurnEnded {
                turn,
                estimated_tokens: Some(estimated_tokens as u64),
                actual_input_tokens: Some(actual_input_tokens),
                actual_output_tokens: Some(actual_output_tokens),
                cache_read_tokens: Some(cache_read_tokens),
                provider_telemetry: provider_telemetry.map(|snapshot| ProviderTelemetrySnapshot {
                    provider: snapshot.provider,
                    source: snapshot.source,
                    unified_5h_utilization_pct: snapshot.unified_5h_utilization_pct,
                    unified_7d_utilization_pct: snapshot.unified_7d_utilization_pct,
                    requests_remaining: snapshot.requests_remaining,
                    tokens_remaining: snapshot.tokens_remaining,
                    retry_after_secs: snapshot.retry_after_secs,
                    request_id: snapshot.request_id,
                    codex_active_limit: snapshot.codex_active_limit,
                    codex_primary_used_pct: snapshot.codex_primary_used_pct,
                    codex_secondary_used_pct: snapshot.codex_secondary_used_pct,
                    codex_primary_reset_secs: snapshot.codex_primary_reset_secs,
                    codex_secondary_reset_secs: snapshot.codex_secondary_reset_secs,
                    codex_credits_unlimited: snapshot.codex_credits_unlimited,
                    codex_limit_name: snapshot.codex_limit_name,
                }),
            },
            IpcEventPayload::MessageDelta { text } => Self::MessageDelta { text },
            IpcEventPayload::ThinkingDelta { text } => Self::ThinkingDelta { text },
            IpcEventPayload::MessageCompleted => Self::MessageCompleted,
            IpcEventPayload::ToolStarted { id, name, args } => Self::ToolStarted {
                id,
                name,
                args: Some(args),
            },
            IpcEventPayload::ToolUpdated { id, .. } => Self::ToolUpdated { id, partial: None },
            IpcEventPayload::ToolEnded {
                id,
                name,
                is_error,
                summary,
            } => Self::ToolEnded {
                id,
                name: Some(name),
                is_error,
                result: summary,
            },
            IpcEventPayload::AgentCompleted => Self::AgentCompleted,
            IpcEventPayload::PhaseChanged { phase } => Self::PhaseChanged { phase },
            IpcEventPayload::DecompositionStarted { children } => {
                Self::DecompositionStarted { children }
            }
            IpcEventPayload::DecompositionChildCompleted { label, success } => {
                Self::DecompositionChildCompleted { label, success }
            }
            IpcEventPayload::DecompositionCompleted { merged } => {
                Self::DecompositionCompleted { merged }
            }
            IpcEventPayload::FamilyVitalSignsUpdated { .. } => Self::SystemNotification {
                message: "Family vital signs updated".into(),
            },
            IpcEventPayload::HarnessChanged => Self::HarnessChanged,
            IpcEventPayload::StateChanged { sections } => Self::StateChanged { sections },
            IpcEventPayload::SystemNotification { message } => Self::SystemNotification { message },
            IpcEventPayload::SessionReset => Self::SessionReset,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn websocket_events_normalize_into_transport_neutral_variants() {
        let event = OmegonEvent::MessageChunk {
            text: "hello".into(),
        };

        assert_eq!(
            SessionEvent::from(event),
            SessionEvent::MessageDelta {
                text: "hello".into()
            }
        );
    }

    #[test]
    fn ipc_events_normalize_into_transport_neutral_variants() {
        let event = IpcEventPayload::ToolEnded {
            id: "tool-1".into(),
            name: "read".into(),
            is_error: false,
            summary: Some("ok".into()),
        };

        assert_eq!(
            SessionEvent::from(event),
            SessionEvent::ToolEnded {
                id: "tool-1".into(),
                name: Some("read".into()),
                is_error: false,
                result: Some("ok".into()),
            }
        );
    }
}
