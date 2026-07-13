use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Manifest {
    pub project: Project,
    #[serde(default)]
    pub app: Option<App>,
    #[serde(default)]
    pub sidecars: Vec<Sidecar>,
    #[serde(default)]
    pub inspect: Inspect,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Project {
    pub name: String,
    #[serde(default = "default::namespace")]
    pub namespace: String,
    #[serde(default = "default::root")]
    pub root: String,
    #[serde(default, rename = "data_dir")]
    pub data: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct App {
    pub name: String,
    pub command: String,
    #[serde(default)]
    pub args: Vec<String>,
    #[serde(default = "default::root")]
    pub cwd: String,
    #[serde(default = "default::mode")]
    pub mode: String,
    #[serde(default)]
    pub env: BTreeMap<String, String>,
    #[serde(default, rename = "inherits_env")]
    pub inherits: Vec<Inherit>,
    #[serde(default, rename = "inspect_socket")]
    pub socket: Option<String>,
    #[serde(default, rename = "health_url")]
    pub health: Option<String>,
    #[serde(default)]
    pub ready: Option<Ready>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Sidecar {
    pub name: String,
    pub command: String,
    #[serde(default)]
    pub args: Vec<String>,
    #[serde(default = "default::root")]
    pub cwd: String,
    #[serde(default = "default::mode")]
    pub mode: String,
    #[serde(default)]
    pub env: BTreeMap<String, String>,
    #[serde(default, rename = "inherits_env")]
    pub inherits: Vec<Inherit>,
    #[serde(default, rename = "inspect_socket")]
    pub socket: Option<String>,
    #[serde(default, rename = "health_url")]
    pub health: Option<String>,
    #[serde(default)]
    pub ready: Option<Ready>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Ready {
    pub role: String,
    #[serde(default = "default::timeout", rename = "timeout_secs")]
    pub timeout: u64,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Inherit {
    pub name: String,
    pub from: String,
}

#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Inspect {
    #[serde(default)]
    pub endpoints: Vec<Endpoint>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Endpoint {
    pub name: String,
    pub kind: String,
    pub url: String,
}

mod default {
    pub(super) fn root() -> String {
        ".".to_string()
    }

    pub(super) fn namespace() -> String {
        crate::stamp::default::NAMESPACE.to_string()
    }

    pub(super) fn mode() -> String {
        crate::stamp::default::MODE.to_string()
    }

    pub(super) fn timeout() -> u64 {
        120
    }
}
