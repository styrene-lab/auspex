use std::{collections::BTreeMap, path::PathBuf};

pub const DEFAULT_LOCAL_CONTROL_PORTS: &[u16] = &[7842];

/// Where a local Omegon runtime candidate was discovered.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum LocalOmegonCandidateSource {
    AuspexOwnedPidFile,
    ProcessTable,
    KnownControlPort,
    ConfiguredLocal,
    IpcSocket,
}

/// What Auspex is allowed to assume about lifecycle authority.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum LocalOmegonOwnership {
    /// Auspex spawned this runtime and recorded the PID.
    AuspexOwned,
    /// Runtime appears to be a user-owned local Omegon process.
    UserOwned,
    /// Discovery evidence is insufficient to assign ownership.
    Unknown,
}

/// Non-mutating discovery candidate for a local Omegon runtime.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LocalOmegonCandidate {
    pub source: LocalOmegonCandidateSource,
    pub ownership: LocalOmegonOwnership,
    pub pid: Option<u32>,
    pub command: Option<String>,
    pub cwd: Option<PathBuf>,
    pub startup_url: Option<String>,
    pub state_url: Option<String>,
    pub ipc_socket: Option<PathBuf>,
}

impl LocalOmegonCandidate {
    pub fn auspex_owned_pid(pid: u32) -> Self {
        Self {
            source: LocalOmegonCandidateSource::AuspexOwnedPidFile,
            ownership: LocalOmegonOwnership::AuspexOwned,
            pid: Some(pid),
            command: None,
            cwd: None,
            startup_url: None,
            state_url: None,
            ipc_socket: None,
        }
    }

    pub fn from_process_table(pid: u32, command: impl Into<String>) -> Self {
        let command = command.into();
        Self {
            source: LocalOmegonCandidateSource::ProcessTable,
            ownership: LocalOmegonOwnership::UserOwned,
            pid: Some(pid),
            startup_url: infer_startup_url_from_command(&command),
            state_url: infer_state_url_from_command(&command),
            command: Some(command),
            cwd: None,
            ipc_socket: None,
        }
    }

    pub fn known_control_port(port: u16) -> Self {
        Self {
            source: LocalOmegonCandidateSource::KnownControlPort,
            ownership: LocalOmegonOwnership::Unknown,
            pid: None,
            command: None,
            cwd: None,
            startup_url: Some(format!("http://127.0.0.1:{port}/api/startup")),
            state_url: Some(format!("http://127.0.0.1:{port}/api/state")),
            ipc_socket: None,
        }
    }
}

pub fn parse_omegon_process_table(output: &str) -> Vec<LocalOmegonCandidate> {
    output
        .lines()
        .filter_map(parse_process_line)
        .filter(|(_, command)| is_omegon_serve_command(command))
        .map(|(pid, command)| LocalOmegonCandidate::from_process_table(pid, command))
        .collect()
}

fn parse_process_line(line: &str) -> Option<(u32, String)> {
    let trimmed = line.trim();
    let (pid, command) = trimmed.split_once(' ')?;
    Some((pid.trim().parse().ok()?, command.trim().to_string()))
}

fn is_omegon_serve_command(command: &str) -> bool {
    command.contains("omegon") && command.contains("serve")
}

fn infer_startup_url_from_command(command: &str) -> Option<String> {
    infer_control_port_from_command(command)
        .map(|port| format!("http://127.0.0.1:{port}/api/startup"))
}

fn infer_state_url_from_command(command: &str) -> Option<String> {
    infer_control_port_from_command(command)
        .map(|port| format!("http://127.0.0.1:{port}/api/state"))
}

fn infer_control_port_from_command(command: &str) -> Option<u16> {
    let mut parts = command.split_whitespace();
    while let Some(part) = parts.next() {
        if part == "--control-port" {
            return parts.next()?.parse().ok();
        }
        if let Some(value) = part.strip_prefix("--control-port=") {
            return value.parse().ok();
        }
    }
    None
}

pub fn discover_known_control_port_candidates(ports: &[u16]) -> Vec<LocalOmegonCandidate> {
    ports
        .iter()
        .copied()
        .map(LocalOmegonCandidate::known_control_port)
        .collect()
}

