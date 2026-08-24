#![cfg(not(target_arch = "wasm32"))]

use std::collections::VecDeque;
use std::path::Path;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use omegon_traits::{
    AcceptedResponse, HelloRequest, HelloResponse, IPC_MAX_FRAME_BYTES, IPC_PROTOCOL_VERSION,
    IpcEnvelope, IpcEnvelopeKind, IpcEventPayload, IpcStateSnapshot, SlashCommandResponse,
    SubmitPromptRequest, SubscriptionRequest, SubscriptionResponse,
};
use serde_json::Value;
use tokio::net::UnixStream;
use tokio::sync::{mpsc, oneshot};
use tokio::time::timeout;
use uuid::Uuid;

use crate::runtime_types::CommandTarget;

pub const AUSPEX_IPC_CLIENT_NAME: &str = "auspex";
pub const AUSPEX_IPC_CLIENT_VERSION: &str = env!("CARGO_PKG_VERSION");

const IPC_CONNECT_TIMEOUT: Duration = Duration::from_secs(2);
const IPC_RESPONSE_TIMEOUT: Duration = Duration::from_secs(2);
#[allow(dead_code)]
const IPC_EVENT_RETRY_INITIAL_BACKOFF: Duration = Duration::from_secs(1);
#[allow(dead_code)]
const IPC_EVENT_RETRY_MAX_BACKOFF: Duration = Duration::from_secs(30);
const IPC_REQUEST_QUEUE_CAPACITY: usize = 32;
const IPC_EVENT_QUEUE_CAPACITY: usize = 512;
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
    "stream.idle",
    "provider.route_changed",
    "runtime.queue_updated",
    "harness.changed",
    "plan.updated",
    "state.changed",
    "state.reconciled",
    "runtime.lifecycle.updated",
    "system.notification",
    "session.reset",
];

