use std::path::PathBuf;

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
    infer_control_port_from_command(command).map(|port| format!("http://127.0.0.1:{port}/api/startup"))
}

fn infer_state_url_from_command(command: &str) -> Option<String> {
    infer_control_port_from_command(command).map(|port| format!("http://127.0.0.1:{port}/api/state"))
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
    fn owned_pid_candidate_is_lifecycle_manageable_by_classification() {
        let candidate = LocalOmegonCandidate::auspex_owned_pid(4242);
        assert_eq!(candidate.source, LocalOmegonCandidateSource::AuspexOwnedPidFile);
        assert_eq!(candidate.ownership, LocalOmegonOwnership::AuspexOwned);
        assert_eq!(candidate.pid, Some(4242));
    }
}
