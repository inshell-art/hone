#![allow(dead_code)]

use std::collections::{BTreeMap, BTreeSet};
use std::fs::{self, File, OpenOptions};
use std::io::{Read, Write};
use std::path::{Component, Path, PathBuf};

use fs2::FileExt;
use me_core::{
    AppAnalysis, AppDefinitionPayload, AppFinding, AppRunOutput, AppRunPayload, CognitionPayload,
    DecisionPayload, GeneratedBy, MeSnapshotPayload, MeTreePayload, ObjectEnvelope, Origin,
    RelatedCognition, SCHEMA_VERSION, SelectedCognition, THOUGHT_KINDS, ThoughtPayload,
    WORKSPACE_VERSION, cognition_state_allowed, is_sha_ref, operation_allowed, sha_ref,
    strip_sha_prefix, thought_kind_allowed,
};
use me_index::{CognitionDoc, MatchResult, rank_cognitions};
use me_markdown::markdown_to_text;
use rusqlite::{Connection, params};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use tar::{Archive, Builder, Header};
use tempfile::NamedTempFile;
use thiserror::Error;
use time::{OffsetDateTime, format_description::well_known::Rfc3339};
use ulid::Ulid;
use walkdir::WalkDir;

pub type Result<T> = std::result::Result<T, MeError>;

#[derive(Debug, Error)]
pub enum MeError {
    #[error("{message}")]
    InvalidInput {
        code: &'static str,
        message: String,
        details: Value,
    },
    #[error("{message}")]
    NotFound {
        code: &'static str,
        message: String,
        details: Value,
    },
    #[error("{message}")]
    StaleProposal {
        code: &'static str,
        message: String,
        details: Value,
    },
    #[error("{message}")]
    WorkspaceLocked {
        code: &'static str,
        message: String,
        details: Value,
    },
    #[error("{message}")]
    Integrity {
        code: &'static str,
        message: String,
        details: Value,
    },
    #[error("{message}")]
    UnsupportedWorkspace {
        code: &'static str,
        message: String,
        details: Value,
    },
    #[error(transparent)]
    Internal(#[from] anyhow::Error),
}

impl MeError {
    pub fn code(&self) -> &'static str {
        match self {
            Self::InvalidInput { code, .. }
            | Self::NotFound { code, .. }
            | Self::StaleProposal { code, .. }
            | Self::WorkspaceLocked { code, .. }
            | Self::Integrity { code, .. }
            | Self::UnsupportedWorkspace { code, .. } => code,
            Self::Internal(_) => "INTERNAL_ERROR",
        }
    }

    pub fn details(&self) -> Value {
        match self {
            Self::InvalidInput { details, .. }
            | Self::NotFound { details, .. }
            | Self::StaleProposal { details, .. }
            | Self::WorkspaceLocked { details, .. }
            | Self::Integrity { details, .. }
            | Self::UnsupportedWorkspace { details, .. } => details.clone(),
            Self::Internal(_) => json!({}),
        }
    }

    pub fn exit_code(&self) -> i32 {
        match self {
            Self::InvalidInput { .. } => 2,
            Self::StaleProposal { .. } => 3,
            Self::WorkspaceLocked { .. } => 4,
            Self::Integrity { .. } => 5,
            Self::NotFound { .. } => 6,
            Self::UnsupportedWorkspace { .. } => 8,
            Self::Internal(_) => 9,
        }
    }
}

fn invalid(message: impl Into<String>) -> MeError {
    MeError::InvalidInput {
        code: "INVALID_INPUT",
        message: message.into(),
        details: json!({}),
    }
}

fn not_found(message: impl Into<String>) -> MeError {
    MeError::NotFound {
        code: "NOT_FOUND",
        message: message.into(),
        details: json!({}),
    }
}

fn integrity(message: impl Into<String>) -> MeError {
    MeError::Integrity {
        code: "INTEGRITY_FAILURE",
        message: message.into(),
        details: json!({}),
    }
}

fn parse_decision_value(raw: &str) -> Result<Value> {
    if raw.trim().is_empty() {
        Ok(json!({}))
    } else {
        serde_json::from_str(raw).map_err(|err| MeError::Internal(err.into()))
    }
}

fn has_explicit_keep_approval(value: &Value) -> bool {
    value.get("approved").and_then(Value::as_bool) == Some(true)
}

fn now_rfc3339() -> Result<String> {
    OffsetDateTime::now_utc()
        .format(&Rfc3339)
        .map_err(|err| MeError::Internal(err.into()))
}

fn new_id(prefix: &str) -> String {
    format!("{prefix}_{}", Ulid::new())
}

#[derive(Debug, Clone)]
pub struct Workspace {
    root: PathBuf,
}

