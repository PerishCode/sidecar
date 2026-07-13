mod render;
mod runtime;

use crate::cli::Format;
use runtime::{Chain, Ready};
use serde_json::Value;
use sidecar_core::plan::{Plan, Target};
use sidecar_core::{inspect, process, socket, Paths, State};
use std::fs::{self, OpenOptions};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::Duration;

const SOCKET: &str = "SIDECAR_INSPECT_SOCKET";

pub(crate) fn start(state: &State, paths: &Paths, sidecar: Option<&str>) -> Result<(), String> {
    let plan = state.plan();
    let targets = pick(&plan, sidecar)?;
    let endpoint = runtime::ensure(&plan)?;
    let mut chain = Chain::load(paths, &plan)?;
    for target in targets {
        if let Some(running) = runtime::running(paths, target)?.first() {
            return Err(format!(
                "sidecar `{}` is already running (pid {}); run `sidecar stop` first",
                target.name, running
            ));
        }
        let env = chain.inherits(target)?;
        let (pid, ready, path) = spawn(state.path.parent(), paths, target, &endpoint, &env)?;
        runtime::state::record(paths, target, pid, ready.as_ref(), &path)?;
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
    let targets = pick(&plan, sidecar)?;
    let mut stopped = 0;
    for target in targets {
        let pids = runtime::running(paths, target)?;
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
            runtime::reap(*pid, force)?;
            println!("stopped {} pid={}", target.name, pid);
            stopped += 1;
        }
        runtime::state::remove(paths, &target.name)?;
    }
    if stopped == 0 && sidecar.is_none() {
        println!("no sidecars were running");
    }
    runtime::sweep(&plan, paths, sidecar, force)?;
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

pub(crate) fn status(state: &State, paths: &Paths, format: Format) -> Result<(), String> {
    let plan = state.plan();
    let mut rows = Vec::new();
    for target in &plan.targets {
        let pids = runtime::running(paths, target)?;
        rows.push((target.name.clone(), pids));
    }
    let broker = runtime::status(&plan)?;
    render::status(&plan.namespace, &rows, &broker, format)
}

pub(crate) fn list(state: &State, paths: &Paths, format: Format) -> Result<(), String> {
    let plan = state.plan();
    let hits = process::Stamped::discover(None, &plan.namespace)
        .map_err(|err| format!("discovery failed for namespace `{}`: {err}", plan.namespace))?;
    let broker = runtime::status(&plan)?;
    render::list(
        &plan.namespace,
        &hits,
        &broker,
        &runtime::state::load(paths)?,
        format,
    )
}

pub(crate) fn reset(state: &State, paths: &Paths, all: bool, force: bool) -> Result<(), String> {
    let plan = state.plan();
    for target in &plan.targets {
        for pid in runtime::running(paths, target)? {
            process::terminate(pid)
                .map_err(|err| format!("failed to terminate pid {pid}: {err}"))?;
            runtime::reap(pid, force)?;
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
            runtime::reap(hit.pid, force)?;
            println!("terminated pid={} cmd={}", hit.pid, hit.command);
        }
    }
    runtime::halt(&plan, force)?;
    purge(&paths.project, "project data")?;
    if all {
        purge(&paths.state, "global state")?;
    }
    Ok(())
}

fn purge(path: &Path, label: &str) -> Result<(), String> {
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
    format: Format,
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
    let body: Value = match payload {
        Some(text) if !text.is_empty() => serde_json::from_str(text).map_err(|err| {
            format!("payload is not valid JSON: {err}; quote the payload as a single argument")
        })?,
        _ => serde_json::json!({}),
    };
    let request = inspect::Request {
        event: event.to_string(),
        payload: body,
    };
    let response = inspect::send(&endpoint, &request, Some(timeout))?;
    render::inspect(sidecar, event, &response, format)
}

fn pick<'plan>(plan: &'plan Plan, sidecar: Option<&str>) -> Result<Vec<&'plan Target>, String> {
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

fn spawn(
    config: Option<&Path>,
    paths: &Paths,
    target: &Target,
    endpoint: &str,
    env: &[(String, String)],
) -> Result<(u32, Option<Ready>, PathBuf), String> {
    let cwd = cwd(config, &target.cwd);
    let path = log(paths, &target.name);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|err| format!("failed to create {}: {err}", parent.display()))?;
    }
    let file = OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(true)
        .open(&path)
        .map_err(|err| format!("failed to open {}: {err}", path.display()))?;
    let stderr = file
        .try_clone()
        .map_err(|err| format!("failed to clone {}: {err}", path.display()))?;
    let mut command = Command::new(&target.command);
    command
        .args(target.launch(endpoint))
        .current_dir(&cwd)
        .stdin(Stdio::null())
        .stdout(Stdio::from(file))
        .stderr(Stdio::from(stderr));
    for (key, value) in &target.env {
        command.env(key, value);
    }
    for (key, value) in env {
        command.env(key, value);
    }
    if let Some(socket) = &target.socket {
        command.env(SOCKET, socket);
    }
    runtime::detach(&mut command);
    let mut child = command
        .spawn()
        .map_err(|err| format!("failed to spawn `{}`: {err}", target.command))?;
    let pid = child.id();
    let ready = match &target.ready {
        Some(ready) => Some(runtime::watch(
            &mut child,
            &path,
            &ready.role,
            ready.timeout,
        )?),
        None => None,
    };
    Ok((pid, ready, path))
}

fn cwd(config: Option<&Path>, cwd: &str) -> std::path::PathBuf {
    let path = std::path::Path::new(cwd);
    if path.is_absolute() {
        return path.to_path_buf();
    }
    match config {
        Some(dir) => dir.join(path),
        None => path.to_path_buf(),
    }
}

fn log(paths: &Paths, name: &str) -> PathBuf {
    paths.project.join("logs").join(format!("{name}.log"))
}
