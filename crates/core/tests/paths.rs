#![cfg(unix)]

use sidecar_core::resolve_data_paths;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

static ENV_LOCK: Mutex<()> = Mutex::new(());

fn with_env<F: FnOnce()>(test: F) {
    let _guard = ENV_LOCK.lock().unwrap_or_else(|err| err.into_inner());
    let prev_data = std::env::var_os("SIDECAR_DATA_HOME");
    let prev_xdg = std::env::var_os("XDG_DATA_HOME");
    let prev_home = std::env::var_os("HOME");
    std::env::remove_var("SIDECAR_DATA_HOME");
    std::env::remove_var("XDG_DATA_HOME");
    std::env::set_var("HOME", "/tmp/fake-home");
    test();
    match prev_data {
        Some(v) => std::env::set_var("SIDECAR_DATA_HOME", v),
        None => std::env::remove_var("SIDECAR_DATA_HOME"),
    }
    match prev_xdg {
        Some(v) => std::env::set_var("XDG_DATA_HOME", v),
        None => std::env::remove_var("XDG_DATA_HOME"),
    }
    match prev_home {
        Some(v) => std::env::set_var("HOME", v),
        None => std::env::remove_var("HOME"),
    }
}

#[test]
fn cli_override() {
    with_env(|| {
        std::env::set_var("SIDECAR_DATA_HOME", "/from/env");
        let paths = resolve_data_paths("default", Some(Path::new("/from/cli")), None);
        assert_eq!(paths.root, PathBuf::from("/from/cli"));
        assert_eq!(paths.state, PathBuf::from("/from/cli/state"));
        assert_eq!(paths.project, PathBuf::from("/from/cli/projects/default"));
    });
}

#[test]
fn env_default() {
    with_env(|| {
        std::env::set_var("SIDECAR_DATA_HOME", "/from/env");
        let paths = resolve_data_paths("staging", None, None);
        assert_eq!(paths.root, PathBuf::from("/from/env"));
        assert_eq!(paths.project, PathBuf::from("/from/env/projects/staging"));
    });
}

#[test]
fn xdg_default() {
    with_env(|| {
        std::env::set_var("XDG_DATA_HOME", "/xdg/data");
        let paths = resolve_data_paths("default", None, None);
        assert_eq!(paths.root, PathBuf::from("/xdg/data/sidecar"));
    });
}

#[test]
fn home_default() {
    with_env(|| {
        let paths = resolve_data_paths("default", None, None);
        assert_eq!(
            paths.root,
            PathBuf::from("/tmp/fake-home/.local/share/sidecar")
        );
    });
}

#[test]
fn manifest_project_dir() {
    with_env(|| {
        std::env::set_var("SIDECAR_DATA_HOME", "/from/env");
        let paths = resolve_data_paths("default", None, Some("/elsewhere/proj"));
        assert_eq!(paths.root, PathBuf::from("/from/env"));
        assert_eq!(paths.state, PathBuf::from("/from/env/state"));
        assert_eq!(paths.project, PathBuf::from("/elsewhere/proj"));
    });
}
