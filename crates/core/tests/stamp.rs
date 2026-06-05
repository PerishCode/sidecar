use sidecar_core::{decode_stamp, encode_stamp, read_stamp, read_stamp_flag, Stamp};

#[test]
fn canonical_flag() {
    let stamp = Stamp {
        app: "controller".into(),
        namespace: "default".into(),
        mode: "dev".into(),
        source: "tool:sidecar".into(),
    };
    let args = stamp.args();
    assert_eq!(
        args,
        vec!["--sidecar-stamp=a=controller;n=default;m=dev;s=tool%3Asidecar"]
    );
}

#[test]
fn flag_forms() {
    let inline = vec!["--sidecar-stamp=a=api;n=default;m=dev;s=tool%3Asidecar".to_string()];
    assert_eq!(
        read_stamp_flag(&inline).as_deref(),
        Some("a=api;n=default;m=dev;s=tool%3Asidecar")
    );

    let separated = vec![
        "--sidecar-stamp".to_string(),
        "a=api;n=design;m=dev;s=tool%3Asidecar".to_string(),
    ];
    assert_eq!(
        read_stamp_flag(&separated).as_deref(),
        Some("a=api;n=design;m=dev;s=tool%3Asidecar")
    );
}

#[test]
fn required_keys() {
    let partial = vec!["--sidecar-stamp=a=api;n=default;m=dev".to_string()];
    assert!(read_stamp(&partial).is_none());

    let full = vec!["--sidecar-stamp=a=api;n=default;m=dev;s=tool%3Asidecar".into()];
    let stamp = read_stamp(&full).unwrap();
    assert_eq!(stamp.app, "api");
    assert_eq!(stamp.source, "tool:sidecar");
}

#[test]
fn reserved_chars() {
    let stamp = Stamp {
        app: "api worker".into(),
        namespace: "dev;blue".into(),
        mode: "runtime=1".into(),
        source: "tool:%sidecar".into(),
    };
    let encoded = encode_stamp(&stamp);
    assert_eq!(
        encoded,
        "a=api%20worker;n=dev%3Bblue;m=runtime%3D1;s=tool%3A%25sidecar"
    );
    assert_eq!(decode_stamp(&encoded).unwrap(), stamp);
}

#[test]
fn bad_keys() {
    assert!(decode_stamp("a=api;n=default;m=dev;s=tool%3Asidecar;x=no").is_err());
    assert!(decode_stamp("a=api;a=api2;n=default;m=dev;s=tool%3Asidecar").is_err());
}

#[test]
fn bad_percent() {
    assert!(decode_stamp("a=api%XX;n=default;m=dev;s=tool%3Asidecar").is_err());
}
