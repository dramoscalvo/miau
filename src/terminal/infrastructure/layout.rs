//! Responsive terminal layout.

use crate::terminal::application::prompt_rows;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
pub struct Areas {
    pub flow: Rect,
    pub channel: Rect,
    pub help: Rect,
}

pub fn areas(area: Rect, prompt: Option<&str>) -> Areas {
    let help_height = prompt.map_or(1, |prompt| {
        let inner_width = area.width.saturating_sub(2) as usize;
        let row_count = u16::try_from(prompt_rows(prompt, inner_width).len()).unwrap_or(u16::MAX);
        let desired = row_count.saturating_add(2);
        let maximum = (area.height / 3).max(3).min(area.height);
        desired.min(maximum)
    });
    let [body, help] = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(1), Constraint::Length(help_height)])
        .areas(area);
    let [flow, channel] = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Ratio(3, 4), Constraint::Ratio(1, 4)])
        .areas(body);
    Areas {
        flow,
        channel,
        help,
    }
}

pub fn run_list_areas(area: Rect) -> Areas {
    let [body, help] = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(1), Constraint::Length(1)])
        .areas(area);
    let [flow, channel] = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Ratio(2, 3), Constraint::Ratio(1, 3)])
        .areas(body);
    Areas {
        flow,
        channel,
        help,
    }
}

#[cfg(test)]
mod tests {
    use super::{areas, run_list_areas};
    use ratatui::layout::Rect;

    #[test]
    fn activity_pane_uses_one_quarter_of_the_window_width() {
        let areas = areas(Rect::new(0, 0, 80, 24), None);

        assert_eq!((areas.flow.width, areas.channel.width), (60, 20));
    }

    #[test]
    fn run_list_gives_the_selected_run_summary_one_third_of_the_window() {
        let areas = run_list_areas(Rect::new(0, 0, 80, 24));

        assert_eq!((areas.flow.width, areas.channel.width), (53, 27));
    }

    #[test]
    fn prompt_mode_reserves_a_bordered_input_row() {
        let areas = areas(Rect::new(0, 0, 80, 24), Some(""));

        assert_eq!(areas.help.height, 3);
    }

    #[test]
    fn prompt_grows_as_its_text_wraps() {
        let prompt = "x".repeat(157);
        let areas = areas(Rect::new(0, 0, 80, 24), Some(&prompt));

        assert_eq!(areas.help.height, 5);
    }

    #[test]
    fn prompt_height_is_capped_at_one_third_of_the_window() {
        let prompt = "x".repeat(1_000);
        let areas = areas(Rect::new(0, 0, 80, 24), Some(&prompt));

        assert_eq!(areas.help.height, 8);
    }
}
