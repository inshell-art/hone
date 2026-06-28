use std::env;
use std::fs;
use std::io::{self, Read};
use std::path::{Path, PathBuf};
use std::process::{self, Command};

use clap::{Args, Parser, Subcommand};
use me_core::{SCHEMA_VERSION, WORKSPACE_VERSION};
use me_store::{MeError, Workspace};
use serde_json::{Value, json};

#[derive(Debug, Parser)]
#[command(
    name = "me",
    about = "ME — a local meaning environment",
    disable_version_flag = true
)]
struct Cli {
    #[arg(long, global = true)]
    workspace: Option<PathBuf>,

    #[arg(long, global = true)]
    json: bool,

    #[arg(long, global = true)]
    markdown: bool,

    #[arg(long, global = true, default_value_t = 1_048_576)]
    stdin_max_bytes: usize,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Debug, Subcommand)]
enum Commands {
    Version,
    Start(StartArgs),
    New(NewArgs),
    Init(InitArgs),
    Welcome,
    Home,
    Guide,
    Workspace(WorkspaceArgs),
    Status,
    Current,
    Doctor(DoctorArgs),
    Codex(CodexArgs),
    Thought(ThoughtArgs),
    Proposal(ProposalArgs),
    Review(ReviewArgs),
    Decide(DecideArgs),
    Reject(RejectArgs),
    Defer(DeferArgs),
    Cognition(CognitionArgs),
    Read(ReadArgs),
    Search(SearchArgs),
    Context(ContextArgs),
    Association(AssociationArgs),
    App(AppArgs),
    Run(RunArgs),
    History,
    Diff(DiffArgs),
    Snapshot(SnapshotArgs),
    Index(IndexArgs),
    Fsck,
    Bundle(BundleArgs),
    Export(ExportArgs),
    Migrate(MigrateArgs),
    Contract(ContractArgs),
}

#[derive(Debug, Args)]
struct ContractArgs {
    #[command(subcommand)]
    command: ContractCommand,
}

#[derive(Debug, Subcommand)]
enum ContractCommand {
    Show,
    Check,
}

#[derive(Debug, Args)]
struct StartArgs {
    #[arg(long)]
    workspace: Option<PathBuf>,
    #[arg(long)]
    no_open: bool,
    #[arg(long)]
    print_url: bool,
    #[arg(long)]
    prompt: Option<String>,
}

#[derive(Debug, Args)]
struct NewArgs {
    path: PathBuf,
    #[arg(long)]
    demo: bool,
}

#[derive(Debug, Args)]
struct InitArgs {
    path: Option<PathBuf>,
    #[arg(long)]
    demo: bool,
}

#[derive(Debug, Args)]
struct DoctorArgs {
    #[arg(long)]
    repair: bool,
}

#[derive(Debug, Args)]
struct CodexArgs {
    #[command(subcommand)]
    command: CodexCommand,
}

#[derive(Debug, Subcommand)]
enum CodexCommand {
    Sync,
}

#[derive(Debug, Args)]
struct ThoughtArgs {
    #[command(subcommand)]
    command: ThoughtCommand,
}

#[derive(Debug, Subcommand)]
enum ThoughtCommand {
    Capture(ThoughtCaptureArgs),
    List(ThoughtListArgs),
    Show(IdArg),
    Similar(ThoughtRelateArgs),
    Relate(ThoughtRelateArgs),
    Context(ThoughtRelateArgs),
}

#[derive(Debug, Args)]
struct ThoughtCaptureArgs {
    #[arg(long)]
    file: Option<PathBuf>,
    #[arg(long)]
    stdin: bool,
    #[arg(long)]
    kind: String,
}

#[derive(Debug, Args)]
struct ThoughtListArgs {
    #[arg(long)]
    state: Option<String>,
}

#[derive(Debug, Args)]
struct ThoughtRelateArgs {
    thought_id: String,
    #[arg(long, default_value_t = 5)]
    limit: usize,
}

#[derive(Debug, Args)]
struct ProposalArgs {
    #[command(subcommand)]
    command: ProposalCommand,
}

#[derive(Debug, Subcommand)]
enum ProposalCommand {
    Validate(FileArg),
    Save(FileArg),
    Show(IdArg),
    List(ProposalListArgs),
}

#[derive(Debug, Args)]
struct ProposalListArgs {
    #[arg(long)]
    status: Option<String>,
}

#[derive(Debug, Args)]
struct FileArg {
    file: PathBuf,
}

#[derive(Debug, Args)]
struct IdArg {
    id: String,
}

#[derive(Debug, Args)]
struct ReviewArgs {
    proposal_id: String,
}

#[derive(Debug, Args)]
struct DecideArgs {
    proposal_id: String,
    #[arg(long)]
    decision: PathBuf,
}

#[derive(Debug, Args)]
struct RejectArgs {
    proposal_id: String,
    #[arg(long)]
    note: Option<String>,
}

#[derive(Debug, Args)]
struct DeferArgs {
    proposal_id: String,
    #[arg(long)]
    note: Option<String>,
}

#[derive(Debug, Args)]
struct CognitionArgs {
    #[command(subcommand)]
    command: CognitionCommand,
}

#[derive(Debug, Subcommand)]
enum CognitionCommand {
    Add(CognitionAddArgs),
    List(CognitionListArgs),
    Show(IdArg),
    History(IdArg),
    Retire(CognitionStateArgs),
    Reactivate(CognitionStateArgs),
    Synthesize(SynthesizeArgs),
}

#[derive(Debug, Args)]
struct CognitionAddArgs {
    #[arg(long)]
    thought: String,
    #[arg(long)]
    decision: Option<PathBuf>,
    #[arg(long = "decision-stdin")]
    decision_stdin: bool,
}

#[derive(Debug, Args)]
struct CognitionListArgs {
    #[arg(long)]
    state: Option<String>,
}

#[derive(Debug, Args)]
struct CognitionStateArgs {
    cognition_id: String,
    #[arg(long)]
    decision: PathBuf,
}

#[derive(Debug, Args)]
struct SynthesizeArgs {
    #[arg(long)]
    spec: PathBuf,
}

