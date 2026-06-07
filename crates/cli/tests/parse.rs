use sidecar_cli::{cli_test, help_text};

#[test]
fn help_boundary() {
    let help = help_text();
    assert!(help.contains("Product-neutral sidecar lifecycle and inspect IPC manager."));
    assert!(help.contains("consumers own product semantics"));
    assert!(help.contains("doctor   [--config <path>]"));
    assert!(help.contains("inspect  <sidecar> <event> [<json-payload>]"));
    assert!(help.contains("--inspect-timeout <s>"));
    assert!(help.contains("when omitted, sidecar walks"));
    assert!(help.contains("like docker compose -p"));
    assert!(help.contains("--sidecar-stamp=a=<app>;n=<namespace>;m=<mode>;s=<source>"));
    assert!(help.contains("README.md for usage/schema"));
    assert!(help.contains("AGENTS.md for boundaries and PR workflow"));
    assert!(help.contains("Source:  https://github.com/PerishCode/sidecar"));
    assert!(help.contains("https://github.com/PerishCode/sidecar/issues"));
    assert!(help
        .contains("0 on success. 1 on config, diagnostic, lifecycle, inspect, or update failure."));
    assert!(!help.contains("%LOCALAPPDATA%"));
    assert!(!help.contains("fully recover"));
}

#[test]
fn global_config() {
    let parsed = cli_test::parse_args(vec![
        "sidecar",
        "doctor",
        "--config",
        "examples/minimal.toml",
        "--format=json",
    ])
    .unwrap();

    assert_eq!(parsed.command, vec!["doctor"]);
    assert_eq!(parsed.config.as_deref(), Some("examples/minimal.toml"));
    assert_eq!(parsed.format, "json");
}

#[test]
fn version_flags() {
    let parsed = cli_test::parse_args(vec!["sidecar", "--version"]).unwrap();
    assert_eq!(parsed.command, vec!["--version"]);
}

#[test]
fn inspect_payload() {
    let parsed = cli_test::parse_args(vec![
        "sidecar",
        "inspect",
        "controller",
        "host",
        "{\"window\":\"main\"}",
        "--config",
        "x.toml",
    ])
    .unwrap();
    assert_eq!(
        parsed.command,
        vec!["inspect", "controller", "host", "{\"window\":\"main\"}"]
    );
    assert_eq!(parsed.config.as_deref(), Some("x.toml"));
    assert_eq!(parsed.timeout_secs, 5);
}

#[test]
fn inspect_timeout() {
    let parsed = cli_test::parse_args(vec![
        "sidecar",
        "inspect",
        "controller",
        "accept.messaging",
        "{\"text\":\"hello\"}",
        "--inspect-timeout",
        "60",
        "--config",
        "x.toml",
    ])
    .unwrap();

    assert_eq!(parsed.timeout_secs, 60);
}

#[test]
fn timeout_zero() {
    let error = cli_test::parse_args(vec![
        "sidecar",
        "inspect",
        "controller",
        "runtime.snapshot",
        "--inspect-timeout=0",
        "--config",
        "x.toml",
    ])
    .unwrap_err();

    assert!(error.contains("--inspect-timeout requires a positive integer value"));
}

#[test]
fn project_flags() {
    let parsed = cli_test::parse_args(vec![
        "sidecar",
        "-p",
        "staging",
        "status",
        "--config=x.toml",
    ])
    .unwrap();
    assert_eq!(parsed.project.as_deref(), Some("staging"));

    let parsed = cli_test::parse_args(vec![
        "sidecar",
        "--project=prod",
        "list",
        "--config",
        "x.toml",
    ])
    .unwrap();
    assert_eq!(parsed.project.as_deref(), Some("prod"));
}

#[test]
fn reset_all() {
    let parsed = cli_test::parse_args(vec![
        "sidecar",
        "--data-home",
        "/var/sidecar",
        "reset",
        "--all",
        "--config",
        "x.toml",
    ])
    .unwrap();
    assert_eq!(parsed.data_home.as_deref(), Some("/var/sidecar"));
    assert!(parsed.reset_all);
}
