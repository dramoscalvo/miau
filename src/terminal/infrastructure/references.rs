//! Independent document reference pane.

use super::pane;
use crate::{
    runs::domain::Run,
    terminal::application::{Focus, Model},
};
use ratatui::{
    Frame,
    layout::{Constraint, Layout, Rect},
    style::Stylize,
    text::Line,
    widgets::{Paragraph, Wrap},
};

pub fn render(
    frame: &mut Frame<'_>,
    area: Rect,
    run: Option<&Run>,
    artifact: &str,
    model: &mut Model,
) {
    let block = pane::block(" References · c activity ", model.focus == Focus::Channel);
    let inner = block.inner(area);
    frame.render_widget(block, area);
    let [heading, body] =
        Layout::vertical([Constraint::Length(3), Constraint::Min(0)]).areas(inner);
    let node = run.and_then(|run| run.nodes.get(model.reference_node));
    let title = node.map_or_else(
        || "Reference document".to_owned(),
        |node| format!("{}. {}", model.reference_node + 1, node.name),
    );
    frame.render_widget(
        Paragraph::new(vec![
            Line::from(title).cyan().bold(),
            Line::from(node.map_or("", |node| node.writes.as_str())).dim(),
            Line::from("Tab focus · ←/→ step · ↑↓ scroll · R reload").dim(),
        ]),
        heading,
    );
    let text = if artifact.trim().is_empty() {
        "No saved artifact for this step yet."
    } else {
        artifact
    };
    let paragraph = Paragraph::new(text).wrap(Wrap { trim: false });
    let max_scroll = paragraph
        .line_count(body.width.max(1))
        .saturating_sub(usize::from(body.height));
    model.reference_scroll = model
        .reference_scroll
        .min(u16::try_from(max_scroll).unwrap_or(u16::MAX));
    frame.render_widget(paragraph.scroll((model.reference_scroll, 0)), body);
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::{Terminal, backend::TestBackend};

    #[test]
    fn reference_keeps_points_from_both_review_and_handoff() {
        let mut terminal = Terminal::new(TestBackend::new(70, 12)).unwrap();
        let mut model = Model::default();
        terminal
            .draw(|frame| {
                render(
                    frame,
                    frame.area(),
                    None,
                    "# Review\nD1: Which database?\n# Handoff\nBR-3: Retain audit records.",
                    &mut model,
                )
            })
            .unwrap();
        let text: String = terminal
            .backend()
            .buffer()
            .content
            .iter()
            .map(|cell| cell.symbol())
            .collect();
        assert!(text.contains("D1: Which database?"));
        assert!(text.contains("BR-3: Retain audit records."));
    }

    #[test]
    fn reference_scroll_is_clamped_and_empty_documents_explain_their_state() {
        let mut terminal = Terminal::new(TestBackend::new(70, 8)).unwrap();
        let mut model = Model::default();
        model.reference_scroll = u16::MAX;
        terminal
            .draw(|frame| render(frame, frame.area(), None, "", &mut model))
            .unwrap();
        assert_eq!(model.reference_scroll, 0);
        let text: String = terminal
            .backend()
            .buffer()
            .content
            .iter()
            .map(|cell| cell.symbol())
            .collect();
        assert!(text.contains("No saved artifact for this step yet."));
    }
}
