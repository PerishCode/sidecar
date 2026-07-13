use crate::update;
use crate::{broker, commands, output};
use sidecar_core::{Paths, Severity, State};
use std::{
    path::{Path, PathBuf},
    time::Duration,
};

mod default {
    pub(super) const TIMEOUT: u64 = 5;
    pub(super) const MANIFEST: &str = "sidecar.toml";
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Format {
    Text,
    Json,
}

impl Format {
    fn parse(value: &str) -> Result<Self, String> {
        match value {
            "text" => Ok(Format::Text),
            "json" => Ok(Format::Json),
            _ => Err(format!("unsupported output format: {value}")),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct Args {
    command: Vec<String>,
    config: Option<String>,
    format: Format,
    home: Option<String>,
    project: Option<String>,
    timeout: u64,
    all: bool,
    force: bool,
}

pub fn version() -> &'static str {
    option_env!("SIDECAR_BUILD_VERSION").unwrap_or(concat!("v", env!("CARGO_PKG_VERSION")))
}

pub fn channel() -> &'static str {
    option_env!("SIDECAR_BUILD_CHANNEL").unwrap_or("dev")
}

pub fn help() -> &'static str {
    r#"sidecar

Product-neutral sidecar lifecycle and inspect IPC manager.
It owns manifest-closed lifecycle, appends stamp identity, discovers/stops
targets, and sends one-shot inspect events; consumers own product semantics.

Commands:
  doctor   [--config <path>] [--format text|json]
  plan     [--config <path>] [--format text|json]
  inspect  config [--config <path>] [--format text|json]
  inspect  <sidecar> <event> [<json-payload>] [--config <path>] [--format text|json] [--inspect-timeout <seconds>]
  start    [--config <path>] [<sidecar>]
  restart  [--config <path>] [<sidecar>]
  stop     [--config <path>] [--force] [<sidecar>]
  status   [--config <path>] [--format text|json]
  list     [--config <path>] [--format text|json]
  reset    [--config <path>] [--all] [--force]
  update
  help
  version

Global flags:
  --config <path>       explicit manifest path; when omitted, sidecar walks
                        ancestors of cwd for sidecar.toml
  -p, --project <name>  override [project].namespace, like docker compose -p
  --data-home <path>    override global state/update-cache root
  --format text|json    output format where the command supports it
  --inspect-timeout <s> inspect round-trip timeout in seconds (default: 5)
  --force               force-kill sidecar-owned pids after graceful stop waits

Model:
  Manifest: [project], optional [app], repeated [[sidecars]], ready/env/inspect
  fields, and optional [[inspect.endpoints]]. See README.md for the schema.
  Lifecycle: command/cwd/args/env/stamps/ready/inspect/stop/reset close in manifest.
  Stamps: --sidecar-stamp=v=1;a=<app>;n=<namespace>;m=<mode>;s=<source>;e=<endpoint>;
  values are percent-encoded; the stamp is the only sidecar launch metadata.
  Inspect: one SidecarRuntime event frame over unix:// sockets; TCP is fallback.
  State: <data-home>/state plus <data-home>/projects/<namespace>; see AGENTS.md.

Safety:
  stop/reset are signal-first and observe sidecar-owned pids; add --force to
  kill after graceful waits. reset removes project state; add --all to also
  remove global state.
  update delegates to the released manager. Dev builds cannot self-update.

Exit shape:
  0 on success. 1 on config, diagnostic, lifecycle, inspect, or update failure.

Project:
  Source:  https://github.com/PerishCode/sidecar
  Issues:  https://github.com/PerishCode/sidecar/issues
  Details: README.md for usage/schema; AGENTS.md for boundaries and PR workflow.
"#
}

pub fn run(args: Vec<String>) -> Result<(), String> {
    let parsed = parse(args)?;
    if parsed.command.is_empty() {
        print!("{help}", help = help());
        println!();
        return Ok(());
    }

    if let Some(home) = &parsed.home {
        std::env::set_var("SIDECAR_DATA_HOME", home);
    }
    if let Some(project) = &parsed.project {
        std::env::set_var("SIDECAR_PROJECT", project);
    }

    let cmd = parsed.command[0].as_str();
    if !matches!(
        cmd,
        "help" | "--help" | "-h" | "version" | "--version" | "-V" | "update" | "runtime"
    ) {
        update::notice(version(), channel());
    }
    match cmd {
        "help" | "--help" | "-h" => {
            println!("{}", help());
            Ok(())
        }
        "version" | "--version" | "-V" => {
            println!("sidecar {} ({})", version(), channel());
            Ok(())
        }
        "update" => {
            parsed.exact(1, "update")?;
            update::run(channel())
        }
        "runtime" => runtime(&parsed),
        "doctor" => {
            parsed.exact(1, "doctor")?;
            let state = parsed.state()?;
            let diagnostics = state.diagnostics();
            output::diagnostics(&diagnostics, parsed.format)?;
            if diagnostics
                .iter()
                .any(|diagnostic| diagnostic.severity == Severity::Error)
            {
                Err("sidecar doctor found configuration errors".to_string())
            } else {
                Ok(())
            }
        }
        "plan" => {
            parsed.exact(1, "plan")?;
            let state = parsed.state()?;
            output::plan(&state.plan(), parsed.format)
        }
        "inspect" => inspect(&parsed),
        "start" | "stop" | "restart" => {
            let target = parsed.target(cmd)?;
            let state = parsed.state()?;
            let paths = parsed.paths(&state);
            match cmd {
                "start" => commands::start(&state, &paths, target),
                "stop" => commands::stop(&state, &paths, target, parsed.force),
                "restart" => commands::restart(&state, &paths, target, parsed.force),
                _ => unreachable!(),
            }
        }
        "status" => {
            parsed.exact(1, "status")?;
            let state = parsed.state()?;
            let paths = parsed.paths(&state);
            commands::status(&state, &paths, parsed.format)
        }
        "list" => {
            parsed.exact(1, "list")?;
            let state = parsed.state()?;
            let paths = parsed.paths(&state);
            commands::list(&state, &paths, parsed.format)
        }
        "reset" => {
            parsed.exact(1, "reset")?;
            let state = parsed.state()?;
            let paths = parsed.paths(&state);
            commands::reset(&state, &paths, parsed.all, parsed.force)
        }
        _ => Err(format!(
            "unknown command: {}; run `sidecar help`",
            parsed.command.join(" ")
        )),
    }
}

fn runtime(parsed: &Args) -> Result<(), String> {
    match parsed.command.as_slice() {
        [_, verb, project, namespace, ..] if verb == "serve" => broker::serve(project, namespace),
        [_, verb, ..] if verb == "serve" => {
            Err("runtime serve requires <project> <namespace>".to_string())
        }
        _ => Err(
            "unknown runtime command; expected `runtime serve <project> <namespace>`".to_string(),
        ),
    }
}

fn inspect(parsed: &Args) -> Result<(), String> {
    match parsed.command.len() {
        1 => Err("inspect requires `config` or `<sidecar> <event> [payload]`".to_string()),
        _ if parsed.command[1] == "config" => {
            parsed.exact(2, "inspect config")?;
            let state = parsed.state()?;
            output::plan(&state.plan(), parsed.format)
        }
        len if len < 3 => Err("inspect <sidecar> <event> [payload] — event is required".into()),
        len if len > 4 => Err(format!(
            "unsupported inspect arguments: {}",
            parsed.command[4..].join(" ")
        )),
        _ => {
            let state = parsed.state()?;
            let payload = parsed.command.get(3).map(String::as_str);
            commands::inspect(
                &state,
                &parsed.command[1],
                &parsed.command[2],
                payload,
                Duration::from_secs(parsed.timeout),
                parsed.format,
            )
        }
    }
}

impl Args {
    fn target(&self, command: &str) -> Result<Option<&str>, String> {
        match self.command.len() {
            1 => Ok(None),
            2 => Ok(Some(self.command[1].as_str())),
            _ => Err(format!(
                "unsupported {command} arguments: {}",
                self.command[2..].join(" ")
            )),
        }
    }

