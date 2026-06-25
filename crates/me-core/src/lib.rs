use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use serde_json::Value;

pub const SCHEMA_VERSION: u32 = 5;
pub const WORKSPACE_VERSION: &str = "0.7.0";

pub const THOUGHT_KINDS: &[&str] = &[
    "idea",
    "observation",
    "question",
    "memory",
    "quotation",
    "reaction",
    "recurrence",
    "doubt",
    "repeated-realization",
    "document-excerpt",
    "chat-excerpt",
    "external-excerpt",
    "unfinished-statement",
    "other",
];

pub const THOUGHT_STATES: &[&str] = &["pending", "added", "kept-only", "dismissed"];

pub const COGNITION_STATES: &[&str] = &["active", "retired"];

pub const COLLECTION_ACTIONS: &[&str] = &[
    "add-cognition",
    "keep-thought-only",
    "dismiss-thought",
    "retire-cognition",
    "reactivate-cognition",
];

pub const OPERATIONS: &[&str] = &[
    "init",
    "capture-thought",
    "add-cognition",
    "keep-thought-only",
    "dismiss-thought",
    "retire-cognition",
    "reactivate-cognition",
    "restore-snapshot",
    "demo-seed",
    "migrate-from-my-model",
    "migrate-from-v3",
    "migrate-from-v4",
];

pub const APP_FINDING_LABELS: &[&str] = &[
    "similar",
    "supports",
    "extends",
    "qualifies",
    "challenges",
    "contradicts",
    "depends-on",
    "exemplifies",
    "supersedes-for-this-task",
    "irrelevant",
    "unclear",
];

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ObjectEnvelope<T> {
    #[serde(rename = "schemaVersion")]
    pub schema_version: u32,
    #[serde(rename = "objectType")]
    pub object_type: String,
    pub payload: T,
}

