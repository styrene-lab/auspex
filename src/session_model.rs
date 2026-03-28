use crate::fixtures::{ChatMessage, ComposerState, DevScenario, HostSessionSummary, ShellState};

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
}
