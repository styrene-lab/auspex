//! Async ownership boundary for the managed-agent MQTT bearer.
//!
//! The orchestrator owns the mutable rumqttc event loop. UI/controller code talks
//! to it through bounded channels and never holds a signal lock across `.await`.

use std::time::{Duration, SystemTime, UNIX_EPOCH};

use tokio::sync::mpsc;

use crate::managed_agent_mqtt::{
    ManagedAgentA2aEvent, ManagedAgentMqttBridge, ManagedAgentMqttConfig, ManagedAgentMqttError,
    ManagedRunA2aRequest,
};

const COMMAND_CHANNEL_CAPACITY: usize = 64;
const EVENT_CHANNEL_CAPACITY: usize = 128;
const RECEIVE_SLICE: Duration = Duration::from_millis(250);
const INITIAL_RECONNECT_DELAY: Duration = Duration::from_millis(250);
const MAX_RECONNECT_DELAY: Duration = Duration::from_secs(10);

#[derive(Clone, Debug)]
pub struct ManagedAgentMqttOrchestratorConfig {
    pub bridge: ManagedAgentMqttConfig,
    pub command_ttl: Duration,
}

impl ManagedAgentMqttOrchestratorConfig {
    pub fn from_env() -> Result<Option<Self>, String> {
        if !env_flag("AUSPEX_MQTT_ENABLED") {
            return Ok(None);
        }
        let broker = std::env::var("AUSPEX_MQTT_BROKER_URL")
            .unwrap_or_else(|_| "mqtt://127.0.0.1:1883".into());
        let (host, port) = parse_mqtt_url(&broker)?;
        let local = std::env::var("AUSPEX_MQTT_LOCAL_AGENT_ID")
            .unwrap_or_else(|_| "styrene:agent:auspex".into());
        let target = std::env::var("AUSPEX_MQTT_TARGET_AGENT_ID")
            .unwrap_or_else(|_| "styrene:agent:omegon".into());
        Ok(Some(Self {
            bridge: ManagedAgentMqttConfig {
                tenant: std::env::var("AUSPEX_MQTT_TENANT").unwrap_or_else(|_| "local".into()),
                host,
                port,
                client_id: std::env::var("AUSPEX_MQTT_CLIENT_ID")
                    .unwrap_or_else(|_| format!("auspex-{}", std::process::id())),
                local_agent_id: styrene_a2a::AgentId::new(local).map_err(|e| e.to_string())?,
                target_agent_id: styrene_a2a::AgentId::new(target).map_err(|e| e.to_string())?,
                session_expiry: Duration::from_secs(env_u64(
                    "AUSPEX_MQTT_SESSION_EXPIRY_SECS",
                    900,
                )),
            },
            command_ttl: Duration::from_secs(env_u64("AUSPEX_MQTT_COMMAND_TTL_SECS", 30)),
        }))
    }
}

#[derive(Clone, Debug)]
pub enum ManagedAgentMqttCommand {
    Dispatch(ManagedRunA2aRequest),
    Shutdown,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ManagedAgentMqttOrchestratorEvent {
    Connected,
    Disconnected {
        error: String,
    },
    Outcome(ManagedAgentA2aEvent),
    CommandPublished {
        managed_run_id: crate::managed_agents::ManagedRunId,
    },
    CommandFailed {
        managed_run_id: crate::managed_agents::ManagedRunId,
        error: String,
    },
    Stopped,
}

#[derive(Clone)]
pub struct ManagedAgentMqttOrchestratorHandle {
    commands: mpsc::Sender<ManagedAgentMqttCommand>,
}

impl ManagedAgentMqttOrchestratorHandle {
    pub async fn dispatch(&self, request: ManagedRunA2aRequest) -> Result<(), String> {
        self.commands
            .send(ManagedAgentMqttCommand::Dispatch(request))
            .await
            .map_err(|_| "managed-agent MQTT orchestrator stopped".to_string())
    }

