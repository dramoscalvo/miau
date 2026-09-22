//! A stable whole-graph UML canvas; selection highlights without filtering the graph.
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

const GAP: usize = 26;

struct Stroke {
    x: usize,
    y: usize,
    end_y: usize,
    text: String,
    style: Style,
}

#[derive(Default)]
struct Canvas {
    strokes: Vec<Stroke>,
}

impl Canvas {
    fn vertical(&mut self, x: usize, start: usize, end: usize, line: char, style: Style) {
        self.strokes.push(Stroke {
            x,
            y: start.min(end),
            end_y: start.max(end),
            text: if line == '╌' { "╎" } else { "│" }.into(),
            style,
        });
    }
    fn text(&mut self, x: usize, y: usize, text: impl Into<String>, style: Style) {
        self.strokes.push(Stroke {
            x,
            y,
            end_y: y,
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
            // Clip long connectors before painting; large graphs must not allocate per cell.
            let visible_end = stroke
                .end_y
                .saturating_add(1)
                .min(pan_y.saturating_add(usize::from(area.height)));
            for y in stroke.y.max(pan_y)..visible_end {
                let mut x = stroke.x;
                let span = Span::raw(stroke.text.as_str());
                for grapheme in span.styled_graphemes(stroke.style) {
                    let width = grapheme.symbol.width();
                    if width > 0 && x >= pan_x && x - pan_x + width <= usize::from(area.width) {
                        buffer.set_string(
                            area.x + (x - pan_x) as u16,
                            area.y + (y - pan_y) as u16,
                            grapheme.symbol,
                            grapheme.style,
                        );
                    }
                    x += width;
                }
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
    let mut canvas = Canvas::default();
    // Two stable columns keep the layout independent of selection and viewport size.
    let width = diagram
        .rows
        .iter()
        .map(|(index, _)| card_width(&graph.nodes[*index]))
        .max()
        .unwrap_or(26);
    let mut positions = std::collections::HashMap::new();
    let mut y = 1;
    for row in diagram.rows.chunks(2) {
        let mut row_height = 0;
        for (column, (index, _)) in row.iter().enumerate() {
            let item = &graph.nodes[*index];
            let x = GAP + column * (width + GAP);
            let style = if item.id == node.id {
                Style::default().cyan().bold()
            } else {
                Style::default()
            };
            let height = canvas.card(item, x, y, width, 8, style);
            positions.insert(item.id.as_str(), (x, y, height));
            row_height = row_height.max(height);
        }
        for (index, _) in row {
            if let Some(position) = positions.get_mut(graph.nodes[*index].id.as_str()) {
                position.2 = row_height;
            }
        }
        y += row_height + 4;
    }
    let active = relations.get(diagram.relation_selected).copied();
    // Draw the selected connection last so its full route remains traceable at crossings.
    let mut edges: Vec<_> = graph.edges.iter().collect();
    edges.sort_by_key(|edge| Some(*edge) == active);
    for (index, edge) in edges.into_iter().enumerate() {
        let (Some(&(from_x, from_y, from_height)), Some(&(to_x, to_y, _))) = (
            positions.get(edge.from.as_str()),
            positions.get(edge.to.as_str()),
        ) else {
            continue;
        };
        let style = if Some(edge) == active {
            Style::default().yellow().bold()
        } else {
            Style::default().dim()
        };
        let outgoing = from_x < to_x;
        let (left, right, line) = endpoints(edge.kind, outgoing);
        if from_y == to_y && from_x != to_x {
            let x = from_x.min(to_x) + width;
            let end = from_x.max(to_x);
            let port_y = from_y + 3;
            canvas.horizontal(x, end, port_y, line, style);
            canvas.text(x - 1, port_y, "┼", style);
            canvas.text(end, port_y, "┼", style);
            canvas.text(x, port_y, left, style);
            if !right.is_empty() {
                canvas.text(end - 1, port_y, right, style);
            }
        } else {
            // Route through column gutters and the space below the source card.
            // Both endpoints face right, including self-links and reverse relationships.
            let start = from_x + width;
            let end = to_x + width;
            let start_y = from_y + 3;
            let end_y = to_y + if edge.from == edge.to { 5 } else { 3 };
            let source_lane = start + 3 + index % 8;
            let target_lane = end + 13 + index % 8;
            let channel_y = from_y + from_height + 1;
            let (diamond, _, _) = endpoints(edge.kind, true);
            let (arrow, _, _) = endpoints(edge.kind, false);
            canvas.horizontal(start, source_lane + 1, start_y, line, style);
            canvas.vertical(source_lane, start_y, channel_y, line, style);
            canvas.horizontal(
                source_lane.min(target_lane),
                source_lane.max(target_lane) + 1,
                channel_y,
                line,
                style,
            );
            canvas.vertical(target_lane, channel_y, end_y, line, style);
            canvas.horizontal(end, target_lane + 1, end_y, line, style);
            for (x, row) in [
                (source_lane, start_y),
                (source_lane, channel_y),
                (target_lane, channel_y),
                (target_lane, end_y),
            ] {
                canvas.text(x, row, "┼", style);
            }
            canvas.text(start - 1, start_y, "┼", style);
            canvas.text(end - 1, end_y, "┼", style);
            canvas.text(start, start_y, diamond, style);
            canvas.text(end, end_y, arrow, style);
        }
    }
    let viewport_title = format!(
        " Whole change · {} classes · {} relationships · pan {},{} ",
        diagram.rows.len(),
        graph.edges.len(),
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
