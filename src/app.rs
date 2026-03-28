use dioxus::prelude::*;

use crate::fixtures::{DevScenario, MessageRole, MockHostSession};
use crate::session_model::HostSessionModel;

#[component]
pub fn App() -> Element {
    let mut session = use_signal(MockHostSession::default);

    rsx! {
        document::Stylesheet { href: asset!("/assets/main.css") }
        div { class: "shell",
            header { class: "header",
                div {
                    h1 { "Auspex" }
                    p { "Conversation-first scaffold" }
                }
                div { class: session.read().shell_state().status_class(), "{session.read().shell_state().label()}" }
            }

            section { class: "devbar",
                label { "Scenario" }
                select {
                    value: session.read().scenario().key(),
                    onchange: move |event| {
                        let next = match event.value().as_str() {
                            "booting" => DevScenario::Booting,
                            "degraded" => DevScenario::Degraded,
                            "compat-failure" => DevScenario::CompatibilityFailure,
                            "reconnecting" => DevScenario::Reconnecting,
                            _ => DevScenario::Ready,
                        };
                        session.write().set_scenario(next);
                    },
                    for scenario in DevScenario::ALL {
                        option { value: scenario.key(), "{scenario.label()}" }
                    }
                }
            }

            section { class: "summary-bar",
                div { class: "summary-card",
                    h2 { "Connection" }
                    p { "{session.read().summary().connection}" }
                }
                div { class: "summary-card",
                    h2 { "Activity" }
                    p { "{session.read().summary().activity}" }
                }
                div { class: "summary-card",
                    h2 { "Work" }
                    p { "{session.read().summary().work}" }
                }
            }

            main { class: "transcript",
                for message in session.read().messages().iter() {
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
                    session.write().submit();
                },
                textarea {
                    class: "composer-input",
                    rows: "3",
                    value: session.read().composer().draft().to_string(),
                    disabled: !session.read().can_submit(),
                    placeholder: if session.read().can_submit() {
                        "Start with the smallest useful prompt…"
                    } else {
                        "Conversation input is unavailable in the current host state."
                    },
                    oninput: move |event| session.write().composer_mut().set_draft(event.value()),
                }
                button {
                    class: "composer-submit",
                    r#type: "submit",
                    disabled: !session.read().can_submit(),
                    "Send"
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::fixtures::*;
    use crate::session_model::HostSessionModel;

    #[test]
    fn blank_draft_does_not_submit() {
        let mut session = MockHostSession::default();
        session.composer_mut().set_draft("   ");

        assert!(!session.submit());
        assert_eq!(session.messages().len(), 1);
        assert_eq!(session.composer().draft(), "   ");
    }

    #[test]
    fn submit_appends_user_and_placeholder_reply() {
        let mut session = MockHostSession::default();
        session.composer_mut().set_draft("hello world");

        assert!(session.submit());
        assert_eq!(session.composer().draft(), "");
        assert_eq!(session.messages().len(), 3);
        assert_eq!(session.messages()[1].role, MessageRole::User);
        assert_eq!(session.messages()[1].text, "hello world");
        assert_eq!(session.messages()[2].role, MessageRole::Assistant);
    }

    #[test]
    fn booting_state_blocks_submit() {
        let mut session = MockHostSession::from_scenario(DevScenario::Booting);
        session.composer_mut().set_draft("hello world");

        assert!(!session.submit());
        assert_eq!(session.messages().len(), 1);
    }

    #[test]
    fn degraded_state_allows_submit() {
        let mut session = MockHostSession::from_scenario(DevScenario::Degraded);
        session.composer_mut().set_draft("still there?");

        assert!(session.submit());
        assert_eq!(session.messages().len(), 4);
    }

    #[test]
    fn reconnecting_state_blocks_submit() {
        let mut session = MockHostSession::from_scenario(DevScenario::Reconnecting);
        session.composer_mut().set_draft("can you hear me?");

        assert!(!session.submit());
        assert_eq!(session.messages().len(), 2);
    }

    #[test]
    fn trait_can_read_core_fields() {
        let session = MockHostSession::ready_session();
        let model: &dyn HostSessionModel = &session;

        assert_eq!(model.shell_state(), crate::fixtures::ShellState::Ready);
        assert_eq!(model.scenario(), DevScenario::Ready);
        assert_eq!(model.messages().len(), 1);
    }
}
