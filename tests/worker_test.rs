use std::sync::Arc;

use async_trait::async_trait;
use swarm_ops::models::*;
use swarm_ops::swarm::state::SwarmState;
use swarm_ops::swarm::worker::WorkerAgent;

struct FailingWorker;

#[async_trait]
impl WorkerAgent for FailingWorker {
    fn name(&self) -> &str {
        "failing"
    }
    fn capabilities(&self) -> Vec<String> {
        vec!["fail".to_string()]
    }
    async fn analyze(
        &self,
        _input: &str,
        _context: &SwarmState,
    ) -> Result<Vec<Finding>, AgentError> {
        Err(AgentError::AnalysisFailed("intentional failure".into()))
    }
}

struct MultiWorker;

#[async_trait]
impl WorkerAgent for MultiWorker {
    fn name(&self) -> &str {
        "multi"
    }
    fn capabilities(&self) -> Vec<String> {
        vec!["log_analysis".to_string(), "health_check".to_string()]
    }
    async fn analyze(
        &self,
        input: &str,
        _context: &SwarmState,
    ) -> Result<Vec<Finding>, AgentError> {
        let mut findings = Vec::new();
        if input.contains("error") {
            findings.push(Finding::new(
                Severity::High,
                FindingCategory::ErrorPattern,
                "found error".into(),
                "multi worker".into(),
                0.8,
                "multi".into(),
            ));
        }
        Ok(findings)
    }
}

#[tokio::test]
async fn test_failing_worker() {
    let worker = FailingWorker;
    let state = SwarmState::new();
    let result = worker.analyze("data", &state).await;
    assert!(result.is_err());
}

#[tokio::test]
async fn test_worker_trait_name_and_capabilities() {
    let worker = MultiWorker;
    assert_eq!(worker.name(), "multi");
    assert_eq!(worker.capabilities().len(), 2);
}

#[tokio::test]
async fn test_worker_produces_findings_conditionally() {
    let worker = MultiWorker;
    let state = SwarmState::new();

    let r1 = worker.analyze("some error here", &state).await.unwrap();
    assert_eq!(r1.len(), 1);

    let r2 = worker.analyze("all good", &state).await.unwrap();
    assert!(r2.is_empty());
}

#[tokio::test]
async fn test_worker_as_trait_object() {
    let worker: Arc<dyn WorkerAgent> = Arc::new(MultiWorker);
    assert_eq!(worker.name(), "multi");
    let state = SwarmState::new();
    let findings = worker.analyze("error", &state).await.unwrap();
    assert!(!findings.is_empty());
}
