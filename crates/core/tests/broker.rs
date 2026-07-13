use sidecar_core::broker;
use sidecar_core::broker::{Identity, Request, Response};

#[test]
fn canonical() {
    let identity = Identity::new("sidecar", "default");
    assert_eq!(
        identity.args(),
        vec!["--sidecar-broker=p=sidecar;n=default;s=tool%3Asidecar"]
    );
}

#[test]
fn forms() {
    let inline = vec!["--sidecar-broker=p=app;n=default;s=tool%3Asidecar".to_string()];
    assert_eq!(
        broker::flag(&inline).as_deref(),
        Some("p=app;n=default;s=tool%3Asidecar")
    );

    let separated = vec![
        "--sidecar-broker".to_string(),
        "p=app;n=prod;s=tool%3Asidecar".to_string(),
    ];
    assert_eq!(
        broker::flag(&separated).as_deref(),
        Some("p=app;n=prod;s=tool%3Asidecar")
    );
}

#[test]
fn required() {
    assert!(Identity::decode("p=app;n=default").is_err());

    let args = vec!["--sidecar-broker=p=app;n=default;s=tool%3Asidecar".into()];
    let identity = broker::read(&args).unwrap();
    assert_eq!(identity.project, "app");
    assert_eq!(identity.namespace, "default");
    assert_eq!(identity.source, "tool:sidecar");
}

#[test]
fn reserved() {
    let identity = Identity {
        project: "local app".into(),
        namespace: "dev;blue".into(),
        source: "tool:%sidecar".into(),
    };
    let encoded = identity.encode();
    assert_eq!(encoded, "p=local%20app;n=dev%3Bblue;s=tool%3A%25sidecar");
    assert_eq!(Identity::decode(&encoded).unwrap(), identity);
}

#[test]
fn invalid() {
    assert!(Identity::decode("p=app;n=default;s=tool%3Asidecar;x=no").is_err());
    assert!(Identity::decode("p=app;p=app2;n=default;s=tool%3Asidecar").is_err());
}

#[test]
fn hello() {
    let identity = Identity::new("sidecar", "default");
    let request = identity.hello();
    assert_eq!(
        identity.validate(&request).unwrap(),
        Response::Ok {
            protocol: 1,
            project: "sidecar".into(),
            namespace: "default".into(),
        }
    );

    let mismatched = Request::Hello {
        protocol: 1,
        project: "sidecar".into(),
        namespace: "other".into(),
    };
    assert!(identity.validate(&mismatched).is_err());
}

#[test]
fn probe() {
    use std::io::{BufRead, BufReader, Write};
    use std::net::TcpListener;
    use std::time::Duration;

    let listener = TcpListener::bind(("127.0.0.1", 0)).unwrap();
    let addr = listener.local_addr().unwrap();
    let worker = std::thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        let mut line = String::new();
        BufReader::new(stream.try_clone().unwrap())
            .read_line(&mut line)
            .unwrap();
        assert_eq!(
            line.trim(),
            r#"{"kind":"hello","protocol":1,"project":"sidecar","namespace":"default"}"#
        );
        stream
            .write_all(
                br#"{"kind":"hello_ok","protocol":1,"project":"sidecar","namespace":"default"}
"#,
            )
            .unwrap();
    });

    let identity = Identity::new("sidecar", "default");
    assert!(identity.probe(addr, Duration::from_secs(2)).unwrap());
    worker.join().unwrap();
}
