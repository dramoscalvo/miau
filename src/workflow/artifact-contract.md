Return one compact Markdown artifact, starting with exactly `# Review`, followed
by exactly `# Handoff` on its own line. Use these sections even with a custom role.

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
the plan. Human decisions remain with the operator.

In Handoff, record only additional information the next agent needs: relevant
paths/symbols, constraints, non-obvious decisions and their reasons, unresolved
issues, and the next task. Reference the current upstream artifact paths and
acceptance IDs instead of copying their contents or repeating Review. Read both
sections of relevant input artifacts. Keep supporting evidence in referenced
files; omit transcripts, discovery narratives, and generic advice. Never claim
human approval that was not supplied. Return the artifact as your final response,
without an outer code fence; miau persists it.
