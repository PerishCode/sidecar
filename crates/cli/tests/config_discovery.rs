use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

fn bin() -> Command {
    Command::new(env!("CARGO_BIN_EXE_sidecar"))
}

fn temp_root(name: &str) -> std::path::PathBuf {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock should be after epoch")
        .as_nanos();
    let root = std::env::temp_dir().join(format!("sidecar-{name}-{nonce}"));
    std::fs::create_dir_all(&root).expect("temp root should be created");
    root
}

fn minimal_config(project: &str) -> String {
    format!(
        r#"[project]
name = "{project}"
namespace = "default"
root = "."

[[sidecars]]
name = "api"
command = "sh"
args = ["-c", "sleep 1"]
cwd = "."
mode = "dev"
"#
    )
}

#[test]
fn discovers_sidecar_toml() {
    let root = temp_root("discovers");
    let nested = root.join("a/b");
    std::fs::create_dir_all(&nested).expect("nested dir should be created");
    let config = root.join("sidecar.toml");
    std::fs::write(&config, minimal_config("discovered-project"))
        .expect("config should be written");

    let output = bin()
        .current_dir(&nested)
        .args(["plan", "--format=json"])
        .output()
        .expect("sidecar should run");

    assert!(
        output.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stdout.contains("\"project\": \"discovered-project\""));
    assert!(stderr.contains("sidecar: using config"));
    assert!(stderr.contains("sidecar.toml"));

    std::fs::remove_dir_all(root).ok();
}

#[test]
fn explicit_config_wins() {
    let root = temp_root("explicit");
    let nested = root.join("nested");
    let explicit = root.join("explicit.toml");
    std::fs::create_dir_all(&nested).expect("nested dir should be created");
    std::fs::write(
        root.join("sidecar.toml"),
        minimal_config("discovered-project"),
    )
    .expect("discovered config should be written");
    std::fs::write(&explicit, minimal_config("explicit-project"))
        .expect("explicit config should be written");

    let output = bin()
        .current_dir(&nested)
        .arg("plan")
        .arg("--format=json")
        .arg("--config")
        .arg(&explicit)
        .output()
        .expect("sidecar should run");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stdout.contains("\"project\": \"explicit-project\""));
    assert!(!stderr.contains("sidecar: using config"));

    std::fs::remove_dir_all(root).ok();
}

#[test]
fn missing_config_reports_search() {
    let root = temp_root("missing");
    let nested = root.join("nested");
    std::fs::create_dir_all(&nested).expect("nested dir should be created");

    let output = bin()
        .current_dir(&nested)
        .arg("doctor")
        .output()
        .expect("sidecar should run");

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("no sidecar config found from"));
    assert!(stderr.contains("Hint: create sidecar.toml here or pass --config <path>."));
    assert!(stderr.contains("Searched:"));
    assert!(stderr.contains("sidecar.toml"));

    std::fs::remove_dir_all(root).ok();
}
