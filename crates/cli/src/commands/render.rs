use super::runtime::Status;
use crate::cli::Format;
use serde_json::{Map, Value};
use sidecar_core::inspect;
use sidecar_core::process::Stamped;

pub(super) fn status(
    namespace: &str,
    rows: &[(String, Vec<u32>)],
    broker: &Status,
    format: Format,
) -> Result<(), String> {
    match format {
        Format::Text => text::status(namespace, rows, broker),
        Format::Json => json::status(namespace, rows, broker),
    }
}

pub(super) fn list(
    namespace: &str,
    hits: &[Stamped],
    broker: &Status,
    state: &Map<String, Value>,
    format: Format,
) -> Result<(), String> {
    match format {
        Format::Text => text::list(namespace, hits, broker, state),
        Format::Json => json::list(namespace, hits, broker, state),
    }
}

pub(super) fn inspect(
    sidecar: &str,
    event: &str,
    response: &inspect::Response,
    format: Format,
) -> Result<(), String> {
    match format {
        Format::Text => text::inspect(sidecar, event, response),
        Format::Json => json::inspect(sidecar, event, response),
    }
}

mod text {
    use super::Status;
    use serde_json::{Map, Value};
    use sidecar_core::inspect;
    use sidecar_core::process::Stamped;

    pub(super) fn status(
        namespace: &str,
        rows: &[(String, Vec<u32>)],
        broker: &Status,
    ) -> Result<(), String> {
        println!("namespace: {namespace}");
        runtime(broker);
        for (name, pids) in rows {
            if let Some(first) = pids.first() {
                println!("- {name}: running (pid {})", first);
                for extra in pids.iter().skip(1) {
                    println!("  + duplicate (pid {})", extra);
                }
            } else {
                println!("- {name}: stopped");
            }
        }
        Ok(())
    }

    pub(super) fn list(
        namespace: &str,
        hits: &[Stamped],
        broker: &Status,
        state: &Map<String, Value>,
    ) -> Result<(), String> {
        println!("namespace: {namespace}");
        runtime(broker);
        if hits.is_empty() {
            println!("no stamped processes");
        }
        for hit in hits {
            println!("- pid={} cmd={}", hit.pid, hit.command);
        }
        for (name, entry) in state {
            if let Some(pid) = entry.get("pid").and_then(Value::as_u64) {
                println!("- target={name} pid={pid} source=state");
            }
        }
        Ok(())
    }

    fn runtime(broker: &Status) {
        match (&broker.endpoint, broker.pids.first()) {
            (Some(endpoint), Some(pid)) => println!("runtime: running (pid {pid}) {endpoint}"),
            (None, Some(pid)) => println!("runtime: starting or stale (pid {pid})"),
            _ => println!("runtime: stopped"),
        }
    }

    pub(super) fn inspect(
        sidecar: &str,
        event: &str,
        response: &inspect::Response,
    ) -> Result<(), String> {
        match response {
            inspect::Response::Ok(value) => {
                println!("ok {sidecar} {event}");
                println!(
                    "{}",
                    serde_json::to_string_pretty(value).unwrap_or_default()
                );
                Ok(())
            }
            inspect::Response::Err(message) => Err(format!("inspect error: {message}")),
        }
    }
}

mod json {
    use super::Status;
    use serde_json::{Map, Value};
    use sidecar_core::inspect;
    use sidecar_core::process::Stamped;

    pub(super) fn status(
        namespace: &str,
        rows: &[(String, Vec<u32>)],
        broker: &Status,
    ) -> Result<(), String> {
        let value = serde_json::json!({
            "namespace": namespace,
            "runtime": runtime(broker),
            "targets": rows.iter().map(|(name, pids)| serde_json::json!({
                "name": name,
                "running": !pids.is_empty(),
                "pids": pids,
            })).collect::<Vec<_>>(),
        });
        println!(
            "{}",
            serde_json::to_string_pretty(&value).map_err(|err| err.to_string())?
        );
        Ok(())
    }

    pub(super) fn list(
        namespace: &str,
        hits: &[Stamped],
        broker: &Status,
        state: &Map<String, Value>,
    ) -> Result<(), String> {
        let value = serde_json::json!({
            "namespace": namespace,
            "runtime": runtime(broker),
            "processes": hits.iter().map(|hit| serde_json::json!({
                "pid": hit.pid,
                "command": hit.command,
            })).collect::<Vec<_>>(),
            "targets": state,
        });
        println!(
            "{}",
            serde_json::to_string_pretty(&value).map_err(|err| err.to_string())?
        );
        Ok(())
    }

    fn runtime(broker: &Status) -> Value {
        serde_json::json!({
            "running": !broker.pids.is_empty() && broker.endpoint.is_some(),
            "pids": broker.pids,
            "endpoint": broker.endpoint,
        })
    }

    pub(super) fn inspect(
        sidecar: &str,
        event: &str,
        response: &inspect::Response,
    ) -> Result<(), String> {
        let body = match response {
            inspect::Response::Ok(value) => serde_json::json!({
                "sidecar": sidecar,
                "event": event,
                "ok": true,
                "data": value,
            }),
            inspect::Response::Err(message) => serde_json::json!({
                "sidecar": sidecar,
                "event": event,
                "ok": false,
                "error": message,
            }),
        };
        println!(
            "{}",
            serde_json::to_string_pretty(&body).map_err(|err| err.to_string())?
        );
        if matches!(response, inspect::Response::Err(_)) {
            return Err("inspect endpoint returned ok=false".to_string());
        }
        Ok(())
    }
}
