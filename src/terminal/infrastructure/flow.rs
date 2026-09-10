//! Run flow and artifact views.

use std::{collections::HashMap, path::Path};

use super::pane;
use crate::{
    execution::application::AgentConfig,
    runs::domain::{Node, NodeStatus, Run},
    terminal::application::{DetailView, Focus, artifact_review},
    workflow::domain::{WorkingTreeChange, WorkingTreeChangeKind},
};
use chrono::{DateTime, Utc};
use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Style, Stylize},
    text::{Line, Span},
    widgets::{List, ListItem, ListState, Paragraph, Wrap},
};

pub(crate) fn node_status_label(status: NodeStatus, complete: bool) -> &'static str {
    match (status, complete) {
        (NodeStatus::Done, true) => "Complete",
        (NodeStatus::Pending, _) => "Ready",
        (NodeStatus::Running, _) => "Working",
        (NodeStatus::Done, false) => "Waiting for you",
        (NodeStatus::Failed, _) => "Needs attention",
        (NodeStatus::Skipped, _) => "Skipped",
    }
}

fn detail_title(
    status: Option<NodeStatus>,
    complete: bool,
    activity: &str,
    spinner: usize,
) -> String {
    match (status, complete) {
        (Some(NodeStatus::Running), _) => {
            let spinner = ["⠋", "⠙", "⠹", "⠸"].get(spinner).copied().unwrap_or("⠋");
            format!(" Working {spinner} · {activity} ")
        }
        (Some(NodeStatus::Done), true) => " Artifact · complete ".into(),
        (Some(NodeStatus::Done), false) => " Artifact · waiting for you ".into(),
        (Some(NodeStatus::Failed), _) => " Agent output · needs attention ".into(),
        _ => " Artifact ".into(),
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RunStatus {
    Ready,
    Working,
    Waiting,
    NeedsAttention,
    Complete,
}

impl RunStatus {
    fn for_run(run: &Run) -> Self {
        match run.current().map(|node| node.status) {
            Some(NodeStatus::Pending | NodeStatus::Skipped) => Self::Ready,
            Some(NodeStatus::Running) => Self::Working,
            Some(NodeStatus::Done) => Self::Waiting,
            Some(NodeStatus::Failed) => Self::NeedsAttention,
            None => Self::Complete,
        }
    }

    fn label(self) -> &'static str {
        match self {
            Self::Ready => "Ready",
            Self::Working => "Working",
            Self::Waiting => "Waiting for you",
            Self::NeedsAttention => "Needs attention",
            Self::Complete => "Complete",
        }
    }

    fn marker(self) -> &'static str {
        match self {
            Self::Ready => "○",
            Self::Working => "●",
            Self::Waiting => "◆",
            Self::NeedsAttention => "!",
            Self::Complete => "✓",
        }
    }

    fn style(self) -> Style {
        match self {
            Self::Ready => Style::default().dim(),
            Self::Working => Style::default().cyan(),
            Self::Waiting => Style::default().yellow(),
            Self::NeedsAttention => Style::default().red().bold(),
            Self::Complete => Style::default().green(),
        }
    }
}

fn project_name(project: &Path) -> String {
    project
        .file_name()
        .filter(|name| !name.is_empty())
        .map_or_else(
            || project.display().to_string(),
            |name| name.to_string_lossy().into(),
        )
}

fn completed_nodes(run: &Run) -> usize {
    run.nodes
        .iter()
        .filter(|node| matches!(node.status, NodeStatus::Done | NodeStatus::Skipped))
        .count()
}

fn node_agent_label(node: &Node, agents: &HashMap<String, AgentConfig>) -> String {
    match agents.get(&node.agent).and_then(AgentConfig::effort) {
        Some(effort) => format!("{} · {} · {effort} effort", node.name, node.agent),
        None => format!("{} · {}", node.name, node.agent),
    }
}

fn node_status_style(status: NodeStatus, complete: bool) -> Style {
    match (status, complete) {
        (NodeStatus::Done, true) => Style::default().green(),
        (NodeStatus::Pending, _) => Style::default().dim(),
        (NodeStatus::Running, _) => Style::default().cyan(),
        (NodeStatus::Done, false) => Style::default().yellow(),
        (NodeStatus::Failed, _) => Style::default().red().bold(),
        (NodeStatus::Skipped, _) => Style::default().dark_gray(),
    }
}

