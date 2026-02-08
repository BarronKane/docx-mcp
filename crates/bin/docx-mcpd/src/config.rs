use std::env;
use std::error::Error;
use std::fmt;
use std::net::SocketAddr;
use std::time::Duration;

/// Runtime configuration loaded from environment variables.
#[derive(Clone)]
pub struct DocxConfig {
    pub db_namespace: String,
    pub registry_ttl: Option<Duration>,
    pub sweep_interval: Duration,
    pub max_entries: Option<usize>,
    pub mcp_http_addr: SocketAddr,
    pub ingest_addr: SocketAddr,
    pub ingest_timeout: Duration,
    pub ingest_max_body_bytes: usize,
}

#[derive(Debug)]
pub enum ConfigError {
    InvalidNumber { name: &'static str, value: String },
    InvalidAddress { name: &'static str, value: String },
}

impl fmt::Display for ConfigError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidNumber { name, value } => {
                write!(f, "invalid {name} value: {value}")
            }
            Self::InvalidAddress { name, value } => {
                write!(f, "invalid {name} address: {value}")
            }
        }
    }
}

impl Error for ConfigError {}

impl DocxConfig {
    pub fn from_env() -> Result<Self, ConfigError> {
        let db_namespace =
            env::var("DOCX_DB_NAMESPACE").unwrap_or_else(|_| "docx".to_string());

        let ttl_secs = read_u64("DOCX_REGISTRY_TTL_SECS")?.unwrap_or(300);
        let sweep_secs = read_u64("DOCX_REGISTRY_SWEEP_SECS")?.unwrap_or(ttl_secs);
        let registry_ttl = if ttl_secs == 0 {
            None
        } else {
            Some(Duration::from_secs(ttl_secs))
        };
        let sweep_interval = Duration::from_secs(sweep_secs);

        let max_entries = read_usize("DOCX_REGISTRY_MAX")?;

        let mcp_http_addr = read_addr("DOCX_MCP_HTTP_ADDR")?.unwrap_or_else(|| {
            "127.0.0.1:4020"
                .parse()
                .expect("valid default MCP HTTP address")
        });
        let ingest_addr = read_addr("DOCX_INGEST_ADDR")?.unwrap_or_else(|| {
            "127.0.0.1:4010"
                .parse()
                .expect("valid default ingest address")
        });
        let ingest_timeout_secs = read_u64("DOCX_INGEST_TIMEOUT_SECS")?.unwrap_or(30);
        let ingest_timeout = Duration::from_secs(ingest_timeout_secs);
        let ingest_max_body_bytes =
            read_usize("DOCX_INGEST_MAX_BODY_BYTES")?.unwrap_or(25 * 1024 * 1024);

        Ok(Self {
            db_namespace,
            registry_ttl,
            sweep_interval,
            max_entries,
            mcp_http_addr,
            ingest_addr,
            ingest_timeout,
            ingest_max_body_bytes,
        })
    }

    pub fn db_name_for_solution(solution: &str) -> String {
        solution.to_string()
    }
}

fn read_u64(name: &'static str) -> Result<Option<u64>, ConfigError> {
    let Ok(value) = env::var(name) else {
        return Ok(None);
    };
    let parsed = value
        .parse::<u64>()
        .map_err(|_| ConfigError::InvalidNumber { name, value })?;
    Ok(Some(parsed))
}

fn read_usize(name: &'static str) -> Result<Option<usize>, ConfigError> {
    let Ok(value) = env::var(name) else {
        return Ok(None);
    };
    let parsed = value
        .parse::<usize>()
        .map_err(|_| ConfigError::InvalidNumber { name, value })?;
    Ok(Some(parsed))
}

fn read_addr(name: &'static str) -> Result<Option<SocketAddr>, ConfigError> {
    let Ok(value) = env::var(name) else {
        return Ok(None);
    };
    let parsed = value
        .parse::<SocketAddr>()
        .map_err(|_| ConfigError::InvalidAddress { name, value })?;
    Ok(Some(parsed))
}
