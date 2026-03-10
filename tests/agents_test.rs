use std::sync::Arc;

use async_trait::async_trait;
use swarm_ops::models::*;
use swarm_ops::services::claude_client::ClaudeClient;
use swarm_ops::swarm::state::SwarmState;
use swarm_ops::swarm::worker::WorkerAgent;

use swarm_ops::agents::health_checker::HealthChecker;
use swarm_ops::agents::incident_responder::IncidentResponder;
use swarm_ops::agents::log_analyzer::LogAnalyzer;
use swarm_ops::agents::metrics_agent::MetricsAgent;

/// Mock Claude client for integration-level agent tests.
struct MockClaude(String);

#[async_trait]
impl ClaudeClient for MockClaude {
    async fn send_message(
        &self,
        _system_prompt: &str,
        _user_message: &str,
    ) -> Result<String, AgentError> {
        Ok(self.0.clone())
    }
}

fn mock_claude(response: &str) -> Arc<dyn ClaudeClient> {
    Arc::new(MockClaude(response.to_string()))
}

// ── LogAnalyzer ─────────────────────────────────────────────────────

#[tokio::test]
async fn test_log_analyzer_error_cluster() {
    let agent = LogAnalyzer::new(mock_claude("summary"));
    let state = SwarmState::new();
    let input = "ERROR auth failed\nERROR auth failed\nERROR auth failed\nINFO ok";
    let findings = agent.analyze(input, &state).await.unwrap();
    assert!(findings
        .iter()
        .any(|f| f.description.contains("error") || f.description.contains("cluster")));
}

// ── HealthChecker ───────────────────────────────────────────────────

#[tokio::test]
async fn test_health_checker_timeout() {
    let agent = HealthChecker::new(mock_claude("analysis"));
    let state = SwarmState::new();
    let input = "timeout timeout timeout timeout timeout";
    let findings = agent.analyze(input, &state).await.unwrap();
    assert!(findings
        .iter()
        .any(|f| f.category == FindingCategory::PerformanceDegradation));
}

#[tokio::test]
async fn test_health_checker_disk_full() {
    let agent = HealthChecker::new(mock_claude("ok"));
    let state = SwarmState::new();
    let input = "Alert: no space left on device /dev/sda1";
    let findings = agent.analyze(input, &state).await.unwrap();
    assert!(findings
        .iter()
        .any(|f| f.category == FindingCategory::ResourceExhaustion));
}

// ── IncidentResponder ───────────────────────────────────────────────

#[tokio::test]
async fn test_incident_responder_rollback_detection() {
    let agent = IncidentResponder::new(mock_claude("ok"));
    let state = SwarmState::new();
    let input = "Deploy v3.0 pushed. Error spike. Rollback initiated.";
    let findings = agent.analyze(input, &state).await.unwrap();
    assert!(findings
        .iter()
        .any(|f| f.description.contains("Rollback") || f.description.contains("deploy")));
}

// ── MetricsAgent ────────────────────────────────────────────────────

#[tokio::test]
async fn test_metrics_agent_decreasing_trend() {
    let agent = MetricsAgent::new(mock_claude("ok"));
    let state = SwarmState::new();
    let input = "throughput: 100 90 80 70 60 50";
    let findings = agent.analyze(input, &state).await.unwrap();
    assert!(findings
        .iter()
        .any(|f| f.description.contains("decreasing")));
}

#[tokio::test]
async fn test_metrics_agent_drop_keyword() {
    let agent = MetricsAgent::new(mock_claude("ok"));
    let state = SwarmState::new();
    let input = "Revenue drop observed Q3";
    let findings = agent.analyze(input, &state).await.unwrap();
    assert!(findings
        .iter()
        .any(|f| f.category == FindingCategory::MetricDrop));
}
