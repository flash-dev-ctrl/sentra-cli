# AI Agent CLI

Use this reference for coding agents, autonomous workflows, tool execution views, and AI-assisted developer CLIs.

## Agent State Model

Represent agent work with explicit states:

| State | User question answered |
| --- | --- |
| Intent | What is the agent trying to do? |
| Planning | How will it approach the work? |
| Tool Execution | What is it running or inspecting now? |
| Verification | How is it checking the result? |
| Result | What changed and what remains? |

Example:

```text
Agent:
  Security Auditor

◌ Planning

Goal:
  Analyze skill security risks

▶ Running tool:
  sandbox inspect

✓ Verification completed
```

## Tool Execution Block

Default human view:

```text
▶ Running tool
  Command: rg "unsafe|panic" crates/
  Goal:    find risky implementation notes
```

Completed view:

```text
✓ Tool completed
  Command: rg "unsafe|panic" crates/
  Result:  12 matches in 5 files
```

Failed view:

```text
✗ Tool failed
  Command: cargo test -p sentra-core

Problem:
  Tests failed in sentra_config::tests

Next:
  Review the failure summary or rerun with --verbose for full output.
```

## Output Disclosure

Default mode:

- show agent intent
- show current phase
- summarize tool purpose
- summarize result
- expose next step
- hide raw logs unless needed

Verbose mode:

- include command arguments
- include working directory
- include exit code and duration
- include selected output excerpts
- include retries and sandbox details

Machine mode:

- emit event objects with stable `type`, `status`, `tool`, `goal`, `started_at`, `completed_at`, and `result` fields
- do not include ANSI styling
- do not require terminal interaction

## Approval and Sandbox Feedback

When approval is needed:

```text
▶ Approval required
  Action: modify files
  Target: .codex/skills/cli-design
  Reason: create requested skill documents

  Press Enter to approve, Esc to cancel.
```

When sandboxing blocks work:

```text
✗ Tool blocked

Problem:
  The command needs write access outside the allowed workspace.

Cause:
  Sandbox policy denies this path.

Solution:
  Approve elevated access or choose a workspace-local target.
```

## File Mutation Feedback

Show mutations as an inspectable summary:

```text
✓ Files updated
  Added:    7
  Modified: 2

  Primary changes:
    .codex/skills/cli-design/SKILL.md
    .codex/skills/cli-design/references/status-progress.md
```

Avoid raw diffs by default unless the user's next decision depends on the diff. Offer verbose or inspect mode for full details.

## Verification

Agent workflows should not end at "done." Show verification:

```text
✓ Verification completed
  Checks:
    quick_validate.py  passed
    schema review      passed
```

If verification was skipped:

```text
⚠ Verification not run
  Cause: test dependency is unavailable
  Next:  run `cli doctor` after installing dependencies
```