#[derive(Debug, Args)]
struct ReadArgs {
    #[arg(long)]
    about: String,
    #[arg(long, default_value_t = 5)]
    limit: usize,
}

#[derive(Debug, Args)]
struct SearchArgs {
    query: String,
    #[arg(long, default_value_t = 20)]
    limit: usize,
    #[arg(long)]
    state: Option<String>,
}

#[derive(Debug, Args)]
struct ContextArgs {
    #[arg(long)]
    task: Option<PathBuf>,
    #[arg(long)]
    stdin: bool,
    #[arg(long, default_value_t = 20)]
    limit: usize,
}

#[derive(Debug, Args)]
struct WorkspaceArgs {
    #[command(subcommand)]
    command: WorkspaceCommand,
}

#[derive(Debug, Subcommand)]
enum WorkspaceCommand {
    Default,
    SetDefault(WorkspaceSetDefaultArgs),
    List,
}

#[derive(Debug, Args)]
struct WorkspaceSetDefaultArgs {
    path: PathBuf,
}

#[derive(Debug, Args)]
struct AssociationArgs {
    #[command(subcommand)]
    command: AssociationCommand,
}

#[derive(Debug, Subcommand)]
enum AssociationCommand {
    Infer(AssociationInferArgs),
    List(AssociationListArgs),
    Confirm(AssociationConfirmArgs),
    Remove(AssociationRemoveArgs),
}

#[derive(Debug, Args)]
struct AssociationInferArgs {
    #[arg(long)]
    cognition: Option<String>,
}

#[derive(Debug, Args)]
struct AssociationListArgs {
    #[arg(long)]
    kind: Option<String>,
}

#[derive(Debug, Args)]
struct AssociationConfirmArgs {
    #[arg(long)]
    spec: PathBuf,
}

#[derive(Debug, Args)]
struct AssociationRemoveArgs {
    association_id: String,
    #[arg(long)]
    decision: PathBuf,
}

#[derive(Debug, Args)]
struct AppArgs {
    #[command(subcommand)]
    command: AppCommand,
}

#[derive(Debug, Subcommand)]
enum AppCommand {
    List,
    Show(AppShowArgs),
    Validate(AppDirectoryArgs),
    Install(AppDirectoryArgs),
    Run(AppRunArgs),
    Prepare(AppRunArgs),
    Analyze(AppAnalyzeArgs),
    Resolve(AppResolveArgs),
    SaveRun(FileArg),
}

#[derive(Debug, Args)]
struct AppShowArgs {
    app_id: String,
}

#[derive(Debug, Args)]
struct AppDirectoryArgs {
    app_directory: PathBuf,
}

#[derive(Debug, Args)]
struct AppRunArgs {
    app_id: String,
    #[arg(long)]
    task: PathBuf,
    #[arg(long)]
    context_only: bool,
}

#[derive(Debug, Args)]
struct AppAnalyzeArgs {
    app_id: String,
    #[arg(long)]
    context: PathBuf,
    #[arg(long)]
    analysis: PathBuf,
}

#[derive(Debug, Args)]
struct AppResolveArgs {
    run_id: String,
    #[arg(long)]
    decision: PathBuf,
    #[arg(long)]
    scope: String,
}

#[derive(Debug, Args)]
struct RunArgs {
    #[command(subcommand)]
    command: RunCommand,
}

#[derive(Debug, Subcommand)]
enum RunCommand {
    List(RunListArgs),
    Show(RunShowArgs),
}

#[derive(Debug, Args)]
struct RunListArgs {
    #[arg(long)]
    app: Option<String>,
}

#[derive(Debug, Args)]
struct RunShowArgs {
    run_id: String,
}

#[derive(Debug, Args)]
struct DiffArgs {
    snapshot_a: String,
    snapshot_b: String,
}

#[derive(Debug, Args)]
struct SnapshotArgs {
    #[command(subcommand)]
    command: SnapshotCommand,
}

#[derive(Debug, Subcommand)]
enum SnapshotCommand {
    List,
    Show(IdArg),
    Restore(SnapshotRestoreArgs),
}

#[derive(Debug, Args)]
struct SnapshotRestoreArgs {
    snapshot_id: String,
    #[arg(long)]
    decision: PathBuf,
}

#[derive(Debug, Args)]
struct IndexArgs {
    #[command(subcommand)]
    command: IndexCommand,
}

#[derive(Debug, Subcommand)]
enum IndexCommand {
    Rebuild,
}

#[derive(Debug, Args)]
struct BundleArgs {
    #[command(subcommand)]
    command: BundleCommand,
}

#[derive(Debug, Subcommand)]
enum BundleCommand {
    Create(OutputArg),
    Verify(FileArg),
    Restore(BundleRestoreArgs),
}

#[derive(Debug, Args)]
struct OutputArg {
    output: PathBuf,
}

#[derive(Debug, Args)]
struct BundleRestoreArgs {
    file: PathBuf,
    target_directory: PathBuf,
}

#[derive(Debug, Args)]
struct ExportArgs {
    #[command(subcommand)]
    command: ExportCommand,
}

#[derive(Debug, Subcommand)]
enum ExportCommand {
    Workspace(ExportWorkspaceArgs),
}

#[derive(Debug, Args)]
struct ExportWorkspaceArgs {
    #[arg(long, default_value = "json")]
    format: String,
    #[arg(long)]
    output: PathBuf,
}

#[derive(Debug, Args)]
struct MigrateArgs {
    #[arg(long = "from-my-model")]
    from_my_model: Option<PathBuf>,
    #[arg(long = "from-v3")]
    from_v3: Option<PathBuf>,
    #[arg(long = "from-v4")]
    from_v4: Option<PathBuf>,
}

fn main() {
    let raw_args: Vec<String> = env::args().collect();
    if raw_args.len() == 2 && matches!(raw_args[1].as_str(), "--version" | "-V") {
        println!("ME {WORKSPACE_VERSION}");
        return;
    }

    let cli = Cli::parse();
    if matches!(cli.command, Commands::Version) {
        print_version(cli.json);
        return;
    }

    let (command, result) = dispatch(&cli);
    match result {
        Ok(data) => print_success(command, &data, cli.json),
        Err(err) => {
            print_error(command, &err, cli.json);
            process::exit(err.exit_code());
        }
    }
}

