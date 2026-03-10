use std::sync::Arc;

use async_trait::async_trait;
use axum::body::Body;
use axum::http::{Request, StatusCode};
use http_body_util::BodyExt;
use tower::ServiceExt;

use swarm_ops::models::*;
use swarm_ops::routes::AppState;
use swarm_ops::services::claude_client::ClaudeClient;
use swarm_ops::swarm::state::SwarmState;

struct MockClaude;

#[async_trait]
impl ClaudeClient for MockClaude {
    async fn send_message(
        &self,
        _system_prompt: &str,
        _user_message: &str,
    ) -> Result<String, AgentError> {
        Ok("mock analysis".to_string())
    }
}

fn app() -> (axum::Router, SwarmState) {
    let state = SwarmState::new();
    let app_state = AppState {
        swarm_state: state.clone(),
        claude: Arc::new(MockClaude),
    };
    (swarm_ops::build_router(app_state), state)
}

#[tokio::test]
async fn test_health_endpoint() {
    let (app, _) = app();
    let resp = app
        .oneshot(
            Request::builder()
                .uri("/health")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let body = resp.into_body().collect().await.unwrap().to_bytes();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(json["status"], "ok");
}

#[tokio::test]
async fn test_health_has_security_headers() {
    let (app, _) = app();
    let resp = app
        .oneshot(
            Request::builder()
                .uri("/health")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(
        resp.headers().get("x-content-type-options").unwrap(),
        "nosniff"
    );
    assert_eq!(resp.headers().get("x-frame-options").unwrap(), "DENY");
}

#[tokio::test]
async fn test_stats_endpoint() {
    let (app, _) = app();
    let resp = app
        .oneshot(
            Request::builder()
                .uri("/stats")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let body = resp.into_body().collect().await.unwrap().to_bytes();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(json["data"]["total_tasks"], 0);
}

#[tokio::test]
async fn test_list_tasks_empty() {
    let (app, _) = app();
    let resp = app
        .oneshot(
            Request::builder()
                .uri("/swarm/tasks")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
}

#[tokio::test]
async fn test_get_task_not_found() {
    let (app, _) = app();
    let resp = app
        .oneshot(
            Request::builder()
                .uri("/swarm/tasks/nonexistent")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn test_register_agent() {
    let (app, _) = app();
    let body = serde_json::json!({
        "name": "TestAgent",
        "agent_type": "log_analyzer",
        "capabilities": ["log_analysis"]
    });
    let resp = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/swarm/agents")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&body).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::CREATED);
}

#[tokio::test]
async fn test_register_agent_empty_name() {
    let (app, _) = app();
    let body = serde_json::json!({
        "name": "",
        "agent_type": "log_analyzer",
        "capabilities": []
    });
    let resp = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/swarm/agents")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&body).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn test_swarm_analyze_empty_data() {
    let (app, _) = app();
    let body = serde_json::json!({
        "data": "",
        "task_type": "log_analysis",
        "agents": ["log_analyzer"]
    });
    let resp = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/swarm/analyze")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&body).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn test_swarm_analyze_success() {
    let (app, _) = app();
    let body = serde_json::json!({
        "data": "ERROR NullPointer at line 42\nERROR NullPointer at line 43\nFATAL OOM",
        "task_type": "log_analysis",
        "agents": ["log_analyzer"]
    });
    let resp = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/swarm/analyze")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&body).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let bytes = resp.into_body().collect().await.unwrap().to_bytes();
    let json: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(json["success"], true);
    assert!(json["data"]["findings"].is_array());
}

#[tokio::test]
async fn test_consensus_empty_ids() {
    let (app, _) = app();
    let body = serde_json::json!({ "finding_ids": [] });
    let resp = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/swarm/consensus")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&body).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn test_consensus_nonexistent_ids() {
    let (app, _) = app();
    let body = serde_json::json!({ "finding_ids": ["no-such-id"] });
    let resp = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/swarm/consensus")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&body).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn test_agent_findings_empty() {
    let (app, _) = app();
    let resp = app
        .oneshot(
            Request::builder()
                .uri("/swarm/agents/some-agent/findings")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
}

#[tokio::test]
async fn test_list_agents_empty() {
    let (app, _) = app();
    let resp = app
        .oneshot(
            Request::builder()
                .uri("/swarm/agents")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let bytes = resp.into_body().collect().await.unwrap().to_bytes();
    let json: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert!(json["data"].is_array());
}
