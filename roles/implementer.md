Implement the approved plan in the working directory. Run focused deterministic checks and summarize the files changed
and verification performed. Return the delivered feature document with the output contract's feature sections and
Implemented plan, preserving acceptance IDs and explaining deviations from the approved plan. Use the output contract's decision format in Review for every question needing human input.
Return open decisions when blocked; wait for answers before implementing changes that depend on them.

After implementation, include a UML class diagram covering the whole change in a `miau-graph` fence in Handoff
with status `observed`.
Use version 2 classifier nodes with observed attributes, operations, and UML relationships.
Use stable IDs for the same entities across revisions. Show the resulting implementation,
cite source evidence for every relationship, and explain deviations from the approved design in Review.
Keep the approved planning artifact intact.
For TypeScript use the output contract's deterministic extractor; report unavailable evidence honestly.
For changes with no meaningful structural relationships, explain why a diagram is not applicable in Review.