fn dispatch(cli: &Cli) -> (&'static str, Result<Value, MeError>) {
    match &cli.command {
        Commands::Version => ("version", Ok(version_info())),
        Commands::Start(args) => ("start", start_me(cli, args)),
        Commands::New(args) => ("new", Workspace::new_workspace(&args.path, args.demo)),
        Commands::Init(args) => {
            let path = args
                .path
                .clone()
                .or_else(|| cli.workspace.clone())
                .unwrap_or_else(|| env::current_dir().expect("current directory"));
            ("init", Workspace::init(path, args.demo))
        }
        Commands::Welcome => with_workspace(cli, "welcome", |ws| ws.welcome()),
        Commands::Home => with_workspace(cli, "home", |ws| {
            ws.home(if cli.json { "json" } else { "markdown" })
        }),
        Commands::Guide => with_workspace(cli, "guide", |ws| ws.guide()),
        Commands::Workspace(args) => ("workspace", workspace_command(args)),
        Commands::Status => with_workspace(cli, "status", |ws| ws.status()),
        Commands::Current => with_workspace(cli, "current", |ws| ws.current()),
        Commands::Doctor(args) => with_workspace(cli, "doctor", |ws| ws.doctor(args.repair)),
        Commands::Codex(args) => match args.command {
            CodexCommand::Sync => with_workspace(cli, "codex sync", |ws| ws.codex_sync()),
        },
        Commands::Thought(args) => match &args.command {
            ThoughtCommand::Capture(inner) => with_workspace(cli, "thought capture", |ws| {
                if inner.stdin && inner.file.is_some() {
                    Err(invalid_cli(
                        "thought capture accepts only one of --file or --stdin",
                    ))
                } else if inner.stdin {
                    let body = read_stdin_utf8(cli.stdin_max_bytes)?;
                    ws.thought_capture_body(body, &inner.kind)
                } else if let Some(file) = &inner.file {
                    ws.thought_capture(file, &inner.kind)
                } else {
                    Err(invalid_cli("thought capture requires --file or --stdin"))
                }
            }),
            ThoughtCommand::List(inner) => with_workspace(cli, "thought list", |ws| {
                ws.thought_list(inner.state.clone())
            }),
            ThoughtCommand::Show(inner) => {
                with_workspace(cli, "thought show", |ws| ws.thought_show(&inner.id))
            }
            ThoughtCommand::Similar(inner) => with_workspace(cli, "thought similar", |ws| {
                ws.thought_relate(&inner.thought_id, inner.limit)
            }),
            ThoughtCommand::Relate(inner) => with_workspace(cli, "thought relate", |ws| {
                ws.thought_relate(&inner.thought_id, inner.limit)
            }),
            ThoughtCommand::Context(inner) => with_workspace(cli, "thought context", |ws| {
                ws.thought_context(&inner.thought_id, inner.limit)
            }),
        },
        Commands::Proposal(args) => match &args.command {
            ProposalCommand::Validate(inner) => with_workspace(cli, "proposal validate", |ws| {
                ws.validate_proposal_file(&inner.file)
            }),
            ProposalCommand::Save(inner) => with_workspace(cli, "proposal save", |ws| {
                ws.save_proposal_file(&inner.file)
            }),
            ProposalCommand::Show(inner) => {
                with_workspace(cli, "proposal show", |ws| ws.show_proposal(&inner.id))
            }
            ProposalCommand::List(inner) => with_workspace(cli, "proposal list", |ws| {
                ws.list_proposals(inner.status.clone())
            }),
        },
        Commands::Review(args) => with_workspace(cli, "review", |ws| {
            ws.review(
                &args.proposal_id,
                if cli.markdown { "markdown" } else { "json" },
            )
        }),
        Commands::Decide(args) => with_workspace(cli, "decide", |ws| {
            ws.decide(&args.proposal_id, &args.decision)
        }),
        Commands::Reject(args) => with_workspace(cli, "reject", |ws| {
            ws.reject_or_defer(&args.proposal_id, "reject", args.note.clone())
        }),
        Commands::Defer(args) => with_workspace(cli, "defer", |ws| {
            ws.reject_or_defer(&args.proposal_id, "defer", args.note.clone())
        }),
        Commands::Cognition(args) => match &args.command {
            CognitionCommand::Add(inner) => with_workspace(cli, "cognition add", |ws| {
                if inner.decision_stdin && inner.decision.is_some() {
                    Err(invalid_cli(
                        "cognition add accepts only one of --decision or --decision-stdin",
                    ))
                } else if inner.decision_stdin {
                    let raw = read_stdin_utf8(cli.stdin_max_bytes)?;
                    let value = Workspace::parse_decision_input(&raw)?;
                    ws.cognition_add_value(&inner.thought, value)
                } else if let Some(decision) = &inner.decision {
                    ws.cognition_add(&inner.thought, decision)
                } else {
                    Err(invalid_cli(
                        "cognition add requires --decision or --decision-stdin",
                    ))
                }
            }),
            CognitionCommand::List(inner) => with_workspace(cli, "cognition list", |ws| {
                ws.cognition_list(inner.state.clone())
            }),
            CognitionCommand::Show(inner) => {
                with_workspace(cli, "cognition show", |ws| ws.cognition_show(&inner.id))
            }
            CognitionCommand::History(inner) => with_workspace(cli, "cognition history", |ws| {
                ws.cognition_history(&inner.id)
            }),
            CognitionCommand::Retire(inner) => with_workspace(cli, "cognition retire", |ws| {
                ws.cognition_retire(&inner.cognition_id, &inner.decision)
            }),
            CognitionCommand::Reactivate(inner) => {
                with_workspace(cli, "cognition reactivate", |ws| {
                    ws.cognition_reactivate(&inner.cognition_id, &inner.decision)
                })
            }
            CognitionCommand::Synthesize(inner) => {
                with_workspace(cli, "cognition synthesize", |ws| {
                    ws.cognition_synthesize(&inner.spec)
                })
            }
        },
        Commands::Read(args) => with_workspace(cli, "read", |ws| ws.read(&args.about, args.limit)),
        Commands::Search(args) => with_workspace(cli, "search", |ws| {
            ws.search(&args.query, args.limit, args.state.clone())
        }),
        Commands::Context(args) => with_workspace(cli, "context", |ws| {
            if args.stdin && args.task.is_some() {
                Err(invalid_cli("context accepts only one of --task or --stdin"))
            } else if args.stdin {
                let task = read_stdin_utf8(cli.stdin_max_bytes)?;
                ws.context_body(task, args.limit)
            } else if let Some(task) = &args.task {
                ws.context(task, args.limit)
            } else {
                Err(invalid_cli("context requires --task or --stdin"))
            }
        }),
        Commands::Association(args) => match &args.command {
            AssociationCommand::Infer(inner) => with_workspace(cli, "association infer", |ws| {
                ws.association_infer(inner.cognition.clone())
            }),
            AssociationCommand::List(inner) => with_workspace(cli, "association list", |ws| {
                ws.association_list(inner.kind.clone())
            }),
            AssociationCommand::Confirm(inner) => {
                with_workspace(cli, "association confirm", |ws| {
                    ws.association_confirm(&inner.spec)
                })
            }
            AssociationCommand::Remove(inner) => with_workspace(cli, "association remove", |ws| {
                ws.association_remove(&inner.association_id, &inner.decision)
            }),
        },
        Commands::App(args) => match &args.command {
            AppCommand::List => with_workspace(cli, "app list", |ws| ws.app_list()),
            AppCommand::Show(inner) => {
                with_workspace(cli, "app show", |ws| ws.app_show(&inner.app_id))
            }
            AppCommand::Validate(inner) => with_workspace(cli, "app validate", |ws| {
                ws.app_validate(&inner.app_directory)
            }),
            AppCommand::Install(inner) => with_workspace(cli, "app install", |ws| {
                ws.app_install(&inner.app_directory)
            }),
            AppCommand::Run(inner) => with_workspace(cli, "app run", |ws| {
                ws.app_run(&inner.app_id, &inner.task, inner.context_only)
            }),
            AppCommand::Prepare(inner) => with_workspace(cli, "app prepare", |ws| {
                ws.app_prepare(&inner.app_id, &inner.task)
            }),
            AppCommand::Analyze(inner) => with_workspace(cli, "app analyze", |ws| {
                ws.app_analyze(&inner.app_id, &inner.context, &inner.analysis)
            }),
            AppCommand::Resolve(inner) => with_workspace(cli, "app resolve", |ws| {
                ws.app_resolve(&inner.run_id, &inner.decision, &inner.scope)
            }),
            AppCommand::SaveRun(inner) => {
                with_workspace(cli, "app save-run", |ws| ws.app_save_run(&inner.file))
            }
        },
        Commands::Run(args) => match &args.command {
            RunCommand::List(inner) => {
                with_workspace(cli, "run list", |ws| ws.run_list(inner.app.clone()))
            }
            RunCommand::Show(inner) => with_workspace(cli, "run show", |ws| {
                ws.run_show(
                    &inner.run_id,
                    if cli.markdown { "markdown" } else { "json" },
                )
            }),
        },
        Commands::History => with_workspace(cli, "history", |ws| ws.history()),
        Commands::Diff(args) => with_workspace(cli, "diff", |ws| {
            ws.diff(
                &args.snapshot_a,
                &args.snapshot_b,
                if cli.markdown { "markdown" } else { "json" },
            )
        }),
        Commands::Snapshot(args) => match &args.command {
            SnapshotCommand::List => with_workspace(cli, "snapshot list", |ws| ws.snapshot_list()),
            SnapshotCommand::Show(inner) => {
                with_workspace(cli, "snapshot show", |ws| ws.snapshot_show(&inner.id))
            }
            SnapshotCommand::Restore(inner) => with_workspace(cli, "snapshot restore", |ws| {
                ws.snapshot_restore(&inner.snapshot_id, &inner.decision)
            }),
        },
        Commands::Index(args) => match args.command {
            IndexCommand::Rebuild => with_workspace(cli, "index rebuild", |ws| ws.index_rebuild()),
        },
        Commands::Fsck => with_workspace(cli, "fsck", |ws| ws.fsck()),
        Commands::Bundle(args) => match &args.command {
            BundleCommand::Create(inner) => {
                with_workspace(cli, "bundle create", |ws| ws.bundle_create(&inner.output))
            }
            BundleCommand::Verify(inner) => {
                ("bundle verify", Workspace::bundle_verify_file(&inner.file))
            }
            BundleCommand::Restore(inner) => (
                "bundle restore",
                Workspace::bundle_restore(&inner.file, &inner.target_directory),
            ),
        },
        Commands::Export(args) => match &args.command {
            ExportCommand::Workspace(inner) => {
                if inner.format != "json" {
                    return (
                        "export workspace",
                        Err(MeError::InvalidInput {
                            code: "INVALID_INPUT",
                            message: "Only JSON workspace export is supported".to_string(),
                            details: json!({ "format": inner.format }),
                        }),
                    );
                }
                with_workspace(cli, "export workspace", |ws| {
                    ws.export_workspace(&inner.output)
                })
            }
        },
        Commands::Migrate(args) => {
            if let Some(path) = &args.from_my_model {
                (
                    "migrate from-my-model",
                    Workspace::migrate_from_my_model(path),
                )
            } else if let Some(path) = &args.from_v3 {
                ("migrate from-v3", Workspace::migrate_from_v3(path))
            } else if let Some(path) = &args.from_v4 {
                ("migrate from-v4", Workspace::migrate_from_v4(path))
            } else {
                (
                    "migrate",
                    Err(MeError::InvalidInput {
                        code: "INVALID_INPUT",
                        message: "migrate requires --from-my-model, --from-v3, or --from-v4"
                            .to_string(),
                        details: json!({}),
                    }),
                )
            }
        }
        Commands::Contract(args) => match args.command {
            ContractCommand::Show => ("contract show", contract_show()),
            ContractCommand::Check => ("contract check", contract_check()),
        },
    }
}

