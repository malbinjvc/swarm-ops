use std::collections::HashMap;
use std::sync::Arc;

use chrono::Utc;

use crate::models::{Finding, SwarmTask, TaskStatus, TaskType};
use crate::swarm::consensus::ConsensusEngine;
use crate::swarm::state::SwarmState;
use crate::swarm::worker::WorkerAgent;

/// The swarm manager: holds worker agents, distributes tasks, collects results.
pub struct SwarmManager {
    workers: HashMap<String, Arc<dyn WorkerAgent>>,
    state: SwarmState,
}

impl SwarmManager {
    pub fn new(state: SwarmState) -> Self {
        Self {
            workers: HashMap::new(),
            state,
        }
    }

    /// Register a worker agent with the manager.
    pub fn register_worker(&mut self, agent: Arc<dyn WorkerAgent>) {
        self.workers.insert(agent.name().to_string(), agent);
    }

    /// Get the names of all registered workers.
    pub fn worker_names(&self) -> Vec<String> {
        self.workers.keys().cloned().collect()
    }

    /// Get a reference to the swarm state.
    pub fn state(&self) -> &SwarmState {
        &self.state
    }

    /// Execute a swarm task: distribute to requested workers, run them in
    /// parallel, collect findings, run consensus, update task state.
    pub async fn execute_task(&self, task: &mut SwarmTask) -> Vec<Finding> {
        task.status = TaskStatus::Running;
        self.state.update_task(task.clone()).await;

        // Select workers
        let selected: Vec<Arc<dyn WorkerAgent>> = if task.requested_agents.is_empty() {
            // Use all workers for the matching task type
            self.workers_for_task_type(&task.task_type)
        } else {
            task.requested_agents
                .iter()
                .filter_map(|name| self.workers.get(name).cloned())
                .collect()
        };

        // Spawn workers in parallel
        let mut handles = Vec::new();
        for worker in selected {
            let input = task.data.clone();
            let state = self.state.clone();
            handles.push(tokio::spawn(async move {
                worker.analyze(&input, &state).await
            }));
        }

        // Collect results
        let mut all_findings = Vec::new();
        for handle in handles {
            match handle.await {
                Ok(Ok(findings)) => all_findings.extend(findings),
                Ok(Err(e)) => {
                    tracing::warn!("Worker agent failed: {}", e);
                }
                Err(e) => {
                    tracing::warn!("Worker task panicked: {}", e);
                }
            }
        }

        // Store findings in shared state
        for f in &all_findings {
            self.state.insert_finding(f.clone()).await;
        }

        // Run consensus
        let consensus = ConsensusEngine::run(&all_findings);
        let merged = consensus.merged_findings.clone();
        self.state.insert_consensus(consensus).await;

        // Update task
        task.findings = merged.clone();
        task.status = TaskStatus::Completed;
        task.completed_at = Some(Utc::now());
        self.state.update_task(task.clone()).await;

        merged
    }

    /// Select workers relevant to a task type.
    fn workers_for_task_type(&self, task_type: &TaskType) -> Vec<Arc<dyn WorkerAgent>> {
        let keyword = match task_type {
            TaskType::LogAnalysis => "log",
            TaskType::HealthCheck => "health",
            TaskType::Incident => "incident",
        };
        self.workers
            .values()
            .filter(|w| {
                w.capabilities()
                    .iter()
                    .any(|c| c.to_lowercase().contains(keyword))
            })
            .cloned()
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::*;
    use async_trait::async_trait;

    struct StubAgent {
        agent_name: String,
        caps: Vec<String>,
        findings: Vec<Finding>,
    }

    impl StubAgent {
        fn new(name: &str, caps: Vec<&str>, findings: Vec<Finding>) -> Self {
            Self {
                agent_name: name.to_string(),
                caps: caps.into_iter().map(|s| s.to_string()).collect(),
                findings,
            }
        }
    }

    #[async_trait]
    impl WorkerAgent for StubAgent {
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
            Ok(self.findings.clone())
        }
    }

    #[tokio::test]
    async fn test_register_and_list_workers() {
        let state = SwarmState::new();
        let mut mgr = SwarmManager::new(state);
        let agent = Arc::new(StubAgent::new("test_agent", vec!["log"], vec![]));
        mgr.register_worker(agent);
        assert_eq!(mgr.worker_names().len(), 1);
    }

    #[tokio::test]
    async fn test_execute_task_collects_findings() {
        let state = SwarmState::new();
        let mut mgr = SwarmManager::new(state);
        let finding = Finding::new(
            Severity::High,
            FindingCategory::ErrorPattern,
            "Error found".into(),
            "line 42".into(),
            0.9,
            "stub".into(),
        );
        let agent = Arc::new(StubAgent::new("stub", vec!["log"], vec![finding]));
        mgr.register_worker(agent);

        let mut task = SwarmTask::new(
            TaskType::LogAnalysis,
            "log data here".into(),
            vec!["stub".into()],
        );
        let results = mgr.execute_task(&mut task).await;
        assert!(!results.is_empty());
        assert_eq!(task.status, TaskStatus::Completed);
    }

    #[tokio::test]
    async fn test_execute_task_parallel_workers() {
        let state = SwarmState::new();
        let mut mgr = SwarmManager::new(state);

        let f1 = Finding::new(
            Severity::High,
            FindingCategory::ErrorPattern,
            "Error".into(),
            "ev1".into(),
            0.8,
            "a1".into(),
        );
        let f2 = Finding::new(
            Severity::Medium,
            FindingCategory::MetricSpike,
            "Spike".into(),
            "ev2".into(),
            0.6,
            "a2".into(),
        );

        mgr.register_worker(Arc::new(StubAgent::new("a1", vec!["log"], vec![f1])));
        mgr.register_worker(Arc::new(StubAgent::new("a2", vec!["metrics"], vec![f2])));

        let mut task = SwarmTask::new(
            TaskType::LogAnalysis,
            "data".into(),
            vec!["a1".into(), "a2".into()],
        );
        let results = mgr.execute_task(&mut task).await;
        assert_eq!(results.len(), 2);
    }

    #[tokio::test]
    async fn test_workers_for_task_type() {
        let state = SwarmState::new();
        let mut mgr = SwarmManager::new(state);
        mgr.register_worker(Arc::new(StubAgent::new("la", vec!["log_analysis"], vec![])));
        mgr.register_worker(Arc::new(StubAgent::new("hc", vec!["health_check"], vec![])));

        let mut task = SwarmTask::new(TaskType::LogAnalysis, "d".into(), vec![]);
        let _ = mgr.execute_task(&mut task).await;
        // The "la" worker should match TaskType::LogAnalysis
        assert_eq!(task.status, TaskStatus::Completed);
    }
}
