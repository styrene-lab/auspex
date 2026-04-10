use dioxus::prelude::*;

use crate::audit_timeline::{AuditEntry, AuditEntryKind, AuditTimelineStore};
use crate::bootstrap::BootstrapResult;
#[cfg(not(target_arch = "wasm32"))]
use crate::command_transport::CommandTransport;
use crate::controller::{AppController, SessionMode};
use crate::event_stream::EventStreamHandle;
use crate::fixtures::{MessageRole, TranscriptData};
#[cfg(not(target_arch = "wasm32"))]
use crate::ipc_client::IpcEventStreamHandle;
use crate::runtime_types::TargetedCommand;
use crate::screens::{GraphScreen, ScribeScreen, SessionScreen};

const APP_VERSION: &str = env!("CARGO_PKG_VERSION");
const LAYOUT_DEBUG_ENABLED: bool = true;
const SHELL_BLOCKOUT_MODE: bool = true;
#[cfg(not(target_arch = "wasm32"))]
const SETTINGS_MENU_ID: &str = "auspex-open-settings";


#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Workspace {
    Chat,
    Session,
    Scribe,
    Graph,
    Audit,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum SettingsAuthAction {
    Refresh,
    Login,
    Logout,
    Unlock,
}

impl SettingsAuthAction {
    fn label(self) -> &'static str {
        match self {
            Self::Refresh => "Refresh status",
            Self::Login => "Login",
            Self::Logout => "Logout",
            Self::Unlock => "Unlock",
        }
    }

    fn command_slug(self) -> &'static str {
        match self {
            Self::Refresh => "auth.refresh",
            Self::Login => "auth.login",
            Self::Logout => "auth.logout",
            Self::Unlock => "auth.unlock",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct LayoutDebugBox {
    selector: String,
    top: i32,
    left: i32,
    width: i32,
    height: i32,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct LayoutDebugSnapshot {
    inner_height: i32,
    boxes: Vec<LayoutDebugBox>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct SettingsActionModel {
    action: SettingsAuthAction,
    detail: String,
    enabled: bool,
    provider: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct SettingsProviderCardModel {
    name: String,
    provider_key: Option<String>,
    status_label: String,
    status_detail: String,
    capability_detail: String,
    guidance: String,
    login_action: SettingsActionModel,
    logout_action: SettingsActionModel,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct SettingsStatusRowModel {
    label: String,
    value: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct SettingsPanelModel {
    selected_route_id: String,
    route_options: Vec<(String, String, String)>,
    target_label: String,
    target_detail: String,
    route_detail: String,
    lifecycle_summary: String,
    auth_status_label: String,
    auth_status_detail: String,
    provider_guidance: String,
    provider_cards: Vec<SettingsProviderCardModel>,
    secrets_rows: Vec<SettingsStatusRowModel>,
    vault_rows: Vec<SettingsStatusRowModel>,
    general_actions: Vec<SettingsActionModel>,
    last_error: Option<String>,
    last_action: Option<String>,
    provider_ready: bool,
}

#[cfg(not(target_arch = "wasm32"))]
fn dispatch_targeted_command(
    transport: &CommandTransport,
    event_stream: Option<&EventStreamHandle>,
    command: &TargetedCommand,
) -> Result<(), String> {
    transport.dispatch_targeted_command(event_stream, command)
}

#[cfg(target_arch = "wasm32")]
fn dispatch_targeted_command(stream: &EventStreamHandle, command: &TargetedCommand) {
    stream.send_targeted_command(command);
}

fn parse_layout_debug_snapshot(raw: &str) -> Option<LayoutDebugSnapshot> {
    let value: serde_json::Value = serde_json::from_str(raw).ok()?;
    let inner_height = value.get("innerHeight")?.as_i64()? as i32;
    let boxes = value
        .get("boxes")?
        .as_array()?
        .iter()
        .filter_map(|entry| {
            Some(LayoutDebugBox {
                selector: entry.get("selector")?.as_str()?.to_string(),
                top: entry.get("top")?.as_i64()? as i32,
                left: entry.get("left")?.as_i64()? as i32,
                width: entry.get("width")?.as_i64()? as i32,
                height: entry.get("height")?.as_i64()? as i32,
            })
        })
        .collect::<Vec<_>>();
    Some(LayoutDebugSnapshot { inner_height, boxes })
}

fn provider_command_name(name: &str) -> Option<String> {
    let normalized: String = name
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() {
                character.to_ascii_lowercase()
            } else {
                ' '
            }
        })
        .collect();
    normalized
        .split_whitespace()
        .next()
        .filter(|token| !token.is_empty())
        .map(ToOwned::to_owned)
}

fn merged_provider_inventory(
    runtime_providers: &[crate::fixtures::ProviderInfo],
    settings_providers: Option<&[crate::fixtures::ProviderInfo]>,
) -> Vec<crate::fixtures::ProviderInfo> {
    let Some(settings_providers) = settings_providers.filter(|providers| !providers.is_empty())
    else {
        return runtime_providers.to_vec();
    };

    let mut merged = Vec::with_capacity(runtime_providers.len().max(settings_providers.len()));
    let mut seen = std::collections::HashSet::new();

    for runtime in runtime_providers {
        let key = provider_command_name(&runtime.name)
            .unwrap_or_else(|| runtime.name.trim().to_ascii_lowercase());
        let settings = settings_providers.iter().find(|candidate| {
            provider_command_name(&candidate.name)
                .unwrap_or_else(|| candidate.name.trim().to_ascii_lowercase())
                == key
        });
        merged.push(crate::fixtures::ProviderInfo {
            name: runtime.name.clone(),
            authenticated: runtime.authenticated,
            auth_method: settings
                .and_then(|provider| provider.auth_method.clone())
                .or_else(|| runtime.auth_method.clone()),
            model: runtime
                .model
                .clone()
                .or_else(|| settings.and_then(|provider| provider.model.clone())),
        });
        seen.insert(key);
    }

    for settings in settings_providers {
        let key = provider_command_name(&settings.name)
            .unwrap_or_else(|| settings.name.trim().to_ascii_lowercase());
        if seen.insert(key) {
            merged.push(settings.clone());
        }
    }

    merged
}

fn build_settings_panel_model(
    controller: &crate::controller::AppController,
    session: &crate::fixtures::SessionData,
    auth_state: Option<&crate::controller::SettingsAuthState>,
) -> SettingsPanelModel {
    let dispatcher_binding = session.dispatcher_binding.as_ref();
    let route_options = controller.available_command_routes();
    let selected_route_id = controller.selected_command_route_id();
    let selected_route = route_options
        .iter()
        .find(|route| route.route_id == selected_route_id)
        .or_else(|| route_options.first());
    let descriptor = dispatcher_binding
        .and_then(|binding| binding.instance_descriptor.as_ref())
        .or(session.instance_descriptor.as_ref());

    let target_label = selected_route
        .map(|route| route.label.clone())
        .or_else(|| {
            dispatcher_binding
                .map(|binding| binding.dispatcher_instance_id.clone())
                .filter(|value| !value.is_empty())
        })
        .or_else(|| {
            descriptor
                .map(|descriptor| descriptor.identity.instance_id.clone())
                .filter(|value| !value.is_empty())
        })
        .unwrap_or_else(|| "No attached target".to_string());

    let target_role = dispatcher_binding
        .map(|binding| binding.expected_role.as_str())
        .filter(|value| !value.is_empty())
        .or_else(|| {
            descriptor
                .map(|descriptor| descriptor.identity.role.as_str())
                .filter(|value| !value.is_empty())
        })
        .unwrap_or("operator");

    let target_profile = dispatcher_binding
        .map(|binding| binding.expected_profile.as_str())
        .filter(|value| !value.is_empty())
        .or_else(|| {
            descriptor
                .map(|descriptor| descriptor.identity.profile.as_str())
                .filter(|value| !value.is_empty())
        })
        .unwrap_or("current-profile");

    let target_session = dispatcher_binding
        .map(|binding| binding.session_id.as_str())
        .filter(|value| !value.is_empty())
        .unwrap_or("local-session");

    let target_detail = if let Some(route) = selected_route {
        format!("{} · {}", route.label, route.detail)
    } else {
        format!("{target_role} · {target_profile} · session {target_session}")
    };

    let route_detail = if let Some(route) = selected_route {
        format!(
            "Operator actions currently target {} ({}) via route {}. This will later bind to Omegon's canonical slash executor without changing the surface.",
            route.label, route.detail, route.route_id
        )
    } else if let Some(binding) = dispatcher_binding {
        let schema = binding.control_plane_schema;
        let endpoint = binding
            .observed_base_url
            .as_deref()
            .filter(|value| !value.is_empty())
            .unwrap_or("No control-plane endpoint reported");
        format!(
            "Prepared to route operator actions through command envelopes for target {target_label} over control-plane schema {schema} ({endpoint})."
        )
    } else {
        format!(
            "Prepared to route operator actions through a target-aware command adapter once Auspex is attached to a concrete Omegon instance. No concrete target is attached yet; current UI label is {target_label}."
        )
    };

    let lifecycle_summary = controller
        .attached_instances()
        .iter()
        .find(|instance| instance.instance_id == target_label)
        .and_then(|instance| instance.registry_record.as_ref())
        .map(|record| {
            let freshness = record
                .observed
                .health
                .freshness
                .as_ref()
                .map(|freshness| format!("{freshness:?}").to_ascii_lowercase())
                .unwrap_or_else(|| "unknown".into());
            format!(
                "Lifecycle: {:?} · freshness {}",
                record.identity.status, freshness
            )
        })
        .unwrap_or_else(|| "Lifecycle: unavailable".into());

    let provider_inventory = merged_provider_inventory(
        session.providers.as_slice(),
        auth_state.map(|state| state.providers.as_slice()),
    );
    let authenticated = provider_inventory
        .iter()
        .filter(|provider| provider.authenticated)
        .count();
    let provider_total = provider_inventory.len();
    let auth_status_label = if authenticated > 0 {
        format!("{authenticated} authenticated provider(s)")
    } else if provider_total > 0 {
        "Providers reported, authentication missing".to_string()
    } else {
        "No providers reported".to_string()
    };

    let auth_status_detail = if let Some(auth_state) = auth_state {
        if let Some(error) = auth_state.last_error.as_deref() {
            format!("Last auth bridge error: {error}")
        } else if let Some(last_action) = auth_state.last_action.as_deref() {
            format!("Last completed auth bridge action: {last_action}")
        } else if provider_total == 0 {
            "No host providers are currently visible. Refresh the auth bridge to inspect current credentials.".to_string()
        } else {
            format!(
                "Auth and secret actions stay instance-scoped to {target_label}. Use the provider cards below to log in, log out, and confirm which backend is currently runnable."
            )
        }
    } else if provider_total == 0 {
        "No host providers are currently visible. Refresh through the command adapter before prompting for login or unlock work.".to_string()
    } else {
        format!(
            "Auth bridge actions stay instance-scoped to {target_label}. UI wiring is ready to call the command adapter instead of a singleton event-stream path."
        )
    };

    let action_target_label = target_label.clone();
    let action_detail = |verb: &str| {
        format!(
            "Will dispatch {verb} against target {} via the command adapter when the auth bridge is connected.",
            action_target_label
        )
    };

    let provider_guidance = if provider_total == 0 {
        "Refresh status first. Auspex cannot offer provider-specific login or logout until the attached host reports a provider inventory.".to_string()
    } else if authenticated == 0 {
        "At least one provider is known, but none are authenticated. Use a Login action on the provider you want to activate, then refresh status if the browser handoff completes outside the app.".to_string()
    } else {
        "Authenticated providers can execute prompts immediately. Log out from a card to force account rotation, or log in to add a second provider before switching models.".to_string()
    };

    let provider_cards = if provider_inventory.is_empty() {
        vec![SettingsProviderCardModel {
            name: "No provider inventory".to_string(),
            provider_key: None,
            status_label: "Unknown".to_string(),
            status_detail: "The attached host did not report providers.".to_string(),
            capability_detail: "Model availability is unknown until the next auth refresh.".to_string(),
            guidance: "Run Refresh status to query the desktop auth bridge before asking the operator to fix credentials manually.".to_string(),
            login_action: SettingsActionModel {
                action: SettingsAuthAction::Login,
                detail: action_detail("auth.login"),
                enabled: false,
                provider: None,
            },
            logout_action: SettingsActionModel {
                action: SettingsAuthAction::Logout,
                detail: action_detail("auth.logout"),
                enabled: false,
                provider: None,
            },
        }]
    } else {
        provider_inventory
            .iter()
            .map(|provider| {
                let provider_key = provider_command_name(&provider.name);
                let auth_method = provider
                    .auth_method
                    .as_deref()
                    .unwrap_or("auth method not reported");
                let model = provider.model.as_deref().unwrap_or("model not reported");
                let status_label = if provider.authenticated {
                    "Authenticated"
                } else {
                    "Needs login"
                }
                .to_string();
                let status_detail = if provider.authenticated {
                    format!("Signed in via {auth_method}")
                } else {
                    format!("Credential handshake missing via {auth_method}")
                };
                let capability_detail = format!("Current model surface: {model}");
                let guidance = if provider.authenticated {
                    "Provider looks runnable from the current snapshot. Logout is available if you need to clear credentials or switch accounts in the browser.".to_string()
                } else {
                    "This provider is known but not authenticated. Login will trigger the host-backed auth flow; refresh after browser completion if state does not update automatically.".to_string()
                };
                SettingsProviderCardModel {
                    name: provider.name.clone(),
                    provider_key: provider_key.clone(),
                    status_label,
                    status_detail,
                    capability_detail,
                    guidance,
                    login_action: SettingsActionModel {
                        action: SettingsAuthAction::Login,
                        detail: action_detail("auth.login"),
                        enabled: provider_key.is_some(),
                        provider: provider_key.clone(),
                    },
                    logout_action: SettingsActionModel {
                        action: SettingsAuthAction::Logout,
                        detail: action_detail("auth.logout"),
                        enabled: provider_key.is_some(),
                        provider: provider_key,
                    },
                }
            })
            .collect()
    };

    let dispatcher_token_ref = dispatcher_binding
        .and_then(|binding| binding.token_ref.as_deref())
        .filter(|value| !value.is_empty())
        .unwrap_or("No dispatcher token reference reported")
        .to_string();
    let control_plane_auth_mode = session
        .telemetry
        .control_plane
        .as_ref()
        .and_then(|control_plane| control_plane.auth_mode.as_deref())
        .filter(|value| !value.is_empty())
        .unwrap_or("unreported")
        .to_string();
    let control_plane_base_url = session
        .telemetry
        .control_plane
        .as_ref()
        .and_then(|control_plane| control_plane.base_url.as_deref())
        .filter(|value| !value.is_empty())
        .or_else(|| {
            dispatcher_binding
                .and_then(|binding| binding.observed_base_url.as_deref())
                .filter(|value| !value.is_empty())
        })
        .unwrap_or("unreported")
        .to_string();

    let secrets_rows = vec![
        SettingsStatusRowModel {
            label: "Provider coverage".into(),
            value: if provider_total == 0 {
                "No provider inventory reported".into()
            } else {
                format!("{authenticated} / {provider_total} authenticated")
            },
        },
        SettingsStatusRowModel {
            label: "Dispatcher token ref".into(),
            value: dispatcher_token_ref,
        },
        SettingsStatusRowModel {
            label: "Control-plane auth".into(),
            value: control_plane_auth_mode,
        },
    ];

    let vault_rows = vec![
        SettingsStatusRowModel {
            label: "Vault bridge".into(),
            value: if session.memory_available {
                "Available to the attached host".into()
            } else if let Some(warning) = session.memory_warning.as_deref() {
                format!("Limited: {warning}")
            } else {
                "Unavailable from current snapshot".into()
            },
        },
        SettingsStatusRowModel {
            label: "Unlock flow".into(),
            value: if auth_state.and_then(|state| state.last_action.as_deref()) == Some("unlock") {
                "Last requested action was unlock".into()
            } else {
                "Use Unlock to reopen the host vault or keyring flow".into()
            },
        },
        SettingsStatusRowModel {
            label: "Control-plane endpoint".into(),
            value: control_plane_base_url,
        },
    ];

    SettingsPanelModel {
        selected_route_id,
        route_options: route_options
            .into_iter()
            .map(|route| (route.route_id, route.label, route.detail))
            .collect(),
        target_label,
        target_detail,
        route_detail,
        lifecycle_summary,
        auth_status_label,
        auth_status_detail,
        provider_guidance,
        provider_cards,
        secrets_rows,
        vault_rows,
        general_actions: vec![
            SettingsActionModel {
                action: SettingsAuthAction::Refresh,
                detail: "Refresh live provider auth status through the desktop auth bridge.".into(),
                enabled: true,
                provider: None,
            },
            SettingsActionModel {
                action: SettingsAuthAction::Unlock,
                detail: action_detail("auth.unlock"),
                enabled: true,
                provider: None,
            },
        ],
        last_error: auth_state.and_then(|state| state.last_error.clone()),
        last_action: auth_state.and_then(|state| state.last_action.clone()),
        provider_ready: authenticated > 0,
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct ProviderBlockedComposerModel {
    title: String,
    detail: String,
    action_label: String,
}

fn build_provider_blocked_composer_model(
    _session: &crate::fixtures::SessionData,
    can_submit: bool,
) -> Option<ProviderBlockedComposerModel> {
    if can_submit {
        return None;
    }

    Some(ProviderBlockedComposerModel {
        title: "Prompt execution blocked".into(),
        detail:
            "Omegon has no authenticated providers. Authenticate a provider in Settings before sending prompts so Auspex can route work to a runnable model backend.".into(),
        action_label: "Open Settings".into(),
    })
}

#[component]
pub fn App() -> Element {
    let bootstrap = try_consume_context::<BootstrapResult>();
    // Extract spawning binary before bootstrap is consumed by use_signal.
    #[cfg(not(target_arch = "wasm32"))]
    let spawning_binary: Option<String> = bootstrap.as_ref().and_then(|b| {
        if let crate::bootstrap::BootstrapSource::SpawningOmegon { binary } = &b.source {
            Some(binary.clone())
        } else {
            None
        }
    });
    let mut event_stream = use_signal(|| None::<EventStreamHandle>);
    #[cfg(not(target_arch = "wasm32"))]
    let mut ipc_event_stream = use_signal(|| None::<IpcEventStreamHandle>);
    #[cfg(not(target_arch = "wasm32"))]
    let mut command_transport = use_signal(|| None::<CommandTransport>);
    #[cfg(not(target_arch = "wasm32"))]
    let settings_status_message = use_signal(|| None::<String>);
    let composer_ready_notice = use_signal(|| None::<String>);
    let mut workspace = use_signal(|| Workspace::Chat);
    let mut settings_open = use_signal(|| false);
    let mut controller = use_signal(move || {
        if let Some(bootstrap) = bootstrap {
            event_stream.set(bootstrap.event_stream);
            #[cfg(not(target_arch = "wasm32"))]
            ipc_event_stream.set(bootstrap.ipc_event_stream);
            #[cfg(not(target_arch = "wasm32"))]
            command_transport.set(bootstrap.command_transport);
            let mut controller = bootstrap.controller;
            controller.set_bootstrap_note(bootstrap.note);
            controller
        } else {
            AppController::default()
        }
    });

    use_future(move || {
        let mut controller = controller;
        let event_stream = event_stream;
        #[cfg(not(target_arch = "wasm32"))]
        let ipc_event_stream = ipc_event_stream;
        #[cfg(not(target_arch = "wasm32"))]
        let mut settings_status_message = settings_status_message;
        let mut composer_ready_notice = composer_ready_notice;
        let mut settings_open = settings_open;
        let mut workspace = workspace;
        async move {
            loop {
                #[cfg(not(target_arch = "wasm32"))]
                {
                    let now_epoch_seconds = std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .map(|duration| duration.as_secs())
                        .unwrap_or(0);
                    let mut controller = controller.write();
                    controller.evaluate_instance_lifecycle(now_epoch_seconds);
                    controller.ensure_settings_auth_status();
                }
                #[cfg(not(target_arch = "wasm32"))]
                if let (
                    Some(ipc_handle),
                    Some(crate::command_transport::CommandTransport::Ipc(client)),
                ) = (
                    ipc_event_stream.read().clone(),
                    command_transport.read().clone(),
                ) {
                    let ipc_events = ipc_handle.inbox.drain();
                    if !ipc_events.is_empty() {
                        let mut needs_refresh = false;
                        {
                            let mut controller = controller.write();
                            for event in ipc_events {
                                needs_refresh |= crate::controller::AppController::ipc_event_requires_refresh(&event);
                                let _ = controller.apply_ipc_event(event);
                            }
                        }
                        if needs_refresh && let Ok(snapshot) = client.get_state().await {
                            let mut controller = controller.write();
                            let _ = controller.apply_ipc_state_snapshot(&snapshot);
                        }
                    }
                }
                if let Some(handle) = event_stream.read().clone() {
                    let events = handle.inbox.drain();
                    if !events.is_empty() {
                        let mut controller = controller.write();
                        for event in events {
                            #[cfg(not(target_arch = "wasm32"))]
                            if let Some(result) =
                                crate::controller::AppController::parse_slash_command_result(&event)
                            {
                                let message = if result.accepted {
                                    format!(
                                        "Slash {} {} succeeded: {}",
                                        result.name, result.args, result.output
                                    )
                                } else {
                                    format!(
                                        "Slash {} {} failed: {}",
                                        result.name, result.args, result.output
                                    )
                                };
                                settings_status_message.set(Some(message));
                                if result.accepted
                                    && matches!(result.name.as_str(), "login" | "logout" | "auth")
                                {
                                    let _ = controller.refresh_settings_auth_status();
                                    if controller.can_submit() {
                                        composer_ready_notice.set(Some(
                                            "Provider ready — prompting is available again.".into(),
                                        ));
                                        settings_open.set(false);
                                        workspace.set(Workspace::Chat);
                                    }
                                }
                            }
                            let _ = controller.apply_remote_event_json(&event);
                        }
                    }
                }
                #[cfg(not(target_arch = "wasm32"))]
                tokio::time::sleep(std::time::Duration::from_millis(150)).await;
                #[cfg(target_arch = "wasm32")]
                gloo_timers::future::TimeoutFuture::new(150).await;
            }
        }
    });

    // Async Omegon spawn: desktop-only.
    #[cfg(not(target_arch = "wasm32"))]
    use_future(move || {
        let binary = spawning_binary.clone();
        let mut controller = controller;
        let mut event_stream = event_stream;
        #[cfg(not(target_arch = "wasm32"))]
        let mut ipc_event_stream = ipc_event_stream;
        #[cfg(not(target_arch = "wasm32"))]
        let mut command_transport = command_transport;
        async move {
            let Some(binary_str) = binary else { return };
            let binary_path = std::path::PathBuf::from(binary_str);
            let result = crate::bootstrap::spawn_and_attach_omegon(&binary_path).await;
            if let Some(stream) = result.event_stream {
                event_stream.set(Some(stream));
            }
            #[cfg(not(target_arch = "wasm32"))]
            if let Some(stream) = result.ipc_event_stream {
                ipc_event_stream.set(Some(stream));
            }
            command_transport.set(result.command_transport);
            let mut c = result.controller;
            if let Some(note) = result.note {
                c.set_bootstrap_note(Some(note));
            }
            controller.set(c);
        }
    });

    // Auto-scroll the transcript pane to the latest message whenever messages change.
    use_effect(move || {
        let _ = controller.read().messages().len();
        spawn(async move {
            let _ = document::eval(
                r#"
                var transcript = document.querySelector('.cockpit-transcript');
                if (transcript) {
                  transcript.scrollTop = transcript.scrollHeight;
                }
            "#,
            )
            .await;
        });
    });

    let mut audit_session_filter = use_signal(String::new);
    let mut audit_turn_filter = use_signal(String::new);
    let mut audit_kind_filter = use_signal(|| "all".to_string());
    let mut audit_text_filter = use_signal(String::new);
    let mut selected_cockpit_entity = use_signal(|| Option::<SelectedCockpitEntity>::None);
    let mut promoted_cockpit_entity = use_signal(|| Option::<PromotedCockpitEntity>::None);

    #[cfg(not(target_arch = "wasm32"))]
    use_future(move || {
        let mut settings_open = settings_open;
        let mut controller = controller;
        async move {
            loop {
                while let Ok(event) = dioxus::desktop::muda::MenuEvent::receiver().try_recv() {
                    if event.id().as_ref() == SETTINGS_MENU_ID {
                        let _ = controller.write().refresh_settings_auth_status();
                        settings_open.set(true);
                    }
                }
                tokio::time::sleep(std::time::Duration::from_millis(75)).await;
            }
        }
    });

    let readiness = controller.read().operator_readiness();
    let session = controller.read().session_data();
    #[cfg(not(target_arch = "wasm32"))]
    let settings_model = build_settings_panel_model(
        &controller.read(),
        &session,
        Some(controller.read().settings_auth_state()),
    );
    #[cfg(target_arch = "wasm32")]
    let settings_model = build_settings_panel_model(&controller.read(), &session, None);
    let context_status = context_window_label(&session);
    let dispatch_context = build_dispatch_context_strip_model(
        *workspace.read(),
        controller.read().session_mode(),
        controller.read().summary(),
        &session,
        controller.read().composer().draft(),
        controller.read().is_run_active(),
        controller.read().can_submit(),
    );
    let provider_blocked_composer =
        build_provider_blocked_composer_model(&session, controller.read().can_submit());

    let mut layout_debug_snapshot = use_signal(|| None::<LayoutDebugSnapshot>);

    if LAYOUT_DEBUG_ENABLED {
        use_effect(move || {
            spawn(async move {
                let script = r#"
                    JSON.stringify({
                      innerHeight: window.innerHeight,
                      boxes: [
                        '.shell',
                        '.shell-cockpit',
                        '.cockpit-top-rail',
                        '.cockpit-console-shell',
                        '.cockpit-console-side-left .cockpit-panel:first-child'
                      ].map((selector) => {
                        const el = document.querySelector(selector);
                        if (!el) return { selector, top: -1, left: -1, width: -1, height: -1 };
                        const rect = el.getBoundingClientRect();
                        return {
                          selector,
                          top: Math.round(rect.top),
                          left: Math.round(rect.left),
                          width: Math.round(rect.width),
                          height: Math.round(rect.height)
                        };
                      })
                    })
                "#;
                if let Ok(reply) = document::eval(script).await
                    && let Some(raw) = reply.as_str()
                    && let Some(snapshot) = parse_layout_debug_snapshot(raw)
                {
                    layout_debug_snapshot.set(Some(snapshot));
                }
            });
        });
    }

    let cockpit = build_cockpit_summary_model(
        *workspace.read(),
        controller.read().session_mode(),
        controller.read().summary(),
        &session,
    );

    let cockpit_center_body = rsx! {
        if *workspace.read() == Workspace::Graph {
            GraphScreen { data: controller.read().graph_data() }
        } else if *workspace.read() == Workspace::Audit {
            {render_audit_workspace(
                controller.read().audit_timeline(),
                controller.read().current_audit_session_key().as_str(),
                AuditPanelControls {
                    filters: AuditFilters {
                        session_key: audit_session_filter.read().clone(),
                        turn_query: audit_turn_filter.read().clone(),
                        kind_key: audit_kind_filter.read().clone(),
                        text_query: audit_text_filter.read().clone(),
                    },
                    on_session_filter: EventHandler::new(move |value: String| audit_session_filter.set(value)),
                    on_turn_filter: EventHandler::new(move |value: String| audit_turn_filter.set(value)),
                    on_kind_filter: EventHandler::new(move |value: String| audit_kind_filter.set(value)),
                    on_text_filter: EventHandler::new(move |value: String| audit_text_filter.set(value)),
                    on_focus_entry: EventHandler::new(move |target: String| {
                        focus_transcript_target(controller.read().transcript(), &target);
                    }),
                },
            )}
        } else if *workspace.read() == Workspace::Session {
            SessionScreen {
                data: controller.read().session_data(),
                selected_entity: selected_cockpit_entity.read().clone(),
                on_dispatcher_switch: Some(EventHandler::new(move |(profile, model): (String, Option<String>)| {
                    let command = controller.write().request_dispatcher_switch_command(&profile, model.as_deref());
                    #[cfg(not(target_arch = "wasm32"))]
                    if let (Some(command), Some(transport)) = (command, command_transport.read().clone()) {
                        let _ = dispatch_targeted_command(&transport, event_stream.read().as_ref(), &command);
                    }
                    #[cfg(target_arch = "wasm32")]
                    if let (Some(command), Some(stream)) = (command, event_stream.read().clone()) {
                        dispatch_targeted_command(&stream, &command);
                    }
                })),
                on_transcript_focus: Some(EventHandler::new(move |target: String| {
                    focus_transcript_target(controller.read().transcript(), &target);
                }))
            }
        } else if *workspace.read() == Workspace::Scribe {
            ScribeScreen {
                summary: controller.read().summary().clone(),
                data: controller.read().session_data(),
                session_mode: controller.read().session_mode(),
                scenario_key: controller.read().scenario().key().to_string(),
                transcript_auto_expand: controller.read().transcript_auto_expand(),
                on_set_session_mode: Some(EventHandler::new(move |mode: String| controller.write().switch_session_mode(mode.as_str()))),
                on_set_scenario: Some(EventHandler::new(move |scenario: String| controller.write().select_scenario(scenario.as_str()))),
                on_set_transcript_auto_expand: Some(EventHandler::new(move |enabled: bool| controller.write().set_transcript_auto_expand(enabled))),
                on_dispatcher_switch: Some(EventHandler::new(move |(profile, model): (String, Option<String>)| {
                    let command = controller.write().request_dispatcher_switch_command(&profile, model.as_deref());
                    #[cfg(not(target_arch = "wasm32"))]
                    if let (Some(command), Some(transport)) = (command, command_transport.read().clone()) {
                        let _ = dispatch_targeted_command(&transport, event_stream.read().as_ref(), &command);
                    }
                    #[cfg(target_arch = "wasm32")]
                    if let (Some(command), Some(stream)) = (command, event_stream.read().clone()) {
                        dispatch_targeted_command(&stream, &command);
                    }
                })),
                on_transcript_focus: Some(EventHandler::new(move |target: String| {
                    focus_transcript_target(controller.read().transcript(), &target);
                }))
            }
        } else if let Some(promoted) = promoted_cockpit_entity.read().as_ref() {
            match promoted {
                PromotedCockpitEntity::DeploymentInstance(instance_id) => {
                    {render_selected_deployment_cop(
                        &controller.read().session_data(),
                        instance_id,
                        EventHandler::new(move |_| promoted_cockpit_entity.set(None)),
                    )}
                }
                PromotedCockpitEntity::ActivityActor(task_id) => {
                    {render_selected_activity_cop(
                        &controller.read().session_data(),
                        task_id,
                        Some(EventHandler::new(move |target: String| {
                            focus_transcript_target(controller.read().transcript(), &target);
                        })),
                        EventHandler::new(move |_| promoted_cockpit_entity.set(None)),
                    )}
                }
            }
        } else {
            {render_chat_cop_host(
                ChatCopHostModel {
                    summary: controller.read().summary(),
                    work: &controller.read().work_data(),
                    session: &controller.read().session_data(),
                    transcript: controller.read().transcript(),
                    messages: controller.read().messages(),
                    scenario: controller.read().scenario(),
                    auto_expand: controller.read().transcript_auto_expand(),
                    is_run_active: controller.read().is_run_active(),
                    can_submit: controller.read().can_submit(),
                    draft: controller.read().composer().draft(),
                    dispatch_context: &dispatch_context,
                    provider_blocked_composer: provider_blocked_composer.as_ref(),
                    composer_ready_notice: composer_ready_notice.read().as_deref(),
                },
                ChatCopHostActions {
                    on_submit: EventHandler::new(move |event: dioxus::events::FormEvent| {
                        event.prevent_default();
                        let command = controller.write().submit_prompt_command();
                        #[cfg(not(target_arch = "wasm32"))]
                        if let (Some(command), Some(transport)) = (command, command_transport.read().clone()) {
                            let _ = dispatch_targeted_command(&transport, event_stream.read().as_ref(), &command);
                        }
                        #[cfg(target_arch = "wasm32")]
                        if let (Some(command), Some(stream)) = (command, event_stream.read().clone()) {
                            dispatch_targeted_command(&stream, &command);
                        }
                    }),
                    on_update_draft: EventHandler::new(move |value: String| controller.write().update_draft(value)),
                    on_open_settings: EventHandler::new(move |_| {
                        #[cfg(not(target_arch = "wasm32"))]
                        {
                            let _ = controller.write().refresh_settings_auth_status();
                        }
                        settings_open.set(true)
                    }),
                    on_cancel: EventHandler::new(move |_| {
                        #[cfg(not(target_arch = "wasm32"))]
                        if let Some(command) = controller.read().cancel_command() && let Some(transport) = command_transport.read().clone() {
                            let _ = dispatch_targeted_command(&transport, event_stream.read().as_ref(), &command);
                        }
                        #[cfg(target_arch = "wasm32")]
                        if let Some(command) = controller.read().cancel_command() && let Some(stream) = event_stream.read().clone() {
                            dispatch_targeted_command(&stream, &command);
                        }
                    }),
                },
            )}
        }
    };

    let cockpit_center_body_for_blockout = cockpit_center_body.clone();

    rsx! {
        div { class: if SHELL_BLOCKOUT_MODE { "shell shell-cockpit shell-blockout-mode" } else { "shell shell-cockpit" },
            div { class: "cockpit-canvas", "aria-hidden": "true" }

            if LAYOUT_DEBUG_ENABLED {
                if let Some(snapshot) = layout_debug_snapshot.read().as_ref() {
                    div {
                        style: "position: fixed; top: 72px; right: 12px; z-index: 99999; max-width: 420px; background: rgba(120,0,0,0.92); color: white; font: 12px/1.4 ui-monospace, SFMono-Regular, Menlo, monospace; padding: 10px 12px; border: 1px solid rgba(255,255,255,0.25); border-radius: 8px; white-space: pre-wrap;",
                        "innerHeight: {snapshot.inner_height}\n",
                        for item in &snapshot.boxes {
                            "{item.selector}: top={item.top} left={item.left} w={item.width} h={item.height}\n"
                        }
                    }
                }
            }

            if SHELL_BLOCKOUT_MODE {
                {render_cockpit_top_rail(&cockpit, selected_cockpit_entity)}
                div { class: "debug-shell-main",
                    div { class: "cockpit-console-side cockpit-console-side-left debug-shell-left-host",
                        article { class: "cockpit-panel cockpit-panel-auspex cockpit-console-card", "data-surface": "panel", "data-elevation": "1",
                            div { class: "cockpit-panel-toprail",
                                span { class: "cockpit-panel-label", "{cockpit.auspex.label}" }
                                span { class: "cockpit-panel-tag", "{cockpit.auspex.tag}" }
                            }
                            p { class: "cockpit-panel-primary", "{cockpit.auspex.primary}" }
                            for line in &cockpit.auspex.secondary {
                                p { class: "cockpit-panel-secondary", "{line}" }
                            }
                        }

                        article { class: "cockpit-panel cockpit-panel-deployment cockpit-console-card", "data-surface": "panel", "data-elevation": "1",
                            div { class: "cockpit-panel-toprail",
                                span { class: "cockpit-panel-label", "{cockpit.deployment.label}" }
                                span { class: "cockpit-panel-tag", "{cockpit.deployment.tag}" }
                            }
                            p { class: "cockpit-panel-primary", "{cockpit.deployment.primary}" }
                            for line in &cockpit.deployment.secondary {
                                p { class: "cockpit-panel-secondary", "{line}" }
                            }
                            if !cockpit.deployment.preview.is_empty() {
                                div { class: "cockpit-panel-preview-rail",
                                    for item in &cockpit.deployment.preview {
                                        button {
                                            class: if selected_cockpit_entity.read().as_ref() == Some(&SelectedCockpitEntity::DeploymentInstance(item.key.clone())) { "cockpit-panel-preview-chip cockpit-panel-preview-chip-selected" } else { "cockpit-panel-preview-chip" },
                                            r#type: "button",
                                            onclick: {
                                                let key = item.key.clone();
                                                move |_| {
                                                    if selected_cockpit_entity.read().as_ref() == Some(&SelectedCockpitEntity::DeploymentInstance(key.clone())) {
                                                        selected_cockpit_entity.set(None);
                                                    } else {
                                                        selected_cockpit_entity.set(Some(SelectedCockpitEntity::DeploymentInstance(key.clone())));
                                                    }
                                                }
                                            },
                                            "{item.label}"
                                        }
                                    }
                                }
                            }
                        }
                    }
                    div { class: "debug-shell-center-host",
                        {render_cockpit_center_stage(workspace, cockpit_center_body_for_blockout)}
                    }
                    div { class: "debug-shell-right-host",
                        {render_cockpit_sidecar(
                            &session,
                            selected_cockpit_entity.read().clone(),
                            CockpitSidecarActions {
                                on_dispatcher_switch: Some(EventHandler::new(move |(profile, model): (String, Option<String>)| {
                                    let command = controller.write().request_dispatcher_switch_command(&profile, model.as_deref());
                                    #[cfg(not(target_arch = "wasm32"))]
                                    if let (Some(command), Some(transport)) = (command, command_transport.read().clone()) {
                                        let _ = dispatch_targeted_command(&transport, event_stream.read().as_ref(), &command);
                                    }
                                    #[cfg(target_arch = "wasm32")]
                                    if let (Some(command), Some(stream)) = (command, event_stream.read().clone()) {
                                        dispatch_targeted_command(&stream, &command);
                                    }
                                })),
                                on_transcript_focus: Some(EventHandler::new(move |target: String| {
                                    focus_transcript_target(controller.read().transcript(), &target);
                                })),
                                on_promote_selection: Some(EventHandler::new(move |entity| {
                                    promoted_cockpit_entity.set(Some(match entity {
                                        SelectedCockpitEntity::DeploymentInstance(instance_id) => PromotedCockpitEntity::DeploymentInstance(instance_id),
                                        SelectedCockpitEntity::ActivityActor(task_id) => PromotedCockpitEntity::ActivityActor(task_id),
                                    }));
                                })),
                            },
                        )}
                    }
                }
            }

            if !SHELL_BLOCKOUT_MODE {
                {render_cockpit_top_rail(&cockpit, selected_cockpit_entity)}
            }

            if !readiness.ready && !matches!(controller.read().shell_state(), crate::fixtures::ShellState::Failed) {
                div { class: "cockpit-layout cockpit-layout-readiness",
                    section {
                        class: "state-screen state-screen-starting",
                        div { class: "state-screen-icon", "⏳" }
                        h2 { "{readiness.title}" }
                        p { class: "state-screen-detail", "{readiness.detail}" }
                        div { class: "startup-step-list",
                            for step in &readiness.steps {
                                div {
                                    class: "startup-step-row",
                                    "data-state": match step.state {
                                        crate::fixtures::ReadinessStepState::Pending => "pending",
                                        crate::fixtures::ReadinessStepState::Active => "active",
                                        crate::fixtures::ReadinessStepState::Complete => "complete",
                                        crate::fixtures::ReadinessStepState::Blocked => "blocked",
                                    },
                                    strong { class: "startup-step-label", "{step.label}" }
                                    span { class: "startup-step-detail", "{step.detail}" }
                                }
                            }
                        }
                    }
                }
            } else {
                div { class: "cockpit-console-shell",
                    div { class: "cockpit-console-side cockpit-console-side-left",
                        article { class: "cockpit-panel cockpit-panel-auspex cockpit-console-card", "data-surface": "panel", "data-elevation": "1",
                            div { class: "cockpit-panel-toprail",
                                span { class: "cockpit-panel-label", "{cockpit.auspex.label}" }
                                span { class: "cockpit-panel-tag", "{cockpit.auspex.tag}" }
                            }
                            p { class: "cockpit-panel-primary", "{cockpit.auspex.primary}" }
                            for line in &cockpit.auspex.secondary {
                                p { class: "cockpit-panel-secondary", "{line}" }
                            }
                        }

                        article { class: "cockpit-panel cockpit-panel-deployment cockpit-console-card", "data-surface": "panel", "data-elevation": "1",
                            div { class: "cockpit-panel-toprail",
                                span { class: "cockpit-panel-label", "{cockpit.deployment.label}" }
                                span { class: "cockpit-panel-tag", "{cockpit.deployment.tag}" }
                            }
                            p { class: "cockpit-panel-primary", "{cockpit.deployment.primary}" }
                            for line in &cockpit.deployment.secondary {
                                p { class: "cockpit-panel-secondary", "{line}" }
                            }
                            if !cockpit.deployment.preview.is_empty() {
                                div { class: "cockpit-panel-preview-rail",
                                    for item in &cockpit.deployment.preview {
                                        button {
                                            class: if selected_cockpit_entity.read().as_ref() == Some(&SelectedCockpitEntity::DeploymentInstance(item.key.clone())) { "cockpit-panel-preview-chip cockpit-panel-preview-chip-selected" } else { "cockpit-panel-preview-chip" },
                                            r#type: "button",
                                            onclick: {
                                                let key = item.key.clone();
                                                move |_| {
                                                    if selected_cockpit_entity.read().as_ref() == Some(&SelectedCockpitEntity::DeploymentInstance(key.clone())) {
                                                        selected_cockpit_entity.set(None);
                                                    } else {
                                                        selected_cockpit_entity.set(Some(SelectedCockpitEntity::DeploymentInstance(key.clone())));
                                                    }
                                                }
                                            },
                                            "{item.label}"
                                        }
                                    }
                                }
                            }
                        }
                    }

                    div { class: "cockpit-console-center-column",
                        div { class: "cockpit-main-frame",
                            div { class: "cockpit-stage-shell cockpit-stage-shell-console",
                                {render_cockpit_center_stage(workspace, cockpit_center_body)}
                            }
                        }
                    }

                    div { class: "cockpit-console-side cockpit-console-side-right",
                        article { class: "cockpit-panel cockpit-panel-primary-omegon cockpit-console-card", "data-surface": "panel", "data-elevation": "1",
                            div { class: "cockpit-panel-toprail",
                                span { class: "cockpit-panel-label", "{cockpit.attached.label}" }
                                span { class: "cockpit-panel-tag", "{cockpit.attached.tag}" }
                            }
                            p { class: "cockpit-panel-primary", "{cockpit.attached.primary}" }
                            for line in &cockpit.attached.secondary {
                                p { class: "cockpit-panel-secondary", "{line}" }
                            }
                        }

                        article { class: "cockpit-panel cockpit-panel-activity cockpit-console-card", "data-surface": "panel", "data-elevation": "1",
                            div { class: "cockpit-panel-toprail",
                                span { class: "cockpit-panel-label", "{cockpit.activity.label}" }
                                span { class: "cockpit-panel-tag", "{cockpit.activity.tag}" }
                            }
                            p { class: "cockpit-panel-primary", "{cockpit.activity.primary}" }
                            for line in &cockpit.activity.secondary {
                                p { class: "cockpit-panel-secondary", "{line}" }
                            }
                            if !cockpit.activity.preview.is_empty() {
                                div { class: "cockpit-panel-preview-rail",
                                    for item in &cockpit.activity.preview {
                                        button {
                                            class: if selected_cockpit_entity.read().as_ref() == Some(&SelectedCockpitEntity::ActivityActor(item.key.clone())) { "cockpit-panel-preview-chip cockpit-panel-preview-chip-selected" } else { "cockpit-panel-preview-chip" },
                                            r#type: "button",
                                            onclick: {
                                                let key = item.key.clone();
                                                move |_| {
                                                    if selected_cockpit_entity.read().as_ref() == Some(&SelectedCockpitEntity::ActivityActor(key.clone())) {
                                                        selected_cockpit_entity.set(None);
                                                    } else {
                                                        selected_cockpit_entity.set(Some(SelectedCockpitEntity::ActivityActor(key.clone())));
                                                    }
                                                }
                                            },
                                            "{item.label}"
                                        }
                                    }
                                }
                            }
                        }

                        {render_cockpit_sidecar(
                            &controller.read().session_data(),
                            selected_cockpit_entity.read().clone(),
                            CockpitSidecarActions {
                                on_dispatcher_switch: Some(EventHandler::new(move |(profile, model): (String, Option<String>)| {
                                    let command = controller.write().request_dispatcher_switch_command(&profile, model.as_deref());
                                    #[cfg(not(target_arch = "wasm32"))]
                                    if let (Some(command), Some(transport)) = (command, command_transport.read().clone()) {
                                        let _ = dispatch_targeted_command(&transport, event_stream.read().as_ref(), &command);
                                    }
                                    #[cfg(target_arch = "wasm32")]
                                    if let (Some(command), Some(stream)) = (command, event_stream.read().clone()) {
                                        dispatch_targeted_command(&stream, &command);
                                    }
                                })),
                                on_transcript_focus: Some(EventHandler::new(move |target: String| {
                                    focus_transcript_target(controller.read().transcript(), &target);
                                })),
                                on_promote_selection: Some(EventHandler::new(move |selection: crate::app::SelectedCockpitEntity| {
                                    match selection {
                                        crate::app::SelectedCockpitEntity::DeploymentInstance(instance_id) => promoted_cockpit_entity.set(Some(PromotedCockpitEntity::DeploymentInstance(instance_id))),
                                        crate::app::SelectedCockpitEntity::ActivityActor(task_id) => promoted_cockpit_entity.set(Some(PromotedCockpitEntity::ActivityActor(task_id))),
                                    }
                                })),
                            },
                        )}
                    }
                }
            }
            if *settings_open.read() {
                div {
                    class: "settings-modal-backdrop",
                    onclick: move |_| settings_open.set(false),
                    div {
                        class: "settings-modal",
                        "data-surface": "panel",
                        "data-elevation": "1",
                        onclick: move |event| event.stop_propagation(),
                        header { class: "settings-modal-header",
                            div { class: "settings-modal-heading",
                                span { class: "settings-modal-kicker", "Operator controls" }
                                h2 { class: "settings-modal-title", "Settings" }
                                p { class: "settings-modal-detail", "Manage provider auth, secrets exposure, and vault readiness from the same in-app operator workflow that now targets the attached Omegon instance." }
                            }
                            button {
                                class: "settings-modal-close",
                                r#type: "button",
                                onclick: move |_| settings_open.set(false),
                                "Close"
                            }
                        }

                        div { class: "settings-modal-grid",
                            section { class: "settings-panel-card settings-panel-card-target",
                                h3 { class: "settings-panel-title", "Command target" }
                                div { class: "settings-target-chip",
                                    span { class: "settings-target-label", "Target" }
                                    span { class: "settings-target-value", "{settings_model.target_label}" }
                                }
                                p { class: "settings-panel-detail", "{settings_model.target_detail}" }
                                p { class: "settings-panel-detail", "{settings_model.route_detail}" }
                                p { class: "settings-panel-detail", "{settings_model.lifecycle_summary}" }
                                div { class: "settings-route-list",
                                    for (route_id, label, detail) in &settings_model.route_options {
                                        button {
                                            class: "settings-action-button settings-route-button",
                                            r#type: "button",
                                            disabled: route_id == &settings_model.selected_route_id,
                                            title: detail.clone(),
                                            onclick: {
                                                let mut controller = controller;
                                                let route_id = route_id.clone();
                                                move |_| controller.write().select_command_route(&route_id)
                                            },
                                            span { class: "settings-route-button-label", "{label}" }
                                            span { class: "settings-route-button-detail", "{detail}" }
                                        }
                                    }
                                }
                            }

                            section { class: "settings-panel-card settings-panel-card-auth",
                                h3 { class: "settings-panel-title", "Provider auth" }
                                if settings_model.provider_ready {
                                    div { class: "settings-ready-notice",
                                        strong { class: "settings-ready-title", "Providers are ready" }
                                        p { class: "settings-ready-detail", "At least one authenticated provider is available. You can return to Chat and start prompting now." }
                                        button {
                                            class: "settings-ready-action",
                                            r#type: "button",
                                            onclick: move |_| {
                                                settings_open.set(false);
                                                workspace.set(Workspace::Chat);
                                            },
                                            "Return to Chat"
                                        }
                                    }
                                }
                                p { class: "settings-auth-status", "{settings_model.auth_status_label}" }
                                p { class: "settings-panel-detail", "{settings_model.auth_status_detail}" }
                                p { class: "settings-panel-detail", "{settings_model.provider_guidance}" }
                                if let Some(last_action) = settings_model.last_action.as_deref() {
                                    p { class: "settings-panel-detail", "Last action: {last_action}" }
                                }
                                if let Some(error) = settings_model.last_error.as_deref() {
                                    p { class: "settings-panel-detail settings-panel-detail-error", "Last error: {error}" }
                                }
                                div { class: "settings-provider-card-list",
                                    for provider_card in &settings_model.provider_cards {
                                        article {
                                            class: "settings-provider-card",
                                            "data-provider": provider_card.provider_key.clone().unwrap_or_else(|| "unreported".into()),
                                            h4 { class: "settings-provider-name", "{provider_card.name}" }
                                            p { class: "settings-provider-status", "{provider_card.status_label}" }
                                            p { class: "settings-provider-detail", "{provider_card.status_detail}" }
                                            p { class: "settings-provider-detail", "{provider_card.capability_detail}" }
                                            p { class: "settings-provider-guidance", "{provider_card.guidance}" }
                                            div { class: "settings-provider-action-row",
                                                button {
                                                    class: "settings-action-button settings-provider-action-button",
                                                    r#type: "button",
                                                    disabled: !provider_card.login_action.enabled,
                                                    title: provider_card.login_action.detail.clone(),
                                                    "data-command": provider_card.login_action.action.command_slug(),
                                                    "data-provider": provider_card.provider_key.clone().unwrap_or_default(),
                                                    onclick: {
                                                        let mut controller = controller;
                                                        #[cfg(not(target_arch = "wasm32"))]
                                                        let mut settings_status_message = settings_status_message;
                                                        let action_kind = provider_card.login_action.action;
                                                        let provider = provider_card.login_action.provider.clone();
                                                        let target_label = settings_model.target_label.clone();
                                                        move |_| {
                                                            #[cfg(not(target_arch = "wasm32"))]
                                                            {
                                                                let result = match action_kind {
                                                                    SettingsAuthAction::Refresh => controller.write().refresh_settings_auth_status(),
                                                                    SettingsAuthAction::Login => {
                                                                        if let Some(transport) = command_transport.read().clone() {
                                                                            let provider_name = provider.clone().unwrap_or_else(|| "anthropic".into());
                                                                            let slash = crate::runtime_types::CanonicalSlashCommand {
                                                                                name: "login".into(),
                                                                                args: provider_name.clone(),
                                                                                raw_input: format!("/login {provider_name}"),
                                                                            };
                                                                            let target = controller.read().current_command_target();
                                                                            let command = crate::runtime_types::TargetedCommand::canonical_slash(target, slash);
                                                                            dispatch_targeted_command(&transport, event_stream.read().as_ref(), &command)
                                                                        } else {
                                                                            controller.write().run_settings_auth_action(
                                                                                crate::bootstrap::DesktopAuthAction::Login,
                                                                                provider.as_deref(),
                                                                            )
                                                                        }
                                                                    }
                                                                    SettingsAuthAction::Logout => unreachable!(),
                                                                    SettingsAuthAction::Unlock => unreachable!(),
                                                                };
                                                                let message = match result {
                                                                    Ok(()) => format!("{} dispatched for {}", action_kind.label(), target_label),
                                                                    Err(error) => format!("{} failed: {}", action_kind.label(), error),
                                                                };
                                                                settings_status_message.set(Some(message));
                                                            }
                                                        }
                                                    },
                                                    "Browser login"
                                                }
                                                button {
                                                    class: "settings-action-button settings-provider-action-button settings-provider-action-button-secondary",
                                                    r#type: "button",
                                                    disabled: !provider_card.logout_action.enabled,
                                                    title: provider_card.logout_action.detail.clone(),
                                                    "data-command": provider_card.logout_action.action.command_slug(),
                                                    "data-provider": provider_card.provider_key.clone().unwrap_or_default(),
                                                    onclick: {
                                                        let mut controller = controller;
                                                        #[cfg(not(target_arch = "wasm32"))]
                                                        let mut settings_status_message = settings_status_message;
                                                        let action_kind = provider_card.logout_action.action;
                                                        let provider = provider_card.logout_action.provider.clone();
                                                        let target_label = settings_model.target_label.clone();
                                                        move |_| {
                                                            #[cfg(not(target_arch = "wasm32"))]
                                                            {
                                                                let result = match action_kind {
                                                                    SettingsAuthAction::Refresh => controller.write().refresh_settings_auth_status(),
                                                                    SettingsAuthAction::Login => unreachable!(),
                                                                    SettingsAuthAction::Logout => {
                                                                        if let Some(transport) = command_transport.read().clone() {
                                                                            let provider_name = provider.clone().unwrap_or_default();
                                                                            let raw_input = if provider_name.is_empty() {
                                                                                "/logout".to_string()
                                                                            } else {
                                                                                format!("/logout {provider_name}")
                                                                            };
                                                                            let slash = crate::runtime_types::CanonicalSlashCommand {
                                                                                name: "logout".into(),
                                                                                args: provider_name,
                                                                                raw_input,
                                                                            };
                                                                            let target = controller.read().current_command_target();
                                                                            let command = crate::runtime_types::TargetedCommand::canonical_slash(target, slash);
                                                                            dispatch_targeted_command(&transport, event_stream.read().as_ref(), &command)
                                                                        } else {
                                                                            controller.write().run_settings_auth_action(
                                                                                crate::bootstrap::DesktopAuthAction::Logout,
                                                                                provider.as_deref(),
                                                                            )
                                                                        }
                                                                    }
                                                                    SettingsAuthAction::Unlock => unreachable!(),
                                                                };
                                                                let message = match result {
                                                                    Ok(()) => format!("{} dispatched for {}", action_kind.label(), target_label),
                                                                    Err(error) => format!("{} failed: {}", action_kind.label(), error),
                                                                };
                                                                settings_status_message.set(Some(message));
                                                            }
                                                        }
                                                    },
                                                    "Logout"
                                                }
                                            }
                                        }
                                    }
                                }
                            }

                            section { class: "settings-panel-card settings-panel-card-status",
                                h3 { class: "settings-panel-title", "Secrets and vault" }
                                p { class: "settings-panel-detail", "Status parity here is derived from the same attached-session snapshot as the rest of the operator UI, so secret and vault hints stay tied to the current route and target." }
                                div { class: "settings-status-section",
                                    h4 { class: "settings-status-section-title", "Secrets" }
                                    div { class: "settings-status-list",
                                        for row in &settings_model.secrets_rows {
                                            div { class: "settings-status-row",
                                                span { class: "settings-status-label", "{row.label}" }
                                                span { class: "settings-status-value", "{row.value}" }
                                            }
                                        }
                                    }
                                }
                                div { class: "settings-status-section",
                                    h4 { class: "settings-status-section-title", "Vault" }
                                    div { class: "settings-status-list",
                                        for row in &settings_model.vault_rows {
                                            div { class: "settings-status-row",
                                                span { class: "settings-status-label", "{row.label}" }
                                                span { class: "settings-status-value", "{row.value}" }
                                            }
                                        }
                                    }
                                }
                            }

                            section { class: "settings-panel-card settings-panel-card-actions",
                                h3 { class: "settings-panel-title", "Operator actions" }
                                p { class: "settings-panel-detail", "Refresh provider state after browser auth, or unlock the host vault/keyring flow when secrets are present but not usable." }
                                if let Some(message) = settings_status_message.read().as_deref() {
                                    p { class: "settings-panel-detail settings-operator-message", "{message}" }
                                }
                                div { class: "settings-action-grid",
                                    for action in &settings_model.general_actions {
                                        button {
                                            class: "settings-action-button settings-general-action-button",
                                            r#type: "button",
                                            disabled: !action.enabled,
                                            title: action.detail.clone(),
                                            "data-command": action.action.command_slug(),
                                            "data-target": settings_model.target_label.clone(),
                                            onclick: {
                                                let mut controller = controller;
                                                #[cfg(not(target_arch = "wasm32"))]
                                                let mut settings_status_message = settings_status_message;
                                                let action_kind = action.action;
                                                let provider = action.provider.clone();
                                                let target_label = settings_model.target_label.clone();
                                                move |_| {
                                                    #[cfg(not(target_arch = "wasm32"))]
                                                    {
                                                        let result = match action_kind {
                                                            SettingsAuthAction::Refresh => controller.write().refresh_settings_auth_status(),
                                                            SettingsAuthAction::Login => unreachable!(),
                                                            SettingsAuthAction::Logout => unreachable!(),
                                                            SettingsAuthAction::Unlock => {
                                                                if let Some(transport) = command_transport.read().clone() {
                                                                    let slash = crate::runtime_types::CanonicalSlashCommand {
                                                                        name: "auth".into(),
                                                                        args: "unlock".into(),
                                                                        raw_input: "/auth unlock".into(),
                                                                    };
                                                                    let target = controller.read().current_command_target();
                                                                    let command = crate::runtime_types::TargetedCommand::canonical_slash(target, slash);
                                                                    dispatch_targeted_command(&transport, event_stream.read().as_ref(), &command)
                                                                } else {
                                                                    controller.write().run_settings_auth_action(
                                                                        crate::bootstrap::DesktopAuthAction::Unlock,
                                                                        provider.as_deref(),
                                                                    )
                                                                }
                                                            }
                                                        };
                                                        let message = match result {
                                                            Ok(()) => format!("{} dispatched for {}", action_kind.label(), target_label),
                                                            Err(error) => format!("{} failed: {}", action_kind.label(), error),
                                                        };
                                                        settings_status_message.set(Some(message));
                                                    }
                                                }
                                            },
                                            "{action.action.label()}"
                                        }
                                    }
                                }
                                ul { class: "settings-action-notes",
                                    for action in &settings_model.general_actions {
                                        li {
                                            strong { "{action.action.label()}" }
                                            span { " — {action.detail}" }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }

            // ── Bottom bar ──────────────────────────────────────────────
            footer { class: "bottombar",
                // Bottom-left corner box — org/operator identity
                div { class: "bottombar-org",
                    span { class: "bottombar-label", "Operator" }
                }

                // Bottom-center — instrumentation
                div { class: "bottombar-instruments",
                    span { class: "instrument", "{controller.read().summary().connection}" }
                    span { class: "instrument", "{context_status}" }
                }

                // Bottom-right corner box — reserved aperture
                div { class: "bottombar-reserved" }
            }
        }
    }
}

fn focus_transcript_target(transcript: &TranscriptData, target: &str) {
    if let Some(anchor) = find_transcript_anchor(transcript, target) {
        let anchor_json = serde_json::to_string(&anchor).unwrap_or_else(|_| "\"\"".to_string());
        spawn(async move {
            let _ = document::eval(&format!(
                r#"
                (function() {{
                  var anchor = {anchor_json};
                  if (!anchor) return;
                  var match = document.getElementById(anchor);
                  if (match) {{
                    match.scrollIntoView({{ behavior: 'instant', block: 'center' }});
                    match.classList.add('transcript-focus-hit');
                    setTimeout(function() {{ match.classList.remove('transcript-focus-hit'); }}, 1400);
                  }}
                }})();
                "#
            ))
            .await;
        });
    }
}

fn find_transcript_anchor(transcript: &TranscriptData, target: &str) -> Option<String> {
    for turn in &transcript.turns {
        for (block_index, block) in turn.blocks.iter().enumerate() {
            if transcript_block_matches_target(block, target) {
                return Some(transcript_block_dom_id(turn.number, block_index));
            }
        }
    }
    None
}

fn transcript_block_dom_id(turn_number: u32, block_index: usize) -> String {
    format!("turn-{turn_number}-block-{block_index}")
}

fn transcript_block_matches_target(block: &crate::fixtures::TurnBlock, target: &str) -> bool {
    if let Some(task_id) = target.strip_prefix("delegate:") {
        return match block {
            crate::fixtures::TurnBlock::Text(text) | crate::fixtures::TurnBlock::System(text) => {
                block_origin_label(text.origin.as_ref())
                    .is_some_and(|label| label.contains(task_id))
                    || text.text.contains(task_id)
            }
            _ => false,
        };
    }

    if let Some(token) = target.strip_prefix("dispatcher-switch:") {
        return match block {
            crate::fixtures::TurnBlock::System(text) => {
                text.notice_kind == Some(crate::fixtures::SystemNoticeKind::DispatcherSwitch)
                    && (token.is_empty() || text.text.contains(token))
            }
            _ => false,
        };
    }

    transcript_block_search_text(block).contains(target)
}

fn transcript_block_search_text(block: &crate::fixtures::TurnBlock) -> String {
    match block {
        crate::fixtures::TurnBlock::Thinking(thinking) => thinking.text.clone(),
        crate::fixtures::TurnBlock::Text(text) | crate::fixtures::TurnBlock::System(text) => {
            format!(
                "{} {}",
                block_origin_label(text.origin.as_ref()).unwrap_or_default(),
                text.text
            )
        }
        crate::fixtures::TurnBlock::Tool(tool) => format!(
            "{} {} {} {} {}",
            tool.id,
            tool.name,
            tool.args,
            tool.partial_output,
            tool.result.clone().unwrap_or_default()
        ),
        crate::fixtures::TurnBlock::Aborted(text) => text.clone(),
    }
}

fn block_origin_label(origin: Option<&crate::fixtures::BlockOrigin>) -> Option<&str> {
    origin.map(|origin| origin.label.as_str())
}

const TRANSCRIPT_DISCLOSURE_LINE_THRESHOLD: usize = 7;
const TRANSCRIPT_DISCLOSURE_CHAR_THRESHOLD: usize = 360;
const SYSTEM_NOTICE_DISCLOSURE_LINE_THRESHOLD: usize = 5;
const SYSTEM_NOTICE_DISCLOSURE_CHAR_THRESHOLD: usize = 220;
const STRUCTURED_PAYLOAD_PREFIXES: [&str; 8] = ["{", "[", "(", "<", "---", "diff --", "@@", "{"];

fn transcript_disclosure_meta(content: &str) -> String {
    let line_count = content.lines().count().max(1);
    format!(
        "{line_count} line{} · {} chars",
        if line_count == 1 { "" } else { "s" },
        content.chars().count()
    )
}

fn should_expand_tool_args(content: &str) -> bool {
    should_expand_tool_payload(content)
}

fn should_expand_tool_output(content: &str) -> bool {
    should_expand_tool_payload(content)
}

fn should_expand_system_notice(content: &str) -> bool {
    should_expand_transcript_content(
        content,
        SYSTEM_NOTICE_DISCLOSURE_LINE_THRESHOLD,
        SYSTEM_NOTICE_DISCLOSURE_CHAR_THRESHOLD,
    )
}

fn should_expand_tool_payload(content: &str) -> bool {
    !looks_like_structured_payload(content)
        && !should_expand_transcript_content(
            content,
            TRANSCRIPT_DISCLOSURE_LINE_THRESHOLD,
            TRANSCRIPT_DISCLOSURE_CHAR_THRESHOLD,
        )
}

fn looks_like_structured_payload(content: &str) -> bool {
    let trimmed = content.trim_start();
    if trimmed.is_empty() {
        return false;
    }

    if STRUCTURED_PAYLOAD_PREFIXES
        .iter()
        .any(|prefix| trimmed.starts_with(prefix))
    {
        return true;
    }

    let first_line = trimmed.lines().next().unwrap_or_default();
    first_line.contains(": ")
        && (first_line.contains('{')
            || first_line.contains('[')
            || first_line.contains("=>")
            || first_line.contains("::"))
}

fn should_expand_transcript_content(
    content: &str,
    line_threshold: usize,
    char_threshold: usize,
) -> bool {
    content.lines().count() > line_threshold || content.chars().count() > char_threshold
}

fn system_notice_summary_label(text: &crate::fixtures::AttributedText) -> &'static str {
    match text.notice_kind {
        Some(crate::fixtures::SystemNoticeKind::DispatcherSwitch) => "Dispatcher switch notice",
        Some(crate::fixtures::SystemNoticeKind::CleaveStart) => "Cleave start notice",
        Some(crate::fixtures::SystemNoticeKind::CleaveComplete) => "Cleave completion notice",
        Some(crate::fixtures::SystemNoticeKind::ChildStatus) => "Child status notice",
        Some(crate::fixtures::SystemNoticeKind::Failure) => "Failure notice",
        Some(crate::fixtures::SystemNoticeKind::Generic) | None => "System notice",
    }
}

struct TranscriptDisclosure<'a> {
    section_class: &'static str,
    details_class: &'static str,
    summary_class: &'static str,
    summary_label_class: &'static str,
    summary_meta_class: Option<&'static str>,
    body_class: &'static str,
    content_class: &'static str,
    label: &'static str,
    content: &'a str,
    meta: String,
    open: bool,
    copy_label: &'static str,
}

fn render_transcript_disclosure(disclosure: TranscriptDisclosure<'_>) -> Element {
    let TranscriptDisclosure {
        section_class,
        details_class,
        summary_class,
        summary_label_class,
        summary_meta_class,
        body_class,
        content_class,
        label,
        content,
        meta,
        open,
        copy_label,
    } = disclosure;

    rsx! {
        div { class: section_class,
            details {
                class: details_class,
                open,
                summary { class: summary_class,
                    span { class: summary_label_class, "{label}" }
                    if let Some(summary_meta_class) = summary_meta_class {
                        span { class: summary_meta_class, "{meta}" }
                    }
                }
                div { class: body_class,
                    div { class: "transcript-detail-actions",
                        button {
                            id: format!("copy-{}", label.to_lowercase().replace(' ', "-")),
                            class: "transcript-copy-button",
                            r#type: "button",
                            "data-copy-label": format!("Copy {copy_label}"),
                            onclick: {
                                let content = content.to_string();
                                let button_id = format!("copy-{}", label.to_lowercase().replace(' ', "-"));
                                move |_| copy_to_clipboard(&content, &button_id)
                            },
                            "Copy {copy_label}"
                        }
                    }
                    p { class: content_class, "{content}" }
                }
            }
        }
    }
}

fn copy_to_clipboard(text: &str, button_id: &str) {
    let text_json = serde_json::to_string(text).unwrap_or_else(|_| "\"\"".to_string());
    let button_id_json = serde_json::to_string(button_id).unwrap_or_else(|_| "\"\"".to_string());
    spawn(async move {
        let _ = document::eval(&format!(
            r#"
            (async function() {{
              var text = {text_json};
              var buttonId = {button_id_json};
              if (!text) return;
              var copied = false;
              if (navigator.clipboard && navigator.clipboard.writeText) {{
                try {{
                  await navigator.clipboard.writeText(text);
                  copied = true;
                }} catch (_) {{}}
              }}
              if (!copied) {{
                var area = document.createElement('textarea');
                area.value = text;
                area.setAttribute('readonly', 'readonly');
                area.style.position = 'fixed';
                area.style.opacity = '0';
                document.body.appendChild(area);
                area.select();
                copied = document.execCommand('copy');
                document.body.removeChild(area);
              }}
              if (copied && buttonId) {{
                var button = document.getElementById(buttonId);
                if (button) {{
                  var original = button.getAttribute('data-copy-label') || button.textContent || 'Copy';
                  button.textContent = 'Copied';
                  button.classList.add('transcript-copy-button-copied');
                  setTimeout(function() {{
                    button.textContent = original;
                    button.classList.remove('transcript-copy-button-copied');
                  }}, 1400);
                }}
              }}
            }})();
            "#
        ))
        .await;
    });
}

fn transcript_disclosure_open(open_by_default: bool, auto_expand: bool) -> bool {
    auto_expand && open_by_default
}

fn render_transcript(
    summary: &crate::fixtures::HostSessionSummary,
    work: &crate::fixtures::WorkData,
    session: &crate::fixtures::SessionData,
    transcript: &TranscriptData,
    messages: &[crate::fixtures::ChatMessage],
    auto_expand: bool,
    scenario: crate::fixtures::DevScenario,
) -> Element {
    if let Some(empty_state) =
        build_chat_empty_state_model(summary, work, session, transcript, messages, scenario)
    {
        rsx! {
            section {
                class: "chat-empty-state",
                "data-surface": "panel",
                "data-tone": if empty_state.detached { "warn" } else { "info" },
                span { class: "chat-empty-kicker", "{empty_state.kicker}" }
                h2 { "{empty_state.title}" }
                p { class: "chat-empty-detail", "{empty_state.detail}" }
                ul { class: "chat-empty-list",
                    for item in &empty_state.guidance {
                        li { "{item}" }
                    }
                }
            }
            for message in messages.iter() {
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
    } else if transcript.turns.is_empty() {
        rsx! {
            for message in messages.iter() {
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
    } else {
        rsx! {
            for turn in &transcript.turns {
                article {
                    class: "turn-card",
                    id: format!("turn-{}", turn.number),
                    "data-surface": "panel",
                    "data-elevation": "1",
                    h2 { class: "turn-title", "Turn {turn.number}" }
                    for (block_index, block) in turn.blocks.iter().enumerate() {
                        match block {
                            crate::fixtures::TurnBlock::Thinking(thinking) => rsx! {
                                section {
                                    id: transcript_block_dom_id(turn.number, block_index),
                                    class: if thinking.collapsed { "block block-thinking block-collapsed" } else { "block block-thinking" },
                                    "data-surface": "panel",
                                    "data-tone": "muted",
                                    h3 { "Thinking" }
                                    p { "{thinking.text}" }
                                }
                            },
                            crate::fixtures::TurnBlock::Text(text) => rsx! {
                                section {
                                    id: transcript_block_dom_id(turn.number, block_index),
                                    class: text_block_class(text.origin.as_ref()),
                                    "data-surface": "panel",
                                    "data-tone": text_block_tone(text.origin.as_ref()),
                                    if let Some(origin) = &text.origin {
                                        h3 { class: origin_class(origin), "{origin.label}" }
                                    }
                                    p { "{text.text}" }
                                }
                            },
                            crate::fixtures::TurnBlock::Tool(tool) => rsx! {
                                section {
                                    id: transcript_block_dom_id(turn.number, block_index),
                                    class: tool_block_class(tool),
                                    "data-surface": "panel",
                                    "data-kind": "tool",
                                    "data-state": tool_visual_state(tool),
                                    "data-tone": tool_block_tone(tool),
                                    div { class: "tool-header",
                                        div { class: "tool-header-main",
                                            if let Some(origin) = &tool.origin {
                                                h3 { class: origin_class(origin), "{origin.label}" }
                                            }
                                            h3 { class: "tool-name", "{tool.name}" }
                                        }
                                        span { class: tool_status_class(tool), "{tool_status_label(tool)}" }
                                    }
                                    if !tool.args.is_empty() {
                                        {render_transcript_disclosure(TranscriptDisclosure {
                                            section_class: "tool-section",
                                            details_class: "tool-details",
                                            summary_class: "tool-summary",
                                            summary_label_class: "tool-summary-label",
                                            summary_meta_class: Some("tool-summary-meta"),
                                            body_class: "tool-detail-body",
                                            content_class: "tool-args",
                                            label: "Args",
                                            content: &tool.args,
                                            meta: transcript_disclosure_meta(&tool.args),
                                            open: transcript_disclosure_open(
                                                should_expand_tool_args(&tool.args),
                                                auto_expand,
                                            ),
                                            copy_label: "args",
                                        })}
                                    }
                                    if !tool.partial_output.is_empty() {
                                        {render_transcript_disclosure(TranscriptDisclosure {
                                            section_class: "tool-section tool-section-stream",
                                            details_class: "tool-details",
                                            summary_class: "tool-summary",
                                            summary_label_class: "tool-summary-label",
                                            summary_meta_class: Some("tool-summary-meta"),
                                            body_class: "tool-detail-body",
                                            content_class: "tool-partial",
                                            label: tool_partial_label(tool),
                                            content: &tool.partial_output,
                                            meta: transcript_disclosure_meta(&tool.partial_output),
                                            open: transcript_disclosure_open(
                                                should_expand_tool_output(&tool.partial_output),
                                                auto_expand,
                                            ),
                                            copy_label: "output",
                                        })}
                                    }
                                    if let Some(result) = &tool.result {
                                        {render_transcript_disclosure(TranscriptDisclosure {
                                            section_class: "tool-section tool-section-result",
                                            details_class: "tool-details",
                                            summary_class: "tool-summary",
                                            summary_label_class: "tool-summary-label",
                                            summary_meta_class: Some("tool-summary-meta"),
                                            body_class: "tool-detail-body",
                                            content_class: "tool-result",
                                            label: tool_result_label(tool),
                                            content: result,
                                            meta: transcript_disclosure_meta(result),
                                            open: transcript_disclosure_open(
                                                should_expand_tool_output(result),
                                                auto_expand,
                                            ),
                                            copy_label: "result",
                                        })}
                                    } else if !tool.is_error {
                                        p { class: "tool-awaiting", "Waiting for final tool result…" }
                                    }
                                }
                            },
                            crate::fixtures::TurnBlock::System(text) => rsx! {
                                section {
                                    id: transcript_block_dom_id(turn.number, block_index),
                                    class: system_block_class(text),
                                    "data-surface": "panel",
                                    "data-tone": system_block_tone(text),
                                    if let Some(origin) = &text.origin {
                                        h3 { class: origin_class(origin), "{origin.label}" }
                                    }
                                    if should_expand_system_notice(&text.text) {
                                        {render_transcript_disclosure(TranscriptDisclosure {
                                            section_class: "system-section",
                                            details_class: "system-details",
                                            summary_class: "system-summary",
                                            summary_label_class: "system-summary-label",
                                            summary_meta_class: Some("system-summary-meta"),
                                            body_class: "system-detail-body",
                                            content_class: "system-text",
                                            label: system_notice_summary_label(text),
                                            content: &text.text,
                                            meta: transcript_disclosure_meta(&text.text),
                                            open: transcript_disclosure_open(true, auto_expand),
                                            copy_label: "notice",
                                        })}
                                    } else {
                                        p { class: "system-text", "{text.text}" }
                                    }
                                }
                            },
                            crate::fixtures::TurnBlock::Aborted(text) => rsx! {
                                section {
                                    id: transcript_block_dom_id(turn.number, block_index),
                                    class: "block block-aborted",
                                    "data-surface": "panel",
                                    "data-tone": "danger",
                                    p { "{text}" }
                                }
                            },
                        }
                    }
                }
            }
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct ChatEmptyStateModel {
    kicker: String,
    title: String,
    detail: String,
    guidance: Vec<String>,
    detached: bool,
}

fn build_chat_empty_state_model(
    summary: &crate::fixtures::HostSessionSummary,
    work: &crate::fixtures::WorkData,
    session: &crate::fixtures::SessionData,
    transcript: &TranscriptData,
    messages: &[crate::fixtures::ChatMessage],
    scenario: crate::fixtures::DevScenario,
) -> Option<ChatEmptyStateModel> {
    if !transcript.turns.is_empty() {
        return None;
    }

    let allow_starter_guidance = matches!(
        scenario,
        crate::fixtures::DevScenario::Ready | crate::fixtures::DevScenario::LocalDevQuiet
    );
    if !allow_starter_guidance {
        return None;
    }

    let is_starter_state = messages.len() <= 2 || session.git_detached;
    if !is_starter_state {
        return None;
    }

    let branch = session.git_branch.as_deref().unwrap_or("detached");
    let work_title = work
        .focused_title
        .as_deref()
        .filter(|value| !value.is_empty())
        .unwrap_or(summary.work.as_str());
    let dispatcher_target = session
        .dispatcher_binding
        .as_ref()
        .and_then(|binding| binding.expected_model.as_deref())
        .unwrap_or("current dispatcher model");
    let session_label = session
        .dispatcher_binding
        .as_ref()
        .map(|binding| binding.session_id.as_str())
        .filter(|value| !value.is_empty())
        .unwrap_or("local session");

    let (kicker, title, detail) = if session.git_detached {
        (
            format!("Detached workspace · {branch}"),
            "New Project starter".into(),
            format!(
                "Auspex is attached to {session_label}, but the workspace is detached from {branch}. Re-anchor the branch or state the project goal before the first run so dispatch stays grounded."
            ),
        )
    } else {
        (
            format!("{session_label} · {branch}"),
            "New Project starter".into(),
            format!(
                "No transcript history is attached yet. Start with the smallest directive that establishes project intent, validates dispatcher posture, or confirms the next work item around {work_title}."
            ),
        )
    };

    let mut guidance = vec![format!(
        "Summarize the current session, branch, and work focus around {work_title}."
    )];
    guidance.push(format!(
        "Confirm whether {dispatcher_target} is the right model before starting implementation."
    ));
    if session.git_detached {
        guidance.push(format!(
            "Explain whether to reattach the workspace or continue detached from {branch}."
        ));
    } else {
        guidance.push(format!(
            "Plan the first concrete step that advances {work_title} without over-scoping the run."
        ));
    }

    Some(ChatEmptyStateModel {
        kicker,
        title,
        detail,
        guidance,
        detached: session.git_detached,
    })
}

fn render_chat_status_banner(
    summary: &crate::fixtures::HostSessionSummary,
    session: &crate::fixtures::SessionData,
    is_run_active: bool,
    can_submit: bool,
) -> Element {
    let provider_ready = session
        .providers
        .iter()
        .any(|provider| provider.authenticated);
    let no_work_yet = summary.work.trim() == "No focused work item";
    let (banner_class, banner_state, label, detail) = if is_run_active {
        (
            "chat-status-banner chat-status-banner-running",
            "running",
            "Run active",
            "Omegon is working. New input is paused until the current run completes or you cancel it.",
        )
    } else if !can_submit && !provider_ready {
        (
            "chat-status-banner chat-status-banner-blocked",
            "blocked",
            "Prompt execution blocked",
            "No authenticated providers are available for prompt execution. Open Settings to authenticate a runnable backend.",
        )
    } else if !can_submit {
        (
            "chat-status-banner chat-status-banner-blocked",
            "blocked",
            "Input paused",
            "Conversation input is unavailable in the current host state.",
        )
    } else if no_work_yet && summary.activity_kind == crate::fixtures::ActivityKind::Idle {
        (
            "chat-status-banner",
            "ready",
            "Ready",
            "Host attached. No focused work item reported yet.",
        )
    } else {
        (
            "chat-status-banner",
            "ready",
            "Ready",
            summary.activity.as_str(),
        )
    };

    let activity_kind = summary.activity_kind.label();

    rsx! {
        section {
            class: banner_class,
            "data-surface": "banner",
            "data-state": banner_state,
            "data-tone": chat_status_tone(is_run_active, can_submit),
            "data-activity-kind": activity_kind,
            title: "Activity: {activity_kind}",
            strong { class: "chat-status-label", "{label}" }
            span { class: "chat-status-detail", "{detail}" }
        }
    }
}

#[allow(dead_code)]
fn render_compact_right_rail(session: &crate::fixtures::SessionData) -> Element {
    rsx! {
        section { class: "screen-section right-rail-compact-section",
            h2 { class: "screen-section-title", "Harness" }
            div { class: "kv-grid",
                if let Some(branch) = session.git_branch.as_deref() {
                    div { class: "kv-row", span { class: "kv-key", "Branch" } span { class: "kv-value", "{branch}" } }
                }
                if !session.thinking_level.trim().is_empty() {
                    div { class: "kv-row", span { class: "kv-key", "Thinking" } span { class: "kv-value", "{session.thinking_level}" } }
                }
                if !session.capability_tier.trim().is_empty() {
                    div { class: "kv-row", span { class: "kv-key", "Tier" } span { class: "kv-value", "{session.capability_tier}" } }
                }
                if !session.memory_available {
                    div { class: "kv-row", span { class: "kv-key", "Memory" } span { class: "kv-value", "not available" } }
                }
                if !session.cleave_available {
                    div { class: "kv-row", span { class: "kv-key", "Cleave" } span { class: "kv-value", "not available" } }
                }
            }
        }

        section { class: "screen-section right-rail-compact-section",
            h2 { class: "screen-section-title", "Providers" }
            if session.providers.is_empty() {
                p { class: "screen-empty", "No provider inventory loaded yet." }
            } else {
                div { class: "kv-grid",
                    for provider in &session.providers {
                        div { class: "kv-row",
                            span { class: "kv-key", "{provider.name}" }
                            span { class: "kv-value",
                                if provider.authenticated {
                                    "authenticated"
                                } else {
                                    "not authenticated"
                                }
                                if !provider.authenticated { " ⚠" }
                            }
                        }
                    }
                }
            }
        }

        section { class: "screen-section right-rail-compact-section",
            h2 { class: "screen-section-title", "Session stats" }
            div { class: "kv-grid",
                div { class: "kv-row", span { class: "kv-key", "Turns" } span { class: "kv-value", "{session.session_turns}" } }
                div { class: "kv-row", span { class: "kv-key", "Tool calls" } span { class: "kv-value", "{session.session_tool_calls}" } }
                div { class: "kv-row", span { class: "kv-key", "Compactions" } span { class: "kv-value", "{session.session_compactions}" } }
            }
        }
    }
}

#[allow(dead_code)]
fn app_surface_surface(kind: crate::fixtures::AppSurfaceKind) -> &'static str {
    match kind {
        crate::fixtures::AppSurfaceKind::Startup => "panel",
        crate::fixtures::AppSurfaceKind::Reconnecting => "banner",
        crate::fixtures::AppSurfaceKind::StartupFailure
        | crate::fixtures::AppSurfaceKind::CompatibilityFailure => "panel",
        crate::fixtures::AppSurfaceKind::BootstrapNote => "panel",
    }
}

#[allow(dead_code)]
fn app_surface_state(kind: crate::fixtures::AppSurfaceKind) -> &'static str {
    match kind {
        crate::fixtures::AppSurfaceKind::Startup => "starting",
        crate::fixtures::AppSurfaceKind::Reconnecting => "reconnecting",
        crate::fixtures::AppSurfaceKind::StartupFailure => "startup-failure",
        crate::fixtures::AppSurfaceKind::CompatibilityFailure => "compatibility-failure",
        crate::fixtures::AppSurfaceKind::BootstrapNote => "info",
    }
}

#[allow(dead_code)]
fn app_surface_tone(kind: crate::fixtures::AppSurfaceKind) -> &'static str {
    match kind {
        crate::fixtures::AppSurfaceKind::Startup
        | crate::fixtures::AppSurfaceKind::BootstrapNote => "info",
        crate::fixtures::AppSurfaceKind::Reconnecting => "warn",
        crate::fixtures::AppSurfaceKind::StartupFailure
        | crate::fixtures::AppSurfaceKind::CompatibilityFailure => "danger",
    }
}

fn chat_status_tone(is_run_active: bool, can_submit: bool) -> &'static str {
    if is_run_active {
        "info"
    } else if !can_submit {
        "warn"
    } else {
        "success"
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct DispatchContextItem {
    label: &'static str,
    value: String,
    tone: &'static str,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct DispatchContextStripModel {
    state: &'static str,
    send_detail: String,
    items: Vec<DispatchContextItem>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct ChatCopHostModel<'a> {
    summary: &'a crate::fixtures::HostSessionSummary,
    work: &'a crate::fixtures::WorkData,
    session: &'a crate::fixtures::SessionData,
    transcript: &'a TranscriptData,
    messages: &'a [crate::fixtures::ChatMessage],
    scenario: crate::fixtures::DevScenario,
    auto_expand: bool,
    is_run_active: bool,
    can_submit: bool,
    draft: &'a str,
    dispatch_context: &'a DispatchContextStripModel,
    provider_blocked_composer: Option<&'a ProviderBlockedComposerModel>,
    composer_ready_notice: Option<&'a str>,
}

struct ChatCopHostActions {
    on_submit: EventHandler<dioxus::events::FormEvent>,
    on_update_draft: EventHandler<String>,
    on_open_settings: EventHandler<()>,
    on_cancel: EventHandler<()>,
}

struct FocusHostShell<'a> {
    title: &'a str,
    kicker: &'a str,
    body: Element,
    footer: Option<Element>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SelectedCockpitEntity {
    DeploymentInstance(String),
    ActivityActor(String),
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum PromotedCockpitEntity {
    DeploymentInstance(String),
    ActivityActor(String),
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct TruthPanelModel {
    label: &'static str,
    tag: &'static str,
    primary: String,
    secondary: Vec<String>,
    preview: Vec<CockpitPreviewChip>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct CockpitSummaryModel {
    auspex: TruthPanelModel,
    attached: TruthPanelModel,
    deployment: TruthPanelModel,
    activity: TruthPanelModel,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct CockpitPreviewChip {
    key: String,
    label: String,
}

#[derive(Clone)]
struct CockpitSidecarActions {
    on_dispatcher_switch: Option<EventHandler<(String, Option<String>)>>,
    on_transcript_focus: Option<EventHandler<String>>,
    on_promote_selection: Option<EventHandler<crate::app::SelectedCockpitEntity>>,
}

fn render_cockpit_top_rail(
    cockpit: &CockpitSummaryModel,
    mut selected_cockpit_entity: Signal<Option<SelectedCockpitEntity>>,
) -> Element {
    rsx! {
        div { class: "cockpit-top-rail",
            header { class: "cockpit-spine",
                article { class: "cockpit-panel cockpit-panel-auspex", "data-surface": "panel", "data-elevation": "1",
                    div { class: "cockpit-panel-toprail",
                        span { class: "cockpit-panel-label", "{cockpit.auspex.label}" }
                        span { class: "cockpit-panel-tag", "{cockpit.auspex.tag}" }
                    }
                    p { class: "cockpit-panel-primary", "{cockpit.auspex.primary}" }
                    for line in &cockpit.auspex.secondary {
                        p { class: "cockpit-panel-secondary", "{line}" }
                    }
                }
                article { class: "cockpit-panel cockpit-panel-primary-omegon", "data-surface": "panel", "data-elevation": "1",
                    div { class: "cockpit-panel-toprail",
                        span { class: "cockpit-panel-label", "{cockpit.attached.label}" }
                        span { class: "cockpit-panel-tag", "{cockpit.attached.tag}" }
                    }
                    p { class: "cockpit-panel-primary", "{cockpit.attached.primary}" }
                    for line in &cockpit.attached.secondary {
                        p { class: "cockpit-panel-secondary", "{line}" }
                    }
                }
                article { class: "cockpit-panel cockpit-panel-deployment", "data-surface": "panel", "data-elevation": "1",
                    div { class: "cockpit-panel-toprail",
                        span { class: "cockpit-panel-label", "{cockpit.deployment.label}" }
                        span { class: "cockpit-panel-tag", "{cockpit.deployment.tag}" }
                    }
                    p { class: "cockpit-panel-primary", "{cockpit.deployment.primary}" }
                    for line in &cockpit.deployment.secondary {
                        p { class: "cockpit-panel-secondary", "{line}" }
                    }
                    if !cockpit.deployment.preview.is_empty() {
                        div { class: "cockpit-panel-preview-rail",
                            for item in &cockpit.deployment.preview {
                                button {
                                    class: if selected_cockpit_entity.read().as_ref() == Some(&SelectedCockpitEntity::DeploymentInstance(item.key.clone())) { "cockpit-panel-preview-chip cockpit-panel-preview-chip-selected" } else { "cockpit-panel-preview-chip" },
                                    r#type: "button",
                                    onclick: {
                                        let key = item.key.clone();
                                        move |_| {
                                            if selected_cockpit_entity.read().as_ref() == Some(&SelectedCockpitEntity::DeploymentInstance(key.clone())) {
                                                selected_cockpit_entity.set(None);
                                            } else {
                                                selected_cockpit_entity.set(Some(SelectedCockpitEntity::DeploymentInstance(key.clone())));
                                            }
                                        }
                                    },
                                    "{item.label}"
                                }
                            }
                        }
                    }
                }
                article { class: "cockpit-panel cockpit-panel-activity", "data-surface": "panel", "data-elevation": "1",
                    div { class: "cockpit-panel-toprail",
                        span { class: "cockpit-panel-label", "{cockpit.activity.label}" }
                        span { class: "cockpit-panel-tag", "{cockpit.activity.tag}" }
                    }
                    p { class: "cockpit-panel-primary", "{cockpit.activity.primary}" }
                    for line in &cockpit.activity.secondary {
                        p { class: "cockpit-panel-secondary", "{line}" }
                    }
                    if !cockpit.activity.preview.is_empty() {
                        div { class: "cockpit-panel-preview-rail",
                            for item in &cockpit.activity.preview {
                                button {
                                    class: if selected_cockpit_entity.read().as_ref() == Some(&SelectedCockpitEntity::ActivityActor(item.key.clone())) { "cockpit-panel-preview-chip cockpit-panel-preview-chip-selected" } else { "cockpit-panel-preview-chip" },
                                    r#type: "button",
                                    onclick: {
                                        let key = item.key.clone();
                                        move |_| {
                                            if selected_cockpit_entity.read().as_ref() == Some(&SelectedCockpitEntity::ActivityActor(key.clone())) {
                                                selected_cockpit_entity.set(None);
                                            } else {
                                                selected_cockpit_entity.set(Some(SelectedCockpitEntity::ActivityActor(key.clone())));
                                            }
                                        }
                                    },
                                    "{item.label}"
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

fn render_cockpit_center_stage(mut workspace: Signal<Workspace>, body: Element) -> Element {
    rsx! {
        section { class: "cockpit-cop-stage",
            section { class: "cockpit-cop-bay cockpit-focus-host",
                nav { class: "cockpit-workspace-nav",
                    button { class: if *workspace.read() == Workspace::Chat { "tab tab-active" } else { "tab" }, onclick: move |_| workspace.set(Workspace::Chat), "Chat" }
                    button { class: if *workspace.read() == Workspace::Session { "tab tab-active" } else { "tab" }, onclick: move |_| workspace.set(Workspace::Session), "Session" }
                    button { class: if *workspace.read() == Workspace::Scribe { "tab tab-active" } else { "tab" }, onclick: move |_| workspace.set(Workspace::Scribe), "Scribe" }
                    button { class: if *workspace.read() == Workspace::Graph { "tab tab-active" } else { "tab" }, onclick: move |_| workspace.set(Workspace::Graph), "Graph" }
                    button { class: if *workspace.read() == Workspace::Audit { "tab tab-active" } else { "tab" }, onclick: move |_| workspace.set(Workspace::Audit), "Audit" }
                }
                {body}
            }
        }
    }
}

fn render_cockpit_sidecar(
    session: &crate::fixtures::SessionData,
    selected_entity: Option<SelectedCockpitEntity>,
    actions: CockpitSidecarActions,
) -> Element {
    let CockpitSidecarActions {
        on_dispatcher_switch,
        on_transcript_focus,
        on_promote_selection,
    } = actions;

    rsx! {
        aside { class: "cockpit-sidecar",
            aside { class: "cockpit-support-bay cockpit-contextual-detail",
                SessionScreen {
                    data: session.clone(),
                    selected_entity: selected_entity,
                    on_dispatcher_switch: on_dispatcher_switch,
                    on_transcript_focus: on_transcript_focus,
                    on_promote_selection: on_promote_selection,
                }
            }
        }
    }
}

fn build_cockpit_summary_model(
    workspace: Workspace,
    session_mode: SessionMode,
    summary: &crate::fixtures::HostSessionSummary,
    session: &crate::fixtures::SessionData,
) -> CockpitSummaryModel {
    let shell_tag = match summary.activity_kind {
        crate::fixtures::ActivityKind::Idle => "OK",
        crate::fixtures::ActivityKind::Running => "LIVE",
        crate::fixtures::ActivityKind::Waiting => "WAIT",
        crate::fixtures::ActivityKind::Degraded => "DEGRADED",
        crate::fixtures::ActivityKind::Completed => "OK",
        crate::fixtures::ActivityKind::Failure => "FAILED",
    };

    let attached_role = session
        .dispatcher_binding
        .as_ref()
        .map(|binding| binding.expected_role.as_str())
        .or_else(|| {
            session
                .instance_descriptor
                .as_ref()
                .map(|instance| instance.identity.role.as_str())
        })
        .filter(|value| !value.is_empty())
        .unwrap_or("attached");
    let attached_id = session
        .dispatcher_binding
        .as_ref()
        .map(|binding| binding.dispatcher_instance_id.as_str())
        .or_else(|| {
            session
                .instance_descriptor
                .as_ref()
                .map(|instance| instance.identity.instance_id.as_str())
        })
        .filter(|value| !value.is_empty())
        .unwrap_or("unreported-instance");
    let attached_profile = session
        .dispatcher_binding
        .as_ref()
        .map(|binding| binding.expected_profile.as_str())
        .or_else(|| {
            session
                .instance_descriptor
                .as_ref()
                .map(|instance| instance.identity.profile.as_str())
        })
        .filter(|value| !value.is_empty())
        .unwrap_or("profile unreported");
    let attached_model = session
        .dispatcher_binding
        .as_ref()
        .and_then(|binding| binding.expected_model.as_deref())
        .or_else(|| {
            session
                .instance_descriptor
                .as_ref()
                .and_then(|instance| instance.policy.as_ref())
                .and_then(|policy| policy.model.as_deref())
        })
        .unwrap_or("model unreported");
    let attached_endpoint = session
        .dispatcher_binding
        .as_ref()
        .and_then(|binding| binding.observed_base_url.as_deref())
        .or_else(|| {
            session
                .instance_descriptor
                .as_ref()
                .and_then(|instance| instance.control_plane.as_ref())
                .and_then(|cp| cp.base_url.as_deref())
        })
        .unwrap_or("endpoint unreported");
    let primary_missing = attached_id == "unreported-instance" && attached_role == "attached";
    let attached_primary = if primary_missing {
        "Primary runtime unbound".into()
    } else {
        format!("{attached_role} · {attached_id}")
    };
    let attached_secondary_1 = if primary_missing {
        "Awaiting owned runtime identity".into()
    } else {
        format!("{attached_profile} · {attached_model}")
    };
    let attached_secondary_2 = if primary_missing {
        "booting/attaching · no verified endpoint".into()
    } else if session.telemetry.lifecycle.counts.stale > 0 {
        format!("degraded · {attached_endpoint}")
    } else {
        format!("verified · {attached_endpoint}")
    };
    let attached_tag = if primary_missing {
        match summary.activity_kind {
            crate::fixtures::ActivityKind::Waiting => "ATTACHING",
            crate::fixtures::ActivityKind::Running => "BOOTING",
            crate::fixtures::ActivityKind::Failure => "FAILED",
            crate::fixtures::ActivityKind::Degraded => "DEGRADED",
            _ => "BOOTING",
        }
    } else if session.telemetry.lifecycle.counts.stale > 0 {
        "DEGRADED"
    } else if matches!(
        summary.activity_kind,
        crate::fixtures::ActivityKind::Failure
    ) {
        "FAILED"
    } else {
        "LIVE"
    };

    let serve_count = session
        .telemetry
        .lifecycle
        .instances
        .iter()
        .filter(|instance| instance.role == "detached-service")
        .count();
    let temporary_count = session
        .active_delegate_count
        .max(session.active_delegates.len());
    let deployment_primary = format!(
        "{} total · {} live · {} stale",
        session.telemetry.lifecycle.counts.total_attached,
        session.telemetry.lifecycle.counts.fresh,
        session.telemetry.lifecycle.counts.stale
    );
    let deployment_secondary_1 = format!(
        "{} attached · {} serve · {} temporary",
        session.telemetry.lifecycle.attached_count, serve_count, temporary_count
    );
    let deployment_secondary_2 = if session.telemetry.lifecycle.instances.is_empty() {
        session.telemetry.lifecycle.summary.clone()
    } else {
        let seen = session
            .telemetry
            .lifecycle
            .instances
            .iter()
            .map(|instance| instance.instance_id.as_str())
            .collect::<Vec<_>>()
            .join(", ");
        format!("seen: {seen}")
    };
    let deployment_preview = session
        .telemetry
        .lifecycle
        .instances
        .iter()
        .take(4)
        .map(|instance| {
            let freshness = instance.freshness.as_deref().unwrap_or("unknown");
            CockpitPreviewChip {
                key: instance.instance_id.clone(),
                label: format!("{} · {}", instance.instance_id, freshness),
            }
        })
        .collect::<Vec<_>>();
    let deployment_tag = if session.telemetry.lifecycle.counts.stale > 0
        || session.telemetry.lifecycle.counts.lost > 0
        || session.telemetry.lifecycle.counts.abandoned > 0
    {
        "DRIFT"
    } else {
        "STABLE"
    };

    let active_count = temporary_count;
    let activity_tag = if active_count > 0 {
        "RUNNING"
    } else {
        match summary.activity_kind {
            crate::fixtures::ActivityKind::Degraded => "ALERT",
            crate::fixtures::ActivityKind::Waiting => "WAITING",
            crate::fixtures::ActivityKind::Completed => "COMPLETED",
            crate::fixtures::ActivityKind::Failure => "FAILED",
            crate::fixtures::ActivityKind::Running => "RUNNING",
            crate::fixtures::ActivityKind::Idle => "IDLE",
        }
    };
    let activity_primary = if let Some(delegate) = session.active_delegates.first() {
        format!("{} · {}", delegate.agent_name, delegate.status)
    } else {
        match summary.activity_kind {
            crate::fixtures::ActivityKind::Completed | crate::fixtures::ActivityKind::Failure => {
                summary.activity.clone()
            }
            _ => summary
                .activity_kind
                .label()
                .to_ascii_uppercase()
                .to_string(),
        }
    };
    let activity_secondary_1 = if active_count > 0 {
        format!("{active_count} dispatches in flight")
    } else {
        cockpit_work_hint(summary)
    };
    let activity_secondary_2 = if active_count > 0 {
        summary.activity.clone()
    } else {
        summary.connection.clone()
    };
    let activity_preview = session
        .active_delegates
        .iter()
        .take(4)
        .map(|delegate| CockpitPreviewChip {
            key: delegate.task_id.clone(),
            label: format!("{} · {}", delegate.agent_name, delegate.status),
        })
        .collect::<Vec<_>>();

    CockpitSummaryModel {
        auspex: TruthPanelModel {
            label: "Auspex",
            tag: shell_tag,
            primary: format!("{} · {}", workspace_label(workspace), session_mode.label()),
            secondary: vec![format!("v{APP_VERSION}"), context_window_label(session)],
            preview: vec![],
        },
        attached: TruthPanelModel {
            label: "Primary Omegon",
            tag: attached_tag,
            primary: attached_primary,
            secondary: vec![attached_secondary_1, attached_secondary_2],
            preview: vec![],
        },
        deployment: TruthPanelModel {
            label: "Deployment",
            tag: deployment_tag,
            primary: deployment_primary,
            secondary: vec![deployment_secondary_1, deployment_secondary_2],
            preview: deployment_preview,
        },
        activity: TruthPanelModel {
            label: "Activity",
            tag: activity_tag,
            primary: activity_primary,
            secondary: vec![activity_secondary_1, activity_secondary_2],
            preview: activity_preview,
        },
    }
}

fn cockpit_work_hint(summary: &crate::fixtures::HostSessionSummary) -> String {
    if summary.work.trim().is_empty() {
        "no focused work".into()
    } else {
        summary.work.clone()
    }
}

fn workspace_label(workspace: Workspace) -> &'static str {
    match workspace {
        Workspace::Chat => "Chat",
        Workspace::Session => "Session",
        Workspace::Scribe => "Scribe",
        Workspace::Graph => "Graph",
        Workspace::Audit => "Audit",
    }
}

fn context_window_label(session: &crate::fixtures::SessionData) -> String {
    if let Some(tokens) = session.context_tokens {
        if let Some(window) = session.context_window {
            format!("{tokens} / {window} tokens")
        } else {
            format!("{tokens} tokens")
        }
    } else {
        "Context usage not reported yet".to_string()
    }
}

fn build_dispatch_context_strip_model(
    workspace: Workspace,
    session_mode: SessionMode,
    summary: &crate::fixtures::HostSessionSummary,
    session: &crate::fixtures::SessionData,
    draft: &str,
    is_run_active: bool,
    can_submit: bool,
) -> DispatchContextStripModel {
    let route = format!(
        "{} · {}",
        match workspace {
            Workspace::Chat => "chat",
            Workspace::Session => "session",
            Workspace::Scribe => "scribe",
            Workspace::Graph => "graph",
            Workspace::Audit => "audit",
        },
        session_mode.label().to_ascii_lowercase()
    );

    let session_label = session
        .dispatcher_binding
        .as_ref()
        .map(|binding| binding.session_id.clone())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| {
            if session_mode == SessionMode::Live {
                "host-session".into()
            } else {
                "local-session".into()
            }
        });

    let who = session
        .dispatcher_binding
        .as_ref()
        .map(|binding| {
            if !binding.expected_role.is_empty() {
                binding.expected_role.clone()
            } else {
                binding.expected_profile.clone()
            }
        })
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| summary.connection.clone());

    let model = session
        .dispatcher_binding
        .as_ref()
        .and_then(|binding| binding.expected_model.clone())
        .or_else(|| {
            session
                .instance_descriptor
                .as_ref()
                .and_then(|descriptor| descriptor.policy.as_ref())
                .and_then(|policy| policy.model.clone())
        })
        .or_else(|| {
            session
                .providers
                .iter()
                .find_map(|provider| provider.model.clone())
        });

    let thinking =
        (!session.thinking_level.trim().is_empty()).then(|| session.thinking_level.clone());

    let tier =
        (!session.capability_tier.trim().is_empty()).then(|| session.capability_tier.clone());

    let context = context_window_label(session);

    let (state, state_label, state_tone) = if is_run_active {
        ("running", "Run active".to_string(), "info")
    } else if !can_submit {
        ("blocked", "Input paused".to_string(), "warn")
    } else {
        (
            "ready",
            format!("{} · {}", summary.activity_kind.label(), summary.activity),
            "success",
        )
    };

    let trimmed_draft = draft.trim();
    let provider_ready = session
        .providers
        .iter()
        .any(|provider| provider.authenticated);
    let (send_label, send_tone, send_detail) = if is_run_active {
        (
            "Blocked by active run".to_string(),
            "info",
            "Wait for the current run to finish or cancel it before sending another prompt."
                .to_string(),
        )
    } else if !provider_ready {
        (
            "Host missing providers".to_string(),
            "warn",
            "Omegon did not report any authenticated providers, so prompt execution is unavailable until the host regains a runnable model backend.".to_string(),
        )
    } else if !can_submit {
        (
            "Unavailable in current host state".to_string(),
            "warn",
            "Conversation input is unavailable until the host returns to a ready or degraded state."
                .to_string(),
        )
    } else if trimmed_draft.is_empty() {
        (
            "Needs prompt text".to_string(),
            "muted",
            "Draft a prompt before sending so the dispatcher has work to route.".to_string(),
        )
    } else {
        (
            "Ready to send".to_string(),
            "success",
            format!(
                "Prompt ready: {} character(s) queued for dispatch.",
                trimmed_draft.chars().count()
            ),
        )
    };

    DispatchContextStripModel {
        state,
        send_detail,
        items: {
            let mut items = vec![
                DispatchContextItem {
                    label: "Route",
                    value: route,
                    tone: "muted",
                },
                DispatchContextItem {
                    label: "Session",
                    value: session_label,
                    tone: "muted",
                },
                DispatchContextItem {
                    label: "Who",
                    value: who,
                    tone: "accent",
                },
            ];
            if let Some(model) = model {
                items.push(DispatchContextItem {
                    label: "Model",
                    value: model,
                    tone: "accent",
                });
            }
            if let Some(thinking) = thinking {
                items.push(DispatchContextItem {
                    label: "Thinking",
                    value: thinking,
                    tone: "muted",
                });
            }
            if let Some(tier) = tier {
                items.push(DispatchContextItem {
                    label: "Tier",
                    value: tier,
                    tone: "muted",
                });
            }
            items.push(DispatchContextItem {
                label: "State",
                value: state_label,
                tone: state_tone,
            });
            items.push(DispatchContextItem {
                label: "Context",
                value: context,
                tone: "muted",
            });
            items.push(DispatchContextItem {
                label: "Send",
                value: send_label,
                tone: send_tone,
            });
            items
        },
    }
}

fn render_selected_deployment_cop(
    session: &crate::fixtures::SessionData,
    instance_id: &str,
    on_return: EventHandler<()>,
) -> Element {
    let instance = session
        .telemetry
        .lifecycle
        .instances
        .iter()
        .find(|instance| instance.instance_id == instance_id);
    let Some(instance) = instance else {
        return rsx! { Fragment {} };
    };
    let body = rsx! {
        div { class: "kv-grid widget-kv-grid",
            div { class: "kv-row", span { class: "kv-key", "Instance" } span { class: "kv-value", "{instance.instance_id}" } }
            div { class: "kv-row", span { class: "kv-key", "Route" } span { class: "kv-value", "{instance.route_id}" } }
            div { class: "kv-row", span { class: "kv-key", "Role" } span { class: "kv-value", "{instance.role}" } }
            div { class: "kv-row", span { class: "kv-key", "Profile" } span { class: "kv-value", "{instance.profile}" } }
            if let Some(status) = instance.status.as_deref() { div { class: "kv-row", span { class: "kv-key", "Status" } span { class: "kv-value", "{status}" } } }
            if let Some(freshness) = instance.freshness.as_deref() { div { class: "kv-row", span { class: "kv-key", "Freshness" } span { class: "kv-value", "{freshness}" } } }
            if let Some(base_url) = instance.base_url.as_deref() { div { class: "kv-row", span { class: "kv-key", "Base URL" } span { class: "kv-value", "{base_url}" } } }
        }
    };
    let footer = rsx! { button { class: "transcript-focus-link", r#type: "button", onclick: move |_| on_return.call(()), "Return to chat COP" } };
    render_focus_host_shell(FocusHostShell {
        title: "Deployment drilldown",
        kicker: "Operator-selected COP occupant",
        body,
        footer: Some(footer),
    })
}

fn render_selected_activity_cop(
    session: &crate::fixtures::SessionData,
    task_id: &str,
    on_transcript_focus: Option<EventHandler<String>>,
    on_return: EventHandler<()>,
) -> Element {
    let delegate = session
        .active_delegates
        .iter()
        .find(|delegate| delegate.task_id == task_id);
    let Some(delegate) = delegate else {
        return rsx! { Fragment {} };
    };
    let body = rsx! {
        div { class: "kv-grid widget-kv-grid",
            div { class: "kv-row", span { class: "kv-key", "Actor" } span { class: "kv-value", "{delegate.agent_name}" } }
            div { class: "kv-row", span { class: "kv-key", "Task" } span { class: "kv-value", "{delegate.task_id}" } }
            div { class: "kv-row", span { class: "kv-key", "Status" } span { class: "kv-value", "{delegate.status}" } }
            div { class: "kv-row", span { class: "kv-key", "Elapsed" } span { class: "kv-value", "{delegate.elapsed_ms} ms" } }
        }
    };
    let footer = rsx! {
        div { class: "focus-host-footer-actions",
            if let Some(handler) = on_transcript_focus {
                button {
                    class: "transcript-focus-link",
                    r#type: "button",
                    onclick: {
                        let task_id = delegate.task_id.clone();
                        move |_| handler.call(format!("delegate:{task_id}"))
                    },
                    "Focus related transcript events"
                }
            }
            button { class: "transcript-focus-link", r#type: "button", onclick: move |_| on_return.call(()), "Return to chat COP" }
        }
    };
    render_focus_host_shell(FocusHostShell {
        title: "Activity drilldown",
        kicker: "Operator-selected COP occupant",
        body,
        footer: Some(footer),
    })
}

fn render_focus_host_shell(shell: FocusHostShell<'_>) -> Element {
    let FocusHostShell {
        title,
        kicker,
        body,
        footer,
    } = shell;

    rsx! {
        div { class: "focus-host-shell",
            header { class: "focus-host-header",
                span { class: "focus-host-kicker", "{kicker}" }
                h2 { class: "focus-host-title", "{title}" }
            }
            div { class: "focus-host-body", {body} }
            if let Some(footer) = footer {
                div { class: "focus-host-footer", {footer} }
            }
        }
    }
}

fn render_chat_cop_host(model: ChatCopHostModel<'_>, actions: ChatCopHostActions) -> Element {
    let ChatCopHostModel {
        summary,
        work,
        session,
        transcript,
        messages,
        scenario,
        auto_expand,
        is_run_active,
        can_submit,
        draft,
        dispatch_context,
        provider_blocked_composer,
        composer_ready_notice,
    } = model;
    let ChatCopHostActions {
        on_submit,
        on_update_draft,
        on_open_settings,
        on_cancel,
    } = actions;

    let body = rsx! {
        div { class: "cockpit-cop-body",
            if provider_blocked_composer.is_none() {
                {render_chat_status_banner(summary, session, is_run_active, can_submit)}
            }
            main { class: "transcript cockpit-transcript cockpit-cop-focus",
                {render_transcript(summary, work, session, transcript, messages, auto_expand, scenario)}
                div { id: "transcript-end" }
            }
        }
    };

    let footer = rsx! {
        form {
            class: "composer cockpit-composer cockpit-composer-docked",
            onsubmit: move |event| on_submit.call(event),
            {render_dispatch_context_strip(dispatch_context)}
            if let Some(message) = composer_ready_notice {
                div { class: "composer-ready-notice", "{message}" }
            }
            if let Some(blocked) = provider_blocked_composer {
                div { class: "composer-blocked-callout",
                    h3 { class: "composer-blocked-title", "{blocked.title}" }
                    p { class: "composer-blocked-detail", "{blocked.detail}" }
                    button { class: "composer-blocked-action", r#type: "button", onclick: move |_| on_open_settings.call(()), "{blocked.action_label}" }
                }
            } else {
                textarea {
                    class: "composer-input",
                    rows: "3",
                    value: draft.to_string(),
                    disabled: !can_submit,
                    placeholder: if can_submit { "Start with the smallest useful prompt…" } else { "Conversation input is unavailable in the current host state." },
                    oninput: move |event| on_update_draft.call(event.value()),
                }
            }
            div { class: "composer-actions",
                if is_run_active {
                    button { class: "composer-cancel", r#type: "button", onclick: move |_| on_cancel.call(()), "Cancel" }
                }
                button { class: "composer-submit", r#type: "submit", disabled: !can_submit, title: dispatch_context.send_detail.clone(), "Send" }
            }
        }
    };

    render_focus_host_shell(FocusHostShell {
        title: "Chat",
        kicker: "Default COP occupant",
        body,
        footer: Some(footer),
    })
}

fn render_dispatch_context_strip(model: &DispatchContextStripModel) -> Element {
    rsx! {
        section {
            class: "dispatch-context-strip",
            "data-surface": "panel",
            "data-state": model.state,
            "data-tone": "muted",
            "aria-label": "Dispatch context",
            for item in &model.items {
                div {
                    class: "dispatch-context-chip",
                    "data-tone": item.tone,
                    span { class: "dispatch-context-label", "{item.label}" }
                    span { class: "dispatch-context-value", title: if item.label == "Send" { model.send_detail.clone() } else { item.value.clone() }, "{item.value}" }
                }
            }
        }
    }
}

fn text_block_class(origin: Option<&crate::fixtures::BlockOrigin>) -> &'static str {
    match origin.map(|origin| &origin.kind) {
        Some(crate::fixtures::OriginKind::Dispatcher) => "block block-text",
        Some(crate::fixtures::OriginKind::Child) => "block block-system block-child-origin",
        Some(crate::fixtures::OriginKind::System) => "block block-system",
        None => "block block-text",
    }
}

fn text_block_tone(origin: Option<&crate::fixtures::BlockOrigin>) -> &'static str {
    match origin.map(|origin| &origin.kind) {
        Some(crate::fixtures::OriginKind::Child) => "accent",
        Some(crate::fixtures::OriginKind::System) => "muted",
        Some(crate::fixtures::OriginKind::Dispatcher) | None => "default",
    }
}

fn system_block_class(text: &crate::fixtures::AttributedText) -> &'static str {
    match text.notice_kind {
        Some(crate::fixtures::SystemNoticeKind::DispatcherSwitch) => {
            "block block-system block-dispatcher-system"
        }
        Some(crate::fixtures::SystemNoticeKind::CleaveStart) => {
            "block block-system block-dispatcher-system block-control-cleave"
        }
        Some(crate::fixtures::SystemNoticeKind::CleaveComplete) => {
            "block block-system block-dispatcher-system block-control-complete"
        }
        Some(crate::fixtures::SystemNoticeKind::ChildStatus) => {
            "block block-system block-child-origin block-control-child"
        }
        Some(crate::fixtures::SystemNoticeKind::Failure) => {
            match text.origin.as_ref().map(|origin| &origin.kind) {
                Some(crate::fixtures::OriginKind::Child) => {
                    "block block-system block-child-origin block-control-failure"
                }
                Some(crate::fixtures::OriginKind::Dispatcher) => {
                    "block block-system block-dispatcher-system block-control-failure"
                }
                _ => "block block-system block-control-failure",
            }
        }
        Some(crate::fixtures::SystemNoticeKind::Generic) | None => {
            match text.origin.as_ref().map(|origin| &origin.kind) {
                Some(crate::fixtures::OriginKind::Dispatcher) => {
                    "block block-system block-dispatcher-system"
                }
                Some(crate::fixtures::OriginKind::Child) => "block block-system block-child-origin",
                Some(crate::fixtures::OriginKind::System) => "block block-system",
                None => "block block-system",
            }
        }
    }
}

fn system_block_tone(text: &crate::fixtures::AttributedText) -> &'static str {
    match text.notice_kind {
        Some(crate::fixtures::SystemNoticeKind::Failure) => "danger",
        Some(crate::fixtures::SystemNoticeKind::CleaveStart)
        | Some(crate::fixtures::SystemNoticeKind::DispatcherSwitch) => "info",
        Some(crate::fixtures::SystemNoticeKind::CleaveComplete) => "success",
        Some(crate::fixtures::SystemNoticeKind::ChildStatus) => "accent",
        Some(crate::fixtures::SystemNoticeKind::Generic) | None => {
            text_block_tone(text.origin.as_ref())
        }
    }
}

fn tool_block_class(tool: &crate::fixtures::ToolCard) -> &'static str {
    if tool.is_error {
        "block block-tool block-error"
    } else if tool.result.is_some() {
        "block block-tool block-tool-complete"
    } else {
        "block block-tool block-tool-running"
    }
}

fn tool_block_tone(tool: &crate::fixtures::ToolCard) -> &'static str {
    if tool.is_error {
        "danger"
    } else if tool.result.is_some() {
        "success"
    } else if tool.partial_output.is_empty() {
        "muted"
    } else {
        "info"
    }
}

fn tool_status_class(tool: &crate::fixtures::ToolCard) -> &'static str {
    if tool.is_error {
        "tool-status tool-status-error"
    } else if tool.result.is_some() {
        "tool-status tool-status-complete"
    } else {
        "tool-status tool-status-running"
    }
}

fn tool_visual_state(tool: &crate::fixtures::ToolCard) -> &'static str {
    if tool.is_error {
        "error"
    } else if tool.result.is_some() {
        "complete"
    } else if tool.partial_output.is_empty() {
        "queued"
    } else {
        "streaming"
    }
}

fn tool_status_label(tool: &crate::fixtures::ToolCard) -> &'static str {
    if tool.is_error {
        "Error"
    } else if tool.result.is_some() {
        "Complete"
    } else if tool.partial_output.is_empty() {
        "Queued"
    } else {
        "Streaming"
    }
}

fn tool_partial_label(tool: &crate::fixtures::ToolCard) -> &'static str {
    if tool.result.is_some() {
        "Streamed output"
    } else {
        "Live output"
    }
}

fn tool_result_label(tool: &crate::fixtures::ToolCard) -> &'static str {
    if tool.is_error {
        "Error result"
    } else {
        "Final result"
    }
}

fn origin_class(origin: &crate::fixtures::BlockOrigin) -> &'static str {
    match origin.kind {
        crate::fixtures::OriginKind::Dispatcher => "block-origin block-origin-dispatcher",
        crate::fixtures::OriginKind::Child => "block-origin block-origin-child",
        crate::fixtures::OriginKind::System => "block-origin block-origin-system",
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
#[allow(dead_code)]
struct LeftRailInventory {
    workspace_label: String,
    workspace_detail: String,
    project_label: String,
    session_label: String,
    session_detail: String,
    agent_rows: Vec<(String, String)>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
struct AuditFilters {
    session_key: String,
    turn_query: String,
    kind_key: String,
    text_query: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct AuditPanelEntry {
    heading: String,
    meta: String,
    content: String,
    kind_key: &'static str,
    focus_target: Option<String>,
}

#[derive(Clone, Debug)]
struct AuditPanelControls {
    filters: AuditFilters,
    on_session_filter: EventHandler<String>,
    on_turn_filter: EventHandler<String>,
    on_kind_filter: EventHandler<String>,
    on_text_filter: EventHandler<String>,
    on_focus_entry: EventHandler<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct AuditPanelModel {
    total_count: usize,
    filtered_count: usize,
    latest_label: String,
    session_options: Vec<String>,
    entries: Vec<AuditPanelEntry>,
}

#[allow(dead_code)]
fn build_left_rail_inventory(
    summary: &crate::fixtures::HostSessionSummary,
    work: &crate::fixtures::WorkData,
    session: &crate::fixtures::SessionData,
    is_run_active: bool,
) -> LeftRailInventory {
    let workspace_label = session
        .dispatcher_binding
        .as_ref()
        .map(|binding| binding.dispatcher_instance_id.clone())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| {
            session
                .git_branch
                .clone()
                .unwrap_or_else(|| "detached".into())
        });
    let workspace_detail = session
        .git_branch
        .clone()
        .map(|branch| {
            if session.git_detached {
                format!("workspace · detached from {branch}")
            } else {
                format!("workspace · branch {branch}")
            }
        })
        .unwrap_or_else(|| {
            if summary.connection.contains("Omegon host") {
                "attached host workspace not identified yet".into()
            } else {
                "workspace not identified yet".into()
            }
        });
    let project_label = work
        .focused_title
        .clone()
        .or_else(|| Some(summary.work.clone()))
        .unwrap_or_else(|| "No focused work item".into());

    let session_label = session
        .dispatcher_binding
        .as_ref()
        .map(|binding| binding.session_id.clone())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| {
            if summary.connection.contains("Omegon host") {
                "host-session".into()
            } else {
                "local-session".into()
            }
        });

    let session_detail = session
        .dispatcher_binding
        .as_ref()
        .map(|binding| {
            let target = binding
                .expected_model
                .clone()
                .unwrap_or_else(|| binding.expected_profile.clone());
            if binding.expected_role.is_empty() {
                target
            } else {
                format!("{} · {target}", binding.expected_role)
            }
        })
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| summary.connection.clone());

    let mut agent_rows = Vec::new();
    if let Some(binding) = session.dispatcher_binding.as_ref() {
        let dispatcher_name = if let Some(model) = binding.expected_model.as_ref() {
            format!("Dispatcher · {model}")
        } else if !binding.dispatcher_instance_id.is_empty() {
            format!("Dispatcher · {}", binding.dispatcher_instance_id)
        } else {
            format!("Dispatcher · {}", binding.expected_profile)
        };
        let dispatcher_status = if is_run_active {
            "running".to_string()
        } else {
            binding.expected_role.clone()
        };
        agent_rows.push((dispatcher_name, dispatcher_status));
    }

    for delegate in &session.active_delegates {
        agent_rows.push((
            format!("Delegate · {}", delegate.agent_name),
            format!("{} · {} ms", delegate.status, delegate.elapsed_ms),
        ));
    }

    if agent_rows.is_empty() {
        let label = if summary.connection.contains("Omegon host") {
            "Host attached · no dispatcher binding"
        } else {
            "No dispatcher binding"
        };
        agent_rows.push((label.into(), "idle".into()));
    }

    LeftRailInventory {
        workspace_label,
        workspace_detail,
        project_label,
        session_label,
        session_detail,
        agent_rows,
    }
}

#[allow(dead_code)]
fn render_left_rail_inventory(
    summary: &crate::fixtures::HostSessionSummary,
    work: &crate::fixtures::WorkData,
    session: &crate::fixtures::SessionData,
    is_run_active: bool,
    audit_timeline: &AuditTimelineStore,
) -> Element {
    let inventory = build_left_rail_inventory(summary, work, session, is_run_active);
    let audit_count = audit_timeline.entries.len();
    let latest_audit_label = audit_timeline
        .entries
        .last()
        .map(|entry| entry.label.as_str())
        .unwrap_or("No transcript blocks retained yet");

    rsx! {
        section { class: "rail-section",
            h2 { class: "rail-heading", "Workspace" }
            div { class: "rail-card",
                div { class: "rail-card-title", "{inventory.workspace_label}" }
                p { class: "rail-card-detail", "{inventory.workspace_detail}" }
                p { class: "rail-card-detail", "{inventory.project_label}" }
            }
        }
        section { class: "rail-section",
            h2 { class: "rail-heading", "Session" }
            div { class: "rail-card",
                div { class: "rail-card-title", "{inventory.session_label}" }
                p { class: "rail-card-detail", "{inventory.session_detail}" }
            }
        }
        section { class: "rail-section",
            h2 { class: "rail-heading", "Agents" }
            div { class: "rail-list",
                for (name, detail) in &inventory.agent_rows {
                    div { class: "rail-list-item",
                        span { class: "rail-list-title", "{name}" }
                        span { class: "rail-list-detail", "{detail}" }
                    }
                }
            }
        }
        section { class: "rail-section",
            h2 { class: "rail-heading", "Audit" }
            div { class: "rail-card audit-summary-card",
                div { class: "rail-card-title", "{audit_count} retained block(s)" }
                p { class: "rail-card-detail", "Latest: {latest_audit_label}" }
            }
        }
    }
}

fn render_audit_workspace(
    audit_timeline: &AuditTimelineStore,
    current_audit_session_key: &str,
    controls: AuditPanelControls,
) -> Element {
    let audit_panel =
        build_audit_panel_model(audit_timeline, current_audit_session_key, &controls.filters);

    rsx! {
        div { class: "screen screen-audit",
            section { class: "screen-section",
                h2 { class: "screen-section-title", "Audit history" }
                p { class: "screen-empty",
                    "Project-wide retained transcript blocks across sessions. Filter by session, turn, kind, or text; jump to live transcript blocks when they belong to the current session."
                }
            }
            div { class: "audit-workspace-layout",
                section { class: "screen-section audit-panel audit-panel-controls",
                    div { class: "rail-card audit-summary-card",
                        div { class: "rail-card-title", "{audit_panel.filtered_count} of {audit_panel.total_count} retained block(s)" }
                        p { class: "rail-card-detail", "Latest: {audit_panel.latest_label}" }
                    }
                    div { class: "rail-list audit-filter-list",
                        label { class: "audit-filter-field",
                            span { class: "audit-filter-label", "Session" }
                            select {
                                class: "audit-filter-control",
                                value: controls.filters.session_key.clone(),
                                onchange: move |event| controls.on_session_filter.call(event.value()),
                                option { value: "", "All sessions" }
                                for session_key in &audit_panel.session_options {
                                    option { value: session_key.clone(), "{session_key}" }
                                }
                            }
                        }
                        label { class: "audit-filter-field",
                            span { class: "audit-filter-label", "Turn" }
                            input {
                                class: "audit-filter-control",
                                r#type: "search",
                                inputmode: "numeric",
                                placeholder: "All turns",
                                value: controls.filters.turn_query.clone(),
                                oninput: move |event| controls.on_turn_filter.call(event.value()),
                            }
                        }
                        label { class: "audit-filter-field",
                            span { class: "audit-filter-label", "Kind" }
                            select {
                                class: "audit-filter-control",
                                value: controls.filters.kind_key.clone(),
                                onchange: move |event| controls.on_kind_filter.call(event.value()),
                                for (value, label) in audit_kind_options() {
                                    option { value: value, "{label}" }
                                }
                            }
                        }
                        label { class: "audit-filter-field",
                            span { class: "audit-filter-label", "Search" }
                            input {
                                class: "audit-filter-control",
                                r#type: "search",
                                placeholder: "Label or retained text",
                                value: controls.filters.text_query.clone(),
                                oninput: move |event| controls.on_text_filter.call(event.value()),
                            }
                        }
                    }
                }
                section { class: "screen-section audit-panel audit-panel-results",
                    div { class: "audit-entry-list audit-entry-list-workspace",
                        if audit_panel.entries.is_empty() {
                            p { class: "rail-placeholder", "No retained transcript blocks match the current filters." }
                        } else {
                            for entry in &audit_panel.entries {
                                article {
                                    class: "audit-entry-card",
                                    "data-kind": entry.kind_key,
                                    h3 { class: "audit-entry-title", "{entry.heading}" }
                                    p { class: "audit-entry-meta", "{entry.meta}" }
                                    if let Some(target) = &entry.focus_target {
                                        button {
                                            class: "audit-entry-jump",
                                            r#type: "button",
                                            onclick: {
                                                let target = target.clone();
                                                let handler = controls.on_focus_entry;
                                                move |_| handler.call(target.clone())
                                            },
                                            "Jump to transcript"
                                        }
                                    } else {
                                        p { class: "audit-entry-unavailable", "Transcript block not present in the current session." }
                                    }
                                    pre { class: "audit-entry-content", "{entry.content}" }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

fn audit_kind_options() -> [(&'static str, &'static str); 7] {
    [
        ("all", "All kinds"),
        ("thinking", "Thinking"),
        ("text", "Message"),
        ("tool", "Tool"),
        ("system", "System"),
        ("telemetry", "Telemetry"),
        ("aborted", "Aborted"),
    ]
}

fn build_audit_panel_model(
    audit_timeline: &AuditTimelineStore,
    current_audit_session_key: &str,
    filters: &AuditFilters,
) -> AuditPanelModel {
    let latest_label = audit_timeline
        .entries
        .last()
        .map(|entry| entry.label.clone())
        .unwrap_or_else(|| "No transcript blocks retained yet".into());
    let mut session_options = audit_timeline
        .entries
        .iter()
        .map(|entry| entry.session_key.clone())
        .collect::<std::collections::BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    session_options.sort();

    let entries = audit_timeline
        .entries
        .iter()
        .filter(|entry| audit_entry_matches_filters(entry, filters))
        .map(|entry| AuditPanelEntry {
            heading: entry.label.clone(),
            meta: format!(
                "{} · turn {} · {}",
                entry.session_key,
                entry.turn_number,
                audit_kind_label(entry.kind.clone())
            ),
            content: entry.content.clone(),
            kind_key: audit_kind_key(entry.kind.clone()),
            focus_target: (entry.session_key == current_audit_session_key)
                .then(|| transcript_block_dom_id(entry.turn_number, entry.block_index)),
        })
        .collect::<Vec<_>>();

    AuditPanelModel {
        total_count: audit_timeline.entries.len(),
        filtered_count: entries.len(),
        latest_label,
        session_options,
        entries,
    }
}

fn audit_entry_matches_filters(entry: &AuditEntry, filters: &AuditFilters) -> bool {
    let session_filter = filters.session_key.trim();
    if !session_filter.is_empty() && entry.session_key != session_filter {
        return false;
    }

    let turn_filter = filters.turn_query.trim();
    if !turn_filter.is_empty() {
        let Ok(turn_number) = turn_filter.parse::<u32>() else {
            return false;
        };
        if entry.turn_number != turn_number {
            return false;
        }
    }

    let kind_filter = filters.kind_key.trim();
    if !kind_filter.is_empty()
        && kind_filter != "all"
        && audit_kind_key(entry.kind.clone()) != kind_filter
    {
        return false;
    }

    let text_filter = filters.text_query.trim().to_ascii_lowercase();
    if !text_filter.is_empty() {
        let haystack = format!(
            "{}\n{}\n{}\n{}",
            entry.session_key, entry.turn_number, entry.label, entry.content,
        )
        .to_ascii_lowercase();
        if !haystack.contains(&text_filter) {
            return false;
        }
    }

    true
}

fn audit_kind_key(kind: AuditEntryKind) -> &'static str {
    match kind {
        AuditEntryKind::Thinking => "thinking",
        AuditEntryKind::Text => "text",
        AuditEntryKind::Tool => "tool",
        AuditEntryKind::System => "system",
        AuditEntryKind::Telemetry => "telemetry",
        AuditEntryKind::Aborted => "aborted",
    }
}

fn audit_kind_label(kind: AuditEntryKind) -> &'static str {
    match kind {
        AuditEntryKind::Thinking => "Thinking",
        AuditEntryKind::Text => "Message",
        AuditEntryKind::Tool => "Tool",
        AuditEntryKind::System => "System",
        AuditEntryKind::Telemetry => "Telemetry",
        AuditEntryKind::Aborted => "Aborted",
    }
}

#[cfg(test)]
mod tests {
    use super::{
        AuditFilters, SettingsAuthAction, Workspace, app_surface_state, app_surface_tone,
        audit_entry_matches_filters, audit_kind_key, block_origin_label, build_audit_panel_model,
        build_chat_empty_state_model, build_dispatch_context_strip_model,
        build_left_rail_inventory, build_provider_blocked_composer_model,
        build_settings_panel_model, chat_status_tone, context_window_label,
        dispatch_targeted_command, find_transcript_anchor, looks_like_structured_payload,
        render_chat_status_banner, render_dispatch_context_strip, should_expand_system_notice,
        should_expand_tool_args, should_expand_tool_output, system_block_class, system_block_tone,
        system_notice_summary_label, text_block_class, text_block_tone, tool_block_class,
        tool_block_tone, tool_partial_label, tool_result_label, tool_status_label,
        tool_visual_state, transcript_block_dom_id, transcript_disclosure_meta,
        transcript_disclosure_open,
    };
    use crate::audit_timeline::{AuditEntry, AuditEntryKind, AuditTimelineStore};
    use crate::controller::{AppController, SessionMode};
    use crate::event_stream::EventStreamHandle;
    use crate::fixtures::{
        ActivityKind, AttributedText, BlockOrigin, DevScenario, HostSessionSummary, MessageRole,
        MockHostSession, OriginKind, SystemNoticeKind, ToolCard, TranscriptData,
    };
    #[cfg(not(target_arch = "wasm32"))]
    use crate::runtime_types::TargetedCommand;
    use crate::session_model::HostSessionModel;

    #[test]
    fn transcript_anchor_lookup_matches_delegate_and_dispatcher_switch_targets() {
        let transcript = TranscriptData {
            turns: vec![crate::fixtures::Turn {
                number: 7,
                blocks: vec![
                    crate::fixtures::TurnBlock::System(AttributedText {
                        text: "Dispatcher switch confirmed (dispatcher-switch-9): supervisor-heavy · openai:gpt-4.1".into(),
                        origin: Some(BlockOrigin {
                            kind: OriginKind::Dispatcher,
                            label: "anthropic:claude-sonnet-4-6".into(),
                        }),
                        notice_kind: Some(SystemNoticeKind::DispatcherSwitch),
                    }),
                    crate::fixtures::TurnBlock::System(AttributedText {
                        text: "Cleave child child-b completed successfully".into(),
                        origin: Some(BlockOrigin {
                            kind: OriginKind::Child,
                            label: "Child child-b".into(),
                        }),
                        notice_kind: Some(SystemNoticeKind::ChildStatus),
                    }),
                ],
            }],
            active_turn: None,
            context_tokens: None,
        };

        assert_eq!(transcript_block_dom_id(7, 1), "turn-7-block-1");
        assert_eq!(
            find_transcript_anchor(&transcript, "delegate:child-b").as_deref(),
            Some("turn-7-block-1")
        );
        assert_eq!(
            find_transcript_anchor(&transcript, "dispatcher-switch:dispatcher-switch-9").as_deref(),
            Some("turn-7-block-0")
        );
        assert_eq!(block_origin_label(None), None);
    }

    #[test]
    fn text_block_class_keeps_dispatcher_text_as_normal_text() {
        let origin = BlockOrigin {
            kind: OriginKind::Dispatcher,
            label: "anthropic:claude-sonnet-4-6".into(),
        };

        assert_eq!(text_block_class(Some(&origin)), "block block-text");
        assert_eq!(text_block_tone(Some(&origin)), "default");
    }

    #[test]
    fn system_block_class_marks_dispatcher_switch_notices_distinctly() {
        let text = AttributedText {
            text: "Dispatcher switch confirmed (dispatcher-switch-1): supervisor-heavy · openai:gpt-4.1".into(),
            origin: Some(BlockOrigin {
                kind: OriginKind::Dispatcher,
                label: "anthropic:claude-sonnet-4-6".into(),
            }),
            notice_kind: Some(SystemNoticeKind::DispatcherSwitch),
        };

        assert_eq!(
            system_block_class(&text),
            "block block-system block-dispatcher-system"
        );
    }

    #[test]
    fn system_block_class_marks_cleave_notices_from_notice_kind() {
        let start = AttributedText {
            text: "Dispatcher requested decomposition into 2 child task(s)".into(),
            origin: Some(BlockOrigin {
                kind: OriginKind::Dispatcher,
                label: "anthropic:claude-sonnet-4-6".into(),
            }),
            notice_kind: Some(SystemNoticeKind::CleaveStart),
        };
        let complete = AttributedText {
            text: "Dispatcher completed decomposition and merged child results".into(),
            origin: Some(BlockOrigin {
                kind: OriginKind::Dispatcher,
                label: "anthropic:claude-sonnet-4-6".into(),
            }),
            notice_kind: Some(SystemNoticeKind::CleaveComplete),
        };

        assert_eq!(
            system_block_class(&start),
            "block block-system block-dispatcher-system block-control-cleave"
        );
        assert_eq!(
            system_block_class(&complete),
            "block block-system block-dispatcher-system block-control-complete"
        );
    }

    #[test]
    fn system_block_class_marks_failures_from_notice_kind() {
        let dispatcher_failure = AttributedText {
            text: "Dispatcher switch failed (dispatcher-switch-1): supervisor-heavy · openai:gpt-4.1 [backend_rejected]".into(),
            origin: Some(BlockOrigin {
                kind: OriginKind::Dispatcher,
                label: "anthropic:claude-sonnet-4-6".into(),
            }),
            notice_kind: Some(SystemNoticeKind::Failure),
        };
        let child_failure = AttributedText {
            text: "Cleave child child-b failed".into(),
            origin: Some(BlockOrigin {
                kind: OriginKind::Child,
                label: "Child child-b".into(),
            }),
            notice_kind: Some(SystemNoticeKind::Failure),
        };

        assert_eq!(
            system_block_class(&dispatcher_failure),
            "block block-system block-dispatcher-system block-control-failure"
        );
        assert_eq!(system_block_tone(&dispatcher_failure), "danger");
        assert_eq!(
            system_block_class(&child_failure),
            "block block-system block-child-origin block-control-failure"
        );
        assert_eq!(system_block_tone(&child_failure), "danger");
    }

    #[test]
    fn system_block_class_marks_child_status_from_notice_kind() {
        let text = AttributedText {
            text: "Cleave child child-a completed successfully".into(),
            origin: Some(BlockOrigin {
                kind: OriginKind::Child,
                label: "Child child-a".into(),
            }),
            notice_kind: Some(SystemNoticeKind::ChildStatus),
        };

        assert_eq!(
            system_block_class(&text),
            "block block-system block-child-origin block-control-child"
        );
    }

    #[test]
    fn tool_status_label_distinguishes_queued_streaming_complete_and_error() {
        let queued = ToolCard {
            id: "tool-1".into(),
            name: "bash".into(),
            args: String::new(),
            partial_output: String::new(),
            result: None,
            is_error: false,
            origin: None,
        };
        let streaming = ToolCard {
            partial_output: "compiling…".into(),
            ..queued.clone()
        };
        let complete = ToolCard {
            result: Some("done".into()),
            ..streaming.clone()
        };
        let errored = ToolCard {
            is_error: true,
            result: Some("boom".into()),
            ..queued.clone()
        };

        assert_eq!(tool_status_label(&queued), "Queued");
        assert_eq!(tool_status_label(&streaming), "Streaming");
        assert_eq!(tool_status_label(&complete), "Complete");
        assert_eq!(tool_status_label(&errored), "Error");
        assert_eq!(tool_visual_state(&queued), "queued");
        assert_eq!(tool_visual_state(&streaming), "streaming");
        assert_eq!(tool_visual_state(&complete), "complete");
        assert_eq!(tool_visual_state(&errored), "error");
        assert_eq!(tool_block_tone(&queued), "muted");
        assert_eq!(tool_block_tone(&streaming), "info");
        assert_eq!(tool_block_tone(&complete), "success");
        assert_eq!(tool_block_tone(&errored), "danger");
    }

    #[test]
    fn tool_labels_and_classes_reflect_partial_and_final_output_state() {
        let running = ToolCard {
            id: "tool-1".into(),
            name: "bash".into(),
            args: "echo hi".into(),
            partial_output: "hi".into(),
            result: None,
            is_error: false,
            origin: None,
        };
        let complete = ToolCard {
            result: Some("status 0".into()),
            ..running.clone()
        };
        let errored = ToolCard {
            is_error: true,
            result: Some("status 1".into()),
            ..running.clone()
        };

        assert_eq!(
            tool_block_class(&running),
            "block block-tool block-tool-running"
        );
        assert_eq!(
            tool_block_class(&complete),
            "block block-tool block-tool-complete"
        );
        assert_eq!(tool_block_class(&errored), "block block-tool block-error");
        assert_eq!(tool_partial_label(&running), "Live output");
        assert_eq!(tool_partial_label(&complete), "Streamed output");
        assert_eq!(tool_result_label(&complete), "Final result");
        assert_eq!(tool_result_label(&errored), "Error result");
    }

    #[test]
    fn transcript_disclosure_helpers_expand_only_verbose_or_human_readable_content() {
        let short = "echo hi";
        let structured_json = r#"{"cmd":"cargo test","cwd":"/tmp/project"}"#;
        let structured_yaml = "---\nname: release\nintent: cut rc";
        let long_lines = (1..=8)
            .map(|i| format!("line-{i}"))
            .collect::<Vec<_>>()
            .join("\n");
        let long_chars = "x".repeat(361);

        assert!(should_expand_tool_args(short));
        assert!(should_expand_tool_output(short));
        assert!(!looks_like_structured_payload(short));
        assert!(looks_like_structured_payload(structured_json));
        assert!(looks_like_structured_payload(structured_yaml));
        assert!(!should_expand_tool_args(structured_json));
        assert!(!should_expand_tool_output(structured_yaml));
        assert!(!should_expand_tool_args(&long_lines));
        assert!(!should_expand_tool_output(&long_chars));
        assert_eq!(
            transcript_disclosure_meta("alpha\nbeta"),
            "2 lines · 10 chars"
        );
    }

    #[test]
    fn transcript_disclosure_open_respects_operator_setting() {
        assert!(transcript_disclosure_open(true, true));
        assert!(!transcript_disclosure_open(true, false));
        assert!(!transcript_disclosure_open(false, true));
    }

    #[test]
    fn left_rail_inventory_audit_summary_uses_latest_entry() {
        let controller = AppController::default();
        let panel = build_audit_panel_model(
            controller.audit_timeline(),
            &controller.current_audit_session_key(),
            &AuditFilters::default(),
        );

        assert_eq!(panel.total_count, 0);
        assert_eq!(panel.filtered_count, 0);
        assert_eq!(panel.latest_label, "No transcript blocks retained yet");
    }

    #[test]
    fn chat_empty_state_uses_new_project_starter_when_history_is_empty() {
        let controller = AppController::default();
        let model = build_chat_empty_state_model(
            controller.summary(),
            &controller.work_data(),
            &controller.session_data(),
            controller.transcript(),
            controller.messages(),
            controller.scenario(),
        )
        .expect("default controller should expose the starter state");

        assert_eq!(model.title, "New Project starter");
        assert!(!model.detached);
        assert!(model.kicker.contains("main"));
        assert!(
            model
                .detail
                .contains("No transcript history is attached yet")
        );
        assert_eq!(model.guidance.len(), 3);
        assert!(model.guidance[0].contains("Summarize the current session"));
        assert!(model.guidance[1].contains("right model"));
    }

    #[test]
    fn chat_empty_state_calls_out_detached_workspace() {
        let controller = AppController::default();
        let mut session = controller.session_data();
        session.git_detached = true;
        session.git_branch = Some("feature/prototype".into());

        let model = build_chat_empty_state_model(
            controller.summary(),
            &controller.work_data(),
            &session,
            controller.transcript(),
            controller.messages(),
            controller.scenario(),
        )
        .expect("detached empty state should still render starter guidance");

        assert!(model.detached);
        assert!(model.kicker.contains("Detached workspace"));
        assert!(model.detail.contains("detached from feature/prototype"));
        assert!(model.guidance[2].contains("reattach the workspace"));
    }

    #[test]
    fn chat_empty_state_hides_once_transcript_turns_exist() {
        let controller = AppController::default();
        let transcript = TranscriptData {
            turns: vec![crate::fixtures::Turn {
                number: 1,
                blocks: vec![crate::fixtures::TurnBlock::Text(AttributedText {
                    text: "hello".into(),
                    origin: None,
                    notice_kind: None,
                })],
            }],
            active_turn: None,
            context_tokens: None,
        };

        let model = build_chat_empty_state_model(
            controller.summary(),
            &controller.work_data(),
            &controller.session_data(),
            &transcript,
            controller.messages(),
            controller.scenario(),
        );

        assert!(model.is_none());
    }

    #[test]
    fn audit_panel_filters_by_session_turn_kind_and_text() {
        let store = AuditTimelineStore::from_json(
            r#"{
              "schema_version": 1,
              "entries": [
                {
                  "session_key": "remote:a",
                  "turn_number": 4,
                  "block_index": 0,
                  "block_id": "remote:a:turn-4-block-0",
                  "kind": "System",
                  "label": "Dispatcher",
                  "content": "Switch confirmed"
                },
                {
                  "session_key": "remote:b",
                  "turn_number": 9,
                  "block_index": 1,
                  "block_id": "remote:b:turn-9-block-1",
                  "kind": "Tool",
                  "label": "Tool · bash",
                  "content": "cargo test"
                }
              ]
            }"#,
        )
        .expect("audit timeline fixture should deserialize");

        let filters = AuditFilters {
            session_key: "remote:b".into(),
            turn_query: "9".into(),
            kind_key: "tool".into(),
            text_query: "cargo".into(),
        };
        let panel = build_audit_panel_model(&store, "remote:b", &filters);

        assert_eq!(panel.total_count, 2);
        assert_eq!(panel.filtered_count, 1);
        assert_eq!(
            panel.session_options,
            vec!["remote:a".to_string(), "remote:b".to_string()]
        );
        assert_eq!(panel.entries[0].heading, "Tool · bash");
        assert_eq!(panel.entries[0].kind_key, "tool");
        assert_eq!(panel.entries[0].meta, "remote:b · turn 9 · Tool");
        assert_eq!(
            panel.entries[0].focus_target.as_deref(),
            Some("turn-9-block-1")
        );
    }

    #[test]
    fn audit_panel_only_offers_focus_for_current_session_entries() {
        let store = AuditTimelineStore::from_json(
            r#"{
              "schema_version": 1,
              "entries": [
                {
                  "session_key": "remote:a",
                  "turn_number": 4,
                  "block_index": 0,
                  "block_id": "remote:a:turn-4-block-0",
                  "kind": "System",
                  "label": "Dispatcher",
                  "content": "Switch confirmed"
                }
              ]
            }"#,
        )
        .expect("audit timeline fixture should deserialize");

        let panel = build_audit_panel_model(&store, "remote:b", &AuditFilters::default());
        assert_eq!(panel.entries[0].focus_target, None);
    }

    #[test]
    fn audit_entry_filter_rejects_invalid_turn_query() {
        let entry = AuditEntry {
            session_key: "remote:a".into(),
            turn_number: 3,
            block_index: 0,
            block_id: "remote:a:turn-3-block-0".into(),
            kind: AuditEntryKind::Thinking,
            label: "Thinking".into(),
            content: "inspect".into(),
        };

        assert!(!audit_entry_matches_filters(
            &entry,
            &AuditFilters {
                turn_query: "three".into(),
                ..AuditFilters::default()
            }
        ));
        assert_eq!(audit_kind_key(AuditEntryKind::Thinking), "thinking");
    }

    #[test]
    fn system_notice_disclosure_and_labels_follow_notice_kind() {
        let short_generic = AttributedText {
            text: "Background refresh complete".into(),
            origin: None,
            notice_kind: Some(SystemNoticeKind::Generic),
        };
        let long_failure = AttributedText {
            text: (1..=6)
                .map(|i| format!("failure detail {i}"))
                .collect::<Vec<_>>()
                .join("\n"),
            origin: Some(BlockOrigin {
                kind: OriginKind::Dispatcher,
                label: "anthropic:claude-sonnet-4-6".into(),
            }),
            notice_kind: Some(SystemNoticeKind::Failure),
        };

        assert!(!should_expand_system_notice(&short_generic.text));
        assert!(should_expand_system_notice(&long_failure.text));
        assert_eq!(system_notice_summary_label(&short_generic), "System notice");
        assert_eq!(system_notice_summary_label(&long_failure), "Failure notice");
    }

    #[test]
    fn context_window_label_formats_reported_usage() {
        let session = crate::fixtures::SessionData {
            context_tokens: Some(640),
            context_window: Some(200_000),
            ..Default::default()
        };
        let no_window = crate::fixtures::SessionData {
            context_tokens: Some(640),
            context_window: None,
            ..Default::default()
        };

        assert_eq!(context_window_label(&session), "640 / 200000 tokens");
        assert_eq!(context_window_label(&no_window), "640 tokens");
        assert_eq!(
            context_window_label(&crate::fixtures::SessionData::default()),
            "Context usage not reported yet"
        );
    }

    #[test]
    fn cockpit_summary_model_prioritizes_primary_omegon_identity_and_activity_semantics() {
        let controller = AppController::remote_demo();
        let model = super::build_cockpit_summary_model(
            Workspace::Chat,
            SessionMode::Live,
            controller.summary(),
            &controller.session_data(),
        );

        assert_eq!(model.auspex.label, "Auspex");
        assert_eq!(model.attached.label, "Primary Omegon");
        assert_eq!(model.attached.tag, "LIVE");
        assert!(model.attached.primary.contains("primary-driver"));
        assert!(model.attached.secondary[0].contains("primary-interactive"));
        assert!(model.deployment.primary.contains("total"));
        assert!(
            model.deployment.secondary[1].contains("seen:")
                || !controller
                    .session_data()
                    .telemetry
                    .lifecycle
                    .instances
                    .is_empty()
        );
        assert!(!model.activity.primary.is_empty());
        assert_eq!(model.activity.label, "Activity");
    }

    #[test]
    fn cockpit_summary_model_uses_owned_runtime_states_for_unbound_primary() {
        let summary = crate::fixtures::HostSessionSummary {
            connection: "Starting owned runtime".into(),
            activity: "Launching omegon serve".into(),
            activity_kind: crate::fixtures::ActivityKind::Waiting,
            work: "Attach to owned runtime".into(),
        };
        let session = crate::fixtures::SessionData::default();
        let model = super::build_cockpit_summary_model(
            Workspace::Chat,
            SessionMode::Live,
            &summary,
            &session,
        );

        assert_eq!(model.attached.label, "Primary Omegon");
        assert_eq!(model.attached.tag, "ATTACHING");
        assert_eq!(model.attached.primary, "Primary runtime unbound");
        assert!(model.attached.secondary[1].contains("booting/attaching"));
        assert!(model.deployment.preview.is_empty());
        assert!(model.activity.preview.is_empty());
    }

    #[test]
    fn cockpit_summary_model_exposes_preview_rails_for_field_overview() {
        let controller = AppController::remote_demo();
        let model = super::build_cockpit_summary_model(
            Workspace::Chat,
            SessionMode::Live,
            controller.summary(),
            &controller.session_data(),
        );

        assert!(model.deployment.preview.len() <= 4);
        assert!(model.activity.preview.len() <= 4);
        if !model.activity.preview.is_empty() {
            assert!(model.activity.preview[0].label.contains("·"));
        }
    }

    #[test]
    fn chat_cop_host_model_carries_focus_and_footer_state() {
        let summary = HostSessionSummary {
            connection: "Attached to local shell".into(),
            activity: "Waiting for input".into(),
            activity_kind: ActivityKind::Idle,
            work: "No focused work".into(),
        };
        let model = super::ChatCopHostModel {
            summary: &summary,
            work: &crate::fixtures::WorkData::default(),
            session: &crate::fixtures::SessionData::default(),
            transcript: &TranscriptData::default(),
            messages: &[],
            scenario: crate::fixtures::DevScenario::Ready,
            auto_expand: false,
            is_run_active: false,
            can_submit: true,
            draft: "hello",
            dispatch_context: &super::DispatchContextStripModel {
                state: "ready",
                send_detail: "Prompt ready".into(),
                items: vec![],
            },
            provider_blocked_composer: None,
            composer_ready_notice: None,
        };

        assert_eq!(model.draft, "hello");
        assert!(model.can_submit);
        assert!(!model.is_run_active);
    }

    #[test]
    fn cockpit_summary_model_uses_coherent_completed_activity_state() {
        let summary = HostSessionSummary {
            connection: "primary_driver attached to workspace unknown".into(),
            activity: "Agent turn finished".into(),
            activity_kind: ActivityKind::Completed,
            work: "No focused work".into(),
        };
        let model = super::build_cockpit_summary_model(
            Workspace::Chat,
            SessionMode::Live,
            &summary,
            &crate::fixtures::SessionData::default(),
        );

        assert_eq!(model.activity.tag, "COMPLETED");
        assert_eq!(model.activity.primary, "Agent turn finished");
        assert_eq!(model.activity.secondary[0], "No focused work");
    }

    #[test]
    fn dispatch_context_strip_prefers_dispatcher_identity_and_ready_send_state() {
        let controller = AppController::remote_demo();
        let model = build_dispatch_context_strip_model(
            Workspace::Chat,
            SessionMode::Live,
            controller.summary(),
            &controller.session_data(),
            "Inspect the current dispatcher posture.",
            false,
            true,
        );

        assert_eq!(model.state, "ready");
        assert!(model.send_detail.contains("Prompt ready: 39 character(s)"));
        assert!(model.items.contains(&super::DispatchContextItem {
            label: "Route",
            value: "chat · live".into(),
            tone: "muted",
        }));
        assert!(model.items.contains(&super::DispatchContextItem {
            label: "Session",
            value: "session_01HVDEMO".into(),
            tone: "muted",
        }));
        assert!(model.items.contains(&super::DispatchContextItem {
            label: "Who",
            value: "primary-driver".into(),
            tone: "accent",
        }));
        assert!(model.items.contains(&super::DispatchContextItem {
            label: "Model",
            value: "anthropic:claude-sonnet-4-6".into(),
            tone: "accent",
        }));
        assert!(model.items.contains(&super::DispatchContextItem {
            label: "Thinking",
            value: "medium".into(),
            tone: "muted",
        }));
        assert!(model.items.contains(&super::DispatchContextItem {
            label: "Tier",
            value: "victory".into(),
            tone: "muted",
        }));
        assert!(
            model
                .items
                .iter()
                .any(|item| item.label == "Send" && item.value == "Ready to send")
        );

        let rendered = render_dispatch_context_strip(&model);
        let debug = format!("{rendered:?}");
        assert!(debug.contains("Dispatch context"));
        assert!(debug.contains("primary-driver"));
        assert!(debug.contains("Ready to send"));
    }

    #[test]
    fn dispatch_context_strip_reports_blocked_and_blank_send_states() {
        let summary = HostSessionSummary {
            connection: "Attached to local shell".into(),
            activity: "Waiting for input".into(),
            activity_kind: ActivityKind::Idle,
            work: "No focused work".into(),
        };
        let session = crate::fixtures::SessionData {
            git_branch: Some("main".into()),
            thinking_level: "low".into(),
            capability_tier: "retribution".into(),
            providers: vec![crate::fixtures::ProviderInfo {
                name: "Anthropic".into(),
                authenticated: true,
                auth_method: Some("oauth".into()),
                model: Some("claude-sonnet".into()),
            }],
            ..Default::default()
        };

        let active_run = build_dispatch_context_strip_model(
            Workspace::Chat,
            SessionMode::Mock,
            &summary,
            &session,
            "hello",
            true,
            false,
        );
        assert_eq!(active_run.state, "running");
        assert!(
            active_run
                .items
                .iter()
                .any(|item| item.label == "Send" && item.value == "Blocked by active run")
        );

        let blank = build_dispatch_context_strip_model(
            Workspace::Chat,
            SessionMode::Mock,
            &summary,
            &session,
            "   ",
            false,
            true,
        );
        assert_eq!(blank.state, "ready");
        assert!(
            blank
                .items
                .iter()
                .any(|item| item.label == "Send" && item.value == "Needs prompt text")
        );
        assert!(
            blank
                .items
                .iter()
                .any(|item| item.label == "Who" && item.value == "Attached to local shell")
        );

        let blocked_providers = build_dispatch_context_strip_model(
            Workspace::Chat,
            SessionMode::Live,
            &summary,
            &crate::fixtures::SessionData {
                providers: vec![],
                ..session.clone()
            },
            "hello",
            false,
            false,
        );
        assert!(
            blocked_providers
                .items
                .iter()
                .any(|item| item.label == "Send" && item.value == "Host missing providers")
        );
        assert!(
            blocked_providers
                .send_detail
                .contains("authenticated providers")
        );
    }

    #[test]
    fn provider_blocked_composer_model_requires_actual_submit_readiness() {
        let blocked = build_provider_blocked_composer_model(
            &crate::fixtures::SessionData {
                providers: vec![],
                ..Default::default()
            },
            false,
        )
        .expect("blocked state should render a setup callout");
        assert_eq!(blocked.title, "Prompt execution blocked");
        assert!(
            blocked
                .detail
                .contains("Authenticate a provider in Settings")
        );
        assert_eq!(blocked.action_label, "Open Settings");

        let still_blocked = build_provider_blocked_composer_model(
            &crate::fixtures::SessionData {
                providers: vec![crate::fixtures::ProviderInfo {
                    name: "Anthropic".into(),
                    authenticated: true,
                    auth_method: Some("oauth".into()),
                    model: Some("claude-sonnet".into()),
                }],
                ..Default::default()
            },
            false,
        )
        .expect("authenticated inventory alone must not unblock prompting");
        assert_eq!(still_blocked.title, "Prompt execution blocked");

        assert!(
            build_provider_blocked_composer_model(
                &crate::fixtures::SessionData {
                    providers: vec![crate::fixtures::ProviderInfo {
                        name: "Anthropic".into(),
                        authenticated: true,
                        auth_method: Some("oauth".into()),
                        model: Some("claude-sonnet".into()),
                    }],
                    ..Default::default()
                },
                true,
            )
            .is_none()
        );
    }

    #[test]
    fn settings_panel_model_prefers_dispatcher_targeting() {
        let controller = AppController::remote_demo();
        let model = build_settings_panel_model(
            &controller,
            &controller.session_data(),
            Some(controller.settings_auth_state()),
        );

        assert_eq!(model.selected_route_id, "session-dispatcher");
        assert_eq!(model.target_label, "primary-driver · omg_primary_01HVDEMO");
        assert!(model.target_detail.contains("primary-driver"));
        assert!(model.route_detail.contains("session-dispatcher"));
        assert!(model.lifecycle_summary.contains("Lifecycle:"));
        assert_eq!(model.auth_status_label, "1 authenticated provider(s)");
        assert_eq!(
            model.general_actions[0].action.command_slug(),
            "auth.refresh"
        );
        assert!(model.general_actions.iter().all(|action| action.enabled));
        assert_eq!(model.provider_cards.len(), 1);
        assert_eq!(model.provider_cards[0].status_label, "Authenticated");
        assert!(
            model
                .secrets_rows
                .iter()
                .any(|row| row.label == "Provider coverage" && row.value == "1 / 1 authenticated")
        );
    }

    #[test]
    fn settings_panel_model_falls_back_to_local_shell() {
        let controller = AppController::default();
        let model = build_settings_panel_model(
            &controller,
            &controller.session_data(),
            Some(controller.settings_auth_state()),
        );

        assert_eq!(model.target_label, "Local shell");
        assert!(
            model.route_detail.contains("local-shell")
                || model.route_detail.contains("Local shell")
        );
        assert!(model.lifecycle_summary.contains("Lifecycle:"));
        assert_eq!(model.auth_status_label, "1 authenticated provider(s)");
        assert!(model.general_actions.iter().all(|action| action.enabled));
        assert!(
            model
                .provider_guidance
                .contains("Authenticated providers can execute")
        );
    }

    #[test]
    fn settings_panel_model_uses_detached_host_label_for_remote_fallback() {
        let controller = AppController::from_remote_snapshot_json_with_registry(
            r#"{
              "design": {"focused": null, "implementing": [], "actionable": [], "all_nodes": [], "counts": {}},
              "openspec": {"total_tasks": 0, "done_tasks": 0},
              "cleave": {"active": false, "total_children": 0, "completed": 0, "failed": 0},
              "session": {"turns": 0, "tool_calls": 0, "compactions": 0},
              "harness": {"git_branch":"main","git_detached":false,"thinking_level":"medium","capability_tier":"victory","providers":[],"memory_available":true,"cleave_available":true,"memory_warning":null,"active_delegates":[]}
            }"#,
            crate::instance_registry::InstanceRegistryStore::default(),
        )
        .unwrap();
        let model = build_settings_panel_model(
            &controller,
            &controller.session_data(),
            Some(controller.settings_auth_state()),
        );

        assert_eq!(model.target_label, "Detached host session");
        assert!(
            model
                .target_detail
                .contains("No attached host instance reported")
        );
    }

    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    fn settings_panel_model_merges_runtime_and_settings_provider_metadata() {
        let controller = AppController::remote_demo();
        let auth_state = crate::controller::SettingsAuthState {
            providers: vec![
                crate::fixtures::ProviderInfo {
                    name: "Anthropic/Claude".into(),
                    authenticated: true,
                    auth_method: Some("oauth".into()),
                    model: None,
                },
                crate::fixtures::ProviderInfo {
                    name: "OpenAI/Codex".into(),
                    authenticated: true,
                    auth_method: Some("oauth".into()),
                    model: Some("gpt-4.1".into()),
                },
            ],
            last_error: None,
            last_action: None,
            inventory_refreshed: true,
        };
        let model =
            build_settings_panel_model(&controller, &controller.session_data(), Some(&auth_state));

        assert_eq!(model.auth_status_label, "2 authenticated provider(s)");
        assert_eq!(model.provider_cards.len(), 2);
        assert_eq!(model.provider_cards[0].name, "Anthropic");
        assert!(
            model.provider_cards[0]
                .capability_detail
                .contains("claude-sonnet")
        );
        assert_eq!(model.provider_cards[1].name, "OpenAI/Codex");
        assert!(
            model.provider_cards[1]
                .capability_detail
                .contains("gpt-4.1")
        );
    }

    #[test]
    fn settings_panel_model_disables_provider_actions_without_inventory() {
        let controller = AppController::default();
        let session = crate::fixtures::SessionData::default();
        let model = build_settings_panel_model(&controller, &session, None);

        assert_eq!(model.auth_status_label, "No providers reported");
        assert_eq!(model.provider_cards.len(), 1);
        assert!(!model.provider_cards[0].login_action.enabled);
        assert!(!model.provider_cards[0].logout_action.enabled);
        assert!(model.provider_guidance.contains("Refresh status first"));
    }

    #[test]
    fn settings_auth_actions_expose_stable_labels_and_command_slugs() {
        assert_eq!(SettingsAuthAction::Refresh.label(), "Refresh status");
        assert_eq!(SettingsAuthAction::Refresh.command_slug(), "auth.refresh");
        assert_eq!(SettingsAuthAction::Login.command_slug(), "auth.login");
        assert_eq!(SettingsAuthAction::Logout.command_slug(), "auth.logout");
        assert_eq!(SettingsAuthAction::Unlock.command_slug(), "auth.unlock");

        let slash = crate::runtime_types::CanonicalSlashCommand {
            name: "login".into(),
            args: "anthropic".into(),
            raw_input: "/login anthropic".into(),
        };
        let targeted = crate::runtime_types::TargetedCommand::canonical_slash(
            crate::runtime_types::CommandTarget {
                session_key: "remote:session_01HVDEMO".into(),
                dispatcher_instance_id: Some("omg_primary_01HVDEMO".into()),
            },
            slash,
        );
        assert_eq!(
            targeted.compatibility_command_json(),
            r#"{"args":"anthropic","name":"login","type":"slash_command"}"#
        );
        assert_eq!(
            targeted.transport_json().unwrap(),
            r#"{"target":{"session_key":"remote:session_01HVDEMO","dispatcher_instance_id":"omg_primary_01HVDEMO"},"command":{"kind":"canonical_slash","slash":{"name":"login","args":"anthropic","raw_input":"/login anthropic"}}}"#
        );
    }

    #[test]
    fn dispatch_targeted_command_requires_ipc_transport() {
        let command = TargetedCommand::legacy_json(
            crate::runtime_types::CommandTarget {
                session_key: "remote:session_01HVDEMO".into(),
                dispatcher_instance_id: Some("omg_primary_01HVDEMO".into()),
            },
            r#"{"type":"user_prompt","text":"hello"}"#,
        );
        let transport = crate::command_transport::CommandTransport::Ipc(
            crate::ipc_client::IpcCommandClient::new("/tmp/nonexistent-omegon.sock"),
        );

        let result = dispatch_targeted_command(&transport, None, &command);

        assert!(result.is_err());
    }

    #[test]
    fn dispatch_targeted_command_supports_websocket_transport_for_remote_control() {
        let handle = EventStreamHandle::websocket("ws://127.0.0.1:1/ws");
        let command = TargetedCommand::legacy_json(
            crate::runtime_types::CommandTarget {
                session_key: "remote:session_01HVDEMO".into(),
                dispatcher_instance_id: Some("omg_primary_01HVDEMO".into()),
            },
            r#"{"type":"user_prompt","text":"hello"}"#,
        );
        let transport = crate::command_transport::CommandTransport::EventStream;

        let result = dispatch_targeted_command(&transport, Some(&handle), &command);

        assert!(result.is_ok());
        assert_eq!(
            handle.debug_drain_outbox(),
            vec![r#"{"type":"user_prompt","text":"hello"}"#.to_string()]
        );
    }

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
    fn chat_status_banner_reports_run_state() {
        let summary = HostSessionSummary {
            connection: "Attached".into(),
            activity: "Waiting for input".into(),
            activity_kind: ActivityKind::Idle,
            work: "No focused work".into(),
        };
        let session = crate::fixtures::SessionData::default();
        let banner = render_chat_status_banner(&summary, &session, true, false);
        let debug = format!("{banner:?}");

        assert!(debug.contains("Run active"));
        assert!(debug.contains("current run completes"));
        assert_eq!(chat_status_tone(true, false), "info");
        assert_eq!(chat_status_tone(false, false), "warn");
        assert_eq!(chat_status_tone(false, true), "success");
    }

    #[test]
    fn app_surface_helpers_assign_semantic_state_and_tone() {
        use crate::fixtures::AppSurfaceKind;

        assert_eq!(app_surface_state(AppSurfaceKind::Startup), "starting");
        assert_eq!(app_surface_tone(AppSurfaceKind::Startup), "info");
        assert_eq!(
            app_surface_state(AppSurfaceKind::Reconnecting),
            "reconnecting"
        );
        assert_eq!(app_surface_tone(AppSurfaceKind::Reconnecting), "warn");
        assert_eq!(
            app_surface_state(AppSurfaceKind::CompatibilityFailure),
            "compatibility-failure"
        );
        assert_eq!(
            app_surface_tone(AppSurfaceKind::CompatibilityFailure),
            "danger"
        );
    }

    #[test]
    fn left_rail_inventory_prefers_dispatcher_and_delegate_state() {
        let controller = AppController::remote_demo();
        let inventory = build_left_rail_inventory(
            controller.summary(),
            &controller.work_data(),
            &controller.session_data(),
            controller.is_run_active(),
        );

        assert_eq!(inventory.workspace_label, "omg_primary_01HVDEMO");
        assert_eq!(inventory.workspace_detail, "workspace · branch main");
        assert_eq!(inventory.session_label, "session_01HVDEMO");
        assert_eq!(
            inventory.session_detail,
            "primary-driver · anthropic:claude-sonnet-4-6"
        );
        assert!(inventory.agent_rows[0].0.contains("Dispatcher"));
    }

    #[test]
    fn left_rail_inventory_falls_back_when_dispatcher_absent() {
        let controller = AppController::default();
        let inventory = build_left_rail_inventory(
            controller.summary(),
            &controller.work_data(),
            &controller.session_data(),
            controller.is_run_active(),
        );

        assert_eq!(inventory.workspace_label, "main");
        assert_eq!(inventory.workspace_detail, "workspace · branch main");
        assert_eq!(inventory.session_label, "local-session");
        assert_eq!(inventory.agent_rows[0].0, "No dispatcher binding");
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

    #[test]
    fn remote_demo_controller_exposes_remote_mode() {
        let controller = AppController::remote_demo();

        assert!(controller.is_remote());
        assert!(
            controller
                .summary()
                .connection
                .contains("Attached to Omegon host")
        );
        assert_eq!(controller.messages().len(), 1);
    }
}
