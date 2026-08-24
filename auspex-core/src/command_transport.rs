#[cfg(not(target_arch = "wasm32"))]
use crate::ipc_client::{
    IpcCommandClient, IpcCommandKind, IpcCommandOutcome, IpcCommandOutcomeStatus,
};
use crate::runtime_types::TargetedCommand;

#[derive(Clone, Debug)]
pub enum CommandTransport {
    EventStream,
    #[cfg(not(target_arch = "wasm32"))]
    Ipc(IpcCommandClient),
}

impl CommandTransport {
    pub fn dispatch_targeted_command(
        &self,
        event_stream: Option<&crate::event_stream::EventStreamHandle>,
        command: &TargetedCommand,
    ) -> Result<(), String> {
        match self {
            Self::EventStream => {
                let stream = event_stream.ok_or_else(|| {
                    "event stream unavailable for websocket web-command dispatch".to_string()
                })?;
                stream.send_targeted_command(command);
                Ok(())
            }
            #[cfg(not(target_arch = "wasm32"))]
            Self::Ipc(client) => dispatch_over_ipc(client, command),
        }
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn dispatch_over_ipc(client: &IpcCommandClient, command: &TargetedCommand) -> Result<(), String> {
    let runtime = tokio::runtime::Handle::try_current()
        .map_err(|error| format!("tokio runtime unavailable for IPC dispatch: {error}"))?;

    match &command.command {
        crate::runtime_types::OperatorCommand::PromptSubmit { text } => {
            let client = client.clone();
            let text = text.clone();
            let target = command.target.clone();
            runtime.spawn(async move {
                let status = ingress_status(client.submit_prompt(&text).await);
                client.publish_command_outcome(IpcCommandOutcome {
                    target,
                    command: IpcCommandKind::PromptSubmit,
                    status,
                });
            });
            Ok(())
        }
        crate::runtime_types::OperatorCommand::TurnCancel => {
            let client = client.clone();
            let target = command.target.clone();
            runtime.spawn(async move {
                let status = ingress_status(client.cancel().await);
                client.publish_command_outcome(IpcCommandOutcome {
                    target,
                    command: IpcCommandKind::TurnCancel,
                    status,
                });
            });
            Ok(())
        }
        crate::runtime_types::OperatorCommand::CanonicalSlash { slash } => {
            let client = client.clone();
            let name = slash.name.clone();
            let args = slash.args.clone();
            runtime.spawn(async move {
                match client.run_slash_command(&name, &args).await {
                    Ok(result) if result.accepted => {}
                    Ok(result) => {
                        eprintln!(
                            "auspex: IPC slash command rejected: {}",
                            result
                                .output
                                .unwrap_or_else(|| "unknown rejection".to_string())
                        );
                    }
                    Err(error) => eprintln!("auspex: IPC slash command failed: {error}"),
                }
            });
            Ok(())
        }
        crate::runtime_types::OperatorCommand::ControlMethod {
            command_id,
            method,
            payload,
        } => {
            let client = client.clone();
            let method = method.clone();
            let payload = control_method_payload(command_id, payload.clone());
            runtime.spawn(async move {
                match client.control_method(&method, payload).await {
                    Ok(true) => {}
                    Ok(false) => {
                        eprintln!("auspex: IPC control method {method} was rejected by Omegon")
                    }
                    Err(error) => eprintln!("auspex: IPC control method {method} failed: {error}"),
                }
            });
            Ok(())
        }
        crate::runtime_types::OperatorCommand::DispatcherSwitch {
            request_id,
            profile,
            model,
        } => {
            let client = client.clone();
            let request_id = request_id.clone();
            let profile = profile.clone();
            let model = model.clone();
            runtime.spawn(async move {
                match client
                    .switch_dispatcher(&request_id, &profile, model.as_deref())
                    .await
                {
                    Ok(true) => {}
                    Ok(false) => {
                        eprintln!("auspex: IPC switch_dispatcher was rejected by Omegon");
                    }
                    Err(error) => eprintln!("auspex: IPC switch_dispatcher failed: {error}"),
                }
            });
            Ok(())
        }
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn ingress_status(result: Result<bool, String>) -> IpcCommandOutcomeStatus {
    match result {
        Ok(true) => IpcCommandOutcomeStatus::Accepted,
        Ok(false) => IpcCommandOutcomeStatus::Rejected,
        Err(error) => IpcCommandOutcomeStatus::Failed(error),
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn control_method_payload(
    command_id: &crate::runtime_types::ManagedCommandId,
    mut payload: serde_json::Value,
) -> serde_json::Value {
    if let serde_json::Value::Object(fields) = &mut payload {
        fields.insert(
            "command_id".into(),
            serde_json::to_value(command_id).expect("managed command identity must serialize"),
        );
    }
    payload
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn event_stream_transport_queues_web_command_json() {
        let transport = CommandTransport::EventStream;
        let handle = crate::event_stream::EventStreamHandle::websocket("ws://127.0.0.1:1/ws");
        let command = TargetedCommand::prompt_submit(
            crate::runtime_types::CommandTarget {
                session_key: "remote:session_01HVDEMO".into(),
                dispatcher_instance_id: Some("omg_primary_01HVDEMO".into()),
            },
            "hello",
        );

        transport
            .dispatch_targeted_command(Some(&handle), &command)
            .expect("event-stream dispatch should queue websocket web-command JSON");

        let commands = handle.debug_drain_outbox();
        assert_eq!(
            commands,
            vec![r#"{"text":"hello","type":"user_prompt"}"#.to_string()]
        );
    }

    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    fn ipc_control_payload_preserves_managed_command_identity() {
        let command_id = crate::runtime_types::ManagedCommandId::new();
        let payload =
            control_method_payload(&command_id, serde_json::json!({ "schema_version": 1 }));

        assert_eq!(
            payload["command_id"],
            serde_json::to_value(command_id).unwrap()
        );
        assert_eq!(payload["schema_version"], 1);
    }

    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    fn ipc_boolean_response_is_only_an_ingress_outcome() {
        assert_eq!(ingress_status(Ok(true)), IpcCommandOutcomeStatus::Accepted);
        assert_eq!(ingress_status(Ok(false)), IpcCommandOutcomeStatus::Rejected);
        assert_eq!(
            ingress_status(Err("disconnected".into())),
            IpcCommandOutcomeStatus::Failed("disconnected".into())
        );
    }

    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    fn ipc_dispatch_failure_does_not_fall_back_to_websocket() {
        let transport = CommandTransport::Ipc(IpcCommandClient::new("/missing/omegon.sock"));
        let websocket = crate::event_stream::EventStreamHandle::websocket("ws://127.0.0.1:1/ws");
        let command = TargetedCommand::turn_cancel(crate::runtime_types::CommandTarget {
            session_key: "remote:session-1".into(),
            dispatcher_instance_id: None,
        });

        assert!(
            transport
                .dispatch_targeted_command(Some(&websocket), &command)
                .is_err()
        );
        assert!(websocket.debug_drain_outbox().is_empty());
    }
}
