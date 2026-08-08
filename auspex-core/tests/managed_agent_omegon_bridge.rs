#![cfg(feature = "desktop")]

use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::Arc;
use std::time::Duration;

use auspex_core::controller::AppController;
use auspex_core::managed_agent_bridge_server::ManagedAgentBridgeServer;
use auspex_core::managed_agent_feature::{
    ManagedAgentProjectionSource, SharedManagedAgentProjectionSnapshot,
};
use auspex_core::managed_agent_mutation_coordinator::ManagedAgentMutationCoordinator;
use auspex_core::omegon_control::OmegonStartupInfo;
use futures_util::{SinkExt, StreamExt};
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio_tungstenite::tungstenite::Message;

fn fixture_binary() -> Option<PathBuf> {
    std::env::var_os("AUSPEX_OMEGON_E2E_BIN")
        .map(PathBuf::from)
        .or_else(|| which("omegon"))
}

fn which(name: &str) -> Option<PathBuf> {
    let output = std::process::Command::new("which")
        .arg(name)
        .output()
        .ok()?;
    output
        .status
        .success()
        .then(|| PathBuf::from(String::from_utf8_lossy(&output.stdout).trim().to_string()))
}

fn copy_plugin(root: &Path) {
    let plugin = root.join(".omegon/plugins/auspex-managed-agents");
    std::fs::create_dir_all(plugin.join("tools")).unwrap();
    std::fs::write(
        plugin.join("plugin.toml"),
        include_str!("../../assets/managed-agent-plugin/plugin.toml"),
    )
    .unwrap();
    std::fs::write(
        plugin.join("tools/agents_status.py"),
        include_str!("../../assets/managed-agent-plugin/tools/agents_status.py"),
    )
    .unwrap();
    std::fs::write(
        plugin.join("tools/agents_dispatch.py"),
        include_str!("../../assets/managed-agent-plugin/tools/agents_dispatch.py"),
    )
    .unwrap();
    std::fs::write(
        plugin.join("tools/agents_cancel.py"),
        include_str!("../../assets/managed-agent-plugin/tools/agents_cancel.py"),
    )
    .unwrap();
}

fn fixture_supports_agents_status(binary: &Path) -> bool {
    std::process::Command::new(binary)
        .args(["serve", "--help"])
        .output()
        .ok()
        .is_some_and(|output| output.status.success())
}

