# Artifact diagrams

[Back to usage](usage.md)

UML review happens after implementation; planning and pre-implementation critique do not generate UML.
Implementation reports provide an **Observed UML class diagram** covering the whole change.
The shared output contract and bundled roles request
version 2 classifier nodes with attributes, operations, and UML relationships. A module dependency graph does not
satisfy that request. Stable type IDs let you retain selection when browsing stored diagrams.
Agents explain deviations from the plan and disclose omitted members. For changes without meaningful class/type
relationships, they explain why a class diagram is not applicable. These are agent instructions, not an automatic
completeness check or approval rule.

Press `g` to open Diagram, or `v` to reach it after Git changes. Graphs containing class, abstract-class, interface,
or enum nodes open the UML class view automatically. Classes appear across module/package containers, with name,
attribute, and operation compartments. Interfaces show `«interface»`, abstract classes show `{abstract}`, and enums
show `«enumeration»`. Empty member arrays mean empty compartments; absent arrays show **not recorded**, rather than
claiming that an older artifact describes a class with no members.

The canvas shows all classes and their relationships together in a stable two-column layout. Each class appears once,
including disconnected types. Selection highlights a class and its active relationship without filtering or rearranging
the graph or resetting the pan position. Large graphs extend beyond the viewport and can be panned;
there is no four-relationship page limit.
Solid lines with hollow triangles show inheritance; dashed lines with hollow triangles show interface
implementation. Dependencies use dashed arrows; associations use solid lines. Hollow/filled diamonds show aggregation/
composition at the whole end. The active relationship is highlighted. Use `}` to cycle relationships, Enter to follow
the active relationship, and Backspace to return. Use Up/Down or `j`/`k` to select any class, including disconnected types.

Use uppercase `H`/`J`/`K`/`L` to pan left/down/up/right and Home to reset the canvas. Narrow terminals may need horizontal
panning to see both ends. Long signatures are clipped in boxes; their full text remains in the scrollable class details.
Page Up/Page Down scrolls those details. Class notes, source opening, reload, and human review gates work as before.
Left/Right browses workflow steps while retaining selection by type ID, so you can inspect stored artifacts
without leaving Diagram. Each diagram stays in its own versioned artifact; there is no side-by-side diff.

Existing artifacts are preserved. Untyped/module-only graphs retain the hierarchy browser. Request an updated version 2
class diagram through Request changes on the implementation step. A report carries one
`miau-graph` JSON fence inside `# Handoff`; ordinary Mermaid/text fences do not activate the interactive view.

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

For a type graph limited to new and edited TypeScript files plus their directly related types, add `--scope types
--changed`. The changed set comes from Git status and includes staged, unstaged, and untracked files; deleted files are
omitted. The graph retains all supported types declared in changed files and the other endpoints of relationships
touching those types. It does not recursively expand another relationship hop. Git changes within `--root` include
pre-existing unrelated edits. Repository-relative paths are normalized to the project root, and renames use their
destination paths. The compiler still analyzes the selected project to resolve symbols, while
the displayed graph is narrowed to the changed files and directly related types. When no TypeScript source changed,
the command emits a short no-diagram artifact rather than failing.

The node and edge limits apply after filtering. A large unrelated graph does not prevent a small changed graph from
being generated. Outside a Git worktree, the command emits an explanatory artifact that changed-file extraction is
not applicable; it does not claim the project is unchanged. Missing Git and other Git errors still fail the command.

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

Type nodes also record declared attributes and operation signatures, including visibility, parameter/return types,
constructor parameter properties, and static/abstract/readonly modifiers. Enum compartments list literal names.
Inherited members are not copied into subclasses. Unsupported member forms produce warnings. Member signatures use
source annotations where present and compiler-inferred types otherwise; bodies and initializers are not copied.

The type graph is a compiler-derived class model. It is not a runtime object graph, call graph,
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
relationships. Describe intended or interpreted architecture in prose. No additional companion agent is required.

### The model's role

The extraction pipeline is `source + tsconfig → TypeScript compiler API → validated graph → Markdown artifact`.
Extraction and TUI rendering consume no model tokens. Agent-authored diagrams, agent interpretation, and reading graph
content in an agent session still use tokens. The bundled `impacted-diagram` command step runs without an agent.
Model choice has no effect on its relationships or output ordering. An optional coding agent can choose a config,
invoke the command, summarize warnings, or propose a redesign through the ordinary human-gated workflow.
Assess that agent on interpretation and instruction-following; extraction correctness is covered by compiler fixtures
and repeatability tests. An agent's prose or proposed design has no determinism guarantee.

## Reviewing diagrams

The active agent produces diagrams as part of its ordinary report. Request missing or updated diagrams through the
initial prompt or Request changes. Regeneration uses the existing human-submitted revision flow. Browsing, reloading,
opening sources, and saving notes never approve a step or start an agent.

