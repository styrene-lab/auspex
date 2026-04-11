use std::sync::{Arc, Mutex};
use std::time::Duration;

use crate::runtime_types::TargetedCommand;
use reqwest::Url;

#[derive(Clone, Debug, Default)]
pub struct EventInbox {
    queue: Arc<Mutex<Vec<String>>>,
}

impl EventInbox {
    pub fn push(&self, event_json: impl Into<String>) {
        if let Ok(mut queue) = self.queue.lock() {
            queue.push(event_json.into());
        }
    }

    pub fn drain(&self) -> Vec<String> {
        if let Ok(mut queue) = self.queue.lock() {
            return std::mem::take(&mut *queue);
        }

        Vec::new()
    }
}

#[derive(Clone, Debug, Default)]
pub struct CommandOutbox {
    queue: Arc<Mutex<Vec<String>>>,
}

impl CommandOutbox {
    pub fn push_raw(&self, command_json: impl Into<String>) {
        if let Ok(mut queue) = self.queue.lock() {
            queue.push(command_json.into());
        }
    }

    pub fn drain(&self) -> Vec<String> {
        if let Ok(mut queue) = self.queue.lock() {
            return std::mem::take(&mut *queue);
        }
        Vec::new()
    }
}

#[derive(Clone, Debug)]
pub enum EventStreamSource {
    WebSocket { url: String },
}

#[derive(Clone, Debug)]
pub struct EventStreamHandle {
    pub inbox: EventInbox,
    pub source: EventStreamSource,
    outbox: CommandOutbox,
}

impl EventStreamHandle {
    pub fn websocket(url: impl Into<String>) -> Self {
        Self {
            inbox: EventInbox::default(),
            source: EventStreamSource::WebSocket { url: url.into() },
            outbox: CommandOutbox::default(),
        }
    }

    pub fn url(&self) -> &str {
        match &self.source {
            EventStreamSource::WebSocket { url } => url,
        }
    }

    pub fn send_targeted_command(&self, command: &TargetedCommand) {
        self.outbox.push_raw(command.web_command_json());
    }

    #[cfg(test)]
    pub fn debug_drain_outbox(&self) -> Vec<String> {
        self.outbox.drain()
    }

    fn push_system_notice(&self, message: impl Into<String>) {
        let payload = serde_json::json!({
            "type": "system_notification",
            "message": message.into(),
        });
        self.inbox.push(payload.to_string());
    }
}

// ── URL helpers (shared across all platforms) ─────────────────────────────────

pub fn derive_ws_url_from_state_url(state_url: &str) -> Result<String, String> {
    let mut url = Url::parse(state_url).map_err(|error| format!("invalid state URL: {error}"))?;

    let ws_scheme = match url.scheme() {
        "http" => "ws",
        "https" => "wss",
        other => return Err(format!("unsupported state URL scheme {other}")),
    };

    url.set_scheme(ws_scheme)
        .map_err(|_| "could not set websocket URL scheme".to_string())?;
    url.set_path("/ws");
    url.set_query(None);
    url.set_fragment(None);

    Ok(url.to_string())
}

pub fn apply_ws_auth_token(url: &str, token: Option<&str>) -> Result<String, String> {
    let Some(token) = token.map(str::trim).filter(|token| !token.is_empty()) else {
        return Ok(url.to_string());
    };

    let mut parsed = Url::parse(url).map_err(|error| format!("invalid websocket URL: {error}"))?;
    let has_token = parsed.query_pairs().any(|(key, _)| key == "token");
    if !has_token {
        parsed.query_pairs_mut().append_pair("token", token);
    }

    Ok(parsed.to_string())
}

pub fn derive_authenticated_ws_url(state_url: &str, token: Option<&str>) -> Result<String, String> {
    let ws_url = derive_ws_url_from_state_url(state_url)?;
    apply_ws_auth_token(&ws_url, token)
}

