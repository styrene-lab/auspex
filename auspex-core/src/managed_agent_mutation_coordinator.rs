//! Core-owned coordinator for private managed-agent bridge mutations.

use std::collections::{BTreeMap, VecDeque};

use crate::controller::{AppController, ManagedAgentTransportAction};
use crate::managed_agent_bridge_server::{ManagedAgentDispatchCommand, ManagedAgentMutation};
use crate::managed_agents::{ManagedRunId, ManagedRunRequest, WorkerId};

pub struct ManagedAgentMutationCoordinator {
    receiver: tokio::sync::mpsc::Receiver<ManagedAgentDispatchCommand>,
    completed_operations: BTreeMap<(String, String), ManagedRunId>,
}

impl ManagedAgentMutationCoordinator {
    pub fn channel(
        capacity: usize,
    ) -> (tokio::sync::mpsc::Sender<ManagedAgentDispatchCommand>, Self) {
        let (sender, receiver) = tokio::sync::mpsc::channel(capacity);
        (
            sender,
            Self {
                receiver,
                completed_operations: BTreeMap::new(),
            },
        )
    }

    pub fn process_pending(
        &mut self,
        controller: &mut AppController,
        actions: &mut VecDeque<ManagedAgentTransportAction>,
        now_unix_ms: u64,
    ) -> usize {
        let mut processed = 0;
        while let Ok(command) = self.receiver.try_recv() {
            let result = self.apply(controller, actions, &command, now_unix_ms);
            let _ = command.respond_to.send(result);
            processed += 1;
        }
        processed
    }

    fn apply(
        &mut self,
        controller: &mut AppController,
        actions: &mut VecDeque<ManagedAgentTransportAction>,
        command: &ManagedAgentDispatchCommand,
        now_unix_ms: u64,
    ) -> Result<(ManagedRunId, bool), String> {
        let key = (
            command.parent_session_id.clone(),
            command.operation_id.clone(),
        );
        if let Some(run_id) = self.completed_operations.get(&key).copied() {
            return Ok((run_id, true));
        }
        let (run_id, action) = match &command.operation {
            ManagedAgentMutation::Dispatch {
                directive,
                worker_profile,
                scope,
                supervisor_deadline_seconds,
            } => {
                let request = ManagedRunRequest::new(
                    directive.clone(),
                    *worker_profile,
                    scope.clone(),
                    *supervisor_deadline_seconds,
                )
                .map_err(|error| format!("invalid managed dispatch: {error:?}"))?;
                controller.prepare_styrene_a2a_dispatch(
                    WorkerId::new(),
                    (
                        command.parent_session_id.clone(),
                        command.operation_id.clone(),
                    ),
                    request,
                    "auspex-managed-worker",
                    now_unix_ms,
                )?
            }
            ManagedAgentMutation::Cancel { run_id, reason } => {
                controller
                    .managed_agent_run_projection(*run_id, &command.parent_session_id)
                    .map_err(|error| format!("managed cancellation rejected: {error:?}"))?;
                (
                    *run_id,
                    controller.prepare_styrene_a2a_cancel(*run_id, reason.clone())?,
                )
            }
        };
        self.completed_operations.insert(key, run_id);
        actions.push_back(action);
        Ok((run_id, false))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::managed_agents::WorkerProfile;
    use std::collections::BTreeSet;

    #[tokio::test]
    async fn dispatch_is_idempotent_and_enqueues_one_action() {
        let (sender, mut coordinator) = ManagedAgentMutationCoordinator::channel(4);
        let mut controller = AppController::default();
        let mut actions = VecDeque::new();
        let send = |respond_to| ManagedAgentDispatchCommand {
            operation_id: "dispatch-1".into(),
            parent_session_id: "parent-1".into(),
            operation: ManagedAgentMutation::Dispatch {
                directive: "inspect runtime".into(),
                worker_profile: WorkerProfile::Scout,
                scope: BTreeSet::from(["src".into()]),
                supervisor_deadline_seconds: 60,
            },
            respond_to,
        };
        let (reply, first) = tokio::sync::oneshot::channel();
        sender.send(send(reply)).await.unwrap();
        assert_eq!(
            coordinator.process_pending(&mut controller, &mut actions, 1),
            1
        );
        let (run_id, duplicate) = first.await.unwrap().unwrap();
        assert!(!duplicate);
        assert_eq!(actions.len(), 1);

        let (reply, second) = tokio::sync::oneshot::channel();
        sender.send(send(reply)).await.unwrap();
        coordinator.process_pending(&mut controller, &mut actions, 2);
        assert_eq!(second.await.unwrap().unwrap(), (run_id, true));
        assert_eq!(actions.len(), 1);
    }

    #[tokio::test]
    async fn cancellation_requires_parent_scope_and_is_idempotent() {
        let (sender, mut coordinator) = ManagedAgentMutationCoordinator::channel(4);
        let mut controller = AppController::default();
        let mut actions = VecDeque::new();
        let (reply, run) = tokio::sync::oneshot::channel();
        sender
            .send(ManagedAgentDispatchCommand {
                operation_id: "dispatch-1".into(),
                parent_session_id: "parent-1".into(),
                operation: ManagedAgentMutation::Dispatch {
                    directive: "inspect runtime".into(),
                    worker_profile: WorkerProfile::Scout,
                    scope: BTreeSet::new(),
                    supervisor_deadline_seconds: 60,
                },
                respond_to: reply,
            })
            .await
            .unwrap();
        coordinator.process_pending(&mut controller, &mut actions, 1);
        let run_id = run.await.unwrap().unwrap().0;
        controller
            .managed_agent_runtime_mut()
            .apply_a2a_event(&crate::managed_agent_mqtt::ManagedAgentA2aEvent::Accepted {
                managed_run_id: run_id,
                task_id: "task-1".into(),
                message_id: [1; 16],
            })
            .unwrap();
        actions.clear();

        let (reply, cancellation) = tokio::sync::oneshot::channel();
        sender
            .send(ManagedAgentDispatchCommand {
                operation_id: "cancel-1".into(),
                parent_session_id: "parent-1".into(),
                operation: ManagedAgentMutation::Cancel {
                    run_id,
                    reason: Some("stop".into()),
                },
                respond_to: reply,
            })
            .await
            .unwrap();
        coordinator.process_pending(&mut controller, &mut actions, 2);
        assert_eq!(cancellation.await.unwrap().unwrap(), (run_id, false));
        assert_eq!(actions.len(), 1);
    }
}
