use dioxus::prelude::*;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ChatMessage {
    pub role: MessageRole,
    pub text: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MessageRole {
    User,
    Assistant,
    System,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ShellState {
    Booting,
    StartingStyrene,
    StartingOmegon,
    CompatibilityChecking,
    Ready,
    Degraded,
    Failed,
}

impl ShellState {
    fn label(self) -> &'static str {
        match self {
            Self::Booting => "Booting",
            Self::StartingStyrene => "Starting Styrene",
            Self::StartingOmegon => "Starting Omegon",
            Self::CompatibilityChecking => "Checking compatibility",
            Self::Ready => "Ready",
            Self::Degraded => "Degraded",
            Self::Failed => "Failed",
        }
    }

    fn status_class(self) -> &'static str {
        match self {
            Self::Ready => "status status-ready",
            Self::Degraded => "status status-degraded",
            Self::Failed => "status status-failed",
            _ => "status status-pending",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DevScenario {
    Ready,
    Booting,
    Degraded,
    CompatibilityFailure,
}

impl DevScenario {
    const ALL: [Self; 4] = [
        Self::Ready,
        Self::Booting,
        Self::Degraded,
        Self::CompatibilityFailure,
    ];

    fn key(self) -> &'static str {
        match self {
            Self::Ready => "ready",
            Self::Booting => "booting",
            Self::Degraded => "degraded",
            Self::CompatibilityFailure => "compat-failure",
        }
    }

    fn label(self) -> &'static str {
        match self {
            Self::Ready => "Ready",
            Self::Booting => "Booting",
            Self::Degraded => "Degraded",
            Self::CompatibilityFailure => "Compat failure",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ConversationState {
    draft: String,
    messages: Vec<ChatMessage>,
    shell_state: ShellState,
    scenario: DevScenario,
}

impl Default for ConversationState {
    fn default() -> Self {
        Self::from_scenario(DevScenario::Ready)
    }
}

impl ConversationState {
    pub fn from_scenario(scenario: DevScenario) -> Self {
        match scenario {
            DevScenario::Ready => Self {
                draft: String::new(),
                messages: vec![ChatMessage {
                    role: MessageRole::Assistant,
                    text: "Auspex scaffold ready. Type a prompt to grow the shell from here."
                        .into(),
                }],
                shell_state: ShellState::Ready,
                scenario,
            },
            DevScenario::Booting => Self {
                draft: String::new(),
                messages: vec![ChatMessage {
                    role: MessageRole::System,
                    text: "Auspex is starting Styrene and Omegon. The conversation shell will become interactive when the host session is ready.".into(),
                }],
                shell_state: ShellState::StartingOmegon,
                scenario,
            },
            DevScenario::Degraded => Self {
                draft: String::new(),
                messages: vec![
                    ChatMessage {
                        role: MessageRole::Assistant,
                        text: "Cached session restored. The local shell is still usable.".into(),
                    },
                    ChatMessage {
                        role: MessageRole::System,
                        text: "Styrene relay is degraded. Phone clients may reconnect automatically while local work continues.".into(),
                    },
                ],
                shell_state: ShellState::Degraded,
                scenario,
            },
            DevScenario::CompatibilityFailure => Self {
                draft: String::new(),
                messages: vec![ChatMessage {
                    role: MessageRole::System,
                    text: "Compatibility failure: Auspex expects Omegon control-plane schema 1, but the detected host did not satisfy the declared contract.".into(),
                }],
                shell_state: ShellState::Failed,
                scenario,
            },
        }
    }

    pub fn draft(&self) -> &str {
        &self.draft
    }

    pub fn messages(&self) -> &[ChatMessage] {
        &self.messages
    }

    pub fn shell_state(&self) -> ShellState {
        self.shell_state
    }

    pub fn scenario(&self) -> DevScenario {
        self.scenario
    }

    pub fn set_draft(&mut self, value: impl Into<String>) {
        self.draft = value.into();
    }

    pub fn set_scenario(&mut self, scenario: DevScenario) {
        *self = Self::from_scenario(scenario);
    }

    pub fn can_submit(&self) -> bool {
        self.shell_state == ShellState::Ready || self.shell_state == ShellState::Degraded
    }

    pub fn submit(&mut self) -> bool {
        if !self.can_submit() {
            return false;
        }

        let trimmed = self.draft.trim();
        if trimmed.is_empty() {
            return false;
        }

        self.messages.push(ChatMessage {
            role: MessageRole::User,
            text: trimmed.to_string(),
        });
        self.messages.push(ChatMessage {
            role: MessageRole::Assistant,
            text:
                "No engine is attached yet. This scaffold only proves the basic conversation shell."
                    .into(),
        });
        self.draft.clear();
        true
    }
}

#[component]
pub fn App() -> Element {
    let mut state = use_signal(ConversationState::default);

    rsx! {
        document::Stylesheet { href: asset!("/assets/main.css") }
        div { class: "shell",
            header { class: "header",
                div {
                    h1 { "Auspex" }
                    p { "Conversation-first scaffold" }
                }
                div { class: state.read().shell_state().status_class(), "{state.read().shell_state().label()}" }
            }

            section { class: "devbar",
                label { "Scenario" }
                select {
                    value: state.read().scenario().key(),
                    onchange: move |event| {
                        let next = match event.value().as_str() {
                            "booting" => DevScenario::Booting,
                            "degraded" => DevScenario::Degraded,
                            "compat-failure" => DevScenario::CompatibilityFailure,
                            _ => DevScenario::Ready,
                        };
                        state.write().set_scenario(next);
                    },
                    for scenario in DevScenario::ALL {
                        option { value: scenario.key(), "{scenario.label()}" }
                    }
                }
            }

            main { class: "transcript",
                for message in state.read().messages().iter() {
                    article {
                        class: match message.role {
                            MessageRole::User => "bubble bubble-user",
                            MessageRole::Assistant => "bubble bubble-assistant",
                            MessageRole::System => "bubble bubble-system",
                        },
                        h2 {
                            match message.role {
                                MessageRole::User => "You",
                                MessageRole::Assistant => "Auspex",
                                MessageRole::System => "System",
                            }
                        }
                        p { "{message.text}" }
                    }
                }
            }

            form {
                class: "composer",
                onsubmit: move |event| {
                    event.prevent_default();
                    state.write().submit();
                },
                textarea {
                    class: "composer-input",
                    rows: "3",
                    value: state.read().draft().to_string(),
                    disabled: !state.read().can_submit(),
                    placeholder: if state.read().can_submit() {
                        "Start with the smallest useful prompt…"
                    } else {
                        "Conversation input is unavailable in the current host state."
                    },
                    oninput: move |event| state.write().set_draft(event.value()),
                }
                button {
                    class: "composer-submit",
                    r#type: "submit",
                    disabled: !state.read().can_submit(),
                    "Send"
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn blank_draft_does_not_submit() {
        let mut state = ConversationState::default();
        state.set_draft("   ");

        assert!(!state.submit());
        assert_eq!(state.messages().len(), 1);
        assert_eq!(state.draft(), "   ");
    }

    #[test]
    fn submit_appends_user_and_placeholder_reply() {
        let mut state = ConversationState::default();
        state.set_draft("hello world");

        assert!(state.submit());
        assert_eq!(state.draft(), "");
        assert_eq!(state.messages().len(), 3);
        assert_eq!(state.messages()[1].role, MessageRole::User);
        assert_eq!(state.messages()[1].text, "hello world");
        assert_eq!(state.messages()[2].role, MessageRole::Assistant);
    }

    #[test]
    fn booting_state_blocks_submit() {
        let mut state = ConversationState::from_scenario(DevScenario::Booting);
        state.set_draft("hello world");

        assert!(!state.submit());
        assert_eq!(state.messages().len(), 1);
    }

    #[test]
    fn degraded_state_allows_submit() {
        let mut state = ConversationState::from_scenario(DevScenario::Degraded);
        state.set_draft("still there?");

        assert!(state.submit());
        assert_eq!(state.messages().len(), 4);
    }
}
