//! Activity log view.

use super::pane;
use crate::{runs::domain::Run, terminal::application::Focus};
use ratatui::{
    Frame,
    layout::Rect,
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

fn tail_scroll(text: &str, area: Rect, history_offset: u16) -> u16 {
    let width = area.width.saturating_sub(2).max(1);
    let visible_height = usize::from(area.height.saturating_sub(2));
    let rendered_lines = Paragraph::new(text)
        .wrap(Wrap { trim: true })
        .line_count(width);
    let bottom = rendered_lines.saturating_sub(visible_height);
    u16::try_from(bottom.saturating_sub(usize::from(history_offset))).unwrap_or(u16::MAX)
}

pub fn render(frame: &mut Frame<'_>, area: Rect, run: Option<&Run>, scroll: u16, focus: Focus) {
    let width = area.width.saturating_sub(2) as usize;
    let text = run
        .map(|r| {
            r.channel
                .iter()
                .map(|e| {
                    let route =
                        e.to.as_ref()
                            .map(|to| format!("{} → {to}", e.from))
                            .unwrap_or_else(|| e.from.clone());
                    format!(
                        "{} {}\n{}",
                        e.at.format("%H:%M"),
                        truncate(&route, width.saturating_sub(6)),
                        truncate(&e.summary, width)
                    )
                })
                .collect::<Vec<_>>()
                .join("\n")
        })
        .unwrap_or_default();
    let scroll = tail_scroll(&text, area, scroll);
    frame.render_widget(
        Paragraph::new(text)
            .block(pane::block(" Activity ", focus == Focus::Channel))
            .scroll((scroll, 0))
            .wrap(Wrap { trim: true }),
        area,
    );
}

#[cfg(test)]
mod tests {
    use super::{render, tail_scroll};
    use crate::terminal::application::Focus;
    use ratatui::layout::Rect;
    use ratatui::{Terminal, backend::TestBackend};

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
}
