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
    widgets::{Block, BorderType, Borders, Clear, Paragraph, Wrap},
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
    pub show_cursor: bool,
}

fn prompt_help(
    kind: &PromptKind,
    prompt_edit_mode: PromptEditMode,
) -> (&'static str, &'static str) {
    match (kind, prompt_edit_mode) {
        (PromptKind::DiagramNote, PromptEditMode::Normal) => (
            " Diagram note · saved draft · NORMAL ",
            " i edit · Enter/Esc back · S sends notes from Diagram as Request changes ",
        ),
        (PromptKind::DiagramNote, PromptEditMode::Insert) => (
            " Diagram note · saved draft · INSERT ",
            " type to edit · Enter newline · Esc/Ctrl+C normal · S sends notes from Diagram ",
        ),
        (PromptKind::Answer, _) => (
            " Decision answer · saved draft ",
            " type to edit · Enter newline · Esc/Ctrl+C back to decisions · S submits from decisions ",
        ),
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
            " Enter request update · Esc/Ctrl+C normal ",
        ),
        (PromptKind::Initial, PromptEditMode::Insert) => (
            " Initial prompt · INSERT ",
            " type to edit · Backspace/Delete remove · Enter save · Esc/Ctrl+C normal ",
        ),
        (PromptKind::Revision, PromptEditMode::Insert) => (
            " Request changes · INSERT ",
            " type to edit · Backspace/Delete remove · Enter send · Esc/Ctrl+C normal ",
        ),
    }
}

