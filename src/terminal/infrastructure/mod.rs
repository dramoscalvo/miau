//! Ratatui and crossterm adapters.

pub mod channel;
pub mod flow;
pub mod handoff;
pub mod help;
pub mod layout;
mod pane;

use crate::{
    execution::{
        application::{AgentConfig, Request},
        domain::{Event, EventKind},
        infrastructure::{ConfiguredAdapter, RunningAgent, ShellAdapter, spawn},
    },
    runs::{
        application::{ArtifactRepository, RunRepository},
        domain::{Decision, Node, NodeStatus, Run},
        infrastructure::FileRepository,
    },
    terminal::application::{
        Action, Message, Mode, Model, PromptEditMode, PromptKind, PromptOperator,
        PromptPendingCommand, PromptTextObject, PromptWordStyle, SubmittedPrompt,
    },
    workflow::{
        application::Orchestrator,
        domain::WorkflowConfig,
        infrastructure::{GitChangeDetector, load_agents, load_workflow},
    },
};
use anyhow::{Result, anyhow};
use chrono::Utc;
use crossterm::{
    event::{EnableMouseCapture, Event as TerminalEvent, EventStream, KeyCode, KeyEventKind},
    execute,
    terminal::{EnterAlternateScreen, enable_raw_mode},
};
use futures_util::StreamExt;
use ratatui::{Terminal, backend::CrosstermBackend};
use std::{
    collections::HashMap,
    io,
    path::PathBuf,
    process::Command,
    time::{Duration, Instant},
};

#[derive(Debug, Clone)]
pub struct UiConfig {
    pub runs: PathBuf,
    pub agents: PathBuf,
    pub workflow: PathBuf,
    pub roles: PathBuf,
}
struct App {
    ui: Model,
    runs: Vec<Run>,
    run: Option<Run>,
    artifact: String,
    stream: Vec<String>,
    agent_activity: String,
    running: Option<RunningAgent>,
    started: Option<Instant>,
    agents: HashMap<String, AgentConfig>,
    workflow: WorkflowConfig,
    repository: FileRepository,
    orchestrator: Orchestrator<FileRepository>,
}

fn returns_to_run_list(code: KeyCode, mode: &Mode) -> bool {
    code == KeyCode::Char('b') && matches!(mode, Mode::Gate)
}

fn prompt_kind(run: Option<&Run>, viewed_node: usize) -> Option<PromptKind> {
    let run = run?;
    let node = run.nodes.get(viewed_node)?;
    if node.command.is_some() {
        return None;
    }
    match node.status {
        NodeStatus::Pending if viewed_node == run.cursor => Some(PromptKind::Initial),
        NodeStatus::Done | NodeStatus::Failed => Some(PromptKind::Revision),
        NodeStatus::Pending | NodeStatus::Running | NodeStatus::Skipped => None,
    }
}