const SEMANTIC_CONTRACT_JSON: &str =
    include_str!("../../../contracts/me-semantic-contract.v1.json");
const WORKSPACE_SKILL_MD: &str =
    include_str!("../../../templates/workspace/.agents/skills/me/SKILL.md");
const README_MD: &str = include_str!("../../../README.md");
const CONSTITUTION_MD: &str = include_str!("../../../docs/constitution.md");
const PHILOSOPHY_MD: &str = include_str!("../../../docs/philosophy.md");
const CONTRACTS_MD: &str = include_str!("../../../docs/contracts.md");
const HARNESS_MD: &str = include_str!("../../../docs/harness.md");
const TESTING_AGENT_TOOLS_MD: &str = include_str!("../../../docs/testing-agent-tools.md");
const THOUGHT_CAPTURED_TEMPLATE: &str =
    include_str!("../../../templates/render/thought-captured.md");
const COGNITION_KEPT_TEMPLATE: &str = include_str!("../../../templates/render/cognition-kept.md");
const USING_ME_TEMPLATE: &str = include_str!("../../../templates/render/using-me-read-only.md");
const OUTPUT_FEEDBACK_TEMPLATE: &str = include_str!("../../../templates/render/output-feedback.md");
const INVALID_MISSING_APPROVAL_TEMPLATE: &str =
    include_str!("../../../templates/render/invalid-missing-approval.md");
