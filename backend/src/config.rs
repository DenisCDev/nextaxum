use std::env::VarError;
use std::num::ParseIntError;

#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    #[error("required env `{0}` is not set")]
    Missing(&'static str),
    #[error("env `{key}` is not a valid {expected}: {source}")]
    InvalidNumber {
        key: &'static str,
        expected: &'static str,
        #[source]
        source: ParseIntError,
    },
    #[error("env `{key}` is not a valid URL: {value}")]
    InvalidUrl { key: &'static str, value: String },
}

#[derive(Debug, Clone)]
pub struct Config {
    pub database_url: String,
    pub supabase_jwt_secret: String,
    pub supabase_jwks_url: Option<String>,
    pub jwks_ttl_secs: u64,
    pub frontend_url: String,
    pub port: u16,
    pub db_max_connections: u32,
    pub db_min_connections: u32,
    pub request_timeout_secs: u64,
    pub body_limit_bytes: usize,
    pub items_page_size: i64,
    pub rate_limit_per_sec: u64,
    pub rate_limit_burst: u32,
    /// Shared secret used to verify HMAC-SHA256 signatures on
    /// `POST /webhooks/{provider}`. None disables the endpoint.
    pub webhook_secret: Option<String>,
}

impl Config {
    /// Validates and parses every environment variable up-front so a misconfigured
    /// deployment fails on startup with a precise error rather than panicking on
    /// the first request that touches a bad value.
    pub fn from_env() -> Result<Self, ConfigError> {
        Ok(Self {
            database_url: required("DATABASE_URL")?,
            supabase_jwt_secret: required("SUPABASE_JWT_SECRET")?,
            supabase_jwks_url: optional_url("SUPABASE_JWKS_URL")?,
            jwks_ttl_secs: parse_or("JWKS_TTL_SECS", "3600", "u64")?,
            frontend_url: optional("FRONTEND_URL")?
                .unwrap_or_else(|| "http://localhost:3000".to_string()),
            port: parse_port()?,
            db_max_connections: parse_or("DB_MAX_CONNECTIONS", "20", "u32")?,
            db_min_connections: parse_or("DB_MIN_CONNECTIONS", "2", "u32")?,
            request_timeout_secs: parse_or("REQUEST_TIMEOUT_SECS", "30", "u64")?,
            body_limit_bytes: parse_or("BODY_LIMIT_BYTES", "2097152", "usize")?,
            items_page_size: parse_or("ITEMS_PAGE_SIZE", "50", "i64")?,
            rate_limit_per_sec: parse_or("RATE_LIMIT_PER_SEC", "10", "u64")?,
            rate_limit_burst: parse_or("RATE_LIMIT_BURST", "20", "u32")?,
            webhook_secret: optional("WEBHOOK_SECRET")?,
        })
    }
}

fn required(key: &'static str) -> Result<String, ConfigError> {
    match std::env::var(key) {
        Ok(v) if !v.is_empty() => Ok(v),
        Ok(_) | Err(VarError::NotPresent) => Err(ConfigError::Missing(key)),
        Err(VarError::NotUnicode(_)) => Err(ConfigError::Missing(key)),
    }
}

fn optional(key: &'static str) -> Result<Option<String>, ConfigError> {
    match std::env::var(key) {
        Ok(v) if v.is_empty() => Ok(None),
        Ok(v) => Ok(Some(v)),
        Err(VarError::NotPresent) => Ok(None),
        Err(VarError::NotUnicode(_)) => Err(ConfigError::Missing(key)),
    }
}

fn optional_url(key: &'static str) -> Result<Option<String>, ConfigError> {
    let Some(value) = optional(key)? else {
        return Ok(None);
    };
    if !(value.starts_with("http://") || value.starts_with("https://")) {
        return Err(ConfigError::InvalidUrl { key, value });
    }
    Ok(Some(value))
}

fn parse_or<T>(key: &'static str, default: &str, expected: &'static str) -> Result<T, ConfigError>
where
    T: std::str::FromStr<Err = ParseIntError>,
{
    let raw = std::env::var(key).unwrap_or_else(|_| default.to_string());
    raw.parse::<T>()
        .map_err(|source| ConfigError::InvalidNumber {
            key,
            expected,
            source,
        })
}

fn parse_port() -> Result<u16, ConfigError> {
    // Railway injects PORT; fall back to BACKEND_PORT for local dev.
    let raw = std::env::var("PORT")
        .or_else(|_| std::env::var("BACKEND_PORT"))
        .unwrap_or_else(|_| "8080".to_string());
    raw.parse::<u16>()
        .map_err(|source| ConfigError::InvalidNumber {
            key: "PORT",
            expected: "u16",
            source,
        })
}
