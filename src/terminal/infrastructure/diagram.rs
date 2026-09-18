//! Terminal architecture hierarchy and relationship inspector.

use super::pane;
use crate::{
    runs::domain::diagram::Status,
    terminal::application::{Focus, Model},
};
use ratatui::{
    Frame,
    layout::{Constraint, Layout, Rect},
    style::Style,
    widgets::{Clear, List, ListItem, ListState, Paragraph, Wrap},
};

#[cfg(test)]
mod tests;

pub fn render(frame: &mut Frame<'_>, area: Rect, model: &Model, step: &str) {
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
        Status::Proposed => "Proposed",
        Status::Observed if graph.provenance.is_some() => "Observed · extractor-reported",
        Status::Observed => "Observed · agent-reported",
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
            "{selected_source} · ] next source · o open · R reload"
        ))
        .block(pane::block(&title, focused)),
        header,
    );
    let [tree, details] = if area.width < 90 {
        Layout::vertical([Constraint::Percentage(40), Constraint::Percentage(60)]).areas(content)
    } else {
        Layout::horizontal([Constraint::Percentage(40), Constraint::Percentage(60)]).areas(content)
    };
    let items: Vec<_> = diagram
        .rows
        .iter()
        .filter_map(|(index, depth)| {
            graph.nodes.get(*index).map(|node| {
                ListItem::new(format!("{}{}", "  ".repeat((*depth).min(12)), node.label))
            })
        })
        .collect();
    let mut state = ListState::default().with_selected(Some(diagram.selected));
    frame.render_stateful_widget(
        List::new(items)
            .block(pane::block(
                " Hierarchy · Enter child · Backspace parent ",
                focused,
            ))
            .highlight_symbol("> ")
            .highlight_style(Style::default().bold().cyan()),
        tree,
        &mut state,
    );
    let Some(node) = diagram.selected_node() else {
        return;
    };
    let mut text = format!("{} [{}]\n", node.label, node.id);
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