const AGENT_FIXTURES_JSON: &str =
    include_str!("../../../tests/agent-fixtures/semantic-boundary-fixtures.json");

fn contract_show() -> Result<Value, MeError> {
    let contract: Value = serde_json::from_str(SEMANTIC_CONTRACT_JSON)
        .map_err(|err| MeError::Internal(err.into()))?;
    Ok(json!({
        "schemaVersion": 1,
        "kind": "me.semantic-contract",
        "contract": contract,
        "markdown": contract_markdown(&contract)
    }))
}

fn contract_check() -> Result<Value, MeError> {
    let mut checks = Vec::new();
    let mut failures = Vec::new();
    let contract: Value = match serde_json::from_str(SEMANTIC_CONTRACT_JSON) {
        Ok(value) => {
            push_check(&mut checks, "semantic-contract-json-valid", true);
            value
        }
        Err(err) => {
            push_check(&mut checks, "semantic-contract-json-valid", false);
            failures.push(format!("semantic contract JSON is invalid: {err}"));
            Value::Null
        }
    };

    if !contract.is_null() {
        check_bool(
            &mut checks,
            &mut failures,
            "principle-present",
            contract["principle"].as_str()
                == Some("Prompts guide the model. Transactions govern the product."),
        );
        check_bool(
            &mut checks,
            &mut failures,
            "add-cognition-requires-approved",
            contract["transitions"].as_array().is_some_and(|items| {
                items.iter().any(|item| {
                    item["id"] == "add-cognition"
                        && item["requiresApproval"] == true
                        && item["requiresDecisionField"]["approved"] == true
                })
            }),
        );
        check_bool(
            &mut checks,
            &mut failures,
            "direct-boundaries-forbidden",
            contract["boundaries"]["outputToCognitionDirect"] == "forbidden"
                && contract["boundaries"]["referenceToCognitionDirect"] == "forbidden"
                && contract["boundaries"]["procedureToCognitionDirect"] == "forbidden",
        );
    }

    for phrase in [
        "A user utterance can supply a Thought.",
        "It cannot also serve as approval unless the user is responding to a specific Thought that has just been shown back.",
        "Never call `me cognition add` immediately after first seeing a Thought.",
        "Never infer `approved: true` from the same message that supplied the Thought.",
        "Never save Codex Output as a Cognition directly.",
        "Never bulk-add a Reference file.",
        "A captured thought is not a cognition and is not in ME yet.",
    ] {
        check_bool(
            &mut checks,
            &mut failures,
            format!("skill-contains-{phrase}"),
            WORKSPACE_SKILL_MD.contains(phrase),
        );
    }

    check_bool(
        &mut checks,
        &mut failures,
        "readme-references-constitution",
        README_MD.contains("docs/constitution.md")
            && README_MD.contains("Why ME asks before keeping a Thought"),
    );
    check_bool(
        &mut checks,
        &mut failures,
        "docs-present",
        CONSTITUTION_MD.contains("Thought Is Not Cognition")
            && PHILOSOPHY_MD.contains("Count design")
            && CONTRACTS_MD.contains("Vocabulary Contract")
            && HARNESS_MD.contains("Codex is the runtime harness.")
            && TESTING_AGENT_TOOLS_MD.contains("Simulated Agent-Harness Tests"),
    );
    check_bool(
        &mut checks,
        &mut failures,
        "render-templates-present",
        THOUGHT_CAPTURED_TEMPLATE.contains("not in ME yet")
            && COGNITION_KEPT_TEMPLATE.contains("This thought is now a cognition.")
            && USING_ME_TEMPLATE.contains("ME was read, not changed.")
            && OUTPUT_FEEDBACK_TEMPLATE.contains("This is my thought. Add it to ME.")
            && INVALID_MISSING_APPROVAL_TEMPLATE.contains("explicitly approve keeping it"),
    );

    let fixtures: Value = match serde_json::from_str(AGENT_FIXTURES_JSON) {
        Ok(value) => {
            push_check(&mut checks, "agent-fixtures-json-valid", true);
            value
        }
        Err(err) => {
            push_check(&mut checks, "agent-fixtures-json-valid", false);
            failures.push(format!("agent fixture JSON is invalid: {err}"));
            Value::Null
        }
    };
    if let Some(items) = fixtures.as_array() {
        for required in [
            "casual-add-captures-only",
            "save-wording-captures-only",
            "remember-wording-captures-only",
            "note-wording-captures-only",
            "put-this-in-me-captures-only",
            "explicit-approval-promotes",
            "ambiguous-yes-without-pending-thought-fails-safely",
            "read-only-use-does-not-mutate",
            "output-feedback-reenters-as-thought",
            "reference-not-bulk-imported",
            "procedure-not-cognition",
        ] {
            check_bool(
                &mut checks,
                &mut failures,
                format!("fixture-{required}"),
                items.iter().any(|item| item["name"] == required),
            );
        }
    }

    if failures.is_empty() {
        Ok(json!({
            "ok": true,
            "checked": checks.len(),
            "checks": checks,
            "failures": []
        }))
    } else {
        Err(MeError::InvalidInput {
            code: "CONTRACT_CHECK_FAILED",
            message: "ME semantic contract check failed".to_string(),
            details: json!({
                "checked": checks.len(),
                "checks": checks,
                "failures": failures
            }),
        })
    }
}

