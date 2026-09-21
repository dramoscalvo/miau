//! A navigable UML neighborhood. Layout is bounded even for very large artifacts.
use super::super::pane;
use crate::{
    runs::domain::diagram::{EdgeKind, Node, NodeKind, Status},
    terminal::application::{Focus, Model},
};
use ratatui::{
    Frame,
    layout::{Constraint, Layout, Rect},
    style::Style,
    text::Span,
    widgets::{Clear, Paragraph, Wrap},
};
use unicode_width::UnicodeWidthStr;

const PAGE: usize = 4;
const GAP: usize = 26;

struct Stroke {
    x: usize,
    y: usize,
    text: String,
    style: Style,
}

#[derive(Default)]
struct Canvas {
    strokes: Vec<Stroke>,
}

impl Canvas {
    fn text(&mut self, x: usize, y: usize, text: impl Into<String>, style: Style) {
        self.strokes.push(Stroke {
            x,
            y,
            text: text.into(),
            style,
        });
    }

    fn horizontal(&mut self, x: usize, end: usize, y: usize, ch: char, style: Style) {
        self.text(x, y, ch.to_string().repeat(end.saturating_sub(x)), style);
    }

    fn card(
        &mut self,
        node: &Node,
        x: usize,
        y: usize,
        width: usize,
        min_height: usize,
        style: Style,
    ) -> usize {
        let inner = width.saturating_sub(2);
        let stereotype = match node.kind {
            Some(NodeKind::Interface) => "«interface»",
            Some(NodeKind::AbstractClass) => "{abstract}",
            Some(NodeKind::Enum) => "«enumeration»",
            _ => "",
        };
        let mut lines = vec![format!("┌{}┐", "─".repeat(inner))];
        if !stereotype.is_empty() {
            lines.push(super::box_line(stereotype, inner));
        }
        lines.push(super::box_line(&node.label, inner));
        lines.push(format!("├{}┤", "─".repeat(inner)));
        for members in [&node.attributes, &node.operations] {
            match members {
                Some(members) if !members.is_empty() => {
                    for member in members {
                        lines.push(super::box_line(member, inner));
                    }
                }
                Some(_) => lines.push(super::box_line("", inner)),
                None => lines.push(super::box_line("(not recorded)", inner)),
            }
            lines.push(format!("├{}┤", "─".repeat(inner)));
        }
        lines.pop();
        while lines.len() + 1 < min_height {
            lines.push(super::box_line("", inner));
        }
        lines.push(format!("└{}┘", "─".repeat(inner)));
        let height = lines.len();
        for (row, line) in lines.into_iter().enumerate() {
            self.text(x, y + row, line, style);
        }
        height
    }

    /// Clip in display cells, including wide characters crossing either viewport edge.
    fn paint(&self, frame: &mut Frame<'_>, area: Rect, pan_x: usize, pan_y: usize) {
        let buffer = frame.buffer_mut();
        for stroke in &self.strokes {
            if stroke.y < pan_y || stroke.y - pan_y >= usize::from(area.height) {
                continue;
            }
            let mut x = stroke.x;
            let span = Span::raw(stroke.text.as_str());
            for grapheme in span.styled_graphemes(stroke.style) {
                let width = grapheme.symbol.width();
                if width > 0 && x >= pan_x && x - pan_x + width <= usize::from(area.width) {
                    buffer.set_string(
                        area.x + (x - pan_x) as u16,
                        area.y + (stroke.y - pan_y) as u16,
                        grapheme.symbol,
                        grapheme.style,
                    );
                }
                x += width;
            }
        }
    }
}

fn card_width(node: &Node) -> usize {
    node.attributes
        .iter()
        .chain(node.operations.iter())
        .flatten()
        .map(|text| text.width() + 2)
        .chain([node.label.width() + 2])
        .max()
        .unwrap_or(26)
        .clamp(26, 72)
}

