use std::collections::BTreeMap;

use serde_json::Value;

use crate::managed_agent_supervisor::{
    DelegateCancelRequest, DelegateCancelResponse, DelegateDispatchRequest,
    DelegateDispatchResponse, DelegateObservationResponse, DelegateTaskRequest,
    SupervisorContractError,
};
use crate::managed_agents::{
    ManagedAgentRun, ManagedAgentValidationError, ManagedRunId, ManagedRunRequest,
    ManagedRunTransitionError, SupervisorEvent, WorkerId,
};
use crate::runtime_types::{CommandTarget, TargetedCommand};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ManagedAgentSupervisorRuntime {
    runs: BTreeMap<ManagedRunId, RuntimeEntry>,
    max_result_bytes: usize,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct RuntimeEntry {
    run: ManagedAgentRun,
    target: CommandTarget,
    dispatched_at_unix_ms: u64,
    deadline_at_unix_ms: u64,
    events: Vec<SupervisorEvent>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SupervisorRuntimeError {
    UnknownRun,
    DuplicateRun,
    UnexpectedResponseType(String),
    ResponseRejected { code: String, safe_message: String },
    Contract(SupervisorContractError),
    Domain(ManagedRunTransitionError),
    Validation(ManagedAgentValidationError),
    InvalidJson(String),
    DeadlineExceeded,
}

impl From<SupervisorContractError> for SupervisorRuntimeError {
    fn from(value: SupervisorContractError) -> Self { Self::Contract(value) }
}
impl From<ManagedRunTransitionError> for SupervisorRuntimeError {
    fn from(value: ManagedRunTransitionError) -> Self { Self::Domain(value) }
}
impl From<ManagedAgentValidationError> for SupervisorRuntimeError {
    fn from(value: ManagedAgentValidationError) -> Self { Self::Validation(value) }
}

impl ManagedAgentSupervisorRuntime {
    pub fn new(max_result_bytes: usize) -> Self {
        Self { runs: BTreeMap::new(), max_result_bytes }
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
        let run = ManagedAgentRun::new(worker_id, parent_session_id, parent_turn_id, request)?;
        let run_id = run.run_id();
        if self.runs.contains_key(&run_id) { return Err(SupervisorRuntimeError::DuplicateRun); }
        let deadline_at_unix_ms = now_unix_ms.saturating_add(run.request().supervisor_deadline_seconds().saturating_mul(1000));
        let command = DelegateDispatchRequest::new(run_id, worker_id, run.request().clone()).command(target.clone())?;
        self.runs.insert(run_id, RuntimeEntry {
            run,
            target,
            dispatched_at_unix_ms: now_unix_ms,
            deadline_at_unix_ms,
            events: vec![SupervisorEvent::DispatchRequested { run_id, worker_id }],
        });
        Ok((run_id, command))
    }

    pub fn poll_command(&self, run_id: ManagedRunId) -> Result<TargetedCommand, SupervisorRuntimeError> {
        let entry = self.runs.get(&run_id).ok_or(SupervisorRuntimeError::UnknownRun)?;
        let task_id = entry.run.omegon_task_id().ok_or(ManagedRunTransitionError::DispatchNotAccepted)?;
        Ok(DelegateTaskRequest::new(run_id, entry.run.worker_id(), task_id.clone()).get_command(entry.target.clone())?)
    }

    pub fn cancel_command(
        &mut self,
        run_id: ManagedRunId,
        reason: Option<String>,
    ) -> Result<TargetedCommand, SupervisorRuntimeError> {
        let entry = self.runs.get_mut(&run_id).ok_or(SupervisorRuntimeError::UnknownRun)?;
        let task_id = entry.run.omegon_task_id().ok_or(ManagedRunTransitionError::DispatchNotAccepted)?.clone();
        entry.run.request_cancellation(reason.clone())?;
        entry.events.push(SupervisorEvent::CancellationRequested { run_id, reason: reason.clone() });
        Ok(DelegateCancelRequest::new(run_id, entry.run.worker_id(), task_id, reason)?.command(entry.target.clone())?)
    }

    pub fn apply_response_json(&mut self, raw: &str) -> Result<ManagedRunId, SupervisorRuntimeError> {
        let value: Value = serde_json::from_str(raw).map_err(|error| SupervisorRuntimeError::InvalidJson(error.to_string()))?;
        let response_type = value.get("type").and_then(Value::as_str).unwrap_or_default();
        let run_id: ManagedRunId = serde_json::from_value(value.get("managed_run_id").cloned().ok_or_else(|| SupervisorRuntimeError::InvalidJson("missing managed_run_id".into()))?)
            .map_err(|error| SupervisorRuntimeError::InvalidJson(error.to_string()))?;

        if let Some(rejection) = value.get("rejection").filter(|value| !value.is_null()) {
            let rejection: crate::managed_agent_supervisor::SupervisorRejection = serde_json::from_value(rejection.clone())
                .map_err(|error| SupervisorRuntimeError::InvalidJson(error.to_string()))?;
            if let Some(entry) = self.runs.get_mut(&run_id) {
                entry.events.push(SupervisorEvent::DispatchRejected { run_id, reason: rejection.safe_message.clone() });
            }
            return Err(SupervisorRuntimeError::ResponseRejected { code: rejection.code, safe_message: rejection.safe_message });
        }

        let entry = self.runs.get_mut(&run_id).ok_or(SupervisorRuntimeError::UnknownRun)?;
        match response_type {
            "delegate_dispatch_result" => {
                let response: DelegateDispatchResponse = serde_json::from_value(value).map_err(|error| SupervisorRuntimeError::InvalidJson(error.to_string()))?;
                let task_id = response.validate_for(&run_id, &entry.run.worker_id())?.clone();
                entry.run.accept_dispatch(task_id.clone())?;
                entry.events.push(SupervisorEvent::DispatchAccepted { run_id, task_id });
            }
            "delegate_get_result" => {
                let response: DelegateObservationResponse = serde_json::from_value(value).map_err(|error| SupervisorRuntimeError::InvalidJson(error.to_string()))?;
                let expected = entry.run.omegon_task_id().ok_or(ManagedRunTransitionError::DispatchNotAccepted)?.clone();
                response.validate_for(&run_id, &entry.run.worker_id(), &expected)?;
                entry.run.apply_observation(response.observation)?;
            }
            "delegate_cancel_result" => {
                let response: DelegateCancelResponse = serde_json::from_value(value).map_err(|error| SupervisorRuntimeError::InvalidJson(error.to_string()))?;
                let expected = entry.run.omegon_task_id().ok_or(ManagedRunTransitionError::DispatchNotAccepted)?.clone();
                response.validate_for(&run_id, &entry.run.worker_id(), &expected)?;
            }
            other => return Err(SupervisorRuntimeError::UnexpectedResponseType(other.to_string())),
        }
        Ok(run_id)
    }

    pub fn mark_worker_disconnected(&mut self, worker_id: WorkerId) {
        for entry in self.runs.values_mut().filter(|entry| entry.run.worker_id() == worker_id && !entry.run.state().is_terminal()) {
            let run_id = entry.run.run_id();
            if entry.run.mark_disconnected().is_ok() {
                entry.events.push(SupervisorEvent::WorkerDisconnected { run_id });
            }
        }
    }

    pub fn expired_runs(&self, now_unix_ms: u64) -> Vec<ManagedRunId> {
        self.runs.iter().filter_map(|(run_id, entry)| {
            (!entry.run.state().is_terminal() && now_unix_ms >= entry.deadline_at_unix_ms).then_some(*run_id)
        }).collect()
    }

    pub fn run(&self, run_id: ManagedRunId) -> Option<&ManagedAgentRun> { self.runs.get(&run_id).map(|entry| &entry.run) }
    pub fn events(&self, run_id: ManagedRunId) -> Option<&[SupervisorEvent]> { self.runs.get(&run_id).map(|entry| entry.events.as_slice()) }
    pub fn dispatched_at_unix_ms(&self, run_id: ManagedRunId) -> Option<u64> { self.runs.get(&run_id).map(|entry| entry.dispatched_at_unix_ms) }
    pub fn max_result_bytes(&self) -> usize { self.max_result_bytes }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::managed_agent_supervisor::{METHOD_DELEGATE_DISPATCH, METHOD_DELEGATE_GET};
    use crate::managed_agents::ManagedRunState;
    use crate::managed_agents::{WorkerProfile, OmegonTaskId};
    use std::collections::BTreeSet;

    fn request() -> ManagedRunRequest {
        ManagedRunRequest::new("inspect", WorkerProfile::Scout, BTreeSet::from(["src".into()]), 30).unwrap()
    }
    fn target() -> CommandTarget { CommandTarget { session_key: "remote:s".into(), dispatcher_instance_id: Some("w".into()) } }

    #[test]
    fn dispatch_correlates_response_and_builds_poll_command() {
        let worker = WorkerId::new();
        let mut runtime = ManagedAgentSupervisorRuntime::new(1024);
        let (run_id, command) = runtime.dispatch(worker, "session", "turn", request(), target(), 1000).unwrap();
        assert!(command.web_command_json().contains(METHOD_DELEGATE_DISPATCH));
        let response = serde_json::json!({
            "type": "delegate_dispatch_result", "schema_version": 1,
            "managed_run_id": run_id, "worker_id": worker, "accepted": true,
            "task_id": "delegate_1", "effective_policy": null, "rejection": null
        });
        runtime.apply_response_json(&response.to_string()).unwrap();
        assert_eq!(runtime.run(run_id).unwrap().omegon_task_id(), Some(&OmegonTaskId::parse("delegate_1").unwrap()));
        assert!(runtime.poll_command(run_id).unwrap().web_command_json().contains(METHOD_DELEGATE_GET));
    }

    #[test]
    fn rejects_cross_worker_response_and_detects_deadline() {
        let worker = WorkerId::new();
        let mut runtime = ManagedAgentSupervisorRuntime::new(1024);
        let (run_id, _) = runtime.dispatch(worker, "session", "turn", request(), target(), 1000).unwrap();
        let response = serde_json::json!({
            "type": "delegate_dispatch_result", "schema_version": 1,
            "managed_run_id": run_id, "worker_id": WorkerId::new(), "accepted": true,
            "task_id": "delegate_1", "effective_policy": null, "rejection": null
        });
        assert!(matches!(runtime.apply_response_json(&response.to_string()), Err(SupervisorRuntimeError::Contract(SupervisorContractError::IdentityMismatch))));
        assert!(runtime.expired_runs(30_999).is_empty());
        assert_eq!(runtime.expired_runs(31_000), vec![run_id]);
    }

    #[test]
    fn disconnect_preserves_last_observation_and_marks_event() {
        let worker = WorkerId::new();
        let mut runtime = ManagedAgentSupervisorRuntime::new(1024);
        let (run_id, _) = runtime.dispatch(worker, "session", "turn", request(), target(), 0).unwrap();
        let response = serde_json::json!({
            "type": "delegate_dispatch_result", "schema_version": 1,
            "managed_run_id": run_id, "worker_id": worker, "accepted": true,
            "task_id": "delegate_1", "effective_policy": null, "rejection": null
        });
        runtime.apply_response_json(&response.to_string()).unwrap();
        runtime.mark_worker_disconnected(worker);
        assert!(matches!(runtime.run(run_id).unwrap().state(), ManagedRunState::Disconnected { .. }));
        assert!(matches!(runtime.events(run_id).unwrap().last(), Some(SupervisorEvent::WorkerDisconnected { .. })));
    }
}
