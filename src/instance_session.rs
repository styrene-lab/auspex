//! Per-instance session management for multi-agent COP.
//!
//! Each remote omegon instance gets its own `InstanceSession` — a bundle of
//! WebSocket event stream + RemoteHostSession. The `InstanceSessionMap` holds
//! all of them and provides drain/focus operations for the app loop.

use std::collections::HashMap;

use crate::event_stream::{EventStreamHandle, spawn_websocket_event_stream};
use crate::fixtures::{ChatMessage, TranscriptData};
use crate::omegon_control::{OmegonEvent, OmegonStateSnapshot};
use crate::remote_session::RemoteHostSession;
use crate::runtime_types::TargetedCommand;
use crate::session_event::SessionEvent;
use crate::session_model::HostSessionModel;

// ── ActivitySummary ────────────────────────────────────────────────────────

/// Lightweight status for non-focused instances (deployment widget badges).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ActivitySummary {
    pub run_active: bool,
    pub turn_count: u32,
    pub tool_call_count: u32,
    pub last_activity: Option<String>,
}

// ── InstanceSession ────────────────────────────────────────────────────────

/// A self-contained session for one remote omegon instance.
#[derive(Clone, Debug)]
pub struct InstanceSession {
    pub instance_id: String,
    pub ws_url: String,
    session: RemoteHostSession,
    event_stream: EventStreamHandle,
    pub activity: ActivitySummary,
}

impl InstanceSession {
    /// Create a new instance session. Spawns a WebSocket event stream and
    /// initializes an empty session (will be populated by the first
    /// state_snapshot event from the remote instance).
    pub fn connect(instance_id: impl Into<String>, ws_url: impl Into<String>) -> Self {
        let ws_url = ws_url.into();
        let event_stream = spawn_websocket_event_stream(&ws_url);
        let session = RemoteHostSession::from_snapshot(OmegonStateSnapshot::default());

        Self {
            instance_id: instance_id.into(),
            ws_url,
            session,
            event_stream,
            activity: ActivitySummary::default(),
        }
    }

    /// Create an instance session with a pre-built event stream handle.
    /// Useful for testing with mock inboxes.
    #[cfg(test)]
    pub fn with_handle(
        instance_id: impl Into<String>,
        ws_url: impl Into<String>,
        event_stream: EventStreamHandle,
    ) -> Self {
        Self {
            instance_id: instance_id.into(),
            ws_url: ws_url.into(),
            session: RemoteHostSession::from_snapshot(OmegonStateSnapshot::default()),
            event_stream,
            activity: ActivitySummary::default(),
        }
    }

    /// Drain the event stream inbox and apply all events to the session.
    /// Updates the activity summary. Returns true if any events were applied.
    pub fn drain_and_apply(&mut self) -> bool {
        let events = self.event_stream.inbox.drain();
        if events.is_empty() {
            return false;
        }

        for event_json in &events {
            match serde_json::from_str::<OmegonEvent>(event_json) {
                Ok(event) => {
                    let session_event = SessionEvent::from(event.clone());
                    self.update_activity(&session_event);
                    self.session.apply_event(event);
                }
                Err(error) => {
                    eprintln!(
                        "auspex: instance {}: failed to parse event: {error}",
                        self.instance_id
                    );
                }
            }
        }

        true
    }

    /// Update the activity summary from a session event.
    fn update_activity(&mut self, event: &SessionEvent) {
        match event {
            SessionEvent::TurnStarted { .. } => {
                self.activity.run_active = true;
                self.activity.turn_count += 1;
                self.activity.last_activity = Some("turn started".into());
            }
            SessionEvent::TurnEnded { .. } => {
                self.activity.run_active = false;
                self.activity.last_activity = Some("turn ended".into());
            }
            SessionEvent::ToolStarted { name, .. } => {
                self.activity.tool_call_count += 1;
                self.activity.last_activity = Some(format!("tool: {name}"));
            }
            SessionEvent::AgentCompleted => {
                self.activity.run_active = false;
                self.activity.last_activity = Some("agent completed".into());
            }
            SessionEvent::MessageAbort => {
                self.activity.run_active = false;
                self.activity.last_activity = Some("message aborted".into());
            }
            _ => {}
        }
    }

    /// Send a command to this instance via its WebSocket.
    pub fn send_command(&self, command: &TargetedCommand) {
        self.event_stream.send_targeted_command(command);
    }

    /// Access the session's transcript.
    pub fn transcript(&self) -> &TranscriptData {
        self.session.transcript()
    }

