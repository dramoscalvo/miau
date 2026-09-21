//! Generate review artifacts through a language extraction port.

use crate::runs::domain::diagram::{Graph, Status};
use std::{io, path::Path};
use thiserror::Error;

pub struct ExtractionRequest<'a> {
    pub project: &'a Path,
    pub root: &'a Path,
    pub title: &'a str,
    pub scope: ExtractionScope,
    pub changed_files: Option<&'a [String]>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExtractionScope {
    Modules,
    Types,
}

impl ExtractionScope {
    pub fn argument(self) -> &'static str {
        match self {
            Self::Modules => "modules",
            Self::Types => "types",
        }
    }
}

#[derive(Debug, Error)]
pub enum ExtractionError {
    #[error("diagram extraction I/O failed: {0}")]
    Io(#[from] io::Error),
    #[error("diagram extractor returned invalid JSON: {0}")]
    Json(#[from] serde_json::Error),
    #[error("{0}")]
    Invalid(String),
}

pub trait DiagramExtractor {
    fn extract(&self, request: &ExtractionRequest<'_>) -> Result<Graph, ExtractionError>;
}

pub fn generate(
    extractor: &impl DiagramExtractor,
    request: &ExtractionRequest<'_>,
) -> Result<String, ExtractionError> {
    let graph = extractor.extract(request)?;
    if graph.status != Status::Observed || graph.provenance.is_none() {
        return Err(ExtractionError::Invalid(
            "Extractor must return an observed graph with provenance".into(),
        ));
    }
    let json = serde_json::to_string_pretty(&graph)?;
    let warnings = graph
        .provenance
        .as_ref()
        .map_or(0, |metadata| metadata.warnings.len());
    let (summary, relationship, limitation) = match request.scope {
        ExtractionScope::Modules => (
            "Observed TypeScript module dependencies extracted from source.",
            "dependencies",
            "This is a module dependency graph, not a class model or a type-check result.",
        ),
        ExtractionScope::Types => (
            "Observed TypeScript semantic type relationships extracted from source.",
            "relationships",
            "This is a compiler-derived type graph, not a runtime object graph, call graph, ownership model, or complete UML interpretation.",
        ),
    };
    let artifact = format!(
        "# Review\n{summary}\n\
         {} nodes; {} {relationship}; {warnings} extraction warnings.\n\
         Review the Diagram view, source evidence, and extraction warnings before deciding.\n\
         {limitation}\n\n\
         # Handoff\nGenerated graph; regenerate from source instead of editing its relationships.\n\
         ```miau-graph\n{json}\n```\n",
        graph.nodes.len(),
        graph.edges.len(),
    );
    super::diagram::parse(&artifact).map_err(ExtractionError::Invalid)?;
    Ok(artifact)
}
