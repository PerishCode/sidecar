use super::render::BrokerRuntimeStatus;
use serde_json::{Map, Value};
use sidecar_core::{
    discover_broker_endpoint, discover_brokers, discover_by_app_namespace, signal_terminate,
    BrokerIdentity, DataPaths, ExecutionPlan, TargetPlan,
};
use std::collections::BTreeMap;
use std::fs::{self, OpenOptions};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

#[derive(Clone, Debug)]
pub(super) struct ReadySummary {
    pub(super) role: String,
    pub(super) endpoint: Option<String>,
    pub(super) runtime_endpoint: Option<String>,
    pub(super) instance_id: Option<String>,
}

#[derive(Default)]
pub(super) struct RuntimeChain {
    ready: BTreeMap<String, ReadySummary>,
    published_env: BTreeMap<String, String>,
}

impl RuntimeChain {
    pub(super) fn from_state(paths: &DataPaths, plan: &ExecutionPlan) -> Result<Self, String> {
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

    pub(super) fn record(&mut self, target: &TargetPlan, ready: &ReadySummary) {
        if let (Some(env_name), Some(endpoint)) = (&target.endpoint_env, &ready.endpoint) {
            self.published_env
                .insert(env_name.clone(), endpoint.clone());
        }
        self.ready.insert(target.name.clone(), ready.clone());
    }

    pub(super) fn resolve_inherits(
        &self,
        target: &TargetPlan,
    ) -> Result<Vec<(String, String)>, String> {
        let mut env = Vec::new();
        for binding in &target.inherits_env {
            let Some((source, field)) = binding.from.split_once('.') else {
                return Err(format!(
                    "invalid inherits_env source {:?}; expected '<target>.<field>'",
                    binding.from
                ));
            };
            let value = inherited_value(self.ready.get(source), field)?;
            if let Some(value) = value {
                env.push((binding.name.clone(), value));
            }
        }
        Ok(env)
    }
}

fn inherited_value(ready: Option<&ReadySummary>, field: &str) -> Result<Option<String>, String> {
    let Some(ready) = ready else {
        return Ok(None);
    };
    match field {
        "endpoint" | "endpoint_env" => Ok(ready.endpoint.clone()),
        "runtime_endpoint" => Ok(ready.runtime_endpoint.clone()),
        "instance_id" => Ok(ready.instance_id.clone()),
        other => Err(format!(
            "invalid inherits_env field {other:?}; expected endpoint|runtime_endpoint|instance_id|endpoint_env"
        )),
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

pub(super) fn wait_ready_from_log(
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

fn target_state_path(paths: &DataPaths) -> PathBuf {
    paths.project.join("targets.json")
}

pub(super) fn load_target_state(paths: &DataPaths) -> Result<Map<String, Value>, String> {
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

pub(super) fn record_target_state(
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

pub(super) fn remove_target_state(paths: &DataPaths, name: &str) -> Result<(), String> {
    let mut state = load_target_state(paths)?;
    state.remove(name);
    save_target_state(paths, &state)
}

pub(super) fn running_pids_for_target(
    paths: &DataPaths,
    target: &TargetPlan,
) -> Result<Vec<u32>, String> {
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

pub(super) fn ensure_broker(plan: &ExecutionPlan) -> Result<String, String> {
    let identity = broker_identity(plan);
    if let Some(addr) = discover_broker_endpoint(&identity, Duration::from_millis(200))? {
        return Ok(format!("tcp://{addr}"));
    }

    let exe =
        std::env::current_exe().map_err(|err| format!("failed to resolve sidecar exe: {err}"))?;
    let log_path = std::env::temp_dir().join(format!(
        "sidecar-broker-{}-{}.log",
        sanitize_log_part(&identity.project),
        sanitize_log_part(&identity.namespace)
    ));
    let log = OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(true)
        .open(&log_path)
        .map_err(|err| format!("failed to open {}: {err}", log_path.display()))?;
    let stderr = log
        .try_clone()
        .map_err(|err| format!("failed to clone {}: {err}", log_path.display()))?;
    let mut command = Command::new(exe);
    command
        .args(["runtime", "serve", &identity.project, &identity.namespace])
        .args(identity.args())
        .stdin(Stdio::null())
        .stdout(Stdio::from(log))
        .stderr(Stdio::from(stderr));
    detach_process_group(&mut command);
    let child = command
        .spawn()
        .map_err(|err| format!("failed to spawn broker: {err}"))?;
    let pid = child.id();
    let deadline = Instant::now() + Duration::from_secs(5);
    while Instant::now() < deadline {
        if let Some(addr) = discover_broker_endpoint(&identity, Duration::from_millis(200))? {
            println!("broker runtime pid={pid} endpoint=tcp://{addr}");
            return Ok(format!("tcp://{addr}"));
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    Err(format!(
        "timed out waiting for broker runtime pid={pid}; see {}",
        log_path.display()
    ))
}

pub(super) fn broker_status(plan: &ExecutionPlan) -> Result<BrokerRuntimeStatus, String> {
    let identity = broker_identity(plan);
    let brokers = discover_brokers(&identity.project, &identity.namespace)?;
    let endpoint = discover_broker_endpoint(&identity, Duration::from_millis(200))?;
    Ok(BrokerRuntimeStatus {
        pids: brokers.into_iter().map(|broker| broker.pid).collect(),
        endpoint: endpoint.map(|addr| format!("tcp://{addr}")),
    })
}

pub(super) fn maybe_stop_broker(
    plan: &ExecutionPlan,
    paths: &DataPaths,
    sidecar: Option<&str>,
) -> Result<(), String> {
    if sidecar.is_none() || running_target_count(plan, paths)? == 0 {
        stop_broker(plan)?;
    }
    Ok(())
}

pub(super) fn stop_broker(plan: &ExecutionPlan) -> Result<(), String> {
    let identity = broker_identity(plan);
    let brokers = discover_brokers(&identity.project, &identity.namespace)?;
    for broker in brokers {
        signal_terminate(broker.pid)
            .map_err(|err| format!("failed to terminate broker pid {}: {err}", broker.pid))?;
        wait_for_exit(broker.pid)?;
        println!("stopped broker pid={}", broker.pid);
    }
    Ok(())
}

pub(super) fn wait_for_exit(pid: u32) -> Result<(), String> {
    if wait_until_exit(pid, Duration::from_millis(500)) {
        return Ok(());
    }
    force_kill(pid)?;
    if wait_until_exit(pid, Duration::from_secs(2)) {
        return Ok(());
    }
    Err(format!("timed out waiting for pid {pid} to exit"))
}

pub(super) fn detach_process_group(command: &mut Command) {
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

fn broker_identity(plan: &ExecutionPlan) -> BrokerIdentity {
    BrokerIdentity::new(&plan.project, &plan.namespace)
}

fn running_target_count(plan: &ExecutionPlan, paths: &DataPaths) -> Result<usize, String> {
    let mut count = 0;
    for target in &plan.targets {
        if !running_pids_for_target(paths, target)?.is_empty() {
            count += 1;
        }
    }
    Ok(count)
}

fn wait_until_exit(pid: u32, timeout: Duration) -> bool {
    let deadline = Instant::now() + timeout;
    while Instant::now() < deadline {
        if !process_exists(pid) {
            return true;
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    !process_exists(pid)
}

fn force_kill(pid: u32) -> Result<(), String> {
    #[cfg(unix)]
    {
        let process_group = format!("-{pid}");
        let group_status = Command::new("kill")
            .args(["-KILL", "--", &process_group])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .map_err(|err| format!("kill failed: {err}"))?;
        if group_status.success() {
            return Ok(());
        }
        if !process_exists(pid) {
            return Ok(());
        }

        let status = Command::new("kill")
            .args(["-KILL", "--", &pid.to_string()])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .map_err(|err| format!("kill failed: {err}"))?;
        if status.success() || !process_exists(pid) {
            Ok(())
        } else {
            Err(format!(
                "kill -KILL -{pid} exited with status {group_status}; kill -KILL {pid} exited with status {status}"
            ))
        }
    }

    #[cfg(not(unix))]
    {
        let _ = pid;
        Ok(())
    }
}

fn sanitize_log_part(value: &str) -> String {
    value
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_') {
                ch
            } else {
                '_'
            }
        })
        .collect()
}

fn process_exists(pid: u32) -> bool {
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

    #[cfg(not(unix))]
    {
        let _ = pid;
        false
    }
}
