use std::collections::{BTreeMap, BTreeSet};
use std::fs::{self, File, OpenOptions};
use std::io::{Read, Write};
use std::path::{Component, Path, PathBuf};

use fs2::FileExt;
use me_core::{
    association_relation_allowed, cognition_state_allowed, is_sha_ref, operation_allowed, sha_ref,
    strip_sha_prefix, thought_kind_allowed, AppDefinitionPayload, AppRunOutput, AppRunPayload,
    AssociationPayload, CognitionOrigin, CognitionPayload, DecisionPayload, GeneratedBy,
    MeSnapshotPayload, MeTreePayload, ObjectEnvelope, Origin, ProposedAssociation,
    RelatedCognition, SelectedCognition, ThoughtPayload, SCHEMA_VERSION, THOUGHT_KINDS,
    WORKSPACE_VERSION,
};
use me_index::{rank_cognitions, CognitionDoc, MatchResult};
use me_markdown::markdown_to_text;
use rusqlite::{params, Connection};
use serde::de::DeserializeOwned;
use serde::Serialize;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use tar::{Archive, Builder, Header};
use tempfile::NamedTempFile;
use thiserror::Error;
use time::{format_description::well_known::Rfc3339, OffsetDateTime};
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
        if demo {
            let demo_result = ws.seed_demo()?;
            Ok(json!({ "workspace": ws.root, "demo": demo_result }))
        } else {
            Ok(json!({ "workspace": ws.root, "currentSnapshot": ws.current_ref()? }))
        }
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
        let data = json!({
            "workspace": {
                "name": self.config()?.name,
                "path": self.root,
                "currentSnapshot": current.snapshot_hash
            },
            "product": {
                "name": "ME",
                "expansion": "Meaning Environment",
                "line": "Tell ME a Thought."
            },
            "cognitionLibrary": {
                "activeCount": active_count,
                "retiredCount": retired_count
            },
            "waiting": {
                "pendingThoughtCount": pending_thoughts,
                "pendingDecisionCount": self.pending_proposal_count()?
            },
            "associations": {
                "confirmedCount": current.tree.confirmed_associations.len(),
                "inferredAvailable": true
            },
            "apps": self.app_summaries(&current.tree)?,
            "starterActions": [
                { "label": "Add Thought", "prompt": "Add this Thought:" },
                { "label": "Inspect ME", "prompt": "Show ME what I have about ..." },
                { "label": "Speak for Me", "prompt": "Use Speak for Me to draft ..." }
            ],
            "health": { "status": "ok", "message": Value::Null }
        });
        if format == "markdown" {
            Ok(json!({ "markdown": home_markdown(&data), "home": data }))
        } else {
            Ok(data)
        }
    }

    pub fn guide(&self) -> Result<Value> {
        Ok(json!({
            "markdown": r#"# ME Guide

THOUGHT
  "The rules and possibility space are part of the artwork."

SUGGESTED EFFECT
  Add this Thought to ME as its own Cognition.

STATUS
  PENDING -- NOT IN ME

RELATED COGNITIONS
  ME may suggest loose inferred Associations.

WHAT WILL CHANGE
  1 Cognition added.
  0 existing Cognitions changed.

ME keeps Cognitions loosely. ME Apps apply explicit rules when you use them.
"#
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
                "confirmedAssociations": current.tree.confirmed_associations.len(),
                "proposals": current.tree.proposals.len(),
                "decisions": current.tree.decisions.len(),
                "apps": current.tree.apps.len(),
                "appRuns": current.tree.app_runs.len()
            }
        }))
    }

    pub fn current(&self) -> Result<Value> {
        let current = self.load_current()?;
        Ok(json!({
            "statusLabel": "CURRENT ME -- user-authorized local state",
            "currentSnapshot": current.snapshot_hash,
            "cognitions": self.current_cognition_summaries(&current.tree)?,
            "confirmedAssociations": self.confirmed_association_summaries(&current.tree)?
        }))
    }

    pub fn doctor(&self, repair: bool) -> Result<Value> {
        self.ensure_supported()?;
        let mut repaired = Vec::new();
        if repair {
            self.regenerate_views()?;
            self.rebuild_index()?;
            self.sync_workspace_docs()?;
            repaired.push("derived-views");
            repaired.push("sqlite-index");
            repaired.push("codex-instructions");
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
            .chain(current.tree.confirmed_associations.values())
            .chain(current.tree.proposals.values())
            .chain(current.tree.decisions.values())
            .chain(current.tree.apps.values())
            .chain(current.tree.app_runs.values())
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
                ".agents/skills/me/references/cognition-library.md",
                ".agents/skills/me/references/associations.md",
                ".agents/skills/me/references/apps.md",
                ".agents/skills/me/references/authorization.md",
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
                "state": "pending",
                "snapshot": snapshot_hash,
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
                .unwrap_or_else(|| "captured".to_string());
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
            .unwrap_or_else(|| "captured".to_string());
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
        let proposal_id = new_id("proposal");
        Ok(json!({
            "thought": {
                "thoughtId": thought_id,
                "hash": thought_hash,
                "bodyMarkdown": thought.payload.body_markdown,
                "kind": thought.payload.kind,
                "statusLabel": "THOUGHT CAPTURED -- PENDING, NOT IN ME"
            },
            "currentSnapshot": current.snapshot_hash,
            "relatedCognitions": related,
            "suggestedEffect": {
                "operation": "add-cognition",
                "bodyMarkdown": thought.payload.body_markdown,
                "existingCognitionsChanged": 0,
                "statusLabel": "PENDING -- NOT IN ME"
            },
            "possibleAssociations": related,
            "proposalObject": {
                "schemaVersion": SCHEMA_VERSION,
                "objectType": "proposal",
                "payload": {
                    "proposalId": proposal_id,
                    "thought": thought_hash,
                    "baseSnapshot": current.snapshot_hash,
                    "relatedCognitions": related,
                    "recommendation": {
                        "operation": "add-cognition",
                        "bodyMarkdown": thought.payload.body_markdown,
                        "confirmAssociations": []
                    },
                    "alternatives": [
                        { "operation": "keep-thought-only" },
                        { "operation": "add-cognition-and-confirm-associations" }
                    ],
                    "generatedBy": { "host": "codex", "model": null },
                    "createdAt": now_rfc3339()?
                }
            }
        }))
    }

    pub fn validate_proposal_file(&self, file: impl AsRef<Path>) -> Result<Value> {
        let proposal = self.parse_proposal_file(file.as_ref())?;
        let current = self.load_current()?;
        let hash = self.validate_proposal(&proposal, &current, false)?;
        Ok(json!({ "proposalId": proposal.payload.proposal_id, "proposal": hash, "valid": true }))
    }

    pub fn save_proposal_file(&self, file: impl AsRef<Path>) -> Result<Value> {
        self.with_lock(|| {
            let proposal = self.parse_proposal_file(file.as_ref())?;
            let current = self.load_current()?;
            if self
                .pending_proposal_ref(&proposal.payload.proposal_id)
                .exists()
                || current
                    .tree
                    .proposals
                    .contains_key(&proposal.payload.proposal_id)
            {
                return Err(invalid(format!(
                    "Proposal ID already exists: {}",
                    proposal.payload.proposal_id
                )));
            }
            let hash = self.validate_proposal(&proposal, &current, true)?;
            fs::create_dir_all(self.pending_proposals_dir())
                .map_err(|err| MeError::Internal(err.into()))?;
            atomic_write(
                &self.pending_proposal_ref(&proposal.payload.proposal_id),
                format!("{hash}\n").as_bytes(),
            )?;
            fs::create_dir_all(self.root.join("drafts/proposals"))
                .map_err(|err| MeError::Internal(err.into()))?;
            atomic_write(
                &self
                    .root
                    .join("drafts/proposals")
                    .join(format!("{}.json", proposal.payload.proposal_id)),
                &serde_json::to_vec_pretty(&proposal)
                    .map_err(|err| MeError::Internal(err.into()))?,
            )?;
            Ok(json!({
                "proposalId": proposal.payload.proposal_id,
                "proposal": hash,
                "statusLabel": "PENDING -- NOT IN ME",
                "status": "pending"
            }))
        })
    }

    pub fn show_proposal(&self, proposal_id_or_hash: &str) -> Result<Value> {
        let (hash, proposal) = self.resolve_proposal(proposal_id_or_hash)?;
        Ok(json!({
            "statusLabel": "PENDING -- NOT IN ME",
            "proposal": hash,
            "payload": proposal.payload
        }))
    }

    pub fn list_proposals(&self, status: Option<String>) -> Result<Value> {
        let mut proposals = Vec::new();
        let pending_dir = self.pending_proposals_dir();
        if pending_dir.exists() {
            for entry in fs::read_dir(pending_dir).map_err(|err| MeError::Internal(err.into()))? {
                let entry = entry.map_err(|err| MeError::Internal(err.into()))?;
                if entry
                    .file_type()
                    .map_err(|err| MeError::Internal(err.into()))?
                    .is_file()
                {
                    let hash = fs::read_to_string(entry.path())
                        .map_err(|err| MeError::Internal(err.into()))?
                        .trim()
                        .to_string();
                    let proposal =
                        self.read_object::<me_core::ProposalPayload>(&hash, "proposal")?;
                    proposals.push(json!({
                        "proposalId": proposal.payload.proposal_id,
                        "proposal": hash,
                        "status": "pending",
                        "statusLabel": "PENDING -- NOT IN ME",
                        "baseSnapshot": proposal.payload.base_snapshot
                    }));
                }
            }
        }
        if status.as_deref().is_some_and(|s| s != "pending") {
            proposals.clear();
        }
        Ok(json!({ "proposals": proposals }))
    }

    pub fn review(&self, proposal_id_or_hash: &str, format: &str) -> Result<Value> {
        let (proposal_hash, proposal) = self.resolve_proposal(proposal_id_or_hash)?;
        let current = self.load_current()?;
        let (_, _, thought) = self.resolve_thought(&current.tree, &proposal.payload.thought)?;
        let text = format_review(
            &proposal_hash,
            &proposal.payload,
            &thought.payload.body_markdown,
        );
        Ok(json!({
            "format": format,
            "statusLabel": "PENDING -- NOT IN ME",
            "proposal": proposal_hash,
            "review": text,
            "thought": {
                "bodyMarkdown": thought.payload.body_markdown,
                "statusLabel": "THOUGHT CAPTURED -- PENDING, NOT IN ME"
            },
            "relatedCognitions": proposal.payload.related_cognitions,
            "proposedEffect": proposal.payload.recommendation,
            "resultingChanges": {
                "cognitionsAdded": 1,
                "existingCognitionsChanged": 0
            }
        }))
    }

    pub fn decide(
        &self,
        proposal_id_or_hash: &str,
        decision_file: impl AsRef<Path>,
    ) -> Result<Value> {
        self.with_lock(|| {
            let current = self.load_current()?;
            let (proposal_hash, proposal) = self.resolve_proposal(proposal_id_or_hash)?;
            if proposal.payload.base_snapshot != current.snapshot_hash {
                return Err(MeError::StaleProposal {
                    code: "STALE_PROPOSAL",
                    message: format!(
                        "Proposal was based on Snapshot {}, current is {}",
                        proposal.payload.base_snapshot, current.snapshot_hash
                    ),
                    details: json!({
                        "proposalBaseSnapshot": proposal.payload.base_snapshot,
                        "currentSnapshot": current.snapshot_hash
                    }),
                });
            }
            self.validate_proposal(&proposal, &current, false)?;
            let mut decision = self.parse_decision_file(
                decision_file.as_ref(),
                &proposal_hash,
                &proposal.payload,
            )?;
            decision.decision_id = new_id("decision");
            decision.proposal = proposal_hash.clone();
            decision.base_snapshot = current.snapshot_hash.clone();
            if decision.actor.is_empty() {
                decision.actor = self.config()?.default_actor;
            }
            if decision.decided_at.is_empty() {
                decision.decided_at = now_rfc3339()?;
            }
            let decision_hash = self.write_object("decision", &decision)?;
            self.apply_decision(
                &current,
                &proposal_hash,
                &proposal.payload,
                &decision_hash,
                &decision,
            )
        })
    }

    pub fn reject_or_defer(
        &self,
        proposal_id_or_hash: &str,
        action: &str,
        note: Option<String>,
    ) -> Result<Value> {
        self.with_lock(|| {
            let current = self.load_current()?;
            let (proposal_hash, proposal) = self.resolve_proposal(proposal_id_or_hash)?;
            let decision = DecisionPayload {
                decision_id: new_id("decision"),
                proposal: proposal_hash.clone(),
                base_snapshot: current.snapshot_hash.clone(),
                action: if action == "defer" {
                    "keep-thought-only".to_string()
                } else {
                    "reject-proposal".to_string()
                },
                actor: self.config()?.default_actor,
                final_body_markdown: None,
                confirmed_associations: Vec::new(),
                note_markdown: note,
                decided_at: now_rfc3339()?,
            };
            let decision_hash = self.write_object("decision", &decision)?;
            self.apply_decision(
                &current,
                &proposal_hash,
                &proposal.payload,
                &decision_hash,
                &decision,
            )
        })
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
            "possiblyRelated": self.inferred_for_cognition(&hash)?,
            "confirmedRelations": self.confirmed_for_cognition(&current.tree, &hash)?,
            "usedBy": self.runs_using_cognition(&current.tree, &hash)?,
            "payload": cognition.payload
        }))
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
        let raw =
            fs::read_to_string(spec_file.as_ref()).map_err(|err| MeError::Internal(err.into()))?;
        let spec: Value =
            serde_json::from_str(&raw).map_err(|err| MeError::Internal(err.into()))?;
        let body = spec
            .get("bodyMarkdown")
            .and_then(Value::as_str)
            .ok_or_else(|| invalid("Synthesis spec requires bodyMarkdown"))?;
        let derived = string_array(spec.get("derivedFromCognitions")).unwrap_or_default();
        if derived.len() < 2 {
            return Err(invalid("Synthesis requires at least two source Cognitions"));
        }
        self.with_lock(|| {
            let current = self.load_current()?;
            let mut derived_hashes = Vec::new();
            for item in &derived {
                let (_, hash, _) = self.resolve_cognition(&current.tree, item)?;
                derived_hashes.push(hash);
            }
            let thought = ThoughtPayload {
                thought_id: new_id("thought"),
                kind: "idea".to_string(),
                body_markdown: body.to_string(),
                body_text: markdown_to_text(body),
                origin: Origin::local_input(),
                captured_at: now_rfc3339()?,
                captured_by: self.config()?.default_actor,
            };
            let thought_hash = self.write_object("thought", &thought)?;
            let decision = DecisionPayload {
                decision_id: new_id("decision"),
                proposal: "direct-synthesis".to_string(),
                base_snapshot: current.snapshot_hash.clone(),
                action: "save-synthesis-cognition".to_string(),
                actor: self.config()?.default_actor,
                final_body_markdown: Some(body.to_string()),
                confirmed_associations: Vec::new(),
                note_markdown: spec
                    .get("noteMarkdown")
                    .and_then(Value::as_str)
                    .map(str::to_string),
                decided_at: now_rfc3339()?,
            };
            let decision_hash = self.write_object("decision", &decision)?;
            let cognition = CognitionPayload {
                cognition_id: new_id("cognition"),
                body_markdown: body.to_string(),
                body_text: markdown_to_text(body),
                display_title: spec
                    .get("displayTitle")
                    .and_then(Value::as_str)
                    .map(str::to_string),
                origin_thought: thought_hash.clone(),
                origin: CognitionOrigin::synthesis(derived_hashes.clone()),
                added_by_decision: decision_hash.clone(),
                added_at: now_rfc3339()?,
            };
            let cognition_hash = self.write_object("cognition", &cognition)?;
            let mut tree = current.tree.clone();
            tree.thoughts
                .insert(thought.thought_id.clone(), thought_hash);
            tree.thought_states
                .insert(thought.thought_id, "added".to_string());
            tree.decisions.insert(decision.decision_id, decision_hash);
            tree.cognitions
                .insert(cognition.cognition_id.clone(), cognition_hash.clone());
            tree.cognition_states
                .insert(cognition.cognition_id.clone(), "active".to_string());
            let snapshot = self.commit_tree(
                &current,
                tree,
                "save-synthesis-cognition",
                "local-user",
                "Save Synthesis Cognition".to_string(),
            )?;
            Ok(json!({
                "statusLabel": "COGNITION ADDED -- synthesis",
                "cognitionId": cognition.cognition_id,
                "cognition": cognition_hash,
                "derivedFromCognitions": derived_hashes,
                "snapshot": snapshot,
                "existingCognitionsChanged": 0
            }))
        })
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
                "associationStatus": "inferred"
            }));
        }
        Ok(json!({
            "statusLabel": "READING -- temporary assembly",
            "about": about,
            "currentSnapshot": current.snapshot_hash,
            "cognitions": cognitions,
            "confirmedAssociations": self.confirmed_association_summaries(&current.tree)?,
            "note": "Inspect ME preserves contradictions and does not synthesize a single position unless asked."
        }))
    }

    pub fn association_infer(&self, cognition: Option<String>) -> Result<Value> {
        let current = self.load_current()?;
        self.rebuild_index()?;
        let inferred = self.compute_inferred_associations(&current.tree, cognition.as_deref())?;
        self.write_inferred_associations(&inferred)?;
        Ok(json!({
            "kind": "inferred",
            "count": inferred.len(),
            "snapshotUnchanged": current.snapshot_hash,
            "associations": inferred
        }))
    }

    pub fn association_list(&self, kind: Option<String>) -> Result<Value> {
        let current = self.load_current()?;
        match kind.as_deref().unwrap_or("confirmed") {
            "confirmed" => Ok(json!({
                "kind": "confirmed",
                "associations": self.confirmed_association_summaries(&current.tree)?
            })),
            "inferred" => Ok(json!({
                "kind": "inferred",
                "associations": self.read_inferred_associations()?
            })),
            other => Err(invalid(format!("Unsupported Association kind: {other}"))),
        }
    }

    pub fn association_confirm(&self, spec_file: impl AsRef<Path>) -> Result<Value> {
        let raw =
            fs::read_to_string(spec_file.as_ref()).map_err(|err| MeError::Internal(err.into()))?;
        let spec: Value =
            serde_json::from_str(&raw).map_err(|err| MeError::Internal(err.into()))?;
        let relation = spec
            .get("relation")
            .and_then(Value::as_str)
            .ok_or_else(|| invalid("Association spec requires relation"))?;
        if !association_relation_allowed(relation) {
            return Err(invalid(format!(
                "Unsupported Association relation: {relation}"
            )));
        }
        let from = string_array(spec.get("fromCognitions"))
            .ok_or_else(|| invalid("Association spec requires fromCognitions"))?;
        let to = string_array(spec.get("toCognitions"))
            .ok_or_else(|| invalid("Association spec requires toCognitions"))?;
        self.with_lock(|| {
            let current = self.load_current()?;
            let from_hashes = self.resolve_cognition_list(&current.tree, &from)?;
            let to_hashes = self.resolve_cognition_list(&current.tree, &to)?;
            let decision = DecisionPayload {
                decision_id: new_id("decision"),
                proposal: "direct-association-confirmation".to_string(),
                base_snapshot: current.snapshot_hash.clone(),
                action: "add-cognition-and-confirm-associations".to_string(),
                actor: self.config()?.default_actor,
                final_body_markdown: None,
                confirmed_associations: Vec::new(),
                note_markdown: spec
                    .get("noteMarkdown")
                    .and_then(Value::as_str)
                    .map(str::to_string),
                decided_at: now_rfc3339()?,
            };
            let decision_hash = self.write_object("decision", &decision)?;
            let association = AssociationPayload {
                association_id: new_id("association"),
                relation: relation.to_string(),
                from_cognitions: from_hashes,
                to_cognitions: to_hashes,
                note_markdown: spec
                    .get("noteMarkdown")
                    .and_then(Value::as_str)
                    .map(str::to_string),
                confirmed_by_decision: decision_hash.clone(),
                confirmed_at: now_rfc3339()?,
            };
            let association_hash = self.write_object("association", &association)?;
            let mut tree = current.tree.clone();
            tree.decisions.insert(decision.decision_id, decision_hash);
            tree.confirmed_associations
                .insert(association.association_id.clone(), association_hash.clone());
            let snapshot = self.commit_tree(
                &current,
                tree,
                "confirm-association",
                "local-user",
                format!("Confirm Association {}", association.association_id),
            )?;
            Ok(json!({
                "statusLabel": "ASSOCIATION -- confirmed",
                "associationId": association.association_id,
                "association": association_hash,
                "snapshot": snapshot
            }))
        })
    }

    pub fn association_remove(
        &self,
        association_id: &str,
        decision_file: impl AsRef<Path>,
    ) -> Result<Value> {
        let _ = fs::read_to_string(decision_file.as_ref())
            .map_err(|err| MeError::Internal(err.into()))?;
        self.with_lock(|| {
            let current = self.load_current()?;
            let mut tree = current.tree.clone();
            let removed = tree
                .confirmed_associations
                .remove(association_id)
                .ok_or_else(|| not_found(format!("Association not found: {association_id}")))?;
            let snapshot = self.commit_tree(
                &current,
                tree,
                "remove-association",
                "local-user",
                format!("Remove Association {association_id}"),
            )?;
            Ok(json!({ "associationId": association_id, "removed": removed, "snapshot": snapshot }))
        })
    }

    pub fn app_list(&self) -> Result<Value> {
        let current = self.load_current()?;
        Ok(json!({ "apps": self.app_summaries(&current.tree)? }))
    }

    pub fn app_show(&self, app_id: &str) -> Result<Value> {
        let current = self.load_current()?;
        let hash = current
            .tree
            .apps
            .get(app_id)
            .ok_or_else(|| not_found(format!("ME App not found: {app_id}")))?;
        let app = self.read_object::<AppDefinitionPayload>(hash, "app-definition")?;
        Ok(json!({
            "statusLabel": "ME APP -- local",
            "app": hash,
            "payload": app.payload,
            "manifest": self.app_manifest_json(app_id)?
        }))
    }

    pub fn app_validate(&self, app_directory: impl AsRef<Path>) -> Result<Value> {
        let manifest = read_app_manifest(app_directory.as_ref())?;
        validate_app_manifest(&manifest)?;
        Ok(json!({
            "valid": true,
            "appId": manifest.get("app_id").and_then(Value::as_str),
            "externalActionsAllowed": false
        }))
    }

    pub fn app_install(&self, app_directory: impl AsRef<Path>) -> Result<Value> {
        let manifest = read_app_manifest(app_directory.as_ref())?;
        validate_app_manifest(&manifest)?;
        let app_id = manifest
            .get("app_id")
            .and_then(Value::as_str)
            .ok_or_else(|| invalid("App manifest missing app_id"))?;
        self.with_lock(|| {
            let current = self.load_current()?;
            let dest = self.root.join("apps").join(app_id);
            copy_dir_replace(app_directory.as_ref(), &dest)?;
            let app = AppDefinitionPayload {
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
                manifest_hash: file_hash(&dest.join("app.toml"))?,
                installed_at: now_rfc3339()?,
            };
            let app_hash = self.write_object("app-definition", &app)?;
            let mut tree = current.tree.clone();
            tree.apps.insert(app.app_id.clone(), app_hash.clone());
            let snapshot = self.commit_tree(
                &current,
                tree,
                "install-app",
                "local-user",
                format!("Install ME App {}", app.app_id),
            )?;
            Ok(json!({ "appId": app.app_id, "app": app_hash, "snapshot": snapshot }))
        })
    }

    pub fn app_run(
        &self,
        app_id: &str,
        task_file: impl AsRef<Path>,
        context_only: bool,
    ) -> Result<Value> {
        let task =
            fs::read_to_string(task_file.as_ref()).map_err(|err| MeError::Internal(err.into()))?;
        let current = self.load_current()?;
        let app = self.app_definition(&current.tree, app_id)?;
        let selected = self.select_cognitions_for_task(&current.tree, &task, 20)?;
        let conflicts = infer_conflicts_from_selected(&selected);
        let gaps = if selected.is_empty() {
            vec!["No directly relevant Cognitions found.".to_string()]
        } else {
            Vec::new()
        };
        let output = build_app_output(app_id, &task, &selected, &conflicts, &gaps);
        let run = AppRunPayload {
            run_id: new_id("run"),
            app_id: app.payload.app_id.clone(),
            app_version: app.payload.version.clone(),
            base_snapshot: current.snapshot_hash.clone(),
            task_markdown: task,
            selected_cognitions: selected,
            confirmed_associations_used: Vec::new(),
            inferred_associations_used: Vec::new(),
            conflicts,
            gaps,
            output,
            created_at: now_rfc3339()?,
        };
        if context_only {
            return Ok(json!({
                "statusLabel": "ME APP CONTEXT -- not saved",
                "contextOnly": true,
                "appRun": run,
                "cognitionLibraryChanged": false
            }));
        }
        self.with_lock(|| {
            let refreshed = self.load_current()?;
            let run_hash = self.write_object("app-run", &run)?;
            let mut tree = refreshed.tree.clone();
            tree.app_runs.insert(run.run_id.clone(), run_hash.clone());
            let snapshot = self.commit_tree(
                &refreshed,
                tree,
                "create-app-run",
                "local-user",
                format!("Run ME App {}", app_id),
            )?;
            Ok(json!({
                "statusLabel": "ME APP RUN -- LOCAL OUTPUT, NOT SENT",
                "runId": run.run_id,
                "run": run_hash,
                "snapshot": snapshot,
                "externalAction": false,
                "cognitionLibraryChanged": false
            }))
        })
    }

    pub fn app_save_run(&self, file: impl AsRef<Path>) -> Result<Value> {
        let raw = fs::read_to_string(file.as_ref()).map_err(|err| MeError::Internal(err.into()))?;
        let value: Value =
            serde_json::from_str(&raw).map_err(|err| MeError::Internal(err.into()))?;
        let run: AppRunPayload =
            if value.get("objectType").and_then(Value::as_str) == Some("app-run") {
                serde_json::from_value::<ObjectEnvelope<AppRunPayload>>(value)
                    .map_err(|err| MeError::Internal(err.into()))?
                    .payload
            } else {
                serde_json::from_value(value).map_err(|err| MeError::Internal(err.into()))?
            };
        if run.output.external_action {
            return Err(invalid(
                "ME App Run output cannot record an external action in v0.3",
            ));
        }
        self.with_lock(|| {
            let current = self.load_current()?;
            let run_hash = self.write_object("app-run", &run)?;
            let mut tree = current.tree.clone();
            tree.app_runs.insert(run.run_id.clone(), run_hash.clone());
            let snapshot = self.commit_tree(
                &current,
                tree,
                "save-app-run",
                "local-user",
                format!("Save App Run {}", run.run_id),
            )?;
            Ok(json!({ "runId": run.run_id, "run": run_hash, "snapshot": snapshot }))
        })
    }

    pub fn run_list(&self, app_id: Option<String>) -> Result<Value> {
        let current = self.load_current()?;
        let mut runs = Vec::new();
        for (run_id, hash) in &current.tree.app_runs {
            let run = self.read_object::<AppRunPayload>(hash, "app-run")?;
            if app_id
                .as_deref()
                .is_some_and(|filter| filter != run.payload.app_id)
            {
                continue;
            }
            runs.push(json!({
                "runId": run_id,
                "run": hash,
                "appId": run.payload.app_id,
                "appVersion": run.payload.app_version,
                "outputKind": run.payload.output.kind,
                "externalAction": run.payload.output.external_action,
                "createdAt": run.payload.created_at
            }));
        }
        Ok(json!({ "runs": runs }))
    }

    pub fn run_show(&self, run_id_or_hash: &str, format: &str) -> Result<Value> {
        let current = self.load_current()?;
        let hash = if is_sha_ref(run_id_or_hash) {
            run_id_or_hash.to_string()
        } else {
            current
                .tree
                .app_runs
                .get(run_id_or_hash)
                .ok_or_else(|| not_found(format!("App Run not found: {run_id_or_hash}")))?
                .clone()
        };
        let run = self.read_object::<AppRunPayload>(&hash, "app-run")?;
        let markdown = app_run_markdown(&run.payload);
        Ok(json!({
            "format": format,
            "statusLabel": "ME APP RUN -- LOCAL OUTPUT, NOT SENT",
            "run": hash,
            "payload": run.payload,
            "markdown": markdown
        }))
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
        let added_associations: Vec<_> = b
            .confirmed_associations
            .keys()
            .filter(|key| !a.confirmed_associations.contains_key(*key))
            .cloned()
            .collect();
        let text = format!(
            "Thoughts added: {}\nCognitions changed: {}\nConfirmed Associations added: {}",
            added_thoughts.len(),
            changed_cognitions.len(),
            added_associations.len()
        );
        Ok(json!({
            "format": format,
            "snapshotA": snapshot_a,
            "snapshotB": snapshot_b,
            "text": text,
            "addedThoughts": added_thoughts,
            "changedCognitions": changed_cognitions,
            "addedConfirmedAssociations": added_associations
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

    pub fn snapshot_restore(&self, snapshot_id: &str, message: &str) -> Result<Value> {
        self.with_lock(|| {
            let current = self.load_current()?;
            let restored_tree = self.snapshot_tree(snapshot_id)?;
            let snapshot_hash = self.commit_tree(
                &current,
                restored_tree,
                "restore",
                "local-user",
                message.to_string(),
            )?;
            Ok(json!({ "restoredFrom": snapshot_id, "snapshot": snapshot_hash }))
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
            proposal: "my-model-v2-migration".to_string(),
            base_snapshot: old_current.clone(),
            action: "add-cognition".to_string(),
            actor: "local-user".to_string(),
            final_body_markdown: None,
            confirmed_associations: Vec::new(),
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
                let mut previous_new: Option<String> = None;
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
                        origin: CognitionOrigin::thought(),
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
                    if let Some(previous_hash) = previous_new {
                        let association = AssociationPayload {
                            association_id: new_id("association"),
                            relation: "supersedes".to_string(),
                            from_cognitions: vec![new_hash.clone()],
                            to_cognitions: vec![previous_hash],
                            note_markdown: Some("Migrated My Model revision edge.".to_string()),
                            confirmed_by_decision: migration_decision_hash.clone(),
                            confirmed_at: migrated_at.clone(),
                        };
                        let association_hash = ws.write_object("association", &association)?;
                        tree.confirmed_associations
                            .insert(association.association_id, association_hash.clone());
                        mappings.push(json!({ "kind": "association", "relation": "supersedes", "new": association_hash }));
                    }
                    previous_new = Some(new_hash);
                }
            }
        }
        let app_hashes = ws.write_builtin_app_definitions(&mut tree)?;
        let tree_hash = ws.write_object("me-tree", &tree)?;
        let snapshot = MeSnapshotPayload {
            parent: None,
            tree: tree_hash,
            operation: "migrate-from-my-model".to_string(),
            actor: "local-user".to_string(),
            message: "Migrate My Model v0.2 workspace to ME v0.3".to_string(),
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
            "Migrate My Model v0.2 workspace to ME v0.3",
        )?;
        ws.regenerate_views()?;
        ws.rebuild_index()?;
        ws.fsck()?;
        let manifest = json!({
            "schemaVersion": 1,
            "sourceSystem": "my-model",
            "sourceWorkspaceVersion": 2,
            "targetSystem": "me",
            "targetWorkspaceVersion": 3,
            "migratedAt": migrated_at,
            "oldCurrentSnapshot": old_current,
            "newCurrentSnapshot": new_current,
            "objects": mappings,
            "apps": app_hashes,
            "oldMyModelPreserved": true
        });
        let manifest_path = root.join(".me/migrations/my-model-v2-to-me-v3-manifest.json");
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
            "drafts/proposals",
            "drafts/app-runs",
            "apps/inspect-me",
            "apps/speak-for-me",
            "views/thoughts",
            "views/cognitions",
            "views/associations",
            "views/apps",
            "views/runs",
            "exports",
            ".me/objects",
            ".me/refs",
            ".me/journal",
            ".me/migrations",
            ".me/tmp",
            ".me/pending/proposals",
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
        self.write_builtin_app_packages()?;
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
        let mut tree = MeTreePayload::default();
        self.write_builtin_app_definitions(&mut tree)?;
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
                "Generative art is produced by a generative system.",
                "Definition",
            ),
            (
                "The system's rules and possibility space are part of the artwork.",
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
                let proposal = me_core::ProposalPayload {
                    proposal_id: new_id("proposal"),
                    thought: thought_hash.clone(),
                    base_snapshot: current.snapshot_hash.clone(),
                    related_cognitions: Vec::new(),
                    recommendation: json!({
                        "operation": "add-cognition",
                        "bodyMarkdown": body,
                        "confirmAssociations": []
                    }),
                    alternatives: Vec::new(),
                    generated_by: GeneratedBy {
                        host: "me-demo".to_string(),
                        model: None,
                    },
                    created_at: now_rfc3339()?,
                };
                let proposal_hash = self.write_object("proposal", &proposal)?;
                let decision = DecisionPayload {
                    decision_id: new_id("decision"),
                    proposal: proposal_hash.clone(),
                    base_snapshot: current.snapshot_hash.clone(),
                    action: "add-cognition".to_string(),
                    actor: "local-user".to_string(),
                    final_body_markdown: Some(body.to_string()),
                    confirmed_associations: Vec::new(),
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
                    origin: CognitionOrigin::thought(),
                    added_by_decision: decision_hash.clone(),
                    added_at: now_rfc3339()?,
                };
                let cognition_hash = self.write_object("cognition", &cognition)?;
                tree.thoughts
                    .insert(thought.thought_id.clone(), thought_hash.clone());
                tree.thought_states
                    .insert(thought.thought_id.clone(), "added".to_string());
                tree.proposals
                    .insert(proposal.proposal_id.clone(), proposal_hash.clone());
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
        let (thought_id, thought_hash, thought) = self.resolve_thought(&tree, &proposal.thought)?;
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
            "add-cognition"
            | "add-cognition-and-confirm-associations"
            | "save-synthesis-cognition" => {
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
                    origin: CognitionOrigin::thought(),
                    added_by_decision: decision_hash.to_string(),
                    added_at: now_rfc3339()?,
                };
                let cognition_hash = self.write_object("cognition", &cognition)?;
                tree.cognitions
                    .insert(cognition.cognition_id.clone(), cognition_hash.clone());
                tree.cognition_states
                    .insert(cognition.cognition_id.clone(), "active".to_string());
                tree.thought_states.insert(thought_id, "added".to_string());
                let mut association_hashes = Vec::new();
                for association in &decision.confirmed_associations {
                    let association_hash =
                        self.write_confirmed_association(&mut tree, association, decision_hash)?;
                    association_hashes.push(association_hash);
                }
                result["statusLabel"] = json!("ADDED TO ME");
                result["cognitionId"] = json!(cognition.cognition_id);
                result["cognition"] = json!(cognition_hash);
                result["cognitionsAdded"] = json!(1);
                result["confirmedAssociationsAdded"] = json!(association_hashes.len());
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
        let _ = fs::read_to_string(decision_file.as_ref())
            .map_err(|err| MeError::Internal(err.into()))?;
        self.with_lock(|| {
            let current = self.load_current()?;
            let (cognition_id, cognition_hash, _) =
                self.resolve_cognition(&current.tree, cognition_id_or_hash)?;
            let mut tree = current.tree.clone();
            tree.cognition_states
                .insert(cognition_id.clone(), state.to_string());
            let snapshot = self.commit_tree(
                &current,
                tree,
                operation,
                "local-user",
                format!("{operation} {cognition_id}"),
            )?;
            Ok(json!({
                "cognitionId": cognition_id,
                "cognition": cognition_hash,
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
        proposal_hash: &str,
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
                proposal: value
                    .get("proposal")
                    .and_then(Value::as_str)
                    .unwrap_or(proposal_hash)
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
                final_body_markdown: value
                    .get("finalBodyMarkdown")
                    .and_then(Value::as_str)
                    .map(str::to_string),
                confirmed_associations: parse_proposed_associations(
                    value.get("confirmedAssociations"),
                )?,
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
            return Err(invalid(
                "Proposal must be a schemaVersion 3 proposal object",
            ));
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
        self.resolve_thought(&current.tree, &proposal.payload.thought)?;
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
        let mut cognitions = Vec::new();
        for (cognition_id, hash) in &tree.cognitions {
            let state = tree
                .cognition_states
                .get(cognition_id)
                .cloned()
                .unwrap_or_else(|| "active".to_string());
            if state != "active" {
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

    fn related_from_matches(
        &self,
        matches: &[MatchResult],
        thought_text: &str,
    ) -> Result<Vec<RelatedCognition>> {
        let current = self.load_current()?;
        let mut related = Vec::new();
        for matched in matches {
            let (_, _, cognition) = self.resolve_cognition(&current.tree, &matched.cognition)?;
            related.push(RelatedCognition {
                cognition: matched.cognition.clone(),
                cognition_id: matched.cognition_id.clone(),
                score: matched.score,
                relation_suggestion: Some(suggest_relation(
                    thought_text,
                    &cognition.payload.body_text,
                )),
                status: "inferred".to_string(),
                matched_terms: matched.matched_terms.clone(),
                explanation: Some("Deterministic lexical match; not authoritative.".to_string()),
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

    fn confirmed_association_summaries(&self, tree: &MeTreePayload) -> Result<Vec<Value>> {
        let mut out = Vec::new();
        for (association_id, hash) in &tree.confirmed_associations {
            let association = self.read_object::<AssociationPayload>(hash, "association")?;
            out.push(json!({
                "statusLabel": "ASSOCIATION -- confirmed",
                "associationId": association_id,
                "association": hash,
                "relation": association.payload.relation,
                "fromCognitions": association.payload.from_cognitions,
                "toCognitions": association.payload.to_cognitions,
                "confirmedAt": association.payload.confirmed_at
            }));
        }
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
        for child in ["thoughts", "cognitions", "associations", "apps", "runs"] {
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
        for (thought_id, hash) in &current.tree.thoughts {
            let thought = self.read_object::<ThoughtPayload>(hash, "thought")?;
            let state = current
                .tree
                .thought_states
                .get(thought_id)
                .cloned()
                .unwrap_or_else(|| "captured".to_string());
            let body = format!(
                "---\ngeneratedBy: me\nsnapshot: {}\nthoughtId: {}\nobject: {}\nstate: {}\n---\n\nGenerated by ME from snapshot {}.\nDo not edit directly.\n\n# Thought {}\n\n{}\n",
                current.snapshot_hash, thought_id, hash, state, current.snapshot_hash, thought_id, thought.payload.body_markdown
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
                current.snapshot_hash, cognition_id, hash, state, current.snapshot_hash, title, cognition.payload.body_markdown
            );
            atomic_write(&views.join("cognitions").join(file_name), body.as_bytes())?;
        }
        for (association_id, hash) in &current.tree.confirmed_associations {
            let association = self.read_object::<AssociationPayload>(hash, "association")?;
            let body = format!(
                "---\ngeneratedBy: me\nsnapshot: {}\nassociationId: {}\nobject: {}\nkind: confirmed\n---\n\n# Confirmed Association {}\n\nrelation: {}\n\nfrom: {:?}\n\nto: {:?}\n",
                current.snapshot_hash,
                association_id,
                hash,
                association_id,
                association.payload.relation,
                association.payload.from_cognitions,
                association.payload.to_cognitions
            );
            atomic_write(
                &views
                    .join("associations")
                    .join(format!("{association_id}.md")),
                body.as_bytes(),
            )?;
        }
        for (run_id, hash) in &current.tree.app_runs {
            let run = self.read_object::<AppRunPayload>(hash, "app-run")?;
            atomic_write(
                &views.join("runs").join(format!("{run_id}.md")),
                app_run_markdown(&run.payload).as_bytes(),
            )?;
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
            CREATE TABLE meta (key TEXT PRIMARY KEY, value TEXT NOT NULL);
            CREATE TABLE thoughts (thought_id TEXT PRIMARY KEY, object TEXT NOT NULL, kind TEXT NOT NULL, state TEXT NOT NULL, body_markdown TEXT NOT NULL, body_text TEXT NOT NULL);
            CREATE TABLE cognitions (cognition_id TEXT PRIMARY KEY, object TEXT NOT NULL, state TEXT NOT NULL, display_title TEXT, body_markdown TEXT NOT NULL, body_text TEXT NOT NULL);
            CREATE TABLE confirmed_associations (association_id TEXT PRIMARY KEY, object TEXT NOT NULL, relation TEXT NOT NULL, from_cognitions TEXT NOT NULL, to_cognitions TEXT NOT NULL);
            CREATE TABLE inferred_associations (from_cognition TEXT NOT NULL, to_cognition TEXT NOT NULL, relation TEXT NOT NULL, score REAL NOT NULL, matched_terms TEXT NOT NULL, generated_at TEXT NOT NULL, PRIMARY KEY (from_cognition, to_cognition, relation));
            CREATE TABLE app_runs (run_id TEXT PRIMARY KEY, object TEXT NOT NULL, app_id TEXT NOT NULL, output_kind TEXT NOT NULL, external_action INTEGER NOT NULL);
            CREATE TABLE proposals (proposal_id TEXT PRIMARY KEY, object TEXT NOT NULL, base_snapshot TEXT NOT NULL);
            CREATE TABLE decisions (decision_id TEXT PRIMARY KEY, object TEXT NOT NULL, proposal TEXT NOT NULL, action TEXT NOT NULL);
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
                .unwrap_or_else(|| "captured".to_string());
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
        for (association_id, hash) in &current.tree.confirmed_associations {
            let association = self.read_object::<AssociationPayload>(hash, "association")?;
            conn.execute(
                "INSERT INTO confirmed_associations VALUES (?1, ?2, ?3, ?4, ?5)",
                params![
                    association_id,
                    hash,
                    association.payload.relation,
                    serde_json::to_string(&association.payload.from_cognitions).unwrap(),
                    serde_json::to_string(&association.payload.to_cognitions).unwrap()
                ],
            )
            .map_err(|err| MeError::Internal(err.into()))?;
        }
        for (proposal_id, hash) in &current.tree.proposals {
            let proposal = self.read_object::<me_core::ProposalPayload>(hash, "proposal")?;
            conn.execute(
                "INSERT INTO proposals VALUES (?1, ?2, ?3)",
                params![proposal_id, hash, proposal.payload.base_snapshot],
            )
            .map_err(|err| MeError::Internal(err.into()))?;
        }
        for (decision_id, hash) in &current.tree.decisions {
            let decision = self.read_object::<DecisionPayload>(hash, "decision")?;
            conn.execute(
                "INSERT INTO decisions VALUES (?1, ?2, ?3, ?4)",
                params![
                    decision_id,
                    hash,
                    decision.payload.proposal,
                    decision.payload.action
                ],
            )
            .map_err(|err| MeError::Internal(err.into()))?;
        }
        for (run_id, hash) in &current.tree.app_runs {
            let run = self.read_object::<AppRunPayload>(hash, "app-run")?;
            conn.execute(
                "INSERT INTO app_runs VALUES (?1, ?2, ?3, ?4, ?5)",
                params![
                    run_id,
                    hash,
                    run.payload.app_id,
                    run.payload.output.kind,
                    run.payload.output.external_action as i32
                ],
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
        let inferred = self.compute_inferred_associations(&current.tree, None)?;
        self.write_inferred_associations_to_conn(&conn, &inferred)?;
        Ok(())
    }

    fn sync_workspace_docs(&self) -> Result<()> {
        let readme = r#"# ME

ME is your local Meaning Environment.

Tell ME a Thought. It becomes a Cognition only when you choose to add it. ME keeps Cognitions loosely: they may overlap, recur, qualify, or contradict one another without being forced into one canonical statement.

ME Apps apply explicit domain rules when you use those Cognitions for a task. App Outputs never become Cognitions automatically.

ME stores its Cognition Library and history in your local workspace. The `me` engine itself does not use the network.

ME is not a digital clone, a complete identity, or an agent authorized to act as you.
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
            &skill_dir.join("references/cognition-library.md"),
            workspace_cognition_library_md().as_bytes(),
        )?;
        atomic_write(
            &skill_dir.join("references/associations.md"),
            workspace_associations_md().as_bytes(),
        )?;
        atomic_write(
            &skill_dir.join("references/apps.md"),
            workspace_apps_md().as_bytes(),
        )?;
        atomic_write(
            &skill_dir.join("references/authorization.md"),
            workspace_authorization_md().as_bytes(),
        )?;
        atomic_write(
            &skill_dir.join("references/cli-contract.md"),
            workspace_cli_contract_md().as_bytes(),
        )?;
        atomic_write(
            &skill_dir.join("agents/openai.yaml"),
            b"interface:\n  display_name: \"ME\"\n  short_description: \"Capture Thoughts and add Cognitions.\"\n  default_prompt: \"Add this Thought to ME.\"\n\npolicy:\n  allow_implicit_invocation: true\n",
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

    fn write_confirmed_association(
        &self,
        tree: &mut MeTreePayload,
        association: &ProposedAssociation,
        decision_hash: &str,
    ) -> Result<String> {
        if !association_relation_allowed(&association.relation) {
            return Err(invalid(format!(
                "Unsupported Association relation: {}",
                association.relation
            )));
        }
        let from = self.resolve_cognition_list(tree, &association.from_cognitions)?;
        let to = self.resolve_cognition_list(tree, &association.to_cognitions)?;
        let payload = AssociationPayload {
            association_id: new_id("association"),
            relation: association.relation.clone(),
            from_cognitions: from,
            to_cognitions: to,
            note_markdown: association.note_markdown.clone(),
            confirmed_by_decision: decision_hash.to_string(),
            confirmed_at: now_rfc3339()?,
        };
        let hash = self.write_object("association", &payload)?;
        tree.confirmed_associations
            .insert(payload.association_id, hash.clone());
        Ok(hash)
    }

    fn resolve_cognition_list(
        &self,
        tree: &MeTreePayload,
        items: &[String],
    ) -> Result<Vec<String>> {
        let mut hashes = Vec::new();
        for item in items {
            let (_, hash, _) = self.resolve_cognition(tree, item)?;
            hashes.push(hash);
        }
        Ok(hashes)
    }

    fn confirmed_for_cognition(
        &self,
        tree: &MeTreePayload,
        cognition_hash: &str,
    ) -> Result<Vec<Value>> {
        Ok(self
            .confirmed_association_summaries(tree)?
            .into_iter()
            .filter(|association| {
                association["fromCognitions"]
                    .as_array()
                    .is_some_and(|items| {
                        items
                            .iter()
                            .any(|item| item.as_str() == Some(cognition_hash))
                    })
                    || association["toCognitions"].as_array().is_some_and(|items| {
                        items
                            .iter()
                            .any(|item| item.as_str() == Some(cognition_hash))
                    })
            })
            .collect())
    }

    fn inferred_for_cognition(&self, cognition_hash: &str) -> Result<Vec<Value>> {
        Ok(self
            .read_inferred_associations()?
            .into_iter()
            .filter(|item| {
                item["fromCognition"].as_str() == Some(cognition_hash)
                    || item["toCognition"].as_str() == Some(cognition_hash)
            })
            .collect())
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

    fn compute_inferred_associations(
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
        for (id, hash, cognition) in &cognitions {
            if only.is_some_and(|target| target != id && target != hash) {
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
                    "fromCognition": hash,
                    "toCognition": matched.cognition,
                    "relation": suggest_relation(&cognition.body_text, &matched.title),
                    "score": matched.score,
                    "matchedTerms": matched.matched_terms,
                    "status": "inferred"
                }));
            }
        }
        out.sort_by(|a, b| {
            a["fromCognition"]
                .as_str()
                .unwrap_or("")
                .cmp(b["fromCognition"].as_str().unwrap_or(""))
                .then_with(|| {
                    a["toCognition"]
                        .as_str()
                        .unwrap_or("")
                        .cmp(b["toCognition"].as_str().unwrap_or(""))
                })
        });
        Ok(out)
    }

    fn write_inferred_associations(&self, associations: &[Value]) -> Result<()> {
        let conn = Connection::open(self.root.join(".me/index.sqlite"))
            .map_err(|err| MeError::Internal(err.into()))?;
        conn.execute("DELETE FROM inferred_associations", [])
            .map_err(|err| MeError::Internal(err.into()))?;
        self.write_inferred_associations_to_conn(&conn, associations)
    }

    fn write_inferred_associations_to_conn(
        &self,
        conn: &Connection,
        associations: &[Value],
    ) -> Result<()> {
        let generated_at = now_rfc3339()?;
        for item in associations {
            conn.execute(
                "INSERT OR REPLACE INTO inferred_associations VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                params![
                    item["fromCognition"].as_str(),
                    item["toCognition"].as_str(),
                    item["relation"].as_str(),
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

    fn read_inferred_associations(&self) -> Result<Vec<Value>> {
        let path = self.root.join(".me/index.sqlite");
        if !path.exists() {
            return Ok(Vec::new());
        }
        let conn = Connection::open(path).map_err(|err| MeError::Internal(err.into()))?;
        let mut stmt = conn
            .prepare("SELECT from_cognition, to_cognition, relation, score, matched_terms, generated_at FROM inferred_associations ORDER BY from_cognition, to_cognition")
            .map_err(|err| MeError::Internal(err.into()))?;
        let rows = stmt
            .query_map([], |row| {
                let matched_terms: String = row.get(4)?;
                Ok(json!({
                    "fromCognition": row.get::<_, String>(0)?,
                    "toCognition": row.get::<_, String>(1)?,
                    "relation": row.get::<_, String>(2)?,
                    "score": row.get::<_, f64>(3)?,
                    "matchedTerms": serde_json::from_str::<Value>(&matched_terms).unwrap_or_else(|_| json!([])),
                    "generatedAt": row.get::<_, String>(5)?,
                    "status": "inferred"
                }))
            })
            .map_err(|err| MeError::Internal(err.into()))?;
        let mut out = Vec::new();
        for row in rows {
            out.push(row.map_err(|err| MeError::Internal(err.into()))?);
        }
        Ok(out)
    }

    fn write_builtin_app_packages(&self) -> Result<()> {
        write_builtin_app(
            &self.root.join("apps/inspect-me"),
            "inspect-me",
            "Inspect ME",
            "Retrieve and present Cognitions relevant to a question.",
            "reading",
        )?;
        write_builtin_app(
            &self.root.join("apps/speak-for-me"),
            "speak-for-me",
            "Speak for Me",
            "Draft communication grounded in selected Cognitions.",
            "draft",
        )?;
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
                reason: format!("Matched task terms: {}", matched.matched_terms.join(", ")),
            });
        }
        Ok(selected)
    }
}

fn home_markdown(data: &Value) -> String {
    let active = data["cognitionLibrary"]["activeCount"]
        .as_u64()
        .unwrap_or(0);
    let retired = data["cognitionLibrary"]["retiredCount"]
        .as_u64()
        .unwrap_or(0);
    let pending = data["waiting"]["pendingThoughtCount"].as_u64().unwrap_or(0);
    let pending_decisions = data["waiting"]["pendingDecisionCount"]
        .as_u64()
        .unwrap_or(0);
    let confirmed = data["associations"]["confirmedCount"].as_u64().unwrap_or(0);
    let apps = data["apps"]
        .as_array()
        .map(|items| {
            items
                .iter()
                .filter_map(|item| item["name"].as_str())
                .collect::<Vec<_>>()
                .join("\n  ")
        })
        .unwrap_or_default();
    format!(
        r#"# ME

Meaning Environment

Tell ME a Thought.

## Cognition Library

  {active} active Cognitions
  {retired} retired Cognitions

## Waiting

  {pending} Thoughts waiting
  {pending_decisions} pending decisions

## Associations

  {confirmed} confirmed
  inferred links available

## ME Apps

  {apps}

## Start

  Add this Thought: ...
  Show ME what I have about ...
  Use Speak for Me to draft ...
"#
    )
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
                    "{}. {} ({:.0}%) -- {} suggestion",
                    idx + 1,
                    related.cognition_id,
                    related.score * 100.0,
                    related.relation_suggestion.as_deref().unwrap_or("similar")
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

RELATED COGNITIONS
{related}

SUGGESTED EFFECT
Add this Thought to ME as its own Cognition.

POSSIBLE ASSOCIATIONS
Inferred only. They are suggestions, not authority.

WHAT WILL CHANGE
1 Cognition added.
0 existing Cognitions changed.
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

fn suggest_relation(a: &str, b: &str) -> String {
    let a = a.to_ascii_lowercase();
    let b = b.to_ascii_lowercase();
    if (a.contains("only") && a.contains("final") && b.contains("possibility"))
        || (b.contains("only") && b.contains("final") && a.contains("possibility"))
    {
        "contradicts".to_string()
    } else if a.contains("again") || a.contains("return") || a.contains("recurr") {
        "recurs".to_string()
    } else {
        "similar".to_string()
    }
}

fn infer_conflicts_from_selected(selected: &[SelectedCognition]) -> Vec<String> {
    let has_final_only = selected
        .iter()
        .any(|item| item.reason.contains("final") || item.reason.contains("only"));
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
        .map(|item| format!("  {} -- {}", item.cognition_id, item.reason))
        .collect::<Vec<_>>()
        .join("\n");
    let conflicts = if run.conflicts.is_empty() {
        "None".to_string()
    } else {
        run.conflicts.join("\n  ")
    };
    let gaps = if run.gaps.is_empty() {
        "None".to_string()
    } else {
        run.gaps.join("\n  ")
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

fn string_field<'a>(value: &'a Value, field: &str) -> Result<&'a str> {
    value
        .get(field)
        .and_then(Value::as_str)
        .ok_or_else(|| invalid(format!("Missing string field: {field}")))
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

fn parse_proposed_associations(value: Option<&Value>) -> Result<Vec<ProposedAssociation>> {
    let Some(values) = value.and_then(Value::as_array) else {
        return Ok(Vec::new());
    };
    let mut out = Vec::new();
    for value in values {
        let relation = value
            .get("relation")
            .and_then(Value::as_str)
            .ok_or_else(|| invalid("Confirmed Association requires relation"))?;
        let from = string_array(value.get("fromCognitions"))
            .ok_or_else(|| invalid("Confirmed Association requires fromCognitions"))?;
        let to = string_array(value.get("toCognitions"))
            .ok_or_else(|| invalid("Confirmed Association requires toCognitions"))?;
        out.push(ProposedAssociation {
            relation: relation.to_string(),
            from_cognitions: from,
            to_cognitions: to,
            note_markdown: value
                .get("noteMarkdown")
                .and_then(Value::as_str)
                .map(str::to_string),
        });
    }
    Ok(out)
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
        format!("{new_text}\n<!-- MYMODEL:USER-BEGIN -->\n{section}\n<!-- MYMODEL:USER-END -->\n")
    } else {
        format!("{new_text}\n<!-- MYMODEL:USER-BEGIN -->\n<!-- MYMODEL:USER-END -->\n")
    };
    atomic_write(path, output.as_bytes())
}

fn extract_user_section(input: &str) -> Option<String> {
    let start = "<!-- MYMODEL:USER-BEGIN -->";
    let end = "<!-- MYMODEL:USER-END -->";
    let start_idx = input.find(start)? + start.len();
    let end_idx = input.find(end)?;
    Some(input[start_idx..end_idx].trim().to_string())
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
    r#"# ME Workspace Rules

- This directory is a ME workspace, not a software repository.
- Use the ME skill for Thought capture and semantic changes.
- Use `me ... --json` for deterministic operations.
- Never edit `.me/**` directly.
- Never edit `views/**` directly.
- New material must first become a Thought.
- Default to adding a Thought as its own Cognition after approval.
- Compare new input with current Cognitions before proposing Associations.
- Treat recurrence as meaningful, not redundant.
- Model output is a Proposal, never authority.
- Run `me decide` only after explicit user authorization.
- Publishing or external sharing is outside ME.
- Do not make network requests unless the user separately asks for unrelated research.
"#
}

fn workspace_skill_md() -> &'static str {
    r#"---
name: me
description: Capture a Thought, help the user decide whether to add it to ME as its own Cognition, suggest loose Associations without forcing merges, and use installed ME Apps for domain-specific tasks. Use when the user says "this is my thought," "add this to ME," asks what ME contains, or requests a task grounded in their Cognitions.
---

# ME Skill

1. Confirm workspace with `me home --json`.
2. Preserve exact user input as a Thought.
3. Retrieve possibly related Cognitions.
4. Propose adding the Thought as its own Cognition by default.
5. Show inferred Associations as optional and non-authoritative.
6. State that no existing Cognition will be rewritten.
7. Wait for explicit authorization.
8. Write the Decision.
9. Run `me decide`.
10. Report Cognitions added, Associations confirmed, and existing Cognitions changed.

Never publish, send, or externally share without a separate request.
"#
}

fn workspace_mental_model_md() -> &'static str {
    "A Thought is something I tell ME.\n\nA Cognition is a Thought I choose to keep.\n\nLoose at rest. Strict at use.\n"
}

fn workspace_cognition_library_md() -> &'static str {
    "ME tolerates overlapping, repeated, conditional, and contradictory Cognitions.\n"
}

fn workspace_associations_md() -> &'static str {
    "An inferred Association is a suggestion. A confirmed Association is a relation the user chose to preserve.\n"
}

fn workspace_apps_md() -> &'static str {
    "A ME App defines strict rules for using Cognitions in one domain. Outputs never become Cognitions automatically.\n"
}

fn workspace_authorization_md() -> &'static str {
    "Codex must not run `me decide` merely because a Proposal exists. Wait for explicit user authorization.\n"
}

fn workspace_cli_contract_md() -> &'static str {
    r#"# CLI Contract

Use `--json` for agent operations.

```bash
me home --json
me thought capture --file inbox/input.md --kind idea --json
me thought context <thought-id> --json
me proposal validate drafts/proposals/proposal.json --json
me proposal save drafts/proposals/proposal.json --json
me review <proposal-id> --json
me decide <proposal-id> --decision drafts/proposals/decision.json --json
```
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
include_inferred_associations = true
include_confirmed_associations = true
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
        return Err(invalid("ME v0.3 Apps cannot send or publish"));
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

    #[test]
    fn empty_workspace_initializes_as_me() {
        let dir = tempdir().unwrap();
        Workspace::init(dir.path(), false).unwrap();
        let ws = Workspace::open(dir.path()).unwrap();
        let home = ws.home("json").unwrap();
        assert_eq!(home["product"]["name"], "ME");
        assert!(dir.path().join(".me/refs/current").exists());
        assert!(dir.path().join("me.toml").exists());
        ws.fsck().unwrap();
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
        let context = ws.thought_context(thought_id, 5).unwrap();
        let proposal_path = dir.path().join("proposal.json");
        fs::write(
            &proposal_path,
            serde_json::to_vec_pretty(&context["proposalObject"]).unwrap(),
        )
        .unwrap();
        ws.save_proposal_file(&proposal_path).unwrap();
        let proposal_id = context["proposalObject"]["payload"]["proposalId"]
            .as_str()
            .unwrap();
        let decision_path = dir.path().join("decision.json");
        fs::write(&decision_path, r#"{"action":"add-cognition"}"#).unwrap();
        let result = ws.decide(proposal_id, &decision_path).unwrap();
        assert_eq!(result["cognitionsAdded"], 1);
        assert_eq!(result["existingCognitionsChanged"], 0);
        let after = ws.current().unwrap();
        assert_eq!(
            after["cognitions"].as_array().unwrap().len(),
            before["cognitions"].as_array().unwrap().len() + 1
        );
    }

    #[test]
    fn inferred_association_does_not_change_snapshot_but_confirmed_does() {
        let dir = tempdir().unwrap();
        Workspace::init(dir.path(), true).unwrap();
        let ws = Workspace::open(dir.path()).unwrap();
        let before = ws.current_ref().unwrap();
        let inferred = ws.association_infer(None).unwrap();
        assert_eq!(inferred["snapshotUnchanged"], before);
        assert_eq!(ws.current_ref().unwrap(), before);
        let cognitions = ws.cognition_list(Some("active".to_string())).unwrap();
        let items = cognitions["cognitions"].as_array().unwrap();
        let spec = dir.path().join("association.json");
        fs::write(
            &spec,
            format!(
                r#"{{"relation":"recurs","fromCognitions":["{}"],"toCognitions":["{}"]}}"#,
                items[0]["cognitionId"].as_str().unwrap(),
                items[1]["cognitionId"].as_str().unwrap()
            ),
        )
        .unwrap();
        let confirmed = ws.association_confirm(&spec).unwrap();
        assert_ne!(confirmed["snapshot"], before);
    }

    #[test]
    fn app_run_does_not_mutate_cognition_library() {
        let dir = tempdir().unwrap();
        Workspace::init(dir.path(), true).unwrap();
        let ws = Workspace::open(dir.path()).unwrap();
        let before = ws.cognition_list(Some("active".to_string())).unwrap();
        let task = dir.path().join("task.md");
        fs::write(&task, "Draft a reply about generative art and authorship.").unwrap();
        let run = ws.app_run("speak-for-me", &task, false).unwrap();
        assert_eq!(run["externalAction"], false);
        assert_eq!(run["cognitionLibraryChanged"], false);
        let after = ws.cognition_list(Some("active".to_string())).unwrap();
        assert_eq!(before["cognitions"], after["cognitions"]);
    }

    #[test]
    fn bundle_round_trip_passes_fsck() {
        let dir = tempdir().unwrap();
        Workspace::init(dir.path(), true).unwrap();
        let ws = Workspace::open(dir.path()).unwrap();
        let bundle = dir.path().join("bundle.tar");
        ws.bundle_create(&bundle).unwrap();
        ws.bundle_verify(&bundle).unwrap();
        let restored = tempdir().unwrap();
        let target = restored.path().join("restored");
        Workspace::bundle_restore(&bundle, &target).unwrap();
        Workspace::open(&target).unwrap().fsck().unwrap();
    }
}
