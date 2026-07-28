//! Desktop-only MQTT bearer boundary for managed-agent A2A traffic.
//!
//! This module deliberately owns only configuration, canonical envelope creation,
//! and the `styrene-mqtt` client. Managed-run state remains in
//! `managed_agent_runtime`; MQTT is a bearer, not a second supervisor model.

use std::time::Duration;

use serde::{Deserialize, Serialize};
use styrene_a2a::{AgentEnvelope, AgentEnvelopeKind, AgentId, RootOperationId, RuntimeId};
use styrene_mqtt::{MqttA2aClient, ReceivedA2aEnvelope};

use crate::managed_agents::{ManagedRunId, ManagedRunRequest, WorkerId};

pub const MANAGED_RUN_REQUEST_SCHEMA: &str = "io.styrene.auspex.managed-run-request.v1";
pub const MANAGED_RUN_OUTCOME_SCHEMA: &str = "io.styrene.auspex.managed-run-outcome.v1";

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ManagedRunA2aOutcome {
    Accepted,
    Duplicate,
    Completed { result: String },
    Failed { code: String, safe_message: String },
    CancellationAccepted { reason: Option<String> },
    TerminationConfirmed { reason: Option<String> },
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ManagedRunA2aOutcomePayload {
    pub managed_run_id: ManagedRunId,
    pub outcome: ManagedRunA2aOutcome,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ManagedAgentA2aEvent {
    Accepted {
        managed_run_id: ManagedRunId,
        task_id: String,
        message_id: [u8; 16],
    },
    Duplicate {
        managed_run_id: ManagedRunId,
        task_id: String,
        message_id: [u8; 16],
    },
    Completed {
        managed_run_id: ManagedRunId,
        task_id: String,
        message_id: [u8; 16],
        result: String,
    },
    Failed {
        managed_run_id: ManagedRunId,
        task_id: String,
        message_id: [u8; 16],
        code: String,
        safe_message: String,
    },
    CancellationAccepted {
        managed_run_id: ManagedRunId,
        task_id: String,
        message_id: [u8; 16],
        reason: Option<String>,
    },
    TerminationConfirmed {
        managed_run_id: ManagedRunId,
        task_id: String,
        message_id: [u8; 16],
        reason: Option<String>,
    },
}

impl ManagedAgentA2aEvent {
    pub fn managed_run_id(&self) -> ManagedRunId {
        match self {
            Self::Accepted { managed_run_id, .. }
            | Self::Duplicate { managed_run_id, .. }
            | Self::Completed { managed_run_id, .. }
            | Self::Failed { managed_run_id, .. }
            | Self::CancellationAccepted { managed_run_id, .. }
            | Self::TerminationConfirmed { managed_run_id, .. } => *managed_run_id,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ManagedAgentMqttConfig {
    pub tenant: String,
    pub host: String,
    pub port: u16,
    pub client_id: String,
    pub local_agent_id: AgentId,
    pub target_agent_id: AgentId,
    pub session_expiry: Duration,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ManagedRunA2aRequest {
    pub managed_run_id: ManagedRunId,
    pub worker_id: WorkerId,
    pub parent_session_id: String,
    pub parent_turn_id: String,
    pub request: ManagedRunRequest,
}

pub struct ManagedAgentMqttBridge {
    client: MqttA2aClient,
    config: ManagedAgentMqttConfig,
    runtime_id: RuntimeId,
    next_sequence: u64,
}

impl ManagedAgentMqttBridge {
    pub fn connect(config: ManagedAgentMqttConfig) -> Self {
        let client = MqttA2aClient::connect_persistent(
            config.tenant.clone(),
            config.client_id.clone(),
            config.host.clone(),
            config.port,
            Duration::from_secs(15),
            64,
            config.session_expiry,
        );
        Self {
            client,
            config,
            runtime_id: RuntimeId::new(),
            next_sequence: 1,
        }
    }

    pub async fn initialize(&mut self) -> Result<(), ManagedAgentMqttError> {
        self.client
            .subscribe_agent(self.config.local_agent_id.as_str())
            .await?;
        // Progress CONNACK and SUBACK before accepting local dispatch work.
        self.client.poll_transport().await?;
        self.client.poll_transport().await?;
        Ok(())
    }

    pub fn command_envelope(
        &mut self,
        command: ManagedRunA2aRequest,
        now_ms: u64,
        ttl: Duration,
    ) -> Result<AgentEnvelope, ManagedAgentMqttError> {
        let task_id = command.managed_run_id.to_string();
        let payload = serde_json::to_vec(&command)?;
        let root = RootOperationId::new(command.parent_session_id.clone())?;
        let mut envelope = AgentEnvelope::new(
            AgentEnvelopeKind::Command,
            &self.config.local_agent_id,
            self.runtime_id,
            &self.config.target_agent_id,
            &root,
            Some(task_id.clone()),
            task_id,
            self.next_sequence,
            now_ms,
            MANAGED_RUN_REQUEST_SCHEMA,
            payload,
        );
        envelope.parent_task_id = Some(command.parent_turn_id);
        envelope.expires_at_ms = Some(now_ms.saturating_add(ttl.as_millis() as u64));
        self.next_sequence = self
            .next_sequence
            .checked_add(1)
            .ok_or(ManagedAgentMqttError::SequenceExhausted)?;
        Ok(envelope)
    }

    pub async fn publish_command(
        &mut self,
        envelope: &AgentEnvelope,
        now_ms: u64,
    ) -> Result<(), ManagedAgentMqttError> {
        self.client.publish(envelope, now_ms).await?;
        self.client.flush_publish().await?;
        Ok(())
    }

    pub async fn receive(
        &mut self,
        now_ms: u64,
    ) -> Result<ReceivedA2aEnvelope, ManagedAgentMqttError> {
        Ok(self.client.recv(now_ms).await?)
    }
    pub async fn receive_event(
        &mut self,
        now_ms: u64,
    ) -> Result<ManagedAgentA2aEvent, ManagedAgentMqttError> {
        let received = self.receive(now_ms).await?;
        self.decode_event(received)
    }

    pub fn decode_event(
        &self,
        received: ReceivedA2aEnvelope,
    ) -> Result<ManagedAgentA2aEvent, ManagedAgentMqttError> {
        let envelope = received.envelope;
        if envelope.target_agent_id != self.config.local_agent_id.as_str() {
            return Err(ManagedAgentMqttError::WrongTarget);
        }
        if envelope.payload_schema != MANAGED_RUN_OUTCOME_SCHEMA {
            return Err(ManagedAgentMqttError::UnsupportedSchema(
                envelope.payload_schema,
            ));
        }
        let task_id = envelope
            .task_id
            .clone()
            .ok_or(ManagedAgentMqttError::MissingTaskId)?;
        let payload: ManagedRunA2aOutcomePayload = serde_json::from_slice(&envelope.a2a_payload)?;
        if payload.managed_run_id.to_string() != task_id {
            return Err(ManagedAgentMqttError::RunIdentityMismatch);
        }
        let common = (payload.managed_run_id, task_id, envelope.message_id);
        Ok(match payload.outcome {
            ManagedRunA2aOutcome::Accepted => ManagedAgentA2aEvent::Accepted {
                managed_run_id: common.0,
                task_id: common.1,
                message_id: common.2,
            },
            ManagedRunA2aOutcome::Duplicate => ManagedAgentA2aEvent::Duplicate {
                managed_run_id: common.0,
                task_id: common.1,
                message_id: common.2,
            },
            ManagedRunA2aOutcome::Completed { result } => ManagedAgentA2aEvent::Completed {
                managed_run_id: common.0,
                task_id: common.1,
                message_id: common.2,
                result,
            },
            ManagedRunA2aOutcome::Failed { code, safe_message } => ManagedAgentA2aEvent::Failed {
                managed_run_id: common.0,
                task_id: common.1,
                message_id: common.2,
                code,
                safe_message,
            },
            ManagedRunA2aOutcome::CancellationAccepted { reason } => {
                ManagedAgentA2aEvent::CancellationAccepted {
                    managed_run_id: common.0,
                    task_id: common.1,
                    message_id: common.2,
                    reason,
                }
            }
            ManagedRunA2aOutcome::TerminationConfirmed { reason } => {
                ManagedAgentA2aEvent::TerminationConfirmed {
                    managed_run_id: common.0,
                    task_id: common.1,
                    message_id: common.2,
                    reason,
                }
            }
        })
    }
}

#[derive(Debug, thiserror::Error)]
pub enum ManagedAgentMqttError {
    #[error(transparent)]
    Mqtt(#[from] styrene_mqtt::MqttA2aError),
    #[error(transparent)]
    Json(#[from] serde_json::Error),
    #[error(transparent)]
    Extension(#[from] styrene_a2a::ExtensionValidationError),
    #[error("managed-agent outcome targeted another agent")]
    WrongTarget,
    #[error("unsupported managed-agent outcome schema {0}")]
    UnsupportedSchema(String),
    #[error("managed-agent outcome has no task id")]
    MissingTaskId,
    #[error("managed-agent payload and envelope run identities differ")]
    RunIdentityMismatch,
    #[error("managed-agent MQTT stream sequence exhausted")]
    SequenceExhausted,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::managed_agents::WorkerProfile;
    use std::collections::BTreeSet;

    #[test]
    fn builds_canonical_managed_run_command() {
        let config = ManagedAgentMqttConfig {
            tenant: "local".into(),
            host: "127.0.0.1".into(),
            port: 1883,
            client_id: "auspex-local".into(),
            local_agent_id: AgentId::new("styrene:agent:auspex").unwrap(),
            target_agent_id: AgentId::new("styrene:agent:omegon").unwrap(),
            session_expiry: Duration::from_secs(900),
        };
        let mut bridge = ManagedAgentMqttBridge::connect(config);
        let request = ManagedRunA2aRequest {
            managed_run_id: ManagedRunId::new(),
            worker_id: WorkerId::new(),
            parent_session_id: "session-1".into(),
            parent_turn_id: "turn-4".into(),
            request: ManagedRunRequest::new(
                "inspect",
                WorkerProfile::Scout,
                BTreeSet::from(["src".into()]),
                30,
            )
            .unwrap(),
        };
        let envelope = bridge
            .command_envelope(request.clone(), 1_000, Duration::from_secs(30))
            .unwrap();
        assert_eq!(envelope.kind, AgentEnvelopeKind::Command);
        assert_eq!(envelope.sequence, 1);
        assert_eq!(envelope.parent_task_id.as_deref(), Some("turn-4"));
        assert_eq!(envelope.expires_at_ms, Some(31_000));
        assert_eq!(
            serde_json::from_slice::<ManagedRunA2aRequest>(&envelope.a2a_payload).unwrap(),
            request
        );
    }
}
