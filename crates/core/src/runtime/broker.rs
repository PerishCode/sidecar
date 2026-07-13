use crate::percent;
use crate::stamp;
use serde::{Deserialize, Serialize};
use std::io::{BufRead, BufReader, Write};
use std::net::{SocketAddr, TcpStream};
use std::time::Duration;

pub const FLAG: &str = "--sidecar-broker";
pub const VERSION: u8 = 1;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Identity {
    pub project: String,
    pub namespace: String,
    pub source: String,
}

impl Identity {
    pub fn new(project: impl Into<String>, namespace: impl Into<String>) -> Self {
        Self {
            project: project.into(),
            namespace: namespace.into(),
            source: stamp::default::SOURCE.to_string(),
        }
    }

    pub fn args(&self) -> Vec<String> {
        vec![format!("{FLAG}={}", self.encode())]
    }

    pub fn hello(&self) -> Request {
        Request::Hello {
            protocol: VERSION,
            project: self.project.clone(),
            namespace: self.namespace.clone(),
        }
    }

    pub fn validate(&self, request: &Request) -> Result<Response, String> {
        match request {
            Request::Hello {
                protocol,
                project,
                namespace,
            } if *protocol == VERSION
                && project == &self.project
                && namespace == &self.namespace =>
            {
                Ok(Response::Ok {
                    protocol: VERSION,
                    project: self.project.clone(),
                    namespace: self.namespace.clone(),
                })
            }
            Request::Hello { .. } => Err(
                "broker hello did not match expected protocol, project, or namespace".to_string(),
            ),
        }
    }

    pub fn endpoint(&self, timeout: Duration) -> Result<Option<SocketAddr>, String> {
        let hits = crate::runtime::process::Broker::discover(&self.project, &self.namespace)?;
        for hit in hits {
            let listeners = crate::runtime::tcp::listeners(hit.pid)?;
            for addr in listeners {
                if self.probe(addr, timeout)? {
                    return Ok(Some(addr));
                }
            }
        }
        Ok(None)
    }

    pub fn probe(&self, addr: SocketAddr, timeout: Duration) -> Result<bool, String> {
        let Ok(mut stream) = TcpStream::connect_timeout(&addr, timeout) else {
            return Ok(false);
        };
        stream
            .set_read_timeout(Some(timeout))
            .map_err(|err| format!("failed to set broker read timeout: {err}"))?;
        stream
            .set_write_timeout(Some(timeout))
            .map_err(|err| format!("failed to set broker write timeout: {err}"))?;
        let request = serde_json::to_string(&self.hello()).map_err(|err| err.to_string())?;
        stream
            .write_all(request.as_bytes())
            .and_then(|_| stream.write_all(b"\n"))
            .map_err(|err| format!("failed to write broker hello: {err}"))?;

        let mut line = String::new();
        BufReader::new(stream)
            .read_line(&mut line)
            .map_err(|err| format!("failed to read broker hello response: {err}"))?;
        let Ok(response) = serde_json::from_str::<Response>(line.trim()) else {
            return Ok(false);
        };
        Ok(matches!(
            response,
            Response::Ok {
                protocol: VERSION,
                project,
                namespace,
            } if project == self.project && namespace == self.namespace
        ))
    }

    pub fn encode(&self) -> String {
        format!(
            "p={};n={};s={}",
            percent::encode(&self.project),
            percent::encode(&self.namespace),
            percent::encode(&self.source),
        )
    }

    pub fn decode(value: &str) -> Result<Identity, String> {
        let mut project = None;
        let mut namespace = None;
        let mut source = None;

        for part in value.split(';') {
            let Some((key, raw)) = part.split_once('=') else {
                return Err("broker identity segment must use key=value form".to_string());
            };
            let decoded = percent::decode(raw)?;
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

        Ok(Identity {
            project: required(project, "p")?,
            namespace: required(namespace, "n")?,
            source: required(source, "s")?,
        })
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "kind")]
pub enum Request {
    #[serde(rename = "hello")]
    Hello {
        protocol: u8,
        project: String,
        namespace: String,
    },
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "kind")]
pub enum Response {
    #[serde(rename = "hello_ok")]
    Ok {
        protocol: u8,
        project: String,
        namespace: String,
    },
    #[serde(rename = "hello_error")]
    Error { message: String },
}

pub fn flag(args: &[String]) -> Option<String> {
    let prefix = format!("{FLAG}=");
    for (index, value) in args.iter().enumerate() {
        if value == FLAG {
            return args.get(index + 1).cloned();
        }
        if let Some(stripped) = value.strip_prefix(&prefix) {
            return Some(stripped.to_string());
        }
    }
    None
}

pub fn read(args: &[String]) -> Option<Identity> {
    flag(args).and_then(|value| Identity::decode(&value).ok())
}

fn required(value: Option<String>, key: &str) -> Result<String, String> {
    value.ok_or_else(|| format!("broker identity missing {key:?}"))
}