// ── Desktop: tokio-tungstenite WebSocket ──────────────────────────────────────

#[cfg(not(target_arch = "wasm32"))]
pub fn spawn_websocket_event_stream(url: &str) -> EventStreamHandle {
    use futures_util::{SinkExt, StreamExt};
    use tungstenite::Message;

    let handle = EventStreamHandle::websocket(url);
    let worker_handle = handle.clone();
    let url = url.to_string();

    tokio::spawn(async move {
        let mut backoff = Duration::from_secs(1);
        const MAX_BACKOFF: Duration = Duration::from_secs(30);
        let mut first_attempt = true;

        loop {
            if !first_attempt {
                worker_handle.push_system_notice(format!(
                    "Reconnecting to Omegon event stream in {}s\u{2026}",
                    backoff.as_secs()
                ));
                tokio::time::sleep(backoff).await;
                backoff = (backoff * 2).min(MAX_BACKOFF);
            }
            first_attempt = false;

            let connect_result = tokio_tungstenite::connect_async(&url).await;

            match connect_result {
                Err(error) => {
                    worker_handle.push_system_notice(format!(
                        "Could not connect to Omegon event stream at {}: {error}",
                        worker_handle.url()
                    ));
                }
                Ok((ws_stream, _response)) => {
                    backoff = Duration::from_secs(1);
                    worker_handle.push_system_notice(format!(
                        "Connected to Omegon event stream at {}",
                        worker_handle.url()
                    ));

                    let (mut sink, mut stream) = ws_stream.split();

                    loop {
                        for cmd in worker_handle.outbox.drain() {
                            if let Err(error) = sink.send(Message::Text(cmd.into())).await {
                                worker_handle.push_system_notice(format!(
                                    "Failed to send command to Omegon: {error}"
                                ));
                            }
                        }

                        let read_result =
                            tokio::time::timeout(Duration::from_millis(50), stream.next()).await;

                        match read_result {
                            Ok(Some(Ok(Message::Text(text)))) => {
                                worker_handle.inbox.push(text.to_string());
                            }
                            Ok(Some(Ok(Message::Binary(_)))) => {
                                worker_handle.push_system_notice(
                                    "Ignoring binary WebSocket frame from Omegon event stream",
                                );
                            }
                            Ok(Some(Ok(Message::Close(_)))) => {
                                worker_handle.push_system_notice(
                                    "Omegon event stream closed by server. Will reconnect.",
                                );
                                break;
                            }
                            Ok(Some(Ok(
                                Message::Ping(_) | Message::Pong(_) | Message::Frame(_),
                            ))) => {}
                            Ok(Some(Err(error))) => {
                                worker_handle.push_system_notice(format!(
                                    "Omegon event stream error: {error}. Will reconnect."
                                ));
                                break;
                            }
                            Ok(None) => {
                                worker_handle.push_system_notice(
                                    "Omegon event stream ended. Will reconnect.",
                                );
                                break;
                            }
                            Err(_) => {
                                // Timeout — no data ready, loop back to check outbox.
                            }
                        }
                    }
                }
            }
        }
    });

    handle
}

// ── Web: web-sys WebSocket ────────────────────────────────────────────────────

