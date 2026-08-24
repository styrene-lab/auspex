//! Primary-agent read boundary for supervised managed-agent projections.
//!
//! This feature does not own worker transport or runtime state. It delegates reads to a
//! projection source supplied by the controller boundary, preserving the direction:
//! primary tool -> Auspex projection -> supervised runtime.

use std::sync::Arc;

use async_trait::async_trait;
use omegon_traits::{ContentBlock, Feature, ToolDefinition, ToolResult};
use serde_json::Value;

use crate::managed_agent_runtime::{ManagedAgentRunProjection, ManagedAgentSupervisorRuntime};
use crate::managed_agents::ManagedRunId;

pub type SharedManagedAgentProjectionSnapshot =
    Arc<parking_lot::RwLock<ManagedAgentProjectionSnapshot>>;

pub trait ManagedAgentProjectionSource: Send + Sync {
    fn status(
        &self,
        parent_session_id: &str,
        run_id: Option<ManagedRunId>,
    ) -> Result<Vec<ManagedAgentRunProjection>, String>;
}

#[derive(Clone, Debug, Default)]
pub struct ManagedAgentProjectionSnapshot {
    runs_by_parent: std::collections::BTreeMap<String, Vec<ManagedAgentRunProjection>>,
}

impl ManagedAgentProjectionSnapshot {
    pub fn replace_parent(
        &mut self,
        parent_session_id: impl Into<String>,
        runs: Vec<ManagedAgentRunProjection>,
    ) {
        self.runs_by_parent.insert(parent_session_id.into(), runs);
    }

    pub fn clear(&mut self) {
        self.runs_by_parent.clear();
    }

    pub fn remove_parent(&mut self, parent_session_id: &str) {
        self.runs_by_parent.remove(parent_session_id);
    }
}

impl ManagedAgentProjectionSource for parking_lot::RwLock<ManagedAgentProjectionSnapshot> {
    fn status(
        &self,
        parent_session_id: &str,
        run_id: Option<ManagedRunId>,
    ) -> Result<Vec<ManagedAgentRunProjection>, String> {
        self.read().status(parent_session_id, run_id)
    }
}

impl ManagedAgentProjectionSource for ManagedAgentProjectionSnapshot {
    fn status(
        &self,
        parent_session_id: &str,
        run_id: Option<ManagedRunId>,
    ) -> Result<Vec<ManagedAgentRunProjection>, String> {
        let runs = self
            .runs_by_parent
            .get(parent_session_id)
            .cloned()
            .unwrap_or_default();
        match run_id {
            Some(run_id) => runs
                .into_iter()
                .find(|run| run.run_id == run_id)
                .map(|run| vec![run])
                .ok_or_else(|| "managed run is unknown or outside the parent session scope".into()),
            None => Ok(runs),
        }
    }
}

impl ManagedAgentProjectionSource for SharedManagedAgentProjectionSnapshot {
    fn status(
        &self,
        parent_session_id: &str,
        run_id: Option<ManagedRunId>,
    ) -> Result<Vec<ManagedAgentRunProjection>, String> {
        self.read().status(parent_session_id, run_id)
    }
}

impl ManagedAgentProjectionSource for ManagedAgentSupervisorRuntime {
    fn status(
        &self,
        parent_session_id: &str,
        run_id: Option<ManagedRunId>,
    ) -> Result<Vec<ManagedAgentRunProjection>, String> {
        match run_id {
            Some(run_id) => self
                .project_run_for_parent(run_id, parent_session_id)
                .map(|projection| vec![projection])
                .map_err(|error| format!("managed-agent projection rejected: {error:?}")),
            None => Ok(self.project_runs_for_parent(parent_session_id)),
        }
    }
}

#[derive(Clone)]
pub struct ManagedAgentFeature {
    source: Arc<dyn ManagedAgentProjectionSource>,
}

impl ManagedAgentFeature {
    pub fn new(source: Arc<dyn ManagedAgentProjectionSource>) -> Self {
        Self { source }
    }
}

#[async_trait]
impl Feature for ManagedAgentFeature {
    fn name(&self) -> &str {
        "managed_agents"
    }

    fn tools(&self) -> Vec<ToolDefinition> {
        vec![ToolDefinition {
            name: "agents_status".into(),
            label: "Managed Agent Status".into(),
            description: "Read canonical supervised managed-agent run status for the calling parent session. Returns bounded lifecycle and command-delivery evidence; it never returns raw worker transcripts or transport internals.".into(),
            capabilities: Vec::new(),
            parameters: serde_json::json!({
                "type": "object",
                "additionalProperties": false,
                "properties": {
                    "parent_session_id": {
                        "type": "string",
                        "minLength": 1,
                        "description": "Authoritative parent session identity used for isolation"
                    },
                    "run_id": {
                        "type": "string",
                        "format": "uuid",
                        "description": "Optional managed run identity; omit to list this parent's runs"
                    }
                },
                "required": ["parent_session_id"]
            }),
        }]
    }

