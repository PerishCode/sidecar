use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Manifest {
    pub project: ProjectConfig,
    #[serde(default)]
    pub app: Option<AppConfig>,
    #[serde(default)]
    pub sidecars: Vec<SidecarConfig>,
    #[serde(default)]
    pub inspect: InspectConfig,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ProjectConfig {
    pub name: String,
    #[serde(default = "default_namespace")]
    pub namespace: String,
    #[serde(default = "default_root")]
    pub root: String,
    #[serde(default)]
    pub data_dir: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct AppConfig {
    pub name: String,
    pub command: String,
    #[serde(default)]
    pub args: Vec<String>,
    #[serde(default = "default_root")]
    pub cwd: String,
    #[serde(default = "default_mode")]
    pub mode: String,
    #[serde(default)]
    pub env: BTreeMap<String, String>,
    #[serde(default)]
    pub inherits_env: Vec<InheritEnvConfig>,
    #[serde(default)]
    pub inspect_socket: Option<String>,
    #[serde(default)]
    pub health_url: Option<String>,
    #[serde(default)]
    pub ready: Option<ReadyConfig>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SidecarConfig {
    pub name: String,
    pub command: String,
    #[serde(default)]
    pub args: Vec<String>,
    #[serde(default = "default_root")]
    pub cwd: String,
    #[serde(default = "default_mode")]
    pub mode: String,
    #[serde(default)]
    pub env: BTreeMap<String, String>,
    #[serde(default)]
    pub inherits_env: Vec<InheritEnvConfig>,
    #[serde(default)]
    pub inspect_socket: Option<String>,
    #[serde(default)]
    pub health_url: Option<String>,
    #[serde(default)]
    pub ready: Option<ReadyConfig>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ReadyConfig {
    pub role: String,
    #[serde(default = "default_ready_timeout_secs")]
    pub timeout_secs: u64,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct InheritEnvConfig {
    pub name: String,
    pub from: String,
}

#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct InspectConfig {
    #[serde(default)]
    pub endpoints: Vec<InspectEndpointConfig>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct InspectEndpointConfig {
    pub name: String,
    pub kind: String,
    pub url: String,
}

fn default_root() -> String {
    ".".to_string()
}

fn default_namespace() -> String {
    crate::stamp::DEFAULT_NAMESPACE.to_string()
}

fn default_mode() -> String {
    crate::stamp::DEFAULT_MODE.to_string()
}

fn default_ready_timeout_secs() -> u64 {
    120
}
