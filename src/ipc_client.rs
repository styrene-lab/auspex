#![cfg(not(target_arch = "wasm32"))]

use std::path::Path;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use omegon_traits::{
    AcceptedResponse, HelloRequest, IpcEnvelope, IpcEnvelopeKind, IpcEventPayload,
    IpcStateSnapshot, SlashCommandResponse, SubmitPromptRequest, SubscriptionRequest,
    SubscriptionResponse, IPC_MAX_FRAME_BYTES, IPC_PROTOCOL_VERSION,
};
use serde_json::Value;
use tokio::net::UnixStream;
use tokio::time::timeout;

pub const AUSPEX_IPC_CLIENT_NAME: &str = "auspex";
pub const AUSPEX_IPC_CLIENT_VERSION: &str = env!("CARGO_PKG_VERSION");

const IPC_CONNECT_TIMEOUT: Duration = Duration::from_secs(2);
const IPC_RESPONSE_TIMEOUT: Duration = Duration::from_secs(2);
#[allow(dead_code)]
const IPC_EVENT_RETRY_INITIAL_BACKOFF: Duration = Duration::from_secs(1);
#[allow(dead_code)]
const IPC_EVENT_RETRY_MAX_BACKOFF: Duration = Duration::from_secs(30);
const HELLO_REQUEST_ID: [u8; 16] = *b"auspex-hello-001";
const COMMAND_REQUEST_ID: [u8; 16] = *b"auspex-request01";
#[allow(dead_code)]
const SUBSCRIBE_REQUEST_ID: [u8; 16] = *b"auspex-subs-0001";
#[allow(dead_code)]
const IPC_SERVER_EVENT_NAMES: &[&str] = &[
    "turn.started",
    "turn.ended",
    "message.delta",
    "thinking.delta",
    "message.completed",
    "tool.started",
    "tool.updated",
    "tool.ended",
    "agent.completed",
    "phase.changed",
    "decomposition.started",
    "decomposition.child_completed",
    "decomposition.completed",
    "harness.changed",
    "state.changed",
    "system.notification",
    "session.reset",
];

#[derive(Clone, Debug)]
pub struct IpcCommandClient {
    socket_path: String,
}

#[allow(dead_code)]
#[derive(Clone, Debug, Default)]
pub struct IpcEventInbox {
    queue: Arc<Mutex<Vec<IpcEventPayload>>>,
}

#[allow(dead_code)]
impl IpcEventInbox {
    pub fn push(&self, event: IpcEventPayload) {
        if let Ok(mut queue) = self.queue.lock() {
            queue.push(event);
        }
    }

    pub fn drain(&self) -> Vec<IpcEventPayload> {
        if let Ok(mut queue) = self.queue.lock() {
            return std::mem::take(&mut *queue);
        }

        Vec::new()
    }
}

#[allow(dead_code)]
#[derive(Clone, Debug)]
pub struct IpcEventStreamHandle {
    pub inbox: IpcEventInbox,
    socket_path: String,
}

#[allow(dead_code)]
impl IpcEventStreamHandle {
    fn new(socket_path: impl Into<String>) -> Self {
        Self {
            inbox: IpcEventInbox::default(),
            socket_path: socket_path.into(),
        }
    }

    pub fn socket_path(&self) -> &str {
        &self.socket_path
    }
}