#[derive(Debug, Clone)]
pub struct CurrentState {
    pub snapshot_hash: String,
    pub snapshot: MeSnapshotPayload,
    pub tree_hash: String,
    pub tree: MeTreePayload,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GuidanceState {
    schema_version: u32,
    #[serde(default)]
    first_cognition_guide_shown: bool,
    #[serde(default)]
    first_read_guide_shown: bool,
    #[serde(default)]
    feedback_loop_guide_shown: bool,
    #[serde(default)]
    two_cognition_guide_shown: bool,
    #[serde(default)]
    five_cognition_guide_shown: bool,
}

impl Default for GuidanceState {
    fn default() -> Self {
        Self {
            schema_version: 1,
            first_cognition_guide_shown: false,
            first_read_guide_shown: false,
            feedback_loop_guide_shown: false,
            two_cognition_guide_shown: false,
            five_cognition_guide_shown: false,
        }
    }
}

impl Workspace {
    pub fn open(path: impl AsRef<Path>) -> Result<Self> {
        let root = path.as_ref().to_path_buf();
        let ws = Self { root };
        ws.ensure_supported()?;
        Ok(ws)
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn init(path: impl AsRef<Path>, demo: bool) -> Result<Value> {
        let root = path.as_ref().to_path_buf();
        if root.join(".me/refs/current").exists() {
            return Err(invalid("Workspace is already initialized"));
        }
        fs::create_dir_all(&root).map_err(|err| MeError::Internal(err.into()))?;
        let ws = Self { root };
        ws.create_layout()?;
        ws.write_config()?;
        ws.sync_workspace_docs()?;
        ws.write_initial_snapshot()?;
        ws.rebuild_index()?;
        let mut result = workspace_created_result(&ws.root);
        if demo {
            let demo_result = ws.seed_demo()?;
            result["demo"] = demo_result;
        }
        Ok(result)
    }

    pub fn new_workspace(path: impl AsRef<Path>, demo: bool) -> Result<Value> {
        let path = path.as_ref();
        if path.exists()
            && path
                .read_dir()
                .map_err(|err| MeError::Internal(err.into()))?
                .next()
                .is_some()
        {
            return Err(invalid(format!(
                "Target directory is not empty: {}",
                path.display()
            )));
        }
        Self::init(path, demo)
    }

    pub fn home(&self, format: &str) -> Result<Value> {
        let current = self.load_current()?;
        let active_count = current
            .tree
            .cognition_states
            .values()
            .filter(|state| state.as_str() == "active")
            .count();
        let retired_count = current
            .tree
            .cognition_states
            .values()
            .filter(|state| state.as_str() == "retired")
            .count();
        let pending_thoughts = current
            .tree
            .thought_states
            .values()
            .filter(|state| state.as_str() == "pending")
            .count();
        let workspace_state = if current.tree.thoughts.is_empty()
            && current.tree.cognitions.is_empty()
            && current.tree.decisions.is_empty()
        {
            "empty"
        } else {
            "established"
        };
        let mut data = json!({
            "schemaVersion": 1,
            "kind": "me.home",
            "workspaceState": workspace_state,
            "product": {
                "name": "ME",
                "descriptor": "a local meaning environment",
                "primarySurface": "Codex App"
            },
            "examples": {
                "add": "Add this thought to ME:",
                "inspect": if workspace_state == "empty" {
                    "What do I have in ME about authorship?"
                } else {
                    "What do I have in ME about generative art?"
                },
                "compose": if workspace_state == "empty" {
                    "Draft a reply using ME."
                } else {
                    "Draft an artist statement using ME."
                },
                "compare": "Find tension in ME about authorship.",
                "return": "This is my thought. Add it to ME."
            },
            "health": {
                "status": "ok",
                "userMessage": Value::Null
            }
        });
        if workspace_state == "empty" {
            data["summary"] = json!({
                "cognitionCount": 0,
                "pendingThoughtCount": 0
            });
            data["starterPrompt"] = json!("Add this thought to ME:");
        } else {
            data["summary"] = json!({
                "activeCognitionCount": active_count,
                "retiredCognitionCount": retired_count,
                "pendingThoughtCount": pending_thoughts
            });
            data["recent"] = json!(self.recent_home_events(&current.tree)?);
        }
        if format == "markdown" {
            Ok(json!({ "markdown": home_markdown(&data), "home": data }))
        } else {
            Ok(data)
        }
    }

    pub fn welcome(&self) -> Result<Value> {
        let current = self.load_current()?;
        let active_count = current
            .tree
            .cognition_states
            .values()
            .filter(|state| state.as_str() == "active")
            .count();
        let pending_thoughts = current
            .tree
            .thought_states
            .values()
            .filter(|state| state.as_str() == "pending")
            .count();
        let state = if active_count == 0 && current.tree.decisions.is_empty() {
            "empty"
        } else {
            "established"
        };
        Ok(json!({
            "schemaVersion": 2,
            "kind": "me.welcome",
            "state": state,
            "renderedMarkdown": welcome_markdown(state),
            "starterPrompt": "Add this thought to ME:",
            "technical": {
                "activeCognitionCount": active_count,
                "pendingThoughtCount": pending_thoughts
            }
        }))
    }

    pub fn guide(&self) -> Result<Value> {
        let scenarios = json!([
            {
                "title": "A thought occurs",
                "body": "Suppose you think: Designing a generative system is part of authorship. Tell Codex: Add this thought to ME: Designing a generative system is part of authorship. ME captures the exact text first. It is not in ME yet."
            },
            {
                "title": "Keep the thought",
                "body": "Codex asks whether to keep it. After you approve, ME adds it to the local Cognition Library. In ME, a thought you choose to keep is called a cognition."
            },
            {
                "title": "Use a cognition",
                "body": "Ask: What do I have in ME about authorship? Or ask: Draft a short statement using ME. Codex may read and compose from your cognitions. Reading and composing do not change ME."
            },
            {
                "title": "Keep something Codex produced",
                "body": "If Codex writes a sentence worth retaining, say: This is my thought. Add it to ME. The sentence returns through the same capture and keep flow."
            }
        ]);
        Ok(json!({
            "schemaVersion": 1,
            "kind": "me.guide",
            "scenarios": scenarios,
            "markdown": guide_markdown()
        }))
    }

    pub fn status(&self) -> Result<Value> {
        let config = self.config()?;
        let current = self.load_current()?;
        Ok(json!({
            "workspace": self.root,
            "workspaceId": config.workspace_id,
            "schemaVersion": config.schema_version,
            "currentSnapshot": current.snapshot_hash,
            "meTree": current.tree_hash,
            "counts": {
                "thoughts": current.tree.thoughts.len(),
                "cognitions": current.tree.cognitions.len(),
                "decisions": current.tree.decisions.len()
            }
        }))
    }

    pub fn current(&self) -> Result<Value> {
        let current = self.load_current()?;
        Ok(json!({
            "statusLabel": "CURRENT ME -- user-authorized local state",
            "currentSnapshot": current.snapshot_hash,
            "counts": {
                "thoughts": current.tree.thoughts.len(),
                "pendingThoughts": current.tree.thought_states.values().filter(|state| state.as_str() == "pending").count(),
                "activeCognitions": current.tree.cognition_states.values().filter(|state| state.as_str() == "active").count(),
                "retiredCognitions": current.tree.cognition_states.values().filter(|state| state.as_str() == "retired").count(),
                "decisions": current.tree.decisions.len()
            },
            "cognitions": self.current_cognition_summaries(&current.tree)?
        }))
    }

    pub fn doctor(&self, repair: bool) -> Result<Value> {
        self.ensure_supported()?;
        let mut repaired = Vec::new();
        if repair {
            self.regenerate_views()?;
            self.rebuild_index()?;
            self.sync_workspace_docs()?;
            self.ensure_guidance_state()?;
            repaired.push("derived-views");
            repaired.push("sqlite-index");
            repaired.push("codex-instructions");
            repaired.push("guidance-state");
        }
        Ok(json!({ "workspace": self.root, "ok": true, "repair": repair, "repaired": repaired }))
    }

    pub fn fsck(&self) -> Result<Value> {
        let current_hash = self.current_ref()?;
        if !is_sha_ref(&current_hash) {
            return Err(integrity("Current ME ref is not a valid sha256 reference"));
        }
        let current = self.load_current()?;
        let mut checked = BTreeSet::new();
        for entry in WalkDir::new(self.objects_dir())
            .follow_links(false)
            .into_iter()
            .filter_map(std::result::Result::ok)
            .filter(|entry| entry.file_type().is_file())
        {
            let path = entry.path();
            let mut bytes = fs::read(path).map_err(|err| MeError::Internal(err.into()))?;
            if bytes.ends_with(b"\n") {
                bytes.pop();
            }
            let digest = hex_digest(&bytes);
            if path.file_name().and_then(|name| name.to_str()) != Some(digest.as_str()) {
                return Err(integrity(format!(
                    "Object filename does not match hash: {}",
                    path.display()
                )));
            }
            checked.insert(sha_ref(&digest));
        }
        self.read_object::<MeSnapshotPayload>(&current_hash, "me-snapshot")?;
        self.read_object::<MeTreePayload>(&current.snapshot.tree, "me-tree")?;
        for hash in current
            .tree
            .thoughts
            .values()
            .chain(current.tree.cognitions.values())
            .chain(current.tree.decisions.values())
        {
            if !checked.contains(hash) {
                return Err(integrity(format!(
                    "ME Tree references missing object {hash}"
                )));
            }
        }
        Ok(json!({
            "workspace": self.root,
            "ok": true,
            "currentSnapshot": current_hash,
            "objectsChecked": checked.len()
        }))
    }

    pub fn codex_sync(&self) -> Result<Value> {
        self.sync_workspace_docs()?;
        Ok(json!({
            "workspace": self.root,
            "synced": [
                "AGENTS.md",
                ".agents/skills/me/SKILL.md",
                ".agents/skills/me/references/mental-model.md",
                ".agents/skills/me/references/mutation-boundary.md",
                ".agents/skills/me/references/read-context.md",
                ".agents/skills/me/references/references-and-procedures.md",
                ".agents/skills/me/references/cli-contract.md",
                ".agents/skills/me/agents/openai.yaml"
            ]
        }))
    }

    pub fn thought_capture(&self, file: impl AsRef<Path>, kind: &str) -> Result<Value> {
        if !thought_kind_allowed(kind) {
            return Err(invalid(format!(
                "Unsupported Thought kind '{kind}'. Supported kinds: {}",
                THOUGHT_KINDS.join(", ")
            )));
        }
        let body =
            fs::read_to_string(file.as_ref()).map_err(|err| MeError::Internal(err.into()))?;
        self.thought_capture_body(body, kind)
    }

    pub fn thought_capture_body(&self, body: String, kind: &str) -> Result<Value> {
        if !thought_kind_allowed(kind) {
            return Err(invalid(format!(
                "Unsupported Thought kind '{kind}'. Supported kinds: {}",
                THOUGHT_KINDS.join(", ")
            )));
        }
        self.with_lock(|| {
            let current = self.load_current()?;
            let duplicate_of = self.find_exact_thought_duplicate(&current.tree, kind, &body)?;
            let payload = ThoughtPayload {
                thought_id: new_id("thought"),
                kind: kind.to_string(),
                body_text: markdown_to_text(&body),
                body_markdown: body,
                origin: Origin::local_input(),
                captured_at: now_rfc3339()?,
                captured_by: self.config()?.default_actor,
            };
            let thought_hash = self.write_object("thought", &payload)?;
            let mut tree = current.tree.clone();
            tree.thoughts
                .insert(payload.thought_id.clone(), thought_hash.clone());
            tree.thought_states
                .insert(payload.thought_id.clone(), "pending".to_string());
            let snapshot_hash = self.commit_tree(
                &current,
                tree,
                "capture-thought",
                "local-user",
                format!("Capture Thought {}", payload.thought_id),
            )?;
            let mut warnings = Vec::new();
            if let Some(hash) = duplicate_of {
                warnings.push(format!("exact duplicate of {hash}"));
            }
            Ok(json!({
                "statusLabel": "THOUGHT CAPTURED -- PENDING, NOT IN ME",
                "thoughtId": payload.thought_id,
                "thought": thought_hash,
                "bodyMarkdown": payload.body_markdown,
                "state": "pending",
                "snapshot": snapshot_hash.clone(),
                "renderedMarkdown": thought_capture_markdown(&payload.body_markdown),
                "next": {
                    "question": "Keep it in ME?",
                    "choices": [
                        "Keep in ME",
                        "Keep as thought only",
                        "Dismiss"
                    ]
                },
                "technical": {
                    "currentSnapshot": snapshot_hash
                },
                "warnings": warnings
            }))
        })
    }

    pub fn thought_list(&self, state: Option<String>) -> Result<Value> {
        let current = self.load_current()?;
        let mut thoughts = Vec::new();
        for (thought_id, hash) in &current.tree.thoughts {
            let thought = self.read_object::<ThoughtPayload>(hash, "thought")?;
            let thought_state = current
                .tree
                .thought_states
                .get(thought_id)
                .cloned()
                .unwrap_or_else(|| "pending".to_string());
            if state
                .as_deref()
                .is_some_and(|filter| filter != thought_state)
            {
                continue;
            }
            thoughts.push(json!({
                "thoughtId": thought_id,
                "thought": hash,
                "kind": thought.payload.kind,
                "state": thought_state,
                "statusLabel": thought_status_label(&thought_state)
            }));
        }
        Ok(json!({ "thoughts": thoughts, "currentSnapshot": current.snapshot_hash }))
    }

    pub fn thought_show(&self, thought_id_or_hash: &str) -> Result<Value> {
        let current = self.load_current()?;
        let (thought_id, hash, thought) =
            self.resolve_thought(&current.tree, thought_id_or_hash)?;
        let state = current
            .tree
            .thought_states
            .get(&thought_id)
            .cloned()
            .unwrap_or_else(|| "pending".to_string());
        Ok(json!({
            "statusLabel": thought_status_label(&state),
            "thoughtId": thought_id,
            "thought": hash,
            "state": state,
            "payload": thought.payload
        }))
    }

    pub fn thought_relate(&self, thought_id_or_hash: &str, limit: usize) -> Result<Value> {
        let current = self.load_current()?;
        let (thought_id, thought_hash, thought) =
            self.resolve_thought(&current.tree, thought_id_or_hash)?;
        let matches = self.match_thought(&current.tree, &thought.payload.body_markdown, limit)?;
        Ok(json!({
            "thoughtId": thought_id,
            "thought": thought_hash,
            "currentSnapshot": current.snapshot_hash,
            "matches": self.related_from_matches(&matches, &thought.payload.body_text)?
        }))
    }

    pub fn thought_context(&self, thought_id_or_hash: &str, limit: usize) -> Result<Value> {
        let current = self.load_current()?;
        let (thought_id, thought_hash, thought) =
            self.resolve_thought(&current.tree, thought_id_or_hash)?;
        let matches = self.match_thought(&current.tree, &thought.payload.body_markdown, limit)?;
        let related = self.related_from_matches(&matches, &thought.payload.body_text)?;
        Ok(json!({
            "thought": {
                "thoughtId": thought_id,
                "hash": thought_hash,
                "bodyMarkdown": thought.payload.body_markdown,
                "kind": thought.payload.kind,
                "statusLabel": "THOUGHT CAPTURED -- PENDING, NOT IN ME"
            },
            "currentSnapshot": current.snapshot_hash,
            "optionalSimilarCognitions": related,
            "suggestedEffect": {
                "operation": "add-cognition",
                "bodyMarkdown": thought.payload.body_markdown,
                "existingCognitionsChanged": 0,
                "statusLabel": "PENDING -- NOT IN ME"
            },
            "directCommand": {
                "command": "me cognition add",
                "thought": thought_id,
                "decisionTemplate": { "action": "add-cognition", "approved": true },
                "approvalRequired": "Set approved=true only after the user explicitly approves keeping the captured thought."
            },
            "note": "Similarity is derived retrieval only. ME Core will not create a global relationship."
        }))
    }

    pub fn validate_proposal_file(&self, file: impl AsRef<Path>) -> Result<Value> {
        let _ = file.as_ref();
        Ok(legacy_command_message("proposal validate"))
    }

    pub fn save_proposal_file(&self, file: impl AsRef<Path>) -> Result<Value> {
        let _ = file.as_ref();
        Ok(legacy_command_message("proposal save"))
    }

    pub fn show_proposal(&self, proposal_id_or_hash: &str) -> Result<Value> {
        let _ = proposal_id_or_hash;
        Ok(legacy_command_message("proposal show"))
    }

    pub fn list_proposals(&self, status: Option<String>) -> Result<Value> {
        let _ = status;
        Ok(legacy_command_message("proposal list"))
    }

    pub fn review(&self, proposal_id_or_hash: &str, format: &str) -> Result<Value> {
        let _ = (proposal_id_or_hash, format);
        Ok(legacy_command_message("review"))
    }

    pub fn decide(
        &self,
        proposal_id_or_hash: &str,
        decision_file: impl AsRef<Path>,
    ) -> Result<Value> {
        let _ = (proposal_id_or_hash, decision_file.as_ref());
        Ok(legacy_command_message("decide"))
    }

    pub fn reject_or_defer(
        &self,
        proposal_id_or_hash: &str,
        action: &str,
        note: Option<String>,
    ) -> Result<Value> {
        let _ = (proposal_id_or_hash, action, note);
        Ok(legacy_command_message("reject/defer"))
    }

    pub fn cognition_list(&self, state: Option<String>) -> Result<Value> {
        let current = self.load_current()?;
        let mut cognitions = self.current_cognition_summaries(&current.tree)?;
        if let Some(state) = state {
            cognitions.retain(|item| item["state"] == state);
        }
        Ok(json!({ "cognitions": cognitions, "currentSnapshot": current.snapshot_hash }))
    }

    pub fn cognition_show(&self, cognition_id_or_hash: &str) -> Result<Value> {
        let current = self.load_current()?;
        let (cognition_id, hash, cognition) =
            self.resolve_cognition(&current.tree, cognition_id_or_hash)?;
        let state = current
            .tree
            .cognition_states
            .get(&cognition_id)
            .cloned()
            .unwrap_or_else(|| "active".to_string());
        Ok(json!({
            "statusLabel": "COGNITION -- user-authorized",
            "cognitionId": cognition_id,
            "cognition": hash,
            "bodyMarkdown": cognition.payload.body_markdown,
            "displayTitle": cognition.payload.display_title,
            "state": state,
            "fromThought": cognition.payload.origin_thought,
            "addedAt": cognition.payload.added_at,
            "possiblyRelevant": self.retrieval_neighbors_for_cognition(&hash)?,
            "usedBy": self.runs_using_cognition(&current.tree, &hash)?,
            "payload": cognition.payload
        }))
    }

    pub fn cognition_history(&self, cognition_id_or_hash: &str) -> Result<Value> {
        let current = self.load_current()?;
        let (cognition_id, hash, cognition) =
            self.resolve_cognition(&current.tree, cognition_id_or_hash)?;
        let origin_thought_id =
            self.thought_id_for_hash(&current.tree, &cognition.payload.origin_thought);
        let adding_decision = self
            .read_object::<DecisionPayload>(&cognition.payload.added_by_decision, "decision")
            .ok()
            .map(|decision| {
                json!({
                    "decision": cognition.payload.added_by_decision,
                    "payload": decision.payload
                })
            });
        let mut appearances = Vec::new();
        let mut state_changes = Vec::new();
        let mut previous_state: Option<String> = None;
        for snapshot in self.snapshot_chain(&current.snapshot_hash)? {
            let Some(snapshot_hash) = snapshot["meSnapshot"].as_str() else {
                continue;
            };
            let tree = self.snapshot_tree(snapshot_hash)?;
            if tree.cognitions.get(&cognition_id) != Some(&hash) {
                continue;
            }
            let state = tree
                .cognition_states
                .get(&cognition_id)
                .cloned()
                .unwrap_or_else(|| "active".to_string());
            if previous_state.as_deref() != Some(state.as_str()) {
                state_changes.push(json!({
                    "snapshot": snapshot_hash,
                    "state": state,
                    "operation": snapshot["operation"],
                    "createdAt": snapshot["createdAt"]
                }));
            }
            previous_state = Some(state.clone());
            appearances.push(json!({
                "snapshot": snapshot_hash,
                "state": state,
                "operation": snapshot["operation"],
                "createdAt": snapshot["createdAt"]
            }));
        }
        Ok(json!({
            "cognitionId": cognition_id,
            "cognition": hash,
            "originThoughtId": origin_thought_id,
            "originThought": cognition.payload.origin_thought,
            "addedByDecision": adding_decision,
            "stateChanges": state_changes,
            "snapshotAppearances": appearances,
            "currentSnapshot": current.snapshot_hash
        }))
    }

    pub fn cognition_add(
        &self,
        thought_id_or_hash: &str,
        decision_file: impl AsRef<Path>,
    ) -> Result<Value> {
        let raw = fs::read_to_string(decision_file.as_ref())
            .map_err(|err| MeError::Internal(err.into()))?;
        let value = parse_decision_value(&raw)?;
        self.cognition_add_value(thought_id_or_hash, value)
    }

    pub fn cognition_add_value(&self, thought_id_or_hash: &str, value: Value) -> Result<Value> {
        let action = value
            .get("action")
            .and_then(Value::as_str)
            .or_else(|| value.get("operation").and_then(Value::as_str))
            .unwrap_or("add-cognition");
        if action != "add-cognition" {
            return Err(invalid(format!(
                "cognition add requires add-cognition Decision, got {action}"
            )));
        }
        if !has_explicit_keep_approval(&value) {
            return Err(invalid(
                "cognition add requires explicit keep approval: capture the thought first, show it as THOUGHT, ask whether to keep it, then pass a Decision with approved=true only after the user approves",
            ));
        }
        self.with_lock(|| {
            let current = self.load_current()?;
            let first_cognition = current.tree.cognitions.is_empty();
            let active_after_count = current
                .tree
                .cognition_states
                .values()
                .filter(|state| state.as_str() == "active")
                .count()
                + 1;
            validate_base_snapshot(&value, &current.snapshot_hash)?;
            let (thought_id, thought_hash, thought) =
                self.resolve_thought(&current.tree, thought_id_or_hash)?;
            if let Some(decision_thought) = value.get("thought").and_then(Value::as_str) {
                if decision_thought != thought_hash && decision_thought != thought_id {
                    return Err(invalid(format!(
                        "Decision thought {decision_thought} does not match {thought_id}"
                    )));
                }
            }
            let state = current
                .tree
                .thought_states
                .get(&thought_id)
                .map(String::as_str)
                .unwrap_or("pending");
            if state == "added" {
                return Err(invalid(format!(
                    "Thought already added to ME: {thought_id}"
                )));
            }
            let actor = value
                .get("actor")
                .and_then(Value::as_str)
                .map(str::to_string)
                .unwrap_or(self.config()?.default_actor);
            let final_body = value
                .get("finalBodyMarkdown")
                .and_then(Value::as_str)
                .map(str::to_string)
                .unwrap_or_else(|| thought.payload.body_markdown.clone());
            let decision = DecisionPayload {
                decision_id: new_id("decision"),
                base_snapshot: current.snapshot_hash.clone(),
                action: "add-cognition".to_string(),
                actor: actor.clone(),
                thought: Some(thought_hash.clone()),
                final_body_markdown: Some(final_body.clone()),
                note_markdown: value
                    .get("noteMarkdown")
                    .and_then(Value::as_str)
                    .map(str::to_string),
                decided_at: value
                    .get("decidedAt")
                    .and_then(Value::as_str)
                    .map(str::to_string)
                    .unwrap_or(now_rfc3339()?),
            };
            let decision_hash = self.write_object("decision", &decision)?;
            let cognition = CognitionPayload {
                cognition_id: new_id("cognition"),
                body_markdown: final_body.clone(),
                body_text: markdown_to_text(&final_body),
                display_title: value
                    .get("displayTitle")
                    .and_then(Value::as_str)
                    .map(str::to_string),
                origin_thought: thought_hash.clone(),
                added_by_decision: decision_hash.clone(),
                added_at: now_rfc3339()?,
            };
            let cognition_hash = self.write_object("cognition", &cognition)?;
            let mut tree = current.tree.clone();
            tree.decisions
                .insert(decision.decision_id.clone(), decision_hash.clone());
            tree.cognitions
                .insert(cognition.cognition_id.clone(), cognition_hash.clone());
            tree.cognition_states
                .insert(cognition.cognition_id.clone(), "active".to_string());
            tree.thought_states.insert(thought_id, "added".to_string());
            let snapshot = self.commit_tree(
                &current,
                tree,
                "add-cognition",
                &actor,
                "Add Thought as Cognition".to_string(),
            )?;
            let topic = infer_topic_phrase(&final_body).unwrap_or_else(|| "this topic".to_string());
            let mut guidance_state = self.read_guidance_state()?;
            let mut next_guidance = if first_cognition {
                guidance_state.first_cognition_guide_shown = true;
                json!({
                    "kind": "first-cognition",
                    "mentalModel": "In ME, a thought you choose to keep is called a cognition.",
                    "useExamples": [
                        format!("What do I have in ME about {topic}?"),
                        "Draft a short statement using ME."
                    ],
                    "addExample": "Add this thought to ME:"
                })
            } else {
                json!({
                    "kind": "later-cognition",
                    "message": "Add another thought, or ask Codex to use what you have kept."
                })
            };
            let mut rendered_markdown = if first_cognition {
                first_cognition_markdown(&final_body, &topic)
            } else {
                later_cognition_markdown(&final_body)
            };
            if !first_cognition
                && active_after_count == 2
                && !guidance_state.two_cognition_guide_shown
            {
                guidance_state.two_cognition_guide_shown = true;
                let rendered = two_cognition_guidance_markdown(&topic);
                next_guidance = json!({
                    "kind": "two-cognitions",
                    "message": "ME now contains more than one cognition.",
                    "useExamples": [
                        format!("Compare what I have in ME about {topic}."),
                        format!("Find tension in ME about {topic}.")
                    ],
                    "renderedMarkdown": rendered
                });
                rendered_markdown.push_str("\n\n");
                rendered_markdown.push_str(&rendered);
            } else if !first_cognition
                && active_after_count == 5
                && !guidance_state.five_cognition_guide_shown
            {
                guidance_state.five_cognition_guide_shown = true;
                let rendered = five_cognition_guidance_markdown();
                next_guidance = json!({
                    "kind": "five-cognitions",
                    "message": "ME now has enough material to explore broader patterns.",
                    "useExamples": [
                        "What themes recur in ME?",
                        "Draft a longer piece using ME."
                    ],
                    "renderedMarkdown": rendered
                });
                rendered_markdown.push_str("\n\n");
                rendered_markdown.push_str(rendered);
            }
            self.write_guidance_state(&guidance_state)?;
            let mut result = json!({
                "statusLabel": "KEPT IN ME",
                "decisionId": decision.decision_id,
                "decision": decision_hash,
                "cognitionId": cognition.cognition_id,
                "cognition": cognition_hash,
                "bodyMarkdown": final_body,
                "snapshot": snapshot.clone(),
                "cognitionAdded": true,
                "cognitionsAdded": 1,
                "existingCognitionsChanged": 0,
                "firstCognition": first_cognition,
                "renderedMarkdown": rendered_markdown,
                "nextGuidance": next_guidance,
                "technical": {
                    "existingCognitionsChanged": 0,
                    "currentSnapshot": snapshot
                }
            });
            if first_cognition {
                result["next"] = json!({
                    "message": "In ME, a thought you choose to keep is called a cognition.",
                    "examples": [
                        format!("What do I have in ME about {topic}?"),
                        "Draft a short statement using ME.",
                        "Add this thought to ME:"
                    ]
                });
            }
            Ok(result)
        })
    }

    pub fn parse_decision_input(raw: &str) -> Result<Value> {
        parse_decision_value(raw)
    }

    pub fn cognition_retire(
        &self,
        cognition_id_or_hash: &str,
        decision_file: impl AsRef<Path>,
    ) -> Result<Value> {
        self.set_cognition_state(
            cognition_id_or_hash,
            decision_file,
            "retired",
            "retire-cognition",
        )
    }

    pub fn cognition_reactivate(
        &self,
        cognition_id_or_hash: &str,
        decision_file: impl AsRef<Path>,
    ) -> Result<Value> {
        self.set_cognition_state(
            cognition_id_or_hash,
            decision_file,
            "active",
            "reactivate-cognition",
        )
    }

    pub fn cognition_synthesize(&self, spec_file: impl AsRef<Path>) -> Result<Value> {
        let _ = spec_file.as_ref();
        Ok(legacy_command_message("cognition synthesize"))
    }

    pub fn read(&self, about: &str, limit: usize) -> Result<Value> {
        let current = self.load_current()?;
        let matches = self.match_thought(&current.tree, about, limit)?;
        let mut cognitions = Vec::new();
        for matched in matches {
            let (_, _, cognition) = self.resolve_cognition(&current.tree, &matched.cognition)?;
            cognitions.push(json!({
                "cognitionId": matched.cognition_id,
                "cognition": matched.cognition,
                "bodyMarkdown": cognition.payload.body_markdown,
                "displayTitle": cognition.payload.display_title,
                "score": matched.score,
                "matchedTerms": matched.matched_terms,
                "selectionStatus": "derived"
            }));
        }
        Ok(json!({
            "statusLabel": "READING -- temporary assembly",
            "about": about,
            "currentSnapshot": current.snapshot_hash,
            "cognitions": cognitions,
            "note": "Inspect ME preserves contradictions and does not synthesize a single position unless asked."
        }))
    }

    pub fn search(&self, query: &str, limit: usize, state: Option<String>) -> Result<Value> {
        let current = self.load_current()?;
        let state_filter = state.as_deref().or(Some("active"));
        let matches = self.match_cognitions(&current.tree, query, limit, state_filter)?;
        let mut cognitions = Vec::new();
        for matched in matches {
            let (_, _, cognition) = self.resolve_cognition(&current.tree, &matched.cognition)?;
            let cognition_state = current
                .tree
                .cognition_states
                .get(&matched.cognition_id)
                .cloned()
                .unwrap_or_else(|| "active".to_string());
            cognitions.push(json!({
                "cognitionId": matched.cognition_id,
                "objectHash": matched.cognition,
                "bodyMarkdown": cognition.payload.body_markdown,
                "displayTitle": cognition.payload.display_title,
                "state": cognition_state,
                "score": matched.score,
                "matchedTerms": matched.matched_terms,
                "originThoughtId": self.thought_id_for_hash(&current.tree, &cognition.payload.origin_thought),
                "originThought": cognition.payload.origin_thought,
                "addedAt": cognition.payload.added_at
            }));
        }
        Ok(json!({
            "baseSnapshot": current.snapshot_hash,
            "query": query,
            "limit": limit,
            "state": state.unwrap_or_else(|| "active".to_string()),
            "cognitions": cognitions
        }))
    }

    pub fn context(&self, task_file: impl AsRef<Path>, limit: usize) -> Result<Value> {
        let task =
            fs::read_to_string(task_file.as_ref()).map_err(|err| MeError::Internal(err.into()))?;
        self.context_body(task, limit)
    }

    pub fn context_body(&self, task: String, limit: usize) -> Result<Value> {
        let current = self.load_current()?;
        let matches = self.match_cognitions(&current.tree, &task, limit, Some("active"))?;
        let mut selected = Vec::new();
        for matched in matches {
            let (_, _, cognition) = self.resolve_cognition(&current.tree, &matched.cognition)?;
            selected.push(json!({
                "cognitionId": matched.cognition_id,
                "objectHash": matched.cognition,
                "bodyMarkdown": cognition.payload.body_markdown,
                "displayTitle": cognition.payload.display_title,
                "score": matched.score,
                "matchedTerms": matched.matched_terms,
                "selectionReason": if matched.matched_terms.is_empty() {
                    "Selected by local lexical retrieval".to_string()
                } else {
                    format!("Lexical match: {}", matched.matched_terms.join(", "))
                }
            }));
        }
        let mut guidance_state = self.read_guidance_state()?;
        let guidance = if !guidance_state.first_read_guide_shown {
            guidance_state.first_read_guide_shown = true;
            guidance_state.feedback_loop_guide_shown = true;
            self.write_guidance_state(&guidance_state)?;
            json!({
                "kind": "first-read",
                "renderedMarkdown": first_read_guidance_markdown(),
                "message": "ME was read, not changed.",
                "feedbackPrompt": "This is my thought. Add it to ME."
            })
        } else {
            Value::Null
        };
        Ok(json!({
            "baseSnapshot": current.snapshot_hash,
            "taskMarkdown": task,
            "selectedCognitions": selected,
            "coverage": {
                "selectedCount": selected.len(),
                "limit": limit
            },
            "cognitionLibraryChanged": false,
            "guidance": guidance
        }))
    }

    pub fn association_infer(&self, _cognition: Option<String>) -> Result<Value> {
        Ok(association_removed_message())
    }

    pub fn association_list(&self, _kind: Option<String>) -> Result<Value> {
        Ok(association_removed_message())
    }

    pub fn association_confirm(&self, _spec_file: impl AsRef<Path>) -> Result<Value> {
        Ok(association_removed_message())
    }

    pub fn association_remove(
        &self,
        _association_id: &str,
        _decision_file: impl AsRef<Path>,
    ) -> Result<Value> {
        Ok(association_removed_message())
    }

    pub fn app_list(&self) -> Result<Value> {
        Ok(legacy_command_message("app list"))
    }

    pub fn app_show(&self, app_id: &str) -> Result<Value> {
        let _ = app_id;
        Ok(legacy_command_message("app show"))
    }

    pub fn app_validate(&self, app_directory: impl AsRef<Path>) -> Result<Value> {
        let _ = app_directory.as_ref();
        Ok(legacy_command_message("app validate"))
    }

    pub fn app_install(&self, app_directory: impl AsRef<Path>) -> Result<Value> {
        let _ = app_directory.as_ref();
        Ok(legacy_command_message("app install"))
    }

    pub fn app_run(
        &self,
        app_id: &str,
        task_file: impl AsRef<Path>,
        context_only: bool,
    ) -> Result<Value> {
        let _ = (app_id, task_file.as_ref(), context_only);
        Ok(legacy_command_message("app run"))
    }

    pub fn app_prepare(&self, app_id: &str, task_file: impl AsRef<Path>) -> Result<Value> {
        let _ = (app_id, task_file.as_ref());
        Ok(legacy_command_message("app prepare"))
    }

    pub fn app_analyze(
        &self,
        app_id: &str,
        context_file: impl AsRef<Path>,
        analysis_file: impl AsRef<Path>,
    ) -> Result<Value> {
        let _ = (app_id, context_file.as_ref(), analysis_file.as_ref());
        Ok(legacy_command_message("app analyze"))
    }

    pub fn app_resolve(
        &self,
        run_id_or_hash: &str,
        decision_file: impl AsRef<Path>,
        scope: &str,
    ) -> Result<Value> {
        let _ = (run_id_or_hash, decision_file.as_ref(), scope);
        Ok(legacy_command_message("app resolve"))
    }

    pub fn app_save_run(&self, file: impl AsRef<Path>) -> Result<Value> {
        let _ = file.as_ref();
        Ok(legacy_command_message("app save-run"))
    }

    pub fn run_list(&self, app_id: Option<String>) -> Result<Value> {
        let _ = app_id;
        Ok(legacy_command_message("run list"))
    }

    pub fn run_show(&self, run_id_or_hash: &str, format: &str) -> Result<Value> {
        let _ = (run_id_or_hash, format);
        Ok(legacy_command_message("run show"))
    }

    pub fn history(&self) -> Result<Value> {
        let current = self.load_current()?;
        Ok(json!({
            "currentSnapshot": current.snapshot_hash,
            "snapshots": self.snapshot_chain(&current.snapshot_hash)?
        }))
    }

    pub fn diff(&self, snapshot_a: &str, snapshot_b: &str, format: &str) -> Result<Value> {
        let a = self.snapshot_tree(snapshot_a)?;
        let b = self.snapshot_tree(snapshot_b)?;
        let changed_cognitions = changed_keys(&a.cognitions, &b.cognitions);
        let added_thoughts: Vec<_> = b
            .thoughts
            .keys()
            .filter(|key| !a.thoughts.contains_key(*key))
            .cloned()
            .collect();
        let text = format!(
            "Thoughts added: {}\nCognitions changed: {}",
            added_thoughts.len(),
            changed_cognitions.len()
        );
        Ok(json!({
            "format": format,
            "snapshotA": snapshot_a,
            "snapshotB": snapshot_b,
            "text": text,
            "addedThoughts": added_thoughts,
            "changedCognitions": changed_cognitions
        }))
    }

    pub fn snapshot_list(&self) -> Result<Value> {
        let current = self.current_ref()?;
        Ok(json!({ "currentSnapshot": current, "snapshots": self.snapshot_chain(&current)? }))
    }

    pub fn snapshot_show(&self, snapshot_id: &str) -> Result<Value> {
        let snapshot = self.read_object::<MeSnapshotPayload>(snapshot_id, "me-snapshot")?;
        let tree = self.read_object::<MeTreePayload>(&snapshot.payload.tree, "me-tree")?;
        Ok(
            json!({ "meSnapshot": snapshot_id, "payload": snapshot.payload, "meTree": tree.payload }),
        )
    }

    pub fn snapshot_restore(
        &self,
        snapshot_id: &str,
        decision_file: impl AsRef<Path>,
    ) -> Result<Value> {
        let raw = fs::read_to_string(decision_file.as_ref())
            .map_err(|err| MeError::Internal(err.into()))?;
        let value: Value = if raw.trim().is_empty() {
            json!({})
        } else {
            serde_json::from_str(&raw).map_err(|err| MeError::Internal(err.into()))?
        };
        let action = value
            .get("action")
            .and_then(Value::as_str)
            .or_else(|| value.get("operation").and_then(Value::as_str))
            .unwrap_or("restore-snapshot");
        if action != "restore-snapshot" {
            return Err(invalid(format!(
                "snapshot restore requires restore-snapshot Decision, got {action}"
            )));
        }
        self.with_lock(|| {
            let current = self.load_current()?;
            validate_base_snapshot(&value, &current.snapshot_hash)?;
            let actor = value
                .get("actor")
                .and_then(Value::as_str)
                .map(str::to_string)
                .unwrap_or(self.config()?.default_actor);
            let decision = DecisionPayload {
                decision_id: new_id("decision"),
                base_snapshot: current.snapshot_hash.clone(),
                action: "restore-snapshot".to_string(),
                actor: actor.clone(),
                thought: None,
                final_body_markdown: None,
                note_markdown: value
                    .get("noteMarkdown")
                    .and_then(Value::as_str)
                    .map(str::to_string),
                decided_at: value
                    .get("decidedAt")
                    .and_then(Value::as_str)
                    .map(str::to_string)
                    .unwrap_or(now_rfc3339()?),
            };
            let decision_hash = self.write_object("decision", &decision)?;
            let mut restored_tree = self.snapshot_tree(snapshot_id)?;
            restored_tree
                .decisions
                .insert(decision.decision_id.clone(), decision_hash.clone());
            let snapshot_hash = self.commit_tree(
                &current,
                restored_tree,
                "restore-snapshot",
                &actor,
                format!("Restore Snapshot {snapshot_id}"),
            )?;
            Ok(json!({
                "restoredFrom": snapshot_id,
                "decisionId": decision.decision_id,
                "decision": decision_hash,
                "snapshot": snapshot_hash
            }))
        })
    }

    pub fn index_rebuild(&self) -> Result<Value> {
        self.rebuild_index()?;
        Ok(
            json!({ "workspace": self.root, "index": self.root.join(".me/index.sqlite"), "rebuilt": true }),
        )
    }

    pub fn bundle_create(&self, output: impl AsRef<Path>) -> Result<Value> {
        let output = output.as_ref();
        let objects = self.object_hashes()?;
        let manifest = json!({
            "schemaVersion": SCHEMA_VERSION,
            "createdAt": now_rfc3339()?,
            "current": self.current_ref()?,
            "objects": objects
        });
        let file = File::create(output).map_err(|err| MeError::Internal(err.into()))?;
        let mut builder = Builder::new(file);
        append_bytes_to_tar(
            &mut builder,
            "manifest.json",
            serde_json::to_vec_pretty(&manifest).unwrap(),
        )?;
        append_file_to_tar(&mut builder, "me.toml", &self.root.join("me.toml"))?;
        append_file_to_tar(
            &mut builder,
            "refs/current",
            &self.root.join(".me/refs/current"),
        )?;
        let journal = self.root.join(".me/journal/transitions.ndjson");
        if journal.exists() {
            append_file_to_tar(&mut builder, "journal/transitions.ndjson", &journal)?;
        } else {
            append_bytes_to_tar(&mut builder, "journal/transitions.ndjson", Vec::new())?;
        }
        for object in self.object_file_paths()? {
            let rel = object
                .strip_prefix(self.objects_dir())
                .map_err(|err| MeError::Internal(err.into()))?;
            append_file_to_tar(&mut builder, PathBuf::from("objects").join(rel), &object)?;
        }
        builder
            .finish()
            .map_err(|err| MeError::Internal(err.into()))?;
        Ok(json!({
            "bundle": output,
            "currentSnapshot": self.current_ref()?,
            "objects": manifest["objects"].as_array().map_or(0, Vec::len)
        }))
    }

    pub fn bundle_verify(&self, file: impl AsRef<Path>) -> Result<Value> {
        Self::bundle_verify_file(file)
    }

    pub fn bundle_verify_file(file: impl AsRef<Path>) -> Result<Value> {
        let verified = verify_bundle(file.as_ref())?;
        Ok(json!({
            "bundle": file.as_ref(),
            "valid": true,
            "currentSnapshot": verified.current,
            "objects": verified.objects
        }))
    }

    pub fn bundle_restore(file: impl AsRef<Path>, target: impl AsRef<Path>) -> Result<Value> {
        let file = file.as_ref();
        let target = target.as_ref();
        if target.exists()
            && target
                .read_dir()
                .map_err(|err| MeError::Internal(err.into()))?
                .next()
                .is_some()
        {
            return Err(invalid(format!(
                "Target directory is not empty: {}",
                target.display()
            )));
        }
        let verified = verify_bundle(file)?;
        fs::create_dir_all(target).map_err(|err| MeError::Internal(err.into()))?;
        let ws = Workspace {
            root: target.to_path_buf(),
        };
        ws.create_layout()?;
        let archive_file = File::open(file).map_err(|err| MeError::Internal(err.into()))?;
        let mut archive = Archive::new(archive_file);
        for entry in archive
            .entries()
            .map_err(|err| MeError::Internal(err.into()))?
        {
            let mut entry = entry.map_err(|err| MeError::Internal(err.into()))?;
            let path = entry
                .path()
                .map_err(|err| MeError::Internal(err.into()))?
                .to_path_buf();
            validate_archive_path(&path)?;
            let dest = bundle_path_to_workspace_path(target, &path)?;
            if let Some(parent) = dest.parent() {
                fs::create_dir_all(parent).map_err(|err| MeError::Internal(err.into()))?;
            }
            entry
                .unpack(&dest)
                .map_err(|err| MeError::Internal(err.into()))?;
        }
        ws.sync_workspace_docs()?;
        ws.regenerate_views()?;
        ws.rebuild_index()?;
        ws.fsck()?;
        Ok(json!({
            "bundle": file,
            "workspace": target,
            "currentSnapshot": verified.current,
            "objects": verified.objects
        }))
    }

    pub fn export_workspace(&self, output: impl AsRef<Path>) -> Result<Value> {
        let current = self.load_current()?;
        let data = json!({
            "currentSnapshot": current.snapshot_hash,
            "meTree": current.tree,
            "snapshots": self.snapshot_chain(&current.snapshot_hash)?
        });
        atomic_write(
            output.as_ref(),
            &serde_json::to_vec_pretty(&data).map_err(|err| MeError::Internal(err.into()))?,
        )?;
        Ok(json!({ "output": output.as_ref(), "format": "json" }))
    }

    pub fn migrate_from_my_model(path: impl AsRef<Path>) -> Result<Value> {
        let root = path.as_ref().to_path_buf();
        if !root.join(".my-model/refs/current").exists() {
            return Err(not_found(format!(
                "Not a My Model workspace: {}",
                root.display()
            )));
        }
        if root.join(".me/refs/current").exists() {
            return Err(invalid(
                "ME workspace already exists; migration will not run twice",
            ));
        }
        let _legacy_lock = lock_file(&root.join(".my-model/lock"), "My Model")?;
        let ws = Workspace { root: root.clone() };
        ws.create_layout()?;
        ws.write_config()?;
        ws.sync_workspace_docs()?;
        let migrated_at = now_rfc3339()?;
        let old_current = fs::read_to_string(root.join(".my-model/refs/current"))
            .map_err(|err| MeError::Internal(err.into()))?
            .trim()
            .to_string();
        let old_snapshot = legacy_read_object(&root, &old_current, "model-snapshot")?;
        let old_tree_hash = old_snapshot["payload"]["tree"]
            .as_str()
            .ok_or_else(|| integrity("Legacy snapshot missing tree"))?;
        let old_tree = legacy_read_object(&root, old_tree_hash, "model-tree")?;
        let mut tree = MeTreePayload::default();
        let migration_decision = DecisionPayload {
            decision_id: new_id("decision"),
            base_snapshot: old_current.clone(),
            action: "add-cognition".to_string(),
            actor: "local-user".to_string(),
            thought: None,
            final_body_markdown: None,
            note_markdown: Some("Migration from My Model v0.2".to_string()),
            decided_at: migrated_at.clone(),
        };
        let migration_decision_hash = ws.write_object("decision", &migration_decision)?;
        tree.decisions.insert(
            migration_decision.decision_id.clone(),
            migration_decision_hash.clone(),
        );
        let mut mappings = Vec::new();
        if let Some(thoughts) = old_tree["payload"]["thoughts"].as_object() {
            for (thought_id, old_hash_value) in thoughts {
                let Some(old_hash) = old_hash_value.as_str() else {
                    continue;
                };
                let old = legacy_read_object(&root, old_hash, "thought")?;
                let body = old["payload"]["bodyMarkdown"]
                    .as_str()
                    .unwrap_or("")
                    .to_string();
                let thought = ThoughtPayload {
                    thought_id: thought_id.clone(),
                    kind: old["payload"]["kind"]
                        .as_str()
                        .unwrap_or("other")
                        .to_string(),
                    body_markdown: body.clone(),
                    body_text: old["payload"]["bodyText"]
                        .as_str()
                        .map(str::to_string)
                        .unwrap_or_else(|| markdown_to_text(&body)),
                    origin: Origin::local_input(),
                    captured_at: old["payload"]["capturedAt"]
                        .as_str()
                        .unwrap_or(&migrated_at)
                        .to_string(),
                    captured_by: old["payload"]["capturedBy"]
                        .as_str()
                        .unwrap_or("local-user")
                        .to_string(),
                };
                let new_hash = ws.write_object("thought", &thought)?;
                tree.thoughts.insert(thought_id.clone(), new_hash.clone());
                tree.thought_states
                    .insert(thought_id.clone(), "pending".to_string());
                mappings.push(json!({ "kind": "thought", "old": old_hash, "new": new_hash }));
            }
        }
        if let Some(cognitions) = old_tree["payload"]["cognitions"].as_object() {
            for (legacy_cognition_id, current_revision_value) in cognitions {
                let Some(current_revision_hash) = current_revision_value.as_str() else {
                    continue;
                };
                let chain = legacy_cognition_chain(&root, current_revision_hash)?;
                for (old_hash, rev) in chain {
                    let revision = rev["revision"].as_u64().unwrap_or(1);
                    let active = old_hash == current_revision_hash;
                    let cognition_id = if active {
                        migrate_id(legacy_cognition_id)
                    } else {
                        format!("{}--r{}", migrate_id(legacy_cognition_id), revision)
                    };
                    let body = rev["formulationMarkdown"]
                        .as_str()
                        .unwrap_or("")
                        .to_string();
                    let origin_thought = rev["derivedFromThoughts"]
                        .as_array()
                        .and_then(|items| items.first())
                        .and_then(Value::as_str)
                        .and_then(|old_thought_hash| {
                            mappings
                                .iter()
                                .find(|item| item["old"] == old_thought_hash)
                                .and_then(|item| item["new"].as_str())
                        })
                        .unwrap_or("")
                        .to_string();
                    let cognition = CognitionPayload {
                        cognition_id: cognition_id.clone(),
                        body_markdown: body.clone(),
                        body_text: rev["formulationText"]
                            .as_str()
                            .map(str::to_string)
                            .unwrap_or_else(|| markdown_to_text(&body)),
                        display_title: rev["title"].as_str().map(str::to_string),
                        origin_thought,
                        added_by_decision: migration_decision_hash.clone(),
                        added_at: rev["createdAt"]
                            .as_str()
                            .unwrap_or(&migrated_at)
                            .to_string(),
                    };
                    let new_hash = ws.write_object("cognition", &cognition)?;
                    tree.cognitions
                        .insert(cognition_id.clone(), new_hash.clone());
                    tree.cognition_states.insert(
                        cognition_id,
                        if active { "active" } else { "retired" }.to_string(),
                    );
                    mappings.push(json!({ "kind": "cognition", "old": old_hash, "new": new_hash, "state": if active { "active" } else { "retired" } }));
                }
            }
        }
        let tree_hash = ws.write_object("me-tree", &tree)?;
        let snapshot = MeSnapshotPayload {
            parent: None,
            tree: tree_hash,
            operation: "migrate-from-my-model".to_string(),
            actor: "local-user".to_string(),
            message: "Migrate My Model v0.2 workspace to ME v0.5".to_string(),
            created_at: migrated_at.clone(),
        };
        let new_current = ws.write_object("me-snapshot", &snapshot)?;
        atomic_write(
            &root.join(".me/refs/current"),
            format!("{new_current}\n").as_bytes(),
        )?;
        ws.append_journal(
            None,
            &new_current,
            "migrate-from-my-model",
            "Migrate My Model v0.2 workspace to ME v0.5",
        )?;
        ws.regenerate_views()?;
        ws.rebuild_index()?;
        ws.fsck()?;
        let manifest = json!({
            "schemaVersion": 1,
            "sourceSystem": "my-model",
            "sourceWorkspaceVersion": 2,
            "targetSystem": "me",
            "targetWorkspaceVersion": 5,
            "migratedAt": migrated_at,
            "oldCurrentSnapshot": old_current,
            "newCurrentSnapshot": new_current,
            "objects": mappings,
            "oldMyModelPreserved": true
        });
        let manifest_path = root.join(".me/migrations/my-model-v2-to-me-v5-manifest.json");
        atomic_write(
            &manifest_path,
            &serde_json::to_vec_pretty(&manifest).map_err(|err| MeError::Internal(err.into()))?,
        )?;
        Ok(json!({
            "workspace": root,
            "manifest": manifest_path,
            "objects": manifest["objects"].as_array().map_or(0, Vec::len),
            "oldCurrentSnapshot": manifest["oldCurrentSnapshot"],
            "newCurrentSnapshot": manifest["newCurrentSnapshot"],
            "oldMyModelPreserved": true
        }))
    }

    pub fn migrate_from_v3(path: impl AsRef<Path>) -> Result<Value> {
        let root = path.as_ref().to_path_buf();
        if !root.join(".me/refs/current").exists() {
            return Err(not_found(format!(
                "Not a ME v3 workspace: {}",
                root.display()
            )));
        }
        let raw_config = fs::read_to_string(root.join("me.toml"))
            .map_err(|err| MeError::Internal(err.into()))?;
        let mut config: me_core::WorkspaceConfig =
            toml::from_str(&raw_config).map_err(|err| MeError::Internal(err.into()))?;
        if config.schema_version == SCHEMA_VERSION {
            return Err(invalid("ME workspace is already schema v5"));
        }
        if config.schema_version != 3 {
            return Err(invalid(format!(
                "migrate --from-v3 requires schemaVersion 3, got {}",
                config.schema_version
            )));
        }
        let _lock = lock_file(&root.join(".me/lock"), "ME v3")?;
        let ws = Workspace { root: root.clone() };
        ws.create_layout()?;
        ws.sync_workspace_docs()?;
        let migrated_at = now_rfc3339()?;
        let old_current = fs::read_to_string(root.join(".me/refs/current"))
            .map_err(|err| MeError::Internal(err.into()))?
            .trim()
            .to_string();
        let old_snapshot = me_object_raw(&root, &old_current, "me-snapshot")?;
        let old_tree_hash = old_snapshot["payload"]["tree"]
            .as_str()
            .ok_or_else(|| integrity("v3 snapshot missing tree"))?;
        let old_tree = me_object_raw(&root, old_tree_hash, "me-tree")?;
        let payload = &old_tree["payload"];
        let mut tree = MeTreePayload::default();
        let mut mappings = Vec::new();
        let mut hash_map: BTreeMap<String, String> = BTreeMap::new();

        let migration_decision = DecisionPayload {
            decision_id: new_id("decision"),
            base_snapshot: old_current.clone(),
            action: "add-cognition".to_string(),
            actor: "local-user".to_string(),
            thought: None,
            final_body_markdown: None,
            note_markdown: Some("Migration from ME v0.3 to ME v0.5".to_string()),
            decided_at: migrated_at.clone(),
        };
        let migration_decision_hash = ws.write_object("decision", &migration_decision)?;
        tree.decisions.insert(
            migration_decision.decision_id.clone(),
            migration_decision_hash.clone(),
        );

        if let Some(thoughts) = payload["thoughts"].as_object() {
            for (thought_id, old_hash_value) in thoughts {
                let Some(old_hash) = old_hash_value.as_str() else {
                    continue;
                };
                let old = me_object_raw(&root, old_hash, "thought")?;
                let body = old["payload"]["bodyMarkdown"]
                    .as_str()
                    .unwrap_or("")
                    .to_string();
                let thought = ThoughtPayload {
                    thought_id: thought_id.clone(),
                    kind: old["payload"]["kind"]
                        .as_str()
                        .unwrap_or("other")
                        .to_string(),
                    body_markdown: body.clone(),
                    body_text: old["payload"]["bodyText"]
                        .as_str()
                        .map(str::to_string)
                        .unwrap_or_else(|| markdown_to_text(&body)),
                    origin: Origin::local_input(),
                    captured_at: old["payload"]["capturedAt"]
                        .as_str()
                        .unwrap_or(&migrated_at)
                        .to_string(),
                    captured_by: old["payload"]["capturedBy"]
                        .as_str()
                        .unwrap_or("local-user")
                        .to_string(),
                };
                let new_hash = ws.write_object("thought", &thought)?;
                tree.thoughts.insert(thought_id.clone(), new_hash.clone());
                let state = payload["thoughtStates"][thought_id]
                    .as_str()
                    .unwrap_or("pending")
                    .to_string();
                tree.thought_states.insert(thought_id.clone(), state);
                hash_map.insert(old_hash.to_string(), new_hash.clone());
                mappings.push(json!({ "kind": "thought", "id": thought_id, "old": old_hash, "new": new_hash }));
            }
        }

        if let Some(decisions) = payload["decisions"].as_object() {
            for (decision_id, old_hash_value) in decisions {
                let Some(old_hash) = old_hash_value.as_str() else {
                    continue;
                };
                let old = me_object_raw(&root, old_hash, "decision")?;
                let old_payload = &old["payload"];
                let decision = DecisionPayload {
                    decision_id: decision_id.clone(),
                    base_snapshot: old_payload["baseSnapshot"]
                        .as_str()
                        .unwrap_or(&old_current)
                        .to_string(),
                    action: old_payload["action"]
                        .as_str()
                        .unwrap_or("add-cognition")
                        .to_string(),
                    actor: old_payload["actor"]
                        .as_str()
                        .unwrap_or("local-user")
                        .to_string(),
                    thought: old_payload["thought"].as_str().map(str::to_string),
                    final_body_markdown: old_payload["finalBodyMarkdown"]
                        .as_str()
                        .map(str::to_string),
                    note_markdown: old_payload["noteMarkdown"].as_str().map(str::to_string),
                    decided_at: old_payload["decidedAt"]
                        .as_str()
                        .unwrap_or(&migrated_at)
                        .to_string(),
                };
                let new_hash = ws.write_object("decision", &decision)?;
                tree.decisions.insert(decision_id.clone(), new_hash.clone());
                hash_map.insert(old_hash.to_string(), new_hash.clone());
                mappings.push(json!({ "kind": "decision", "id": decision_id, "old": old_hash, "new": new_hash }));
            }
        }

        if let Some(cognitions) = payload["cognitions"].as_object() {
            for (cognition_id, old_hash_value) in cognitions {
                let Some(old_hash) = old_hash_value.as_str() else {
                    continue;
                };
                let old = me_object_raw(&root, old_hash, "cognition")?;
                let old_payload = &old["payload"];
                let origin_thought = old_payload["originThought"]
                    .as_str()
                    .and_then(|hash| hash_map.get(hash).cloned())
                    .or_else(|| old_payload["originThought"].as_str().map(str::to_string))
                    .unwrap_or_default();
                let added_by_decision = old_payload["addedByDecision"]
                    .as_str()
                    .and_then(|hash| hash_map.get(hash).cloned())
                    .unwrap_or_else(|| migration_decision_hash.clone());
                let body = old_payload["bodyMarkdown"]
                    .as_str()
                    .unwrap_or("")
                    .to_string();
                let cognition = CognitionPayload {
                    cognition_id: cognition_id.clone(),
                    body_markdown: body.clone(),
                    body_text: old_payload["bodyText"]
                        .as_str()
                        .map(str::to_string)
                        .unwrap_or_else(|| markdown_to_text(&body)),
                    display_title: old_payload["displayTitle"].as_str().map(str::to_string),
                    origin_thought,
                    added_by_decision,
                    added_at: old_payload["addedAt"]
                        .as_str()
                        .unwrap_or(&migrated_at)
                        .to_string(),
                };
                let new_hash = ws.write_object("cognition", &cognition)?;
                tree.cognitions
                    .insert(cognition_id.clone(), new_hash.clone());
                let state = payload["cognitionStates"][cognition_id]
                    .as_str()
                    .unwrap_or("active")
                    .to_string();
                tree.cognition_states.insert(cognition_id.clone(), state);
                hash_map.insert(old_hash.to_string(), new_hash.clone());
                mappings.push(json!({ "kind": "cognition", "id": cognition_id, "old": old_hash, "new": new_hash }));
            }
        }

        if let Some(proposals) = payload["proposals"].as_object() {
            for (proposal_id, old_hash_value) in proposals {
                let Some(old_hash) = old_hash_value.as_str() else {
                    continue;
                };
                let old = me_object_raw(&root, old_hash, "proposal")?;
                let old_payload = &old["payload"];
                let proposal = me_core::ProposalPayload {
                    proposal_id: proposal_id.clone(),
                    kind: old_payload["kind"]
                        .as_str()
                        .unwrap_or("migration-history")
                        .to_string(),
                    base_snapshot: old_payload["baseSnapshot"]
                        .as_str()
                        .unwrap_or(&old_current)
                        .to_string(),
                    inputs: json!({
                        "thought": old_payload["thought"].as_str().and_then(|hash| hash_map.get(hash).cloned())
                    }),
                    recommendation: old_payload["recommendation"].clone(),
                    related_cognitions: Vec::new(),
                    alternatives: old_payload["alternatives"]
                        .as_array()
                        .cloned()
                        .unwrap_or_default(),
                    generated_by: GeneratedBy {
                        host: "migration".to_string(),
                        model: None,
                    },
                    created_at: old_payload["createdAt"]
                        .as_str()
                        .unwrap_or(&migrated_at)
                        .to_string(),
                };
                let new_hash = ws.write_object("proposal", &proposal)?;
                tree.proposals.insert(proposal_id.clone(), new_hash.clone());
                hash_map.insert(old_hash.to_string(), new_hash.clone());
                mappings.push(json!({ "kind": "proposal", "id": proposal_id, "old": old_hash, "new": new_hash }));
            }
        }

        if let Some(apps) = payload["apps"].as_object() {
            for (app_id, old_hash_value) in apps {
                let Some(old_hash) = old_hash_value.as_str() else {
                    continue;
                };
                let old = me_object_raw(&root, old_hash, "app-definition")?;
                let old_payload = &old["payload"];
                let app = AppDefinitionPayload {
                    app_id: app_id.clone(),
                    name: old_payload["name"].as_str().unwrap_or(app_id).to_string(),
                    version: old_payload["version"]
                        .as_str()
                        .unwrap_or("0.2.0")
                        .to_string(),
                    manifest_hash: old_payload["manifestHash"]
                        .as_str()
                        .unwrap_or("")
                        .to_string(),
                    installed_at: old_payload["installedAt"]
                        .as_str()
                        .unwrap_or(&migrated_at)
                        .to_string(),
                };
                let new_hash = ws.write_object("app-definition", &app)?;
                tree.apps.insert(app_id.clone(), new_hash.clone());
                hash_map.insert(old_hash.to_string(), new_hash.clone());
                mappings.push(json!({ "kind": "app-definition", "id": app_id, "old": old_hash, "new": new_hash }));
            }
        }

        if let Some(runs) = payload["appRuns"].as_object() {
            for (run_id, old_hash_value) in runs {
                let Some(old_hash) = old_hash_value.as_str() else {
                    continue;
                };
                let old = me_object_raw(&root, old_hash, "app-run")?;
                let old_payload = &old["payload"];
                let mut selected = Vec::new();
                if let Some(items) = old_payload["selectedCognitions"].as_array() {
                    for item in items {
                        let Some(old_cognition) = item["cognition"].as_str() else {
                            continue;
                        };
                        selected.push(SelectedCognition {
                            cognition: hash_map
                                .get(old_cognition)
                                .cloned()
                                .unwrap_or_else(|| old_cognition.to_string()),
                            cognition_id: item["cognitionId"]
                                .as_str()
                                .unwrap_or("cognition")
                                .to_string(),
                            selection_reason: item["selectionReason"]
                                .as_str()
                                .or_else(|| item["reason"].as_str())
                                .unwrap_or("Migrated v3 App Run selection.")
                                .to_string(),
                        });
                    }
                }
                let conflicts = old_payload["conflicts"]
                    .as_array()
                    .map(|items| {
                        items
                            .iter()
                            .filter_map(Value::as_str)
                            .map(str::to_string)
                            .collect()
                    })
                    .unwrap_or_default();
                let gaps = old_payload["gaps"]
                    .as_array()
                    .map(|items| {
                        items
                            .iter()
                            .filter_map(Value::as_str)
                            .map(str::to_string)
                            .collect()
                    })
                    .unwrap_or_default();
                let findings = if old_payload["confirmedAssociationsUsed"]
                    .as_array()
                    .is_some_and(|items| !items.is_empty())
                    || old_payload["inferredAssociationsUsed"]
                        .as_array()
                        .is_some_and(|items| !items.is_empty())
                {
                    vec![AppFinding {
                        label: "unclear".to_string(),
                        cognitions: selected.iter().map(|item| item.cognition.clone()).collect(),
                        passages: Vec::new(),
                        reason_markdown:
                            "Migrated v3 App Run relation context as task-scoped analysis."
                                .to_string(),
                        app_rule: Some("migration-from-v3".to_string()),
                    }]
                } else {
                    Vec::new()
                };
                let output = AppRunOutput {
                    kind: old_payload["output"]["kind"]
                        .as_str()
                        .unwrap_or("analysis")
                        .to_string(),
                    body_markdown: old_payload["output"]["bodyMarkdown"]
                        .as_str()
                        .unwrap_or("")
                        .to_string(),
                    external_action: old_payload["output"]["externalAction"]
                        .as_bool()
                        .unwrap_or(false),
                };
                let run = AppRunPayload {
                    run_id: run_id.clone(),
                    app_id: old_payload["appId"]
                        .as_str()
                        .unwrap_or("unknown-app")
                        .to_string(),
                    app_version: old_payload["appVersion"]
                        .as_str()
                        .unwrap_or("0.1.0")
                        .to_string(),
                    base_snapshot: old_payload["baseSnapshot"]
                        .as_str()
                        .unwrap_or(&old_current)
                        .to_string(),
                    task_markdown: old_payload["taskMarkdown"]
                        .as_str()
                        .unwrap_or("")
                        .to_string(),
                    selected_cognitions: selected,
                    analysis: AppAnalysis {
                        findings,
                        gaps,
                        conflicts,
                    },
                    resolutions: Vec::new(),
                    app_policies_used: Vec::new(),
                    output,
                    created_at: old_payload["createdAt"]
                        .as_str()
                        .unwrap_or(&migrated_at)
                        .to_string(),
                };
                let new_hash = ws.write_object("app-run", &run)?;
                tree.app_runs.insert(run_id.clone(), new_hash.clone());
                hash_map.insert(old_hash.to_string(), new_hash.clone());
                mappings.push(
                    json!({ "kind": "app-run", "id": run_id, "old": old_hash, "new": new_hash }),
                );
            }
        }

        let mut archived_associations = Vec::new();
        if let Some(associations) = payload["confirmedAssociations"].as_object() {
            for (association_id, old_hash_value) in associations {
                let Some(old_hash) = old_hash_value.as_str() else {
                    continue;
                };
                let old = me_object_raw(&root, old_hash, "association")?;
                let old_payload = &old["payload"];
                archived_associations.push(json!({
                    "associationId": association_id,
                    "oldObjectHash": old_hash,
                    "relation": old_payload["relation"],
                    "fromCognitions": old_payload["fromCognitions"],
                    "toCognitions": old_payload["toCognitions"],
                    "confirmationDecision": old_payload["confirmedByDecision"],
                    "confirmedAt": old_payload["confirmedAt"]
                }));
            }
        }
        let association_archive = json!({
            "schemaVersion": 1,
            "note": "v3 global Associations are preserved for audit but are not active in ME v5.",
            "records": archived_associations
        });
        let association_archive_path = root.join(".me/migrations/v3-associations.json");
        atomic_write(
            &association_archive_path,
            &serde_json::to_vec_pretty(&association_archive)
                .map_err(|err| MeError::Internal(err.into()))?,
        )?;

        tree.proposals.clear();
        tree.apps.clear();
        tree.app_policies.clear();
        tree.app_runs.clear();

        let tree_hash = ws.write_object("me-tree", &tree)?;
        let snapshot = MeSnapshotPayload {
            parent: None,
            tree: tree_hash,
            operation: "migrate-from-v3".to_string(),
            actor: "local-user".to_string(),
            message: "Migrate ME v0.3 workspace to ME v0.5".to_string(),
            created_at: migrated_at.clone(),
        };
        let new_current = ws.write_object("me-snapshot", &snapshot)?;
        atomic_write(
            &root.join(".me/refs/current"),
            format!("{new_current}\n").as_bytes(),
        )?;
        ws.append_journal(
            None,
            &new_current,
            "migrate-from-v3",
            "Migrate ME v0.3 workspace to ME v0.5",
        )?;
        config.schema_version = SCHEMA_VERSION;
        let toml = toml::to_string_pretty(&config).map_err(|err| MeError::Internal(err.into()))?;
        atomic_write(&root.join("me.toml"), toml.as_bytes())?;
        ws.regenerate_views()?;
        ws.rebuild_index()?;
        ws.fsck()?;
        let manifest = json!({
            "schemaVersion": 1,
            "sourceSystem": "me",
            "sourceWorkspaceVersion": 3,
            "targetSystem": "me",
            "targetWorkspaceVersion": 5,
            "migratedAt": migrated_at,
            "oldCurrentSnapshot": old_current,
            "newCurrentSnapshot": new_current,
            "objects": mappings,
            "confirmedAssociationArchive": association_archive_path,
            "confirmedAssociationArchiveCount": association_archive["records"].as_array().map_or(0, Vec::len),
            "oldObjectsPreserved": true
        });
        let manifest_path = root.join(".me/migrations/me-v3-to-me-v5-manifest.json");
        atomic_write(
            &manifest_path,
            &serde_json::to_vec_pretty(&manifest).map_err(|err| MeError::Internal(err.into()))?,
        )?;
        Ok(json!({
            "workspace": root,
            "manifest": manifest_path,
            "associationArchive": association_archive_path,
            "confirmedAssociationArchiveCount": manifest["confirmedAssociationArchiveCount"],
            "oldCurrentSnapshot": manifest["oldCurrentSnapshot"],
            "newCurrentSnapshot": manifest["newCurrentSnapshot"],
            "oldObjectsPreserved": true
        }))
    }

    pub fn migrate_from_v4(path: impl AsRef<Path>) -> Result<Value> {
        let root = path.as_ref().to_path_buf();
        if !root.join(".me/refs/current").exists() {
            return Err(not_found(format!(
                "Not a ME v4 workspace: {}",
                root.display()
            )));
        }
        let raw_config = fs::read_to_string(root.join("me.toml"))
            .map_err(|err| MeError::Internal(err.into()))?;
        let mut config: me_core::WorkspaceConfig =
            toml::from_str(&raw_config).map_err(|err| MeError::Internal(err.into()))?;
        if config.schema_version == SCHEMA_VERSION {
            return Err(invalid("ME workspace is already schema v5"));
        }
        if config.schema_version != 4 {
            return Err(invalid(format!(
                "migrate --from-v4 requires schemaVersion 4, got {}",
                config.schema_version
            )));
        }
        let _lock = lock_file(&root.join(".me/lock"), "ME v4")?;
        let ws = Workspace { root: root.clone() };
        ws.create_layout()?;
        ws.sync_workspace_docs()?;
        let migrated_at = now_rfc3339()?;
        let old_current = fs::read_to_string(root.join(".me/refs/current"))
            .map_err(|err| MeError::Internal(err.into()))?
            .trim()
            .to_string();
        let old_snapshot = me_object_raw(&root, &old_current, "me-snapshot")?;
        let old_tree_hash = old_snapshot["payload"]["tree"]
            .as_str()
            .ok_or_else(|| integrity("v4 snapshot missing tree"))?;
        let old_tree = me_object_raw(&root, old_tree_hash, "me-tree")?;
        let payload = &old_tree["payload"];
        let mut tree = MeTreePayload::default();
        let mut mappings = Vec::new();
        let mut hash_map: BTreeMap<String, String> = BTreeMap::new();
        let fallback_decision = DecisionPayload {
            decision_id: new_id("decision"),
            base_snapshot: old_current.clone(),
            action: "add-cognition".to_string(),
            actor: "local-user".to_string(),
            thought: None,
            final_body_markdown: None,
            note_markdown: Some("Migration fallback Decision from ME v0.4 to ME v0.5".to_string()),
            decided_at: migrated_at.clone(),
        };
        let fallback_decision_hash = ws.write_object("decision", &fallback_decision)?;

        if let Some(thoughts) = payload["thoughts"].as_object() {
            for (thought_id, old_hash_value) in thoughts {
                let Some(old_hash) = old_hash_value.as_str() else {
                    continue;
                };
                let old = me_object_raw(&root, old_hash, "thought")?;
                let old_payload = &old["payload"];
                let body = old_payload["bodyMarkdown"]
                    .as_str()
                    .unwrap_or("")
                    .to_string();
                let origin: Origin = serde_json::from_value(old_payload["origin"].clone())
                    .unwrap_or_else(|_| Origin::local_input());
                let thought = ThoughtPayload {
                    thought_id: thought_id.clone(),
                    kind: old_payload["kind"].as_str().unwrap_or("other").to_string(),
                    body_markdown: body.clone(),
                    body_text: old_payload["bodyText"]
                        .as_str()
                        .map(str::to_string)
                        .unwrap_or_else(|| markdown_to_text(&body)),
                    origin,
                    captured_at: old_payload["capturedAt"]
                        .as_str()
                        .unwrap_or(&migrated_at)
                        .to_string(),
                    captured_by: old_payload["capturedBy"]
                        .as_str()
                        .unwrap_or("local-user")
                        .to_string(),
                };
                let new_hash = ws.write_object("thought", &thought)?;
                tree.thoughts.insert(thought_id.clone(), new_hash.clone());
                let state = payload["thoughtStates"][thought_id]
                    .as_str()
                    .unwrap_or("pending");
                tree.thought_states.insert(
                    thought_id.clone(),
                    if state == "captured" {
                        "pending"
                    } else {
                        state
                    }
                    .to_string(),
                );
                hash_map.insert(old_hash.to_string(), new_hash.clone());
                mappings.push(json!({ "kind": "thought", "id": thought_id, "old": old_hash, "new": new_hash }));
            }
        }

        let mut app_decision_archive = Vec::new();
        if let Some(decisions) = payload["decisions"].as_object() {
            for (decision_id, old_hash_value) in decisions {
                let Some(old_hash) = old_hash_value.as_str() else {
                    continue;
                };
                let old = me_object_raw(&root, old_hash, "decision")?;
                let old_payload = &old["payload"];
                let old_action = old_payload["action"].as_str().unwrap_or("add-cognition");
                let old_kind = old_payload["kind"].as_str().unwrap_or("collection");
                if old_kind == "app-resolution"
                    || matches!(old_action, "create-app-policy" | "save-app-run")
                {
                    app_decision_archive.push(json!({
                        "decisionId": decision_id,
                        "oldObjectHash": old_hash,
                        "action": old_action,
                        "payload": old_payload
                    }));
                    continue;
                }
                let action = match old_action {
                    "reject-proposal" => "dismiss-thought",
                    "save-synthesis-cognition" => "add-cognition",
                    other => other,
                };
                if !matches!(
                    action,
                    "add-cognition"
                        | "keep-thought-only"
                        | "dismiss-thought"
                        | "retire-cognition"
                        | "reactivate-cognition"
                        | "restore-snapshot"
                ) {
                    app_decision_archive.push(json!({
                        "decisionId": decision_id,
                        "oldObjectHash": old_hash,
                        "action": old_action,
                        "payload": old_payload
                    }));
                    continue;
                }
                let thought = old_payload["thought"]
                    .as_str()
                    .and_then(|hash| hash_map.get(hash).cloned())
                    .or_else(|| old_payload["thought"].as_str().map(str::to_string));
                let decision = DecisionPayload {
                    decision_id: decision_id.clone(),
                    base_snapshot: old_payload["baseSnapshot"]
                        .as_str()
                        .unwrap_or(&old_current)
                        .to_string(),
                    action: action.to_string(),
                    actor: old_payload["actor"]
                        .as_str()
                        .unwrap_or("local-user")
                        .to_string(),
                    thought,
                    final_body_markdown: old_payload["finalBodyMarkdown"]
                        .as_str()
                        .map(str::to_string),
                    note_markdown: old_payload["noteMarkdown"].as_str().map(str::to_string),
                    decided_at: old_payload["decidedAt"]
                        .as_str()
                        .unwrap_or(&migrated_at)
                        .to_string(),
                };
                let new_hash = ws.write_object("decision", &decision)?;
                tree.decisions.insert(decision_id.clone(), new_hash.clone());
                hash_map.insert(old_hash.to_string(), new_hash.clone());
                mappings.push(json!({ "kind": "decision", "id": decision_id, "old": old_hash, "new": new_hash }));
            }
        }

        if let Some(cognitions) = payload["cognitions"].as_object() {
            for (cognition_id, old_hash_value) in cognitions {
                let Some(old_hash) = old_hash_value.as_str() else {
                    continue;
                };
                let old = me_object_raw(&root, old_hash, "cognition")?;
                let old_payload = &old["payload"];
                let body = old_payload["bodyMarkdown"]
                    .as_str()
                    .unwrap_or("")
                    .to_string();
                let origin_thought = old_payload["originThought"]
                    .as_str()
                    .and_then(|hash| hash_map.get(hash).cloned())
                    .or_else(|| old_payload["originThought"].as_str().map(str::to_string))
                    .unwrap_or_default();
                let added_by_decision = old_payload["addedByDecision"]
                    .as_str()
                    .and_then(|hash| hash_map.get(hash).cloned())
                    .unwrap_or_else(|| fallback_decision_hash.clone());
                if added_by_decision == fallback_decision_hash
                    && !tree.decisions.contains_key(&fallback_decision.decision_id)
                {
                    tree.decisions.insert(
                        fallback_decision.decision_id.clone(),
                        fallback_decision_hash.clone(),
                    );
                }
                let cognition = CognitionPayload {
                    cognition_id: cognition_id.clone(),
                    body_markdown: body.clone(),
                    body_text: old_payload["bodyText"]
                        .as_str()
                        .map(str::to_string)
                        .unwrap_or_else(|| markdown_to_text(&body)),
                    display_title: old_payload["displayTitle"].as_str().map(str::to_string),
                    origin_thought,
                    added_by_decision,
                    added_at: old_payload["addedAt"]
                        .as_str()
                        .unwrap_or(&migrated_at)
                        .to_string(),
                };
                let new_hash = ws.write_object("cognition", &cognition)?;
                tree.cognitions
                    .insert(cognition_id.clone(), new_hash.clone());
                let state = payload["cognitionStates"][cognition_id]
                    .as_str()
                    .unwrap_or("active")
                    .to_string();
                tree.cognition_states.insert(cognition_id.clone(), state);
                hash_map.insert(old_hash.to_string(), new_hash.clone());
                mappings.push(json!({ "kind": "cognition", "id": cognition_id, "old": old_hash, "new": new_hash }));
            }
        }

        let export_dir = root.join("exports/migration/v4-app-runs");
        fs::create_dir_all(&export_dir).map_err(|err| MeError::Internal(err.into()))?;
        let mut app_records = Vec::new();
        for (field, object_type) in [
            ("apps", "app-definition"),
            ("appPolicies", "app-policy"),
            ("appRuns", "app-run"),
            ("proposals", "proposal"),
        ] {
            if let Some(items) = payload[field].as_object() {
                for (id, old_hash_value) in items {
                    let Some(old_hash) = old_hash_value.as_str() else {
                        continue;
                    };
                    let old = me_object_raw(&root, old_hash, object_type)?;
                    let old_payload = &old["payload"];
                    let mut record = json!({
                        "id": id,
                        "oldObjectHash": old_hash,
                        "objectType": object_type,
                        "appId": old_payload["appId"].as_str().or_else(|| old_payload["app_id"].as_str()),
                        "appVersion": old_payload["appVersion"].as_str().or_else(|| old_payload["version"].as_str()),
                        "referencedCognitions": old_payload["selectedCognitions"],
                        "task": old_payload["taskMarkdown"],
                        "outputHash": old_payload["outputHash"],
                        "policyScope": old_payload["scope"],
                        "time": old_payload["createdAt"].as_str().or_else(|| old_payload["installedAt"].as_str()).or_else(|| old_payload["created_at"].as_str()),
                        "payload": old_payload
                    });
                    if object_type == "app-run" {
                        let body = old_payload["output"]["bodyMarkdown"].as_str().unwrap_or("");
                        let export_path = export_dir.join(format!("{}.md", safe_file_stem(id)));
                        atomic_write(&export_path, body.as_bytes())?;
                        record["exportPath"] = json!(export_path);
                    }
                    app_records.push(record);
                }
            }
        }
        for record in app_decision_archive {
            app_records.push(json!({
                "objectType": "decision",
                "oldObjectHash": record["oldObjectHash"],
                "decisionId": record["decisionId"],
                "action": record["action"],
                "payload": record["payload"]
            }));
        }
        let app_archive = json!({
            "schemaVersion": 1,
            "sourceSchemaVersion": 4,
            "targetSchemaVersion": SCHEMA_VERSION,
            "migratedAt": migrated_at,
            "suggestedProcedures": {
                "inspect-me": "procedures/inspect-me.md",
                "speak-for-me": "procedures/speak-from-me.md",
                "other": "manual review required"
            },
            "records": app_records
        });
        let app_archive_path = root.join(".me/migrations/v4-apps.json");
        atomic_write(
            &app_archive_path,
            &serde_json::to_vec_pretty(&app_archive)
                .map_err(|err| MeError::Internal(err.into()))?,
        )?;

        let tree_hash = ws.write_object("me-tree", &tree)?;
        let snapshot = MeSnapshotPayload {
            parent: None,
            tree: tree_hash,
            operation: "migrate-from-v4".to_string(),
            actor: "local-user".to_string(),
            message: "Migrate ME v0.4 workspace to ME v0.5".to_string(),
            created_at: migrated_at.clone(),
        };
        let new_current = ws.write_object("me-snapshot", &snapshot)?;
        atomic_write(
            &root.join(".me/refs/current"),
            format!("{new_current}\n").as_bytes(),
        )?;
        ws.append_journal(
            None,
            &new_current,
            "migrate-from-v4",
            "Migrate ME v0.4 workspace to ME v0.5",
        )?;
        config.schema_version = SCHEMA_VERSION;
        let toml = toml::to_string_pretty(&config).map_err(|err| MeError::Internal(err.into()))?;
        atomic_write(&root.join("me.toml"), toml.as_bytes())?;
        ws.regenerate_views()?;
        ws.rebuild_index()?;
        ws.fsck()?;
        let manifest = json!({
            "schemaVersion": 1,
            "sourceSystem": "me",
            "sourceWorkspaceVersion": 4,
            "targetSystem": "me",
            "targetWorkspaceVersion": 5,
            "migratedAt": migrated_at,
            "oldCurrentSnapshot": old_current,
            "newCurrentSnapshot": new_current,
            "objects": mappings,
            "appAuditArchive": app_archive_path,
            "appAuditArchiveCount": app_archive["records"].as_array().map_or(0, Vec::len),
            "appRunExportDirectory": export_dir,
            "appRunExportCount": payload["appRuns"].as_object().map_or(0, |items| items.len()),
            "oldObjectsPreserved": true
        });
        let manifest_path = root.join(".me/migrations/me-v4-to-me-v5-manifest.json");
        atomic_write(
            &manifest_path,
            &serde_json::to_vec_pretty(&manifest).map_err(|err| MeError::Internal(err.into()))?,
        )?;
        Ok(json!({
            "workspace": root,
            "manifest": manifest_path,
            "appAuditArchive": manifest["appAuditArchive"],
            "appAuditArchiveCount": manifest["appAuditArchiveCount"],
            "appRunExportDirectory": manifest["appRunExportDirectory"],
            "appRunExportCount": manifest["appRunExportCount"],
            "oldCurrentSnapshot": manifest["oldCurrentSnapshot"],
            "newCurrentSnapshot": manifest["newCurrentSnapshot"],
            "oldObjectsPreserved": true
        }))
    }

    fn ensure_supported(&self) -> Result<()> {
        let version_path = self.root.join(".me/VERSION");
        if !version_path.exists() {
            return Err(not_found(format!(
                "Not a ME workspace: {}",
                self.root.display()
            )));
        }
        let config = self.config()?;
        if config.schema_version != SCHEMA_VERSION {
            return Err(MeError::UnsupportedWorkspace {
                code: "UNSUPPORTED_WORKSPACE_VERSION",
                message: format!(
                    "Unsupported workspace schema version {}",
                    config.schema_version
                ),
                details: json!({ "schemaVersion": config.schema_version }),
            });
        }
        Ok(())
    }

    fn config(&self) -> Result<me_core::WorkspaceConfig> {
        let raw = fs::read_to_string(self.root.join("me.toml"))
            .map_err(|err| MeError::Internal(err.into()))?;
        toml::from_str(&raw).map_err(|err| MeError::Internal(err.into()))
    }

    fn create_layout(&self) -> Result<()> {
        for dir in [
            "inbox",
            "references",
            "procedures",
            "views/thoughts",
            "views/cognitions",
            "views/history",
            "exports",
            ".me/objects",
            ".me/refs",
            ".me/journal",
            ".me/derived",
            ".me/migrations",
            ".me/tmp",
            ".agents/skills/me/references",
            ".agents/skills/me/agents",
        ] {
            fs::create_dir_all(self.root.join(dir)).map_err(|err| MeError::Internal(err.into()))?;
        }
        atomic_write(
            &self.root.join(".me/VERSION"),
            format!("{WORKSPACE_VERSION}\n").as_bytes(),
        )?;
        if !self.root.join(".me/lock").exists() {
            File::create(self.root.join(".me/lock"))
                .map_err(|err| MeError::Internal(err.into()))?;
        }
        self.ensure_guidance_state()?;
        self.write_builtin_procedures()?;
        Ok(())
    }

    fn write_config(&self) -> Result<()> {
        let name = self
            .root
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("ME")
            .to_string();
        let config = me_core::WorkspaceConfig::new(new_id("mews"), name);
        let toml = toml::to_string_pretty(&config).map_err(|err| MeError::Internal(err.into()))?;
        atomic_write(&self.root.join("me.toml"), toml.as_bytes())
    }

    fn write_initial_snapshot(&self) -> Result<()> {
        let tree = MeTreePayload::default();
        let tree_hash = self.write_object("me-tree", &tree)?;
        let snapshot = MeSnapshotPayload {
            parent: None,
            tree: tree_hash,
            operation: "init".to_string(),
            actor: "me".to_string(),
            message: "Initialize ME workspace".to_string(),
            created_at: now_rfc3339()?,
        };
        let snapshot_hash = self.write_object("me-snapshot", &snapshot)?;
        atomic_write(
            &self.root.join(".me/refs/current"),
            format!("{snapshot_hash}\n").as_bytes(),
        )?;
        self.append_journal(None, &snapshot_hash, "init", "Initialize ME workspace")?;
        self.regenerate_views()?;
        Ok(())
    }

    fn seed_demo(&self) -> Result<Value> {
        let seeds = [
            (
                "Designing a generative system is part of authorship.",
                "Authorship",
            ),
            (
                "The possibility space is part of the artwork.",
                "Possibility Space",
            ),
            (
                "An individual output is the form directly encountered by a viewer.",
                "Encountered Output",
            ),
        ];
        self.with_lock(|| {
            let current = self.load_current()?;
            let mut tree = current.tree.clone();
            let mut records = Vec::new();
            for (body, title) in seeds {
                let thought = ThoughtPayload {
                    thought_id: new_id("thought"),
                    kind: "idea".to_string(),
                    body_markdown: body.to_string(),
                    body_text: markdown_to_text(body),
                    origin: Origin::local_input(),
                    captured_at: now_rfc3339()?,
                    captured_by: "local-user".to_string(),
                };
                let thought_hash = self.write_object("thought", &thought)?;
                let decision = DecisionPayload {
                    decision_id: new_id("decision"),
                    base_snapshot: current.snapshot_hash.clone(),
                    action: "add-cognition".to_string(),
                    actor: "local-user".to_string(),
                    thought: Some(thought_hash.clone()),
                    final_body_markdown: Some(body.to_string()),
                    note_markdown: Some("Demo seed".to_string()),
                    decided_at: now_rfc3339()?,
                };
                let decision_hash = self.write_object("decision", &decision)?;
                let cognition = CognitionPayload {
                    cognition_id: new_id("cognition"),
                    body_markdown: body.to_string(),
                    body_text: markdown_to_text(body),
                    display_title: Some(title.to_string()),
                    origin_thought: thought_hash.clone(),
                    added_by_decision: decision_hash.clone(),
                    added_at: now_rfc3339()?,
                };
                let cognition_hash = self.write_object("cognition", &cognition)?;
                tree.thoughts
                    .insert(thought.thought_id.clone(), thought_hash.clone());
                tree.thought_states
                    .insert(thought.thought_id.clone(), "added".to_string());
                tree.decisions
                    .insert(decision.decision_id.clone(), decision_hash.clone());
                tree.cognitions
                    .insert(cognition.cognition_id.clone(), cognition_hash.clone());
                tree.cognition_states
                    .insert(cognition.cognition_id.clone(), "active".to_string());
                records.push(json!({
                    "thoughtId": thought.thought_id,
                    "thought": thought_hash,
                    "cognitionId": cognition.cognition_id,
                    "cognition": cognition_hash
                }));
            }
            let pending = ThoughtPayload {
                thought_id: new_id("thought"),
                kind: "idea".to_string(),
                body_markdown:
                    "Delegating execution does not necessarily delegate artistic judgment."
                        .to_string(),
                body_text: "Delegating execution does not necessarily delegate artistic judgment."
                    .to_string(),
                origin: Origin::local_input(),
                captured_at: now_rfc3339()?,
                captured_by: "local-user".to_string(),
            };
            let pending_hash = self.write_object("thought", &pending)?;
            tree.thoughts
                .insert(pending.thought_id.clone(), pending_hash.clone());
            tree.thought_states
                .insert(pending.thought_id.clone(), "pending".to_string());
            records.push(json!({
                "thoughtId": pending.thought_id,
                "thought": pending_hash,
                "state": "pending"
            }));
            let snapshot = self.commit_tree(
                &current,
                tree,
                "demo-seed",
                "local-user",
                "Create ME demo Cognition Library".to_string(),
            )?;
            Ok(json!({ "records": records, "snapshot": snapshot }))
        })
    }

    fn apply_decision(
        &self,
        current: &CurrentState,
        proposal_hash: &str,
        proposal: &me_core::ProposalPayload,
        decision_hash: &str,
        decision: &DecisionPayload,
    ) -> Result<Value> {
        let mut tree = current.tree.clone();
        tree.proposals
            .insert(proposal.proposal_id.clone(), proposal_hash.to_string());
        tree.decisions
            .insert(decision.decision_id.clone(), decision_hash.to_string());
        let proposal_thought = proposal_thought_hash(proposal)
            .ok_or_else(|| invalid("Proposal requires inputs.thought"))?;
        let (thought_id, thought_hash, thought) = self.resolve_thought(&tree, proposal_thought)?;
        let action = decision.action.as_str();
        let mut result = json!({
            "proposalId": proposal.proposal_id,
            "proposal": proposal_hash,
            "decisionId": decision.decision_id,
            "decision": decision_hash,
            "operation": action,
            "existingCognitionsChanged": 0
        });

        match action {
            "add-cognition" | "save-synthesis-cognition" => {
                let body = decision
                    .final_body_markdown
                    .clone()
                    .or_else(|| {
                        proposal
                            .recommendation
                            .get("bodyMarkdown")
                            .and_then(Value::as_str)
                            .map(str::to_string)
                    })
                    .unwrap_or_else(|| thought.payload.body_markdown.clone());
                let cognition = CognitionPayload {
                    cognition_id: new_id("cognition"),
                    body_text: markdown_to_text(&body),
                    body_markdown: body,
                    display_title: proposal
                        .recommendation
                        .get("displayTitle")
                        .and_then(Value::as_str)
                        .map(str::to_string),
                    origin_thought: thought_hash.clone(),
                    added_by_decision: decision_hash.to_string(),
                    added_at: now_rfc3339()?,
                };
                let cognition_hash = self.write_object("cognition", &cognition)?;
                tree.cognitions
                    .insert(cognition.cognition_id.clone(), cognition_hash.clone());
                tree.cognition_states
                    .insert(cognition.cognition_id.clone(), "active".to_string());
                tree.thought_states.insert(thought_id, "added".to_string());
                result["statusLabel"] = json!("ADDED TO ME");
                result["cognitionId"] = json!(cognition.cognition_id);
                result["cognition"] = json!(cognition_hash);
                result["cognitionsAdded"] = json!(1);
                result["existingCognitionsChanged"] = json!(0);
            }
            "keep-thought-only" => {
                tree.thought_states
                    .insert(thought_id, "kept-only".to_string());
                result["statusLabel"] = json!("THOUGHT -- kept only");
                result["cognitionsAdded"] = json!(0);
            }
            "dismiss-thought" | "reject-proposal" => {
                tree.thought_states
                    .insert(thought_id, "dismissed".to_string());
                result["statusLabel"] = json!("THOUGHT -- dismissed");
                result["cognitionsAdded"] = json!(0);
            }
            other => return Err(invalid(format!("Unsupported Decision action: {other}"))),
        }
        self.remove_pending_proposal(&proposal.proposal_id)?;
        let snapshot = self.commit_tree(
            current,
            tree,
            action,
            &decision.actor,
            format!("Decision {} for {}", decision.action, proposal.proposal_id),
        )?;
        result["snapshot"] = json!(snapshot);
        Ok(result)
    }

    fn set_cognition_state(
        &self,
        cognition_id_or_hash: &str,
        decision_file: impl AsRef<Path>,
        state: &str,
        operation: &str,
    ) -> Result<Value> {
        if !cognition_state_allowed(state) {
            return Err(invalid(format!("Unsupported Cognition state: {state}")));
        }
        let raw = fs::read_to_string(decision_file.as_ref())
            .map_err(|err| MeError::Internal(err.into()))?;
        let value: Value = if raw.trim().is_empty() {
            json!({})
        } else {
            serde_json::from_str(&raw).map_err(|err| MeError::Internal(err.into()))?
        };
        let action = value
            .get("action")
            .and_then(Value::as_str)
            .or_else(|| value.get("operation").and_then(Value::as_str))
            .unwrap_or(operation);
        if action != operation {
            return Err(invalid(format!(
                "{operation} requires {operation} Decision, got {action}"
            )));
        }
        self.with_lock(|| {
            let current = self.load_current()?;
            validate_base_snapshot(&value, &current.snapshot_hash)?;
            let (cognition_id, cognition_hash, _) =
                self.resolve_cognition(&current.tree, cognition_id_or_hash)?;
            let actor = value
                .get("actor")
                .and_then(Value::as_str)
                .map(str::to_string)
                .unwrap_or(self.config()?.default_actor);
            let decision = DecisionPayload {
                decision_id: new_id("decision"),
                base_snapshot: current.snapshot_hash.clone(),
                action: operation.to_string(),
                actor: actor.clone(),
                thought: None,
                final_body_markdown: None,
                note_markdown: value
                    .get("noteMarkdown")
                    .and_then(Value::as_str)
                    .map(str::to_string),
                decided_at: value
                    .get("decidedAt")
                    .and_then(Value::as_str)
                    .map(str::to_string)
                    .unwrap_or(now_rfc3339()?),
            };
            let decision_hash = self.write_object("decision", &decision)?;
            let mut tree = current.tree.clone();
            tree.decisions
                .insert(decision.decision_id.clone(), decision_hash.clone());
            tree.cognition_states
                .insert(cognition_id.clone(), state.to_string());
            let snapshot = self.commit_tree(
                &current,
                tree,
                operation,
                &actor,
                format!("{operation} {cognition_id}"),
            )?;
            Ok(json!({
                "cognitionId": cognition_id,
                "cognition": cognition_hash,
                "decisionId": decision.decision_id,
                "decision": decision_hash,
                "state": state,
                "snapshot": snapshot
            }))
        })
    }

    fn with_lock<T>(&self, f: impl FnOnce() -> Result<T>) -> Result<T> {
        fs::create_dir_all(self.root.join(".me")).map_err(|err| MeError::Internal(err.into()))?;
        let lock_file_path = self.root.join(".me/lock");
        let file = lock_file(&lock_file_path, "ME")?;
        let result = f();
        let _ = FileExt::unlock(&file);
        result
    }

    fn current_ref(&self) -> Result<String> {
        Ok(fs::read_to_string(self.root.join(".me/refs/current"))
            .map_err(|err| MeError::Internal(err.into()))?
            .trim()
            .to_string())
    }

    fn load_current(&self) -> Result<CurrentState> {
        let snapshot_hash = self.current_ref()?;
        let snapshot = self.read_object::<MeSnapshotPayload>(&snapshot_hash, "me-snapshot")?;
        let tree_hash = snapshot.payload.tree.clone();
        let tree = self.read_object::<MeTreePayload>(&tree_hash, "me-tree")?;
        Ok(CurrentState {
            snapshot_hash,
            snapshot: snapshot.payload,
            tree_hash,
            tree: tree.payload,
        })
    }

    fn guidance_state_path(&self) -> PathBuf {
        self.root.join(".me/derived/guidance.json")
    }

    fn ensure_guidance_state(&self) -> Result<()> {
        let path = self.guidance_state_path();
        if !path.exists() {
            self.write_guidance_state(&GuidanceState::default())?;
        }
        Ok(())
    }

    fn read_guidance_state(&self) -> Result<GuidanceState> {
        let path = self.guidance_state_path();
        if !path.exists() {
            return Ok(GuidanceState::default());
        }
        let raw = fs::read_to_string(&path).map_err(|err| MeError::Internal(err.into()))?;
        if raw.trim().is_empty() {
            return Ok(GuidanceState::default());
        }
        let mut state: GuidanceState =
            serde_json::from_str(&raw).map_err(|err| MeError::Internal(err.into()))?;
        if state.schema_version == 0 {
            state.schema_version = 1;
        }
        Ok(state)
    }

    fn write_guidance_state(&self, state: &GuidanceState) -> Result<()> {
        atomic_write(
            &self.guidance_state_path(),
            &serde_json::to_vec_pretty(state).map_err(|err| MeError::Internal(err.into()))?,
        )
    }

    fn snapshot_tree(&self, snapshot_hash: &str) -> Result<MeTreePayload> {
        let snapshot = self.read_object::<MeSnapshotPayload>(snapshot_hash, "me-snapshot")?;
        let tree = self.read_object::<MeTreePayload>(&snapshot.payload.tree, "me-tree")?;
        Ok(tree.payload)
    }

    fn commit_tree(
        &self,
        current: &CurrentState,
        tree: MeTreePayload,
        operation: &str,
        actor: &str,
        message: String,
    ) -> Result<String> {
        let tree_hash = self.write_object("me-tree", &tree)?;
        let snapshot = MeSnapshotPayload {
            parent: Some(current.snapshot_hash.clone()),
            tree: tree_hash,
            operation: operation.to_string(),
            actor: actor.to_string(),
            message: message.clone(),
            created_at: now_rfc3339()?,
        };
        let snapshot_hash = self.write_object("me-snapshot", &snapshot)?;
        atomic_write(
            &self.root.join(".me/refs/current"),
            format!("{snapshot_hash}\n").as_bytes(),
        )?;
        self.append_journal(
            Some(&current.snapshot_hash),
            &snapshot_hash,
            operation,
            &message,
        )?;
        self.regenerate_views()?;
        self.rebuild_index()?;
        Ok(snapshot_hash)
    }

    fn append_journal(
        &self,
        parent: Option<&str>,
        snapshot: &str,
        operation: &str,
        message: &str,
    ) -> Result<()> {
        let path = self.root.join(".me/journal/transitions.ndjson");
        let record = json!({
            "parent": parent,
            "snapshot": snapshot,
            "operation": operation,
            "message": message,
            "recordedAt": now_rfc3339()?
        });
        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(path)
            .map_err(|err| MeError::Internal(err.into()))?;
        writeln!(
            file,
            "{}",
            serde_json::to_string(&record).map_err(|err| MeError::Internal(err.into()))?
        )
        .map_err(|err| MeError::Internal(err.into()))
    }

    fn write_object<T: Serialize>(&self, object_type: &str, payload: &T) -> Result<String> {
        let envelope = ObjectEnvelope::new(object_type, payload);
        let bytes = canonical_json_bytes(&envelope)?;
        let hex = hex_digest(&bytes);
        let hash = sha_ref(&hex);
        let path = self.object_path(&hash)?;
        if path.exists() {
            let mut existing = fs::read(&path).map_err(|err| MeError::Internal(err.into()))?;
            if existing.ends_with(b"\n") {
                existing.pop();
            }
            if existing != bytes {
                return Err(integrity(format!(
                    "Existing object bytes differ for {hash}"
                )));
            }
            return Ok(hash);
        }
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|err| MeError::Internal(err.into()))?;
        }
        fs::create_dir_all(self.root.join(".me/tmp"))
            .map_err(|err| MeError::Internal(err.into()))?;
        let mut tmp = NamedTempFile::new_in(self.root.join(".me/tmp"))
            .map_err(|err| MeError::Internal(err.into()))?;
        tmp.write_all(&bytes)
            .map_err(|err| MeError::Internal(err.into()))?;
        tmp.write_all(b"\n")
            .map_err(|err| MeError::Internal(err.into()))?;
        tmp.as_file()
            .sync_all()
            .map_err(|err| MeError::Internal(err.into()))?;
        tmp.persist(&path)
            .map_err(|err| MeError::Internal(anyhow::anyhow!(err)))?;
        Ok(hash)
    }

