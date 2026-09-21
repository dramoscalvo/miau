//! Pure class/relationship navigation, legacy hierarchy navigation, and source-file lookup.

use crate::runs::{
    application::diagram::parse,
    domain::diagram::{Edge, Graph, Node, Source},
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
    /// Classifier indices in UML mode, or node indices/depth at the legacy hierarchy level.
    pub rows: Vec<(usize, usize)>,
    pub selected: usize,
    pub scope: Option<String>,
    pub source_selected: usize,
    pub relation_selected: usize,
    pub pan_x: usize,
    pub pan_y: usize,
    history: Vec<String>,
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

    pub fn is_uml(&self) -> bool {
        self.graph.as_ref().is_some_and(|graph| {
            graph
                .nodes
                .iter()
                .any(|node| node.kind.is_some_and(|kind| kind.is_classifier()))
        })
    }

    pub fn relations(&self) -> Vec<&Edge> {
        let (Some(graph), Some(node)) = (&self.graph, self.selected_node()) else {
            return Vec::new();
        };
        graph
            .edges
            .iter()
            .filter(|edge| edge.from == node.id || edge.to == node.id)
            .collect()
    }

    pub fn next_relation(&mut self) {
        let count = self.relations().len();
        if count > 0 {
            self.relation_selected = (self.relation_selected + 1) % count;
            self.pan_x = 0;
            self.pan_y = 0;
        }
    }

    pub fn pan(&mut self, x: i16, y: i16) {
        self.pan_x = self.pan_x.saturating_add_signed(isize::from(x)).min(256);
        self.pan_y = self.pan_y.saturating_add_signed(isize::from(y));
    }

    fn reset_view(&mut self) {
        self.source_selected = 0;
        self.relation_selected = 0;
        self.pan_x = 0;
        self.pan_y = 0;
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
        self.reset_view();
    }

    fn rebuild_rows(&mut self, selected: Option<&str>) {
        let Some(graph) = &self.graph else { return };
        let uml = self.is_uml();
        self.rows = graph
            .nodes
            .iter()
            .enumerate()
            .filter(|(_, node)| {
                if uml {
                    node.kind.is_some_and(|kind| kind.is_classifier())
                } else {
                    node.parent == self.scope
                }
            })
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
        if self.is_uml() {
            let Some(node) = self.selected_node() else {
                return;
            };
            let Some(edge) = self.relations().get(self.relation_selected).copied() else {
                return;
            };
            let target = if edge.from == node.id {
                &edge.to
            } else {
                &edge.from
            };
            let Some(graph) = &self.graph else { return };
            let Some(index) = self
                .rows
                .iter()
                .position(|(index, _)| graph.nodes[*index].id == *target)
            else {
                return;
            };
            self.history.push(node.id.clone());
            self.selected = index;
            self.reset_view();
            return;
        }
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
        if self.is_uml() {
            if let Some(id) = self.history.pop() {
                self.rebuild_rows(Some(&id));
                self.reset_view();
            }
            return;
        }
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