fn field_line(label: &'static str, value: String) -> Line<'static> {
    Line::from(vec![label.cyan().bold(), value.into()])
}

fn last_activity_at(run: &Run) -> DateTime<Utc> {
    run.channel
        .iter()
        .map(|entry| entry.at)
        .max()
        .unwrap_or(run.created_at)
}

fn relative_age(at: DateTime<Utc>, now: DateTime<Utc>) -> String {
    let seconds = now.signed_duration_since(at).num_seconds().max(0);
    if seconds < 60 {
        "now".into()
    } else if seconds < 3_600 {
        format!("{}m ago", seconds / 60)
    } else if seconds < 86_400 {
        format!("{}h ago", seconds / 3_600)
    } else {
        format!("{}d ago", seconds / 86_400)
    }
}

pub fn render_runs(
    frame: &mut Frame<'_>,
    area: Rect,
    runs: &[Run],
    selected: usize,
    focus: Focus,
    now: DateTime<Utc>,
    agents: &HashMap<String, AgentConfig>,
) {
    if runs.is_empty() {
        render_empty_runs(frame, area, focus);
        return;
    }

    let items = runs
        .iter()
        .enumerate()
        .map(|(index, run)| {
            let status = RunStatus::for_run(run);
            let current = run.current().map_or_else(
                || "workflow complete".into(),
                |node| node_agent_label(node, agents),
            );
            let heading = Line::from(vec![
                Span::styled(format!("{} ", status.marker()), status.style()),
                Span::styled(format!("{}  ", status.label()), status.style()),
                Span::from(format!("{}  ", run.id)).cyan().bold(),
                Span::from(format!("{}  ", project_name(&run.project))).magenta(),
            ]);
            let detail = Line::from(vec![
                Span::raw("  "),
                Span::styled(current, Style::default().bold()),
                Span::from(format!(
                    "  {}/{} · {}",
                    completed_nodes(run),
                    run.nodes.len(),
                    relative_age(last_activity_at(run), now)
                ))
                .dark_gray(),
            ]);
            ListItem::new(vec![heading, detail]).style(if index == selected {
                Style::default().on_dark_gray().bold()
            } else {
                Style::default()
            })
        })
        .collect::<Vec<_>>();
    let mut state = ListState::default().with_selected(Some(selected));
    frame.render_stateful_widget(
        List::new(items).block(pane::block(" Runs ", focus == Focus::Flow)),
        area,
        &mut state,
    );
}

fn render_empty_runs(frame: &mut Frame<'_>, area: Rect, focus: Focus) {
    let block = pane::block(" Runs ", focus == Focus::Flow);
    let inner = block.inner(area);
    frame.render_widget(block, area);

    if inner.width < 49 || inner.height < 12 {
        frame.render_widget(
            Paragraph::new("No runs yet — press n to create one").style(Style::default().dim()),
            inner,
        );
        return;
    }

    let lines = vec![
        Line::from("▄█▄       ▄█▄").magenta().bold(),
        Line::from("███▄     ▄███").magenta().bold(),
        Line::from("█████████████").magenta().bold(),
        Line::from("███ ▀███▀ ███").magenta().bold(),
        Line::from("███   ▄   ███").magenta().bold(),
        Line::from("▀███▄▀▀▀▄███▀").magenta().bold(),
        Line::from("  ▀███████▀").magenta().bold(),
        Line::default(),
        Line::from("miau").cyan().bold(),
        Line::from("one agent at a time · you decide what happens next").dim(),
        Line::default(),
        Line::from("No runs yet — press n to create one").dim(),
    ];
    let height = u16::try_from(lines.len()).unwrap_or(u16::MAX);
    let [_, art, _] = Layout::vertical([
        Constraint::Fill(1),
        Constraint::Length(height),
        Constraint::Fill(1),
    ])
    .areas(inner);
    frame.render_widget(Paragraph::new(lines).centered(), art);
}

fn next_action(run: &Run) -> String {
    match run.current() {
        Some(node) => match node.status {
            NodeStatus::Pending | NodeStatus::Skipped => format!("Start {}", node.name),
            NodeStatus::Running => format!("Wait for {}", node.name),
            NodeStatus::Done => format!("Review {} artifact", node.name),
            NodeStatus::Failed => format!("Resolve {} failure", node.name),
        },
        None => "Workflow complete".into(),
    }
}

pub fn render_run_summary(
    frame: &mut Frame<'_>,
    area: Rect,
    run: Option<&Run>,
    agents: &HashMap<String, AgentConfig>,
) {
    let Some(run) = run else {
        frame.render_widget(
            Paragraph::new("Select a run to inspect it")
                .style(Style::default().dim())
                .block(pane::block(" Selected run ", false)),
            area,
        );
        return;
    };

    let status = RunStatus::for_run(run);
    let mut lines = vec![
        Line::from(Span::styled(status.label(), status.style())),
        Line::default(),
        field_line(
            "Run ",
            format!("{} · {}", run.id, project_name(&run.project)),
        ),
    ];
    if let Some(node) = run.current() {
        lines.push(field_line("Current: ", node_agent_label(node, agents)));
    }
    lines.push(field_line(
        "Progress: ",
        format!("{}/{} complete", completed_nodes(run), run.nodes.len()),
    ));
    if let Some(node) = run.current() {
        let duration = node
            .duration
            .map(|duration| format!(" · Duration: {:.1}s", duration.as_secs_f64()))
            .unwrap_or_default();
        lines.push(field_line(
            "Attempts: ",
            format!("{}{duration}", node.attempts),
        ));
    }
    lines.extend([
        Line::default(),
        Line::from(vec![
            "Next: ".cyan().bold(),
            Span::from(next_action(run)).yellow().bold(),
        ]),
    ]);

    if !run.channel.is_empty() {
        lines.extend([
            Line::default(),
            Line::from("Latest activity".magenta().bold()),
        ]);
        for entry in run.channel.iter().rev().take(2).rev() {
            let route = entry
                .to
                .as_ref()
                .map_or_else(|| entry.from.clone(), |to| format!("{} → {to}", entry.from));
            lines.push(Line::from(vec![
                Span::from(format!("{} ", entry.at.format("%H:%M"))).dark_gray(),
                Span::from(route).cyan().bold(),
            ]));
            lines.push(Line::from(entry.summary.as_str()).gray());
        }
    }

    frame.render_widget(
        Paragraph::new(lines)
            .block(pane::block(" Selected run ", false))
            .wrap(Wrap { trim: true }),
        area,
    );
}

pub struct FlowView<'a> {
    pub run: Option<&'a Run>,
    pub viewed_node: usize,
    pub artifact: &'a str,
    pub stream: &'a [String],
    pub scroll: u16,
    pub focus: Focus,
    pub spinner: usize,
    pub activity: &'a str,
    pub detail_view: DetailView,
    pub working_tree: WorkingTreeView<'a>,
}

