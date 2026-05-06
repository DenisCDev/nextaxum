use std::sync::Arc;
use std::time::Duration;

use sqlx::postgres::{PgConnectOptions, PgPoolOptions};
use sqlx::PgPool;
use std::str::FromStr;

use crate::config::Config;

#[derive(Clone)]
pub struct AppState {
    pub inner: Arc<AppStateInner>,
}

pub struct AppStateInner {
    pub db: PgPool,
    pub jwt_secret: String,
    pub frontend_url: String,
    pub config: Config,
    pub jwks: Option<crate::middleware::jwks::JwksCache>,
}

impl AppState {
    pub async fn new(config: Config) -> Self {
        // Supavisor / PgBouncer transaction mode (port 6543 or pgbouncer=true)
        // does not support prepared statements. Disable the cache so query macros
        // fall back to the simple-query path. Direct connection (5432) keeps the cache.
        // Backends should prefer the direct connection — pooler is for serverless.
        let mut connect_opts = PgConnectOptions::from_str(&config.database_url)
            .expect("invalid DATABASE_URL");
        let url_lower = config.database_url.to_lowercase();
        if url_lower.contains(":6543") || url_lower.contains("pgbouncer=true") {
            connect_opts = connect_opts.statement_cache_capacity(0);
            tracing::warn!("transaction-mode pooler detected — prepared statement cache disabled (use direct 5432 in persistent backends)");
        }

        let pool = PgPoolOptions::new()
            .max_connections(config.db_max_connections)
            .min_connections(config.db_min_connections)
            .acquire_timeout(Duration::from_secs(3))
            .idle_timeout(Duration::from_secs(600))
            .max_lifetime(Duration::from_secs(1800))
            .connect_with(connect_opts)
            .await
            .expect("failed to connect to database");

        sqlx::migrate!()
            .run(&pool)
            .await
            .expect("failed to run migrations");

        let jwks = config
            .supabase_jwks_url
            .as_ref()
            .map(|url| crate::middleware::jwks::JwksCache::new(url.clone(), config.jwks_ttl_secs));

        Self {
            inner: Arc::new(AppStateInner {
                db: pool,
                jwt_secret: config.supabase_jwt_secret.clone(),
                frontend_url: config.frontend_url.clone(),
                jwks,
                config,
            }),
        }
    }

    pub fn db(&self) -> &PgPool {
        &self.inner.db
    }
}
