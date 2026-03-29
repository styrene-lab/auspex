use crate::fixtures::{
    ChatMessage, ComposerState, DevScenario, HostSessionSummary, MessageRole, ProviderInfo,
    SessionData, ShellState, WorkData, WorkNode,
};
use crate::omegon_control::{HarnessStatusSnapshot, OmegonEvent, OmegonStateSnapshot};
use crate::session_model::HostSessionModel;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RemoteHostSession {
    shell_state: ShellState,
    scenario: DevScenario,
    summary: HostSessionSummary,
    messages: Vec<ChatMessage>,
    composer: ComposerState,
    pending_role: Option<MessageRole>,
    pending_text: String,
    run_active: bool,
    // Raw snapshot sub-sections kept for Power mode screens.
    design: crate::omegon_control::DesignSnapshot,
    openspec: crate::omegon_control::OpenSpecSnapshot,
    cleave: crate::omegon_control::CleaveSnapshot,
    session_stats: crate::omegon_control::SessionSnapshot,
    harness_snapshot: Option<HarnessStatusSnapshot>,
}

impl RemoteHostSession {
    pub fn from_snapshot(snapshot: OmegonStateSnapshot) -> Self {
        let (shell_state, scenario) = status_from_harness(snapshot.harness.as_ref());
        let summary = summary_from_snapshot(&snapshot);

        Self {
            shell_state,
            scenario,
            summary,
            messages: vec![ChatMessage {
                role: MessageRole::System,
                text: "Attached to Omegon host control plane. Live transcript will appear here as WebSocket events arrive.".into(),
            }],
            composer: ComposerState::default(),
            pending_role: None,
            pending_text: String::new(),
            run_active: false,
            design: snapshot.design,
            openspec: snapshot.openspec,
            cleave: snapshot.cleave,
            session_stats: snapshot.session,
            harness_snapshot: snapshot.harness.clone(),
        }
    }

    pub fn from_snapshot_json(json: &str) -> Result<Self, serde_json::Error> {
        let snapshot = serde_json::from_str::<OmegonStateSnapshot>(json)?;
        Ok(Self::from_snapshot(snapshot))
    }

    pub fn apply_event(&mut self, event: OmegonEvent) -> bool {
        match event {
            OmegonEvent::StateSnapshot { data } => {
                let (shell_state, scenario) = status_from_harness(data.harness.as_ref());
                self.shell_state = shell_state;
                self.scenario = scenario;
                self.summary = summary_from_snapshot(&data);
                self.design = data.design;
                self.openspec = data.openspec;
                self.cleave = data.cleave;
                self.session_stats = data.session;
                self.harness_snapshot = data.harness;
                true
            }
            OmegonEvent::MessageStart { role } => {
                self.pending_role = Some(role_from_wire(&role));
                self.pending_text.clear();
                true
            }
            OmegonEvent::MessageChunk { text } | OmegonEvent::ThinkingChunk { text } => {
                if self.pending_role.is_some() {
                    self.pending_text.push_str(&text);
                    true
                } else {
                    false
                }
            }
            OmegonEvent::MessageEnd => {
                let Some(role) = self.pending_role.take() else {
                    return false;
                };
                self.messages.push(ChatMessage {
                    role,
                    text: self.pending_text.trim().to_string(),
                });
                self.pending_text.clear();
                true
            }
            OmegonEvent::SystemNotification { message } => {
                self.messages.push(ChatMessage {
                    role: MessageRole::System,
                    text: message,
                });
                true
            }
            OmegonEvent::HarnessStatusChanged { status } => {
                let (shell_state, scenario) = status_from_harness(Some(&status));
                self.shell_state = shell_state;
                self.scenario = scenario;
                apply_harness_summary(&mut self.summary, &status);
                self.harness_snapshot = Some(status);
                true
            }
            OmegonEvent::SessionReset => {
                self.messages.clear();
                self.messages.push(ChatMessage {
                    role: MessageRole::System,
                    text: "Omegon reported a session reset. Auspex cleared the cached transcript and is waiting for fresh host events.".into(),
                });
                self.pending_role = None;
                self.pending_text.clear();
                self.run_active = false;
                true
            }
            OmegonEvent::TurnStart { turn } => {
                self.run_active = true;
                self.summary.activity = format!("Turn {turn} in progress");
                true
            }
            OmegonEvent::TurnEnd { turn } => {
                self.run_active = false;
                self.summary.activity = format!("Turn {turn} completed");
                true
            }
            OmegonEvent::ToolStart { name, .. } => {
                self.summary.activity = format!("Running tool {name}");
                true
            }
            OmegonEvent::ToolUpdate { .. } => true,
            OmegonEvent::ToolEnd { is_error, .. } => {
                self.summary.activity = if is_error {
                    "Tool run completed with an error".into()
                } else {
                    "Tool run completed".into()
                };
                true
            }
            OmegonEvent::AgentEnd => {
                self.run_active = false;
                self.summary.activity = "Agent turn finished".into();
                true
            }
            OmegonEvent::PhaseChanged { phase } => {
                self.summary.activity = format!("Lifecycle phase: {phase}");
                true
            }
            OmegonEvent::DecompositionStarted { children } => {
                self.summary.activity =
                    format!("Cleave started with {} child task(s)", children.len());
                true
            }
            OmegonEvent::DecompositionChildCompleted { label, success } => {
                self.messages.push(ChatMessage {
                    role: MessageRole::System,
                    text: format!(
                        "Cleave child {label} {}",
                        if success {
                            "completed successfully"
                        } else {
                            "failed"
                        }
                    ),
                });
                true
            }
            OmegonEvent::DecompositionCompleted { merged } => {
                self.summary.activity = if merged {
                    "Cleave completed and merged".into()
                } else {
                    "Cleave completed without merge".into()
                };
                true
            }
        }
    }