#[allow(dead_code)]
pub fn spawn_ipc_event_stream(socket_path: impl Into<String>) -> IpcEventStreamHandle {
    let handle = IpcEventStreamHandle::new(socket_path);
    let worker_handle = handle.clone();

    tokio::spawn(async move {
        run_ipc_event_stream(worker_handle).await;
    });

    handle
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

    #[allow(dead_code)]
    pub async fn get_state(&self) -> Result<IpcStateSnapshot, String> {
        let response = self.request("get_state", None).await?;
        serde_json::from_value::<IpcStateSnapshot>(response)
            .map_err(|error| format!("decode get_state response: {error}"))
    }

    pub async fn run_slash_command(
        &self,
        name: &str,
        args: &str,
    ) -> Result<SlashCommandResponse, String> {
        let payload = serde_json::json!({
            "name": name,
            "args": args,
        });
        let response = self.request("run_slash_command", Some(payload)).await?;
        serde_json::from_value::<SlashCommandResponse>(response)
            .map_err(|error| format!("decode run_slash_command response: {error}"))
    }

    async fn request(&self, method: &str, payload: Option<Value>) -> Result<Value, String> {
        let mut stream = connect_ipc_stream(&self.socket_path).await?;
        perform_hello(
            &mut stream,
            vec![
                "submit_prompt".into(),
                "cancel".into(),
                "run_slash_command".into(),
            ],
        )
        .await?;

        let request = IpcEnvelope {
            protocol_version: IPC_PROTOCOL_VERSION,
            kind: IpcEnvelopeKind::Request,
            request_id: Some(COMMAND_REQUEST_ID),
            method: Some(method.to_string()),
            payload,
            error: None,
        };
        write_envelope(&mut stream, &request).await?;
        let response = read_envelope_with_timeout(&mut stream, Some(IPC_RESPONSE_TIMEOUT)).await?;
        validate_response_envelope(&response, method, &format!("IPC request {method}"))?;

        response
            .payload
            .ok_or_else(|| format!("IPC request {method} returned no payload"))
    }
}

#[allow(dead_code)]
async fn run_ipc_event_stream(handle: IpcEventStreamHandle) {
    let mut backoff = IPC_EVENT_RETRY_INITIAL_BACKOFF;
    let mut first_attempt = true;

    loop {
        if !first_attempt {
            tokio::time::sleep(backoff).await;
            backoff = (backoff * 2).min(IPC_EVENT_RETRY_MAX_BACKOFF);
        }
        first_attempt = false;

        match connect_and_subscribe(&handle.socket_path).await {
            Ok(mut stream) => {
                backoff = IPC_EVENT_RETRY_INITIAL_BACKOFF;
                loop {
                    match read_envelope_with_timeout(&mut stream, None).await {
                        Ok(envelope) => match decode_event_envelope(envelope) {
                            Ok(Some(event)) => handle.inbox.push(event),
                            Ok(None) => {}
                            Err(error) => {
                                eprintln!(
                                    "Ignoring malformed IPC event frame from {}: {error}",
                                    handle.socket_path()
                                );
                            }
                        },
                        Err(error) => {
                            eprintln!(
                                "IPC event stream disconnected from {}: {error}",
                                handle.socket_path()
                            );
                            break;
                        }
                    }
                }
            }
            Err(error) => {
                eprintln!(
                    "IPC event stream attach failed for {}: {error}",
                    handle.socket_path()
                );
            }
        }
    }
}

#[allow(dead_code)]
async fn connect_and_subscribe(socket_path: &str) -> Result<UnixStream, String> {
    let mut stream = connect_ipc_stream(socket_path).await?;
    perform_hello(&mut stream, vec!["subscribe".into()]).await?;

    let payload = serde_json::to_value(SubscriptionRequest {
        events: IPC_SERVER_EVENT_NAMES
            .iter()
            .map(|event| (*event).to_string())
            .collect(),
    })
    .map_err(|error| format!("encode subscribe payload: {error}"))?;
    let request = IpcEnvelope {
        protocol_version: IPC_PROTOCOL_VERSION,
        kind: IpcEnvelopeKind::Request,
        request_id: Some(SUBSCRIBE_REQUEST_ID),
        method: Some("subscribe".into()),
        payload: Some(payload),
        error: None,
    };
    write_envelope(&mut stream, &request).await?;
    let response = read_envelope_with_timeout(&mut stream, Some(IPC_RESPONSE_TIMEOUT)).await?;
    validate_response_envelope(&response, "subscribe", "IPC subscribe")?;
    let subscribed = response
        .payload
        .ok_or_else(|| "IPC subscribe returned no payload".to_string())?;
    serde_json::from_value::<SubscriptionResponse>(subscribed)
        .map_err(|error| format!("decode subscribe response: {error}"))?;

    Ok(stream)
}

