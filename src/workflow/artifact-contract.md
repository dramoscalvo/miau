Return one compact Markdown artifact, starting with exactly `# Review`, followed
by exactly `# Handoff` on its own line. Use these sections even with a custom role.

Hard-wrap Markdown prose with actual newline characters at word boundaries,
aiming for 120 characters per line and keeping lines within 150 characters where
practical. Preserve paragraph breaks and list indentation. Do not insert breaks
inside code blocks, tables, URLs, paths, or other indivisible tokens when doing
so would damage their meaning or Markdown syntax.

In Review, aim for 24 short lines (roughly one screen). Put the goal and human
decisions/risks first; never hide a blocking issue to meet the length target.
Show a compact file tree marked A/M/D/R with one-line purposes, labelled Proposed
before implementation or Observed after inspection. Distinguish pre-existing
changes; do not infer ownership from Git status alone. Include a few short
Given/When/Then acceptance scenarios with stable IDs, or reference existing IDs
when unchanged. Report verification as passed, failed, or not run, with commands
and evidence references. Scenarios alone are not executed tests. Include a small
text diagram only when relationships or execution order need explaining.
For critique/review, prioritize findings with severity and evidence over repeating
the plan. Human decisions remain with the operator. If a finding needs a human
choice, request that choice using the decision format below and reference the
finding's severity/evidence in its context. Recommending an option for a question
is allowed; approving or rejecting the workflow step belongs to the operator.

Before implementation, include a focused proposed UML class diagram when the change
involves types or their relationships. After implementation, include an observed UML
class diagram of the resulting code and explain deviations from the approved design
in Review. Preserve the planning artifact and reuse node IDs for the same types
across both stages. Critiques/reviews can reference these diagrams without duplicating
them. For changes without meaningful class/type relationships, explain why a class
diagram is not applicable instead of inventing classes for files or functions.

Include one JSON code fence tagged `miau-graph` inside Handoff per artifact.
This creates miau's navigable UML class view with connected arrows, member compartments,
and notes on individual classes. Keep the human explanation in Review and the complete
graph in the same artifact. Use version 2 and explicit classifier kinds. A directory
or module dependency graph does not satisfy the class diagram requirement.

```miau-graph
{"version":2,"title":"Storage classes","status":"proposed",
 "nodes":[{"id":"port","label":"Repository","kind":"interface",
           "attributes":[],"operations":["+ save(artifact: Artifact): Result"]},
          {"id":"files","label":"FileRepository","kind":"class",
           "attributes":["- root: Path"],"operations":["+ save(artifact: Artifact): Result"]}],
 "edges":[{"from":"files","to":"port","kind":"implements"}]}
```

Show the relevant classes, abstract classes, interfaces, and enums using `kind` values
`class`, `abstract-class`, `interface`, and `enum`. Map Rust structs to class boxes and
traits to interface boxes; explain language-specific mappings in Review when needed.
Include `attributes` and `operations` arrays of single-line UML signatures on each
classifier. Use `+` public, `-` private, `#` protected, names, parameter/return types,
and `{static}`, `{abstract}`, or `{readOnly}` where applicable. Enum attributes list
literals. Keep members focused on the change; disclose omitted members in Review.
An empty array means no members in that compartment; omit a field only when members
are unknown, in which case the viewer labels it not recorded. Do not invent members.

Use stable, unique type IDs across planning, revisions, and implementation. For TypeScript,
use the extractor's canonical `type:<project-relative-path>#<declared-name>` IDs in the
plan too (for example, `type:src/storage.ts#Repository`). Explain identity changes when
moving or renaming a declaration. Human notes
reference these IDs; address submitted notes in the revised artifact. Browsing or saving
notes is not approval; only explicit submission requests changes. Optional `parent`
references can preserve package/module grouping; the class view shows types together
across those containers. Optional `source` uses a real project-relative `file` and a
one-based `line`. Nodes require `id` and `label`.

