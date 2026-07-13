use sidecar_core::{Manifest, State};
use std::path::PathBuf;

#[test]
fn duplicates() {
    let state = seed(
        r#"
        [project]
        name = "app"

        [[sidecars]]
        name = "api"
        command = "cargo"

        [[sidecars]]
        name = "api"
        command = "cargo"
        "#,
    );

    let diagnostics = state.diagnostics();
    assert!(diagnostics
        .iter()
        .any(|diagnostic| diagnostic.message.contains("duplicate sidecar name")));
}

#[test]
fn optional() {
    let state = seed(
        r#"
        [project]
        name = "app"

        [[sidecars]]
        name = "api"
        command = "cargo"
        "#,
    );
    let diagnostics = state.diagnostics();
    assert!(!diagnostics
        .iter()
        .any(|diagnostic| diagnostic.path == "sidecars[0].inspect_socket"));
}

#[test]
fn solo() {
    let state = seed(
        r#"
        [project]
        name = "cells"

        [[sidecars]]
        name = "server"
        command = "cargo"
        args = ["run", "--quiet", "-p", "server-cell", "--"]
        "#,
    );

    let diagnostics = state.diagnostics();
    assert!(!diagnostics
        .iter()
        .any(|diagnostic| diagnostic.path == "app"));
}

#[test]
fn empty() {
    let state = seed(
        r#"
        [project]
        name = "empty"
        "#,
    );

    let diagnostics = state.diagnostics();
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic.path == "app" && diagnostic.message.contains("no app or sidecar command")
    }));
}

#[test]
fn warned() {
    let state = seed(
        r#"
        [project]
        name = "cells"

        [[sidecars]]
        name = "server"
        command = "cargo"
        args = ["run", "--quiet", "-p", "server-cell"]
        "#,
    );

    let diagnostics = state.diagnostics();
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic.path == "sidecars[0].args" && diagnostic.message.contains("--sidecar-stamp")
    }));
}

#[test]
fn separated() {
    let state = seed(
        r#"
        [project]
        name = "cells"

        [[sidecars]]
        name = "server"
        command = "cargo"
        args = ["run", "--quiet", "-p", "server-cell", "--"]
        "#,
    );

    let diagnostics = state.diagnostics();
    assert!(!diagnostics
        .iter()
        .any(|diagnostic| diagnostic.message.contains("--sidecar-stamp")));
}

#[test]
fn legacy() {
    let err = toml::from_str::<Manifest>(
        r#"
        [project]
        name = "legacy"

        [[sidecars]]
        name = "server"
        command = "cargo"
        stamp_via_env = true
        endpoint_env = "SIDECAR_RUNTIME_ENDPOINT"
        "#,
    )
    .unwrap_err()
    .to_string();

    assert!(err.contains("unknown field"));
}

#[test]
fn planned() {
    let state = seed(
        r#"
        [project]
        name = "app"

        [app]
        name = "desktop"
        command = "pnpm"
        args = ["tauri", "dev"]

        [[inspect.endpoints]]
        name = "health"
        kind = "http"
        url = "http://127.0.0.1:3000/health"
        "#,
    );

    let plan = state.plan();
    assert_eq!(plan.project, "app");
    assert_eq!(plan.namespace, "default");
    assert_eq!(plan.app.unwrap().command, "pnpm");
    assert_eq!(plan.targets.len(), 1);
    assert_eq!(plan.endpoints.len(), 1);
}

#[test]
fn endpoint() {
    let state = seed(
        r#"
        [project]
        name = "app"

        [[sidecars]]
        name = "api"
        command = "cargo"
        "#,
    );

    let plan = state.plan();
    let args = plan.targets[0].launch("tcp://127.0.0.1:4100");
    let stamp = args
        .iter()
        .find(|arg| arg.starts_with("--sidecar-stamp="))
        .expect("stamp arg should exist");
    assert!(stamp.contains("v=1;"));
    assert!(stamp.contains(";e=tcp%3A%2F%2F127.0.0.1%3A4100"));
}

fn seed(text: &str) -> State {
    State {
        path: PathBuf::from("inline.toml"),
        config: toml::from_str(text).unwrap(),
    }
}
