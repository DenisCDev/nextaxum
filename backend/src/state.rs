use std::sync::Arc;
use std::time::Duration;

use moka::future::Cache;
use sqlx::postgres::PgPoolOptions;
use sqlx::PgPool;
use uuid::Uuid;

use crate::config::Config;
use crate::models::Item;

#[derive(Clone)]
pub struct AppState {
    pub inner: Arc<AppStateInner>,
}

pub struct AppStateInner {
    pub db: PgPool,
    pub jwt_secret: String,
    pub frontend_url: String,
    pub config: Config,
    pub items_cache: Cache<Uuid, Vec<Item>>,
}

impl AppState {
    pub async fn new(config: Config) -> Self {
        let pool = PgPoolOptions::new()
            .max_connections(config.db_max_connections)
            .min_connections(config.db_min_connections)
            .acquire_timeout(Duration::from_secs(3))
            .idle_timeout(Duration::from_secs(600))
            .max_lifetime(Duration::from_secs(1800))
            .connect(&config.database_url)
            .await
            .expect("failed to connect to database");

        sqlx::migrate!()
            .run(&pool)
            .await
            .expect("failed to run migrations");

        let items_cache = Cache::builder()
            .max_capacity(config.cache_max_capacity)
            .time_to_live(Duration::from_secs(config.cache_ttl_secs))
            .build();

        Self {
            inner: Arc::new(AppStateInner {
                db: pool,
                jwt_secret: config.supabase_jwt_secret.clone(),
                frontend_url: config.frontend_url.clone(),
                items_cache,
                config,
            }),
        }
    }

    pub fn db(&self) -> &PgPool {
        &self.inner.db
    }
}
