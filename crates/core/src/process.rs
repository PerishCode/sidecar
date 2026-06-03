//! Cross-platform process discovery via stamp args matching.
//!
//! Unix uses `ps -axo pid=,command=`. Windows is not yet implemented.

use crate::stamp::read_stamp;
#[cfg(unix)]
use std::process::{Command, Stdio};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StampedProcess {
    pub pid: u32,
    pub command: String,
}

pub fn discover_by_app_namespace(
    app: &str,
    namespace: &str,
) -> Result<Vec<StampedProcess>, String> {
    Ok(filter_stamped(ps_command_lines()?, |args| {
        match_app(args, app) && match_namespace(args, namespace)
    }))
}

pub fn discover_by_namespace(namespace: &str) -> Result<Vec<StampedProcess>, String> {
    Ok(filter_stamped(ps_command_lines()?, |args| {
        match_namespace(args, namespace)
    }))
}

fn match_app(args: &[String], app: &str) -> bool {
    read_stamp(args)
        .map(|stamp| stamp.app == app)
        .unwrap_or(false)
}

fn match_namespace(args: &[String], namespace: &str) -> bool {
    read_stamp(args)
        .map(|stamp| stamp.namespace == namespace)
        .unwrap_or(false)
}

fn filter_stamped<F>(rows: Vec<(u32, String)>, predicate: F) -> Vec<StampedProcess>
where
    F: Fn(&[String]) -> bool,
{
    rows.into_iter()
        .filter_map(|(pid, command)| {
            let args: Vec<String> = command.split_whitespace().map(String::from).collect();
            if predicate(&args) {
                Some(StampedProcess { pid, command })
            } else {
                None
            }
        })
        .collect()
}

#[cfg(unix)]
fn ps_command_lines() -> Result<Vec<(u32, String)>, String> {
    let output = Command::new("ps")
        .args(["-axo", "pid=,command="])
        .output()
        .map_err(|err| format!("ps failed: {err}"))?;
    if !output.status.success() {
        return Err(format!(
            "ps exited with status: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }
    let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
    Ok(parse_ps_output(&stdout))
}

#[cfg(not(unix))]
fn ps_command_lines() -> Result<Vec<(u32, String)>, String> {
    Err("process discovery is not yet implemented on this platform".to_string())
}

pub fn parse_ps_output(text: &str) -> Vec<(u32, String)> {
    text.lines()
        .filter_map(|line| {
            let trimmed = line.trim_start();
            let mut parts = trimmed.splitn(2, char::is_whitespace);
            let pid_str = parts.next()?.trim();
            let command = parts.next()?.trim().to_string();
            let pid: u32 = pid_str.parse().ok()?;
            Some((pid, command))
        })
        .collect()
}

#[cfg(unix)]
pub fn signal_terminate(pid: u32) -> Result<(), String> {
    let process_group = format!("-{pid}");
    let group_status = Command::new("kill")
        .args(["-TERM", &process_group])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map_err(|err| format!("kill failed: {err}"))?;
    if group_status.success() {
        return Ok(());
    }

    let status = Command::new("kill")
        .args(["-TERM", &pid.to_string()])
        .status()
        .map_err(|err| format!("kill failed: {err}"))?;
    if status.success() {
        Ok(())
    } else {
        Err(format!(
            "kill -TERM -{pid} exited with status {group_status}; kill -TERM {pid} exited with status {status}"
        ))
    }
}

#[cfg(not(unix))]
pub fn signal_terminate(_pid: u32) -> Result<(), String> {
    Err("process termination is not yet implemented on this platform".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_ps_extracts_pid_and_command() {
        let text =
            "  123 cargo run --sidecar-stamp=a=api;n=default;m=dev;s=tool%3Asidecar\n  456 node server.js\n";
        let parsed = parse_ps_output(text);
        assert_eq!(parsed.len(), 2);
        assert_eq!(
            parsed[0],
            (
                123,
                "cargo run --sidecar-stamp=a=api;n=default;m=dev;s=tool%3Asidecar".into()
            )
        );
        assert_eq!(parsed[1], (456, "node server.js".into()));
    }

    #[test]
    fn filter_picks_app_namespace_matches() {
        let rows = vec![
            (
                10,
                "controller --sidecar-stamp=a=controller;n=default;m=dev;s=tool%3Asidecar".into(),
            ),
            (
                11,
                "renderer --sidecar-stamp=a=renderer;n=default;m=dev;s=tool%3Asidecar".into(),
            ),
            (12, "noise --no-stamp".into()),
        ];
        let hits = filter_stamped(rows, |args| {
            match_app(args, "controller") && match_namespace(args, "default")
        });
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].pid, 10);
    }
}
