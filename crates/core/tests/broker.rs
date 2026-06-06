use sidecar_core::{
    broker_hello_request, decode_broker_identity, encode_broker_identity, read_broker_flag,
    read_broker_identity, validate_broker_hello, BrokerIdentity, BrokerRequest, BrokerResponse,
};

#[test]
fn canonical_flag() {
    let identity = BrokerIdentity::new("sidecar", "default");
    assert_eq!(
        identity.args(),
        vec!["--sidecar-broker=p=sidecar;n=default;s=tool%3Asidecar"]
    );
}

#[test]
fn flag_forms() {
    let inline = vec!["--sidecar-broker=p=app;n=default;s=tool%3Asidecar".to_string()];
    assert_eq!(
        read_broker_flag(&inline).as_deref(),
        Some("p=app;n=default;s=tool%3Asidecar")
    );

    let separated = vec![
        "--sidecar-broker".to_string(),
        "p=app;n=prod;s=tool%3Asidecar".to_string(),
    ];
    assert_eq!(
        read_broker_flag(&separated).as_deref(),
        Some("p=app;n=prod;s=tool%3Asidecar")
    );
}

#[test]
fn required_keys() {
    assert!(decode_broker_identity("p=app;n=default").is_err());

    let args = vec!["--sidecar-broker=p=app;n=default;s=tool%3Asidecar".into()];
    let identity = read_broker_identity(&args).unwrap();
    assert_eq!(identity.project, "app");
    assert_eq!(identity.namespace, "default");
    assert_eq!(identity.source, "tool:sidecar");
}

#[test]
fn reserved_chars() {
    let identity = BrokerIdentity {
        project: "local app".into(),
        namespace: "dev;blue".into(),
        source: "tool:%sidecar".into(),
    };
    let encoded = encode_broker_identity(&identity);
    assert_eq!(encoded, "p=local%20app;n=dev%3Bblue;s=tool%3A%25sidecar");
    assert_eq!(decode_broker_identity(&encoded).unwrap(), identity);
}

#[test]
fn bad_keys() {
    assert!(decode_broker_identity("p=app;n=default;s=tool%3Asidecar;x=no").is_err());
    assert!(decode_broker_identity("p=app;p=app2;n=default;s=tool%3Asidecar").is_err());
}

#[test]
fn hello_protocol() {
    let identity = BrokerIdentity::new("sidecar", "default");
    let request = broker_hello_request(&identity);
    assert_eq!(
        validate_broker_hello(&request, &identity).unwrap(),
        BrokerResponse::HelloOk {
            protocol: 1,
            project: "sidecar".into(),
            namespace: "default".into(),
        }
    );

    let wrong_namespace = BrokerRequest::Hello {
        protocol: 1,
        project: "sidecar".into(),
        namespace: "other".into(),
    };
    assert!(validate_broker_hello(&wrong_namespace, &identity).is_err());
}