#[tokio::test]
async fn omegon_agents_status_reaches_private_auspex_bridge() {
    let Some(binary) = fixture_binary() else {
        eprintln!("skipping: Omegon fixture is unavailable");
        return;
    };
    if !fixture_supports_agents_status(&binary) {
        eprintln!("skipping: Omegon fixture cannot serve the control plane");
        return;
    }

    let nonce = uuid::Uuid::new_v4().simple().to_string();
    let root = std::env::temp_dir().join(format!("axb-{}", &nonce[..8]));
    std::fs::create_dir_all(&root).unwrap();
    copy_plugin(&root);

    let socket_path = root.join("managed-agents.sock");
    let capability = format!(
        "{}{}",
        uuid::Uuid::new_v4().simple(),
        uuid::Uuid::new_v4().simple()
    );
    let parent_session_id = format!("primary-{nonce}");
    let snapshot = SharedManagedAgentProjectionSnapshot::default();
    snapshot
        .write()
        .replace_parent(parent_session_id.clone(), Vec::new());
    let source: Arc<dyn auspex_core::managed_agent_feature::ManagedAgentProjectionSource> =
        snapshot.clone();
    let (mutations, mut coordinator) = ManagedAgentMutationCoordinator::channel(8);
    let bridge = ManagedAgentBridgeServer::bind(
        &socket_path,
        capability.clone(),
        parent_session_id.clone(),
        source,
        Some(mutations),
    )
    .await
    .unwrap();

    let listener = std::net::TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, 0)).unwrap();
    let port = listener.local_addr().unwrap().port();
    drop(listener);

    let mut child = tokio::process::Command::new(&binary);
    child
        .current_dir(&root)
        .args([
            "serve",
            "--control-port",
            &port.to_string(),
            "--strict-port",
        ])
        .env("AUSPEX_MANAGED_AGENT_BRIDGE_SOCKET", &socket_path)
        .env("AUSPEX_MANAGED_AGENT_BRIDGE_CAPABILITY", &capability)
        .env("AUSPEX_MANAGED_AGENT_PARENT_SESSION_ID", &parent_session_id)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let mut child = child.spawn().unwrap();

    let stdout = child.stdout.take().unwrap();
    let mut lines = BufReader::new(stdout).lines();
    let startup = tokio::time::timeout(Duration::from_secs(60), async {
        loop {
            let line = lines.next_line().await.unwrap()?;
            if let Ok(startup) = serde_json::from_str::<OmegonStartupInfo>(&line) {
                break Some(startup);
            }
        }
    })
    .await
    .expect("Omegon startup timed out")
    .expect("Omegon exited before startup");

    let (mut websocket, _) = tokio_tungstenite::connect_async(&startup.ws_url)
        .await
        .unwrap();
    let managed_run_id = uuid::Uuid::new_v4().to_string();
    let worker_id = uuid::Uuid::new_v4().to_string();
    websocket
        .send(Message::Text(
            serde_json::json!({
                "type": "agents_status",
                "schema_version": 1,
                "managed_run_id": managed_run_id,
                "worker_id": worker_id,
                "run_id": null,
                "caller_role": "read"
            })
            .to_string()
            .into(),
        ))
        .await
        .unwrap();

    let response = tokio::time::timeout(Duration::from_secs(10), async {
        loop {
            let message = websocket.next().await?.ok()?;
            let Message::Text(text) = message else {
                continue;
            };
            let value: serde_json::Value = serde_json::from_str(&text).ok()?;
            if value.get("managed_run_id").and_then(|v| v.as_str()) == Some(&managed_run_id) {
                break Some(value);
            }
            eprintln!("ignored websocket event: {value}");
        }
    })
    .await
    .expect("agents_status response timed out")
    .expect("websocket closed before agents_status response");

    assert_eq!(response["schema_version"], 1);
    assert_eq!(response["managed_run_id"], managed_run_id);
    assert_eq!(response["worker_id"], worker_id);
    assert!(response.get("status").is_some(), "response: {response}");

    let mut controller = AppController::default();
    let mut actions = std::collections::VecDeque::new();
    let tool_script = root.join(".omegon/plugins/auspex-managed-agents/tools/agents_dispatch.py");
    let mut tool = tokio::process::Command::new("python3")
        .arg(&tool_script)
        .env("AUSPEX_MANAGED_AGENT_BRIDGE_SOCKET", &socket_path)
        .env("AUSPEX_MANAGED_AGENT_BRIDGE_CAPABILITY", &capability)
        .env("AUSPEX_MANAGED_AGENT_PARENT_SESSION_ID", &parent_session_id)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    tokio::io::AsyncWriteExt::write_all(
        tool.stdin.as_mut().unwrap(),
        serde_json::json!({
            "operation_id": "e2e-operation-1",
            "directive": "inspect the managed bridge"
        })
        .to_string()
        .as_bytes(),
    )
    .await
    .unwrap();
    drop(tool.stdin.take());
    let run_id = tokio::time::timeout(Duration::from_secs(10), async {
        loop {
            coordinator.process_pending(
                &mut controller,
                &mut actions,
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_millis() as u64,
            );
            if let Some(run_id) = controller.managed_agent_runtime().active_run_ids().first() {
                break *run_id;
            }
            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("managed dispatch timed out");
    assert!(matches!(
        actions.pop_front(),
        Some(auspex_core::controller::ManagedAgentTransportAction::StyreneA2aDispatch(_))
    ));
    let output = tool.wait_with_output().await.unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stdout)
    );
    let tool_result: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(tool_result["result"]["run_id"], run_id.to_string());
    assert_eq!(tool_result["result"]["duplicate"], false);

    let mut duplicate_tool = tokio::process::Command::new("python3")
        .arg(&tool_script)
        .env("AUSPEX_MANAGED_AGENT_BRIDGE_SOCKET", &socket_path)
        .env("AUSPEX_MANAGED_AGENT_BRIDGE_CAPABILITY", &capability)
        .env("AUSPEX_MANAGED_AGENT_PARENT_SESSION_ID", &parent_session_id)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    tokio::io::AsyncWriteExt::write_all(
        duplicate_tool.stdin.as_mut().unwrap(),
        serde_json::json!({
            "operation_id": "e2e-operation-1",
            "directive": "inspect the managed bridge"
        })
        .to_string()
        .as_bytes(),
    )
    .await
    .unwrap();
    drop(duplicate_tool.stdin.take());
    tokio::time::timeout(Duration::from_secs(10), async {
        loop {
            if coordinator.process_pending(
                &mut controller,
                &mut actions,
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_millis() as u64,
            ) > 0
            {
                break;
            }
            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("duplicate dispatch timed out");
    let duplicate_output = duplicate_tool.wait_with_output().await.unwrap();
    assert!(
        duplicate_output.status.success(),
        "{}",
        String::from_utf8_lossy(&duplicate_output.stdout)
    );
    let duplicate_result: serde_json::Value =
        serde_json::from_slice(&duplicate_output.stdout).unwrap();
    assert_eq!(duplicate_result["result"]["run_id"], run_id.to_string());
    assert_eq!(duplicate_result["result"]["duplicate"], true);
    assert!(
        actions.is_empty(),
        "duplicate dispatch enqueued transport work"
    );

    controller
        .managed_agent_runtime_mut()
        .apply_a2a_event(
            &auspex_core::managed_agent_mqtt::ManagedAgentA2aEvent::Accepted {
                managed_run_id: run_id,
                task_id: "e2e-task-1".into(),
                message_id: [7; 16],
            },
        )
        .unwrap();

    let cancel_script = root.join(".omegon/plugins/auspex-managed-agents/tools/agents_cancel.py");
    let mut cancel_tool = tokio::process::Command::new("python3")
        .arg(&cancel_script)
        .env("AUSPEX_MANAGED_AGENT_BRIDGE_SOCKET", &socket_path)
        .env("AUSPEX_MANAGED_AGENT_BRIDGE_CAPABILITY", &capability)
        .env("AUSPEX_MANAGED_AGENT_PARENT_SESSION_ID", &parent_session_id)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    tokio::io::AsyncWriteExt::write_all(
        cancel_tool.stdin.as_mut().unwrap(),
        serde_json::json!({
            "operation_id": "e2e-cancel-1",
            "run_id": run_id,
            "reason": "integration complete"
        })
        .to_string()
        .as_bytes(),
    )
    .await
    .unwrap();
    drop(cancel_tool.stdin.take());
    tokio::time::timeout(Duration::from_secs(10), async {
        loop {
            if coordinator.process_pending(
                &mut controller,
                &mut actions,
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_millis() as u64,
            ) > 0
            {
                break;
            }
            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("managed cancellation timed out");
    assert!(matches!(
        actions.pop_front(),
        Some(auspex_core::controller::ManagedAgentTransportAction::StyreneA2aCancel { .. })
    ));
    let cancel_output = cancel_tool.wait_with_output().await.unwrap();
    assert!(
        cancel_output.status.success(),
        "{}",
        String::from_utf8_lossy(&cancel_output.stdout)
    );
    let cancel_result: serde_json::Value = serde_json::from_slice(&cancel_output.stdout).unwrap();
    assert_eq!(cancel_result["result"]["run_id"], run_id.to_string());
    assert_eq!(cancel_result["result"]["duplicate"], false);

    let mut duplicate_cancel = tokio::process::Command::new("python3")
        .arg(&cancel_script)
        .env("AUSPEX_MANAGED_AGENT_BRIDGE_SOCKET", &socket_path)
        .env("AUSPEX_MANAGED_AGENT_BRIDGE_CAPABILITY", &capability)
        .env("AUSPEX_MANAGED_AGENT_PARENT_SESSION_ID", &parent_session_id)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    tokio::io::AsyncWriteExt::write_all(
        duplicate_cancel.stdin.as_mut().unwrap(),
        serde_json::json!({
            "operation_id": "e2e-cancel-1",
            "run_id": run_id,
            "reason": "integration complete"
        })
        .to_string()
        .as_bytes(),
    )
    .await
    .unwrap();
    drop(duplicate_cancel.stdin.take());
    tokio::time::timeout(Duration::from_secs(10), async {
        loop {
            if coordinator.process_pending(
                &mut controller,
                &mut actions,
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_millis() as u64,
            ) > 0
            {
                break;
            }
            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("duplicate cancellation timed out");
    let duplicate_cancel_output = duplicate_cancel.wait_with_output().await.unwrap();
    assert!(duplicate_cancel_output.status.success());
    let duplicate_cancel_result: serde_json::Value =
        serde_json::from_slice(&duplicate_cancel_output.stdout).unwrap();
    assert_eq!(
        duplicate_cancel_result["result"]["run_id"],
        run_id.to_string()
    );
    assert_eq!(duplicate_cancel_result["result"]["duplicate"], true);
    assert!(
        actions.is_empty(),
        "duplicate cancellation enqueued transport work"
    );

    controller.refresh_managed_agent_projection_snapshot(&parent_session_id, &snapshot);
    let projection = snapshot
        .read()
        .status(&parent_session_id, Some(run_id))
        .unwrap()
        .remove(0);
    assert!(matches!(
        projection.state,
        auspex_core::managed_agents::ManagedRunState::Cancelling { .. }
    ));

    let _ = child.start_kill();
    let _ = child.wait().await;
    drop(bridge);
    let _ = std::fs::remove_dir_all(root);
}
