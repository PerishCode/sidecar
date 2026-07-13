mod render;
mod runtime;

use crate::cli::OutputFormat;
use render::{print_inspect_response, print_list, print_status};
use runtime::{
    broker_status, detach_process_group, ensure_broker, load_target_state, maybe_stop_broker,
    record_target_state, remove_target_state, running_pids_for_target, stop_broker, wait_for_exit,
    wait_ready_from_log, ReadySummary, RuntimeChain,
};
use serde_json::Value;
use sidecar_core::plan::{Plan, Target};
use sidecar_core::{inspect, process, socket, Paths, State};
use std::fs::{self, OpenOptions};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::Duration;

const SIDECAR_INSPECT_SOCKET_ENV: &str = "SIDECAR_INSPECT_SOCKET";

pub(crate) fn start(state: &State, paths: &Paths, sidecar: Option<&str>) -> Result<(), String> {
    let plan = state.plan();
    let targets = select_targets(&plan, sidecar)?;
    let runtime_endpoint = ensure_broker(&plan)?;
    let mut chain = RuntimeChain::from_state(paths, &plan)?;
    for target in targets {
        if let Some(running) = running_pids_for_target(paths, target)?.first() {
            return Err(format!(
                "sidecar `{}` is already running (pid {}); run `sidecar stop` first",
                target.name, running
            ));
        }
        let extra_env = chain.resolve_inherits(target)?;
        let (pid, ready, log_path) = spawn_detached(
            state.path.parent(),
            paths,
            target,
            &runtime_endpoint,
            &extra_env,
        )?;
        record_target_state(paths, target, pid, ready.as_ref(), &log_path)?;
        if let Some(ready) = &ready {
            chain.record(&target.name, ready);
        }
        println!("started {} pid={pid}", target.name);
    }
    Ok(())
}

pub(crate) fn stop(
    state: &State,
    paths: &Paths,
    sidecar: Option<&str>,
    force: bool,
) -> Result<(), String> {
    let plan = state.plan();
    let targets = select_targets(&plan, sidecar)?;
    let mut stopped_total = 0;
    for target in targets {
        let pids = running_pids_for_target(paths, target)?;
        if pids.is_empty() {
            println!("not running: {}", target.name);
            continue;
        }
        for pid in &pids {
            process::terminate(*pid).map_err(|err| {
                format!(
                    "failed to terminate sidecar `{}` (pid {}): {err}",
                    target.name, pid
                )
            })?;
            wait_for_exit(*pid, force)?;
            println!("stopped {} pid={}", target.name, pid);
            stopped_total += 1;
        }
        remove_target_state(paths, &target.name)?;
    }
    if stopped_total == 0 && sidecar.is_none() {
        println!("no sidecars were running");
    }
    maybe_stop_broker(&plan, paths, sidecar, force)?;
    Ok(())
}

pub(crate) fn restart(
    state: &State,
    paths: &Paths,
    sidecar: Option<&str>,
    force: bool,
) -> Result<(), String> {
    stop(state, paths, sidecar, force)?;
    start(state, paths, sidecar)
}

pub(crate) fn status(state: &State, paths: &Paths, format: OutputFormat) -> Result<(), String> {
    let plan = state.plan();
    let mut rows = Vec::new();
    for target in &plan.targets {
        let pids = running_pids_for_target(paths, target)?;
        rows.push((target.name.clone(), pids));
    }
    let broker = broker_status(&plan)?;
    print_status(&plan.namespace, &rows, &broker, format)
}

pub(crate) fn list(state: &State, paths: &Paths, format: OutputFormat) -> Result<(), String> {
    let plan = state.plan();
    let hits = process::Stamped::discover(None, &plan.namespace)
        .map_err(|err| format!("discovery failed for namespace `{}`: {err}", plan.namespace))?;
    let broker = broker_status(&plan)?;
    print_list(
        &plan.namespace,
        &hits,
        &broker,
        &load_target_state(paths)?,
        format,
    )
}

pub(crate) fn reset(state: &State, paths: &Paths, all: bool, force: bool) -> Result<(), String> {
    let plan = state.plan();
    for target in &plan.targets {
        for pid in running_pids_for_target(paths, target)? {
            process::terminate(pid)
                .map_err(|err| format!("failed to terminate pid {pid}: {err}"))?;
            wait_for_exit(pid, force)?;
            println!("terminated pid={pid} target={}", target.name);
        }
    }
    let hits = process::Stamped::discover(None, &plan.namespace)
        .map_err(|err| format!("discovery failed for namespace `{}`: {err}", plan.namespace))?;
    if hits.is_empty() {
        println!("namespace `{}` has no stamped processes", plan.namespace);
    } else {
        for hit in &hits {
            process::terminate(hit.pid)
                .map_err(|err| format!("failed to terminate pid {}: {err}", hit.pid))?;
            wait_for_exit(hit.pid, force)?;
            println!("terminated pid={} cmd={}", hit.pid, hit.command);
        }
    }
    stop_broker(&plan, force)?;
    remove_dir_if_exists(&paths.project, "project data")?;
    if all {
        remove_dir_if_exists(&paths.state, "global state")?;
    }
    Ok(())
}

