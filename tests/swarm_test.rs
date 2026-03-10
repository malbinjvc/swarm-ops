use std::sync::Arc;

use async_trait::async_trait;
use swarm_ops::models::*;
use swarm_ops::swarm::manager::SwarmManager;
use swarm_ops::swarm::state::SwarmState;
use swarm_ops::swarm::worker::WorkerAgent;

struct StubWorker {
    agent_name: String,
    caps: Vec<String>,
    result_findings: Vec<Finding>,
}

impl StubWorker {
    fn new(name: &str, caps: Vec<&str>, findings: Vec<Finding>) -> Self {
        Self {
            agent_name: name.to_string(),
            caps: caps.into_iter().map(|s| s.to_string()).collect(),
            result_findings: findings,
        }
    }
}

#[async_trait]
impl WorkerAgent for StubWorker {
    fn name(&self) -> &str {
        &self.agent_name
    }
    fn capabilities(&self) -> Vec<String> {
        self.caps.clone()
    }
    async fn analyze(
        &self,
        _input: &str,
        _context: &SwarmState,
    ) -> Result<Vec<Finding>, AgentError> {
        Ok(self.result_findings.clone())
    }
}

#[tokio::test]
async fn test_swarm_manager_empty_workers() {
    let state = SwarmState::new();
    let manager = SwarmManager::new(state);
    assert!(manager.worker_names().is_empty());
}

#[tokio::test]
async fn test_swarm_manager_register_multiple() {
    let state = SwarmState::new();
    let mut mgr = SwarmManager::new(state);
    mgr.register_worker(Arc::new(StubWorker::new("w1", vec!["log"], vec![])));
    mgr.register_worker(Arc::new(StubWorker::new("w2", vec!["health"], vec![])));
    mgr.register_worker(Arc::new(StubWorker::new("w3", vec!["incident"], vec![])));
    assert_eq!(mgr.worker_names().len(), 3);
}

#[tokio::test]
async fn test_swarm_task_lifecycle() {
    let state = SwarmState::new();
    let mut mgr = SwarmManager::new(state.clone());

    let finding = Finding::new(
        Severity::Medium,
        FindingCategory::ErrorPattern,
        "test finding".into(),
        "ev".into(),
        0.7,
        "w1".into(),
    );
    mgr.register_worker(Arc::new(StubWorker::new("w1", vec!["log"], vec![finding])));

    let mut task = SwarmTask::new(TaskType::LogAnalysis, "logs".into(), vec!["w1".into()]);
    let task_id = task.id.clone();

    assert_eq!(task.status, TaskStatus::Pending);
    let results = mgr.execute_task(&mut task).await;
    assert_eq!(task.status, TaskStatus::Completed);
    assert!(!results.is_empty());

    // Task should be in shared state
    let stored = state.get_task(&task_id).await.unwrap();
    assert_eq!(stored.status, TaskStatus::Completed);
}

#[tokio::test]
async fn test_swarm_findings_stored_in_state() {
    let state = SwarmState::new();
    let mut mgr = SwarmManager::new(state.clone());

    let finding = Finding::new(
        Severity::High,
        FindingCategory::ServiceOutage,
        "outage".into(),
        "ev".into(),
        0.9,
        "w1".into(),
    );
    mgr.register_worker(Arc::new(StubWorker::new("w1", vec!["health"], vec![finding])));

    let mut task = SwarmTask::new(TaskType::HealthCheck, "data".into(), vec!["w1".into()]);
    mgr.execute_task(&mut task).await;

    let all_findings = state.list_findings().await;
    assert!(!all_findings.is_empty());
}
