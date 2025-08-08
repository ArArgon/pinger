use crate::metric::PingMetrics;
use axum::{extract::State, http::StatusCode, response::IntoResponse, routing::get, Router};
use prometheus_client::encoding::text::encode;
use std::sync::Arc;
use tower_http::cors::CorsLayer;

pub type SharedMetrics = Arc<PingMetrics>;

pub fn create_metrics_router(metrics: SharedMetrics) -> Router {
    Router::new()
        .route("/metrics", get(metrics_handler))
        .route("/health", get(health_handler))
        .layer(CorsLayer::permissive())
        .with_state(metrics)
}

async fn metrics_handler(State(metrics): State<SharedMetrics>) -> impl IntoResponse {
    let mut buffer = String::new();

    match encode(&mut buffer, &metrics.registry) {
        Ok(_) => (StatusCode::OK, buffer).into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Failed to encode metrics: {}", e),
        )
            .into_response(),
    }
}

async fn health_handler() -> impl IntoResponse {
    (StatusCode::OK, "{\"status\": \"ok\"}")
}

pub async fn start_metrics_server(
    metrics: SharedMetrics,
    port: u16,
) -> Result<(), Box<dyn std::error::Error>> {
    let app = create_metrics_router(metrics);

    let listener = tokio::net::TcpListener::bind(format!("0.0.0.0:{}", port)).await?;

    println!("Metrics server starting on http://0.0.0.0:{}", port);
    println!("Metrics available at: http://localhost:{}/metrics", port);
    println!(
        "Health check available at: http://localhost:{}/health",
        port
    );

    axum::serve(listener, app).await?;

    Ok(())
}
