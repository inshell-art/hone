use std::collections::{BTreeMap, BTreeSet};
use std::fs::{self, File, OpenOptions};
use std::io::{Read, Write};
use std::path::{Component, Path, PathBuf};

use fs2::FileExt;
use hone_core::{
    is_sha_ref, operation_allowed, relationship_allowed, sha_ref, slugify, source_kind_allowed,
    strip_sha_prefix, ArticleEditionPayload, ArticleSegment, DecisionPayload, EventTarget,
    FacetRevisionPayload, GeneratedBy, HoneEventPayload, ObjectEnvelope, Origin, ProposalPayload,
    SnapshotPayload, SourcePayload, TreePayload, WorkspaceConfig, RELATIONSHIPS, SCHEMA_VERSION,
    SOURCE_KINDS, WORKSPACE_VERSION,
};
use hone_index::{rank_facets, FacetDoc, MatchResult};
use hone_markdown::{markdown_to_text, render_article, validate_facet_body, RenderedSegment};
use rusqlite::{params, Connection};
use serde::de::DeserializeOwned;
use serde::Serialize;
use serde_json::{json, Map, Value};
use sha2::{Digest, Sha256};
use tar::{Archive, Builder, Header};
use tempfile::NamedTempFile;
use thiserror::Error;
use time::{format_description::well_known::Rfc3339, OffsetDateTime};
use ulid::Ulid;
use walkdir::WalkDir;

pub type Result<T> = std::result::Result<T, HoneError>;

#[derive(Debug, Error)]
pub enum HoneError {
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

impl HoneError {
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

fn invalid(message: impl Into<String>) -> HoneError {
    HoneError::InvalidInput {
        code: "INVALID_INPUT",
        message: message.into(),
        details: json!({}),
    }
}

fn not_found(message: impl Into<String>) -> HoneError {
    HoneError::NotFound {
        code: "NOT_FOUND",
        message: message.into(),
        details: json!({}),
    }
}

fn integrity(message: impl Into<String>) -> HoneError {
    HoneError::Integrity {
        code: "INTEGRITY_FAILURE",
        message: message.into(),
        details: json!({}),
    }
}

fn now_rfc3339() -> Result<String> {
    OffsetDateTime::now_utc()
        .format(&Rfc3339)
        .map_err(|err| HoneError::Internal(err.into()))
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
    pub snapshot: SnapshotPayload,
    pub tree_hash: String,
    pub tree: TreePayload,
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
        if root.join(".hone/refs/current").exists() {
            return Err(invalid("Workspace is already initialized"));
        }
        fs::create_dir_all(&root).map_err(|err| HoneError::Internal(err.into()))?;
        let ws = Self { root };
        ws.create_layout()?;
        ws.write_config()?;
        ws.sync_workspace_docs()?;
        ws.write_initial_snapshot()?;
        ws.rebuild_index()?;
        if demo {
            let demo_result = ws.seed_demo()?;
            Ok(json!({
                "workspace": ws.root,
                "demo": demo_result
            }))
        } else {
            Ok(json!({
                "workspace": ws.root,
                "currentSnapshot": ws.current_ref()?
            }))
        }
    }

    pub fn new_workspace(path: impl AsRef<Path>, demo: bool) -> Result<Value> {
        let path = path.as_ref();
        if path.exists()
            && path
                .read_dir()
                .map_err(|err| HoneError::Internal(err.into()))?
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

    pub fn status(&self) -> Result<Value> {
        let config = self.config()?;
        let current = self.load_current()?;
        Ok(json!({
            "workspace": self.root,
            "workspaceId": config.workspace_id,
            "schemaVersion": config.schema_version,
            "currentSnapshot": current.snapshot_hash,
            "tree": current.tree_hash,
            "counts": {
                "sources": current.tree.sources.len(),
                "facets": current.tree.facets.len(),
                "articles": current.tree.articles.len(),
                "events": current.tree.events.len(),
                "proposals": current.tree.proposals.len(),
                "decisions": current.tree.decisions.len()
            }
        }))
    }

    pub fn doctor(&self, repair: bool) -> Result<Value> {
        let mut repaired = Vec::new();
        self.ensure_supported()?;
        if repair {
            self.regenerate_views()?;
            self.rebuild_index()?;
            self.sync_workspace_docs()?;
            repaired.push("derived-views");
            repaired.push("sqlite-index");
            repaired.push("codex-instructions");
        }
        Ok(json!({
            "workspace": self.root,
            "ok": true,
            "repair": repair,
            "repaired": repaired
        }))
    }

    pub fn fsck(&self) -> Result<Value> {
        let current_hash = self.current_ref()?;
        if !is_sha_ref(&current_hash) {
            return Err(integrity("Current ref is not a valid sha256 reference"));
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
            let mut bytes = fs::read(path).map_err(|err| HoneError::Internal(err.into()))?;
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

        self.read_object::<SnapshotPayload>(&current_hash, "snapshot")?;
        self.read_object::<TreePayload>(&current.snapshot.tree, "tree")?;
        for hash in current
            .tree
            .sources
            .values()
            .chain(current.tree.facets.values())
            .chain(current.tree.articles.values())
            .chain(current.tree.events.values())
            .chain(current.tree.proposals.values())
            .chain(current.tree.decisions.values())
        {
            if !checked.contains(hash) {
                return Err(integrity(format!("Tree references missing object {hash}")));
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
                ".agents/skills/hone/SKILL.md",
                ".agents/skills/hone/references/concepts.md",
                ".agents/skills/hone/references/relationships.md",
                ".agents/skills/hone/references/authorization.md",
                ".agents/skills/hone/references/cli-contract.md",
                ".agents/skills/hone/agents/openai.yaml"
            ]
        }))
    }

    pub fn capture(
        &self,
        file: impl AsRef<Path>,
        kind: &str,
        title: Option<String>,
    ) -> Result<Value> {
        if !source_kind_allowed(kind) {
            return Err(invalid(format!(
                "Unsupported source kind '{kind}'. Supported kinds: {}",
                SOURCE_KINDS.join(", ")
            )));
        }
        let body =
            fs::read_to_string(file.as_ref()).map_err(|err| HoneError::Internal(err.into()))?;
        self.with_lock(|| {
            let current = self.load_current()?;
            let duplicate_of =
                self.find_exact_source_duplicate(&current.tree, kind, title.as_deref(), &body)?;
            let payload = SourcePayload {
                source_id: new_id("src"),
                kind: kind.to_string(),
                title,
                body_markdown: body,
                origin: Origin {
                    origin_type: "local-input".to_string(),
                    uri: None,
                },
                captured_at: now_rfc3339()?,
                captured_by: self.config()?.default_actor,
            };
            let source_hash = self.write_object("source", &payload)?;
            let mut tree = current.tree.clone();
            tree.sources
                .insert(payload.source_id.clone(), source_hash.clone());
            let snapshot_hash = self.commit_tree(
                &current,
                tree,
                "capture",
                "local-user",
                format!("Capture {}", payload.source_id),
            )?;
            let mut warnings = Vec::new();
            if let Some(hash) = duplicate_of {
                warnings.push(format!("exact duplicate of {hash}"));
            }
            Ok(json!({
                "sourceId": payload.source_id,
                "source": source_hash,
                "snapshot": snapshot_hash,
                "warnings": warnings
            }))
        })
    }

    pub fn relate(&self, source_id_or_hash: &str, limit: usize) -> Result<Value> {
        let current = self.load_current()?;
        let (source_id, source_hash, source) =
            self.resolve_source(&current.tree, source_id_or_hash)?;
        let matches = self.match_source(&current.tree, &source.payload.body_markdown, limit)?;
        Ok(json!({
            "sourceId": source_id,
            "source": source_hash,
            "currentSnapshot": current.snapshot_hash,
            "matches": matches
        }))
    }

    pub fn proposal_context(&self, source_id_or_hash: &str, limit: usize) -> Result<Value> {
        let current = self.load_current()?;
        let (source_id, source_hash, source) =
            self.resolve_source(&current.tree, source_id_or_hash)?;
        let matches = self.match_source(&current.tree, &source.payload.body_markdown, limit)?;
        let mut facets = Vec::new();
        for matched in &matches {
            let facet = self
                .read_object::<FacetRevisionPayload>(&matched.facet_revision, "facet-revision")?;
            facets.push(json!({
                "facetId": matched.facet_id,
                "facetRevision": matched.facet_revision,
                "title": facet.payload.title,
                "bodyMarkdown": facet.payload.body_markdown,
                "bodyText": facet.payload.body_text,
                "score": matched.score,
                "components": matched.components,
                "matchedTerms": matched.matched_terms
            }));
        }
        Ok(json!({
            "source": {
                "sourceId": source_id,
                "hash": source_hash,
                "bodyMarkdown": source.payload.body_markdown,
                "kind": source.payload.kind,
                "title": source.payload.title
            },
            "currentSnapshot": current.snapshot_hash,
            "facets": facets,
            "relationships": RELATIONSHIPS,
            "operations": hone_core::OPERATIONS,
            "proposalObject": {
                "schemaVersion": SCHEMA_VERSION,
                "objectType": "proposal",
                "payload": {
                    "proposalId": "pro_<ULID>",
                    "source": "<source sha256>",
                    "baseSnapshot": "<current snapshot sha256>",
                    "candidates": [],
                    "recommendation": {},
                    "unresolved": [],
                    "generatedBy": { "host": "codex", "model": null },
                    "createdAt": "<RFC3339>"
                }
            }
        }))
    }

