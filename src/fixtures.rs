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
    pub fn label(self) -> &'static str {
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

    pub fn status_class(self) -> &'static str {
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
    Reconnecting,
}

impl DevScenario {
    pub const ALL: [Self; 5] = [
        Self::Ready,
        Self::Booting,
        Self::Degraded,
        Self::CompatibilityFailure,
        Self::Reconnecting,
    ];

    pub fn key(self) -> &'static str {
        match self {
            Self::Ready => "ready",
            Self::Booting => "booting",
            Self::Degraded => "degraded",
            Self::CompatibilityFailure => "compat-failure",
            Self::Reconnecting => "reconnecting",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Ready => "Ready",
            Self::Booting => "Booting",
            Self::Degraded => "Degraded",
            Self::CompatibilityFailure => "Compat failure",
            Self::Reconnecting => "Reconnecting",
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
            DevScenario::Reconnecting => Self {
                draft: String::new(),
                messages: vec![
                    ChatMessage {
                        role: MessageRole::Assistant,
                        text: "Last known session state is still visible.".into(),
                    },
                    ChatMessage {
                        role: MessageRole::System,
                        text: "Connection to the host is reconnecting. New input is temporarily paused while Auspex restores the session link.".into(),
                    },
                ],
                shell_state: ShellState::CompatibilityChecking,
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
