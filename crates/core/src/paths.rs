use std::env;
use std::path::{Path, PathBuf};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DataPaths {
    pub root: PathBuf,
    pub state: PathBuf,
    pub project: PathBuf,
}

pub fn resolve_data_paths(
    namespace: &str,
    cli_data_home: Option<&Path>,
    manifest_data_dir: Option<&str>,
) -> DataPaths {
    let root = resolve_data_home(cli_data_home);
    let state = root.join("state");
    let project = manifest_data_dir
        .map(|value| PathBuf::from(value.replace("{namespace}", namespace)))
        .unwrap_or_else(|| root.join("projects").join(namespace));
    DataPaths {
        root,
        state,
        project,
    }
}

pub fn resolve_data_home(cli_override: Option<&Path>) -> PathBuf {
    if let Some(path) = cli_override {
        return path.to_path_buf();
    }
    if let Some(value) = env::var_os("SIDECAR_DATA_HOME") {
        if !value.is_empty() {
            return PathBuf::from(value);
        }
    }
    default_data_home()
}

fn default_data_home() -> PathBuf {
    if cfg!(windows) {
        if let Some(local) = env::var_os("LOCALAPPDATA") {
            return PathBuf::from(local).join("sidecar");
        }
        if let Some(profile) = env::var_os("USERPROFILE") {
            return PathBuf::from(profile).join("AppData/Local/sidecar");
        }
        return PathBuf::from("sidecar");
    }
    if let Some(xdg) = env::var_os("XDG_DATA_HOME") {
        if !xdg.is_empty() {
            return PathBuf::from(xdg).join("sidecar");
        }
    }
    if let Some(home) = env::var_os("HOME") {
        return PathBuf::from(home).join(".local/share/sidecar");
    }
    PathBuf::from("sidecar")
}