pub fn render(frame: &mut Frame<'_>, area: Rect, view: &HelpView<'_>) {
    let HelpView {
        mode,
        prompt_edit_mode,
        error,
        prompt,
        prompt_cursor,
        show_cursor,
        ..
    } = view;
    if let Mode::Prompt(kind) = mode {
        let width = area.width.saturating_sub(2).max(1) as usize;
        let (cursor_row, cursor_column) = prompt_cursor_position(prompt, *prompt_cursor, width);
        let visible_height = area.height.saturating_sub(2) as usize;
        let scroll = cursor_row.saturating_sub(visible_height.saturating_sub(1));
        let lines = prompt_rows(prompt, width)
            .into_iter()
            .map(Line::from)
            .collect::<Vec<_>>();
        let (title, _) = prompt_help(kind, *prompt_edit_mode);
        let instructions = match (kind, prompt_edit_mode) {
            (PromptKind::Answer, _) => " Enter newline · Esc back · F1 help ",
            (PromptKind::DiagramNote, PromptEditMode::Normal) => " i edit · Enter back · ? help ",
            (PromptKind::DiagramNote, PromptEditMode::Insert) => {
                " Enter newline · Esc normal · F1 help "
            }
            (PromptKind::Initial, PromptEditMode::Normal) => {
                " i edit · Enter save · Esc cancel · ? help "
            }
            (PromptKind::Initial, PromptEditMode::Insert) => " Enter save · Esc normal · F1 help ",
            (_, PromptEditMode::Normal) => " i edit · Enter send · Esc cancel · ? help ",
            (_, PromptEditMode::Insert) => " Enter send · Esc normal · F1 help ",
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
        if *show_cursor && area.width >= 3 && area.height >= 3 {
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
        let hints = compact_hints(view);
        hint_line(&hints)
    };
    frame.render_widget(Paragraph::new(text).wrap(Wrap { trim: true }), area);
}

fn all_hints(view: &HelpView<'_>) -> String {
    if let Mode::Prompt(kind) = view.mode {
        return prompt_help(kind, view.prompt_edit_mode).1.trim().to_owned();
    }
    let mut hints = match view.mode {
        Mode::RunList => "↑↓ select run · enter open · n new · q quit".into(),
        Mode::Streaming => "←/→ agents · tab pane · ↑↓ scroll · x stop · q quit".into(),
        Mode::Gate => gate_text(
            view.status,
            view.is_current,
            view.can_prompt,
            view.can_discuss,
            view.can_finish,
        ),
        Mode::Prompt(_) => String::new(),
        Mode::Confirm(action) => confirmation_text(action).into(),
    };
    if !matches!(view.mode, Mode::RunList | Mode::Confirm(_)) {
        let view_hints = match view.detail_view {
            DetailView::Diagram => {
                "Esc document · v next view · j/k select · } relationship · Enter follow/in · Backspace back · H/J/K/L pan · n note · S send notes · ] source · o open · R reload · PgUp/PgDn details"
            }
            DetailView::Decisions => {
                "↑↓ decisions · Enter answer · S submit answers · v review · PgUp/PgDn scroll"
            }
            DetailView::Review if view.has_review => {
                "g diagram · v complete document · PgUp/PgDn summary"
            }
            DetailView::Review => "v changes · PgUp/PgDn document",
            DetailView::Artifact => "v changes · PgUp/PgDn document",
            DetailView::Changes => "v next view · j/k files · PgUp/PgDn diff",
        };
        hints = format!("{hints} · {view_hints}");
    }
    hints
}

fn compact_hints(view: &HelpView<'_>) -> String {
    let hints = match view.mode {
        Mode::RunList => "enter open · n new · q quit".to_owned(),
        Mode::Streaming => "x stop · v view".to_owned(),
        Mode::Confirm(action) => return confirmation_text(action).to_owned(),
        Mode::Gate => {
            let actions = gate_text(
                view.status,
                view.is_current,
                view.can_prompt,
                view.can_discuss,
                view.can_finish,
            );
            let mut main: Vec<&str> = actions
                .split(" · ")
                .filter(|hint| {
                    matches!(
                        hint.split_once(' ').map(|(key, _)| key),
                        Some("p" | "s" | "a" | "r")
                    )
                })
                .collect();
            if view.detail_view == DetailView::Decisions {
                main = vec!["Enter answer", "S submit"];
            } else if view.detail_view == DetailView::Diagram {
                main = vec!["Esc document", "Enter follow", "n note", "S send notes"];
                if view.can_prompt {
                    main.push("p/r prompt");
                }
            }
            main.extend(["v view", "b runs"]);
            main.join(" · ")
        }
        Mode::Prompt(_) => String::new(),
    };
    format!("{hints} · ? help")
}

pub fn render_popup(frame: &mut Frame<'_>, view: &HelpView<'_>, scroll: &mut u16) {
    let screen = frame.area();
    if screen.is_empty() {
        return;
    }
    let width = screen.width.min(76);
    let hints = all_hints(view);
    let inner_width = width.saturating_sub(4).max(1) as usize;
    let lines: Vec<Line<'static>> = hints
        .split(" · ")
        .flat_map(|hint| prompt_rows(hint, inner_width).into_iter().map(hint_line))
        .collect();
    let height = screen
        .height
        .saturating_sub(2)
        .min(lines.len().saturating_add(2) as u16)
        .max(1);
    let area = Rect::new(
        screen.x + (screen.width - width) / 2,
        screen.y + (screen.height - height) / 2,
        width,
        height,
    );
    let visible = height.saturating_sub(2).max(1);
    *scroll = (*scroll).min((lines.len() as u16).saturating_sub(visible));
    frame.render_widget(Clear, area);
    frame.render_widget(
        Paragraph::new(lines).scroll((*scroll, 0)).block(
            Block::bordered()
                .border_type(BorderType::Rounded)
                .title(" Keyboard help ".bold().cyan())
                .title_bottom(hint_line(" Esc close · ↑↓ scroll "))
                .padding(ratatui::widgets::Padding::horizontal(1)),
        ),
        area,
    );
}

#[cfg(test)]
mod tests {
    use super::{confirmation_text, gate_text};
    use crate::runs::domain::NodeStatus;
    use crate::terminal::application::{
        Action, DetailView, Mode, PromptEditMode, PromptKind, prompt_rows,
    };
    use ratatui::{Terminal, backend::TestBackend, style::Color};

    fn gate_view() -> super::HelpView<'static> {
        super::HelpView {
            mode: &Mode::Gate,
            prompt_edit_mode: PromptEditMode::Normal,
            detail_view: DetailView::Review,
            has_review: true,
            status: Some(NodeStatus::Done),
            is_current: true,
            can_prompt: true,
            can_discuss: true,
            can_finish: true,
            error: None,
            prompt: "",
            prompt_cursor: 0,
            show_cursor: true,
        }
    }

    #[test]
    fn footer_keeps_primary_actions_and_moves_secondary_actions_to_help() {
        let view = gate_view();
        assert_eq!(
            super::compact_hints(&view),
            "a approve & continue · r request changes · v view · b runs · ? help"
        );
        let full = super::all_hints(&view);
        for shortcut in ["d discuss", "e edit", "f finish", "tab pane", "q quit"] {
            assert!(full.contains(shortcut));
        }
    }

    #[test]
    fn popup_fits_small_terminals_and_clamps_scrolling_to_visible_content() {
        for (width, height) in [(1, 1), (25, 8), (100, 30)] {
            let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
            let mut scroll = u16::MAX;
            terminal
                .draw(|frame| super::render_popup(frame, &gate_view(), &mut scroll))
                .unwrap();
            assert!(scroll < u16::MAX);
            if width == 100 {
                let text: String = terminal
                    .backend()
                    .buffer()
                    .content
                    .iter()
                    .map(|cell| cell.symbol())
                    .collect();
                assert!(text.contains("Keyboard help"));
                assert!(text.contains("discuss with agent"));
                assert!(text.contains("Esc close"));
                assert_eq!(scroll, 0);
            }
        }
    }

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
                    &super::HelpView {
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
                        show_cursor: true,
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
    fn diagram_footer_advertises_prompt_return_when_available() {
        let mut view = gate_view();
        view.detail_view = DetailView::Diagram;

        assert_eq!(
            super::compact_hints(&view),
            "Esc document · Enter follow · n note · S send notes · p/r prompt · v view · b runs · ? help"
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
                    &super::HelpView {
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
                        show_cursor: true,
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
