use crate::commands;
use crate::output::{print_diagnostics, print_plan};
use crate::update;
use sidecar_core::{resolve_data_paths, DataPaths, DevState, Severity};
use std::{path::Path, time::Duration};

const INSPECT_DEFAULT_TIMEOUT_SECS: u64 = 5;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum OutputFormat {
    Text,
    Json,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct ParsedArgs {
    command: Vec<String>,
    config: Option<String>,
    format: OutputFormat,
    data_home: Option<String>,
    project_override: Option<String>,
    inspect_timeout_secs: u64,
    reset_all: bool,
}

pub fn version() -> &'static str {
    option_env!("SIDECAR_BUILD_VERSION").unwrap_or(concat!("v", env!("CARGO_PKG_VERSION")))
}

pub fn channel() -> &'static str {
    option_env!("SIDECAR_BUILD_CHANNEL").unwrap_or("dev")
}

pub fn help_text() -> &'static str {
    r#"sidecar

Product-neutral sidecar lifecycle and inspect IPC manager.
It owns manifest-closed lifecycle, appends stamp identity, discovers/stops
targets, and sends one-shot inspect events; consumers own product semantics.

Commands:
  doctor   --config <path> [--format text|json]
  plan     --config <path> [--format text|json]
  inspect  config --config <path> [--format text|json]
  inspect  <sidecar> <event> [<json-payload>] --config <path> [--format text|json] [--inspect-timeout <seconds>]
  start    --config <path> [<sidecar>]
  restart  --config <path> [<sidecar>]
  stop     --config <path> [<sidecar>]
  status   --config <path> [--format text|json]
  list     --config <path> [--format text|json]
  reset    --config <path> [--all]
  update
  help
  version

Global flags:
  --config <path>       explicit manifest path; no default filename is reserved
  -p, --project <name>  override [project].namespace, like docker compose -p
  --data-home <path>    override global state/update-cache root
  --format text|json    output format where the command supports it
  --inspect-timeout <s> inspect round-trip timeout in seconds (default: 5)

Model:
  Manifest: [project], optional [app], repeated [[sidecars]], ready/env/inspect
  fields, and optional [[inspect.endpoints]]. See README.md for the schema.
  Lifecycle: command/cwd/args/env/stamps/ready/inspect/stop/reset close in manifest.
  Stamps: --sidecar-stamp=a=<app>;n=<namespace>;m=<mode>;s=<source>;
  values are percent-encoded; env stamping is explicit.
  Inspect: one SidecarRuntime event frame over unix:// sockets; TCP is fallback.
  State: <data-home>/state plus <data-home>/projects/<namespace>; see AGENTS.md.

