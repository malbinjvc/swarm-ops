use std::sync::Arc;

use async_trait::async_trait;

use crate::models::{AgentError, Finding, FindingCategory, Severity};
use crate::services::claude_client::ClaudeClient;
use crate::swarm::state::SwarmState;
use crate::swarm::worker::WorkerAgent;

/// Analyzes numerical patterns, detects spikes/drops, identifies trends.
pub struct MetricsAgent {
    claude: Arc<dyn ClaudeClient>,
}

impl MetricsAgent {
    pub fn new(claude: Arc<dyn ClaudeClient>) -> Self {
        Self { claude }
    }

    /// Extract numbers from the input text for basic statistical analysis.
    fn extract_numbers(input: &str) -> Vec<f64> {
        input
            .split(|c: char| !c.is_ascii_digit() && c != '.' && c != '-')
            .filter_map(|s| s.parse::<f64>().ok())
            .collect()
    }

    fn heuristic_analyze(&self, input: &str) -> Vec<Finding> {
        let mut findings = Vec::new();
        let lower = input.to_lowercase();

        // Keyword-based detection
        if lower.contains("spike") || lower.contains("surge") {
            findings.push(Finding::new(
                Severity::High,
                FindingCategory::MetricSpike,
                "Metric spike detected in data".to_string(),
                "Spike/surge keyword found".to_string(),
                0.80,
                "metrics_agent".to_string(),
            ));
        }

        if lower.contains("drop") || lower.contains("plummet") || lower.contains("crash") {
            findings.push(Finding::new(
                Severity::High,
                FindingCategory::MetricDrop,
                "Metric drop detected in data".to_string(),
                "Drop/plummet/crash keyword found".to_string(),
                0.80,
                "metrics_agent".to_string(),
            ));
        }

        if lower.contains("trend") || lower.contains("increasing") || lower.contains("decreasing")
        {
            findings.push(Finding::new(
                Severity::Medium,
                FindingCategory::Trend,
                "Trend pattern detected in metrics".to_string(),
                "Trend/increasing/decreasing keyword found".to_string(),
                0.65,
                "metrics_agent".to_string(),
            ));
        }

        // Numerical analysis
        let numbers = Self::extract_numbers(input);
        if numbers.len() >= 3 {
            let mean = numbers.iter().sum::<f64>() / numbers.len() as f64;
            let variance = numbers.iter().map(|x| (x - mean).powi(2)).sum::<f64>()
                / numbers.len() as f64;
            let stddev = variance.sqrt();

            // Check for outliers (> 2 stddev from mean)
            let outliers: Vec<f64> = numbers
                .iter()
                .filter(|&&x| (x - mean).abs() > 2.0 * stddev)
                .copied()
                .collect();

            if !outliers.is_empty() && stddev > 0.0 {
                let high_outliers = outliers.iter().filter(|&&x| x > mean).count();
                let low_outliers = outliers.iter().filter(|&&x| x < mean).count();

                if high_outliers > 0 {
                    findings.push(Finding::new(
                        Severity::High,
                        FindingCategory::MetricSpike,
                        format!(
                            "Statistical spike: {} values exceed 2 standard deviations above mean ({:.1})",
                            high_outliers, mean
                        ),
                        format!(
                            "Mean: {:.2}, StdDev: {:.2}, Outliers: {:?}",
                            mean, stddev, outliers
                        ),
                        0.85,
                        "metrics_agent".to_string(),
                    ));
                }

                if low_outliers > 0 {
                    findings.push(Finding::new(
                        Severity::High,
                        FindingCategory::MetricDrop,
                        format!(
                            "Statistical drop: {} values fall 2 standard deviations below mean ({:.1})",
                            low_outliers, mean
                        ),
                        format!(
                            "Mean: {:.2}, StdDev: {:.2}, Outliers: {:?}",
                            mean, stddev, outliers
                        ),
                        0.85,
                        "metrics_agent".to_string(),
                    ));
                }
            }

            // Simple trend detection: is the series monotonically increasing or decreasing?
            if numbers.len() >= 4 {
                let diffs: Vec<f64> = numbers.windows(2).map(|w| w[1] - w[0]).collect();
                let increasing = diffs.iter().all(|&d| d > 0.0);
                let decreasing = diffs.iter().all(|&d| d < 0.0);

                if increasing {
                    findings.push(Finding::new(
                        Severity::Medium,
                        FindingCategory::Trend,
                        "Monotonically increasing trend detected".to_string(),
                        format!("Values: {:?}", numbers),
                        0.75,
                        "metrics_agent".to_string(),
                    ));
                } else if decreasing {
                    findings.push(Finding::new(
                        Severity::Medium,
                        FindingCategory::Trend,
                        "Monotonically decreasing trend detected".to_string(),
                        format!("Values: {:?}", numbers),
                        0.75,
                        "metrics_agent".to_string(),
                    ));
                }
            }
        }

        findings
    }
}

#[async_trait]
impl WorkerAgent for MetricsAgent {
    fn name(&self) -> &str {
        "metrics_agent"
    }

    fn capabilities(&self) -> Vec<String> {
        vec![
            "metrics_analysis".to_string(),
            "spike_detection".to_string(),
            "trend_analysis".to_string(),
        ]
    }

    async fn analyze(
        &self,
        input: &str,
        _context: &SwarmState,
    ) -> Result<Vec<Finding>, AgentError> {
        let mut findings = self.heuristic_analyze(input);

        let prompt = "You are a metrics analysis expert. Analyze the following metrics data, \
                       detect spikes, drops, or trends. Respond briefly.";
        match self.claude.send_message(prompt, input).await {
            Ok(response) if !response.is_empty() => {
                findings.push(Finding::new(
                    Severity::Info,
                    FindingCategory::Trend,
                    "AI-assisted metrics analysis summary".to_string(),
                    response,
                    0.70,
                    "metrics_agent".to_string(),
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

    fn make_agent(response: &str) -> MetricsAgent {
        MetricsAgent::new(Arc::new(MockClaudeClient::new(response)))
    }

    #[tokio::test]
    async fn test_detects_spike_keyword() {
        let agent = make_agent("ok");
        let state = SwarmState::new();
        let input = "CPU spike observed at 14:00 UTC";
        let findings = agent.analyze(input, &state).await.unwrap();
        assert!(findings
            .iter()
            .any(|f| f.category == FindingCategory::MetricSpike));
    }

    #[tokio::test]
    async fn test_detects_statistical_outlier() {
        let agent = make_agent("ok");
        let state = SwarmState::new();
        // Normal values ~100, outlier = 500
        let input = "response_time: 100 102 98 101 99 500 103 97";
        let findings = agent.analyze(input, &state).await.unwrap();
        assert!(findings
            .iter()
            .any(|f| f.category == FindingCategory::MetricSpike));
    }

    #[tokio::test]
    async fn test_detects_increasing_trend() {
        let agent = make_agent("ok");
        let state = SwarmState::new();
        let input = "latency: 10 20 30 40 50 60";
        let findings = agent.analyze(input, &state).await.unwrap();
        assert!(findings
            .iter()
            .any(|f| f.description.contains("increasing")));
    }

    #[tokio::test]
    async fn test_extract_numbers() {
        let nums = MetricsAgent::extract_numbers("cpu 45.2 mem 78 disk 92.1");
        assert_eq!(nums, vec![45.2, 78.0, 92.1]);
    }

    #[tokio::test]
    async fn test_capabilities() {
        let agent = make_agent("");
        assert!(agent.capabilities().contains(&"metrics_analysis".to_string()));
    }
}
