#![cfg(feature = "desktop")]

use auspex_core::managed_agent_mqtt::{
    MANAGED_RUN_OUTCOME_SCHEMA, ManagedAgentMqttBridge, ManagedAgentMqttConfig,
    ManagedRunA2aOutcome, ManagedRunA2aOutcomePayload, ManagedRunA2aRequest,
};
use auspex_core::managed_agents::{ManagedRunRequest, WorkerId, WorkerProfile};
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use styrene_a2a::{AgentEnvelope, AgentEnvelopeKind, AgentId, RootOperationId, RuntimeId};
use styrene_mqtt::MqttA2aClient;

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_millis() as u64
}
fn broker() -> Option<(String, u16)> {
    let url = std::env::var("STYRENE_MQTT_TEST_URL").ok()?;
    let a = url.strip_prefix("mqtt://")?;
    let (h, p) = a.rsplit_once(':')?;
    Some((h.into(), p.parse().ok()?))
}

#[tokio::test]
async fn managed_run_round_trips_over_mosquitto() {
    let Some((host, port)) = broker() else { return };
    let n = now_ms();
    let tenant = format!("auspex-{n}");
    let local = AgentId::new(format!("auspex-{n}")).unwrap();
    let target = AgentId::new(format!("worker-{n}")).unwrap();
    let mut bridge = ManagedAgentMqttBridge::connect(ManagedAgentMqttConfig {
        tenant: tenant.clone(),
        host: host.clone(),
        port,
        client_id: format!("auspex-{n}"),
        local_agent_id: local.clone(),
        target_agent_id: target.clone(),
        session_expiry: Duration::from_secs(60),
    });
    bridge.initialize().await.unwrap();
    let mut worker = MqttA2aClient::connect(
        &tenant,
        format!("worker-{n}"),
        &host,
        port,
        Duration::from_secs(5),
        16,
    );
    worker.subscribe_agent(target.as_str()).await.unwrap();
    worker.poll_transport().await.unwrap();
    worker.poll_transport().await.unwrap();
    let request =
        ManagedRunRequest::new("prove mqtt", WorkerProfile::Scout, Default::default(), 30).unwrap();
    let worker_id = WorkerId::new();
    let run_id = auspex_core::managed_agents::ManagedRunId::new();
    let envelope = bridge
        .command_envelope(
            ManagedRunA2aRequest {
                managed_run_id: run_id,
                worker_id,
                parent_session_id: format!("root-{n}"),
                parent_turn_id: "turn-1".into(),
                request,
            },
            now_ms(),
            Duration::from_secs(30),
        )
        .unwrap();
    bridge.publish_command(&envelope, now_ms()).await.unwrap();
    let received = tokio::time::timeout(Duration::from_secs(3), worker.recv(now_ms()))
        .await
        .unwrap()
        .unwrap();
    assert_eq!(received.envelope.message_id, envelope.message_id);
    let payload = ManagedRunA2aOutcomePayload {
        managed_run_id: run_id,
        outcome: ManagedRunA2aOutcome::Completed {
            result: "mqtt-complete".into(),
        },
    };
    let outcome = AgentEnvelope::new(
        AgentEnvelopeKind::Result,
        &target,
        RuntimeId::new(),
        &local,
        &RootOperationId::new(format!("root-{n}")).unwrap(),
        Some(run_id.to_string()),
        run_id.to_string(),
        1,
        now_ms(),
        MANAGED_RUN_OUTCOME_SCHEMA,
        serde_json::to_vec(&payload).unwrap(),
    );
    worker.publish(&outcome, now_ms()).await.unwrap();
    worker.flush_publish().await.unwrap();
    let event = tokio::time::timeout(Duration::from_secs(3), bridge.receive_event(now_ms()))
        .await
        .unwrap()
        .unwrap();
    assert_eq!(event.managed_run_id(), run_id);
}
