pub struct Config {
    pub database_url: String,
    pub supabase_jwt_secret: String,
    pub frontend_url: String,
    pub port: u16,
    pub db_max_connections: u32,
    pub db_min_connections: u32,
    pub request_timeout_secs: u64,
    pub body_limit_bytes: usize,
    pub items_page_size: i64,
    pub cache_ttl_secs: u64,
    pub cache_max_capacity: u64,
}

impl Config {
    pub fn from_env() -> Self {
        Self {
            database_url: env("DATABASE_URL"),
            supabase_jwt_secret: env("SUPABASE_JWT_SECRET"),
            frontend_url: env_or("FRONTEND_URL", "http://localhost:3000"),
            // Railway injects PORT; fall back to BACKEND_PORT for local dev
            port: std::env::var("PORT")
                .or_else(|_| std::env::var("BACKEND_PORT"))
                .unwrap_or_else(|_| "8080".to_string())
                .parse()
                .expect("PORT must be a number"),
            db_max_connections: env_or("DB_MAX_CONNECTIONS", "20").parse().expect("DB_MAX_CONNECTIONS must be a number"),
            db_min_connections: env_or("DB_MIN_CONNECTIONS", "2").parse().expect("DB_MIN_CONNECTIONS must be a number"),
            request_timeout_secs: env_or("REQUEST_TIMEOUT_SECS", "30").parse().expect("REQUEST_TIMEOUT_SECS must be a number"),
            body_limit_bytes: env_or("BODY_LIMIT_BYTES", "2097152").parse().expect("BODY_LIMIT_BYTES must be a number"),
            items_page_size: env_or("ITEMS_PAGE_SIZE", "50").parse().expect("ITEMS_PAGE_SIZE must be a number"),
            cache_ttl_secs: env_or("CACHE_TTL_SECS", "300").parse().expect("CACHE_TTL_SECS must be a number"),
            cache_max_capacity: env_or("CACHE_MAX_CAPACITY", "10000").parse().expect("CACHE_MAX_CAPACITY must be a number"),
        }
    }
}

fn env(key: &str) -> String {
    std::env::var(key).unwrap_or_else(|_| panic!("{key} must be set"))
}

fn env_or(key: &str, default: &str) -> String {
    std::env::var(key).unwrap_or_else(|_| default.to_string())
}