fn handle_normal_prompt_key(ui: &mut Model, code: KeyCode, prompt_width: usize) {
    if let Some(pending) = ui.take_prompt_pending_command() {
        match (pending, code) {
            (PromptPendingCommand::Goto, KeyCode::Char('g')) => {
                ui.update(Message::MovePromptToStart);
            }
            (PromptPendingCommand::Operator(PromptOperator::Delete), KeyCode::Char('d')) => ui
                .update(Message::EditPromptLine {
                    operator: PromptOperator::Delete,
                }),
            (PromptPendingCommand::Operator(PromptOperator::Yank), KeyCode::Char('y')) => ui
                .update(Message::EditPromptLine {
                    operator: PromptOperator::Yank,
                }),
            (PromptPendingCommand::Operator(operator), KeyCode::Char('i')) => {
                ui.begin_prompt_text_object(operator, PromptTextObject::Inner);
            }
            (PromptPendingCommand::Operator(operator), KeyCode::Char('a')) => {
                ui.begin_prompt_text_object(operator, PromptTextObject::Around);
            }
            (
                PromptPendingCommand::TextObject {
                    operator,
                    text_object,
                },
                KeyCode::Char('w'),
            ) => ui.update(Message::EditPromptWord {
                operator,
                text_object,
                style: PromptWordStyle::Word,
            }),
            (
                PromptPendingCommand::TextObject {
                    operator,
                    text_object,
                },
                KeyCode::Char('W'),
            ) => ui.update(Message::EditPromptWord {
                operator,
                text_object,
                style: PromptWordStyle::BigWord,
            }),
            _ => {}
        }
        return;
    }

    match code {
        KeyCode::Esc => ui.cancel_prompt(),
        KeyCode::Left | KeyCode::Char('h') => ui.update(Message::MovePromptLeft),
        KeyCode::Right | KeyCode::Char('l') => ui.update(Message::MovePromptRight),
        KeyCode::Up | KeyCode::Char('k') => ui.update(Message::MovePromptUp {
            width: prompt_width,
        }),
        KeyCode::Down | KeyCode::Char('j') => ui.update(Message::MovePromptDown {
            width: prompt_width,
        }),
        KeyCode::Char('i') => ui.update(Message::EnterPromptInsertMode),
        KeyCode::Char('a') => {
            ui.update(Message::MovePromptRight);
            ui.update(Message::EnterPromptInsertMode);
        }
        KeyCode::Char('I') => {
            ui.update(Message::MovePromptToFirstNonBlank);
            ui.update(Message::EnterPromptInsertMode);
        }
        KeyCode::Char('A') => {
            ui.update(Message::MovePromptToEnd);
            ui.update(Message::EnterPromptInsertMode);
        }
        KeyCode::Char('o') => ui.update(Message::OpenPromptLineBelow),
        KeyCode::Char('O') => ui.update(Message::OpenPromptLineAbove),
        KeyCode::Char('x') | KeyCode::Delete => ui.update(Message::DeletePromptCharacter),
        KeyCode::Char('0') | KeyCode::Home => ui.update(Message::MovePromptToStart),
        KeyCode::Char('$') | KeyCode::End => ui.update(Message::MovePromptToEnd),
        KeyCode::Char('^') => ui.update(Message::MovePromptToFirstNonBlank),
        KeyCode::Char('w') => ui.update(Message::MovePromptWordForward {
            style: PromptWordStyle::Word,
        }),
        KeyCode::Char('W') => ui.update(Message::MovePromptWordForward {
            style: PromptWordStyle::BigWord,
        }),
        KeyCode::Char('b') => ui.update(Message::MovePromptWordBackward {
            style: PromptWordStyle::Word,
        }),
        KeyCode::Char('B') => ui.update(Message::MovePromptWordBackward {
            style: PromptWordStyle::BigWord,
        }),
        KeyCode::Char('e') => ui.update(Message::MovePromptWordEnd {
            style: PromptWordStyle::Word,
        }),
        KeyCode::Char('E') => ui.update(Message::MovePromptWordEnd {
            style: PromptWordStyle::BigWord,
        }),
        KeyCode::Char('g') => ui.begin_prompt_g_prefix(),
        KeyCode::Char('G') => ui.update(Message::MovePromptToEnd),
        KeyCode::Char('d') => ui.begin_prompt_operator(PromptOperator::Delete),
        KeyCode::Char('D') => ui.update(Message::DeletePromptToLineEnd),
        KeyCode::Char('c') => ui.begin_prompt_operator(PromptOperator::Change),
        KeyCode::Char('y') => ui.begin_prompt_operator(PromptOperator::Yank),
        KeyCode::Char('Y') => ui.update(Message::EditPromptLine {
            operator: PromptOperator::Yank,
        }),
        KeyCode::Char('p') => ui.update(Message::PastePromptAfter),
        _ => {}
    }
}

fn activity_for_event(kind: &EventKind) -> String {
    match kind {
        EventKind::Progress(message) => message.clone(),
        EventKind::Text(_) => "writing a response".into(),
        EventKind::Reasoning(_) => "thinking".into(),
        EventKind::ToolCall { name, .. } => format!("using {name}"),
        EventKind::FileChange { path, .. } => format!("changing {}", path.display()),
        EventKind::SessionStarted { .. } => "session started".into(),
        EventKind::Done { .. } => "waiting for your decision".into(),
        EventKind::Failed { .. } => "needs your attention".into(),
    }
}

impl App {
    fn load(config: UiConfig) -> Result<Self> {
        let repository = FileRepository::new(config.runs);
        let runs = repository.list()?;
        Ok(Self {
            ui: Model::default(),
            runs,
            run: None,
            artifact: String::new(),
            stream: vec![],
            agent_activity: String::new(),
            running: None,
            started: None,
            agents: load_agents(&config.agents)?,
            workflow: load_workflow(&config.workflow)?,
            orchestrator: Orchestrator::new(repository.clone(), config.roles),
            repository,
        })
    }

    fn create_run(&mut self) -> Result<()> {
        let next = self
            .runs
            .iter()
            .filter_map(|r| r.id.parse::<u32>().ok())
            .max()
            .unwrap_or(0)
            + 1;
        let nodes = self
            .workflow
            .nodes
            .iter()
            .map(|n| Node {
                name: n.name.clone(),
                agent: n.agent.clone().unwrap_or_else(|| "shell".into()),
                role: n.role.clone().unwrap_or_else(|| "command".into()),
                writes: n.writes.clone().unwrap_or_else(|| format!("{}.md", n.name)),
                status: NodeStatus::Pending,
                session_id: None,
                session_group: n.session_group.clone(),
                duration: None,
                attempts: 0,
                command: n.command.clone(),
                skip_if_no_python_changes: n.skip_if_no_python_changes,
            })
            .collect();
        let run = Run {
            id: format!("{next:03}"),
            project: std::env::current_dir()?,
            spec: None,
            nodes,
            cursor: 0,
            channel: vec![],
            created_at: Utc::now(),
        };
        self.repository.save(&run)?;
        self.runs.push(run);
        self.ui.selected = self.runs.len().saturating_sub(1);
        self.open_selected()
    }

