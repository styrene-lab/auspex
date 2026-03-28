use crate::fixtures::{
    ChatMessage, ComposerState, DevScenario, HostSessionSummary, MockHostSession, ShellState,
};
use crate::remote_session::RemoteHostSession;
use crate::session_model::HostSessionModel;

#[derive(Debug)]
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
}

pub struct AppController {
    session: SessionSource,
}

impl Default for AppController {
    fn default() -> Self {
        Self {
            session: SessionSource::default(),
        }
    }
}

impl AppController {
    pub fn from_remote_snapshot_json(json: &str) -> Result<Self, serde_json::Error> {
        let session = RemoteHostSession::from_snapshot_json(json)?;
        Ok(Self {
            session: SessionSource::Remote(session),
        })
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

    const REMOTE_SNAPSHOT_JSON: &str = r#"{
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
            "actionable": []
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

    #[test]
    fn default_controller_uses_mock_session_source() {
        let controller = AppController::default();

        assert_eq!(controller.scenario(), DevScenario::Ready);
        assert_eq!(controller.messages().len(), 1);
        assert_eq!(controller.summary().connection, "Connected to local host session");
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
    fn remote_events_route_only_for_remote_session_source() {
        let mut controller = AppController::from_remote_snapshot_json(REMOTE_SNAPSHOT_JSON).unwrap();

        assert!(controller
            .apply_remote_event_json(r#"{"type":"message_start","role":"assistant"}"#)
            .unwrap());
        assert!(controller
            .apply_remote_event_json(r#"{"type":"message_chunk","text":"hello remote"}"#)
            .unwrap());
        assert!(controller
            .apply_remote_event_json(r#"{"type":"message_end"}"#)
            .unwrap());
        assert_eq!(controller.messages().last().unwrap().text, "hello remote");

        let mut mock_controller = AppController::default();
        assert!(!mock_controller
            .apply_remote_event_json(r#"{"type":"message_start","role":"assistant"}"#)
            .unwrap());
    }
}
