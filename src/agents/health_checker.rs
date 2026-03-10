use std::sync::Arc;

use async_trait::async_trait;

use crate::models::{AgentError, Finding, FindingCategory, Severity};
use crate::services::claude_client::ClaudeClient;
use crate::swarm::state::SwarmState;
use crate::swarm::worker::WorkerAgent;

/// Checks service status patterns, identifies degradation.
pub struct HealthChecker {
    claude: Arc<dyn ClaudeClient>,
}

impl HealthChecker {
    pub fn new(claude: Arc<dyn ClaudeClient>) -> Self {
        Self { claude }
    }

    fn heuristic_analyze(&self, input: &str) -> Vec<Finding> {
        let mut findings = Vec::new();
        let lower = input.to_lowercase();

        // Service status keywords
        let down_count = lower.matches("down").count()
            + lower.matches("unreachable").count()
            + lower.matches("unavailable").count();
        let timeout_count = lower.matches("timeout").count()
            + lower.matches("timed out").count();
        let degraded_count = lower.matches("degraded").count()
            + lower.matches("slow").count()
            + lower.matches("high latency").count();

        if down_count > 0 {
            findings.push(Finding::new(
                Severity::Critical,
                FindingCategory::ServiceOutage,
                format!("Service outage detected: {} down indicators", down_count),
                format!("Keywords: down/unreachable/unavailable (count: {})", down_count),
                0.90,
                "health_checker".to_string(),
            ));
        }

        if timeout_count > 3 {
            findings.push(Finding::new(
                Severity::High,
                FindingCategory::PerformanceDegradation,
                format!("Excessive timeouts: {} timeout events", timeout_count),
                "Repeated timeouts indicate connectivity or performance issues".to_string(),
                0.85,
                "health_checker".to_string(),
            ));
        } else if timeout_count > 0 {
            findings.push(Finding::new(
                Severity::Medium,
                FindingCategory::PerformanceDegradation,
                format!("Timeout events detected: {}", timeout_count),
                "Occasional timeouts observed".to_string(),
                0.65,
                "health_checker".to_string(),
            ));
        }

        if degraded_count > 2 {
            findings.push(Finding::new(
                Severity::High,
                FindingCategory::PerformanceDegradation,
                format!(
                    "Service degradation pattern: {} degradation indicators",
                    degraded_count
                ),
                "Multiple slow/degraded/high-latency references".to_string(),
                0.80,
                "health_checker".to_string(),
            ));
        }

        // Memory / CPU / disk
        if lower.contains("out of memory") || lower.contains("oom") {
            findings.push(Finding::new(
                Severity::Critical,
                FindingCategory::ResourceExhaustion,
                "Out of memory condition detected".to_string(),
                "OOM or out-of-memory keyword found".to_string(),
                0.92,
                "health_checker".to_string(),
            ));
        }

        if lower.contains("disk full") || lower.contains("no space left") {
            findings.push(Finding::new(
                Severity::Critical,
                FindingCategory::ResourceExhaustion,
                "Disk space exhaustion detected".to_string(),
                "Disk full / no space left keyword found".to_string(),
                0.93,
                "health_checker".to_string(),
            ));
        }

        if lower.contains("cpu") && (lower.contains("100%") || lower.contains("high cpu")) {
            findings.push(Finding::new(
                Severity::High,
                FindingCategory::ResourceExhaustion,
                "High CPU utilization detected".to_string(),
                "CPU 100% or high CPU keyword".to_string(),
                0.80,
                "health_checker".to_string(),
            ));
        }

        findings
    }
}

#[async_trait]
impl WorkerAgent for HealthChecker {
    fn name(&self) -> &str {
        "health_checker"
    }

    fn capabilities(&self) -> Vec<String> {
        vec![
            "health_check".to_string(),
            "service_monitoring".to_string(),
            "resource_monitoring".to_string(),
        ]
    }

    async fn analyze(
        &self,
        input: &str,
        _context: &SwarmState,
    ) -> Result<Vec<Finding>, AgentError> {
        let mut findings = self.heuristic_analyze(input);

        let prompt = "You are a service health expert. Analyze the following health data and \
                       identify any degradation, outages, or resource issues. Respond briefly.";
        match self.claude.send_message(prompt, input).await {
            Ok(response) if !response.is_empty() => {
                findings.push(Finding::new(
                    Severity::Info,
                    FindingCategory::PerformanceDegradation,
                    "AI-assisted health analysis summary".to_string(),
                    response,
                    0.70,
                    "health_checker".to_string(),
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

    fn make_checker(response: &str) -> HealthChecker {
        HealthChecker::new(Arc::new(MockClaudeClient::new(response)))
    }

    #[tokio::test]
    async fn test_detects_outage() {
        let agent = make_checker("ok");
        let state = SwarmState::new();
        let input = "Service payment-api is DOWN. Host unreachable.";
        let findings = agent.analyze(input, &state).await.unwrap();
        assert!(findings
            .iter()
            .any(|f| f.category == FindingCategory::ServiceOutage));
    }

    #[tokio::test]
    async fn test_detects_oom() {
        let agent = make_checker("ok");
        let state = SwarmState::new();
        let input = "Container killed: Out of memory (OOM)";
        let findings = agent.analyze(input, &state).await.unwrap();
        assert!(findings
            .iter()
            .any(|f| f.category == FindingCategory::ResourceExhaustion));
    }

    #[tokio::test]
    async fn test_healthy_input() {
        let agent = make_checker("all clear");
        let state = SwarmState::new();
        let input = "All services healthy. Response time 45ms.";
        let findings = agent.analyze(input, &state).await.unwrap();
        // Only the AI summary finding
        assert!(findings
            .iter()
            .all(|f| f.severity != Severity::Critical));
    }
}
