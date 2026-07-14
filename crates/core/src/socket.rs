use std::path::PathBuf;

const UNIX: &str = "unix://";
const TCP: &str = "tcp://";

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Endpoint {
    Unix(PathBuf),
    Tcp(String),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Error {
    message: String,
}

impl Endpoint {
    pub fn parse(value: &str) -> Result<Self, Error> {
        let value = value.trim();
        if value.is_empty() {
            return Err(Error::new("socket endpoint must not be empty"));
        }

        if let Some(path) = value.strip_prefix(UNIX) {
            if path.is_empty() {
                return Err(Error::new("unix socket endpoint must include a path"));
            }
            if !path.starts_with('/') {
                return Err(Error::new(
                    "unix socket endpoint must use unix:///absolute/path.sock form",
                ));
            }
            return Ok(Self::Unix(PathBuf::from(path)));
        }

        if let Some(addr) = value.strip_prefix(TCP) {
            validate(addr)?;
            return Ok(Self::Tcp(addr.to_string()));
        }

        Err(Error::new(
            "socket endpoint must use unix:///path.sock form, or tcp://host:port on non-Unix platforms",
        ))
    }
}

impl std::fmt::Display for Endpoint {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Unix(path) => write!(formatter, "unix://{}", path.display()),
            Self::Tcp(addr) => write!(formatter, "{TCP}{addr}"),
        }
    }
}

impl Error {
    fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

impl std::fmt::Display for Error {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl std::error::Error for Error {}

fn validate(addr: &str) -> Result<(), Error> {
    if addr.trim().is_empty() {
        return Err(Error::new("tcp socket endpoint must include host:port"));
    }
    if !addr.contains(':') {
        return Err(Error::new("tcp socket endpoint must include a port"));
    }
    Ok(())
}
