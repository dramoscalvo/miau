# Using miau

[Back to README](../README.md)

The initial screen summarizes each run's status, project, current workflow step, progress, and latest activity time.
Moving the selection updates the detail pane with the next human action, current attempt and duration, and recent
activity. Press `n` to create a run, or Enter to open the selected run.

Before starting a pending node, press `p`, type an initial prompt, and press Enter to save it; then press `s` to start
the node. After an agent finishes, `r` opens **Request changes** and Enter sends it immediately. `p` remains an alias.
Before a step starts, `r` also opens the initial prompt box. The box opens in Vim-style Normal mode and wraps and grows
to one-third of the window. Use `i`, `a`, `I`, or `A` to enter Insert mode, or `o`/`O` to open a new line below/above
the current line; `Esc` returns to Normal mode. In Normal mode, `h`, `j`, `k`, `l` (or the arrow keys) move the cursor;
`w`/`b`/`e` move by word; and `W`/`B`/`E` move by whitespace-delimited WORD. Use `0`, `^`, and `$` for line boundaries,
`gg`/`G` for document boundaries, and `x` to delete a character. Use `dd` to delete the current line, `D` to delete
through the end of the line, `yy` or `Y` to yank the current line, and `p` to paste the last yanked or deleted text. The
`diw`/`ciw`/`yiw` commands delete/change/yank the word under the cursor; `daw`/`caw`/`yaw` include its adjacent
whitespace, and uppercase `W` works for each text object. Press `Esc` again from Normal mode to cancel the prompt. In
Insert mode, the terminal's clipboard shortcut (commonly `Ctrl+Shift+V` or `Shift+Insert`) pastes system clipboard text,
including multiple lines. Normal-mode `p` only pastes text yanked or deleted within miau. At each decision gate,
guidance explains what to check and which step approval will start. Choose the action that matches your intention:

| Intention | Action | Result |
| --- | --- | --- |
| Accept this result | `a` **Approve & continue** | Starts the next step, or completes the workflow. |
| Ask the agent to change something | `r` **Request changes** (`p` also works) | Sends your feedback and produces a new artifact version for human review. |
| Understand or explore the result | `d` **Discuss with agent** | Opens the captured interactive session, then offers to update the artifact. |
| Correct the document yourself | `e` **Edit artifact** | Opens the current Markdown file in `$EDITOR`; the preview reloads on return. |

Request changes is the normal feedback path. Describe the desired change, for example: “Keep the existing storage format
and add a case for an empty input.” You do not need to create a custom feedback file. miau saves each submitted request
in its run directory as `feedback-N.md` (N is the zero-based step index), with numbered versions for later requests.
These preserve the full text; the activity channel shows only a short summary. Unsent drafts are not persisted.

Decision answers have a dedicated view and persisted drafts. Whenever a selected step's artifact contains open
decisions, **Decisions** opens by default. Use Up/Down or `j`/`k` to select a question and Enter to edit its answer.
The view shows each decision's ID and draft status, with the selected question's context, recommendation, and answer
below. Page Up/Page Down scroll the question and answer. Left/Right still select workflow steps, and `v` cycles through
Decisions, Review summary, Complete document, and Git changes; views absent from the artifact are skipped.

Answer fields open in Insert mode. Enter inserts a newline; Escape returns to Normal mode, where Enter or Escape
returns to Decisions. The usual Vim editing commands and multiline clipboard paste work. Each edit saves the draft
in `decision-drafts-N.json` in the run directory, scoped to the zero-based step index N. Drafts survive navigation and
restarting miau. This differs from the unsent free-form Request changes box described above.

From Decisions, press uppercase `S` (**Submit answers**) to send all nonblank answers directly to that step's agent.
Partial answers are allowed. Submission uses Request changes: it saves a versioned feedback file containing the
questions and answers, resumes the agent when a session is available, and returns the revised artifact for review.
It does not approve or advance the step. Submission is available only for completed or failed agent steps while no
process is active. Drafts remain available after submission; the agent should mark answered decisions resolved in the
updated document. Unanswered decisions remain open.

