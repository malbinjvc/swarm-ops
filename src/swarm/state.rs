use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

use crate::models::{AgentConfig, ConsensusResult, Finding, SwarmTask};

/// Thread-safe shared state for the entire swarm.
#[derive(Clone)]
pub struct SwarmState {
    pub tasks: Arc<RwLock<HashMap<String, SwarmTask>>>,
    pub agents: Arc<RwLock<HashMap<String, AgentConfig>>>,
    pub findings: Arc<RwLock<HashMap<String, Finding>>>,
    pub consensus_results: Arc<RwLock<HashMap<String, ConsensusResult>>>,
}

impl SwarmState {
    pub fn new() -> Self {
        Self {
            tasks: Arc::new(RwLock::new(HashMap::new())),
            agents: Arc::new(RwLock::new(HashMap::new())),
            findings: Arc::new(RwLock::new(HashMap::new())),
            consensus_results: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Insert a task into shared state.
    pub async fn insert_task(&self, task: SwarmTask) {
        self.tasks.write().await.insert(task.id.clone(), task);
    }

    /// Get a task by ID.
    pub async fn get_task(&self, id: &str) -> Option<SwarmTask> {
        self.tasks.read().await.get(id).cloned()
    }

    /// List all tasks.
    pub async fn list_tasks(&self) -> Vec<SwarmTask> {
        self.tasks.read().await.values().cloned().collect()
    }

    /// Update a task in-place.
    pub async fn update_task(&self, task: SwarmTask) {
        self.tasks.write().await.insert(task.id.clone(), task);
    }

    /// Register an agent configuration.
    pub async fn register_agent(&self, config: AgentConfig) {
        self.agents.write().await.insert(config.id.clone(), config);
    }

    /// List all agents.
    pub async fn list_agents(&self) -> Vec<AgentConfig> {
        self.agents.read().await.values().cloned().collect()
    }

    /// Get agent by ID.
    pub async fn get_agent(&self, id: &str) -> Option<AgentConfig> {
        self.agents.read().await.get(id).cloned()
    }

    /// Store a finding.
    pub async fn insert_finding(&self, finding: Finding) {
        self.findings
            .write()
            .await
            .insert(finding.id.clone(), finding);
    }

    /// Get findings for a specific agent.
    pub async fn get_findings_by_agent(&self, agent_id: &str) -> Vec<Finding> {
        self.findings
            .read()
            .await
            .values()
            .filter(|f| f.agent_id == agent_id)
            .cloned()
            .collect()
    }

    /// Get findings by IDs.
    pub async fn get_findings_by_ids(&self, ids: &[String]) -> Vec<Finding> {
        let store = self.findings.read().await;
        ids.iter().filter_map(|id| store.get(id).cloned()).collect()
    }

    /// Get all findings.
    pub async fn list_findings(&self) -> Vec<Finding> {
        self.findings.read().await.values().cloned().collect()
    }

    /// Store a consensus result.
    pub async fn insert_consensus(&self, result: ConsensusResult) {
        self.consensus_results
            .write()
            .await
            .insert(result.id.clone(), result);
    }

    /// List all consensus results.
    pub async fn list_consensus(&self) -> Vec<ConsensusResult> {
        self.consensus_results
            .read()
            .await
            .values()
            .cloned()
            .collect()
    }
}

impl Default for SwarmState {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::*;

    #[tokio::test]
    async fn test_insert_and_get_task() {
        let state = SwarmState::new();
        let task = SwarmTask::new(TaskType::LogAnalysis, "data".into(), vec![]);
        let id = task.id.clone();
        state.insert_task(task).await;
        let got = state.get_task(&id).await.unwrap();
        assert_eq!(got.data, "data");
    }

    #[tokio::test]
    async fn test_list_tasks() {
        let state = SwarmState::new();
        state
            .insert_task(SwarmTask::new(TaskType::LogAnalysis, "a".into(), vec![]))
            .await;
        state
            .insert_task(SwarmTask::new(TaskType::HealthCheck, "b".into(), vec![]))
            .await;
        assert_eq!(state.list_tasks().await.len(), 2);
    }

    #[tokio::test]
    async fn test_register_and_list_agents() {
        let state = SwarmState::new();
        let agent = AgentConfig::new("Test".into(), "log_analyzer".into(), vec!["logs".into()]);
        state.register_agent(agent).await;
        assert_eq!(state.list_agents().await.len(), 1);
    }

    #[tokio::test]
    async fn test_insert_and_get_findings_by_agent() {
        let state = SwarmState::new();
        let f = Finding::new(
            Severity::High,
            FindingCategory::ErrorPattern,
            "test".into(),
            "evidence".into(),
            0.9,
            "agent-1".into(),
        );
        state.insert_finding(f).await;
        let findings = state.get_findings_by_agent("agent-1").await;
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].agent_id, "agent-1");
    }

    #[tokio::test]
    async fn test_concurrent_state_access() {
        let state = SwarmState::new();
        let s1 = state.clone();
        let s2 = state.clone();

        let h1 = tokio::spawn(async move {
            for i in 0..50 {
                let task = SwarmTask::new(
                    TaskType::LogAnalysis,
                    format!("task-a-{}", i),
                    vec![],
                );
                s1.insert_task(task).await;
            }
        });

        let h2 = tokio::spawn(async move {
            for i in 0..50 {
                let task = SwarmTask::new(
                    TaskType::HealthCheck,
                    format!("task-b-{}", i),
                    vec![],
                );
                s2.insert_task(task).await;
            }
        });

        h1.await.unwrap();
        h2.await.unwrap();
        assert_eq!(state.list_tasks().await.len(), 100);
    }
}
