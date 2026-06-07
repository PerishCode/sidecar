use sidecar_core::{DevState, Manifest};
use std::path::PathBuf;

#[test]
fn duplicate_sidecars() {
    let state = state_from(
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
fn inspect_optional() {
    let state = state_from(
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
fn sidecar_only_ok() {
    let state = state_from(
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
fn empty_warns() {
    let state = state_from(
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
fn cargo_run_warns() {
    let state = state_from(
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
fn cargo_run_ok() {
    let state = state_from(
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
fn legacy_env_fields_rejected() {
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
fn execution_plan() {
    let state = state_from(
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

    let plan = state.execution_plan();
    assert_eq!(plan.project, "app");
    assert_eq!(plan.namespace, "default");
    assert_eq!(plan.app.unwrap().command, "pnpm");
    assert_eq!(plan.targets.len(), 1);
    assert_eq!(plan.inspect_endpoints.len(), 1);
}

#[test]
fn endpoint_in_stamp_args() {
    let state = state_from(
        r#"
        [project]
        name = "app"

        [[sidecars]]
        name = "api"
        command = "cargo"
        "#,
    );

    let plan = state.execution_plan();
    let args = plan.targets[0].spawn_args_with_endpoint("tcp://127.0.0.1:4100");
    let stamp = args
        .iter()
        .find(|arg| arg.starts_with("--sidecar-stamp="))
        .expect("stamp arg should exist");
    assert!(stamp.contains("v=1;"));
    assert!(stamp.contains(";e=tcp%3A%2F%2F127.0.0.1%3A4100"));
}

fn state_from(text: &str) -> DevState {
    DevState {
        config_path: PathBuf::from("inline.toml"),
        config: toml::from_str(text).unwrap(),
    }
}
