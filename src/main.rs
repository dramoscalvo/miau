use anyhow::{Context, Result, anyhow};
use clap::{Args, Parser, Subcommand, ValueEnum};
use miau::{
    execution::{
        application::Request,
        domain::EventKind,
        infrastructure::{ConfiguredAdapter, spawn},
    },
    runs::{
        application::{ArtifactRepository, RunRepository},
        domain::{ChannelEntry, Node, NodeStatus, Run},
        infrastructure::FileRepository,
    },
    workflow::infrastructure::{ensure_config, load_agents},
};
use std::{
    fs,
    io::{self, Write},
    path::PathBuf,
    process::Command,
    sync::Mutex,
};

#[derive(Parser)]
#[command(
    name = "miau",
    version,
    about = "Human-gated coding-agent orchestrator"
)]
struct Cli {
    #[arg(long, default_value = "miaus")]
    runs: PathBuf,
    #[arg(long)]
    agents: Option<PathBuf>,
    #[arg(long)]
    workflow: Option<PathBuf>,
    #[arg(long)]
    roles: Option<PathBuf>,
    #[command(subcommand)]
    command: Option<Commands>,
}
#[derive(Subcommand)]
enum Commands {
    Debug(DebugArgs),
    /// Generate architecture diagrams from source, without an agent.
    Diagram(DiagramArgs),
}
#[derive(Args)]
struct DiagramArgs {
    #[command(subcommand)]
    command: DiagramCommands,
}
#[derive(Subcommand)]
enum DiagramCommands {
    /// Extract TypeScript module or semantic type relationships using the project's compiler.
    Typescript(TypeScriptArgs),
}
#[derive(Args)]
struct TypeScriptArgs {
    /// Path to the leaf tsconfig.json to analyze.
    #[arg(long, default_value = "tsconfig.json")]
    project: PathBuf,
    /// Project root used for source links and portable node IDs.
    #[arg(long, default_value = ".")]
    root: PathBuf,
    /// Compiler-derived graph scope.
    #[arg(long, value_enum, default_value_t = TypeScriptScope::Modules)]
    scope: TypeScriptScope,
    /// Include only new or edited TypeScript files and their directly related types.
    #[arg(long)]
    changed: bool,
    #[arg(long)]
    title: Option<String>,
    /// Write a new Markdown artifact; existing files are never overwritten. Defaults to stdout.
    #[arg(long)]
    output: Option<PathBuf>,
    /// Node.js executable (Node 18 or newer).
    #[arg(long, default_value = "node")]
    node: std::ffi::OsString,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
enum TypeScriptScope {
    Modules,
    Types,
}
#[derive(Args)]
struct DebugArgs {
    #[command(subcommand)]
    command: DebugCommands,
}
#[derive(Subcommand)]
enum DebugCommands {
    Run(DebugRun),
}
#[derive(Args)]
struct DebugRun {
    #[arg(long)]
    agent: String,
    #[arg(long)]
    prompt: String,
    #[arg(long, default_value = "debug")]
    id: String,
    #[arg(long, default_value = ".")]
    cwd: PathBuf,
    #[arg(long, default_value = "output.md")]
    artifact: String,
    #[arg(long)]
    schema: Option<PathBuf>,
}

#[tokio::main]
async fn main() -> Result<()> {
    install_panic_hook();
    let cli = Cli::parse();
    match cli.command {
        Some(Commands::Diagram(args)) => match args.command {
            DiagramCommands::Typescript(args) => generate_typescript(args),
        },
        Some(Commands::Debug(args)) => match args.command {
            DebugCommands::Run(request) => {
                let agents = match cli.agents {
                    Some(path) => path,
                    None => ensure_config()?.agents,
                };
                debug_run(&cli.runs, &agents, request).await
            }
        },
        None => {
            let defaults = ensure_config()?;
            miau::terminal::infrastructure::run(miau::terminal::infrastructure::UiConfig {
                runs: cli.runs,
                agents: cli.agents.unwrap_or(defaults.agents),
                workflow: cli.workflow.unwrap_or(defaults.workflow),
                roles: cli.roles.unwrap_or(defaults.roles),
            })
            .await
        }
    }
}

fn generate_typescript(args: TypeScriptArgs) -> Result<()> {
    use miau::runs::{
        application::generate_diagram::{ExtractionRequest, ExtractionScope, generate},
        infrastructure::typescript::TypeScriptExtractor,
    };
    let scope = match args.scope {
        TypeScriptScope::Modules => ExtractionScope::Modules,
        TypeScriptScope::Types => ExtractionScope::Types,
    };
    if args.changed && scope != ExtractionScope::Types {
        return Err(anyhow!("--changed is supported only with --scope types"));
    }
    let changed_files = if args.changed {
        let Some(files) = changed_typescript_files(&args.root)? else {
            return write_diagram_artifact(
                "# Review\nThe project is not a Git worktree. Changed-file extraction is not applicable.\n\n\
                 # Handoff\nNo diagram was generated because the changed files could not be determined.\n",
                args.output,
            );
        };
        if files.is_empty() {
            let artifact = "# Review\nNo new or edited TypeScript files were found. No impacted UML graph applies.\n\n# Handoff\nThe deterministic extractor found no changed TypeScript files to diagram.\n";
            return write_diagram_artifact(artifact, args.output);
        }
        Some(files)
    } else {
        None
    };
    let title = args.title.unwrap_or_else(|| match (scope, args.changed) {
        (ExtractionScope::Modules, _) => "TypeScript module dependencies".into(),
        (ExtractionScope::Types, true) => "Impacted TypeScript type relationships".into(),
        (ExtractionScope::Types, false) => "TypeScript type relationships".into(),
    });
    let artifact = generate(
        &TypeScriptExtractor { node: args.node },
        &ExtractionRequest {
            project: &args.project,
            root: &args.root,
            title: &title,
            scope,
            changed_files: changed_files.as_deref(),
        },
    )?;
    write_diagram_artifact(&artifact, args.output)
}

fn write_diagram_artifact(artifact: &str, output: Option<PathBuf>) -> Result<()> {
    match output {
        Some(path) => {
            let mut file = fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&path)
                .with_context(|| format!("cannot create new artifact {}", path.display()))?;
            file.write_all(artifact.as_bytes())?;
        }
        None => io::stdout().lock().write_all(artifact.as_bytes())?,
    }
    Ok(())
}

