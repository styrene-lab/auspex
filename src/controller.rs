use crate::fixtures::{DevScenario, MockHostSession};
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
    pub fn session(&self) -> &dyn HostSessionModel {
        &self.session
    }

    pub fn session_mut(&mut self) -> &mut dyn HostSessionModel {
        &mut self.session
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
        assert_eq!(controller.session().scenario(), DevScenario::Degraded);

        controller.select_scenario("compat-failure");
        assert_eq!(
            controller.session().scenario(),
            DevScenario::CompatibilityFailure
        );
    }

    #[test]
    fn select_scenario_defaults_unknown_values_to_ready() {
        let mut controller = AppController::default();
        controller.select_scenario("not-a-real-scenario");

        assert_eq!(controller.session().scenario(), DevScenario::Ready);
    }

    #[test]
    fn submit_prompt_uses_session_model() {
        let mut controller = AppController::default();
        controller.update_draft("hello world");

        assert!(controller.submit_prompt());
        assert_eq!(controller.session().messages().len(), 3);
    }
}
