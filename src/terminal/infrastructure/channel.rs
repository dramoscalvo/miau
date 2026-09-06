//! Activity log view.

use super::pane;
use crate::{runs::domain::Run, terminal::application::Focus};
use ratatui::{
    Frame,
    layout::Rect,
    style::Stylize,
    text::{Line, Span},
    widgets::{Paragraph, Wrap},
};
use unicode_width::UnicodeWidthChar;

fn truncate(text: &str, width: usize) -> String {
    let mut used = 0;
    text.chars()
        .take_while(|c| {
            let next = used + c.width().unwrap_or(0);
            if next <= width {
                used = next;
                true
            } else {
                false
            }
        })
        .collect()
}

#[cfg(test)]
fn tail_scroll(text: &str, area: Rect, history_offset: u16) -> u16 {
    let paragraph = Paragraph::new(text).wrap(Wrap { trim: true });
    tail_scroll_for_paragraph(&paragraph, area, history_offset)
}

fn tail_scroll_for_paragraph(paragraph: &Paragraph<'_>, area: Rect, history_offset: u16) -> u16 {
    let width = area.width.saturating_sub(2).max(1);
    let visible_height = usize::from(area.height.saturating_sub(2));
    let rendered_lines = paragraph.line_count(width);
    let bottom = rendered_lines.saturating_sub(visible_height);
    u16::try_from(bottom.saturating_sub(usize::from(history_offset))).unwrap_or(u16::MAX)
}

pub fn render(frame: &mut Frame<'_>, area: Rect, run: Option<&Run>, scroll: u16, focus: Focus) {
    let width = area.width.saturating_sub(2) as usize;
    let lines = run
        .map(|r| {
            r.channel
                .iter()
                .flat_map(|e| {
                    let route =
                        e.to.as_ref()
                            .map_or_else(|| e.from.clone(), |to| format!("{} → {to}", e.from));
                    [
                        Line::from(vec![
                            Span::from(format!("{} ", e.at.format("%H:%M"))).dark_gray(),
                            Span::from(truncate(&route, width.saturating_sub(6)))
                                .cyan()
                                .bold(),
                        ]),
                        Line::from(truncate(&e.summary, width).gray()),
                    ]
                })
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    let paragraph = Paragraph::new(lines).wrap(Wrap { trim: true });
    let scroll = tail_scroll_for_paragraph(&paragraph, area, scroll);
    frame.render_widget(
        paragraph
            .block(pane::block(" Activity ", focus == Focus::Channel))
            .scroll((scroll, 0)),
        area,
    );
}

#[cfg(test)]
mod tests {
    use super::{render, tail_scroll};
    use crate::{
        runs::domain::{ChannelEntry, Run},
        terminal::application::Focus,
    };
    use chrono::{TimeZone, Utc};
    use ratatui::{Terminal, backend::TestBackend};
    use ratatui::{layout::Rect, style::Color};

    fn run_with_activity() -> Run {
        Run {
            id: "001".into(),
            project: ".".into(),
            spec: None,
            nodes: vec![],
            cursor: 0,
            channel: vec![ChannelEntry {
                at: Utc.with_ymd_and_hms(2026, 9, 6, 14, 5, 0).unwrap(),
                from: "codex".into(),
                to: Some("you".into()),
                summary: "review ready".into(),
            }],
            created_at: Utc.with_ymd_and_hms(2026, 9, 6, 14, 0, 0).unwrap(),
        }
    }

    #[test]
    fn pane_is_titled_activity() {
        let mut terminal = Terminal::new(TestBackend::new(16, 3)).unwrap();

        terminal
            .draw(|frame| render(frame, frame.area(), None, 0, Focus::Flow))
            .unwrap();

        let title = (1..11)
            .map(|x| terminal.backend().buffer()[(x, 0)].symbol())
            .collect::<String>();
        assert_eq!(title, " Activity ");
    }

    #[test]
    fn tail_scroll_shows_the_last_row_when_content_overflows() {
        let area = Rect::new(0, 0, 10, 5);

        assert_eq!(tail_scroll("one\ntwo\nthree\nfour", area, 0), 1);
    }

    #[test]
    fn tail_scroll_allows_browsing_older_rows() {
        let area = Rect::new(0, 0, 10, 5);

        assert_eq!(tail_scroll("one\ntwo\nthree\nfour", area, 1), 0);
    }

    #[test]
    fn activity_visually_separates_time_route_and_summary() {
        let mut terminal = Terminal::new(TestBackend::new(30, 5)).unwrap();
        let run = run_with_activity();

        terminal
            .draw(|frame| render(frame, frame.area(), Some(&run), 0, Focus::Channel))
            .unwrap();

        let buffer = terminal.backend().buffer();
        assert_eq!(
            (buffer[(1, 1)].fg, buffer[(7, 1)].fg, buffer[(1, 2)].fg),
            (Color::DarkGray, Color::Cyan, Color::Gray)
        );
    }
}
