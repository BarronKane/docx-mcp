use clap::{Parser, builder::BoolishValueParser};
use std::error::Error;
use std::fmt;
use std::net::SocketAddr;
use std::time::Duration;

const DEFAULT_DB_NAMESPACE: &str = "docx";
const DEFAULT_MCP_HTTP_ADDR: &str = "127.0.0.1:4020";
const DEFAULT_INGEST_ADDR: &str = "127.0.0.1:4010";
const DEFAULT_REGISTRY_TTL_SECS: u64 = 300;
const DEFAULT_REGISTRY_HEALTH_CHECK_SECS: u64 = 60;
const DEFAULT_INGEST_TIMEOUT_SECS: u64 = 30;
const DEFAULT_INGEST_MAX_BODY_BYTES: usize = 25 * 1024 * 1024;

#[derive(Parser, Debug)]
#[command(name = "docx-mcpd", version, about = "Docx MCP daemon.")]
#[allow(clippy::struct_excessive_bools)]
struct CliArgs {
    #[arg(long, env = "DOCX_DB_NAMESPACE", default_value = DEFAULT_DB_NAMESPACE)]
    db_namespace: String,

    #[arg(
        long,
        env = "DOCX_REGISTRY_TTL_SECS",
        default_value_t = DEFAULT_REGISTRY_TTL_SECS
    )]
    registry_ttl_secs: u64,

    #[arg(long, env = "DOCX_REGISTRY_SWEEP_SECS")]
    registry_sweep_secs: Option<u64>,

    #[arg(
        long,
        env = "DOCX_REGISTRY_HEALTH_CHECK_SECS",
        default_value_t = DEFAULT_REGISTRY_HEALTH_CHECK_SECS
    )]
    registry_health_check_secs: u64,

    #[arg(long, env = "DOCX_REGISTRY_MAX")]
    max_entries: Option<usize>,

    #[arg(
        long = "stdio",
        env = "DOCX_ENABLE_STDIO",
        default_value_t = false,
        value_parser = BoolishValueParser::new()
    )]
    enable_stdio: bool,

    #[arg(
        long,
        env = "DOCX_MCP_SERVE",
        default_value_t = true,
        value_parser = BoolishValueParser::new()
    )]
    mcp_serve: bool,

    #[arg(
        long,
        env = "DOCX_INGEST_SERVE",
        default_value_t = true,
        value_parser = BoolishValueParser::new()
    )]
    ingest_serve: bool,

    #[arg(long, env = "DOCX_MCP_HTTP_ADDR", default_value = DEFAULT_MCP_HTTP_ADDR)]
    mcp_http_addr: SocketAddr,

    #[arg(long, env = "DOCX_INGEST_ADDR", default_value = DEFAULT_INGEST_ADDR)]
    ingest_addr: SocketAddr,

    #[arg(
        long,
        env = "DOCX_INGEST_TIMEOUT_SECS",
        default_value_t = DEFAULT_INGEST_TIMEOUT_SECS
    )]
    ingest_timeout_secs: u64,

    #[arg(
        long,
        env = "DOCX_INGEST_MAX_BODY_BYTES",
        default_value_t = DEFAULT_INGEST_MAX_BODY_BYTES
    )]
    ingest_max_body_bytes: usize,

    #[arg(
        long,
        env = "DOCX_DB_IN_MEMORY",
        default_value_t = true,
        value_parser = BoolishValueParser::new()
    )]
    db_in_memory: bool,

    #[arg(long, env = "DOCX_DB_URI")]
    db_uri: Option<String>,

    #[arg(long, env = "DOCX_DB_USERNAME")]
    db_username: Option<String>,

    #[arg(long, env = "DOCX_DB_PASSWORD")]
    db_password: Option<String>,

    #[arg(
        long,
        env = "DOCX_TEST",
        default_value_t = false,
        value_parser = BoolishValueParser::new()
    )]
    test_mode: bool,
}

/// Runtime configuration loaded from CLI arguments and environment variables.
#[derive(Clone)]
#[allow(clippy::struct_excessive_bools)]
pub struct DocxConfig {
    pub db_namespace: String,
    pub registry_ttl: Option<Duration>,
    pub sweep_interval: Duration,
    pub max_entries: Option<usize>,
    pub health_check_after: Duration,
    pub enable_stdio: bool,
    pub mcp_serve: bool,
    pub ingest_serve: bool,
    pub mcp_http_addr: SocketAddr,
    pub ingest_addr: SocketAddr,
    pub ingest_timeout: Duration,
    pub ingest_max_body_bytes: usize,
    pub db_in_memory: bool,
    pub db_uri: Option<String>,
    pub db_username: Option<String>,
    pub db_password: Option<String>,
    pub test_mode: bool,
}