#[cfg(target_arch = "wasm32")]
pub fn spawn_websocket_event_stream(url: &str) -> EventStreamHandle {
    use wasm_bindgen::prelude::*;
    use web_sys::{MessageEvent, WebSocket};

    let handle = EventStreamHandle::websocket(url);
    let worker_handle = handle.clone();

    let ws = WebSocket::new(url).expect("failed to create WebSocket");
    ws.set_binary_type(web_sys::BinaryType::Arraybuffer);

    // onmessage — push text frames into the inbox.
    let inbox = worker_handle.inbox.clone();
    let onmessage = Closure::<dyn FnMut(MessageEvent)>::new(move |event: MessageEvent| {
        if let Ok(text) = event.data().dyn_into::<js_sys::JsString>() {
            inbox.push(String::from(text));
        }
    });
    ws.set_onmessage(Some(onmessage.as_ref().unchecked_ref()));
    onmessage.forget();

    // onclose — push a system notice.
    let notice_handle = worker_handle.clone();
    let onclose = Closure::<dyn FnMut()>::new(move || {
        notice_handle.push_system_notice("Omegon event stream closed by server.");
    });
    ws.set_onclose(Some(onclose.as_ref().unchecked_ref()));
    onclose.forget();

    // onerror — push a system notice.
    let notice_handle = worker_handle.clone();
    let onerror = Closure::<dyn FnMut()>::new(move || {
        notice_handle.push_system_notice("Omegon event stream error.");
    });
    ws.set_onerror(Some(onerror.as_ref().unchecked_ref()));
    onerror.forget();

    // Command sender — poll outbox on an interval and send via the WebSocket.
    let outbox = worker_handle.outbox.clone();
    let ws_clone = ws.clone();
    wasm_bindgen_futures::spawn_local(async move {
        loop {
            gloo_timers::future::TimeoutFuture::new(50).await;
            for cmd in outbox.drain() {
                let _ = ws_clone.send_with_str(&cmd);
            }
        }
    });

    handle
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn derive_ws_url_rewrites_default_state_endpoint() {
        let ws_url = derive_ws_url_from_state_url("http://127.0.0.1:7842/api/state").unwrap();
        assert_eq!(ws_url, "ws://127.0.0.1:7842/ws");
    }

    #[test]
    fn derive_ws_url_rewrites_https_to_wss() {
        let ws_url =
            derive_ws_url_from_state_url("https://example.test/api/state?token=abc").unwrap();
        assert_eq!(ws_url, "wss://example.test/ws");
    }

    #[test]
    fn apply_ws_auth_token_adds_token_query() {
        let ws_url = apply_ws_auth_token("ws://127.0.0.1:7842/ws", Some("secret-token")).unwrap();
        assert_eq!(ws_url, "ws://127.0.0.1:7842/ws?token=secret-token");
    }

    #[test]
    fn apply_ws_auth_token_preserves_existing_token() {
        let ws_url =
            apply_ws_auth_token("ws://127.0.0.1:7842/ws?token=existing", Some("ignored")).unwrap();
        assert_eq!(ws_url, "ws://127.0.0.1:7842/ws?token=existing");
    }

    #[test]
    fn derive_authenticated_ws_url_rewrites_and_authenticates() {
        let ws_url =
            derive_authenticated_ws_url("http://127.0.0.1:7842/api/state", Some("abc123")).unwrap();
        assert_eq!(ws_url, "ws://127.0.0.1:7842/ws?token=abc123");
    }

    #[test]
    fn event_inbox_drains_fifo_payloads() {
        let inbox = EventInbox::default();
        inbox.push(r#"{"type":"message_start","role":"assistant"}"#);
        inbox.push(r#"{"type":"message_end"}"#);

        assert_eq!(
            inbox.drain(),
            vec![
                r#"{"type":"message_start","role":"assistant"}"#.to_string(),
                r#"{"type":"message_end"}"#.to_string(),
            ]
        );
        assert!(inbox.drain().is_empty());
    }

    #[test]
    fn send_targeted_command_queues_web_command_json() {
        let handle = EventStreamHandle::websocket("ws://127.0.0.1:1/ws");
        let command = TargetedCommand::prompt_submit(
            crate::runtime_types::CommandTarget {
                session_key: "remote:session_01HVDEMO".into(),
                dispatcher_instance_id: Some("omg_primary_01HVDEMO".into()),
            },
            "hello",
        );

        handle.send_targeted_command(&command);

        let commands = handle.debug_drain_outbox();
        assert_eq!(
            commands,
            vec![r#"{"text":"hello","type":"user_prompt"}"#.to_string()]
        );
    }
}
