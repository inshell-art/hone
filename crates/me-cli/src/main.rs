use std::env;
use std::path::PathBuf;
use std::process;

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

    #[command(subcommand)]
    command: Commands,
}

#[derive(Debug, Subcommand)]
enum Commands {
    Version,
    New(NewArgs),
    Init(InitArgs),
    Home,
    Guide,
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
    file: PathBuf,
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
    decision: PathBuf,
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
    task: PathBuf,
    #[arg(long, default_value_t = 20)]
    limit: usize,
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
        Ok(data) => print_success(command, &data, cli.json || cli.markdown),
        Err(err) => {
            print_error(command, &err, cli.json);
            process::exit(err.exit_code());
        }
    }
}

fn dispatch(cli: &Cli) -> (&'static str, Result<Value, MeError>) {
    match &cli.command {
        Commands::Version => ("version", Ok(version_info())),
        Commands::New(args) => ("new", Workspace::new_workspace(&args.path, args.demo)),
        Commands::Init(args) => {
            let path = args
                .path
                .clone()
                .or_else(|| cli.workspace.clone())
                .unwrap_or_else(|| env::current_dir().expect("current directory"));
            ("init", Workspace::init(path, args.demo))
        }
        Commands::Home => with_workspace(cli, "home", |ws| {
            ws.home(if cli.markdown { "markdown" } else { "json" })
        }),
        Commands::Guide => with_workspace(cli, "guide", |ws| ws.guide()),
        Commands::Status => with_workspace(cli, "status", |ws| ws.status()),
        Commands::Current => with_workspace(cli, "current", |ws| ws.current()),
        Commands::Doctor(args) => with_workspace(cli, "doctor", |ws| ws.doctor(args.repair)),
        Commands::Codex(args) => match args.command {
            CodexCommand::Sync => with_workspace(cli, "codex sync", |ws| ws.codex_sync()),
        },
        Commands::Thought(args) => match &args.command {
            ThoughtCommand::Capture(inner) => with_workspace(cli, "thought capture", |ws| {
                ws.thought_capture(&inner.file, &inner.kind)
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
                ws.cognition_add(&inner.thought, &inner.decision)
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
        Commands::Context(args) => {
            with_workspace(cli, "context", |ws| ws.context(&args.task, args.limit))
        }
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
    }
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

fn print_success(command: &str, data: &Value, structured_output: bool) {
    if structured_output {
        println!(
            "{}",
            serde_json::to_string_pretty(&json!({
                "ok": true,
                "command": command,
                "data": data,
                "warnings": []
            }))
            .expect("json")
        );
        return;
    }

    if let Some(markdown) = data.get("markdown").and_then(Value::as_str) {
        print!("{markdown}");
    } else if let Some(review) = data.get("review").and_then(Value::as_str) {
        print!("{review}");
    } else if let Some(text) = data.get("text").and_then(Value::as_str) {
        println!("{text}");
    } else {
        println!("{}", serde_json::to_string_pretty(data).expect("json"));
    }
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
    } else {
        eprintln!("error[{}]: {}", err.code(), err);
    }
}
