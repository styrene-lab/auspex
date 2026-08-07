//! Launch-scoped Unix-domain listener for the bundled managed-agent extension.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::{UnixListener, UnixStream};
use tokio::task::JoinHandle;

use crate::managed_agent_bridge::{
    MAX_MANAGED_AGENT_BRIDGE_FRAME_BYTES, ManagedAgentBridgeErrorCode, ManagedAgentBridgeOperation,
    ManagedAgentBridgeResponse, capability_matches, decode_bridge_request,
};
use crate::managed_agent_feature::ManagedAgentProjectionSource;
use crate::managed_agents::{ManagedRunId, WorkerProfile};

#[derive(Debug)]
pub struct ManagedAgentDispatchCommand {
    pub operation_id: String,
    pub parent_session_id: String,
    pub directive: String,
    pub worker_profile: WorkerProfile,
    pub scope: std::collections::BTreeSet<String>,
    pub supervisor_deadline_seconds: u64,
    pub respond_to: tokio::sync::oneshot::Sender<Result<(ManagedRunId, bool), String>>,
}

pub type ManagedAgentDispatchSender = tokio::sync::mpsc::Sender<ManagedAgentDispatchCommand>;

pub struct ManagedAgentBridgeBinding {
    pub socket_path: PathBuf,
    pub capability: String,
    pub parent_session_id: String,
}

pub struct ManagedAgentBridgeServer {
    binding: ManagedAgentBridgeBinding,
    task: JoinHandle<()>,
}

impl ManagedAgentBridgeServer {
    pub async fn bind(
        socket_path: impl Into<PathBuf>,
        capability: String,
        parent_session_id: String,
        source: Arc<dyn ManagedAgentProjectionSource>,
        dispatch: Option<ManagedAgentDispatchSender>,
    ) -> Result<Self, String> {
        if capability.len() < 43 || parent_session_id.is_empty() {
            return Err("invalid launch binding".into());
        }
        let socket_path = socket_path.into();
        prepare_socket_path(&socket_path)?;
        let listener = UnixListener::bind(&socket_path)
            .map_err(|error| format!("bind managed-agent bridge: {error}"))?;
        set_owner_only_permissions(&socket_path)?;
        let binding = ManagedAgentBridgeBinding {
            socket_path: socket_path.clone(),
            capability: capability.clone(),
            parent_session_id: parent_session_id.clone(),
        };
        let task = tokio::spawn(async move {
            while let Ok((stream, _)) = listener.accept().await {
                let source = Arc::clone(&source);
                let capability = capability.clone();
                let parent_session_id = parent_session_id.clone();
                let dispatch = dispatch.clone();
                tokio::spawn(async move {
                    let _ =
                        serve_connection(stream, &capability, &parent_session_id, source, dispatch)
                            .await;
                });
            }
        });
        Ok(Self { binding, task })
    }

    pub fn binding(&self) -> &ManagedAgentBridgeBinding {
        &self.binding
    }
}

impl Drop for ManagedAgentBridgeServer {
    fn drop(&mut self) {
        self.task.abort();
        let _ = std::fs::remove_file(&self.binding.socket_path);
    }
}

async fn serve_connection(
    stream: UnixStream,
    capability: &str,
    parent_session_id: &str,
    source: Arc<dyn ManagedAgentProjectionSource>,
    dispatch: Option<ManagedAgentDispatchSender>,
) -> Result<(), String> {
    let (reader, mut writer) = stream.into_split();
    let mut lines = BufReader::new(reader).lines();
    while let Some(line) = lines.next_line().await.map_err(|error| error.to_string())? {
        let request = match decode_bridge_request(line.as_bytes()) {
            Ok(request) => request,
            Err(code) => {
                write_response(
                    &mut writer,
                    ManagedAgentBridgeResponse::error("", code, "request rejected"),
                )
                .await?;
                continue;
            }
        };
        if !capability_matches(capability, &request.capability) {
            write_response(
                &mut writer,
                ManagedAgentBridgeResponse::error(
                    request.request_id,
                    ManagedAgentBridgeErrorCode::Unauthorized,
                    "request rejected",
                ),
            )
            .await?;
            continue;
        }
        let request_id = request.request_id;
        let response = match request.operation {
            ManagedAgentBridgeOperation::AgentsStatus { run_id } => {
                match source.status(parent_session_id, run_id) {
                    Ok(runs) => ManagedAgentBridgeResponse::ok(request_id, runs),
                    Err(_) => ManagedAgentBridgeResponse::error(
                        request_id,
                        ManagedAgentBridgeErrorCode::ScopeDenied,
                        "managed run unavailable",
                    ),
                }
            }
            ManagedAgentBridgeOperation::AgentsDispatch {
                operation_id,
                directive,
                worker_profile,
                scope,
                supervisor_deadline_seconds,
            } => match &dispatch {
                Some(dispatch) => {
                    let (respond_to, response) = tokio::sync::oneshot::channel();
                    let command = ManagedAgentDispatchCommand {
                        operation_id,
                        parent_session_id: parent_session_id.to_string(),
                        directive,
                        worker_profile,
                        scope,
                        supervisor_deadline_seconds,
                        respond_to,
                    };
                    match dispatch.send(command).await {
                        Ok(()) => match response.await {
                            Ok(Ok((run_id, duplicate))) => ManagedAgentBridgeResponse::dispatched(
                                request_id, run_id, duplicate,
                            ),
                            _ => ManagedAgentBridgeResponse::error(
                                request_id,
                                ManagedAgentBridgeErrorCode::Internal,
                                "managed dispatch failed",
                            ),
                        },
                        Err(_) => ManagedAgentBridgeResponse::error(
                            request_id,
                            ManagedAgentBridgeErrorCode::Internal,
                            "managed dispatch unavailable",
                        ),
                    }
                }
                None => ManagedAgentBridgeResponse::error(
                    request_id,
                    ManagedAgentBridgeErrorCode::Internal,
                    "managed dispatch unavailable",
                ),
            },
        };
        write_response(&mut writer, response).await?;
    }
    Ok(())
}