fn push_check(checks: &mut Vec<Value>, name: impl Into<String>, ok: bool) {
    checks.push(json!({
        "name": name.into(),
        "ok": ok
    }));
}

fn check_bool(
    checks: &mut Vec<Value>,
    failures: &mut Vec<String>,
    name: impl Into<String>,
    ok: bool,
) {
    let name = name.into();
    push_check(checks, name.clone(), ok);
    if !ok {
        failures.push(name);
    }
}

fn contract_markdown(contract: &Value) -> String {
    let thought_states = contract["states"]["thought"]
        .as_array()
        .map(|items| {
            items
                .iter()
                .filter_map(Value::as_str)
                .collect::<Vec<_>>()
                .join(", ")
        })
        .unwrap_or_default();
    let cognition_states = contract["states"]["cognition"]
        .as_array()
        .map(|items| {
            items
                .iter()
                .filter_map(Value::as_str)
                .collect::<Vec<_>>()
                .join(", ")
        })
        .unwrap_or_default();
    let transitions = contract["transitions"]
        .as_array()
        .map(|items| {
            items
                .iter()
                .filter_map(|item| item["id"].as_str())
                .map(|id| format!("- {id}"))
                .collect::<Vec<_>>()
                .join("\n")
        })
        .unwrap_or_default();
    format!(
        r#"# ME Semantic Contract v1

{}

## Semantic State Machine

```text
Utterance
  -> interpreted intent
  -> counted product meaning
  -> legal transition
  -> deterministic transaction
  -> canonical state
  -> rendered proof
```

## States

Thought: {thought_states}

Cognition: {cognition_states}

## Transitions

{transitions}

## Count Design

- Casual add, capture, save, note, remember, or put-in-ME wording counts as Thought capture only.
- Approval phrases count only when a specific pending Thought was just shown back.
- Read-only use must not advance the current Snapshot.
- Output, Reference, and Procedure routes cannot create Cognitions directly.

## Product Law

Prompts guide the model. Transactions govern the product.
"#,
        contract["principle"].as_str().unwrap_or("")
    )
}

fn version_info() -> Value {
    json!({
        "product": "ME",
        "descriptor": "a local meaning environment",
        "version": WORKSPACE_VERSION,
        "binary": "me",
        "cargoPackage": "me-cli",
        "workspaceSchema": SCHEMA_VERSION
    })
}

fn print_version(json_output: bool) {
    if json_output {
        println!(
            "{}",
            serde_json::to_string_pretty(&version_info()).expect("json")
        );
    } else {
        println!("ME {WORKSPACE_VERSION}");
    }
}

fn with_workspace(
    cli: &Cli,
    command: &'static str,
    f: impl FnOnce(&Workspace) -> Result<Value, MeError>,
) -> (&'static str, Result<Value, MeError>) {
    let root = cli
        .workspace
        .clone()
        .unwrap_or_else(|| env::current_dir().expect("current directory"));
    let result = Workspace::open(root).and_then(|ws| f(&ws));
    (command, result)
}

fn invalid_cli(message: impl Into<String>) -> MeError {
    MeError::InvalidInput {
        code: "INVALID_INPUT",
        message: message.into(),
        details: json!({}),
    }
}

fn read_stdin_utf8(max_bytes: usize) -> Result<String, MeError> {
    let mut bytes = Vec::new();
    let limit = max_bytes.saturating_add(1) as u64;
    io::stdin()
        .take(limit)
        .read_to_end(&mut bytes)
        .map_err(|err| MeError::Internal(err.into()))?;
    if bytes.len() > max_bytes {
        return Err(invalid_cli(format!(
            "stdin input exceeds maximum size of {max_bytes} bytes"
        )));
    }
    String::from_utf8(bytes).map_err(|_| invalid_cli("stdin input must be valid UTF-8"))
}

fn start_me(cli: &Cli, args: &StartArgs) -> Result<Value, MeError> {
    let prompt = args.prompt.as_deref().unwrap_or("Start ME");
    let resolved = resolve_start_workspace(cli, args)?;
    let ws = Workspace::open(&resolved.path)?;
    let before = ws.status()?;
    ws.fsck().map_err(|err| match err {
        MeError::Integrity { .. } => MeError::Integrity {
            code: "INTEGRITY_FAILURE",
            message:
                "ME could not start because the workspace failed integrity checks.\n\nRun:\n  me fsck\n\nNo canonical state was changed."
                    .to_string(),
            details: json!({ "workspace": resolved.path }),
        },
        other => other,
    })?;
    ws.doctor(true)?;
    let after = ws.status()?;
    if before["currentSnapshot"] != after["currentSnapshot"] {
        return Err(MeError::Integrity {
            code: "INTEGRITY_FAILURE",
            message: "ME start preflight changed the canonical Snapshot".to_string(),
            details: json!({ "workspace": resolved.path }),
        });
    }
    let deep_link = codex_deep_link(&resolved.path, prompt);
    let no_open = args.no_open || args.print_url;
    let mut attempted_open = false;
    let mut warnings = resolved.warnings;
    if !no_open {
        if cfg!(target_os = "macos") {
            match Command::new("/usr/bin/open")
                .args(["-Ra", "Codex"])
                .status()
            {
                Ok(status) if status.success() => {
                    attempted_open = true;
                    match Command::new("/usr/bin/open").arg(&deep_link).status() {
                        Ok(status) if status.success() => {}
                        Ok(status) => warnings
                            .push(format!("Codex deep link open exited with status {status}")),
                        Err(err) => warnings.push(format!("Codex deep link open failed: {err}")),
                    }
                }
                _ => warnings.push("Codex App was not found.".to_string()),
            }
        } else {
            warnings
                .push("Codex App deep links are supported only on macOS in ME v0.x.".to_string());
        }
    }
    Ok(json!({
        "workspacePath": resolved.path,
        "workspaceCreated": resolved.created,
        "preflight": {
            "canonicalIntegrity": "ok",
            "derivedState": "ok"
        },
        "codex": {
            "attemptedOpen": attempted_open,
            "deepLink": deep_link,
            "prompt": prompt,
            "promptSubmitted": false
        },
        "printUrlOnly": args.print_url,
        "warnings": warnings
    }))
}