Safety:
  reset is the compatibility escape hatch: stop stamped processes and remove
  project state; add --all to also remove global state.
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
        print!("{help}", help = help_text());
        println!();
        return Ok(());
    }

    if let Some(home) = &parsed.data_home {
        std::env::set_var("SIDECAR_DATA_HOME", home);
    }
    if let Some(project) = &parsed.project_override {
        std::env::set_var("SIDECAR_PROJECT", project);
    }

    let cmd = parsed.command[0].as_str();
    if !matches!(
        cmd,
        "help" | "--help" | "-h" | "version" | "--version" | "-V" | "update"
    ) {
        update::maybe_emit_check_notice(version(), channel());
    }
    match cmd {
        "help" | "--help" | "-h" => {
            println!("{}", help_text());
            Ok(())
        }
        "version" | "--version" | "-V" => {
            println!("sidecar {} ({})", version(), channel());
            Ok(())
        }
        "update" => {
            require_no_extra_args(&parsed, 1, "update")?;
            update::run_update(channel())
        }
        "doctor" => {
            require_no_extra_args(&parsed, 1, "doctor")?;
            let state = load_state(&parsed)?;
            let diagnostics = state.diagnostics();
            print_diagnostics(&diagnostics, parsed.format)?;
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
            require_no_extra_args(&parsed, 1, "plan")?;
            let state = load_state(&parsed)?;
            print_plan(&state.execution_plan(), parsed.format)
        }
        "inspect" => run_inspect_command(&parsed),
        "start" | "stop" | "restart" => {
            let target = optional_target(&parsed, cmd)?;
            let state = load_state(&parsed)?;
            let paths = data_paths_for(&parsed, &state);
            match cmd {
                "start" => commands::start(&state, &paths, target),
                "stop" => commands::stop(&state, &paths, target),
                "restart" => commands::restart(&state, &paths, target),
                _ => unreachable!(),
            }
        }
        "status" => {
            require_no_extra_args(&parsed, 1, "status")?;
            let state = load_state(&parsed)?;
            let paths = data_paths_for(&parsed, &state);
            commands::status(&state, &paths, parsed.format)
        }
        "list" => {
            require_no_extra_args(&parsed, 1, "list")?;
            let state = load_state(&parsed)?;
            let paths = data_paths_for(&parsed, &state);
            commands::list(&state, &paths, parsed.format)
        }
        "reset" => {
            require_no_extra_args(&parsed, 1, "reset")?;
            let state = load_state(&parsed)?;
            let paths = data_paths_for(&parsed, &state);
            commands::reset(&state, &paths, parsed.reset_all)
        }
        _ => Err(format!(
            "unknown command: {}; run `sidecar help`",
            parsed.command.join(" ")
        )),
    }
}

fn run_inspect_command(parsed: &ParsedArgs) -> Result<(), String> {
    match parsed.command.len() {
        1 => Err("inspect requires `config` or `<sidecar> <event> [payload]`".to_string()),
        _ if parsed.command[1] == "config" => {
            require_no_extra_args(parsed, 2, "inspect config")?;
            let state = load_state(parsed)?;
            print_plan(&state.execution_plan(), parsed.format)
        }
        len if len < 3 => Err("inspect <sidecar> <event> [payload] — event is required".into()),
        len if len > 4 => Err(format!(
            "unsupported inspect arguments: {}",
            parsed.command[4..].join(" ")
        )),
        _ => {
            let state = load_state(parsed)?;
            let payload = parsed.command.get(3).map(String::as_str);
            commands::inspect(
                &state,
                &parsed.command[1],
                &parsed.command[2],
                payload,
                Duration::from_secs(parsed.inspect_timeout_secs),
                parsed.format,
            )
        }
    }
}

fn optional_target<'a>(parsed: &'a ParsedArgs, command: &str) -> Result<Option<&'a str>, String> {
    match parsed.command.len() {
        1 => Ok(None),
        2 => Ok(Some(parsed.command[1].as_str())),
        _ => Err(format!(
            "unsupported {command} arguments: {}",
            parsed.command[2..].join(" ")
        )),
    }
}

fn require_no_extra_args(
    parsed: &ParsedArgs,
    expected_len: usize,
    command: &str,
) -> Result<(), String> {
    if parsed.command.len() > expected_len {
        return Err(format!(
            "unsupported {command} arguments: {}",
            parsed.command[expected_len..].join(" ")
        ));
    }
    Ok(())
}

fn load_state(parsed: &ParsedArgs) -> Result<DevState, String> {
    let config = parsed
        .config
        .as_ref()
        .ok_or_else(|| "--config <path> is required".to_string())?;
    let mut state = DevState::from_config_file(config).map_err(|error| error.to_string())?;
    let env_project = std::env::var("SIDECAR_PROJECT")
        .ok()
        .filter(|value| !value.is_empty());
    if let Some(ns) = parsed.project_override.clone().or(env_project) {
        state.config.project.namespace = ns;
    }
    Ok(state)
}

