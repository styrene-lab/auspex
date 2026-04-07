#[cfg(not(target_arch = "wasm32"))]
use crate::ipc_client::IpcCommandClient;
use crate::runtime_types::TargetedCommand;

#[derive(Clone, Debug)]
pub enum CommandTransport {
    #[cfg(not(target_arch = "wasm32"))]
    Ipc(IpcCommandClient),
}

impl CommandTransport {
    pub fn dispatch_targeted_command(&self, command: &TargetedCommand) -> Result<(), String> {
        match self {
            #[cfg(not(target_arch = "wasm32"))]
            Self::Ipc(client) => dispatch_over_ipc(client, command),
        }
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn dispatch_over_ipc(client: &IpcCommandClient, command: &TargetedCommand) -> Result<(), String> {
    let runtime = tokio::runtime::Handle::try_current()
        .map_err(|error| format!("tokio runtime unavailable for IPC dispatch: {error}"))?;
    let value: serde_json::Value = serde_json::from_str(&command.command_json)
        .map_err(|error| format!("invalid command JSON for IPC dispatch: {error}"))?;
    let command_type = value
        .get("type")
        .and_then(|value| value.as_str())
        .ok_or_else(|| "command JSON missing type field".to_string())?;
    match command_type {
        "user_prompt" => {
            let text = value
                .get("text")
                .and_then(|value| value.as_str())
                .ok_or_else(|| "user_prompt missing text field".to_string())?;
            let accepted = runtime.block_on(client.submit_prompt(text))?;
            if accepted {
                Ok(())
            } else {
                Err("IPC submit_prompt was rejected by Omegon".into())
            }
        }
        "cancel" => {
            let accepted = runtime.block_on(client.cancel())?;
            if accepted {
                Ok(())
            } else {
                Err("IPC cancel was rejected by Omegon".into())
            }
        }
        "slash_command" => {
            let name = value
                .get("name")
                .and_then(|value| value.as_str())
                .ok_or_else(|| "slash_command missing name field".to_string())?;
            let args = value
                .get("args")
                .and_then(|value| value.as_str())
                .unwrap_or_default();
            let result = runtime.block_on(client.run_slash_command(name, args))?;
            if result.accepted {
                Ok(())
            } else {
                Err(result
                    .output
                    .unwrap_or_else(|| "IPC slash command was rejected by Omegon".to_string()))
            }
        }
        other => Err(format!("unsupported IPC command type: {other}")),
    }
}

