use chrono::Utc;
use uuid::Uuid;

use crate::models::{ConsensusResult, Finding, FindingCategory};

/// The consensus engine: merges overlapping findings, passes through unique
/// ones, and produces a final sorted report.
pub struct ConsensusEngine;

impl ConsensusEngine {
    /// Run consensus on a set of findings.
    ///
    /// Algorithm:
    /// 1. Group findings by category.
    /// 2. Within each group, merge findings whose descriptions are similar
    ///    (simple substring match) by averaging confidence.
    /// 3. Unique findings pass through with original confidence.
    /// 4. Sort the final list by severity (Critical > … > Info), then by
    ///    descending confidence.
    pub fn run(findings: &[Finding]) -> ConsensusResult {
        let total_input = findings.len();

        // Group by category
        let mut groups: std::collections::HashMap<FindingCategory, Vec<&Finding>> =
            std::collections::HashMap::new();
        for f in findings {
            groups.entry(f.category.clone()).or_default().push(f);
        }

        let mut merged: Vec<Finding> = Vec::new();

        for group in groups.values() {
            let mut used = vec![false; group.len()];

            for i in 0..group.len() {
                if used[i] {
                    continue;
                }
                let mut cluster = vec![group[i]];
                used[i] = true;

                for j in (i + 1)..group.len() {
                    if used[j] {
                        continue;
                    }
                    if Self::similar(&group[i].description, &group[j].description) {
                        cluster.push(group[j]);
                        used[j] = true;
                    }
                }

                merged.push(Self::merge_cluster(&cluster));
            }
        }

        // Sort by severity then descending confidence
        merged.sort_by(|a, b| {
            a.severity
                .cmp(&b.severity)
                .then_with(|| b.confidence.partial_cmp(&a.confidence).unwrap())
        });

        let total_merged = merged.len();

        // Agreement rate: proportion of findings that were merged (i.e. had
        // overlapping peers).
        let agreement_rate = if total_input > 0 {
            let merged_away = total_input - total_merged;
            merged_away as f64 / total_input as f64
        } else {
            0.0
        };

        ConsensusResult {
            id: Uuid::new_v4().to_string(),
            merged_findings: merged,
            agreement_rate,
            total_input_findings: total_input,
            total_merged_findings: total_merged,
            created_at: Utc::now(),
        }
    }

    /// Simple similarity check: two descriptions are "similar" if one contains
    /// the other, or they share a long common prefix (>= 20 chars).
    fn similar(a: &str, b: &str) -> bool {
        let al = a.to_lowercase();
        let bl = b.to_lowercase();
        if al.contains(&bl) || bl.contains(&al) {
            return true;
        }
        // common-prefix heuristic
        let common: usize = al
            .chars()
            .zip(bl.chars())
            .take_while(|(ca, cb)| ca == cb)
            .count();
        common >= 20
    }

    /// Merge a cluster of similar findings into one.
    fn merge_cluster(cluster: &[&Finding]) -> Finding {
        let avg_confidence =
            cluster.iter().map(|f| f.confidence).sum::<f64>() / cluster.len() as f64;

        // Pick the highest severity in the cluster
        let best_severity = cluster
            .iter()
            .map(|f| &f.severity)
            .min() // Severity ordering: Critical < High < … < Info
            .unwrap()
            .clone();

        let evidence = cluster
            .iter()
            .map(|f| f.evidence.as_str())
            .collect::<Vec<_>>()
            .join(" | ");

        let agents: Vec<String> = cluster.iter().map(|f| f.agent_id.clone()).collect();
        let agent_label = agents.join("+");

        Finding::new(
            best_severity,
            cluster[0].category.clone(),
            cluster[0].description.clone(),
            evidence,
            avg_confidence,
            agent_label,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{FindingCategory, Severity};

    fn make_finding(
        severity: Severity,
        category: FindingCategory,
        desc: &str,
        confidence: f64,
        agent: &str,
    ) -> Finding {
        Finding::new(
            severity,
            category,
            desc.to_string(),
            "ev".to_string(),
            confidence,
            agent.to_string(),
        )
    }

    #[test]
    fn test_consensus_no_findings() {
        let result = ConsensusEngine::run(&[]);
        assert_eq!(result.total_input_findings, 0);
        assert_eq!(result.total_merged_findings, 0);
        assert_eq!(result.agreement_rate, 0.0);
    }

    #[test]
    fn test_consensus_unique_findings_pass_through() {
        let findings = vec![
            make_finding(Severity::High, FindingCategory::ErrorPattern, "Error A", 0.8, "a1"),
            make_finding(Severity::Low, FindingCategory::MetricSpike, "Spike B", 0.6, "a2"),
        ];
        let result = ConsensusEngine::run(&findings);
        assert_eq!(result.total_merged_findings, 2);
    }

    #[test]
    fn test_consensus_merges_overlapping() {
        let findings = vec![
            make_finding(
                Severity::High,
                FindingCategory::ErrorPattern,
                "NullPointerException in auth service module",
                0.8,
                "agent-1",
            ),
            make_finding(
                Severity::Medium,
                FindingCategory::ErrorPattern,
                "NullPointerException in auth service module",
                0.6,
                "agent-2",
            ),
        ];
        let result = ConsensusEngine::run(&findings);
        assert_eq!(result.total_merged_findings, 1);
        let merged = &result.merged_findings[0];
        assert!((merged.confidence - 0.7).abs() < 0.01);
        assert_eq!(merged.severity, Severity::High); // picks highest
    }

    #[test]
    fn test_consensus_sorted_by_severity_then_confidence() {
        let findings = vec![
            make_finding(Severity::Low, FindingCategory::Trend, "trend", 0.9, "a"),
            make_finding(
                Severity::Critical,
                FindingCategory::ServiceOutage,
                "outage",
                0.5,
                "b",
            ),
            make_finding(Severity::High, FindingCategory::ErrorPattern, "err", 0.7, "c"),
        ];
        let result = ConsensusEngine::run(&findings);
        assert_eq!(result.merged_findings[0].severity, Severity::Critical);
        assert_eq!(result.merged_findings[1].severity, Severity::High);
        assert_eq!(result.merged_findings[2].severity, Severity::Low);
    }

    #[test]
    fn test_consensus_agreement_rate() {
        // 3 findings, 2 merge into 1 => 1 merged away out of 3 => 1/3
        let findings = vec![
            make_finding(
                Severity::High,
                FindingCategory::ErrorPattern,
                "same error pattern found in logs",
                0.8,
                "a1",
            ),
            make_finding(
                Severity::High,
                FindingCategory::ErrorPattern,
                "same error pattern found in logs",
                0.7,
                "a2",
            ),
            make_finding(Severity::Low, FindingCategory::Trend, "unique", 0.5, "a3"),
        ];
        let result = ConsensusEngine::run(&findings);
        assert_eq!(result.total_input_findings, 3);
        assert_eq!(result.total_merged_findings, 2);
        assert!((result.agreement_rate - 1.0 / 3.0).abs() < 0.01);
    }

    #[test]
    fn test_similar_substring() {
        assert!(ConsensusEngine::similar("Error in auth", "Error in auth service"));
    }

    #[test]
    fn test_not_similar() {
        assert!(!ConsensusEngine::similar("Error A", "Spike B"));
    }
}
