//! Run flow and artifact views.

use std::{collections::HashMap, path::Path};

use super::pane;
use crate::{
    execution::application::AgentConfig,
    runs::domain::{Node, NodeStatus, Run},
    terminal::application::Focus,
};
use chrono::{DateTime, Utc};
use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Style, Stylize},
    text::{Line, Span},
    widgets::{List, ListItem, ListState, Paragraph, Wrap},
};

pub(crate) fn node_status_label(status: NodeStatus) -> &'static str {
    match status {
        NodeStatus::Pending => "Ready",
        NodeStatus::Running => "Working",
        NodeStatus::Done => "Waiting for you",
        NodeStatus::Failed => "Needs attention",
        NodeStatus::Skipped => "Skipped",
    }
}

fn detail_title(status: Option<NodeStatus>, activity: &str, spinner: usize) -> String {
    match status {
        Some(NodeStatus::Running) => {
            let spinner = ["⠋", "⠙", "⠹", "⠸"].get(spinner).copied().unwrap_or("⠋");
            format!(" Working {spinner} · {activity} ")
        }
        Some(NodeStatus::Done) => " Artifact · waiting for you ".into(),
        Some(NodeStatus::Failed) => " Agent output · needs attention ".into(),
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

fn node_status_style(status: NodeStatus) -> Style {
    match status {
        NodeStatus::Pending => Style::default().dim(),
        NodeStatus::Running => Style::default().cyan(),
        NodeStatus::Done => Style::default().yellow(),
        NodeStatus::Failed => Style::default().red().bold(),
        NodeStatus::Skipped => Style::default().dark_gray(),
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
        frame.render_widget(
            Paragraph::new("No runs yet — press n to create one")
                .style(Style::default().dim())
                .block(pane::block(" Runs ", focus == Focus::Flow)),
            area,
        );
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
                        Span::styled(node_status_label(n.status), node_status_style(n.status)),
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
    let show_agent_output = viewing_current && !stream.is_empty() && (running || failed);
    let text = if show_agent_output {
        stream.join("\n")
    } else {
        artifact.to_owned()
    };
    let title = detail_title(status, activity, spinner);
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
        FlowView, detail_title, node_status_label, render, render_run_summary, render_runs,
        streaming_scroll,
    };
    use crate::{
        execution::application::AgentConfig,
        runs::domain::{ChannelEntry, Node, NodeStatus, Run},
        terminal::application::Focus,
    };
    use chrono::{TimeZone, Utc};
    use ratatui::{Terminal, backend::TestBackend};
    use ratatui::{layout::Rect, style::Color};
    use std::collections::HashMap;

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
    fn empty_run_list_invites_the_user_to_create_a_run() {
        let mut terminal = Terminal::new(TestBackend::new(60, 5)).unwrap();
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
                node_status_label(NodeStatus::Pending),
                node_status_label(NodeStatus::Running),
                node_status_label(NodeStatus::Done),
                node_status_label(NodeStatus::Failed),
                node_status_label(NodeStatus::Skipped),
            ],
            [
                "Ready",
                "Working",
                "Waiting for you",
                "Needs attention",
                "Skipped"
            ]
        );
    }

    #[test]
    fn detail_title_distinguishes_work_from_human_attention() {
        assert_eq!(
            [
                detail_title(Some(NodeStatus::Running), "thinking", 0),
                detail_title(Some(NodeStatus::Done), "", 0),
            ],
            [
                " Working ⠋ · thinking ".to_owned(),
                " Artifact · waiting for you ".to_owned(),
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
}
