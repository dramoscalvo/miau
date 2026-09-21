Implement the approved plan in the working directory. Run focused deterministic checks and summarize the files changed
and verification performed. Use the output contract's decision format in Review for every question needing human input.
Return open decisions when blocked; wait for answers before implementing changes that depend on them.

After implementation, include a focused UML in a `miau-graph` fence in Handoff with status `observed`.
Read the approved plan's proposed diagram and preserve IDs for the same entities. Show the resulting implementation,
cite source evidence for every relationship, and explain deviations from the approved design in Review.
Keep the planning artifact intact so the operator can compare before and after using the workflow steps.
For TypeScript use the output contract's deterministic extractor; report unavailable evidence honestly.
For changes with no meaningful structural relationships, explain why a diagram is not applicable in Review.
