use crate::fixtures::{
    ChatMessage, ComposerState, DevScenario, GraphData, HostSessionSummary, MockHostSession,
    SessionData, ShellState, WorkData,
};
use crate::remote_session::RemoteHostSession;
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
    "openspec": {"totalTasks": 5, "doneTasks": 2},
    "cleave": {"active": false, "totalChildren": 0, "completed": 0, "failed": 0},
    "session": {"turns": 12, "toolCalls": 34, "compactions": 1},
    "harness": {
        "gitBranch": "main",
        "gitDetached": false,
        "thinkingLevel": "medium",
        "capabilityTier": "victory",
        "providers": [{"name": "Anthropic", "authenticated": true, "authMethod": "api-key", "model": "claude-sonnet"}],
        "memoryAvailable": true,
        "cleaveAvailable": true,
        "memoryWarning": null,
        "activeDelegates": []
    }
}"#;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SessionMode {
    Mock,
    RemoteDemo,
}

impl SessionMode {
    pub const ALL: [Self; 2] = [Self::Mock, Self::RemoteDemo];

    pub fn key(self) -> &'static str {
        match self {
            Self::Mock => "mock",
            Self::RemoteDemo => "remote-demo",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Mock => "Mock local",
            Self::RemoteDemo => "Remote demo",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum SessionSource {
    Mock(MockHostSession),
    Remote(RemoteHostSession),
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
            Self::Remote(session) => session,
        }
    }

    fn model_mut(&mut self) -> &mut dyn HostSessionModel {
        match self {
            Self::Mock(session) => session,
            Self::Remote(session) => session,
        }
    }

    fn mode(&self) -> SessionMode {
        match self {
            Self::Mock(_) => SessionMode::Mock,
            Self::Remote(_) => SessionMode::RemoteDemo,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AppController {
    session: SessionSource,
    bootstrap_note: Option<String>,
}

impl Default for AppController {
    fn default() -> Self {
        Self {
            session: SessionSource::default(),
            bootstrap_note: None,
        }
    }
}

impl AppController {
    pub fn from_remote_snapshot_json(json: &str) -> Result<Self, serde_json::Error> {
        let session = RemoteHostSession::from_snapshot_json(json)?;
        Ok(Self {
            session: SessionSource::Remote(session),
            bootstrap_note: None,
        })
    }

    pub fn remote_demo() -> Self {
        Self::from_remote_snapshot_json(DEMO_REMOTE_SNAPSHOT_JSON)
            .expect("embedded remote demo snapshot must stay valid")
    }

    pub fn session_mode(&self) -> SessionMode {
        self.session.mode()
    }

    pub fn bootstrap_note(&self) -> Option<&str> {
        self.bootstrap_note.as_deref()
    }

    pub fn set_bootstrap_note(&mut self, note: Option<String>) {
        self.bootstrap_note = note;
    }

    pub fn is_remote(&self) -> bool {
        self.session_mode() == SessionMode::RemoteDemo
    }

    pub fn switch_session_mode(&mut self, raw: &str) {
        self.session = match raw {
            "remote-demo" => SessionSource::Remote(
                RemoteHostSession::from_snapshot_json(DEMO_REMOTE_SNAPSHOT_JSON)
                    .expect("embedded remote demo snapshot must stay valid"),
            ),
            _ => SessionSource::Mock(MockHostSession::default()),
        };
        self.bootstrap_note = None;
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

    pub fn as_model(&self) -> &dyn HostSessionModel {
        self.session.model()
    }

    pub fn set_scenario(&mut self, scenario: DevScenario) {
        self.session.model_mut().set_scenario(scenario);
    }

    pub fn select_scenario(&mut self, raw: &str) {
        let next = match raw {
            "booting" => DevScenario::Booting,
            "degraded" => DevScenario::Degraded,
            "compat-failure" => DevScenario::CompatibilityFailure,
            "reconnecting" => DevScenario::Reconnecting,
            _ => DevScenario::Ready,
        };
        self.set_scenario(next);
    }

    pub fn update_draft(&mut self, value: impl Into<String>) {
        self.session.model_mut().composer_mut().set_draft(value);
    }

    pub fn submit_prompt(&mut self) -> bool {
        self.session.model_mut().submit()
    }

    pub fn submit_prompt_command_json(&mut self) -> Option<String> {
        match &mut self.session {
            SessionSource::Remote(session) => {
                let trimmed = session.composer().draft().trim().to_string();
                if trimmed.is_empty() || !session.can_submit() {
                    return None;
                }
                if !session.submit() {
                    return None;
                }
                Some(
                    serde_json::json!({
                        "type": "user_prompt",
                        "text": trimmed,
                    })
                    .to_string(),
                )
            }
            SessionSource::Mock(session) => session.submit().then(|| String::new()),
        }
        .filter(|command| !command.is_empty())
    }

    pub fn cancel_command_json(&self) -> Option<String> {
        match &self.session {
            SessionSource::Remote(session) if session.is_run_active() => {
                Some(serde_json::json!({ "type": "cancel" }).to_string())
            }
            _ => None,
        }
    }

    pub fn apply_remote_event_json(&mut self, json: &str) -> Result<bool, serde_json::Error> {
        match &mut self.session {
            SessionSource::Remote(session) => session.apply_event_json(json),
            SessionSource::Mock(_) => Ok(false),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
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
    }

    #[test]
    fn select_scenario_maps_known_values() {
        let mut controller = AppController::default();

        controller.select_scenario("degraded");
        assert_eq!(controller.scenario(), DevScenario::Degraded);

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

        let command = controller.submit_prompt_command_json().unwrap();

        assert_eq!(command, r#"{"text":"ship it","type":"user_prompt"}"#);
        assert_eq!(controller.messages()[1].role, MessageRole::User);
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

        controller.switch_session_mode("remote-demo");
        assert_eq!(controller.session_mode(), SessionMode::RemoteDemo);
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
        let result = controller.submit_prompt_command_json();
        assert!(
            result.is_none(),
            "submit must be blocked while run is active"
        );
    }

    #[test]
    fn cancel_command_json_produced_when_run_active() {
        let mut controller =
            AppController::from_remote_snapshot_json(REMOTE_SNAPSHOT_JSON).unwrap();

        assert!(controller.cancel_command_json().is_none());

        controller
            .apply_remote_event_json(r#"{"type":"turn_start","turn":1}"#)
            .unwrap();

        let cancel = controller
            .cancel_command_json()
            .expect("cancel command expected during active run");
        assert_eq!(cancel, r#"{"type":"cancel"}"#);
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
}
