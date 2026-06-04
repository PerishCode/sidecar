use crate::config::Manifest;
use crate::diagnostics::Diagnostic;
use crate::plan::ExecutionPlan;
use crate::socket::SocketEndpoint;
use std::collections::HashSet;
use std::error::Error;
use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Clone, Debug)]
pub struct DevState {
    pub config_path: PathBuf,
    pub config: Manifest,
}

#[derive(Debug)]
pub enum LoadError {
    Read {
        path: PathBuf,
        source: std::io::Error,
    },
    Parse {
        path: PathBuf,
        source: Box<toml::de::Error>,
    },
}

impl DevState {
    pub fn from_config_file(path: impl AsRef<Path>) -> Result<Self, LoadError> {
        let path = path.as_ref().to_path_buf();
        let text = fs::read_to_string(&path).map_err(|source| LoadError::Read {
            path: path.clone(),
            source,
        })?;
        let config = toml::from_str(&text).map_err(|source| LoadError::Parse {
            path: path.clone(),
            source: Box::new(source),
        })?;
        Ok(Self {
            config_path: path,
            config,
        })
    }

    pub fn diagnostics(&self) -> Vec<Diagnostic> {
        let mut diagnostics = Vec::new();
        validate_required_name(&mut diagnostics, "project.name", &self.config.project.name);
        validate_required_name(
            &mut diagnostics,
            "project.namespace",
            &self.config.project.namespace,
        );

        if let Some(app) = &self.config.app {
            validate_required_name(&mut diagnostics, "app.name", &app.name);
            validate_required_name(&mut diagnostics, "app.command", &app.command);
            validate_required_name(&mut diagnostics, "app.mode", &app.mode);
            if let Some(socket) = &app.inspect_socket {
                if let Err(error) = SocketEndpoint::parse(socket) {
                    diagnostics.push(Diagnostic::error("app.inspect_socket", error.to_string()));
                }
            }
            if let Some(ready) = &app.ready {
                validate_required_name(&mut diagnostics, "app.ready.role", &ready.role);
            }
        } else {
            diagnostics.push(Diagnostic::warning(
                "app",
                "no app command is configured; only sidecar and inspect plans can run",
            ));
        }

        let mut sidecar_names = HashSet::new();
        for (index, sidecar) in self.config.sidecars.iter().enumerate() {
            let path = format!("sidecars[{index}]");
            validate_required_name(&mut diagnostics, format!("{path}.name"), &sidecar.name);
            validate_required_name(
                &mut diagnostics,
                format!("{path}.command"),
                &sidecar.command,
            );
            validate_required_name(&mut diagnostics, format!("{path}.mode"), &sidecar.mode);
            if !sidecar.name.trim().is_empty() && !sidecar_names.insert(sidecar.name.as_str()) {
                diagnostics.push(Diagnostic::error(
                    format!("{path}.name"),
                    format!("duplicate sidecar name `{}`", sidecar.name),
                ));
            }
            if let Some(socket) = &sidecar.inspect_socket {
                if let Err(error) = SocketEndpoint::parse(socket) {
                    diagnostics.push(Diagnostic::error(
                        format!("{path}.inspect_socket"),
                        error.to_string(),
                    ));
                }
            }
            if let Some(ready) = &sidecar.ready {
                validate_required_name(&mut diagnostics, format!("{path}.ready.role"), &ready.role);
            }
        }

        let mut endpoint_names = HashSet::new();
        for (index, endpoint) in self.config.inspect.endpoints.iter().enumerate() {
            let path = format!("inspect.endpoints[{index}]");
            validate_required_name(&mut diagnostics, format!("{path}.name"), &endpoint.name);
            validate_required_name(&mut diagnostics, format!("{path}.kind"), &endpoint.kind);
            validate_required_name(&mut diagnostics, format!("{path}.url"), &endpoint.url);
            if !endpoint.name.trim().is_empty() && !endpoint_names.insert(endpoint.name.as_str()) {
                diagnostics.push(Diagnostic::error(
                    format!("{path}.name"),
                    format!("duplicate inspect endpoint name `{}`", endpoint.name),
                ));
            }
        }

        diagnostics
    }

    pub fn execution_plan(&self) -> ExecutionPlan {
        ExecutionPlan::from_config(&self.config)
    }
}

fn validate_required_name(diagnostics: &mut Vec<Diagnostic>, path: impl Into<String>, value: &str) {
    if value.trim().is_empty() {
        diagnostics.push(Diagnostic::error(path, "value must not be empty"));
    }
}

impl fmt::Display for LoadError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            LoadError::Read { path, source } => {
                write!(formatter, "failed to read {}: {source}", path.display())
            }
            LoadError::Parse { path, source } => {
                write!(formatter, "failed to parse {}: {source}", path.display())
            }
        }
    }
}

impl Error for LoadError {}
