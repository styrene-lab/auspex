use serde::{Deserialize, Serialize};

use crate::managed_agents::{
    ManagedRunId, ManagedRunRequest, OmegonDelegateObservation, OmegonEffectivePolicy,
    OmegonTaskId, WorkerId,
};
use crate::runtime_types::{CommandTarget, TargetedCommand};

pub const SUPERVISOR_SCHEMA_VERSION: u32 = 1;
pub const METHOD_DELEGATE_DISPATCH: &str = "delegate_dispatch";
pub const METHOD_DELEGATE_GET: &str = "delegate_get";
pub const METHOD_DELEGATE_RESULT: &str = "delegate_result";
pub const METHOD_DELEGATE_CANCEL: &str = "delegate_cancel";

pub const EVENT_MANAGED_AGENT_COMMAND_ACK: &str = "managed_agent_command_ack";

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ManagedAgentCommandAckStatus {
    Accepted,
    Rejected,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ManagedAgentCommandAck {
    pub schema_version: u32,
    pub command_id: crate::runtime_types::ManagedCommandId,
    pub method: String,
    pub managed_run_id: ManagedRunId,
    pub worker_id: WorkerId,
    pub task_id: Option<OmegonTaskId>,
    pub status: ManagedAgentCommandAckStatus,
    pub rejection: Option<ManagedAgentCommandAckRejection>,
    pub accepted_at_unix_ms: u64,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ManagedAgentCommandAckRejection {
    pub code: String,
    pub safe_message: String,
    pub retryable: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ControlCommandReceiptStatus {
    Accepted,
    Duplicate,
    Rejected,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ControlCommandReceipt {
    pub schema_version: u32,
    pub command_id: crate::runtime_types::ManagedCommandId,
    pub receipt_status: ControlCommandReceiptStatus,
    pub received_at_unix_ms: u64,
    pub rejection: Option<SupervisorRejection>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DelegateDispatchRequest {
    pub schema_version: u32,
    pub managed_run_id: ManagedRunId,
    pub worker_id: WorkerId,
    pub request: ManagedRunRequest,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DelegateTaskRequest {
    pub schema_version: u32,
    pub managed_run_id: ManagedRunId,
    pub worker_id: WorkerId,
    pub task_id: OmegonTaskId,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DelegateCancelRequest {
    pub schema_version: u32,
    pub managed_run_id: ManagedRunId,
    pub worker_id: WorkerId,
    pub task_id: OmegonTaskId,
    pub reason: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DelegateDispatchResponse {
    pub schema_version: u32,
    pub managed_run_id: ManagedRunId,
    pub worker_id: WorkerId,
    pub accepted: bool,
    pub task_id: Option<OmegonTaskId>,
    pub effective_policy: Option<OmegonEffectivePolicy>,
    pub rejection: Option<SupervisorRejection>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DelegateObservationResponse {
    pub schema_version: u32,
    pub managed_run_id: ManagedRunId,
    pub worker_id: WorkerId,
    pub observation: OmegonDelegateObservation,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DelegateResultResponse {
    pub schema_version: u32,
    pub managed_run_id: ManagedRunId,
    pub worker_id: WorkerId,
    pub task_id: OmegonTaskId,
    pub result: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DelegateCancelResponse {
    pub schema_version: u32,
    pub managed_run_id: ManagedRunId,
    pub worker_id: WorkerId,
    pub task_id: OmegonTaskId,
    pub acknowledged: bool,
    pub termination_confirmed: bool,
    pub reason: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SupervisorErrorResponse {
    pub schema_version: u32,
    pub managed_run_id: ManagedRunId,
    pub worker_id: WorkerId,
    pub rejection: SupervisorRejection,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SupervisorRejection {
    pub code: String,
    pub safe_message: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SupervisorContractError {
    UnsupportedSchema { received: u32 },
    IdentityMismatch,
    MissingTaskId,
    InvalidAcknowledgement,
    OversizedSafeMessage,
    ContradictoryDispatchResponse,
    OversizedReason,
    OversizedResult,
    Serialization(String),
}

impl ManagedAgentCommandAck {
    pub fn validate(&self) -> Result<(), SupervisorContractError> {
        if self.schema_version != SUPERVISOR_SCHEMA_VERSION {
            return Err(SupervisorContractError::UnsupportedSchema {
                received: self.schema_version,
            });
        }
        if !matches!(
            self.method.as_str(),
            METHOD_DELEGATE_DISPATCH
                | METHOD_DELEGATE_GET
                | METHOD_DELEGATE_RESULT
                | METHOD_DELEGATE_CANCEL
        ) {
            return Err(SupervisorContractError::InvalidAcknowledgement);
        }
        match (&self.status, &self.rejection) {
            (ManagedAgentCommandAckStatus::Accepted, None) => Ok(()),
            (ManagedAgentCommandAckStatus::Rejected, Some(rejection)) => {
                if rejection.safe_message.len() > 1024 {
                    return Err(SupervisorContractError::OversizedSafeMessage);
                }
                if rejection.code.is_empty()
                    || matches!(
                        rejection.code.as_str(),
                        "unsupported_schema"
                            | "invalid_envelope"
                            | "unauthorized"
                            | "unknown_method"
                            | "command_id_conflict"
                    ) && rejection.retryable
                    || matches!(
                        rejection.code.as_str(),
                        "overloaded" | "temporarily_unavailable"
                    ) && !rejection.retryable
                {
                    return Err(SupervisorContractError::InvalidAcknowledgement);
                }
                Ok(())
            }
            _ => Err(SupervisorContractError::InvalidAcknowledgement),
        }
    }
}

impl DelegateDispatchRequest {
    pub fn new(managed_run_id: ManagedRunId, worker_id: WorkerId, request: ManagedRunRequest) -> Self {
        Self { schema_version: SUPERVISOR_SCHEMA_VERSION, managed_run_id, worker_id, request }
    }

    pub fn command(&self, target: CommandTarget) -> Result<TargetedCommand, SupervisorContractError> {
        encode_command(target, METHOD_DELEGATE_DISPATCH, self)
    }
}

impl DelegateTaskRequest {
    pub fn new(managed_run_id: ManagedRunId, worker_id: WorkerId, task_id: OmegonTaskId) -> Self {
        Self { schema_version: SUPERVISOR_SCHEMA_VERSION, managed_run_id, worker_id, task_id }
    }

    pub fn get_command(&self, target: CommandTarget) -> Result<TargetedCommand, SupervisorContractError> {
        encode_command(target, METHOD_DELEGATE_GET, self)
    }

    pub fn result_command(&self, target: CommandTarget) -> Result<TargetedCommand, SupervisorContractError> {
        encode_command(target, METHOD_DELEGATE_RESULT, self)
    }
}

impl DelegateCancelRequest {
    pub fn new(
        managed_run_id: ManagedRunId,
        worker_id: WorkerId,
        task_id: OmegonTaskId,
        reason: Option<String>,
    ) -> Result<Self, SupervisorContractError> {
        if reason.as_ref().is_some_and(|value| value.len() > 1024) {
            return Err(SupervisorContractError::OversizedReason);
        }
        Ok(Self { schema_version: SUPERVISOR_SCHEMA_VERSION, managed_run_id, worker_id, task_id, reason })
    }

    pub fn command(&self, target: CommandTarget) -> Result<TargetedCommand, SupervisorContractError> {
        encode_command(target, METHOD_DELEGATE_CANCEL, self)
    }
}

impl DelegateDispatchResponse {
    pub fn validate_for(
        &self,
        expected_run: &ManagedRunId,
        expected_worker: &WorkerId,
    ) -> Result<&OmegonTaskId, SupervisorContractError> {
        validate_envelope(self.schema_version, &self.managed_run_id, &self.worker_id, expected_run, expected_worker)?;
        match (self.accepted, self.task_id.as_ref(), self.rejection.as_ref()) {
            (true, Some(task_id), None) => Ok(task_id),
            (true, None, _) => Err(SupervisorContractError::MissingTaskId),
            (false, None, Some(_)) => Err(SupervisorContractError::ContradictoryDispatchResponse),
            _ => Err(SupervisorContractError::ContradictoryDispatchResponse),
        }
    }
}

impl DelegateObservationResponse {
    pub fn validate_for(
        &self,
        expected_run: &ManagedRunId,
        expected_worker: &WorkerId,
        expected_task: &OmegonTaskId,
    ) -> Result<(), SupervisorContractError> {
        validate_envelope(self.schema_version, &self.managed_run_id, &self.worker_id, expected_run, expected_worker)?;
        if &self.observation.task_id != expected_task {
            return Err(SupervisorContractError::IdentityMismatch);
        }
        Ok(())
    }
}

impl DelegateResultResponse {
    pub fn validate_for(
        &self,
        expected_run: &ManagedRunId,
        expected_worker: &WorkerId,
        expected_task: &OmegonTaskId,
        max_result_bytes: usize,
    ) -> Result<(), SupervisorContractError> {
        validate_envelope(self.schema_version, &self.managed_run_id, &self.worker_id, expected_run, expected_worker)?;
        if &self.task_id != expected_task {
            return Err(SupervisorContractError::IdentityMismatch);
        }
        if self.result.len() > max_result_bytes {
            return Err(SupervisorContractError::OversizedResult);
        }
        Ok(())
    }
}

impl DelegateCancelResponse {
    pub fn validate_for(
        &self,
        expected_run: &ManagedRunId,
        expected_worker: &WorkerId,
        expected_task: &OmegonTaskId,
    ) -> Result<(), SupervisorContractError> {
        validate_envelope(self.schema_version, &self.managed_run_id, &self.worker_id, expected_run, expected_worker)?;
        if &self.task_id != expected_task || (self.termination_confirmed && !self.acknowledged) {
            return Err(SupervisorContractError::IdentityMismatch);
        }
        Ok(())
    }
}

fn validate_envelope(
    schema_version: u32,
    run: &ManagedRunId,
    worker: &WorkerId,
    expected_run: &ManagedRunId,
    expected_worker: &WorkerId,
) -> Result<(), SupervisorContractError> {
    if schema_version != SUPERVISOR_SCHEMA_VERSION {
        return Err(SupervisorContractError::UnsupportedSchema { received: schema_version });
    }
    if run != expected_run || worker != expected_worker {
        return Err(SupervisorContractError::IdentityMismatch);
    }
    Ok(())
}

fn encode_command<T: Serialize>(
    target: CommandTarget,
    method: &str,
    value: &T,
) -> Result<TargetedCommand, SupervisorContractError> {
    serde_json::to_value(value)
        .map(|payload| TargetedCommand::control_method(target, method, payload))
        .map_err(|error| SupervisorContractError::Serialization(error.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::managed_agents::{ManagedRunRequest, WorkerProfile};

    fn ids() -> (ManagedRunId, WorkerId, OmegonTaskId) {
        (ManagedRunId::new(), WorkerId::new(), OmegonTaskId::parse("delegate_7").unwrap())
    }

    fn target() -> CommandTarget {
        CommandTarget { session_key: "remote:session".into(), dispatcher_instance_id: Some("worker".into()) }
    }

    fn rejected_ack(code: &str, retryable: bool) -> ManagedAgentCommandAck {
        let (managed_run_id, worker_id, task_id) = ids();
        ManagedAgentCommandAck {
            schema_version: SUPERVISOR_SCHEMA_VERSION,
            command_id: crate::runtime_types::ManagedCommandId::new(),
            method: METHOD_DELEGATE_GET.into(),
            managed_run_id,
            worker_id,
            task_id: Some(task_id),
            status: ManagedAgentCommandAckStatus::Rejected,
            rejection: Some(ManagedAgentCommandAckRejection {
                code: code.into(),
                safe_message: "not admitted".into(),
                retryable,
            }),
            accepted_at_unix_ms: 1,
        }
    }

    #[test]
    fn command_ack_validation_enforces_method_and_rejection_semantics() {
        let mut ack = rejected_ack("overloaded", true);
        assert_eq!(ack.validate(), Ok(()));

        ack.method = "private_method".into();
        assert_eq!(
            ack.validate(),
            Err(SupervisorContractError::InvalidAcknowledgement)
        );

        let terminal_retry = rejected_ack("unsupported_schema", true);
        assert_eq!(
            terminal_retry.validate(),
            Err(SupervisorContractError::InvalidAcknowledgement)
        );

        let retryable_without_retry = rejected_ack("temporarily_unavailable", false);
        assert_eq!(
            retryable_without_retry.validate(),
            Err(SupervisorContractError::InvalidAcknowledgement)
        );

        let empty_code = rejected_ack("", false);
        assert_eq!(
            empty_code.validate(),
            Err(SupervisorContractError::InvalidAcknowledgement)
        );
    }

    #[test]
    fn dispatch_encodes_authenticated_control_method_payload() {
        let (run, worker, _) = ids();
        let request = ManagedRunRequest::new("inspect", WorkerProfile::Scout, std::collections::BTreeSet::from(["src".into()]), 300).unwrap();
        let command = DelegateDispatchRequest::new(run, worker, request).command(target()).unwrap();
        let json: serde_json::Value = serde_json::from_str(&command.web_command_json()).unwrap();
        assert_eq!(json["type"], METHOD_DELEGATE_DISPATCH);
        assert_eq!(json["managed_run_id"], serde_json::to_value(run).unwrap());
        assert_eq!(json["worker_id"], serde_json::to_value(worker).unwrap());
        assert_eq!(json["schema_version"], SUPERVISOR_SCHEMA_VERSION);
    }

    #[test]
    fn dispatch_response_requires_correlated_task_identity() {
        let (run, worker, task) = ids();
        let accepted = DelegateDispatchResponse {
            schema_version: 1, managed_run_id: run, worker_id: worker, accepted: true,
            task_id: Some(task.clone()), effective_policy: None, rejection: None,
        };
        assert_eq!(accepted.validate_for(&run, &worker).unwrap(), &task);
        let malformed = DelegateDispatchResponse { task_id: None, ..accepted };
        assert_eq!(malformed.validate_for(&run, &worker), Err(SupervisorContractError::MissingTaskId));
    }

    #[test]
    fn response_validation_rejects_cross_worker_and_oversized_result() {
        let (run, worker, task) = ids();
        let response = DelegateResultResponse {
            schema_version: 1, managed_run_id: run, worker_id: worker, task_id: task.clone(), result: "12345".into(),
        };
        assert_eq!(response.validate_for(&run, &WorkerId::new(), &task, 10), Err(SupervisorContractError::IdentityMismatch));
        assert_eq!(response.validate_for(&run, &worker, &task, 4), Err(SupervisorContractError::OversizedResult));
    }

    #[test]
    fn cancellation_confirmation_requires_acknowledgement() {
        let (run, worker, task) = ids();
        let response = DelegateCancelResponse {
            schema_version: 1, managed_run_id: run, worker_id: worker, task_id: task.clone(),
            acknowledged: false, termination_confirmed: true, reason: None,
        };
        assert_eq!(response.validate_for(&run, &worker, &task), Err(SupervisorContractError::IdentityMismatch));
    }

    #[test]
    fn unsupported_schema_is_rejected() {
        let (run, worker, task) = ids();
        let response = DelegateResultResponse {
            schema_version: 99, managed_run_id: run, worker_id: worker, task_id: task.clone(), result: String::new(),
        };
        assert_eq!(response.validate_for(&run, &worker, &task, 10), Err(SupervisorContractError::UnsupportedSchema { received: 99 }));
    }

    #[test]
    fn deserializes_exact_omegon_golden_fixtures() {
        for fixture in [
            include_str!("../tests/fixtures/managed_agent/01-running.json"),
            include_str!("../tests/fixtures/managed_agent/02-completed-success.json"),
            include_str!("../tests/fixtures/managed_agent/03-completed-unsuccessful.json"),
            include_str!("../tests/fixtures/managed_agent/04-typed-failure.json"),
            include_str!("../tests/fixtures/managed_agent/05-cancel-acknowledged.json"),
            include_str!("../tests/fixtures/managed_agent/06-cancel-confirmed.json"),
        ] {
            let response: DelegateObservationResponse = serde_json::from_str(fixture).unwrap();
            response
                .validate_for(
                    &response.managed_run_id.clone(),
                    &response.worker_id.clone(),
                    &response.observation.task_id.clone(),
                )
                .unwrap();
        }

        let dispatch: DelegateDispatchResponse = serde_json::from_str(include_str!(
            "../tests/fixtures/managed_agent/07-dispatch-accepted.json"
        ))
        .unwrap();
        dispatch
            .validate_for(&dispatch.managed_run_id.clone(), &dispatch.worker_id.clone())
            .unwrap();

        let result: DelegateResultResponse = serde_json::from_str(include_str!(
            "../tests/fixtures/managed_agent/08-result.json"
        ))
        .unwrap();
        result
            .validate_for(
                &result.managed_run_id.clone(),
                &result.worker_id.clone(),
                &result.task_id.clone(),
                1024,
            )
            .unwrap();
    }

    #[test]
    fn deserializes_omegon_rejection_fixtures() {
        for fixture in [
            include_str!("../tests/fixtures/managed_agent/09-unknown-task.json"),
            include_str!("../tests/fixtures/managed_agent/10-unsupported-schema.json"),
        ] {
            let rejection: SupervisorErrorResponse = serde_json::from_str(fixture).unwrap();
            assert!(!rejection.rejection.code.is_empty());
            assert!(!rejection.rejection.safe_message.is_empty());
        }
    }
}