    /// Access the session's chat messages.
    pub fn messages(&self) -> &[ChatMessage] {
        self.session.messages()
    }

    /// Whether the session has an active turn.
    pub fn is_run_active(&self) -> bool {
        self.session.is_run_active()
    }

    /// Build session data for the right panel.
    pub fn session_data(&self) -> crate::fixtures::SessionData {
        self.session.session_data()
    }

    /// Access the session's summary.
    pub fn summary(&self) -> &crate::fixtures::HostSessionSummary {
        self.session.summary()
    }
}

// Manual PartialEq — compare session state, skip event stream handle internals.
impl PartialEq for InstanceSession {
    fn eq(&self, other: &Self) -> bool {
        self.instance_id == other.instance_id
            && self.ws_url == other.ws_url
            && self.session == other.session
            && self.activity == other.activity
    }
}

impl Eq for InstanceSession {}

// ── InstanceSessionMap ─────────────────────────────────────────────────────

/// Manages all per-instance sessions.
#[derive(Clone, Debug, Default)]
pub struct InstanceSessionMap {
    sessions: HashMap<String, InstanceSession>,
}

impl InstanceSessionMap {
    /// Connect to a remote instance. Spawns a WebSocket and creates
    /// an empty session that will be populated on first state_snapshot.
    pub fn connect(&mut self, instance_id: impl Into<String>, ws_url: impl Into<String>) {
        let instance_id = instance_id.into();
        let session = InstanceSession::connect(instance_id.clone(), ws_url);
        self.sessions.insert(instance_id, session);
    }

    /// Disconnect from a remote instance. Cancels the WebSocket task
    /// and drops the session.
    pub fn disconnect(&mut self, instance_id: &str) {
        if let Some(session) = self.sessions.remove(instance_id) {
            session.event_stream.cancel();
        }
    }

    /// Check if an instance has a connected session.
    pub fn is_connected(&self, instance_id: &str) -> bool {
        self.sessions.contains_key(instance_id)
    }

    /// Drain all instance inboxes and apply events. Returns true if any
    /// instance had events.
    pub fn drain_all(&mut self) -> bool {
        let mut any = false;
        for session in self.sessions.values_mut() {
            any |= session.drain_and_apply();
        }
        any
    }

    /// Look up a session by instance_id.
    pub fn get(&self, instance_id: &str) -> Option<&InstanceSession> {
        self.sessions.get(instance_id)
    }

    /// Look up a session mutably.
    pub fn get_mut(&mut self, instance_id: &str) -> Option<&mut InstanceSession> {
        self.sessions.get_mut(instance_id)
    }

    /// Number of connected instance sessions.
    pub fn len(&self) -> usize {
        self.sessions.len()
    }

    /// Whether the map is empty.
    pub fn is_empty(&self) -> bool {
        self.sessions.is_empty()
    }

    /// Iterate over all sessions.
    pub fn iter(&self) -> impl Iterator<Item = (&str, &InstanceSession)> {
        self.sessions.iter().map(|(k, v)| (k.as_str(), v))
    }

    /// Collect activity summaries for all instances.
    pub fn activity_summaries(&self) -> Vec<(String, ActivitySummary)> {
        self.sessions
            .iter()
            .map(|(id, session)| (id.clone(), session.activity.clone()))
            .collect()
    }
}

// Manual PartialEq — compare session map contents, skip event stream internals.
impl PartialEq for InstanceSessionMap {
    fn eq(&self, other: &Self) -> bool {
        if self.sessions.len() != other.sessions.len() {
            return false;
        }
        self.sessions.iter().all(|(k, v)| {
            other.sessions.get(k).is_some_and(|other_v| v == other_v)
        })
    }
}

impl Eq for InstanceSessionMap {}

#[cfg(test)]
mod tests {
    use super::*;

    fn mock_handle(url: &str) -> EventStreamHandle {
        EventStreamHandle::websocket(url)
    }

    #[test]
    fn instance_session_starts_empty() {
        let session = InstanceSession::with_handle(
            "test-instance",
            "ws://localhost:7843/ws",
            mock_handle("ws://localhost:7843/ws"),
        );

        assert_eq!(session.instance_id, "test-instance");
        assert!(!session.is_run_active());
        assert_eq!(session.activity.turn_count, 0);
        assert!(session.transcript().turns.is_empty());
    }