    fn open_selected(&mut self) -> Result<()> {
        let Some(run) = self.runs.get(self.ui.selected) else {
            return Ok(());
        };
        let mut loaded = self.repository.load(&run.id)?;
        if let Some(node) = loaded.current_mut()
            && node.status == NodeStatus::Running
        {
            node.status = NodeStatus::Failed;
            loaded.channel.push(crate::runs::domain::ChannelEntry::new(
                "miau",
                Some("you".into()),
                "interrupted process detected on resume",
            ));
            self.repository.save(&loaded)?;
        }
        self.run = Some(loaded);
        self.ui.viewed_node = self
            .run
            .as_ref()
            .map_or(0, |run| run.cursor.min(run.nodes.len().saturating_sub(1)));
        self.reload_artifact()?;
        self.ui.mode = self
            .run
            .as_ref()
            .and_then(Run::current)
            .map_or(Mode::Gate, |n| match n.status {
                NodeStatus::Running => Mode::Gate,
                NodeStatus::Done | NodeStatus::Failed => Mode::Gate,
                _ => Mode::Gate,
            });
        Ok(())
    }

    fn reload_artifact(&mut self) -> Result<()> {
        self.artifact = match self.run.as_ref() {
            Some(run) => match run.nodes.get(self.ui.viewed_node) {
                Some(node) => self.repository.read(&run.id, &node.writes)?,
                None => String::new(),
            },
            None => String::new(),
        };
        Ok(())
    }

    fn return_to_run_list(&mut self) {
        self.ui.return_to_run_list();
        self.run = None;
        self.artifact.clear();
        self.stream.clear();
        self.agent_activity.clear();
    }

    async fn start_current(&mut self) -> Result<()> {
        let pending_prompt = self.ui.pending_prompt.clone();
        let run = self
            .run
            .as_mut()
            .ok_or_else(|| anyhow!("no run selected"))?;
        let node = loop {
            let candidate = run
                .current()
                .cloned()
                .ok_or_else(|| anyhow!("workflow complete"))?;
            let should_skip = candidate.command.is_some()
                && candidate.skip_if_no_python_changes
                && matches!(
                    GitChangeDetector::has_python_changes(&run.project),
                    Ok(false)
                );
            if !should_skip {
                break candidate;
            }
            if let Some(current) = run.current_mut() {
                current.status = NodeStatus::Skipped;
            }
            run.channel.push(crate::runs::domain::ChannelEntry::new(
                "miau",
                Some("you".into()),
                format!("skipped {}: no Python changes", candidate.name),
            ));
            run.cursor += 1;
            self.repository.save(run)?;
        };
        let resume = self
            .orchestrator
            .session_for_current(run)
            .map(str::to_owned);
        self.orchestrator.begin(run)?;
        self.ui.viewed_node = run.cursor;
        let request = Request {
            prompt: if node.command.is_none() {
                self.orchestrator
                    .assemble_prompt(run, pending_prompt.as_deref())?
            } else {
                String::new()
            },
            cwd: run.project.clone(),
            resume,
            schema: None,
        };
        let raw_log = self
            .repository
            .run_dir(&run.id)
            .join("events")
            .join(format!("{}.jsonl", node.name));
        self.stream.clear();
        self.agent_activity = "starting".into();
        self.ui.error = None;
        self.started = Some(Instant::now());
        self.running = if let Some(command) = node.command {
            Some(spawn(ShellAdapter::new(&node.name, command), request, &raw_log).await?)
        } else {
            let config = self
                .agents
                .get(&node.agent)
                .cloned()
                .ok_or_else(|| anyhow!("agent '{}' is not configured", node.agent))?;
            Some(
                spawn(
                    ConfiguredAdapter::new(&node.agent, config)?,
                    request,
                    &raw_log,
                )
                .await?,
            )
        };
        self.ui.pending_prompt = None;
        self.ui.mode = Mode::Streaming;
        Ok(())
    }