pub fn merge_local_omegon_candidates(
    candidates: impl IntoIterator<Item = LocalOmegonCandidate>,
) -> Vec<LocalOmegonCandidate> {
    let mut merged: BTreeMap<String, LocalOmegonCandidate> = BTreeMap::new();
    for candidate in candidates {
        let key = candidate_identity_key(&candidate);
        merged
            .entry(key)
            .and_modify(|existing| merge_candidate(existing, &candidate))
            .or_insert(candidate);
    }
    let mut by_pid = BTreeMap::new();
    for candidate in merged.into_values() {
        let key = candidate_identity_key(&candidate);
        by_pid
            .entry(key)
            .and_modify(|existing| merge_candidate(existing, &candidate))
            .or_insert(candidate);
    }
    by_pid.into_values().collect()
}

fn candidate_identity_key(candidate: &LocalOmegonCandidate) -> String {
    if let Some(state_url) = candidate.state_url.as_ref().filter(|url| !url.is_empty()) {
        return format!("state:{state_url}");
    }
    if let Some(pid) = candidate.pid {
        return format!("pid:{pid}");
    }
    if let Some(startup_url) = candidate.startup_url.as_ref().filter(|url| !url.is_empty()) {
        return format!("startup:{startup_url}");
    }
    if let Some(ipc_socket) = candidate.ipc_socket.as_ref() {
        return format!("ipc:{}", ipc_socket.display());
    }
    format!("source:{:?}", candidate.source)
}

fn merge_candidate(existing: &mut LocalOmegonCandidate, incoming: &LocalOmegonCandidate) {
    existing.ownership = strongest_ownership(&existing.ownership, &incoming.ownership);
    if incoming.source == LocalOmegonCandidateSource::AuspexOwnedPidFile {
        existing.source = LocalOmegonCandidateSource::AuspexOwnedPidFile;
    }
    if existing.pid.is_none() {
        existing.pid = incoming.pid;
    }
    if existing.command.is_none() {
        existing.command = incoming.command.clone();
    }
    if existing.cwd.is_none() {
        existing.cwd = incoming.cwd.clone();
    }
    if existing.startup_url.is_none() {
        existing.startup_url = incoming.startup_url.clone();
    }
    if existing.state_url.is_none() {
        existing.state_url = incoming.state_url.clone();
    }
    if existing.ipc_socket.is_none() {
        existing.ipc_socket = incoming.ipc_socket.clone();
    }
}

fn strongest_ownership(
    left: &LocalOmegonOwnership,
    right: &LocalOmegonOwnership,
) -> LocalOmegonOwnership {
    use LocalOmegonOwnership::*;
    match (left, right) {
        (AuspexOwned, _) | (_, AuspexOwned) => AuspexOwned,
        (UserOwned, _) | (_, UserOwned) => UserOwned,
        _ => Unknown,
    }
}

pub fn upgrade_owned_pid_candidates(
    candidates: impl IntoIterator<Item = LocalOmegonCandidate>,
    owned_pid: Option<u32>,
) -> Vec<LocalOmegonCandidate> {
    candidates
        .into_iter()
        .map(|mut candidate| {
            if candidate.pid.is_some() && candidate.pid == owned_pid {
                candidate.ownership = LocalOmegonOwnership::AuspexOwned;
                candidate.source = LocalOmegonCandidateSource::AuspexOwnedPidFile;
            }
            candidate
        })
        .collect()
}

#[cfg(not(target_arch = "wasm32"))]
pub fn discover_local_omegon_candidates() -> Vec<LocalOmegonCandidate> {
    let owned_pid = discover_owned_pid_candidate();
    let process_candidates = upgrade_owned_pid_candidates(
        discover_process_table_candidates(),
        owned_pid.as_ref().and_then(|candidate| candidate.pid),
    );
    merge_local_omegon_candidates(owned_pid.into_iter().chain(process_candidates).chain(
        discover_known_control_port_candidates(DEFAULT_LOCAL_CONTROL_PORTS),
    ))
}

