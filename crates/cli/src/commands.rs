//! CLI verb implementations: lifecycle (start/stop/restart/status/list/reset)
//! and inspect (line-JSON IPC over a sidecar's `inspect_socket`).

use crate::cli::OutputFormat;
use serde_json::{Map, Value};
use sidecar_core::{
    discover_by_app_namespace, discover_by_namespace, inspect_send, signal_terminate, DataPaths,
    DevState, ExecutionPlan, InspectRequest, InspectResponse, SocketEndpoint, StampedProcess,
    TargetPlan,
};
use std::collections::BTreeMap;
use std::fs::{self, OpenOptions};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

const SIDECAR_INSPECT_SOCKET_ENV: &str = "SIDECAR_INSPECT_SOCKET";
const STIM_SIDECAR_APP_ENV: &str = "STIM_SIDECAR_APP";
const STIM_SIDECAR_NAMESPACE_ENV: &str = "STIM_SIDECAR_NAMESPACE";
const STIM_SIDECAR_MODE_ENV: &str = "STIM_SIDECAR_MODE";
const STIM_SIDECAR_SOURCE_ENV: &str = "STIM_SIDECAR_SOURCE";

pub(crate) fn start(
    state: &DevState,
    paths: &DataPaths,
    sidecar: Option<&str>,
) -> Result<(), String> {
    let plan = state.execution_plan();
    let targets = select_targets(&plan, sidecar)?;
    let mut chain = RuntimeChain::from_state(paths, &plan)?;
    for target in targets {
        if let Some(running) = running_pids_for_target(paths, target)?.first() {
            return Err(format!(
                "sidecar `{}` is already running (pid {}); run `sidecar stop` first",
                target.name, running
            ));
        }
        let extra_env = chain.resolve_inherits(target)?;
        let (pid, ready, log_path) =
            spawn_detached(state.config_path.parent(), paths, target, &extra_env)?;
        record_target_state(paths, target, pid, ready.as_ref(), &log_path)?;
        if let Some(ready) = &ready {
            chain.record(target, ready);
        }
        println!("started {} pid={pid}", target.name);
    }
    Ok(())
}

pub(crate) fn stop(
    state: &DevState,
    paths: &DataPaths,
    sidecar: Option<&str>,
) -> Result<(), String> {
    let plan = state.execution_plan();
    let targets = select_targets(&plan, sidecar)?;
    let mut stopped_total = 0;
    for target in targets {
        let pids = running_pids_for_target(paths, target)?;
        if pids.is_empty() {
            println!("not running: {}", target.name);
            continue;
        }
        for pid in &pids {
            signal_terminate(*pid).map_err(|err| {
                format!(
                    "failed to terminate sidecar `{}` (pid {}): {err}",
                    target.name, pid
                )
            })?;
            println!("stopped {} pid={}", target.name, pid);
            stopped_total += 1;
        }
        remove_target_state(paths, &target.name)?;
    }
    if stopped_total == 0 && sidecar.is_none() {
        println!("no sidecars were running");
    }
    Ok(())
}

pub(crate) fn restart(
    state: &DevState,
    paths: &DataPaths,
    sidecar: Option<&str>,
) -> Result<(), String> {
    stop(state, paths, sidecar)?;
    start(state, paths, sidecar)
}

pub(crate) fn status(
    state: &DevState,
    paths: &DataPaths,
    format: OutputFormat,
) -> Result<(), String> {
    let plan = state.execution_plan();
    let mut rows = Vec::new();
    for target in &plan.targets {
        let pids = running_pids_for_target(paths, target)?;
        rows.push((target.name.clone(), pids));
    }
    print_status(&plan.namespace, &rows, format)
}

pub(crate) fn list(
    state: &DevState,
    paths: &DataPaths,
    format: OutputFormat,
) -> Result<(), String> {
    let plan = state.execution_plan();
    let hits = discover_by_namespace(&plan.namespace)
        .map_err(|err| format!("discovery failed for namespace `{}`: {err}", plan.namespace))?;
    print_list(&plan.namespace, &hits, &load_target_state(paths)?, format)
}

