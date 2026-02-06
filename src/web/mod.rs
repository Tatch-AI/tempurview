pub mod handlers;

use crate::client::TemporalClient;
use crate::config::Config;
use axum::{
    routing::{get, post},
    Router,
};
use std::sync::Arc;

/// Shared state for all Axum handlers.
#[derive(Clone)]
pub struct AppState {
    pub client: Arc<dyn TemporalClient>,
    pub config: Config,
}

/// Build the Axum router with all routes.
pub fn router(state: AppState) -> Router {
    Router::new()
        .route("/", get(handlers::index))
        .route("/api/workflows", get(handlers::list_workflows))
        .route("/api/workflows/{id}", get(handlers::get_workflow))
        .route(
            "/api/workflows/{id}/activities",
            get(handlers::list_activities),
        )
        .route("/api/workflows/{id}/events", get(handlers::list_events))
        .route("/api/insights", get(handlers::get_insights))
        .route(
            "/api/workflows/{id}/cancel",
            post(handlers::cancel_workflow),
        )
        .route(
            "/api/workflows/{id}/terminate",
            post(handlers::terminate_workflow),
        )
        .with_state(state)
}

/// Start the web server on the given bind address and port.
pub async fn run_server(
    client: Arc<dyn TemporalClient>,
    config: Config,
    bind: &str,
    port: u16,
) -> color_eyre::Result<()> {
    let state = AppState { client, config };
    let app = router(state);

    let addr = format!("{bind}:{port}");
    let listener = tokio::net::TcpListener::bind(&addr).await?;

    eprintln!("Tempurview web UI running at http://{addr}");
    eprintln!("Press Ctrl+C to stop");

    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await?;

    Ok(())
}

async fn shutdown_signal() {
    tokio::signal::ctrl_c()
        .await
        .expect("failed to install Ctrl+C handler");
    eprintln!("\nShutting down...");
}
