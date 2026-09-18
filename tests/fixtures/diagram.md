# Review
Proposed storage dependencies for review.

# Handoff
```miau-graph
{
  "version": 1,
  "title": "Storage boundary",
  "status": "proposed",
  "nodes": [
    {"id": "runs", "label": "Runs"},
    {"id": "port", "label": "ArtifactRepository", "parent": "runs",
     "source": {"file": "src/runs/application.rs", "line": 40}},
    {"id": "files", "label": "FileRepository", "parent": "runs",
     "source": {"file": "src/runs/infrastructure.rs", "line": 1}}
  ],
  "edges": [
    {"from": "files", "to": "port", "kind": "implements",
     "source": {"file": "src/runs/infrastructure.rs", "line": 1}}
  ]
}
```
