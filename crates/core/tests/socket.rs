use sidecar_core::SocketEndpoint;
use std::path::PathBuf;

#[test]
fn unix_endpoint() {
    let endpoint = SocketEndpoint::parse("unix:///tmp/sidecar.sock").unwrap();
    assert_eq!(
        endpoint,
        SocketEndpoint::Unix(PathBuf::from("/tmp/sidecar.sock"))
    );
    assert_eq!(endpoint.as_endpoint(), "unix:///tmp/sidecar.sock");
}

#[test]
fn bare_tcp() {
    let error = SocketEndpoint::parse("127.0.0.1:3901").unwrap_err();
    assert!(error.to_string().contains("unix:///path.sock"));
}