    fn accept_event(&mut self, event: Event) -> Result<()> {
        self.agent_activity = activity_for_event(&event.kind);
        match &event.kind {
            EventKind::Progress(_) => {}
            EventKind::Text(text) | EventKind::Reasoning(text) => {
                self.stream.extend(text.lines().map(str::to_owned))
            }
            EventKind::ToolCall { name, summary } => self.stream.push(format!("{name}: {summary}")),
            EventKind::FileChange { path, kind } => {
                self.stream.push(format!("{kind:?}: {}", path.display()))
            }
            EventKind::Failed { message } => self.ui.error = Some(message.clone()),
            _ => {}
        }
        if matches!(
            event.kind,
            EventKind::SessionStarted { .. } | EventKind::Done { .. } | EventKind::Failed { .. }
        ) && let Some(run) = self.run.as_mut()
        {
            self.orchestrator.apply_event(
                run,
                &event,
                self.started.map_or(Duration::ZERO, |at| at.elapsed()),
            )?;
        }
        if matches!(
            event.kind,
            EventKind::Done { .. } | EventKind::Failed { .. }
        ) {
            self.ui.mode = Mode::Gate;
            self.reload_artifact()?;
        }
        Ok(())
    }

    fn stream_closed(&mut self) -> Result<()> {
        if matches!(self.ui.mode, Mode::Streaming) {
            let still_running = self
                .run
                .as_ref()
                .and_then(Run::current)
                .is_some_and(|n| n.status == NodeStatus::Running);
            if still_running {
                let text = self.stream.join("\n");
                let agent = self
                    .run
                    .as_ref()
                    .and_then(Run::current)
                    .map(|n| n.agent.clone())
                    .unwrap_or_else(|| "agent".into());
                self.accept_event(Event::now(
                    agent,
                    EventKind::Done {
                        text,
                        session_id: None,
                    },
                ))?;
            }
        }
        self.running = None;
        Ok(())
    }

    async fn key(
        &mut self,
        code: KeyCode,
        terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    ) -> Result<bool> {
        match &self.ui.mode {
            Mode::Confirm(action) => {
                let action = *action;
                return match code {
                    KeyCode::Char('y') => {
                        self.stop_running()?;
                        Ok(action == Action::Quit)
                    }
                    KeyCode::Char('n') | KeyCode::Esc => {
                        self.ui.mode = Mode::Streaming;
                        Ok(false)
                    }
                    _ => Ok(false),
                };
            }
            Mode::Prompt(_) => {
                let prompt_width = terminal.size()?.width.saturating_sub(2).max(1) as usize;
                if code == KeyCode::Enter {
                    if let Some(SubmittedPrompt::Revision(note)) = self.ui.submit_prompt() {
                        if let Some(run) = self.run.as_mut() {
                            self.ui.pending_prompt = if self.ui.viewed_node == run.cursor {
                                self.orchestrator.decide(run, Decision::Revise { note })?
                            } else {
                                Some(self.orchestrator.revisit(run, self.ui.viewed_node, note)?)
                            };
                        }
                        self.start_current().await?;
                    }
                    return Ok(false);
                }

                if self.ui.prompt_edit_mode == PromptEditMode::Normal {
                    handle_normal_prompt_key(&mut self.ui, code, prompt_width);
                    return Ok(false);
                }

                match code {
                    KeyCode::Esc => self.ui.update(Message::LeavePromptInsertMode),
                    KeyCode::Backspace => self.ui.update(Message::Backspace),
                    KeyCode::Delete => self.ui.update(Message::DeletePromptCharacter),
                    KeyCode::Left => self.ui.update(Message::MovePromptLeft),
                    KeyCode::Right => self.ui.update(Message::MovePromptRight),
                    KeyCode::Up => self.ui.update(Message::MovePromptUp {
                        width: prompt_width,
                    }),
                    KeyCode::Down => self.ui.update(Message::MovePromptDown {
                        width: prompt_width,
                    }),
                    KeyCode::Char(c) => self.ui.update(Message::Input(c)),
                    _ => {}
                }
                return Ok(false);
            }
            _ => {}
        }
        if code == KeyCode::Char('q') {
            if matches!(self.ui.mode, Mode::Streaming) {
                self.ui.mode = Mode::Confirm(Action::Quit);
                return Ok(false);
            }
            return Ok(true);
        }
        if code == KeyCode::Char('x') && matches!(self.ui.mode, Mode::Streaming) {
            self.ui.mode = Mode::Confirm(Action::Stop);
            return Ok(false);
        }
        if matches!(self.ui.mode, Mode::RunList) {
            match code {
                KeyCode::Char('n') => self.create_run()?,
                KeyCode::Enter => self.open_selected()?,
                KeyCode::Up => self.ui.update(Message::SelectPrevious),
                KeyCode::Down => self.ui.update(Message::SelectNext {
                    last: self.runs.len().saturating_sub(1),
                }),
                _ => {}
            }
            return Ok(false);
        }
        match code {
            _ if returns_to_run_list(code, &self.ui.mode) => {
                self.return_to_run_list();
            }
            KeyCode::Left => {
                self.ui.update(Message::ViewPreviousNode);
                self.reload_artifact()?;
            }
            KeyCode::Right => {
                let last = self
                    .run
                    .as_ref()
                    .map_or(0, |run| run.nodes.len().saturating_sub(1));
                self.ui.update(Message::ViewNextNode { last });
                self.reload_artifact()?;
            }
            KeyCode::Tab => self.ui.update(Message::ToggleFocus),
            KeyCode::Up => self.scroll(-1),
            KeyCode::Down => self.scroll(1),
            KeyCode::PageUp => self.scroll(-10),
            KeyCode::PageDown => self.scroll(10),
            KeyCode::Char('s')
                if matches!(self.ui.mode, Mode::Gate)
                    && self.run.as_ref().is_some_and(|run| {
                        run.cursor == self.ui.viewed_node
                            && run
                                .current()
                                .is_some_and(|node| node.status == NodeStatus::Pending)
                    }) =>
            {
                self.start_current().await?
            }
            KeyCode::Char('a') if self.gate_ready() => {
                if let Some(run) = self.run.as_mut() {
                    self.orchestrator.decide(run, Decision::Approve)?;
                }
                self.reload_artifact()?;
                if self.run.as_ref().and_then(Run::current).is_some() {
                    self.start_current().await?;
                }
            }
            KeyCode::Char('p' | 'r') if self.prompt_ready() => {
                if let Some(kind) = self.selected_prompt_kind() {
                    self.ui.open_prompt(kind);
                }
            }
            KeyCode::Char('e') if self.gate_ready() => self.edit(terminal)?,
            KeyCode::Char('d') if self.discuss_ready() => self.discuss(terminal)?,
            _ => {}
        }
        Ok(false)
    }