pub struct WorkingTreeView<'a> {
    pub changes: &'a [WorkingTreeChange],
    pub selected: usize,
    pub diff: &'a str,
    pub scroll: u16,
    pub error: Option<&'a str>,
}

impl WorkingTreeView<'_> {
    #[cfg(test)]
    fn empty() -> Self {
        Self {
            changes: &[],
            selected: 0,
            diff: "",
            scroll: 0,
            error: None,
        }
    }
}

fn gate_guidance(run: &Run, viewed_node: usize) -> String {
    let Some(node) = run.nodes.get(viewed_node) else {
        return String::new();
    };
    if !matches!(node.status, NodeStatus::Done | NodeStatus::Failed) {
        return String::new();
    }
    let check = if node.status == NodeStatus::Failed {
        "This step failed. Inspect the error and any partial changes before deciding."
    } else {
        match node.role.as_str() {
            "planner" => "Check the approach, scope, and acceptance cases.",
            "critic" | "reviewer" => {
                "Check findings and evidence; decide which issues need changes before continuing."
            }
            "implementer" => {
                "Inspect the code changes and verification results against the accepted plan."
            }
            _ => "Check the result, unresolved issues, and verification evidence.",
        }
    };
    let mut guidance = format!("{}: {check}\n", node.name);
    if viewed_node == run.cursor {
        let next = run
            .nodes
            .iter()
            .skip(run.cursor + 1)
            .find(|node| node.status != NodeStatus::Skipped);
        guidance.push_str(&next.map_or_else(
            || "a approve & continue: complete the workflow. ".to_owned(),
            |next| format!("a approve & continue: start {}. ", next.name),
        ));
    }
    if node.command.is_none() {
        guidance.push_str(if viewed_node == run.cursor {
            "r request changes: revise with your feedback. "
        } else {
            "r request changes: revisits this step; later steps will be offered again. "
        });
        if node.session_id.is_some() {
            guidance.push_str("d discuss: clarify, then update. ");
        }
    }
    if viewed_node == run.cursor {
        guidance.push_str("\ne edit artifact: document only; saving keeps this gate open.");
    }
    guidance
}

