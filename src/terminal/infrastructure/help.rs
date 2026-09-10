//! Context-sensitive help view.

use crate::{
    runs::domain::NodeStatus,
    terminal::application::{
        Action, DetailView, Mode, PromptEditMode, PromptKind, prompt_cursor_position, prompt_rows,
    },
};
use ratatui::{
    Frame,
    layout::Rect,
    style::{Style, Stylize},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Paragraph, Wrap},
};

fn hint_line(text: &str) -> Line<'static> {
    let mut spans = Vec::new();
    for (index, hint) in text.trim().split(" · ").enumerate() {
        if index > 0 {
            spans.push(Span::from(" · ").dark_gray());
        }
        if let Some((key, description)) = hint.split_once(' ') {
            spans.extend([
                Span::from(key.to_owned()).cyan().bold(),
                Span::from(format!(" {description}")).dark_gray(),
            ]);
        } else {
            spans.push(Span::from(hint.to_owned()).dark_gray());
        }
    }
    Line::from(spans)
}

fn confirmation_text(action: &Action) -> &'static str {
    match action {
        Action::Quit => "kill running agent and quit? y/n",
        Action::Stop => "stop running agent and keep miau open? y/n",
        Action::Finish => "finish this run and skip all unfinished steps? y/n",
    }
}

fn gate_text(
    status: Option<NodeStatus>,
    is_current: bool,
    can_prompt: bool,
    can_discuss: bool,
    can_finish: bool,
) -> String {
    let mut commands = Vec::new();
    match status {
        Some(NodeStatus::Pending) if is_current => {
            if can_prompt {
                commands.push("p prompt");
            }
            commands.push("s start");
        }
        Some(NodeStatus::Done | NodeStatus::Failed) if is_current => {
            commands.push("a approve & continue");
            if can_prompt {
                commands.push("r request changes");
            }
            if can_discuss {
                commands.push("d discuss with agent");
            }
            commands.push("e edit artifact");
        }
        Some(NodeStatus::Done | NodeStatus::Failed) => {
            if can_prompt {
                commands.push("r request changes");
            }
            if can_discuss {
                commands.push("d discuss with agent");
            }
        }
        Some(NodeStatus::Pending | NodeStatus::Running | NodeStatus::Skipped) => {}
        None => commands.push("workflow complete"),
    }
    if can_finish {
        commands.push("f finish");
    }
    commands.extend(["←/→ agents", "b runs", "tab pane", "q quit"]);
    commands.join(" · ")
}

pub struct HelpView<'a> {
    pub mode: &'a Mode,
    pub prompt_edit_mode: PromptEditMode,
    pub detail_view: DetailView,
    pub has_review: bool,
    pub status: Option<NodeStatus>,
    pub is_current: bool,
    pub can_prompt: bool,
    pub can_discuss: bool,
    pub can_finish: bool,
    pub error: Option<&'a str>,
    pub prompt: &'a str,
    pub prompt_cursor: usize,
}

pub fn render(frame: &mut Frame<'_>, area: Rect, view: HelpView<'_>) {
    let HelpView {
        mode,
        prompt_edit_mode,
        detail_view,
        has_review,
        status,
        is_current,
        can_prompt,
        can_discuss,
        can_finish,
        error,
        prompt,
        prompt_cursor,
    } = view;
    if let Mode::Prompt(kind) = mode {
        let width = area.width.saturating_sub(2).max(1) as usize;
        let (cursor_row, cursor_column) = prompt_cursor_position(prompt, prompt_cursor, width);
        let visible_height = area.height.saturating_sub(2) as usize;
        let scroll = cursor_row.saturating_sub(visible_height.saturating_sub(1));
        let lines = prompt_rows(prompt, width)
            .into_iter()
            .map(Line::from)
            .collect::<Vec<_>>();
        let (title, instructions) = match (kind, prompt_edit_mode) {
            (PromptKind::Initial, PromptEditMode::Normal) => (
                " Initial prompt · NORMAL ",
                " i/a/I/A insert · o/O open line · hjkl · w/W b/B e/E · x dd/D delete · yy/Y yank · p paste · text objects · 0/^/$ gg/G · Enter save · Esc cancel ",
            ),
            (PromptKind::Revision, PromptEditMode::Normal) => (
                " Request changes · NORMAL ",
                " i/a/I/A insert · o/O open line · hjkl · w/W b/B e/E · x dd/D delete · yy/Y yank · p paste · text objects · 0/^/$ gg/G · Enter send · Esc cancel ",
            ),
            (PromptKind::Discussion, PromptEditMode::Normal) => (
                " Back from discussion · update artifact? ",
                " Enter request update · i edit request · Esc back to review ",
            ),
            (PromptKind::Discussion, PromptEditMode::Insert) => (
                " Back from discussion · edit update request ",
                " Enter request update · Esc normal ",
            ),
            (PromptKind::Initial, PromptEditMode::Insert) => (
                " Initial prompt · INSERT ",
                " type to edit · Backspace/Delete remove · Enter save · Esc normal ",
            ),
            (PromptKind::Revision, PromptEditMode::Insert) => (
                " Request changes · INSERT ",
                " type to edit · Backspace/Delete remove · Enter send · Esc normal ",
            ),
        };
        frame.render_widget(
            Paragraph::new(lines).scroll((scroll as u16, 0)).block(
                Block::default()
                    .title(title.bold().cyan())
                    .title_bottom(hint_line(instructions))
                    .borders(Borders::ALL)
                    .border_type(BorderType::Rounded)
                    .border_style(Style::default().cyan()),
            ),
            area,
        );
        if area.width >= 3 && area.height >= 3 {
            frame.set_cursor_position((
                area.x + 1 + cursor_column as u16,
                area.y + 1 + cursor_row.saturating_sub(scroll) as u16,
            ));
        }
        return;
    }

    let text = if let Some(error) = error {
        Line::from(format!("error: {error}").red().bold())
    } else {
        let mut hints = match mode {
            Mode::RunList => "enter open · n new · q quit".into(),
            Mode::Streaming => "←/→ agents · tab pane · ↑↓ scroll · x stop · q quit".into(),
            Mode::Gate => gate_text(status, is_current, can_prompt, can_discuss, can_finish),
            Mode::Prompt(_) => String::new(),
            Mode::Confirm(action) => confirmation_text(action).into(),
        };
        if !matches!(mode, Mode::RunList | Mode::Confirm(_)) {
            let view_hints = match detail_view {
                DetailView::Review if has_review => "v complete document · PgUp/PgDn summary",
                DetailView::Review => "v changes · PgUp/PgDn document",
                DetailView::Artifact => "v changes · PgUp/PgDn document",
                DetailView::Changes if has_review => {
                    "v review summary · j/k files · PgUp/PgDn diff"
                }
                DetailView::Changes => "v complete document · j/k files · PgUp/PgDn diff",
            };
            hints = format!("{hints} · {view_hints}");
        }
        hint_line(&hints)
    };
    frame.render_widget(Paragraph::new(text).wrap(Wrap { trim: true }), area);
}

