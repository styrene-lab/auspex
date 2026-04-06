use crate::audit_timeline::{AuditTimelineQuery, AuditTimelineStore, AuditTimelineView};
use crate::fixtures::{
    AppSurfaceKind, AppSurfaceNotice, ChatMessage, ComposerState, DevScenario, GraphData,
    HostSessionSummary, MockHostSession, SessionData, ShellState, WorkData,
};
use crate::remote_session::{DispatcherSwitchCommandOutcome, RemoteHostSession};
use crate::runtime_types::{CommandTarget, TargetedCommand};
use crate::session_model::HostSessionModel;

const DEMO_REMOTE_SNAPSHOT_JSON: &str = r#"{
    "design": {
        "focused": {
            "id": "auspex-remote",
            "title": "Remote session adapter",
            "status": "implementing",
            "open_questions": ["How should reconnect work?"],
            "decisions": 1,
            "children": 2
        },
        "implementing": [{"id": "auspex-remote", "title": "Remote session adapter", "status": "implementing"}],
        "actionable": [{"id": "compat-handshake", "title": "Compatibility handshake", "status": "ready"}]
    },
    "openspec": {"total_tasks": 5, "done_tasks": 2},
    "cleave": {"active": false, "total_children": 0, "completed": 0, "failed": 0},
    "session": {"turns": 12, "tool_calls": 34, "compactions": 1},
    "dispatcher": {
        "session_id": "session_01HVDEMO",
        "dispatcher_instance_id": "omg_primary_01HVDEMO",
        "expected_role": "primary-driver",
        "expected_profile": "primary-interactive",
        "expected_model": "anthropic:claude-sonnet-4-6",
        "control_plane_schema": 2,
        "token_ref": "secret://auspex/instances/omg_primary_01HVDEMO/token",
        "observed_base_url": "http://127.0.0.1:7842",
        "last_verified_at": "2026-04-04T12:00:00Z",
        "available_options": [
            {"profile": "primary-interactive", "label": "Primary Interactive", "model": "anthropic:claude-sonnet-4-6"},
            {"profile": "supervisor-heavy", "label": "Supervisor Heavy", "model": "openai:gpt-4.1"}
        ],
        "switch_state": {
            "requested_profile": null,
            "requested_model": null,
            "status": "idle",
            "note": null
        }
    },
    "harness": {
        "git_branch": "main",
        "git_detached": false,
        "thinking_level": "medium",
        "capability_tier": "victory",
        "providers": [{"name": "Anthropic", "authenticated": true, "auth_method": "api-key", "model": "claude-sonnet"}],
        "memory_available": true,
        "cleave_available": true,
        "memory_warning": null,
        "active_delegates": []
    }
}"#;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SessionMode {
    Mock,
    Live,
}

impl SessionMode {
    pub const ALL: [Self; 2] = [Self::Live, Self::Mock];

