//! Auspex-owned managed-agent supervision domain.
//!
//! Omegon owns delegate execution. Auspex owns fleet identity, dispatch policy,
//! attachment, outer deadlines, and the mapping from an Auspex run to the
//! worker-reported task handle. Worker snapshots are observations, never the
//! canonical run state and never a fabricated event log.

use serde::{Deserialize, Serialize};
use std::{collections::BTreeSet, fmt};
use uuid::Uuid;

macro_rules! uuid_id {
    ($name:ident) => {
        #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
        #[serde(transparent)]
        pub struct $name(Uuid);

        impl $name {
            pub fn new() -> Self { Self(Uuid::new_v4()) }
            pub fn from_uuid(value: Uuid) -> Self { Self(value) }
            pub fn as_uuid(self) -> Uuid { self.0 }
        }

        impl Default for $name {
            fn default() -> Self { Self::new() }
        }

        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result { self.0.fmt(f) }
        }
    };
}

uuid_id!(ManagedRunId);
uuid_id!(WorkerId);

#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct OmegonTaskId(String);

impl OmegonTaskId {
    pub fn parse(value: impl Into<String>) -> Result<Self, ManagedAgentValidationError> {
        let value = value.into();
        let trimmed = value.trim();
        if trimmed.is_empty() {
            Err(ManagedAgentValidationError::EmptyOmegonTaskId)
        } else {
            Ok(Self(trimmed.to_string()))
        }
    }