| Key | Action |
| --- | --- |
| `g` | Open Diagram directly from another artifact view. |
| Up/Down or `k`/`j` | Select a class; in legacy graphs, select a box at the current hierarchy level. |
| Enter | Follow the active UML relationship; in legacy graphs, drill into children. |
| Backspace | Return to the previous class; in legacy graphs, return to the parent level. |
| `}` | Highlight the next relationship of the selected class on the whole graph. |
| `H`/`J`/`K`/`L`, Home | Pan the UML canvas left/down/up/right, or reset its position. |
| Page Up/Page Down | Scroll the selected node's relationship details. |
| `]` | Cycle through its source reference and the sources cited by its relationships. |
| `o` | Open the selected source file in `$EDITOR` (default `vi`) while no process is active. |
| `R` | Reload the diagram from the artifact on disk. |
| `Esc` | Return to the complete document view. |
| Left/Right | Browse before/after workflow steps, retaining the selected entity when possible. |
| `n` | Add or edit a saved note on the selected node of an agent step. |
| `S` | Submit current diagram notes as Request changes; return to human review after the agent finishes. |
| `p`/`r` | Open the prompt to send a request for changes while the UML view is open. |
| `v` | Continue cycling through the artifact views. |

Notes use the existing multiline editor: type to edit, Enter inserts a newline, Esc returns to normal mode, then
Enter/Esc returns to Diagram. Edits save as you type. A `[note]` marker identifies annotated boxes; the selected note
appears above its relationships. Clear the text to remove a note from submission. `S` explicitly sends all nonblank
notes for the current graph through the existing revision/revisit flow. It does not approve the step. The ordinary
`a` approval action remains separate. Notes are available while no process is active on a completed/failed agent step;
command steps cannot receive agent revisions.

Notes persist in `diagram-notes-N.json`, scoped to the run, step, and exact parsed graph snapshot. Changing graph
structure, labels, evidence, or metadata creates a distinct snapshot: old notes remain on disk but are not reused or
submitted for the changed graph. Review and re-enter applicable feedback after a change. Returning to the original
graph restores its notes. Prose-only edits do not invalidate diagram notes. Opening a note or submitting notes rereads
the artifact; a changed graph requires reviewing the updated view first. Submitted notes are preserved in the ordinary
versioned feedback file before the agent starts.

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
  "version": 2,
  "title": "Storage classes",
  "status": "proposed",
  "nodes": [
    {"id": "port", "label": "Repository", "kind": "interface",
     "attributes": [], "operations": ["+ save(artifact: Artifact): Result"],
     "source": {"file": "src/repository.rs", "line": 12}},
    {"id": "files", "label": "FileRepository", "kind": "class",
     "attributes": ["- root: Path"], "operations": ["+ save(artifact: Artifact): Result"]}
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
provenance rule version 4.

An observed graph may also contain `provenance` with `extractor`, positive integer `version`, `typescript`, `project`,
`scope`, a 64-digit hexadecimal SHA-256 `fingerprint`, and a `warnings` string array. `project` is a relative config
path. The TypeScript generator fills these fields; agents should preserve them unchanged. The version identifies
the extraction rules. Provenance scope is `module-dependencies` for module scope and `type-relations` for type scope.

Nodes require nonblank `id` and `label` strings; optional `parent` references another node's ID. Version 2 also accepts
an optional semantic `kind`: `directory`, `module`, `class`, `abstract-class`, `interface`, `enum`, or `external`.
Unknown kinds are rejected. Version 1 artifacts remain valid. IDs must be unique and should remain stable across
revisions. Classifiers display in document order, independent of their containers. Legacy roots and siblings retain
hierarchy order, with Enter revealing a selected node's children.

Version 2 classifier nodes accept optional `attributes` and `operations` arrays of nonblank, single-line strings.
Use UML signatures such as `- path: Path`, `+ save(value: Artifact): Result`, and `# count: number {static}`.
`+`, `-`, and `#` mean public, private, and protected. `{readOnly}` and `{abstract}` retain those modifiers.
Enum attributes list literals. Include both arrays in new implementation diagrams, using `[]` for empty
compartments. Omitted fields represent unknown members. Member fields on container nodes or version 1 graphs are rejected.
The optional node `source` has `file` and `line`. Paths use `/`, are relative to the project, and cannot contain
`.` or `..` segments, backslashes, colons, or control characters. Lines must be positive integers.

Edges require `from`, `to`, and `kind`; endpoints must exist. Kinds are `dependency`, `association`, `implements`,
`inheritance`, `aggregation`, and `composition`. Optional `label` adds context, and optional `source` uses the same
shape as a node's reference. Inheritance and implementation point from subtype/implementer to supertype/interface;
aggregation and composition run from whole to part, with the diamond at `from`. Dependency cycles and self-references
are allowed; parent cycles are not.

Unknown fields are rejected to expose typos. Limits are 1 MiB of diagram JSON, 1000 nodes, and 5000 edges. Keep diagrams
focused on relationships that help the current review. These limits and validation errors affect presentation only;
the human retains control of every workflow decision.
