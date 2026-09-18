//! One-shot Node runner for the project's installed TypeScript compiler.

use crate::runs::{
    application::generate_diagram::{DiagramExtractor, ExtractionError, ExtractionRequest},
    domain::diagram::Graph,
};
use std::{
    ffi::OsString,
    process::{Command, Stdio},
};

pub struct TypeScriptExtractor {
    pub node: OsString,
}

impl DiagramExtractor for TypeScriptExtractor {
    fn extract(&self, request: &ExtractionRequest<'_>) -> Result<Graph, ExtractionError> {
        let project = request.project.canonicalize()?;
        let root = request.root.canonicalize()?;
        let output = Command::new(&self.node)
            .arg("-e")
            .arg(include_str!("typescript.cjs"))
            .arg("--")
            .arg(&project)
            .arg(&root)
            .arg(request.title)
            .env("MIAU_TYPESCRIPT_EXTRACT", "1")
            .current_dir(&root)
            .stdin(Stdio::null())
            .output()?;
        if !output.status.success() {
            return Err(ExtractionError::Invalid(format!(
                "TypeScript extractor exited with {}: {}",
                output.status,
                String::from_utf8_lossy(&output.stderr).trim()
            )));
        }
        Ok(serde_json::from_slice(&output.stdout)?)
    }
}
