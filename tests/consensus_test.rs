use swarm_ops::models::*;
use swarm_ops::swarm::consensus::ConsensusEngine;

fn make(sev: Severity, cat: FindingCategory, desc: &str, conf: f64, agent: &str) -> Finding {
    Finding::new(sev, cat, desc.into(), "ev".into(), conf, agent.into())
}

#[test]
fn test_empty_consensus() {
    let r = ConsensusEngine::run(&[]);
    assert_eq!(r.total_input_findings, 0);
    assert_eq!(r.total_merged_findings, 0);
}

#[test]
fn test_all_unique_findings() {
    let findings = vec![
        make(Severity::High, FindingCategory::ErrorPattern, "err A", 0.9, "a1"),
        make(Severity::Low, FindingCategory::MetricSpike, "spike B", 0.6, "a2"),
        make(Severity::Medium, FindingCategory::Trend, "trend C", 0.7, "a3"),
    ];
    let r = ConsensusEngine::run(&findings);
    assert_eq!(r.total_merged_findings, 3);
    assert_eq!(r.agreement_rate, 0.0); // nothing merged
}

#[test]
fn test_merge_identical_findings() {
    let findings = vec![
        make(
            Severity::High,
            FindingCategory::ErrorPattern,
            "NullPointerException in auth module",
            0.8,
            "a1",
        ),
        make(
            Severity::Medium,
            FindingCategory::ErrorPattern,
            "NullPointerException in auth module",
            0.6,
            "a2",
        ),
    ];
    let r = ConsensusEngine::run(&findings);
    assert_eq!(r.total_merged_findings, 1);
    let merged = &r.merged_findings[0];
    assert!((merged.confidence - 0.7).abs() < 0.01);
    assert_eq!(merged.severity, Severity::High);
}

#[test]
fn test_merge_three_way() {
    let findings = vec![
        make(
            Severity::High,
            FindingCategory::ErrorPattern,
            "timeout in payment service handler",
            0.9,
            "a1",
        ),
        make(
            Severity::High,
            FindingCategory::ErrorPattern,
            "timeout in payment service handler",
            0.8,
            "a2",
        ),
        make(
            Severity::Medium,
            FindingCategory::ErrorPattern,
            "timeout in payment service handler",
            0.7,
            "a3",
        ),
    ];
    let r = ConsensusEngine::run(&findings);
    assert_eq!(r.total_merged_findings, 1);
    assert!((r.merged_findings[0].confidence - 0.8).abs() < 0.01);
}

#[test]
fn test_severity_ordering_in_output() {
    let findings = vec![
        make(Severity::Low, FindingCategory::Trend, "trend", 0.5, "a"),
        make(Severity::Critical, FindingCategory::ServiceOutage, "outage", 0.9, "b"),
        make(Severity::High, FindingCategory::ErrorPattern, "error", 0.7, "c"),
        make(Severity::Info, FindingCategory::AnomalousBehavior, "anomaly", 0.4, "d"),
    ];
    let r = ConsensusEngine::run(&findings);
    assert_eq!(r.merged_findings[0].severity, Severity::Critical);
    assert_eq!(r.merged_findings[1].severity, Severity::High);
    assert_eq!(r.merged_findings[2].severity, Severity::Low);
    assert_eq!(r.merged_findings[3].severity, Severity::Info);
}

#[test]
fn test_confidence_clamped() {
    let f = Finding::new(
        Severity::High,
        FindingCategory::ErrorPattern,
        "test".into(),
        "ev".into(),
        1.5, // should clamp to 1.0
        "a".into(),
    );
    assert!((f.confidence - 1.0).abs() < f64::EPSILON);
}