#[cfg(test)]
mod tests {
    use super::{confirmation_text, gate_text};
    use crate::runs::domain::NodeStatus;
    use crate::terminal::application::{
        Action, DetailView, Mode, PromptEditMode, PromptKind, prompt_rows,
    };
    use ratatui::{Terminal, backend::TestBackend, style::Color};

    #[test]
    fn prompt_editor_has_rounded_corners() {
        let backend = TestBackend::new(40, 5);
        let mut terminal = Terminal::new(backend).expect("test terminal should be created");
        let mode = Mode::Prompt(PromptKind::Initial);

        terminal
            .draw(|frame| {
                super::render(
                    frame,
                    frame.area(),
                    super::HelpView {
                        mode: &mode,
                        prompt_edit_mode: PromptEditMode::Normal,
                        detail_view: DetailView::Artifact,
                        has_review: false,
                        status: None,
                        is_current: false,
                        can_prompt: false,
                        can_discuss: false,
                        can_finish: false,
                        error: None,
                        prompt: "",
                        prompt_cursor: 0,
                    },
                );
            })
            .expect("prompt editor should render");

        assert_eq!(terminal.backend().buffer()[(0, 0)].symbol(), "╭");
    }

    #[test]
    fn prompt_rows_respect_unicode_display_width() {
        assert_eq!(prompt_rows("ab界", 3), vec!["ab", "界"]);
    }

    #[test]
    fn stop_confirmation_explains_that_miau_stays_open() {
        assert_eq!(
            confirmation_text(&Action::Stop),
            "stop running agent and keep miau open? y/n"
        );
    }

    #[test]
    fn finish_confirmation_explains_that_unfinished_steps_are_skipped() {
        assert_eq!(
            confirmation_text(&Action::Finish),
            "finish this run and skip all unfinished steps? y/n"
        );
    }

    #[test]
    fn pending_current_agent_advertises_prompt_before_start() {
        assert_eq!(
            gate_text(Some(NodeStatus::Pending), true, true, false, true),
            "p prompt · s start · f finish · ←/→ agents · b runs · tab pane · q quit"
        );
    }

    #[test]
    fn completed_gate_advertises_only_decision_commands() {
        assert_eq!(
            gate_text(Some(NodeStatus::Done), true, true, true, true),
            "a approve & continue · r request changes · d discuss with agent · e edit artifact · f finish · ←/→ agents · b runs · tab pane · q quit"
        );
    }

    #[test]
    fn completed_historical_agent_advertises_follow_up_without_approval() {
        assert_eq!(
            gate_text(Some(NodeStatus::Done), false, true, true, true),
            "r request changes · d discuss with agent · f finish · ←/→ agents · b runs · tab pane · q quit"
        );
    }

    #[test]
    fn command_nodes_do_not_advertise_agent_prompting() {
        assert_eq!(
            gate_text(Some(NodeStatus::Done), false, false, false, false),
            "←/→ agents · b runs · tab pane · q quit"
        );
    }

    #[test]
    fn help_visually_separates_keys_from_descriptions() {
        let mut terminal = Terminal::new(TestBackend::new(40, 1)).unwrap();
        let mode = Mode::RunList;

        terminal
            .draw(|frame| {
                super::render(
                    frame,
                    frame.area(),
                    super::HelpView {
                        mode: &mode,
                        prompt_edit_mode: PromptEditMode::Normal,
                        detail_view: DetailView::Artifact,
                        has_review: false,
                        status: None,
                        is_current: false,
                        can_prompt: false,
                        can_discuss: false,
                        can_finish: false,
                        error: None,
                        prompt: "",
                        prompt_cursor: 0,
                    },
                );
            })
            .unwrap();

        let buffer = terminal.backend().buffer();
        assert_eq!(
            (buffer[(0, 0)].fg, buffer[(6, 0)].fg),
            (Color::Cyan, Color::DarkGray)
        );
    }
}
