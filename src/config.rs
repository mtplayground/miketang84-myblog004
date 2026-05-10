use std::{
    env,
    error::Error,
    fmt,
    net::{AddrParseError, SocketAddr},
};

use url::{ParseError as UrlParseError, Url};

#[derive(Clone)]
pub struct Config {
    pub bind_addr: SocketAddr,
    pub database_url: String,
    pub base_url: Url,
    pub session_secret: String,
    pub title: String,
    pub admin_username: String,
    pub admin_password: String,
}

impl Config {
    pub fn from_env() -> Result<Self, ConfigError> {
        Ok(Self {
            bind_addr: read_bind_addr()?,
            database_url: read_required_string("BLOG_DATABASE_URL")?,
            base_url: read_required_url("BLOG_BASE_URL")?,
            session_secret: read_min_length_string("BLOG_SESSION_SECRET", 32)?,
            title: read_required_string("BLOG_TITLE")?,
            admin_username: read_required_string("ADMIN_USERNAME")?,
            admin_password: read_required_string("ADMIN_PASSWORD")?,
        })
    }
}

#[derive(Debug)]
pub enum ConfigError {
    MissingVar(&'static str),
    EmptyVar(&'static str),
    TooShortVar {
        name: &'static str,
        min_len: usize,
    },
    InvalidBindAddr {
        value: String,
        source: AddrParseError,
    },
    InvalidUrl {
        name: &'static str,
        value: String,
        source: UrlParseError,
    },
}

impl fmt::Display for ConfigError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingVar(name) => write!(f, "missing required environment variable {name}"),
            Self::EmptyVar(name) => write!(f, "environment variable {name} cannot be empty"),
            Self::TooShortVar { name, min_len } => {
                write!(f, "environment variable {name} must be at least {min_len} bytes long")
            }
            Self::InvalidBindAddr { value, source } => {
                write!(f, "invalid BLOG_BIND_ADDR value `{value}`: {source}")
            }
            Self::InvalidUrl {
                name,
                value,
                source,
            } => {
                write!(f, "invalid {name} value `{value}`: {source}")
            }
        }
    }
}

impl Error for ConfigError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::InvalidBindAddr { source, .. } => Some(source),
            Self::InvalidUrl { source, .. } => Some(source),
            Self::MissingVar(_) | Self::EmptyVar(_) | Self::TooShortVar { .. } => None,
        }
    }
}

fn read_bind_addr() -> Result<SocketAddr, ConfigError> {
    let value =
        env::var("BLOG_BIND_ADDR").unwrap_or_else(|_| String::from("0.0.0.0:8080"));

    value.parse().map_err(|source| ConfigError::InvalidBindAddr {
        value,
        source,
    })
}

fn read_required_string(name: &'static str) -> Result<String, ConfigError> {
    let value = env::var(name).map_err(|_| ConfigError::MissingVar(name))?;
    let trimmed = value.trim();

    if trimmed.is_empty() {
        return Err(ConfigError::EmptyVar(name));
    }

    Ok(trimmed.to_string())
}

fn read_min_length_string(name: &'static str, min_len: usize) -> Result<String, ConfigError> {
    let value = read_required_string(name)?;

    if value.len() < min_len {
        return Err(ConfigError::TooShortVar { name, min_len });
    }

    Ok(value)
}

fn read_required_url(name: &'static str) -> Result<Url, ConfigError> {
    let value = read_required_string(name)?;

    Url::parse(&value).map_err(|source| ConfigError::InvalidUrl {
        name,
        value,
        source,
    })
}
