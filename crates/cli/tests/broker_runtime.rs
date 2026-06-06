use sidecar_cli::{broker_runtime_test, cli_test};

#[test]
fn parse_runtime_serve() {
    let parsed = cli_test::parse_args(vec![
        "sidecar",
        "runtime",
        "serve",
        "example",
        "default",
        "--sidecar-broker=p=example;n=default;s=tool%3Asidecar",
    ])
    .unwrap();

    assert_eq!(
        parsed.command,
        vec![
            "runtime",
            "serve",
            "example",
            "default",
            "--sidecar-broker=p=example;n=default;s=tool%3Asidecar"
        ]
    );
}

#[test]
fn broker_hello_round_trip() {
    let response = broker_runtime_test::round_trip(
        r#"{"kind":"hello","protocol":1,"project":"sidecar","namespace":"default"}"#,
    )
    .unwrap();

    assert_eq!(
        response.trim(),
        r#"{"kind":"hello_ok","protocol":1,"project":"sidecar","namespace":"default"}"#
    );
}

#[test]
fn hello_wrong_namespace() {
    let response = broker_runtime_test::round_trip(
        r#"{"kind":"hello","protocol":1,"project":"sidecar","namespace":"other"}"#,
    )
    .unwrap();

    assert!(response.contains(r#""kind":"hello_error""#));
}