    fn read_object<T: DeserializeOwned>(
        &self,
        hash: &str,
        expected_type: &str,
    ) -> Result<ObjectEnvelope<T>> {
        let path = self.object_path(hash)?;
        if !path.exists() {
            return Err(not_found(format!("Object not found: {hash}")));
        }
        let mut bytes = fs::read(path).map_err(|err| MeError::Internal(err.into()))?;
        if bytes.ends_with(b"\n") {
            bytes.pop();
        }
        let digest = sha_ref(&hex_digest(&bytes));
        if digest != hash {
            return Err(integrity(format!("Object hash mismatch for {hash}")));
        }
        let envelope: ObjectEnvelope<T> =
            serde_json::from_slice(&bytes).map_err(|err| MeError::Internal(err.into()))?;
        if envelope.schema_version != SCHEMA_VERSION {
            return Err(integrity(format!(
                "Unsupported object schema version {} for {hash}",
                envelope.schema_version
            )));
        }
        if envelope.object_type != expected_type {
            return Err(integrity(format!(
                "Object {hash} has type {}, expected {expected_type}",
                envelope.object_type
            )));
        }
        Ok(envelope)
    }

    fn object_path(&self, hash: &str) -> Result<PathBuf> {
        let hex = strip_sha_prefix(hash).ok_or_else(|| invalid(format!("Invalid hash: {hash}")))?;
        if hex.len() != 64
            || !hex
                .chars()
                .all(|ch| ch.is_ascii_hexdigit() && !ch.is_ascii_uppercase())
        {
            return Err(invalid(format!("Invalid hash: {hash}")));
        }
        Ok(self.objects_dir().join(&hex[0..2]).join(hex))
    }