pub fn render(frame: &mut Frame<'_>, area: Rect, view: FlowView<'_>) {
    let FlowView {
        run,
        viewed_node,
        artifact,
        stream,
        scroll,
        focus,
        spinner,
        activity,
        detail_view,
        working_tree,
    } = view;
    let [nodes_area, artifact_area] = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(9), Constraint::Min(1)])
        .areas(area);
    let items: Vec<ListItem<'_>> = run
        .map(|r| {
            r.nodes
                .iter()
                .enumerate()
                .map(|(i, n)| {
                    let complete = i < r.cursor;
                    let marker = if i == r.cursor {
                        Span::from("> ").yellow().bold()
                    } else {
                        Span::from("  ").dim()
                    };
                    let duration = n
                        .duration
                        .map(|d| format!(" {:.1}s", d.as_secs_f64()))
                        .unwrap_or_default();
                    ListItem::new(Line::from(vec![
                        marker,
                        Span::from(format!("{:<12} ", n.name)).bold(),
                        Span::from(format!("{:<8} ", n.agent)).magenta(),
                        Span::styled(
                            node_status_label(n.status, complete),
                            node_status_style(n.status, complete),
                        ),
                        Span::from(duration).dark_gray(),
                    ]))
                    .style(if i == viewed_node {
                        Style::default().on_dark_gray()
                    } else {
                        Style::default()
                    })
                })
                .collect()
        })
        .unwrap_or_default();
    let mut node_state = ListState::default().with_selected(Some(viewed_node));
    frame.render_stateful_widget(
        List::new(items).block(pane::block(" Flow ", focus == Focus::Flow)),
        nodes_area,
        &mut node_state,
    );
    let viewed = run.and_then(|run| run.nodes.get(viewed_node));
    let status = viewed.map(|node| node.status);
    let running = status == Some(NodeStatus::Running);
    let failed = status == Some(NodeStatus::Failed);
    let viewing_current = run.is_some_and(|run| run.cursor == viewed_node);
    let viewing_complete = run.is_some_and(|run| viewed_node < run.cursor);
    let show_agent_output = viewing_current && !stream.is_empty() && (running || failed);
    if detail_view == DetailView::Changes {
        render_working_tree(frame, artifact_area, working_tree, focus);
        return;
    }
    let review = (detail_view == DetailView::Review && !show_agent_output)
        .then(|| artifact_review(artifact))
        .flatten();
    let mut text = if show_agent_output {
        stream.join("\n")
    } else {
        review.unwrap_or(artifact).to_owned()
    };
    if let Some(run) = run {
        let guidance = gate_guidance(run, viewed_node);
        if !guidance.is_empty() {
            text = format!("{guidance}\n\n{text}");
        }
    }
    let title = if review.is_some() {
        format!(
            " Review · {} · v full artifact · PgDn more ",
            status.map_or("", |status| node_status_label(status, viewing_complete))
        )
    } else if detail_view == DetailView::Review && !show_agent_output && !artifact.is_empty() {
        " Full artifact · no compact review · PgDn more ".into()
    } else {
        detail_title(status, viewing_complete, activity, spinner)
    };
    let scroll = if show_agent_output {
        streaming_scroll(&text, artifact_area)
    } else {
        scroll
    };
    frame.render_widget(
        Paragraph::new(text)
            .block(pane::block(&title, focus == Focus::Flow))
            .scroll((scroll, 0))
            .wrap(Wrap { trim: false }),
        artifact_area,
    );
}

