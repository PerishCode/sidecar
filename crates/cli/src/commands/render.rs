use crate::cli::OutputFormat;
use serde_json::{Map, Value};
use sidecar_core::inspect;
use sidecar_core::process::Stamped;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct BrokerRuntimeStatus {
    pub(super) pids: Vec<u32>,
    pub(super) endpoint: Option<String>,
}

pub(super) fn print_status(
    namespace: &str,
    rows: &[(String, Vec<u32>)],
    broker: &BrokerRuntimeStatus,
    format: OutputFormat,
) -> Result<(), String> {
    match format {
        OutputFormat::Text => print_status_text(namespace, rows, broker),
        OutputFormat::Json => print_status_json(namespace, rows, broker),
    }
}

fn print_status_text(
    namespace: &str,
    rows: &[(String, Vec<u32>)],
    broker: &BrokerRuntimeStatus,
) -> Result<(), String> {
    println!("namespace: {namespace}");
    print_broker_text(broker);
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

fn print_status_json(
    namespace: &str,
    rows: &[(String, Vec<u32>)],
    broker: &BrokerRuntimeStatus,
) -> Result<(), String> {
    let value = serde_json::json!({
        "namespace": namespace,
        "runtime": broker_json(broker),
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

pub(super) fn print_list(
    namespace: &str,
    hits: &[Stamped],
    broker: &BrokerRuntimeStatus,
    state: &Map<String, Value>,
    format: OutputFormat,
) -> Result<(), String> {
    match format {
        OutputFormat::Text => print_list_text(namespace, hits, broker, state),
        OutputFormat::Json => print_list_json(namespace, hits, broker, state),
    }
}

fn print_list_text(
    namespace: &str,
    hits: &[Stamped],
    broker: &BrokerRuntimeStatus,
    state: &Map<String, Value>,
) -> Result<(), String> {
    println!("namespace: {namespace}");
    print_broker_text(broker);
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

fn print_list_json(
    namespace: &str,
    hits: &[Stamped],
    broker: &BrokerRuntimeStatus,
    state: &Map<String, Value>,
) -> Result<(), String> {
    let value = serde_json::json!({
        "namespace": namespace,
        "runtime": broker_json(broker),
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

fn print_broker_text(broker: &BrokerRuntimeStatus) {
    match (&broker.endpoint, broker.pids.first()) {
        (Some(endpoint), Some(pid)) => println!("runtime: running (pid {pid}) {endpoint}"),
        (None, Some(pid)) => println!("runtime: starting or stale (pid {pid})"),
        _ => println!("runtime: stopped"),
    }
}

fn broker_json(broker: &BrokerRuntimeStatus) -> Value {
    serde_json::json!({
        "running": !broker.pids.is_empty() && broker.endpoint.is_some(),
        "pids": broker.pids,
        "endpoint": broker.endpoint,
    })
}

pub(super) fn print_inspect_response(
    sidecar: &str,
    event: &str,
    response: &inspect::Response,
    format: OutputFormat,
) -> Result<(), String> {
    match format {
        OutputFormat::Text => print_inspect_text(sidecar, event, response),
        OutputFormat::Json => print_inspect_json(sidecar, event, response),
    }
}

fn print_inspect_text(
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

fn print_inspect_json(
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
