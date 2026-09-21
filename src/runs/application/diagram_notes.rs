//! Human notes scoped to a step and the exact reviewed graph snapshot.

use super::{ArtifactRepository, RepositoryError};
use crate::runs::domain::diagram::Graph;
use serde::{Deserialize, Serialize};
use std::{collections::BTreeMap, path::PathBuf};

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Notes {
    snapshots: Vec<Snapshot>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Snapshot {
    graph: Graph,
    notes: BTreeMap<String, String>,
}

impl Notes {
    pub fn note(&self, graph: &Graph, node: &str) -> &str {
        self.snapshots
            .iter()
            .find(|entry| &entry.graph == graph)
            .and_then(|entry| entry.notes.get(node))
            .map_or("", String::as_str)
    }

    pub fn set(&mut self, graph: &Graph, node: &str, text: String) {
        if !graph.nodes.iter().any(|item| item.id == node) {
            return;
        }
        if let Some(snapshot) = self
            .snapshots
            .iter_mut()
            .find(|entry| &entry.graph == graph)
        {
            snapshot.notes.insert(node.into(), text);
        } else {
            self.snapshots.push(Snapshot {
                graph: graph.clone(),
                notes: BTreeMap::from([(node.into(), text)]),
            });
        }
    }

    pub fn feedback(&self, graph: &Graph) -> Option<String> {
        let notes: Vec<_> = graph
            .nodes
            .iter()
            .filter_map(|node| {
                let text = self.note(graph, &node.id);
                (!text.trim().is_empty())
                    .then(|| format!("## {} [{}]\n\n{}", node.label, node.id, text))
            })
            .collect();
        (!notes.is_empty()).then(|| format!(
            "# Diagram review notes: {} ({:?})\n\nApply this human feedback and return the complete updated artifact, including its diagram, for review. Keep stable node IDs. This submission requests changes; it does not approve the workflow step.\n\n{}\n",
            graph.title, graph.status, notes.join("\n\n")
        ))
    }

    pub fn load(
        repo: &impl ArtifactRepository,
        run: &str,
        step: usize,
    ) -> Result<Self, RepositoryError> {
        let name = format!("diagram-notes-{step}.json");
        let text = repo.read(run, &name)?;
        if text.is_empty() {
            return Ok(Self::default());
        }
        serde_json::from_str(&text).map_err(|source| RepositoryError::Json {
            path: PathBuf::from(name),
            source,
        })
    }

    pub fn save(
        &self,
        repo: &impl ArtifactRepository,
        run: &str,
        step: usize,
    ) -> Result<(), RepositoryError> {
        let name = format!("diagram-notes-{step}.json");
        let text = serde_json::to_string_pretty(self).map_err(|source| RepositoryError::Json {
            path: PathBuf::from(&name),
            source,
        })?;
        repo.write(run, &name, &text)
    }
}