fn render_working_tree(frame: &mut Frame<'_>, area: Rect, view: WorkingTreeView<'_>, focus: Focus) {
    if let Some(error) = view.error {
        frame.render_widget(
            Paragraph::new(format!("Unable to inspect working tree: {error}"))
                .red()
                .block(pane::block(" Working tree ", focus == Focus::Flow))
                .wrap(Wrap { trim: false }),
            area,
        );
        return;
    }
    if view.changes.is_empty() {
        frame.render_widget(
            Paragraph::new("Working tree clean")
                .dim()
                .block(pane::block(" Working tree ", focus == Focus::Flow)),
            area,
        );
        return;
    }

    let [tree_area, diff_area] = if area.width >= 80 {
        Layout::horizontal([Constraint::Percentage(38), Constraint::Fill(1)]).areas(area)
    } else {
        Layout::vertical([Constraint::Percentage(40), Constraint::Fill(1)]).areas(area)
    };
    let (items, selected_row) = working_tree_items(view.changes, view.selected);
    let mut state = ListState::default().with_selected(Some(selected_row));
    frame.render_stateful_widget(
        List::new(items)
            .block(pane::block(" Working tree ", focus == Focus::Flow))
            .highlight_style(Style::default().on_dark_gray().bold()),
        tree_area,
        &mut state,
    );
    frame.render_widget(
        Paragraph::new(styled_diff(view.diff))
            .block(pane::block(" Diff ", false))
            .scroll((view.scroll, 0))
            .wrap(Wrap { trim: false }),
        diff_area,
    );
}

fn working_tree_items(
    changes: &[WorkingTreeChange],
    selected: usize,
) -> (Vec<ListItem<'static>>, usize) {
    let mut items = Vec::new();
    let mut previous_directories = Vec::<String>::new();
    let mut selected_row = 0;

    for (index, change) in changes.iter().enumerate() {
        let directories = change
            .path
            .parent()
            .into_iter()
            .flat_map(Path::components)
            .map(|component| component.as_os_str().to_string_lossy().into_owned())
            .collect::<Vec<_>>();
        let shared = directories
            .iter()
            .zip(&previous_directories)
            .take_while(|(left, right)| left == right)
            .count();
        for (depth, directory) in directories.iter().enumerate().skip(shared) {
            items.push(ListItem::new(Line::from(vec![
                Span::from("  ".repeat(depth)).dim(),
                Span::from(format!("{directory}/")).magenta().bold(),
            ])));
        }
        if index == selected {
            selected_row = items.len();
        }
        let name = change.display_path().file_name().map_or_else(
            || change.path.display().to_string(),
            |name| name.to_string_lossy().into(),
        );
        let rename = change
            .previous_path
            .as_ref()
            .map_or_else(String::new, |previous| format!(" ← {}", previous.display()));
        items.push(ListItem::new(Line::from(vec![
            Span::from("  ".repeat(directories.len())).dim(),
            Span::styled(
                format!("{} ", change_marker(change.kind)),
                change_style(change.kind),
            ),
            Span::from(name),
            Span::from(rename).dark_gray(),
        ])));
        previous_directories = directories;
    }

    (items, selected_row)
}

fn change_marker(kind: WorkingTreeChangeKind) -> &'static str {
    match kind {
        WorkingTreeChangeKind::Added => "A",
        WorkingTreeChangeKind::Modified => "M",
        WorkingTreeChangeKind::Deleted => "D",
        WorkingTreeChangeKind::Renamed => "R",
    }
}

fn change_style(kind: WorkingTreeChangeKind) -> Style {
    match kind {
        WorkingTreeChangeKind::Added => Style::default().green().bold(),
        WorkingTreeChangeKind::Modified => Style::default().yellow().bold(),
        WorkingTreeChangeKind::Deleted => Style::default().red().bold(),
        WorkingTreeChangeKind::Renamed => Style::default().cyan().bold(),
    }
}

fn styled_diff(diff: &str) -> Vec<Line<'static>> {
    diff.lines()
        .map(|line| {
            let span = if line.starts_with("+++") || line.starts_with("---") {
                Span::from(line.to_owned()).dim()
            } else if line.starts_with('+') {
                Span::from(line.to_owned()).green()
            } else if line.starts_with('-') {
                Span::from(line.to_owned()).red()
            } else if line.starts_with("@@") {
                Span::from(line.to_owned()).cyan()
            } else if line.starts_with("diff ") || line.starts_with("index ") {
                Span::from(line.to_owned()).dark_gray()
            } else {
                Span::from(line.to_owned())
            };
            Line::from(span)
        })
        .collect()
}

fn streaming_scroll(text: &str, area: Rect) -> u16 {
    let width = area.width.saturating_sub(2);
    let visible_height = usize::from(area.height.saturating_sub(2));
    let rendered_lines = Paragraph::new(text)
        .wrap(Wrap { trim: false })
        .line_count(width);

    u16::try_from(rendered_lines.saturating_sub(visible_height)).unwrap_or(u16::MAX)
}