    #[test]
    fn drain_and_apply_processes_events() {
        let handle = mock_handle("ws://localhost:7843/ws");
        let mut session = InstanceSession::with_handle(
            "test-instance",
            "ws://localhost:7843/ws",
            handle.clone(),
        );

        // Inject a turn_start event.
        handle.inbox.push(
            serde_json::json!({"type": "turn_start", "turn": 1}).to_string(),
        );

        assert!(session.drain_and_apply());
        assert!(session.is_run_active());
        assert_eq!(session.activity.turn_count, 1);
        assert!(session.activity.run_active);
    }

    #[test]
    fn drain_and_apply_returns_false_when_empty() {
        let handle = mock_handle("ws://localhost:7843/ws");
        let mut session = InstanceSession::with_handle(
            "test-instance",
            "ws://localhost:7843/ws",
            handle,
        );

        assert!(!session.drain_and_apply());
    }

    #[test]
    fn activity_summary_tracks_tools() {
        let handle = mock_handle("ws://localhost:7843/ws");
        let mut session = InstanceSession::with_handle(
            "test",
            "ws://localhost:7843/ws",
            handle.clone(),
        );

        handle.inbox.push(
            serde_json::json!({"type": "tool_start", "id": "t1", "name": "read", "args": {}}).to_string(),
        );
        session.drain_and_apply();

        assert_eq!(session.activity.tool_call_count, 1);
        assert_eq!(session.activity.last_activity.as_deref(), Some("tool: read"));
    }

    #[test]
    fn instance_session_map_connect_disconnect() {
        let mut map = InstanceSessionMap::default();
        assert!(map.is_empty());

        // Use with_handle to avoid tokio runtime requirement in tests.
        map.sessions.insert(
            "agent-1".into(),
            InstanceSession::with_handle("agent-1", "ws://host1:7843/ws", mock_handle("ws://host1:7843/ws")),
        );
        map.sessions.insert(
            "agent-2".into(),
            InstanceSession::with_handle("agent-2", "ws://host2:7843/ws", mock_handle("ws://host2:7843/ws")),
        );
        assert_eq!(map.len(), 2);
        assert!(map.is_connected("agent-1"));
        assert!(map.is_connected("agent-2"));

        map.disconnect("agent-1");
        assert_eq!(map.len(), 1);
        assert!(!map.is_connected("agent-1"));
        assert!(map.is_connected("agent-2"));
    }

    #[test]
    fn drain_all_drains_multiple_sessions() {
        let mut map = InstanceSessionMap::default();

        let handle1 = mock_handle("ws://host1:7843/ws");
        let handle2 = mock_handle("ws://host2:7843/ws");

        map.sessions.insert(
            "agent-1".into(),
            InstanceSession::with_handle("agent-1", "ws://host1:7843/ws", handle1.clone()),
        );
        map.sessions.insert(
            "agent-2".into(),
            InstanceSession::with_handle("agent-2", "ws://host2:7843/ws", handle2.clone()),
        );

        // Only agent-1 has events.
        handle1.inbox.push(
            serde_json::json!({"type": "turn_start", "turn": 1}).to_string(),
        );

        assert!(map.drain_all());

        let s1 = map.get("agent-1").unwrap();
        assert!(s1.is_run_active());
        assert_eq!(s1.activity.turn_count, 1);

        let s2 = map.get("agent-2").unwrap();
        assert!(!s2.is_run_active());
        assert_eq!(s2.activity.turn_count, 0);
    }

    #[test]
    fn activity_summaries_returns_all_instances() {
        let mut map = InstanceSessionMap::default();
        map.sessions.insert(
            "agent-1".into(),
            InstanceSession::with_handle("agent-1", "ws://host1:7843/ws", mock_handle("ws://host1:7843/ws")),
        );
        map.sessions.insert(
            "agent-2".into(),
            InstanceSession::with_handle("agent-2", "ws://host2:7843/ws", mock_handle("ws://host2:7843/ws")),
        );

        let summaries = map.activity_summaries();
        assert_eq!(summaries.len(), 2);
    }

    #[test]
    fn send_command_queues_to_outbox() {
        let handle = mock_handle("ws://localhost:7843/ws");
        let session = InstanceSession::with_handle(
            "test",
            "ws://localhost:7843/ws",
            handle.clone(),
        );

        let command = TargetedCommand::prompt_submit(
            crate::runtime_types::CommandTarget {
                session_key: "test-session".into(),
                dispatcher_instance_id: None,
            },
            "hello",
        );
        session.send_command(&command);

        let outbox = handle.debug_drain_outbox();
        assert_eq!(outbox.len(), 1);
        assert!(outbox[0].contains("hello"));
    }
}
