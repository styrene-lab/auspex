use crate::fixtures::{
    ChatMessage, ComposerState, DevScenario, HostSessionSummary, MockHostSession, ShellState,
};
use crate::session_model::HostSessionModel;

pub struct AppController {
    session: MockHostSession,
}

impl Default for AppController {
    fn default() -> Self {
        Self {
            session: MockHostSession::default(),
        }
    }
}

impl AppController {
    pub fn shell_state(&self) -> ShellState {
        self.session.shell_state()
    }

    pub fn scenario(&self) -> DevScenario {
        self.session.scenario()
    }

    pub fn summary(&self) -> &HostSessionSummary {
        self.session.summary()
    }

    pub fn messages(&self) -> &[ChatMessage] {
        self.session.messages()
    }

    pub fn composer(&self) -> &ComposerState {
        self.session.composer()
    }

    pub fn can_submit(&self) -> bool {
        self.session.can_submit()
    }

    pub fn as_model(&self) -> &dyn HostSessionModel {
        &self.session
    }

    pub fn set_scenario(&mut self, scenario: DevScenario) {
        self.session.set_scenario(scenario);
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
        self.session.composer_mut().set_draft(value);
    }

    pub fn submit_prompt(&mut self) -> bool {
        self.session.submit()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
    fn submit_prompt_uses_session_model() {
        let mut controller = AppController::default();
        controller.update_draft("hello world");

        assert!(controller.submit_prompt());
        assert_eq!(controller.messages().len(), 3);
    }
}
