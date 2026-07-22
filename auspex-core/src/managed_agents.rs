use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

pub type RunId = String;
pub type WorkerId = String;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ManagedAgentWorker {
    pub worker_id: WorkerId,
    pub role: WorkerRole,
    pub profile_id: String,
    pub model: String,
    pub capabilities: BTreeSet<String>,
    pub lifecycle: WorkerLifecycle,
    pub current_run_id: Option<RunId>,
    pub last_verified_at: Option<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum WorkerRole {
    PrimaryDriver,
    SupervisedChild,
    DetachedService,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WorkerLifecycle {
    Attaching,
    Ready,
    Busy,
    Degraded,
    Detached,
    Stopped,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ManagedAgentBudget {
    pub max_turns: u32,
    pub token_limit: u64,
    pub wall_clock_seconds: u64,
    pub allowed_tools: BTreeSet<String>,
    pub allowed_paths: BTreeSet<String>,
    pub network_policy: NetworkPolicy,
    pub max_result_bytes: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NetworkPolicy {
    Denied,
    Allowlisted,
    Unrestricted,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ManagedAgentRun {
    pub run_id: RunId,
    pub worker_id: WorkerId,
    pub parent_session_id: String,
    pub parent_turn_id: String,
    pub directive: String,
    pub scope: BTreeSet<String>,
    pub budget: ManagedAgentBudget,
    pub status: RunStatus,
    pub last_sequence: u64,
    pub result: Option<ManagedAgentResult>,
    pub failure: Option<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RunStatus {
    Queued,
    Running,
    WaitingForGuidance,
    Completed,
    Failed,
    Cancelled,
}

impl RunStatus {
    pub fn is_terminal(self) -> bool {
        matches!(self, Self::Completed | Self::Failed | Self::Cancelled)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ManagedAgentEvent {
    pub run_id: RunId,
    pub worker_id: WorkerId,
    pub sequence: u64,
    pub timestamp: String,
    pub kind: ManagedAgentEventKind,
    pub payload: String,
    pub artifact_refs: Vec<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ManagedAgentEventKind {
    RunAccepted,
    StatusChanged,
    ProgressReported,
    ToolStarted,
    ToolCompleted,
    QuestionRaised,
    ArtifactProduced,
    ResultReady,
    RunFailed,
    RunCancelled,
    GuidanceSent,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ManagedAgentResult {
    pub summary: String,
    pub changed_files: Vec<String>,
    pub validation: Vec<String>,
    pub commits: Vec<String>,
    pub artifacts: Vec<String>,
    pub questions: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RunTransitionError {
    TerminalRun,
    InvalidTransition { from: RunStatus, to: RunStatus },
    NonMonotonicSequence { last: u64, received: u64 },
    IdentityMismatch,
    ResultRequired,
}

impl ManagedAgentRun {
    pub fn apply_event(&mut self, event: &ManagedAgentEvent) -> Result<(), RunTransitionError> {
        if event.run_id != self.run_id || event.worker_id != self.worker_id {
            return Err(RunTransitionError::IdentityMismatch);
        }
        if self.status.is_terminal() {
            return Err(RunTransitionError::TerminalRun);
        }
        if event.sequence <= self.last_sequence {
            return Err(RunTransitionError::NonMonotonicSequence {
                last: self.last_sequence,
                received: event.sequence,
            });
        }

        let next = match event.kind {
            ManagedAgentEventKind::RunAccepted => RunStatus::Running,
            ManagedAgentEventKind::QuestionRaised => RunStatus::WaitingForGuidance,
            ManagedAgentEventKind::GuidanceSent => RunStatus::Running,
            ManagedAgentEventKind::ResultReady => RunStatus::Completed,
            ManagedAgentEventKind::RunFailed => RunStatus::Failed,
            ManagedAgentEventKind::RunCancelled => RunStatus::Cancelled,
            _ => self.status,
        };
        if !valid_transition(self.status, next) {
            return Err(RunTransitionError::InvalidTransition { from: self.status, to: next });
        }
        self.status = next;
        self.last_sequence = event.sequence;
        if next == RunStatus::Failed {
            self.failure = Some(event.payload.clone());
        }
        Ok(())
    }

    pub fn complete(&mut self, result: ManagedAgentResult) -> Result<(), RunTransitionError> {
        if self.status != RunStatus::Completed {
            return Err(RunTransitionError::InvalidTransition {
                from: self.status,
                to: RunStatus::Completed,
            });
        }
        self.result = Some(result);
        Ok(())
    }
}

fn valid_transition(from: RunStatus, to: RunStatus) -> bool {
    from == to
        || matches!(
            (from, to),
            (RunStatus::Queued, RunStatus::Running)
                | (RunStatus::Queued, RunStatus::Cancelled)
                | (RunStatus::Running, RunStatus::WaitingForGuidance)
                | (RunStatus::WaitingForGuidance, RunStatus::Running)
                | (RunStatus::Running, RunStatus::Completed)
                | (RunStatus::Running, RunStatus::Failed)
                | (RunStatus::Running, RunStatus::Cancelled)
                | (RunStatus::WaitingForGuidance, RunStatus::Failed)
                | (RunStatus::WaitingForGuidance, RunStatus::Cancelled)
        )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run() -> ManagedAgentRun {
        ManagedAgentRun {
            run_id: "run-1".into(),
            worker_id: "worker-1".into(),
            parent_session_id: "session-1".into(),
            parent_turn_id: "turn-1".into(),
            directive: "inspect the failure".into(),
            scope: BTreeSet::from(["src".into()]),
            budget: ManagedAgentBudget {
                max_turns: 20,
                token_limit: 40_000,
                wall_clock_seconds: 600,
                allowed_tools: BTreeSet::from(["read".into()]),
                allowed_paths: BTreeSet::from(["src".into()]),
                network_policy: NetworkPolicy::Denied,
                max_result_bytes: 64 * 1024,
            },
            status: RunStatus::Queued,
            last_sequence: 0,
            result: None,
            failure: None,
        }
    }

    fn event(sequence: u64, kind: ManagedAgentEventKind) -> ManagedAgentEvent {
        ManagedAgentEvent {
            run_id: "run-1".into(), worker_id: "worker-1".into(), sequence,
            timestamp: "2026-07-22T00:00:00Z".into(), kind, payload: String::new(),
            artifact_refs: Vec::new(),
        }
    }

    #[test]
    fn applies_bounded_run_lifecycle() {
        let mut run = run();
        run.apply_event(&event(1, ManagedAgentEventKind::RunAccepted)).unwrap();
        run.apply_event(&event(2, ManagedAgentEventKind::QuestionRaised)).unwrap();
        run.apply_event(&event(3, ManagedAgentEventKind::GuidanceSent)).unwrap();
        run.apply_event(&event(4, ManagedAgentEventKind::ResultReady)).unwrap();
        assert_eq!(run.status, RunStatus::Completed);
        run.complete(ManagedAgentResult { summary: "done".into(), ..Default::default() }).unwrap();
        assert_eq!(run.result.unwrap().summary, "done");
    }

    #[test]
    fn rejects_replayed_or_cross_run_events() {
        let mut run = run();
        run.apply_event(&event(1, ManagedAgentEventKind::RunAccepted)).unwrap();
        assert!(matches!(run.apply_event(&event(1, ManagedAgentEventKind::ProgressReported)), Err(RunTransitionError::NonMonotonicSequence { .. })));
        let mut foreign = event(2, ManagedAgentEventKind::ProgressReported);
        foreign.worker_id = "worker-2".into();
        assert_eq!(run.apply_event(&foreign), Err(RunTransitionError::IdentityMismatch));
    }

    #[test]
    fn terminal_runs_are_immutable() {
        let mut run = run();
        run.apply_event(&event(1, ManagedAgentEventKind::RunAccepted)).unwrap();
        run.apply_event(&event(2, ManagedAgentEventKind::RunCancelled)).unwrap();
        assert_eq!(run.apply_event(&event(3, ManagedAgentEventKind::ProgressReported)), Err(RunTransitionError::TerminalRun));
    }
}