    pub fn apply_event_json(&mut self, json: &str) -> Result<bool, serde_json::Error> {
        let event = serde_json::from_str::<OmegonEvent>(json)?;
        Ok(self.apply_event(event))
    }
}

impl HostSessionModel for RemoteHostSession {
    fn shell_state(&self) -> ShellState {
        self.shell_state
    }

    fn scenario(&self) -> DevScenario {
        self.scenario
    }

    fn summary(&self) -> &HostSessionSummary {
        &self.summary
    }

    fn messages(&self) -> &[ChatMessage] {
        &self.messages
    }

    fn composer(&self) -> &ComposerState {
        &self.composer
    }

    fn composer_mut(&mut self) -> &mut ComposerState {
        &mut self.composer
    }

    fn set_scenario(&mut self, scenario: DevScenario) {
        self.scenario = scenario;
        self.shell_state = match scenario {
            DevScenario::Ready => ShellState::Ready,
            DevScenario::Booting => ShellState::StartingOmegon,
            DevScenario::Degraded => ShellState::Degraded,
            DevScenario::CompatibilityFailure => ShellState::Failed,
            DevScenario::Reconnecting => ShellState::CompatibilityChecking,
        };
    }

    fn can_submit(&self) -> bool {
        !self.run_active && matches!(self.shell_state, ShellState::Ready | ShellState::Degraded)
    }

    fn is_run_active(&self) -> bool {
        self.run_active
    }

    fn work_data(&self) -> WorkData {
        let focused = self.design.focused.as_ref();
        WorkData {
            focused_id: focused.map(|n| n.id.clone()),
            focused_title: focused.map(|n| n.title.clone()),
            focused_status: focused.map(|n| n.status.clone()),
            open_question_count: focused.map(|n| n.open_questions.len()).unwrap_or(0),
            implementing: self
                .design
                .implementing
                .iter()
                .map(|n| WorkNode {
                    id: n.id.clone(),
                    title: n.title.clone(),
                    status: n.status.clone(),
                })
                .collect(),
            actionable: self
                .design
                .actionable
                .iter()
                .map(|n| WorkNode {
                    id: n.id.clone(),
                    title: n.title.clone(),
                    status: n.status.clone(),
                })
                .collect(),
            openspec_total: self.openspec.total_tasks,
            openspec_done: self.openspec.done_tasks,
            cleave_active: self.cleave.active,
            cleave_total: self.cleave.total_children,
            cleave_completed: self.cleave.completed,
            cleave_failed: self.cleave.failed,
        }
    }