async fn connect_ipc_stream(socket_path: &str) -> Result<UnixStream, String> {
    timeout(IPC_CONNECT_TIMEOUT, UnixStream::connect(socket_path))
        .await
        .map_err(|_| format!("IPC connect timed out for {socket_path}"))?
        .map_err(|error| format!("IPC connect failed for {socket_path}: {error}"))
}

async fn perform_hello(stream: &mut UnixStream, capabilities: Vec<String>) -> Result<(), String> {
    let hello = IpcEnvelope {
        protocol_version: IPC_PROTOCOL_VERSION,
        kind: IpcEnvelopeKind::Hello,
        request_id: Some(HELLO_REQUEST_ID),
        method: Some("hello".into()),
        payload: Some(
            serde_json::to_value(HelloRequest {
                client_name: AUSPEX_IPC_CLIENT_NAME.into(),
                client_version: AUSPEX_IPC_CLIENT_VERSION.into(),
                supported_protocol_versions: vec![IPC_PROTOCOL_VERSION],
                capabilities,
            })
            .map_err(|error| format!("encode hello payload: {error}"))?,
        ),
        error: None,
    };
    write_envelope(stream, &hello).await?;
    let hello_response = read_envelope_with_timeout(stream, Some(IPC_RESPONSE_TIMEOUT)).await?;
    validate_response_envelope(&hello_response, "hello", "IPC handshake")
}

fn validate_response_envelope(
    envelope: &IpcEnvelope,
    method: &str,
    context: &str,
) -> Result<(), String> {
    if envelope.kind == IpcEnvelopeKind::Error {
        let message = envelope
            .error
            .as_ref()
            .map(|error| error.message.clone())
            .unwrap_or_else(|| "unknown error".to_string());
        return Err(format!("{context} failed: {message}"));
    }

    if envelope.kind != IpcEnvelopeKind::Response {
        return Err(format!(
            "{context} failed: expected response envelope for {method}, got {:?}",
            envelope.kind
        ));
    }

    if envelope.method.as_deref() != Some(method) {
        return Err(format!(
            "{context} failed: expected {method} response, got {:?}",
            envelope.method
        ));
    }

    if let Some(error) = &envelope.error {
        return Err(format!("{context} failed: {}", error.message));
    }

    Ok(())
}

