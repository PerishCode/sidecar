//! Inspect IPC bridge: connect to a sidecar's inspect socket and exchange a
//! single SidecarRuntime event frame.
//!
//! Wire format (one line per direction):
//!   request:  `{"kind":"event","id":"...","verb":"...","payload":<json>}\n`
//!   response: `{"kind":"event_response","id":"...","payload":<json>}\n`
//!          or `{"kind":"event_error","id":"...","error":{"code":"...","message":"..."}}\n`

use crate::socket::SocketEndpoint;
use serde_json::Value;
use std::io::{BufRead, BufReader, Write};
use std::net::TcpStream;
#[cfg(unix)]
use std::os::unix::net::UnixStream;
use std::time::Duration;

#[derive(Clone, Debug)]
pub struct InspectRequest {
    pub event: String,
    pub payload: Value,
}

#[derive(Clone, Debug)]
pub enum InspectResponse {
    Ok(Value),
    Err(String),
}

pub fn send(
    endpoint: &SocketEndpoint,
    request: &InspectRequest,
    timeout: Option<Duration>,
) -> Result<InspectResponse, String> {
    let id = next_event_id();
    let mut line = serde_json::to_string(&serde_json::json!({
        "kind": "event",
        "id": id,
        "verb": request.event,
        "payload": request.payload,
    }))
    .map_err(|err| err.to_string())?;
    line.push('\n');

    let raw = match endpoint {
        SocketEndpoint::Unix(path) => unix_round_trip(path, &line, timeout)?,
        SocketEndpoint::Tcp(address) => tcp_round_trip(address, &line, timeout)?,
    };
    parse_response(&raw, &id)
}

fn parse_response(text: &str, expected_id: &str) -> Result<InspectResponse, String> {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return Err("inspect endpoint returned empty response".to_string());
    }
    let value: Value = serde_json::from_str(trimmed).map_err(|err| err.to_string())?;
    let kind = value
        .get("kind")
        .and_then(Value::as_str)
        .ok_or_else(|| "inspect response missing kind".to_string())?;
    let id = value.get("id").and_then(Value::as_str).unwrap_or_default();
    if id != expected_id {
        return Err(format!(
            "inspect response id mismatch: expected {expected_id}, got {id}"
        ));
    }

    match kind {
        "event_response" => Ok(InspectResponse::Ok(
            value.get("payload").cloned().unwrap_or(Value::Null),
        )),
        "event_error" => {
            let error = value
                .get("error")
                .map(format_event_error)
                .unwrap_or_else(|| "inspect endpoint returned event_error".to_string());
            Ok(InspectResponse::Err(error))
        }
        other => Err(format!(
            "expected event_response/event_error inspect frame, got {other}"
        )),
    }
}

fn format_event_error(value: &Value) -> String {
    let code = value
        .get("code")
        .and_then(Value::as_str)
        .unwrap_or("event_error");
    let message = value
        .get("message")
        .and_then(Value::as_str)
        .unwrap_or("inspect endpoint returned event_error");
    format!("{code}: {message}")
}

#[cfg(unix)]
fn unix_round_trip(
    path: &std::path::PathBuf,
    line: &str,
    timeout: Option<Duration>,
) -> Result<String, String> {
    let mut stream = UnixStream::connect(path).map_err(|err| err.to_string())?;
    if let Some(timeout) = timeout {
        let _ = stream.set_read_timeout(Some(timeout));
        let _ = stream.set_write_timeout(Some(timeout));
    }
    stream
        .write_all(line.as_bytes())
        .map_err(|err| err.to_string())?;
    let mut reader = BufReader::new(stream);
    let mut response = String::new();
    reader
        .read_line(&mut response)
        .map_err(|err| err.to_string())?;
    Ok(response)
}

#[cfg(not(unix))]
fn unix_round_trip(
    _path: &std::path::PathBuf,
    _line: &str,
    _timeout: Option<Duration>,
) -> Result<String, String> {
    Err("unix inspect transport is not available on this platform".to_string())
}

fn tcp_round_trip(address: &str, line: &str, timeout: Option<Duration>) -> Result<String, String> {
    let mut stream = TcpStream::connect(address).map_err(|err| err.to_string())?;
    if let Some(timeout) = timeout {
        let _ = stream.set_read_timeout(Some(timeout));
        let _ = stream.set_write_timeout(Some(timeout));
    }
    stream
        .write_all(line.as_bytes())
        .map_err(|err| err.to_string())?;
    let mut reader = BufReader::new(stream);
    let mut response = String::new();
    reader
        .read_line(&mut response)
        .map_err(|err| err.to_string())?;
    Ok(response)
}

fn next_event_id() -> String {
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::time::{SystemTime, UNIX_EPOCH};
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let count = COUNTER.fetch_add(1, Ordering::Relaxed);
    let micros = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_micros())
        .unwrap_or(0);
    format!("{micros}-{count}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_ok_response() {
        let parsed = parse_response(
            "{\"kind\":\"event_response\",\"id\":\"req-1\",\"payload\":{\"answer\":42}}",
            "req-1",
        )
        .unwrap();
        match parsed {
            InspectResponse::Ok(value) => {
                assert_eq!(value.get("answer").and_then(Value::as_i64), Some(42));
            }
            other => panic!("expected ok response, got {other:?}"),
        }
    }

    #[test]
    fn parses_error_response() {
        let parsed = parse_response(
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
    fn rejects_empty_response() {
        let err = parse_response("", "req-1").unwrap_err();
        assert!(err.contains("empty"));
    }
}
