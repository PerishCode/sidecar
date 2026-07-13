use std::env;
use std::path::{Path, PathBuf};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Paths {
    pub root: PathBuf,
    pub state: PathBuf,
    pub project: PathBuf,
}

impl Paths {
    pub fn resolve(namespace: &str, explicit: Option<&Path>, data: Option<&str>) -> Paths {
        let root = home(explicit);
        let state = root.join("state");
        let project = data
            .map(|value| PathBuf::from(value.replace("{namespace}", namespace)))
            .unwrap_or_else(|| root.join("projects").join(namespace));
        Paths {
            root,
            state,
            project,
        }
    }
}

pub fn home(explicit: Option<&Path>) -> PathBuf {
    if let Some(path) = explicit {
        return path.to_path_buf();
    }
    if let Some(value) = env::var_os("SIDECAR_DATA_HOME") {
        if !value.is_empty() {
            return PathBuf::from(value);
        }
    }
    fallback()
}

fn fallback() -> PathBuf {
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