#[cfg(test)]
mod tests {
    use super::{
        FlowView, WorkingTreeView, detail_title, node_status_label, render, render_run_summary,
        render_runs, render_working_tree, streaming_scroll,
    };
    use crate::{
        execution::application::AgentConfig,
        runs::domain::{ChannelEntry, Node, NodeStatus, Run},
        terminal::application::{DetailView, Focus},
        workflow::domain::{WorkingTreeChange, WorkingTreeChangeKind},
    };
    use chrono::{TimeZone, Utc};
    use ratatui::{Terminal, backend::TestBackend};
    use ratatui::{layout::Rect, style::Color};
    use std::collections::HashMap;

    #[test]
    fn gate_guidance_explains_current_and_historical_decisions() {
        let mut run = run_with_status(NodeStatus::Done);
        run.cursor = 0;
        let guidance = super::gate_guidance(&run, 0);
        assert!(guidance.contains("scope"));
        assert!(guidance.contains("start review"));
        assert!(guidance.contains("r request changes"));
        assert!(guidance.contains("e edit artifact"));
        run.cursor = 1;
        let guidance = super::gate_guidance(&run, 0);
        assert!(guidance.contains("revisits this step"));
        assert!(!guidance.contains("a approve"));
        run.nodes[1].status = NodeStatus::Failed;
        assert!(super::gate_guidance(&run, 1).contains("failed"));
        run.nodes[1].status = NodeStatus::Running;
        assert!(super::gate_guidance(&run, 1).is_empty());
    }

    fn buffer_text(terminal: &Terminal<TestBackend>) -> String {
        terminal
            .backend()
            .buffer()
            .content
            .iter()
            .map(|cell| cell.symbol())
            .collect()
    }

    fn run_with_status(status: NodeStatus) -> Run {
        Run {
            id: "014".into(),
            project: "/work/miau".into(),
            spec: None,
            nodes: vec![
                Node {
                    name: "plan".into(),
                    agent: "claude".into(),
                    role: "planner".into(),
                    writes: "plan.md".into(),
                    status: NodeStatus::Done,
                    session_id: None,
                    session_group: None,
                    duration: Some(std::time::Duration::from_secs(8)),
                    attempts: 1,
                    command: None,
                    skip_if_no_python_changes: false,
                },
                Node {
                    name: "review".into(),
                    agent: "codex".into(),
                    role: "reviewer".into(),
                    writes: "review.md".into(),
                    status,
                    session_id: None,
                    session_group: None,
                    duration: Some(std::time::Duration::from_secs(38)),
                    attempts: 2,
                    command: None,
                    skip_if_no_python_changes: false,
                },
            ],
            cursor: 1,
            channel: vec![ChannelEntry {
                at: Utc.with_ymd_and_hms(2026, 9, 4, 10, 48, 0).unwrap(),
                from: "codex".into(),
                to: Some("you".into()),
                summary: "review needs attention".into(),
            }],
            created_at: Utc.with_ymd_and_hms(2026, 9, 4, 10, 0, 0).unwrap(),
        }
    }

    fn agent_configs() -> HashMap<String, AgentConfig> {
        HashMap::from([(
            "codex".into(),
            AgentConfig {
                bin: "codex".into(),
                args: vec![],
                effort: Some("medium".into()),
                resume_args: vec![],
                resume_insert_at: None,
                schema_args: vec![],
                discuss_args: vec![],
                parser: "codex".into(),
            },
        )])
    }

    #[test]
    fn run_list_row_surfaces_status_project_step_progress_and_age() {
        let mut terminal = Terminal::new(TestBackend::new(53, 5)).unwrap();
        let runs = [run_with_status(NodeStatus::Failed)];
        let now = Utc.with_ymd_and_hms(2026, 9, 4, 11, 0, 0).unwrap();
        let agents = agent_configs();

        terminal
            .draw(|frame| {
                render_runs(frame, frame.area(), &runs, 0, Focus::Flow, now, &agents);
            })
            .unwrap();

        let text = buffer_text(&terminal);
        assert!(
            [
                "!",
                "Needs attention",
                "014",
                "miau",
                "review · codex · medium effort",
                "1/2",
                "12m ago",
            ]
            .iter()
            .all(|expected| text.contains(expected)),
            "rendered run row was: {text}"
        );
    }

