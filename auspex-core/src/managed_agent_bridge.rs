//! Versioned private protocol between the bundled managed-agent extension and Auspex.

use serde::{Deserialize, Serialize};

use crate::managed_agent_runtime::ManagedAgentRunProjection;
use crate::managed_agents::{ManagedRunId, WorkerProfile};

pub const MANAGED_AGENT_BRIDGE_SCHEMA_VERSION: u32 = 1;
pub const MANAGED_AGENT_BRIDGE_PROTOCOL: &str = "auspex.managed-agents.v1";
pub const MAX_MANAGED_AGENT_BRIDGE_FRAME_BYTES: usize = 64 * 1024;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ManagedAgentBridgeRequest {
    pub schema_version: u32,
    pub protocol: String,
    pub request_id: String,
    pub capability: String,
    pub operation: ManagedAgentBridgeOperation,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum ManagedAgentBridgeOperation {
    AgentsStatus {
        run_id: Option<ManagedRunId>,
    },
    AgentsDispatch {
        operation_id: String,
        directive: String,
        worker_profile: WorkerProfile,
        scope: std::collections::BTreeSet<String>,
        supervisor_deadline_seconds: u64,
    },
    AgentsCancel {
        operation_id: String,
        run_id: ManagedRunId,
        reason: Option<String>,
    },
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ManagedAgentBridgeResponse {
    pub schema_version: u32,
    pub protocol: String,
    pub request_id: String,
    #[serde(flatten)]
    pub result: ManagedAgentBridgeResult,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "status", rename_all = "snake_case", deny_unknown_fields)]
pub enum ManagedAgentBridgeResult {
    Ok {
        runs: Vec<ManagedAgentRunProjection>,
    },
    Dispatched {
        run_id: ManagedRunId,
        duplicate: bool,
    },
    Error {
        code: ManagedAgentBridgeErrorCode,
        message: String,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ManagedAgentBridgeErrorCode {
    InvalidRequest,
    Unauthorized,
    UnsupportedProtocol,
    UnknownRun,
    ScopeDenied,
    Internal,
}

impl ManagedAgentBridgeResponse {
    pub fn ok(request_id: impl Into<String>, runs: Vec<ManagedAgentRunProjection>) -> Self {
        Self {
            schema_version: MANAGED_AGENT_BRIDGE_SCHEMA_VERSION,
            protocol: MANAGED_AGENT_BRIDGE_PROTOCOL.into(),
            request_id: request_id.into(),
            result: ManagedAgentBridgeResult::Ok { runs },
        }
    }

    pub fn dispatched(
        request_id: impl Into<String>,
        run_id: ManagedRunId,
        duplicate: bool,
    ) -> Self {
        Self {
            schema_version: MANAGED_AGENT_BRIDGE_SCHEMA_VERSION,
            protocol: MANAGED_AGENT_BRIDGE_PROTOCOL.into(),
            request_id: request_id.into(),
            result: ManagedAgentBridgeResult::Dispatched { run_id, duplicate },
        }
    }

    pub fn error(
        request_id: impl Into<String>,
        code: ManagedAgentBridgeErrorCode,
        message: impl Into<String>,
    ) -> Self {
        Self {
            schema_version: MANAGED_AGENT_BRIDGE_SCHEMA_VERSION,
            protocol: MANAGED_AGENT_BRIDGE_PROTOCOL.into(),
            request_id: request_id.into(),
            result: ManagedAgentBridgeResult::Error {
                code,
                message: message.into(),
            },
        }
    }
}

pub fn decode_bridge_request(
    frame: &[u8],
) -> Result<ManagedAgentBridgeRequest, ManagedAgentBridgeErrorCode> {
    if frame.len() > MAX_MANAGED_AGENT_BRIDGE_FRAME_BYTES {
        return Err(ManagedAgentBridgeErrorCode::InvalidRequest);
    }
    let request: ManagedAgentBridgeRequest =
        serde_json::from_slice(frame).map_err(|_| ManagedAgentBridgeErrorCode::InvalidRequest)?;
    if request.schema_version != MANAGED_AGENT_BRIDGE_SCHEMA_VERSION
        || request.protocol != MANAGED_AGENT_BRIDGE_PROTOCOL
    {
        return Err(ManagedAgentBridgeErrorCode::UnsupportedProtocol);
    }
    if request.request_id.is_empty() || request.capability.len() < 43 {
        return Err(ManagedAgentBridgeErrorCode::InvalidRequest);
    }
    if let ManagedAgentBridgeOperation::AgentsDispatch {
        operation_id,
        directive,
        scope,
        supervisor_deadline_seconds,
        ..
    } = &request.operation
        && (operation_id.trim().is_empty()
            || operation_id.len() > 128
            || directive.trim().is_empty()
            || directive.len() > 16 * 1024
            || scope.len() > 128
            || scope
                .iter()
                .any(|path| path.is_empty() || path.len() > 1024)
            || !(1..=86_400).contains(supervisor_deadline_seconds))
    {
        return Err(ManagedAgentBridgeErrorCode::InvalidRequest);
    }
    if let ManagedAgentBridgeOperation::AgentsCancel {
        operation_id,
        reason,
        ..
    } = &request.operation
        && (operation_id.trim().is_empty()
            || operation_id.len() > 128
            || reason.as_ref().is_some_and(|reason| reason.len() > 4096))
    {
        return Err(ManagedAgentBridgeErrorCode::InvalidRequest);
    }
    Ok(request)
}

pub fn capability_matches(expected: &str, supplied: &str) -> bool {
    if expected.len() != supplied.len() {
        return false;
    }
    expected
        .as_bytes()
        .iter()
        .zip(supplied.as_bytes())
        .fold(0_u8, |difference, (left, right)| {
            difference | (left ^ right)
        })
        == 0
}

#[cfg(test)]
mod tests {
    use super::*;

    fn request() -> ManagedAgentBridgeRequest {
        ManagedAgentBridgeRequest {
            schema_version: MANAGED_AGENT_BRIDGE_SCHEMA_VERSION,
            protocol: MANAGED_AGENT_BRIDGE_PROTOCOL.into(),
            request_id: "request-1".into(),
            capability: "A".repeat(43),
            operation: ManagedAgentBridgeOperation::AgentsStatus { run_id: None },
        }
    }

    #[test]
    fn status_request_round_trips_without_parent_identity() {
        let json = serde_json::to_value(request()).unwrap();
        assert!(json.get("parent_session_id").is_none());
        assert_eq!(
            decode_bridge_request(&serde_json::to_vec(&json).unwrap()).unwrap(),
            request()
        );
    }

    #[test]
    fn rejects_unknown_fields_and_unsupported_protocol() {
        let mut json = serde_json::to_value(request()).unwrap();
        json["parent_session_id"] = serde_json::json!("spoofed");
        assert_eq!(
            decode_bridge_request(&serde_json::to_vec(&json).unwrap()),
            Err(ManagedAgentBridgeErrorCode::InvalidRequest)
        );

        let mut wrong = request();
        wrong.protocol = "other".into();
        assert_eq!(
            decode_bridge_request(&serde_json::to_vec(&wrong).unwrap()),
            Err(ManagedAgentBridgeErrorCode::UnsupportedProtocol)
        );
    }

    #[test]
    fn capability_comparison_requires_exact_value() {
        assert!(capability_matches(&"a".repeat(43), &"a".repeat(43)));
        assert!(!capability_matches(&"a".repeat(43), &"b".repeat(43)));
        assert!(!capability_matches(&"a".repeat(43), &"a".repeat(42)));
    }
}
