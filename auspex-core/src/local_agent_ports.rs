#[cfg(not(target_arch = "wasm32"))]
use serde::{Deserialize, Serialize};

pub const LOCAL_AGENT_PORT_START: u16 = 7900;
pub const LOCAL_AGENT_PORT_END: u16 = 7999;

#[cfg(not(target_arch = "wasm32"))]
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct LocalAgentPortState {
    pub last_allocated: Option<u16>,
    #[serde(default)]
    pub reservations: Vec<LocalAgentPortReservation>,
}

#[cfg(not(target_arch = "wasm32"))]
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct LocalAgentPortReservation {
    pub port: u16,
    pub instance_id: Option<String>,
    pub label: Option<String>,
}

#[cfg(not(target_arch = "wasm32"))]
pub fn allocate_local_agent_port(label: Option<&str>) -> std::io::Result<u16> {
    let path = port_state_path();
    let mut state = read_port_state(&path).unwrap_or_default();
    let port = choose_next_port(&state)?;
    state.last_allocated = Some(port);
    state
        .reservations
        .retain(|reservation| reservation.port != port);
    state.reservations.push(LocalAgentPortReservation {
        port,
        instance_id: None,
        label: label.map(str::to_string),
    });
    write_port_state(&path, &state)?;
    Ok(port)
}

#[cfg(not(target_arch = "wasm32"))]
pub fn record_local_agent_port_instance(port: u16, instance_id: &str) -> std::io::Result<()> {
    let path = port_state_path();
    let mut state = read_port_state(&path).unwrap_or_default();
    if let Some(reservation) = state
        .reservations
        .iter_mut()
        .find(|reservation| reservation.port == port)
    {
        reservation.instance_id = Some(instance_id.to_string());
    } else {
        state.reservations.push(LocalAgentPortReservation {
            port,
            instance_id: Some(instance_id.to_string()),
            label: None,
        });
    }
    write_port_state(&path, &state)
}

#[cfg(not(target_arch = "wasm32"))]
pub fn choose_next_port(state: &LocalAgentPortState) -> std::io::Result<u16> {
    for port in monotonic_port_candidates(state.last_allocated) {
        if port_is_available(port) {
            return Ok(port);
        }
    }
    Err(std::io::Error::new(
        std::io::ErrorKind::AddrNotAvailable,
        format!(
            "no free agent control-plane port in {LOCAL_AGENT_PORT_START}..={LOCAL_AGENT_PORT_END}"
        ),
    ))
}

#[cfg(not(target_arch = "wasm32"))]
fn monotonic_port_candidates(last_allocated: Option<u16>) -> Vec<u16> {
    let start = match last_allocated {
        Some(port) if (LOCAL_AGENT_PORT_START..LOCAL_AGENT_PORT_END).contains(&port) => port + 1,
        _ => LOCAL_AGENT_PORT_START,
    };
    (start..=LOCAL_AGENT_PORT_END)
        .chain(LOCAL_AGENT_PORT_START..start)
        .collect()
}

#[cfg(not(target_arch = "wasm32"))]
fn port_is_available(port: u16) -> bool {
    std::net::TcpListener::bind(("127.0.0.1", port)).is_ok()
}

#[cfg(not(target_arch = "wasm32"))]
fn port_state_path() -> std::path::PathBuf {
    std::env::temp_dir().join("auspex-local-agent-ports.json")
}

#[cfg(not(target_arch = "wasm32"))]
fn read_port_state(path: &std::path::Path) -> Option<LocalAgentPortState> {
    std::fs::read_to_string(path)
        .ok()
        .and_then(|raw| serde_json::from_str(&raw).ok())
}

#[cfg(not(target_arch = "wasm32"))]
fn write_port_state(path: &std::path::Path, state: &LocalAgentPortState) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let raw = serde_json::to_string_pretty(state).map_err(std::io::Error::other)?;
    std::fs::write(path, raw)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    fn monotonic_candidates_start_after_last_allocation() {
        let candidates = monotonic_port_candidates(Some(7902));
        assert_eq!(candidates[0], 7903);
        assert_eq!(candidates.last().copied(), Some(7902));
    }

    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    fn monotonic_candidates_start_at_range_start_without_state() {
        let candidates = monotonic_port_candidates(None);
        assert_eq!(candidates[0], LOCAL_AGENT_PORT_START);
        assert_eq!(candidates.last().copied(), Some(LOCAL_AGENT_PORT_END));
    }
}