    fn gate_ready(&self) -> bool {
        matches!(self.ui.mode, Mode::Gate)
            && self
                .run
                .as_ref()
                .is_some_and(|run| run.cursor == self.ui.viewed_node)
            && self
                .run
                .as_ref()
                .and_then(Run::current)
                .is_some_and(|n| matches!(n.status, NodeStatus::Done | NodeStatus::Failed))
    }

    fn prompt_ready(&self) -> bool {
        matches!(self.ui.mode, Mode::Gate) && self.selected_prompt_kind().is_some()
    }

    fn selected_prompt_kind(&self) -> Option<PromptKind> {
        prompt_kind(self.run.as_ref(), self.ui.viewed_node)
    }

    fn discuss_ready(&self) -> bool {
        matches!(self.ui.mode, Mode::Gate)
            && self
                .run
                .as_ref()
                .and_then(|run| run.nodes.get(self.ui.viewed_node))
                .is_some_and(|node| {
                    node.command.is_none()
                        && node.session_id.is_some()
                        && matches!(node.status, NodeStatus::Done | NodeStatus::Failed)
                })
    }
    fn stop_running(&mut self) -> Result<()> {
        let Some(mut running) = self.running.take() else {
            return Ok(());
        };
        let _ = running.kill();
        let agent = self
            .run
            .as_ref()
            .and_then(Run::current)
            .map(|node| node.agent.clone())
            .unwrap_or_else(|| "agent".into());
        self.accept_event(Event::now(
            agent,
            EventKind::Failed {
                message: "stopped by operator".into(),
            },
        ))?;
        self.ui.error = None;
        Ok(())
    }
    fn scroll(&mut self, delta: i16) {
        self.ui.update(Message::Scroll(delta));
    }
    fn edit(&mut self, terminal: &mut Terminal<CrosstermBackend<io::Stdout>>) -> Result<()> {
        let run = self.run.as_mut().ok_or_else(|| anyhow!("no run"))?;
        self.orchestrator.decide(run, Decision::Edit)?;
        let node = run.current().ok_or_else(|| anyhow!("no node"))?;
        let path = self.repository.path(&run.id, &node.writes)?;
        let editor = std::env::var("EDITOR").unwrap_or_else(|_| "vi".into());
        let mut command = Command::new(editor);
        command.arg(path);
        let result = handoff::run(terminal, &mut command);
        self.run = Some(self.repository.load(&run.id)?);
        self.reload_artifact()?;
        result?;
        Ok(())
    }
    fn discuss(&mut self, terminal: &mut Terminal<CrosstermBackend<io::Stdout>>) -> Result<()> {
        let run = self.run.as_ref().ok_or_else(|| anyhow!("no run"))?;
        let node = run
            .nodes
            .get(self.ui.viewed_node)
            .ok_or_else(|| anyhow!("no node"))?;
        let session = node
            .session_id
            .as_ref()
            .ok_or_else(|| anyhow!("this node has no captured session"))?;
        let config = self
            .agents
            .get(&node.agent)
            .ok_or_else(|| anyhow!("agent not configured"))?;
        let mut command = Command::new(&config.bin);
        let args = if config.discuss_args.is_empty() {
            &config.resume_args
        } else {
            &config.discuss_args
        };
        command
            .args(args.iter().map(|a| a.replace("{session}", session)))
            .current_dir(&run.project);
        let handoff_result = handoff::run(terminal, &mut command);
        self.run = Some(self.repository.load(&run.id)?);
        self.reload_artifact()?;
        handoff_result?;
        Ok(())
    }
}

