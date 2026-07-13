use crate::stamp::{decode_value, encode_value, DEFAULT_SOURCE};
use serde::{Deserialize, Serialize};
use std::io::{BufRead, BufReader, Write};
use std::net::{SocketAddr, TcpStream};
use std::time::Duration;

pub const BROKER_FLAG: &str = "--sidecar-broker";
pub const BROKER_PROTOCOL_VERSION: u8 = 1;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BrokerIdentity {
    pub project: String,
    pub namespace: String,
    pub source: String,
}

impl BrokerIdentity {
    pub fn new(project: impl Into<String>, namespace: impl Into<String>) -> Self {
        Self {
            project: project.into(),
            namespace: namespace.into(),
            source: DEFAULT_SOURCE.to_string(),
        }
    }

    pub fn args(&self) -> Vec<String> {
        vec![format!("{BROKER_FLAG}={}", encode_identity(self))]
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "kind")]
pub enum BrokerRequest {
    #[serde(rename = "hello")]
    Hello {
        protocol: u8,
        project: String,
        namespace: String,
    },
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "kind")]
pub enum BrokerResponse {
    #[serde(rename = "hello_ok")]
    HelloOk {
        protocol: u8,
        project: String,
        namespace: String,
    },
    #[serde(rename = "hello_error")]
    HelloError { message: String },
}

pub fn hello_request(identity: &BrokerIdentity) -> BrokerRequest {
    BrokerRequest::Hello {
        protocol: BROKER_PROTOCOL_VERSION,
        project: identity.project.clone(),
        namespace: identity.namespace.clone(),
    }
}

pub fn hello_ok(identity: &BrokerIdentity) -> BrokerResponse {
    BrokerResponse::HelloOk {
        protocol: BROKER_PROTOCOL_VERSION,
        project: identity.project.clone(),
        namespace: identity.namespace.clone(),
    }
}

pub fn validate_hello(
    request: &BrokerRequest,
    expected: &BrokerIdentity,
) -> Result<BrokerResponse, String> {
    match request {
        BrokerRequest::Hello {
            protocol,
            project,
            namespace,
        } if *protocol == BROKER_PROTOCOL_VERSION
            && project == &expected.project
            && namespace == &expected.namespace =>
        {
            Ok(hello_ok(expected))
        }
        BrokerRequest::Hello { .. } => {
            Err("broker hello did not match expected protocol, project, or namespace".to_string())
        }
    }
}

pub fn discover_endpoint(
    identity: &BrokerIdentity,
    timeout: Duration,
) -> Result<Option<SocketAddr>, String> {
    let hits = crate::runtime::process::discover_brokers(&identity.project, &identity.namespace)?;
    for hit in hits {
        let listeners = crate::runtime::tcp::tcp_listeners_for_pid(hit.pid)?;
        for addr in listeners {
            if probe_endpoint(addr, identity, timeout)? {
                return Ok(Some(addr));
            }
        }
    }
    Ok(None)
}

pub fn probe_endpoint(
    addr: SocketAddr,
    identity: &BrokerIdentity,
    timeout: Duration,
) -> Result<bool, String> {
    let Ok(mut stream) = TcpStream::connect_timeout(&addr, timeout) else {
        return Ok(false);
    };
    stream
        .set_read_timeout(Some(timeout))
        .map_err(|err| format!("failed to set broker read timeout: {err}"))?;
    stream
        .set_write_timeout(Some(timeout))
        .map_err(|err| format!("failed to set broker write timeout: {err}"))?;
    let request = serde_json::to_string(&hello_request(identity)).map_err(|err| err.to_string())?;
    stream
        .write_all(request.as_bytes())
        .and_then(|_| stream.write_all(b"\n"))
        .map_err(|err| format!("failed to write broker hello: {err}"))?;

    let mut line = String::new();
    BufReader::new(stream)
        .read_line(&mut line)
        .map_err(|err| format!("failed to read broker hello response: {err}"))?;
    let Ok(response) = serde_json::from_str::<BrokerResponse>(line.trim()) else {
        return Ok(false);
    };
    Ok(matches!(
        response,
        BrokerResponse::HelloOk {
            protocol: BROKER_PROTOCOL_VERSION,
            project,
            namespace,
        } if project == identity.project && namespace == identity.namespace
    ))
}

pub fn read_broker_flag(args: &[String]) -> Option<String> {
    let prefix = format!("{BROKER_FLAG}=");
    for (index, value) in args.iter().enumerate() {
        if value == BROKER_FLAG {
            return args.get(index + 1).cloned();
        }
        if let Some(stripped) = value.strip_prefix(&prefix) {
            return Some(stripped.to_string());
        }
    }
    None
}

pub fn read_broker_identity(args: &[String]) -> Option<BrokerIdentity> {
    read_broker_flag(args).and_then(|value| decode_identity(&value).ok())
}

pub fn encode_identity(identity: &BrokerIdentity) -> String {
    format!(
        "p={};n={};s={}",
        encode_value(&identity.project),
        encode_value(&identity.namespace),
        encode_value(&identity.source),
    )
}

pub fn decode_identity(value: &str) -> Result<BrokerIdentity, String> {
    let mut project = None;
    let mut namespace = None;
    let mut source = None;

    for part in value.split(';') {
        let Some((key, raw_value)) = part.split_once('=') else {
            return Err("broker identity segment must use key=value form".to_string());
        };
        let decoded = decode_value(raw_value)?;
        match key {
            "p" if project.is_none() => project = Some(decoded),
            "n" if namespace.is_none() => namespace = Some(decoded),
            "s" if source.is_none() => source = Some(decoded),
            "p" | "n" | "s" => {
                return Err(format!("duplicate broker identity key {key:?}"));
            }
            other => return Err(format!("unknown broker identity key {other:?}")),
        }
    }

    Ok(BrokerIdentity {
        project: required(project, "p")?,
        namespace: required(namespace, "n")?,
        source: required(source, "s")?,
    })
}

fn required(value: Option<String>, key: &str) -> Result<String, String> {
    value.ok_or_else(|| format!("broker identity missing {key:?}"))
}
