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
}
