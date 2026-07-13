#![cfg(unix)]

use sidecar_core::Paths;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

static LOCK: Mutex<()> = Mutex::new(());

fn scoped<F: FnOnce()>(test: F) {
    let _guard = LOCK.lock().unwrap_or_else(|err| err.into_inner());
    let data = std::env::var_os("SIDECAR_DATA_HOME");
    let xdg = std::env::var_os("XDG_DATA_HOME");
    let home = std::env::var_os("HOME");
    std::env::remove_var("SIDECAR_DATA_HOME");
    std::env::remove_var("XDG_DATA_HOME");
    std::env::set_var("HOME", "/tmp/fake-home");
    test();
    match data {
        Some(v) => std::env::set_var("SIDECAR_DATA_HOME", v),
        None => std::env::remove_var("SIDECAR_DATA_HOME"),
    }
    match xdg {
        Some(v) => std::env::set_var("XDG_DATA_HOME", v),
        None => std::env::remove_var("XDG_DATA_HOME"),
    }
    match home {
        Some(v) => std::env::set_var("HOME", v),
        None => std::env::remove_var("HOME"),
    }
}

#[test]
fn cli() {
    scoped(|| {
        std::env::set_var("SIDECAR_DATA_HOME", "/from/env");
        let paths = Paths::resolve("default", Some(Path::new("/from/cli")), None);
        assert_eq!(paths.root, PathBuf::from("/from/cli"));
        assert_eq!(paths.state, PathBuf::from("/from/cli/state"));
        assert_eq!(paths.project, PathBuf::from("/from/cli/projects/default"));
    });
}

#[test]
fn env() {
    scoped(|| {
        std::env::set_var("SIDECAR_DATA_HOME", "/from/env");
        let paths = Paths::resolve("staging", None, None);
        assert_eq!(paths.root, PathBuf::from("/from/env"));
        assert_eq!(paths.project, PathBuf::from("/from/env/projects/staging"));
    });
}

#[test]
fn xdg() {
    scoped(|| {
        std::env::set_var("XDG_DATA_HOME", "/xdg/data");
        let paths = Paths::resolve("default", None, None);
        assert_eq!(paths.root, PathBuf::from("/xdg/data/sidecar"));
    });
}

#[test]
fn home() {
    scoped(|| {
        let paths = Paths::resolve("default", None, None);
        assert_eq!(
            paths.root,
            PathBuf::from("/tmp/fake-home/.local/share/sidecar")
        );
    });
}

#[test]
fn manifest() {
    scoped(|| {
        std::env::set_var("SIDECAR_DATA_HOME", "/from/env");
        let paths = Paths::resolve("default", None, Some("/elsewhere/proj"));
        assert_eq!(paths.root, PathBuf::from("/from/env"));
        assert_eq!(paths.state, PathBuf::from("/from/env/state"));
        assert_eq!(paths.project, PathBuf::from("/elsewhere/proj"));
    });
}
