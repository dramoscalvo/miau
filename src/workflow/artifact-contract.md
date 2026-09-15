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
