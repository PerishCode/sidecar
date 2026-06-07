use sidecar_core::process::{filter_brokers_for_test, filter_for_test, parse_ps_output};

#[test]
fn parse_ps() {
    let text =
        "  123 cargo run --sidecar-stamp=v=1;a=api;n=default;m=dev;s=tool%3Asidecar\n  456 node server.js\n";
    let parsed = parse_ps_output(text);
    assert_eq!(parsed.len(), 2);
    assert_eq!(
        parsed[0],
        (
            123,
            "cargo run --sidecar-stamp=v=1;a=api;n=default;m=dev;s=tool%3Asidecar".into()
        )
    );
    assert_eq!(parsed[1], (456, "node server.js".into()));
}

#[test]
fn filter_stamp() {
    let rows = vec![
        (
            10,
            "controller --sidecar-stamp=v=1;a=controller;n=default;m=dev;s=tool%3Asidecar".into(),
        ),
        (
            11,
            "renderer --sidecar-stamp=v=1;a=renderer;n=default;m=dev;s=tool%3Asidecar".into(),
        ),
        (12, "noise --no-stamp".into()),
    ];
    let hits = filter_for_test(rows, "controller", "default");
    assert_eq!(hits.len(), 1);
    assert_eq!(hits[0].pid, 10);
}

#[test]
fn filter_broker() {
    let rows = vec![
        (
            10,
            "sidecar runtime serve --sidecar-broker=p=local;n=default;s=tool%3Asidecar".into(),
        ),
        (
            11,
            "sidecar runtime serve --sidecar-broker=p=local;n=other;s=tool%3Asidecar".into(),
        ),
        (
            12,
            "controller --sidecar-stamp=v=1;a=controller;n=default;m=dev;s=tool%3Asidecar".into(),
        ),
    ];
    let hits = filter_brokers_for_test(rows, "local", "default");
    assert_eq!(hits.len(), 1);
    assert_eq!(hits[0].pid, 10);
}
