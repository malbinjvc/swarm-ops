use std::sync::Arc;

use async_trait::async_trait;

use crate::models::{AgentError, Finding, FindingCategory, Severity};
use crate::services::claude_client::ClaudeClient;
use crate::swarm::state::SwarmState;
use crate::swarm::worker::WorkerAgent;

/// Parses log patterns, detects anomalies, identifies error clusters.
pub struct LogAnalyzer {
    claude: Arc<dyn ClaudeClient>,
}

impl LogAnalyzer {
    pub fn new(claude: Arc<dyn ClaudeClient>) -> Self {
        Self { claude }
    }

    /// Local heuristic analysis — no API call required.
    fn heuristic_analyze(&self, input: &str) -> Vec<Finding> {
        let mut findings = Vec::new();
        let lines: Vec<&str> = input.lines().collect();

        // Count error/warn occurrences
        let error_count = lines
            .iter()
            .filter(|l| l.to_lowercase().contains("error"))
            .count();
        let warn_count = lines
            .iter()
            .filter(|l| l.to_lowercase().contains("warn"))
            .count();
        let fatal_count = lines
            .iter()
            .filter(|l| l.to_lowercase().contains("fatal"))
            .count();

        if fatal_count > 0 {
            findings.push(Finding::new(
                Severity::Critical,
                FindingCategory::ErrorPattern,
                format!("Detected {} fatal log entries", fatal_count),
                format!(
                    "Lines containing FATAL: {}",
                    lines
                        .iter()
                        .filter(|l| l.to_lowercase().contains("fatal"))
                        .take(3)
                        .copied()
                        .collect::<Vec<_>>()
                        .join(" | ")
                ),
                0.95,
                "log_analyzer".to_string(),
            ));
        }

        if error_count > 5 {
            findings.push(Finding::new(
                Severity::High,
                FindingCategory::ErrorPattern,
                format!(
                    "High error rate detected: {} errors in {} lines",
                    error_count,
                    lines.len()
                ),
                format!("Error density: {:.1}%", error_count as f64 / lines.len().max(1) as f64 * 100.0),
                0.85,
                "log_analyzer".to_string(),
            ));
        } else if error_count > 0 {
            findings.push(Finding::new(
                Severity::Medium,
                FindingCategory::ErrorPattern,
                format!("Found {} error entries in logs", error_count),
                "Error entries present".to_string(),
                0.75,
                "log_analyzer".to_string(),
            ));
        }

        if warn_count > 10 {
            findings.push(Finding::new(
                Severity::Medium,
                FindingCategory::AnomalousBehavior,
                format!("Elevated warning count: {} warnings", warn_count),
                "High warning frequency may indicate degrading service".to_string(),
                0.70,
                "log_analyzer".to_string(),
            ));
        }

        // Look for repeated patterns (error clusters)
        let mut pattern_counts: std::collections::HashMap<String, usize> =
            std::collections::HashMap::new();
        for line in &lines {
            let lower = line.to_lowercase();
            if lower.contains("error") || lower.contains("exception") {
                // Normalise by taking first 50 chars as pattern key
                let key: String = lower.chars().take(50).collect();
                *pattern_counts.entry(key).or_insert(0) += 1;
            }
        }
        for (pattern, count) in &pattern_counts {
            if *count >= 3 {
                findings.push(Finding::new(
                    Severity::High,
                    FindingCategory::ErrorPattern,
                    format!("Repeated error cluster ({} occurrences)", count),
                    format!("Pattern prefix: {}", pattern),
                    0.80,
                    "log_analyzer".to_string(),
                ));
            }
        }

        findings
    }
}

#[async_trait]
impl WorkerAgent for LogAnalyzer {
    fn name(&self) -> &str {
        "log_analyzer"
    }

    fn capabilities(&self) -> Vec<String> {
        vec![
            "log_analysis".to_string(),
            "error_detection".to_string(),
            "anomaly_detection".to_string(),
        ]
    }

    async fn analyze(
        &self,
        input: &str,
        _context: &SwarmState,
    ) -> Result<Vec<Finding>, AgentError> {
        // Start with heuristic findings
        let mut findings = self.heuristic_analyze(input);

        // Optionally enrich with Claude (best-effort)
        let prompt = "You are a log analysis expert. Analyze the following logs and identify \
                       any error patterns, anomalies, or clusters. Respond with a brief summary.";
        match self.claude.send_message(prompt, input).await {
            Ok(response) => {
                if !response.is_empty() {
                    findings.push(Finding::new(
                        Severity::Info,
                        FindingCategory::AnomalousBehavior,
                        "AI-assisted log analysis summary".to_string(),
                        response,
                        0.70,
                        "log_analyzer".to_string(),
                    ));
                }
            }
            Err(_) => {
                // Claude enrichment is optional; heuristic findings are enough
            }
        }

        Ok(findings)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::services::claude_client::tests::MockClaudeClient;

    fn make_analyzer(response: &str) -> LogAnalyzer {
        LogAnalyzer::new(Arc::new(MockClaudeClient::new(response)))
    }

    #[tokio::test]
    async fn test_detects_fatal() {
        let agent = make_analyzer("ok");
        let state = SwarmState::new();
        let input = "2024-01-01 FATAL OutOfMemory\n2024-01-01 INFO started";
        let findings = agent.analyze(input, &state).await.unwrap();
        assert!(findings.iter().any(|f| f.severity == Severity::Critical));
    }

    #[tokio::test]
    async fn test_detects_high_error_rate() {
        let agent = make_analyzer("analysis");
        let state = SwarmState::new();
        let input = (0..10)
            .map(|i| format!("ERROR service failed attempt {}", i))
            .collect::<Vec<_>>()
            .join("\n");
        let findings = agent.analyze(&input, &state).await.unwrap();
        assert!(findings.iter().any(|f| f.severity == Severity::High));
    }

    #[tokio::test]
    async fn test_clean_logs_no_critical() {
        let agent = make_analyzer("all good");
        let state = SwarmState::new();
        let input = "INFO started\nINFO healthy\nINFO ok";
        let findings = agent.analyze(input, &state).await.unwrap();
        assert!(findings
            .iter()
            .all(|f| f.severity != Severity::Critical));
    }

    #[tokio::test]
    async fn test_capabilities() {
        let agent = make_analyzer("");
        assert!(agent.capabilities().contains(&"log_analysis".to_string()));
    }
}
