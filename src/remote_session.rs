use crate::fixtures::{
    ActivityKind, BlockOrigin, ChatMessage, ComposerState, DelegateSummaryData, DevScenario,
    DispatcherBindingData, DispatcherOptionData, DispatcherSwitchStateData, GraphData,
    HostSessionSummary, InstanceControlPlaneData, InstanceDescriptorData, InstanceIdentityData,
    InstancePolicyData, InstanceRuntimeData, InstanceSessionDescriptorData,
    InstanceWorkspaceData, MessageRole, OriginKind, ProviderInfo, SessionData, ShellState,
    SystemNoticeKind, TranscriptData, WorkData, WorkNode,
};
use crate::omegon_control::{
    HarnessStatusSnapshot, OmegonEvent, OmegonInstanceDescriptor, OmegonStateSnapshot,
};
use crate::telemetry::{build_session_telemetry, LatestTurnTelemetry};
use crate::session_model::HostSessionModel;

#[derive(Clone, Debug, PartialEq, Eq)]
#[allow(dead_code)]
pub enum DispatcherSwitchCommandOutcome {
    Issued { request_id: String },
    Noop,
}

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
    next_dispatcher_request_id: u64,
    latest_turn_telemetry: LatestTurnTelemetry,
    // Raw snapshot sub-sections kept for Power mode screens.
    design: crate::omegon_control::DesignSnapshot,
    openspec: crate::omegon_control::OpenSpecSnapshot,
    cleave: crate::omegon_control::CleaveSnapshot,
    session_stats: crate::omegon_control::SessionSnapshot,
    harness_snapshot: Option<HarnessStatusSnapshot>,
    instance_descriptor: Option<OmegonInstanceDescriptor>,
    context_tokens: Option<u64>,
    context_window: Option<u64>,
    dispatcher_binding: Option<crate::omegon_control::DispatcherBindingSnapshot>,
    transcript: TranscriptData,
}

impl RemoteHostSession {
    fn push_chat_message(&mut self, role: MessageRole, text: impl Into<String>) {
        self.messages.push(ChatMessage {
            role,
            text: text.into(),
        });
    }

    fn push_system_notice(
        &mut self,
        text: impl Into<String>,
        origin: Option<BlockOrigin>,
        notice_kind: SystemNoticeKind,
    ) {
        let text = text.into();
        self.push_chat_message(MessageRole::System, text.clone());
        if let Some(turn) = self.transcript.turns.last_mut() {
            turn.blocks.push(crate::fixtures::TurnBlock::System(
                crate::fixtures::AttributedText {
                    text,
                    origin,
                    notice_kind: Some(notice_kind),
                },
            ));
        }
    }

    fn push_dispatcher_notice(&mut self, text: impl Into<String>, notice_kind: SystemNoticeKind) {
        let origin = dispatcher_origin(&self.dispatcher_binding);
        self.push_system_notice(text, Some(origin), notice_kind);
    }

    fn push_text_block(&mut self, text: impl Into<String>, origin: Option<BlockOrigin>) {
        if let Some(turn) = self.transcript.turns.last_mut() {
            turn.blocks.push(crate::fixtures::TurnBlock::Text(
                crate::fixtures::AttributedText {
                    text: text.into(),
                    origin,
                    notice_kind: None,
                },
            ));
        }
    }