pub(crate) fn reset(state: &DevState, paths: &DataPaths, all: bool) -> Result<(), String> {
    let plan = state.execution_plan();
    for target in &plan.targets {
        for pid in running_pids_for_target(paths, target)? {
            signal_terminate(pid).map_err(|err| format!("failed to terminate pid {pid}: {err}"))?;
            println!("terminated pid={pid} target={}", target.name);
        }
    }
    let hits = discover_by_namespace(&plan.namespace)
        .map_err(|err| format!("discovery failed for namespace `{}`: {err}", plan.namespace))?;
    if hits.is_empty() {
        println!("namespace `{}` has no stamped processes", plan.namespace);
    } else {
        for hit in &hits {
            signal_terminate(hit.pid)
                .map_err(|err| format!("failed to terminate pid {}: {err}", hit.pid))?;
            println!("terminated pid={} cmd={}", hit.pid, hit.command);
        }
    }
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
    state: &DevState,
    sidecar: &str,
    event: &str,
    payload: Option<&str>,
    timeout: Duration,
    format: OutputFormat,
) -> Result<(), String> {
    let plan = state.execution_plan();
    let target = plan
        .targets
        .iter()
        .find(|item| item.name == sidecar)
        .ok_or_else(|| format!("unknown target `{sidecar}` in this manifest"))?;
    let socket = target.inspect_socket.as_deref().ok_or_else(|| {
        format!("target `{sidecar}` has no inspect_socket configured in this manifest")
    })?;
    let endpoint = SocketEndpoint::parse(socket).map_err(|err| err.to_string())?;
    let payload_value: Value = match payload {
        Some(text) if !text.is_empty() => serde_json::from_str(text).map_err(|err| {
            format!("payload is not valid JSON: {err}; quote the payload as a single argument")
        })?,
        _ => Value::Null,
    };
    let request = InspectRequest {
        event: event.to_string(),
        payload: payload_value,
    };
    let response = inspect_send(&endpoint, &request, Some(timeout))?;
    print_inspect_response(sidecar, event, &response, format)
}

fn select_targets<'plan>(
    plan: &'plan ExecutionPlan,
    sidecar: Option<&str>,
) -> Result<Vec<&'plan TargetPlan>, String> {
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
    paths: &DataPaths,
    target: &TargetPlan,
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
        .args(target.spawn_args())
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
    if let Some(socket) = &target.inspect_socket {
        command.env(SIDECAR_INSPECT_SOCKET_ENV, socket);
    }
    if target.stamp_via_env {
        command
            .env(STIM_SIDECAR_APP_ENV, &target.stamp.app)
            .env(STIM_SIDECAR_NAMESPACE_ENV, &target.stamp.namespace)
            .env(STIM_SIDECAR_MODE_ENV, &target.stamp.mode)
            .env(STIM_SIDECAR_SOURCE_ENV, &target.stamp.source);
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
            ready.timeout_secs,
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

fn detach_process_group(command: &mut Command) {
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        command.process_group(0);
    }

    #[cfg(not(unix))]
    {
        let _ = command;
    }
}

#[derive(Clone, Debug)]
struct ReadySummary {
    role: String,
    endpoint: Option<String>,
    runtime_endpoint: Option<String>,
    instance_id: Option<String>,
}

#[derive(Default)]
struct RuntimeChain {
    ready: BTreeMap<String, ReadySummary>,
    published_env: BTreeMap<String, String>,
}

impl RuntimeChain {
    fn from_state(paths: &DataPaths, plan: &ExecutionPlan) -> Result<Self, String> {
        let state = load_target_state(paths)?;
        let mut chain = Self::default();
        for target in &plan.targets {
            if running_pids_for_target(paths, target)?.is_empty() {
                continue;
            }
            let Some(entry) = state.get(&target.name) else {
                continue;
            };
            let Some(ready) = ready_summary_from_state(entry) else {
                continue;
            };
            chain.ready.insert(target.name.clone(), ready);
        }
        Ok(chain)
    }

    fn record(&mut self, target: &TargetPlan, ready: &ReadySummary) {
        if let (Some(env_name), Some(endpoint)) = (&target.endpoint_env, &ready.endpoint) {
            self.published_env
                .insert(env_name.clone(), endpoint.clone());
        }
        self.ready.insert(target.name.clone(), ready.clone());
    }

    fn resolve_inherits(&self, target: &TargetPlan) -> Result<Vec<(String, String)>, String> {
        let mut env = Vec::new();
        for binding in &target.inherits_env {
            let Some((source, field)) = binding.from.split_once('.') else {
                return Err(format!(
                    "invalid inherits_env source {:?}; expected '<target>.<field>'",
                    binding.from
                ));
            };
            let value = match field {
                "endpoint" => self
                    .ready
                    .get(source)
                    .and_then(|ready| ready.endpoint.clone()),
                "runtime_endpoint" => self
                    .ready
                    .get(source)
                    .and_then(|ready| ready.runtime_endpoint.clone()),
                "instance_id" => self
                    .ready
                    .get(source)
                    .and_then(|ready| ready.instance_id.clone()),
                "endpoint_env" => self
                    .ready
                    .get(source)
                    .and_then(|ready| ready.endpoint.clone()),
                other => {
                    return Err(format!(
                        "invalid inherits_env field {other:?}; expected endpoint|runtime_endpoint|instance_id|endpoint_env"
                    ));
                }
            };
            if let Some(value) = value {
                env.push((binding.name.clone(), value));
            }
        }
        Ok(env)
    }
}

