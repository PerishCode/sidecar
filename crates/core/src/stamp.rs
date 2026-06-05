//! Stamp args protocol — a packed identity label injected into a sidecar
//! process command line so the `sidecar` tool can identify and operate on it
//! later.
//!
//! The canonical flag is `--sidecar-stamp=a=<app>;n=<namespace>;m=<mode>;s=<source>`.
//! Values are percent-encoded. Discovery only relies on this flag (not env
//! vars), so any consumer that accepts and ignores the canonical flag is
//! interoperable with the `sidecar` CLI.

pub const STAMP_FLAG: &str = "--sidecar-stamp";

pub const DEFAULT_NAMESPACE: &str = "default";
pub const DEFAULT_MODE: &str = "dev";
pub const DEFAULT_SOURCE: &str = "tool:sidecar";

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Stamp {
    pub app: String,
    pub namespace: String,
    pub mode: String,
    pub source: String,
}

impl Stamp {
    pub fn args(&self) -> Vec<String> {
        vec![format!("{STAMP_FLAG}={}", encode(self))]
    }
}

pub fn read_flag(args: &[String]) -> Option<String> {
    let prefix = format!("{STAMP_FLAG}=");
    for (index, value) in args.iter().enumerate() {
        if value == STAMP_FLAG {
            return args.get(index + 1).cloned();
        }
        if let Some(stripped) = value.strip_prefix(&prefix) {
            return Some(stripped.to_string());
        }
    }
    None
}

pub fn read_stamp(args: &[String]) -> Option<Stamp> {
    read_flag(args).and_then(|value| decode(&value).ok())
}

pub fn encode(stamp: &Stamp) -> String {
    format!(
        "a={};n={};m={};s={}",
        encode_value(&stamp.app),
        encode_value(&stamp.namespace),
        encode_value(&stamp.mode),
        encode_value(&stamp.source),
    )
}

pub fn decode(value: &str) -> Result<Stamp, String> {
    let mut app = None;
    let mut namespace = None;
    let mut mode = None;
    let mut source = None;

    for part in value.split(';') {
        let Some((key, raw_value)) = part.split_once('=') else {
            return Err("stamp segment must use key=value form".to_string());
        };
        let decoded = decode_value(raw_value)?;
        match key {
            "a" if app.is_none() => app = Some(decoded),
            "n" if namespace.is_none() => namespace = Some(decoded),
            "m" if mode.is_none() => mode = Some(decoded),
            "s" if source.is_none() => source = Some(decoded),
            "a" | "n" | "m" | "s" => {
                return Err(format!("duplicate stamp key {key:?}"));
            }
            other => return Err(format!("unknown stamp key {other:?}")),
        }
    }

    Ok(Stamp {
        app: required(app, "a")?,
        namespace: required(namespace, "n")?,
        mode: required(mode, "m")?,
        source: required(source, "s")?,
    })
}

fn required(value: Option<String>, key: &str) -> Result<String, String> {
    value.ok_or_else(|| format!("stamp missing {key:?}"))
}

fn encode_value(value: &str) -> String {
    let mut encoded = String::new();
    for byte in value.bytes() {
        if is_unreserved(byte) {
            encoded.push(byte as char);
        } else {
            encoded.push('%');
            encoded.push(hex_char(byte >> 4));
            encoded.push(hex_char(byte & 0x0f));
        }
    }
    encoded
}

fn decode_value(value: &str) -> Result<String, String> {
    let bytes = value.as_bytes();
    let mut decoded = Vec::new();
    let mut index = 0;
    while index < bytes.len() {
        match bytes[index] {
            b'%' => {
                let Some(high) = bytes.get(index + 1).and_then(|byte| hex_value(*byte)) else {
                    return Err("stamp value contains invalid percent escape".to_string());
                };
                let Some(low) = bytes.get(index + 2).and_then(|byte| hex_value(*byte)) else {
                    return Err("stamp value contains invalid percent escape".to_string());
                };
                decoded.push((high << 4) | low);
                index += 3;
            }
            byte => {
                decoded.push(byte);
                index += 1;
            }
        }
    }
    String::from_utf8(decoded).map_err(|_| "stamp value is not valid UTF-8".to_string())
}

fn is_unreserved(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'.' | b'_')
}

fn hex_char(value: u8) -> char {
    match value {
        0..=9 => (b'0' + value) as char,
        10..=15 => (b'A' + value - 10) as char,
        _ => unreachable!(),
    }
}

fn hex_value(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        b'A'..=b'F' => Some(byte - b'A' + 10),
        _ => None,
    }
}
