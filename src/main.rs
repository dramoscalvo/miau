use anyhow::{Context, Result, anyhow};
use clap::{Args, Parser, Subcommand};
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
                repository.write(&run.id, &args.artifact, text)?;
                let node = run
                    .current_mut()
                    .ok_or_else(|| anyhow!("debug run lost its node"))?;
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
