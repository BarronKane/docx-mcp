use std::env;
use std::error::Error;
use std::fmt;
use std::time::Duration;

/// Runtime configuration loaded from environment variables.
#[derive(Clone)]
pub struct DocxConfig {
    pub db_endpoint: String,
    pub db_namespace: String,
    pub db_username: Option<String>,
    pub db_password: Option<String>,
    pub registry_ttl: Option<Duration>,
    pub sweep_interval: Duration,
    pub max_entries: Option<usize>,
}

#[derive(Debug)]
pub enum ConfigError {
    InvalidNumber { name: &'static str, value: String },
    MissingPassword,
}

impl fmt::Display for ConfigError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidNumber { name, value } => {
                write!(f, "invalid {name} value: {value}")
            }
            Self::MissingPassword => write!(
                f,
                "DOCX_DB_PASSWORD is required when DOCX_DB_USERNAME is set"
            ),
        }
    }
}

impl Error for ConfigError {}

impl DocxConfig {
    pub fn from_env() -> Result<Self, ConfigError> {
        let db_endpoint =
            env::var("DOCX_DB_ENDPOINT").unwrap_or_else(|_| "ws://127.0.0.1:8000".to_string());
        let db_namespace =
            env::var("DOCX_DB_NAMESPACE").unwrap_or_else(|_| "docx".to_string());

        let db_username = env::var("DOCX_DB_USERNAME").ok();
        let db_password = env::var("DOCX_DB_PASSWORD").ok();
        if db_username.is_some() && db_password.is_none() {
            return Err(ConfigError::MissingPassword);
        }

        let ttl_secs = read_u64("DOCX_REGISTRY_TTL_SECS")?.unwrap_or(300);
        let sweep_secs = read_u64("DOCX_REGISTRY_SWEEP_SECS")?.unwrap_or(ttl_secs);
        let registry_ttl = if ttl_secs == 0 {
            None
        } else {
            Some(Duration::from_secs(ttl_secs))
        };
        let sweep_interval = Duration::from_secs(sweep_secs);

        let max_entries = read_usize("DOCX_REGISTRY_MAX")?;

        Ok(Self {
            db_endpoint,
            db_namespace,
            db_username,
            db_password,
            registry_ttl,
            sweep_interval,
            max_entries,
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
