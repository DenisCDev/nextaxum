//! Background jobs spawned at startup. Each job is a `tokio::time::interval`
//! loop that respects a single CancellationToken so SIGTERM cleanly stops
//! every job before the runtime shuts down.

use std::time::Duration;

use chrono::Duration as ChronoDuration;
use sqlx::PgPool;
use tokio::task::JoinSet;
use tokio_util::sync::CancellationToken;

use crate::extractors::idempotency;

/// Handle held by main(); call `.shutdown().await` to ask every job loop to
/// drain and exit. Drops the JoinSet, which awaits all spawned tasks.
pub struct Jobs {
    set: JoinSet<()>,
    cancel: CancellationToken,
}

impl Jobs {
    pub fn spawn(pool: PgPool) -> Self {
        let cancel = CancellationToken::new();
        let mut set = JoinSet::new();

        set.spawn(idempotency_cleanup(pool.clone(), cancel.clone()));

        Self { set, cancel }
    }

    pub async fn shutdown(mut self) {
        self.cancel.cancel();
        while self.set.join_next().await.is_some() {}
    }
}

/// Prune idempotency_keys older than 24h. The Stripe convention is "remember
/// for 24 hours" — keys you ask for outside that window are no longer
/// guaranteed to short-circuit, which is fine because retries that span a
/// full day are pathological.
async fn idempotency_cleanup(pool: PgPool, cancel: CancellationToken) {
    let mut tick = tokio::time::interval(Duration::from_secs(60 * 60));
    // Skip the immediate first tick so the job doesn't run during startup
    // alongside migration / pool warm-up.
    tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
    tick.tick().await;

    loop {
        tokio::select! {
            _ = cancel.cancelled() => {
                tracing::info!("idempotency_cleanup: shutdown signal received");
                return;
            }
            _ = tick.tick() => {
                match idempotency::cleanup_older_than(&pool, ChronoDuration::hours(24)).await {
                    Ok(rows) if rows > 0 => {
                        tracing::info!(rows, "idempotency_cleanup: pruned stale keys")
                    }
                    Ok(_) => {}
                    Err(e) => tracing::warn!(error = %e, "idempotency_cleanup failed"),
                }
            }
        }
    }
}
