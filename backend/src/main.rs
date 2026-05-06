use std::net::SocketAddr;

use backend::config::Config;
use backend::jobs::Jobs;
use backend::routes::create_router;
use backend::state::AppState;
use backend::telemetry;
use tokio::net::TcpListener;

#[tokio::main]
async fn main() {
    dotenvy::dotenv().ok();
    telemetry::init();

    let config = match Config::from_env() {
        Ok(cfg) => cfg,
        Err(e) => {
            // Log AND print so the failure is visible whether stderr is captured
            // (Docker/Railway) or not.
            tracing::error!(error = %e, "configuration error — refusing to start");
            eprintln!("Configuration error: {e}");
            std::process::exit(2);
        }
    };
    let port = config.port;
    let state = AppState::new(config).await;
    let jobs = Jobs::spawn(state.db().clone());
    let app = create_router(state);

    let addr = SocketAddr::from(([0, 0, 0, 0], port));
    let listener = TcpListener::bind(addr).await.expect("failed to bind");
    tracing::info!("listening on {addr}");

    axum::serve(listener, app.into_make_service_with_connect_info::<SocketAddr>())
        .with_graceful_shutdown(shutdown_signal())
        .await
        .expect("server error");

    // Hyper has finished accepting; tell every cron loop to drain.
    jobs.shutdown().await;
}

async fn shutdown_signal() {
    let ctrl_c = tokio::signal::ctrl_c();

    #[cfg(unix)]
    {
        let mut sigterm =
            tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate()).unwrap();
        tokio::select! {
            _ = ctrl_c => {},
            _ = sigterm.recv() => {},
        }
    }

    #[cfg(not(unix))]
    {
        ctrl_c.await.ok();
    }

    tracing::info!("shutdown signal received, draining connections...");
}
