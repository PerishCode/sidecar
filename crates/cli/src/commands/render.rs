use crate::cli::OutputFormat;
use serde_json::{Map, Value};
use sidecar_core::{InspectResponse, StampedProcess};

pub(super) fn print_status(
    namespace: &str,
    rows: &[(String, Vec<u32>)],
    format: OutputFormat,
) -> Result<(), String> {
    match format {
        OutputFormat::Text => print_status_text(namespace, rows),
        OutputFormat::Json => print_status_json(namespace, rows),
    }
}

fn print_status_text(namespace: &str, rows: &[(String, Vec<u32>)]) -> Result<(), String> {
    println!("namespace: {namespace}");
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

fn print_status_json(namespace: &str, rows: &[(String, Vec<u32>)]) -> Result<(), String> {
    let value = serde_json::json!({
        "namespace": namespace,
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
    hits: &[StampedProcess],
    state: &Map<String, Value>,
    format: OutputFormat,
) -> Result<(), String> {
    match format {
        OutputFormat::Text => print_list_text(namespace, hits, state),
        OutputFormat::Json => print_list_json(namespace, hits, state),
    }
}

fn print_list_text(
    namespace: &str,
    hits: &[StampedProcess],
    state: &Map<String, Value>,
) -> Result<(), String> {
    println!("namespace: {namespace}");
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
    hits: &[StampedProcess],
    state: &Map<String, Value>,
) -> Result<(), String> {
    let value = serde_json::json!({
        "namespace": namespace,
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

pub(super) fn print_inspect_response(
    sidecar: &str,
    event: &str,
    response: &InspectResponse,
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
    response: &InspectResponse,
) -> Result<(), String> {
    match response {
        InspectResponse::Ok(value) => {
            println!("ok {sidecar} {event}");
            println!(
                "{}",
                serde_json::to_string_pretty(value).unwrap_or_default()
            );
            Ok(())
        }
        InspectResponse::Err(message) => Err(format!("inspect error: {message}")),
    }
}

fn print_inspect_json(
    sidecar: &str,
    event: &str,
    response: &InspectResponse,
) -> Result<(), String> {
    let body = match response {
        InspectResponse::Ok(value) => serde_json::json!({
            "sidecar": sidecar,
            "event": event,
            "ok": true,
            "data": value,
        }),
        InspectResponse::Err(message) => serde_json::json!({
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
    if matches!(response, InspectResponse::Err(_)) {
        return Err("inspect endpoint returned ok=false".to_string());
    }
    Ok(())
}
