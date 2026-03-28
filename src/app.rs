use dioxus::prelude::*;

use crate::fixtures::{ConversationState, DevScenario, MessageRole};

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
                            "reconnecting" => DevScenario::Reconnecting,
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
    use crate::fixtures::*;

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

    #[test]
    fn reconnecting_state_blocks_submit() {
        let mut state = ConversationState::from_scenario(DevScenario::Reconnecting);
        state.set_draft("can you hear me?");

        assert!(!state.submit());
        assert_eq!(state.messages().len(), 2);
    }
}
