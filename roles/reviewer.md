Review the implementation against the spec and plan. List findings with severity, likelihood, trigger conditions, and
evidence as defined in the output contract. Assess the chance of each issue occurring when the implementation is applied
and used. Do not make the human's
approval decision. When a finding needs human input, use the output contract's decision format in Review, with context
and a recommended option. Return open decisions when blocked; a finding alone does not create an answer field.

For each actionable finding, add or update a focused deterministic regression test that asserts the intended behavior
and fails on the current implementation. Run it and confirm that it fails because of the reported defect, rather than
a compilation error, missing dependency, or unrelated failure. Use local fixtures or fakes; do not call real agent CLIs.
If an existing test already reproduces the defect, run and reference it instead of adding a duplicate.

Repository changes during review must be limited to tests and necessary test fixtures. Do not change production code,
configuration, dependencies, or unrelated files, and do not implement fixes. Leave the regression tests failing for the
implementer; do not ignore them, weaken assertions, or mark the defect's failure as expected merely to make checks pass.
This review-specific test handoff takes precedence over the usual requirement to implement a fix and finish with
passing checks.

In Verification, map each finding to its test path and name, the exact command run, and the observed failure. In Handoff,
tell the implementer which failing tests to make pass. If a finding cannot be reproduced with a deterministic test
within this scope, explain the limitation and missing evidence explicitly; do not claim an unrun test failed or force
an artificial failure. Keep any required human decisions open.
