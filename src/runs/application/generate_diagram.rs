//! Generate review artifacts through a language extraction port.

use crate::runs::domain::diagram::{Graph, Status};
use std::{io, path::Path};
use thiserror::Error;

pub struct ExtractionRequest<'a> {
    pub project: &'a Path,
    pub root: &'a Path,
    pub title: &'a str,
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
    let artifact = format!(
        "# Review\nObserved TypeScript module dependencies extracted from source.\n\
         {} nodes; {} dependencies; {warnings} extraction warnings.\n\
         Review the Diagram view, source evidence, and extraction warnings before deciding.\n\
         This is a module dependency graph, not a class model or a type-check result.\n\n\
         # Handoff\nGenerated graph; regenerate from source instead of editing its relationships.\n\
         ```miau-graph\n{json}\n```\n",
        graph.nodes.len(),
        graph.edges.len(),
    );
    super::diagram::parse(&artifact).map_err(ExtractionError::Invalid)?;
    Ok(artifact)
}