    #[test]
    fn selected_run_summary_explains_failure_and_latest_activity() {
        let mut terminal = Terminal::new(TestBackend::new(46, 18)).unwrap();
        let run = run_with_status(NodeStatus::Failed);
        let agents = agent_configs();

        terminal
            .draw(|frame| render_run_summary(frame, frame.area(), Some(&run), &agents))
            .unwrap();

        let text = buffer_text(&terminal);
        assert!(
            [
                "Needs attention",
                "Run 014 · miau",
                "Current: review · codex · medium effort",
                "Progress: 1/2 complete",
                "Attempts: 2 · Duration: 38.0s",
                "Next: Resolve review failure",
                "codex → you",
                "review needs attention",
            ]
            .iter()
            .all(|expected| text.contains(expected)),
            "rendered summary was: {text}"
        );
    }

    #[test]
    fn empty_run_list_shows_the_miau_cat_and_human_gated_purpose() {
        let mut terminal = Terminal::new(TestBackend::new(60, 15)).unwrap();
        let agents = HashMap::new();

        terminal
            .draw(|frame| {
                render_runs(
                    frame,
                    frame.area(),
                    &[],
                    0,
                    Focus::Flow,
                    Utc::now(),
                    &agents,
                );
            })
            .unwrap();

        let text = buffer_text(&terminal);
        let expected = [
            "▄█▄       ▄█▄",
            "███ ▀███▀ ███",
            "miau",
            "one agent at a time · you decide what happens next",
            "No runs yet",
            "create one",
        ];
        let missing = expected
            .iter()
            .filter(|expected| !text.contains(*expected))
            .collect::<Vec<_>>();
        assert!(
            missing.is_empty(),
            "missing {missing:?} from rendered empty state: {text}"
        );
    }

    #[test]
    fn empty_run_list_keeps_the_creation_prompt_in_a_small_pane() {
        let mut terminal = Terminal::new(TestBackend::new(40, 5)).unwrap();
        let agents = HashMap::new();

        terminal
            .draw(|frame| {
                render_runs(
                    frame,
                    frame.area(),
                    &[],
                    0,
                    Focus::Flow,
                    Utc::now(),
                    &agents,
                );
            })
            .unwrap();

        assert!(buffer_text(&terminal).contains("No runs yet — press n to create one"));
    }

    #[test]
    fn review_view_hides_handoff_but_full_artifact_keeps_it_accessible() {
        let run = run_with_status(NodeStatus::Done);
        let mut terminal = Terminal::new(TestBackend::new(90, 28)).unwrap();
        for (detail_view, show_handoff) in
            [(DetailView::Review, false), (DetailView::Artifact, true)]
        {
            terminal.draw(|frame| {
                render(frame, frame.area(), FlowView {
                    run: Some(&run), viewed_node: 0,
                    artifact: "# Review\nGoal: compact review\n\n# Handoff\nTechnical evidence",
                    stream: &[], scroll: 0, focus: Focus::Flow, spinner: 0,
                    activity: "", detail_view, working_tree: WorkingTreeView::empty(),
                });
            }).unwrap();
            let text = buffer_text(&terminal);
            assert!(text.contains("Goal: compact review"));
            assert_eq!(text.contains("Technical evidence"), show_handoff);
        }
    }

    #[test]
    fn streaming_scroll_keeps_the_last_line_visible() {
        let text = "one\ntwo\nthree\nfour";

        assert_eq!(streaming_scroll(text, Rect::new(0, 0, 20, 5)), 1);
    }

    #[test]
    fn streaming_scroll_accounts_for_wrapped_lines() {
        let text = "123456789";

        assert_eq!(streaming_scroll(text, Rect::new(0, 0, 6, 4)), 1);
    }

    #[test]
    fn streaming_scroll_stays_at_the_top_when_content_fits() {
        assert_eq!(streaming_scroll("one\ntwo", Rect::new(0, 0, 20, 5)), 0);
    }

    #[test]
    fn node_status_labels_describe_the_human_action() {
        assert_eq!(
            [
                node_status_label(NodeStatus::Pending, false),
                node_status_label(NodeStatus::Running, false),
                node_status_label(NodeStatus::Done, false),
                node_status_label(NodeStatus::Done, true),
                node_status_label(NodeStatus::Failed, false),
                node_status_label(NodeStatus::Skipped, true),
            ],
            [
                "Ready",
                "Working",
                "Waiting for you",
                "Complete",
                "Needs attention",
                "Skipped"
            ]
        );
    }