    pub fn from_snapshot(snapshot: OmegonStateSnapshot) -> Self {
        let (shell_state, scenario) =
            status_from_runtime_or_harness(snapshot.instance_descriptor.as_ref(), snapshot.harness.as_ref());
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
            next_dispatcher_request_id: 1,
            latest_turn_telemetry: LatestTurnTelemetry::default(),
            design: snapshot.design,
            openspec: snapshot.openspec,
            cleave: snapshot.cleave,
            session_stats: snapshot.session,
            harness_snapshot: snapshot.harness.clone(),
            instance_descriptor: snapshot.instance_descriptor,
            context_tokens: None,
            context_window: None,
            dispatcher_binding: snapshot.dispatcher,
            transcript: TranscriptData::default(),
        }
    }

    pub fn from_snapshot_json(json: &str) -> Result<Self, serde_json::Error> {
        let snapshot = serde_json::from_str::<OmegonStateSnapshot>(json)?;
        Ok(Self::from_snapshot(snapshot))
    }

    pub fn request_dispatcher_switch(
        &mut self,
        profile: &str,
        model: Option<&str>,
    ) -> Option<DispatcherSwitchCommandOutcome> {
        let origin = dispatcher_origin(&self.dispatcher_binding);

        self.dispatcher_binding.as_ref()?;

        let already_active = {
            let dispatcher = self.dispatcher_binding.as_ref()?;
            dispatcher.expected_profile == profile
                && match model {
                    Some(model) => dispatcher.expected_model.as_deref() == Some(model),
                    None => true,
                }
        };

        let request_id = if already_active {
            None
        } else {
            Some(self.allocate_dispatcher_request_id())
        };

        let mut superseded_message = None;
        let noop_notice = {
            let dispatcher = self.dispatcher_binding.as_mut()?;

            if already_active {
                let note = format!("Dispatcher already active: {profile}");
                dispatcher.switch_state = Some(crate::omegon_control::DispatcherSwitchStateSnapshot {
                    request_id: None,
                    requested_profile: Some(profile.to_string()),
                    requested_model: model.map(str::to_string),
                    status: "active".into(),
                    failure_code: None,
                    note: Some(note.clone()),
                });
                self.summary.activity = note.clone();
                self.summary.activity_kind = ActivityKind::Completed;
                Some(note)
            } else {
                if let Some(previous_switch) = dispatcher.switch_state.as_ref()
                    && previous_switch.status == "pending"
                {
                    let superseded_target = switch_target_label(
                        previous_switch.requested_profile.as_deref(),
                        previous_switch.requested_model.as_deref(),
                    );
                    superseded_message =
                        Some(format!("Dispatcher switch superseded: {superseded_target}"));
                }

                let request_id = request_id
                    .as_ref()
                    .expect("request_id must exist for non-noop dispatcher switches");
                dispatcher.switch_state = Some(crate::omegon_control::DispatcherSwitchStateSnapshot {
                    request_id: Some(request_id.clone()),
                    requested_profile: Some(profile.to_string()),
                    requested_model: model.map(str::to_string),
                    status: "pending".into(),
                    failure_code: None,
                    note: Some("Awaiting backend dispatcher switch confirmation".into()),
                });
                None
            }
        };

        if let Some(note) = noop_notice {
            self.push_system_notice(note, Some(origin), SystemNoticeKind::DispatcherSwitch);
            return Some(DispatcherSwitchCommandOutcome::Noop);
        }

        if let Some(superseded_message) = superseded_message {
            self.push_system_notice(
                superseded_message,
                Some(origin.clone()),
                SystemNoticeKind::DispatcherSwitch,
            );
        }

        let request_id = request_id.expect("request_id must exist for non-noop dispatcher switches");
        self.summary.activity = format!("Requesting dispatcher switch to {profile}");
        self.summary.activity_kind = ActivityKind::Waiting;
        let request_message = format!(
            "Dispatcher switch requested: {}",
            switch_target_label(Some(profile), model)
        );
        self.push_system_notice(request_message, Some(origin), SystemNoticeKind::DispatcherSwitch);
        Some(DispatcherSwitchCommandOutcome::Issued { request_id })
    }

    pub fn refresh_provider_auth(
        &mut self,
        providers: Vec<crate::omegon_control::ProviderStatusSnapshot>,
    ) {
        let mut harness = self.harness_snapshot.clone().unwrap_or_default();
        harness.providers = providers;
        let (shell_state, scenario) = status_from_harness(Some(&harness));
        self.shell_state = shell_state;
        self.scenario = scenario;
        apply_harness_summary(&mut self.summary, &harness);
        self.harness_snapshot = Some(harness);
    }

    fn allocate_dispatcher_request_id(&mut self) -> String {
        let request_id = format!("dispatcher-switch-{}", self.next_dispatcher_request_id);
        self.next_dispatcher_request_id += 1;
        request_id
    }

    pub fn apply_event(&mut self, event: OmegonEvent) -> bool {
        match event {
            OmegonEvent::StateSnapshot { data } => {
                let (shell_state, scenario) =
                    status_from_runtime_or_harness(data.instance_descriptor.as_ref(), data.harness.as_ref());
                self.shell_state = shell_state;
                self.scenario = scenario;
                self.summary = summary_from_snapshot(&data);
                self.design = data.design;
                self.openspec = data.openspec;
                self.cleave = data.cleave;
                self.session_stats = data.session;
                let previous_dispatcher = self.dispatcher_binding.take();
                self.dispatcher_binding = reconcile_dispatcher_binding(
                    previous_dispatcher.clone(),
                    data.dispatcher,
                );
                append_dispatcher_switch_transition_notice(
                    &mut self.messages,
                    &mut self.transcript,
                    previous_dispatcher.as_ref(),
                    self.dispatcher_binding.as_ref(),
                );
                self.transcript.context_tokens = self.context_tokens;
                true
            }
            OmegonEvent::MessageStart { role } => {
                self.pending_role = Some(role_from_wire(&role));
                self.pending_text.clear();
                true
            }
            OmegonEvent::MessageChunk { text } => {
                if self.pending_role.is_some() {
                    self.pending_text.push_str(&text);
                    true
                } else {
                    false
                }
            }
            OmegonEvent::ThinkingChunk { text } => {
                if let Some(turn) = self.transcript.turns.last_mut() {
                    match turn.blocks.last_mut() {
                        Some(crate::fixtures::TurnBlock::Thinking(thinking)) => {
                            thinking.text.push_str(&text);
                        }
                        _ => turn.blocks.push(crate::fixtures::TurnBlock::Thinking(
                            crate::fixtures::TurnBlockText {
                                text,
                                collapsed: true,
                            },
                        )),
                    }
                    true
                } else {
                    false
                }
            }
            OmegonEvent::MessageEnd => {
                let Some(role) = self.pending_role.take() else {
                    return false;
                };
                let text = self.pending_text.trim().to_string();
                self.push_chat_message(role, text.clone());
                self.push_text_block(text, Some(dispatcher_origin(&self.dispatcher_binding)));
                self.pending_text.clear();
                true
            }
            OmegonEvent::MessageAbort => {
                let aborted = self.pending_text.trim().to_string();
                if let Some(role) = self.pending_role.take()
                    && matches!(role, MessageRole::Assistant)
                {
                    self.push_chat_message(
                        MessageRole::System,
                        format!("Assistant message aborted: {aborted}"),
                    );
                }
                if let Some(turn) = self.transcript.turns.last_mut() {
                    turn.blocks.push(crate::fixtures::TurnBlock::Aborted(aborted));
                }
                self.pending_text.clear();
                self.run_active = false;
                true
            }
            OmegonEvent::SystemNotification { message } => {
                self.push_system_notice(
                    message,
                    Some(BlockOrigin {
                        kind: OriginKind::System,
                        label: "System".into(),
                    }),
                    SystemNoticeKind::Generic,
                );
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
                if let Some(dispatcher) = self.dispatcher_binding.as_mut()
                    && let Some(switch_state) = dispatcher.switch_state.as_mut()
                    && switch_state.status == "pending"
                {
                    switch_state.status = "failed".into();
                    switch_state.failure_code = Some("conflict".into());
                    switch_state.note = Some("Session reset during dispatcher switch".into());
                }
                self.messages.clear();
                self.push_chat_message(
                    MessageRole::System,
                    "Omegon reported a session reset. Auspex cleared the cached transcript and is waiting for fresh host events.",
                );
                self.pending_role = None;
                self.pending_text.clear();
                self.run_active = false;
                self.transcript.turns.clear();
                self.transcript.active_turn = None;
                true
            }
            OmegonEvent::TurnStart { turn } => {
                self.transcript.active_turn = Some(turn);
                self.run_active = true;
                self.summary.activity = format!("Turn {turn} in progress");
                self.summary.activity_kind = ActivityKind::Running;
                self.transcript.turns.push(crate::fixtures::Turn {
                    number: turn,
                    blocks: vec![],
                });
                true
            }
            OmegonEvent::TurnEnd {
                turn,
                estimated_tokens,
                actual_input_tokens,
                actual_output_tokens,
                cache_read_tokens,
                provider_telemetry,
            } => {
                self.transcript.active_turn = None;
                self.run_active = false;
                self.summary.activity = format!("Turn {turn} completed");
                self.summary.activity_kind = ActivityKind::Completed;
                self.latest_turn_telemetry = LatestTurnTelemetry {
                    provider_telemetry,
                    estimated_tokens,
                    actual_input_tokens,
                    actual_output_tokens,
                    cache_read_tokens,
                };
                true
            }
            OmegonEvent::ToolStart { id, name, args } => {
                self.summary.activity = format!("Running tool {name}");
                self.summary.activity_kind = ActivityKind::Running;
                if let Some(turn) = self.transcript.turns.last_mut() {
                    turn.blocks.push(crate::fixtures::TurnBlock::Tool(crate::fixtures::ToolCard {
                        id,
                        name,
                        args: args.map(|v| v.to_string()).unwrap_or_default(),
                        partial_output: String::new(),
                        result: None,
                        is_error: false,
                        origin: Some(dispatcher_origin(&self.dispatcher_binding)),
                    }));
                }
                true
            }
            OmegonEvent::ToolUpdate { id, partial } => {
                if let Some(turn) = self.transcript.turns.last_mut()
                    && let Some(crate::fixtures::TurnBlock::Tool(tool)) = turn.blocks.last_mut()
                    && tool.id == id
                {
                    if !tool.partial_output.is_empty() {
                        tool.partial_output.push('\n');
                    }
                    tool.partial_output.push_str(&partial);
                }
                true
            }
            OmegonEvent::ToolEnd { id, is_error, result } => {
                self.summary.activity = if is_error {
                    "Tool run completed with an error".into()
                } else {
                    "Tool run completed".into()
                };
                self.summary.activity_kind = if is_error {
                    ActivityKind::Failure
                } else {
                    ActivityKind::Completed
                };
                if let Some(turn) = self.transcript.turns.last_mut()
                    && let Some(crate::fixtures::TurnBlock::Tool(tool)) = turn.blocks.last_mut()
                    && tool.id == id
                {
                    tool.is_error = is_error;
                    tool.result = result;
                }
                true
            }
            OmegonEvent::ContextUpdated { tokens } => {
                self.transcript.context_tokens = Some(tokens);
                self.context_tokens = Some(tokens);
                true
            }
            OmegonEvent::AgentEnd => {
                self.run_active = false;
                self.summary.activity = "Agent turn finished".into();
                self.summary.activity_kind = ActivityKind::Completed;
                true
            }
            OmegonEvent::PhaseChanged { phase } => {
                self.summary.activity = format!("Lifecycle phase: {phase}");
                self.summary.activity_kind = ActivityKind::Running;
                true
            }
            OmegonEvent::DecompositionStarted { children } => {
                self.summary.activity =
                    format!("Cleave started with {} child task(s)", children.len());
                self.summary.activity_kind = ActivityKind::Running;
                self.push_dispatcher_notice(
                    format!("Dispatcher requested decomposition into {} child task(s)", children.len()),
                    SystemNoticeKind::CleaveStart,
                );
                true
            }
            OmegonEvent::DecompositionChildCompleted { label, success } => {
                let message = format!(
                    "Cleave child {label} {}",
                    if success {
                        "completed successfully"
                    } else {
                        "failed"
                    }
                );
                self.push_system_notice(
                    message,
                    Some(BlockOrigin {
                        kind: OriginKind::Child,
                        label: format!("Child {label}"),
                    }),
                    if success {
                        SystemNoticeKind::ChildStatus
                    } else {
                        SystemNoticeKind::Failure
                    },
                );
                true
            }
            OmegonEvent::DecompositionCompleted { merged } => {
                self.summary.activity = if merged {
                    "Cleave completed and merged".into()
                } else {
                    "Cleave completed without merge".into()
                };
                self.summary.activity_kind = ActivityKind::Completed;
                let message = if merged {
                    "Dispatcher completed decomposition and merged child results".to_string()
                } else {
                    "Dispatcher completed decomposition without merge".to_string()
                };
                self.push_dispatcher_notice(message, SystemNoticeKind::CleaveComplete);
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

    fn transcript(&self) -> &TranscriptData {
        &self.transcript
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
            DevScenario::StartupFailure | DevScenario::CompatibilityFailure => ShellState::Failed,
            DevScenario::Reconnecting => ShellState::CompatibilityChecking,
        };
    }

    fn can_submit(&self) -> bool {
        !self.run_active
            && matches!(self.shell_state, ShellState::Ready | ShellState::Degraded)
            && harness_can_execute_prompts(
                self.harness_snapshot.as_ref(),
                self.instance_descriptor.as_ref(),
            )
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
        let instance_descriptor = self.instance_descriptor.as_ref();
        let dispatcher = self.dispatcher_binding.as_ref();
        SessionData {
            git_branch: h
                .and_then(|h| h.git_branch.clone())
                .or_else(|| {
                    instance_descriptor
                        .and_then(|descriptor| descriptor.workspace.as_ref())
                        .and_then(|workspace| workspace.branch.clone())
                }),
            git_detached: h.map(|h| h.git_detached).unwrap_or(false),
            thinking_level: h
                .map(|h| h.thinking_level.clone())
                .filter(|value| !value.trim().is_empty())
                .or_else(|| {
                    dispatcher
                        .and_then(|binding| binding.instance_descriptor.as_ref())
                        .and_then(|descriptor| descriptor.policy.as_ref())
                        .and_then(|policy| policy.thinking_level.clone())
                })
                .or_else(|| {
                    instance_descriptor
                        .and_then(|descriptor| descriptor.policy.as_ref())
                        .and_then(|policy| policy.thinking_level.clone())
                })
                .unwrap_or_default(),
            capability_tier: h
                .map(|h| h.capability_tier.clone())
                .filter(|value| !value.trim().is_empty())
                .or_else(|| {
                    dispatcher
                        .and_then(|binding| binding.instance_descriptor.as_ref())
                        .and_then(|descriptor| descriptor.policy.as_ref())
                        .and_then(|policy| policy.capability_tier.clone())
                })
                .or_else(|| {
                    instance_descriptor
                        .and_then(|descriptor| descriptor.policy.as_ref())
                        .and_then(|policy| policy.capability_tier.clone())
                })
                .unwrap_or_default(),
            providers: h
                .map(|h| {
                    h.providers
                        .iter()
                        .map(|p| ProviderInfo {
                            name: p.name.clone(),
                            authenticated: p.authenticated,
                            auth_method: p.auth_method.clone(),
                            model: p.model.clone(),
                        })
                        .collect()
                })
                .unwrap_or_default(),
            memory_available: h.map(|h| h.memory_available).unwrap_or(true),
            cleave_available: h.map(|h| h.cleave_available).unwrap_or(true),
            memory_warning: h.and_then(|h| h.memory_warning.clone()),
            active_delegate_count: h.map(|h| h.active_delegates.len()).unwrap_or(0),
            active_delegates: h
                .map(|h| {
                    h.active_delegates
                        .iter()
                        .map(|d| DelegateSummaryData {
                            task_id: d.task_id.clone(),
                            agent_name: d.agent_name.clone(),
                            status: d.status.clone(),
                            elapsed_ms: d.elapsed_ms,
                        })
                        .collect()
                })
                .unwrap_or_default(),
            session_turns: self.session_stats.turns,
            session_tool_calls: self.session_stats.tool_calls,
            session_compactions: self.session_stats.compactions,
            context_tokens: self.context_tokens,
            context_window: self.context_window,
            telemetry: build_session_telemetry(
                h,
                self.session_stats.turns,
                self.session_stats.tool_calls,
                self.dispatcher_binding.as_ref(),
                self.instance_descriptor.as_ref(),
                &self.latest_turn_telemetry,
            ),
            instance_descriptor: self.instance_descriptor.as_ref().map(project_instance_descriptor),
            dispatcher_binding: self.dispatcher_binding.as_ref().map(|binding| {
                DispatcherBindingData {
                    session_id: binding
                        .instance_descriptor
                        .as_ref()
                        .and_then(|descriptor| descriptor.session.as_ref())
                        .and_then(|session| session.session_id.clone())
                        .unwrap_or_else(|| binding.session_id.clone()),
                    dispatcher_instance_id: binding
                        .instance_descriptor
                        .as_ref()
                        .map(|descriptor| descriptor.identity.instance_id.clone())
                        .filter(|value| !value.is_empty())
                        .unwrap_or_else(|| binding.dispatcher_instance_id.clone()),
                    expected_role: binding
                        .instance_descriptor
                        .as_ref()
                        .map(|descriptor| descriptor.identity.role.clone())
                        .filter(|value| !value.is_empty())
                        .unwrap_or_else(|| binding.expected_role.clone()),
                    expected_profile: binding
                        .instance_descriptor
                        .as_ref()
                        .map(|descriptor| descriptor.identity.profile.clone())
                        .filter(|value| !value.is_empty())
                        .unwrap_or_else(|| binding.expected_profile.clone()),
                    expected_model: binding
                        .instance_descriptor
                        .as_ref()
                        .and_then(|descriptor| descriptor.policy.as_ref())
                        .and_then(|policy| policy.model.clone())
                        .or_else(|| binding.expected_model.clone()),
                    control_plane_schema: binding
                        .instance_descriptor
                        .as_ref()
                        .and_then(|descriptor| descriptor.control_plane.as_ref())
                        .map(|control_plane| control_plane.schema_version)
                        .filter(|value| *value > 0)
                        .unwrap_or(binding.control_plane_schema),
                    token_ref: binding
                        .instance_descriptor
                        .as_ref()
                        .and_then(|descriptor| descriptor.control_plane.as_ref())
                        .and_then(|control_plane| control_plane.token_ref.clone())
                        .or_else(|| binding.token_ref.clone()),
                    observed_base_url: binding
                        .instance_descriptor
                        .as_ref()
                        .and_then(|descriptor| descriptor.control_plane.as_ref())
                        .and_then(|control_plane| control_plane.base_url.clone())
                        .or_else(|| binding.observed_base_url.clone()),
                    last_verified_at: binding
                        .instance_descriptor
                        .as_ref()
                        .and_then(|descriptor| descriptor.control_plane.as_ref())
                        .and_then(|control_plane| control_plane.last_verified_at.clone())
                        .or_else(|| binding.last_verified_at.clone()),
                    instance_descriptor: binding.instance_descriptor.as_ref().map(project_instance_descriptor),
                    available_options: binding
                        .available_options
                        .iter()
                        .map(|option| DispatcherOptionData {
                            profile: option.profile.clone(),
                            label: option.label.clone(),
                            model: option.model.clone(),
                        })
                        .collect(),
                    switch_state: binding.switch_state.as_ref().map(|state| {
                        DispatcherSwitchStateData {
                            request_id: state.request_id.clone(),
                            requested_profile: state.requested_profile.clone(),
                            requested_model: state.requested_model.clone(),
                            status: state.status.clone(),
                            failure_code: state.failure_code.clone(),
                            note: state.note.clone(),
                        }
                    }),
                }
            }),
        }
    }

    fn graph_data(&self) -> GraphData {
        let (nodes, is_full) = if !self.design.all_nodes.is_empty() {
            let nodes = self
                .design
                .all_nodes
                .iter()
                .map(|n| WorkNode {
                    id: n.id.clone(),
                    title: n.title.clone(),
                    status: n.status.clone(),
                })
                .collect();
            (nodes, true)
        } else {
            let nodes = self
                .design
                .implementing
                .iter()
                .chain(self.design.actionable.iter())
                .map(|n| WorkNode {
                    id: n.id.clone(),
                    title: n.title.clone(),
                    status: n.status.clone(),
                })
                .collect();
            (nodes, false)
        };

        let mut counts: Vec<(String, usize)> = self
            .design
            .counts
            .iter()
            .map(|(k, v)| (k.clone(), *v))
            .collect();
        counts.sort_by(|a, b| b.1.cmp(&a.1));

        GraphData {
            nodes,
            is_full_inventory: is_full,
            counts,
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

fn project_instance_descriptor(
    descriptor: &OmegonInstanceDescriptor,
) -> InstanceDescriptorData {
    InstanceDescriptorData {
        identity: InstanceIdentityData {
            instance_id: descriptor.identity.instance_id.clone(),
            role: descriptor.identity.role.clone(),
            profile: descriptor.identity.profile.clone(),
            status: descriptor.identity.status.clone(),
        },
        workspace: descriptor.workspace.as_ref().map(|workspace| InstanceWorkspaceData {
            cwd: workspace.cwd.clone(),
            workspace_id: workspace.workspace_id.clone(),
            branch: workspace.branch.clone(),
        }),
        control_plane: descriptor
            .control_plane
            .as_ref()
            .map(|control_plane| InstanceControlPlaneData {
                schema_version: control_plane.schema_version,
                omegon_version: control_plane.omegon_version.clone(),
                base_url: control_plane.base_url.clone(),
                startup_url: control_plane.startup_url.clone(),
                state_url: control_plane.state_url.clone(),
                health_url: control_plane.health_url.clone(),
                ready_url: control_plane.ready_url.clone(),
                ws_url: control_plane.ws_url.clone(),
                auth_mode: control_plane.auth_mode.clone(),
                token_ref: control_plane.token_ref.clone(),
                last_ready_at: control_plane.last_ready_at.clone(),
                last_verified_at: control_plane.last_verified_at.clone(),
                capabilities: control_plane.capabilities.clone(),
            }),
        runtime: descriptor.runtime.as_ref().map(|runtime| InstanceRuntimeData {
            backend: runtime.backend.clone(),
            host: runtime.host.clone(),
            pid: runtime.pid,
            placement_id: runtime.placement_id.clone(),
            namespace: runtime.namespace.clone(),
            pod_name: runtime.pod_name.clone(),
            container_name: runtime.container_name.clone(),
        }),
        session: descriptor.session.as_ref().map(|session| InstanceSessionDescriptorData {
            session_id: session.session_id.clone(),
        }),
        policy: descriptor.policy.as_ref().map(|policy| InstancePolicyData {
            model: policy.model.clone(),
            thinking_level: policy.thinking_level.clone(),
            capability_tier: policy.capability_tier.clone(),
        }),
    }
}

fn dispatcher_origin(
    dispatcher: &Option<crate::omegon_control::DispatcherBindingSnapshot>,
) -> BlockOrigin {
    BlockOrigin {
        kind: OriginKind::Dispatcher,
        label: dispatcher
            .as_ref()
            .and_then(|binding| binding.instance_descriptor.as_ref())
            .and_then(|descriptor| descriptor.policy.as_ref())
            .and_then(|policy| policy.model.clone())
            .or_else(|| {
                dispatcher
                    .as_ref()
                    .and_then(|binding| binding.instance_descriptor.as_ref())
                    .map(|descriptor| descriptor.identity.instance_id.clone())
                    .filter(|value| !value.is_empty())
            })
            .or_else(|| dispatcher.as_ref().and_then(|binding| binding.expected_model.clone()))
            .or_else(|| dispatcher.as_ref().map(|binding| binding.dispatcher_instance_id.clone()))
            .unwrap_or_else(|| "Dispatcher".into()),
    }
}

fn reconcile_dispatcher_binding(
    previous: Option<crate::omegon_control::DispatcherBindingSnapshot>,
    next: Option<crate::omegon_control::DispatcherBindingSnapshot>,
) -> Option<crate::omegon_control::DispatcherBindingSnapshot> {
    let previous_switch = previous.as_ref().and_then(|binding| binding.switch_state.clone());
    let previous_pending = previous_switch
        .as_ref()
        .filter(|switch_state| switch_state.status == "pending")
        .cloned();
    let mut next = next?;

    if let Some(explicit_switch) = next.switch_state.as_ref() {
        if explicit_switch.status == "failed" || explicit_switch.status == "superseded" {
            return Some(next);
        }

        if let Some(previous_pending) = previous_pending.as_ref()
            && explicit_switch.status == "active"
            && explicit_switch.request_id.is_some()
            && explicit_switch.request_id != previous_pending.request_id
        {
            return Some(next);
        }
    }

    if next.switch_state.is_none() && let Some(previous_switch) = previous_pending {
        let requested_profile = previous_switch.requested_profile.as_deref();
        let requested_model = previous_switch.requested_model.as_deref();
        let profile_matches = requested_profile == Some(next.expected_profile.as_str());
        let model_matches = match requested_model {
            Some(requested_model) => next.expected_model.as_deref() == Some(requested_model),
            None => true,
        };

        next.switch_state = Some(if profile_matches && model_matches {
            crate::omegon_control::DispatcherSwitchStateSnapshot {
                request_id: previous_switch.request_id,
                requested_profile: previous_switch.requested_profile,
                requested_model: previous_switch.requested_model,
                status: "active".into(),
                failure_code: None,
                note: Some("Dispatcher switch confirmed by snapshot".into()),
            }
        } else {
            previous_switch
        });
    }

    Some(next)
}

fn switch_target_label(profile: Option<&str>, model: Option<&str>) -> String {
    match (profile, model) {
        (Some(profile), Some(model)) => format!("{profile} · {model}"),
        (Some(profile), None) => profile.to_string(),
        (None, Some(model)) => model.to_string(),
        (None, None) => "unknown target".into(),
    }
}

fn append_dispatcher_switch_transition_notice(
    messages: &mut Vec<ChatMessage>,
    transcript: &mut TranscriptData,
    previous: Option<&crate::omegon_control::DispatcherBindingSnapshot>,
    next: Option<&crate::omegon_control::DispatcherBindingSnapshot>,
) {
    let previous_state = previous.and_then(|binding| binding.switch_state.as_ref());
    let next_state = next.and_then(|binding| binding.switch_state.as_ref());
    if previous_state == next_state {
        return;
    }

    let Some(next_state) = next_state else {
        return;
    };

    let request_suffix = next_state
        .request_id
        .as_deref()
        .map(|request_id| format!(" ({request_id})"))
        .unwrap_or_default();
    let target = switch_target_label(
        next_state.requested_profile.as_deref(),
        next_state.requested_model.as_deref(),
    );

    let message = match next_state.status.as_str() {
        "active"
            if previous_state.is_some_and(|state| {
                state.status == "pending" && state.request_id == next_state.request_id
            }) =>
        {
            Some(format!(
                "Dispatcher switch confirmed{request_suffix}: {target}"
            ))
        }
        "active"
            if previous_state.is_some_and(|state| {
                state.status == "pending" && state.request_id != next_state.request_id
            }) =>
        {
            Some(format!(
                "Dispatcher reports a different active request{request_suffix}: {target}"
            ))
        }
        "failed" => Some(match next_state.failure_code.as_deref() {
            Some(code) => format!(
                "Dispatcher switch failed{request_suffix}: {target} [{code}]"
            ),
            None => format!("Dispatcher switch failed{request_suffix}: {target}"),
        }),
        "superseded" => Some(format!(
            "Dispatcher switch superseded{request_suffix}: {target}"
        )),
        _ => None,
    };

    let Some(message) = message else {
        return;
    };

    push_system_notice_to_buffers(
        messages,
        transcript,
        message,
        Some(dispatcher_origin(&next.cloned())),
        match next_state.status.as_str() {
            "failed" => SystemNoticeKind::Failure,
            _ => SystemNoticeKind::DispatcherSwitch,
        },
    );
}

fn push_system_notice_to_buffers(
    messages: &mut Vec<ChatMessage>,
    transcript: &mut TranscriptData,
    text: impl Into<String>,
    origin: Option<BlockOrigin>,
    notice_kind: SystemNoticeKind,
) {
    let text = text.into();
    messages.push(ChatMessage {
        role: MessageRole::System,
        text: text.clone(),
    });
    if let Some(turn) = transcript.turns.last_mut() {
        turn.blocks.push(crate::fixtures::TurnBlock::System(
            crate::fixtures::AttributedText {
                text,
                origin,
                notice_kind: Some(notice_kind),
            },
        ));
    }
}

fn summary_from_snapshot(snapshot: &OmegonStateSnapshot) -> HostSessionSummary {
    let connection = if let Some(descriptor) = snapshot.instance_descriptor.as_ref() {
        let branch = descriptor
            .workspace
            .as_ref()
            .and_then(|workspace| workspace.branch.as_deref())
            .or_else(|| snapshot.harness.as_ref().and_then(|harness| harness.git_branch.as_deref()))
            .unwrap_or("detached");
        let identity = if descriptor.identity.instance_id.is_empty() {
            "instance unknown".to_string()
        } else {
            descriptor.identity.instance_id.clone()
        };
        let model = descriptor
            .policy
            .as_ref()
            .and_then(|policy| policy.model.as_deref())
            .or_else(|| {
                snapshot.harness.as_ref().and_then(|harness| {
                    harness.providers.iter().find_map(|provider| provider.model.as_deref())
                })
            })
            .unwrap_or("provider unknown");
        format!("Attached to Omegon instance {identity} on branch {branch} ({model})")
    } else {
        match snapshot.harness.as_ref() {
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
        }
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
    } else if let Some(descriptor) = snapshot.instance_descriptor.as_ref() {
        let workspace = descriptor
            .workspace
            .as_ref()
            .and_then(|workspace| workspace.workspace_id.as_deref())
            .unwrap_or("workspace unknown");
        let role = if descriptor.identity.role.is_empty() {
            "role unknown"
        } else {
            descriptor.identity.role.as_str()
        };
        format!("{role} attached to {workspace}")
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
        "No focused work item".into()
    };

    HostSessionSummary {
        connection,
        activity,
        activity_kind: if snapshot.cleave.active {
            ActivityKind::Running
        } else if snapshot.harness.as_ref().and_then(|h| h.memory_warning.as_ref()).is_some() {
            ActivityKind::Degraded
        } else if snapshot.design.focused.is_some() {
            ActivityKind::Running
        } else {
            ActivityKind::Idle
        },
        work,
    }
}

fn status_from_runtime_or_harness(
    instance_descriptor: Option<&OmegonInstanceDescriptor>,
    harness: Option<&HarnessStatusSnapshot>,
) -> (ShellState, DevScenario) {
    if let Some(harness) = harness {
        return status_from_harness(Some(harness));
    }

    if let Some(runtime) = instance_descriptor.and_then(|descriptor| descriptor.runtime.as_ref()) {
        if runtime.provider_ok && runtime.memory_ok {
            return (ShellState::Ready, DevScenario::Ready);
        }
        return (ShellState::Degraded, DevScenario::Degraded);
    }

    (ShellState::Ready, DevScenario::Ready)
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

    if !harness_can_execute_prompts(Some(harness), None) {
        return (ShellState::Degraded, DevScenario::Degraded);
    }

    (ShellState::Ready, DevScenario::Ready)
}

fn harness_can_execute_prompts(
    harness: Option<&HarnessStatusSnapshot>,
    instance_descriptor: Option<&OmegonInstanceDescriptor>,
) -> bool {
    if let Some(harness) = harness {
        return harness.providers.iter().any(|provider| provider.authenticated);
    }

    if let Some(runtime) = instance_descriptor.and_then(|descriptor| descriptor.runtime.as_ref()) {
        return runtime.provider_ok;
    }

    true
}

fn apply_harness_summary(summary: &mut HostSessionSummary, harness: &HarnessStatusSnapshot) {
    if let Some(branch) = harness.git_branch.as_ref() {
        summary.connection = format!("Attached to Omegon host on branch {branch}");
    }

    if let Some(warning) = harness.memory_warning.as_ref() {
        summary.activity = warning.clone();
        summary.activity_kind = ActivityKind::Degraded;
    } else if !harness_can_execute_prompts(Some(harness), None) {
        summary.activity = "No authenticated providers reported by Omegon".into();
        summary.activity_kind = ActivityKind::Degraded;
    } else if !harness.active_delegates.is_empty() {
        summary.activity = format!("{} delegate task(s) active", harness.active_delegates.len());
        summary.activity_kind = ActivityKind::Running;
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
    use crate::omegon_control::{
        CleaveSnapshot, DelegateSummarySnapshot, DesignSnapshot,
        DispatcherBindingSnapshot, HarnessStatusSnapshot, OmegonControlPlaneDescriptor,
        OmegonInstanceDescriptor, OmegonInstanceIdentity, OmegonPolicyDescriptor,
        OmegonRuntimeDescriptor, OmegonSessionDescriptor, OmegonStateSnapshot,
        OmegonWorkspaceDescriptor, OpenSpecSnapshot, ProviderStatusSnapshot,
        SessionSnapshot,
    };

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
        "openspec": {"total_tasks": 5, "done_tasks": 2},
        "cleave": {"active": true, "total_children": 3, "completed": 1, "failed": 0},
        "session": {"turns": 12, "tool_calls": 34, "compactions": 1},
        "instance_descriptor": {
            "identity": {
                "instance_id": "omg_primary_01HVTEST",
                "role": "primary-driver",
                "profile": "primary-interactive",
                "status": "busy"
            },
            "workspace": {
                "cwd": "/repo/main",
                "workspace_id": "repo:main",
                "branch": "main"
            },
            "control_plane": {
                "schema_version": 2,
                "base_url": "http://127.0.0.1:7842",
                "state_url": "http://127.0.0.1:7842/api/state",
                "ws_url": "ws://127.0.0.1:7842/ws?token=test",
                "auth_mode": "ephemeral-bearer",
                "token_ref": "secret://auspex/instances/omg_primary_01HVTEST/token",
                "last_verified_at": "2026-04-04T12:00:00Z"
            },
            "runtime": {
                "backend": "local-process",
                "host": "desktop:local",
                "pid": 8123
            },
            "session": {
                "session_id": "session_01HVTEST"
            },
            "policy": {
                "model": "anthropic:claude-sonnet-4-6",
                "thinking_level": "medium",
                "capability_tier": "victory"
            }
        },
        "dispatcher": {
            "session_id": "session_01HVTEST",
            "dispatcher_instance_id": "legacy_dispatcher_id",
            "expected_role": "legacy-role",
            "expected_profile": "legacy-profile",
            "expected_model": "legacy:model",
            "control_plane_schema": 1,
            "token_ref": "secret://auspex/instances/legacy_dispatcher_id/token",
            "observed_base_url": "http://127.0.0.1:9999",
            "last_verified_at": "2026-04-04T12:00:00Z",
            "instance_descriptor": {
                "identity": {
                    "instance_id": "omg_dispatcher_01HVTEST",
                    "role": "primary-driver",
                    "profile": "primary-interactive",
                    "status": "ready"
                },
                "control_plane": {
                    "schema_version": 2,
                    "base_url": "http://127.0.0.1:7842",
                    "token_ref": "secret://auspex/instances/omg_dispatcher_01HVTEST/token",
                    "last_verified_at": "2026-04-04T12:01:00Z"
                },
                "session": {
                    "session_id": "session_01HVTEST"
                },
                "policy": {
                    "model": "anthropic:claude-sonnet-4-6"
                }
            }
        },
        "harness": {
            "git_branch": "wrong-legacy-branch",
            "git_detached": false,
            "thinking_level": "medium",
            "capability_tier": "victory",
            "providers": [{"name": "Anthropic", "authenticated": true, "auth_method": "api-key", "model": "claude-sonnet"}],
            "memory_available": true,
            "cleave_available": true,
            "memory_warning": null,
            "active_delegates": []
        }
    }"#;

    #[test]
    fn session_projection_falls_back_to_descriptor_policy_and_workspace_fields() {
        let session = RemoteHostSession::from_snapshot(OmegonStateSnapshot {
            design: DesignSnapshot::default(),
            openspec: OpenSpecSnapshot::default(),
            cleave: CleaveSnapshot::default(),
            session: SessionSnapshot::default(),
            harness: Some(HarnessStatusSnapshot {
                git_branch: None,
                git_detached: false,
                thinking_level: String::new(),
                capability_tier: String::new(),
                providers: vec![],
                memory_available: true,
                cleave_available: true,
                memory_warning: None,
                active_delegates: vec![],
            }),
            dispatcher: Some(DispatcherBindingSnapshot {
                expected_role: "primary-driver".into(),
                expected_profile: "primary-interactive".into(),
                expected_model: Some("anthropic:claude-sonnet-4-6".into()),
                instance_descriptor: Some(OmegonInstanceDescriptor {
                    identity: OmegonInstanceIdentity {
                        instance_id: "omg_primary_01HVDEMO".into(),
                        role: "primary-driver".into(),
                        profile: "primary-interactive".into(),
                        status: "ready".into(),
                    },
                    policy: Some(OmegonPolicyDescriptor {
                        model: Some("anthropic:claude-sonnet-4-6".into()),
                        thinking_level: Some("high".into()),
                        capability_tier: Some("gloriana".into()),
                    }),
                    ..OmegonInstanceDescriptor::default()
                }),
                ..DispatcherBindingSnapshot::default()
            }),
            instance_descriptor: Some(OmegonInstanceDescriptor {
                identity: OmegonInstanceIdentity {
                    instance_id: "omg_host_01HVDEMO".into(),
                    role: "primary-driver".into(),
                    profile: "control-plane".into(),
                    status: "ready".into(),
                },
                workspace: Some(OmegonWorkspaceDescriptor {
                    cwd: Some("/repo".into()),
                    workspace_id: Some("repo:demo".into()),
                    branch: Some("main".into()),
                }),
                policy: Some(OmegonPolicyDescriptor {
                    model: Some("openai:gpt-4.1".into()),
                    thinking_level: Some("medium".into()),
                    capability_tier: Some("victory".into()),
                }),
                ..OmegonInstanceDescriptor::default()
            }),
        });

        let data = session.session_data();
        assert_eq!(data.git_branch.as_deref(), Some("main"));
        assert_eq!(data.thinking_level, "high");
        assert_eq!(data.capability_tier, "gloriana");
        assert_eq!(
            data.dispatcher_binding
                .as_ref()
                .and_then(|binding| binding.expected_model.as_deref()),
            Some("anthropic:claude-sonnet-4-6")
        );
    }

    fn snapshot_without_harness_uses_runtime_provider_health_for_submit_gate() {
        let session = RemoteHostSession::from_snapshot(OmegonStateSnapshot {
            design: crate::omegon_control::DesignSnapshot::default(),
            openspec: crate::omegon_control::OpenSpecSnapshot::default(),
            cleave: crate::omegon_control::CleaveSnapshot::default(),
            session: crate::omegon_control::SessionSnapshot::default(),
            harness: None,
            dispatcher: None,
            instance_descriptor: Some(OmegonInstanceDescriptor {
                identity: crate::omegon_control::OmegonInstanceIdentity {
                    instance_id: "web-compat".into(),
                    role: "primary_driver".into(),
                    profile: "primary-interactive".into(),
                    status: "ready".into(),
                },
                runtime: Some(crate::omegon_control::OmegonRuntimeDescriptor {
                    health: Some("ready".into()),
                    provider_ok: false,
                    memory_ok: true,
                    cleave_available: false,
                    context_class: Some("Squad".into()),
                    thinking_level: Some("Medium".into()),
                    capability_tier: Some("victory".into()),
                    ..crate::omegon_control::OmegonRuntimeDescriptor::default()
                }),
                session: Some(crate::omegon_control::OmegonSessionDescriptor {
                    session_id: Some("detached".into()),
                }),
                ..OmegonInstanceDescriptor::default()
            }),
        });

        assert_eq!(session.shell_state(), ShellState::Degraded);
        assert!(!session.can_submit());
    }

    #[test]
    fn snapshot_projection_builds_remote_summary() {
        let session = RemoteHostSession::from_snapshot_json(SNAPSHOT_JSON).unwrap();

        assert_eq!(session.shell_state(), ShellState::Ready);
        assert_eq!(session.scenario(), DevScenario::Ready);
        assert!(session.summary().connection.contains("omg_primary_01HVTEST"));
        assert!(session.summary().connection.contains("branch main"));
        assert!(!session.summary().connection.contains("wrong-legacy-branch"));
        assert!(session.summary().activity.contains("Parallel work running"));
        assert_eq!(session.summary().activity_kind, ActivityKind::Running);
        assert_eq!(
            session.summary().work,
            "Focused node: Remote session adapter"
        );
        assert_eq!(session.messages().len(), 1);

        let session_data = session.session_data();
        let instance = session_data.instance_descriptor.as_ref().unwrap();
        assert_eq!(instance.identity.instance_id, "omg_primary_01HVTEST");
        assert_eq!(instance.workspace.as_ref().unwrap().workspace_id.as_deref(), Some("repo:main"));
        assert_eq!(instance.control_plane.as_ref().unwrap().schema_version, 2);
        assert_eq!(instance.runtime.as_ref().unwrap().pid, Some(8123));
        assert_eq!(instance.policy.as_ref().unwrap().model.as_deref(), Some("anthropic:claude-sonnet-4-6"));

        let dispatcher = session_data.dispatcher_binding.as_ref().unwrap();
        assert_eq!(dispatcher.session_id, "session_01HVTEST");
        assert_eq!(dispatcher.dispatcher_instance_id, "omg_dispatcher_01HVTEST");
        assert_eq!(dispatcher.expected_role, "primary-driver");
        assert_eq!(dispatcher.expected_profile, "primary-interactive");
        assert_eq!(dispatcher.expected_model.as_deref(), Some("anthropic:claude-sonnet-4-6"));
        assert_eq!(dispatcher.control_plane_schema, 2);
        assert_eq!(dispatcher.observed_base_url.as_deref(), Some("http://127.0.0.1:7842"));
        assert_eq!(dispatcher.last_verified_at.as_deref(), Some("2026-04-04T12:01:00Z"));
        assert_eq!(dispatcher.token_ref.as_deref(), Some("secret://auspex/instances/omg_dispatcher_01HVTEST/token"));
        assert_eq!(dispatcher.instance_descriptor.as_ref().unwrap().identity.instance_id, "omg_dispatcher_01HVTEST");
    }

    #[test]
    fn dispatcher_origin_prefers_canonical_instance_descriptor_identity() {
        let session = RemoteHostSession::from_snapshot_json(SNAPSHOT_JSON).unwrap();

        assert_eq!(
            dispatcher_origin(&session.dispatcher_binding),
            BlockOrigin {
                kind: OriginKind::Dispatcher,
                label: "anthropic:claude-sonnet-4-6".into(),
            }
        );
    }

    #[test]
    fn websocket_message_events_append_transcript() {
        let mut session = RemoteHostSession::from_snapshot_json(SNAPSHOT_JSON).unwrap();

        session
            .apply_event_json(r#"{"type":"turn_start","turn":1}"#)
            .unwrap();
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
        assert_eq!(session.transcript.turns.len(), 1);
        assert_eq!(session.transcript.turns[0].number, 1);
        assert_eq!(
            session.transcript.turns[0].blocks,
            vec![crate::fixtures::TurnBlock::Text(crate::fixtures::AttributedText {
                text: "hello world".into(),
                origin: Some(crate::fixtures::BlockOrigin {
                    kind: crate::fixtures::OriginKind::Dispatcher,
                    label: "anthropic:claude-sonnet-4-6".into(),
                }),
                notice_kind: None,
            })]
        );
    }

    #[test]
    fn harness_warning_downgrades_shell_state() {
        let mut session = RemoteHostSession::from_snapshot_json(SNAPSHOT_JSON).unwrap();

        session
            .apply_event_json(
                r#"{"type":"harness_status_changed","status":{"git_branch":"main","git_detached":false,"thinking_level":"medium","capability_tier":"victory","providers":[],"memory_available":false,"cleave_available":true,"memory_warning":"Memory database unavailable","active_delegates":[]}}"#,
            )
            .unwrap();

        assert_eq!(session.shell_state(), ShellState::Degraded);
        assert_eq!(session.scenario(), DevScenario::Degraded);
        assert_eq!(session.summary().activity, "Memory database unavailable");
        assert_eq!(session.summary().activity_kind, ActivityKind::Degraded);
    }

    #[test]
    fn missing_authenticated_providers_block_submit_and_degrade_shell_state() {
        let mut session = RemoteHostSession::from_snapshot_json(SNAPSHOT_JSON).unwrap();

        session
            .apply_event_json(
                r#"{"type":"harness_status_changed","status":{"git_branch":"main","git_detached":false,"thinking_level":"medium","capability_tier":"victory","providers":[],"memory_available":true,"cleave_available":true,"memory_warning":null,"active_delegates":[]}}"#,
            )
            .unwrap();
        session.composer_mut().set_draft("hello");

        assert_eq!(session.shell_state(), ShellState::Degraded);
        assert_eq!(session.scenario(), DevScenario::Degraded);
        assert_eq!(session.summary().activity, "No authenticated providers reported by Omegon");
        assert_eq!(session.summary().activity_kind, ActivityKind::Degraded);
        assert!(!session.can_submit());
        assert!(!session.submit());
        assert_eq!(session.messages().len(), 1);
    }

    #[test]
    fn thinking_chunks_render_as_distinct_collapsed_blocks() {
        let mut session = RemoteHostSession::from_snapshot_json(SNAPSHOT_JSON).unwrap();

        session
            .apply_event_json(r#"{"type":"turn_start","turn":7}"#)
            .unwrap();
        session
            .apply_event_json(r#"{"type":"message_start","role":"assistant"}"#)
            .unwrap();
        session
            .apply_event_json(r#"{"type":"thinking_chunk","text":"inspect "}"#)
            .unwrap();
        session
            .apply_event_json(r#"{"type":"thinking_chunk","text":"state"}"#)
            .unwrap();
        session
            .apply_event_json(r#"{"type":"message_chunk","text":"done"}"#)
            .unwrap();
        session
            .apply_event_json(r#"{"type":"message_end"}"#)
            .unwrap();

        assert_eq!(session.messages().last().unwrap().text, "done");
        assert_eq!(session.transcript.turns.len(), 1);
        assert_eq!(session.transcript.turns[0].blocks.len(), 2);
        assert_eq!(
            session.transcript.turns[0].blocks[0],
            crate::fixtures::TurnBlock::Thinking(crate::fixtures::TurnBlockText {
                text: "inspect state".into(),
                collapsed: true,
            })
        );
        assert_eq!(
            session.transcript.turns[0].blocks[1],
            crate::fixtures::TurnBlock::Text(crate::fixtures::AttributedText {
                text: "done".into(),
                origin: Some(crate::fixtures::BlockOrigin {
                    kind: crate::fixtures::OriginKind::Dispatcher,
                    label: "anthropic:claude-sonnet-4-6".into(),
                }),
                notice_kind: None,
            })
        );
    }

    #[test]
    fn session_reset_clears_structured_transcript() {
        let mut session = RemoteHostSession::from_snapshot_json(SNAPSHOT_JSON).unwrap();

        session
            .apply_event_json(r#"{"type":"turn_start","turn":1}"#)
            .unwrap();
        session
            .apply_event_json(r#"{"type":"message_start","role":"assistant"}"#)
            .unwrap();
        session
            .apply_event_json(r#"{"type":"message_chunk","text":"hello"}"#)
            .unwrap();
        session
            .apply_event_json(r#"{"type":"message_end"}"#)
            .unwrap();
        assert_eq!(session.transcript.turns.len(), 1);
        assert_eq!(session.transcript.active_turn, Some(1));

        session
            .apply_event_json(r#"{"type":"session_reset"}"#)
            .unwrap();

        assert!(session.transcript.turns.is_empty());
        assert_eq!(session.transcript.active_turn, None);
        assert_eq!(session.messages().len(), 1);
        assert!(session.messages()[0].text.contains("cleared the cached transcript"));
    }

    #[test]
    fn tool_and_decomposition_events_refresh_activity_and_notices() {
        let mut session = RemoteHostSession::from_snapshot_json(SNAPSHOT_JSON).unwrap();

        session
            .apply_event_json(r#"{"type":"turn_start","turn":1}"#)
            .unwrap();
        session
            .apply_event_json(r#"{"type":"tool_start","id":"1","name":"read","args":{}}"#)
            .unwrap();
        assert_eq!(session.summary().activity, "Running tool read");
        assert_eq!(session.summary().activity_kind, ActivityKind::Running);

        session
            .apply_event_json(r#"{"type":"tool_end","id":"1","is_error":false,"result":"ok"}"#)
            .unwrap();
        assert_eq!(session.summary().activity, "Tool run completed");
        assert_eq!(session.summary().activity_kind, ActivityKind::Completed);

        session
            .apply_event_json(
                r#"{"type":"decomposition_started","children":["child-a","child-b"]}"#,
            )
            .unwrap();
        assert!(
            session
                .messages()
                .last()
                .unwrap()
                .text
                .contains("Dispatcher requested decomposition into 2 child task(s)")
        );

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

        session
            .apply_event_json(r#"{"type":"decomposition_completed","merged":true}"#)
            .unwrap();
        assert_eq!(session.summary().activity, "Cleave completed and merged");
        assert_eq!(session.summary().activity_kind, ActivityKind::Completed);
        assert!(
            session
                .messages()
                .last()
                .unwrap()
                .text
                .contains("merged child results")
        );
        assert!(matches!(
            session.transcript.turns[0].blocks.last(),
            Some(crate::fixtures::TurnBlock::System(crate::fixtures::AttributedText {
                text,
                origin,
                notice_kind: Some(crate::fixtures::SystemNoticeKind::CleaveComplete)
            }))
                if text.contains("merged child results")
                    && matches!(origin, Some(crate::fixtures::BlockOrigin { kind: crate::fixtures::OriginKind::Dispatcher, label }) if label == "anthropic:claude-sonnet-4-6")
        ));
    }

    #[test]
    fn dispatcher_and_child_system_blocks_carry_explicit_notice_kinds() {
        let mut session = RemoteHostSession::from_snapshot_json(SNAPSHOT_JSON).unwrap();

        session
            .apply_event_json(r#"{"type":"turn_start","turn":1}"#)
            .unwrap();
        session
            .apply_event_json(r#"{"type":"decomposition_started","children":["child-a","child-b"]}"#)
            .unwrap();
        session
            .apply_event_json(r#"{"type":"decomposition_child_completed","label":"child-a","success":true}"#)
            .unwrap();
        session
            .apply_event_json(r#"{"type":"decomposition_child_completed","label":"child-b","success":false}"#)
            .unwrap();

        let blocks = &session.transcript.turns[0].blocks;
        assert!(matches!(
            &blocks[0],
            crate::fixtures::TurnBlock::System(crate::fixtures::AttributedText {
                notice_kind: Some(crate::fixtures::SystemNoticeKind::CleaveStart),
                ..
            })
        ));
        assert!(matches!(
            &blocks[1],
            crate::fixtures::TurnBlock::System(crate::fixtures::AttributedText {
                notice_kind: Some(crate::fixtures::SystemNoticeKind::ChildStatus),
                ..
            })
        ));
        assert!(matches!(
            &blocks[2],
            crate::fixtures::TurnBlock::System(crate::fixtures::AttributedText {
                notice_kind: Some(crate::fixtures::SystemNoticeKind::Failure),
                ..
            })
        ));
    }
}
