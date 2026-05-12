use std::collections::BTreeMap;

use crate::config::{
    AppConfig, InheritEnvConfig, InspectEndpointConfig, Manifest, ProjectConfig, ReadyConfig,
    SidecarConfig,
};
use crate::stamp::{Stamp, DEFAULT_SOURCE};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExecutionPlan {
    pub project: String,
    pub namespace: String,
    pub root: String,
    pub app: Option<AppPlan>,
    pub sidecars: Vec<SidecarPlan>,
    pub targets: Vec<TargetPlan>,
    pub inspect_endpoints: Vec<InspectEndpointPlan>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AppPlan {
    pub name: String,
    pub command: String,
    pub args: Vec<String>,
    pub cwd: String,
    pub stamp: Stamp,
    pub stamp_via_env: bool,
    pub env: BTreeMap<String, String>,
    pub endpoint_env: Option<String>,
    pub inherits_env: Vec<InheritEnvPlan>,
    pub inspect_socket: Option<String>,
    pub health_url: Option<String>,
    pub ready: Option<ReadyPlan>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SidecarPlan {
    pub name: String,
    pub command: String,
    pub args: Vec<String>,
    pub cwd: String,
    pub stamp: Stamp,
    pub stamp_via_env: bool,
    pub env: BTreeMap<String, String>,
    pub endpoint_env: Option<String>,
    pub inherits_env: Vec<InheritEnvPlan>,
    pub inspect_socket: Option<String>,
    pub health_url: Option<String>,
    pub ready: Option<ReadyPlan>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TargetPlan {
    pub name: String,
    pub kind: TargetKind,
    pub command: String,
    pub args: Vec<String>,
    pub cwd: String,
    pub stamp: Stamp,
    pub stamp_via_env: bool,
    pub env: BTreeMap<String, String>,
    pub endpoint_env: Option<String>,
    pub inherits_env: Vec<InheritEnvPlan>,
    pub inspect_socket: Option<String>,
    pub health_url: Option<String>,
    pub ready: Option<ReadyPlan>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TargetKind {
    App,
    Sidecar,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ReadyPlan {
    pub role: String,
    pub timeout_secs: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct InheritEnvPlan {
    pub name: String,
    pub from: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct InspectEndpointPlan {
    pub name: String,
    pub kind: String,
    pub url: String,
}

impl ExecutionPlan {
    pub fn from_config(config: &Manifest) -> Self {
        Self {
            project: config.project.name.clone(),
            namespace: config.project.namespace.clone(),
            root: config.project.root.clone(),
            app: config
                .app
                .as_ref()
                .map(|app| AppPlan::from_config(app, &config.project)),
            sidecars: config
                .sidecars
                .iter()
                .map(|sidecar| SidecarPlan::from_config(sidecar, &config.project))
                .collect(),
            targets: build_targets(config),
            inspect_endpoints: config
                .inspect
                .endpoints
                .iter()
                .map(InspectEndpointPlan::from_config)
                .collect(),
        }
    }
}

impl AppPlan {
    fn from_config(config: &AppConfig, project: &ProjectConfig) -> Self {
        let stamp = Stamp {
            app: config.name.clone(),
            namespace: project.namespace.clone(),
            mode: config.mode.clone(),
            source: DEFAULT_SOURCE.to_string(),
        };
        Self {
            name: config.name.clone(),
            command: config.command.clone(),
            args: config.args.clone(),
            cwd: config.cwd.clone(),
            stamp,
            stamp_via_env: config.stamp_via_env,
            env: config.env.clone(),
            endpoint_env: config.endpoint_env.clone(),
            inherits_env: config
                .inherits_env
                .iter()
                .map(InheritEnvPlan::from_config)
                .collect(),
            inspect_socket: config
                .inspect_socket
                .as_ref()
                .map(|value| expand_target_template(value, project, &config.name)),
            health_url: config.health_url.clone(),
            ready: config.ready.as_ref().map(ReadyPlan::from_config),
        }
    }
}

impl SidecarPlan {
    fn from_config(config: &SidecarConfig, project: &ProjectConfig) -> Self {
        let stamp = Stamp {
            app: config.name.clone(),
            namespace: project.namespace.clone(),
            mode: config.mode.clone(),
            source: DEFAULT_SOURCE.to_string(),
        };
        Self {
            name: config.name.clone(),
            command: config.command.clone(),
            args: config.args.clone(),
            cwd: config.cwd.clone(),
            stamp,
            stamp_via_env: config.stamp_via_env,
            env: config.env.clone(),
            endpoint_env: config.endpoint_env.clone(),
            inherits_env: config
                .inherits_env
                .iter()
                .map(InheritEnvPlan::from_config)
                .collect(),
            inspect_socket: config
                .inspect_socket
                .as_ref()
                .map(|value| expand_target_template(value, project, &config.name)),
            health_url: config.health_url.clone(),
            ready: config.ready.as_ref().map(ReadyPlan::from_config),
        }
    }

    /// Final argv to spawn (sidecar args followed by stamp args).
    pub fn spawn_args(&self) -> Vec<String> {
        let mut argv = self.args.clone();
        if !self.stamp_via_env {
            argv.extend(self.stamp.args());
        }
        argv
    }
}

impl TargetPlan {
    /// Final argv to spawn (target args followed by stamp args unless env stamping is requested).
    pub fn spawn_args(&self) -> Vec<String> {
        let mut argv = self.args.clone();
        if !self.stamp_via_env {
            argv.extend(self.stamp.args());
        }
        argv
    }
}

impl ReadyPlan {
    fn from_config(config: &ReadyConfig) -> Self {
        Self {
            role: config.role.clone(),
            timeout_secs: config.timeout_secs,
        }
    }
}

impl InheritEnvPlan {
    fn from_config(config: &InheritEnvConfig) -> Self {
        Self {
            name: config.name.clone(),
            from: config.from.clone(),
        }
    }
}

impl InspectEndpointPlan {
    fn from_config(config: &InspectEndpointConfig) -> Self {
        Self {
            name: config.name.clone(),
            kind: config.kind.clone(),
            url: config.url.clone(),
        }
    }
}

fn build_targets(config: &Manifest) -> Vec<TargetPlan> {
    let mut targets = Vec::new();
    targets.extend(config.sidecars.iter().map(|sidecar| {
        let plan = SidecarPlan::from_config(sidecar, &config.project);
        TargetPlan {
            name: plan.name,
            kind: TargetKind::Sidecar,
            command: plan.command,
            args: plan.args,
            cwd: plan.cwd,
            stamp: plan.stamp,
            stamp_via_env: plan.stamp_via_env,
            env: plan.env,
            endpoint_env: plan.endpoint_env,
            inherits_env: plan.inherits_env,
            inspect_socket: plan.inspect_socket,
            health_url: plan.health_url,
            ready: plan.ready,
        }
    }));
    if let Some(app) = &config.app {
        let plan = AppPlan::from_config(app, &config.project);
        targets.push(TargetPlan {
            name: plan.name,
            kind: TargetKind::App,
            command: plan.command,
            args: plan.args,
            cwd: plan.cwd,
            stamp: plan.stamp,
            stamp_via_env: plan.stamp_via_env,
            env: plan.env,
            endpoint_env: plan.endpoint_env,
            inherits_env: plan.inherits_env,
            inspect_socket: plan.inspect_socket,
            health_url: plan.health_url,
            ready: plan.ready,
        });
    }
    targets
}

fn expand_target_template(value: &str, project: &ProjectConfig, name: &str) -> String {
    value
        .replace("{project}", &project.name)
        .replace("{namespace}", &project.namespace)
        .replace("{name}", name)
}