    pub fn validate_proposal_file(&self, file: impl AsRef<Path>) -> Result<Value> {
        let proposal = self.parse_proposal_file(file.as_ref())?;
        let current = self.load_current()?;
        let hash = self.validate_proposal(&proposal, &current, false)?;
        Ok(json!({
            "proposalId": proposal.payload.proposal_id,
            "proposal": hash,
            "valid": true
        }))
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
                .map_err(|err| HoneError::Internal(err.into()))?;
            atomic_write(
                &self.pending_proposal_ref(&proposal.payload.proposal_id),
                format!("{hash}\n").as_bytes(),
            )?;
            fs::create_dir_all(self.root.join("drafts/proposals"))
                .map_err(|err| HoneError::Internal(err.into()))?;
            let pretty = serde_json::to_vec_pretty(&proposal)
                .map_err(|err| HoneError::Internal(err.into()))?;
            atomic_write(
                &self
                    .root
                    .join("drafts/proposals")
                    .join(format!("{}.json", proposal.payload.proposal_id)),
                &pretty,
            )?;
            Ok(json!({
                "proposalId": proposal.payload.proposal_id,
                "proposal": hash,
                "status": "pending"
            }))
        })
    }

    pub fn show_proposal(&self, proposal_id_or_hash: &str) -> Result<Value> {
        let (hash, proposal) = self.resolve_proposal(proposal_id_or_hash)?;
        Ok(json!({
            "proposal": hash,
            "payload": proposal.payload
        }))
    }

    pub fn list_proposals(&self, status: Option<String>) -> Result<Value> {
        let mut proposals = Vec::new();
        let pending_dir = self.pending_proposals_dir();
        if pending_dir.exists() {
            for entry in fs::read_dir(pending_dir).map_err(|err| HoneError::Internal(err.into()))? {
                let entry = entry.map_err(|err| HoneError::Internal(err.into()))?;
                if entry
                    .file_type()
                    .map_err(|err| HoneError::Internal(err.into()))?
                    .is_file()
                {
                    let hash = fs::read_to_string(entry.path())
                        .map_err(|err| HoneError::Internal(err.into()))?;
                    let hash = hash.trim().to_string();
                    let proposal = self.read_object::<ProposalPayload>(&hash, "proposal")?;
                    proposals.push(json!({
                        "proposalId": proposal.payload.proposal_id,
                        "proposal": hash,
                        "status": "pending",
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
        let (_, _, source) = self.resolve_source(&current.tree, &proposal.payload.source)?;
        let mut candidate_views = Vec::new();
        for candidate in &proposal.payload.candidates {
            let facet = self
                .read_object::<FacetRevisionPayload>(&candidate.facet_revision, "facet-revision")?;
            candidate_views.push(json!({
                "facetId": candidate.facet_id,
                "facetRevision": candidate.facet_revision,
                "relationship": candidate.relationship,
                "title": facet.payload.title,
                "explanation": candidate.explanation
            }));
        }
        let text = format_review(
            &proposal_hash,
            &proposal.payload,
            &source.payload.body_markdown,
            &candidate_views,
        );
        Ok(json!({
            "format": format,
            "proposal": proposal_hash,
            "review": text,
            "candidates": candidate_views
        }))
    }

    pub fn approve(
        &self,
        proposal_id_or_hash: &str,
        decision_file: impl AsRef<Path>,
    ) -> Result<Value> {
        self.with_lock(|| {
            let current = self.load_current()?;
            let (proposal_hash, proposal) = self.resolve_proposal(proposal_id_or_hash)?;
            if proposal.payload.base_snapshot != current.snapshot_hash {
                return Err(HoneError::StaleProposal {
                    code: "STALE_PROPOSAL",
                    message: format!(
                        "Proposal was based on snapshot {}, current is {}",
                        proposal.payload.base_snapshot, current.snapshot_hash
                    ),
                    details: json!({
                        "proposalBaseSnapshot": proposal.payload.base_snapshot,
                        "currentSnapshot": current.snapshot_hash
                    }),
                });
            }
            self.validate_proposal(&proposal, &current, false)?;
            let mut decision_payload = self.parse_decision_file(
                decision_file.as_ref(),
                &proposal_hash,
                &proposal.payload,
                "approve",
            )?;
            decision_payload.decision_id = new_id("dec");
            decision_payload.proposal = proposal_hash.clone();
            decision_payload.base_snapshot = current.snapshot_hash.clone();
            if decision_payload.actor.is_empty() {
                decision_payload.actor = self.config()?.default_actor;
            }
            if decision_payload.decided_at.is_empty() {
                decision_payload.decided_at = now_rfc3339()?;
            }
            let decision_hash = self.write_object("decision", &decision_payload)?;
            let outcome = self.apply_decision(
                &current,
                &proposal_hash,
                &proposal.payload,
                &decision_hash,
                &decision_payload,
            )?;
            Ok(outcome)
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
            if proposal.payload.base_snapshot != current.snapshot_hash {
                return Err(HoneError::StaleProposal {
                    code: "STALE_PROPOSAL",
                    message: format!(
                        "Proposal was based on snapshot {}, current is {}",
                        proposal.payload.base_snapshot, current.snapshot_hash
                    ),
                    details: json!({}),
                });
            }
            let operation = if action == "reject" {
                json!({ "type": "reject" })
            } else {
                json!({ "type": "defer" })
            };
            let decision_payload = DecisionPayload {
                decision_id: new_id("dec"),
                proposal: proposal_hash.clone(),
                base_snapshot: current.snapshot_hash.clone(),
                action: action.to_string(),
                actor: self.config()?.default_actor,
                final_operation: operation,
                note,
                decided_at: now_rfc3339()?,
            };
            let decision_hash = self.write_object("decision", &decision_payload)?;
            let mut tree = current.tree.clone();
            tree.proposals
                .insert(proposal.payload.proposal_id.clone(), proposal_hash.clone());
            tree.decisions
                .insert(decision_payload.decision_id.clone(), decision_hash.clone());
            self.remove_pending_proposal(&proposal.payload.proposal_id)?;
            let snapshot_hash = self.commit_tree(
                &current,
                tree,
                action,
                &decision_payload.actor,
                format!("{action} {}", proposal.payload.proposal_id),
            )?;
            Ok(json!({
                "proposalId": proposal.payload.proposal_id,
                "proposal": proposal_hash,
                "decisionId": decision_payload.decision_id,
                "decision": decision_hash,
                "snapshot": snapshot_hash
            }))
        })
    }

    pub fn facet_list(&self) -> Result<Value> {
        let current = self.load_current()?;
        let mut facets = Vec::new();
        for (facet_id, hash) in &current.tree.facets {
            let facet = self.read_object::<FacetRevisionPayload>(hash, "facet-revision")?;
            facets.push(json!({
                "facetId": facet_id,
                "revision": facet.payload.revision,
                "title": facet.payload.title,
                "object": hash
            }));
        }
        Ok(json!({ "facets": facets, "currentSnapshot": current.snapshot_hash }))
    }

    pub fn facet_show(&self, facet_id: &str, revision: Option<u64>) -> Result<Value> {
        let current = self.load_current()?;
        let current_hash = current
            .tree
            .facets
            .get(facet_id)
            .ok_or_else(|| not_found(format!("Facet not found: {facet_id}")))?;
        let mut hash = current_hash.clone();
        let mut facet = self.read_object::<FacetRevisionPayload>(&hash, "facet-revision")?;
        if let Some(target_revision) = revision {
            loop {
                if facet.payload.revision == target_revision {
                    break;
                }
                let parent = facet.payload.parent_revision.clone().ok_or_else(|| {
                    not_found(format!(
                        "Facet revision not found: {facet_id} r{target_revision}"
                    ))
                })?;
                hash = parent;
                facet = self.read_object::<FacetRevisionPayload>(&hash, "facet-revision")?;
            }
        }
        Ok(json!({ "object": hash, "payload": facet.payload }))
    }

    pub fn article_list(&self) -> Result<Value> {
        let current = self.load_current()?;
        let mut articles = Vec::new();
        for (article_id, hash) in &current.tree.articles {
            let article = self.read_object::<ArticleEditionPayload>(hash, "article-edition")?;
            articles.push(json!({
                "articleId": article_id,
                "edition": article.payload.edition,
                "title": article.payload.title,
                "object": hash
            }));
        }
        Ok(json!({ "articles": articles, "currentSnapshot": current.snapshot_hash }))
    }

    pub fn article_show(
        &self,
        article_id: &str,
        edition: Option<u64>,
        format: &str,
    ) -> Result<Value> {
        let current = self.load_current()?;
        let current_hash = current
            .tree
            .articles
            .get(article_id)
            .ok_or_else(|| not_found(format!("Article not found: {article_id}")))?;
        let mut hash = current_hash.clone();
        let mut article = self.read_object::<ArticleEditionPayload>(&hash, "article-edition")?;
        if let Some(target_edition) = edition {
            loop {
                if article.payload.edition == target_edition {
                    break;
                }
                let parent = article.payload.parent_edition.clone().ok_or_else(|| {
                    not_found(format!(
                        "Article edition not found: {article_id} e{target_edition}"
                    ))
                })?;
                hash = parent;
                article = self.read_object::<ArticleEditionPayload>(&hash, "article-edition")?;
            }
        }
        let rendered = self.render_article_payload(&article.payload)?;
        if format == "markdown" {
            Ok(json!({ "article": hash, "markdown": rendered }))
        } else {
            Ok(json!({ "article": hash, "payload": article.payload, "markdown": rendered }))
        }
    }

    pub fn history(&self, facet: Option<String>, article: Option<String>) -> Result<Value> {
        let current = self.load_current()?;
        let snapshots = self.snapshot_chain(&current.snapshot_hash)?;
        let mut entries = Vec::new();
        if let Some(facet_id) = facet {
            let current_hash = current
                .tree
                .facets
                .get(&facet_id)
                .ok_or_else(|| not_found(format!("Facet not found: {facet_id}")))?;
            for (hash, payload) in self.facet_revision_chain(current_hash)? {
                entries.push(json!({
                    "kind": "facet",
                    "facetId": facet_id,
                    "object": hash,
                    "revision": payload.revision,
                    "createdAt": payload.created_at,
                    "changeKind": payload.change_kind
                }));
            }
        } else if let Some(article_id) = article {
            let current_hash = current
                .tree
                .articles
                .get(&article_id)
                .ok_or_else(|| not_found(format!("Article not found: {article_id}")))?;
            for (hash, payload) in self.article_edition_chain(current_hash)? {
                entries.push(json!({
                    "kind": "article",
                    "articleId": article_id,
                    "object": hash,
                    "edition": payload.edition,
                    "createdAt": payload.created_at
                }));
            }
        }
        Ok(json!({
            "currentSnapshot": current.snapshot_hash,
            "snapshots": snapshots,
            "entries": entries
        }))
    }

    pub fn diff(&self, snapshot_a: &str, snapshot_b: &str, format: &str) -> Result<Value> {
        let a = self.snapshot_tree(snapshot_a)?;
        let b = self.snapshot_tree(snapshot_b)?;
        let mut changed_facets = Vec::new();
        for key in a
            .facets
            .keys()
            .chain(b.facets.keys())
            .collect::<BTreeSet<_>>()
        {
            if a.facets.get(key) != b.facets.get(key) {
                changed_facets.push(key.clone());
            }
        }
        let mut changed_articles = Vec::new();
        for key in a
            .articles
            .keys()
            .chain(b.articles.keys())
            .collect::<BTreeSet<_>>()
        {
            if a.articles.get(key) != b.articles.get(key) {
                changed_articles.push(key.clone());
            }
        }
        let added_sources: Vec<_> = b
            .sources
            .keys()
            .filter(|key| !a.sources.contains_key(*key))
            .cloned()
            .collect();
        let added_events: Vec<_> = b
            .events
            .keys()
            .filter(|key| !a.events.contains_key(*key))
            .cloned()
            .collect();
        let text = format!(
            "Sources added: {}\nEvents added: {}\nFacets changed: {}\nArticles changed: {}",
            added_sources.len(),
            added_events.len(),
            changed_facets.len(),
            changed_articles.len()
        );
        Ok(json!({
            "format": format,
            "snapshotA": snapshot_a,
            "snapshotB": snapshot_b,
            "text": text,
            "addedSources": added_sources,
            "addedEvents": added_events,
            "changedFacets": changed_facets,
            "changedArticles": changed_articles
        }))
    }

    pub fn snapshot_list(&self) -> Result<Value> {
        let current = self.current_ref()?;
        Ok(json!({
            "currentSnapshot": current,
            "snapshots": self.snapshot_chain(&current)?
        }))
    }

    pub fn snapshot_show(&self, snapshot_id: &str) -> Result<Value> {
        let snapshot = self.read_object::<SnapshotPayload>(snapshot_id, "snapshot")?;
        let tree = self.read_object::<TreePayload>(&snapshot.payload.tree, "tree")?;
        Ok(json!({
            "snapshot": snapshot_id,
            "payload": snapshot.payload,
            "tree": tree.payload
        }))
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
            Ok(json!({
                "restoredFrom": snapshot_id,
                "snapshot": snapshot_hash
            }))
        })
    }

    pub fn index_rebuild(&self) -> Result<Value> {
        self.rebuild_index()?;
        Ok(json!({
            "workspace": self.root,
            "index": self.root.join(".hone/index.sqlite"),
            "rebuilt": true
        }))
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
        let file = File::create(output).map_err(|err| HoneError::Internal(err.into()))?;
        let mut builder = Builder::new(file);
        append_bytes_to_tar(
            &mut builder,
            "manifest.json",
            serde_json::to_vec_pretty(&manifest).unwrap(),
        )?;
        append_file_to_tar(&mut builder, "hone.toml", &self.root.join("hone.toml"))?;
        append_file_to_tar(
            &mut builder,
            "refs/current",
            &self.root.join(".hone/refs/current"),
        )?;
        let journal = self.root.join(".hone/journal/transitions.ndjson");
        if journal.exists() {
            append_file_to_tar(&mut builder, "journal/transitions.ndjson", &journal)?;
        } else {
            append_bytes_to_tar(&mut builder, "journal/transitions.ndjson", Vec::new())?;
        }
        for object in self.object_file_paths()? {
            let rel = object
                .strip_prefix(self.objects_dir())
                .map_err(|err| HoneError::Internal(err.into()))?;
            let tar_path = PathBuf::from("objects").join(rel);
            append_file_to_tar(&mut builder, tar_path, &object)?;
        }
        builder
            .finish()
            .map_err(|err| HoneError::Internal(err.into()))?;
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
                .map_err(|err| HoneError::Internal(err.into()))?
                .next()
                .is_some()
        {
            return Err(invalid(format!(
                "Target directory is not empty: {}",
                target.display()
            )));
        }
        let verified = verify_bundle(file)?;
        fs::create_dir_all(target).map_err(|err| HoneError::Internal(err.into()))?;
        let ws = Workspace {
            root: target.to_path_buf(),
        };
        ws.create_layout()?;
        let archive_file = File::open(file).map_err(|err| HoneError::Internal(err.into()))?;
        let mut archive = Archive::new(archive_file);
        for entry in archive
            .entries()
            .map_err(|err| HoneError::Internal(err.into()))?
        {
            let mut entry = entry.map_err(|err| HoneError::Internal(err.into()))?;
            let path = entry
                .path()
                .map_err(|err| HoneError::Internal(err.into()))?
                .to_path_buf();
            validate_archive_path(&path)?;
            let dest = bundle_path_to_workspace_path(target, &path)?;
            if let Some(parent) = dest.parent() {
                fs::create_dir_all(parent).map_err(|err| HoneError::Internal(err.into()))?;
            }
            entry
                .unpack(&dest)
                .map_err(|err| HoneError::Internal(err.into()))?;
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

    pub fn export_article(
        &self,
        article_id: &str,
        format: &str,
        output: impl AsRef<Path>,
    ) -> Result<Value> {
        let article = self.article_show(article_id, None, format)?;
        let bytes = if format == "markdown" {
            article["markdown"]
                .as_str()
                .unwrap_or_default()
                .as_bytes()
                .to_vec()
        } else {
            serde_json::to_vec_pretty(&article).map_err(|err| HoneError::Internal(err.into()))?
        };
        atomic_write(output.as_ref(), &bytes)?;
        Ok(json!({ "output": output.as_ref(), "articleId": article_id, "format": format }))
    }

    pub fn export_workspace(&self, output: impl AsRef<Path>) -> Result<Value> {
        let current = self.load_current()?;
        let data = json!({
            "currentSnapshot": current.snapshot_hash,
            "tree": current.tree,
            "snapshots": self.snapshot_chain(&current.snapshot_hash)?
        });
        atomic_write(
            output.as_ref(),
            &serde_json::to_vec_pretty(&data).map_err(|err| HoneError::Internal(err.into()))?,
        )?;
        Ok(json!({ "output": output.as_ref(), "format": "json" }))
    }

    fn ensure_supported(&self) -> Result<()> {
        let version_path = self.root.join(".hone/VERSION");
        if !version_path.exists() {
            return Err(not_found(format!(
                "Not a Hone workspace: {}",
                self.root.display()
            )));
        }
        let config_path = self.root.join("hone.toml");
        if config_path.exists() {
            let config = self.config()?;
            if config.schema_version != SCHEMA_VERSION {
                return Err(HoneError::UnsupportedWorkspace {
                    code: "UNSUPPORTED_WORKSPACE_VERSION",
                    message: format!(
                        "Unsupported workspace schema version {}",
                        config.schema_version
                    ),
                    details: json!({ "schemaVersion": config.schema_version }),
                });
            }
        }
        Ok(())
    }

    fn config(&self) -> Result<WorkspaceConfig> {
        let raw = fs::read_to_string(self.root.join("hone.toml"))
            .map_err(|err| HoneError::Internal(err.into()))?;
        toml::from_str(&raw).map_err(|err| HoneError::Internal(err.into()))
    }

    fn create_layout(&self) -> Result<()> {
        for dir in [
            "inbox",
            "drafts/proposals",
            "views/current/facets",
            "views/current/articles",
            "views/history",
            "views/sources",
            "exports",
            ".hone/objects",
            ".hone/refs",
            ".hone/journal",
            ".hone/tmp",
            ".hone/pending/proposals",
            ".agents/skills/hone/references",
            ".agents/skills/hone/agents",
        ] {
            fs::create_dir_all(self.root.join(dir))
                .map_err(|err| HoneError::Internal(err.into()))?;
        }
        atomic_write(
            &self.root.join(".hone/VERSION"),
            format!("{WORKSPACE_VERSION}\n").as_bytes(),
        )?;
        if !self.root.join(".hone/lock").exists() {
            File::create(self.root.join(".hone/lock"))
                .map_err(|err| HoneError::Internal(err.into()))?;
        }
        Ok(())
    }

    fn write_config(&self) -> Result<()> {
        let name = self
            .root
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("My Hone")
            .to_string();
        let config = WorkspaceConfig::new(new_id("hws"), name);
        let toml =
            toml::to_string_pretty(&config).map_err(|err| HoneError::Internal(err.into()))?;
        atomic_write(&self.root.join("hone.toml"), toml.as_bytes())
    }

    fn write_initial_snapshot(&self) -> Result<()> {
        let tree = TreePayload::default();
        let tree_hash = self.write_object("tree", &tree)?;
        let snapshot = SnapshotPayload {
            parent: None,
            tree: tree_hash.clone(),
            operation: "init".to_string(),
            actor: "hone".to_string(),
            message: "Initialize Hone workspace".to_string(),
            created_at: now_rfc3339()?,
        };
        let snapshot_hash = self.write_object("snapshot", &snapshot)?;
        atomic_write(
            &self.root.join(".hone/refs/current"),
            format!("{snapshot_hash}\n").as_bytes(),
        )?;
        self.append_journal(None, &snapshot_hash, "init", "Initialize Hone workspace")?;
        self.regenerate_views()?;
        Ok(())
    }

    fn seed_demo(&self) -> Result<Value> {
        self.with_lock(|| {
            let current = self.load_current()?;
            let source_payload = SourcePayload {
                source_id: new_id("src"),
                kind: "note".to_string(),
                title: Some("Demo seed".to_string()),
                body_markdown: "Create the initial Generative Art facets.".to_string(),
                origin: Origin {
                    origin_type: "local-input".to_string(),
                    uri: None,
                },
                captured_at: now_rfc3339()?,
                captured_by: "local-user".to_string(),
            };
            let source_hash = self.write_object("source", &source_payload)?;
            let proposal_payload = ProposalPayload {
                proposal_id: new_id("pro"),
                source: source_hash.clone(),
                base_snapshot: current.snapshot_hash.clone(),
                candidates: vec![],
                recommendation: json!({
                    "operation": "create-article",
                    "articleId": "article:generative-art",
                    "title": "Generative Art"
                }),
                unresolved: vec![],
                generated_by: GeneratedBy {
                    host: "hone-demo".to_string(),
                    model: None,
                },
                created_at: now_rfc3339()?,
            };
            let proposal_hash = self.write_object("proposal", &proposal_payload)?;
            let decision_payload = DecisionPayload {
                decision_id: new_id("dec"),
                proposal: proposal_hash.clone(),
                base_snapshot: current.snapshot_hash.clone(),
                action: "approve".to_string(),
                actor: "local-user".to_string(),
                final_operation: json!({
                    "type": "create-article",
                    "articleId": "article:generative-art",
                    "title": "Generative Art",
                    "facets": [
                        {
                            "facetId": "facet:generative-art/definition",
                            "title": "Definition",
                            "bodyMarkdown": "Generative art is an artwork produced by a generative system."
                        },
                        {
                            "facetId": "facet:generative-art/authorship",
                            "title": "Authorship",
                            "bodyMarkdown": "The artist authors the system, constraints, and conditions through which outcomes emerge."
                        }
                    ]
                }),
                note: Some("Demo initial snapshot".to_string()),
                decided_at: now_rfc3339()?,
            };
            let decision_hash = self.write_object("decision", &decision_payload)?;
            let mut tree = current.tree.clone();
            tree.sources
                .insert(source_payload.source_id.clone(), source_hash.clone());
            tree.proposals
                .insert(proposal_payload.proposal_id.clone(), proposal_hash.clone());
            tree.decisions
                .insert(decision_payload.decision_id.clone(), decision_hash.clone());

            let mut segments = Vec::new();
            for facet in decision_payload
                .final_operation
                .get("facets")
                .and_then(Value::as_array)
                .ok_or_else(|| invalid("Demo seed facets missing"))?
            {
                let facet_id = string_field(facet, "facetId")?;
                let title = string_field(facet, "title")?;
                let body = string_field(facet, "bodyMarkdown")?;
                validate_facet_body(body).map_err(|err| invalid(err.to_string()))?;
                let facet_payload = FacetRevisionPayload {
                    facet_id: facet_id.to_string(),
                    revision: 1,
                    title: title.to_string(),
                    body_markdown: body.to_string(),
                    body_text: markdown_to_text(body),
                    parent_revision: None,
                    change_kind: "create-facet".to_string(),
                    derived_from_sources: vec![source_hash.clone()],
                    honed_against_facets: vec![],
                    decision: decision_hash.clone(),
                    created_at: now_rfc3339()?,
                };
                let facet_hash = self.write_object("facet-revision", &facet_payload)?;
                tree.facets.insert(facet_id.to_string(), facet_hash.clone());
                segments.push(ArticleSegment::facet(facet_id, facet_hash));
            }
            let article_payload = ArticleEditionPayload {
                article_id: "article:generative-art".to_string(),
                edition: 1,
                title: "Generative Art".to_string(),
                parent_edition: None,
                segments,
                decision: decision_hash.clone(),
                created_at: now_rfc3339()?,
            };
            let article_hash = self.write_object("article-edition", &article_payload)?;
            tree.articles
                .insert(article_payload.article_id.clone(), article_hash.clone());
            let event_payload = HoneEventPayload {
                event_id: new_id("hev"),
                source: source_hash.clone(),
                relationship: "novel".to_string(),
                targets: vec![],
                effect: json!({
                    "type": "create-article",
                    "articleEdition": article_hash
                }),
                proposal: proposal_hash.clone(),
                decision: decision_hash.clone(),
                recorded_at: now_rfc3339()?,
            };
            let event_hash = self.write_object("hone-event", &event_payload)?;
            tree.events
                .insert(event_payload.event_id.clone(), event_hash.clone());
            let snapshot_hash = self.commit_tree(
                &current,
                tree,
                "demo-seed",
                "local-user",
                "Create demo Generative Art facets".to_string(),
            )?;
            Ok(json!({
                "sourceId": source_payload.source_id,
                "source": source_hash,
                "proposalId": proposal_payload.proposal_id,
                "proposal": proposal_hash,
                "decisionId": decision_payload.decision_id,
                "decision": decision_hash,
                "eventId": event_payload.event_id,
                "event": event_hash,
                "article": article_payload.article_id,
                "articleEdition": article_hash,
                "snapshot": snapshot_hash
            }))
        })
    }

    fn apply_decision(
        &self,
        current: &CurrentState,
        proposal_hash: &str,
        proposal: &ProposalPayload,
        decision_hash: &str,
        decision: &DecisionPayload,
    ) -> Result<Value> {
        let op_type = decision_operation_type(&decision.final_operation)?.to_string();
        let mut tree = current.tree.clone();
        tree.proposals
            .insert(proposal.proposal_id.clone(), proposal_hash.to_string());
        tree.decisions
            .insert(decision.decision_id.clone(), decision_hash.to_string());
        let mut result = json!({
            "proposalId": proposal.proposal_id,
            "proposal": proposal_hash,
            "decisionId": decision.decision_id,
            "decision": decision_hash,
            "operation": op_type
        });

        match op_type.as_str() {
            "revise-facet" => {
                let target = string_field(&decision.final_operation, "targetFacetId")?;
                let base_revision = string_field(&decision.final_operation, "baseFacetRevision")?;
                let title = string_field(&decision.final_operation, "title")?;
                let body = string_field(&decision.final_operation, "bodyMarkdown")?;
                validate_facet_body(body).map_err(|err| invalid(err.to_string()))?;
                let current_hash = tree
                    .facets
                    .get(target)
                    .ok_or_else(|| not_found(format!("Facet not found: {target}")))?
                    .clone();
                if current_hash != base_revision {
                    return Err(HoneError::StaleProposal {
                        code: "STALE_PROPOSAL",
                        message: format!("Facet {target} is no longer at {base_revision}"),
                        details: json!({
                            "targetFacetId": target,
                            "baseFacetRevision": base_revision,
                            "currentFacetRevision": current_hash
                        }),
                    });
                }
                let base =
                    self.read_object::<FacetRevisionPayload>(base_revision, "facet-revision")?;
                if normalize_for_noop(&base.payload.title) == normalize_for_noop(title)
                    && normalize_for_noop(&base.payload.body_markdown) == normalize_for_noop(body)
                {
                    return Err(invalid(
                        "No-op facet revision rejected: title and body are unchanged",
                    ));
                }
                let relationship = proposal
                    .candidates
                    .iter()
                    .find(|candidate| candidate.facet_id == target)
                    .map(|candidate| candidate.relationship.as_str())
                    .unwrap_or("deepening");
                let facet_payload = FacetRevisionPayload {
                    facet_id: target.to_string(),
                    revision: base.payload.revision + 1,
                    title: title.to_string(),
                    body_markdown: body.to_string(),
                    body_text: markdown_to_text(body),
                    parent_revision: Some(base_revision.to_string()),
                    change_kind: relationship.to_string(),
                    derived_from_sources: vec![proposal.source.clone()],
                    honed_against_facets: vec![base_revision.to_string()],
                    decision: decision_hash.to_string(),
                    created_at: now_rfc3339()?,
                };
                let new_facet_hash = self.write_object("facet-revision", &facet_payload)?;
                tree.facets
                    .insert(target.to_string(), new_facet_hash.clone());
                let changed_articles = self.propagate_article_revisions(
                    &mut tree,
                    target,
                    base_revision,
                    &new_facet_hash,
                    decision_hash,
                )?;
                let event_payload = HoneEventPayload {
                    event_id: new_id("hev"),
                    source: proposal.source.clone(),
                    relationship: relationship.to_string(),
                    targets: vec![EventTarget {
                        facet_id: target.to_string(),
                        facet_revision: base_revision.to_string(),
                    }],
                    effect: json!({
                        "type": "facet-revision",
                        "facetRevision": new_facet_hash
                    }),
                    proposal: proposal_hash.to_string(),
                    decision: decision_hash.to_string(),
                    recorded_at: now_rfc3339()?,
                };
                let event_hash = self.write_object("hone-event", &event_payload)?;
                tree.events
                    .insert(event_payload.event_id.clone(), event_hash.clone());
                result["eventId"] = json!(event_payload.event_id);
                result["event"] = json!(event_hash);
                result["facetId"] = json!(target);
                result["facetRevision"] = json!(new_facet_hash);
                result["articleEditions"] = json!(changed_articles);
            }
            "create-facet" => {
                let title = string_field(&decision.final_operation, "title")?;
                let body = string_field(&decision.final_operation, "bodyMarkdown")?;
                validate_facet_body(body).map_err(|err| invalid(err.to_string()))?;
                let facet_id = decision
                    .final_operation
                    .get("facetId")
                    .and_then(Value::as_str)
                    .map(str::to_string)
                    .unwrap_or_else(|| format!("facet:uncategorized/{}", slugify(title)));
                if tree.facets.contains_key(&facet_id) {
                    return Err(invalid(format!("Facet already exists: {facet_id}")));
                }
                let facet_payload = FacetRevisionPayload {
                    facet_id: facet_id.clone(),
                    revision: 1,
                    title: title.to_string(),
                    body_markdown: body.to_string(),
                    body_text: markdown_to_text(body),
                    parent_revision: None,
                    change_kind: "novel".to_string(),
                    derived_from_sources: vec![proposal.source.clone()],
                    honed_against_facets: vec![],
                    decision: decision_hash.to_string(),
                    created_at: now_rfc3339()?,
                };
                let facet_hash = self.write_object("facet-revision", &facet_payload)?;
                tree.facets.insert(facet_id.clone(), facet_hash.clone());
                let event_payload = HoneEventPayload {
                    event_id: new_id("hev"),
                    source: proposal.source.clone(),
                    relationship: "novel".to_string(),
                    targets: vec![],
                    effect: json!({ "type": "facet-revision", "facetRevision": facet_hash }),
                    proposal: proposal_hash.to_string(),
                    decision: decision_hash.to_string(),
                    recorded_at: now_rfc3339()?,
                };
                let event_hash = self.write_object("hone-event", &event_payload)?;
                tree.events
                    .insert(event_payload.event_id.clone(), event_hash.clone());
                result["eventId"] = json!(event_payload.event_id);
                result["event"] = json!(event_hash);
                result["facetId"] = json!(facet_id);
                result["facetRevision"] = json!(facet_hash);
            }
            "record-recurrence" | "record-reinforcement" | "record-challenge" => {
                let target = string_field(&decision.final_operation, "targetFacetId")?;
                let base_revision = decision
                    .final_operation
                    .get("baseFacetRevision")
                    .and_then(Value::as_str)
                    .or_else(|| tree.facets.get(target).map(String::as_str))
                    .ok_or_else(|| not_found(format!("Facet not found: {target}")))?;
                if tree.facets.get(target).map(String::as_str) != Some(base_revision) {
                    return Err(HoneError::StaleProposal {
                        code: "STALE_PROPOSAL",
                        message: format!("Facet {target} is no longer at {base_revision}"),
                        details: json!({}),
                    });
                }
                let relationship = op_type.trim_start_matches("record-");
                let event_payload = HoneEventPayload {
                    event_id: new_id("hev"),
                    source: proposal.source.clone(),
                    relationship: relationship.to_string(),
                    targets: vec![EventTarget {
                        facet_id: target.to_string(),
                        facet_revision: base_revision.to_string(),
                    }],
                    effect: json!({ "type": "no-facet-revision" }),
                    proposal: proposal_hash.to_string(),
                    decision: decision_hash.to_string(),
                    recorded_at: now_rfc3339()?,
                };
                let event_hash = self.write_object("hone-event", &event_payload)?;
                tree.events
                    .insert(event_payload.event_id.clone(), event_hash.clone());
                result["eventId"] = json!(event_payload.event_id);
                result["event"] = json!(event_hash);
                result["facetId"] = json!(target);
                result["facetRevision"] = json!(base_revision);
                result["facetRevisionCreated"] = json!(false);
            }
            "create-article" => {
                let article_id = decision
                    .final_operation
                    .get("articleId")
                    .and_then(Value::as_str)
                    .map(str::to_string)
                    .unwrap_or_else(|| {
                        let title = decision
                            .final_operation
                            .get("title")
                            .and_then(Value::as_str)
                            .unwrap_or("Untitled");
                        format!("article:{}", slugify(title))
                    });
                if tree.articles.contains_key(&article_id) {
                    return Err(invalid(format!("Article already exists: {article_id}")));
                }
                let title = string_field(&decision.final_operation, "title")?;
                let mut segments = Vec::new();
                if let Some(facets) = decision
                    .final_operation
                    .get("facets")
                    .and_then(Value::as_array)
                {
                    for facet in facets {
                        let facet_id = string_field(facet, "facetId")?;
                        let facet_title = string_field(facet, "title")?;
                        let body = string_field(facet, "bodyMarkdown")?;
                        validate_facet_body(body).map_err(|err| invalid(err.to_string()))?;
                        let facet_payload = FacetRevisionPayload {
                            facet_id: facet_id.to_string(),
                            revision: 1,
                            title: facet_title.to_string(),
                            body_markdown: body.to_string(),
                            body_text: markdown_to_text(body),
                            parent_revision: None,
                            change_kind: "novel".to_string(),
                            derived_from_sources: vec![proposal.source.clone()],
                            honed_against_facets: vec![],
                            decision: decision_hash.to_string(),
                            created_at: now_rfc3339()?,
                        };
                        let facet_hash = self.write_object("facet-revision", &facet_payload)?;
                        tree.facets.insert(facet_id.to_string(), facet_hash.clone());
                        segments.push(ArticleSegment::facet(facet_id, facet_hash));
                    }
                } else if let Some(raw_segments) = decision
                    .final_operation
                    .get("segments")
                    .and_then(Value::as_array)
                {
                    segments = serde_json::from_value(Value::Array(raw_segments.clone()))
                        .map_err(|err| HoneError::Internal(err.into()))?;
                }
                let article_payload = ArticleEditionPayload {
                    article_id: article_id.clone(),
                    edition: 1,
                    title: title.to_string(),
                    parent_edition: None,
                    segments,
                    decision: decision_hash.to_string(),
                    created_at: now_rfc3339()?,
                };
                let article_hash = self.write_object("article-edition", &article_payload)?;
                tree.articles
                    .insert(article_id.clone(), article_hash.clone());
                let event_payload = HoneEventPayload {
                    event_id: new_id("hev"),
                    source: proposal.source.clone(),
                    relationship: "novel".to_string(),
                    targets: vec![],
                    effect: json!({ "type": "create-article", "articleEdition": article_hash }),
                    proposal: proposal_hash.to_string(),
                    decision: decision_hash.to_string(),
                    recorded_at: now_rfc3339()?,
                };
                let event_hash = self.write_object("hone-event", &event_payload)?;
                tree.events
                    .insert(event_payload.event_id.clone(), event_hash.clone());
                result["eventId"] = json!(event_payload.event_id);
                result["event"] = json!(event_hash);
                result["articleId"] = json!(article_id);
                result["articleEdition"] = json!(article_hash);
            }
            _ => return Err(invalid(format!("Unsupported final operation: {op_type}"))),
        }

        self.remove_pending_proposal(&proposal.proposal_id)?;
        let snapshot_hash = self.commit_tree(
            current,
            tree,
            "approve",
            &decision.actor,
            format!("Approve {}", proposal.proposal_id),
        )?;
        result["snapshot"] = json!(snapshot_hash);
        Ok(result)
    }

    fn with_lock<T>(&self, f: impl FnOnce() -> Result<T>) -> Result<T> {
        fs::create_dir_all(self.root.join(".hone"))
            .map_err(|err| HoneError::Internal(err.into()))?;
        let lock_path = self.root.join(".hone/lock");
        let mut file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(&lock_path)
            .map_err(|err| HoneError::Internal(err.into()))?;
        if let Err(err) = file.try_lock_exclusive() {
            return Err(HoneError::WorkspaceLocked {
                code: "WORKSPACE_LOCKED",
                message: format!(
                    "Workspace is locked at {}. Retry after the active Hone command exits.",
                    lock_path.display()
                ),
                details: json!({ "workspace": self.root, "lock": lock_path, "ioError": err.to_string() }),
            });
        }
        file.set_len(0)
            .map_err(|err| HoneError::Internal(err.into()))?;
        writeln!(file, "pid={}", std::process::id())
            .map_err(|err| HoneError::Internal(err.into()))?;
        let result = f();
        let _ = FileExt::unlock(&file);
        result
    }

    fn current_ref(&self) -> Result<String> {
        Ok(fs::read_to_string(self.root.join(".hone/refs/current"))
            .map_err(|err| HoneError::Internal(err.into()))?
            .trim()
            .to_string())
    }

    fn load_current(&self) -> Result<CurrentState> {
        let snapshot_hash = self.current_ref()?;
        let snapshot = self.read_object::<SnapshotPayload>(&snapshot_hash, "snapshot")?;
        let tree_hash = snapshot.payload.tree.clone();
        let tree = self.read_object::<TreePayload>(&tree_hash, "tree")?;
        Ok(CurrentState {
            snapshot_hash,
            snapshot: snapshot.payload,
            tree_hash,
            tree: tree.payload,
        })
    }

    fn snapshot_tree(&self, snapshot_hash: &str) -> Result<TreePayload> {
        let snapshot = self.read_object::<SnapshotPayload>(snapshot_hash, "snapshot")?;
        let tree = self.read_object::<TreePayload>(&snapshot.payload.tree, "tree")?;
        Ok(tree.payload)
    }

    fn commit_tree(
        &self,
        current: &CurrentState,
        tree: TreePayload,
        operation: &str,
        actor: &str,
        message: String,
    ) -> Result<String> {
        let tree_hash = self.write_object("tree", &tree)?;
        let snapshot = SnapshotPayload {
            parent: Some(current.snapshot_hash.clone()),
            tree: tree_hash,
            operation: operation.to_string(),
            actor: actor.to_string(),
            message: message.clone(),
            created_at: now_rfc3339()?,
        };
        let snapshot_hash = self.write_object("snapshot", &snapshot)?;
        atomic_write(
            &self.root.join(".hone/refs/current"),
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
        let path = self.root.join(".hone/journal/transitions.ndjson");
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
            .map_err(|err| HoneError::Internal(err.into()))?;
        writeln!(
            file,
            "{}",
            serde_json::to_string(&record).map_err(|err| HoneError::Internal(err.into()))?
        )
        .map_err(|err| HoneError::Internal(err.into()))
    }

    fn write_object<T: Serialize>(&self, object_type: &str, payload: &T) -> Result<String> {
        let envelope = ObjectEnvelope::new(object_type, payload);
        let bytes = canonical_json_bytes(&envelope)?;
        let hex = hex_digest(&bytes);
        let hash = sha_ref(&hex);
        let path = self.object_path(&hash)?;
        if path.exists() {
            let mut existing = fs::read(&path).map_err(|err| HoneError::Internal(err.into()))?;
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
            fs::create_dir_all(parent).map_err(|err| HoneError::Internal(err.into()))?;
        }
        fs::create_dir_all(self.root.join(".hone/tmp"))
            .map_err(|err| HoneError::Internal(err.into()))?;
        let mut tmp = NamedTempFile::new_in(self.root.join(".hone/tmp"))
            .map_err(|err| HoneError::Internal(err.into()))?;
        tmp.write_all(&bytes)
            .map_err(|err| HoneError::Internal(err.into()))?;
        tmp.write_all(b"\n")
            .map_err(|err| HoneError::Internal(err.into()))?;
        tmp.as_file()
            .sync_all()
            .map_err(|err| HoneError::Internal(err.into()))?;
        tmp.persist(&path)
            .map_err(|err| HoneError::Internal(anyhow::anyhow!(err)))?;
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
        let mut bytes = fs::read(path).map_err(|err| HoneError::Internal(err.into()))?;
        if bytes.ends_with(b"\n") {
            bytes.pop();
        }
        let digest = sha_ref(&hex_digest(&bytes));
        if digest != hash {
            return Err(integrity(format!("Object hash mismatch for {hash}")));
        }
        let envelope: ObjectEnvelope<T> =
            serde_json::from_slice(&bytes).map_err(|err| HoneError::Internal(err.into()))?;
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
        self.root.join(".hone/objects")
    }

    fn pending_proposals_dir(&self) -> PathBuf {
        self.root.join(".hone/pending/proposals")
    }

    fn pending_proposal_ref(&self, proposal_id: &str) -> PathBuf {
        self.pending_proposals_dir()
            .join(format!("{proposal_id}.ref"))
    }

    fn remove_pending_proposal(&self, proposal_id: &str) -> Result<()> {
        let path = self.pending_proposal_ref(proposal_id);
        if path.exists() {
            fs::remove_file(path).map_err(|err| HoneError::Internal(err.into()))?;
        }
        Ok(())
    }

    fn parse_proposal_file(&self, file: &Path) -> Result<ObjectEnvelope<ProposalPayload>> {
        let raw = fs::read_to_string(file).map_err(|err| HoneError::Internal(err.into()))?;
        let value: Value =
            serde_json::from_str(&raw).map_err(|err| HoneError::Internal(err.into()))?;
        if contains_approval_fields(&value) {
            return Err(invalid("Proposal contains approval/decision fields"));
        }
        if value.get("objectType").and_then(Value::as_str) == Some("proposal") {
            serde_json::from_value(value).map_err(|err| HoneError::Internal(err.into()))
        } else {
            let payload: ProposalPayload =
                serde_json::from_value(value).map_err(|err| HoneError::Internal(err.into()))?;
            Ok(ObjectEnvelope::new("proposal", payload))
        }
    }

    fn parse_decision_file(
        &self,
        file: &Path,
        proposal_hash: &str,
        proposal: &ProposalPayload,
        default_action: &str,
    ) -> Result<DecisionPayload> {
        let raw = fs::read_to_string(file).map_err(|err| HoneError::Internal(err.into()))?;
        let value: Value =
            serde_json::from_str(&raw).map_err(|err| HoneError::Internal(err.into()))?;
        if value.get("objectType").and_then(Value::as_str) == Some("decision") {
            let envelope: ObjectEnvelope<DecisionPayload> =
                serde_json::from_value(value).map_err(|err| HoneError::Internal(err.into()))?;
            Ok(envelope.payload)
        } else {
            let final_operation = value
                .get("finalOperation")
                .cloned()
                .or_else(|| value.get("final_operation").cloned())
                .unwrap_or_else(|| value.clone());
            let action = value
                .get("action")
                .and_then(Value::as_str)
                .unwrap_or(default_action)
                .to_string();
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
                action,
                actor: value
                    .get("actor")
                    .and_then(Value::as_str)
                    .unwrap_or("")
                    .to_string(),
                final_operation,
                note: value
                    .get("note")
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
        proposal: &ObjectEnvelope<ProposalPayload>,
        current: &CurrentState,
        write_object: bool,
    ) -> Result<String> {
        if proposal.schema_version != SCHEMA_VERSION || proposal.object_type != "proposal" {
            return Err(invalid(
                "Proposal must be a schemaVersion 1 proposal object",
            ));
        }
        if proposal.payload.proposal_id.trim().is_empty() {
            return Err(invalid("Proposal ID is required"));
        }
        if proposal.payload.base_snapshot != current.snapshot_hash {
            return Err(HoneError::StaleProposal {
                code: "STALE_PROPOSAL",
                message: format!(
                    "Proposal was based on snapshot {}, current is {}",
                    proposal.payload.base_snapshot, current.snapshot_hash
                ),
                details: json!({}),
            });
        }
        self.read_object::<SnapshotPayload>(&proposal.payload.base_snapshot, "snapshot")?;
        let (_, _, source) = self.resolve_source(&current.tree, &proposal.payload.source)?;
        let operation = string_field(&proposal.payload.recommendation, "operation")?;
        if !operation_allowed(operation) {
            return Err(invalid(format!(
                "Unsupported proposal operation: {operation}"
            )));
        }
        if let Some(body) = proposal
            .payload
            .recommendation
            .get("proposedBodyMarkdown")
            .and_then(Value::as_str)
            .or_else(|| {
                proposal
                    .payload
                    .recommendation
                    .get("bodyMarkdown")
                    .and_then(Value::as_str)
            })
        {
            validate_facet_body(body).map_err(|err| invalid(err.to_string()))?;
        }
        for candidate in &proposal.payload.candidates {
            if !relationship_allowed(&candidate.relationship) {
                return Err(invalid(format!(
                    "Unsupported relationship: {}",
                    candidate.relationship
                )));
            }
            let current_revision = current
                .tree
                .facets
                .get(&candidate.facet_id)
                .ok_or_else(|| not_found(format!("Facet not found: {}", candidate.facet_id)))?;
            if current_revision != &candidate.facet_revision {
                return Err(HoneError::StaleProposal {
                    code: "STALE_PROPOSAL",
                    message: format!("Candidate facet {} is not current", candidate.facet_id),
                    details: json!({}),
                });
            }
            let facet = self
                .read_object::<FacetRevisionPayload>(&candidate.facet_revision, "facet-revision")?;
            for passage in &candidate.input_passages {
                if !source.payload.body_markdown.contains(passage) {
                    return Err(invalid(format!(
                        "Quoted source passage not found: {passage}"
                    )));
                }
            }
            for passage in &candidate.facet_passages {
                if !facet.payload.body_markdown.contains(passage)
                    && !facet.payload.title.contains(passage)
                {
                    return Err(invalid(format!(
                        "Quoted facet passage not found: {passage}"
                    )));
                }
            }
        }
        match operation {
            "revise-facet" => {
                let target = string_field(&proposal.payload.recommendation, "targetFacetId")?;
                let base = string_field(&proposal.payload.recommendation, "baseFacetRevision")?;
                if current.tree.facets.get(target).map(String::as_str) != Some(base) {
                    return Err(HoneError::StaleProposal {
                        code: "STALE_PROPOSAL",
                        message: format!("Recommendation target {target} is not current"),
                        details: json!({}),
                    });
                }
                if string_field(&proposal.payload.recommendation, "proposedTitle")?
                    .trim()
                    .is_empty()
                {
                    return Err(invalid("Proposed facet title is empty"));
                }
            }
            "create-facet" => {
                let title = proposal
                    .payload
                    .recommendation
                    .get("proposedTitle")
                    .or_else(|| proposal.payload.recommendation.get("title"))
                    .and_then(Value::as_str)
                    .unwrap_or("");
                if title.trim().is_empty() {
                    return Err(invalid("Proposed facet title is empty"));
                }
            }
            _ => {}
        }
        let hash = if write_object {
            self.write_object("proposal", &proposal.payload)?
        } else {
            let bytes = canonical_json_bytes(proposal)?;
            sha_ref(&hex_digest(&bytes))
        };
        Ok(hash)
    }

    fn resolve_source(
        &self,
        tree: &TreePayload,
        source_id_or_hash: &str,
    ) -> Result<(String, String, ObjectEnvelope<SourcePayload>)> {
        if is_sha_ref(source_id_or_hash) {
            let source = self.read_object::<SourcePayload>(source_id_or_hash, "source")?;
            return Ok((
                source.payload.source_id.clone(),
                source_id_or_hash.to_string(),
                source,
            ));
        }
        let hash = tree
            .sources
            .get(source_id_or_hash)
            .ok_or_else(|| not_found(format!("Source not found: {source_id_or_hash}")))?
            .clone();
        let source = self.read_object::<SourcePayload>(&hash, "source")?;
        Ok((source_id_or_hash.to_string(), hash, source))
    }

    fn resolve_proposal(
        &self,
        proposal_id_or_hash: &str,
    ) -> Result<(String, ObjectEnvelope<ProposalPayload>)> {
        if is_sha_ref(proposal_id_or_hash) {
            let proposal = self.read_object::<ProposalPayload>(proposal_id_or_hash, "proposal")?;
            return Ok((proposal_id_or_hash.to_string(), proposal));
        }
        let pending = self.pending_proposal_ref(proposal_id_or_hash);
        if pending.exists() {
            let hash = fs::read_to_string(pending)
                .map_err(|err| HoneError::Internal(err.into()))?
                .trim()
                .to_string();
            let proposal = self.read_object::<ProposalPayload>(&hash, "proposal")?;
            return Ok((hash, proposal));
        }
        let current = self.load_current()?;
        let hash = current
            .tree
            .proposals
            .get(proposal_id_or_hash)
            .ok_or_else(|| not_found(format!("Proposal not found: {proposal_id_or_hash}")))?
            .clone();
        let proposal = self.read_object::<ProposalPayload>(&hash, "proposal")?;
        Ok((hash, proposal))
    }

    fn find_exact_source_duplicate(
        &self,
        tree: &TreePayload,
        kind: &str,
        title: Option<&str>,
        body: &str,
    ) -> Result<Option<String>> {
        for hash in tree.sources.values() {
            let source = self.read_object::<SourcePayload>(hash, "source")?;
            if source.payload.kind == kind
                && source.payload.title.as_deref() == title
                && source.payload.body_markdown == body
                && source.payload.origin.origin_type == "local-input"
                && source.payload.origin.uri.is_none()
            {
                return Ok(Some(hash.clone()));
            }
        }
        Ok(None)
    }

    fn match_source(
        &self,
        tree: &TreePayload,
        body: &str,
        limit: usize,
    ) -> Result<Vec<MatchResult>> {
        let mut facets = Vec::new();
        for (facet_id, hash) in &tree.facets {
            let facet = self.read_object::<FacetRevisionPayload>(hash, "facet-revision")?;
            facets.push(FacetDoc {
                facet_id: facet_id.clone(),
                revision_hash: hash.clone(),
                title: facet.payload.title,
                body: facet.payload.body_text,
            });
        }
        Ok(rank_facets(body, &facets, limit))
    }

    fn propagate_article_revisions(
        &self,
        tree: &mut TreePayload,
        facet_id: &str,
        base_revision: &str,
        new_revision: &str,
        decision_hash: &str,
    ) -> Result<Vec<Value>> {
        let mut changed = Vec::new();
        let articles: Vec<(String, String)> = tree
            .articles
            .iter()
            .map(|(id, hash)| (id.clone(), hash.clone()))
            .collect();
        for (article_id, article_hash) in articles {
            let article =
                self.read_object::<ArticleEditionPayload>(&article_hash, "article-edition")?;
            let mut segments = article.payload.segments.clone();
            let mut did_change = false;
            for segment in &mut segments {
                if segment.segment_type == "facet"
                    && segment.facet_id.as_deref() == Some(facet_id)
                    && segment.facet_revision.as_deref() == Some(base_revision)
                {
                    segment.facet_revision = Some(new_revision.to_string());
                    did_change = true;
                }
            }
            if did_change {
                let new_article = ArticleEditionPayload {
                    article_id: article_id.clone(),
                    edition: article.payload.edition + 1,
                    title: article.payload.title,
                    parent_edition: Some(article_hash.clone()),
                    segments,
                    decision: decision_hash.to_string(),
                    created_at: now_rfc3339()?,
                };
                let new_hash = self.write_object("article-edition", &new_article)?;
                tree.articles.insert(article_id.clone(), new_hash.clone());
                changed.push(json!({
                    "articleId": article_id,
                    "articleEdition": new_hash,
                    "edition": new_article.edition
                }));
            }
        }
        Ok(changed)
    }

    fn render_article_payload(&self, article: &ArticleEditionPayload) -> Result<String> {
        let mut rendered_segments = Vec::new();
        for segment in &article.segments {
            match segment.segment_type.as_str() {
                "prose" => {
                    rendered_segments.push(RenderedSegment::Prose(
                        segment.body_markdown.clone().unwrap_or_default(),
                    ));
                }
                "facet" => {
                    let hash = segment
                        .facet_revision
                        .as_deref()
                        .ok_or_else(|| integrity("Article facet segment missing revision"))?;
                    let facet = self.read_object::<FacetRevisionPayload>(hash, "facet-revision")?;
                    rendered_segments.push(RenderedSegment::Facet {
                        title: facet.payload.title,
                        body: facet.payload.body_markdown,
                    });
                }
                other => {
                    return Err(invalid(format!(
                        "Unsupported article segment type: {other}"
                    )))
                }
            }
        }
        Ok(render_article(&article.title, &rendered_segments))
    }

    fn facet_revision_chain(
        &self,
        current_hash: &str,
    ) -> Result<Vec<(String, FacetRevisionPayload)>> {
        let mut chain = Vec::new();
        let mut hash = current_hash.to_string();
        loop {
            let facet = self.read_object::<FacetRevisionPayload>(&hash, "facet-revision")?;
            let parent = facet.payload.parent_revision.clone();
            chain.push((hash.clone(), facet.payload));
            if let Some(parent_hash) = parent {
                hash = parent_hash;
            } else {
                break;
            }
        }
        chain.sort_by_key(|(_, payload)| payload.revision);
        Ok(chain)
    }

    fn article_edition_chain(
        &self,
        current_hash: &str,
    ) -> Result<Vec<(String, ArticleEditionPayload)>> {
        let mut chain = Vec::new();
        let mut hash = current_hash.to_string();
        loop {
            let article = self.read_object::<ArticleEditionPayload>(&hash, "article-edition")?;
            let parent = article.payload.parent_edition.clone();
            chain.push((hash.clone(), article.payload));
            if let Some(parent_hash) = parent {
                hash = parent_hash;
            } else {
                break;
            }
        }
        chain.sort_by_key(|(_, payload)| payload.edition);
        Ok(chain)
    }

    fn snapshot_chain(&self, current_hash: &str) -> Result<Vec<Value>> {
        let mut chain = Vec::new();
        let mut seen = BTreeSet::new();
        let mut hash = current_hash.to_string();
        loop {
            if !seen.insert(hash.clone()) {
                return Err(integrity("Snapshot parent chain contains a cycle"));
            }
            let snapshot = self.read_object::<SnapshotPayload>(&hash, "snapshot")?;
            let parent = snapshot.payload.parent.clone();
            chain.push(json!({
                "snapshot": hash,
                "parent": parent,
                "tree": snapshot.payload.tree,
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

    fn regenerate_views(&self) -> Result<()> {
        let current = self.load_current()?;
        let views = self.root.join("views");
        if views.exists() {
            for child in ["current", "history", "sources"] {
                let path = views.join(child);
                if path.exists() {
                    fs::remove_dir_all(&path).map_err(|err| HoneError::Internal(err.into()))?;
                }
            }
        }
        fs::create_dir_all(views.join("current/facets"))
            .map_err(|err| HoneError::Internal(err.into()))?;
        fs::create_dir_all(views.join("current/articles"))
            .map_err(|err| HoneError::Internal(err.into()))?;
        fs::create_dir_all(views.join("history/facets"))
            .map_err(|err| HoneError::Internal(err.into()))?;
        fs::create_dir_all(views.join("history/articles"))
            .map_err(|err| HoneError::Internal(err.into()))?;
        fs::create_dir_all(views.join("sources")).map_err(|err| HoneError::Internal(err.into()))?;

        for (source_id, hash) in &current.tree.sources {
            let source = self.read_object::<SourcePayload>(hash, "source")?;
            let body = format!(
                "---\ngeneratedBy: hone\nsnapshot: {}\nsourceId: {}\nobject: {}\n---\n\nGenerated by Hone from snapshot {}.\nDo not edit directly.\n\n# Source {}\n\n{}\n",
                current.snapshot_hash,
                source_id,
                hash,
                current.snapshot_hash,
                source_id,
                source.payload.body_markdown
            );
            atomic_write(
                &views.join("sources").join(format!("{source_id}.md")),
                body.as_bytes(),
            )?;
        }

        for (facet_id, hash) in &current.tree.facets {
            let facet = self.read_object::<FacetRevisionPayload>(hash, "facet-revision")?;
            let file_name = facet_file_name(facet_id);
            let body = format!(
                "---\ngeneratedBy: hone\nsnapshot: {}\nfacetId: {}\nrevision: {}\nobject: {}\n---\n\nGenerated by Hone from snapshot {}.\nDo not edit directly.\n\n## {}\n\n{}\n",
                current.snapshot_hash,
                facet_id,
                facet.payload.revision,
                hash,
                current.snapshot_hash,
                facet.payload.title,
                facet.payload.body_markdown
            );
            atomic_write(
                &views.join("current/facets").join(&file_name),
                body.as_bytes(),
            )?;
            let history_dir = views
                .join("history/facets")
                .join(file_name.trim_end_matches(".md"));
            fs::create_dir_all(&history_dir).map_err(|err| HoneError::Internal(err.into()))?;
            for (rev_hash, rev) in self.facet_revision_chain(hash)? {
                let rev_body = format!(
                    "---\ngeneratedBy: hone\nsnapshot: {}\nfacetId: {}\nrevision: {}\nobject: {}\n---\n\nGenerated by Hone from snapshot {}.\nDo not edit directly.\n\n## {}\n\n{}\n",
                    current.snapshot_hash,
                    facet_id,
                    rev.revision,
                    rev_hash,
                    current.snapshot_hash,
                    rev.title,
                    rev.body_markdown
                );
                atomic_write(
                    &history_dir.join(format!("r{}.md", rev.revision)),
                    rev_body.as_bytes(),
                )?;
            }
        }

        for (article_id, hash) in &current.tree.articles {
            let article = self.read_object::<ArticleEditionPayload>(hash, "article-edition")?;
            let rendered = self.render_article_payload(&article.payload)?;
            let file_name = format!(
                "{}.md",
                article_id.trim_start_matches("article:").replace('/', "--")
            );
            let body = format!(
                "---\ngeneratedBy: hone\nsnapshot: {}\narticleId: {}\nedition: {}\nobject: {}\n---\n\nGenerated by Hone from snapshot {}.\nDo not edit directly.\n\n{}",
                current.snapshot_hash,
                article_id,
                article.payload.edition,
                hash,
                current.snapshot_hash,
                rendered
            );
            atomic_write(
                &views.join("current/articles").join(&file_name),
                body.as_bytes(),
            )?;
            let history_dir = views
                .join("history/articles")
                .join(file_name.trim_end_matches(".md"));
            fs::create_dir_all(&history_dir).map_err(|err| HoneError::Internal(err.into()))?;
            for (edition_hash, edition) in self.article_edition_chain(hash)? {
                let rendered = self.render_article_payload(&edition)?;
                let edition_body = format!(
                    "---\ngeneratedBy: hone\nsnapshot: {}\narticleId: {}\nedition: {}\nobject: {}\n---\n\nGenerated by Hone from snapshot {}.\nDo not edit directly.\n\n{}",
                    current.snapshot_hash,
                    article_id,
                    edition.edition,
                    edition_hash,
                    current.snapshot_hash,
                    rendered
                );
                atomic_write(
                    &history_dir.join(format!("e{}.md", edition.edition)),
                    edition_body.as_bytes(),
                )?;
            }
        }
        Ok(())
    }

    fn rebuild_index(&self) -> Result<()> {
        let current = self.load_current()?;
        let index_path = self.root.join(".hone/index.sqlite");
        if index_path.exists() {
            fs::remove_file(&index_path).map_err(|err| HoneError::Internal(err.into()))?;
        }
        let conn = Connection::open(&index_path).map_err(|err| HoneError::Internal(err.into()))?;
        conn.execute_batch(
            r#"
            CREATE TABLE meta (key TEXT PRIMARY KEY, value TEXT NOT NULL);
            CREATE TABLE sources (source_id TEXT PRIMARY KEY, object TEXT NOT NULL, kind TEXT NOT NULL, title TEXT, body_markdown TEXT NOT NULL);
            CREATE TABLE facets_current (facet_id TEXT PRIMARY KEY, object TEXT NOT NULL, revision INTEGER NOT NULL, title TEXT NOT NULL, body_markdown TEXT NOT NULL, body_text TEXT NOT NULL);
            CREATE TABLE facet_revisions (facet_id TEXT NOT NULL, object TEXT PRIMARY KEY, revision INTEGER NOT NULL, title TEXT NOT NULL, body_markdown TEXT NOT NULL);
            CREATE TABLE hone_events (event_id TEXT PRIMARY KEY, object TEXT NOT NULL, relationship TEXT NOT NULL, source TEXT NOT NULL);
            CREATE TABLE articles_current (article_id TEXT PRIMARY KEY, object TEXT NOT NULL, edition INTEGER NOT NULL, title TEXT NOT NULL, rendered_text TEXT NOT NULL);
            CREATE TABLE article_editions (article_id TEXT NOT NULL, object TEXT PRIMARY KEY, edition INTEGER NOT NULL, title TEXT NOT NULL, rendered_text TEXT NOT NULL);
            CREATE TABLE proposals (proposal_id TEXT PRIMARY KEY, object TEXT NOT NULL, base_snapshot TEXT NOT NULL);
            CREATE TABLE decisions (decision_id TEXT PRIMARY KEY, object TEXT NOT NULL, proposal TEXT NOT NULL, action TEXT NOT NULL);
            CREATE TABLE snapshots (snapshot TEXT PRIMARY KEY, parent TEXT, tree_hash TEXT NOT NULL, operation TEXT NOT NULL, message TEXT NOT NULL, created_at TEXT NOT NULL);
            CREATE TABLE relationships (event_id TEXT NOT NULL, relationship TEXT NOT NULL, facet_id TEXT, facet_revision TEXT);
            CREATE VIRTUAL TABLE source_fts USING fts5(source_id, body_markdown);
            CREATE VIRTUAL TABLE facet_fts USING fts5(facet_id, title, body_text);
            CREATE VIRTUAL TABLE article_fts USING fts5(article_id, rendered_text);
            "#,
        )
        .map_err(|err| HoneError::Internal(err.into()))?;
        conn.execute(
            "INSERT INTO meta (key, value) VALUES ('currentSnapshot', ?1)",
            params![current.snapshot_hash],
        )
        .map_err(|err| HoneError::Internal(err.into()))?;

        for (source_id, hash) in &current.tree.sources {
            let source = self.read_object::<SourcePayload>(hash, "source")?;
            conn.execute(
                "INSERT INTO sources VALUES (?1, ?2, ?3, ?4, ?5)",
                params![
                    source_id,
                    hash,
                    source.payload.kind,
                    source.payload.title,
                    source.payload.body_markdown
                ],
            )
            .map_err(|err| HoneError::Internal(err.into()))?;
            conn.execute(
                "INSERT INTO source_fts (source_id, body_markdown) VALUES (?1, ?2)",
                params![source_id, source.payload.body_markdown],
            )
            .map_err(|err| HoneError::Internal(err.into()))?;
        }

        for (facet_id, hash) in &current.tree.facets {
            let facet = self.read_object::<FacetRevisionPayload>(hash, "facet-revision")?;
            conn.execute(
                "INSERT INTO facets_current VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                params![
                    facet_id,
                    hash,
                    facet.payload.revision,
                    facet.payload.title,
                    facet.payload.body_markdown,
                    facet.payload.body_text
                ],
            )
            .map_err(|err| HoneError::Internal(err.into()))?;
            conn.execute(
                "INSERT INTO facet_fts (facet_id, title, body_text) VALUES (?1, ?2, ?3)",
                params![facet_id, facet.payload.title, facet.payload.body_text],
            )
            .map_err(|err| HoneError::Internal(err.into()))?;
            for (rev_hash, rev) in self.facet_revision_chain(hash)? {
                conn.execute(
                    "INSERT INTO facet_revisions VALUES (?1, ?2, ?3, ?4, ?5)",
                    params![
                        facet_id,
                        rev_hash,
                        rev.revision,
                        rev.title,
                        rev.body_markdown
                    ],
                )
                .map_err(|err| HoneError::Internal(err.into()))?;
            }
        }

        for (article_id, hash) in &current.tree.articles {
            let article = self.read_object::<ArticleEditionPayload>(hash, "article-edition")?;
            let rendered = self.render_article_payload(&article.payload)?;
            conn.execute(
                "INSERT INTO articles_current VALUES (?1, ?2, ?3, ?4, ?5)",
                params![
                    article_id,
                    hash,
                    article.payload.edition,
                    article.payload.title,
                    rendered
                ],
            )
            .map_err(|err| HoneError::Internal(err.into()))?;
            conn.execute(
                "INSERT INTO article_fts (article_id, rendered_text) VALUES (?1, ?2)",
                params![article_id, rendered],
            )
            .map_err(|err| HoneError::Internal(err.into()))?;
            for (edition_hash, edition) in self.article_edition_chain(hash)? {
                let rendered = self.render_article_payload(&edition)?;
                conn.execute(
                    "INSERT INTO article_editions VALUES (?1, ?2, ?3, ?4, ?5)",
                    params![
                        article_id,
                        edition_hash,
                        edition.edition,
                        edition.title,
                        rendered
                    ],
                )
                .map_err(|err| HoneError::Internal(err.into()))?;
            }
        }

        for (event_id, hash) in &current.tree.events {
            let event = self.read_object::<HoneEventPayload>(hash, "hone-event")?;
            conn.execute(
                "INSERT INTO hone_events VALUES (?1, ?2, ?3, ?4)",
                params![
                    event_id,
                    hash,
                    event.payload.relationship,
                    event.payload.source
                ],
            )
            .map_err(|err| HoneError::Internal(err.into()))?;
            for target in event.payload.targets {
                conn.execute(
                    "INSERT INTO relationships VALUES (?1, ?2, ?3, ?4)",
                    params![
                        event_id,
                        event.payload.relationship,
                        target.facet_id,
                        target.facet_revision
                    ],
                )
                .map_err(|err| HoneError::Internal(err.into()))?;
            }
        }

        for (proposal_id, hash) in &current.tree.proposals {
            let proposal = self.read_object::<ProposalPayload>(hash, "proposal")?;
            conn.execute(
                "INSERT INTO proposals VALUES (?1, ?2, ?3)",
                params![proposal_id, hash, proposal.payload.base_snapshot],
            )
            .map_err(|err| HoneError::Internal(err.into()))?;
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
            .map_err(|err| HoneError::Internal(err.into()))?;
        }

        for entry in self.snapshot_chain(&current.snapshot_hash)? {
            conn.execute(
                "INSERT INTO snapshots VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                params![
                    entry["snapshot"].as_str(),
                    entry["parent"].as_str(),
                    entry["tree"].as_str(),
                    entry["operation"].as_str(),
                    entry["message"].as_str(),
                    entry["createdAt"].as_str()
                ],
            )
            .map_err(|err| HoneError::Internal(err.into()))?;
        }
        Ok(())
    }

    fn sync_workspace_docs(&self) -> Result<()> {
        let readme = r#"# Hone Workspace

This directory is a local Hone workspace. Open it in Codex App and use Local mode to refine thought through Sources, Facets, Proposals, Decisions, and Snapshots.

Canonical state lives in `.hone/objects/**` and `.hone/refs/current`. Generated views under `views/**` are readable projections and may be overwritten.
"#;
        atomic_write_preserving_user_section(&self.root.join("README.md"), readme.as_bytes())?;
        atomic_write_preserving_user_section(
            &self.root.join("AGENTS.md"),
            workspace_agents_md().as_bytes(),
        )?;
        let skill_dir = self.root.join(".agents/skills/hone");
        fs::create_dir_all(skill_dir.join("references"))
            .map_err(|err| HoneError::Internal(err.into()))?;
        fs::create_dir_all(skill_dir.join("agents"))
            .map_err(|err| HoneError::Internal(err.into()))?;
        atomic_write_preserving_user_section(
            &skill_dir.join("SKILL.md"),
            workspace_skill_md().as_bytes(),
        )?;
        atomic_write(
            &skill_dir.join("references/concepts.md"),
            workspace_concepts_md().as_bytes(),
        )?;
        atomic_write(
            &skill_dir.join("references/relationships.md"),
            workspace_relationships_md().as_bytes(),
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
            b"interface:\n  display_name: \"Hone\"\n  short_description: \"Refine thought through durable facets.\"\n  default_prompt: \"Hone this thought against my current facets.\"\n\npolicy:\n  allow_implicit_invocation: true\n",
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
            let bytes = fs::read(&path).map_err(|err| HoneError::Internal(err.into()))?;
            let name = path
                .file_name()
                .and_then(|name| name.to_str())
                .ok_or_else(|| integrity(format!("Invalid object filename: {}", path.display())))?;
            objects.push(json!({
                "hash": sha_ref(name),
                "bytes": bytes.len()
            }));
        }
        Ok(objects)
    }
}

#[derive(Debug)]
struct VerifiedBundle {
    current: String,
    objects: usize,
}

fn verify_bundle(path: &Path) -> Result<VerifiedBundle> {
    let file = File::open(path).map_err(|err| HoneError::Internal(err.into()))?;
    let mut archive = Archive::new(file);
    let mut current = None;
    let mut object_count = 0usize;
    for entry in archive
        .entries()
        .map_err(|err| HoneError::Internal(err.into()))?
    {
        let mut entry = entry.map_err(|err| HoneError::Internal(err.into()))?;
        let path = entry
            .path()
            .map_err(|err| HoneError::Internal(err.into()))?
            .to_path_buf();
        validate_archive_path(&path)?;
        if path == Path::new("refs/current") {
            let mut text = String::new();
            entry
                .read_to_string(&mut text)
                .map_err(|err| HoneError::Internal(err.into()))?;
            let trimmed = text.trim().to_string();
            if !is_sha_ref(&trimmed) {
                return Err(integrity("Bundle current ref is invalid"));
            }
            current = Some(trimmed);
        } else if path.starts_with("objects") {
            let mut bytes = Vec::new();
            entry
                .read_to_end(&mut bytes)
                .map_err(|err| HoneError::Internal(err.into()))?;
            if bytes.ends_with(b"\n") {
                bytes.pop();
            }
            let digest = hex_digest(&bytes);
            let file_name = path
                .file_name()
                .and_then(|name| name.to_str())
                .ok_or_else(|| integrity("Invalid object entry in bundle"))?;
            if file_name != digest {
                return Err(integrity(format!(
                    "Bundle object hash mismatch: {}",
                    path.display()
                )));
            }
            object_count += 1;
        }
    }
    Ok(VerifiedBundle {
        current: current.ok_or_else(|| integrity("Bundle missing refs/current"))?,
        objects: object_count,
    })
}

fn append_file_to_tar<P: AsRef<Path>>(
    builder: &mut Builder<File>,
    tar_path: P,
    source: &Path,
) -> Result<()> {
    let bytes = fs::read(source).map_err(|err| HoneError::Internal(err.into()))?;
    append_bytes_to_tar(builder, tar_path, bytes)
}

fn append_bytes_to_tar<P: AsRef<Path>>(
    builder: &mut Builder<File>,
    tar_path: P,
    bytes: Vec<u8>,
) -> Result<()> {
    let mut header = Header::new_gnu();
    header.set_size(bytes.len() as u64);
    header.set_mode(0o644);
    header.set_mtime(0);
    header.set_cksum();
    builder
        .append_data(&mut header, tar_path, bytes.as_slice())
        .map_err(|err| HoneError::Internal(err.into()))
}

fn validate_archive_path(path: &Path) -> Result<()> {
    if path.is_absolute()
        || path.components().any(|component| {
            matches!(
                component,
                Component::ParentDir | Component::RootDir | Component::Prefix(_)
            )
        })
    {
        return Err(invalid(format!(
            "Bundle path traversal rejected: {}",
            path.display()
        )));
    }
    let allowed = path == Path::new("manifest.json")
        || path == Path::new("hone.toml")
        || path == Path::new("refs/current")
        || path == Path::new("journal/transitions.ndjson")
        || path.starts_with("objects");
    if !allowed {
        return Err(invalid(format!(
            "Unsupported bundle path: {}",
            path.display()
        )));
    }
    Ok(())
}

fn bundle_path_to_workspace_path(target: &Path, path: &Path) -> Result<PathBuf> {
    if path == Path::new("hone.toml") {
        Ok(target.join("hone.toml"))
    } else if path == Path::new("refs/current") {
        Ok(target.join(".hone/refs/current"))
    } else if path == Path::new("journal/transitions.ndjson") {
        Ok(target.join(".hone/journal/transitions.ndjson"))
    } else if path.starts_with("objects") {
        let rel = path
            .strip_prefix("objects")
            .map_err(|err| HoneError::Internal(err.into()))?;
        Ok(target.join(".hone/objects").join(rel))
    } else if path == Path::new("manifest.json") {
        Ok(target.join(".hone/bundle-manifest.json"))
    } else {
        Err(invalid(format!(
            "Unsupported bundle path: {}",
            path.display()
        )))
    }
}

fn canonical_json_bytes<T: Serialize>(value: &T) -> Result<Vec<u8>> {
    let value = serde_json::to_value(value).map_err(|err| HoneError::Internal(err.into()))?;
    let sorted = sort_json(value);
    serde_json::to_vec(&sorted).map_err(|err| HoneError::Internal(err.into()))
}

fn sort_json(value: Value) -> Value {
    match value {
        Value::Array(values) => Value::Array(values.into_iter().map(sort_json).collect()),
        Value::Object(map) => {
            let mut sorted = BTreeMap::new();
            for (key, value) in map {
                sorted.insert(key, sort_json(value));
            }
            let mut out = Map::new();
            for (key, value) in sorted {
                out.insert(key, value);
            }
            Value::Object(out)
        }
        other => other,
    }
}

fn hex_digest(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    let digest = hasher.finalize();
    digest.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn atomic_write(path: &Path, bytes: &[u8]) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|err| HoneError::Internal(err.into()))?;
    }
    let tmp_dir = path
        .parent()
        .map(Path::to_path_buf)
        .unwrap_or_else(|| PathBuf::from("."));
    let mut tmp = NamedTempFile::new_in(tmp_dir).map_err(|err| HoneError::Internal(err.into()))?;
    tmp.write_all(bytes)
        .map_err(|err| HoneError::Internal(err.into()))?;
    tmp.as_file()
        .sync_all()
        .map_err(|err| HoneError::Internal(err.into()))?;
    tmp.persist(path)
        .map(|_| ())
        .map_err(|err| HoneError::Internal(anyhow::anyhow!(err)))
}

fn atomic_write_preserving_user_section(path: &Path, generated: &[u8]) -> Result<()> {
    let generated =
        String::from_utf8(generated.to_vec()).map_err(|err| HoneError::Internal(err.into()))?;
    let user_section = if path.exists() {
        let existing = fs::read_to_string(path).map_err(|err| HoneError::Internal(err.into()))?;
        extract_user_section(&existing)
    } else {
        None
    };
    let output = if let Some(section) = user_section {
        replace_user_section(&generated, &section)
    } else {
        generated
    };
    atomic_write(path, output.as_bytes())
}

fn extract_user_section(input: &str) -> Option<String> {
    let begin = "<!-- HONE:USER-BEGIN -->";
    let end = "<!-- HONE:USER-END -->";
    let start = input.find(begin)?;
    let finish = input.find(end)?;
    if finish < start {
        return None;
    }
    Some(input[start + begin.len()..finish].to_string())
}

fn replace_user_section(generated: &str, user_section: &str) -> String {
    let begin = "<!-- HONE:USER-BEGIN -->";
    let end = "<!-- HONE:USER-END -->";
    if let (Some(start), Some(finish)) = (generated.find(begin), generated.find(end)) {
        if finish > start {
            let mut out = String::new();
            out.push_str(&generated[..start + begin.len()]);
            out.push_str(user_section);
            out.push_str(&generated[finish..]);
            return out;
        }
    }
    generated.to_string()
}

fn string_field<'a>(value: &'a Value, key: &str) -> Result<&'a str> {
    value
        .get(key)
        .and_then(Value::as_str)
        .filter(|text| !text.trim().is_empty())
        .ok_or_else(|| invalid(format!("Missing required string field: {key}")))
}

fn decision_operation_type(value: &Value) -> Result<&str> {
    value
        .get("type")
        .and_then(Value::as_str)
        .or_else(|| value.get("operation").and_then(Value::as_str))
        .filter(|text| !text.trim().is_empty())
        .ok_or_else(|| invalid("Missing required string field: type"))
}

fn contains_approval_fields(value: &Value) -> bool {
    match value {
        Value::Object(map) => map.iter().any(|(key, value)| {
            matches!(
                key.as_str(),
                "decision" | "decisionId" | "finalOperation" | "authorization" | "approved"
            ) || contains_approval_fields(value)
        }),
        Value::Array(values) => values.iter().any(contains_approval_fields),
        _ => false,
    }
}

fn normalize_for_noop(input: &str) -> String {
    input.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn facet_file_name(facet_id: &str) -> String {
    let trimmed = facet_id.trim_start_matches("facet:");
    format!("{}.md", trimmed.replace('/', "--"))
}

fn format_review(
    proposal_hash: &str,
    proposal: &ProposalPayload,
    source_body: &str,
    candidates: &[Value],
) -> String {
    let mut out = String::new();
    out.push_str(&format!("Proposal: {proposal_hash}\n"));
    out.push_str(&format!("Proposal ID: {}\n", proposal.proposal_id));
    out.push_str(&format!("Base snapshot: {}\n\n", proposal.base_snapshot));
    out.push_str("Source:\n");
    out.push_str(source_body.trim());
    out.push_str("\n\nCandidates:\n");
    for candidate in candidates {
        out.push_str(&format!(
            "- {} ({}) relationship: {}\n",
            candidate["facetId"].as_str().unwrap_or(""),
            candidate["title"].as_str().unwrap_or(""),
            candidate["relationship"].as_str().unwrap_or("")
        ));
    }
    out.push_str("\nRecommendation:\n");
    out.push_str(&serde_json::to_string_pretty(&proposal.recommendation).unwrap_or_default());
    out.push('\n');
    out
}

fn workspace_agents_md() -> String {
    r#"# Hone Workspace Rules

- This directory is a Hone workspace, not a software repository.
- Use the Hone skill for thought capture and semantic changes.
- Use `hone ... --json` for deterministic operations.
- Never edit `.hone/**` directly.
- Never edit `views/**` directly.
- New material must first become a Source.
- Compare new input with current facets before proposing a new facet.
- Treat recurrence as potentially meaningful, not automatically redundant.
- Model output is a Proposal, never authority.
- Show the current facet, proposed relationship, semantic diff, and impact before requesting approval.
- Run `hone approve` only after explicit user authorization.
- Publishing or external sharing is outside Hone.
- Do not make network requests unless the user separately asks for unrelated research.

<!-- HONE:USER-BEGIN -->
<!-- HONE:USER-END -->
"#
    .to_string()
}

fn workspace_skill_md() -> String {
    r#"---
name: hone
description: Capture a thought, compare it with the user's current Hone facets, propose a traceable relationship or refinement, and apply it only after explicit human authorization. Use for "hone this," preserving ideas, checking recurrence, refining principles, or tracing how thinking changed.
---

# Hone Skill

Use this skill when the user asks to hone, preserve, refine, compare, or trace a thought.

## Workflow

1. Confirm the selected directory is a valid Hone workspace with `hone status --json`.
2. Save the exact user input to `inbox/` as UTF-8 Markdown.
3. Capture it using `hone capture --file <path> --kind note --json`.
4. Retrieve matches with `hone relate <source-id> --json`.
5. Request bounded context with `hone context proposal <source-id> --json`.
6. Construct Proposal JSON without hidden chain-of-thought.
7. Validate and save it with `hone proposal validate <file> --json` and `hone proposal save <file> --json`.
8. Render the review with `hone review <proposal-id> --format markdown`.
9. Wait for explicit authorization.
10. Write a Decision JSON file with the exact authorized operation and wording.
11. Run `hone approve <proposal-id> --decision <file> --json`.
12. Report the new snapshot, event, facet revision, article edition, and affected views.

Never edit `.hone/**` or `views/**` directly. Model output is a Proposal, never authority. Approval requires explicit user language such as "approve", "accept this", "use the second version", "record this as reinforcement", or "create this facet".

<!-- HONE:USER-BEGIN -->
<!-- HONE:USER-END -->
"#
    .to_string()
}

fn workspace_concepts_md() -> &'static str {
    r#"# Hone Concepts

Hone is a local system for refining thought over time.

- Source: an immutable thought occurrence.
- Facet: a durable idea, principle, belief, position, question, or method.
- Article: a readable composition of selected facet revisions.
- Proposal: model-produced and non-authoritative.
- Decision: explicit human authorization.
- Hone Event: the trace of how a source related to current thought.
- Snapshot: one complete semantic state.
"#
}

fn workspace_relationships_md() -> &'static str {
    r#"# Relationships

Supported relationships:

- exact-duplicate
- recurrence
- reinforcement
- clarification
- deepening
- extension
- reframing
- narrowing
- challenge
- contradiction
- novel
- unresolved

Do not treat repeated thought as automatically redundant. Recurrence and reinforcement may matter without changing facet text.
"#
}

fn workspace_authorization_md() -> &'static str {
    r#"# Authorization

Codex must not run `hone approve` merely because a proposal exists. Wait for explicit user authorization.

Ambiguous replies require clarification. The Decision JSON must contain the exact final operation and authorized wording.
"#
}

fn workspace_cli_contract_md() -> &'static str {
    r#"# CLI Contract

Use `--json` for agent operations.

Common flow:

```bash
hone status --json
hone capture --file inbox/input.md --kind note --json
hone relate <source-id> --json
hone context proposal <source-id> --json
hone proposal validate drafts/proposals/proposal.json --json
hone proposal save drafts/proposals/proposal.json --json
hone review <proposal-id> --format markdown
hone approve <proposal-id> --decision drafts/proposals/decision.json --json
```
"#
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn canonical_json_sorts_object_keys() {
        let a = json!({ "b": 1, "a": 2 });
        let b = json!({ "a": 2, "b": 1 });
        assert_eq!(
            canonical_json_bytes(&a).unwrap(),
            canonical_json_bytes(&b).unwrap()
        );
    }

    #[test]
    fn demo_workspace_initializes_and_fsck_passes() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("hone-demo");
        Workspace::new_workspace(&root, true).unwrap();
        let ws = Workspace::open(&root).unwrap();
        let status = ws.status().unwrap();
        assert_eq!(status["counts"]["facets"], 2);
        assert!(ws.fsck().unwrap()["ok"].as_bool().unwrap());
        assert!(root.join(".agents/skills/hone/SKILL.md").exists());
        assert!(root
            .join("views/current/articles/generative-art.md")
            .exists());
    }

    #[test]
    fn demo_scenario_deepening_then_reinforcement() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("hone-demo");
        Workspace::new_workspace(&root, true).unwrap();
        let ws = Workspace::open(&root).unwrap();

        let source_a_path = root.join("inbox/scenario-a.md");
        fs::write(
            &source_a_path,
            "A generative artwork is not only one rendered result.\nIts rules and possibility space are part of the work.\n",
        )
        .unwrap();
        let capture_a = ws.capture(&source_a_path, "note", None).unwrap();
        let source_a = capture_a["sourceId"].as_str().unwrap();
        let facet_list = ws.facet_list().unwrap();
        let definition = facet_list["facets"]
            .as_array()
            .unwrap()
            .iter()
            .find(|facet| facet["facetId"] == "facet:generative-art/definition")
            .unwrap();
        let definition_hash = definition["object"].as_str().unwrap();
        let current_snapshot_a = capture_a["snapshot"].as_str().unwrap();

        let context = ws.proposal_context(source_a, 5).unwrap();
        assert_eq!(
            context["facets"][0]["facetId"].as_str().unwrap(),
            "facet:generative-art/definition"
        );

        let proposal_a = ObjectEnvelope::new(
            "proposal",
            ProposalPayload {
                proposal_id: new_id("pro"),
                source: capture_a["source"].as_str().unwrap().to_string(),
                base_snapshot: current_snapshot_a.to_string(),
                candidates: vec![hone_core::ProposalCandidate {
                    facet_id: "facet:generative-art/definition".to_string(),
                    facet_revision: definition_hash.to_string(),
                    relationship: "deepening".to_string(),
                    input_passages: vec![
                        "A generative artwork is not only one rendered result.".to_string()
                    ],
                    facet_passages: vec![
                        "Generative art is an artwork produced by a generative system.".to_string(),
                    ],
                    explanation: Some("The source adds possibility space.".to_string()),
                }],
                recommendation: json!({
                    "operation": "revise-facet",
                    "targetFacetId": "facet:generative-art/definition",
                    "baseFacetRevision": definition_hash,
                    "proposedTitle": "Definition",
                    "proposedBodyMarkdown": "Generative art is a generative system, its rules, and its possibility space, not only one rendered outcome."
                }),
                unresolved: vec![],
                generated_by: GeneratedBy {
                    host: "codex".to_string(),
                    model: None,
                },
                created_at: now_rfc3339().unwrap(),
            },
        );
        let proposal_a_path = root.join("drafts/proposals/scenario-a.json");
        fs::write(
            &proposal_a_path,
            serde_json::to_string_pretty(&proposal_a).unwrap(),
        )
        .unwrap();
        let saved_a = ws.save_proposal_file(&proposal_a_path).unwrap();
        let decision_a_path = root.join("drafts/proposals/scenario-a-decision.json");
        fs::write(
            &decision_a_path,
            serde_json::to_string_pretty(&json!({
                "action": "approve-with-edit",
                "finalOperation": {
                    "type": "revise-facet",
                    "targetFacetId": "facet:generative-art/definition",
                    "baseFacetRevision": definition_hash,
                    "title": "Definition",
                    "bodyMarkdown": "Generative art is a generative system, its rules, and its possibility space, not only one rendered outcome."
                }
            }))
            .unwrap(),
        )
        .unwrap();
        let approved_a = ws
            .approve(saved_a["proposalId"].as_str().unwrap(), &decision_a_path)
            .unwrap();
        assert_eq!(approved_a["operation"], "revise-facet");
        assert_eq!(approved_a["articleEditions"].as_array().unwrap().len(), 1);
        let facet_after_a = ws
            .facet_show("facet:generative-art/definition", None)
            .unwrap();
        assert_eq!(facet_after_a["payload"]["revision"], 2);
        let definition_r2 = facet_after_a["object"].as_str().unwrap().to_string();

        let source_b_path = root.join("inbox/scenario-b.md");
        fs::write(
            &source_b_path,
            "Again I find that the possibility space matters more than any one output.\n",
        )
        .unwrap();
        let capture_b = ws.capture(&source_b_path, "note", None).unwrap();
        let current_snapshot_b = capture_b["snapshot"].as_str().unwrap();
        let proposal_b = ObjectEnvelope::new(
            "proposal",
            ProposalPayload {
                proposal_id: new_id("pro"),
                source: capture_b["source"].as_str().unwrap().to_string(),
                base_snapshot: current_snapshot_b.to_string(),
                candidates: vec![hone_core::ProposalCandidate {
                    facet_id: "facet:generative-art/definition".to_string(),
                    facet_revision: definition_r2.clone(),
                    relationship: "reinforcement".to_string(),
                    input_passages: vec!["possibility space matters".to_string()],
                    facet_passages: vec!["possibility space".to_string()],
                    explanation: Some("The source reinforces the current definition.".to_string()),
                }],
                recommendation: json!({
                    "operation": "record-reinforcement",
                    "targetFacetId": "facet:generative-art/definition",
                    "baseFacetRevision": definition_r2
                }),
                unresolved: vec![],
                generated_by: GeneratedBy {
                    host: "codex".to_string(),
                    model: None,
                },
                created_at: now_rfc3339().unwrap(),
            },
        );
        let proposal_b_path = root.join("drafts/proposals/scenario-b.json");
        fs::write(
            &proposal_b_path,
            serde_json::to_string_pretty(&proposal_b).unwrap(),
        )
        .unwrap();
        let saved_b = ws.save_proposal_file(&proposal_b_path).unwrap();
        let decision_b_path = root.join("drafts/proposals/scenario-b-decision.json");
        fs::write(
            &decision_b_path,
            serde_json::to_string_pretty(&json!({
                "action": "record-reinforcement",
                "finalOperation": {
                    "type": "record-reinforcement",
                    "targetFacetId": "facet:generative-art/definition",
                    "baseFacetRevision": facet_after_a["object"].as_str().unwrap()
                }
            }))
            .unwrap(),
        )
        .unwrap();
        let approved_b = ws
            .approve(saved_b["proposalId"].as_str().unwrap(), &decision_b_path)
            .unwrap();
        assert_eq!(approved_b["facetRevisionCreated"], false);
        let facet_after_b = ws
            .facet_show("facet:generative-art/definition", None)
            .unwrap();
        assert_eq!(facet_after_b["payload"]["revision"], 2);
        assert_eq!(facet_after_b["object"], facet_after_a["object"]);
    }
}
