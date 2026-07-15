mod render;
mod runtime;

use crate::cli::Format;
use runtime::{Broker, Chain, Launch};
use serde_json::{Map, Value};
use sidecar_core::plan::{Plan, Target};
use sidecar_core::{inspect, process, socket, Paths, State};
use std::fs::{self, OpenOptions};
use std::net::TcpListener;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::Duration;

const SOCKET: &str = "SIDECAR_INSPECT_SOCKET";
const PORT: &str = "SIDECAR_PORT";

pub(crate) struct Session {
    pub(crate) state: State,
    pub(crate) paths: Paths,
}

pub(crate) struct Probe<'a> {
    pub(crate) sidecar: &'a str,
    pub(crate) event: &'a str,
    pub(crate) payload: Option<&'a str>,
    pub(crate) timeout: Duration,
}

impl Session {
    pub(crate) fn start(&self, sidecar: Option<&str>) -> Result<(), String> {
        let plan = self.state.plan();
        let targets = pick(&plan, sidecar)?;
        let endpoint = Broker::new(&plan).ensure()?;
        let mut chain = Chain::load(&self.paths, &plan)?;
        for target in targets {
            if let Some(running) = runtime::running(&self.paths, target)?.first() {
                return Err(format!(
                    "sidecar `{}` is already running (pid {}); run `sidecar stop` first",
                    target.name, running
                ));
            }
            let env = chain.inherits(target)?;
            let launch = self.spawn(target, &endpoint, &env)?;
            runtime::state::record(&self.paths, target, &launch)?;
            if let Some(ready) = &launch.ready {
                chain.record(&target.name, ready);
            }
            println!("started {} pid={}", target.name, launch.pid);
        }
        Ok(())
    }

    pub(crate) fn stop(&self, sidecar: Option<&str>, force: bool) -> Result<(), String> {
        let plan = self.state.plan();
        let targets = pick(&plan, sidecar)?;
        let mut stopped = 0;
        for target in targets {
            let pids = runtime::running(&self.paths, target)?;
            if pids.is_empty() {
                println!("not running: {}", target.name);
                continue;
            }
            for pid in &pids {
                process::stop(*pid).map_err(|err| {
                    format!(
                        "failed to terminate sidecar `{}` (pid {}): {err}",
                        target.name, pid
                    )
                })?;
                runtime::reap(*pid, force)?;
                println!("stopped {} pid={}", target.name, pid);
                stopped += 1;
            }
            runtime::state::remove(&self.paths, &target.name)?;
        }
        if stopped == 0 && sidecar.is_none() {
            println!("no sidecars were running");
        }
        let broker = Broker::new(&plan);
        if sidecar.is_none() || broker.idle(&self.paths)? {
            broker.stop(force)?;
        }
        Ok(())
    }

    pub(crate) fn restart(&self, sidecar: Option<&str>, force: bool) -> Result<(), String> {
        self.stop(sidecar, force)?;
        self.start(sidecar)
    }

    pub(crate) fn status(&self, format: Format) -> Result<(), String> {
        let plan = self.state.plan();
        let state = runtime::state::load(&self.paths)?;
        let mut rows = Vec::new();
        for target in &plan.targets {
            let pids = runtime::running(&self.paths, target)?;
            rows.push(render::Row {
                name: target.name.clone(),
                pids,
                health: health(target, &state),
            });
        }
        let broker = Broker::new(&plan).status()?;
        render::status(&plan.namespace, &rows, &broker, format)
    }

    pub(crate) fn list(&self, format: Format) -> Result<(), String> {
        let plan = self.state.plan();
        let hits = process::Stamped::discover(None, &plan.namespace)
            .map_err(|err| format!("discovery failed for namespace `{}`: {err}", plan.namespace))?;
        let broker = Broker::new(&plan).status()?;
        let state = runtime::state::load(&self.paths)?;
        let listing = render::Listing {
            namespace: &plan.namespace,
            hits: &hits,
            broker: &broker,
            state: &state,
        };
        render::list(&listing, format)
    }

    pub(crate) fn reset(&self, all: bool, force: bool) -> Result<(), String> {
        let plan = self.state.plan();
        for target in &plan.targets {
            for pid in runtime::running(&self.paths, target)? {
                process::stop(pid)
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
                process::stop(hit.pid)
                    .map_err(|err| format!("failed to terminate pid {}: {err}", hit.pid))?;
                runtime::reap(hit.pid, force)?;
                println!("terminated pid={} cmd={}", hit.pid, hit.command);
            }
        }
        Broker::new(&plan).stop(force)?;
        purge(&self.paths.project, "project data")?;
        if all {
            purge(&self.paths.state, "global state")?;
        }
        Ok(())
    }

    pub(crate) fn inspect(&self, probe: &Probe, format: Format) -> Result<(), String> {
        let plan = self.state.plan();
        let target = plan
            .targets
            .iter()
            .find(|item| item.name == probe.sidecar)
            .ok_or_else(|| format!("unknown target `{}` in this manifest", probe.sidecar))?;
        let socket = target.socket.as_deref().ok_or_else(|| {
            format!(
                "target `{}` has no inspect_socket configured in this manifest",
                probe.sidecar
            )
        })?;
        let endpoint = socket::Endpoint::parse(socket).map_err(|err| err.to_string())?;
        let body: Value = match probe.payload {
            Some(text) if !text.is_empty() => serde_json::from_str(text).map_err(|err| {
                format!("payload is not valid JSON: {err}; quote the payload as a single argument")
            })?,
            _ => serde_json::json!({}),
        };
        let request = inspect::Request {
            event: probe.event.to_string(),
            payload: body,
        };
        let response = inspect::send(&endpoint, &request, Some(probe.timeout))?;
        render::inspect(probe.sidecar, probe.event, &response, format)
    }

    fn spawn(
        &self,
        target: &Target,
        endpoint: &str,
        env: &[(String, String)],
    ) -> Result<Launch, String> {
        let cwd = self.cwd(&target.cwd);
        let path = self.log(&target.name);
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
        let port = lease(target)?;
        if let Some(port) = port {
            command.env(PORT, port.to_string());
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
        Ok(Launch {
            pid,
            ready,
            log: path,
            port,
        })
    }

    fn cwd(&self, cwd: &str) -> PathBuf {
        let path = Path::new(cwd);
        if path.is_absolute() {
            return path.to_path_buf();
        }
        match self.state.path.parent() {
            Some(dir) => dir.join(path),
            None => path.to_path_buf(),
        }
    }

    fn log(&self, name: &str) -> PathBuf {
        self.paths.project.join("logs").join(format!("{name}.log"))
    }
}

fn lease(target: &Target) -> Result<Option<u16>, String> {
    match target.port {
        Some(0) => {
            let listener = TcpListener::bind(("127.0.0.1", 0))
                .map_err(|err| format!("failed to lease a port for `{}`: {err}", target.name))?;
            let local = listener
                .local_addr()
                .map_err(|err| format!("failed to read the leased port: {err}"))?;
            Ok(Some(local.port()))
        }
        other => Ok(other),
    }
}

fn health(target: &Target, state: &Map<String, Value>) -> Option<String> {
    let template = target.health.as_ref()?;
    if !template.contains("{port}") {
        return Some(template.clone());
    }
    let port = state.get(&target.name)?.get("port")?.as_u64()?;
    Some(template.replace("{port}", &port.to_string()))
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
