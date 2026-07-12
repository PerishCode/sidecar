//! Cross-platform process discovery via stamp args matching.
//!
//! Unix uses `ps -axo pid=,command=`. Windows queries Win32_Process through
//! the platform PowerShell host so stamp discovery retains full argv data.

use crate::runtime::broker::read_broker_identity;
use crate::stamp::read_stamp;
#[cfg(any(unix, windows))]
use std::process::Command;
#[cfg(unix)]
use std::process::Stdio;

#[cfg(windows)]
use serde::Deserialize;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StampedProcess {
    pub pid: u32,
    pub command: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BrokerProcess {
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

pub fn discover_brokers(project: &str, namespace: &str) -> Result<Vec<BrokerProcess>, String> {
    Ok(filter_brokers(ps_command_lines()?, |args| {
        match_broker(args, project, namespace)
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

fn match_broker(args: &[String], project: &str, namespace: &str) -> bool {
    read_broker_identity(args)
        .map(|identity| identity.project == project && identity.namespace == namespace)
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

fn filter_brokers<F>(rows: Vec<(u32, String)>, predicate: F) -> Vec<BrokerProcess>
where
    F: Fn(&[String]) -> bool,
{
    rows.into_iter()
        .filter_map(|(pid, command)| {
            let args: Vec<String> = command.split_whitespace().map(String::from).collect();
            if predicate(&args) {
                Some(BrokerProcess { pid, command })
            } else {
                None
            }
        })
        .collect()
}

#[doc(hidden)]
pub fn filter_for_test(
    rows: Vec<(u32, String)>,
    app: &str,
    namespace: &str,
) -> Vec<StampedProcess> {
    filter_stamped(rows, |args| {
        match_app(args, app) && match_namespace(args, namespace)
    })
}

#[doc(hidden)]
pub fn filter_brokers_for_test(
    rows: Vec<(u32, String)>,
    project: &str,
    namespace: &str,
) -> Vec<BrokerProcess> {
    filter_brokers(rows, |args| match_broker(args, project, namespace))
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

#[cfg(windows)]
fn ps_command_lines() -> Result<Vec<(u32, String)>, String> {
    use std::os::windows::process::CommandExt;

    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    const QUERY: &str = concat!(
        "$ErrorActionPreference='Stop';",
        "[Console]::OutputEncoding=[Text.UTF8Encoding]::new($false);",
        "@(Get-CimInstance Win32_Process | Select-Object ProcessId,CommandLine)",
        "| ConvertTo-Json -Compress"
    );
    let output = Command::new("powershell.exe")
        .args([
            "-NoLogo",
            "-NoProfile",
            "-NonInteractive",
            "-Command",
            QUERY,
        ])
        .creation_flags(CREATE_NO_WINDOW)
        .output()
        .map_err(|err| format!("PowerShell process query failed: {err}"))?;
    if !output.status.success() {
        return Err(format!(
            "PowerShell process query exited with status {}: {}",
            output.status,
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }
    parse_windows_process_json(&String::from_utf8_lossy(&output.stdout))
}

#[cfg(not(any(unix, windows)))]
fn ps_command_lines() -> Result<Vec<(u32, String)>, String> {
    Err("process discovery is not implemented on this platform".to_string())
}

#[cfg(windows)]
#[derive(Deserialize)]
#[serde(rename_all = "PascalCase")]
struct WindowsProcess {
    process_id: u32,
    command_line: Option<String>,
}

#[cfg(windows)]
#[doc(hidden)]
pub fn parse_windows_process_json(text: &str) -> Result<Vec<(u32, String)>, String> {
    let rows: Vec<WindowsProcess> =
        serde_json::from_str(text.trim_start_matches('\u{feff}').trim())
            .map_err(|err| format!("PowerShell process query returned invalid JSON: {err}"))?;
    Ok(rows
        .into_iter()
        .filter_map(|row| row.command_line.map(|line| (row.process_id, line)))
        .collect())
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
        .args(["-TERM", "--", &process_group])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map_err(|err| format!("kill failed: {err}"))?;
    if group_status.success() {
        return Ok(());
    }

    let status = Command::new("kill")
        .args(["-TERM", "--", &pid.to_string()])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
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

#[cfg(windows)]
pub fn signal_terminate(pid: u32) -> Result<(), String> {
    use windows_sys::Win32::System::Console::{GenerateConsoleCtrlEvent, CTRL_BREAK_EVENT};

    if !process_exists(pid) {
        return Ok(());
    }
    if unsafe { GenerateConsoleCtrlEvent(CTRL_BREAK_EVENT, pid) } != 0 || !process_exists(pid) {
        Ok(())
    } else {
        Err(format!(
            "failed to send CTRL_BREAK_EVENT to process group {pid}"
        ))
    }
}

#[cfg(not(any(unix, windows)))]
pub fn signal_terminate(_pid: u32) -> Result<(), String> {
    Err("process termination is not implemented on this platform".to_string())
}

#[cfg(unix)]
pub fn process_exists(pid: u32) -> bool {
    Command::new("kill")
        .arg("-0")
        .arg(pid.to_string())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .is_ok_and(|status| status.success())
}

#[cfg(windows)]
pub fn process_exists(pid: u32) -> bool {
    use windows_sys::Win32::Foundation::{CloseHandle, STILL_ACTIVE};
    use windows_sys::Win32::System::Threading::{
        GetExitCodeProcess, OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION,
    };

    let handle = unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid) };
    if handle.is_null() {
        return false;
    }
    let mut exit_code = 0;
    let active = unsafe { GetExitCodeProcess(handle, &mut exit_code) } != 0
        && exit_code == STILL_ACTIVE as u32;
    unsafe {
        CloseHandle(handle);
    }
    active
}

#[cfg(not(any(unix, windows)))]
pub fn process_exists(_pid: u32) -> bool {
    false
}