#[derive(Debug)]
struct ResolvedWorkspace {
    path: PathBuf,
    created: bool,
    warnings: Vec<String>,
}

fn resolve_start_workspace(cli: &Cli, args: &StartArgs) -> Result<ResolvedWorkspace, MeError> {
    if let Some(path) = args.workspace.as_ref().or(cli.workspace.as_ref()) {
        return ensure_workspace(path);
    }
    let cwd = env::current_dir().map_err(|err| MeError::Internal(err.into()))?;
    if Workspace::open(&cwd).is_ok() {
        return Ok(ResolvedWorkspace {
            path: absolutize(&cwd)?,
            created: false,
            warnings: Vec::new(),
        });
    }
    if let Some(default) = read_default_workspace()? {
        if Workspace::open(&default).is_ok() {
            return Ok(ResolvedWorkspace {
                path: absolutize(&default)?,
                created: false,
                warnings: Vec::new(),
            });
        }
    }
    let home_me = home_dir()?.join("ME");
    if Workspace::open(&home_me).is_ok() {
        return Ok(ResolvedWorkspace {
            path: absolutize(&home_me)?,
            created: false,
            warnings: Vec::new(),
        });
    }
    let resolved = ensure_workspace(&home_me)?;
    write_default_workspace(&resolved.path)?;
    Ok(resolved)
}

fn ensure_workspace(path: &Path) -> Result<ResolvedWorkspace, MeError> {
    let path = absolutize(path)?;
    if Workspace::open(&path).is_ok() {
        return Ok(ResolvedWorkspace {
            path,
            created: false,
            warnings: Vec::new(),
        });
    }
    Workspace::new_workspace(&path, false)?;
    Ok(ResolvedWorkspace {
        path,
        created: true,
        warnings: Vec::new(),
    })
}

fn workspace_command(args: &WorkspaceArgs) -> Result<Value, MeError> {
    match &args.command {
        WorkspaceCommand::Default => {
            let default = read_default_workspace()?;
            Ok(json!({ "defaultWorkspace": default }))
        }
        WorkspaceCommand::SetDefault(inner) => {
            let path = absolutize(&inner.path)?;
            Workspace::open(&path)?;
            write_default_workspace(&path)?;
            Ok(json!({ "defaultWorkspace": path }))
        }
        WorkspaceCommand::List => {
            let default = read_default_workspace()?;
            let workspaces = default
                .as_ref()
                .map(|path| vec![json!({ "path": path, "default": true })])
                .unwrap_or_default();
            Ok(json!({ "workspaces": workspaces }))
        }
    }
}

fn config_path() -> Result<PathBuf, MeError> {
    if let Some(config_home) = env::var_os("XDG_CONFIG_HOME") {
        Ok(PathBuf::from(config_home).join("me/config.toml"))
    } else {
        Ok(home_dir()?.join(".config/me/config.toml"))
    }
}

fn read_default_workspace() -> Result<Option<PathBuf>, MeError> {
    let path = config_path()?;
    if !path.exists() {
        return Ok(None);
    }
    let raw = fs::read_to_string(&path).map_err(|err| MeError::Internal(err.into()))?;
    let value: toml::Value = toml::from_str(&raw).map_err(|err| MeError::Internal(err.into()))?;
    Ok(value
        .get("default_workspace")
        .and_then(toml::Value::as_str)
        .map(PathBuf::from))
}

fn write_default_workspace(path: &Path) -> Result<(), MeError> {
    let config = config_path()?;
    if let Some(parent) = config.parent() {
        fs::create_dir_all(parent).map_err(|err| MeError::Internal(err.into()))?;
    }
    let body = format!(
        "schema_version = 1\ndefault_workspace = \"{}\"\n",
        path.display()
            .to_string()
            .replace('\\', "\\\\")
            .replace('"', "\\\"")
    );
    fs::write(config, body).map_err(|err| MeError::Internal(err.into()))
}

fn home_dir() -> Result<PathBuf, MeError> {
    env::var_os("HOME")
        .map(PathBuf::from)
        .ok_or_else(|| invalid_cli("HOME is not set"))
}

fn absolutize(path: &Path) -> Result<PathBuf, MeError> {
    if path.is_absolute() {
        Ok(path.to_path_buf())
    } else {
        Ok(env::current_dir()
            .map_err(|err| MeError::Internal(err.into()))?
            .join(path))
    }
}

fn codex_deep_link(path: &Path, prompt: &str) -> String {
    format!(
        "codex://new?path={}&prompt={}",
        percent_encode(&path.display().to_string()),
        percent_encode(prompt)
    )
}

fn percent_encode(input: &str) -> String {
    let mut out = String::new();
    for byte in input.bytes() {
        if byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.' | b'~') {
            out.push(byte as char);
        } else {
            out.push_str(&format!("%{byte:02X}"));
        }
    }
    out
}

fn print_success(command: &str, data: &Value, structured_output: bool) {
    if structured_output {
        println!(
            "{}",
            serde_json::to_string_pretty(&structured_success_value(command, data)).expect("json")
        );
        return;
    }

    if matches!(command, "new" | "init") {
        print!("{}", workspace_created_text(data));
    } else if command == "start" {
        if data
            .get("printUrlOnly")
            .and_then(Value::as_bool)
            .unwrap_or(false)
        {
            println!("{}", data["codex"]["deepLink"].as_str().unwrap_or(""));
        } else {
            print!("{}", start_text(data));
        }
    } else if let Some(markdown) = data.get("markdown").and_then(Value::as_str) {
        print!("{markdown}");
    } else if let Some(markdown) = data.get("renderedMarkdown").and_then(Value::as_str) {
        print!("{markdown}");
    } else if let Some(review) = data.get("review").and_then(Value::as_str) {
        print!("{review}");
    } else if let Some(text) = data.get("text").and_then(Value::as_str) {
        println!("{text}");
    } else {
        println!("{}", serde_json::to_string_pretty(data).expect("json"));
    }
}