fn changed_typescript_files(root: &std::path::Path) -> Result<Option<Vec<String>>> {
    let root = root.canonicalize()?;
    let repository = Command::new("git")
        .args(["rev-parse", "--show-toplevel"])
        .env("LC_ALL", "C")
        .current_dir(&root)
        .output()
        .context("cannot inspect changed files; --changed requires Git")?;
    if !repository.status.success() {
        let error = String::from_utf8_lossy(&repository.stderr);
        if error.starts_with("fatal: not a git repository") {
            return Ok(None);
        }
        return Err(anyhow!("cannot locate Git worktree: {}", error.trim()));
    }
    let repository_text = std::str::from_utf8(&repository.stdout)?;
    let repository_root = std::path::Path::new(
        repository_text
            .strip_suffix('\n')
            .unwrap_or(repository_text),
    )
    .canonicalize()?;
    let output = Command::new("git")
        .args(["status", "--porcelain=v1", "-z", "--untracked-files=all"])
        .arg("--")
        .arg(&root)
        .current_dir(&repository_root)
        .output()
        .context("cannot inspect changed files; --changed requires Git")?;
    if !output.status.success() {
        return Err(anyhow!(
            "cannot inspect changed files: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }
    let mut files = std::collections::BTreeSet::new();
    let mut records = output.stdout.split(|byte| *byte == 0);
    while let Some(record) = records.next() {
        if record.len() < 4 || record[2] != b' ' {
            continue;
        }
        let status = &record[..2];
        // Porcelain -z emits the destination first, then the original path without a status prefix.
        if status.contains(&b'R') || status.contains(&b'C') {
            records.next();
        }
        if status.contains(&b'D') || status == b"!!" {
            continue;
        }
        let repository_path = repository_root.join(std::str::from_utf8(&record[3..])?);
        let Ok(project_path) = repository_path.strip_prefix(&root) else {
            continue;
        };
        let file = project_path
            .to_str()
            .context("changed source path is not UTF-8")?;
        if matches!(
            std::path::Path::new(file)
                .extension()
                .and_then(|ext| ext.to_str()),
            Some("ts" | "tsx" | "mts" | "cts")
        ) {
            files.insert(file.replace(std::path::MAIN_SEPARATOR, "/"));
        }
    }
    Ok(Some(files.into_iter().collect()))
}

async fn debug_run(
    runs_root: &std::path::Path,
    agents_path: &std::path::Path,
    args: DebugRun,
) -> Result<()> {
    let repository = FileRepository::new(runs_root);
    init_logger(&repository.run_dir(&args.id).join("miau.log"))?;
    let configs = load_agents(agents_path)?;
    let config = configs
        .get(&args.agent)
        .cloned()
        .ok_or_else(|| anyhow!("agent '{}' is not configured", args.agent))?;
    let adapter = ConfiguredAdapter::new(&args.agent, config)?;
    let node = Node {
        name: "debug".into(),
        agent: args.agent.clone(),
        role: "debug".into(),
        writes: args.artifact.clone(),
        status: NodeStatus::Running,
        session_id: None,
        session_group: None,
        duration: None,
        attempts: 1,
        command: None,
        skip_if_no_python_changes: false,
    };
    let mut run = Run {
        id: args.id.clone(),
        project: args.cwd.clone(),
        spec: None,
        nodes: vec![node],
        cursor: 0,
        channel: vec![ChannelEntry::new(
            "you",
            Some(args.agent.clone()),
            "debug run started",
        )],
        created_at: chrono::Utc::now(),
    };
    repository.save(&run)?;
    let log = repository.run_dir(&args.id).join("events/debug.jsonl");
    let mut running = spawn(
        adapter,
        Request {
            prompt: args.prompt,
            cwd: args.cwd,
            resume: None,
            schema: args.schema,
        },
        &log,
    )
    .await?;
    let mut output = io::stdout().lock();
    while let Some(event) = running.events.recv().await {
        serde_json::to_writer(&mut output, &event)?;
        writeln!(&mut output)?;
        match &event.kind {
            EventKind::SessionStarted { id } => {
                run.current_mut()
                    .ok_or_else(|| anyhow!("debug run lost its node"))?
                    .session_id = Some(id.clone());
            }
            EventKind::Done { text, session_id } => {
                let artifact = repository.write_versioned(&run.id, &args.artifact, text)?;
                let node = run
                    .current_mut()
                    .ok_or_else(|| anyhow!("debug run lost its node"))?;
                node.writes = artifact;
                node.status = NodeStatus::Done;
                if session_id.is_some() {
                    node.session_id.clone_from(session_id);
                }
            }
            EventKind::Failed { .. } => {
                run.current_mut()
                    .ok_or_else(|| anyhow!("debug run lost its node"))?
                    .status = NodeStatus::Failed;
            }
            _ => {}
        }
        repository.save(&run)?;
    }
    Ok(())
}

fn init_logger(path: &std::path::Path) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let file =
        fs::File::create(path).with_context(|| format!("cannot create {}", path.display()))?;
    tracing_subscriber::fmt()
        .with_ansi(false)
        .with_writer(Mutex::new(file))
        .try_init()
        .map_err(|e| anyhow!(e.to_string()))
}

fn install_panic_hook() {
    let original = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        miau::terminal::infrastructure::handoff::restore_stdio();
        original(info);
    }));
}
