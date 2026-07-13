use crate::cli::Format;
use sidecar_core::plan::Plan;
use sidecar_core::{Diagnostic, Severity};

pub(crate) fn diagnostics(diagnostics: &[Diagnostic], format: Format) -> Result<(), String> {
    match format {
        Format::Text => text::diagnostics(diagnostics),
        Format::Json => json::diagnostics(diagnostics),
    }
}

pub(crate) fn plan(plan: &Plan, format: Format) -> Result<(), String> {
    match format {
        Format::Text => text::plan(plan),
        Format::Json => json::plan(plan),
    }
}

fn severity(severity: Severity) -> &'static str {
    match severity {
        Severity::Error => "error",
        Severity::Warning => "warning",
    }
}

fn line(command: &str, args: &[String]) -> String {
    std::iter::once(command)
        .chain(args.iter().map(String::as_str))
        .collect::<Vec<_>>()
        .join(" ")
}

mod text {
    use super::{line, severity};
    use sidecar_core::plan::Plan;
    use sidecar_core::Diagnostic;

    pub(super) fn diagnostics(diagnostics: &[Diagnostic]) -> Result<(), String> {
        if diagnostics.is_empty() {
            println!("sidecar doctor found no issues");
            return Ok(());
        }

        println!("sidecar doctor found {} issue(s)", diagnostics.len());
        for diagnostic in diagnostics {
            println!(
                "{} {} - {}",
                severity(diagnostic.severity),
                diagnostic.path,
                diagnostic.message
            );
        }
        Ok(())
    }

    pub(super) fn plan(plan: &Plan) -> Result<(), String> {
        println!("project: {} (namespace: {})", plan.project, plan.namespace);
        match &plan.app {
            Some(app) => println!("app: {} -> {}", app.name, line(&app.command, &app.args)),
            None => println!("app: <none>"),
        }
        targets(plan);
        endpoints(plan);
        Ok(())
    }

    fn targets(plan: &Plan) {
        println!("targets: {}", plan.targets.len());
        for target in &plan.targets {
            println!(
                "- {} [mode={}] -> {}",
                target.name,
                target.stamp.mode,
                line(&target.command, &target.argv())
            );
            if let Some(socket) = &target.socket {
                println!("    inspect_socket: {socket}");
            }
            if let Some(ready) = &target.ready {
                println!("    ready: {}", ready.role);
            }
        }
    }

    fn endpoints(plan: &Plan) {
        println!("inspect endpoints: {}", plan.endpoints.len());
        for endpoint in &plan.endpoints {
            println!("- {} {} {}", endpoint.name, endpoint.kind, endpoint.url);
        }
    }
}

mod json {
    use super::severity;
    use sidecar_core::plan::{App, Inherit, Plan, Sidecar, Target};
    use sidecar_core::Diagnostic;

    pub(super) fn diagnostics(diagnostics: &[Diagnostic]) -> Result<(), String> {
        let items: Vec<_> = diagnostics
            .iter()
            .map(|diagnostic| {
                serde_json::json!({
                    "severity": severity(diagnostic.severity),
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

    pub(super) fn plan(plan: &Plan) -> Result<(), String> {
        let value = serde_json::json!({
            "project": plan.project,
            "namespace": plan.namespace,
            "root": plan.root,
            "app": plan.app.as_ref().map(app),
            "sidecars": plan.sidecars.iter().map(sidecar).collect::<Vec<_>>(),
            "targets": plan.targets.iter().map(target).collect::<Vec<_>>(),
            "inspectEndpoints": plan.endpoints.iter().map(|endpoint| serde_json::json!({
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

    fn app(app: &App) -> serde_json::Value {
        serde_json::json!({
            "name": app.name,
            "command": app.command,
            "args": app.args,
            "cwd": app.cwd,
            "stamp": stamp(&app.stamp),
            "spawnArgs": app.argv(),
            "inheritsEnv": inherits(&app.inherits),
            "inspectSocket": app.socket,
            "healthUrl": app.health,
        })
    }

    fn target(target: &Target) -> serde_json::Value {
        serde_json::json!({
            "name": target.name,
            "kind": format!("{:?}", target.kind).to_lowercase(),
            "command": target.command,
            "args": target.args,
            "cwd": target.cwd,
            "stamp": stamp(&target.stamp),
            "spawnArgs": target.argv(),
            "inheritsEnv": inherits(&target.inherits),
            "inspectSocket": target.socket,
            "healthUrl": target.health,
            "ready": target.ready.as_ref().map(|ready| serde_json::json!({
                "role": ready.role,
                "timeoutSecs": ready.timeout,
            })),
        })
    }

    fn sidecar(sidecar: &Sidecar) -> serde_json::Value {
        serde_json::json!({
            "name": sidecar.name,
            "command": sidecar.command,
            "args": sidecar.args,
            "cwd": sidecar.cwd,
            "stamp": stamp(&sidecar.stamp),
            "spawnArgs": sidecar.argv(),
            "inheritsEnv": inherits(&sidecar.inherits),
            "inspectSocket": sidecar.socket,
            "healthUrl": sidecar.health,
        })
    }

    fn stamp(stamp: &sidecar_core::Stamp) -> serde_json::Value {
        serde_json::json!({
            "version": stamp.version,
            "app": stamp.app,
            "namespace": stamp.namespace,
            "mode": stamp.mode,
            "source": stamp.source,
            "endpoint": stamp.endpoint,
        })
    }

    fn inherits(bindings: &[Inherit]) -> Vec<serde_json::Value> {
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
}
