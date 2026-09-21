//! Extract and validate optional diagram data without interpreting workflow decisions.

use std::collections::{HashMap, HashSet};

use crate::runs::domain::diagram::{Graph, Status};

const MAX_BYTES: usize = 1_048_576;

/// Only a top-level miau-graph fence inside Handoff is a diagram.
pub fn parse(artifact: &str) -> Result<Option<Graph>, String> {
    let mut handoff = false;
    let mut fence = None;
    let mut collecting = false;
    let mut found = false;
    let mut json = String::new();
    for line in artifact.lines() {
        let trimmed = line.trim();
        if let Some((marker, length)) = fence {
            let count = trimmed.chars().take_while(|ch| *ch == marker).count();
            if count >= length && trimmed.chars().skip(count).all(char::is_whitespace) {
                fence = None;
                collecting = false;
            } else if collecting {
                if json.len() + line.len() + 1 > MAX_BYTES {
                    return Err("Diagram exceeds the 1 MiB limit".into());
                }
                json.push_str(line);
                json.push('\n');
            }
            continue;
        }
        if trimmed.starts_with("# ") {
            handoff = trimmed == "# Handoff";
        }
        if let Some(marker @ ('`' | '~')) = trimmed.chars().next() {
            let length = trimmed.chars().take_while(|ch| *ch == marker).count();
            if length >= 3 {
                fence = Some((marker, length));
                let info: String = trimmed.chars().skip(length).collect();
                collecting = handoff && info.trim() == "miau-graph";
                if collecting {
                    if found {
                        return Err("Only one miau-graph block is allowed".into());
                    }
                    found = true;
                }
            }
        }
    }
    if collecting {
        return Err("Unclosed miau-graph fence".into());
    }
    if !found {
        return Ok(None);
    }
    let graph: Graph =
        serde_json::from_str(&json).map_err(|error| format!("Invalid diagram: {error}"))?;
    validate(&graph)?;
    Ok(Some(graph))
}

fn validate(graph: &Graph) -> Result<(), String> {
    if let Some(provenance) = &graph.provenance
        && (graph.status != Status::Observed
            || provenance.extractor.trim().is_empty()
            || provenance.version == 0
            || provenance.typescript.trim().is_empty()
            || provenance.scope.trim().is_empty()
            || !crate::runs::domain::diagram::Source {
                file: provenance.project.clone(),
                line: 1,
            }
            .is_valid()
            || provenance.fingerprint.len() != 64
            || !provenance
                .fingerprint
                .bytes()
                .all(|byte| byte.is_ascii_hexdigit()))
    {
        return Err("Invalid diagram provenance".into());
    }
    if !matches!(graph.version, 1 | 2) {
        return Err(format!("Unsupported diagram version: {}", graph.version));
    }
    if graph.title.trim().is_empty() || graph.nodes.is_empty() {
        return Err("Diagram needs a title and at least one node".into());
    }
    if graph.nodes.len() > 1_000 || graph.edges.len() > 5_000 {
        return Err("Diagram exceeds 1000 nodes or 5000 edges".into());
    }
    let mut nodes = HashMap::new();
    for node in &graph.nodes {
        if node.id.trim().is_empty() || node.label.trim().is_empty() {
            return Err("Diagram node needs a nonblank id and label".into());
        }
        if node.attributes.is_some() || node.operations.is_some() {
            if graph.version < 2 || !node.kind.is_some_and(|kind| kind.is_classifier()) {
                return Err("UML members require a version 2 classifier node".into());
            }
            for member in node
                .attributes
                .iter()
                .chain(node.operations.iter())
                .flatten()
            {
                if member.trim().is_empty() || member.chars().any(char::is_control) {
                    return Err("UML member signatures must be nonblank single lines".into());
                }
            }
        }
        if nodes.insert(node.id.as_str(), node).is_some() {
            return Err(format!("Duplicate diagram node: {}", node.id));
        }
    }
    for node in &graph.nodes {
        let mut ancestors = HashSet::new();
        ancestors.insert(node.id.as_str());
        let mut parent = node.parent.as_deref();
        while let Some(id) = parent {
            if !ancestors.insert(id) {
                return Err(format!("Hierarchy cycle at {id}"));
            }
            parent = nodes
                .get(id)
                .ok_or_else(|| format!("Unknown parent: {id}"))?
                .parent
                .as_deref();
        }
    }
    for edge in &graph.edges {
        if !nodes.contains_key(edge.from.as_str()) || !nodes.contains_key(edge.to.as_str()) {
            return Err(format!(
                "Unknown edge endpoint: {} → {}",
                edge.from, edge.to
            ));
        }
        if graph.status == Status::Observed && edge.source.is_none() {
            return Err("Observed relationships need source references".into());
        }
    }
    for source in graph
        .nodes
        .iter()
        .filter_map(|node| node.source.as_ref())
        .chain(graph.edges.iter().filter_map(|edge| edge.source.as_ref()))
    {
        if !source.is_valid() {
            return Err(format!(
                "Invalid source reference: {}:{}",
                source.file, source.line
            ));
        }
    }
    Ok(())
}
