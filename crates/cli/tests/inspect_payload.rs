use serde_json::Value;
use std::io::{BufRead, BufReader, Write};
use std::net::TcpListener;
use std::process::Command;
use std::sync::mpsc;
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

#[test]
fn omitted_payload_is_object() {
    let listener = TcpListener::bind("127.0.0.1:0").expect("tcp listener should bind");
    let address = listener.local_addr().expect("listener address");
    let (tx, rx) = mpsc::channel();

    let handle = std::thread::spawn(move || {
        let (mut stream, _) = listener.accept().expect("inspect client should connect");
        let mut line = String::new();
        BufReader::new(stream.try_clone().expect("stream should clone"))
            .read_line(&mut line)
            .expect("inspect request should read");
        let request: Value = serde_json::from_str(line.trim()).expect("request should be json");
        let id = request
            .get("id")
            .and_then(Value::as_str)
            .expect("request should include id")
            .to_string();
        tx.send(request).expect("request should be sent to test");
        writeln!(
            stream,
            "{}",
            serde_json::json!({
                "kind": "event_response",
                "id": id,
                "payload": {"ok": true}
            })
        )
        .expect("inspect response should write");
    });

    let root = temp_root("inspect-payload");
    let config = root.join("sidecar.toml");
    std::fs::write(
        &config,
        format!(
            r#"[project]
name = "inspect-payload"
namespace = "inspect-payload"

[[sidecars]]
name = "server"
command = "sh"
args = ["-c", "sleep 1"]
inspect_socket = "tcp://{address}"
"#
        ),
    )
    .expect("config should be written");

    let output = bin()
        .args([
            "inspect",
            "server",
            "server.status",
            "--config",
            config.to_str().expect("config path should be utf8"),
            "--format=json",
        ])
        .output()
        .expect("sidecar inspect should run");

    assert!(
        output.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let request = rx.recv().expect("request should be captured");
    assert_eq!(request.get("payload"), Some(&serde_json::json!({})));

    handle.join().expect("server thread should finish");
    std::fs::remove_dir_all(root).ok();
}