    fn objects_dir(&self) -> PathBuf {
        self.root.join(".me/objects")
    }

    fn pending_proposals_dir(&self) -> PathBuf {
        self.root.join(".me/pending/proposals")
    }

    fn pending_proposal_ref(&self, proposal_id: &str) -> PathBuf {
        self.pending_proposals_dir()
            .join(format!("{proposal_id}.ref"))
    }

    fn remove_pending_proposal(&self, proposal_id: &str) -> Result<()> {
        let path = self.pending_proposal_ref(proposal_id);
        if path.exists() {
            fs::remove_file(path).map_err(|err| MeError::Internal(err.into()))?;
        }
        Ok(())
    }

    fn pending_proposal_count(&self) -> Result<usize> {
        let dir = self.pending_proposals_dir();
        if !dir.exists() {
            return Ok(0);
        }
        Ok(fs::read_dir(dir)
            .map_err(|err| MeError::Internal(err.into()))?
            .filter_map(std::result::Result::ok)
            .filter(|entry| entry.file_type().map(|ty| ty.is_file()).unwrap_or(false))
            .count())
    }

    fn parse_proposal_file(&self, file: &Path) -> Result<ObjectEnvelope<me_core::ProposalPayload>> {
        let raw = fs::read_to_string(file).map_err(|err| MeError::Internal(err.into()))?;
        let value: Value =
            serde_json::from_str(&raw).map_err(|err| MeError::Internal(err.into()))?;
        if contains_approval_fields(&value) {
            return Err(invalid("Proposal contains approval/Decision fields"));
        }
        if value.get("objectType").and_then(Value::as_str) == Some("proposal") {
            serde_json::from_value(value).map_err(|err| MeError::Internal(err.into()))
        } else {
            let payload: me_core::ProposalPayload =
                serde_json::from_value(value).map_err(|err| MeError::Internal(err.into()))?;
            Ok(ObjectEnvelope::new("proposal", payload))
        }
    }

