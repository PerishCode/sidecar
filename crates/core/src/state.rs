use crate::config::Manifest;
use crate::diagnostics::Diagnostic;
use crate::plan::Plan;
use crate::socket;
use std::collections::HashSet;
use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Clone, Debug)]
pub struct State {
    pub path: PathBuf,
    pub config: Manifest,
}

#[derive(Debug)]
pub enum Error {
    Read {
        path: PathBuf,
        source: std::io::Error,
    },
    Parse {
        path: PathBuf,
        source: Box<toml::de::Error>,
    },
}

impl State {
    pub fn load(path: impl AsRef<Path>) -> Result<Self, Error> {
        let path = path.as_ref().to_path_buf();
        let text = fs::read_to_string(&path).map_err(|source| Error::Read {
            path: path.clone(),
            source,
        })?;
        let config = toml::from_str(&text).map_err(|source| Error::Parse {
            path: path.clone(),
            source: Box::new(source),
        })?;
        Ok(Self { path, config })
    }

    pub fn diagnostics(&self) -> Vec<Diagnostic> {
        let mut diagnostics = Vec::new();
        require(&mut diagnostics, "project.name", &self.config.project.name);
        require(
            &mut diagnostics,
            "project.namespace",
            &self.config.project.namespace,
        );

        if let Some(app) = &self.config.app {
            require(&mut diagnostics, "app.name", &app.name);
            require(&mut diagnostics, "app.command", &app.command);
            require(&mut diagnostics, "app.mode", &app.mode);
            if let Some(socket) = &app.socket {
                if let Err(error) = socket::Endpoint::parse(socket) {
                    diagnostics.push(Diagnostic::error("app.inspect_socket", error.to_string()));
                }
            }
            if let Some(ready) = &app.ready {
                require(&mut diagnostics, "app.ready.role", &ready.role);
            }
            warn(&mut diagnostics, "app", &app.command, &app.args);
        } else if self.config.sidecars.is_empty() {
            diagnostics.push(Diagnostic::warning(
                "app",
                "no app or sidecar command is configured; lifecycle commands have no targets",
            ));
        }

        let mut names = HashSet::new();
        for (index, sidecar) in self.config.sidecars.iter().enumerate() {
            let path = format!("sidecars[{index}]");
            require(&mut diagnostics, format!("{path}.name"), &sidecar.name);
            require(
                &mut diagnostics,
                format!("{path}.command"),
                &sidecar.command,
            );
            require(&mut diagnostics, format!("{path}.mode"), &sidecar.mode);
            if !sidecar.name.trim().is_empty() && !names.insert(sidecar.name.as_str()) {
                diagnostics.push(Diagnostic::error(
                    format!("{path}.name"),
                    format!("duplicate sidecar name `{}`", sidecar.name),
                ));
            }
            if let Some(socket) = &sidecar.socket {
                if let Err(error) = socket::Endpoint::parse(socket) {
                    diagnostics.push(Diagnostic::error(
                        format!("{path}.inspect_socket"),
                        error.to_string(),
                    ));
                }
            }
            if let Some(ready) = &sidecar.ready {
                require(&mut diagnostics, format!("{path}.ready.role"), &ready.role);
            }
            warn(&mut diagnostics, &path, &sidecar.command, &sidecar.args);
        }

        let mut names = HashSet::new();
        for (index, endpoint) in self.config.inspect.endpoints.iter().enumerate() {
            let path = format!("inspect.endpoints[{index}]");
            require(&mut diagnostics, format!("{path}.name"), &endpoint.name);
            require(&mut diagnostics, format!("{path}.kind"), &endpoint.kind);
            require(&mut diagnostics, format!("{path}.url"), &endpoint.url);
            if !endpoint.name.trim().is_empty() && !names.insert(endpoint.name.as_str()) {
                diagnostics.push(Diagnostic::error(
                    format!("{path}.name"),
                    format!("duplicate inspect endpoint name `{}`", endpoint.name),
                ));
            }
        }

        diagnostics
    }

    pub fn plan(&self) -> Plan {
        self.config.plan()
    }
}

fn require(diagnostics: &mut Vec<Diagnostic>, path: impl Into<String>, value: &str) {
    if value.trim().is_empty() {
        diagnostics.push(Diagnostic::error(path, "value must not be empty"));
    }
}

fn warn(diagnostics: &mut Vec<Diagnostic>, path: &str, command: &str, args: &[String]) {
    if !cargo(command) || !consumes(args) {
        return;
    }
    diagnostics.push(Diagnostic::warning(
        format!("{path}.args"),
        "cargo run target may consume the appended --sidecar-stamp argument; add `--` after cargo run options",
    ));
}

fn cargo(command: &str) -> bool {
    command
        .rsplit(['/', '\\'])
        .next()
        .map(|name| name == "cargo" || name == "cargo.exe")
        .unwrap_or(false)
}

fn consumes(args: &[String]) -> bool {
    let Some(index) = args.iter().position(|arg| arg == "run") else {
        return false;
    };
    !args.iter().skip(index + 1).any(|arg| arg == "--")
}

impl fmt::Display for Error {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::Read { path, source } => {
                write!(formatter, "failed to read {}: {source}", path.display())
            }
            Error::Parse { path, source } => {
                write!(formatter, "failed to parse {}: {source}", path.display())
            }
        }
    }
}

impl std::error::Error for Error {}
