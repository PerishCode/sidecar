use sidecar_cli::test::{broker, cli};

#[test]
fn serve() {
    let parsed = cli::parse(vec![
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
fn hello() {
    let response = broker::exchange(
        r#"{"kind":"hello","protocol":1,"project":"sidecar","namespace":"default"}"#,
    )
    .unwrap();

    assert_eq!(
        response.trim(),
        r#"{"kind":"hello_ok","protocol":1,"project":"sidecar","namespace":"default"}"#
    );
}

#[test]
fn mismatch() {
    let response = broker::exchange(
        r#"{"kind":"hello","protocol":1,"project":"sidecar","namespace":"other"}"#,
    )
    .unwrap();

    assert!(response.contains(r#""kind":"hello_error""#));
}