    fn parse_decision_file(
        &self,
        file: &Path,
        proposal: &me_core::ProposalPayload,
    ) -> Result<DecisionPayload> {
        let raw = fs::read_to_string(file).map_err(|err| MeError::Internal(err.into()))?;
        let value: Value =
            serde_json::from_str(&raw).map_err(|err| MeError::Internal(err.into()))?;
        if value.get("objectType").and_then(Value::as_str) == Some("decision") {
            let envelope: ObjectEnvelope<DecisionPayload> =
                serde_json::from_value(value).map_err(|err| MeError::Internal(err.into()))?;
            Ok(envelope.payload)
        } else {
            Ok(DecisionPayload {
                decision_id: value
                    .get("decisionId")
                    .and_then(Value::as_str)
                    .unwrap_or("")
                    .to_string(),
                base_snapshot: value
                    .get("baseSnapshot")
                    .and_then(Value::as_str)
                    .unwrap_or(&proposal.base_snapshot)
                    .to_string(),
                action: value
                    .get("action")
                    .and_then(Value::as_str)
                    .or_else(|| value.get("operation").and_then(Value::as_str))
                    .unwrap_or("add-cognition")
                    .to_string(),
                actor: value
                    .get("actor")
                    .and_then(Value::as_str)
                    .unwrap_or("")
                    .to_string(),
                thought: value
                    .get("thought")
                    .and_then(Value::as_str)
                    .map(str::to_string)
                    .or_else(|| proposal_thought_hash(proposal).map(str::to_string)),
                final_body_markdown: value
                    .get("finalBodyMarkdown")
                    .and_then(Value::as_str)
                    .map(str::to_string),
                note_markdown: value
                    .get("noteMarkdown")
                    .and_then(Value::as_str)
                    .map(str::to_string),
                decided_at: value
                    .get("decidedAt")
                    .and_then(Value::as_str)
                    .unwrap_or("")
                    .to_string(),
            })
        }
    }

    fn validate_proposal(
        &self,
        proposal: &ObjectEnvelope<me_core::ProposalPayload>,
        current: &CurrentState,
        write_object: bool,
    ) -> Result<String> {
        if proposal.schema_version != SCHEMA_VERSION || proposal.object_type != "proposal" {
            return Err(invalid(format!(
                "Proposal must be a schemaVersion {SCHEMA_VERSION} proposal object"
            )));
        }
        if proposal.payload.proposal_id.trim().is_empty() {
            return Err(invalid("Proposal ID is required"));
        }
        if proposal.payload.base_snapshot != current.snapshot_hash {
            return Err(MeError::StaleProposal {
                code: "STALE_PROPOSAL",
                message: format!(
                    "Proposal was based on Snapshot {}, current is {}",
                    proposal.payload.base_snapshot, current.snapshot_hash
                ),
                details: json!({}),
            });
        }
        self.read_object::<MeSnapshotPayload>(&proposal.payload.base_snapshot, "me-snapshot")?;
        let thought_hash = proposal_thought_hash(&proposal.payload)
            .ok_or_else(|| invalid("Proposal requires inputs.thought"))?;
        self.resolve_thought(&current.tree, thought_hash)?;
        let operation = string_field(&proposal.payload.recommendation, "operation")?;
        if !operation_allowed(operation) {
            return Err(invalid(format!(
                "Unsupported proposal operation: {operation}"
            )));
        }
        for related in &proposal.payload.related_cognitions {
            let (_, hash, _) = self.resolve_cognition(&current.tree, &related.cognition)?;
            if hash != related.cognition {
                return Err(invalid(
                    "Related Cognition hash does not match current tree",
                ));
            }
        }
        let hash = if write_object {
            self.write_object("proposal", &proposal.payload)?
        } else {
            let bytes = canonical_json_bytes(proposal)?;
            sha_ref(&hex_digest(&bytes))
        };
        Ok(hash)
    }

    fn resolve_thought(
        &self,
        tree: &MeTreePayload,
        thought_id_or_hash: &str,
    ) -> Result<(String, String, ObjectEnvelope<ThoughtPayload>)> {
        if is_sha_ref(thought_id_or_hash) {
            let thought = self.read_object::<ThoughtPayload>(thought_id_or_hash, "thought")?;
            let thought_id = tree
                .thoughts
                .iter()
                .find_map(|(id, hash)| (hash == thought_id_or_hash).then(|| id.clone()))
                .unwrap_or_else(|| thought.payload.thought_id.clone());
            return Ok((thought_id, thought_id_or_hash.to_string(), thought));
        }
        let hash = tree
            .thoughts
            .get(thought_id_or_hash)
            .ok_or_else(|| not_found(format!("Thought not found: {thought_id_or_hash}")))?
            .clone();
        let thought = self.read_object::<ThoughtPayload>(&hash, "thought")?;
        Ok((thought_id_or_hash.to_string(), hash, thought))
    }

    fn resolve_cognition(
        &self,
        tree: &MeTreePayload,
        cognition_id_or_hash: &str,
    ) -> Result<(String, String, ObjectEnvelope<CognitionPayload>)> {
        if is_sha_ref(cognition_id_or_hash) {
            let cognition =
                self.read_object::<CognitionPayload>(cognition_id_or_hash, "cognition")?;
            let cognition_id = tree
                .cognitions
                .iter()
                .find_map(|(id, hash)| (hash == cognition_id_or_hash).then(|| id.clone()))
                .unwrap_or_else(|| cognition.payload.cognition_id.clone());
            return Ok((cognition_id, cognition_id_or_hash.to_string(), cognition));
        }
        let hash = tree
            .cognitions
            .get(cognition_id_or_hash)
            .ok_or_else(|| not_found(format!("Cognition not found: {cognition_id_or_hash}")))?
            .clone();
        let cognition = self.read_object::<CognitionPayload>(&hash, "cognition")?;
        Ok((cognition_id_or_hash.to_string(), hash, cognition))
    }

    fn resolve_run(
        &self,
        tree: &MeTreePayload,
        run_id_or_hash: &str,
    ) -> Result<(String, String, ObjectEnvelope<AppRunPayload>)> {
        if is_sha_ref(run_id_or_hash) {
            let run = self.read_object::<AppRunPayload>(run_id_or_hash, "app-run")?;
            let run_id = tree
                .app_runs
                .iter()
                .find_map(|(id, hash)| (hash == run_id_or_hash).then(|| id.clone()))
                .unwrap_or_else(|| run.payload.run_id.clone());
            return Ok((run_id, run_id_or_hash.to_string(), run));
        }
        let hash = tree
            .app_runs
            .get(run_id_or_hash)
            .ok_or_else(|| not_found(format!("App Run not found: {run_id_or_hash}")))?
            .clone();
        let run = self.read_object::<AppRunPayload>(&hash, "app-run")?;
        Ok((run_id_or_hash.to_string(), hash, run))
    }

    fn resolve_proposal(
        &self,
        proposal_id_or_hash: &str,
    ) -> Result<(String, ObjectEnvelope<me_core::ProposalPayload>)> {
        if is_sha_ref(proposal_id_or_hash) {
            let proposal =
                self.read_object::<me_core::ProposalPayload>(proposal_id_or_hash, "proposal")?;
            return Ok((proposal_id_or_hash.to_string(), proposal));
        }
        let pending = self.pending_proposal_ref(proposal_id_or_hash);
        if pending.exists() {
            let hash = fs::read_to_string(pending)
                .map_err(|err| MeError::Internal(err.into()))?
                .trim()
                .to_string();
            let proposal = self.read_object::<me_core::ProposalPayload>(&hash, "proposal")?;
            return Ok((hash, proposal));
        }
        let current = self.load_current()?;
        let hash = current
            .tree
            .proposals
            .get(proposal_id_or_hash)
            .ok_or_else(|| not_found(format!("Proposal not found: {proposal_id_or_hash}")))?
            .clone();
        let proposal = self.read_object::<me_core::ProposalPayload>(&hash, "proposal")?;
        Ok((hash, proposal))
    }

    fn find_exact_thought_duplicate(
        &self,
        tree: &MeTreePayload,
        kind: &str,
        body: &str,
    ) -> Result<Option<String>> {
        for hash in tree.thoughts.values() {
            let thought = self.read_object::<ThoughtPayload>(hash, "thought")?;
            if thought.payload.kind == kind
                && thought.payload.body_markdown == body
                && thought.payload.origin.origin_type == "local-input"
            {
                return Ok(Some(hash.clone()));
            }
        }
        Ok(None)
    }

    fn match_thought(
        &self,
        tree: &MeTreePayload,
        body: &str,
        limit: usize,
    ) -> Result<Vec<MatchResult>> {
        self.match_cognitions(tree, body, limit, Some("active"))
    }

    fn match_cognitions(
        &self,
        tree: &MeTreePayload,
        body: &str,
        limit: usize,
        state_filter: Option<&str>,
    ) -> Result<Vec<MatchResult>> {
        let mut cognitions = Vec::new();
        for (cognition_id, hash) in &tree.cognitions {
            let state = tree
                .cognition_states
                .get(cognition_id)
                .cloned()
                .unwrap_or_else(|| "active".to_string());
            if !state_filter.is_none_or(|filter| filter == "all" || filter == state) {
                continue;
            }
            let cognition = self.read_object::<CognitionPayload>(hash, "cognition")?;
            cognitions.push(CognitionDoc {
                cognition_id: cognition_id.clone(),
                cognition_hash: hash.clone(),
                display_title: cognition.payload.display_title,
                body: cognition.payload.body_text,
                state,
            });
        }
        Ok(rank_cognitions(body, &cognitions, limit))
    }

    fn thought_id_for_hash(&self, tree: &MeTreePayload, thought_hash: &str) -> Option<String> {
        tree.thoughts
            .iter()
            .find_map(|(thought_id, hash)| (hash == thought_hash).then(|| thought_id.clone()))
    }

    fn related_from_matches(
        &self,
        matches: &[MatchResult],
        _thought_text: &str,
    ) -> Result<Vec<RelatedCognition>> {
        let mut related = Vec::new();
        for matched in matches {
            related.push(RelatedCognition {
                cognition: matched.cognition.clone(),
                cognition_id: matched.cognition_id.clone(),
                score: matched.score,
                status: "derived".to_string(),
                matched_terms: matched.matched_terms.clone(),
                explanation: Some(
                    "Deterministic lexical match; derived retrieval only, not authoritative."
                        .to_string(),
                ),
            });
        }
        Ok(related)
    }

    fn current_cognition_summaries(&self, tree: &MeTreePayload) -> Result<Vec<Value>> {
        let mut out = Vec::new();
        for (cognition_id, hash) in &tree.cognitions {
            let cognition = self.read_object::<CognitionPayload>(hash, "cognition")?;
            let state = tree
                .cognition_states
                .get(cognition_id)
                .cloned()
                .unwrap_or_else(|| "active".to_string());
            out.push(json!({
                "statusLabel": "COGNITION -- user-authorized",
                "cognitionId": cognition_id,
                "cognition": hash,
                "bodyMarkdown": cognition.payload.body_markdown,
                "displayTitle": cognition.payload.display_title,
                "state": state,
                "originThought": cognition.payload.origin_thought,
                "addedAt": cognition.payload.added_at
            }));
        }
        Ok(out)
    }

    fn recent_home_events(&self, tree: &MeTreePayload) -> Result<Vec<Value>> {
        let mut added = Vec::new();
        let mut retired = Vec::new();
        for (cognition_id, hash) in &tree.cognitions {
            let cognition = self.read_object::<CognitionPayload>(hash, "cognition")?;
            let event = json!({
                "kind": "cognition-added",
                "preview": preview_text(&cognition.payload.body_markdown),
                "cognitionId": cognition_id,
            });
            if tree
                .cognition_states
                .get(cognition_id)
                .is_some_and(|state| state == "retired")
            {
                retired.push(json!({
                    "kind": "cognition-retired",
                    "preview": preview_text(&cognition.payload.body_markdown),
                    "cognitionId": cognition_id,
                }));
            } else {
                added.push(event);
            }
        }
        added.sort_by(|a, b| {
            a["preview"]
                .as_str()
                .unwrap_or("")
                .cmp(b["preview"].as_str().unwrap_or(""))
        });
        retired.sort_by(|a, b| {
            a["preview"]
                .as_str()
                .unwrap_or("")
                .cmp(b["preview"].as_str().unwrap_or(""))
        });
        let mut out = Vec::new();
        out.extend(added.into_iter().take(2));
        out.extend(retired.into_iter().take(1));
        Ok(out)
    }

    fn app_summaries(&self, tree: &MeTreePayload) -> Result<Vec<Value>> {
        let mut out = Vec::new();
        for (app_id, hash) in &tree.apps {
            let app = self.read_object::<AppDefinitionPayload>(hash, "app-definition")?;
            out.push(json!({
                "appId": app_id,
                "app": hash,
                "name": app.payload.name,
                "version": app.payload.version
            }));
        }
        Ok(out)
    }

    fn regenerate_views(&self) -> Result<()> {
        let current = self.load_current()?;
        let views = self.root.join("views");
        fs::create_dir_all(&views).map_err(|err| MeError::Internal(err.into()))?;
        for child in ["app-policies", "apps", "runs"] {
            let path = views.join(child);
            if path.exists() {
                fs::remove_dir_all(&path).map_err(|err| MeError::Internal(err.into()))?;
            }
        }
        for child in ["thoughts", "cognitions", "history"] {
            let path = views.join(child);
            if path.exists() {
                fs::remove_dir_all(&path).map_err(|err| MeError::Internal(err.into()))?;
            }
            fs::create_dir_all(&path).map_err(|err| MeError::Internal(err.into()))?;
        }
        atomic_write(
            &views.join("home.md"),
            home_markdown(&self.home("json")?).as_bytes(),
        )?;
        atomic_write(
            &views.join("welcome.md"),
            self.welcome()?["renderedMarkdown"]
                .as_str()
                .unwrap_or("")
                .as_bytes(),
        )?;
        for (thought_id, hash) in &current.tree.thoughts {
            let thought = self.read_object::<ThoughtPayload>(hash, "thought")?;
            let state = current
                .tree
                .thought_states
                .get(thought_id)
                .cloned()
                .unwrap_or_else(|| "pending".to_string());
            let body = format!(
                "---\ngeneratedBy: me\nsnapshot: {}\nthoughtId: {}\nobject: {}\nstate: {}\n---\n\nGenerated by ME from snapshot {}.\nDo not edit directly.\n\n# Thought {}\n\n{}\n",
                current.snapshot_hash,
                thought_id,
                hash,
                state,
                current.snapshot_hash,
                thought_id,
                thought.payload.body_markdown
            );
            atomic_write(
                &views.join("thoughts").join(format!("{thought_id}.md")),
                body.as_bytes(),
            )?;
        }
        for (cognition_id, hash) in &current.tree.cognitions {
            let cognition = self.read_object::<CognitionPayload>(hash, "cognition")?;
            let state = current
                .tree
                .cognition_states
                .get(cognition_id)
                .cloned()
                .unwrap_or_else(|| "active".to_string());
            let file_name = cognition_file_name(cognition_id);
            let title = cognition
                .payload
                .display_title
                .clone()
                .unwrap_or_else(|| cognition_id.to_string());
            let body = format!(
                "---\ngeneratedBy: me\nsnapshot: {}\ncognitionId: {}\nobject: {}\nstate: {}\n---\n\nGenerated by ME from snapshot {}.\nDo not edit directly.\n\n# {}\n\n{}\n",
                current.snapshot_hash,
                cognition_id,
                hash,
                state,
                current.snapshot_hash,
                title,
                cognition.payload.body_markdown
            );
            atomic_write(&views.join("cognitions").join(file_name), body.as_bytes())?;
        }
        Ok(())
    }

