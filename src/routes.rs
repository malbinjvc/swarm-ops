use std::sync::Arc;

use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::Json;

use crate::agents::health_checker::HealthChecker;
use crate::agents::incident_responder::IncidentResponder;
use crate::agents::log_analyzer::LogAnalyzer;
use crate::agents::metrics_agent::MetricsAgent;
use crate::models::*;
use crate::services::claude_client::ClaudeClient;
use crate::swarm::consensus::ConsensusEngine;
use crate::swarm::manager::SwarmManager;
use crate::swarm::state::SwarmState;
use crate::swarm::worker::WorkerAgent;

/// Shared application state passed to route handlers.
#[derive(Clone)]
pub struct AppState {
    pub swarm_state: SwarmState,
    pub claude: Arc<dyn ClaudeClient>,
}

// ── GET /health ─────────────────────────────────────────────────────

pub async fn health() -> impl IntoResponse {
    Json(serde_json::json!({ "status": "ok" }))
}

// ── POST /swarm/analyze ─────────────────────────────────────────────

pub async fn swarm_analyze(
    State(app): State<AppState>,
    Json(body): Json<AnalyzeRequest>,
) -> impl IntoResponse {
    if body.data.is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::to_value(ErrorResponse::new("data must not be empty")).unwrap()),
        );
    }

    let mut task = SwarmTask::new(body.task_type.clone(), body.data.clone(), body.agents.clone());

    // Build a fresh manager with all known agent types
    let mut manager = SwarmManager::new(app.swarm_state.clone());
    register_default_workers(&mut manager, app.claude.clone());

    let results = manager.execute_task(&mut task).await;

    // Store the completed task
    app.swarm_state.update_task(task.clone()).await;

    (
        StatusCode::OK,
        Json(
            serde_json::to_value(ApiResponse::ok(serde_json::json!({
                "task_id": task.id,
                "status": "completed",
                "findings": results,
            })))
            .unwrap(),
        ),
    )
}

// ── GET /swarm/tasks ────────────────────────────────────────────────

pub async fn list_tasks(State(app): State<AppState>) -> impl IntoResponse {
    let tasks = app.swarm_state.list_tasks().await;
    Json(serde_json::to_value(ApiResponse::ok(tasks)).unwrap())
}

// ── GET /swarm/tasks/:id ────────────────────────────────────────────

pub async fn get_task(
    State(app): State<AppState>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    match app.swarm_state.get_task(&id).await {
        Some(task) => (
            StatusCode::OK,
            Json(serde_json::to_value(ApiResponse::ok(task)).unwrap()),
        ),
        None => (
            StatusCode::NOT_FOUND,
            Json(serde_json::to_value(ErrorResponse::new("task not found")).unwrap()),
        ),
    }
}

// ── POST /swarm/agents ──────────────────────────────────────────────

pub async fn register_agent(
    State(app): State<AppState>,
    Json(body): Json<RegisterAgentRequest>,
) -> impl IntoResponse {
    if body.name.is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::to_value(ErrorResponse::new("name must not be empty")).unwrap()),
        );
    }

    let config = AgentConfig::new(body.name, body.agent_type, body.capabilities);
    let id = config.id.clone();
    app.swarm_state.register_agent(config).await;

    (
        StatusCode::CREATED,
        Json(
            serde_json::to_value(ApiResponse::ok(serde_json::json!({
                "agent_id": id,
            })))
            .unwrap(),
        ),
    )
}

// ── GET /swarm/agents ───────────────────────────────────────────────

pub async fn list_agents(State(app): State<AppState>) -> impl IntoResponse {
    let agents = app.swarm_state.list_agents().await;
    Json(serde_json::to_value(ApiResponse::ok(agents)).unwrap())
}

// ── GET /swarm/agents/:id/findings ──────────────────────────────────

pub async fn agent_findings(
    State(app): State<AppState>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    let findings = app.swarm_state.get_findings_by_agent(&id).await;
    Json(serde_json::to_value(ApiResponse::ok(findings)).unwrap())
}

// ── POST /swarm/consensus ───────────────────────────────────────────

pub async fn trigger_consensus(
    State(app): State<AppState>,
    Json(body): Json<ConsensusRequest>,
) -> impl IntoResponse {
    if body.finding_ids.is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(
                serde_json::to_value(ErrorResponse::new("finding_ids must not be empty")).unwrap(),
            ),
        );
    }

    let findings = app.swarm_state.get_findings_by_ids(&body.finding_ids).await;

    if findings.is_empty() {
        return (
            StatusCode::NOT_FOUND,
            Json(serde_json::to_value(ErrorResponse::new("no findings found for given IDs")).unwrap()),
        );
    }

    let result = ConsensusEngine::run(&findings);
    app.swarm_state.insert_consensus(result.clone()).await;

    (
        StatusCode::OK,
        Json(serde_json::to_value(ApiResponse::ok(result)).unwrap()),
    )
}

// ── GET /stats ──────────────────────────────────────────────────────

pub async fn stats(State(app): State<AppState>) -> impl IntoResponse {
    let tasks = app.swarm_state.list_tasks().await;
    let agents = app.swarm_state.list_agents().await;
    let findings = app.swarm_state.list_findings().await;
    let consensus_results = app.swarm_state.list_consensus().await;

    let mut tasks_by_status = std::collections::HashMap::new();
    for t in &tasks {
        *tasks_by_status
            .entry(format!("{:?}", t.status).to_lowercase())
            .or_insert(0usize) += 1;
    }

    let consensus_rate = if !consensus_results.is_empty() {
        consensus_results
            .iter()
            .map(|c| c.agreement_rate)
            .sum::<f64>()
            / consensus_results.len() as f64
    } else {
        0.0
    };

    let stats = SwarmStats {
        total_tasks: tasks.len(),
        total_agents: agents.len(),
        total_findings: findings.len(),
        consensus_rate,
        tasks_by_status,
    };

    Json(serde_json::to_value(ApiResponse::ok(stats)).unwrap())
}

// ── Helpers ─────────────────────────────────────────────────────────

fn register_default_workers(manager: &mut SwarmManager, claude: Arc<dyn ClaudeClient>) {
    let agents: Vec<Arc<dyn WorkerAgent>> = vec![
        Arc::new(LogAnalyzer::new(claude.clone())),
        Arc::new(HealthChecker::new(claude.clone())),
        Arc::new(IncidentResponder::new(claude.clone())),
        Arc::new(MetricsAgent::new(claude)),
    ];
    for agent in agents {
        manager.register_worker(agent);
    }
}
