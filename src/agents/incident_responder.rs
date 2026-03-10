use std::sync::Arc;

use async_trait::async_trait;

use crate::models::{AgentError, Finding, FindingCategory, Severity};
use crate::services::claude_client::ClaudeClient;
use crate::swarm::state::SwarmState;
use crate::swarm::worker::WorkerAgent;

/// Correlates events, suggests root cause, recommends actions.
pub struct IncidentResponder {
    claude: Arc<dyn ClaudeClient>,
}

impl IncidentResponder {
    pub fn new(claude: Arc<dyn ClaudeClient>) -> Self {
        Self { claude }
    }

    fn heuristic_analyze(&self, input: &str) -> Vec<Finding> {
        let mut findings = Vec::new();
        let lower = input.to_lowercase();

        // Incident severity indicators
        let has_p1 = lower.contains("p1") || lower.contains("sev1") || lower.contains("critical incident");
        let has_p2 = lower.contains("p2") || lower.contains("sev2") || lower.contains("major incident");
        let has_rollback = lower.contains("rollback") || lower.contains("revert");
        let has_deploy = lower.contains("deploy") || lower.contains("release") || lower.contains("push");
        let has_config_change = lower.contains("config change") || lower.contains("configuration update");

        if has_p1 {
            findings.push(Finding::new(
                Severity::Critical,
                FindingCategory::ServiceOutage,
                "P1/SEV1 critical incident identified".to_string(),
                "Input contains P1/SEV1/critical incident markers".to_string(),
                0.95,
                "incident_responder".to_string(),
            ));
        }

        if has_p2 {
            findings.push(Finding::new(
                Severity::High,
                FindingCategory::ServiceOutage,
                "P2/SEV2 major incident identified".to_string(),
                "Input contains P2/SEV2/major incident markers".to_string(),
                0.85,
                "incident_responder".to_string(),
            ));
        }

        // Correlation: deploy + errors => possible bad deploy
        if has_deploy
            && (lower.contains("error") || lower.contains("failure") || lower.contains("crash"))
        {
            findings.push(Finding::new(
                Severity::High,
                FindingCategory::ConfigurationDrift,
                "Potential bad deployment: errors correlate with recent deploy/release".to_string(),
                "Deploy/release event followed by errors or failures".to_string(),
                0.80,
                "incident_responder".to_string(),
            ));

            if has_rollback {
                findings.push(Finding::new(
                    Severity::Info,
                    FindingCategory::ConfigurationDrift,
                    "Rollback detected — deployment may have been reverted".to_string(),
                    "Rollback/revert keyword found alongside deploy".to_string(),
                    0.90,
                    "incident_responder".to_string(),
                ));
            }
        }

        if has_config_change
            && (lower.contains("error") || lower.contains("outage") || lower.contains("failure"))
        {
            findings.push(Finding::new(
                Severity::High,
                FindingCategory::ConfigurationDrift,
                "Configuration change correlated with errors/outage".to_string(),
                "Config change event near error/failure events".to_string(),
                0.82,
                "incident_responder".to_string(),
            ));
        }

        // Cascading failure pattern
        let service_names: Vec<&str> = ["api", "db", "cache", "queue", "worker", "gateway"]
            .iter()
            .filter(|s| lower.contains(**s) && (lower.contains("fail") || lower.contains("down")))
            .copied()
            .collect();
        if service_names.len() >= 2 {
            findings.push(Finding::new(
                Severity::Critical,
                FindingCategory::ServiceOutage,
                format!(
                    "Cascading failure pattern: {} services affected",
                    service_names.len()
                ),
                format!("Affected services: {}", service_names.join(", ")),
                0.88,
                "incident_responder".to_string(),
            ));
        }

        findings
    }
}

#[async_trait]
impl WorkerAgent for IncidentResponder {
    fn name(&self) -> &str {
        "incident_responder"
    }

    fn capabilities(&self) -> Vec<String> {
        vec![
            "incident_response".to_string(),
            "root_cause_analysis".to_string(),
            "event_correlation".to_string(),
        ]
    }

    async fn analyze(
        &self,
        input: &str,
        _context: &SwarmState,
    ) -> Result<Vec<Finding>, AgentError> {
        let mut findings = self.heuristic_analyze(input);

        let prompt = "You are an incident response expert. Correlate the following events, \
                       suggest a root cause, and recommend immediate actions. Respond briefly.";
        match self.claude.send_message(prompt, input).await {
            Ok(response) if !response.is_empty() => {
                findings.push(Finding::new(
                    Severity::Info,
                    FindingCategory::AnomalousBehavior,
                    "AI-assisted incident analysis summary".to_string(),
                    response,
                    0.70,
                    "incident_responder".to_string(),
                ));
            }
            _ => {}
        }

        Ok(findings)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::services::claude_client::tests::MockClaudeClient;

    fn make_responder(response: &str) -> IncidentResponder {
        IncidentResponder::new(Arc::new(MockClaudeClient::new(response)))
    }

    #[tokio::test]
    async fn test_detects_p1_incident() {
        let agent = make_responder("ok");
        let state = SwarmState::new();
        let input = "P1 Critical Incident: payment service unresponsive";
        let findings = agent.analyze(input, &state).await.unwrap();
        assert!(findings.iter().any(|f| f.severity == Severity::Critical));
    }

    #[tokio::test]
    async fn test_detects_bad_deploy() {
        let agent = make_responder("ok");
        let state = SwarmState::new();
        let input = "Deploy v2.3.1 completed. Error rate spiked. Multiple failures observed.";
        let findings = agent.analyze(input, &state).await.unwrap();
        assert!(findings
            .iter()
            .any(|f| f.category == FindingCategory::ConfigurationDrift));
    }

    #[tokio::test]
    async fn test_detects_cascading_failure() {
        let agent = make_responder("ok");
        let state = SwarmState::new();
        let input = "API gateway failure. DB connection pool exhausted. Cache down.";
        let findings = agent.analyze(input, &state).await.unwrap();
        assert!(findings
            .iter()
            .any(|f| f.description.contains("Cascading failure")));
    }

    #[tokio::test]
    async fn test_normal_operations() {
        let agent = make_responder("ok");
        let state = SwarmState::new();
        let input = "All systems nominal. No incidents.";
        let findings = agent.analyze(input, &state).await.unwrap();
        assert!(findings
            .iter()
            .all(|f| f.severity != Severity::Critical));
    }
}
