# Errors and Interaction

Use this reference for failure states, recovery, keyboard interaction, and confirmations.

## Error Structure

Every user-facing error should follow:

```text
Problem
Cause
Solution
```

Template:

```text
✗ Installation failed

Problem:
  Permission denied while writing node_modules

Cause:
  Current user cannot access the target directory

Solution:
  Run:
    cli repair permissions

Debug:
  Error id: install-permission-denied
```

## Error Content Rules

- Title: say what failed, not just that something failed.
- Problem: describe the user-visible failure.
- Cause: explain the likely reason when known; say "Unknown" only if truly unknown.
- Solution: provide the next command, flag, config change, or decision.
- Debug metadata: include request IDs, trace IDs, paths, and docs links only when helpful.

Avoid:

```text
Error: failed
```

## Warning Design

Warnings should say whether execution continues:

```text
⚠ Config file not found
  Cause: sentra.toml was not found in this workspace
  Continuing with defaults.
```

If a warning changes results, make that explicit:

```text
⚠ Partial report generated
  Skipped: 3 private packages
  Next:    run `cli auth login` and retry for full coverage
```

## Dangerous Operations

| Risk | Confirmation |
| --- | --- |
| Reversible | execute, then show brief confirmation |
| Moderate | ask yes/no or Enter/Esc confirmation |
| Destructive single resource | require typing the resource name |
| Destructive batch | require dry-run preview plus explicit `--yes` or typed confirmation |
| Automation | require explicit flags such as `--yes`, `--force`, or `--confirm <name>` |

Never block CI or scripts with an interactive prompt unless the user explicitly requested interactivity.

## Keyboard-First Interaction

Use familiar keys:

| Key | Meaning |
| --- | --- |
| Enter | confirm, select, continue |
| Esc | cancel, back, close |
| q | quit |
| j/k | move down/up |
| h/l | move left/right or collapse/expand |
| `/` | search |
| ? | help |
| Tab | next focus |
| Shift+Tab | previous focus |

Always show the currently available action near the prompt or footer:

```text
  Press Enter to continue, Esc to cancel.
```

## Recovery

A good failure ends with at least one recovery path:

- retry the same command
- run a repair command
- change a flag
- authenticate
- inspect logs
- open docs
- contact support with a debug ID

Do not make users infer recovery from stack traces.
