use sidecar_core::inspect;

#[test]
fn ok() {
    let parsed = inspect::parse(
        "{\"kind\":\"event_response\",\"id\":\"req-1\",\"payload\":{\"answer\":42}}",
        "req-1",
    )
    .unwrap();
    match parsed {
        inspect::Response::Ok(value) => {
            assert_eq!(
                value.get("answer").and_then(serde_json::Value::as_i64),
                Some(42)
            );
        }
        other => panic!("expected ok response, got {other:?}"),
    }
}

#[test]
fn error() {
    let parsed = inspect::parse(
        "{\"kind\":\"event_error\",\"id\":\"req-1\",\"error\":{\"code\":\"boom\",\"message\":\"failed\"}}",
        "req-1",
    )
    .unwrap();
    match parsed {
        inspect::Response::Err(message) => assert_eq!(message, "boom: failed"),
        other => panic!("expected error response, got {other:?}"),
    }
}

#[test]
fn empty() {
    let err = inspect::parse("", "req-1").unwrap_err();
    assert!(err.contains("empty"));
}
