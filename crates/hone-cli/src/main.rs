use std::env;
use std::path::PathBuf;
use std::process;

use clap::{Args, Parser, Subcommand};
use hone_store::{HoneError, Workspace};
use serde_json::{json, Value};

#[derive(Debug, Parser)]
#[command(
    name = "hone",
    version,
    about = "Local system for refining thought over time"
)]
struct Cli {
    #[arg(long, global = true)]
    workspace: Option<PathBuf>,

    #[arg(long, global = true)]
    json: bool,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Debug, Subcommand)]
enum Commands {
    New(NewArgs),
    Init(InitArgs),
    Status,
    Doctor(DoctorArgs),
    Codex(CodexArgs),
    Capture(CaptureArgs),
    Relate(RelateArgs),
    Context(ContextArgs),
    Proposal(ProposalArgs),
    Review(ReviewArgs),
    Approve(ApproveArgs),
    Reject(RejectArgs),
    Defer(DeferArgs),
    Facet(FacetArgs),
    Article(ArticleArgs),
    History(HistoryArgs),
    Diff(DiffArgs),
    Snapshot(SnapshotArgs),
    Index(IndexArgs),
    Fsck,
    Bundle(BundleArgs),
    Export(ExportArgs),
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
struct CaptureArgs {
    #[arg(long)]
    file: PathBuf,
    #[arg(long)]
    kind: String,
    #[arg(long)]
    title: Option<String>,
}

#[derive(Debug, Args)]
struct RelateArgs {
    source_id: String,
    #[arg(long, default_value_t = 5)]
    limit: usize,
}

#[derive(Debug, Args)]
struct ContextArgs {
    #[command(subcommand)]
    command: ContextCommand,
}

#[derive(Debug, Subcommand)]
enum ContextCommand {
    Proposal(ContextProposalArgs),
}

#[derive(Debug, Args)]
struct ContextProposalArgs {
    source_id: String,
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
struct FileArg {
    file: PathBuf,
}

#[derive(Debug, Args)]
struct IdArg {
    id: String,
}

#[derive(Debug, Args)]
struct ProposalListArgs {
    #[arg(long)]
    status: Option<String>,
}

#[derive(Debug, Args)]
struct ReviewArgs {
    proposal_id: String,
    #[arg(long, default_value = "text")]
    format: String,
}

#[derive(Debug, Args)]
struct ApproveArgs {
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
struct FacetArgs {
    #[command(subcommand)]
    command: FacetCommand,
}

#[derive(Debug, Subcommand)]
enum FacetCommand {
    List,
    Show(FacetShowArgs),
}

#[derive(Debug, Args)]
struct FacetShowArgs {
    facet_id: String,
    #[arg(long)]
    revision: Option<u64>,
}

#[derive(Debug, Args)]
struct ArticleArgs {
    #[command(subcommand)]
    command: ArticleCommand,
}

#[derive(Debug, Subcommand)]
enum ArticleCommand {
    List,
    Show(ArticleShowArgs),
}

#[derive(Debug, Args)]
struct ArticleShowArgs {
    article_id: String,
    #[arg(long)]
    edition: Option<u64>,
    #[arg(long, default_value = "markdown")]
    format: String,
}

#[derive(Debug, Args)]
struct HistoryArgs {
    #[arg(long)]
    facet: Option<String>,
    #[arg(long)]
    article: Option<String>,
}

#[derive(Debug, Args)]
struct DiffArgs {
    snapshot_a: String,
    snapshot_b: String,
    #[arg(long, default_value = "text")]
    format: String,
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
    message: String,
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
    Article(ExportArticleArgs),
    Workspace(ExportWorkspaceArgs),
}

#[derive(Debug, Args)]
struct ExportArticleArgs {
    article_id: String,
    #[arg(long, default_value = "markdown")]
    format: String,
    #[arg(long)]
    output: PathBuf,
}

#[derive(Debug, Args)]
struct ExportWorkspaceArgs {
    #[arg(long, default_value = "json")]
    format: String,
    #[arg(long)]
    output: PathBuf,
}

fn main() {
    let cli = Cli::parse();
    let (command, result) = dispatch(&cli);
    match result {
        Ok(data) => {
            print_success(command, &data, cli.json);
        }
        Err(err) => {
            print_error(command, &err, cli.json);
            process::exit(err.exit_code());
        }
    }
}

fn dispatch(cli: &Cli) -> (&'static str, Result<Value, HoneError>) {
    match &cli.command {
        Commands::New(args) => ("new", Workspace::new_workspace(&args.path, args.demo)),
        Commands::Init(args) => {
            let path = args
                .path
                .clone()
                .or_else(|| cli.workspace.clone())
                .unwrap_or_else(|| env::current_dir().expect("current directory"));
            ("init", Workspace::init(path, args.demo))
        }
        Commands::Status => with_workspace(cli, "status", |ws| ws.status()),
        Commands::Doctor(args) => with_workspace(cli, "doctor", |ws| ws.doctor(args.repair)),
        Commands::Codex(args) => match args.command {
            CodexCommand::Sync => with_workspace(cli, "codex sync", |ws| ws.codex_sync()),
        },
        Commands::Capture(args) => with_workspace(cli, "capture", |ws| {
            ws.capture(&args.file, &args.kind, args.title.clone())
        }),
        Commands::Relate(args) => {
            with_workspace(cli, "relate", |ws| ws.relate(&args.source_id, args.limit))
        }
        Commands::Context(args) => match &args.command {
            ContextCommand::Proposal(inner) => with_workspace(cli, "context proposal", |ws| {
                ws.proposal_context(&inner.source_id, inner.limit)
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
            ws.review(&args.proposal_id, &args.format)
        }),
        Commands::Approve(args) => with_workspace(cli, "approve", |ws| {
            ws.approve(&args.proposal_id, &args.decision)
        }),
        Commands::Reject(args) => with_workspace(cli, "reject", |ws| {
            ws.reject_or_defer(&args.proposal_id, "reject", args.note.clone())
        }),
        Commands::Defer(args) => with_workspace(cli, "defer", |ws| {
            ws.reject_or_defer(&args.proposal_id, "defer", args.note.clone())
        }),
        Commands::Facet(args) => match &args.command {
            FacetCommand::List => with_workspace(cli, "facet list", |ws| ws.facet_list()),
            FacetCommand::Show(inner) => with_workspace(cli, "facet show", |ws| {
                ws.facet_show(&inner.facet_id, inner.revision)
            }),
        },
        Commands::Article(args) => match &args.command {
            ArticleCommand::List => with_workspace(cli, "article list", |ws| ws.article_list()),
            ArticleCommand::Show(inner) => with_workspace(cli, "article show", |ws| {
                ws.article_show(&inner.article_id, inner.edition, &inner.format)
            }),
        },
        Commands::History(args) => with_workspace(cli, "history", |ws| {
            ws.history(args.facet.clone(), args.article.clone())
        }),
        Commands::Diff(args) => with_workspace(cli, "diff", |ws| {
            ws.diff(&args.snapshot_a, &args.snapshot_b, &args.format)
        }),
        Commands::Snapshot(args) => match &args.command {
            SnapshotCommand::List => with_workspace(cli, "snapshot list", |ws| ws.snapshot_list()),
            SnapshotCommand::Show(inner) => {
                with_workspace(cli, "snapshot show", |ws| ws.snapshot_show(&inner.id))
            }
            SnapshotCommand::Restore(inner) => with_workspace(cli, "snapshot restore", |ws| {
                ws.snapshot_restore(&inner.snapshot_id, &inner.message)
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
                with_workspace(cli, "bundle verify", |ws| ws.bundle_verify(&inner.file))
            }
            BundleCommand::Restore(inner) => (
                "bundle restore",
                Workspace::bundle_restore(&inner.file, &inner.target_directory),
            ),
        },
        Commands::Export(args) => match &args.command {
            ExportCommand::Article(inner) => with_workspace(cli, "export article", |ws| {
                ws.export_article(&inner.article_id, &inner.format, &inner.output)
            }),
            ExportCommand::Workspace(inner) => {
                if inner.format != "json" {
                    return (
                        "export workspace",
                        Err(HoneError::InvalidInput {
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
    }
}

fn with_workspace(
    cli: &Cli,
    command: &'static str,
    f: impl FnOnce(&Workspace) -> Result<Value, HoneError>,
) -> (&'static str, Result<Value, HoneError>) {
    let root = cli
        .workspace
        .clone()
        .unwrap_or_else(|| env::current_dir().expect("current directory"));
    let result = Workspace::open(root).and_then(|ws| f(&ws));
    (command, result)
}

fn print_success(command: &str, data: &Value, json_output: bool) {
    if json_output {
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

fn print_error(command: &str, err: &HoneError, json_output: bool) {
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
