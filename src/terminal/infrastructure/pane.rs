use ratatui::{
    style::Style,
    text::Span,
    widgets::{Block, BorderType, Borders},
};

pub fn border_style(focused: bool) -> Style {
    if focused {
        Style::default().magenta().bold()
    } else {
        Style::default().dim()
    }
}

pub fn block<'a>(title: &'a str, focused: bool) -> Block<'a> {
    let title_style = if focused {
        Style::default().magenta().bold().reversed()
    } else {
        Style::default().dim()
    };
    Block::default()
        .title(Span::styled(title, title_style))
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(border_style(focused))
}

#[cfg(test)]
mod tests {
    use super::{block, border_style};
    use ratatui::{buffer::Buffer, layout::Rect, style::Style, widgets::Widget};

    #[test]
    fn pane_block_has_rounded_corners() {
        let area = Rect::new(0, 0, 8, 3);
        let mut buffer = Buffer::empty(area);

        block(" pane ", false).render(area, &mut buffer);

        assert_eq!(buffer[(0, 0)].symbol(), "╭");
    }

    #[test]
    fn focused_pane_border_is_bold_magenta() {
        assert_eq!(border_style(true), Style::default().magenta().bold());
    }

    #[test]
    fn inactive_pane_border_is_dimmed() {
        assert_eq!(border_style(false), Style::default().dim());
    }
}