All agents receive the same decision format in the output contract:

```markdown
# Review
## D1: Which storage format should we use?
Status: Open
Context: Settings must persist between runs.
Recommendation: TOML.

## D2: Should existing settings be migrated?
Status: Resolved
Answer: Keep the existing format; no migration.

# Handoff
Additional implementation details.
```

IDs stay stable within a workflow step's revisions. Only `## D<number>: Question` headings in Review with
`Status: Open` appear as answer fields; code examples and Handoff are ignored. Duplicate IDs or missing/invalid status
lines show an error; use `e` at the current gate to correct the artifact or `r` to request a corrected document.
Artifacts without this format retain the existing document views. Questions are re-read from disk before editing
and submitting. If the questions changed, miau asks you to review them again. Drafts attach to the full original
question text, so changed questions start with blank fields; earlier drafts remain preserved on disk.

Editing an artifact changes the document that subsequent agents read. Editing an implementation report does not change
implementation code, and saving does not approve the step.

After a successful discussion, **Back from discussion · update artifact?** opens with an editable request to incorporate
agreed changes. Press Enter to resume the agent and generate an updated artifact, `i` to edit the request, or Escape in
Normal mode to return to review. Nothing is sent until you press Enter. Discussion itself does not capture a new
artifact or advance the workflow; changes the interactive agent makes on disk remain visible even if you cancel the
update request. Session context supplies the discussion; miau does not forward a transcript. If the agent no longer
retains the agreement, include it in the request. A discussion that exits unsuccessfully surfaces an error and leaves
you at review. Failure to launch or restore the terminal exits miau with an error.

Press `f` and confirm to finish the run; miau preserves completed steps and marks every unfinished step as skipped.

Use Left and Right to move between the workflow's agents. The highlighted row is the agent being viewed; `>` still marks
the current workflow gate. Each agent's artifact and status remain available without changing workflow state. On a
completed or failed agent, press `r` to request changes from the prompt box and resume its captured session when
available. This re-queues that workflow step while preserving the results of later steps, which will be offered again in
order. Press `d` to open that agent's captured session interactively without changing workflow state.

The project detail area starts with **Review summary**, showing only the selected agent's `# Review` section. Press `v`
to cycle through **Review summary**, **Complete document** (the same review plus `# Handoff` details for the next
agent), and the project's live Git working tree. Use Up/Down or Page Up/Page Down to scroll document text when it
exceeds the pane. Old or incomplete artifacts without a separate review section show **Complete document**; `v` switches
directly between that document and Git changes, skipping the duplicate summary view. Editing still opens the single
canonical artifact, and the preview reloads after the editor returns. The one-screen target is guidance, not a
truncation or approval rule; diagrams and Gherkin are displayed as text, and scenarios are not automatically executed.

The live Git tree marks added files in green, modified files in yellow, deleted files in red, and renamed files in cyan.
Use `j`/`k` or Up/Down to select a file and Page Up/Page Down to scroll its unified diff. This view reports all current
working-tree changes, including changes that existed before the run; it does not yet attribute files to an individual
run or workflow step.

The Activity pane follows the newest activity by default. Press Tab to focus it, then use Up, Down, Page Up, or Page
Down to browse its history; scrolling down returns toward the latest entry.

While an agent is running, the output pane title reports its latest normalized activity, such as starting, thinking,
writing a response, using a tool, or changing a file. Workflow nodes use action-oriented states: `Working` means the
agent process is active, while `Waiting for you` means the artifact is ready and miau needs your decision. If a run
fails, its captured output remains visible under `Needs attention` instead of being replaced by an empty artifact.

When a revisited step finishes, approving it queues the completed following step again, so a revised plan is critiqued
again before implementation.

From a project screen, press `b` to return to the initial run list. While an agent is running, stop it first; the
project screen remains visible so active work cannot be hidden accidentally. Press `x` and confirm with `y` to stop the
agent without quitting miau. The interrupted node remains at its decision gate.