    fn session_data(&self) -> SessionData {
        let h = self.harness_snapshot.as_ref();
        SessionData {
            git_branch: h.and_then(|h| h.git_branch.clone()),
            git_detached: h.map(|h| h.git_detached).unwrap_or(false),
            thinking_level: h.map(|h| h.thinking_level.clone()).unwrap_or_default(),
            capability_tier: h.map(|h| h.capability_tier.clone()).unwrap_or_default(),
            providers: h
                .map(|h| {
                    h.providers
                        .iter()
                        .map(|p| ProviderInfo {
                            name: p.name.clone(),
                            authenticated: p.authenticated,
                            model: p.model.clone(),
                        })
                        .collect()
                })
                .unwrap_or_default(),
            memory_available: h.map(|h| h.memory_available).unwrap_or(true),
            cleave_available: h.map(|h| h.cleave_available).unwrap_or(true),
            memory_warning: h.and_then(|h| h.memory_warning.clone()),
            active_delegate_count: h.map(|h| h.active_delegates.len()).unwrap_or(0),
            session_turns: self.session_stats.turns,
            session_tool_calls: self.session_stats.tool_calls,
            session_compactions: self.session_stats.compactions,
        }
    }

    fn submit(&mut self) -> bool {
        if !self.can_submit() {
            return false;
        }

        let trimmed = self.composer.draft().trim();
        if trimmed.is_empty() {
            return false;
        }

        self.messages.push(ChatMessage {
            role: MessageRole::User,
            text: trimmed.to_string(),
        });
        self.summary.activity = "Queued prompt for Omegon remote session".into();
        self.composer.clear();
        true
    }
}

fn summary_from_snapshot(snapshot: &OmegonStateSnapshot) -> HostSessionSummary {
    let connection = match snapshot.harness.as_ref() {
        Some(harness) => {
            let branch = harness.git_branch.as_deref().unwrap_or("detached");
            let provider = harness
                .providers
                .iter()
                .find_map(|provider| {
                    provider
                        .model
                        .as_ref()
                        .map(|model| format!("{} {model}", provider.name))
                })
                .unwrap_or_else(|| "provider unknown".into());
            format!("Attached to Omegon host on branch {branch} ({provider})")
        }
        None => "Attached to Omegon host session".into(),
    };

    let activity = if snapshot.cleave.active {
        format!(
            "Parallel work running: {}/{} children complete",
            snapshot.cleave.completed, snapshot.cleave.total_children
        )
    } else if let Some(focused) = snapshot.design.focused.as_ref() {
        format!("Focused on {} ({})", focused.title, focused.status)
    } else {
        format!(
            "Session stats: {} turns, {} tool calls, {} compactions",
            snapshot.session.turns, snapshot.session.tool_calls, snapshot.session.compactions
        )
    };

    let work = if let Some(focused) = snapshot.design.focused.as_ref() {
        format!("Focused node: {}", focused.title)
    } else if !snapshot.design.implementing.is_empty() {
        format!(
            "{} implementation item(s) active",
            snapshot.design.implementing.len()
        )
    } else if snapshot.openspec.total_tasks > 0 {
        format!(
            "OpenSpec progress: {}/{} tasks done",
            snapshot.openspec.done_tasks, snapshot.openspec.total_tasks
        )
    } else {
        "No focused work item reported by Omegon".into()
    };

    HostSessionSummary {
        connection,
        activity,
        work,
    }
}

fn status_from_harness(harness: Option<&HarnessStatusSnapshot>) -> (ShellState, DevScenario) {
    let Some(harness) = harness else {
        return (ShellState::Ready, DevScenario::Ready);
    };

    if harness.memory_warning.is_some() {
        return (ShellState::Degraded, DevScenario::Degraded);
    }

    if !harness.memory_available && !harness.cleave_available {
        return (ShellState::Degraded, DevScenario::Degraded);
    }

    (ShellState::Ready, DevScenario::Ready)
}

fn apply_harness_summary(summary: &mut HostSessionSummary, harness: &HarnessStatusSnapshot) {
    if let Some(branch) = harness.git_branch.as_ref() {
        summary.connection = format!("Attached to Omegon host on branch {branch}");
    }

    if let Some(warning) = harness.memory_warning.as_ref() {
        summary.activity = warning.clone();
    } else if !harness.active_delegates.is_empty() {
        summary.activity = format!("{} delegate task(s) active", harness.active_delegates.len());
    }
}