/// Diamonds belong to the whole (`from`); triangles point to the supertype (`to`).
fn endpoints(kind: EdgeKind, outgoing: bool) -> (&'static str, &'static str, char) {
    match (kind, outgoing) {
        (EdgeKind::Inheritance, true) => ("", "▷", '─'),
        (EdgeKind::Inheritance, false) => ("◁", "", '─'),
        (EdgeKind::Implements, true) => ("", "▷", '╌'),
        (EdgeKind::Implements, false) => ("◁", "", '╌'),
        (EdgeKind::Dependency, true) => ("", ">", '╌'),
        (EdgeKind::Dependency, false) => ("<", "", '╌'),
        (EdgeKind::Aggregation, true) => ("◇", "", '─'),
        (EdgeKind::Aggregation, false) => ("", "◇", '─'),
        (EdgeKind::Composition, true) => ("◆", "", '─'),
        (EdgeKind::Composition, false) => ("", "◆", '─'),
        (EdgeKind::Association, _) => ("", "", '─'),
    }
}

pub fn render(frame: &mut Frame<'_>, area: Rect, model: &Model, step: &str) {
    frame.render_widget(Clear, area);
    let diagram = &model.diagram;
    let (Some(graph), Some(node)) = (&diagram.graph, diagram.selected_node()) else {
        return;
    };
    let focused = model.focus == Focus::Flow;
    let stage = match graph.status {
        Status::Proposed => "Before implementation · Proposed",
        Status::Observed if graph.provenance.is_some() => {
            "Implementation snapshot · Observed · extractor-reported"
        }
        Status::Observed => "Implementation snapshot · Observed · agent-reported",
    };
    let [header, canvas_area, details] = Layout::vertical([
        Constraint::Length(4),
        Constraint::Min(1),
        Constraint::Length((area.height / 3).min(10)),
    ])
    .areas(area);
    let relations = diagram.relations();
    let page = diagram.relation_selected;
    let title = format!(" UML classes · {step} · {stage} · {} ", graph.title);
    let controls = format!(
        "↑/↓ j/k class {}/{} · }} relationship {}/{} · Enter follow · Backspace back\nH/J/K/L pan · Home reset · ] source · o open · R reload · n note · S send notes",
        diagram.selected + 1,
        diagram.rows.len(),
        if relations.is_empty() {
            0
        } else {
            diagram.relation_selected + 1
        },
        relations.len(),
    );
    frame.render_widget(
        Paragraph::new(controls).block(pane::block(&title, focused)),
        header,
    );
    let selected_style = Style::default().cyan().bold();
    let mut canvas = Canvas::default();
    let left_width = card_width(node);
    let right_x = left_width + GAP;
    canvas.card(node, 0, 0, left_width, PAGE + 5, selected_style);
    let mut y = 0;
    for (slot, edge) in relations.iter().skip(page).take(PAGE).enumerate() {
        let outgoing = edge.from == node.id;
        let other_id = if outgoing { &edge.to } else { &edge.from };
        let Some(other) = graph.nodes.iter().find(|other| &other.id == other_id) else {
            continue;
        };
        let active = page + slot == diagram.relation_selected;
        let style = if active {
            Style::default().yellow().bold()
        } else {
            Style::default().dim()
        };
        let height = canvas.card(other, right_x, y, card_width(other), 8, style);
        let start_y = 3 + slot;
        let end_y = y + 3;
        let lane = left_width + 2 + (PAGE - 1 - slot) * 2;
        let (left, right, line) = endpoints(edge.kind, outgoing);
        canvas.text(left_width - 1, start_y, "┼", style);
        canvas.horizontal(left_width, lane, start_y, line, style);
        if start_y != end_y {
            let vertical = if line == '╌' { '╎' } else { '│' };
            for row in start_y.min(end_y) + 1..start_y.max(end_y) {
                canvas.text(lane, row, vertical.to_string(), style);
            }
            canvas.text(
                lane,
                start_y,
                if end_y > start_y { "┐" } else { "┘" },
                style,
            );
            canvas.text(lane, end_y, if end_y > start_y { "└" } else { "┌" }, style);
        } else {
            canvas.text(lane, start_y, line.to_string(), style);
        }
        canvas.horizontal(lane + 1, right_x, end_y, line, style);
        canvas.text(left_width, start_y, left, style);
        if !right.is_empty() {
            canvas.text(right_x - 1, end_y, right, style);
        }
        canvas.text(right_x, end_y, "┼", style);
        canvas.text(
            left_width + 10,
            end_y.saturating_sub(1),
            edge.kind.label(),
            style,
        );
        y += height + 2;
    }
    let viewport_title = format!(
        " {} · neighbors {}–{} of {} · pan {},{} ",
        node.label,
        if relations.is_empty() { 0 } else { page + 1 },
        (page + PAGE).min(relations.len()),
        relations.len(),
        diagram.pan_x,
        diagram.pan_y
    );
    let viewport = pane::block(&viewport_title, focused);
    let inner = viewport.inner(canvas_area);
    frame.render_widget(viewport, canvas_area);
    canvas.paint(frame, inner, diagram.pan_x, diagram.pan_y);

    let kind = node.kind.map_or("Class", |kind| kind.label());
    let mut text = format!("{} [{}] · {kind}\n", node.label, node.id);
    if let Some(source) = diagram.selected_source() {
        text.push_str(&format!("Source: {}:{}\n", source.file, source.line));
    }
    let note = model.diagram_notes.note(graph, &node.id);
    if !note.trim().is_empty() {
        text.push_str(&format!("[note] {note}\n"));
    }
    if let Some(edge) = relations.get(diagram.relation_selected) {
        text.push_str(&format!(
            "{} — {} → {}",
            edge.from,
            edge.kind.label(),
            edge.to
        ));
        if let Some(label) = &edge.label {
            text.push_str(&format!(" · {label}"));
        }
        if let Some(source) = &edge.source {
            text.push_str(&format!(" · {}:{}", source.file, source.line));
        }
        text.push('\n');
    }
    for (label, members) in [
        ("Attributes", &node.attributes),
        ("Operations", &node.operations),
    ] {
        text.push_str(&format!("{label}:\n"));
        match members {
            Some(members) => {
                for member in members {
                    text.push_str(&format!("{member}\n"));
                }
            }
            None => {
                text.push_str("Not recorded in this artifact; request an updated class diagram.\n")
            }
        }
    }
    if let Some(metadata) = &graph.provenance {
        text.push_str(&format!(
            "Extractor: {} v{} · TypeScript {}\n",
            metadata.extractor, metadata.version, metadata.typescript
        ));
        for warning in &metadata.warnings {
            text.push_str(&format!("Warning: {warning}\n"));
        }
    }
    text.push_str("UML: ─▷ inherits · ╌▷ implements · ╌> depends · ─ associates · ◇ aggregates · ◆ composes\n");
    frame.render_widget(
        Paragraph::new(text)
            .wrap(Wrap { trim: false })
            .scroll((model.flow_scroll, 0))
            .block(pane::block(
                " Class details · PgUp/PgDn · v document views ",
                false,
            )),
        details,
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn canvas_pan_preserves_wide_text_and_combining_marks() {
        use ratatui::{Terminal, backend::TestBackend};
        let mut canvas = Canvas::default();
        canvas.text(0, 0, "猫e\u{301}", Style::default());
        let mut terminal = Terminal::new(TestBackend::new(6, 3)).unwrap();
        terminal
            .draw(|frame| canvas.paint(frame, Rect::new(1, 1, 3, 1), 0, 0))
            .unwrap();
        assert_eq!(terminal.backend().buffer()[(1, 1)].symbol(), "猫");
        assert_eq!(terminal.backend().buffer()[(3, 1)].symbol(), "e\u{301}");
        terminal
            .draw(|frame| canvas.paint(frame, Rect::new(1, 1, 3, 1), 2, 0))
            .unwrap();
        assert_eq!(terminal.backend().buffer()[(1, 1)].symbol(), "e\u{301}");
        assert_eq!(terminal.backend().buffer()[(0, 1)].symbol(), " ");
    }

    #[test]
    fn uml_arrow_direction_and_diamond_ownership_are_preserved() {
        assert_eq!(endpoints(EdgeKind::Implements, true), ("", "▷", '╌'));
        assert_eq!(endpoints(EdgeKind::Implements, false), ("◁", "", '╌'));
        assert_eq!(endpoints(EdgeKind::Inheritance, true), ("", "▷", '─'));
        assert_eq!(endpoints(EdgeKind::Dependency, false), ("<", "", '╌'));
        assert_eq!(endpoints(EdgeKind::Composition, true), ("◆", "", '─'));
        assert_eq!(endpoints(EdgeKind::Aggregation, false), ("", "◇", '─'));
        assert_eq!(endpoints(EdgeKind::Association, true), ("", "", '─'));
    }
}
