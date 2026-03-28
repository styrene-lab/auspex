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
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ConversationState {
    draft: String,
    messages: Vec<ChatMessage>,
}

impl Default for ConversationState {
    fn default() -> Self {
        Self {
            draft: String::new(),
            messages: vec![ChatMessage {
                role: MessageRole::Assistant,
                text: "Auspex scaffold ready. Type a prompt to grow the shell from here.".into(),
            }],
        }
    }
}

impl ConversationState {
    pub fn draft(&self) -> &str {
        &self.draft
    }

    pub fn messages(&self) -> &[ChatMessage] {
        &self.messages
    }

    pub fn set_draft(&mut self, value: impl Into<String>) {
        self.draft = value.into();
    }

    pub fn submit(&mut self) -> bool {
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
                div { class: "status", "Simple mode" }
            }

            main { class: "transcript",
                for message in state.read().messages().iter() {
                    article {
                        class: if message.role == MessageRole::User { "bubble bubble-user" } else { "bubble bubble-assistant" },
                        h2 {
                            if message.role == MessageRole::User { "You" } else { "Auspex" }
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
                    placeholder: "Start with the smallest useful prompt…",
                    oninput: move |event| state.write().set_draft(event.value()),
                }
                button {
                    class: "composer-submit",
                    r#type: "submit",
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
}
