# Artifact diagrams

[Back to usage](usage.md)

An artifact can include one optional `miau-graph` JSON fence in its `# Handoff` section. When present, press `v` to
reach Diagram after Git changes. The view shows a hierarchy and the selected node's incoming and outgoing
relationships. TypeScript module dependencies and semantic type relationships can be extracted with the deterministic
command below. Graphical arrow layout and metrics overlays are not included yet.

## Deterministic TypeScript extraction

Run the installed `miau` executable in the project being analyzed:

```sh
miau diagram typescript --project tsconfig.json --root . --output architecture.md
miau diagram typescript --project tsconfig.json --root . --scope types --output types.md
```

This runs Node.js once and uses the TypeScript compiler installed in that project's dependency tree. It needs Node 18+
and the TypeScript 5.6–6.x JavaScript compiler API; development fixtures pin TypeScript 5.9.3. Install the project's
locked dependencies before running. No model, API key, agent CLI, compiler plugin, build, or network access is involved
in extraction. The Node adapter is embedded in the Rust executable, so an installed miau needs no source checkout.
Use `--scope modules` (the default) or `--scope types`. Use `--node /path/to/node` to select a Node executable and
`--title "Application dependencies"` to name the graph. Missing dependencies, configuration errors, and syntax errors
are surfaced. Semantic extraction uses the type checker but does not require `tsc --noEmit` to succeed.

The command emits a complete Markdown review artifact, including a `miau-graph` block. Without `--output`, it writes
the artifact to stdout. With `--output`, it creates a new file and refuses to overwrite any existing file. Use a new
filename for each snapshot, or let a workflow command step capture stdout using miau's existing artifact versioning.
Generating an artifact does not create a run or change an approval gate. An example optional workflow node is:

```toml
[[nodes]]
name = "architecture"
command = ["miau", "diagram", "typescript", "--project", "tsconfig.json", "--root", "."]
writes = "architecture.md"
```

This uses the ordinary command-step lifecycle: start it explicitly, inspect Diagram, then decide whether to continue.
It requires `miau` on PATH. Keep `--root` equal to the run's project directory so source references open correctly.
For a monorepo, select a leaf config with `--project packages/app/tsconfig.json` and keep the workspace as `--root`.
Project references are rejected; extracting multiple referenced projects into one graph is not supported yet.

### Module scope

Module scope records module imports, exports, and resolved dependencies. The compiler reads tsconfig file selection,
inherited configuration, path aliases, module resolution, and package
export conditions. Extraction follows local imports transitively, including files excluded from the initial root
file list. Directory nodes follow actual paths. Each local source file is a module node, with source evidence for
every dependency. External packages are collapsed into package nodes; their declaration files are not displayed as
project implementation. Supported constructs include static and side-effect imports, re-exports, type-only imports,
import-type expressions, import-equals declarations, and literal dynamic imports. TSX is parsed as TSX.

This is a compiler-resolved **module dependency graph**, not a UML class model, call graph, runtime dependency graph,
or successful type-check assertion. It does not infer class inheritance, implementations, injected dependencies,
implicit JSX-runtime imports, or framework-generated relationships. Nonliteral dynamic imports, CommonJS `require()`
calls, and triple-slash references produce explicit warnings. CommonJS calls are not assumed to be the global loader
because their bindings are not analyzed. Syntax errors, unresolved module specifiers (including unsupported assets),
out-of-root sources, and oversized graphs fail generation instead of silently dropping relationships.

### Types scope

Type scope builds a TypeScript `Program` and `TypeChecker`. It discovers actual top-level classes, abstract classes,
interfaces, and enums in project files. Each type is a child of its real module, and its canonical identity combines
the project-relative source path and declared name, such as `type:src/domain/repository.ts#Repository`. Import aliases
and equivalent path spellings resolve to that declaration's symbol rather than creating additional nodes. External
declarations are omitted instead of expanding `node_modules`.

