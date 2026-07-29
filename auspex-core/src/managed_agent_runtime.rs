use std::collections::BTreeMap;
use std::time::{Duration, Instant};

use serde_json::Value;

use crate::managed_agent_supervisor::{
    ControlCommandReceipt, ControlCommandReceiptStatus, DelegateCancelRequest,
    DelegateCancelResponse, DelegateDispatchRequest, DelegateDispatchResponse,
    DelegateObservationResponse, DelegateResultResponse, DelegateTaskRequest,
    SupervisorContractError,
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
    state: ManagedCommandDeliveryState,
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
    Rejected { code: String },
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
        self.command_runs.insert(command_id, run_id);
        self.commands.insert(
            command_id,
            PendingManagedCommand {
                command: command.clone(),
                kind,
                state: ManagedCommandDeliveryState::Pending,
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

    pub fn replay_due_commands(&mut self, receipt_timeout: Duration) -> Vec<TargetedCommand> {
        let now = Instant::now();
        self.commands
            .values_mut()
            .filter_map(|pending| {
                if pending.state == ManagedCommandDeliveryState::Pending
                    && now.duration_since(pending.last_sent_at) >= receipt_timeout
                {
                    pending.last_sent_at = now;
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
            other => {
                return Err(SupervisorRuntimeError::UnexpectedResponseType(
                    other.to_string(),
                ));
            }
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
    use crate::managed_agent_supervisor::{METHOD_DELEGATE_DISPATCH, METHOD_DELEGATE_GET};
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
