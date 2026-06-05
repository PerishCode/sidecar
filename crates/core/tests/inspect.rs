use sidecar_core::inspect::{parse_response_for_test, InspectResponse};

#[test]
fn ok_response() {
    let parsed = parse_response_for_test(
        "{\"kind\":\"event_response\",\"id\":\"req-1\",\"payload\":{\"answer\":42}}",
        "req-1",
    )
    .unwrap();
    match parsed {
        InspectResponse::Ok(value) => {
            assert_eq!(
                value.get("answer").and_then(serde_json::Value::as_i64),
                Some(42)
            );
        }
        other => panic!("expected ok response, got {other:?}"),
    }
}

#[test]
fn error_response() {
    let parsed = parse_response_for_test(
        "{\"kind\":\"event_error\",\"id\":\"req-1\",\"error\":{\"code\":\"boom\",\"message\":\"failed\"}}",
        "req-1",
    )
    .unwrap();
    match parsed {
        InspectResponse::Err(message) => assert_eq!(message, "boom: failed"),
        other => panic!("expected error response, got {other:?}"),
    }
}

#[test]
fn empty_response() {
    let err = parse_response_for_test("", "req-1").unwrap_err();
    assert!(err.contains("empty"));
}
