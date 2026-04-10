#[cfg(not(target_arch = "wasm32"))]
use crate::ipc_client::IpcCommandClient;
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
                    "event stream unavailable for websocket command dispatch".to_string()
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
            runtime.spawn(async move {
                match client.submit_prompt(&text).await {
                    Ok(true) => {}
                    Ok(false) => {
                        eprintln!("auspex: IPC submit_prompt was rejected by Omegon");
                    }
                    Err(error) => eprintln!("auspex: IPC submit_prompt failed: {error}"),
                }
            });
            Ok(())
        }
        crate::runtime_types::OperatorCommand::TurnCancel => {
            let client = client.clone();
            runtime.spawn(async move {
                match client.cancel().await {
                    Ok(true) => {}
                    Ok(false) => {
                        eprintln!("auspex: IPC cancel was rejected by Omegon");
                    }
                    Err(error) => eprintln!("auspex: IPC cancel failed: {error}"),
                }
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
                            result.output.unwrap_or_else(|| "unknown rejection".to_string())
                        );
                    }
                    Err(error) => eprintln!("auspex: IPC slash command failed: {error}"),
                }
            });
            Ok(())
        }
        crate::runtime_types::OperatorCommand::LegacyJson { command_json } => {
            let value: serde_json::Value = serde_json::from_str(command_json)
                .map_err(|error| format!("invalid legacy command JSON for IPC dispatch: {error}"))?;
            let command_type = value
                .get("type")
                .and_then(|value| value.as_str())
                .ok_or_else(|| "legacy command JSON missing type field".to_string())?
                .to_string();

            match command_type.as_str() {
                "user_prompt" => {
                    let text = value
                        .get("text")
                        .and_then(|value| value.as_str())
                        .ok_or_else(|| "user_prompt missing text field".to_string())?
                        .to_string();
                    let client = client.clone();
                    runtime.spawn(async move {
                        match client.submit_prompt(&text).await {
                            Ok(true) => {}
                            Ok(false) => {
                                eprintln!("auspex: IPC submit_prompt was rejected by Omegon");
                            }
                            Err(error) => eprintln!("auspex: IPC submit_prompt failed: {error}"),
                        }
                    });
                    Ok(())
                }
                "cancel" => {
                    let client = client.clone();
                    runtime.spawn(async move {
                        match client.cancel().await {
                            Ok(true) => {}
                            Ok(false) => {
                                eprintln!("auspex: IPC cancel was rejected by Omegon");
                            }
                            Err(error) => eprintln!("auspex: IPC cancel failed: {error}"),
                        }
                    });
                    Ok(())
                }
                "slash_command" => {
                    let name = value
                        .get("name")
                        .and_then(|value| value.as_str())
                        .ok_or_else(|| "slash_command missing name field".to_string())?
                        .to_string();
                    let args = value
                        .get("args")
                        .and_then(|value| value.as_str())
                        .unwrap_or_default()
                        .to_string();
                    let client = client.clone();
                    runtime.spawn(async move {
                        match client.run_slash_command(&name, &args).await {
                            Ok(result) if result.accepted => {}
                            Ok(result) => {
                                eprintln!(
                                    "auspex: IPC slash command rejected: {}",
                                    result.output.unwrap_or_else(|| "unknown rejection".to_string())
                                );
                            }
                            Err(error) => eprintln!("auspex: IPC slash command failed: {error}"),
                        }
                    });
                    Ok(())
                }
                other => Err(format!("unsupported legacy IPC command type: {other}")),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn event_stream_transport_queues_raw_command_json() {
        let transport = CommandTransport::EventStream;
        let handle = crate::event_stream::EventStreamHandle::websocket("ws://127.0.0.1:1/ws");
        let command = TargetedCommand::legacy_json(
            crate::runtime_types::CommandTarget {
                session_key: "remote:session_01HVDEMO".into(),
                dispatcher_instance_id: Some("omg_primary_01HVDEMO".into()),
            },
            r#"{"type":"user_prompt","text":"hello"}"#,
        );

        transport
            .dispatch_targeted_command(Some(&handle), &command)
            .expect("event-stream dispatch should queue raw JSON");

        let commands = handle.debug_drain_outbox();
        assert_eq!(
            commands,
            vec![r#"{"type":"user_prompt","text":"hello"}"#.to_string()]
        );
    }
}
