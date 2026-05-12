use crate::cli::OutputFormat;
use sidecar_core::{AppPlan, Diagnostic, ExecutionPlan, Severity};

pub(crate) fn print_diagnostics(
    diagnostics: &[Diagnostic],
    format: OutputFormat,
) -> Result<(), String> {
    match format {
        OutputFormat::Text => {
            if diagnostics.is_empty() {
                println!("sidecar doctor found no issues");
                return Ok(());
            }

            println!("sidecar doctor found {} issue(s)", diagnostics.len());
            for diagnostic in diagnostics {
                let severity = match diagnostic.severity {
                    Severity::Error => "error",
                    Severity::Warning => "warning",
                };
                println!("{severity} {} - {}", diagnostic.path, diagnostic.message);
            }
            Ok(())
        }
        OutputFormat::Json => {
            let items: Vec<_> = diagnostics
                .iter()
                .map(|diagnostic| {
                    let severity = match diagnostic.severity {
                        Severity::Error => "error",
                        Severity::Warning => "warning",
                    };
                    serde_json::json!({
                        "severity": severity,
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
    }
}

pub(crate) fn print_plan(plan: &ExecutionPlan, format: OutputFormat) -> Result<(), String> {
    match format {
        OutputFormat::Text => {
            println!("project: {} (namespace: {})", plan.project, plan.namespace);
            match &plan.app {
                Some(app) => println!(
                    "app: {} -> {}",
                    app.name,
                    command_line(&app.command, &app.args)
                ),
                None => println!("app: <none>"),
            }
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
            println!("inspect endpoints: {}", plan.inspect_endpoints.len());
            for endpoint in &plan.inspect_endpoints {
                println!("- {} {} {}", endpoint.name, endpoint.kind, endpoint.url);
            }
            Ok(())
        }
        OutputFormat::Json => {
            let value = serde_json::json!({
                "project": plan.project,
                "namespace": plan.namespace,
                "root": plan.root,
                "app": plan.app.as_ref().map(|app| serde_json::json!({
                    "name": app.name,
                    "command": app.command,
                    "args": app.args,
                    "cwd": app.cwd,
                    "stamp": {
                        "app": app.stamp.app,
                        "namespace": app.stamp.namespace,
                        "mode": app.stamp.mode,
                        "source": app.stamp.source,
                    },
                    "spawnArgs": app_spawn_args(app),
                    "stampViaEnv": app.stamp_via_env,
                    "endpointEnv": app.endpoint_env,
                    "inheritsEnv": app.inherits_env.iter().map(|binding| serde_json::json!({
                        "name": binding.name,
                        "from": binding.from,
                    })).collect::<Vec<_>>(),
                    "inspectSocket": app.inspect_socket,
                    "healthUrl": app.health_url,
                })),
                "sidecars": plan.sidecars.iter().map(|sidecar| serde_json::json!({
                    "name": sidecar.name,
                    "command": sidecar.command,
                    "args": sidecar.args,
                    "cwd": sidecar.cwd,
                    "stamp": {
                        "app": sidecar.stamp.app,
                        "namespace": sidecar.stamp.namespace,
                        "mode": sidecar.stamp.mode,
                        "source": sidecar.stamp.source,
                    },
                    "spawnArgs": sidecar.spawn_args(),
                    "stampViaEnv": sidecar.stamp_via_env,
                    "endpointEnv": sidecar.endpoint_env,
                    "inheritsEnv": sidecar.inherits_env.iter().map(|binding| serde_json::json!({
                        "name": binding.name,
                        "from": binding.from,
                    })).collect::<Vec<_>>(),
                    "inspectSocket": sidecar.inspect_socket,
                    "healthUrl": sidecar.health_url,
                })).collect::<Vec<_>>(),
                "targets": plan.targets.iter().map(|target| serde_json::json!({
                    "name": target.name,
                    "kind": format!("{:?}", target.kind).to_lowercase(),
                    "command": target.command,
                    "args": target.args,
                    "cwd": target.cwd,
                    "stamp": {
                        "app": target.stamp.app,
                        "namespace": target.stamp.namespace,
                        "mode": target.stamp.mode,
                        "source": target.stamp.source,
                    },
                    "spawnArgs": target.spawn_args(),
                    "stampViaEnv": target.stamp_via_env,
                    "endpointEnv": target.endpoint_env,
                    "inheritsEnv": target.inherits_env.iter().map(|binding| serde_json::json!({
                        "name": binding.name,
                        "from": binding.from,
                    })).collect::<Vec<_>>(),
                    "inspectSocket": target.inspect_socket,
                    "healthUrl": target.health_url,
                    "ready": target.ready.as_ref().map(|ready| serde_json::json!({
                        "role": ready.role,
                        "timeoutSecs": ready.timeout_secs,
                    })),
                })).collect::<Vec<_>>(),
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
    }
}

fn command_line(command: &str, args: &[String]) -> String {
    std::iter::once(command)
        .chain(args.iter().map(String::as_str))
        .collect::<Vec<_>>()
        .join(" ")
}

fn app_spawn_args(app: &AppPlan) -> Vec<String> {
    let mut argv = app.args.clone();
    if !app.stamp_via_env {
        argv.extend(app.stamp.args());
    }
    argv
}
