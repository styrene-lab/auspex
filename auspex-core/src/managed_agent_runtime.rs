use std::collections::BTreeMap;
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use serde_json::Value;

#[cfg(not(target_arch = "wasm32"))]
use crate::controller::ManagedAgentTransportAction;
#[cfg(not(target_arch = "wasm32"))]
use crate::managed_agent_mqtt_orchestrator::{
    ManagedAgentMqttOrchestratorEvent, ManagedAgentMqttOrchestratorHandle,
};

#[cfg(not(target_arch = "wasm32"))]
#[derive(Clone)]
pub struct ManagedAgentA2aDispatcher {
    handle: ManagedAgentMqttOrchestratorHandle,
}

#[cfg(not(target_arch = "wasm32"))]
impl ManagedAgentA2aDispatcher {
    pub fn new(handle: ManagedAgentMqttOrchestratorHandle) -> Self {
        Self { handle }
    }

    pub async fn execute(&self, action: ManagedAgentTransportAction) -> Result<(), String> {
        match action {
            ManagedAgentTransportAction::StyreneA2aDispatch(request) => {
                self.handle.dispatch(request).await
            }
            ManagedAgentTransportAction::StyreneA2aCancel {
                cancellation,
                root_operation_id,
            } => self.handle.cancel(cancellation, root_operation_id).await,
        }
    }

    pub fn apply_event(
        runtime: &mut ManagedAgentSupervisorRuntime,
        event: ManagedAgentMqttOrchestratorEvent,
    ) -> Result<(), String> {
        match event {
            ManagedAgentMqttOrchestratorEvent::Outcome(outcome) => runtime
                .apply_a2a_event(&outcome)
                .map(|_| ())
                .map_err(|error| format!("MQTT A2A outcome rejected: {error:?}")),
            ManagedAgentMqttOrchestratorEvent::Disconnected { error }
            | ManagedAgentMqttOrchestratorEvent::CommandFailed { error, .. } => {
                Err(format!("MQTT A2A transport: {error}"))
            }
            ManagedAgentMqttOrchestratorEvent::Connected
            | ManagedAgentMqttOrchestratorEvent::CommandPublished { .. }
            | ManagedAgentMqttOrchestratorEvent::Stopped => Ok(()),
        }
    }
}