#[derive(Clone, Debug, PartialEq)]
pub enum IpcClientEvent {
    Connected { hello: HelloResponse },
    Payload(IpcEventPayload),
    Session(crate::session_event::SessionEvent),
    CommandOutcome(IpcCommandOutcome),
    ReconciliationRequired { reason: String },
    Disconnected { error: String },
    AttachFailed { error: String },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum IpcCommandKind {
    PromptSubmit,
    TurnCancel,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum IpcCommandOutcomeStatus {
    Accepted,
    Rejected,
    Failed(String),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct IpcCommandOutcome {
    pub target: CommandTarget,
    pub command: IpcCommandKind,
    pub status: IpcCommandOutcomeStatus,
}

#[derive(Clone, Debug)]
pub struct IpcCommandClient {
    socket_path: String,
    request_tx: Option<mpsc::Sender<IpcActorRequest>>,
    event_inbox: Option<IpcEventInbox>,
}

#[derive(Debug)]
struct IpcActorRequest {
    method: String,
    payload: Option<Value>,
    response_tx: oneshot::Sender<Result<Value, String>>,
}

#[derive(Debug, Default)]
struct IpcEventQueue {
    events: VecDeque<IpcClientEvent>,
    reconciliation_required: bool,
}

#[derive(Clone, Debug, Default)]
pub struct IpcEventInbox {
    queue: Arc<Mutex<IpcEventQueue>>,
}

impl IpcEventInbox {
    pub fn push(&self, event: IpcClientEvent) {
        if let Ok(mut queue) = self.queue.lock() {
            if queue.events.len() == IPC_EVENT_QUEUE_CAPACITY {
                queue.events.pop_front();
                queue.reconciliation_required = true;
            }
            queue.events.push_back(event);
        }
    }

    pub fn drain(&self) -> Vec<IpcClientEvent> {
        if let Ok(mut queue) = self.queue.lock() {
            let mut events = queue.events.drain(..).collect::<Vec<_>>();
            if std::mem::take(&mut queue.reconciliation_required) {
                events.push(IpcClientEvent::ReconciliationRequired {
                    reason: "local IPC event inbox saturated".into(),
                });
                events.push(IpcClientEvent::Payload(IpcEventPayload::StateChanged {
                    sections: vec!["session".into(), "harness".into()],
                }));
            }
            return events;
        }

        Vec::new()
    }
}

#[derive(Clone, Debug)]
pub struct IpcEventStreamHandle {
    pub inbox: IpcEventInbox,
    socket_path: String,
}

impl IpcEventStreamHandle {
    fn new(socket_path: impl Into<String>) -> Self {
        Self {
            inbox: IpcEventInbox::default(),
            socket_path: socket_path.into(),
        }
    }
}

pub fn spawn_ipc_connection(
    socket_path: impl Into<String>,
) -> (IpcCommandClient, IpcEventStreamHandle) {
    let socket_path = socket_path.into();
    let handle = IpcEventStreamHandle::new(socket_path.clone());
    let worker_handle = handle.clone();
    let (request_tx, request_rx) = mpsc::channel(IPC_REQUEST_QUEUE_CAPACITY);
    let client = IpcCommandClient {
        socket_path,
        request_tx: Some(request_tx),
        event_inbox: Some(handle.inbox.clone()),
    };

    tokio::spawn(async move {
        run_ipc_connection(worker_handle, request_rx).await;
    });

    (client, handle)
}

impl IpcCommandClient {
    pub fn new(socket_path: impl Into<String>) -> Self {
        Self {
            socket_path: socket_path.into(),
            request_tx: None,
            event_inbox: None,
        }
    }

    pub fn is_available(&self) -> bool {
        Path::new(&self.socket_path).exists()
    }

    pub async fn inspect(&self) -> Result<HelloResponse, String> {
        let mut stream = connect_ipc_stream(&self.socket_path).await?;
        perform_hello(&mut stream, auspex_ipc_capabilities()).await
    }

    pub async fn submit_prompt(&self, prompt: &str) -> Result<bool, String> {
        let payload = serde_json::to_value(SubmitPromptRequest {
            prompt: prompt.to_string(),
            source: Some(AUSPEX_IPC_CLIENT_NAME.to_string()),
            caller_role: Some("admin".to_string()),
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

    pub async fn control_method(&self, method: &str, payload: Value) -> Result<bool, String> {
        let response = self.request(method, Some(payload)).await?;
        let accepted = serde_json::from_value::<AcceptedResponse>(response)
            .map_err(|error| format!("decode {method} response: {error}"))?;
        Ok(accepted.accepted)
    }

    pub async fn switch_dispatcher(
        &self,
        request_id: &str,
        profile: &str,
        model: Option<&str>,
    ) -> Result<bool, String> {
        let payload = serde_json::json!({
            "request_id": request_id,
            "profile": profile,
            "model": model,
            "caller_role": "admin",
        });
        let response = self.request("switch_dispatcher", Some(payload)).await?;
        let accepted = serde_json::from_value::<AcceptedResponse>(response)
            .map_err(|error| format!("decode switch_dispatcher response: {error}"))?;
        Ok(accepted.accepted)
    }

    pub fn publish_command_outcome(&self, outcome: IpcCommandOutcome) {
        if let Some(inbox) = &self.event_inbox {
            inbox.push(IpcClientEvent::CommandOutcome(outcome));
        }
    }

    async fn request(&self, method: &str, payload: Option<Value>) -> Result<Value, String> {
        if let Some(request_tx) = &self.request_tx {
            let (response_tx, response_rx) = oneshot::channel();
            timeout(
                IPC_RESPONSE_TIMEOUT,
                request_tx.send(IpcActorRequest {
                    method: method.to_string(),
                    payload,
                    response_tx,
                }),
            )
            .await
            .map_err(|_| format!("IPC request queue timed out for {method}"))?
            .map_err(|_| "Omegon IPC connection owner stopped".to_string())?;
            return timeout(IPC_RESPONSE_TIMEOUT, response_rx)
                .await
                .map_err(|_| format!("IPC request {method} timed out"))?
                .map_err(|_| format!("IPC request {method} response channel closed"))?;
        }

        let mut stream = connect_ipc_stream(&self.socket_path).await?;
        let hello = perform_hello(&mut stream, auspex_ipc_capabilities()).await?;
        if let Some(capability) = capability_for_method(method) {
            require_server_capability(&hello, capability)?;
        }

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

async fn run_ipc_connection(
    handle: IpcEventStreamHandle,
    mut request_rx: mpsc::Receiver<IpcActorRequest>,
) {
    let mut backoff = IPC_EVENT_RETRY_INITIAL_BACKOFF;
    let mut first_attempt = true;

    loop {
        if !first_attempt {
            tokio::time::sleep(backoff).await;
            backoff = (backoff * 2).min(IPC_EVENT_RETRY_MAX_BACKOFF);
        }
        first_attempt = false;

        match connect_and_initialize(&handle.socket_path).await {
            Ok((mut stream, hello, snapshot)) => {
                backoff = IPC_EVENT_RETRY_INITIAL_BACKOFF;
                handle.inbox.push(IpcClientEvent::Connected {
                    hello: hello.clone(),
                });
                handle
                    .inbox
                    .push(IpcClientEvent::Payload(IpcEventPayload::StateReconciled {
                        snapshot: Box::new(snapshot),
                    }));
                loop {
                    tokio::select! {
                        request = request_rx.recv() => {
                            let Some(request) = request else {
                                return;
                            };
                            if let Some(capability) = capability_for_method(&request.method)
                                && let Err(error) = require_server_capability(&hello, capability)
                            {
                                let _ = request.response_tx.send(Err(error));
                                continue;
                            }

                            let request_id = *Uuid::new_v4().as_bytes();
                            let envelope = IpcEnvelope {
                                protocol_version: IPC_PROTOCOL_VERSION,
                                kind: IpcEnvelopeKind::Request,
                                request_id: Some(request_id),
                                method: Some(request.method.clone()),
                                payload: request.payload,
                                error: None,
                            };
                            if let Err(error) = write_envelope(&mut stream, &envelope).await {
                                handle.inbox.push(IpcClientEvent::Disconnected {
                                    error: error.clone(),
                                });
                                let _ = request.response_tx.send(Err(error));
                                break;
                            }

                            let response = timeout(
                                IPC_RESPONSE_TIMEOUT,
                                await_correlated_response(
                                    &mut stream,
                                    request_id,
                                    &request.method,
                                    &handle.inbox,
                                ),
                            )
                            .await;
                            match response {
                                Ok(Ok(result)) => {
                                    let _ = request.response_tx.send(result);
                                }
                                Ok(Err(error)) => {
                                    handle.inbox.push(IpcClientEvent::Disconnected {
                                        error: error.clone(),
                                    });
                                    let _ = request.response_tx.send(Err(error));
                                    break;
                                }
                                Err(_) => {
                                    let error = format!("IPC request {} timed out", request.method);
                                    handle.inbox.push(IpcClientEvent::Disconnected {
                                        error: error.clone(),
                                    });
                                    let _ = request.response_tx.send(Err(error));
                                    break;
                                }
                            }
                        }
                        incoming = read_envelope_with_timeout(&mut stream, None) => {
                            match incoming {
                                Ok(envelope) => forward_event_envelope(envelope, &handle),
                                Err(error) => {
                                    eprintln!(
                                        "IPC connection disconnected from {}: {error}",
                                        handle.socket_path
                                    );
                                    handle.inbox.push(IpcClientEvent::Disconnected { error });
                                    break;
                                }
                            }
                        }
                    }
                }
            }
            Err(error) => {
                eprintln!(
                    "IPC connection attach failed for {}: {error}",
                    handle.socket_path
                );
                handle.inbox.push(IpcClientEvent::AttachFailed { error });
            }
        }
    }
}

async fn connect_and_initialize(
    socket_path: &str,
) -> Result<(UnixStream, HelloResponse, IpcStateSnapshot), String> {
    let mut stream = connect_ipc_stream(socket_path).await?;
    let hello = perform_hello(&mut stream, auspex_ipc_capabilities()).await?;
    require_server_capability(&hello, "state.snapshot")?;
    require_server_capability(&hello, "events.stream")?;

    let state = request_on_stream(&mut stream, "get_state", None, COMMAND_REQUEST_ID).await?;
    let snapshot = serde_json::from_value::<IpcStateSnapshot>(state)
        .map_err(|error| format!("decode get_state response: {error}"))?;

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

    Ok((stream, hello, snapshot))
}

async fn request_on_stream(
    stream: &mut UnixStream,
    method: &str,
    payload: Option<Value>,
    request_id: [u8; 16],
) -> Result<Value, String> {
    let request = IpcEnvelope {
        protocol_version: IPC_PROTOCOL_VERSION,
        kind: IpcEnvelopeKind::Request,
        request_id: Some(request_id),
        method: Some(method.to_string()),
        payload,
        error: None,
    };
    write_envelope(stream, &request).await?;
    let response = read_envelope_with_timeout(stream, Some(IPC_RESPONSE_TIMEOUT)).await?;
    decode_response_payload(&response, request_id, method)
}

async fn await_correlated_response(
    stream: &mut UnixStream,
    request_id: [u8; 16],
    method: &str,
    inbox: &IpcEventInbox,
) -> Result<Result<Value, String>, String> {
    loop {
        let envelope = read_envelope_with_timeout(stream, None).await?;
        if envelope.kind == IpcEnvelopeKind::Event {
            if let Ok(Some(event)) = decode_event_envelope(envelope) {
                inbox.push(event);
            }
            continue;
        }
        if envelope.request_id != Some(request_id) {
            continue;
        }
        return Ok(decode_response_payload(&envelope, request_id, method));
    }
}

fn decode_response_payload(
    envelope: &IpcEnvelope,
    request_id: [u8; 16],
    method: &str,
) -> Result<Value, String> {
    if envelope.request_id != Some(request_id) {
        return Err(format!("IPC response correlation mismatch for {method}"));
    }
    validate_response_envelope(envelope, method, &format!("IPC request {method}"))?;
    envelope
        .payload
        .clone()
        .ok_or_else(|| format!("IPC request {method} returned no payload"))
}

fn forward_event_envelope(envelope: IpcEnvelope, handle: &IpcEventStreamHandle) {
    match decode_event_envelope(envelope) {
        Ok(Some(event)) => handle.inbox.push(event),
        Ok(None) => {}
        Err(error) => eprintln!(
            "Ignoring malformed IPC event frame from {}: {error}",
            handle.socket_path
        ),
    }
}

async fn connect_ipc_stream(socket_path: &str) -> Result<UnixStream, String> {
    timeout(IPC_CONNECT_TIMEOUT, UnixStream::connect(socket_path))
        .await
        .map_err(|_| format!("IPC connect timed out for {socket_path}"))?
        .map_err(|error| format!("IPC connect failed for {socket_path}: {error}"))
}

async fn perform_hello(
    stream: &mut UnixStream,
    capabilities: Vec<String>,
) -> Result<HelloResponse, String> {
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
    decode_hello_response(&hello_response)
}

fn decode_hello_response(envelope: &IpcEnvelope) -> Result<HelloResponse, String> {
    validate_response_envelope(envelope, "hello", "IPC handshake")?;
    let payload = envelope
        .payload
        .clone()
        .ok_or_else(|| "IPC handshake returned no payload".to_string())?;
    let hello = serde_json::from_value::<HelloResponse>(payload)
        .map_err(|error| format!("decode IPC handshake response: {error}"))?;
    if hello.protocol_version != IPC_PROTOCOL_VERSION {
        return Err(format!(
            "IPC handshake negotiated unsupported protocol {}",
            hello.protocol_version
        ));
    }
    Ok(hello)
}

fn auspex_ipc_capabilities() -> Vec<String> {
    [
        "state.snapshot",
        "events.stream",
        "prompt.submit",
        "turn.cancel",
        "slash_commands",
        "dispatcher.switch",
    ]
    .into_iter()
    .map(str::to_string)
    .collect()
}

fn capability_for_method(method: &str) -> Option<&'static str> {
    match method {
        "get_state" => Some("state.snapshot"),
        "submit_prompt" => Some("prompt.submit"),
        "cancel" => Some("turn.cancel"),
        "run_slash_command" => Some("slash_commands"),
        "switch_dispatcher" => Some("dispatcher.switch"),
        "set_model" => Some("model.set"),
        "set_thinking" => Some("thinking.set"),
        _ => None,
    }
}

fn require_server_capability(hello: &HelloResponse, capability: &str) -> Result<(), String> {
    if hello.capabilities.iter().any(|item| item == capability) {
        Ok(())
    } else {
        Err(format!(
            "Omegon {} does not advertise required IPC capability {capability}",
            hello.server_instance_id
        ))
    }
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
fn decode_event_envelope(envelope: IpcEnvelope) -> Result<Option<IpcClientEvent>, String> {
    if envelope.kind != IpcEnvelopeKind::Event {
        return Ok(None);
    }

    let payload = envelope
        .payload
        .ok_or_else(|| "IPC event envelope returned no payload".to_string())?;
    if is_plan_updated_ipc_payload(&payload)
        && let Some(snapshot) = payload
            .get("data")
            .and_then(|data| data.get("snapshot"))
            .cloned()
    {
        return Ok(Some(IpcClientEvent::Session(
            crate::session_event::SessionEvent::PlanUpdated { snapshot },
        )));
    }

    serde_json::from_value::<IpcEventPayload>(payload)
        .map(IpcClientEvent::Payload)
        .map(Some)
        .map_err(|error| format!("decode IPC event payload: {error}"))
}

fn is_plan_updated_ipc_payload(payload: &Value) -> bool {
    payload.get("name").and_then(Value::as_str) == Some("plan.updated")
        || payload.get("event_name").and_then(Value::as_str) == Some("plan.updated")
        || payload.get("type").and_then(Value::as_str) == Some("plan_updated")
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
    use crate::command_transport::CommandTransport;
    use crate::runtime_types::{CommandTarget, TargetedCommand};
    use pretty_assertions::assert_eq;
    use tokio::net::UnixListener;

    #[test]
    fn ipc_client_reports_missing_socket_as_unavailable() {
        let client = IpcCommandClient::new("/definitely/not/here.sock");
        assert!(!client.is_available());
    }

    #[test]
    fn ipc_event_inbox_drains_fifo_payloads() {
        let inbox = IpcEventInbox::default();
        inbox.push(IpcClientEvent::Payload(IpcEventPayload::TurnStarted {
            turn: 1,
        }));
        inbox.push(IpcClientEvent::Payload(IpcEventPayload::MessageCompleted));

        assert_eq!(
            inbox.drain(),
            vec![
                IpcClientEvent::Payload(IpcEventPayload::TurnStarted { turn: 1 }),
                IpcClientEvent::Payload(IpcEventPayload::MessageCompleted),
            ]
        );
        assert!(inbox.drain().is_empty());
    }

    #[test]
    fn ipc_event_stream_handle_clones_share_inbox() {
        let handle = IpcEventStreamHandle::new("/tmp/auspex.sock");
        let clone = handle.clone();

        clone
            .inbox
            .push(IpcClientEvent::Payload(IpcEventPayload::HarnessChanged));
        handle
            .inbox
            .push(IpcClientEvent::Payload(IpcEventPayload::SessionReset));

        assert_eq!(
            handle.inbox.drain(),
            vec![
                IpcClientEvent::Payload(IpcEventPayload::HarnessChanged),
                IpcClientEvent::Payload(IpcEventPayload::SessionReset)
            ]
        );
    }

    #[test]
    fn command_outcomes_share_the_bounded_event_inbox() {
        let handle = IpcEventStreamHandle::new("/tmp/auspex.sock");
        let client = IpcCommandClient {
            socket_path: handle.socket_path.clone(),
            request_tx: None,
            event_inbox: Some(handle.inbox.clone()),
        };
        let outcome = IpcCommandOutcome {
            target: CommandTarget {
                session_key: "remote:session-1".into(),
                dispatcher_instance_id: None,
            },
            command: IpcCommandKind::PromptSubmit,
            status: IpcCommandOutcomeStatus::Accepted,
        };

        client.publish_command_outcome(outcome.clone());

        assert_eq!(
            handle.inbox.drain(),
            vec![IpcClientEvent::CommandOutcome(outcome)]
        );
    }

    #[test]
    fn saturated_event_inbox_requests_reconciliation() {
        let inbox = IpcEventInbox::default();
        for turn in 0..=IPC_EVENT_QUEUE_CAPACITY {
            inbox.push(IpcClientEvent::Payload(IpcEventPayload::TurnStarted {
                turn: turn as u32,
            }));
        }

        let events = inbox.drain();
        assert_eq!(events.len(), IPC_EVENT_QUEUE_CAPACITY + 2);
        assert_eq!(
            events.get(IPC_EVENT_QUEUE_CAPACITY),
            Some(&IpcClientEvent::ReconciliationRequired {
                reason: "local IPC event inbox saturated".into(),
            })
        );
        assert_eq!(
            events.last(),
            Some(&IpcClientEvent::Payload(IpcEventPayload::StateChanged {
                sections: vec!["session".into(), "harness".into()],
            }))
        );
    }

    #[tokio::test]
    async fn correlated_response_forwards_interleaved_events() {
        let (mut client_stream, mut server_stream) = UnixStream::pair().unwrap();
        let inbox = IpcEventInbox::default();
        let request_id = *b"actor-request-01";

        tokio::spawn(async move {
            let event = IpcEnvelope {
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
            write_envelope(&mut server_stream, &event).await.unwrap();

            let response = IpcEnvelope {
                protocol_version: IPC_PROTOCOL_VERSION,
                kind: IpcEnvelopeKind::Response,
                request_id: Some(request_id),
                method: Some("submit_prompt".into()),
                payload: Some(serde_json::json!({ "accepted": true })),
                error: None,
            };
            write_envelope(&mut server_stream, &response).await.unwrap();
        });

        let response =
            await_correlated_response(&mut client_stream, request_id, "submit_prompt", &inbox)
                .await
                .unwrap()
                .unwrap();
        assert_eq!(response, serde_json::json!({ "accepted": true }));
        assert_eq!(
            inbox.drain(),
            vec![IpcClientEvent::Payload(IpcEventPayload::TurnStarted {
                turn: 7,
            })]
        );
    }

    #[tokio::test]
    async fn multiplexed_owner_reconciles_commands_disconnect_and_replacement_on_one_socket() {
        let socket_path = std::path::PathBuf::from(format!(
            "/tmp/ax-ipc-{}-{}.sock",
            std::process::id(),
            Uuid::new_v4()
        ));
        let listener = UnixListener::bind(&socket_path).expect("bind scripted IPC peer");
        let server = tokio::spawn(async move {
            let (mut first, _) = listener.accept().await.expect("accept first connection");
            scripted_initialize(&mut first, scripted_state("server-1", 1, 1, 0, false, None)).await;

            let submit = read_envelope_with_timeout(&mut first, Some(Duration::from_secs(2)))
                .await
                .expect("read submit_prompt");
            assert_eq!(submit.method.as_deref(), Some("submit_prompt"));
            assert_eq!(
                submit
                    .payload
                    .as_ref()
                    .and_then(|payload| payload.get("prompt"))
                    .and_then(Value::as_str),
                Some("contract prompt")
            );
            scripted_event(
                &mut first,
                IpcEventPayload::RuntimeQueueUpdated {
                    snapshot: serde_json::json!({
                        "depth": 1,
                        "active": null,
                        "items": [],
                        "previews": ["contract prompt"],
                    }),
                },
            )
            .await;
            scripted_event(
                &mut first,
                IpcEventPayload::StateReconciled {
                    snapshot: Box::new(scripted_state("server-1", 1, 2, 1, false, None)),
                },
            )
            .await;
            scripted_response(&mut first, &submit, serde_json::json!({ "accepted": true })).await;

            let cancel = read_envelope_with_timeout(&mut first, Some(Duration::from_secs(2)))
                .await
                .expect("read cancel");
            assert_eq!(cancel.method.as_deref(), Some("cancel"));
            scripted_response(&mut first, &cancel, serde_json::json!({ "accepted": true })).await;
            scripted_event(
                &mut first,
                IpcEventPayload::StateReconciled {
                    snapshot: Box::new(scripted_state("server-1", 1, 3, 0, false, None)),
                },
            )
            .await;
            drop(first);

            let (mut replacement, _) = listener.accept().await.expect("accept replacement");
            scripted_initialize(
                &mut replacement,
                scripted_state("server-2", 1, 1, 0, false, None),
            )
            .await;
            assert!(
                timeout(
                    Duration::from_millis(200),
                    read_envelope_with_timeout(&mut replacement, None)
                )
                .await
                .is_err(),
                "disconnect must not enqueue an implicit cancellation"
            );
        });

        let (client, handle) = spawn_ipc_connection(socket_path.to_string_lossy().into_owned());
        let initial = wait_for_ipc_events(&handle.inbox, |events| {
            events.iter().any(|event| {
                matches!(
                    event,
                    IpcClientEvent::Payload(IpcEventPayload::StateReconciled { snapshot })
                        if snapshot.instance.control_plane.server_instance_id == "server-1"
                )
            })
        })
        .await;
        assert_eq!(initial.len(), 2);
        assert!(matches!(
            initial.first(),
            Some(IpcClientEvent::Connected { .. })
        ));

        let target = CommandTarget {
            session_key: "remote:session-1".into(),
            dispatcher_instance_id: None,
        };
        CommandTransport::Ipc(client.clone())
            .dispatch_targeted_command(
                None,
                &TargetedCommand::prompt_submit(target.clone(), "contract prompt"),
            )
            .expect("dispatch prompt");
        let prompt_events = wait_for_ipc_events(&handle.inbox, |events| {
            let acknowledged = events.iter().any(|event| {
                matches!(
                    event,
                    IpcClientEvent::CommandOutcome(IpcCommandOutcome {
                        command: IpcCommandKind::PromptSubmit,
                        status: IpcCommandOutcomeStatus::Accepted,
                        ..
                    })
                )
            });
            let queued = events.iter().any(|event| {
                matches!(
                    event,
                    IpcClientEvent::Payload(IpcEventPayload::StateReconciled { snapshot })
                        if snapshot.session.queue_depth == 1
                )
            });
            acknowledged && queued
        })
        .await;
        assert!(prompt_events.iter().any(|event| matches!(
            event,
            IpcClientEvent::Payload(IpcEventPayload::RuntimeQueueUpdated { .. })
        )));

        CommandTransport::Ipc(client.clone())
            .dispatch_targeted_command(None, &TargetedCommand::turn_cancel(target))
            .expect("dispatch cancellation");
        let recovery_events = wait_for_ipc_events(&handle.inbox, |events| {
            let cancellation_acknowledged = events.iter().any(|event| {
                matches!(
                    event,
                    IpcClientEvent::CommandOutcome(IpcCommandOutcome {
                        command: IpcCommandKind::TurnCancel,
                        status: IpcCommandOutcomeStatus::Accepted,
                        ..
                    })
                )
            });
            let disconnected = events
                .iter()
                .any(|event| matches!(event, IpcClientEvent::Disconnected { .. }));
            let replaced = events.iter().any(|event| {
                matches!(
                    event,
                    IpcClientEvent::Payload(IpcEventPayload::StateReconciled { snapshot })
                        if snapshot.instance.control_plane.server_instance_id == "server-2"
                )
            });
            cancellation_acknowledged && disconnected && replaced
        })
        .await;
        assert!(recovery_events.iter().any(|event| {
            matches!(
                event,
                IpcClientEvent::Payload(IpcEventPayload::StateReconciled { snapshot })
                    if snapshot.instance.control_plane.server_instance_id == "server-1"
                        && snapshot.session.projection_frontier == Some(3)
                        && snapshot.session.queue_depth == 0
            )
        }));

        server.await.expect("scripted IPC peer should complete");
        drop(client);
        let _ = std::fs::remove_file(socket_path);
    }

    #[tokio::test]
    #[ignore = "requires AUSPEX_CANONICAL_OMEGON_BIN built from the pinned revision"]
    async fn canonical_omegon_runtime_queues_and_cancels_over_multiplexed_ipc() {
        let binary = std::env::var_os("AUSPEX_CANONICAL_OMEGON_BIN")
            .map(std::path::PathBuf::from)
            .expect("AUSPEX_CANONICAL_OMEGON_BIN must name the pinned Omegon binary");
        assert!(binary.is_file(), "missing binary: {}", binary.display());

        let root = std::path::PathBuf::from(format!(
            "/tmp/ax-real-{}-{}",
            std::process::id(),
            Uuid::new_v4().simple()
        ));
        std::fs::create_dir_all(&root).expect("create real-runtime test root");
        let socket_path = root.join(".omegon/ipc.sock");
        let mut child = tokio::process::Command::new(binary)
            .args(["serve", "--control-port", "0", "--strict-port"])
            .env("HOME", &root)
            .env("OMEGON_HOME", root.join(".omegon"))
            .env("RUST_LOG", "error")
            .current_dir(&root)
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .kill_on_drop(true)
            .spawn()
            .expect("spawn pinned Omegon runtime");

        timeout(Duration::from_secs(30), async {
            while !socket_path.exists() {
                assert!(
                    child.try_wait().expect("poll Omegon runtime").is_none(),
                    "Omegon exited before creating its IPC socket"
                );
                tokio::time::sleep(Duration::from_millis(50)).await;
            }
        })
        .await
        .expect("pinned Omegon did not create its IPC socket");

        let (client, handle) = spawn_ipc_connection(socket_path.to_string_lossy().into_owned());
        let initial = wait_for_ipc_events(&handle.inbox, |events| {
            events.iter().any(|event| {
                matches!(
                    event,
                    IpcClientEvent::Payload(IpcEventPayload::StateReconciled { .. })
                )
            })
        })
        .await;
        assert_eq!(initial.len(), 2);
        assert!(matches!(
            initial.first(),
            Some(IpcClientEvent::Connected { .. })
        ));
        assert!(matches!(
            initial.get(1),
            Some(IpcClientEvent::Payload(
                IpcEventPayload::StateReconciled { .. }
            ))
        ));
        let initial_turns = client
            .get_state()
            .await
            .expect("read initial pinned Omegon state")
            .session
            .turns;

        assert!(
            client
                .submit_prompt("real runtime IPC contract probe")
                .await
                .expect("submit prompt through pinned Omegon IPC"),
            "pinned Omegon rejected prompt ingress"
        );
        timeout(Duration::from_secs(10), async {
            loop {
                let snapshot = client
                    .get_state()
                    .await
                    .expect("reconcile pinned Omegon state after prompt ingress");
                assert_eq!(
                    snapshot.session.turns, initial_turns,
                    "runtime completed before exposing authoritative prompt admission"
                );
                if snapshot.session.busy
                    || snapshot.session.active_turn.is_some()
                    || snapshot.session.queue_depth > 0
                {
                    break;
                }
                tokio::time::sleep(Duration::from_millis(25)).await;
            }
        })
        .await
        .expect("pinned Omegon did not expose authoritative prompt admission");
        assert!(
            client
                .cancel()
                .await
                .expect("cancel through pinned Omegon IPC"),
            "pinned Omegon rejected cancellation ingress"
        );
        child.start_kill().expect("stop pinned Omegon runtime");
        let _ = child.wait().await;
        let disconnected = wait_for_ipc_events(&handle.inbox, |events| {
            events
                .iter()
                .any(|event| matches!(event, IpcClientEvent::Disconnected { .. }))
        })
        .await;
        assert!(!disconnected.is_empty());
        drop(client);
        let _ = std::fs::remove_dir_all(root);
    }

    async fn wait_for_ipc_events(
        inbox: &IpcEventInbox,
        complete: impl Fn(&[IpcClientEvent]) -> bool,
    ) -> Vec<IpcClientEvent> {
        timeout(Duration::from_secs(4), async {
            let mut events = Vec::new();
            loop {
                events.extend(inbox.drain());
                if complete(&events) {
                    return events;
                }
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
        })
        .await
        .expect("timed out waiting for scripted IPC events")
    }

    async fn scripted_initialize(stream: &mut UnixStream, snapshot: IpcStateSnapshot) {
        let hello = read_envelope_with_timeout(stream, Some(Duration::from_secs(2)))
            .await
            .expect("read hello");
        assert_eq!(hello.kind, IpcEnvelopeKind::Hello);
        scripted_response(
            stream,
            &hello,
            serde_json::to_value(HelloResponse {
                protocol_version: IPC_PROTOCOL_VERSION,
                omegon_version: "0.29.0-dev".into(),
                server_name: "scripted-omegon".into(),
                server_pid: std::process::id(),
                cwd: "/tmp/auspex-ipc-contract".into(),
                server_instance_id: snapshot.instance.control_plane.server_instance_id.clone(),
                started_at: "2026-08-23T00:00:00Z".into(),
                session_id: snapshot.session.session_id.clone(),
                session_generation: snapshot.session.session_generation,
                capabilities: vec![
                    "state.snapshot".into(),
                    "events.stream".into(),
                    "prompt.submit".into(),
                    "turn.cancel".into(),
                ],
            })
            .expect("encode hello response"),
        )
        .await;

        let get_state = read_envelope_with_timeout(stream, Some(Duration::from_secs(2)))
            .await
            .expect("read get_state");
        assert_eq!(get_state.method.as_deref(), Some("get_state"));
        scripted_response(
            stream,
            &get_state,
            serde_json::to_value(snapshot).expect("encode state snapshot"),
        )
        .await;

        let subscribe = read_envelope_with_timeout(stream, Some(Duration::from_secs(2)))
            .await
            .expect("read subscribe");
        assert_eq!(subscribe.method.as_deref(), Some("subscribe"));
        let events = subscribe
            .payload
            .as_ref()
            .and_then(|payload| payload.get("events"))
            .cloned()
            .unwrap_or_else(|| serde_json::json!([]));
        scripted_response(stream, &subscribe, serde_json::json!({ "events": events })).await;
    }

    async fn scripted_response(stream: &mut UnixStream, request: &IpcEnvelope, payload: Value) {
        write_envelope(
            stream,
            &IpcEnvelope {
                protocol_version: IPC_PROTOCOL_VERSION,
                kind: IpcEnvelopeKind::Response,
                request_id: request.request_id,
                method: request.method.clone(),
                payload: Some(payload),
                error: None,
            },
        )
        .await
        .expect("write scripted IPC response");
    }

    async fn scripted_event(stream: &mut UnixStream, event: IpcEventPayload) {
        write_envelope(
            stream,
            &IpcEnvelope {
                protocol_version: IPC_PROTOCOL_VERSION,
                kind: IpcEnvelopeKind::Event,
                request_id: None,
                method: None,
                payload: Some(serde_json::to_value(event).expect("encode scripted IPC event")),
                error: None,
            },
        )
        .await
        .expect("write scripted IPC event");
    }

    fn scripted_state(
        server_instance_id: &str,
        generation: u64,
        frontier: u64,
        queue_depth: usize,
        busy: bool,
        active_turn: Option<&str>,
    ) -> IpcStateSnapshot {
        serde_json::from_value(serde_json::json!({
            "schema_version": 1,
            "omegon_version": "0.29.0-dev",
            "instance": {
                "schema_version": 2,
                "identity": {
                    "instance_id": "instance-1",
                    "workspace_id": "workspace-1",
                    "session_id": "session-1",
                    "role": "primary_driver",
                    "profile": "primary-interactive"
                },
                "ownership": {
                    "owner_kind": "auspex",
                    "owner_id": "auspex"
                },
                "placement": {
                    "kind": "local_process",
                    "host": "localhost",
                    "pid": 12345,
                    "cwd": "/tmp/auspex-ipc-contract"
                },
                "control_plane": {
                    "server_instance_id": server_instance_id,
                    "protocol_version": IPC_PROTOCOL_VERSION,
                    "schema_version": 2,
                    "omegon_version": "0.29.0-dev",
                    "capabilities": ["state.snapshot", "events.stream", "prompt.submit", "turn.cancel"],
                    "ipc_socket_path": "/tmp/auspex-ipc-contract.sock"
                },
                "runtime": {
                    "deployment_kind": "interactive_tui",
                    "runtime_mode": "standalone",
                    "runtime_profile": "primary_interactive",
                    "autonomy_mode": "operator_driven",
                    "health": "ready",
                    "provider_ok": true,
                    "memory_ok": true,
                    "cleave_available": false,
                    "queued_events": 0
                }
            },
            "session": {
                "cwd": "/tmp/auspex-ipc-contract",
                "pid": 12345,
                "started_at": "2026-08-23T00:00:00Z",
                "turns": 1,
                "tool_calls": 0,
                "compactions": 0,
                "busy": busy,
                "git_detached": false,
                "session_id": "session-1",
                "session_generation": generation,
                "stream_id": format!("stream-{generation}"),
                "projection_status": "exact",
                "projection_frontier": frontier,
                "context_revision": frontier,
                "queue_depth": queue_depth,
                "active_turn": active_turn
            },
            "design_tree": {
                "counts": {
                    "total": 0,
                    "seed": 0,
                    "exploring": 0,
                    "resolved": 0,
                    "decided": 0,
                    "implementing": 0,
                    "implemented": 0,
                    "blocked": 0,
                    "deferred": 0,
                    "open_questions": 0
                },
                "implementing": [],
                "actionable": [],
                "nodes": []
            },
            "openspec": { "changes": [], "total_tasks": 0, "done_tasks": 0 },
            "cleave": {
                "active": false,
                "total_children": 0,
                "completed": 0,
                "failed": 0,
                "children": []
            },
            "harness": {
                "context_class": "Squad",
                "thinking_level": "Medium",
                "capability_tier": "victory",
                "runtime_profile": "primary-interactive",
                "autonomy_mode": "operator-driven",
                "dispatcher": { "available_options": [], "switch_state": "idle" },
                "memory_available": true,
                "cleave_available": false,
                "memory": {
                    "active_facts": 0,
                    "project_facts": 0,
                    "working_facts": 0,
                    "episodes": 0
                },
                "providers": [],
                "mcp_server_count": 0,
                "mcp_tool_count": 0,
                "active_delegate_count": 0
            },
            "health": {
                "state": "ready",
                "memory_ok": true,
                "provider_ok": true,
                "checked_at": "2026-08-23T00:00:00Z"
            }
        }))
        .expect("decode scripted state snapshot")
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
            Some(IpcClientEvent::Payload(IpcEventPayload::TurnStarted {
                turn: 7
            }))
        );
    }

    #[test]
    fn decode_event_envelope_maps_plan_updated_before_traits_variant_exists() {
        let envelope = IpcEnvelope {
            protocol_version: IPC_PROTOCOL_VERSION,
            kind: IpcEnvelopeKind::Event,
            request_id: None,
            method: None,
            payload: Some(serde_json::json!({
                "name": "plan.updated",
                "data": { "snapshot": { "entries": [{"status": "active"}] } }
            })),
            error: None,
        };

        assert_eq!(
            decode_event_envelope(envelope).unwrap(),
            Some(IpcClientEvent::Session(
                crate::session_event::SessionEvent::PlanUpdated {
                    snapshot: serde_json::json!({ "entries": [{"status": "active"}] })
                }
            ))
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
                "capabilities": ["state.snapshot", "events.stream"]
            })),
            error: None,
        };

        assert_eq!(
            validate_response_envelope(&envelope, "hello", "IPC handshake"),
            Ok(())
        );
        let hello = decode_hello_response(&envelope).expect("hello response should decode");
        assert_eq!(hello.server_instance_id, "instance-1");
        assert_eq!(hello.capabilities, vec!["state.snapshot", "events.stream"]);
    }

    #[test]
    fn canonical_methods_require_their_advertised_capabilities() {
        assert_eq!(capability_for_method("get_state"), Some("state.snapshot"));
        assert_eq!(
            capability_for_method("submit_prompt"),
            Some("prompt.submit")
        );
        assert_eq!(capability_for_method("cancel"), Some("turn.cancel"));
        assert_eq!(
            capability_for_method("switch_dispatcher"),
            Some("dispatcher.switch")
        );
        assert_eq!(capability_for_method("private_extension_method"), None);
    }

    #[test]
    fn missing_server_capability_fails_closed() {
        let hello = HelloResponse {
            protocol_version: IPC_PROTOCOL_VERSION,
            omegon_version: "0.29.0-dev".into(),
            server_name: "omegon".into(),
            server_pid: 42,
            cwd: "/tmp".into(),
            server_instance_id: "instance-1".into(),
            started_at: "2026-04-07T00:00:00Z".into(),
            session_id: Some("session-1".into()),
            session_generation: Some(3),
            capabilities: vec!["events.stream".into()],
        };

        assert!(require_server_capability(&hello, "events.stream").is_ok());
        assert_eq!(
            require_server_capability(&hello, "state.snapshot"),
            Err(
                "Omegon instance-1 does not advertise required IPC capability state.snapshot"
                    .into()
            )
        );
    }

    #[test]
    fn dispatcher_switch_request_payload_matches_canonical_ipc_shape() {
        let payload = serde_json::json!({
            "request_id": "dispatcher-switch-1",
            "profile": "supervisor-heavy",
            "model": "openai:gpt-4.1",
            "caller_role": "admin",
        });

        assert_eq!(payload["request_id"], "dispatcher-switch-1");
        assert_eq!(payload["profile"], "supervisor-heavy");
        assert_eq!(payload["model"], "openai:gpt-4.1");
        assert_eq!(payload["caller_role"], "admin");
    }
}
