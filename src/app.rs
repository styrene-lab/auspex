use dioxus::prelude::*;

use crate::controller::AppController;
use crate::fixtures::{DevScenario, MessageRole};

#[component]
pub fn App() -> Element {
    let mut controller = use_signal(AppController::default);

    rsx! {
        document::Stylesheet { href: asset!("/assets/main.css") }
        div { class: "shell",
            header { class: "header",
                div {
                    h1 { "Auspex" }
                    p { "Conversation-first scaffold" }
                }
                div { class: controller.read().shell_state().status_class(), "{controller.read().shell_state().label()}" }
            }

            section { class: "devbar",
                label { "Scenario" }
                select {
                    value: controller.read().scenario().key(),
                    onchange: move |event| controller.write().select_scenario(event.value().as_str()),
                    for scenario in DevScenario::ALL {
                        option { value: scenario.key(), "{scenario.label()}" }
                    }
                }
            }

            section { class: "summary-bar",
                div { class: "summary-card",
                    h2 { "Connection" }
                    p { "{controller.read().summary().connection}" }
                }
                div { class: "summary-card",
                    h2 { "Activity" }
                    p { "{controller.read().summary().activity}" }
                }
                div { class: "summary-card",
                    h2 { "Work" }
                    p { "{controller.read().summary().work}" }
                }
            }

            main { class: "transcript",
                for message in controller.read().messages().iter() {
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
                    controller.write().submit_prompt();
                },
                textarea {
                    class: "composer-input",
                    rows: "3",
                    value: controller.read().composer().draft().to_string(),
                    disabled: !controller.read().can_submit(),
                    placeholder: if controller.read().can_submit() {
                        "Start with the smallest useful prompt…"
                    } else {
                        "Conversation input is unavailable in the current host state."
                    },
                    oninput: move |event| controller.write().update_draft(event.value()),
                }
                button {
                    class: "composer-submit",
                    r#type: "submit",
                    disabled: !controller.read().can_submit(),
                    "Send"
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::controller::AppController;
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
        let mut controller = AppController::default();
        controller.update_draft("hello world");

        assert!(controller.submit_prompt());
        assert_eq!(controller.composer().draft(), "");
        assert_eq!(controller.messages().len(), 3);
        assert_eq!(controller.messages()[1].role, MessageRole::User);
        assert_eq!(controller.messages()[1].text, "hello world");
        assert_eq!(controller.messages()[2].role, MessageRole::Assistant);
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
        let controller = AppController::default();
        let model: &dyn HostSessionModel = controller.as_model();

        assert_eq!(model.shell_state(), crate::fixtures::ShellState::Ready);
        assert_eq!(model.scenario(), DevScenario::Ready);
        assert_eq!(model.messages().len(), 1);
    }
}
