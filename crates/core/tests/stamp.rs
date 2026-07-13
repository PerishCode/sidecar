use sidecar_core::stamp;
use sidecar_core::Stamp;

#[test]
fn canonical() {
    let stamp = Stamp {
        version: 1,
        app: "controller".into(),
        namespace: "default".into(),
        mode: "dev".into(),
        source: "tool:sidecar".into(),
        endpoint: None,
    };
    let args = stamp.args();
    assert_eq!(
        args,
        vec!["--sidecar-stamp=v=1;a=controller;n=default;m=dev;s=tool%3Asidecar"]
    );
}

#[test]
fn forms() {
    let inline = vec!["--sidecar-stamp=v=1;a=api;n=default;m=dev;s=tool%3Asidecar".to_string()];
    assert_eq!(
        stamp::flag(&inline).as_deref(),
        Some("v=1;a=api;n=default;m=dev;s=tool%3Asidecar")
    );

    let separated = vec![
        "--sidecar-stamp".to_string(),
        "v=1;a=api;n=design;m=dev;s=tool%3Asidecar".to_string(),
    ];
    assert_eq!(
        stamp::flag(&separated).as_deref(),
        Some("v=1;a=api;n=design;m=dev;s=tool%3Asidecar")
    );
}

#[test]
fn required() {
    let partial = vec!["--sidecar-stamp=v=1;a=api;n=default;m=dev".to_string()];
    assert!(stamp::read(&partial).is_none());

    let full = vec!["--sidecar-stamp=v=1;a=api;n=default;m=dev;s=tool%3Asidecar".into()];
    let stamp = stamp::read(&full).unwrap();
    assert_eq!(stamp.version, 1);
    assert_eq!(stamp.app, "api");
    assert_eq!(stamp.source, "tool:sidecar");
    assert_eq!(stamp.endpoint, None);
}

#[test]
fn reserved() {
    let stamp = Stamp {
        version: 1,
        app: "api worker".into(),
        namespace: "dev;blue".into(),
        mode: "runtime=1".into(),
        source: "tool:%sidecar".into(),
        endpoint: Some("tcp://127.0.0.1:4100".into()),
    };
    let encoded = stamp::encode(&stamp);
    assert_eq!(
        encoded,
        "v=1;a=api%20worker;n=dev%3Bblue;m=runtime%3D1;s=tool%3A%25sidecar;e=tcp%3A%2F%2F127.0.0.1%3A4100"
    );
    assert_eq!(stamp::decode(&encoded).unwrap(), stamp);
}

#[test]
fn invalid() {
    assert!(stamp::decode("v=1;a=api;n=default;m=dev;s=tool%3Asidecar;x=no").is_err());
    assert!(stamp::decode("v=1;a=api;a=api2;n=default;m=dev;s=tool%3Asidecar").is_err());
    assert!(stamp::decode("v=2;a=api;n=default;m=dev;s=tool%3Asidecar").is_err());
    assert!(stamp::decode("a=api;n=default;m=dev;s=tool%3Asidecar").is_err());
}

#[test]
fn percent() {
    assert!(stamp::decode("v=1;a=api%XX;n=default;m=dev;s=tool%3Asidecar").is_err());
}