    fn rebuild_index(&self) -> Result<()> {
        let current = self.load_current()?;
        let index_path = self.root.join(".me/index.sqlite");
        if index_path.exists() {
            fs::remove_file(&index_path).map_err(|err| MeError::Internal(err.into()))?;
        }
        let conn = Connection::open(&index_path).map_err(|err| MeError::Internal(err.into()))?;
        conn.execute_batch(
            r#"
            DROP TABLE IF EXISTS cognition_fts;
            DROP TABLE IF EXISTS thought_fts;
            DROP TABLE IF EXISTS snapshots;
            DROP TABLE IF EXISTS decisions;
            DROP TABLE IF EXISTS retrieval_neighbors;
            DROP TABLE IF EXISTS cognitions;
            DROP TABLE IF EXISTS thoughts;
            DROP TABLE IF EXISTS meta;
            CREATE TABLE meta (key TEXT PRIMARY KEY, value TEXT NOT NULL);
            CREATE TABLE thoughts (thought_id TEXT PRIMARY KEY, object TEXT NOT NULL, kind TEXT NOT NULL, state TEXT NOT NULL, body_markdown TEXT NOT NULL, body_text TEXT NOT NULL);
            CREATE TABLE cognitions (cognition_id TEXT PRIMARY KEY, object TEXT NOT NULL, state TEXT NOT NULL, display_title TEXT, body_markdown TEXT NOT NULL, body_text TEXT NOT NULL);
            CREATE TABLE retrieval_neighbors (source_cognition TEXT NOT NULL, target_cognition TEXT NOT NULL, score REAL NOT NULL, matched_terms TEXT NOT NULL, generated_at TEXT NOT NULL, PRIMARY KEY (source_cognition, target_cognition));
            CREATE TABLE decisions (decision_id TEXT PRIMARY KEY, object TEXT NOT NULL, action TEXT NOT NULL);
            CREATE TABLE snapshots (snapshot TEXT PRIMARY KEY, parent TEXT, tree_hash TEXT NOT NULL, operation TEXT NOT NULL, message TEXT NOT NULL, created_at TEXT NOT NULL);
            CREATE VIRTUAL TABLE thought_fts USING fts5(thought_id, body_markdown, body_text);
            CREATE VIRTUAL TABLE cognition_fts USING fts5(cognition_id, display_title, body_text);
            "#,
        )
        .map_err(|err| MeError::Internal(err.into()))?;
        conn.execute(
            "INSERT INTO meta (key, value) VALUES ('currentSnapshot', ?1)",
            params![current.snapshot_hash],
        )
        .map_err(|err| MeError::Internal(err.into()))?;
        for (thought_id, hash) in &current.tree.thoughts {
            let thought = self.read_object::<ThoughtPayload>(hash, "thought")?;
            let state = current
                .tree
                .thought_states
                .get(thought_id)
                .cloned()
                .unwrap_or_else(|| "pending".to_string());
            conn.execute(
                "INSERT INTO thoughts VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                params![
                    thought_id,
                    hash,
                    thought.payload.kind,
                    state,
                    thought.payload.body_markdown,
                    thought.payload.body_text
                ],
            )
            .map_err(|err| MeError::Internal(err.into()))?;
            conn.execute(
                "INSERT INTO thought_fts (thought_id, body_markdown, body_text) VALUES (?1, ?2, ?3)",
                params![thought_id, thought.payload.body_markdown, thought.payload.body_text],
            )
            .map_err(|err| MeError::Internal(err.into()))?;
        }
        for (cognition_id, hash) in &current.tree.cognitions {
            let cognition = self.read_object::<CognitionPayload>(hash, "cognition")?;
            let state = current
                .tree
                .cognition_states
                .get(cognition_id)
                .cloned()
                .unwrap_or_else(|| "active".to_string());
            conn.execute(
                "INSERT INTO cognitions VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                params![
                    cognition_id,
                    hash,
                    state,
                    cognition.payload.display_title,
                    cognition.payload.body_markdown,
                    cognition.payload.body_text
                ],
            )
            .map_err(|err| MeError::Internal(err.into()))?;
            conn.execute(
                "INSERT INTO cognition_fts (cognition_id, display_title, body_text) VALUES (?1, ?2, ?3)",
                params![cognition_id, cognition.payload.display_title, cognition.payload.body_text],
            )
            .map_err(|err| MeError::Internal(err.into()))?;
        }
        for (decision_id, hash) in &current.tree.decisions {
            let decision = self.read_object::<DecisionPayload>(hash, "decision")?;
            conn.execute(
                "INSERT INTO decisions VALUES (?1, ?2, ?3)",
                params![decision_id, hash, decision.payload.action],
            )
            .map_err(|err| MeError::Internal(err.into()))?;
        }
        for entry in self.snapshot_chain(&current.snapshot_hash)? {
            conn.execute(
                "INSERT INTO snapshots VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                params![
                    entry["meSnapshot"].as_str(),
                    entry["parent"].as_str(),
                    entry["meTree"].as_str(),
                    entry["operation"].as_str(),
                    entry["message"].as_str(),
                    entry["createdAt"].as_str()
                ],
            )
            .map_err(|err| MeError::Internal(err.into()))?;
        }
        let neighbors = self.compute_retrieval_neighbors(&current.tree, None)?;
        self.write_retrieval_neighbors_to_conn(&conn, &neighbors)?;
        Ok(())
    }

    fn sync_workspace_docs(&self) -> Result<()> {
        let readme = r#"# ME

ME is a local application operated through Codex App.

When a thought occurs, tell Codex:

> Add this thought to ME:
> ...

ME captures the exact words. You choose whether to keep them.

The prompt captures first. It does not keep the thought until you approve.
Casual add, capture, save, note, or remember wording is still only
thought capture.

A thought you keep in ME is called a cognition.

Codex can inspect, compare, and compose from your cognitions
without changing them.

Start in Codex by running:

```bash
me start
```
"#;
        atomic_write_preserving_user_section(&self.root.join("README.md"), readme.as_bytes())?;
        atomic_write_preserving_user_section(
            &self.root.join("AGENTS.md"),
            workspace_agents_md().as_bytes(),
        )?;
        let skill_dir = self.root.join(".agents/skills/me");
        fs::create_dir_all(skill_dir.join("references"))
            .map_err(|err| MeError::Internal(err.into()))?;
        fs::create_dir_all(skill_dir.join("agents"))
            .map_err(|err| MeError::Internal(err.into()))?;
        atomic_write_preserving_user_section(
            &skill_dir.join("SKILL.md"),
            workspace_skill_md().as_bytes(),
        )?;
        atomic_write(
            &skill_dir.join("references/mental-model.md"),
            workspace_mental_model_md().as_bytes(),
        )?;
        atomic_write(
            &skill_dir.join("references/mutation-boundary.md"),
            workspace_mutation_boundary_md().as_bytes(),
        )?;
        atomic_write(
            &skill_dir.join("references/read-context.md"),
            workspace_read_context_md().as_bytes(),
        )?;
        atomic_write(
            &skill_dir.join("references/references-and-procedures.md"),
            workspace_references_and_procedures_md().as_bytes(),
        )?;
        atomic_write(
            &skill_dir.join("references/cli-contract.md"),
            workspace_cli_contract_md().as_bytes(),
        )?;
        for old in [
            "references/cognition-library.md",
            "references/app-analysis.md",
            "references/apps.md",
            "references/authorization.md",
            "references/associations.md",
        ] {
            let old_path = skill_dir.join(old);
            if old_path.exists() {
                fs::remove_file(old_path).map_err(|err| MeError::Internal(err.into()))?;
            }
        }
        atomic_write(
            &skill_dir.join("agents/openai.yaml"),
            b"interface:\n  display_name: \"ME\"\n  short_description: \"Capture thoughts and add cognitions.\"\n  default_prompt: \"Add this thought to ME.\"\n\npolicy:\n  allow_implicit_invocation: true\n",
        )?;
        Ok(())
    }

    fn object_file_paths(&self) -> Result<Vec<PathBuf>> {
        let mut paths = Vec::new();
        if !self.objects_dir().exists() {
            return Ok(paths);
        }
        for entry in WalkDir::new(self.objects_dir())
            .follow_links(false)
            .into_iter()
            .filter_map(std::result::Result::ok)
            .filter(|entry| entry.file_type().is_file())
        {
            paths.push(entry.path().to_path_buf());
        }
        paths.sort();
        Ok(paths)
    }

    fn object_hashes(&self) -> Result<Vec<Value>> {
        let mut objects = Vec::new();
        for path in self.object_file_paths()? {
            let bytes = fs::read(&path).map_err(|err| MeError::Internal(err.into()))?;
            let name = path
                .file_name()
                .and_then(|name| name.to_str())
                .ok_or_else(|| integrity(format!("Invalid object filename: {}", path.display())))?;
            objects.push(json!({ "hash": sha_ref(name), "bytes": bytes.len() }));
        }
        Ok(objects)
    }

    fn snapshot_chain(&self, current_hash: &str) -> Result<Vec<Value>> {
        let mut chain = Vec::new();
        let mut seen = BTreeSet::new();
        let mut hash = current_hash.to_string();
        loop {
            if !seen.insert(hash.clone()) {
                return Err(integrity("ME Snapshot parent chain contains a cycle"));
            }
            let snapshot = self.read_object::<MeSnapshotPayload>(&hash, "me-snapshot")?;
            let parent = snapshot.payload.parent.clone();
            chain.push(json!({
                "meSnapshot": hash,
                "parent": parent,
                "meTree": snapshot.payload.tree,
                "operation": snapshot.payload.operation,
                "actor": snapshot.payload.actor,
                "message": snapshot.payload.message,
                "createdAt": snapshot.payload.created_at
            }));
            if let Some(parent_hash) = parent {
                hash = parent_hash;
            } else {
                break;
            }
        }
        chain.reverse();
        Ok(chain)
    }

    fn runs_using_cognition(
        &self,
        tree: &MeTreePayload,
        cognition_hash: &str,
    ) -> Result<Vec<Value>> {
        let mut out = Vec::new();
        for (run_id, hash) in &tree.app_runs {
            let run = self.read_object::<AppRunPayload>(hash, "app-run")?;
            if run
                .payload
                .selected_cognitions
                .iter()
                .any(|selected| selected.cognition == cognition_hash)
            {
                out.push(json!({
                    "runId": run_id,
                    "run": hash,
                    "appId": run.payload.app_id,
                    "createdAt": run.payload.created_at
                }));
            }
        }
        Ok(out)
    }

    fn compute_retrieval_neighbors(
        &self,
        tree: &MeTreePayload,
        only: Option<&str>,
    ) -> Result<Vec<Value>> {
        let mut out = Vec::new();
        let mut cognitions = Vec::new();
        for (id, hash) in &tree.cognitions {
            let state = tree
                .cognition_states
                .get(id)
                .map(String::as_str)
                .unwrap_or("active");
            if state != "active" {
                continue;
            }
            let cognition = self.read_object::<CognitionPayload>(hash, "cognition")?;
            cognitions.push((id.clone(), hash.clone(), cognition.payload));
        }
        for (_id, hash, cognition) in &cognitions {
            if only.is_some_and(|target| target != hash) {
                continue;
            }
            let docs: Vec<_> = cognitions
                .iter()
                .filter(|(_, other_hash, _)| other_hash != hash)
                .map(|(other_id, other_hash, other)| CognitionDoc {
                    cognition_id: other_id.clone(),
                    cognition_hash: other_hash.clone(),
                    display_title: other.display_title.clone(),
                    body: other.body_text.clone(),
                    state: "active".to_string(),
                })
                .collect();
            for matched in rank_cognitions(&cognition.body_text, &docs, 3) {
                if matched.score <= 0.0 {
                    continue;
                }
                out.push(json!({
                    "sourceCognition": hash,
                    "targetCognition": matched.cognition,
                    "score": matched.score,
                    "matchedTerms": matched.matched_terms,
                    "status": "derived",
                    "label": "possibly relevant"
                }));
            }
        }
        out.sort_by(|a, b| {
            a["sourceCognition"]
                .as_str()
                .unwrap_or("")
                .cmp(b["sourceCognition"].as_str().unwrap_or(""))
                .then_with(|| {
                    a["targetCognition"]
                        .as_str()
                        .unwrap_or("")
                        .cmp(b["targetCognition"].as_str().unwrap_or(""))
                })
        });
        Ok(out)
    }

    fn write_retrieval_neighbors_to_conn(
        &self,
        conn: &Connection,
        neighbors: &[Value],
    ) -> Result<()> {
        let generated_at = now_rfc3339()?;
        for item in neighbors {
            conn.execute(
                "INSERT OR REPLACE INTO retrieval_neighbors VALUES (?1, ?2, ?3, ?4, ?5)",
                params![
                    item["sourceCognition"].as_str(),
                    item["targetCognition"].as_str(),
                    item["score"].as_f64().unwrap_or_default(),
                    serde_json::to_string(item["matchedTerms"].as_array().unwrap_or(&Vec::new()))
                        .unwrap(),
                    generated_at
                ],
            )
            .map_err(|err| MeError::Internal(err.into()))?;
        }
        Ok(())
    }

    fn read_retrieval_neighbors(&self) -> Result<Vec<Value>> {
        let path = self.root.join(".me/index.sqlite");
        if !path.exists() {
            return Ok(Vec::new());
        }
        let conn = Connection::open(path).map_err(|err| MeError::Internal(err.into()))?;
        let mut stmt = conn
            .prepare("SELECT source_cognition, target_cognition, score, matched_terms, generated_at FROM retrieval_neighbors ORDER BY source_cognition, target_cognition")
            .map_err(|err| MeError::Internal(err.into()))?;
        let rows = stmt
            .query_map([], |row| {
                let matched_terms: String = row.get(3)?;
                Ok(json!({
                    "sourceCognition": row.get::<_, String>(0)?,
                    "targetCognition": row.get::<_, String>(1)?,
                    "score": row.get::<_, f64>(2)?,
                    "matchedTerms": serde_json::from_str::<Value>(&matched_terms).unwrap_or_else(|_| json!([])),
                    "generatedAt": row.get::<_, String>(4)?,
                    "status": "derived",
                    "label": "possibly relevant"
                }))
            })
            .map_err(|err| MeError::Internal(err.into()))?;
        let mut out = Vec::new();
        for row in rows {
            out.push(row.map_err(|err| MeError::Internal(err.into()))?);
        }
        Ok(out)
    }

    fn retrieval_neighbors_for_cognition(&self, cognition_hash: &str) -> Result<Vec<Value>> {
        Ok(self
            .read_retrieval_neighbors()?
            .into_iter()
            .filter(|item| {
                item["sourceCognition"].as_str() == Some(cognition_hash)
                    || item["targetCognition"].as_str() == Some(cognition_hash)
            })
            .collect())
    }

    fn write_builtin_app_packages(&self) -> Result<()> {
        write_builtin_app(
            &self.root.join("apps/inspect-me"),
            "inspect-me",
            "Inspect ME",
            "Retrieve and present cognitions relevant to a question.",
            "reading",
        )?;
        write_builtin_app(
            &self.root.join("apps/speak-for-me"),
            "speak-for-me",
            "Speak for Me",
            "Draft communication grounded in selected cognitions.",
            "draft",
        )?;
        Ok(())
    }

    fn write_builtin_procedures(&self) -> Result<()> {
        let procedures = [
            (
                "inspect-me.md",
                "# Inspect ME\n\nUse `me search`, `me context`, and `me cognition list/show/history` to inspect authorized cognitions. Treat retrieval as bounded context, not a complete model of the user.\n",
            ),
            (
                "speak-from-me.md",
                "# Speak From ME\n\n1. Pass the transient task through standard input.\n2. Run `me context --stdin --json`.\n3. Draft with the selected cognitions as user-authorized context.\n4. Clearly separate ME cognitions from Codex inference.\n5. Do not save the draft into ME unless the user brings an exact excerpt back as a thought and approves a Decision.\n",
            ),
        ];
        for (name, body) in procedures {
            atomic_write_preserving_user_section(
                &self.root.join("procedures").join(name),
                body.as_bytes(),
            )?;
        }
        Ok(())
    }

    fn write_builtin_app_definitions(&self, tree: &mut MeTreePayload) -> Result<Vec<Value>> {
        let mut out = Vec::new();
        for app_id in ["inspect-me", "speak-for-me"] {
            let manifest = read_app_manifest(&self.root.join("apps").join(app_id))?;
            let payload = AppDefinitionPayload {
                app_id: app_id.to_string(),
                name: manifest
                    .get("name")
                    .and_then(Value::as_str)
                    .unwrap_or(app_id)
                    .to_string(),
                version: manifest
                    .get("version")
                    .and_then(Value::as_str)
                    .unwrap_or("0.1.0")
                    .to_string(),
                manifest_hash: file_hash(&self.root.join("apps").join(app_id).join("app.toml"))?,
                installed_at: now_rfc3339()?,
            };
            let hash = self.write_object("app-definition", &payload)?;
            tree.apps.insert(app_id.to_string(), hash.clone());
            out.push(json!({ "appId": app_id, "app": hash }));
        }
        Ok(out)
    }

    fn app_definition(
        &self,
        tree: &MeTreePayload,
        app_id: &str,
    ) -> Result<ObjectEnvelope<AppDefinitionPayload>> {
        let hash = tree
            .apps
            .get(app_id)
            .ok_or_else(|| not_found(format!("ME App not found: {app_id}")))?;
        self.read_object::<AppDefinitionPayload>(hash, "app-definition")
    }

    fn app_manifest_json(&self, app_id: &str) -> Result<Value> {
        read_app_manifest(&self.root.join("apps").join(app_id))
    }

    fn select_cognitions_for_task(
        &self,
        tree: &MeTreePayload,
        task: &str,
        limit: usize,
    ) -> Result<Vec<SelectedCognition>> {
        let matches = self.match_thought(tree, task, limit)?;
        let mut selected = Vec::new();
        for matched in matches {
            selected.push(SelectedCognition {
                cognition: matched.cognition,
                cognition_id: matched.cognition_id,
                selection_reason: format!(
                    "Matched task terms: {}",
                    matched.matched_terms.join(", ")
                ),
            });
        }
        Ok(selected)
    }

    fn build_app_findings(
        &self,
        tree: &MeTreePayload,
        app_id: &str,
        task: &str,
        selected: &[SelectedCognition],
    ) -> Result<Vec<AppFinding>> {
        let mut findings = Vec::new();
        for item in selected {
            let (_, _, cognition) = self.resolve_cognition(tree, &item.cognition)?;
            findings.push(AppFinding {
                label: if app_id == "inspect-me" {
                    "similar".to_string()
                } else {
                    "supports".to_string()
                },
                cognitions: vec![item.cognition.clone()],
                passages: vec![cognition.payload.body_markdown.clone()],
                reason_markdown: format!(
                    "For this task, this Cognition was selected because: {}. Task: {}",
                    item.selection_reason,
                    task.trim()
                ),
                app_rule: Some(format!("{app_id}: task-scoped retrieval")),
            });
        }
        if selected.len() > 1 && task.to_ascii_lowercase().contains("artwork") {
            findings.push(AppFinding {
                label: "unclear".to_string(),
                cognitions: selected.iter().map(|item| item.cognition.clone()).collect(),
                passages: Vec::new(),
                reason_markdown: "Multiple Cognitions are relevant for this task; ME does not synthesize one global position.".to_string(),
                app_rule: Some(format!("{app_id}: preserve task-scoped uncertainty")),
            });
        }
        Ok(findings)
    }
}

fn home_markdown(data: &Value) -> String {
    if data["workspaceState"].as_str() == Some("empty") {
        return welcome_markdown("empty").to_string();
    }

    let active = data["summary"]["activeCognitionCount"]
        .as_u64()
        .unwrap_or(0);
    let pending = data["summary"]["pendingThoughtCount"].as_u64().unwrap_or(0);
    let cognition_word = if active == 1 {
        "cognition"
    } else {
        "cognitions"
    };
    let pending_line = if pending == 0 {
        String::new()
    } else if pending == 1 {
        "A thought is waiting for your decision.\n\n".to_string()
    } else {
        format!("{pending} thoughts are waiting for your decision.\n\n")
    };
    format!(
        r#"ME

{pending_line}You have {active} {cognition_word}.

Add another:

  Add this thought to ME:

Use what you have kept:

  What do I have in ME about <topic>?

  Draft <something> using ME.
"#
    )
}

fn welcome_markdown(state: &str) -> &'static str {
    if state == "empty" {
        r#"ME

ME keeps thoughts you choose.

Start with one:

  Add this thought to ME:
"#
    } else {
        r#"ME

ME is ready.

Add another:

  Add this thought to ME:

Or use what you have kept:

  What do I have in ME about <topic>?

  Draft <something> using ME.
"#
    }
}

fn workspace_created_result(root: &Path) -> Value {
    json!({
        "workspacePath": root,
        "next": {
            "command": format!("me start --workspace {}", root.display()),
            "host": "Codex App",
            "mode": "Local",
            "starterPrompt": "Start ME"
        }
    })
}

fn mental_model_steps() -> Vec<Value> {
    vec![
        json!({
            "step": "collect",
            "title": "When a thought occurs",
            "description": "Tell Codex and choose whether to keep it as a cognition."
        }),
        json!({
            "step": "use",
            "title": "Use what you have kept",
            "description": "Ask Codex to inspect, compare, or compose from your cognitions."
        }),
        json!({
            "step": "return",
            "title": "Keep something from output",
            "description": "Bring it back as a new thought."
        }),
    ]
}

fn guide_markdown() -> &'static str {
    r#"ME GUIDE

SCENARIO 1: A THOUGHT OCCURS

Suppose you think:

  Designing a generative system is part of authorship.

Tell Codex:

  Add this thought to ME:
  Designing a generative system is part of authorship.

ME captures the exact text first. It is not in ME yet.

SCENARIO 2: KEEP THE THOUGHT

Codex asks whether to keep it.

After you approve, ME adds it to the local Cognition Library.

In ME, a thought you choose to keep is called a cognition.

SCENARIO 3: USE A COGNITION

Ask:

  What do I have in ME about authorship?

or:

  Draft a short statement using ME.

Codex may read and compose from the cognition.

Reading and composing do not change ME.

SCENARIO 4: KEEP SOMETHING CODEX PRODUCED

If Codex writes a sentence worth retaining, say:

  This is my thought. Add it to ME.

The sentence returns through the same capture and keep flow.
"#
}

fn thought_capture_markdown(body: &str) -> String {
    format!(
        "THOUGHT\n\n{}\n\nThis thought is captured, but it is not in ME yet.\n\nKeep it?\n",
        quote_exact(body)
    )
}

fn first_cognition_markdown(body: &str, topic: &str) -> String {
    format!(
        "KEPT IN ME\n\n{}\n\nIn ME, a thought you choose to keep is called a cognition.\n\nCodex can now use it without changing ME.\n\nTry:\n\n  What do I have in ME about {topic}?\n\n  Draft a short statement using ME.\n\nOr add another thought:\n\n  Add this thought to ME:\n",
        quote_exact(body)
    )
}

fn later_cognition_markdown(body: &str) -> String {
    format!(
        "KEPT IN ME\n\n{}\n\nME now has this as another cognition.\n\nAdd another thought, or ask Codex to use what you have kept.\n",
        quote_exact(body)
    )
}

fn first_read_guidance_markdown() -> &'static str {
    "ME was read, not changed.\n\nIf this output contains something worth keeping, say:\n\n  This is my thought. Add it to ME.\n"
}

fn two_cognition_guidance_markdown(topic: &str) -> String {
    format!(
        "ME now contains more than one cognition.\n\nCodex can compare them without changing ME.\n\nTry:\n\n  Compare what I have in ME about {topic}.\n\n  Find tension in ME about {topic}.\n"
    )
}

fn five_cognition_guidance_markdown() -> &'static str {
    "ME now has enough material to explore broader patterns.\n\nTry:\n\n  What themes recur in ME?\n\n  Draft a longer piece using ME.\n"
}

fn quote_exact(body: &str) -> String {
    format!("“{}”", body.trim_end_matches('\n'))
}

fn infer_topic_phrase(markdown: &str) -> Option<String> {
    let text = markdown_to_text(markdown);
    let mut run = Vec::new();
    for raw in text.split_whitespace() {
        let word = raw.trim_matches(|ch: char| !ch.is_alphanumeric());
        if word.is_empty() {
            continue;
        }
        let starts_upper = word.chars().next().is_some_and(|ch| ch.is_uppercase());
        let skip = matches!(
            word,
            "A" | "An" | "The" | "This" | "That" | "I" | "ME" | "Codex"
        );
        if starts_upper && !skip {
            run.push(word.to_string());
            if run.len() >= 2 {
                return Some(run.join(" "));
            }
        } else {
            run.clear();
        }
    }
    None
}

fn preview_text(markdown: &str) -> String {
    let mut preview = markdown_to_text(markdown)
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");
    if preview.chars().count() > 80 {
        preview = preview.chars().take(77).collect::<String>();
        preview.push_str("...");
    }
    preview
}

fn format_review(
    proposal_hash: &str,
    proposal: &me_core::ProposalPayload,
    thought_body: &str,
) -> String {
    let related = if proposal.related_cognitions.is_empty() {
        "None found.".to_string()
    } else {
        proposal
            .related_cognitions
            .iter()
            .enumerate()
            .map(|(idx, related)| {
                format!(
                    "{}. {} ({:.0}%) -- derived retrieval hint",
                    idx + 1,
                    related.cognition_id,
                    related.score * 100.0
                )
            })
            .collect::<Vec<_>>()
            .join("\n")
    };
    format!(
        r#"STATUS
PENDING -- NOT IN ME

Proposal: {proposal_hash}
Proposal ID: {}

THOUGHT
{}

OPTIONAL SIMILAR COGNITIONS
{related}

SUGGESTED EFFECT
Add this thought to ME as its own cognition.

DERIVED ONLY
Similarity hints are not authorized facts and create no global relationship.

WHAT WILL CHANGE
1 cognition added.
0 existing cognitions changed.
"#,
        proposal.proposal_id,
        thought_body.trim()
    )
}

fn thought_status_label(state: &str) -> &'static str {
    match state {
        "pending" => "THOUGHT -- pending, not in ME",
        "added" => "THOUGHT -- added to ME",
        "kept-only" => "THOUGHT -- kept only",
        "dismissed" => "THOUGHT -- dismissed",
        _ => "THOUGHT -- captured",
    }
}

fn infer_conflicts_from_selected(selected: &[SelectedCognition]) -> Vec<String> {
    let has_final_only = selected.iter().any(|item| {
        item.selection_reason.contains("final") || item.selection_reason.contains("only")
    });
    if has_final_only && selected.len() > 1 {
        vec!["Selected Cognitions may contain tension; ME preserves it.".to_string()]
    } else {
        Vec::new()
    }
}

fn build_app_output(
    app_id: &str,
    task: &str,
    selected: &[SelectedCognition],
    conflicts: &[String],
    gaps: &[String],
) -> AppRunOutput {
    let used = selected
        .iter()
        .map(|item| item.cognition_id.as_str())
        .collect::<Vec<_>>()
        .join(", ");
    let conflict_text = if conflicts.is_empty() {
        "No explicit conflict detected.".to_string()
    } else {
        conflicts.join("\n")
    };
    let gap_text = if gaps.is_empty() {
        "No major gap detected in selected Cognitions.".to_string()
    } else {
        gaps.join("\n")
    };
    let (kind, body) = if app_id == "speak-for-me" {
        (
            "draft",
            format!(
                "Draft grounded in selected Cognitions ({used}):\n\n{}\n\nConflicts considered:\n{conflict_text}\n\nGaps:\n{gap_text}\n\nStatus: LOCAL DRAFT -- NOT SENT",
                task.trim()
            ),
        )
    } else {
        (
            "reading",
            format!(
                "Reading for task:\n{}\n\nCognitions used: {used}\n\nConflicts:\n{conflict_text}\n\nGaps:\n{gap_text}\n\nStatus: LOCAL READING -- NOT SENT",
                task.trim()
            ),
        )
    };
    AppRunOutput {
        kind: kind.to_string(),
        body_markdown: body,
        external_action: false,
    }
}

fn app_run_markdown(run: &AppRunPayload) -> String {
    let used = run
        .selected_cognitions
        .iter()
        .map(|item| format!("  {} -- {}", item.cognition_id, item.selection_reason))
        .collect::<Vec<_>>()
        .join("\n");
    let conflicts = if run.analysis.conflicts.is_empty() {
        "None".to_string()
    } else {
        run.analysis.conflicts.join("\n  ")
    };
    let gaps = if run.analysis.gaps.is_empty() {
        "None".to_string()
    } else {
        run.analysis.gaps.join("\n  ")
    };
    format!(
        r#"# ME App Run

App
  {} {}

Task
  {}

Cognitions used
{}

Conflicts
  {}

Gaps
  {}

Output

{}

Status
  LOCAL {} -- NOT SENT
"#,
        run.app_id,
        run.app_version,
        run.task_markdown.trim(),
        used,
        conflicts,
        gaps,
        run.output.body_markdown,
        run.output.kind.to_ascii_uppercase()
    )
}

fn changed_keys(a: &BTreeMap<String, String>, b: &BTreeMap<String, String>) -> Vec<String> {
    a.keys()
        .chain(b.keys())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .filter(|key| a.get(*key) != b.get(*key))
        .cloned()
        .collect()
}

fn proposal_thought_hash(proposal: &me_core::ProposalPayload) -> Option<&str> {
    proposal.inputs.get("thought").and_then(Value::as_str)
}

fn association_removed_message() -> Value {
    legacy_command_message("association")
}

fn legacy_command_message(command: &str) -> Value {
    json!({
        "statusLabel": "EARLIER ME COMMAND -- compatibility only",
        "command": command,
        "message": "This command belonged to an earlier experimental ME schema. ME now provides canonical Cognitions and read context. Use Codex directly for task-specific analysis and composition.",
        "cognitionLibraryChanged": false,
        "objectsCreated": 0
    })
}

fn string_field<'a>(value: &'a Value, field: &str) -> Result<&'a str> {
    value
        .get(field)
        .and_then(Value::as_str)
        .ok_or_else(|| invalid(format!("Missing string field: {field}")))
}