async fn next_agent_event(running: &mut Option<RunningAgent>) -> Option<Event> {
    match running {
        Some(agent) => agent.events.recv().await,
        None => std::future::pending().await,
    }
}

pub async fn run(config: UiConfig) -> Result<()> {
    let mut app = App::load(config)?;
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;
    terminal.clear()?;
    let result = event_loop(&mut terminal, &mut app).await;
    handoff::restore_stdio();
    terminal.show_cursor()?;
    result
}

async fn event_loop(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    app: &mut App,
) -> Result<()> {
    let mut input = EventStream::new();
    let mut tick = tokio::time::interval(Duration::from_millis(100));
    loop {
        terminal.draw(|frame| draw(frame, app))?;
        tokio::select! {
            event = input.next() => if let Some(event) = event { match event? { TerminalEvent::Key(key) if key.kind == KeyEventKind::Press => if app.key(key.code, terminal).await? { break; }, TerminalEvent::Resize(_, _) => {}, _ => {} } },
            event = next_agent_event(&mut app.running) => match event { Some(event) => app.accept_event(event)?, None => app.stream_closed()? },
            _ = tick.tick() => app.ui.update(Message::Tick),
        }
    }
    Ok(())
}

fn draw(frame: &mut ratatui::Frame<'_>, app: &App) {
    let prompt = matches!(app.ui.mode, Mode::Prompt(_)).then_some(app.ui.prompt.as_str());
    let areas = if matches!(app.ui.mode, Mode::RunList) {
        layout::run_list_areas(frame.area())
    } else {
        layout::areas(frame.area(), prompt)
    };
    if matches!(app.ui.mode, Mode::RunList) {
        flow::render_runs(
            frame,
            areas.flow,
            &app.runs,
            app.ui.selected,
            app.ui.focus,
            Utc::now(),
            &app.agents,
        );
        flow::render_run_summary(
            frame,
            areas.channel,
            app.runs.get(app.ui.selected),
            &app.agents,
        );
    } else {
        flow::render(
            frame,
            areas.flow,
            flow::FlowView {
                run: app.run.as_ref(),
                viewed_node: app.ui.viewed_node,
                artifact: &app.artifact,
                stream: &app.stream,
                scroll: app.ui.flow_scroll,
                focus: app.ui.focus,
                spinner: app.ui.spinner,
                activity: &app.agent_activity,
            },
        );
        channel::render(
            frame,
            areas.channel,
            app.run.as_ref(),
            app.ui.channel_scroll,
            app.ui.focus,
        );
    }
    help::render(
        frame,
        areas.help,
        help::HelpView {
            mode: &app.ui.mode,
            prompt_edit_mode: app.ui.prompt_edit_mode,
            status: app
                .run
                .as_ref()
                .and_then(|run| run.nodes.get(app.ui.viewed_node).map(|node| node.status)),
            is_current: app
                .run
                .as_ref()
                .is_some_and(|run| run.cursor == app.ui.viewed_node),
            can_prompt: app.prompt_ready(),
            can_discuss: app.discuss_ready(),
            error: if app
                .run
                .as_ref()
                .is_some_and(|run| run.cursor == app.ui.viewed_node)
            {
                app.ui.error.as_deref()
            } else {
                None
            },
            prompt: &app.ui.prompt,
            prompt_cursor: app.ui.prompt_cursor(),
        },
    );
}

#[cfg(test)]
mod tests {
    use super::{handle_normal_prompt_key, prompt_kind, returns_to_run_list};
    use crate::{
        runs::domain::{Node, NodeStatus, Run},
        terminal::application::{Message, Mode, Model, PromptEditMode, PromptKind},
    };
    use chrono::Utc;
    use crossterm::event::KeyCode;

    #[test]
    fn b_returns_to_run_list_only_from_a_project_gate() {
        assert_eq!(
            (
                returns_to_run_list(KeyCode::Char('b'), &Mode::Gate),
                returns_to_run_list(KeyCode::Char('b'), &Mode::Streaming),
            ),
            (true, false)
        );
    }