| TypeScript source fact | Observed edge |
| --- | --- |
| `class A extends B` | inheritance |
| `interface A extends B` | inheritance |
| `class A implements B` | implements |
| instance property `b: B` | association |
| constructor parameter property `private b: B` | association |
| normal constructor parameter `b: B` | dependency |
| method parameter `b: B` | dependency |
| method return type `B` | dependency |
| local variable | ignored |
| `new B()` | ignored |
| decorator mentioning `B` | ignored |
| aggregation | never inferred |
| composition | never inferred |

Optional and nullable unions retain their supported named project targets. `B[]`, `Array<B>`, and
`ReadonlyArray<B>` normalize to `B`. A project generic such as `Repository<Order>` relates only to `Repository`;
generic arguments are not interpreted. `Promise<B>` unwraps to `B` for operation returns. Primitive, builtin,
anonymous-object, function, and type-parameter references are ignored; a project-defined declaration with a builtin
name still resolves normally. Type aliases, functions, variables, namespaces, local variables, calls, decorators, and
runtime expressions do not become semantic nodes or dependency evidence.

Syntax and configuration errors remain fatal. Ordinary semantic diagnostics do not make extraction fail. An unresolved
declared relationship produces a deterministic warning and no edge; declaration merging and nested declarations are
not guessed. Merged supported declarations fail because they lack one unambiguous milestone identity, while nested or
anonymous supported declarations produce warnings and are omitted.

The type graph is a compiler-derived semantic relationship graph. It is not a runtime object graph, call graph,
ownership model, or complete UML interpretation. In particular, visibility, `readonly`, property initializers, and
constructor parameter properties never imply aggregation or composition. Framework and decorator conventions are
irrelevant to extraction.

Given unchanged files, configuration, dependencies, and compiler/runtime environment, sorted output is reproducible.
No timestamp or absolute checkout path is inserted. Metadata records extractor and compiler versions, config path,
scope, warnings, and a SHA-256 fingerprint of the read source/configuration/resolution inputs. Tests compare output
across repeated runs and relocated checkouts. Regenerate and compare fingerprints to check for input changes; miau
does not automatically scan for staleness while viewing a stored artifact. Metadata is editable artifact content,
not a signature or independent verification. The TUI distinguishes extractor-reported and agent-reported diagrams.

Agents may select scope, invoke the extractor, describe observed relationships, and interpret warnings. They must
preserve the generated graph and metadata: they must not modify observed edges, invent semantic nodes, convert
associations into composition, classify directories into architecture layers, or manually repair unresolved compiler
relationships. Use a separate `status: proposed` diagram for intended or interpreted architecture. No additional
companion agent is required.

### The model's role

The extraction pipeline is `source + tsconfig → TypeScript compiler API → validated graph → Markdown artifact`.
Model choice has no effect on its relationships or output ordering. An optional coding agent can choose a config,
invoke the command, summarize warnings, or propose a redesign through the ordinary human-gated workflow.
Assess that agent on interpretation and instruction-following; extraction correctness is covered by compiler fixtures
and repeatability tests. An agent's prose or proposed design has no determinism guarantee.

## Reviewing diagrams

The active agent can produce a diagram as part of its ordinary report. Ask for a relevant architecture diagram in
the initial prompt or Request changes. Regeneration uses the existing human-submitted revision flow. Browsing,
reloading, and opening sources never approve a step, create feedback, or start an agent.

| Key | Action |
| --- | --- |
| Up/Down or `k`/`j` | Select a node in the expanded hierarchy. |
| Enter | Select its first child, if any. |
| Backspace | Select its parent. |
| Page Up/Page Down | Scroll the selected node's relationship details. |
| `]` | Cycle through its source reference and the sources cited by its relationships. |
| `o` | Open the selected source file in `$EDITOR` (default `vi`) while no process is active. |
| `R` | Reload the diagram from the artifact on disk. |
| Left/Right | Browse workflow steps. |
| `v` | Continue cycling through the artifact views. |

