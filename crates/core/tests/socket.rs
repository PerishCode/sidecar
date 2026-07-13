use sidecar_core::socket::Endpoint;
use std::path::PathBuf;

#[test]
fn unix() {
    let endpoint = Endpoint::parse("unix:///tmp/sidecar.sock").unwrap();
    assert_eq!(endpoint, Endpoint::Unix(PathBuf::from("/tmp/sidecar.sock")));
    assert_eq!(endpoint.to_string(), "unix:///tmp/sidecar.sock");
}

#[test]
fn bare() {
    let error = Endpoint::parse("127.0.0.1:3901").unwrap_err();
    assert!(error.to_string().contains("unix:///path.sock"));
}