    #[test]
    fn vim_i_key_enters_prompt_insert_mode() {
        let mut model = Model::default();
        model.open_prompt(PromptKind::Initial);

        handle_normal_prompt_key(&mut model, KeyCode::Char('i'), 80);

        assert_eq!(model.prompt_edit_mode, PromptEditMode::Insert);
    }

    #[test]
    fn vim_uppercase_i_inserts_at_the_first_non_blank_character() {
        let mut model = Model::default();
        model.open_prompt(PromptKind::Initial);
        model.update(Message::EnterPromptInsertMode);
        for character in "  text".chars() {
            model.update(Message::Input(character));
        }
        model.update(Message::LeavePromptInsertMode);

        handle_normal_prompt_key(&mut model, KeyCode::Char('I'), 80);

        assert_eq!(
            (model.prompt_cursor(), model.prompt_edit_mode),
            (2, PromptEditMode::Insert)
        );
    }

    #[test]
    fn vim_o_opens_a_line_below_and_enters_insert_mode() {
        let mut model = prompt_model("one\ntwo\nthree");
        model.update(Message::MovePromptUp { width: 80 });

        handle_normal_prompt_key(&mut model, KeyCode::Char('o'), 80);

        assert_eq!(
            (
                model.prompt.as_str(),
                model.prompt_cursor(),
                model.prompt_edit_mode,
            ),
            ("one\ntwo\n\nthree", 8, PromptEditMode::Insert)
        );
    }

    #[test]
    fn vim_uppercase_o_opens_a_line_above_and_enters_insert_mode() {
        let mut model = prompt_model("one\ntwo\nthree");
        model.update(Message::MovePromptUp { width: 80 });

        handle_normal_prompt_key(&mut model, KeyCode::Char('O'), 80);

        assert_eq!(
            (
                model.prompt.as_str(),
                model.prompt_cursor(),
                model.prompt_edit_mode,
            ),
            ("one\n\ntwo\nthree", 4, PromptEditMode::Insert)
        );
    }

    #[test]
    fn vim_h_and_x_keys_delete_the_previous_prompt_character() {
        let mut model = Model::default();
        model.open_prompt(PromptKind::Initial);
        model.update(Message::EnterPromptInsertMode);
        model.update(Message::Input('a'));
        model.update(Message::Input('b'));
        model.update(Message::LeavePromptInsertMode);

        handle_normal_prompt_key(&mut model, KeyCode::Char('h'), 80);
        handle_normal_prompt_key(&mut model, KeyCode::Char('x'), 80);

        assert_eq!(model.prompt, "b");
    }

    #[test]
    fn vim_gg_and_uppercase_g_move_to_document_boundaries() {
        let mut model = Model::default();
        model.open_prompt(PromptKind::Initial);
        model.update(Message::EnterPromptInsertMode);
        for character in "some words".chars() {
            model.update(Message::Input(character));
        }
        model.update(Message::LeavePromptInsertMode);

        handle_normal_prompt_key(&mut model, KeyCode::Char('g'), 80);
        handle_normal_prompt_key(&mut model, KeyCode::Char('g'), 80);
        let after_gg = model.prompt_cursor();
        handle_normal_prompt_key(&mut model, KeyCode::Char('G'), 80);

        assert_eq!((after_gg, model.prompt_cursor()), (0, "some words".len()));
    }

    #[test]
    fn vim_diw_deletes_the_word_under_the_cursor() {
        let mut model = prompt_model("one two");

        for key in ['d', 'i', 'w'] {
            handle_normal_prompt_key(&mut model, KeyCode::Char(key), 80);
        }

        assert_eq!(model.prompt, "one ");
    }

    #[test]
    fn vim_ciw_removes_the_word_and_enters_insert_mode() {
        let mut model = prompt_model("one two");

        for key in ['c', 'i', 'w'] {
            handle_normal_prompt_key(&mut model, KeyCode::Char(key), 80);
        }

        assert_eq!(
            (model.prompt.as_str(), model.prompt_edit_mode),
            ("one ", PromptEditMode::Insert)
        );
    }

    #[test]
    fn vim_daw_deletes_the_word_and_its_adjacent_space() {
        let mut model = prompt_model("one two");

        for key in ['d', 'a', 'w'] {
            handle_normal_prompt_key(&mut model, KeyCode::Char(key), 80);
        }

        assert_eq!(model.prompt, "one");
    }

    #[test]
    fn vim_caw_deletes_around_the_word_and_enters_insert_mode() {
        let mut model = prompt_model("one two");

        for key in ['c', 'a', 'w'] {
            handle_normal_prompt_key(&mut model, KeyCode::Char(key), 80);
        }

        assert_eq!(
            (model.prompt.as_str(), model.prompt_edit_mode),
            ("one", PromptEditMode::Insert)
        );
    }

