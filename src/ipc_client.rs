#![cfg(not(target_arch = "wasm32"))]

use std::path::Path;
use std::time::Duration;

use omegon_traits::{
    AcceptedResponse, HelloRequest, IPC_PROTOCOL_VERSION, IpcEnvelope, IpcEnvelopeKind,
    SlashCommandResponse, SubmitPromptRequest,
};
use serde_json::Value;
use tokio::net::UnixStream;
use tokio::time::timeout;

pub const AUSPEX_IPC_CLIENT_NAME: &str = "auspex";
pub const AUSPEX_IPC_CLIENT_VERSION: &str = env!("CARGO_PKG_VERSION");

#[derive(Clone, Debug)]
pub struct IpcCommandClient {
    socket_path: String,
}

impl IpcCommandClient {
    pub fn new(socket_path: impl Into<String>) -> Self {
        Self {
            socket_path: socket_path.into(),
        }
    }

    pub fn is_available(&self) -> bool {
        Path::new(&self.socket_path).exists()
    }

    pub async fn submit_prompt(&self, prompt: &str) -> Result<bool, String> {
        let payload = serde_json::to_value(SubmitPromptRequest {
            prompt: prompt.to_string(),
            source: Some(AUSPEX_IPC_CLIENT_NAME.to_string()),
        })
        .map_err(|error| format!("encode submit_prompt payload: {error}"))?;
        let response = self.request("submit_prompt", Some(payload)).await?;
        let accepted = serde_json::from_value::<AcceptedResponse>(response)
            .map_err(|error| format!("decode submit_prompt response: {error}"))?;
        Ok(accepted.accepted)
    }

    pub async fn cancel(&self) -> Result<bool, String> {
        let response = self.request("cancel", None).await?;
        let accepted = serde_json::from_value::<AcceptedResponse>(response)
            .map_err(|error| format!("decode cancel response: {error}"))?;
        Ok(accepted.accepted)
    }

    pub async fn run_slash_command(&self, name: &str, args: &str) -> Result<SlashCommandResponse, String> {
        let payload = serde_json::json!({
            "name": name,
            "args": args,
        });
        let response = self.request("run_slash_command", Some(payload)).await?;
        serde_json::from_value::<SlashCommandResponse>(response)
            .map_err(|error| format!("decode run_slash_command response: {error}"))
    }

    async fn request(&self, method: &str, payload: Option<Value>) -> Result<Value, String> {
        let mut stream = timeout(Duration::from_secs(2), UnixStream::connect(&self.socket_path))
            .await
            .map_err(|_| format!("IPC connect timed out for {}", self.socket_path))?
            .map_err(|error| format!("IPC connect failed for {}: {error}", self.socket_path))?;

        let hello = IpcEnvelope {
            protocol_version: IPC_PROTOCOL_VERSION,
            kind: IpcEnvelopeKind::Hello,
            request_id: Some(*b"auspex-hello-001"),
            method: Some("hello".into()),
            payload: Some(
                serde_json::to_value(HelloRequest {
                    client_name: AUSPEX_IPC_CLIENT_NAME.into(),
                    client_version: AUSPEX_IPC_CLIENT_VERSION.into(),
                    supported_protocol_versions: vec![IPC_PROTOCOL_VERSION],
                    capabilities: vec!["submit_prompt".into(), "cancel".into(), "run_slash_command".into()],
                })
                .map_err(|error| format!("encode hello payload: {error}"))?,
            ),
            error: None,
        };
        write_envelope(&mut stream, &hello).await?;
        let hello_response = read_envelope(&mut stream).await?;
        if hello_response.kind != IpcEnvelopeKind::Hello {
            return Err(format!(
                "IPC handshake failed: expected hello response, got {:?}",
                hello_response.kind
            ));
        }
        if let Some(error) = hello_response.error {
            return Err(format!("IPC handshake failed: {}", error.message));
        }

        let request = IpcEnvelope {
            protocol_version: IPC_PROTOCOL_VERSION,
            kind: IpcEnvelopeKind::Request,
            request_id: Some(*b"auspex-request01"),
            method: Some(method.to_string()),
            payload,
            error: None,
        };
        write_envelope(&mut stream, &request).await?;
        let response = read_envelope(&mut stream).await?;

        if response.kind != IpcEnvelopeKind::Response {
            return Err(format!(
                "IPC request {} failed: expected response envelope, got {:?}",
                method, response.kind
            ));
        }
        if let Some(error) = response.error {
            return Err(format!("IPC request {} failed: {}", method, error.message));
        }
        response
            .payload
            .ok_or_else(|| format!("IPC request {} returned no payload", method))
    }
}

async fn write_envelope(stream: &mut UnixStream, envelope: &IpcEnvelope) -> Result<(), String> {
    let raw = envelope
        .encode_msgpack()
        .map_err(|error| format!("encode IPC envelope: {error}"))?;
    let len = (raw.len() as u32).to_be_bytes();
    use tokio::io::AsyncWriteExt;
    stream
        .write_all(&len)
        .await
        .map_err(|error| format!("write IPC frame length: {error}"))?;
    stream
        .write_all(&raw)
        .await
        .map_err(|error| format!("write IPC frame body: {error}"))?;
    stream
        .flush()
        .await
        .map_err(|error| format!("flush IPC frame: {error}"))?;
    Ok(())
}

async fn read_envelope(stream: &mut UnixStream) -> Result<IpcEnvelope, String> {
    use tokio::io::AsyncReadExt;
    let mut len_buf = [0u8; 4];
    timeout(Duration::from_secs(2), stream.read_exact(&mut len_buf))
        .await
        .map_err(|_| "read IPC frame length timed out".to_string())?
        .map_err(|error| format!("read IPC frame length: {error}"))?;
    let len = u32::from_be_bytes(len_buf) as usize;
    let mut raw = vec![0u8; len];
    timeout(Duration::from_secs(2), stream.read_exact(&mut raw))
        .await
        .map_err(|_| "read IPC frame body timed out".to_string())?
        .map_err(|error| format!("read IPC frame body: {error}"))?;
    IpcEnvelope::decode_msgpack(&raw).map_err(|error| format!("decode IPC envelope: {error}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ipc_client_reports_missing_socket_as_unavailable() {
        let client = IpcCommandClient::new("/definitely/not/here.sock");
        assert!(!client.is_available());
    }
}
