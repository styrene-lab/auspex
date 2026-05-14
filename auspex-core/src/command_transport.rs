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
                    "event stream unavailable for websocket web-command dispatch".to_string()
                })?;
                stream.send_targeted_command(command);
                Ok(())
            }
            #[cfg(not(target_arch = "wasm32"))]
            Self::Ipc(client) => {
                let result = dispatch_over_ipc(client, command);
                if result.is_err() {
                    // IPC failed (broken pipe, etc.) — fall back to WebSocket.
                    if let Some(stream) = event_stream {
                        eprintln!("auspex: IPC dispatch failed, falling back to WebSocket");
                        stream.send_targeted_command(command);
                        return Ok(());
                    }
                }
                result
            }
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
}
