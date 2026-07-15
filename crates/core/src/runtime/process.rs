use crate::runtime::broker;
use crate::stamp;
#[cfg(any(unix, windows))]
use std::process::Command;
#[cfg(unix)]
use std::process::Stdio;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Stamped {
    pub pid: u32,
    pub command: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Broker {
    pub pid: u32,
    pub command: String,
}

impl Stamped {
    pub fn discover(app: Option<&str>, namespace: &str) -> Result<Vec<Stamped>, String> {
        Ok(Self::filter(snapshot()?, app, namespace))
    }

    #[doc(hidden)]
    pub fn filter(rows: Vec<(u32, String)>, app: Option<&str>, namespace: &str) -> Vec<Stamped> {
        filter(rows, |args| {
            stamp::find(args).is_some_and(|stamp| {
                app.is_none_or(|name| stamp.app == name) && stamp.namespace == namespace
            })
        })
        .into_iter()
        .map(|(pid, command)| Stamped { pid, command })
        .collect()
    }
}

impl Broker {
    pub fn discover(project: &str, namespace: &str) -> Result<Vec<Broker>, String> {
        Ok(Self::filter(snapshot()?, project, namespace))
    }

    #[doc(hidden)]
    pub fn filter(rows: Vec<(u32, String)>, project: &str, namespace: &str) -> Vec<Broker> {
        filter(rows, |args| {
            broker::find(args).is_some_and(|identity| {
                identity.project == project && identity.namespace == namespace
            })
        })
        .into_iter()
        .map(|(pid, command)| Broker { pid, command })
        .collect()
    }
}

fn filter<F>(rows: Vec<(u32, String)>, predicate: F) -> Vec<(u32, String)>
where
    F: Fn(&[String]) -> bool,
{
    rows.into_iter()
        .filter(|(_, command)| {
            let args: Vec<String> = command.split_whitespace().map(String::from).collect();
            predicate(&args)
        })
        .collect()
}

#[cfg(unix)]
fn snapshot() -> Result<Vec<(u32, String)>, String> {
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
    Ok(parse(&stdout))
}

#[cfg(windows)]
fn snapshot() -> Result<Vec<(u32, String)>, String> {
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
    windows::parse(&String::from_utf8_lossy(&output.stdout))
}

#[cfg(not(any(unix, windows)))]
fn snapshot() -> Result<Vec<(u32, String)>, String> {
    Err("process discovery is not implemented on this platform".to_string())
}

#[cfg(windows)]
pub mod windows {
    use serde::Deserialize;

    #[derive(Deserialize)]
    struct Row {
        #[serde(rename = "ProcessId")]
        pid: u32,
        #[serde(rename = "CommandLine")]
        command: Option<String>,
    }

    #[doc(hidden)]
    pub fn parse(text: &str) -> Result<Vec<(u32, String)>, String> {
        let rows: Vec<Row> = serde_json::from_str(text.trim_start_matches('\u{feff}').trim())
            .map_err(|err| format!("PowerShell process query returned invalid JSON: {err}"))?;
        Ok(rows
            .into_iter()
            .filter_map(|row| row.command.map(|line| (row.pid, line)))
            .collect())
    }
}

pub fn parse(text: &str) -> Vec<(u32, String)> {
    text.lines()
        .filter_map(|line| {
            let trimmed = line.trim_start();
            let mut parts = trimmed.splitn(2, char::is_whitespace);
            let pid: u32 = parts.next()?.trim().parse().ok()?;
            let command = parts.next()?.trim().to_string();
            Some((pid, command))
        })
        .collect()
}

pub fn stop(pid: u32) -> Result<(), String> {
    #[cfg(unix)]
    {
        if !exists(pid) {
            return Ok(());
        }
        let group = Command::new("kill")
            .args(["-TERM", "--", &format!("-{pid}")])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .map_err(|err| format!("kill failed: {err}"))?;
        if group.success() {
            return Ok(());
        }

        let status = Command::new("kill")
            .args(["-TERM", "--", &pid.to_string()])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .map_err(|err| format!("kill failed: {err}"))?;
        if status.success() || !exists(pid) {
            Ok(())
        } else {
            Err(format!(
                "kill -TERM -{pid} exited with status {group}; kill -TERM {pid} exited with status {status}"
            ))
        }
    }

    #[cfg(windows)]
    {
        use windows_sys::Win32::System::Console::{GenerateConsoleCtrlEvent, CTRL_BREAK_EVENT};

        if !exists(pid) {
            return Ok(());
        }
        if unsafe { GenerateConsoleCtrlEvent(CTRL_BREAK_EVENT, pid) } != 0 || !exists(pid) {
            Ok(())
        } else {
            Err(format!(
                "failed to send CTRL_BREAK_EVENT to process group {pid}"
            ))
        }
    }

    #[cfg(not(any(unix, windows)))]
    {
        let _ = pid;
        Err("process termination is not implemented on this platform".to_string())
    }
}

pub fn exists(pid: u32) -> bool {
    #[cfg(unix)]
    {
        Command::new("kill")
            .arg("-0")
            .arg(pid.to_string())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .is_ok_and(|status| status.success())
    }

    #[cfg(windows)]
    {
        use windows_sys::Win32::Foundation::{CloseHandle, STILL_ACTIVE};
        use windows_sys::Win32::System::Threading::{
            GetExitCodeProcess, OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION,
        };

        let handle = unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid) };
        if handle.is_null() {
            return false;
        }
        let mut code = 0;
        let active =
            unsafe { GetExitCodeProcess(handle, &mut code) } != 0 && code == STILL_ACTIVE as u32;
        unsafe {
            CloseHandle(handle);
        }
        active
    }

    #[cfg(not(any(unix, windows)))]
    {
        let _ = pid;
        false
    }
}
