use crate::percent;

pub const FLAG: &str = "--sidecar-stamp";
pub const VERSION: u8 = 1;

pub mod default {
    pub const NAMESPACE: &str = "default";
    pub const MODE: &str = "dev";
    pub const SOURCE: &str = "tool:sidecar";
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Stamp {
    pub version: u8,
    pub app: String,
    pub namespace: String,
    pub mode: String,
    pub source: String,
    pub endpoint: Option<String>,
}

impl Stamp {
    pub fn args(&self) -> Vec<String> {
        vec![format!("{FLAG}={}", encode(self))]
    }

    pub fn at(&self, endpoint: impl Into<String>) -> Self {
        let mut stamp = self.clone();
        stamp.endpoint = Some(endpoint.into());
        stamp
    }
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

pub fn find(args: &[String]) -> Option<Stamp> {
    flag(args).and_then(|value| decode(&value).ok())
}

pub fn encode(stamp: &Stamp) -> String {
    let mut encoded = format!(
        "v={};a={};n={};m={};s={}",
        stamp.version,
        percent::encode(&stamp.app),
        percent::encode(&stamp.namespace),
        percent::encode(&stamp.mode),
        percent::encode(&stamp.source),
    );
    if let Some(endpoint) = &stamp.endpoint {
        encoded.push_str(";e=");
        encoded.push_str(&percent::encode(endpoint));
    }
    encoded
}

pub fn decode(value: &str) -> Result<Stamp, String> {
    let mut version = None;
    let mut app = None;
    let mut namespace = None;
    let mut mode = None;
    let mut source = None;
    let mut endpoint = None;

    for part in value.split(';') {
        let Some((key, raw)) = part.split_once('=') else {
            return Err("stamp segment must use key=value form".to_string());
        };
        let decoded = percent::decode(raw)?;
        match key {
            "v" if version.is_none() => version = Some(vetted(&decoded)?),
            "a" if app.is_none() => app = Some(decoded),
            "n" if namespace.is_none() => namespace = Some(decoded),
            "m" if mode.is_none() => mode = Some(decoded),
            "s" if source.is_none() => source = Some(decoded),
            "e" if endpoint.is_none() => endpoint = Some(decoded),
            "v" | "a" | "n" | "m" | "s" | "e" => {
                return Err(format!("duplicate stamp key {key:?}"));
            }
            other => return Err(format!("unknown stamp key {other:?}")),
        }
    }

    Ok(Stamp {
        version: required(version, "v")?,
        app: required(app, "a")?,
        namespace: required(namespace, "n")?,
        mode: required(mode, "m")?,
        source: required(source, "s")?,
        endpoint,
    })
}

fn required<T>(value: Option<T>, key: &str) -> Result<T, String> {
    value.ok_or_else(|| format!("stamp missing {key:?}"))
}

fn vetted(decoded: &str) -> Result<u8, String> {
    let parsed = decoded
        .parse::<u8>()
        .map_err(|_| "stamp version must be an integer".to_string())?;
    if parsed != VERSION {
        return Err(format!("unsupported stamp version {parsed}"));
    }
    Ok(parsed)
}