    pub async fn shutdown(&self) -> Result<(), String> {
        self.commands
            .send(ManagedAgentMqttCommand::Shutdown)
            .await
            .map_err(|_| "managed-agent MQTT orchestrator stopped".to_string())
    }
}

pub fn spawn_managed_agent_mqtt_orchestrator(
    config: ManagedAgentMqttOrchestratorConfig,
) -> (
    ManagedAgentMqttOrchestratorHandle,
    mpsc::Receiver<ManagedAgentMqttOrchestratorEvent>,
) {
    let (command_tx, command_rx) = mpsc::channel(COMMAND_CHANNEL_CAPACITY);
    let (event_tx, event_rx) = mpsc::channel(EVENT_CHANNEL_CAPACITY);
    tokio::spawn(run_orchestrator(config, command_rx, event_tx));
    (
        ManagedAgentMqttOrchestratorHandle {
            commands: command_tx,
        },
        event_rx,
    )
}

async fn run_orchestrator(
    config: ManagedAgentMqttOrchestratorConfig,
    mut commands: mpsc::Receiver<ManagedAgentMqttCommand>,
    events: mpsc::Sender<ManagedAgentMqttOrchestratorEvent>,
) {
    let mut delay = INITIAL_RECONNECT_DELAY;
    'outer: loop {
        let mut bridge = ManagedAgentMqttBridge::connect(config.bridge.clone());
        if let Err(error) = bridge.initialize().await {
            if !send_event(
                &events,
                ManagedAgentMqttOrchestratorEvent::Disconnected {
                    error: error.to_string(),
                },
            )
            .await
            {
                break;
            }
            tokio::time::sleep(delay).await;
            delay = (delay * 2).min(MAX_RECONNECT_DELAY);
            continue;
        }
        delay = INITIAL_RECONNECT_DELAY;
        if !send_event(&events, ManagedAgentMqttOrchestratorEvent::Connected).await {
            break;
        }

        loop {
            while let Ok(command) = commands.try_recv() {
                match command {
                    ManagedAgentMqttCommand::Shutdown => break 'outer,
                    ManagedAgentMqttCommand::Dispatch(request) => {
                        let run_id = request.managed_run_id;
                        let result = publish(&mut bridge, request, config.command_ttl).await;
                        let event = match result {
                            Ok(()) => ManagedAgentMqttOrchestratorEvent::CommandPublished {
                                managed_run_id: run_id,
                            },
                            Err(ref error) => ManagedAgentMqttOrchestratorEvent::CommandFailed {
                                managed_run_id: run_id,
                                error: error.to_string(),
                            },
                        };
                        if !send_event(&events, event).await {
                            break 'outer;
                        }
                        if result.is_err() {
                            continue 'outer;
                        }
                    }
                }
            }

            match tokio::time::timeout(RECEIVE_SLICE, bridge.receive_event(now_ms())).await {
                Ok(Ok(event)) => {
                    if !send_event(&events, ManagedAgentMqttOrchestratorEvent::Outcome(event)).await
                    {
                        break 'outer;
                    }
                }
                Ok(Err(error)) => {
                    if !send_event(
                        &events,
                        ManagedAgentMqttOrchestratorEvent::Disconnected {
                            error: error.to_string(),
                        },
                    )
                    .await
                    {
                        break 'outer;
                    }
                    tokio::time::sleep(delay).await;
                    delay = (delay * 2).min(MAX_RECONNECT_DELAY);
                    continue 'outer;
                }
                Err(_) => {}
            }
            if commands.is_closed() {
                break 'outer;
            }
        }
    }
    let _ = events
        .send(ManagedAgentMqttOrchestratorEvent::Stopped)
        .await;
}

async fn publish(
    bridge: &mut ManagedAgentMqttBridge,
    request: ManagedRunA2aRequest,
    ttl: Duration,
) -> Result<(), ManagedAgentMqttError> {
    let now = now_ms();
    let envelope = bridge.command_envelope(request, now, ttl)?;
    bridge.publish_command(&envelope, now).await
}

async fn send_event(
    sender: &mpsc::Sender<ManagedAgentMqttOrchestratorEvent>,
    event: ManagedAgentMqttOrchestratorEvent,
) -> bool {
    sender.send(event).await.is_ok()
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

fn parse_mqtt_url(url: &str) -> Result<(String, u16), String> {
    let authority = url
        .strip_prefix("mqtt://")
        .ok_or_else(|| "AUSPEX_MQTT_BROKER_URL must use mqtt://".to_string())?;
    let (host, port) = authority
        .rsplit_once(':')
        .ok_or_else(|| "AUSPEX_MQTT_BROKER_URL must include a port".to_string())?;
    if host.is_empty() {
        return Err("AUSPEX_MQTT_BROKER_URL host is empty".into());
    }
    Ok((
        host.to_string(),
        port.parse()
            .map_err(|_| "invalid MQTT broker port".to_string())?,
    ))
}

fn env_flag(name: &str) -> bool {
    std::env::var(name)
        .ok()
        .is_some_and(|v| matches!(v.to_ascii_lowercase().as_str(), "1" | "true" | "yes" | "on"))
}

fn env_u64(name: &str, default: u64) -> u64 {
    std::env::var(name)
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(default)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_mqtt_url_and_rejects_non_mqtt_schemes() {
        assert_eq!(
            parse_mqtt_url("mqtt://broker:1883").unwrap(),
            ("broker".into(), 1883)
        );
        assert!(parse_mqtt_url("mqtts://broker:8883").is_err());
        assert!(parse_mqtt_url("mqtt://broker").is_err());
    }
}
