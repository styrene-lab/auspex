use std::sync::{Arc, Mutex};
use std::thread;

use reqwest::Url;
use tungstenite::Message;

pub const WS_URL_ENV: &str = "AUSPEX_OMEGON_WS_URL";
pub const WS_TOKEN_ENV: &str = "AUSPEX_OMEGON_WS_TOKEN";

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

#[derive(Clone, Debug)]
pub enum EventStreamSource {
    WebSocket { url: String },
}

#[derive(Clone, Debug)]
pub struct EventStreamHandle {
    pub inbox: EventInbox,
    pub source: EventStreamSource,
}

impl EventStreamHandle {
    pub fn websocket(url: impl Into<String>) -> Self {
        Self {
            inbox: EventInbox::default(),
            source: EventStreamSource::WebSocket { url: url.into() },
        }
    }

    pub fn url(&self) -> &str {
        match &self.source {
            EventStreamSource::WebSocket { url } => url,
        }
    }

    pub fn send_command(&self, command_json: String) {
        let command_url = self.url().to_string();
        let feedback = self.clone();

        thread::spawn(move || match tungstenite::connect(command_url.as_str()) {
            Ok((mut socket, _response)) => {
                if let Err(error) = socket.send(Message::Text(command_json.into())) {
                    feedback.push_system_notice(format!(
                        "Could not send command to Omegon event stream: {error}"
                    ));
                }
                let _ = socket.close(None);
            }
            Err(error) => feedback.push_system_notice(format!(
                "Could not open command channel to Omegon event stream at {}: {}",
                feedback.url(), error
            )),
        });
    }
}

pub fn derive_ws_url_from_state_url(state_url: &str) -> Result<String, String> {
    let mut url = Url::parse(state_url)
        .map_err(|error| format!("invalid state URL: {error}"))?;

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

    let mut parsed = Url::parse(url)
        .map_err(|error| format!("invalid websocket URL: {error}"))?;
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

pub fn spawn_websocket_event_stream(url: &str) -> EventStreamHandle {
    let handle = EventStreamHandle::websocket(url);
    let worker_handle = handle.clone();
    let url = url.to_string();

    thread::spawn(move || match tungstenite::connect(url.as_str()) {
        Ok((mut socket, _response)) => {
            worker_handle.push_system_notice(format!(
                "Connected to Omegon event stream at {}",
                worker_handle.url()
            ));

            loop {
                match socket.read() {
                    Ok(Message::Text(text)) => worker_handle.inbox.push(text.to_string()),
                    Ok(Message::Binary(_)) => worker_handle.push_system_notice(
                        "Ignoring binary websocket frame from Omegon event stream",
                    ),
                    Ok(Message::Close(_)) => {
                        worker_handle.push_system_notice(
                            "Omegon event stream closed. Live updates are paused.",
                        );
                        break;
                    }
                    Ok(Message::Ping(_)) | Ok(Message::Pong(_)) | Ok(Message::Frame(_)) => {}
                    Err(error) => {
                        worker_handle.push_system_notice(format!(
                            "Omegon event stream disconnected: {error}"
                        ));
                        break;
                    }
                }
            }
        }
        Err(error) => worker_handle.push_system_notice(format!(
            "Could not attach to Omegon event stream at {}: {}",
            worker_handle.url(), error
        )),
    });

    handle
}

impl EventStreamHandle {
    fn push_system_notice(&self, message: impl Into<String>) {
        let payload = serde_json::json!({
            "type": "system_notification",
            "message": message.into(),
        });
        self.inbox.push(payload.to_string());
    }
}

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
        let ws_url = derive_ws_url_from_state_url("https://example.test/api/state?token=abc").unwrap();
        assert_eq!(ws_url, "wss://example.test/ws");
    }

    #[test]
    fn apply_ws_auth_token_adds_token_query() {
        let ws_url = apply_ws_auth_token("ws://127.0.0.1:7842/ws", Some("secret-token")).unwrap();
        assert_eq!(ws_url, "ws://127.0.0.1:7842/ws?token=secret-token");
    }

    #[test]
    fn apply_ws_auth_token_preserves_existing_token() {
        let ws_url = apply_ws_auth_token("ws://127.0.0.1:7842/ws?token=existing", Some("ignored")).unwrap();
        assert_eq!(ws_url, "ws://127.0.0.1:7842/ws?token=existing");
    }

    #[test]
    fn derive_authenticated_ws_url_rewrites_and_authenticates() {
        let ws_url = derive_authenticated_ws_url("http://127.0.0.1:7842/api/state", Some("abc123")).unwrap();
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
    fn send_command_failure_is_reported_as_system_notice() {
        let handle = EventStreamHandle::websocket("ws://127.0.0.1:1/ws");
        handle.send_command(r#"{"type":"user_prompt","text":"hello"}"#.to_string());
        std::thread::sleep(std::time::Duration::from_millis(50));

        assert!(handle
            .inbox
            .drain()
            .into_iter()
            .any(|event| event.contains("Could not open command channel")));
    }
}
