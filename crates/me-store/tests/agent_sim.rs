use me_store::Workspace;
use serde::Deserialize;
use serde_json::{Value, json};
use tempfile::tempdir;

const CONTRACT_JSON: &str = include_str!("../../../contracts/me-semantic-contract.v1.json");
const FIXTURES_JSON: &str =
    include_str!("../../../tests/agent-fixtures/semantic-boundary-fixtures.json");
const THOUGHT_CAPTURED_TEMPLATE: &str =
    include_str!("../../../templates/render/thought-captured.md");
const COGNITION_KEPT_TEMPLATE: &str = include_str!("../../../templates/render/cognition-kept.md");
const USING_ME_TEMPLATE: &str = include_str!("../../../templates/render/using-me-read-only.md");
const OUTPUT_FEEDBACK_TEMPLATE: &str = include_str!("../../../templates/render/output-feedback.md");
const INVALID_MISSING_APPROVAL_TEMPLATE: &str =
    include_str!("../../../templates/render/invalid-missing-approval.md");
const IMPORT_FIXTURE: &str = include_str!("../../../tests/import-boundary/ask.positioning.v1.md");

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct AgentFixture {
    name: String,
    workspace: String,
    user: String,
    pending_thought: Option<String>,
    reference: Option<String>,
    procedure: Option<String>,
    expected_mode: String,
    #[serde(default)]
    expected_tool_calls: Vec<String>,
    #[serde(default)]
    forbidden_tool_calls: Vec<String>,
    #[serde(default)]
    expected_response_contains: Vec<String>,
    expected_decision_fields: Option<Value>,
    expected_state: ExpectedState,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ExpectedState {
    active_cognitions: Option<usize>,
    pending_thoughts: Option<usize>,
    snapshot_changed: bool,
}

#[derive(Debug)]
struct SimResult {
    mode: String,
    tool_calls: Vec<String>,
    response: String,
    decision: Option<Value>,
    before_snapshot: String,
    after_snapshot: String,
    active_cognitions: usize,
    pending_thoughts: usize,
}

#[test]
fn semantic_contract_shape_is_valid() {
    let contract: Value = serde_json::from_str(CONTRACT_JSON).unwrap();
    assert_eq!(contract["schemaVersion"], 1);
    assert_eq!(
        contract["principle"],
        "Prompts guide the model. Transactions govern the product."
    );
    assert_eq!(
        contract["states"]["thought"],
        json!(["pending", "kept-only", "dismissed", "added"])
    );
    assert_eq!(
        contract["states"]["cognition"],
        json!(["active", "retired"])
    );
    assert!(
        contract["transitions"]
            .as_array()
            .unwrap()
            .iter()
            .any(|item| {
                item["id"] == "add-cognition"
                    && item["requiresApproval"] == true
                    && item["requiresDecisionField"]["approved"] == true
            })
    );
    assert_eq!(
        contract["boundaries"]["referenceToCognitionDirect"],
        "forbidden"
    );
}

#[test]
fn agent_sim_fixtures_enforce_semantic_boundaries() {
    let contract: Value = serde_json::from_str(CONTRACT_JSON).unwrap();
    let fixtures: Vec<AgentFixture> = serde_json::from_str(FIXTURES_JSON).unwrap();
    assert!(fixtures.len() >= 11);

    for fixture in fixtures {
        let result = run_fixture(&contract, &fixture);
        assert_eq!(result.mode, fixture.expected_mode, "{}", fixture.name);
        for expected in &fixture.expected_tool_calls {
            assert!(
                result.tool_calls.iter().any(|call| call == expected),
                "{} missing expected tool call {expected}; got {:?}",
                fixture.name,
                result.tool_calls
            );
        }
        for forbidden in &fixture.forbidden_tool_calls {
            assert!(
                !result.tool_calls.iter().any(|call| call == forbidden),
                "{} made forbidden tool call {forbidden}; got {:?}",
                fixture.name,
                result.tool_calls
            );
        }
        for expected in &fixture.expected_response_contains {
            assert!(
                result.response.contains(expected),
                "{} response missing {expected}: {}",
                fixture.name,
                result.response
            );
        }
        if let Some(expected_decision) = &fixture.expected_decision_fields {
            let decision = result
                .decision
                .as_ref()
                .unwrap_or_else(|| panic!("{} missing decision", fixture.name));
            assert_eq!(decision["approved"], expected_decision["approved"]);
        }
        assert_eq!(
            result.before_snapshot != result.after_snapshot,
            fixture.expected_state.snapshot_changed,
            "{} snapshotChanged",
            fixture.name
        );
        if let Some(active) = fixture.expected_state.active_cognitions {
            assert_eq!(result.active_cognitions, active, "{} active", fixture.name);
        }
        if let Some(pending) = fixture.expected_state.pending_thoughts {
            assert_eq!(result.pending_thoughts, pending, "{} pending", fixture.name);
        }
    }
}

#[test]
fn golden_render_templates_match_ordinary_ux() {
    let dir = tempdir().unwrap();
    Workspace::init(dir.path(), false).unwrap();
    let ws = Workspace::open(dir.path()).unwrap();
    let captured = ws
        .thought_capture_body("Agent Art will matter.".to_string(), "idea")
        .unwrap();
    let captured_markdown = captured["renderedMarkdown"].as_str().unwrap();
    assert_template_shape(captured_markdown, THOUGHT_CAPTURED_TEMPLATE);

    let err = ws
        .cognition_add_value(
            captured["thoughtId"].as_str().unwrap(),
            json!({ "action": "add-cognition" }),
        )
        .unwrap_err();
    let missing_approval = err.details()["renderedMarkdown"]
        .as_str()
        .unwrap()
        .to_string();
    assert_template_shape(&missing_approval, INVALID_MISSING_APPROVAL_TEMPLATE);

    let kept = ws
        .cognition_add_value(
            captured["thoughtId"].as_str().unwrap(),
            json!({ "action": "add-cognition", "approved": true }),
        )
        .unwrap();
    let kept_markdown = kept["renderedMarkdown"].as_str().unwrap();
    assert_template_shape(kept_markdown, COGNITION_KEPT_TEMPLATE);

    let before = ws.current().unwrap()["currentSnapshot"]
        .as_str()
        .unwrap()
        .to_string();
    let context = ws.context_body("Draft using ME.".to_string(), 20).unwrap();
    assert_eq!(context["cognitionLibraryChanged"], false);
    assert_eq!(ws.current().unwrap()["currentSnapshot"], before);
    assert_template_shape(
        context["guidance"]["renderedMarkdown"].as_str().unwrap(),
        USING_ME_TEMPLATE,
    );
    assert_template_shape(
        context["guidance"]["renderedMarkdown"].as_str().unwrap(),
        OUTPUT_FEEDBACK_TEMPLATE,
    );

    for output in [
        captured_markdown,
        &missing_approval,
        kept_markdown,
        context["guidance"]["renderedMarkdown"].as_str().unwrap(),
    ] {
        assert_no_golden_leaks(output);
    }
}

#[test]
fn import_boundary_fixture_is_reference_not_cognition_bulk_import() {
    assert!(IMPORT_FIXTURE.contains("Inshell makes its system interrogable."));
    assert!(IMPORT_FIXTURE.contains("Ask Inshell must not invent official claims."));
    assert!(IMPORT_FIXTURE.contains("Do not request private keys."));
    assert!(IMPORT_FIXTURE.contains("This whole document is a Reference."));

    let dir = tempdir().unwrap();
    Workspace::init(dir.path(), false).unwrap();
    let ws = Workspace::open(dir.path()).unwrap();
    let before = ws.current().unwrap();
    let before_snapshot = before["currentSnapshot"].as_str().unwrap().to_string();

    let explanation = reference_boundary_response();
    assert!(explanation.contains("Reference"));
    assert!(explanation.contains("not Cognition"));
    assert!(explanation.contains("exact excerpt"));

    let after = ws.current().unwrap();
    assert_eq!(after["currentSnapshot"], before_snapshot);
    assert_eq!(after["counts"]["activeCognitions"], 0);
    assert_eq!(after["counts"]["pendingThoughts"], 0);
}

#[test]
fn direct_routes_for_output_reference_and_procedure_are_forbidden() {
    let contract: Value = serde_json::from_str(CONTRACT_JSON).unwrap();
    assert_eq!(
        contract["boundaries"]["outputToCognitionDirect"],
        "forbidden"
    );
    assert_eq!(
        contract["boundaries"]["referenceToCognitionDirect"],
        "forbidden"
    );
    assert_eq!(
        contract["boundaries"]["procedureToCognitionDirect"],
        "forbidden"
    );

    let fixtures: Vec<AgentFixture> = serde_json::from_str(FIXTURES_JSON).unwrap();
    for name in [
        "output-feedback-reenters-as-thought",
        "reference-not-bulk-imported",
        "procedure-not-cognition",
    ] {
        let fixture = fixtures
            .iter()
            .find(|fixture| fixture.name == name)
            .unwrap();
        assert!(
            fixture
                .forbidden_tool_calls
                .iter()
                .any(|call| call == "me cognition add")
        );
    }
}

fn run_fixture(contract: &Value, fixture: &AgentFixture) -> SimResult {
    let dir = tempdir().unwrap();
    Workspace::init(dir.path(), fixture.workspace == "seeded").unwrap();
    let ws = Workspace::open(dir.path()).unwrap();
    let mut pending_thought_id = None;
    if fixture.workspace == "pendingThought" {
        let body = fixture.pending_thought.as_ref().unwrap();
        let captured = ws.thought_capture_body(body.clone(), "idea").unwrap();
        pending_thought_id = captured["thoughtId"].as_str().map(str::to_string);
    }
    let before = ws.current().unwrap();
    let before_snapshot = before["currentSnapshot"].as_str().unwrap().to_string();
    let mode = classify(
        contract,
        fixture,
        before["counts"]["pendingThoughts"].as_u64().unwrap(),
    );
    let mut tool_calls = Vec::new();
    let response: String;
    let mut decision = None;

    match mode.as_str() {
        "changing-me.capture-thought" => {
            tool_calls.push("me thought capture".to_string());
            let body = thought_body_from_user(&fixture.user);
            let captured = ws.thought_capture_body(body, "idea").unwrap();
            response = captured["renderedMarkdown"].as_str().unwrap().to_string();
        }
        "changing-me.approve-cognition" => {
            let thought_id = pending_thought_id.expect("pending thought");
            let decision_value = json!({ "action": "add-cognition", "approved": true });
            tool_calls.push("me cognition add".to_string());
            let kept = ws
                .cognition_add_value(&thought_id, decision_value.clone())
                .unwrap();
            response = kept["renderedMarkdown"].as_str().unwrap().to_string();
            decision = Some(decision_value);
        }
        "using-me.read-only" => {
            tool_calls.push("me context".to_string());
            let context = ws.context_body(fixture.user.clone(), 20).unwrap();
            response = context["guidance"]["renderedMarkdown"]
                .as_str()
                .unwrap_or("ME was read, not changed.")
                .to_string();
        }
        "using-reference.boundary" => {
            assert!(fixture.reference.is_some());
            response = reference_boundary_response();
        }
        "using-procedure.boundary" => {
            assert!(fixture.procedure.is_some());
            response = "Procedure can guide Codex. It is not Cognition and no Cognition was added."
                .to_string();
        }
        "general-codex" => {
            response = "What would you like to keep?".to_string();
        }
        other => panic!("unsupported fixture mode {other}"),
    }

    let after = ws.current().unwrap();
    SimResult {
        mode,
        tool_calls,
        response,
        decision,
        before_snapshot,
        after_snapshot: after["currentSnapshot"].as_str().unwrap().to_string(),
        active_cognitions: after["counts"]["activeCognitions"].as_u64().unwrap() as usize,
        pending_thoughts: after["counts"]["pendingThoughts"].as_u64().unwrap() as usize,
    }
}

fn classify(contract: &Value, fixture: &AgentFixture, pending_thoughts: u64) -> String {
    let lower = fixture.user.to_ascii_lowercase();
    if lower.trim() == "yes" && pending_thoughts == 0 {
        return "general-codex".to_string();
    }
    if fixture.reference.is_some() {
        return "using-reference.boundary".to_string();
    }
    if fixture.procedure.is_some() {
        return "using-procedure.boundary".to_string();
    }
    if pending_thoughts > 0
        && contract["intentPolicy"]["approvalPhrasesRequirePendingThought"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(Value::as_str)
            .any(|phrase| lower.contains(phrase) || lower.contains(&phrase.replace(' ', ", ")))
    {
        return "changing-me.approve-cognition".to_string();
    }
    if lower.contains("this is my thought")
        || contract["intentPolicy"]["captureOnlyPhrases"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(Value::as_str)
            .any(|phrase| lower.contains(&phrase.to_ascii_lowercase()))
    {
        return "changing-me.capture-thought".to_string();
    }
    if contract["intentPolicy"]["readOnlyPhrases"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(Value::as_str)
        .any(|phrase| lower.contains(&phrase.to_ascii_lowercase()))
    {
        return "using-me.read-only".to_string();
    }
    "general-codex".to_string()
}

fn thought_body_from_user(user: &str) -> String {
    if let Some((_, body)) = user.split_once('\n') {
        return body.trim().to_string();
    }
    for marker in [" in ME:", "this:", "ME."] {
        if let Some((_, body)) = user.split_once(marker) {
            let trimmed = body.trim();
            if !trimmed.is_empty() {
                return trimmed.to_string();
            }
        }
    }
    user.trim().to_string()
}

fn reference_boundary_response() -> String {
    "This document is a Reference, not Cognition. ME will not bulk-import it. Select an exact excerpt as a Thought if you want to keep it.".to_string()
}

fn assert_template_shape(rendered: &str, template: &str) {
    for line in template.lines().filter(|line| !line.trim().is_empty()) {
        if line.contains("...") {
            continue;
        }
        assert!(
            rendered.contains(line),
            "rendered output missing template line `{line}`:\n{rendered}"
        );
    }
}

fn assert_no_golden_leaks(rendered: &str) {
    for forbidden in [
        "Snapshot",
        "sha256:",
        "temporary task",
        "Decision JSON",
        "panic",
        "thread '",
    ] {
        assert!(
            !rendered.contains(forbidden),
            "golden output leaked {forbidden}: {rendered}"
        );
    }
}
