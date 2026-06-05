use sidecar_cli::update_test;
use std::time::Duration;

#[test]
fn beta_order() {
    assert!(update_test::newer("v0.1.0-beta.2", "v0.1.0-beta.1"));
    assert!(!update_test::newer("v0.1.0-beta.1", "v0.1.0-beta.2"));
}

#[test]
fn stable_beats_beta() {
    assert!(update_test::newer("v0.1.0", "v0.1.0-beta.5"));
    assert!(!update_test::newer("v0.1.0-beta.5", "v0.1.0"));
}

#[test]
fn equal_versions() {
    assert!(!update_test::newer("v0.1.0-beta.1", "v0.1.0-beta.1"));
    assert!(!update_test::newer("v0.1.0", "v0.1.0"));
}

#[test]
fn higher_minor() {
    assert!(update_test::newer("v0.2.0-beta.1", "v0.1.5"));
}

#[test]
fn ttl_units() {
    assert_eq!(update_test::ttl("0"), Some(Duration::from_secs(0)));
    assert_eq!(update_test::ttl("90"), Some(Duration::from_secs(90)));
    assert_eq!(update_test::ttl("30s"), Some(Duration::from_secs(30)));
    assert_eq!(update_test::ttl("5m"), Some(Duration::from_secs(300)));
    assert_eq!(update_test::ttl("2h"), Some(Duration::from_secs(7200)));
    assert_eq!(update_test::ttl("1d"), Some(Duration::from_secs(86400)));
    assert_eq!(update_test::ttl(""), None);
    assert_eq!(update_test::ttl("garbage"), None);
}

#[test]
fn malformed_versions() {
    assert!(!update_test::newer("garbage", "v0.1.0"));
    assert!(!update_test::newer("v0.1.0", "garbage"));
}

#[test]
fn dev_skips_check() {
    assert!(!update_test::check_enabled("dev"));
    assert!(!update_test::check_enabled(""));
}

#[test]
fn no_check_env() {
    let key = "SIDECAR_NO_UPDATE_CHECK";
    let prev = std::env::var(key).ok();
    std::env::set_var(key, "1");
    assert!(!update_test::check_enabled("beta"));
    std::env::set_var(key, "0");
    assert!(update_test::check_enabled("beta"));
    match prev {
        Some(value) => std::env::set_var(key, value),
        None => std::env::remove_var(key),
    }
}