fn role_from_wire(role: &str) -> MessageRole {
    match role {
        "user" => MessageRole::User,
        "system" => MessageRole::System,
        _ => MessageRole::Assistant,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SNAPSHOT_JSON: &str = r#"{
        "design": {
            "focused": {
                "id": "auspex-remote",
                "title": "Remote session adapter",
                "status": "implementing",
                "open_questions": ["How should reconnect work?"],
                "decisions": 1,
                "children": 2
            },
            "implementing": [{"id": "auspex-remote", "title": "Remote session adapter", "status": "implementing"}],
            "actionable": []
        },
        "openspec": {"totalTasks": 5, "doneTasks": 2},
        "cleave": {"active": true, "totalChildren": 3, "completed": 1, "failed": 0},
        "session": {"turns": 12, "tool_calls": 34, "compactions": 1},
        "harness": {
            "gitBranch": "main",
            "gitDetached": false,
            "thinkingLevel": "medium",
            "capabilityTier": "victory",
            "providers": [{"name": "Anthropic", "authenticated": true, "auth_method": "api-key", "model": "claude-sonnet"}],
            "memoryAvailable": true,
            "cleaveAvailable": true,
            "memoryWarning": null,
            "activeDelegates": []
        }
    }"#;

    #[test]
    fn snapshot_projection_builds_remote_summary() {
        let session = RemoteHostSession::from_snapshot_json(SNAPSHOT_JSON).unwrap();

        assert_eq!(session.shell_state(), ShellState::Ready);
        assert_eq!(session.scenario(), DevScenario::Ready);
        assert!(session.summary().connection.contains("main"));
        assert!(session.summary().activity.contains("Parallel work running"));
        assert_eq!(
            session.summary().work,
            "Focused node: Remote session adapter"
        );
        assert_eq!(session.messages().len(), 1);
    }

    #[test]
    fn websocket_message_events_append_transcript() {
        let mut session = RemoteHostSession::from_snapshot_json(SNAPSHOT_JSON).unwrap();

        assert!(
            session
                .apply_event_json(r#"{"type":"message_start","role":"assistant"}"#)
                .unwrap()
        );
        assert!(
            session
                .apply_event_json(r#"{"type":"message_chunk","text":"hello "}"#)
                .unwrap()
        );
        assert!(
            session
                .apply_event_json(r#"{"type":"message_chunk","text":"world"}"#)
                .unwrap()
        );
        assert!(
            session
                .apply_event_json(r#"{"type":"message_end"}"#)
                .unwrap()
        );

        assert_eq!(
            session.messages().last().unwrap().role,
            MessageRole::Assistant
        );
        assert_eq!(session.messages().last().unwrap().text, "hello world");
    }

    #[test]
    fn harness_warning_downgrades_shell_state() {
        let mut session = RemoteHostSession::from_snapshot_json(SNAPSHOT_JSON).unwrap();

        session
            .apply_event_json(
                r#"{"type":"harness_status_changed","status":{"gitBranch":"main","gitDetached":false,"thinkingLevel":"medium","capabilityTier":"victory","providers":[],"memoryAvailable":false,"cleaveAvailable":true,"memoryWarning":"Memory database unavailable","activeDelegates":[]}}"#,
            )
            .unwrap();

        assert_eq!(session.shell_state(), ShellState::Degraded);
        assert_eq!(session.scenario(), DevScenario::Degraded);
        assert_eq!(session.summary().activity, "Memory database unavailable");
    }

    #[test]
    fn tool_and_decomposition_events_refresh_activity_and_notices() {
        let mut session = RemoteHostSession::from_snapshot_json(SNAPSHOT_JSON).unwrap();

        session
            .apply_event_json(r#"{"type":"tool_start","id":"1","name":"read","args":{}}"#)
            .unwrap();
        assert_eq!(session.summary().activity, "Running tool read");

        session
            .apply_event_json(r#"{"type":"tool_end","id":"1","is_error":false,"result":"ok"}"#)
            .unwrap();
        assert_eq!(session.summary().activity, "Tool run completed");

        session
            .apply_event_json(
                r#"{"type":"decomposition_child_completed","label":"child-a","success":true}"#,
            )
            .unwrap();
        assert!(
            session
                .messages()
                .last()
                .unwrap()
                .text
                .contains("child-a completed successfully")
        );
    }
}
