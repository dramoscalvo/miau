//! Pure hierarchy navigation and the source-file lookup port.

use crate::runs::{
    application::diagram::parse,
    domain::diagram::{Graph, Node, Source},
};
use std::{
    collections::HashSet,
    io,
    path::{Path, PathBuf},
};

pub trait SourceRepository {
    fn resolve(&self, project: &Path, source: &Source) -> io::Result<PathBuf>;
}

#[derive(Debug, Default, Clone)]
pub struct Diagram {
    pub graph: Option<Graph>,
    pub error: Option<String>,
    /// Node index and hierarchy depth in display order.
    pub rows: Vec<(usize, usize)>,
    pub selected: usize,
    pub source_selected: usize,
}

impl Diagram {
    pub fn load(&mut self, artifact: &str) {
        let selected = self.selected_node().map(|node| node.id.clone());
        let old_source = self.selected_source().cloned();
        *self = Self::default();
        match parse(artifact) {
            Ok(Some(graph)) => {
                let mut stack: Vec<_> = graph
                    .nodes
                    .iter()
                    .enumerate()
                    .rev()
                    .filter(|(_, node)| node.parent.is_none())
                    .map(|(index, _)| (index, 0))
                    .collect();
                while let Some((index, depth)) = stack.pop() {
                    self.rows.push((index, depth));
                    stack.extend(
                        graph
                            .nodes
                            .iter()
                            .enumerate()
                            .rev()
                            .filter(|(_, node)| {
                                node.parent.as_deref() == Some(&graph.nodes[index].id)
                            })
                            .map(|(index, _)| (index, depth + 1)),
                    );
                }
                self.selected = self
                    .rows
                    .iter()
                    .position(|(index, _)| Some(&graph.nodes[*index].id) == selected.as_ref())
                    .unwrap_or(0);
                self.graph = Some(graph);
                self.source_selected = self
                    .sources()
                    .iter()
                    .position(|source| Some(*source) == old_source.as_ref())
                    .unwrap_or(0);
            }
            Ok(None) => {}
            Err(error) => self.error = Some(error),
        }
    }

    pub fn available(&self) -> bool {
        self.graph.is_some() || self.error.is_some()
    }

    pub fn selected_node(&self) -> Option<&Node> {
        let (index, _) = self.rows.get(self.selected)?;
        self.graph.as_ref()?.nodes.get(*index)
    }

    pub fn sources(&self) -> Vec<&Source> {
        let (Some(graph), Some(node)) = (&self.graph, self.selected_node()) else {
            return Vec::new();
        };
        let mut sources = Vec::new();
        let mut seen = HashSet::new();
        for source in node.source.iter().chain(
            graph
                .edges
                .iter()
                .filter(|edge| edge.from == node.id || edge.to == node.id)
                .filter_map(|edge| edge.source.as_ref()),
        ) {
            if seen.insert(source) {
                sources.push(source);
            }
        }
        sources
    }

    pub fn selected_source(&self) -> Option<&Source> {
        self.sources().get(self.source_selected).copied()
    }

    pub fn move_selection(&mut self, forward: bool) {
        self.selected = if forward {
            self.selected
                .saturating_add(1)
                .min(self.rows.len().saturating_sub(1))
        } else {
            self.selected.saturating_sub(1)
        };
        self.source_selected = 0;
    }

    pub fn child(&mut self) {
        if let (Some((_, depth)), Some((_, next_depth))) = (
            self.rows.get(self.selected),
            self.rows.get(self.selected + 1),
        ) && next_depth > depth
        {
            self.move_selection(true);
        }
    }

    pub fn parent(&mut self) {
        let Some(parent) = self.selected_node().and_then(|node| node.parent.as_deref()) else {
            return;
        };
        let Some(graph) = &self.graph else { return };
        if let Some(index) = self
            .rows
            .iter()
            .position(|(index, _)| graph.nodes[*index].id == parent)
        {
            self.selected = index;
            self.source_selected = 0;
        }
    }

    pub fn next_source(&mut self) {
        let count = self.sources().len();
        if count > 0 {
            self.source_selected = (self.source_selected + 1) % count;
        }
    }
}