fn structured_success_value(command: &str, data: &Value) -> Value {
    if matches!(command, "home" | "guide" | "welcome") {
        data.clone()
    } else if command == "start" {
        let mut data = data.clone();
        let warnings = data.get("warnings").cloned().unwrap_or_else(|| json!([]));
        if let Some(map) = data.as_object_mut() {
            map.remove("warnings");
            map.remove("printUrlOnly");
        }
        json!({
            "ok": true,
            "command": command,
            "data": data,
            "warnings": warnings
        })
    } else {
        json!({
            "ok": true,
            "command": command,
            "data": data,
            "warnings": []
        })
    }
}

fn start_text(data: &Value) -> String {
    let workspace = data["workspacePath"].as_str().unwrap_or("");
    let warnings = data["warnings"].as_array().cloned().unwrap_or_default();
    if warnings
        .iter()
        .any(|warning| warning.as_str() == Some("Codex App was not found."))
    {
        return format!(
            r#"Codex App was not found.

ME is a local application operated through Codex App.

Install or open Codex App, then run:

  me start

Workspace:
  {workspace}
"#
        );
    }
    if data["codex"]["attemptedOpen"].as_bool().unwrap_or(false) {
        r#"Opened ME in Codex App.

Press Enter on:

  Start ME
"#
        .to_string()
    } else {
        format!(
            r#"ME is ready.

Workspace:
  {workspace}

Open Codex App and press Enter on:

  Start ME
"#
        )
    }
}

fn workspace_created_text(data: &Value) -> String {
    let path = data
        .get("workspacePath")
        .and_then(Value::as_str)
        .unwrap_or("ME workspace");
    format!(
        r#"ME workspace created at {path}

Open it now:

  me start --workspace {path}
"#
    )
}

fn print_error(command: &str, err: &MeError, json_output: bool) {
    if json_output {
        println!(
            "{}",
            serde_json::to_string_pretty(&json!({
                "ok": false,
                "command": command,
                "error": {
                    "code": err.code(),
                    "message": err.to_string(),
                    "details": err.details()
                }
            }))
            .expect("json")
        );
    } else if let Some(markdown) = err
        .details()
        .get("renderedMarkdown")
        .and_then(Value::as_str)
    {
        eprint!("{markdown}");
    } else {
        eprintln!("error[{}]: {}", err.code(), err);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn workspace_created_text_points_to_start_command() {
        let data = json!({
            "workspacePath": "/Users/name/ME"
        });
        let text = workspace_created_text(&data);
        assert!(text.starts_with("ME workspace created at /Users/name/ME"));
        assert!(text.contains("Open it now:"));
        assert!(text.contains("me start --workspace /Users/name/ME"));
        assert!(!text.contains("--json"));
        assert!(!text.contains("fsck"));
    }

    #[test]
    fn home_and_guide_json_are_contract_objects() {
        let home = json!({
            "schemaVersion": 1,
            "kind": "me.home"
        });
        let output = structured_success_value("home", &home);
        assert_eq!(output["schemaVersion"], 1);
        assert_eq!(output["kind"], "me.home");
        assert!(output.get("ok").is_none());
        assert!(output.get("data").is_none());

        let welcome = json!({
            "schemaVersion": 1,
            "kind": "me.welcome"
        });
        let output = structured_success_value("welcome", &welcome);
        assert_eq!(output["schemaVersion"], 1);
        assert_eq!(output["kind"], "me.welcome");
        assert!(output.get("ok").is_none());

        let created = json!({
            "workspacePath": "/Users/name/ME",
            "next": {
                "host": "Codex App",
                "mode": "Local",
                "starterPrompt": "Start ME"
            }
        });
        let output = structured_success_value("new", &created);
        assert_eq!(output["ok"], true);
        assert_eq!(output["command"], "new");
        assert_eq!(output["data"], created);
    }

    #[test]
    fn start_no_open_creates_workspace_and_deep_link() {
        let dir = tempdir().unwrap();
        let workspace = dir.path().join("ME Lab");
        let cli = Cli {
            workspace: None,
            json: false,
            markdown: false,
            stdin_max_bytes: 1_048_576,
            command: Commands::Version,
        };
        let args = StartArgs {
            workspace: Some(workspace.clone()),
            no_open: true,
            print_url: false,
            prompt: None,
        };
        let result = start_me(&cli, &args).unwrap();
        assert_eq!(result["workspaceCreated"], true);
        assert_eq!(result["preflight"]["canonicalIntegrity"], "ok");
        assert_eq!(result["preflight"]["derivedState"], "ok");
        assert_eq!(result["codex"]["attemptedOpen"], false);
        assert_eq!(result["codex"]["prompt"], "Start ME");
        assert_eq!(result["codex"]["promptSubmitted"], false);
        assert!(
            result["codex"]["deepLink"]
                .as_str()
                .unwrap()
                .contains("path=")
        );
        assert!(
            result["codex"]["deepLink"]
                .as_str()
                .unwrap()
                .contains("ME%20Lab")
        );
        assert!(
            result["codex"]["deepLink"]
                .as_str()
                .unwrap()
                .contains("prompt=Start%20ME")
        );
        assert!(workspace.join("views/welcome.md").exists());

        let result = start_me(&cli, &args).unwrap();
        assert_eq!(result["workspaceCreated"], false);
        assert_eq!(result["codex"]["attemptedOpen"], false);
    }

    #[test]
    fn start_print_url_outputs_only_deep_link_value() {
        let data = json!({
            "workspacePath": "/tmp/ME",
            "workspaceCreated": false,
            "preflight": { "canonicalIntegrity": "ok", "derivedState": "ok" },
            "codex": {
                "attemptedOpen": false,
                "deepLink": "codex://new?path=%2Ftmp%2FME&prompt=Start%20ME",
                "prompt": "Start ME",
                "promptSubmitted": false
            },
            "printUrlOnly": true,
            "warnings": []
        });
        let output = structured_success_value("start", &data);
        assert_eq!(output["ok"], true);
        assert_eq!(output["command"], "start");
        assert_eq!(output["warnings"].as_array().unwrap().len(), 0);
        assert!(output["data"].get("printUrlOnly").is_none());
    }
}
