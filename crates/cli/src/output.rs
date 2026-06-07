use crate::cli::OutputFormat;
use sidecar_core::{AppPlan, Diagnostic, ExecutionPlan, Severity, SidecarPlan, TargetPlan};

pub(crate) fn print_diagnostics(
    diagnostics: &[Diagnostic],
    format: OutputFormat,
) -> Result<(), String> {
    match format {
        OutputFormat::Text => print_diagnostics_text(diagnostics),
        OutputFormat::Json => print_diagnostics_json(diagnostics),
    }
}

fn print_diagnostics_text(diagnostics: &[Diagnostic]) -> Result<(), String> {
    if diagnostics.is_empty() {
        println!("sidecar doctor found no issues");
        return Ok(());
    }

    println!("sidecar doctor found {} issue(s)", diagnostics.len());
    for diagnostic in diagnostics {
        println!(
            "{} {} - {}",
            severity_label(diagnostic.severity),
            diagnostic.path,
            diagnostic.message
        );
    }
    Ok(())
}

fn print_diagnostics_json(diagnostics: &[Diagnostic]) -> Result<(), String> {
    let items: Vec<_> = diagnostics
        .iter()
        .map(|diagnostic| {
            serde_json::json!({
                "severity": severity_label(diagnostic.severity),
                "path": diagnostic.path,
                "message": diagnostic.message,
            })
        })
        .collect();
    println!(
        "{}",
        serde_json::to_string_pretty(&serde_json::json!({ "diagnostics": items }))
            .map_err(|error| error.to_string())?
    );
    Ok(())
}

pub(crate) fn print_plan(plan: &ExecutionPlan, format: OutputFormat) -> Result<(), String> {
    match format {
        OutputFormat::Text => print_plan_text(plan),
        OutputFormat::Json => print_plan_json(plan),
    }
}

fn print_plan_text(plan: &ExecutionPlan) -> Result<(), String> {
    println!("project: {} (namespace: {})", plan.project, plan.namespace);
    match &plan.app {
        Some(app) => println!(
            "app: {} -> {}",
            app.name,
            command_line(&app.command, &app.args)
        ),
        None => println!("app: <none>"),
    }
    print_targets_text(plan);
    print_endpoints_text(plan);
    Ok(())
}

fn print_targets_text(plan: &ExecutionPlan) {
    println!("targets: {}", plan.targets.len());
    for target in &plan.targets {
        println!(
            "- {} [mode={}] -> {}",
            target.name,
            target.stamp.mode,
            command_line(&target.command, &target.spawn_args())
        );
        if let Some(socket) = &target.inspect_socket {
            println!("    inspect_socket: {socket}");
        }
        if let Some(ready) = &target.ready {
            println!("    ready: {}", ready.role);
        }
    }
}

fn print_endpoints_text(plan: &ExecutionPlan) {
    println!("inspect endpoints: {}", plan.inspect_endpoints.len());
    for endpoint in &plan.inspect_endpoints {
        println!("- {} {} {}", endpoint.name, endpoint.kind, endpoint.url);
    }
}

fn print_plan_json(plan: &ExecutionPlan) -> Result<(), String> {
    let value = serde_json::json!({
        "project": plan.project,
        "namespace": plan.namespace,
        "root": plan.root,
        "app": plan.app.as_ref().map(app_json),
        "sidecars": plan.sidecars.iter().map(sidecar_json).collect::<Vec<_>>(),
        "targets": plan.targets.iter().map(target_json).collect::<Vec<_>>(),
        "inspectEndpoints": plan.inspect_endpoints.iter().map(|endpoint| serde_json::json!({
            "name": endpoint.name,
            "kind": endpoint.kind,
            "url": endpoint.url,
        })).collect::<Vec<_>>(),
    });
    println!(
        "{}",
        serde_json::to_string_pretty(&value).map_err(|error| error.to_string())?
    );
    Ok(())
}

fn severity_label(severity: Severity) -> &'static str {
    match severity {
        Severity::Error => "error",
        Severity::Warning => "warning",
    }
}

fn command_line(command: &str, args: &[String]) -> String {
    std::iter::once(command)
        .chain(args.iter().map(String::as_str))
        .collect::<Vec<_>>()
        .join(" ")
}

fn app_json(app: &AppPlan) -> serde_json::Value {
    serde_json::json!({
        "name": app.name,
        "command": app.command,
        "args": app.args,
        "cwd": app.cwd,
        "stamp": stamp_json(&app.stamp),
        "spawnArgs": app.spawn_args(),
        "inheritsEnv": inherits_json(&app.inherits_env),
        "inspectSocket": app.inspect_socket,
        "healthUrl": app.health_url,
    })
}

fn target_json(target: &TargetPlan) -> serde_json::Value {
    serde_json::json!({
        "name": target.name,
        "kind": format!("{:?}", target.kind).to_lowercase(),
        "command": target.command,
        "args": target.args,
        "cwd": target.cwd,
        "stamp": stamp_json(&target.stamp),
        "spawnArgs": target.spawn_args(),
        "inheritsEnv": inherits_json(&target.inherits_env),
        "inspectSocket": target.inspect_socket,
        "healthUrl": target.health_url,
        "ready": target.ready.as_ref().map(|ready| serde_json::json!({
            "role": ready.role,
            "timeoutSecs": ready.timeout_secs,
        })),
    })
}

fn sidecar_json(sidecar: &SidecarPlan) -> serde_json::Value {
    serde_json::json!({
        "name": sidecar.name,
        "command": sidecar.command,
        "args": sidecar.args,
        "cwd": sidecar.cwd,
        "stamp": stamp_json(&sidecar.stamp),
        "spawnArgs": sidecar.spawn_args(),
        "inheritsEnv": inherits_json(&sidecar.inherits_env),
        "inspectSocket": sidecar.inspect_socket,
        "healthUrl": sidecar.health_url,
    })
}

fn stamp_json(stamp: &sidecar_core::Stamp) -> serde_json::Value {
    serde_json::json!({
        "version": stamp.version,
        "app": stamp.app,
        "namespace": stamp.namespace,
        "mode": stamp.mode,
        "source": stamp.source,
        "endpoint": stamp.endpoint,
    })
}

fn inherits_json(bindings: &[sidecar_core::InheritEnvPlan]) -> Vec<serde_json::Value> {
    bindings
        .iter()
        .map(|binding| {
            serde_json::json!({
                "name": binding.name,
                "from": binding.from,
            })
        })
        .collect()
}
