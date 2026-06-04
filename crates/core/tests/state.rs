use sidecar_core::DevState;
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

fn state_from(text: &str) -> DevState {
    DevState {
        config_path: PathBuf::from("inline.toml"),
        config: toml::from_str(text).unwrap(),
    }
}