#[allow(dead_code)]
fn decode_event_envelope(envelope: IpcEnvelope) -> Result<Option<IpcEventPayload>, String> {
    if envelope.kind != IpcEnvelopeKind::Event {
        return Ok(None);
    }

    let payload = envelope
        .payload
        .ok_or_else(|| "IPC event envelope returned no payload".to_string())?;
    serde_json::from_value::<IpcEventPayload>(payload)
        .map(Some)
        .map_err(|error| format!("decode IPC event payload: {error}"))
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

async fn read_envelope_with_timeout(
    stream: &mut UnixStream,
    timeout_duration: Option<Duration>,
) -> Result<IpcEnvelope, String> {
    let mut len_buf = [0u8; 4];
    read_exact(
        stream,
        &mut len_buf,
        timeout_duration,
        "read IPC frame length",
    )
    .await?;
    let len = u32::from_be_bytes(len_buf) as usize;
    if len > IPC_MAX_FRAME_BYTES {
        return Err(format!(
            "IPC frame body exceeded protocol limit: {len} > {IPC_MAX_FRAME_BYTES}"
        ));
    }

    let mut raw = vec![0u8; len];
    read_exact(stream, &mut raw, timeout_duration, "read IPC frame body").await?;
    IpcEnvelope::decode_msgpack(&raw).map_err(|error| format!("decode IPC envelope: {error}"))
}

async fn read_exact(
    stream: &mut UnixStream,
    buffer: &mut [u8],
    timeout_duration: Option<Duration>,
    operation: &str,
) -> Result<(), String> {
    use tokio::io::AsyncReadExt;

    match timeout_duration {
        Some(duration) => timeout(duration, stream.read_exact(buffer))
            .await
            .map_err(|_| format!("{operation} timed out"))?
            .map(|_| ())
            .map_err(|error| format!("{operation}: {error}")),
        None => stream
            .read_exact(buffer)
            .await
            .map(|_| ())
            .map_err(|error| format!("{operation}: {error}")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn ipc_client_reports_missing_socket_as_unavailable() {
        let client = IpcCommandClient::new("/definitely/not/here.sock");
        assert!(!client.is_available());
    }

    #[test]
    fn ipc_event_inbox_drains_fifo_payloads() {
        let inbox = IpcEventInbox::default();
        inbox.push(IpcEventPayload::TurnStarted { turn: 1 });
        inbox.push(IpcEventPayload::MessageCompleted);

        assert_eq!(
            inbox.drain(),
            vec![
                IpcEventPayload::TurnStarted { turn: 1 },
                IpcEventPayload::MessageCompleted,
            ]
        );
        assert!(inbox.drain().is_empty());
    }

    #[test]
    fn ipc_event_stream_handle_clones_share_inbox() {
        let handle = IpcEventStreamHandle::new("/tmp/auspex.sock");
        let clone = handle.clone();

        clone.inbox.push(IpcEventPayload::HarnessChanged);
        handle.inbox.push(IpcEventPayload::SessionReset);

        assert_eq!(
            handle.inbox.drain(),
            vec![
                IpcEventPayload::HarnessChanged,
                IpcEventPayload::SessionReset
            ]
        );
    }

    #[test]
    fn decode_event_envelope_returns_typed_ipc_payload() {
        let envelope = IpcEnvelope {
            protocol_version: IPC_PROTOCOL_VERSION,
            kind: IpcEnvelopeKind::Event,
            request_id: None,
            method: None,
            payload: Some(serde_json::json!({
                "name": "turn.started",
                "data": { "turn": 7 }
            })),
            error: None,
        };

        assert_eq!(
            decode_event_envelope(envelope).unwrap(),
            Some(IpcEventPayload::TurnStarted { turn: 7 })
        );
    }

    #[test]
    fn decode_event_envelope_ignores_non_event_frames() {
        let envelope = IpcEnvelope {
            protocol_version: IPC_PROTOCOL_VERSION,
            kind: IpcEnvelopeKind::Response,
            request_id: Some(COMMAND_REQUEST_ID),
            method: Some("subscribe".into()),
            payload: Some(serde_json::json!({ "events": ["turn.started"] })),
            error: None,
        };

        assert_eq!(decode_event_envelope(envelope).unwrap(), None);
    }

    #[test]
    fn validate_response_envelope_accepts_hello_response_shape() {
        let envelope = IpcEnvelope {
            protocol_version: IPC_PROTOCOL_VERSION,
            kind: IpcEnvelopeKind::Response,
            request_id: Some(HELLO_REQUEST_ID),
            method: Some("hello".into()),
            payload: Some(serde_json::json!({
                "protocol_version": IPC_PROTOCOL_VERSION,
                "server_name": "omegon",
                "omegon_version": "0.0.0",
                "server_pid": 42,
                "cwd": "/tmp",
                "server_instance_id": "instance-1",
                "started_at": "2026-04-07T00:00:00Z",
                "session_id": "session-1",
                "capabilities": ["subscribe"]
            })),
            error: None,
        };

        assert_eq!(
            validate_response_envelope(&envelope, "hello", "IPC handshake"),
            Ok(())
        );
    }
}
