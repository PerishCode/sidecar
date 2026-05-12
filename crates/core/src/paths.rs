//! Resolves the global sidecar data home and project-scoped paths.
//!
//! Layout:
//! ```text
//! <data_home>/
//! ├── state/                 (global; e.g. update cache)
//! └── projects/<namespace>/  (per-project, isolated by stamp namespace)
//! ```
//!
//! Override precedence (highest wins):
//! 1. `cli_data_home` argument (e.g. `--data-home <path>`)
//! 2. env `SIDECAR_DATA_HOME`
//! 3. platform default (`$XDG_DATA_HOME/sidecar` → `$HOME/.local/share/sidecar`,
//!    or `%LOCALAPPDATA%\sidecar` on Windows)
//!
//! For the per-project subdir, an explicit manifest `[project].data_dir`
//! field replaces `<data_home>/projects/<namespace>` entirely (it does
//! not affect `state/`).

use std::env;
use std::path::{Path, PathBuf};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DataPaths {
    /// `<data_home>` root. May be a synthesized fallback if no HOME / LOCALAPPDATA exists.
    pub root: PathBuf,
    /// `<data_home>/state` — shared, namespace-independent.
    pub state: PathBuf,
    /// `<data_home>/projects/<namespace>` (or manifest `data_dir` override).
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

#[cfg(test)]
mod tests {
    #[cfg(unix)]
    use super::*;
    #[cfg(unix)]
    use std::sync::Mutex;

    #[cfg(unix)]
    static ENV_LOCK: Mutex<()> = Mutex::new(());

    #[cfg(unix)]
    fn with_clean_env<F: FnOnce()>(test: F) {
        let _guard = ENV_LOCK.lock().unwrap_or_else(|err| err.into_inner());
        let prev_data = env::var_os("SIDECAR_DATA_HOME");
        let prev_xdg = env::var_os("XDG_DATA_HOME");
        let prev_home = env::var_os("HOME");
        env::remove_var("SIDECAR_DATA_HOME");
        env::remove_var("XDG_DATA_HOME");
        env::set_var("HOME", "/tmp/fake-home");
        test();
        match prev_data {
            Some(v) => env::set_var("SIDECAR_DATA_HOME", v),
            None => env::remove_var("SIDECAR_DATA_HOME"),
        }
        match prev_xdg {
            Some(v) => env::set_var("XDG_DATA_HOME", v),
            None => env::remove_var("XDG_DATA_HOME"),
        }
        match prev_home {
            Some(v) => env::set_var("HOME", v),
            None => env::remove_var("HOME"),
        }
    }

    #[test]
    #[cfg(unix)]
    fn cli_override_wins() {
        with_clean_env(|| {
            env::set_var("SIDECAR_DATA_HOME", "/from/env");
            let paths = resolve_data_paths("default", Some(Path::new("/from/cli")), None);
            assert_eq!(paths.root, PathBuf::from("/from/cli"));
            assert_eq!(paths.state, PathBuf::from("/from/cli/state"));
            assert_eq!(paths.project, PathBuf::from("/from/cli/projects/default"));
        });
    }

    #[test]
    #[cfg(unix)]
    fn env_beats_default() {
        with_clean_env(|| {
            env::set_var("SIDECAR_DATA_HOME", "/from/env");
            let paths = resolve_data_paths("staging", None, None);
            assert_eq!(paths.root, PathBuf::from("/from/env"));
            assert_eq!(paths.project, PathBuf::from("/from/env/projects/staging"));
        });
    }

    #[test]
    #[cfg(unix)]
    fn xdg_default_used_when_set() {
        with_clean_env(|| {
            env::set_var("XDG_DATA_HOME", "/xdg/data");
            let paths = resolve_data_paths("default", None, None);
            assert_eq!(paths.root, PathBuf::from("/xdg/data/sidecar"));
        });
    }

    #[test]
    #[cfg(unix)]
    fn home_default_used_without_xdg() {
        with_clean_env(|| {
            let paths = resolve_data_paths("default", None, None);
            assert_eq!(
                paths.root,
                PathBuf::from("/tmp/fake-home/.local/share/sidecar")
            );
        });
    }

    #[test]
    #[cfg(unix)]
    fn manifest_data_dir_replaces_project_subdir_only() {
        with_clean_env(|| {
            env::set_var("SIDECAR_DATA_HOME", "/from/env");
            let paths = resolve_data_paths("default", None, Some("/elsewhere/proj"));
            assert_eq!(paths.root, PathBuf::from("/from/env"));
            assert_eq!(paths.state, PathBuf::from("/from/env/state"));
            assert_eq!(paths.project, PathBuf::from("/elsewhere/proj"));
        });
    }
}