fn data_paths_for(parsed: &ParsedArgs, state: &DevState) -> DataPaths {
    let mut paths = resolve_data_paths(
        &state.config.project.namespace,
        parsed.data_home.as_deref().map(Path::new),
        state.config.project.data_dir.as_deref(),
    );
    if state.config.project.data_dir.is_some() && paths.project.is_relative() {
        if let Some(config_dir) = state.config_path.parent() {
            paths.project = config_dir.join(&paths.project);
        }
    }
    paths
}

fn parse(args: Vec<String>) -> Result<ParsedArgs, String> {
    let mut command = Vec::new();
    let mut config = None;
    let mut format = OutputFormat::Text;
    let mut data_home = None;
    let mut project_override = None;
    let mut inspect_timeout_secs = INSPECT_DEFAULT_TIMEOUT_SECS;
    let mut reset_all = false;
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
                format = parse_format(&value)?;
            }
            "--data-home" => {
                data_home = Some(
                    args.next()
                        .ok_or_else(|| "--data-home requires a value".to_string())?,
                );
            }
            "-p" | "--project" => {
                project_override = Some(
                    args.next()
                        .ok_or_else(|| "--project requires a value".to_string())?,
                );
            }
            "--all" => {
                reset_all = true;
            }
            "--inspect-timeout" => {
                let value = args
                    .next()
                    .ok_or_else(|| "--inspect-timeout requires a value".to_string())?;
                inspect_timeout_secs = parse_positive_seconds("--inspect-timeout", &value)?;
            }
            value if value.starts_with("--config=") => {
                config = Some(value.trim_start_matches("--config=").to_string());
            }
            value if value.starts_with("--format=") => {
                format = parse_format(value.trim_start_matches("--format="))?;
            }
            value if value.starts_with("--data-home=") => {
                data_home = Some(value.trim_start_matches("--data-home=").to_string());
            }
            value if value.starts_with("--project=") => {
                project_override = Some(value.trim_start_matches("--project=").to_string());
            }
            value if value.starts_with("--inspect-timeout=") => {
                inspect_timeout_secs = parse_positive_seconds(
                    "--inspect-timeout",
                    value.trim_start_matches("--inspect-timeout="),
                )?;
            }
            value
                if value.starts_with('-')
                    && !matches!(value, "-h" | "--help" | "-V" | "--version") =>
            {
                return Err(format!("unknown option: {value}"));
            }
            value => command.push(value.to_string()),
        }
    }

    Ok(ParsedArgs {
        command,
        config,
        format,
        data_home,
        project_override,
        inspect_timeout_secs,
        reset_all,
    })
}

fn parse_format(value: &str) -> Result<OutputFormat, String> {
    match value {
        "text" => Ok(OutputFormat::Text),
        "json" => Ok(OutputFormat::Json),
        _ => Err(format!("unsupported output format: {value}")),
    }
}

fn parse_positive_seconds(option: &str, value: &str) -> Result<u64, String> {
    let seconds = value
        .parse::<u64>()
        .map_err(|_| format!("{option} requires a positive integer value"))?;
    if seconds == 0 {
        return Err(format!("{option} requires a positive integer value"));
    }
    Ok(seconds)
}

#[doc(hidden)]
pub mod __test {
    use super::{parse, OutputFormat};

    #[derive(Debug, Eq, PartialEq)]
    pub struct ParseSummary {
        pub command: Vec<String>,
        pub config: Option<String>,
        pub format: &'static str,
        pub data_home: Option<String>,
        pub project: Option<String>,
        pub timeout_secs: u64,
        pub reset_all: bool,
    }

    pub fn parse_args(args: Vec<&str>) -> Result<ParseSummary, String> {
        let parsed = parse(args.into_iter().map(String::from).collect())?;
        let format = match parsed.format {
            OutputFormat::Text => "text",
            OutputFormat::Json => "json",
        };
        Ok(ParseSummary {
            command: parsed.command,
            config: parsed.config,
            format,
            data_home: parsed.data_home,
            project: parsed.project_override,
            timeout_secs: parsed.inspect_timeout_secs,
            reset_all: parsed.reset_all,
        })
    }
}