    async fn execute(
        &self,
        tool_name: &str,
        _call_id: &str,
        args: Value,
        _cancel: tokio_util::sync::CancellationToken,
    ) -> anyhow::Result<ToolResult> {
        if tool_name != "agents_status" {
            anyhow::bail!("unsupported managed-agent tool: {tool_name}");
        }
        let parent_session_id = args
            .get("parent_session_id")
            .and_then(Value::as_str)
            .filter(|value| !value.is_empty())
            .ok_or_else(|| anyhow::anyhow!("parent_session_id is required"))?;
        let run_id = args
            .get("run_id")
            .map(|value| serde_json::from_value::<ManagedRunId>(value.clone()))
            .transpose()
            .map_err(|_| anyhow::anyhow!("run_id must be a UUID"))?;
        let projections = self
            .source
            .status(parent_session_id, run_id)
            .map_err(anyhow::Error::msg)?;
        let details = serde_json::to_value(&projections)?;
        Ok(ToolResult {
            content: vec![ContentBlock::Text {
                text: serde_json::to_string(&details)?,
            }],
            details,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::managed_agent_runtime::{
        ManagedCommandDeliveryProjection, ManagedCommandDeliveryProjectionState,
    };
    use crate::managed_agents::{ManagedRunState, WorkerId};
    use crate::runtime_types::ManagedCommandId;

    struct Source {
        projection: ManagedAgentRunProjection,
    }

    impl ManagedAgentProjectionSource for Source {
        fn status(
            &self,
            parent_session_id: &str,
            run_id: Option<ManagedRunId>,
        ) -> Result<Vec<ManagedAgentRunProjection>, String> {
            if parent_session_id != self.projection.parent_session_id {
                return Err("parent session mismatch".into());
            }
            if run_id.is_some_and(|id| id != self.projection.run_id) {
                return Err("unknown managed run".into());
            }
            Ok(vec![self.projection.clone()])
        }
    }

    fn feature() -> ManagedAgentFeature {
        ManagedAgentFeature::new(Arc::new(Source {
            projection: ManagedAgentRunProjection {
                schema_version: 1,
                run_id: ManagedRunId::new(),
                worker_id: WorkerId::new(),
                parent_session_id: "parent-1".into(),
                parent_turn_id: "turn-1".into(),
                state: ManagedRunState::Dispatching,
                task_id: None,
                commands: vec![ManagedCommandDeliveryProjection {
                    command_id: ManagedCommandId::new(),
                    method: "delegate_dispatch".into(),
                    command_kind:
                        crate::managed_agent_runtime::ManagedCommandProjectionKind::Dispatch,
                    attempts: 1,
                    state: ManagedCommandDeliveryProjectionState::Pending,
                }],
            },
        }))
    }

    #[test]
    fn projection_snapshot_is_parent_scoped_and_removable() {
        let feature = feature();
        let projection = feature.source.status("parent-1", None).unwrap().remove(0);
        let run_id = projection.run_id;
        let mut snapshot = ManagedAgentProjectionSnapshot::default();
        snapshot.replace_parent("parent-1", vec![projection]);

        let shared = std::sync::Arc::new(parking_lot::RwLock::new(snapshot));
        let source: std::sync::Arc<dyn ManagedAgentProjectionSource> = shared.clone();
        assert_eq!(source.status("parent-1", Some(run_id)).unwrap().len(), 1);

        shared.write().clear();
        assert!(source.status("parent-1", None).unwrap().is_empty());

        shared.write().replace_parent("parent-1", vec![]);
        shared.write().remove_parent("parent-1");
        assert!(shared.read().status("parent-1", None).unwrap().is_empty());
    }

    #[test]
    fn exposes_only_canonical_status_tool() {
        let tools = feature().tools();
        assert_eq!(tools.len(), 1);
        assert_eq!(tools[0].name, "agents_status");
        assert_eq!(tools[0].parameters["additionalProperties"], false);
    }

    #[tokio::test]
    async fn returns_projection_as_details_without_transcript_fields() {
        let result = feature()
            .execute(
                "agents_status",
                "call-1",
                serde_json::json!({"parent_session_id": "parent-1"}),
                tokio_util::sync::CancellationToken::new(),
            )
            .await
            .unwrap();
        assert_eq!(result.details.as_array().unwrap().len(), 1);
        assert!(result.details[0].get("commands").is_some());
        assert!(result.details[0].get("transcript").is_none());
        assert!(result.details[0].get("events").is_none());
    }

    #[tokio::test]
    async fn enforces_parent_session_isolation() {
        let error = feature()
            .execute(
                "agents_status",
                "call-1",
                serde_json::json!({"parent_session_id": "other"}),
                tokio_util::sync::CancellationToken::new(),
            )
            .await
            .unwrap_err();
        assert!(error.to_string().contains("parent session mismatch"));
    }

    #[test]
    fn runtime_is_a_projection_source_without_duplicate_schema() {
        let worker = WorkerId::new();
        let mut runtime = ManagedAgentSupervisorRuntime::new(1024);
        let request = crate::managed_agents::ManagedRunRequest::new(
            "inspect",
            crate::managed_agents::WorkerProfile::Scout,
            std::collections::BTreeSet::from(["src".into()]),
            30,
        )
        .unwrap();
        let run_id = runtime
            .prepare_run(
                worker,
                "parent-1",
                "turn-1",
                request,
                crate::managed_agent_runtime::ManagedAgentTransportBinding::OmegonControl {
                    instance_id: "worker-instance".into(),
                    target: crate::runtime_types::CommandTarget {
                        session_key: "remote:s".into(),
                        dispatcher_instance_id: Some("worker".into()),
                    },
                },
                1,
            )
            .unwrap();

        let projections =
            ManagedAgentProjectionSource::status(&runtime, "parent-1", Some(run_id)).unwrap();
        assert_eq!(projections.len(), 1);
        assert_eq!(projections[0].run_id, run_id);
        assert!(ManagedAgentProjectionSource::status(&runtime, "other", Some(run_id)).is_err());
    }
}