    #[test]
    fn detail_title_distinguishes_work_from_human_attention() {
        assert_eq!(
            [
                detail_title(Some(NodeStatus::Running), true, "thinking", 0),
                detail_title(Some(NodeStatus::Done), false, "thinking", 0),
                detail_title(Some(NodeStatus::Done), true, "", 0),
            ],
            [
                " Working ⠋ · thinking ".to_owned(),
                " Artifact · waiting for you ".to_owned(),
                " Artifact · complete ".to_owned(),
            ]
        );
    }

    #[test]
    fn selected_agent_is_scrolled_into_the_flow_list_viewport() {
        let run = Run {
            id: "001".into(),
            project: ".".into(),
            spec: None,
            nodes: (0..10)
                .map(|index| Node {
                    name: format!("node{index}"),
                    agent: "agent".into(),
                    role: "role".into(),
                    writes: format!("node{index}.md"),
                    status: NodeStatus::Done,
                    session_id: None,
                    session_group: None,
                    duration: None,
                    attempts: 1,
                    command: None,
                    skip_if_no_python_changes: false,
                })
                .collect(),
            cursor: 0,
            channel: vec![],
            created_at: Utc::now(),
        };
        let mut terminal = Terminal::new(TestBackend::new(50, 15)).unwrap();

        terminal
            .draw(|frame| {
                render(
                    frame,
                    frame.area(),
                    FlowView {
                        run: Some(&run),
                        viewed_node: 9,
                        artifact: "",
                        stream: &[],
                        scroll: 0,
                        focus: Focus::Flow,
                        spinner: 0,
                        activity: "",
                        detail_view: DetailView::Artifact,
                        working_tree: WorkingTreeView::empty(),
                    },
                );
            })
            .unwrap();

        let mut visible_flow = String::new();
        for y in 1..8 {
            for x in 0..50 {
                visible_flow.push_str(terminal.backend().buffer()[(x, y)].symbol());
            }
        }
        assert!(visible_flow.contains("node9"));
    }

    #[test]
    fn flow_rows_visually_separate_agent_status_and_metadata() {
        let run = run_with_status(NodeStatus::Done);
        let mut terminal = Terminal::new(TestBackend::new(50, 15)).unwrap();

        terminal
            .draw(|frame| {
                render(
                    frame,
                    frame.area(),
                    FlowView {
                        run: Some(&run),
                        viewed_node: 0,
                        artifact: "",
                        stream: &[],
                        scroll: 0,
                        focus: Focus::Channel,
                        spinner: 0,
                        activity: "",
                        detail_view: DetailView::Artifact,
                        working_tree: WorkingTreeView::empty(),
                    },
                );
            })
            .unwrap();

        let buffer = terminal.backend().buffer();
        assert_eq!(
            (buffer[(16, 2)].fg, buffer[(25, 2)].fg, buffer[(40, 2)].fg),
            (Color::Magenta, Color::Yellow, Color::DarkGray)
        );
    }

    #[test]
    fn working_tree_renders_hierarchy_status_colors_and_diff_colors() {
        let changes = vec![
            WorkingTreeChange::new("docs/new.md", WorkingTreeChangeKind::Added),
            WorkingTreeChange::new("src/changed.rs", WorkingTreeChangeKind::Modified),
            WorkingTreeChange::new("src/removed.rs", WorkingTreeChangeKind::Deleted),
            WorkingTreeChange::renamed("old.rs", "src/renamed.rs"),
        ];
        let mut terminal = Terminal::new(TestBackend::new(100, 18)).unwrap();

        terminal
            .draw(|frame| {
                render_working_tree(
                    frame,
                    frame.area(),
                    WorkingTreeView {
                        changes: &changes,
                        selected: 0,
                        diff: "@@ -1 +1 @@\n-old line\n+new line",
                        scroll: 0,
                        error: None,
                    },
                    Focus::Flow,
                );
            })
            .unwrap();

        let buffer = terminal.backend().buffer();
        let text = buffer_text(&terminal);
        assert!(text.contains("docs/") && text.contains("src/") && text.contains("← old.rs"));
        assert!(
            [
                ("A", Color::Green),
                ("M", Color::Yellow),
                ("D", Color::Red),
                ("R", Color::Cyan),
                ("+", Color::Green),
                ("-", Color::Red),
            ]
            .iter()
            .all(|(symbol, color)| buffer
                .content
                .iter()
                .any(|cell| cell.symbol() == *symbol && cell.fg == *color))
        );
    }
}
