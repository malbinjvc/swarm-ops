use std::sync::Arc;

use tokio::net::TcpListener;

use swarm_ops::routes::AppState;
use swarm_ops::services::claude_client::HttpClaudeClient;
use swarm_ops::swarm::state::SwarmState;

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();

    let api_key = std::env::var("ANTHROPIC_API_KEY").unwrap_or_default();
    let claude: Arc<dyn swarm_ops::services::claude_client::ClaudeClient> =
        Arc::new(HttpClaudeClient::new(api_key));

    let swarm_state = SwarmState::new();
    let app_state = AppState {
        swarm_state,
        claude,
    };

    let app = swarm_ops::build_router(app_state);

    let addr = "0.0.0.0:3000";
    tracing::info!("SwarmOps listening on {}", addr);
    let listener = TcpListener::bind(addr).await.unwrap();
    axum::serve(listener, app).await.unwrap();
}
