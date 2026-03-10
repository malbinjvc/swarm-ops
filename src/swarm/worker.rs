use async_trait::async_trait;
use crate::models::{AgentError, Finding};
use crate::swarm::state::SwarmState;

/// Core trait implemented by every worker agent in the swarm.
#[async_trait]
pub trait WorkerAgent: Send + Sync {
    /// Human-readable name.
    fn name(&self) -> &str;

    /// List of capability strings this agent offers.
    fn capabilities(&self) -> Vec<String>;

    /// Run analysis on the input data, producing zero or more findings.
    async fn analyze(
        &self,
        input: &str,
        context: &SwarmState,
    ) -> Result<Vec<Finding>, AgentError>;
}