#[derive(Debug)]
pub enum ConfigError {
    MissingSetting(&'static str),
    InvalidSetting { name: &'static str, value: String },
}

impl fmt::Display for ConfigError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingSetting(name) => write!(f, "missing required setting: {name}"),
            Self::InvalidSetting { name, value } => {
                write!(f, "invalid {name} value: {value}")
            }
        }
    }
}

impl Error for ConfigError {}

impl DocxConfig {
    pub fn from_args() -> Result<Self, ConfigError> {
        let args = CliArgs::parse();
        Self::try_from(args)
    }

    pub fn db_name_for_solution(solution: &str) -> String {
        solution.to_string()
    }
}

impl TryFrom<CliArgs> for DocxConfig {
    type Error = ConfigError;

    fn try_from(args: CliArgs) -> Result<Self, Self::Error> {
        let registry_ttl = if args.registry_ttl_secs == 0 {
            None
        } else {
            Some(Duration::from_secs(args.registry_ttl_secs))
        };
        let sweep_secs = args.registry_sweep_secs.unwrap_or(args.registry_ttl_secs);
        let sweep_interval = Duration::from_secs(sweep_secs);

        let db_uri = args.db_uri.filter(|value| !value.trim().is_empty());
        let db_username = args.db_username.filter(|value| !value.trim().is_empty());
        let db_password = args.db_password.filter(|value| !value.trim().is_empty());

        let db_in_memory = args.db_in_memory || db_uri.is_none();

        if !db_in_memory {
            if db_uri.is_none() {
                return Err(ConfigError::MissingSetting("DOCX_DB_URI"));
            }
            if db_username.is_none() {
                return Err(ConfigError::MissingSetting("DOCX_DB_USERNAME"));
            }
            if db_password.is_none() {
                return Err(ConfigError::MissingSetting("DOCX_DB_PASSWORD"));
            }
        }

        if args.db_namespace.trim().is_empty() {
            return Err(ConfigError::InvalidSetting {
                name: "DOCX_DB_NAMESPACE",
                value: args.db_namespace,
            });
        }

        Ok(Self {
            db_namespace: args.db_namespace,
            registry_ttl,
            sweep_interval,
            max_entries: args.max_entries,
            health_check_after: Duration::from_secs(args.registry_health_check_secs),
            enable_stdio: args.enable_stdio,
            mcp_serve: args.mcp_serve,
            ingest_serve: args.ingest_serve,
            mcp_http_addr: args.mcp_http_addr,
            ingest_addr: args.ingest_addr,
            ingest_timeout: Duration::from_secs(args.ingest_timeout_secs),
            ingest_max_body_bytes: args.ingest_max_body_bytes,
            db_in_memory,
            db_uri,
            db_username,
            db_password,
            test_mode: args.test_mode,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn base_args() -> CliArgs {
        CliArgs {
            db_namespace: DEFAULT_DB_NAMESPACE.to_string(),
            registry_ttl_secs: DEFAULT_REGISTRY_TTL_SECS,
            registry_sweep_secs: None,
            registry_health_check_secs: DEFAULT_REGISTRY_HEALTH_CHECK_SECS,
            max_entries: None,
            enable_stdio: false,
            mcp_serve: true,
            ingest_serve: true,
            mcp_http_addr: DEFAULT_MCP_HTTP_ADDR.parse().expect("valid MCP addr"),
            ingest_addr: DEFAULT_INGEST_ADDR.parse().expect("valid ingest addr"),
            ingest_timeout_secs: DEFAULT_INGEST_TIMEOUT_SECS,
            ingest_max_body_bytes: DEFAULT_INGEST_MAX_BODY_BYTES,
            db_in_memory: true,
            db_uri: None,
            db_username: None,
            db_password: None,
            test_mode: false,
        }
    }

    #[test]
    fn defaults_to_in_memory_when_db_uri_missing() {
        let mut args = base_args();
        args.db_in_memory = false;
        args.db_uri = None;
        args.db_username = None;
        args.db_password = None;

        let config = DocxConfig::try_from(args).expect("config should parse");

        assert!(config.db_in_memory);
        assert!(config.db_uri.is_none());
    }
}
