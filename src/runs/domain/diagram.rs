//! Versioned architecture data carried by a run artifact.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Graph {
    pub version: u32,
    pub title: String,
    pub status: Status,
    pub nodes: Vec<Node>,
    pub edges: Vec<Edge>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provenance: Option<Provenance>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Status {
    Proposed,
    Observed,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Node {
    pub id: String,
    pub label: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub kind: Option<NodeKind>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub attributes: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub operations: Option<Vec<String>>,
    pub parent: Option<String>,
    pub source: Option<Source>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum NodeKind {
    Directory,
    Module,
    Class,
    AbstractClass,
    Interface,
    Enum,
    External,
}

impl NodeKind {
    pub fn is_classifier(self) -> bool {
        matches!(
            self,
            Self::Class | Self::AbstractClass | Self::Interface | Self::Enum
        )
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Directory => "Directory",
            Self::Module => "Module",
            Self::Class => "Class",
            Self::AbstractClass => "Abstract class",
            Self::Interface => "Interface",
            Self::Enum => "Enum",
            Self::External => "External",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Edge {
    pub from: String,
    pub to: String,
    pub kind: EdgeKind,
    pub label: Option<String>,
    pub source: Option<Source>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum EdgeKind {
    Dependency,
    Association,
    Implements,
    Inheritance,
    Aggregation,
    Composition,
}

impl EdgeKind {
    pub fn label(self) -> &'static str {
        match self {
            Self::Dependency => "dependency",
            Self::Association => "association",
            Self::Implements => "implements",
            Self::Inheritance => "inheritance",
            Self::Aggregation => "aggregation",
            Self::Composition => "composition",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Source {
    pub file: String,
    pub line: u32,
}

/// Recorded generator inputs, not a signature or independent attestation.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Provenance {
    pub extractor: String,
    pub version: u32,
    pub typescript: String,
    pub project: String,
    pub scope: String,
    pub fingerprint: String,
    pub warnings: Vec<String>,
}

impl Source {
    /// Portable project-relative paths, never commands or editor arguments.
    pub fn is_valid(&self) -> bool {
        self.line > 0
            && !self.file.contains(['\\', ':'])
            && !self.file.chars().any(char::is_control)
            && self
                .file
                .split('/')
                .all(|part| !matches!(part, "" | "." | ".."))
    }
}