fn ready_summary_from_state(entry: &Value) -> Option<ReadySummary> {
    let ready = entry.get("ready")?;
    Some(ReadySummary {
        role: ready
            .get("role")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string(),
        endpoint: ready
            .get("endpoint")
            .and_then(Value::as_str)
            .map(str::to_string),
        runtime_endpoint: ready
            .get("runtimeEndpoint")
            .and_then(Value::as_str)
            .map(str::to_string),
        instance_id: ready
            .get("instanceId")
            .and_then(Value::as_str)
            .map(str::to_string),
    })
}

fn wait_ready_from_log(
    child: &mut Child,
    log_path: &Path,
    role: &str,
    timeout_secs: u64,
) -> Result<ReadySummary, String> {
    let deadline = Instant::now() + Duration::from_secs(timeout_secs);
    while Instant::now() < deadline {
        if let Some(status) = child
            .try_wait()
            .map_err(|err| format!("failed to check child status: {err}"))?
        {
            return Err(format!(
                "target exited before ready with status {status}; see {}",
                log_path.display()
            ));
        }
        if let Some(ready) = read_ready_from_log(log_path, role)? {
            return Ok(ready);
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    Err(format!(
        "timed out waiting for ready role {role:?}; see {}",
        log_path.display()
    ))
}

fn read_ready_from_log(
    log_path: &Path,
    expected_role: &str,
) -> Result<Option<ReadySummary>, String> {
    let Ok(content) = fs::read_to_string(log_path) else {
        return Ok(None);
    };
    for line in content.lines().rev() {
        let Ok(value) = serde_json::from_str::<Value>(line.trim()) else {
            continue;
        };
        let role = value
            .get("role")
            .and_then(Value::as_str)
            .unwrap_or_default();
        if role != expected_role {
            continue;
        }
        return Ok(Some(ReadySummary {
            role: role.to_string(),
            endpoint: value
                .get("endpoint")
                .and_then(Value::as_str)
                .map(str::to_string),
            runtime_endpoint: value
                .get("runtime_endpoint")
                .and_then(Value::as_str)
                .map(str::to_string),
            instance_id: value
                .get("instance_id")
                .and_then(Value::as_str)
                .map(str::to_string),
        }));
    }
    Ok(None)
}

fn log_path(paths: &DataPaths, name: &str) -> PathBuf {
    paths.project.join("logs").join(format!("{name}.log"))
}

fn target_state_path(paths: &DataPaths) -> PathBuf {
    paths.project.join("targets.json")
}

fn load_target_state(paths: &DataPaths) -> Result<Map<String, Value>, String> {
    let path = target_state_path(paths);
    let Ok(text) = fs::read_to_string(&path) else {
        return Ok(Map::new());
    };
    let value: Value = serde_json::from_str(&text)
        .map_err(|err| format!("failed to parse {}: {err}", path.display()))?;
    Ok(value.as_object().cloned().unwrap_or_default())
}

fn save_target_state(paths: &DataPaths, state: &Map<String, Value>) -> Result<(), String> {
    fs::create_dir_all(&paths.project)
        .map_err(|err| format!("failed to create {}: {err}", paths.project.display()))?;
    let path = target_state_path(paths);
    let text = serde_json::to_string_pretty(state).map_err(|err| err.to_string())?;
    fs::write(&path, text).map_err(|err| format!("failed to write {}: {err}", path.display()))
}

fn record_target_state(
    paths: &DataPaths,
    target: &TargetPlan,
    pid: u32,
    ready: Option<&ReadySummary>,
    log_path: &Path,
) -> Result<(), String> {
    let mut state = load_target_state(paths)?;
    state.insert(
        target.name.clone(),
        serde_json::json!({
            "pid": pid,
            "app": target.stamp.app,
            "namespace": target.stamp.namespace,
            "mode": target.stamp.mode,
            "source": target.stamp.source,
            "inspectSocket": target.inspect_socket,
            "logPath": log_path.display().to_string(),
            "ready": ready.map(|ready| serde_json::json!({
                "role": ready.role,
                "endpoint": ready.endpoint,
                "runtimeEndpoint": ready.runtime_endpoint,
                "instanceId": ready.instance_id,
            })),
        }),
    );
    save_target_state(paths, &state)
}

fn remove_target_state(paths: &DataPaths, name: &str) -> Result<(), String> {
    let mut state = load_target_state(paths)?;
    state.remove(name);
    save_target_state(paths, &state)
}

fn running_pids_for_target(paths: &DataPaths, target: &TargetPlan) -> Result<Vec<u32>, String> {
    let mut pids = Vec::new();
    if let Some(pid) = load_target_state(paths)?
        .get(&target.name)
        .and_then(|entry| entry.get("pid"))
        .and_then(Value::as_u64)
        .and_then(|pid| u32::try_from(pid).ok())
        .filter(|pid| process_exists(*pid))
    {
        pids.push(pid);
    }
    let hits = discover_by_app_namespace(&target.stamp.app, &target.stamp.namespace)
        .map_err(|err| format!("discovery failed for `{}`: {err}", target.name))?;
    for hit in hits {
        if !pids.contains(&hit.pid) {
            pids.push(hit.pid);
        }
    }
    Ok(pids)
}

fn process_exists(pid: u32) -> bool {
    #[cfg(unix)]
    {
        Command::new("kill")
            .arg("-0")
            .arg(pid.to_string())
            .status()
            .is_ok_and(|status| status.success())
    }

    #[cfg(not(unix))]
    {
        let _ = pid;
        false
    }
}

fn print_status(
    namespace: &str,
    rows: &[(String, Vec<u32>)],
    format: OutputFormat,
) -> Result<(), String> {
    match format {
        OutputFormat::Text => {
            println!("namespace: {namespace}");
            for (name, pids) in rows {
                if let Some(first) = pids.first() {
                    println!("- {name}: running (pid {})", first);
                    for extra in pids.iter().skip(1) {
                        println!("  + duplicate (pid {})", extra);
                    }
                } else {
                    println!("- {name}: stopped");
                }
            }
            Ok(())
        }
        OutputFormat::Json => {
            let value = serde_json::json!({
                "namespace": namespace,
                "targets": rows.iter().map(|(name, pids)| serde_json::json!({
                    "name": name,
                    "running": !pids.is_empty(),
                    "pids": pids,
                })).collect::<Vec<_>>(),
            });
            println!(
                "{}",
                serde_json::to_string_pretty(&value).map_err(|err| err.to_string())?
            );
            Ok(())
        }
    }
}

fn print_list(
    namespace: &str,
    hits: &[StampedProcess],
    state: &Map<String, Value>,
    format: OutputFormat,
) -> Result<(), String> {
    match format {
        OutputFormat::Text => {
            println!("namespace: {namespace}");
            if hits.is_empty() {
                println!("no stamped processes");
            }
            for hit in hits {
                println!("- pid={} cmd={}", hit.pid, hit.command);
            }
            for (name, entry) in state {
                if let Some(pid) = entry.get("pid").and_then(Value::as_u64) {
                    println!("- target={name} pid={pid} source=state");
                }
            }
            Ok(())
        }
        OutputFormat::Json => {
            let value = serde_json::json!({
                "namespace": namespace,
                "processes": hits.iter().map(|hit| serde_json::json!({
                    "pid": hit.pid,
                    "command": hit.command,
                })).collect::<Vec<_>>(),
                "targets": state,
            });
            println!(
                "{}",
                serde_json::to_string_pretty(&value).map_err(|err| err.to_string())?
            );
            Ok(())
        }
    }
}

fn print_inspect_response(
    sidecar: &str,
    event: &str,
    response: &InspectResponse,
    format: OutputFormat,
) -> Result<(), String> {
    match format {
        OutputFormat::Text => match response {
            InspectResponse::Ok(value) => {
                println!("ok {sidecar} {event}");
                println!(
                    "{}",
                    serde_json::to_string_pretty(value).unwrap_or_default()
                );
                Ok(())
            }
            InspectResponse::Err(message) => Err(format!("inspect error: {message}")),
        },
        OutputFormat::Json => {
            let body = match response {
                InspectResponse::Ok(value) => serde_json::json!({
                    "sidecar": sidecar,
                    "event": event,
                    "ok": true,
                    "data": value,
                }),
                InspectResponse::Err(message) => serde_json::json!({
                    "sidecar": sidecar,
                    "event": event,
                    "ok": false,
                    "error": message,
                }),
            };
            println!(
                "{}",
                serde_json::to_string_pretty(&body).map_err(|err| err.to_string())?
            );
            if matches!(response, InspectResponse::Err(_)) {
                return Err("inspect endpoint returned ok=false".to_string());
            }
            Ok(())
        }
    }
}
