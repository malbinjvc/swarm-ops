pub mod agents;
pub mod middleware;
pub mod models;
pub mod routes;
pub mod services;
pub mod swarm;

// Re-export the router builder for integration tests.
pub use crate::routes::AppState;

use axum::routing::{get, post};
use axum::Router;

/// Build the Axum router (used by main and integration tests).
pub fn build_router(app_state: AppState) -> Router {
    Router::new()
        .route("/health", get(routes::health))
        .route("/swarm/analyze", post(routes::swarm_analyze))
        .route("/swarm/tasks", get(routes::list_tasks))
        .route("/swarm/tasks/{id}", get(routes::get_task))
        .route("/swarm/agents", post(routes::register_agent).get(routes::list_agents))
        .route("/swarm/agents/{id}/findings", get(routes::agent_findings))
        .route("/swarm/consensus", post(routes::trigger_consensus))
        .route("/stats", get(routes::stats))
        .layer(middleware::content_security_policy())
        .layer(middleware::strict_transport_security())
        .layer(middleware::x_xss_protection())
        .layer(middleware::x_frame_options())
        .layer(middleware::x_content_type_options())
        .with_state(app_state)
}
