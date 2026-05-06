use std::net::SocketAddr;

use backend::config::Config;
use backend::routes::create_router;
use backend::state::AppState;
use tokio::net::TcpListener;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

#[tokio::main]
async fn main() {
    dotenvy::dotenv().ok();

    tracing_subscriber::registry()
        .with(EnvFilter::try_from_default_env().unwrap_or_else(|_| "backend=debug,tower_http=debug".into()))
        .with(
            tracing_subscriber::fmt::layer()
                .json()
                .with_target(true)
                .with_thread_ids(true),
        )
        .init();

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
    let app = create_router(state);

    let addr = SocketAddr::from(([0, 0, 0, 0], port));
    let listener = TcpListener::bind(addr).await.expect("failed to bind");
    tracing::info!("listening on {addr}");

    axum::serve(listener, app.into_make_service_with_connect_info::<SocketAddr>())
        .with_graceful_shutdown(shutdown_signal())
        .await
        .expect("server error");
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
