use std::sync::Arc;
use std::time::{Duration, Instant};

use jsonwebtoken::jwk::JwkSet;
use parking_lot::RwLock;

/// JWKS cache for asymmetric (RS256/ES256/EdDSA) Supabase JWT verification.
/// Refreshes from `${SUPABASE_URL}/auth/v1/.well-known/jwks.json` when stale.
#[derive(Clone)]
pub struct JwksCache {
    url: String,
    ttl: Duration,
    inner: Arc<RwLock<Option<Cached>>>,
    client: reqwest::Client,
}

struct Cached {
    keys: JwkSet,
    fetched_at: Instant,
}

impl JwksCache {
    pub fn new(url: String, ttl_secs: u64) -> Self {
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(5))
            .build()
            .expect("failed to build reqwest client");
        Self {
            url,
            ttl: Duration::from_secs(ttl_secs),
            inner: Arc::new(RwLock::new(None)),
            client,
        }
    }

    pub async fn get(&self) -> anyhow::Result<JwkSet> {
        if let Some(cached) = self.inner.read().as_ref() {
            if cached.fetched_at.elapsed() < self.ttl {
                return Ok(cached.keys.clone());
            }
        }

        let keys: JwkSet = self
            .client
            .get(&self.url)
            .send()
            .await?
            .error_for_status()?
            .json()
            .await?;

        *self.inner.write() = Some(Cached {
            keys: keys.clone(),
            fetched_at: Instant::now(),
        });

        Ok(keys)
    }
}