Edges require `from`, `to`, and `kind`; `label` and `source` are optional. Use
`inheritance` from subtype to supertype, `implements` from class to interface,
`dependency` from client to supplier, and `association` for a structural reference.
Use `aggregation` or `composition` only when the whole/part relationship is known,
with `from` as the whole and `to` as the part. Do not infer ownership from field
visibility or readonly alone. The viewer draws UML triangles, diamonds, and solid
or dashed connectors from these kinds. Endpoints and parents must reference existing
IDs; hierarchy cycles are invalid, while relationship cycles and self-links are allowed.

Required graph fields are `version`, `title`, `status`, `nodes`, and `edges`; extra
fields are rejected. Use `status: "proposed"` for the plan, or `status: "observed"`
for the implementation you inspected. Every observed edge must cite a supporting
source reference. Never present proposed structure as observed or invent evidence.
Agent-authored diagrams are reported context, not independent verification or approval.
Limit the graph to the relevant types (at most 1000 nodes, 5000 edges, and 1 MiB of JSON).

For observed TypeScript class diagrams, use the deterministic generator with
`miau diagram typescript --project tsconfig.json --root . --scope types`.
Select the project's actual leaf tsconfig and use the run's project directory as root.
It needs Node and the project's installed TypeScript compiler. Preserve the generated
`miau-graph` block, including its optional `provenance` metadata, in your Handoff;
summarize relevant results and warnings in Review. Do not manually repair extracted
relationships, invent semantic nodes, convert associations to composition, or alter
member signatures. Report unavailable tools or generation failures honestly. For
other languages, inspect source and supply an agent-reported observed class diagram
with evidence. Put intended designs in the separate proposed planning artifact.
Module scope (`--scope modules`, the command default) is useful for import dependencies
but does not substitute for the planning/implementation class diagram.

For every role and agent, put questions requiring human input in Review as
second-level headings: `## D1: Question?`, `## D2: Question?`, and so on. Each
decision must have exactly one plain `Status: Open` or `Status: Resolved` line,
followed by concise context, options when useful, and a recommendation. Keep IDs
unique within the step and stable across its revisions; never renumber or reuse
an ID for a different question. Reserve these headings for human decisions, not
general findings or decisions already made by the agent. Do not put decision
headings inside code fences or repeat them in Handoff; reference their IDs there.
After receiving human answers, incorporate them in the updated artifact, mark
answered decisions `Status: Resolved`, and record `Answer: ...`. Keep unanswered
decisions `Status: Open`; partial answers do not authorize guessing the rest.
Do not invent questions when no human input is needed. Answer submission is not
workflow approval. These decision details may exceed the Review length target.

This is miau's human-feedback interface for every role, including custom roles,
on both initial and resumed runs. Questions in prose, lists, tables, Handoff,
intermediate messages, or native question tools do not create answer fields in
miau. Put every request for clarification, permission, or a product/scope choice
in the final artifact using the exact headings and plain status lines above.
Do not rely on an interactive tool to obtain an answer during this run. If work
is blocked on human input, return the artifact with the open decisions and state
what is blocked; wait for the operator's answers before doing the dependent work.

Example (replace the sample question and context; emit the artifact without the
surrounding code fence):

```markdown
# Review
## D1: Should the migration preserve existing settings?
Status: Open
Context: The plan does not specify how to handle existing settings.
Options: Preserve existing settings, or reset them to defaults.
Recommendation: Preserve existing settings to avoid losing user configuration.

# Handoff
Migration behavior is blocked on D1.
```

Before returning, check that every request for human input has a unique stable
`## D<number>: Question` heading directly inside `# Review` (not nested under a
findings heading), exactly one plain status line, and enough context to answer.

In Handoff, record only additional information the next agent needs: relevant
paths/symbols, constraints, non-obvious decisions and their reasons, unresolved
issues, and the next task. Reference the current upstream artifact paths and
acceptance IDs instead of copying their contents or repeating Review. Read both
sections of relevant input artifacts. Keep supporting evidence in referenced
files; omit transcripts, discovery narratives, and generic advice. Never claim
human approval that was not supplied. Return the artifact as your final response,
without an outer code fence; miau persists it.
