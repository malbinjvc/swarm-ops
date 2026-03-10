use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

// ── Severity & Category ─────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[serde(rename_all = "snake_case")]
pub enum Severity {
    Critical,
    High,
    Medium,
    Low,
    Info,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum FindingCategory {
    ErrorPattern,
    PerformanceDegradation,
    SecurityAnomaly,
    ServiceOutage,
    ResourceExhaustion,
    ConfigurationDrift,
    AnomalousBehavior,
    MetricSpike,
    MetricDrop,
    Trend,
}

// ── Finding ─────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Finding {
    pub id: String,
    pub severity: Severity,
    pub category: FindingCategory,
    pub description: String,
    pub evidence: String,
    pub confidence: f64,
    pub agent_id: String,
    pub timestamp: DateTime<Utc>,
}

impl Finding {
    pub fn new(
        severity: Severity,
        category: FindingCategory,
        description: String,
        evidence: String,
        confidence: f64,
        agent_id: String,
    ) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            severity,
            category,
            description,
            evidence,
            confidence: confidence.clamp(0.0, 1.0),
            agent_id,
            timestamp: Utc::now(),
        }
    }
}

// ── Task types ──────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum TaskType {
    LogAnalysis,
    HealthCheck,
    Incident,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum TaskStatus {
    Pending,
    Running,
    Completed,
    Failed,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SwarmTask {
    pub id: String,
    pub task_type: TaskType,
    pub data: String,
    pub requested_agents: Vec<String>,
    pub status: TaskStatus,
    pub findings: Vec<Finding>,
    pub created_at: DateTime<Utc>,
    pub completed_at: Option<DateTime<Utc>>,
}

impl SwarmTask {
    pub fn new(task_type: TaskType, data: String, requested_agents: Vec<String>) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            task_type,
            data,
            requested_agents,
            status: TaskStatus::Pending,
            findings: Vec::new(),
            created_at: Utc::now(),
            completed_at: None,
        }
    }
}

// ── Agent configuration ─────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentConfig {
    pub id: String,
    pub name: String,
    pub agent_type: String,
    pub capabilities: Vec<String>,
    pub enabled: bool,
    pub registered_at: DateTime<Utc>,
}

impl AgentConfig {
    pub fn new(name: String, agent_type: String, capabilities: Vec<String>) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            name,
            agent_type,
            capabilities,
            enabled: true,
            registered_at: Utc::now(),
        }
    }
}

// ── Consensus ───────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConsensusResult {
    pub id: String,
    pub merged_findings: Vec<Finding>,
    pub agreement_rate: f64,
    pub total_input_findings: usize,
    pub total_merged_findings: usize,
    pub created_at: DateTime<Utc>,
}

// ── Swarm statistics ────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SwarmStats {
    pub total_tasks: usize,
    pub total_agents: usize,
    pub total_findings: usize,
    pub consensus_rate: f64,
    pub tasks_by_status: HashMap<String, usize>,
}

// ── API request / response bodies ───────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct AnalyzeRequest {
    pub data: String,
    pub task_type: TaskType,
    pub agents: Vec<String>,
}

#[derive(Debug, Deserialize)]
pub struct RegisterAgentRequest {
    pub name: String,
    pub agent_type: String,
    pub capabilities: Vec<String>,
}

#[derive(Debug, Deserialize)]
pub struct ConsensusRequest {
    pub finding_ids: Vec<String>,
}

#[derive(Debug, Serialize)]
pub struct ApiResponse<T: Serialize> {
    pub success: bool,
    pub data: T,
}

impl<T: Serialize> ApiResponse<T> {
    pub fn ok(data: T) -> Self {
        Self {
            success: true,
            data,
        }
    }
}

#[derive(Debug, Serialize)]
pub struct ErrorResponse {
    pub success: bool,
    pub error: String,
}

impl ErrorResponse {
    pub fn new(error: impl Into<String>) -> Self {
        Self {
            success: false,
            error: error.into(),
        }
    }
}

// ── Agent errors ────────────────────────────────────────────────────

#[derive(Debug)]
pub enum AgentError {
    AnalysisFailed(String),
    ClaudeApiError(String),
    InvalidInput(String),
}

impl std::fmt::Display for AgentError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AgentError::AnalysisFailed(msg) => write!(f, "Analysis failed: {}", msg),
            AgentError::ClaudeApiError(msg) => write!(f, "Claude API error: {}", msg),
            AgentError::InvalidInput(msg) => write!(f, "Invalid input: {}", msg),
        }
    }
}

impl std::error::Error for AgentError {}

// ── Claude API types ────────────────────────────────────────────────

#[derive(Debug, Serialize)]
pub struct ClaudeMessage {
    pub role: String,
    pub content: String,
}

#[derive(Debug, Serialize)]
pub struct ClaudeRequest {
    pub model: String,
    pub max_tokens: u32,
    pub messages: Vec<ClaudeMessage>,
}

#[derive(Debug, Deserialize)]
pub struct ClaudeContentBlock {
    #[serde(rename = "type")]
    pub block_type: String,
    pub text: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct ClaudeResponse {
    pub id: String,
    pub content: Vec<ClaudeContentBlock>,
}

impl ClaudeResponse {
    pub fn text(&self) -> String {
        self.content
            .iter()
            .filter_map(|b| b.text.as_deref())
            .collect::<Vec<_>>()
            .join("")
    }
}
