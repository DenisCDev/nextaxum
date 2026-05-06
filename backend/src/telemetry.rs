//! OpenTelemetry wiring, behind the `otel` feature flag.
//!
//! - Default build (no flag): zero deps pulled, `init` is a no-op stub so
//!   call sites stay clean.
//! - `cargo build --features otel`: pulls opentelemetry + OTLP exporter and,
//!   when `OTEL_EXPORTER_OTLP_ENDPOINT` is set, layers an OTLP tracing
//!   subscriber alongside the JSON fmt layer.
//!
//! The fmt layer keeps emitting structured JSON to stdout regardless — the
//! OTLP exporter is additive, not a replacement.

use tracing_subscriber::fmt;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;
use tracing_subscriber::EnvFilter;

#[cfg(not(feature = "otel"))]
pub fn init() {
    tracing_subscriber::registry()
        .with(EnvFilter::try_from_default_env().unwrap_or_else(|_| "backend=debug,tower_http=debug".into()))
        .with(fmt::layer().json().with_target(true).with_thread_ids(true))
        .init();
}

#[cfg(feature = "otel")]
pub fn init() {
    use opentelemetry::trace::TracerProvider;
    use opentelemetry_otlp::WithExportConfig;
    use opentelemetry_sdk::{runtime, trace as sdktrace, Resource};

    let registry = tracing_subscriber::registry()
        .with(EnvFilter::try_from_default_env().unwrap_or_else(|_| "backend=debug,tower_http=debug".into()))
        .with(fmt::layer().json().with_target(true).with_thread_ids(true));

    if let Ok(endpoint) = std::env::var("OTEL_EXPORTER_OTLP_ENDPOINT") {
        match opentelemetry_otlp::new_pipeline()
            .tracing()
            .with_exporter(
                opentelemetry_otlp::new_exporter()
                    .tonic()
                    .with_endpoint(&endpoint),
            )
            .with_trace_config(
                sdktrace::Config::default().with_resource(Resource::new(vec![
                    opentelemetry::KeyValue::new("service.name", env!("CARGO_PKG_NAME")),
                    opentelemetry::KeyValue::new("service.version", env!("CARGO_PKG_VERSION")),
                ])),
            )
            .install_batch(runtime::Tokio)
        {
            Ok(provider) => {
                let tracer = provider.tracer(env!("CARGO_PKG_NAME"));
                let otel_layer = tracing_opentelemetry::layer().with_tracer(tracer);
                registry.with(otel_layer).init();
                tracing::info!(endpoint = %endpoint, "OTLP exporter initialised");
                return;
            }
            Err(e) => {
                eprintln!("failed to initialise OTLP exporter ({e}) — falling back to stdout-only");
            }
        }
    }

    registry.init();
}
