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
    /// Node index and depth at the current drill-down level.
    pub rows: Vec<(usize, usize)>,
    pub selected: usize,
    pub scope: Option<String>,
    pub source_selected: usize,
}

impl Diagram {
    pub fn load(&mut self, artifact: &str) {
        let selected = self.selected_node().map(|node| node.id.clone());
        let old_source = self.selected_source().cloned();
        *self = Self::default();
        match parse(artifact) {
            Ok(Some(graph)) => {
                self.scope = selected.as_ref().and_then(|id| {
                    graph
                        .nodes
                        .iter()
                        .find(|node| &node.id == id)
                        .and_then(|node| node.parent.clone())
                });
                self.graph = Some(graph);
                self.rebuild_rows(selected.as_deref());
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

    fn rebuild_rows(&mut self, selected: Option<&str>) {
        let Some(graph) = &self.graph else { return };
        self.rows = graph
            .nodes
            .iter()
            .enumerate()
            .filter(|(_, node)| node.parent == self.scope)
            .map(|(index, _)| (index, 0))
            .collect();
        self.selected = self
            .rows
            .iter()
            .position(|(index, _)| Some(graph.nodes[*index].id.as_str()) == selected)
            .unwrap_or(0);
        self.source_selected = 0;
    }

    pub fn child(&mut self) {
        let Some(node) = self.selected_node() else {
            return;
        };
        let Some(graph) = &self.graph else { return };
        if graph
            .nodes
            .iter()
            .any(|child| child.parent.as_deref() == Some(&node.id))
        {
            self.scope = Some(node.id.clone());
            self.rebuild_rows(None);
        }
    }

    pub fn parent(&mut self) {
        let Some(parent) = self.scope.clone() else {
            return;
        };
        self.scope = self.graph.as_ref().and_then(|graph| {
            graph
                .nodes
                .iter()
                .find(|node| node.id == parent)
                .and_then(|node| node.parent.clone())
        });
        self.rebuild_rows(Some(&parent));
    }

    pub fn next_source(&mut self) {
        let count = self.sources().len();
        if count > 0 {
            self.source_selected = (self.source_selected + 1) % count;
        }
    }
}
