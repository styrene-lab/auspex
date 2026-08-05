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
                tokio::spawn(async move {
                    let _ = serve_connection(stream, &capability, &parent_session_id, source).await;
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
        let result = match request.operation {
            ManagedAgentBridgeOperation::AgentsStatus { run_id } => {
                source.status(parent_session_id, run_id)
            }
        };
        let response = match result {
            Ok(runs) => ManagedAgentBridgeResponse::ok(request.request_id, runs),
            Err(_) => ManagedAgentBridgeResponse::error(
                request.request_id,
                ManagedAgentBridgeErrorCode::ScopeDenied,
                "managed run unavailable",
            ),
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