fn validate_base_snapshot(value: &Value, current_snapshot: &str) -> Result<()> {
    if let Some(base_snapshot) = value.get("baseSnapshot").and_then(Value::as_str) {
        if base_snapshot != current_snapshot {
            return Err(invalid(format!(
                "Decision baseSnapshot {base_snapshot} does not match current Snapshot {current_snapshot}"
            )));
        }
    }
    Ok(())
}

fn safe_file_stem(value: &str) -> String {
    let mut out = String::new();
    for ch in value.chars() {
        if ch.is_ascii_alphanumeric() || ch == '-' || ch == '_' {
            out.push(ch);
        } else if !out.ends_with('-') {
            out.push('-');
        }
    }
    let trimmed = out.trim_matches('-');
    if trimmed.is_empty() {
        "export".to_string()
    } else {
        trimmed.to_string()
    }
}

fn string_array(value: Option<&Value>) -> Option<Vec<String>> {
    value.and_then(Value::as_array).map(|items| {
        items
            .iter()
            .filter_map(Value::as_str)
            .map(str::to_string)
            .collect()
    })
}

fn canonical_json_bytes<T: Serialize>(value: &T) -> Result<Vec<u8>> {
    let value = serde_json::to_value(value).map_err(|err| MeError::Internal(err.into()))?;
    let canonical = canonicalize_value(value);
    serde_json::to_vec(&canonical).map_err(|err| MeError::Internal(err.into()))
}

fn canonicalize_value(value: Value) -> Value {
    match value {
        Value::Object(map) => {
            let mut sorted = serde_json::Map::new();
            let mut entries: Vec<_> = map.into_iter().collect();
            entries.sort_by(|a, b| a.0.cmp(&b.0));
            for (key, value) in entries {
                sorted.insert(key, canonicalize_value(value));
            }
            Value::Object(sorted)
        }
        Value::Array(values) => Value::Array(values.into_iter().map(canonicalize_value).collect()),
        other => other,
    }
}

fn hex_digest(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    format!("{:x}", hasher.finalize())
}

fn file_hash(path: &Path) -> Result<String> {
    let bytes = fs::read(path).map_err(|err| MeError::Internal(err.into()))?;
    Ok(sha_ref(&hex_digest(&bytes)))
}

fn atomic_write(path: &Path, bytes: &[u8]) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|err| MeError::Internal(err.into()))?;
    }
    let dir = path.parent().unwrap_or_else(|| Path::new("."));
    let mut tmp = NamedTempFile::new_in(dir).map_err(|err| MeError::Internal(err.into()))?;
    tmp.write_all(bytes)
        .map_err(|err| MeError::Internal(err.into()))?;
    tmp.as_file()
        .sync_all()
        .map_err(|err| MeError::Internal(err.into()))?;
    tmp.persist(path)
        .map_err(|err| MeError::Internal(anyhow::anyhow!(err)))?;
    Ok(())
}

fn atomic_write_preserving_user_section(path: &Path, bytes: &[u8]) -> Result<()> {
    let new_text =
        String::from_utf8(bytes.to_vec()).map_err(|err| MeError::Internal(err.into()))?;
    let old = fs::read_to_string(path).unwrap_or_default();
    let user_section = extract_user_section(&old);
    let output = if let Some(section) = user_section {
        format!("{new_text}\n<!-- ME:USER-BEGIN -->\n{section}\n<!-- ME:USER-END -->\n")
    } else {
        format!("{new_text}\n<!-- ME:USER-BEGIN -->\n<!-- ME:USER-END -->\n")
    };
    atomic_write(path, output.as_bytes())
}

fn extract_user_section(input: &str) -> Option<String> {
    for (start, end) in [
        ("<!-- ME:USER-BEGIN -->", "<!-- ME:USER-END -->"),
        ("<!-- MYMODEL:USER-BEGIN -->", "<!-- MYMODEL:USER-END -->"),
    ] {
        let Some(start_idx) = input.find(start).map(|idx| idx + start.len()) else {
            continue;
        };
        let Some(end_idx) = input[start_idx..].find(end).map(|idx| start_idx + idx) else {
            continue;
        };
        return Some(input[start_idx..end_idx].trim().to_string());
    }
    None
}

fn lock_file(path: &Path, label: &str) -> Result<File> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|err| MeError::Internal(err.into()))?;
    }
    let file = OpenOptions::new()
        .create(true)
        .read(true)
        .write(true)
        .truncate(false)
        .open(path)
        .map_err(|err| MeError::Internal(err.into()))?;
    file.try_lock_exclusive()
        .map_err(|_| MeError::WorkspaceLocked {
            code: "WORKSPACE_LOCKED",
            message: format!("{label} workspace is locked"),
            details: json!({ "lock": path }),
        })?;
    Ok(file)
}

fn append_bytes_to_tar(
    builder: &mut Builder<File>,
    path: impl AsRef<Path>,
    bytes: Vec<u8>,
) -> Result<()> {
    let mut header = Header::new_gnu();
    header.set_size(bytes.len() as u64);
    header.set_cksum();
    builder
        .append_data(&mut header, path, bytes.as_slice())
        .map_err(|err| MeError::Internal(err.into()))
}

fn append_file_to_tar(
    builder: &mut Builder<File>,
    tar_path: impl AsRef<Path>,
    file_path: &Path,
) -> Result<()> {
    let mut file = File::open(file_path).map_err(|err| MeError::Internal(err.into()))?;
    builder
        .append_file(tar_path, &mut file)
        .map_err(|err| MeError::Internal(err.into()))
}

#[derive(Debug)]
struct VerifiedBundle {
    current: String,
    objects: usize,
}

fn verify_bundle(file: &Path) -> Result<VerifiedBundle> {
    let archive_file = File::open(file).map_err(|err| MeError::Internal(err.into()))?;
    let mut archive = Archive::new(archive_file);
    let mut current = None;
    let mut objects = 0usize;
    for entry in archive
        .entries()
        .map_err(|err| MeError::Internal(err.into()))?
    {
        let mut entry = entry.map_err(|err| MeError::Internal(err.into()))?;
        let path = entry
            .path()
            .map_err(|err| MeError::Internal(err.into()))?
            .to_path_buf();
        validate_archive_path(&path)?;
        if path == Path::new("manifest.json") {
            let mut raw = String::new();
            entry
                .read_to_string(&mut raw)
                .map_err(|err| MeError::Internal(err.into()))?;
            let manifest: Value =
                serde_json::from_str(&raw).map_err(|err| MeError::Internal(err.into()))?;
            current = manifest
                .get("current")
                .and_then(Value::as_str)
                .map(str::to_string);
            objects = manifest
                .get("objects")
                .and_then(Value::as_array)
                .map_or(0, Vec::len);
        }
    }
    let current = current.ok_or_else(|| integrity("Bundle missing manifest current"))?;
    Ok(VerifiedBundle { current, objects })
}

fn validate_archive_path(path: &Path) -> Result<()> {
    if path.components().any(|component| {
        matches!(
            component,
            Component::ParentDir | Component::RootDir | Component::Prefix(_)
        )
    }) {
        return Err(invalid(format!("Unsafe archive path: {}", path.display())));
    }
    Ok(())
}

fn bundle_path_to_workspace_path(root: &Path, path: &Path) -> Result<PathBuf> {
    validate_archive_path(path)?;
    if path == Path::new("manifest.json") {
        Ok(root.join(".me/migrations/restored-bundle-manifest.json"))
    } else if path == Path::new("me.toml") {
        Ok(root.join("me.toml"))
    } else if path == Path::new("refs/current") {
        Ok(root.join(".me/refs/current"))
    } else if let Ok(rel) = path.strip_prefix("objects") {
        Ok(root.join(".me/objects").join(rel))
    } else if let Ok(rel) = path.strip_prefix("journal") {
        Ok(root.join(".me/journal").join(rel))
    } else {
        Err(invalid(format!(
            "Unsupported bundle path: {}",
            path.display()
        )))
    }
}

fn cognition_file_name(cognition_id: &str) -> String {
    format!("{}.md", cognition_id.replace([':', '/'], "--"))
}

fn contains_approval_fields(value: &Value) -> bool {
    let raw = value.to_string();
    raw.contains("decisionId") || raw.contains("decidedAt") || raw.contains("approved")
}

fn workspace_agents_md() -> &'static str {
    r#"# ME Workspace

ME is a local application operated through Codex App.

## Everyday Use

- When the user has a thought, preserve the exact words.
- Add it to ME only after the user approves keeping it.
- Treat casual add, capture, save, note, remember, or put-in-ME wording as thought capture only.
- Create a cognition only after a separate explicit keep decision for the captured thought.
- For exact `Start ME`, call `me welcome --json` and output `renderedMarkdown` verbatim.
- For a simple empty-workspace greeting, call `me welcome --json` and reply with `Hi. ME is ready.` plus `Add this thought to ME:`.
- Use `me welcome --json` for "What can I do here?" and present the canonical welcome.
- Use `me context` when the user asks Codex to inspect, compare, or compose from ME.
- Reading and composition do not change ME.
- Codex output never enters ME automatically.
- For welcome and greetings, do not use memory, create files, call `me context`, attach files, or expose maintenance commands.

## Technical Rules

- This directory is a ME workspace, not a software repository.
- Use `me ... --json` for deterministic operations.
- Never edit `.me/**` directly.
- Never edit `views/**` directly.
- Do not use host memory as Current ME.
- Do not bulk-import References as cognitions.
- Do not treat Procedures as cognitions.
- App, Run, Association, Proposal, and Synthesis commands are legacy compatibility only.
- Publishing, external sharing, and network access are outside ME.
"#
}

fn workspace_skill_md() -> &'static str {
    r#"---
name: me
description: Use the local ME Cognition Library as trustworthy user-authorized context, or change it through the explicit thought capture and keep flow. Use when the user asks what ME contains, wants a task grounded in retained cognitions, or asks to capture a thought.
---

# ME Skill

## Mode Selection

General task: do not use ME unless requested or clearly relevant.

Use ME: call read-only ME commands.

Change ME: capture the thought first, then require a separate keep decision before creating a cognition.

## Start ME

For exact or near-exact "Start ME", "start me", or "Help me start ME":

1. Call `me welcome --json`.
2. Read `renderedMarkdown`.
3. Output `renderedMarkdown` verbatim.
4. Do not use memory.
5. Do not call `me context`.
6. Do not create files.
7. Do not attach files.
8. Do not mention commands.
9. Do not explain cognition yet when `state` is `empty`.

## Welcome And Greetings

For "What can I do here?", "How does ME work?", "How do I use ME?", "Help me start", "What is this?", or "Show me around", call `me welcome --json` and render `renderedMarkdown`.

For simple greetings like "hi", "hello", "hey", "start", or "start ME", call `me welcome --json`. If `state` is `empty`, reply exactly:

Hi. ME is ready.

Add this thought to ME:

If `state` is `established`, reply exactly:

Hi. ME is ready.

Add another thought, or ask Codex to use what you have kept.

Welcome behavior must use one command: `me welcome --json`. Do not use memory, do not call `me context`, do not create files, do not attach files, and do not expose maintenance commands.

## Use ME

1. Verify workspace with `me current --json`.
2. Prefer stdin for transient tasks: `me context --stdin --json`.
3. Use `me context --task <file> --json` only when the user intentionally provided a file.
4. Use selected cognitions as user-authorized context.
5. Clearly distinguish ME cognitions from Codex inference.
6. Do not mutate ME.
7. Do not claim the context is a complete representation of the user.
8. If `guidance.kind` is `first-read`, append `guidance.renderedMarkdown` once after the answer.

## Change ME

1. Preserve exact selected thought text.
2. Prefer stdin for transient text: `me thought capture --stdin --kind <kind> --json`.
3. Show the exact text, say it is not in ME yet, and ask whether to keep it.
4. Do not mention Decision files, canonical mutation, transaction internals, snapshots, or hashes.
5. Treat casual add, capture, save, note, remember, or put-in-ME wording as thought capture only.
6. A captured thought is not a cognition and is not in ME yet.
7. A cognition can be created only after the user explicitly approves keeping that captured thought.
8. Approval must be a separate keep decision after Codex has shown the captured THOUGHT and asked whether to keep it.
9. Do not infer approval from the same message that supplied the thought text.
10. If the user only supplied the thought, stop after capture and wait for the keep decision.
11. Prepare a Decision JSON with `baseSnapshot`, `action`, `actor`, `approved: true`, and exact final body only after that keep decision.
12. The engine rejects `me cognition add` unless the Decision includes `approved: true`.
13. Prefer stdin for transient Decisions: `me cognition add --thought <thought-id> --decision-stdin --json`.
14. After the first successful add, teach: "In ME, a thought you choose to keep is called a cognition."
15. State that Codex can use it without changing ME.
16. Suggest up to two use prompts and one add prompt from `nextGuidance`.
17. For later additions, use the brief `renderedMarkdown` success copy.
18. Hide technical fields unless the user asks for technical status.

## Feedback

When the user says "This sentence is my thought", "Add this part to ME", or "Keep this from the draft", re-enter the normal thought flow. Codex output never enters ME automatically.

## Technical

Only expose CLI and integrity details when asked for technical status, integrity, snapshots, backup, or CLI help.

## Forbidden

Do not edit `.me/**` directly.
Do not use Codex memory as Current ME.
Do not use Codex memory to determine workspace counts, whether this is the first cognition, whether guidance was shown, or canonical state.
Do not bulk-import References as cognitions.
Do not treat Procedures as cognitions.
Do not save Codex output into ME automatically.
Do not invent relationship objects.
Do not force synthesis.
Do not show snapshots, fsck, bundle, index, or other maintenance details unless the user asks for technical status.
"#
}

fn workspace_mental_model_md() -> &'static str {
    "A thought is something I tell ME.\n\nA cognition is a thought I choose to keep.\n\nME stores what I chose to keep. Codex can use my cognitions without changing ME.\n"
}

fn workspace_mutation_boundary_md() -> &'static str {
    "ME Core validates and commits canonical state. Codex can suggest, draft, and prepare inputs, but model output is never authority. Approval requires an explicit Decision.\n"
}

fn workspace_read_context_md() -> &'static str {
    "Use `me context --stdin --json` for transient tasks, or `me context --task <file> --json` when the user intentionally provided a file. Search and context are read-only and must not advance the current Snapshot.\n"
}

fn workspace_references_and_procedures_md() -> &'static str {
    "References and Procedures are ordinary local files available to Codex. They are not Cognitions, not Decisions, and not canonical ME state.\n"
}

fn workspace_cli_contract_md() -> &'static str {
    r#"# CLI Contract

Use `--json` for agent operations.

```bash
me home --json
me thought capture --stdin --kind idea --json
me cognition add --thought <thought-id> --decision-stdin --json
me search "authorship" --limit 20 --json
me context --stdin --limit 20 --json
me cognition history <cognition-id> --json
```

`me cognition add` requires `approved: true` in the Decision JSON and
should run only after the user explicitly approves keeping the captured
thought.
"#
}

fn write_builtin_app(
    dir: &Path,
    app_id: &str,
    name: &str,
    purpose: &str,
    output_kind: &str,
) -> Result<()> {
    fs::create_dir_all(dir.join("fixtures")).map_err(|err| MeError::Internal(err.into()))?;
    let manifest = format!(
        r#"schema_version = 1
app_id = "{app_id}"
name = "{name}"
version = "0.1.0"

purpose = "{purpose}"
scope = "local cognition library"

[retrieval]
max_cognitions = 20
include_derived_neighbors = true
include_task_findings = true
include_retired = false
include_conflicts = true

[behavior]
state_uncertainty = true
invent_positions = false
preserve_conflicts = true
cite_cognition_ids_in_run_record = true

[actions]
draft = true
save_output = true
send = false
publish = false
mutate_cognition_library = false

[approval]
require_before_external_action = true
"#
    );
    atomic_write(&dir.join("app.toml"), manifest.as_bytes())?;
    atomic_write(
        &dir.join("instructions.md"),
        format!(
            "{name}\n\n{purpose}\n\nOutput kind: {output_kind}. External action is forbidden.\n"
        )
        .as_bytes(),
    )?;
    atomic_write(
        &dir.join("output.schema.json"),
        br#"{"type":"object","required":["kind","bodyMarkdown","externalAction"],"properties":{"kind":{"type":"string"},"bodyMarkdown":{"type":"string"},"externalAction":{"const":false}}}"#,
    )
}

fn read_app_manifest(app_directory: &Path) -> Result<Value> {
    let raw = fs::read_to_string(app_directory.join("app.toml"))
        .map_err(|err| MeError::Internal(err.into()))?;
    let toml_value: toml::Value =
        toml::from_str(&raw).map_err(|err| MeError::Internal(err.into()))?;
    serde_json::to_value(toml_value).map_err(|err| MeError::Internal(err.into()))
}

fn validate_app_manifest(manifest: &Value) -> Result<()> {
    if manifest
        .get("app_id")
        .and_then(Value::as_str)
        .unwrap_or("")
        .trim()
        .is_empty()
    {
        return Err(invalid("App manifest missing app_id"));
    }
    if manifest
        .pointer("/actions/send")
        .and_then(Value::as_bool)
        .unwrap_or(false)
        || manifest
            .pointer("/actions/publish")
            .and_then(Value::as_bool)
            .unwrap_or(false)
    {
        return Err(invalid("ME v0.4 Apps cannot send or publish"));
    }
    Ok(())
}

fn copy_dir_replace(from: &Path, to: &Path) -> Result<()> {
    if to.exists() {
        fs::remove_dir_all(to).map_err(|err| MeError::Internal(err.into()))?;
    }
    fs::create_dir_all(to).map_err(|err| MeError::Internal(err.into()))?;
    for entry in WalkDir::new(from)
        .follow_links(false)
        .into_iter()
        .filter_map(std::result::Result::ok)
    {
        let rel = entry
            .path()
            .strip_prefix(from)
            .map_err(|err| MeError::Internal(err.into()))?;
        if rel.as_os_str().is_empty() {
            continue;
        }
        let dest = to.join(rel);
        if entry.file_type().is_dir() {
            fs::create_dir_all(dest).map_err(|err| MeError::Internal(err.into()))?;
        } else if entry.file_type().is_file() {
            fs::copy(entry.path(), dest).map_err(|err| MeError::Internal(err.into()))?;
        }
    }
    Ok(())
}

fn me_object_raw(root: &Path, hash: &str, expected_type: &str) -> Result<Value> {
    let hex = strip_sha_prefix(hash).ok_or_else(|| invalid(format!("Invalid ME hash: {hash}")))?;
    let path = root.join(".me/objects").join(&hex[0..2]).join(hex);
    let mut bytes = fs::read(&path).map_err(|err| MeError::Internal(err.into()))?;
    if bytes.ends_with(b"\n") {
        bytes.pop();
    }
    let digest = sha_ref(&hex_digest(&bytes));
    if digest != hash {
        return Err(integrity(format!("ME object hash mismatch for {hash}")));
    }
    let value: Value =
        serde_json::from_slice(&bytes).map_err(|err| MeError::Internal(err.into()))?;
    if value["objectType"].as_str() != Some(expected_type) {
        return Err(integrity(format!(
            "ME object {hash} expected {expected_type}"
        )));
    }
    Ok(value)
}

fn legacy_read_object(root: &Path, hash: &str, expected_type: &str) -> Result<Value> {
    let hex =
        strip_sha_prefix(hash).ok_or_else(|| invalid(format!("Invalid legacy hash: {hash}")))?;
    let path = root.join(".my-model/objects").join(&hex[0..2]).join(hex);
    let mut bytes = fs::read(&path).map_err(|err| MeError::Internal(err.into()))?;
    if bytes.ends_with(b"\n") {
        bytes.pop();
    }
    let digest = sha_ref(&hex_digest(&bytes));
    if digest != hash {
        return Err(integrity(format!("Legacy object hash mismatch for {hash}")));
    }
    let value: Value =
        serde_json::from_slice(&bytes).map_err(|err| MeError::Internal(err.into()))?;
    if value["objectType"].as_str() != Some(expected_type) {
        return Err(integrity(format!(
            "Legacy object {hash} expected {expected_type}"
        )));
    }
    Ok(value)
}

fn legacy_cognition_chain(root: &Path, current_hash: &str) -> Result<Vec<(String, Value)>> {
    let mut chain = Vec::new();
    let mut hash = current_hash.to_string();
    loop {
        let object = legacy_read_object(root, &hash, "cognition-revision")?;
        let payload = object["payload"].clone();
        let parent = payload["parentRevision"].as_str().map(str::to_string);
        chain.push((hash, payload));
        if let Some(parent_hash) = parent {
            hash = parent_hash;
        } else {
            break;
        }
    }
    chain.reverse();
    Ok(chain)
}