async fn write_response(
    writer: &mut tokio::net::unix::OwnedWriteHalf,
    response: ManagedAgentBridgeResponse,
) -> Result<(), String> {
    let mut bytes = serde_json::to_vec(&response).map_err(|error| error.to_string())?;
    if bytes.len() > MAX_MANAGED_AGENT_BRIDGE_FRAME_BYTES {
        return Err("bridge response exceeds frame limit".into());
    }
    bytes.push(b'\n');
    writer
        .write_all(&bytes)
        .await
        .map_err(|error| error.to_string())
}

fn prepare_socket_path(path: &Path) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    }
    match std::fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error.to_string()),
    }
}

#[cfg(unix)]
fn set_owner_only_permissions(path: &Path) -> Result<(), String> {
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600))
        .map_err(|error| error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::managed_agent_runtime::ManagedAgentRunProjection;
    use crate::managed_agents::ManagedRunId;
    use tokio::io::{AsyncBufReadExt, AsyncWriteExt};

    struct EmptySource;
    impl ManagedAgentProjectionSource for EmptySource {
        fn status(
            &self,
            parent: &str,
            _run: Option<ManagedRunId>,
        ) -> Result<Vec<ManagedAgentRunProjection>, String> {
            if parent == "parent-1" {
                Ok(Vec::new())
            } else {
                Err("scope".into())
            }
        }
    }

    #[tokio::test]
    async fn dispatch_is_forwarded_with_server_owned_parent_scope() {
        let path = std::path::PathBuf::from(format!(
            "/tmp/auspex-bridge-dispatch-{}.sock",
            uuid::Uuid::new_v4().simple()
        ));
        let capability = "B".repeat(43);
        let (dispatch, mut commands) = tokio::sync::mpsc::channel(1);
        let server = ManagedAgentBridgeServer::bind(
            &path,
            capability.clone(),
            "parent-1".into(),
            Arc::new(EmptySource),
            Some(dispatch),
        )
        .await
        .unwrap();
        let stream = UnixStream::connect(&path).await.unwrap();
        let (reader, mut writer) = stream.into_split();
        let request = crate::managed_agent_bridge::ManagedAgentBridgeRequest {
            schema_version: crate::managed_agent_bridge::MANAGED_AGENT_BRIDGE_SCHEMA_VERSION,
            protocol: crate::managed_agent_bridge::MANAGED_AGENT_BRIDGE_PROTOCOL.into(),
            request_id: "r2".into(),
            capability,
            operation: ManagedAgentBridgeOperation::AgentsDispatch {
                operation_id: "operation-1".into(),
                directive: "inspect runtime".into(),
                worker_profile: WorkerProfile::Scout,
                scope: ["src".to_string()].into_iter().collect(),
                supervisor_deadline_seconds: 60,
            },
        };
        writer
            .write_all(format!("{}\n", serde_json::to_string(&request).unwrap()).as_bytes())
            .await
            .unwrap();
        let command = commands.recv().await.unwrap();
        assert_eq!(command.parent_session_id, "parent-1");
        assert_eq!(command.operation_id, "operation-1");
        let run_id = ManagedRunId::new();
        command.respond_to.send(Ok((run_id, false))).unwrap();
        let mut line = String::new();
        BufReader::new(reader).read_line(&mut line).await.unwrap();
        let response: ManagedAgentBridgeResponse = serde_json::from_str(&line).unwrap();
        assert!(matches!(
            response.result,
            crate::managed_agent_bridge::ManagedAgentBridgeResult::Dispatched {
                run_id: returned,
                duplicate: false
            } if returned == run_id
        ));
        drop(server);
    }

    #[tokio::test]
    async fn listener_authenticates_and_uses_bound_parent_scope() {
        let path = std::path::PathBuf::from(format!(
            "/tmp/auspex-bridge-{}.sock",
            uuid::Uuid::new_v4().simple()
        ));
        let capability = "A".repeat(43);
        let server = ManagedAgentBridgeServer::bind(
            &path,
            capability.clone(),
            "parent-1".into(),
            Arc::new(EmptySource),
            None,
        )
        .await
        .unwrap();
        assert_eq!(server.binding().parent_session_id, "parent-1");

        let stream = UnixStream::connect(&path).await.unwrap();
        let (reader, mut writer) = stream.into_split();
        let request = crate::managed_agent_bridge::ManagedAgentBridgeRequest {
            schema_version: crate::managed_agent_bridge::MANAGED_AGENT_BRIDGE_SCHEMA_VERSION,
            protocol: crate::managed_agent_bridge::MANAGED_AGENT_BRIDGE_PROTOCOL.into(),
            request_id: "r1".into(),
            capability,
            operation: ManagedAgentBridgeOperation::AgentsStatus { run_id: None },
        };
        writer
            .write_all(format!("{}\n", serde_json::to_string(&request).unwrap()).as_bytes())
            .await
            .unwrap();
        let mut line = String::new();
        BufReader::new(reader).read_line(&mut line).await.unwrap();
        let response: ManagedAgentBridgeResponse = serde_json::from_str(&line).unwrap();
        assert!(
            matches!(response.result, crate::managed_agent_bridge::ManagedAgentBridgeResult::Ok { runs } if runs.is_empty())
        );
        drop(server);
        assert!(!path.exists());
        let _ = std::fs::remove_file(path);
    }
}