impl<T> ObjectEnvelope<T> {
    pub fn new(object_type: impl Into<String>, payload: T) -> Self {
        Self {
            schema_version: SCHEMA_VERSION,
            object_type: object_type.into(),
            payload,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Origin {
    #[serde(rename = "type")]
    pub origin_type: String,
    pub uri: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub attribution: Option<String>,
}

impl Origin {
    pub fn local_input() -> Self {
        Self {
            origin_type: "local-input".to_string(),
            uri: None,
            attribution: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ThoughtPayload {
    pub thought_id: String,
    pub kind: String,
    pub body_markdown: String,
    pub body_text: String,
    pub origin: Origin,
    pub captured_at: String,
    pub captured_by: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CognitionPayload {
    pub cognition_id: String,
    pub body_markdown: String,
    pub body_text: String,
    pub display_title: Option<String>,
    pub origin_thought: String,
    pub added_by_decision: String,
    pub added_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RelatedCognition {
    pub cognition: String,
    pub cognition_id: String,
    pub score: f64,
    pub status: String,
    #[serde(default)]
    pub matched_terms: Vec<String>,
    pub explanation: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GeneratedBy {
    pub host: String,
    pub model: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProposalPayload {
    pub proposal_id: String,
    pub kind: String,
    pub base_snapshot: String,
    #[serde(default)]
    pub inputs: Value,
    pub recommendation: Value,
    #[serde(default)]
    pub related_cognitions: Vec<RelatedCognition>,
    #[serde(default)]
    pub alternatives: Vec<Value>,
    pub generated_by: GeneratedBy,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DecisionPayload {
    pub decision_id: String,
    pub base_snapshot: String,
    pub action: String,
    pub actor: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub thought: Option<String>,
    pub final_body_markdown: Option<String>,
    pub note_markdown: Option<String>,
    pub decided_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppDefinitionPayload {
    pub app_id: String,
    pub name: String,
    pub version: String,
    pub manifest_hash: String,
    pub installed_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppPolicyPayload {
    pub policy_id: String,
    pub app_id: String,
    pub app_version_range: String,
    pub rule: Value,
    pub created_by_decision: String,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SelectedCognition {
    pub cognition: String,
    pub cognition_id: String,
    pub selection_reason: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppFinding {
    pub label: String,
    #[serde(default)]
    pub cognitions: Vec<String>,
    #[serde(default)]
    pub passages: Vec<String>,
    pub reason_markdown: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub app_rule: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct AppAnalysis {
    #[serde(default)]
    pub findings: Vec<AppFinding>,
    #[serde(default)]
    pub gaps: Vec<String>,
    #[serde(default)]
    pub conflicts: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppResolution {
    pub scope: String,
    pub decision: String,
    pub instruction_markdown: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppRunOutput {
    pub kind: String,
    pub body_markdown: String,
    pub external_action: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppRunPayload {
    pub run_id: String,
    pub app_id: String,
    pub app_version: String,
    pub base_snapshot: String,
    pub task_markdown: String,
    #[serde(default)]
    pub selected_cognitions: Vec<SelectedCognition>,
    #[serde(default)]
    pub analysis: AppAnalysis,
    #[serde(default)]
    pub resolutions: Vec<AppResolution>,
    #[serde(default)]
    pub app_policies_used: Vec<String>,
    pub output: AppRunOutput,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct MeTreePayload {
    pub thoughts: BTreeMap<String, String>,
    pub thought_states: BTreeMap<String, String>,
    pub cognitions: BTreeMap<String, String>,
    pub cognition_states: BTreeMap<String, String>,
    pub decisions: BTreeMap<String, String>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub proposals: BTreeMap<String, String>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub apps: BTreeMap<String, String>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub app_policies: BTreeMap<String, String>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub app_runs: BTreeMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MeSnapshotPayload {
    pub parent: Option<String>,
    pub tree: String,
    pub operation: String,
    pub actor: String,
    pub message: String,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceConfig {
    pub schema_version: u32,
    pub workspace_id: String,
    pub name: String,
    pub default_actor: String,
    pub agent: AgentConfig,
    pub privacy: PrivacyConfig,
    pub index: IndexConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentConfig {
    pub preferred_host: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PrivacyConfig {
    pub me_network_access: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IndexConfig {
    pub engine: String,
    pub rebuild_on_integrity_failure: bool,
}

impl WorkspaceConfig {
    pub fn new(workspace_id: String, name: String) -> Self {
        Self {
            schema_version: SCHEMA_VERSION,
            workspace_id,
            name,
            default_actor: "local-user".to_string(),
            agent: AgentConfig {
                preferred_host: "codex".to_string(),
            },
            privacy: PrivacyConfig {
                me_network_access: "forbidden".to_string(),
            },
            index: IndexConfig {
                engine: "sqlite".to_string(),
                rebuild_on_integrity_failure: true,
            },
        }
    }
}

pub fn sha_ref(hex: &str) -> String {
    format!("sha256:{hex}")
}

pub fn strip_sha_prefix(value: &str) -> Option<&str> {
    value.strip_prefix("sha256:")
}

pub fn is_sha_ref(value: &str) -> bool {
    let Some(hex) = strip_sha_prefix(value) else {
        return false;
    };
    hex.len() == 64
        && hex
            .chars()
            .all(|ch| ch.is_ascii_hexdigit() && !ch.is_ascii_uppercase())
}

pub fn thought_kind_allowed(value: &str) -> bool {
    THOUGHT_KINDS.contains(&value)
}

pub fn thought_state_allowed(value: &str) -> bool {
    THOUGHT_STATES.contains(&value)
}

pub fn cognition_state_allowed(value: &str) -> bool {
    COGNITION_STATES.contains(&value)
}

pub fn collection_action_allowed(value: &str) -> bool {
    COLLECTION_ACTIONS.contains(&value)
}

pub fn app_finding_label_allowed(value: &str) -> bool {
    APP_FINDING_LABELS.contains(&value)
}

pub fn operation_allowed(value: &str) -> bool {
    OPERATIONS.contains(&value)
}

pub fn slugify(input: &str) -> String {
    let mut out = String::new();
    let mut last_dash = false;
    for ch in input.chars().flat_map(char::to_lowercase) {
        if ch.is_ascii_alphanumeric() {
            out.push(ch);
            last_dash = false;
        } else if !last_dash {
            out.push('-');
            last_dash = true;
        }
    }
    out.trim_matches('-').to_string()
}