fn remove_dir_if_exists(path: &Path, label: &str) -> Result<(), String> {
    match fs::metadata(path) {
        Ok(meta) if meta.is_dir() => {
            fs::remove_dir_all(path)
                .map_err(|err| format!("failed to remove {label} dir {}: {err}", path.display()))?;
            println!("removed {label} dir {}", path.display());
            Ok(())
        }
        Ok(_) => Err(format!(
            "{label} path exists but is not a directory: {}",
            path.display()
        )),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(err) => Err(format!(
            "failed to inspect {label} dir {}: {err}",
            path.display()
        )),
    }
}

pub(crate) fn inspect(
    state: &State,
    sidecar: &str,
    event: &str,
    payload: Option<&str>,
    timeout: Duration,
    format: OutputFormat,
) -> Result<(), String> {
    let plan = state.plan();
    let target = plan
        .targets
        .iter()
        .find(|item| item.name == sidecar)
        .ok_or_else(|| format!("unknown target `{sidecar}` in this manifest"))?;
    let socket = target.socket.as_deref().ok_or_else(|| {
        format!("target `{sidecar}` has no inspect_socket configured in this manifest")
    })?;
    let endpoint = socket::Endpoint::parse(socket).map_err(|err| err.to_string())?;
    let payload_value: Value = match payload {
        Some(text) if !text.is_empty() => serde_json::from_str(text).map_err(|err| {
            format!("payload is not valid JSON: {err}; quote the payload as a single argument")
        })?,
        _ => serde_json::json!({}),
    };
    let request = inspect::Request {
        event: event.to_string(),
        payload: payload_value,
    };
    let response = inspect::send(&endpoint, &request, Some(timeout))?;
    print_inspect_response(sidecar, event, &response, format)
}

fn select_targets<'plan>(
    plan: &'plan Plan,
    sidecar: Option<&str>,
) -> Result<Vec<&'plan Target>, String> {
    if let Some(name) = sidecar {
        let hit = plan
            .targets
            .iter()
            .find(|item| item.name == name)
            .ok_or_else(|| format!("unknown target `{name}` in this manifest"))?;
        Ok(vec![hit])
    } else {
        if plan.targets.is_empty() {
            return Err("manifest declares no lifecycle targets".to_string());
        }
        Ok(plan.targets.iter().collect())
    }
}

fn spawn_detached(
    config_dir: Option<&Path>,
    paths: &Paths,
    target: &Target,
    runtime_endpoint: &str,
    extra_env: &[(String, String)],
) -> Result<(u32, Option<ReadySummary>, PathBuf), String> {
    let cwd = resolve_cwd(config_dir, &target.cwd);
    let log_path = log_path(paths, &target.name);
    if let Some(parent) = log_path.parent() {
        fs::create_dir_all(parent)
            .map_err(|err| format!("failed to create {}: {err}", parent.display()))?;
    }
    let log = OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(true)
        .open(&log_path)
        .map_err(|err| format!("failed to open {}: {err}", log_path.display()))?;
    let stderr = log
        .try_clone()
        .map_err(|err| format!("failed to clone {}: {err}", log_path.display()))?;
    let mut command = Command::new(&target.command);
    command
        .args(target.launch(runtime_endpoint))
        .current_dir(&cwd)
        .stdin(Stdio::null())
        .stdout(Stdio::from(log))
        .stderr(Stdio::from(stderr));
    for (key, value) in &target.env {
        command.env(key, value);
    }
    for (key, value) in extra_env {
        command.env(key, value);
    }
    if let Some(socket) = &target.socket {
        command.env(SIDECAR_INSPECT_SOCKET_ENV, socket);
    }
    detach_process_group(&mut command);
    let mut child = command
        .spawn()
        .map_err(|err| format!("failed to spawn `{}`: {err}", target.command))?;
    let pid = child.id();
    let ready = match &target.ready {
        Some(ready) => Some(wait_ready_from_log(
            &mut child,
            &log_path,
            &ready.role,
            ready.timeout,
        )?),
        None => None,
    };
    Ok((pid, ready, log_path))
}

fn resolve_cwd(config_dir: Option<&Path>, cwd: &str) -> std::path::PathBuf {
    let path = std::path::Path::new(cwd);
    if path.is_absolute() {
        return path.to_path_buf();
    }
    match config_dir {
        Some(dir) => dir.join(path),
        None => path.to_path_buf(),
    }
}

fn log_path(paths: &Paths, name: &str) -> PathBuf {
    paths.project.join("logs").join(format!("{name}.log"))
}