    fn exact(&self, expected: usize, command: &str) -> Result<(), String> {
        if self.command.len() > expected {
            return Err(format!(
                "unsupported {command} arguments: {}",
                self.command[expected..].join(" ")
            ));
        }
        Ok(())
    }

    fn state(&self) -> Result<State, String> {
        let (config, discovered) = locate(self.config.as_deref())?;
        if discovered {
            eprintln!("sidecar: using config {}", config.display());
        }
        let mut state = State::load(&config).map_err(|error| error.to_string())?;
        let env = std::env::var("SIDECAR_PROJECT")
            .ok()
            .filter(|value| !value.is_empty());
        if let Some(ns) = self.project.clone().or(env) {
            state.config.project.namespace = ns;
        }
        Ok(state)
    }

    fn paths(&self, state: &State) -> Paths {
        let mut paths = Paths::resolve(
            &state.config.project.namespace,
            self.home.as_deref().map(Path::new),
            state.config.project.data.as_deref(),
        );
        if state.config.project.data.is_some() && paths.project.is_relative() {
            if let Some(config_dir) = state.path.parent() {
                paths.project = config_dir.join(&paths.project);
            }
        }
        paths
    }
}

fn locate(explicit: Option<&str>) -> Result<(PathBuf, bool), String> {
    if let Some(config) = explicit {
        return Ok((PathBuf::from(config), false));
    }

    let cwd = std::env::current_dir().map_err(|error| format!("failed to read cwd: {error}"))?;
    let mut searched = Vec::new();
    for dir in cwd.ancestors() {
        let candidate = dir.join(default::MANIFEST);
        if candidate.is_file() {
            return Ok((candidate, true));
        }
        searched.push(candidate);
    }

    let searched = searched
        .iter()
        .map(|path| format!("- {}", path.display()))
        .collect::<Vec<_>>()
        .join("\n");
    Err(format!(
        "no sidecar config found from {} upward.\nHint: create sidecar.toml here or pass --config <path>.\nSearched:\n{searched}",
        cwd.display()
    ))
}

fn parse(args: Vec<String>) -> Result<Args, String> {
    let mut command = Vec::new();
    let mut config = None;
    let mut format = Format::Text;
    let mut home = None;
    let mut project = None;
    let mut timeout = default::TIMEOUT;
    let mut all = false;
    let mut force = false;
    let mut args = args.into_iter();
    let _binary = args.next();

    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--config" => {
                config = Some(
                    args.next()
                        .ok_or_else(|| "--config requires a value".to_string())?,
                );
            }
            "--format" => {
                let value = args
                    .next()
                    .ok_or_else(|| "--format requires a value".to_string())?;
                format = Format::parse(&value)?;
            }
            "--data-home" => {
                home = Some(
                    args.next()
                        .ok_or_else(|| "--data-home requires a value".to_string())?,
                );
            }
            "-p" | "--project" => {
                project = Some(
                    args.next()
                        .ok_or_else(|| "--project requires a value".to_string())?,
                );
            }
            "--all" => {
                all = true;
            }
            "--force" => {
                force = true;
            }
            "--inspect-timeout" => {
                let value = args
                    .next()
                    .ok_or_else(|| "--inspect-timeout requires a value".to_string())?;
                timeout = seconds("--inspect-timeout", &value)?;
            }
            value if value.starts_with("--config=") => {
                config = Some(value.trim_start_matches("--config=").to_string());
            }
            value if value.starts_with("--format=") => {
                format = Format::parse(value.trim_start_matches("--format="))?;
            }
            value if value.starts_with("--data-home=") => {
                home = Some(value.trim_start_matches("--data-home=").to_string());
            }
            value if value.starts_with("--project=") => {
                project = Some(value.trim_start_matches("--project=").to_string());
            }
            value if value.starts_with("--inspect-timeout=") => {
                timeout = seconds(
                    "--inspect-timeout",
                    value.trim_start_matches("--inspect-timeout="),
                )?;
            }
            value
                if value.starts_with('-')
                    && !matches!(value, "-h" | "--help" | "-V" | "--version")
                    && !value.starts_with("--sidecar-broker") =>
            {
                return Err(format!("unknown option: {value}"));
            }
            value => command.push(value.to_string()),
        }
    }

    Ok(Args {
        command,
        config,
        format,
        home,
        project,
        timeout,
        all,
        force,
    })
}

