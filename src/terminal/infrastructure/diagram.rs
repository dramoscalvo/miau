//! Terminal architecture hierarchy and relationship inspector.

use super::pane;
mod uml;
use crate::{
    runs::domain::diagram::Status,
    terminal::application::{Focus, Model},
};
use ratatui::{
    Frame,
    layout::{Constraint, Layout, Rect},
    style::Style,
    text::{Line, Span},
    widgets::{Clear, List, ListItem, ListState, Paragraph, Wrap},
};

#[cfg(test)]
mod notes_tests;
#[cfg(test)]
mod tests;

use unicode_width::UnicodeWidthStr;

fn box_line(text: &str, width: usize) -> String {
    let mut clipped = String::new();
    let mut used = 0;
    let span = Span::raw(text);
    for grapheme in span.styled_graphemes(Style::default()) {
        let cells = grapheme.symbol.width();
        if used + cells > width {
            break;
        }
        clipped.push_str(grapheme.symbol);
        used += cells;
    }
    format!(
        "│{}{}│",
        clipped,
        " ".repeat(width.saturating_sub(clipped.width()))
    )
}

pub fn render(frame: &mut Frame<'_>, area: Rect, model: &Model, step: &str) {
    if model.diagram.is_uml() {
        uml::render(frame, area, model, step);
        return;
    }
    frame.render_widget(Clear, area);
    let diagram = &model.diagram;
    let focused = model.focus == Focus::Flow;
    let Some(graph) = &diagram.graph else {
        let message = diagram
            .error
            .as_deref()
            .unwrap_or("No diagram in this artifact. R reload · v next view");
        frame.render_widget(
            Paragraph::new(message)
                .wrap(Wrap { trim: false })
                .block(pane::block(" Diagram · v document views ", focused)),
            area,
        );
        return;
    };
    let status = match graph.status {
        Status::Proposed => "Before implementation · Proposed",
        Status::Observed if graph.provenance.is_some() => {
            "Implementation snapshot · Observed · extractor-reported"
        }
        Status::Observed => "Implementation snapshot · Observed · agent-reported",
    };
    let title = format!(" Diagram · {step} · {status} · {} ", graph.title);
    let [header, content] =
        Layout::vertical([Constraint::Length(3), Constraint::Min(1)]).areas(area);
    let selected_source = diagram
        .selected_source()
        .map_or("No source reference".into(), |source| {
            format!("{}:{}", source.file, source.line)
        });
    frame.render_widget(
        Paragraph::new(format!(
            "{selected_source} · ] source · o open · R reload · n note · S send notes"
        ))
        .block(pane::block(&title, focused)),
        header,
    );
    let [tree, details] = if area.width < 90 {
        Layout::vertical([Constraint::Percentage(40), Constraint::Percentage(60)]).areas(content)
    } else {
        Layout::horizontal([Constraint::Percentage(40), Constraint::Percentage(60)]).areas(content)
    };
    let box_width = usize::from(tree.width.saturating_sub(6));
    let items: Vec<_> = diagram
        .rows
        .iter()
        .filter_map(|(index, _)| {
            graph.nodes.get(*index).map(|node| {
                let children = graph
                    .nodes
                    .iter()
                    .filter(|child| child.parent.as_deref() == Some(&node.id))
                    .count();
                let note = if model.diagram_notes.note(graph, &node.id).trim().is_empty() {
                    ""
                } else {
                    " [note]"
                };
                let kind = node.kind.map_or("Component", |kind| kind.label());
                let hint = if children > 0 {
                    format!("{kind} · {children} children{note}")
                } else {
                    format!("{kind}{note}")
                };
                ListItem::new(vec![
                    Line::from(format!("╭{}╮", "─".repeat(box_width))),
                    Line::from(box_line(&node.label, box_width)),
                    Line::from(box_line(&hint, box_width)),
                    Line::from(format!("╰{}╯", "─".repeat(box_width))),
                ])
            })
        })
        .collect();
    let scope = diagram
        .scope
        .as_ref()
        .and_then(|id| graph.nodes.iter().find(|node| &node.id == id))
        .map_or("Overview", |node| node.label.as_str());
    let scope_title = format!(" {scope} · Enter in · Backspace out ");
    let mut state = ListState::default().with_selected(Some(diagram.selected));
    frame.render_stateful_widget(
        List::new(items)
            .block(pane::block(&scope_title, focused))
            .highlight_symbol("> ")
            .highlight_style(Style::default().bold().cyan()),
        tree,
        &mut state,
    );
    let Some(node) = diagram.selected_node() else {
        return;
    };
    let mut text = format!("{} [{}]\n", node.label, node.id);
    let note = model.diagram_notes.note(graph, &node.id);
    if !note.trim().is_empty() {
        text.push_str(&format!(
            "\nYour note (saved; S requests changes):\n{note}\n\n"
        ));
    }
    if let Some(kind) = node.kind {
        text.push_str(kind.label());
        text.push('\n');
    }
    if let Some(metadata) = &graph.provenance {
        text.push_str(&format!(
            "Extractor: {} v{} · TypeScript {}\nScope: {}\n",
            metadata.extractor, metadata.version, metadata.typescript, metadata.scope
        ));
        for warning in &metadata.warnings {
            text.push_str(&format!("Warning: {warning}\n"));
        }
    }
    if let Some(parent) = &node.parent {
        text.push_str(&format!("Parent: {parent}\n"));
    }
    for (heading, incoming) in [("Outgoing", false), ("Incoming", true)] {
        text.push_str(&format!("\n{heading}:\n"));
        let mut count = 0;
        for edge in &graph.edges {
            if (if incoming { &edge.to } else { &edge.from }) != &node.id {
                continue;
            }
            let other = if incoming { &edge.from } else { &edge.to };
            let label = graph
                .nodes
                .iter()
                .find(|node| &node.id == other)
                .map_or(other.as_str(), |node| node.label.as_str());
            text.push_str(&format!("{} {} [{}]", edge.kind.label(), label, other));
            if let Some(label) = &edge.label {
                text.push_str(&format!(" · {label}"));
            }
            if let Some(source) = &edge.source {
                text.push_str(&format!(" · {}:{}", source.file, source.line));
            }
            text.push('\n');
            count += 1;
        }
        if count == 0 {
            text.push_str("None\n");
        }
    }
    if let Some(metadata) = &graph.provenance {
        text.push_str(&format!("\nConfig: {}\nInput SHA-256: {}\nRegenerate to check current source; metadata is not independently verified.\n", metadata.project, metadata.fingerprint));
    } else if graph.status == Status::Observed {
        text.push_str("\nRelationships are reported by the agent, not independently verified.\n");
    }
    frame.render_widget(
        Paragraph::new(text)
            .wrap(Wrap { trim: false })
            .scroll((model.flow_scroll, 0))
            .block(pane::block(" Relationships · PgUp/PgDn scroll ", false)),
        details,
    );
}
