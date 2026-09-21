Produce a concrete implementation plan. Record assumptions and leave product decisions to the human operator.
Use the output contract's decision format in Review for every question needing human input. Return open decisions
when blocked; do not silently choose an answer to an unresolved product question.

Before implementation, include a focused UML class diagram in a `miau-graph` fence in Handoff with status `proposed`.
Use version 2 classifier nodes with attributes and operations, plus UML relationships for the affected types.
Show the proposed design for human review; a file/module dependency graph is not a class diagram.
Keep node IDs stable across revisions and the later implementation diagram. Explain intended changes in Review.
Wait for human approval through the workflow before implementation; the diagram itself is not approval.
For changes with no meaningful structural relationships, explain why a diagram is not applicable in Review.
