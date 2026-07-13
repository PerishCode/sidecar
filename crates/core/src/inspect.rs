use crate::socket;
use serde_json::Value;
use std::io::{BufRead, BufReader, Write};
use std::net::TcpStream;
#[cfg(unix)]
use std::os::unix::net::UnixStream;
use std::time::Duration;

#[derive(Clone, Debug)]
pub struct Request {
    pub event: String,
    pub payload: Value,
}

#[derive(Clone, Debug)]
pub enum Response {
    Ok(Value),
    Err(String),
}

pub fn send(
    endpoint: &socket::Endpoint,
    request: &Request,
    timeout: Option<Duration>,
) -> Result<Response, String> {
    let id = id();
    let mut line = serde_json::to_string(&serde_json::json!({
        "kind": "event",
        "id": id,
        "verb": request.event,
        "payload": request.payload,
    }))
    .map_err(|err| err.to_string())?;
    line.push('\n');

    let raw = match endpoint {
        socket::Endpoint::Unix(path) => unix(path, &line, timeout)?,
        socket::Endpoint::Tcp(address) => tcp(address, &line, timeout)?,
    };
    parse(&raw, &id)
}

#[doc(hidden)]
pub fn parse(text: &str, expected: &str) -> Result<Response, String> {
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
    if id != expected {
        return Err(format!(
            "inspect response id mismatch: expected {expected}, got {id}"
        ));
    }

    match kind {
        "event_response" => Ok(Response::Ok(
            value.get("payload").cloned().unwrap_or(Value::Null),
        )),
        "event_error" => {
            let error = value
                .get("error")
                .map(describe)
                .unwrap_or_else(|| "inspect endpoint returned event_error".to_string());
            Ok(Response::Err(error))
        }
        other => Err(format!(
            "expected event_response/event_error inspect frame, got {other}"
        )),
    }
}

fn describe(value: &Value) -> String {
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
fn unix(
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
fn unix(
    _path: &std::path::PathBuf,
    _line: &str,
    _timeout: Option<Duration>,
) -> Result<String, String> {
    Err("unix inspect transport is not available on this platform".to_string())
}

fn tcp(address: &str, line: &str, timeout: Option<Duration>) -> Result<String, String> {
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

fn id() -> String {
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
