use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use serde_json::Value;

pub const SCHEMA_VERSION: u32 = 1;
pub const WORKSPACE_VERSION: &str = "0.1.0";

pub const RELATIONSHIPS: &[&str] = &[
    "exact-duplicate",
    "recurrence",
    "reinforcement",
    "clarification",
    "deepening",
    "extension",
    "reframing",
    "narrowing",
    "challenge",
    "contradiction",
    "novel",
    "unresolved",
];

pub const OPERATIONS: &[&str] = &[
    "ignore-exact-duplicate",
    "record-recurrence",
    "record-reinforcement",
    "record-challenge",
    "revise-facet",
    "create-facet",
    "create-article",
    "update-article-composition",
    "defer",
    "reject",
];

pub const SOURCE_KINDS: &[&str] = &[
    "note",
    "journal",
    "chat-excerpt",
    "quotation",
    "web-excerpt",
    "social-post",
    "document-excerpt",
    "observation",
    "question",
    "other",
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
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SourcePayload {
    pub source_id: String,
    pub kind: String,
    pub title: Option<String>,
    pub body_markdown: String,
    pub origin: Origin,
    pub captured_at: String,
    pub captured_by: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FacetRevisionPayload {
    pub facet_id: String,
    pub revision: u64,
    pub title: String,
    pub body_markdown: String,
    pub body_text: String,
    pub parent_revision: Option<String>,
    pub change_kind: String,
    pub derived_from_sources: Vec<String>,
    pub honed_against_facets: Vec<String>,
    pub decision: String,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EventTarget {
    pub facet_id: String,
    pub facet_revision: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HoneEventPayload {
    pub event_id: String,
    pub source: String,
    pub relationship: String,
    pub targets: Vec<EventTarget>,
    pub effect: Value,
    pub proposal: String,
    pub decision: String,
    pub recorded_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ArticleSegment {
    #[serde(rename = "type")]
    pub segment_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub body_markdown: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub facet_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub facet_revision: Option<String>,
}

impl ArticleSegment {
    pub fn prose(body_markdown: impl Into<String>) -> Self {
        Self {
            segment_type: "prose".to_string(),
            body_markdown: Some(body_markdown.into()),
            facet_id: None,
            facet_revision: None,
        }
    }

    pub fn facet(facet_id: impl Into<String>, facet_revision: impl Into<String>) -> Self {
        Self {
            segment_type: "facet".to_string(),
            body_markdown: None,
            facet_id: Some(facet_id.into()),
            facet_revision: Some(facet_revision.into()),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ArticleEditionPayload {
    pub article_id: String,
    pub edition: u64,
    pub title: String,
    pub parent_edition: Option<String>,
    pub segments: Vec<ArticleSegment>,
    pub decision: String,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProposalCandidate {
    pub facet_id: String,
    pub facet_revision: String,
    pub relationship: String,
    #[serde(default)]
    pub input_passages: Vec<String>,
    #[serde(default)]
    pub facet_passages: Vec<String>,
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
    pub source: String,
    pub base_snapshot: String,
    #[serde(default)]
    pub candidates: Vec<ProposalCandidate>,
    pub recommendation: Value,
    #[serde(default)]
    pub unresolved: Vec<String>,
    pub generated_by: GeneratedBy,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DecisionPayload {
    pub decision_id: String,
    pub proposal: String,
    pub base_snapshot: String,
    pub action: String,
    pub actor: String,
    pub final_operation: Value,
    pub note: Option<String>,
    pub decided_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct TreePayload {
    pub sources: BTreeMap<String, String>,
    pub facets: BTreeMap<String, String>,
    pub articles: BTreeMap<String, String>,
    pub events: BTreeMap<String, String>,
    pub proposals: BTreeMap<String, String>,
    pub decisions: BTreeMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SnapshotPayload {
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
    pub structure: StructureConfig,
    pub agent: AgentConfig,
    pub privacy: PrivacyConfig,
    pub index: IndexConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StructureConfig {
    pub article_heading_level: u8,
    pub facet_heading_level: u8,
    pub allow_other_heading_levels: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentConfig {
    pub preferred_host: String,
    pub proposal_limit: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PrivacyConfig {
    pub hone_network_access: String,
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
            structure: StructureConfig {
                article_heading_level: 1,
                facet_heading_level: 2,
                allow_other_heading_levels: false,
            },
            agent: AgentConfig {
                preferred_host: "codex".to_string(),
                proposal_limit: 5,
            },
            privacy: PrivacyConfig {
                hone_network_access: "forbidden".to_string(),
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

pub fn slugify(input: &str) -> String {
    let mut out = String::new();
    let mut last_dash = false;
    for ch in input.chars() {
        let lower = ch.to_ascii_lowercase();
        if lower.is_ascii_alphanumeric() {
            out.push(lower);
            last_dash = false;
        } else if !last_dash {
            out.push('-');
            last_dash = true;
        }
    }
    let trimmed = out.trim_matches('-').to_string();
    if trimmed.is_empty() {
        "untitled".to_string()
    } else {
        trimmed
    }
}

pub fn relationship_allowed(value: &str) -> bool {
    RELATIONSHIPS.contains(&value)
}

pub fn operation_allowed(value: &str) -> bool {
    OPERATIONS.contains(&value)
}

pub fn source_kind_allowed(value: &str) -> bool {
    SOURCE_KINDS.contains(&value)
}
