use std::collections::BTreeMap;

use crate::config;
use crate::config::Manifest;
use crate::stamp;
use crate::stamp::Stamp;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Plan {
    pub project: String,
    pub namespace: String,
    pub root: String,
    pub app: Option<App>,
    pub sidecars: Vec<Sidecar>,
    pub targets: Vec<Target>,
    pub endpoints: Vec<Endpoint>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct App {
    pub name: String,
    pub command: String,
    pub args: Vec<String>,
    pub cwd: String,
    pub stamp: Stamp,
    pub env: BTreeMap<String, String>,
    pub inherits: Vec<Inherit>,
    pub socket: Option<String>,
    pub health: Option<String>,
    pub ready: Option<Ready>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Sidecar {
    pub name: String,
    pub command: String,
    pub args: Vec<String>,
    pub cwd: String,
    pub stamp: Stamp,
    pub env: BTreeMap<String, String>,
    pub inherits: Vec<Inherit>,
    pub socket: Option<String>,
    pub health: Option<String>,
    pub ready: Option<Ready>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Target {
    pub name: String,
    pub kind: Kind,
    pub command: String,
    pub args: Vec<String>,
    pub cwd: String,
    pub stamp: Stamp,
    pub env: BTreeMap<String, String>,
    pub inherits: Vec<Inherit>,
    pub socket: Option<String>,
    pub health: Option<String>,
    pub ready: Option<Ready>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Kind {
    App,
    Sidecar,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Ready {
    pub role: String,
    pub timeout: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Inherit {
    pub name: String,
    pub from: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Endpoint {
    pub name: String,
    pub kind: String,
    pub url: String,
}

impl Manifest {
    pub fn plan(&self) -> Plan {
        Plan {
            project: self.project.name.clone(),
            namespace: self.project.namespace.clone(),
            root: self.project.root.clone(),
            app: self.app.as_ref().map(|app| app.plan(&self.project)),
            sidecars: self
                .sidecars
                .iter()
                .map(|sidecar| sidecar.plan(&self.project))
                .collect(),
            targets: self.targets(),
            endpoints: self
                .inspect
                .endpoints
                .iter()
                .map(config::Endpoint::plan)
                .collect(),
        }
    }

    fn targets(&self) -> Vec<Target> {
        let mut targets = Vec::new();
        targets.extend(self.sidecars.iter().map(|sidecar| {
            let plan = sidecar.plan(&self.project);
            Target {
                name: plan.name,
                kind: Kind::Sidecar,
                command: plan.command,
                args: plan.args,
                cwd: plan.cwd,
                stamp: plan.stamp,
                env: plan.env,
                inherits: plan.inherits,
                socket: plan.socket,
                health: plan.health,
                ready: plan.ready,
            }
        }));
        if let Some(app) = &self.app {
            let plan = app.plan(&self.project);
            targets.push(Target {
                name: plan.name,
                kind: Kind::App,
                command: plan.command,
                args: plan.args,
                cwd: plan.cwd,
                stamp: plan.stamp,
                env: plan.env,
                inherits: plan.inherits,
                socket: plan.socket,
                health: plan.health,
                ready: plan.ready,
            });
        }
        targets
    }
}

impl config::App {
    fn plan(&self, project: &config::Project) -> App {
        let stamp = Stamp {
            version: stamp::VERSION,
            app: self.name.clone(),
            namespace: project.namespace.clone(),
            mode: self.mode.clone(),
            source: stamp::default::SOURCE.to_string(),
            endpoint: None,
        };
        App {
            name: self.name.clone(),
            command: self.command.clone(),
            args: self.args.clone(),
            cwd: self.cwd.clone(),
            stamp,
            env: self.env.clone(),
            inherits: self.inherits.iter().map(config::Inherit::plan).collect(),
            socket: self
                .socket
                .as_ref()
                .map(|value| expand(value, project, &self.name)),
            health: self.health.clone(),
            ready: self.ready.as_ref().map(config::Ready::plan),
        }
    }
}

impl config::Sidecar {
    fn plan(&self, project: &config::Project) -> Sidecar {
        let stamp = Stamp {
            version: stamp::VERSION,
            app: self.name.clone(),
            namespace: project.namespace.clone(),
            mode: self.mode.clone(),
            source: stamp::default::SOURCE.to_string(),
            endpoint: None,
        };
        Sidecar {
            name: self.name.clone(),
            command: self.command.clone(),
            args: self.args.clone(),
            cwd: self.cwd.clone(),
            stamp,
            env: self.env.clone(),
            inherits: self.inherits.iter().map(config::Inherit::plan).collect(),
            socket: self
                .socket
                .as_ref()
                .map(|value| expand(value, project, &self.name)),
            health: self.health.clone(),
            ready: self.ready.as_ref().map(config::Ready::plan),
        }
    }
}

impl config::Ready {
    fn plan(&self) -> Ready {
        Ready {
            role: self.role.clone(),
            timeout: self.timeout,
        }
    }
}

impl config::Inherit {
    fn plan(&self) -> Inherit {
        Inherit {
            name: self.name.clone(),
            from: self.from.clone(),
        }
    }
}

impl config::Endpoint {
    fn plan(&self) -> Endpoint {
        Endpoint {
            name: self.name.clone(),
            kind: self.kind.clone(),
            url: self.url.clone(),
        }
    }
}

impl App {
    pub fn argv(&self) -> Vec<String> {
        let mut argv = self.args.clone();
        argv.extend(self.stamp.args());
        argv
    }
}

impl Sidecar {
    pub fn argv(&self) -> Vec<String> {
        let mut argv = self.args.clone();
        argv.extend(self.stamp.args());
        argv
    }
}

impl Target {
    pub fn argv(&self) -> Vec<String> {
        let mut argv = self.args.clone();
        argv.extend(self.stamp.args());
        argv
    }

    pub fn launch(&self, endpoint: &str) -> Vec<String> {
        let mut argv = self.args.clone();
        argv.extend(self.stamp.at(endpoint).args());
        argv
    }
}

fn expand(value: &str, project: &config::Project, name: &str) -> String {
    value
        .replace("{project}", &project.name)
        .replace("{namespace}", &project.namespace)
        .replace("{name}", name)
}
