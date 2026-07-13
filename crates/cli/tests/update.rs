use sidecar_cli::test::update;
use std::time::Duration;

#[test]
fn beta() {
    assert!(update::newer("v0.1.0-beta.2", "v0.1.0-beta.1"));
    assert!(!update::newer("v0.1.0-beta.1", "v0.1.0-beta.2"));
}

#[test]
fn stable() {
    assert!(update::newer("v0.1.0", "v0.1.0-beta.5"));
    assert!(!update::newer("v0.1.0-beta.5", "v0.1.0"));
}

#[test]
fn equal() {
    assert!(!update::newer("v0.1.0-beta.1", "v0.1.0-beta.1"));
    assert!(!update::newer("v0.1.0", "v0.1.0"));
}

#[test]
fn minor() {
    assert!(update::newer("v0.2.0-beta.1", "v0.1.5"));
}

#[test]
fn ttl() {
    assert_eq!(update::ttl("0"), Some(Duration::from_secs(0)));
    assert_eq!(update::ttl("90"), Some(Duration::from_secs(90)));
    assert_eq!(update::ttl("30s"), Some(Duration::from_secs(30)));
    assert_eq!(update::ttl("5m"), Some(Duration::from_secs(300)));
    assert_eq!(update::ttl("2h"), Some(Duration::from_secs(7200)));
    assert_eq!(update::ttl("1d"), Some(Duration::from_secs(86400)));
    assert_eq!(update::ttl(""), None);
    assert_eq!(update::ttl("garbage"), None);
}

#[test]
fn malformed() {
    assert!(!update::newer("garbage", "v0.1.0"));
    assert!(!update::newer("v0.1.0", "garbage"));
}

#[test]
fn dev() {
    assert!(!update::enabled("dev"));
    assert!(!update::enabled(""));
}

#[test]
fn muted() {
    let key = "SIDECAR_NO_UPDATE_CHECK";
    let prev = std::env::var(key).ok();
    std::env::set_var(key, "1");
    assert!(!update::enabled("beta"));
    std::env::set_var(key, "0");
    assert!(update::enabled("beta"));
    match prev {
        Some(value) => std::env::set_var(key, value),
        None => std::env::remove_var(key),
    }
}
