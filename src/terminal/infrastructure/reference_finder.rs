//! Fuzzy finder popup over saved reference documents.

use crate::terminal::application::reference_finder::{Entry, Finder};
use ratatui::{
    Frame,
    layout::{Constraint, Layout, Rect},
    style::{Style, Stylize},
    text::{Line, Span},
    widgets::{Block, BorderType, Clear, List, ListItem, ListState, Paragraph},
};
use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

const STEP_WIDTH: usize = 18;

pub fn render(frame: &mut Frame<'_>, finder: &Finder) {
    let screen = frame.area();
    let width = screen.width.min(96);
    let height = screen.height.saturating_sub(2).clamp(1, 22);
    let area = Rect::new(
        screen.x + (screen.width - width) / 2,
        screen.y + (screen.height - height) / 2,
        width,
        height,
    );
    let title = format!(
        " Find reference · {}/{} ",
        finder.match_count(),
        finder.total()
    );
    let block = Block::bordered()
        .border_type(BorderType::Rounded)
        .border_style(Style::default().magenta())
        .title(title.bold().cyan())
        .title_bottom(" Enter open · ↑↓ select · Esc close ".dim());
    let inner = block.inner(area);
    frame.render_widget(Clear, area);
    frame.render_widget(block, area);
    let [input, list] = Layout::vertical([Constraint::Length(1), Constraint::Min(0)]).areas(inner);
    let prompt = format!("› {}", finder.query);
    frame.render_widget(Paragraph::new(prompt.as_str()).bold(), input);
    let cursor = input
        .x
        .saturating_add(u16::try_from(prompt.width()).unwrap_or(u16::MAX));
    frame.set_cursor_position((cursor.min(input.right().saturating_sub(1)), input.y));
    if finder.match_count() == 0 {
        frame.render_widget(Paragraph::new("No matching reference.").dim(), list);
        return;
    }
    let text_width = usize::from(list.width).saturating_sub(STEP_WIDTH + 3);
    let items: Vec<_> = finder
        .matches()
        .map(|(entry, positions)| ListItem::new(row(entry, positions, text_width)))
        .collect();
    let mut state = ListState::default().with_selected(Some(finder.selected));
    frame.render_stateful_widget(
        List::new(items)
            .highlight_style(Style::default().reversed())
            .highlight_symbol("> "),
        list,
        &mut state,
    );
}

fn row<'a>(entry: &'a Entry, positions: &[usize], width: usize) -> Line<'a> {
    let step = truncate(&entry.step, STEP_WIDTH);
    let padding = STEP_WIDTH.saturating_sub(step.width()) + 1;
    let mut spans = vec![Span::raw(step).dim(), Span::raw(" ".repeat(padding))];
    let mut used = 0;
    for (index, character) in entry.text.chars().enumerate() {
        used += character.width().unwrap_or(0);
        if used > width {
            break;
        }
        let span = Span::raw(character.to_string());
        spans.push(if positions.binary_search(&index).is_ok() {
            span.yellow().bold()
        } else if entry.line.is_none() {
            span.cyan()
        } else {
            span
        });
    }
    Line::from(spans)
}

fn truncate(text: &str, width: usize) -> String {
    let mut used = 0;
    text.chars()
        .take_while(|character| {
            used += character.width().unwrap_or(0);
            used <= width
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::terminal::application::reference_finder::entries;
    use ratatui::{Terminal, backend::TestBackend};

    fn screen(finder: &Finder, width: u16) -> String {
        let mut terminal = Terminal::new(TestBackend::new(width, 12)).unwrap();
        terminal.draw(|frame| render(frame, finder)).unwrap();
        terminal
            .backend()
            .buffer()
            .content
            .iter()
            .map(|cell| cell.symbol())
            .collect()
    }

    #[test]
    fn finder_lists_ranked_matches_with_their_step() {
        let mut finder = Finder::new(entries([
            ("plan", "plan.md", "D1: Which database?"),
            ("review", "review.md", "BR-3: Retain audit records."),
        ]));
        finder.input('b');
        finder.input('r');
        let text = screen(&finder, 80);
        assert!(text.contains("Find reference"));
        assert!(text.contains("› br"));
        assert!(text.contains("2. review"));
        assert!(text.contains("BR-3: Retain audit records."));
    }

    #[test]
    fn wide_text_is_truncated_by_display_width_and_empty_results_explain_themselves() {
        let artifact = "界".repeat(80);
        let mut finder = Finder::new(entries([("計画", "plan.md", artifact.as_str())]));
        let text = screen(&finder, 40);
        assert!(text.contains("界"));
        finder.input('z');
        assert!(screen(&finder, 40).contains("No matching reference."));
    }
}
