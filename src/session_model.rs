use crate::fixtures::{ChatMessage, ComposerState, DevScenario, HostSessionSummary, SessionData, ShellState, WorkData};

pub trait HostSessionModel {
    fn shell_state(&self) -> ShellState;
    fn scenario(&self) -> DevScenario;
    fn summary(&self) -> &HostSessionSummary;
    fn messages(&self) -> &[ChatMessage];
    fn composer(&self) -> &ComposerState;
    fn composer_mut(&mut self) -> &mut ComposerState;
    fn set_scenario(&mut self, scenario: DevScenario);
    fn can_submit(&self) -> bool;
    fn submit(&mut self) -> bool;
    fn is_run_active(&self) -> bool;
    fn work_data(&self) -> WorkData;
    fn session_data(&self) -> SessionData;
}