    #[test]
    fn vim_di_uppercase_w_uses_a_whitespace_delimited_word() {
        let mut model = prompt_model("one.two");

        for key in ['d', 'i', 'W'] {
            handle_normal_prompt_key(&mut model, KeyCode::Char(key), 80);
        }

        assert_eq!(model.prompt, "");
    }

    #[test]
    fn vim_dd_deletes_the_current_line_and_p_pastes_it_below() {
        let mut model = prompt_model("one\ntwo\nthree");
        model.update(Message::MovePromptUp { width: 80 });

        for key in ['d', 'd', 'p'] {
            handle_normal_prompt_key(&mut model, KeyCode::Char(key), 80);
        }

        assert_eq!(model.prompt, "one\nthree\ntwo");
    }

    #[test]
    fn vim_uppercase_d_deletes_from_the_cursor_through_the_line_end() {
        let mut model = prompt_model("one two\nthree");
        model.update(Message::MovePromptToStart);
        for _ in 0..4 {
            model.update(Message::MovePromptRight);
        }

        handle_normal_prompt_key(&mut model, KeyCode::Char('D'), 80);

        assert_eq!(model.prompt, "one \nthree");
    }

    #[test]
    fn vim_yy_yanks_the_current_line_and_p_pastes_it_below() {
        let mut model = prompt_model("one\ntwo");
        model.update(Message::MovePromptUp { width: 80 });

        for key in ['y', 'y', 'p'] {
            handle_normal_prompt_key(&mut model, KeyCode::Char(key), 80);
        }

        assert_eq!(model.prompt, "one\none\ntwo");
    }

    #[test]
    fn vim_uppercase_y_yanks_the_current_line() {
        let mut model = prompt_model("one\ntwo");
        model.update(Message::MovePromptUp { width: 80 });

        for key in ['Y', 'p'] {
            handle_normal_prompt_key(&mut model, KeyCode::Char(key), 80);
        }

        assert_eq!(model.prompt, "one\none\ntwo");
    }

    #[test]
    fn vim_yiw_yanks_the_word_under_the_cursor() {
        let mut model = prompt_model("one two");

        for key in ['y', 'i', 'w', 'p'] {
            handle_normal_prompt_key(&mut model, KeyCode::Char(key), 80);
        }

        assert_eq!(model.prompt, "one twotwo");
    }

    #[test]
    fn escape_cancels_a_pending_operator_without_closing_the_prompt() {
        let mut model = prompt_model("one two");

        handle_normal_prompt_key(&mut model, KeyCode::Char('d'), 80);
        handle_normal_prompt_key(&mut model, KeyCode::Esc, 80);

        assert_eq!(model.mode, Mode::Prompt(PromptKind::Initial));
    }

    #[test]
    fn invalid_text_object_sequence_does_not_execute_the_last_key() {
        let mut model = prompt_model("one two");

        for key in ['d', 'i', 'x'] {
            handle_normal_prompt_key(&mut model, KeyCode::Char(key), 80);
        }

        assert_eq!(model.prompt, "one two");
    }

    fn prompt_model(prompt: &str) -> Model {
        let mut model = Model::default();
        model.open_prompt(PromptKind::Initial);
        model.update(Message::EnterPromptInsertMode);
        for character in prompt.chars() {
            model.update(Message::Input(character));
        }
        model.update(Message::LeavePromptInsertMode);
        model
    }

    #[test]
    fn completed_historical_agent_accepts_a_revision_prompt() {
        let run = run_with_nodes(vec![
            node("plan", NodeStatus::Done),
            node("build", NodeStatus::Pending),
        ]);

        assert_eq!(prompt_kind(Some(&run), 0), Some(PromptKind::Revision));
    }

    #[test]
    fn future_pending_agent_cannot_be_prompted_out_of_order() {
        let run = run_with_nodes(vec![
            node("plan", NodeStatus::Pending),
            node("build", NodeStatus::Pending),
        ]);

        assert_eq!(prompt_kind(Some(&run), 1), None);
    }

    fn run_with_nodes(nodes: Vec<Node>) -> Run {
        Run {
            id: "001".into(),
            project: ".".into(),
            spec: None,
            nodes,
            cursor: 0,
            channel: vec![],
            created_at: Utc::now(),
        }
    }

    fn node(name: &str, status: NodeStatus) -> Node {
        Node {
            name: name.into(),
            agent: "agent".into(),
            role: "role".into(),
            writes: format!("{name}.md"),
            status,
            session_id: None,
            session_group: None,
            duration: None,
            attempts: u32::from(status != NodeStatus::Pending),
            command: None,
            skip_if_no_python_changes: false,
        }
    }
}