    pub fn as_str(&self) -> &str { &self.0 }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ManagedAgentWorker {
    pub worker_id: WorkerId,
    pub role: WorkerRole,
    pub profile_id: String,
    pub model: String,
    pub capabilities: BTreeSet<String>,
    pub state: WorkerState,
    pub last_verified_at: Option<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum WorkerRole {
    PrimaryDriver,
    SupervisedChild,
    DetachedService,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum WorkerState {
    Attaching,
    Ready,
    Busy { run_id: ManagedRunId },
    Degraded { reason: String },
    Detached,
    Stopped,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ManagedRunRequest {
    directive: String,
    pub worker_profile: WorkerProfile,
    pub scope: BTreeSet<String>,
    pub model: Option<String>,
    pub thinking_level: Option<String>,
    /// Auspex-owned outer deadline; Omegon independently enforces its effective
    /// profile turn and idle/wall limits.
    pub supervisor_deadline_seconds: u64,
}

impl ManagedRunRequest {
    pub fn new(
        directive: impl Into<String>,
        worker_profile: WorkerProfile,
        scope: BTreeSet<String>,
        supervisor_deadline_seconds: u64,
    ) -> Result<Self, ManagedAgentValidationError> {
        let directive = directive.into();
        let directive = directive.trim();
        if directive.is_empty() {
            return Err(ManagedAgentValidationError::EmptyDirective);
        }
        if supervisor_deadline_seconds == 0 {
            return Err(ManagedAgentValidationError::ZeroSupervisorDeadline);
        }
        Ok(Self {
            directive: directive.to_string(),
            worker_profile,
            scope,
            model: None,
            thinking_level: None,
            supervisor_deadline_seconds,
        })
    }

    pub fn directive(&self) -> &str { &self.directive }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WorkerProfile { Scout, Patch, Verify }

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ManagedAgentRun {
    run_id: ManagedRunId,
    worker_id: WorkerId,
    parent_session_id: String,
    parent_turn_id: String,
    request: ManagedRunRequest,
    omegon_task_id: Option<OmegonTaskId>,
    state: ManagedRunState,
    latest_observation: Option<OmegonDelegateObservation>,
}

impl ManagedAgentRun {
    pub fn new(
        worker_id: WorkerId,
        parent_session_id: impl Into<String>,
        parent_turn_id: impl Into<String>,
        request: ManagedRunRequest,
    ) -> Result<Self, ManagedAgentValidationError> {
        let parent_session_id = parent_session_id.into();
        let parent_turn_id = parent_turn_id.into();
        if parent_session_id.trim().is_empty() { return Err(ManagedAgentValidationError::EmptyParentSessionId); }
        if parent_turn_id.trim().is_empty() { return Err(ManagedAgentValidationError::EmptyParentTurnId); }
        Ok(Self {
            run_id: ManagedRunId::new(), worker_id,
            parent_session_id, parent_turn_id, request,
            omegon_task_id: None, state: ManagedRunState::Dispatching,
            latest_observation: None,
        })
    }

    pub fn run_id(&self) -> ManagedRunId { self.run_id }
    pub fn worker_id(&self) -> WorkerId { self.worker_id }
    pub fn request(&self) -> &ManagedRunRequest { &self.request }
    pub fn state(&self) -> &ManagedRunState { &self.state }
    pub fn omegon_task_id(&self) -> Option<&OmegonTaskId> { self.omegon_task_id.as_ref() }
    pub fn latest_observation(&self) -> Option<&OmegonDelegateObservation> { self.latest_observation.as_ref() }

    pub fn accept_dispatch(&mut self, task_id: OmegonTaskId) -> Result<(), ManagedRunTransitionError> {
        if !matches!(self.state, ManagedRunState::Dispatching) {
            return Err(ManagedRunTransitionError::InvalidState);
        }
        self.omegon_task_id = Some(task_id);
        self.state = ManagedRunState::Running;
        Ok(())
    }

    pub fn request_cancellation(&mut self, reason: Option<String>) -> Result<(), ManagedRunTransitionError> {
        if !matches!(self.state, ManagedRunState::Running | ManagedRunState::Disconnected { .. }) {
            return Err(ManagedRunTransitionError::InvalidState);
        }
        self.state = ManagedRunState::Cancelling { reason };
        Ok(())
    }

    pub fn apply_observation(&mut self, observation: OmegonDelegateObservation) -> Result<(), ManagedRunTransitionError> {
        let expected = self.omegon_task_id.as_ref().ok_or(ManagedRunTransitionError::DispatchNotAccepted)?;
        if expected != &observation.task_id { return Err(ManagedRunTransitionError::TaskIdentityMismatch); }
        self.state = match &observation.status {
            OmegonDelegateStatus::Running => ManagedRunState::Running,
            OmegonDelegateStatus::Completed { success: true } => {
                let result = observation.result.clone().ok_or(ManagedRunTransitionError::ResultRequired)?;
                ManagedRunState::Completed { result }
            }
            OmegonDelegateStatus::Completed { success: false } => ManagedRunState::Failed {
                failure: ManagedRunFailure::reported("delegate_completed_unsuccessfully", observation.result.clone()),
            },
            OmegonDelegateStatus::Failed { failure_kind, safe_message } => ManagedRunState::Failed {
                failure: ManagedRunFailure { code: failure_kind.code().to_string(), safe_message: safe_message.clone() },
            },
            OmegonDelegateStatus::Cancelled { reason, termination_confirmed } => ManagedRunState::Cancelled {
                reason: reason.clone(), termination_confirmed: *termination_confirmed,
            },
        };
        self.latest_observation = Some(observation);
        Ok(())
    }

    pub fn mark_disconnected(&mut self) -> Result<(), ManagedRunTransitionError> {
        if self.state.is_terminal() { return Err(ManagedRunTransitionError::TerminalRun); }
        self.state = ManagedRunState::Disconnected { last_observation: self.latest_observation.clone() };
        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ManagedRunState {
    Dispatching,
    Running,
    Cancelling { reason: Option<String> },
    Completed { result: ManagedRunResult },
    Failed { failure: ManagedRunFailure },
    Cancelled { reason: Option<String>, termination_confirmed: bool },
    Disconnected { last_observation: Option<OmegonDelegateObservation> },
}

impl ManagedRunState {
    pub fn is_terminal(&self) -> bool {
        matches!(self, Self::Completed { .. } | Self::Failed { .. } | Self::Cancelled { termination_confirmed: true, .. })
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct OmegonEffectivePolicy {
    pub worker_profile: WorkerProfile,
    pub max_turns: u32,
    pub wall_timeout_seconds: u64,
    pub idle_timeout_seconds: u64,
    pub enabled_tools: Vec<String>,
    pub model: Option<String>,
    pub thinking_level: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct OmegonDelegateObservation {
    pub task_id: OmegonTaskId,
    pub label: Option<String>,
    /// Human-readable configured agent/profile label, not fleet identity.
    pub agent_name: Option<String>,
    pub task_description: String,
    pub status: OmegonDelegateStatus,
    pub result: Option<ManagedRunResult>,
    pub result_viewed: bool,
    pub started_at_unix_ms: u64,
    pub completed_at_unix_ms: Option<u64>,
    pub last_tool: Option<ToolActivityObservation>,
    pub last_turn: Option<u32>,
    pub checklist: Vec<ChecklistItemObservation>,
    pub route: Option<RouteObservation>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum OmegonDelegateStatus {
    Running,
    Completed { success: bool },
    Failed { failure_kind: OmegonFailureKind, safe_message: String },
    Cancelled { reason: Option<String>, termination_confirmed: bool },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OmegonFailureKind { MissingLocalModel, MissingCredential, ProviderStartup, WorkspaceStartup, Unknown }

impl OmegonFailureKind {
    fn code(self) -> &'static str {
        match self {
            Self::MissingLocalModel => "missing_local_model",
            Self::MissingCredential => "missing_credential",
            Self::ProviderStartup => "provider_startup",
            Self::WorkspaceStartup => "workspace_startup",
            Self::Unknown => "unknown",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ToolActivityObservation { pub tool: String, pub args_summary: Option<String> }
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChecklistItemObservation { pub label: String, pub done: bool }
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RouteObservation { pub model: Option<String>, pub provider: Option<String>, pub fallback_used: bool }

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ManagedRunResult {
    pub summary: String,
    pub changed_files: Vec<String>,
    pub validation: Vec<String>,
    pub commits: Vec<String>,
    pub artifacts: Vec<String>,
    pub questions: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ManagedRunFailure { pub code: String, pub safe_message: String }
impl ManagedRunFailure {
    fn reported(code: &str, result: Option<ManagedRunResult>) -> Self {
        Self { code: code.into(), safe_message: result.map(|r| r.summary).unwrap_or_else(|| "Delegate reported unsuccessful completion".into()) }
    }
}

/// Events Auspex itself can prove. Worker activity remains a snapshot observation
/// until Omegon exposes a durable sequenced execution stream.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum SupervisorEvent {
    DispatchRequested { run_id: ManagedRunId, worker_id: WorkerId },
    DispatchAccepted { run_id: ManagedRunId, task_id: OmegonTaskId },
    DispatchRejected { run_id: ManagedRunId, reason: String },
    CancellationRequested { run_id: ManagedRunId, reason: Option<String> },
    WorkerDisconnected { run_id: ManagedRunId },
    WorkerReconnected { run_id: ManagedRunId },
    ResultFetched { run_id: ManagedRunId, task_id: OmegonTaskId },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ManagedAgentValidationError {
    EmptyOmegonTaskId, EmptyDirective, ZeroSupervisorDeadline, EmptyParentSessionId, EmptyParentTurnId,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ManagedRunTransitionError {
    InvalidState, TerminalRun, DispatchNotAccepted, TaskIdentityMismatch, ResultRequired,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn request() -> ManagedRunRequest {
        ManagedRunRequest::new("inspect delegate lifecycle", WorkerProfile::Scout, BTreeSet::from(["src".into()]), 600).unwrap()
    }
    fn run() -> ManagedAgentRun { ManagedAgentRun::new(WorkerId::new(), "session-1", "turn-1", request()).unwrap() }
    fn observation(task_id: &str, status: OmegonDelegateStatus, result: Option<ManagedRunResult>) -> OmegonDelegateObservation {
        OmegonDelegateObservation {
            task_id: OmegonTaskId::parse(task_id).unwrap(), label: Some("scout/backend".into()), agent_name: Some("scout".into()),
            task_description: "Inspect backend".into(), status, result, result_viewed: false,
            started_at_unix_ms: 1, completed_at_unix_ms: None,
            last_tool: Some(ToolActivityObservation { tool: "read".into(), args_summary: Some("managed_agents.rs".into()) }),
            last_turn: Some(2), checklist: vec![ChecklistItemObservation { label: "Inspect".into(), done: true }],
            route: Some(RouteObservation { model: Some("test:model".into()), provider: Some("test".into()), fallback_used: false }),
        }
    }

    #[test]
    fn validates_request_and_parent_identity() {
        assert_eq!(ManagedRunRequest::new("", WorkerProfile::Scout, BTreeSet::new(), 10), Err(ManagedAgentValidationError::EmptyDirective));
        assert_eq!(ManagedRunRequest::new("work", WorkerProfile::Scout, BTreeSet::new(), 0), Err(ManagedAgentValidationError::ZeroSupervisorDeadline));
        assert_eq!(ManagedAgentRun::new(WorkerId::new(), "", "turn", request()), Err(ManagedAgentValidationError::EmptyParentSessionId));
    }

    #[test]
    fn dispatch_maps_distinct_auspex_and_omegon_identities() {
        let mut run = run();
        let run_id = run.run_id();
        run.accept_dispatch(OmegonTaskId::parse("delegate_7").unwrap()).unwrap();
        assert_eq!(run.run_id(), run_id);
        assert_eq!(run.omegon_task_id().unwrap().as_str(), "delegate_7");
        assert!(matches!(run.state(), ManagedRunState::Running));
    }

    #[test]
    fn completion_is_atomic_and_requires_result() {
        let mut run = run();
        run.accept_dispatch(OmegonTaskId::parse("delegate_1").unwrap()).unwrap();
        let missing = observation("delegate_1", OmegonDelegateStatus::Completed { success: true }, None);
        assert_eq!(run.apply_observation(missing), Err(ManagedRunTransitionError::ResultRequired));
        let result = ManagedRunResult { summary: "done".into(), ..Default::default() };
        run.apply_observation(observation("delegate_1", OmegonDelegateStatus::Completed { success: true }, Some(result.clone()))).unwrap();
        assert_eq!(run.state(), &ManagedRunState::Completed { result });
    }

    #[test]
    fn rejects_cross_task_observation() {
        let mut run = run();
        run.accept_dispatch(OmegonTaskId::parse("delegate_1").unwrap()).unwrap();
        assert_eq!(run.apply_observation(observation("delegate_2", OmegonDelegateStatus::Running, None)), Err(ManagedRunTransitionError::TaskIdentityMismatch));
    }

    #[test]
    fn cancellation_ack_is_not_terminal_without_termination_confirmation() {
        let mut run = run();
        run.accept_dispatch(OmegonTaskId::parse("delegate_1").unwrap()).unwrap();
        run.request_cancellation(Some("operator request".into())).unwrap();
        run.apply_observation(observation("delegate_1", OmegonDelegateStatus::Cancelled { reason: None, termination_confirmed: false }, None)).unwrap();
        assert!(!run.state().is_terminal());
    }

    #[test]
    fn worker_busy_state_carries_run_identity() {
        let run_id = ManagedRunId::new();
        assert_eq!(WorkerState::Busy { run_id }, WorkerState::Busy { run_id });
    }

    #[test]
    fn ids_and_domain_state_round_trip_json() {
        let mut run = run();
        run.accept_dispatch(OmegonTaskId::parse("delegate_9").unwrap()).unwrap();
        let json = serde_json::to_string(&run).unwrap();
        let restored: ManagedAgentRun = serde_json::from_str(&json).unwrap();
        assert_eq!(restored, run);
    }
}