fn migrate_id(id: &str) -> String {
    id.replace("cognition:", "cognition_")
        .replace(['/', ':'], "_")
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    const EMPTY_WELCOME: &str =
        "ME\n\nME keeps thoughts you choose.\n\nStart with one:\n\n  Add this thought to ME:\n";

    fn value_text(value: &Value) -> String {
        serde_json::to_string(value).unwrap()
    }

    fn capture_and_add(ws: &Workspace, body: &str) -> Value {
        let captured = ws.thought_capture_body(body.to_string(), "idea").unwrap();
        let thought_id = captured["thoughtId"].as_str().unwrap();
        ws.cognition_add_value(
            thought_id,
            json!({ "action": "add-cognition", "approved": true }),
        )
        .unwrap()
    }

    fn assert_no_home_internals(text: &str) {
        let lower = text.to_ascii_lowercase();
        for needle in [
            "currentsnapshot",
            "sha256",
            "--json",
            "temporary task",
            "fsck",
            "doctor",
            "bundle",
            "index rebuild",
            "procedure",
            "reference",
        ] {
            assert!(
                !lower.contains(needle),
                "home output leaked technical term {needle}: {text}"
            );
        }
    }

    fn write_schema4_object(root: &Path, object_type: &str, payload: Value) -> String {
        let object = json!({
            "schemaVersion": 4,
            "objectType": object_type,
            "payload": payload
        });
        let bytes = canonical_json_bytes(&object).unwrap();
        let hash = sha_ref(&hex_digest(&bytes));
        let hex = strip_sha_prefix(&hash).unwrap();
        let path = root.join(".me/objects").join(&hex[0..2]).join(hex);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        let mut bytes_with_newline = bytes;
        bytes_with_newline.push(b'\n');
        fs::write(path, bytes_with_newline).unwrap();
        hash
    }

    fn write_schema4_fixture(root: &Path) -> (String, String) {
        fs::create_dir_all(root.join(".me/objects")).unwrap();
        fs::create_dir_all(root.join(".me/refs")).unwrap();
        fs::create_dir_all(root.join(".me/journal")).unwrap();
        fs::write(root.join(".me/lock"), "").unwrap();
        fs::write(
            root.join("me.toml"),
            r#"schemaVersion = 4
workspaceId = "mews_TEST"
name = "ME"
defaultActor = "local-user"

[agent]
preferred_host = "codex"
proposal_limit = 5

[privacy]
me_network_access = "forbidden"

[index]
engine = "sqlite"
rebuild_on_integrity_failure = true
"#,
        )
        .unwrap();
        let thought = write_schema4_object(
            root,
            "thought",
            json!({
                "thoughtId": "thought_test",
                "kind": "idea",
                "bodyMarkdown": "The possibility space is part of the artwork.",
                "bodyText": "The possibility space is part of the artwork.",
                "origin": { "type": "local-input", "uri": null, "attribution": null },
                "capturedAt": "2026-06-24T00:00:00Z",
                "capturedBy": "local-user"
            }),
        );
        let decision = write_schema4_object(
            root,
            "decision",
            json!({
                "decisionId": "decision_test",
                "kind": "collection",
                "baseSnapshot": "sha256:0000000000000000000000000000000000000000000000000000000000000000",
                "action": "add-cognition",
                "actor": "local-user",
                "thought": thought,
                "proposal": null,
                "finalBodyMarkdown": "The possibility space is part of the artwork.",
                "noteMarkdown": null,
                "decidedAt": "2026-06-24T00:01:00Z"
            }),
        );
        let cognition = write_schema4_object(
            root,
            "cognition",
            json!({
                "cognitionId": "cognition_test",
                "bodyMarkdown": "The possibility space is part of the artwork.",
                "bodyText": "The possibility space is part of the artwork.",
                "displayTitle": "Possibility Space",
                "originThought": thought,
                "addedByDecision": decision,
                "addedAt": "2026-06-24T00:02:00Z"
            }),
        );
        let app = write_schema4_object(
            root,
            "app-definition",
            json!({
                "appId": "speak-for-me",
                "name": "Speak for Me",
                "version": "0.1.0",
                "manifestHash": "sha256:1111111111111111111111111111111111111111111111111111111111111111",
                "installedAt": "2026-06-24T00:03:00Z"
            }),
        );
        let run = write_schema4_object(
            root,
            "app-run",
            json!({
                "runId": "run_test",
                "appId": "speak-for-me",
                "appVersion": "0.1.0",
                "baseSnapshot": "sha256:0000000000000000000000000000000000000000000000000000000000000000",
                "taskMarkdown": "Draft using ME.",
                "selectedCognitions": [
                    {
                        "cognition": cognition,
                        "cognitionId": "cognition_test",
                        "selectionReason": "fixture"
                    }
                ],
                "analysis": { "findings": [], "gaps": [], "conflicts": [] },
                "resolutions": [],
                "appPoliciesUsed": [],
                "output": {
                    "kind": "draft",
                    "bodyMarkdown": "A historical draft.",
                    "externalAction": false
                },
                "createdAt": "2026-06-24T00:04:00Z"
            }),
        );
        let tree = write_schema4_object(
            root,
            "me-tree",
            json!({
                "thoughts": { "thought_test": thought },
                "thoughtStates": { "thought_test": "added" },
                "cognitions": { "cognition_test": cognition },
                "cognitionStates": { "cognition_test": "active" },
                "decisions": { "decision_test": decision },
                "proposals": {},
                "apps": { "speak-for-me": app },
                "appPolicies": {},
                "appRuns": { "run_test": run }
            }),
        );
        let snapshot = write_schema4_object(
            root,
            "me-snapshot",
            json!({
                "parent": null,
                "tree": tree,
                "operation": "demo-seed",
                "actor": "local-user",
                "message": "schema 4 fixture",
                "createdAt": "2026-06-24T00:05:00Z"
            }),
        );
        fs::write(root.join(".me/refs/current"), format!("{snapshot}\n")).unwrap();
        (snapshot, run)
    }

    #[test]
    fn empty_workspace_initializes_as_me() {
        let dir = tempdir().unwrap();
        Workspace::init(dir.path(), false).unwrap();
        let ws = Workspace::open(dir.path()).unwrap();
        let home = ws.home("json").unwrap();
        assert_eq!(home["schemaVersion"], 1);
        assert_eq!(home["kind"], "me.home");
        assert_eq!(home["workspaceState"], "empty");
        assert_eq!(home["product"]["name"], "ME");
        assert_eq!(home["product"]["descriptor"], "a local meaning environment");
        assert_eq!(home["product"]["primarySurface"], "Codex App");
        assert_eq!(home["summary"]["cognitionCount"], 0);
        assert_eq!(home["summary"]["pendingThoughtCount"], 0);
        assert!(home.get("mentalModel").is_none());
        assert_eq!(home["starterPrompt"], "Add this thought to ME:");
        assert!(home.get("currentSnapshot").is_none());
        assert_no_home_internals(&value_text(&home));

        let markdown = ws.home("markdown").unwrap();
        let markdown = markdown["markdown"].as_str().unwrap();
        assert_eq!(markdown, EMPTY_WELCOME);
        assert_no_home_internals(markdown);

        let home_view = fs::read_to_string(dir.path().join("views/home.md")).unwrap();
        assert_eq!(home_view, EMPTY_WELCOME);
        assert_no_home_internals(&home_view);
        let welcome_view = fs::read_to_string(dir.path().join("views/welcome.md")).unwrap();
        assert_eq!(welcome_view, EMPTY_WELCOME);
        assert!(dir.path().join(".me/refs/current").exists());
        assert!(dir.path().join(".me/derived/guidance.json").exists());
        assert!(dir.path().join("me.toml").exists());
        ws.fsck().unwrap();
    }

    #[test]
    fn welcome_empty_is_short_and_canonical() {
        let dir = tempdir().unwrap();
        Workspace::init(dir.path(), false).unwrap();
        let ws = Workspace::open(dir.path()).unwrap();
        let welcome = ws.welcome().unwrap();
        assert_eq!(welcome["schemaVersion"], 2);
        assert_eq!(welcome["kind"], "me.welcome");
        assert_eq!(welcome["state"], "empty");
        assert_eq!(welcome["starterPrompt"], "Add this thought to ME:");
        assert_eq!(welcome["technical"]["activeCognitionCount"], 0);
        assert_eq!(welcome["technical"]["pendingThoughtCount"], 0);
        let markdown = welcome["renderedMarkdown"].as_str().unwrap();
        assert_eq!(markdown, EMPTY_WELCOME);
        for forbidden in [
            "0 Cognitions",
            "0 pending Thoughts",
            "cognition",
            "Cognition",
            "count",
            "authorship",
            "Draft",
            "Snapshot",
            "sha256",
            "hash",
            "command",
            "fsck",
            "memory",
            "Procedure",
            "Reference",
        ] {
            assert!(!markdown.contains(forbidden), "found {forbidden}");
        }
    }

    #[test]
    fn demo_workspace_home_is_established() {
        let dir = tempdir().unwrap();
        Workspace::init(dir.path(), true).unwrap();
        let ws = Workspace::open(dir.path()).unwrap();
        let home = ws.home("json").unwrap();
        assert_eq!(home["workspaceState"], "established");
        assert_eq!(home["summary"]["activeCognitionCount"], 3);
        assert_eq!(home["summary"]["retiredCognitionCount"], 0);
        assert_eq!(home["summary"]["pendingThoughtCount"], 1);
        assert_eq!(home["examples"]["add"], "Add this thought to ME:");
        assert_eq!(
            home["examples"]["inspect"],
            "What do I have in ME about generative art?"
        );
        assert!(!home["recent"].as_array().unwrap().is_empty());
        assert!(home.get("currentSnapshot").is_none());
        assert_no_home_internals(&value_text(&home));

        let markdown = ws.home("markdown").unwrap();
        let markdown = markdown["markdown"].as_str().unwrap();
        assert!(markdown.contains("A thought is waiting for your decision."));
        assert!(markdown.contains("You have 3 cognitions."));
        assert!(markdown.contains("Add this thought to ME:"));
        assert!(markdown.contains("What do I have in ME about <topic>?"));
        assert!(markdown.contains("Draft <something> using ME."));
        assert!(!markdown.contains("RECENT"));
        assert!(!markdown.contains("WORKSPACE NEEDS ATTENTION"));
        assert_no_home_internals(markdown);

        let home_view = fs::read_to_string(dir.path().join("views/home.md")).unwrap();
        assert!(home_view.contains("You have 3 cognitions."));
        assert_no_home_internals(&home_view);
        let welcome = ws.welcome().unwrap();
        assert_eq!(welcome["state"], "established");
        assert!(
            welcome["renderedMarkdown"]
                .as_str()
                .unwrap()
                .contains("ME is ready.")
        );
        assert_eq!(welcome["technical"]["activeCognitionCount"], 3);
    }

    #[test]
    fn guide_is_scenario_tutorial_not_cli_reference() {
        let dir = tempdir().unwrap();
        Workspace::init(dir.path(), false).unwrap();
        let ws = Workspace::open(dir.path()).unwrap();
        let guide = ws.guide().unwrap();
        assert_eq!(guide["schemaVersion"], 1);
        assert_eq!(guide["kind"], "me.guide");
        assert_eq!(guide["scenarios"].as_array().unwrap().len(), 4);
        let markdown = guide["markdown"].as_str().unwrap();
        assert!(markdown.contains("SCENARIO 1: A THOUGHT OCCURS"));
        assert!(markdown.contains("SCENARIO 2: KEEP THE THOUGHT"));
        assert!(markdown.contains("SCENARIO 3: USE A COGNITION"));
        assert!(markdown.contains("SCENARIO 4: KEEP SOMETHING CODEX PRODUCED"));
        assert!(markdown.find("SCENARIO 1").unwrap() < markdown.find("cognition").unwrap());
        assert!(markdown.contains("Reading and composing do not change ME."));
        let lower = markdown.to_ascii_lowercase();
        assert!(!lower.contains("fsck"));
        assert!(!lower.contains("bundle"));
        assert!(!lower.contains("status --json"));
    }

    #[test]
    fn new_workspace_result_points_to_codex() {
        let dir = tempdir().unwrap();
        let workspace = dir.path().join("ME");
        let result = Workspace::new_workspace(&workspace, false).unwrap();
        assert_eq!(
            result["workspacePath"].as_str().unwrap(),
            workspace.to_string_lossy().as_ref()
        );
        assert_eq!(result["next"]["host"], "Codex App");
        assert_eq!(result["next"]["mode"], "Local");
        assert_eq!(result["next"]["starterPrompt"], "Start ME");
        assert!(
            result["next"]["command"]
                .as_str()
                .unwrap()
                .starts_with("me start --workspace ")
        );
    }

    #[test]
    fn codex_skill_start_intent_is_welcome_only() {
        let skill = workspace_skill_md();
        let welcome = skill.split("## Use ME").next().unwrap();
        assert!(welcome.contains("## Start ME"));
        assert!(welcome.contains("Call `me welcome --json`"));
        assert!(welcome.contains("Output `renderedMarkdown` verbatim."));
        assert!(welcome.contains("Do not use memory."));
        assert!(welcome.contains("Do not call `me context`."));
        assert!(welcome.contains("Do not create files."));
        assert!(welcome.contains("Do not attach files."));
        assert!(welcome.contains("Do not mention commands."));
        assert!(welcome.contains("Do not explain cognition yet"));
        assert!(welcome.contains("\"What can I do here?\""));
        assert!(welcome.contains("Hi. ME is ready."));
        assert!(skill.contains(
            "Treat casual add, capture, save, note, remember, or put-in-ME wording as thought capture only."
        ));
        assert!(skill.contains(
            "Do not infer approval from the same message that supplied the thought text."
        ));
        assert!(skill.contains(
            "The engine rejects `me cognition add` unless the Decision includes `approved: true`."
        ));
        assert!(!skill.contains("if not already explicit"));
    }

    #[test]
    fn workspace_agents_are_product_first() {
        let agents = workspace_agents_md();
        assert!(agents.starts_with(
            "# ME Workspace\n\nME is a local application operated through Codex App."
        ));
        assert!(
            agents.find("## Everyday Use").unwrap() < agents.find("## Technical Rules").unwrap()
        );
        assert!(agents.contains("Start ME"));
        assert!(agents.contains("canonical welcome"));
        assert!(agents.contains("thought capture only"));
        assert!(agents.contains("separate explicit keep decision"));
        assert!(agents.contains("Reading and composition do not change ME."));
        assert!(agents.contains("Codex output never enters ME automatically."));
    }

    #[test]
    fn readme_is_scenario_first() {
        let readme = include_str!("../../../README.md");
        assert!(
            readme.starts_with(
                "# ME\n\nME is a local meaning environment operated through Codex App."
            )
        );
        let sections = [
            "## Install and Start ME",
            "## A thought occurs",
            "## Keep the thought",
            "## Use a cognition",
            "## Keep something Codex produced",
            "## The mental model",
            "## Advanced: References and Procedures",
            "## Advanced: backup, export, and CLI",
            "## Privacy",
            "## Development",
        ];
        let mut previous = 0;
        for section in sections {
            let index = readme
                .find(section)
                .unwrap_or_else(|| panic!("missing {section}"));
            assert!(index >= previous, "{section} is out of order");
            previous = index;
        }
        assert!(readme.contains("brew install inshell-art/tap/me"));
        assert!(readme.contains("me start"));
        assert!(readme.contains("Press Enter on:"));
        assert!(readme.contains("Start ME"));
        assert!(readme.contains("Designing a generative system is part of authorship."));
        assert!(readme.contains(
            "The prompt captures first. It does not keep the thought until you approve."
        ));
        assert!(
            readme.contains("Casual add, capture, save, note, or remember wording is still only")
        );
        assert!(
            readme.contains("The local engine requires explicit keep approval before converting a")
        );
        assert!(readme.contains("Reading and composing do not change ME."));
        assert!(readme.contains("This is my thought. Add it to ME."));
        assert!(readme.contains("ME is the complete product."));
        assert!(readme.contains("ME skill"));
        assert!(readme.contains("me executable"));
        assert!(!readme.contains("Codex App integration"));
        assert!(!readme.contains("Codex plugin"));
        assert!(!readme.contains("CLI product"));
        assert!(readme.contains("COLLECT"));
        assert!(readme.contains("USE"));
        assert!(readme.contains("KEEP FROM OUTPUT"));
        assert!(
            readme.find("## Advanced: backup, export, and CLI").unwrap()
                > readme.find("## Keep something Codex produced").unwrap()
        );
    }

    #[test]
    fn technical_state_is_separate_from_home() {
        let dir = tempdir().unwrap();
        Workspace::init(dir.path(), true).unwrap();
        let ws = Workspace::open(dir.path()).unwrap();
        let home = ws.home("json").unwrap();
        assert!(home.get("currentSnapshot").is_none());
        assert!(!value_text(&home).contains("sha256:"));

        let status = ws.status().unwrap();
        assert!(
            status["currentSnapshot"]
                .as_str()
                .unwrap()
                .starts_with("sha256:")
        );
        assert_eq!(status["schemaVersion"], SCHEMA_VERSION);

        let fsck = ws.fsck().unwrap();
        assert_eq!(fsck["ok"], true);
        assert!(
            fsck["currentSnapshot"]
                .as_str()
                .unwrap()
                .starts_with("sha256:")
        );
    }

    #[test]
    fn thought_capture_response_is_plain_and_nontechnical() {
        let dir = tempdir().unwrap();
        Workspace::init(dir.path(), false).unwrap();
        let ws = Workspace::open(dir.path()).unwrap();
        let captured = ws
            .thought_capture_body(
                "I trust Agent Art will be the next generation for Art.".to_string(),
                "idea",
            )
            .unwrap();
        let rendered = captured["renderedMarkdown"].as_str().unwrap();
        assert!(rendered.contains("THOUGHT"));
        assert!(rendered.contains("“I trust Agent Art will be the next generation for Art.”"));
        assert!(rendered.contains("This thought is captured, but it is not in ME yet."));
        assert!(rendered.contains("Keep it?"));
        for forbidden in [
            "Decision file",
            "canonical",
            "transaction",
            "Cognition ID",
            "Snapshot",
            "sha256",
        ] {
            assert!(!rendered.contains(forbidden), "found {forbidden}");
        }
    }

    #[test]
    fn cognition_add_requires_explicit_keep_approval() {
        let dir = tempdir().unwrap();
        Workspace::init(dir.path(), false).unwrap();
        let ws = Workspace::open(dir.path()).unwrap();
        let captured = ws
            .thought_capture_body(
                "A casual thought can be captured any time.".to_string(),
                "idea",
            )
            .unwrap();
        let thought_id = captured["thoughtId"].as_str().unwrap();
        let before = ws.current_ref().unwrap();

        let err = ws
            .cognition_add_value(thought_id, json!({ "action": "add-cognition" }))
            .unwrap_err();
        assert_eq!(err.code(), "INVALID_INPUT");
        assert!(err.to_string().contains("requires explicit keep approval"));
        assert_eq!(ws.current_ref().unwrap(), before);
        assert_eq!(ws.current().unwrap()["counts"]["activeCognitions"], 0);

        let added = ws
            .cognition_add_value(
                thought_id,
                json!({ "action": "add-cognition", "approved": true }),
            )
            .unwrap();
        assert_eq!(added["cognitionAdded"], true);
        assert_ne!(ws.current_ref().unwrap(), before);
    }

    #[test]
    fn cognition_success_guidance_is_progressive() {
        let dir = tempdir().unwrap();
        Workspace::init(dir.path(), false).unwrap();
        let ws = Workspace::open(dir.path()).unwrap();

        let first = capture_and_add(
            &ws,
            "I trust Agent Art will be the next generation for Art.",
        );
        let first_rendered = first["renderedMarkdown"].as_str().unwrap();
        assert!(first["firstCognition"].as_bool().unwrap());
        assert!(first_rendered.contains("KEPT IN ME"));
        assert!(
            first_rendered.contains("“I trust Agent Art will be the next generation for Art.”")
        );
        assert!(
            first_rendered.contains("In ME, a thought you choose to keep is called a cognition.")
        );
        assert!(first_rendered.contains("Codex can now use it without changing ME."));
        assert!(first_rendered.contains("What do I have in ME about Agent Art?"));
        assert!(first_rendered.contains("Add this thought to ME:"));
        assert_eq!(first["nextGuidance"]["kind"], "first-cognition");
        assert_eq!(first["technical"]["existingCognitionsChanged"], 0);
        assert!(
            first["technical"]["currentSnapshot"]
                .as_str()
                .unwrap()
                .starts_with("sha256:")
        );
        assert!(!first_rendered.contains("Snapshot"));
        assert!(!first_rendered.contains("sha256"));
        assert!(!first_rendered.contains("existing Cognitions changed"));

        let guidance = ws.read_guidance_state().unwrap();
        assert!(guidance.first_cognition_guide_shown);

        let later = capture_and_add(&ws, "A second thought belongs in ME.");
        let later_rendered = later["renderedMarkdown"].as_str().unwrap();
        assert!(!later["firstCognition"].as_bool().unwrap());
        assert!(later_rendered.contains("ME now has this as another cognition."));
        assert!(
            !later_rendered.contains("In ME, a thought you choose to keep is called a cognition.")
        );
        assert!(later_rendered.contains("ME now contains more than one cognition."));
        assert_eq!(later["nextGuidance"]["kind"], "two-cognitions");

        let third = capture_and_add(&ws, "A third thought belongs in ME.");
        assert_eq!(third["nextGuidance"]["kind"], "later-cognition");
        let third_rendered = third["renderedMarkdown"].as_str().unwrap();
        assert!(!third_rendered.contains("more than one cognition"));
        assert!(third_rendered.len() < first_rendered.len());

        capture_and_add(&ws, "A fourth thought belongs in ME.");
        let fifth = capture_and_add(&ws, "A fifth thought belongs in ME.");
        assert_eq!(fifth["nextGuidance"]["kind"], "five-cognitions");
        assert!(
            fifth["renderedMarkdown"]
                .as_str()
                .unwrap()
                .contains("broader patterns")
        );
    }

    #[test]
    fn add_cognition_preserves_thought_and_does_not_rewrite_existing() {
        let dir = tempdir().unwrap();
        Workspace::init(dir.path(), true).unwrap();
        let ws = Workspace::open(dir.path()).unwrap();
        let input = dir.path().join("thought.md");
        fs::write(
            &input,
            "The rules and possibility space are part of the artwork, not just the final object.",
        )
        .unwrap();
        let captured = ws.thought_capture(&input, "idea").unwrap();
        let thought_id = captured["thoughtId"].as_str().unwrap();
        let before = ws.current().unwrap();
        let before_ref = ws.current_ref().unwrap();
        let decision_path = dir.path().join("decision.json");
        fs::write(
            &decision_path,
            r#"{"action":"add-cognition","approved":true}"#,
        )
        .unwrap();
        let result = ws.cognition_add(thought_id, &decision_path).unwrap();
        assert_eq!(result["cognitionsAdded"], 1);
        assert_eq!(result["existingCognitionsChanged"], 0);
        assert_ne!(ws.current_ref().unwrap(), before_ref);
        let added = ws
            .cognition_show(result["cognitionId"].as_str().unwrap())
            .unwrap();
        assert_eq!(
            added["bodyMarkdown"],
            "The rules and possibility space are part of the artwork, not just the final object."
        );
        let after = ws.current().unwrap();
        assert_eq!(
            after["cognitions"].as_array().unwrap().len(),
            before["cognitions"].as_array().unwrap().len() + 1
        );
        let tree_value = serde_json::to_value(ws.load_current().unwrap().tree).unwrap();
        assert!(tree_value.get("confirmedAssociations").is_none());
        assert!(tree_value.get("proposals").is_none());
        assert!(tree_value.get("apps").is_none());
        assert!(tree_value.get("appPolicies").is_none());
        assert!(tree_value.get("appRuns").is_none());
    }

    #[test]
    fn stdin_equivalent_operations_do_not_require_temp_files() {
        let dir = tempdir().unwrap();
        Workspace::init(dir.path(), false).unwrap();
        let ws = Workspace::open(dir.path()).unwrap();
        let captured = ws
            .thought_capture_body(
                "Designing a generative system is part of authorship.".to_string(),
                "idea",
            )
            .unwrap();
        let thought_id = captured["thoughtId"].as_str().unwrap();
        let added = ws
            .cognition_add_value(
                thought_id,
                json!({ "action": "add-cognition", "approved": true }),
            )
            .unwrap();
        assert_eq!(added["cognitionAdded"], true);
        assert_eq!(added["firstCognition"], true);
        assert_eq!(
            added["nextGuidance"]["mentalModel"],
            "In ME, a thought you choose to keep is called a cognition."
        );
        let before = ws.current_ref().unwrap();
        let context = ws
            .context_body("Draft a reply about authorship.".to_string(), 20)
            .unwrap();
        assert_eq!(context["cognitionLibraryChanged"], false);
        assert_eq!(context["guidance"]["kind"], "first-read");
        assert!(
            context["guidance"]["renderedMarkdown"]
                .as_str()
                .unwrap()
                .contains("ME was read, not changed.")
        );
        assert!(
            context["guidance"]["renderedMarkdown"]
                .as_str()
                .unwrap()
                .contains("This is my thought. Add it to ME.")
        );
        assert_eq!(ws.current_ref().unwrap(), before);
        let guidance = ws.read_guidance_state().unwrap();
        assert!(guidance.first_read_guide_shown);
        assert!(guidance.feedback_loop_guide_shown);
        let second_context = ws
            .context_body("Draft another reply about authorship.".to_string(), 20)
            .unwrap();
        assert!(second_context["guidance"].is_null());
        assert_eq!(ws.current_ref().unwrap(), before);
        let parsed =
            Workspace::parse_decision_input(r#"{"action":"add-cognition","approved":true}"#)
                .unwrap();
        assert_eq!(parsed["action"], "add-cognition");
        assert_eq!(parsed["approved"], true);
    }

    #[test]
    fn association_commands_are_non_mutating_compatibility_messages() {
        let dir = tempdir().unwrap();
        Workspace::init(dir.path(), true).unwrap();
        let ws = Workspace::open(dir.path()).unwrap();
        let before = ws.current_ref().unwrap();
        let inferred = ws.association_infer(None).unwrap();
        assert_eq!(inferred["objectsCreated"], 0);
        assert_eq!(ws.current_ref().unwrap(), before);
        let spec = dir.path().join("association.json");
        fs::write(
            &spec,
            r#"{"relation":"recurs","fromCognitions":[],"toCognitions":[]}"#,
        )
        .unwrap();
        let confirmed = ws.association_confirm(&spec).unwrap();
        assert_eq!(confirmed["objectsCreated"], 0);
        assert_eq!(ws.current_ref().unwrap(), before);
    }

    #[test]
    fn context_and_legacy_app_run_do_not_mutate_cognition_library() {
        let dir = tempdir().unwrap();
        Workspace::init(dir.path(), true).unwrap();
        let ws = Workspace::open(dir.path()).unwrap();
        let before_ref = ws.current_ref().unwrap();
        let before = ws.cognition_list(Some("active".to_string())).unwrap();
        let task = dir.path().join("task.md");
        fs::write(&task, "Draft a reply about generative art and authorship.").unwrap();
        let context = ws.context(&task, 2).unwrap();
        assert_eq!(context["baseSnapshot"], before_ref);
        assert_eq!(context["cognitionLibraryChanged"], false);
        assert!(context["selectedCognitions"].as_array().unwrap().len() <= 2);
        assert_eq!(ws.current_ref().unwrap(), before_ref);
        let search = ws.search("generative art", 20, None).unwrap();
        assert_eq!(search["baseSnapshot"], before_ref);
        assert_eq!(ws.current_ref().unwrap(), before_ref);
        let run = ws.app_run("speak-for-me", &task, false).unwrap();
        assert_eq!(run["objectsCreated"], 0);
        assert_eq!(run["cognitionLibraryChanged"], false);
        assert!(
            run["message"]
                .as_str()
                .unwrap()
                .contains("earlier experimental ME schema")
        );
        assert_eq!(ws.current_ref().unwrap(), before_ref);
        let after = ws.cognition_list(Some("active".to_string())).unwrap();
        assert_eq!(before["cognitions"], after["cognitions"]);
    }

    #[test]
    fn migrate_from_v4_preserves_cognitions_and_exports_app_runs() {
        let dir = tempdir().unwrap();
        let (old_snapshot, _run_hash) = write_schema4_fixture(dir.path());
        let result = Workspace::migrate_from_v4(dir.path()).unwrap();
        assert_eq!(result["oldCurrentSnapshot"], old_snapshot);
        assert!(dir.path().join(".me/migrations/v4-apps.json").exists());
        assert!(
            dir.path()
                .join("exports/migration/v4-app-runs/run_test.md")
                .exists()
        );
        let ws = Workspace::open(dir.path()).unwrap();
        ws.fsck().unwrap();
        let current = ws.load_current().unwrap();
        assert_eq!(current.tree.thoughts.len(), 1);
        assert_eq!(current.tree.cognitions.len(), 1);
        assert!(current.tree.apps.is_empty());
        assert!(current.tree.app_runs.is_empty());
        assert!(current.tree.app_policies.is_empty());
        assert!(current.tree.proposals.is_empty());
        assert_eq!(ws.config().unwrap().schema_version, SCHEMA_VERSION);
    }

    #[test]
    fn bundle_round_trip_passes_fsck() {
        let dir = tempdir().unwrap();
        Workspace::init(dir.path(), true).unwrap();
        let ws = Workspace::open(dir.path()).unwrap();
        let before_ref = ws.current_ref().unwrap();
        let task = dir.path().join("task.md");
        fs::write(&task, "Draft a reply about generative art and authorship.").unwrap();
        ws.context(&task, 2).unwrap();
        assert_eq!(ws.current_ref().unwrap(), before_ref);
        let guidance_path = dir.path().join(".me/derived/guidance.json");
        assert!(guidance_path.exists());
        let tree_value = serde_json::to_value(ws.load_current().unwrap().tree).unwrap();
        assert!(!value_text(&tree_value).contains("guidance"));

        let bundle = dir.path().join("bundle.tar");
        ws.bundle_create(&bundle).unwrap();
        ws.bundle_verify(&bundle).unwrap();
        let mut archive = Archive::new(File::open(&bundle).unwrap());
        for entry in archive.entries().unwrap() {
            let path = entry.unwrap().path().unwrap().to_string_lossy().to_string();
            assert!(!path.contains("derived"));
            assert!(!path.contains("guidance.json"));
        }

        let restored = tempdir().unwrap();
        let target = restored.path().join("restored");
        Workspace::bundle_restore(&bundle, &target).unwrap();
        let restored_ws = Workspace::open(&target).unwrap();
        restored_ws.fsck().unwrap();
        assert!(target.join(".me/derived/guidance.json").exists());
    }
}
