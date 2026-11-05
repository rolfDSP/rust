//! Starts the task microservice. Run with: cargo run
//!
//! Settings come from the environment variables BIND_ADDR (default 127.0.0.1:3000),
//! REQUEST_TIMEOUT_SECS (default 10), and RUST_LOG (default info).

use std::error::Error;

use r_proj_064::config::Config;
use r_proj_064::routes;
use r_proj_064::store::TaskStore;
use tokio::net::TcpListener;
use tokio::signal;
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new("info,tower_http=debug"));
    tracing_subscriber::fmt().with_env_filter(filter).init();

    let config = Config::from_env()?;
    let app = routes::app(TaskStore::new(), config.request_timeout);

    let listener = TcpListener::bind(config.bind_addr).await?;
    tracing::info!("listening on {}", listener.local_addr()?);

    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await?;

    tracing::info!("shut down cleanly");
    Ok(())
}

/// Completes on Ctrl+C or SIGTERM, the signal Docker and Kubernetes send to stop a
/// service. axum then stops accepting connections and lets running requests finish.
async fn shutdown_signal() {
    let ctrl_c = async {
        signal::ctrl_c().await.expect("cannot listen for Ctrl+C");
    };

    #[cfg(unix)]
    let terminate = async {
        signal::unix::signal(signal::unix::SignalKind::terminate())
            .expect("cannot listen for SIGTERM")
            .recv()
            .await;
    };
    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {}
        _ = terminate => {}
    }
    tracing::info!("shutdown signal received");
}
