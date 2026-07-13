#[cfg(target_os = "linux")]
#[test]
fn linux() {
    use std::collections::BTreeSet;

    let table = "\
  sl  local_address rem_address   st tx_queue rx_queue tr tm->when retrnsmt   uid  timeout inode\n\
   0: 0100007F:1F90 00000000:0000 0A 00000000:00000000 00:00000000 00000000  1000        0 12345 1 0000000000000000 100 0 0 10 0\n\
   1: 0100007F:1F91 00000000:0000 01 00000000:00000000 00:00000000 00000000  1000        0 12346 1 0000000000000000 100 0 0 10 0\n";
    let inodes = BTreeSet::from(["12345".to_string()]);
    let parsed = sidecar_core::tcp::table(table, &inodes).unwrap();
    assert_eq!(parsed.len(), 1);
    assert_eq!(parsed[0].to_string(), "127.0.0.1:8080");
}

#[test]
fn lsof() {
    let text = "\
COMMAND   PID USER   FD   TYPE DEVICE SIZE/OFF NODE NAME\n\
sidecar  1234 fire   11u  IPv4 0x1234      0t0  TCP 127.0.0.1:49222 (LISTEN)\n\
sidecar  1234 fire   12u  IPv6 0x5678      0t0  TCP [::1]:49223 (LISTEN)\n";
    let parsed = sidecar_core::tcp::lsof::listeners(text);
    assert_eq!(parsed.len(), 2);
    assert_eq!(parsed[0].to_string(), "127.0.0.1:49222");
    assert_eq!(parsed[1].to_string(), "[::1]:49223");
}

#[test]
fn port() {
    assert_eq!(sidecar_core::tcp::port(0x901F), 8080);
}

#[cfg(any(target_os = "linux", target_os = "macos", windows))]
#[test]
fn discover() {
    let listener = std::net::TcpListener::bind(("127.0.0.1", 0)).unwrap();
    let expected = listener.local_addr().unwrap();
    let listeners = sidecar_core::tcp::listeners(std::process::id()).unwrap();

    assert!(
        listeners.contains(&expected),
        "expected {expected}, got {listeners:?}"
    );
}