use crate::managed_agent_supervisor::{
    ControlCommandReceipt, ControlCommandReceiptStatus, DelegateCancelRequest,
    DelegateCancelResponse, DelegateDispatchRequest, DelegateDispatchResponse,
    DelegateObservationResponse, DelegateResultResponse, DelegateTaskRequest,
    ManagedAgentCommandAck, ManagedAgentCommandAckStatus, SupervisorContractError,
};
use crate::managed_agents::{
    ManagedAgentRun, ManagedAgentValidationError, ManagedRunId, ManagedRunRequest,
    ManagedRunTransitionError, SupervisorEvent, WorkerId,
};
use crate::runtime_types::{CommandTarget, ManagedCommandId, TargetedCommand};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ManagedAgentSupervisorRuntime {
    runs: BTreeMap<ManagedRunId, RuntimeEntry>,
    commands: BTreeMap<ManagedCommandId, PendingManagedCommand>,
    command_runs: BTreeMap<ManagedCommandId, ManagedRunId>,
    max_result_bytes: usize,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct PendingManagedCommand {
    command: TargetedCommand,
    kind: ManagedCommandKind,
    run_id: ManagedRunId,
    worker_id: WorkerId,
    task_id: Option<crate::managed_agents::OmegonTaskId>,
    state: ManagedCommandDeliveryState,
    attempts: u32,
    last_sent_at: Instant,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ManagedCommandKind {
    Dispatch,
    Poll,
    Cancel,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ManagedCommandDeliveryState {
    Pending,
    Accepted,
    Duplicate,
    RetryPending { code: String },
    Rejected { code: String },
    ProvenByResult,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ManagedCommandProjectionKind {
    Dispatch,
    Poll,
    Cancel,
}

impl From<ManagedCommandKind> for ManagedCommandProjectionKind {
    fn from(value: ManagedCommandKind) -> Self {
        match value {
            ManagedCommandKind::Dispatch => Self::Dispatch,
            ManagedCommandKind::Poll => Self::Poll,
            ManagedCommandKind::Cancel => Self::Cancel,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ManagedCommandDeliveryProjectionState {
    Pending,
    Accepted,
    Duplicate,
    RetryPending { code: String },
    Rejected { code: String },
    ProvenByResult,
}

impl From<&ManagedCommandDeliveryState> for ManagedCommandDeliveryProjectionState {
    fn from(value: &ManagedCommandDeliveryState) -> Self {
        match value {
            ManagedCommandDeliveryState::Pending => Self::Pending,
            ManagedCommandDeliveryState::Accepted => Self::Accepted,
            ManagedCommandDeliveryState::Duplicate => Self::Duplicate,
            ManagedCommandDeliveryState::RetryPending { code } => {
                Self::RetryPending { code: code.clone() }
            }
            ManagedCommandDeliveryState::Rejected { code } => Self::Rejected { code: code.clone() },
            ManagedCommandDeliveryState::ProvenByResult => Self::ProvenByResult,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ManagedCommandDeliveryProjection {
    pub command_id: ManagedCommandId,
    pub method: String,
    pub command_kind: ManagedCommandProjectionKind,
    pub attempts: u32,
    pub state: ManagedCommandDeliveryProjectionState,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ManagedAgentRunProjection {
    pub schema_version: u32,
    pub run_id: ManagedRunId,
    pub worker_id: WorkerId,
    pub parent_session_id: String,
    pub parent_turn_id: String,
    pub task_id: Option<crate::managed_agents::OmegonTaskId>,
    pub state: crate::managed_agents::ManagedRunState,
    pub commands: Vec<ManagedCommandDeliveryProjection>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ManagedAgentProjectionError {
    UnknownRun,
    ParentSessionMismatch,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ManagedAgentTransportBinding {
    OmegonControl {
        instance_id: String,
        target: CommandTarget,
    },
    StyreneA2a {
        target_agent_id: String,
    },
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct RuntimeEntry {
    run: ManagedAgentRun,
    transport: ManagedAgentTransportBinding,
    dispatched_at_unix_ms: u64,
    dispatched_at: Instant,
    deadline_at: Instant,
    events: Vec<SupervisorEvent>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SupervisorRuntimeError {
    UnknownRun,
    UnknownCommand,
    MissingCommandId,
    DuplicateRun,
    CommandAlreadyPending,
    UnexpectedResponseType(String),
    ResponseRejected { code: String, safe_message: String },
    Contract(SupervisorContractError),
    Domain(ManagedRunTransitionError),
    Validation(ManagedAgentValidationError),
    InvalidJson(String),
    WrongTransport,
    DeadlineExceeded,
}

impl From<SupervisorContractError> for SupervisorRuntimeError {
    fn from(value: SupervisorContractError) -> Self {
        Self::Contract(value)
    }
}
impl From<ManagedRunTransitionError> for SupervisorRuntimeError {
    fn from(value: ManagedRunTransitionError) -> Self {
        Self::Domain(value)
    }
}
impl From<ManagedAgentValidationError> for SupervisorRuntimeError {
    fn from(value: ManagedAgentValidationError) -> Self {
        Self::Validation(value)
    }
}

impl ManagedAgentSupervisorRuntime {
    pub fn new(max_result_bytes: usize) -> Self {
        Self {
            runs: BTreeMap::new(),
            commands: BTreeMap::new(),
            command_runs: BTreeMap::new(),
            max_result_bytes,
        }
    }

    fn register_command(
        &mut self,
        run_id: ManagedRunId,
        kind: ManagedCommandKind,
        command: TargetedCommand,
    ) -> Result<TargetedCommand, SupervisorRuntimeError> {
        let command_id = command
            .managed_command_id()
            .ok_or(SupervisorRuntimeError::MissingCommandId)?;
        let entry = self
            .runs
            .get(&run_id)
            .ok_or(SupervisorRuntimeError::UnknownRun)?;
        let worker_id = entry.run.worker_id();
        let task_id = entry.run.omegon_task_id().cloned();
        self.command_runs.insert(command_id, run_id);
        self.commands.insert(
            command_id,
            PendingManagedCommand {
                command: command.clone(),
                kind,
                run_id,
                worker_id,
                task_id,
                state: ManagedCommandDeliveryState::Pending,
                attempts: 1,
                last_sent_at: Instant::now(),
            },
        );
        Ok(command)
    }

    pub fn prepare_run(
        &mut self,
        worker_id: WorkerId,
        parent_session_id: impl Into<String>,
        parent_turn_id: impl Into<String>,
        request: ManagedRunRequest,
        transport: ManagedAgentTransportBinding,
        now_unix_ms: u64,
    ) -> Result<ManagedRunId, SupervisorRuntimeError> {
        let run = ManagedAgentRun::new(worker_id, parent_session_id, parent_turn_id, request)?;
        let run_id = run.run_id();
        if self.runs.contains_key(&run_id) {
            return Err(SupervisorRuntimeError::DuplicateRun);
        }
        let now = Instant::now();
        let deadline_at = now
            .checked_add(Duration::from_secs(
                run.request().supervisor_deadline_seconds(),
            ))
            .unwrap_or(now);
        self.runs.insert(
            run_id,
            RuntimeEntry {
                run,
                transport,
                dispatched_at_unix_ms: now_unix_ms,
                dispatched_at: now,
                deadline_at,
                events: vec![SupervisorEvent::DispatchRequested { run_id, worker_id }],
            },
        );
        Ok(run_id)
    }

    pub fn dispatch(
        &mut self,
        worker_id: WorkerId,
        parent_session_id: impl Into<String>,
        parent_turn_id: impl Into<String>,
        request: ManagedRunRequest,
        target: CommandTarget,
        now_unix_ms: u64,
    ) -> Result<(ManagedRunId, TargetedCommand), SupervisorRuntimeError> {
        let run_id = self.prepare_run(
            worker_id,
            parent_session_id,
            parent_turn_id,
            request,
            ManagedAgentTransportBinding::OmegonControl {
                instance_id: String::new(),
                target: target.clone(),
            },
            now_unix_ms,
        )?;
        let request = self
            .runs
            .get(&run_id)
            .expect("prepared run exists")
            .run
            .request()
            .clone();
        let command = DelegateDispatchRequest::new(run_id, worker_id, request).command(target)?;
        let command = self.register_command(run_id, ManagedCommandKind::Dispatch, command)?;
        Ok((run_id, command))
    }

    pub fn poll_command(
        &mut self,
        run_id: ManagedRunId,
    ) -> Result<TargetedCommand, SupervisorRuntimeError> {
        if self.commands.iter().any(|(command_id, pending)| {
            self.command_runs.get(command_id) == Some(&run_id)
                && pending.kind == ManagedCommandKind::Poll
                && pending.state == ManagedCommandDeliveryState::Pending
        }) {
            return Err(SupervisorRuntimeError::CommandAlreadyPending);
        }
        let entry = self
            .runs
            .get(&run_id)
            .ok_or(SupervisorRuntimeError::UnknownRun)?;
        let task_id = entry
            .run
            .omegon_task_id()
            .ok_or(ManagedRunTransitionError::DispatchNotAccepted)?
            .clone();
        let target = match &entry.transport {
            ManagedAgentTransportBinding::OmegonControl { target, .. } => target.clone(),
            ManagedAgentTransportBinding::StyreneA2a { .. } => {
                return Err(SupervisorRuntimeError::WrongTransport);
            }
        };
        let command =
            DelegateTaskRequest::new(run_id, entry.run.worker_id(), task_id).get_command(target)?;
        self.register_command(run_id, ManagedCommandKind::Poll, command)
    }

    pub fn cancel_command(
        &mut self,
        run_id: ManagedRunId,
        reason: Option<String>,
    ) -> Result<TargetedCommand, SupervisorRuntimeError> {
        let entry = self
            .runs
            .get_mut(&run_id)
            .ok_or(SupervisorRuntimeError::UnknownRun)?;
        let task_id = entry
            .run
            .omegon_task_id()
            .ok_or(ManagedRunTransitionError::DispatchNotAccepted)?
            .clone();
        entry.run.request_cancellation(reason.clone())?;
        entry.events.push(SupervisorEvent::CancellationRequested {
            run_id,
            reason: reason.clone(),
        });
        let target = match &entry.transport {
            ManagedAgentTransportBinding::OmegonControl { target, .. } => target.clone(),
            ManagedAgentTransportBinding::StyreneA2a { .. } => {
                return Err(SupervisorRuntimeError::WrongTransport);
            }
        };
        let command = DelegateCancelRequest::new(run_id, entry.run.worker_id(), task_id, reason)?
            .command(target)?;
        self.register_command(run_id, ManagedCommandKind::Cancel, command)
    }

    pub fn request_a2a_cancellation(
        &mut self,
        run_id: ManagedRunId,
        reason: Option<String>,
    ) -> Result<crate::managed_agent_mqtt::ManagedRunA2aCancel, SupervisorRuntimeError> {
        let entry = self
            .runs
            .get_mut(&run_id)
            .ok_or(SupervisorRuntimeError::UnknownRun)?;
        if !matches!(
            entry.transport,
            ManagedAgentTransportBinding::StyreneA2a { .. }
        ) {
            return Err(SupervisorRuntimeError::WrongTransport);
        }
        entry.run.request_cancellation(reason.clone())?;
        entry.events.push(SupervisorEvent::CancellationRequested {
            run_id,
            reason: reason.clone(),
        });
        Ok(crate::managed_agent_mqtt::ManagedRunA2aCancel {
            managed_run_id: run_id,
            worker_id: entry.run.worker_id(),
            reason,
        })
    }

    pub fn apply_receipt_json(
        &mut self,
        json: &str,
    ) -> Result<ManagedCommandId, SupervisorRuntimeError> {
        let receipt: ControlCommandReceipt = serde_json::from_str(json)
            .map_err(|error| SupervisorRuntimeError::InvalidJson(error.to_string()))?;
        if receipt.schema_version != 1 {
            return Err(SupervisorRuntimeError::Contract(
                SupervisorContractError::UnsupportedSchema {
                    received: receipt.schema_version,
                },
            ));
        }
        let pending = self
            .commands
            .get_mut(&receipt.command_id)
            .ok_or(SupervisorRuntimeError::UnknownCommand)?;
        pending.state = match receipt.receipt_status {
            ControlCommandReceiptStatus::Accepted => ManagedCommandDeliveryState::Accepted,
            ControlCommandReceiptStatus::Duplicate => ManagedCommandDeliveryState::Duplicate,
            ControlCommandReceiptStatus::Rejected => ManagedCommandDeliveryState::Rejected {
                code: receipt
                    .rejection
                    .as_ref()
                    .map(|rejection| rejection.code.clone())
                    .unwrap_or_else(|| "rejected".into()),
            },
        };
        Ok(receipt.command_id)
    }

    pub fn apply_command_ack_json(
        &mut self,
        json: &str,
    ) -> Result<ManagedCommandId, SupervisorRuntimeError> {
        let ack: ManagedAgentCommandAck = serde_json::from_str(json)
            .map_err(|error| SupervisorRuntimeError::InvalidJson(error.to_string()))?;
        ack.validate()?;
        let pending = self
            .commands
            .get_mut(&ack.command_id)
            .ok_or(SupervisorRuntimeError::UnknownCommand)?;
        if pending.run_id != ack.managed_run_id
            || pending.worker_id != ack.worker_id
            || pending.task_id != ack.task_id
            || pending.command.method() != Some(ack.method.as_str())
        {
            return Err(SupervisorRuntimeError::Contract(
                SupervisorContractError::IdentityMismatch,
            ));
        }
        pending.state = match ack.status {
            ManagedAgentCommandAckStatus::Accepted => ManagedCommandDeliveryState::Accepted,
            ManagedAgentCommandAckStatus::Rejected => {
                let rejection = ack.rejection.expect("validated rejection exists");
                if rejection.retryable {
                    ManagedCommandDeliveryState::RetryPending {
                        code: rejection.code,
                    }
                } else {
                    ManagedCommandDeliveryState::Rejected {
                        code: rejection.code,
                    }
                }
            }
        };
        Ok(ack.command_id)
    }

    pub fn replay_due_commands(&mut self, receipt_timeout: Duration) -> Vec<TargetedCommand> {
        let now = Instant::now();
        self.commands
            .values_mut()
            .filter_map(|pending| {
                if matches!(
                    pending.state,
                    ManagedCommandDeliveryState::Pending
                        | ManagedCommandDeliveryState::RetryPending { .. }
                ) && now.duration_since(pending.last_sent_at) >= receipt_timeout
                {
                    pending.last_sent_at = now;
                    pending.attempts = pending.attempts.saturating_add(1);
                    pending.state = ManagedCommandDeliveryState::Pending;
                    Some(pending.command.clone())
                } else {
                    None
                }
            })
            .collect()
    }

    pub fn command_run_id(&self, command_id: ManagedCommandId) -> Option<ManagedRunId> {
        self.command_runs.get(&command_id).copied()
    }

    pub fn command_delivery_state(
        &self,
        command_id: ManagedCommandId,
    ) -> Option<&ManagedCommandDeliveryState> {
        self.commands.get(&command_id).map(|pending| &pending.state)
    }

    pub fn project_run_for_parent(
        &self,
        run_id: ManagedRunId,
        parent_session_id: &str,
    ) -> Result<ManagedAgentRunProjection, ManagedAgentProjectionError> {
        let entry = self
            .runs
            .get(&run_id)
            .ok_or(ManagedAgentProjectionError::UnknownRun)?;
        if entry.run.parent_session_id() != parent_session_id {
            return Err(ManagedAgentProjectionError::ParentSessionMismatch);
        }
        let commands = self
            .commands
            .iter()
            .filter(|(_, command)| command.run_id == run_id)
            .map(|(command_id, command)| ManagedCommandDeliveryProjection {
                command_id: *command_id,
                method: command.command.method().unwrap_or_default().to_string(),
                command_kind: command.kind.into(),
                attempts: command.attempts,
                state: (&command.state).into(),
            })
            .collect();
        Ok(ManagedAgentRunProjection {
            schema_version: 1,
            run_id,
            worker_id: entry.run.worker_id(),
            parent_session_id: entry.run.parent_session_id().to_string(),
            parent_turn_id: entry.run.parent_turn_id().to_string(),
            task_id: entry.run.omegon_task_id().cloned(),
            state: entry.run.state().clone(),
            commands,
        })
    }

    pub fn project_runs_for_parent(
        &self,
        parent_session_id: &str,
    ) -> Vec<ManagedAgentRunProjection> {
        self.runs
            .keys()
            .filter_map(|run_id| self.project_run_for_parent(*run_id, parent_session_id).ok())
            .collect()
    }

    pub fn apply_response_json(
        &mut self,
        raw: &str,
    ) -> Result<ManagedRunId, SupervisorRuntimeError> {
        let value: Value = serde_json::from_str(raw)
            .map_err(|error| SupervisorRuntimeError::InvalidJson(error.to_string()))?;
        let response_type = value
            .get("type")
            .and_then(Value::as_str)
            .unwrap_or_default();
        let run_id: ManagedRunId =
            serde_json::from_value(value.get("managed_run_id").cloned().ok_or_else(|| {
                SupervisorRuntimeError::InvalidJson("missing managed_run_id".into())
            })?)
            .map_err(|error| SupervisorRuntimeError::InvalidJson(error.to_string()))?;

        if let Some(rejection) = value.get("rejection").filter(|value| !value.is_null()) {
            let rejection: crate::managed_agent_supervisor::SupervisorRejection =
                serde_json::from_value(rejection.clone())
                    .map_err(|error| SupervisorRuntimeError::InvalidJson(error.to_string()))?;
            if let Some(entry) = self.runs.get_mut(&run_id) {
                entry.events.push(SupervisorEvent::DispatchRejected {
                    run_id,
                    reason: rejection.safe_message.clone(),
                });
            }
            return Err(SupervisorRuntimeError::ResponseRejected {
                code: rejection.code,
                safe_message: rejection.safe_message,
            });
        }

        let response_kind = match response_type {
            "delegate_dispatch_result" => ManagedCommandKind::Dispatch,
            "delegate_get_result" | "delegate_result_result" => ManagedCommandKind::Poll,
            "delegate_cancel_result" => ManagedCommandKind::Cancel,
            other => {
                return Err(SupervisorRuntimeError::UnexpectedResponseType(
                    other.to_string(),
                ));
            }
        };

        let entry = self
            .runs
            .get_mut(&run_id)
            .ok_or(SupervisorRuntimeError::UnknownRun)?;
        match response_type {
            "delegate_dispatch_result" => {
                let response: DelegateDispatchResponse = serde_json::from_value(value)
                    .map_err(|error| SupervisorRuntimeError::InvalidJson(error.to_string()))?;
                let task_id = response
                    .validate_for(&run_id, &entry.run.worker_id())?
                    .clone();
                entry.run.accept_dispatch(task_id.clone())?;
                entry
                    .events
                    .push(SupervisorEvent::DispatchAccepted { run_id, task_id });
            }
            "delegate_get_result" => {
                let response: DelegateObservationResponse = serde_json::from_value(value)
                    .map_err(|error| SupervisorRuntimeError::InvalidJson(error.to_string()))?;
                let expected = entry
                    .run
                    .omegon_task_id()
                    .ok_or(ManagedRunTransitionError::DispatchNotAccepted)?
                    .clone();
                response.validate_for(&run_id, &entry.run.worker_id(), &expected)?;
                entry.run.apply_observation(response.observation)?;
            }
            "delegate_cancel_result" => {
                let response: DelegateCancelResponse = serde_json::from_value(value)
                    .map_err(|error| SupervisorRuntimeError::InvalidJson(error.to_string()))?;
                let expected = entry
                    .run
                    .omegon_task_id()
                    .ok_or(ManagedRunTransitionError::DispatchNotAccepted)?
                    .clone();
                response.validate_for(&run_id, &entry.run.worker_id(), &expected)?;
                entry.run.apply_cancellation_response(
                    response.acknowledged,
                    response.termination_confirmed,
                    response.reason,
                )?;
            }
            "delegate_result_result" => {
                let response: DelegateResultResponse = serde_json::from_value(value)
                    .map_err(|error| SupervisorRuntimeError::InvalidJson(error.to_string()))?;
                let expected = entry
                    .run
                    .omegon_task_id()
                    .ok_or(ManagedRunTransitionError::DispatchNotAccepted)?
                    .clone();
                response.validate_for(
                    &run_id,
                    &entry.run.worker_id(),
                    &expected,
                    self.max_result_bytes,
                )?;
                entry.run.complete_from_result(response.result)?;
            }
            other => unreachable!("response kind validated above: {other}"),
        }
        if let Some(pending) = self.commands.values_mut().find(|pending| {
            pending.run_id == run_id
                && pending.kind == response_kind
                && matches!(
                    pending.state,
                    ManagedCommandDeliveryState::Pending
                        | ManagedCommandDeliveryState::RetryPending { .. }
                )
        }) {
            pending.state = ManagedCommandDeliveryState::ProvenByResult;
        }
        Ok(run_id)
    }

    pub fn apply_a2a_event(
        &mut self,
        event: &crate::managed_agent_mqtt::ManagedAgentA2aEvent,
    ) -> Result<ManagedRunId, SupervisorRuntimeError> {
        use crate::managed_agent_mqtt::ManagedAgentA2aEvent;
        let run_id = event.managed_run_id();
        let entry = self
            .runs
            .get_mut(&run_id)
            .ok_or(SupervisorRuntimeError::UnknownRun)?;
        match event {
            ManagedAgentA2aEvent::Accepted { task_id, .. }
            | ManagedAgentA2aEvent::Duplicate { task_id, .. } => {
                if matches!(
                    entry.run.state(),
                    crate::managed_agents::ManagedRunState::Dispatching
                ) {
                    entry
                        .run
                        .accept_dispatch(crate::managed_agents::OmegonTaskId::parse(
                            task_id.clone(),
                        )?)?;
                }
            }
            ManagedAgentA2aEvent::Completed { result, .. } => {
                entry.run.complete_from_result(result.clone())?
            }
            ManagedAgentA2aEvent::Failed {
                code, safe_message, ..
            } => entry.run.fail(code.clone(), safe_message.clone())?,
            ManagedAgentA2aEvent::CancellationAccepted { reason, .. } => {
                entry.run.mark_cancellation_accepted(reason.clone())?
            }
            ManagedAgentA2aEvent::TerminationConfirmed { reason, .. } => {
                entry.run.confirm_termination(reason.clone())?
            }
        }
        Ok(run_id)
    }

    pub fn mark_worker_disconnected(&mut self, worker_id: WorkerId) {
        for entry in self
            .runs
            .values_mut()
            .filter(|entry| entry.run.worker_id() == worker_id && !entry.run.state().is_terminal())
        {
            let run_id = entry.run.run_id();
            if entry.run.mark_disconnected().is_ok() {
                entry
                    .events
                    .push(SupervisorEvent::WorkerDisconnected { run_id });
            }
        }
    }

    pub fn remove_run(&mut self, run_id: ManagedRunId) -> Option<ManagedAgentRun> {
        self.runs.remove(&run_id).map(|entry| entry.run)
    }

    pub fn pollable_run_ids(&self) -> Vec<ManagedRunId> {
        self.runs
            .iter()
            .filter_map(|(run_id, entry)| {
                matches!(
                    entry.run.state(),
                    crate::managed_agents::ManagedRunState::Running
                )
                .then_some(*run_id)
            })
            .collect()
    }

    pub fn transport_binding(&self, run_id: ManagedRunId) -> Option<&ManagedAgentTransportBinding> {
        self.runs.get(&run_id).map(|entry| &entry.transport)
    }

    pub fn is_omegon_control_run(&self, run_id: ManagedRunId) -> bool {
        matches!(
            self.transport_binding(run_id),
            Some(ManagedAgentTransportBinding::OmegonControl { .. })
        )
    }

    pub fn active_run_ids(&self) -> Vec<ManagedRunId> {
        self.runs
            .iter()
            .filter_map(|(run_id, entry)| (!entry.run.state().is_terminal()).then_some(*run_id))
            .collect()
    }

    pub fn mark_worker_reconnected(&mut self, worker_id: WorkerId) {
        for entry in self
            .runs
            .values_mut()
            .filter(|entry| entry.run.worker_id() == worker_id && !entry.run.state().is_terminal())
        {
            let run_id = entry.run.run_id();
            if entry.run.mark_reconnected().is_ok() {
                entry
                    .events
                    .push(SupervisorEvent::WorkerReconnected { run_id });
            }
        }
    }

    fn expire_dispatches_at(&mut self, now: Instant, timeout: Duration) -> Vec<ManagedRunId> {
        let mut expired = Vec::new();
        for entry in self.runs.values_mut() {
            if matches!(
                entry.run.state(),
                crate::managed_agents::ManagedRunState::Dispatching
            ) && now.duration_since(entry.dispatched_at) >= timeout
                && entry.run.mark_dispatch_timed_out().is_ok()
            {
                expired.push(entry.run.run_id());
            }
        }
        expired
    }

    pub fn expire_dispatches_for(
        &mut self,
        run_ids: &std::collections::HashSet<ManagedRunId>,
        timeout: Duration,
    ) -> Vec<ManagedRunId> {
        let now = Instant::now();
        let mut expired = Vec::new();
        for run_id in run_ids {
            let Some(entry) = self.runs.get_mut(run_id) else {
                continue;
            };
            if matches!(
                entry.run.state(),
                crate::managed_agents::ManagedRunState::Dispatching
            ) && now.duration_since(entry.dispatched_at) >= timeout
                && entry.run.mark_dispatch_timed_out().is_ok()
            {
                expired.push(*run_id);
            }
        }
        expired
    }

    pub fn expire_dispatches(&mut self, timeout: Duration) -> Vec<ManagedRunId> {
        self.expire_dispatches_at(Instant::now(), timeout)
    }

    fn expired_runs_at(&self, now: Instant) -> Vec<ManagedRunId> {
        self.runs
            .iter()
            .filter_map(|(run_id, entry)| {
                (!entry.run.state().is_terminal() && now >= entry.deadline_at).then_some(*run_id)
            })
            .collect()
    }

    pub fn expired_runs(&self) -> Vec<ManagedRunId> {
        self.expired_runs_at(Instant::now())
    }

    pub fn run(&self, run_id: ManagedRunId) -> Option<&ManagedAgentRun> {
        self.runs.get(&run_id).map(|entry| &entry.run)
    }
    pub fn events(&self, run_id: ManagedRunId) -> Option<&[SupervisorEvent]> {
        self.runs.get(&run_id).map(|entry| entry.events.as_slice())
    }
    pub fn dispatched_at_unix_ms(&self, run_id: ManagedRunId) -> Option<u64> {
        self.runs
            .get(&run_id)
            .map(|entry| entry.dispatched_at_unix_ms)
    }
    pub fn max_result_bytes(&self) -> usize {
        self.max_result_bytes
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::managed_agent_supervisor::{
        METHOD_DELEGATE_DISPATCH, METHOD_DELEGATE_GET, ManagedAgentCommandAck,
        ManagedAgentCommandAckStatus,
    };
    use crate::managed_agents::ManagedRunState;
    use crate::managed_agents::{OmegonTaskId, WorkerProfile};
    use std::collections::BTreeSet;

    fn request() -> ManagedRunRequest {
        ManagedRunRequest::new(
            "inspect",
            WorkerProfile::Scout,
            BTreeSet::from(["src".into()]),
            30,
        )
        .unwrap()
    }
    fn target() -> CommandTarget {
        CommandTarget {
            session_key: "remote:s".into(),
            dispatcher_instance_id: Some("w".into()),
        }
    }

    #[test]
    fn dispatch_correlates_response_and_builds_poll_command() {
        let worker = WorkerId::new();
        let mut runtime = ManagedAgentSupervisorRuntime::new(1024);
        let (run_id, command) = runtime
            .dispatch(worker, "session", "turn", request(), target(), 1000)
            .unwrap();
        assert!(
            command
                .web_command_json()
                .contains(METHOD_DELEGATE_DISPATCH)
        );
        let response = serde_json::json!({
            "type": "delegate_dispatch_result", "schema_version": 1,
            "managed_run_id": run_id, "worker_id": worker, "accepted": true,
            "task_id": "delegate_1", "effective_policy": null, "rejection": null
        });
        runtime.apply_response_json(&response.to_string()).unwrap();
        assert_eq!(
            runtime.run(run_id).unwrap().omegon_task_id(),
            Some(&OmegonTaskId::parse("delegate_1").unwrap())
        );
        assert!(
            runtime
                .poll_command(run_id)
                .unwrap()
                .web_command_json()
                .contains(METHOD_DELEGATE_GET)
        );
    }

    #[test]
    fn rejects_cross_worker_response_and_detects_deadline() {
        let worker = WorkerId::new();
        let mut runtime = ManagedAgentSupervisorRuntime::new(1024);
        let (run_id, _) = runtime
            .dispatch(worker, "session", "turn", request(), target(), 1000)
            .unwrap();
        let response = serde_json::json!({
            "type": "delegate_dispatch_result", "schema_version": 1,
            "managed_run_id": run_id, "worker_id": WorkerId::new(), "accepted": true,
            "task_id": "delegate_1", "effective_policy": null, "rejection": null
        });
        assert!(matches!(
            runtime.apply_response_json(&response.to_string()),
            Err(SupervisorRuntimeError::Contract(
                SupervisorContractError::IdentityMismatch
            ))
        ));
        let start = runtime.runs.get(&run_id).unwrap().dispatched_at;
        assert!(
            runtime
                .expired_runs_at(start + Duration::from_millis(29_999))
                .is_empty()
        );
        assert_eq!(
            runtime.expired_runs_at(start + Duration::from_secs(30)),
            vec![run_id]
        );
    }

    #[test]
    fn result_and_cancellation_responses_drive_terminal_state() {
        let worker = WorkerId::new();
        let mut runtime = ManagedAgentSupervisorRuntime::new(1024);
        let (run_id, _) = runtime
            .dispatch(worker, "session", "turn", request(), target(), 0)
            .unwrap();
        runtime
            .apply_response_json(
                &serde_json::json!({
                    "type": "delegate_dispatch_result", "schema_version": 1,
                    "managed_run_id": run_id, "worker_id": worker, "accepted": true,
                    "task_id": "delegate_1", "effective_policy": null, "rejection": null
                })
                .to_string(),
            )
            .unwrap();
        runtime
            .apply_response_json(
                &serde_json::json!({
                    "type": "delegate_result_result", "schema_version": 1,
                    "managed_run_id": run_id, "worker_id": worker,
                    "task_id": "delegate_1", "result": "done"
                })
                .to_string(),
            )
            .unwrap();
        assert!(matches!(
            runtime.run(run_id).unwrap().state(),
            ManagedRunState::Completed { .. }
        ));

        let (cancel_id, _) = runtime
            .dispatch(worker, "session", "turn", request(), target(), 0)
            .unwrap();
        runtime
            .apply_response_json(
                &serde_json::json!({
                    "type": "delegate_dispatch_result", "schema_version": 1,
                    "managed_run_id": cancel_id, "worker_id": worker, "accepted": true,
                    "task_id": "delegate_2", "effective_policy": null, "rejection": null
                })
                .to_string(),
            )
            .unwrap();
        runtime
            .cancel_command(cancel_id, Some("stop".into()))
            .unwrap();
        runtime
            .apply_response_json(
                &serde_json::json!({
                    "type": "delegate_cancel_result", "schema_version": 1,
                    "managed_run_id": cancel_id, "worker_id": worker,
                    "task_id": "delegate_2", "acknowledged": true,
                    "termination_confirmed": true, "reason": "stop"
                })
                .to_string(),
            )
            .unwrap();
        assert!(matches!(
            runtime.run(cancel_id).unwrap().state(),
            ManagedRunState::Cancelled {
                termination_confirmed: true,
                ..
            }
        ));
    }

    #[test]
    fn prepared_mqtt_run_has_no_control_commands() {
        let mut runtime = ManagedAgentSupervisorRuntime::new(1024);
        let run_id = runtime
            .prepare_run(
                WorkerId::new(),
                "session",
                "turn",
                request(),
                ManagedAgentTransportBinding::StyreneA2a {
                    target_agent_id: "worker".into(),
                },
                42,
            )
            .unwrap();
        assert!(runtime.run(run_id).is_some());
        assert!(runtime.commands.is_empty());
        assert!(runtime.command_runs.is_empty());
    }

    #[test]
    fn dispatch_acceptance_timeout_is_terminal_and_idempotent() {
        let mut runtime = ManagedAgentSupervisorRuntime::new(1024);
        let (run_id, _) = runtime
            .dispatch(
                WorkerId::new(),
                "session",
                "turn",
                request(),
                target(),
                1_000,
            )
            .unwrap();
        let start = runtime.runs.get(&run_id).unwrap().dispatched_at;
        assert!(
            runtime
                .expire_dispatches_at(
                    start + Duration::from_millis(14_999),
                    Duration::from_secs(15)
                )
                .is_empty()
        );
        assert_eq!(
            runtime.expire_dispatches_at(start + Duration::from_secs(15), Duration::from_secs(15)),
            vec![run_id]
        );
        assert!(
            runtime
                .expire_dispatches_at(start + Duration::from_secs(20), Duration::from_secs(15))
                .is_empty()
        );
        assert!(matches!(
            runtime.run(run_id).unwrap().state(),
            ManagedRunState::DispatchTimedOut
        ));
    }

    fn ack_runtime() -> ManagedAgentSupervisorRuntime {
        let worker = WorkerId::new();
        let mut runtime = ManagedAgentSupervisorRuntime::new(1024);
        runtime
            .dispatch(worker, "session", "turn", request(), target(), 1000)
            .unwrap();
        runtime
    }

    fn pending_command_id(runtime: &ManagedAgentSupervisorRuntime) -> ManagedCommandId {
        *runtime
            .commands
            .keys()
            .next()
            .expect("tracked command exists")
    }

    fn command_ack(
        runtime: &ManagedAgentSupervisorRuntime,
        status: ManagedAgentCommandAckStatus,
        rejection: Option<crate::managed_agent_supervisor::ManagedAgentCommandAckRejection>,
    ) -> ManagedAgentCommandAck {
        let pending = runtime
            .commands
            .values()
            .next()
            .expect("tracked command exists");
        ManagedAgentCommandAck {
            schema_version: 1,
            command_id: pending_command_id(runtime),
            managed_run_id: pending.run_id,
            worker_id: pending.worker_id,
            task_id: pending.task_id.clone(),
            method: pending.command.method().expect("control method").into(),
            status,
            rejection,
            accepted_at_unix_ms: 1,
        }
    }

    #[test]
    fn command_ack_acceptance_is_terminal_for_replay() {
        let mut runtime = ack_runtime();
        let ack = command_ack(&runtime, ManagedAgentCommandAckStatus::Accepted, None);

        let command_id = runtime
            .apply_command_ack_json(&serde_json::to_string(&ack).unwrap())
            .unwrap();

        assert_eq!(
            runtime.command_delivery_state(command_id),
            Some(&ManagedCommandDeliveryState::Accepted)
        );
        assert!(runtime.replay_due_commands(Duration::ZERO).is_empty());
    }

    #[test]
    fn retryable_rejection_replays_same_command_identity() {
        let mut runtime = ack_runtime();
        let command_id = pending_command_id(&runtime);
        let original_json = runtime.commands[&command_id].command.web_command_json();
        let ack = command_ack(
            &runtime,
            ManagedAgentCommandAckStatus::Rejected,
            Some(
                crate::managed_agent_supervisor::ManagedAgentCommandAckRejection {
                    code: "busy".into(),
                    safe_message: "worker is busy".into(),
                    retryable: true,
                },
            ),
        );

        runtime
            .apply_command_ack_json(&serde_json::to_string(&ack).unwrap())
            .unwrap();
        assert_eq!(
            runtime.command_delivery_state(command_id),
            Some(&ManagedCommandDeliveryState::RetryPending {
                code: "busy".into()
            })
        );

        let replay = runtime.replay_due_commands(Duration::ZERO);
        assert_eq!(replay.len(), 1);
        assert_eq!(replay[0].managed_command_id(), Some(command_id));
        assert_eq!(replay[0].web_command_json(), original_json);
        assert_eq!(runtime.commands[&command_id].attempts, 2);
    }

    #[test]
    fn dispatch_result_proves_delivery_and_stops_replay_without_ack() {
        let mut runtime = ack_runtime();
        let command_id = pending_command_id(&runtime);
        let pending = &runtime.commands[&command_id];
        let response = serde_json::json!({
            "type": "delegate_dispatch_result",
            "schema_version": 1,
            "managed_run_id": pending.run_id,
            "worker_id": pending.worker_id,
            "accepted": true,
            "task_id": "delegate_1",
            "effective_policy": null,
            "rejection": null
        });

        runtime.apply_response_json(&response.to_string()).unwrap();

        assert_eq!(
            runtime.command_delivery_state(command_id),
            Some(&ManagedCommandDeliveryState::ProvenByResult)
        );
        assert!(runtime.replay_due_commands(Duration::ZERO).is_empty());
    }

    #[test]
    fn command_ack_rejects_mismatched_method_without_mutation() {
        let mut runtime = ack_runtime();
        let command_id = pending_command_id(&runtime);
        let mut ack = command_ack(&runtime, ManagedAgentCommandAckStatus::Accepted, None);
        ack.method = "delegate.cancel".into();

        assert_eq!(
            runtime.apply_command_ack_json(&serde_json::to_string(&ack).unwrap()),
            Err(SupervisorRuntimeError::Contract(
                SupervisorContractError::IdentityMismatch
            ))
        );
        assert_eq!(
            runtime.command_delivery_state(command_id),
            Some(&ManagedCommandDeliveryState::Pending)
        );
    }

    #[test]
    fn run_projection_is_serializable_bounded_and_parent_isolated() {
        let mut runtime = ack_runtime();
        let run_id = *runtime.runs.keys().next().unwrap();
        let command_id = pending_command_id(&runtime);
        let projection = runtime.project_run_for_parent(run_id, "session").unwrap();

        assert_eq!(projection.schema_version, 1);
        assert_eq!(projection.run_id, run_id);
        assert_eq!(projection.commands.len(), 1);
        assert_eq!(projection.commands[0].command_id, command_id);
        assert_eq!(projection.commands[0].attempts, 1);
        assert_eq!(
            projection.commands[0].state,
            ManagedCommandDeliveryProjectionState::Pending
        );
        let json = serde_json::to_value(&projection).unwrap();
        assert_eq!(json["commands"][0]["state"]["kind"], "pending");
        assert!(json.get("events").is_none());
        assert!(json.get("latest_observation").is_none());

        assert_eq!(
            runtime.project_run_for_parent(run_id, "other-session"),
            Err(ManagedAgentProjectionError::ParentSessionMismatch)
        );
        assert!(runtime.project_runs_for_parent("other-session").is_empty());

        let ack = command_ack(&runtime, ManagedAgentCommandAckStatus::Accepted, None);
        runtime
            .apply_command_ack_json(&serde_json::to_string(&ack).unwrap())
            .unwrap();
        assert_eq!(
            runtime
                .project_run_for_parent(run_id, "session")
                .unwrap()
                .commands[0]
                .state,
            ManagedCommandDeliveryProjectionState::Accepted
        );
    }

    #[test]
    fn a2a_dispatcher_maps_transport_events_to_scheduler_results() {
        assert_eq!(
            ManagedAgentA2aDispatcher::apply_event(
                &mut ManagedAgentSupervisorRuntime::new(1024),
                ManagedAgentMqttOrchestratorEvent::Connected,
            ),
            Ok(())
        );

        let error = ManagedAgentA2aDispatcher::apply_event(
            &mut ManagedAgentSupervisorRuntime::new(1024),
            ManagedAgentMqttOrchestratorEvent::Disconnected {
                error: "broker unavailable".into(),
            },
        );
        assert_eq!(error, Err("MQTT A2A transport: broker unavailable".into()));
    }

    #[test]
    fn disconnect_preserves_last_observation_and_marks_event() {
        let worker = WorkerId::new();
        let mut runtime = ManagedAgentSupervisorRuntime::new(1024);
        let (run_id, _) = runtime
            .dispatch(worker, "session", "turn", request(), target(), 0)
            .unwrap();
        let response = serde_json::json!({
            "type": "delegate_dispatch_result", "schema_version": 1,
            "managed_run_id": run_id, "worker_id": worker, "accepted": true,
            "task_id": "delegate_1", "effective_policy": null, "rejection": null
        });
        runtime.apply_response_json(&response.to_string()).unwrap();
        runtime.mark_worker_disconnected(worker);
        assert!(matches!(
            runtime.run(run_id).unwrap().state(),
            ManagedRunState::Disconnected { .. }
        ));
        assert!(matches!(
            runtime.events(run_id).unwrap().last(),
            Some(SupervisorEvent::WorkerDisconnected { .. })
        ));
    }
}