    pub fn key(self) -> &'static str {
        match self {
            Self::Mock => "mock",
            Self::Live => "live",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Mock => "Mock (offline)",
            Self::Live => "Live",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum SessionSource {
    Mock(MockHostSession),
    Remote(Box<RemoteHostSession>),
}

impl Default for SessionSource {
    fn default() -> Self {
        Self::Mock(MockHostSession::default())
    }
}

impl SessionSource {
    fn model(&self) -> &dyn HostSessionModel {
        match self {
            Self::Mock(session) => session,
            Self::Remote(session) => session.as_ref(),
        }
    }

    fn model_mut(&mut self) -> &mut dyn HostSessionModel {
        match self {
            Self::Mock(session) => session,
            Self::Remote(session) => session.as_mut(),
        }
    }

    fn mode(&self) -> SessionMode {
        match self {
            Self::Mock(_) => SessionMode::Mock,
            Self::Remote(_) => SessionMode::Live,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AppController {
    session: SessionSource,
    bootstrap_note: Option<String>,
    transcript_auto_expand: bool,
    audit_timeline: AuditTimelineStore,
}

impl Default for AppController {
    fn default() -> Self {
        Self {
            session: SessionSource::default(),
            bootstrap_note: None,
            transcript_auto_expand: true,
            audit_timeline: AuditTimelineStore::default(),
        }
    }
}

impl AppController {
    pub fn from_remote_snapshot_json(json: &str) -> Result<Self, serde_json::Error> {
        let session = RemoteHostSession::from_snapshot_json(json)?;
        let mut controller = Self {
            session: SessionSource::Remote(Box::new(session)),
            bootstrap_note: None,
            transcript_auto_expand: true,
            audit_timeline: AuditTimelineStore::default(),
        };
        controller.refresh_audit_timeline();
        Ok(controller)
    }

    #[allow(dead_code)]
    pub fn remote_demo() -> Self {
        Self::from_remote_snapshot_json(DEMO_REMOTE_SNAPSHOT_JSON)
            .expect("embedded remote demo snapshot must stay valid")
    }

    pub fn with_audit_timeline(mut self, audit_timeline: AuditTimelineStore) -> Self {
        self.audit_timeline = audit_timeline;
        self.refresh_audit_timeline();
        self
    }

    pub fn session_mode(&self) -> SessionMode {
        self.session.mode()
    }

    pub fn surface_notice(&self) -> Option<AppSurfaceNotice> {
        match self.shell_state() {
            ShellState::StartingOmegon => Some(AppSurfaceNotice {
                kind: AppSurfaceKind::Startup,
                body: "Launching the Omegon engine. The conversation shell will activate automatically once ready.".into(),
                detail: self.bootstrap_note.clone(),
            }),
            ShellState::CompatibilityChecking => Some(AppSurfaceNotice {
                kind: AppSurfaceKind::Reconnecting,
                body: "The connection to the host is being restored. New input is temporarily paused. Cached session state is shown.".into(),
                detail: None,
            }),
            ShellState::Failed => Some(AppSurfaceNotice {
                kind: if self.scenario() == DevScenario::CompatibilityFailure {
                    AppSurfaceKind::CompatibilityFailure
                } else {
                    AppSurfaceKind::StartupFailure
                },
                body: self.summary().connection.clone(),
                detail: Some(
                    if self.scenario() == DevScenario::CompatibilityFailure {
                        self.bootstrap_note.clone().unwrap_or_else(|| {
                            "Auspex cannot operate with the detected host. Update Omegon to a compatible version and restart.".into()
                        })
                    } else {
                        self.bootstrap_note.clone().unwrap_or_else(|| {
                            "Auspex requires its embedded Omegon backend for local operation. Fix backend startup and relaunch, or explicitly attach to a remote Omegon control plane.".into()
                        })
                    }
                ),
            }),
            ShellState::Ready | ShellState::Degraded => self.bootstrap_note.clone().map(|body| {
                AppSurfaceNotice {
                    kind: AppSurfaceKind::BootstrapNote,
                    body,
                    detail: None,
                }
            }),
        }
    }

    pub fn set_bootstrap_note(&mut self, note: Option<String>) {
        self.bootstrap_note = note;
    }

    pub fn is_remote(&self) -> bool {
        self.session_mode() == SessionMode::Live
    }

    pub fn switch_session_mode(&mut self, raw: &str) {
        self.session = match raw {
            "live" => SessionSource::Remote(Box::new(
                RemoteHostSession::from_snapshot_json(DEMO_REMOTE_SNAPSHOT_JSON)
                    .expect("embedded remote demo snapshot must stay valid"),
            )),
            _ => SessionSource::Mock(MockHostSession::default()),
        };
        self.bootstrap_note = None;
        self.refresh_audit_timeline();
    }

    pub fn shell_state(&self) -> ShellState {
        self.session.model().shell_state()
    }

    pub fn scenario(&self) -> DevScenario {
        self.session.model().scenario()
    }

    pub fn summary(&self) -> &HostSessionSummary {
        self.session.model().summary()
    }

    pub fn messages(&self) -> &[ChatMessage] {
        self.session.model().messages()
    }

    pub fn composer(&self) -> &ComposerState {
        self.session.model().composer()
    }

    pub fn can_submit(&self) -> bool {
        self.session.model().can_submit()
    }

    pub fn is_run_active(&self) -> bool {
        self.session.model().is_run_active()
    }

    pub fn work_data(&self) -> WorkData {
        self.session.model().work_data()
    }

    pub fn session_data(&self) -> SessionData {
        self.session.model().session_data()
    }

    pub fn graph_data(&self) -> GraphData {
        self.session.model().graph_data()
    }

    pub fn transcript(&self) -> &crate::fixtures::TranscriptData {
        self.session.model().transcript()
    }

    pub fn transcript_auto_expand(&self) -> bool {
        self.transcript_auto_expand
    }

    pub fn audit_timeline(&self) -> &AuditTimelineStore {
        &self.audit_timeline
    }

    #[allow(dead_code)]
    pub fn query_audit_timeline(&self, query: &AuditTimelineQuery) -> AuditTimelineView<'_> {
        let mut view = self.audit_timeline.query(query);
        let current_session_key = self.session_audit_key();
        if !view
            .sessions
            .iter()
            .any(|session| session == &current_session_key)
        {
            view.sessions.push(current_session_key);
            view.sessions.sort();
        }
        view
    }

    #[allow(dead_code)]
    pub fn current_audit_session_key(&self) -> String {
        self.session_audit_key()
    }

    pub fn set_transcript_auto_expand(&mut self, enabled: bool) {
        self.transcript_auto_expand = enabled;
    }

    #[allow(dead_code)]
    pub fn as_model(&self) -> &dyn HostSessionModel {
        self.session.model()
    }

    pub fn set_scenario(&mut self, scenario: DevScenario) {
        self.session.model_mut().set_scenario(scenario);
        self.refresh_audit_timeline();
    }

    pub fn select_scenario(&mut self, raw: &str) {
        let next = match raw {
            "booting" => DevScenario::Booting,
            "degraded" => DevScenario::Degraded,
            "startup-failure" => DevScenario::StartupFailure,
            "compat-failure" => DevScenario::CompatibilityFailure,
            "reconnecting" => DevScenario::Reconnecting,
            _ => DevScenario::Ready,
        };
        self.set_scenario(next);
    }

    pub fn update_draft(&mut self, value: impl Into<String>) {
        self.session.model_mut().composer_mut().set_draft(value);
    }

    fn command_target(&self) -> CommandTarget {
        let session_key = self.session_audit_key();
        let dispatcher_instance_id = match &self.session {
            SessionSource::Remote(session) => session
                .session_data()
                .dispatcher_binding
                .as_ref()
                .map(|binding| binding.dispatcher_instance_id.clone())
                .filter(|value| !value.is_empty()),
            SessionSource::Mock(_) => None,
        };

        CommandTarget {
            session_key,
            dispatcher_instance_id,
        }
    }

    fn session_audit_key(&self) -> String {
        match &self.session {
            SessionSource::Remote(session) => session
                .session_data()
                .dispatcher_binding
                .as_ref()
                .map(|binding| format!("remote:{}", binding.session_id))
                .unwrap_or_else(|| "remote:detached".into()),
            SessionSource::Mock(_) => format!("mock:{}", self.scenario().key()),
        }
    }

    fn refresh_audit_timeline(&mut self) {
        let session_key = self.session_audit_key();
        let transcript = self.transcript().clone();
        self.audit_timeline
            .append_transcript_snapshot(&session_key, &transcript);
        #[cfg(not(target_arch = "wasm32"))]
        if let Some(path) = crate::audit_timeline::default_audit_timeline_path() {
            let _ = crate::audit_timeline::persist(&path, &self.audit_timeline);
        }
    }

    #[allow(dead_code)]
    pub fn submit_prompt(&mut self) -> bool {
        let submitted = self.session.model_mut().submit();
        if submitted {
            self.refresh_audit_timeline();
        }
        submitted
    }

    pub fn submit_prompt_command(&mut self) -> Option<TargetedCommand> {
        let target = self.command_target();
        match &mut self.session {
            SessionSource::Remote(session) => {
                let trimmed = session.composer().draft().trim().to_string();
                if trimmed.is_empty() || !session.can_submit() {
                    return None;
                }
                if !session.submit() {
                    return None;
                }
                self.refresh_audit_timeline();
                Some(TargetedCommand::legacy_json(
                    target,
                    serde_json::json!({
                        "type": "user_prompt",
                        "text": trimmed,
                    })
                    .to_string(),
                ))
            }
            SessionSource::Mock(session) => {
                let submitted = session.submit();
                if submitted {
                    self.refresh_audit_timeline();
                }
                submitted.then(|| TargetedCommand::legacy_json(target, String::new()))
            }
        }
        .filter(|command| !command.command_json.is_empty())
    }

    pub fn cancel_command(&self) -> Option<TargetedCommand> {
        match &self.session {
            SessionSource::Remote(session) if session.is_run_active() => Some(
                TargetedCommand::legacy_json(
                    self.command_target(),
                    serde_json::json!({ "type": "cancel" }).to_string(),
                ),
            ),
            _ => None,
        }
    }

    #[allow(dead_code)]
    pub fn request_dispatcher_switch_command(
        &mut self,
        profile: &str,
        model: Option<&str>,
    ) -> Option<TargetedCommand> {
        let target = self.command_target();
        match &mut self.session {
            SessionSource::Remote(session) => {
                match session.request_dispatcher_switch(profile, model)? {
                    DispatcherSwitchCommandOutcome::Issued { request_id } => {
                        Some(TargetedCommand::legacy_json(
                            target,
                            serde_json::json!({
                                "type": "switch_dispatcher",
                                "request_id": request_id,
                                "profile": profile,
                                "model": model,
                            })
                            .to_string(),
                        ))
                    }
                    DispatcherSwitchCommandOutcome::Noop => None,
                }
            }
            SessionSource::Mock(_) => None,
        }
    }

    #[allow(dead_code)]
    pub fn request_dispatcher_switch_command_json(
        &mut self,
        profile: &str,
        model: Option<&str>,
    ) -> Option<String> {
        self.request_dispatcher_switch_command(profile, model)
            .map(|command| command.command_json)
    }

    pub fn apply_remote_event_json(&mut self, json: &str) -> Result<bool, serde_json::Error> {
        match &mut self.session {
            SessionSource::Remote(session) => {
                let applied = session.apply_event_json(json)?;
                if applied {
                    self.refresh_audit_timeline();
                }
                Ok(applied)
            }
            SessionSource::Mock(_) => Ok(false),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::audit_timeline::AuditEntryKind;
    use crate::fixtures::MessageRole;

    const REMOTE_SNAPSHOT_JSON: &str = DEMO_REMOTE_SNAPSHOT_JSON;

    #[test]
    fn default_controller_uses_mock_session_source() {
        let controller = AppController::default();

        assert_eq!(controller.scenario(), DevScenario::Ready);
        assert_eq!(controller.messages().len(), 1);
        assert_eq!(
            controller.summary().connection,
            "Connected to local host session"
        );
    }

    #[test]
    fn remote_controller_uses_remote_session_source() {
        let controller = AppController::from_remote_snapshot_json(REMOTE_SNAPSHOT_JSON).unwrap();

        assert_eq!(controller.scenario(), DevScenario::Ready);
        assert!(controller.summary().connection.contains("main"));
        assert_eq!(controller.messages().len(), 1);
        let session = controller.session_data();
        let dispatcher = session.dispatcher_binding.as_ref().unwrap();
        assert_eq!(dispatcher.dispatcher_instance_id, "omg_primary_01HVDEMO");
        assert_eq!(dispatcher.expected_role, "primary-driver");
        assert_eq!(dispatcher.expected_profile, "primary-interactive");
        assert_eq!(
            dispatcher.expected_model.as_deref(),
            Some("anthropic:claude-sonnet-4-6")
        );
        assert_eq!(dispatcher.session_id, "session_01HVDEMO");
        assert_eq!(dispatcher.available_options.len(), 2);
        assert_eq!(dispatcher.switch_state.as_ref().unwrap().status, "idle");
        assert_eq!(dispatcher.switch_state.as_ref().unwrap().request_id, None);
    }

    #[test]
    fn select_scenario_maps_known_values() {
        let mut controller = AppController::default();

        controller.select_scenario("degraded");
        assert_eq!(controller.scenario(), DevScenario::Degraded);

        controller.select_scenario("startup-failure");
        assert_eq!(controller.scenario(), DevScenario::StartupFailure);

        controller.select_scenario("compat-failure");
        assert_eq!(controller.scenario(), DevScenario::CompatibilityFailure);
    }

    #[test]
    fn select_scenario_defaults_unknown_values_to_ready() {
        let mut controller = AppController::default();
        controller.select_scenario("not-a-real-scenario");

        assert_eq!(controller.scenario(), DevScenario::Ready);
    }

    #[test]
    fn update_draft_and_submit_route_through_session_source() {
        let mut controller = AppController::default();
        controller.update_draft("hello world");

        assert_eq!(controller.composer().draft(), "hello world");
        assert!(controller.submit_prompt());
        assert_eq!(controller.messages().len(), 3);
    }

    #[test]
    fn remote_submit_emits_user_prompt_command_json() {
        let mut controller =
            AppController::from_remote_snapshot_json(REMOTE_SNAPSHOT_JSON).unwrap();
        controller.update_draft("ship it");

        let command = controller.submit_prompt_command().unwrap();

        assert_eq!(
            command.command_json,
            r#"{"text":"ship it","type":"user_prompt"}"#
        );
        assert_eq!(command.target.session_key, "remote:session_01HVDEMO");
        assert_eq!(
            command.target.dispatcher_instance_id.as_deref(),
            Some("omg_primary_01HVDEMO")
        );
        assert_eq!(controller.messages()[1].role, MessageRole::User);
        assert_eq!(
            command.transport_json().unwrap(),
            r#"{"target":{"session_key":"remote:session_01HVDEMO","dispatcher_instance_id":"omg_primary_01HVDEMO"},"command":{"kind":"legacy_json","command_json":"{\"text\":\"ship it\",\"type\":\"user_prompt\"}"}}"#
        );
    }

    #[test]
    fn remote_events_route_only_for_remote_session_source() {
        let mut controller =
            AppController::from_remote_snapshot_json(REMOTE_SNAPSHOT_JSON).unwrap();

        assert!(
            controller
                .apply_remote_event_json(r#"{"type":"message_start","role":"assistant"}"#)
                .unwrap()
        );
        assert!(
            controller
                .apply_remote_event_json(r#"{"type":"message_chunk","text":"hello remote"}"#)
                .unwrap()
        );
        assert!(
            controller
                .apply_remote_event_json(r#"{"type":"message_end"}"#)
                .unwrap()
        );
        assert_eq!(controller.messages().last().unwrap().text, "hello remote");

        let mut mock_controller = AppController::default();
        assert!(
            !mock_controller
                .apply_remote_event_json(r#"{"type":"message_start","role":"assistant"}"#)
                .unwrap()
        );
    }

    #[test]
    fn switch_session_mode_swaps_between_mock_and_remote_demo() {
        let mut controller = AppController::default();
        assert_eq!(controller.session_mode(), SessionMode::Mock);

        controller.switch_session_mode("live");
        assert_eq!(controller.session_mode(), SessionMode::Live);
        assert!(
            controller
                .summary()
                .connection
                .contains("Attached to Omegon host")
        );
        assert_eq!(controller.messages().len(), 1);

        controller.switch_session_mode("mock");
        assert_eq!(controller.session_mode(), SessionMode::Mock);
        assert_eq!(
            controller.summary().connection,
            "Connected to local host session"
        );
    }

    #[test]
    fn transcript_auto_expand_defaults_on_and_is_mutable() {
        let mut controller = AppController::default();
        assert!(controller.transcript_auto_expand());

        controller.set_transcript_auto_expand(false);
        assert!(!controller.transcript_auto_expand());

        controller.set_transcript_auto_expand(true);
        assert!(controller.transcript_auto_expand());
    }

    #[test]
    fn controller_retains_transcript_blocks_in_audit_timeline() {
        let mut controller = AppController::default();
        controller.update_draft("hello audit");
        assert!(controller.submit_prompt());

        assert_eq!(controller.audit_timeline().entries.len(), 2);
        assert_eq!(
            controller.audit_timeline().entries[0].block_id,
            "mock:ready:turn-1-block-0"
        );
        assert!(
            controller.audit_timeline().entries[1]
                .content
                .contains("scaffold only proves")
        );
    }

    #[test]
    fn audit_timeline_query_filters_current_session_entries() {
        let mut controller = AppController::default();
        controller.update_draft("hello audit");
        assert!(controller.submit_prompt());

        let filtered = controller.query_audit_timeline(&AuditTimelineQuery {
            session_key: Some(controller.current_audit_session_key()),
            turn_number: Some(1),
            kind: Some(AuditEntryKind::Text),
            text: "scaffold".into(),
        });

        assert_eq!(filtered.entries.len(), 1);
        assert_eq!(filtered.entries[0].turn_number, 1);
        assert_eq!(filtered.entries[0].kind, AuditEntryKind::Text);
        assert_eq!(filtered.entries[0].session_key, "mock:ready");
    }

    #[test]
    fn audit_timeline_query_updates_session_options_after_mode_switch() {
        let mut controller = AppController::default();
        controller.update_draft("hello audit");
        assert!(controller.submit_prompt());
        controller.switch_session_mode("live");

        let filtered = controller.query_audit_timeline(&AuditTimelineQuery::default());

        assert!(filtered.sessions.contains(&"mock:ready".to_string()));
        assert!(
            filtered
                .sessions
                .contains(&"remote:session_01HVDEMO".to_string())
        );
    }

    #[test]
    fn is_run_active_false_by_default() {
        let controller = AppController::from_remote_snapshot_json(REMOTE_SNAPSHOT_JSON).unwrap();
        assert!(!controller.is_run_active());
    }

    #[test]
    fn is_run_active_becomes_true_on_turn_start_and_false_on_turn_end() {
        let mut controller =
            AppController::from_remote_snapshot_json(REMOTE_SNAPSHOT_JSON).unwrap();

        controller
            .apply_remote_event_json(r#"{"type":"turn_start","turn":1}"#)
            .unwrap();
        assert!(controller.is_run_active());

        controller
            .apply_remote_event_json(r#"{"type":"turn_end","turn":1}"#)
            .unwrap();
        assert!(!controller.is_run_active());
    }

    #[test]
    fn run_active_blocks_submit() {
        let mut controller =
            AppController::from_remote_snapshot_json(REMOTE_SNAPSHOT_JSON).unwrap();

        controller
            .apply_remote_event_json(r#"{"type":"turn_start","turn":1}"#)
            .unwrap();

        assert!(!controller.can_submit());

        controller.update_draft("rush message");
        let result = controller.submit_prompt_command();
        assert!(
            result.is_none(),
            "submit must be blocked while run is active"
        );
    }

    #[test]
    fn cancel_command_json_produced_when_run_active() {
        let mut controller =
            AppController::from_remote_snapshot_json(REMOTE_SNAPSHOT_JSON).unwrap();

        assert!(controller.cancel_command().is_none());

        controller
            .apply_remote_event_json(r#"{"type":"turn_start","turn":1}"#)
            .unwrap();

        let cancel = controller
            .cancel_command()
            .expect("cancel command expected during active run");
        assert_eq!(cancel.command_json, r#"{"type":"cancel"}"#);
        assert_eq!(cancel.target.session_key, "remote:session_01HVDEMO");
        assert_eq!(
            cancel.target.dispatcher_instance_id.as_deref(),
            Some("omg_primary_01HVDEMO")
        );
        assert_eq!(
            cancel.transport_json().unwrap(),
            r#"{"target":{"session_key":"remote:session_01HVDEMO","dispatcher_instance_id":"omg_primary_01HVDEMO"},"command":{"kind":"legacy_json","command_json":"{\"type\":\"cancel\"}"}}"#
        );
    }

    #[test]
    fn session_reset_clears_run_active() {
        let mut controller =
            AppController::from_remote_snapshot_json(REMOTE_SNAPSHOT_JSON).unwrap();

        controller
            .apply_remote_event_json(r#"{"type":"turn_start","turn":1}"#)
            .unwrap();
        assert!(controller.is_run_active());

        controller
            .apply_remote_event_json(r#"{"type":"session_reset"}"#)
            .unwrap();
        assert!(!controller.is_run_active());
    }

    #[test]
    fn remote_dispatcher_switch_emits_command_and_updates_pending_state() {
        let mut controller =
            AppController::from_remote_snapshot_json(REMOTE_SNAPSHOT_JSON).unwrap();

        let command = controller
            .request_dispatcher_switch_command("supervisor-heavy", Some("openai:gpt-4.1"))
            .unwrap();

        assert_eq!(
            command.command_json,
            r#"{"model":"openai:gpt-4.1","profile":"supervisor-heavy","request_id":"dispatcher-switch-1","type":"switch_dispatcher"}"#
        );
        assert_eq!(command.target.session_key, "remote:session_01HVDEMO");
        assert_eq!(
            command.target.dispatcher_instance_id.as_deref(),
            Some("omg_primary_01HVDEMO")
        );

        let session = controller.session_data();
        let switch_state = &session.dispatcher_binding.as_ref().unwrap().switch_state;
        let switch_state = switch_state.as_ref().unwrap();
        assert_eq!(
            switch_state.request_id.as_deref(),
            Some("dispatcher-switch-1")
        );
        assert_eq!(
            switch_state.requested_profile.as_deref(),
            Some("supervisor-heavy")
        );
        assert_eq!(
            switch_state.requested_model.as_deref(),
            Some("openai:gpt-4.1")
        );
        assert_eq!(switch_state.status, "pending");
    }

    #[test]
    fn dispatcher_switch_becomes_active_when_snapshot_confirms_binding() {
        let mut controller =
            AppController::from_remote_snapshot_json(REMOTE_SNAPSHOT_JSON).unwrap();

        controller
            .request_dispatcher_switch_command_json("supervisor-heavy", Some("openai:gpt-4.1"))
            .unwrap();

        controller
            .apply_remote_event_json(
                r#"{"type":"state_snapshot","data":{"design":{"focused":null,"implementing":[],"actionable":[],"all_nodes":[],"counts":{}},"openspec":{"total_tasks":5,"done_tasks":2},"cleave":{"active":false,"total_children":0,"completed":0,"failed":0},"session":{"turns":12,"tool_calls":34,"compactions":1},"dispatcher":{"session_id":"session_01HVDEMO","dispatcher_instance_id":"omg_primary_01HVDEMO","expected_role":"primary-driver","expected_profile":"supervisor-heavy","expected_model":"openai:gpt-4.1","control_plane_schema":2,"token_ref":"secret://auspex/instances/omg_primary_01HVDEMO/token","observed_base_url":"http://127.0.0.1:7842","last_verified_at":"2026-04-04T12:05:00Z","available_options":[{"profile":"primary-interactive","label":"Primary Interactive","model":"anthropic:claude-sonnet-4-6"},{"profile":"supervisor-heavy","label":"Supervisor Heavy","model":"openai:gpt-4.1"}]},"harness":{"git_branch":"main","git_detached":false,"thinking_level":"medium","capability_tier":"victory","providers":[{"name":"Anthropic","authenticated":true,"auth_method":"api-key","model":"claude-sonnet"}],"memory_available":true,"cleave_available":true,"memory_warning":null,"active_delegates":[]}}}"#,
            )
            .unwrap();

        let session = controller.session_data();
        let dispatcher = session.dispatcher_binding.as_ref().unwrap();
        assert_eq!(dispatcher.expected_profile, "supervisor-heavy");
        assert_eq!(dispatcher.expected_model.as_deref(), Some("openai:gpt-4.1"));
        let switch_state = dispatcher.switch_state.as_ref().unwrap();
        assert_eq!(switch_state.status, "active");
        assert_eq!(
            switch_state.request_id.as_deref(),
            Some("dispatcher-switch-1")
        );
        assert_eq!(
            switch_state.note.as_deref(),
            Some("Dispatcher switch confirmed by snapshot")
        );
        assert!(controller.messages().last().unwrap().text.contains(
            "Dispatcher switch confirmed (dispatcher-switch-1): supervisor-heavy · openai:gpt-4.1"
        ));
    }

    #[test]
    fn startup_surface_uses_typed_notice() {
        let mut controller = AppController::default();
        controller.set_scenario(DevScenario::Booting);
        controller.set_bootstrap_note(Some("Starting Omegon at /tmp/omegon…".into()));

        let surface = controller.surface_notice().expect("surface notice");
        assert_eq!(surface.kind, AppSurfaceKind::Startup);
        assert!(surface.body.contains("Launching the Omegon engine"));
        assert_eq!(
            surface.detail.as_deref(),
            Some("Starting Omegon at /tmp/omegon…")
        );
    }

    #[test]
    fn reconnecting_surface_uses_typed_notice() {
        let mut controller = AppController::default();
        controller.set_scenario(DevScenario::Reconnecting);

        let surface = controller.surface_notice().expect("surface notice");
        assert_eq!(surface.kind, AppSurfaceKind::Reconnecting);
        assert!(
            surface
                .body
                .contains("connection to the host is being restored")
        );
        assert_eq!(surface.detail, None);
    }

    #[test]
    fn failed_surface_uses_typed_failure_notice() {
        let mut controller = AppController::default();
        controller.set_scenario(DevScenario::CompatibilityFailure);
        controller.set_bootstrap_note(Some(
            "Update Omegon to a compatible version and restart.".into(),
        ));

        let surface = controller.surface_notice().expect("surface notice");
        assert_eq!(surface.kind, AppSurfaceKind::CompatibilityFailure);
        assert_eq!(surface.body, "Host incompatible");
        assert!(surface.detail.as_deref().unwrap().contains("Update Omegon"));
    }

    #[test]
    fn ready_state_uses_bootstrap_note_surface() {
        let mut controller = AppController::default();
        controller.set_bootstrap_note(Some("Attached via startup discovery".into()));

        let surface = controller.surface_notice().expect("surface notice");
        assert_eq!(surface.kind, AppSurfaceKind::BootstrapNote);
        assert_eq!(surface.body, "Attached via startup discovery");
        assert_eq!(surface.detail, None);
    }

    #[test]
    fn snapshot_active_state_for_different_request_id_does_not_confirm_local_pending_switch() {
        let mut controller =
            AppController::from_remote_snapshot_json(REMOTE_SNAPSHOT_JSON).unwrap();

        controller
            .request_dispatcher_switch_command_json("supervisor-heavy", Some("openai:gpt-4.1"))
            .unwrap();

        controller
            .apply_remote_event_json(
                r#"{"type":"state_snapshot","data":{"design":{"focused":null,"implementing":[],"actionable":[],"all_nodes":[],"counts":{}},"openspec":{"total_tasks":5,"done_tasks":2},"cleave":{"active":false,"total_children":0,"completed":0,"failed":0},"session":{"turns":12,"tool_calls":34,"compactions":1},"dispatcher":{"session_id":"session_01HVDEMO","dispatcher_instance_id":"omg_primary_01HVDEMO","expected_role":"primary-driver","expected_profile":"supervisor-heavy","expected_model":"openai:gpt-4.1","control_plane_schema":2,"token_ref":"secret://auspex/instances/omg_primary_01HVDEMO/token","observed_base_url":"http://127.0.0.1:7842","last_verified_at":"2026-04-04T12:05:00Z","available_options":[{"profile":"primary-interactive","label":"Primary Interactive","model":"anthropic:claude-sonnet-4-6"},{"profile":"supervisor-heavy","label":"Supervisor Heavy","model":"openai:gpt-4.1"}],"switch_state":{"request_id":"dispatcher-switch-999","requested_profile":"supervisor-heavy","requested_model":"openai:gpt-4.1","status":"active","failure_code":null,"note":"Different request became active"}},"harness":{"git_branch":"main","git_detached":false,"thinking_level":"medium","capability_tier":"victory","providers":[{"name":"Anthropic","authenticated":true,"auth_method":"api-key","model":"claude-sonnet"}],"memory_available":true,"cleave_available":true,"memory_warning":null,"active_delegates":[]}}}"#,
            )
            .unwrap();

        let switch_state = controller
            .session_data()
            .dispatcher_binding
            .unwrap()
            .switch_state
            .unwrap();
        assert_eq!(switch_state.status, "active");
        assert_eq!(
            switch_state.request_id.as_deref(),
            Some("dispatcher-switch-999")
        );
        assert!(controller.messages().iter().any(|message| {
            message
                .text
                .contains("Dispatcher reports a different active request (dispatcher-switch-999): supervisor-heavy · openai:gpt-4.1")
        }));
        assert!(controller.messages().iter().all(|message| {
            !message
                .text
                .contains("Dispatcher switch confirmed (dispatcher-switch-1)")
        }));
    }

    #[test]
    fn dispatcher_switch_to_current_binding_is_noop() {
        let mut controller =
            AppController::from_remote_snapshot_json(REMOTE_SNAPSHOT_JSON).unwrap();

        let command = controller.request_dispatcher_switch_command_json(
            "primary-interactive",
            Some("anthropic:claude-sonnet-4-6"),
        );

        assert!(command.is_none());
        let switch_state = controller
            .session_data()
            .dispatcher_binding
            .unwrap()
            .switch_state
            .unwrap();
        assert_eq!(switch_state.status, "active");
        assert_eq!(
            switch_state.note.as_deref(),
            Some("Dispatcher already active: primary-interactive")
        );
    }

    #[test]
    fn repeated_dispatcher_switch_requests_supersede_prior_pending_request() {
        let mut controller =
            AppController::from_remote_snapshot_json(REMOTE_SNAPSHOT_JSON).unwrap();

        controller
            .request_dispatcher_switch_command_json("supervisor-heavy", Some("openai:gpt-4.1"))
            .unwrap();
        controller
            .request_dispatcher_switch_command_json("supervisor-heavy", None)
            .unwrap();

        let switch_state = controller
            .session_data()
            .dispatcher_binding
            .unwrap()
            .switch_state
            .unwrap();
        assert_eq!(switch_state.status, "pending");
        assert_eq!(
            switch_state.request_id.as_deref(),
            Some("dispatcher-switch-2")
        );
        assert_eq!(
            switch_state.requested_profile.as_deref(),
            Some("supervisor-heavy")
        );
        assert_eq!(switch_state.requested_model, None);
        assert!(
            controller
                .messages()
                .iter()
                .any(|message| message.text.contains("Dispatcher switch superseded"))
        );
    }

    #[test]
    fn explicit_snapshot_failure_wins_for_dispatcher_switch() {
        let mut controller =
            AppController::from_remote_snapshot_json(REMOTE_SNAPSHOT_JSON).unwrap();

        controller
            .request_dispatcher_switch_command_json("supervisor-heavy", Some("openai:gpt-4.1"))
            .unwrap();
        controller
            .apply_remote_event_json(
                r#"{"type":"state_snapshot","data":{"design":{"focused":null,"implementing":[],"actionable":[],"all_nodes":[],"counts":{}},"openspec":{"total_tasks":5,"done_tasks":2},"cleave":{"active":false,"total_children":0,"completed":0,"failed":0},"session":{"turns":12,"tool_calls":34,"compactions":1},"dispatcher":{"session_id":"session_01HVDEMO","dispatcher_instance_id":"omg_primary_01HVDEMO","expected_role":"primary-driver","expected_profile":"primary-interactive","expected_model":"anthropic:claude-sonnet-4-6","control_plane_schema":2,"token_ref":"secret://auspex/instances/omg_primary_01HVDEMO/token","observed_base_url":"http://127.0.0.1:7842","last_verified_at":"2026-04-04T12:05:00Z","available_options":[{"profile":"primary-interactive","label":"Primary Interactive","model":"anthropic:claude-sonnet-4-6"},{"profile":"supervisor-heavy","label":"Supervisor Heavy","model":"openai:gpt-4.1"}],"switch_state":{"request_id":"dispatcher-switch-1","requested_profile":"supervisor-heavy","requested_model":"openai:gpt-4.1","status":"failed","failure_code":"backend_rejected","note":"Backend rejected dispatcher switch"}},"harness":{"git_branch":"main","git_detached":false,"thinking_level":"medium","capability_tier":"victory","providers":[{"name":"Anthropic","authenticated":true,"auth_method":"api-key","model":"claude-sonnet"}],"memory_available":true,"cleave_available":true,"memory_warning":null,"active_delegates":[]}}}"#,
            )
            .unwrap();

        let switch_state = controller
            .session_data()
            .dispatcher_binding
            .unwrap()
            .switch_state
            .unwrap();
        assert_eq!(switch_state.status, "failed");
        assert_eq!(
            switch_state.request_id.as_deref(),
            Some("dispatcher-switch-1")
        );
        assert_eq!(
            switch_state.failure_code.as_deref(),
            Some("backend_rejected")
        );
        assert!(controller
            .messages()
            .last()
            .unwrap()
            .text
            .contains("Dispatcher switch failed (dispatcher-switch-1): supervisor-heavy · openai:gpt-4.1 [backend_rejected]"));
    }
}