Tab still changes focus to Activity, where the usual scrolling controls apply. Source references show a path and
one-based line number in the diagram header. The editor receives the absolute file path as one argument, without
a shell; this milestone opens the file at the editor's default position. `$EDITOR` is an executable name or path,
consistent with artifact editing. Source opening rejects missing files, directories, and paths or symlinks that
escape the run's project directory. A proposed file that does not exist yet remains visible but cannot be opened.
Saving source edits does not regenerate the diagram; request an updated artifact when its relationships change.

The graph is stored inside the Markdown report, so it follows existing artifact versions. miau rereads the artifact
when changing views, after an editor handoff, on `R`, and before opening a source. If the selected reference changed,
review the updated selection and press `o` again. Invalid JSON, unknown endpoints, duplicate IDs, or hierarchy
cycles show a Diagram error; the report remains accessible with `v`. Artifacts without diagrams retain the existing
view cycle. There is no file watcher in this milestone.

## Format

Only a fence tagged exactly `miau-graph` in Handoff is interpreted. Diagrams in Review or nested code examples are
ignored. Backtick or tilde fences are accepted. An unclosed diagram fence or multiple diagram blocks is an error.

````markdown
# Review
Proposed storage boundary; inspect the relationship before approving.

# Handoff
```miau-graph
{
  "version": 1,
  "title": "Storage boundary",
  "status": "proposed",
  "nodes": [
    {"id": "storage", "label": "Storage"},
    {"id": "port", "label": "Repository", "parent": "storage",
     "source": {"file": "src/repository.rs", "line": 12}},
    {"id": "files", "label": "FileRepository", "parent": "storage"}
  ],
  "edges": [
    {"from": "files", "to": "port", "kind": "implements"}
  ]
}
```
````

Required graph fields are `version` (`1` or `2`), a nonblank `title`, `status`, a nonempty `nodes` array, and an
`edges` array (which may be empty). Status is `proposed` for a design or `observed` for source inspection. Every observed
edge requires a source reference. Without generator metadata, Observed is labelled agent-reported. miau does not
independently verify a cited line, and references may become stale as source files change.

The module generator remains on graph version 1 for byte-compatible output. Type scope emits graph version 2 and
provenance rule version 2.

An observed graph may also contain `provenance` with `extractor`, positive integer `version`, `typescript`, `project`,
`scope`, a 64-digit hexadecimal SHA-256 `fingerprint`, and a `warnings` string array. `project` is a relative config
path. The TypeScript generator fills these fields; agents should preserve them unchanged. The version identifies
the extraction rules. Provenance scope is `module-dependencies` for module scope and `type-relations` for type scope.

Nodes require nonblank `id` and `label` strings; optional `parent` references another node's ID. Version 2 also accepts
an optional semantic `kind`: `directory`, `module`, `class`, `abstract-class`, `interface`, `enum`, or `external`.
Unknown kinds are rejected. Version 1 artifacts remain valid. IDs must be unique and should remain stable across
revisions. Roots and siblings display in document order, with all children visible.
The optional node `source` has `file` and `line`. Paths use `/`, are relative to the project, and cannot contain
`.` or `..` segments, backslashes, colons, or control characters. Lines must be positive integers.

Edges require `from`, `to`, and `kind`; endpoints must exist. Kinds are `dependency`, `association`, `implements`,
`inheritance`, `aggregation`, and `composition`. Optional `label` adds context, and optional `source` uses the same
shape as a node's reference. Dependency cycles and self-references are allowed; parent cycles are not.

Unknown fields are rejected to expose typos. Limits are 1 MiB of diagram JSON, 1000 nodes, and 5000 edges. Keep diagrams
focused on relationships that help the current review. These limits and validation errors affect presentation only;
the human retains control of every workflow decision.