fn seconds(option: &str, value: &str) -> Result<u64, String> {
    let parsed = value
        .parse::<u64>()
        .map_err(|_| format!("{option} requires a positive integer value"))?;
    if parsed == 0 {
        return Err(format!("{option} requires a positive integer value"));
    }
    Ok(parsed)
}

#[doc(hidden)]
pub mod __test {
    use super::Format;

    #[derive(Debug, Eq, PartialEq)]
    pub struct Summary {
        pub command: Vec<String>,
        pub config: Option<String>,
        pub format: &'static str,
        pub home: Option<String>,
        pub project: Option<String>,
        pub timeout: u64,
        pub all: bool,
        pub force: bool,
    }

    pub fn parse(args: Vec<&str>) -> Result<Summary, String> {
        let parsed = super::parse(args.into_iter().map(String::from).collect())?;
        let format = match parsed.format {
            Format::Text => "text",
            Format::Json => "json",
        };
        Ok(Summary {
            command: parsed.command,
            config: parsed.config,
            format,
            home: parsed.home,
            project: parsed.project,
            timeout: parsed.timeout,
            all: parsed.all,
            force: parsed.force,
        })
    }

    pub fn locate(explicit: Option<&str>) -> Result<(String, bool), String> {
        let (path, discovered) = super::locate(explicit)?;
        Ok((path.display().to_string(), discovered))
    }
}