#[cfg(not(target_arch = "wasm32"))]
pub fn discover_owned_pid_candidate() -> Option<LocalOmegonCandidate> {
    std::fs::read_to_string(std::env::temp_dir().join("auspex-owned-omegon.pid"))
        .ok()
        .and_then(|raw| raw.trim().parse::<u32>().ok())
        .map(LocalOmegonCandidate::auspex_owned_pid)
}

#[cfg(not(target_arch = "wasm32"))]
pub fn discover_process_table_candidates() -> Vec<LocalOmegonCandidate> {
    let output = match std::process::Command::new("ps")
        .args(["ax", "-o", "pid=,command="])
        .output()
    {
        Ok(output) if output.status.success() => output,
        _ => return Vec::new(),
    };
    parse_omegon_process_table(&String::from_utf8_lossy(&output.stdout))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_omegon_serve_processes_from_ps_output() {
        let output = r#"
          101 /usr/bin/zsh
          202 /Users/wilson/.cargo/bin/omegon serve --control-port 7842 --strict-port
          303 /Users/wilson/.cargo/bin/other serve --control-port 9999
        "#;

        let candidates = parse_omegon_process_table(output);

        assert_eq!(candidates.len(), 1);
        assert_eq!(candidates[0].pid, Some(202));
        assert_eq!(candidates[0].ownership, LocalOmegonOwnership::UserOwned);
        assert_eq!(
            candidates[0].startup_url.as_deref(),
            Some("http://127.0.0.1:7842/api/startup")
        );
        assert_eq!(
            candidates[0].state_url.as_deref(),
            Some("http://127.0.0.1:7842/api/state")
        );
    }

    #[test]
    fn parses_equals_style_control_port() {
        let output = "404 omegon serve --control-port=7850 --strict-port";
        let candidates = parse_omegon_process_table(output);
        assert_eq!(
            candidates[0].startup_url.as_deref(),
            Some("http://127.0.0.1:7850/api/startup")
        );
    }

    #[test]
    fn known_control_port_candidate_is_unknown_until_probed() {
        let candidate = LocalOmegonCandidate::known_control_port(7842);
        assert_eq!(
            candidate.source,
            LocalOmegonCandidateSource::KnownControlPort
        );
        assert_eq!(candidate.ownership, LocalOmegonOwnership::Unknown);
        assert_eq!(
            candidate.state_url.as_deref(),
            Some("http://127.0.0.1:7842/api/state")
        );
    }

    #[test]
    fn owned_pid_upgrades_matching_process_candidate() {
        let candidates = upgrade_owned_pid_candidates(
            vec![LocalOmegonCandidate::from_process_table(
                202,
                "omegon serve --control-port 7842 --strict-port",
            )],
            Some(202),
        );

        assert_eq!(candidates[0].ownership, LocalOmegonOwnership::AuspexOwned);
        assert_eq!(
            candidates[0].source,
            LocalOmegonCandidateSource::AuspexOwnedPidFile
        );
        assert_eq!(
            candidates[0].command.as_deref(),
            Some("omegon serve --control-port 7842 --strict-port")
        );
    }

    #[test]
    fn merge_deduplicates_candidates_by_state_url() {
        let merged = merge_local_omegon_candidates(vec![
            LocalOmegonCandidate::known_control_port(7842),
            LocalOmegonCandidate::from_process_table(
                202,
                "omegon serve --control-port 7842 --strict-port",
            ),
        ]);

        assert_eq!(merged.len(), 1);
        assert_eq!(merged[0].pid, Some(202));
        assert_eq!(merged[0].ownership, LocalOmegonOwnership::UserOwned);
    }

    #[test]
    fn owned_pid_candidate_is_lifecycle_manageable_by_classification() {
        let candidate = LocalOmegonCandidate::auspex_owned_pid(4242);
        assert_eq!(
            candidate.source,
            LocalOmegonCandidateSource::AuspexOwnedPidFile
        );
        assert_eq!(candidate.ownership, LocalOmegonOwnership::AuspexOwned);
        assert_eq!(candidate.pid, Some(4242));
    }
}
