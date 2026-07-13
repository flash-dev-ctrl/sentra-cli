---
name: cli-design
description: Design and implement trustworthy modern command-line application experiences. Use for CLI UX, developer tools, command output design, progress feedback, status visibility, error messages, interactive CLI flows, AI agent CLI execution views, terminal color tokens, table/list output, stdout/stderr behavior, verbose/debug output, and machine-readable JSON or structured output modes.
---

# CLI Design

Use this skill to design CLI applications that feel visible, understandable, and trustworthy. Treat output as an interaction system, not decoration: every line should help the user know what is happening, what changed, whether the tool is alive, and what they can do next.

## Design Workflow

1. Identify the command type: one-shot, long-running, interactive, automation-first, or AI-agent workflow.
2. Model trust questions: Is it alive? What is it doing? Why is it doing that? How far along is it? How do I recover or continue?
3. Design information architecture: Primary, Secondary, Metadata.
4. Compose human output from line roles before writing text.
5. Select output modes: default, verbose, machine.
6. Apply semantic feedback: status, progress, result, next step.
7. Validate against the anti-pattern checklist before implementing.

The default interaction structure is:

```text
Context
Action
Progress
Result
Next Step
```

## Reference Routing

Load only the reference files needed for the task:

| Task | Read |
| --- | --- |
| Overall CLI UX strategy, HCI, Unix principles, IA, disclosure | `references/design-principles.md` |
| Human output templates, stdout/stderr, default/verbose/machine modes | `references/output-patterns.md` |
| Terminal colors, design tokens, dark theme, `NO_COLOR`, semantic color use | `references/color-system.md` |
| Status symbols, spinners, long-running tasks, phase progress | `references/status-progress.md` |
| Error messages, confirmations, keyboard interaction, dangerous actions | `references/errors-interaction.md` |
| Tables, lists, alignment, truncation, JSON alternatives | `references/tables-lists.md` |
| AI agent CLI state, planning/tool/verification views, approval and sandbox feedback | `references/ai-agent-cli.md` |

## Core Rules

- Compose output from line roles before writing text: headline, metadata, phase, result, next step, and diagnostic.
- Group context, progress, result, and next step with stable spacing. Use two-space indentation for supporting human-output lines.
- Never let long-running work stay silent. Show a spinner, phase transition, heartbeat, or progress indicator within a short human-noticeable delay.
- Design against idle aversion: when work takes time, continuously prove the program is alive with useful status updates, streaming output, or a spinner.
- Make operations transparent: show phase, action, goal, and necessary context instead of waiting silently and revealing only the final result.
- Use goal-gradient feedback: as completion approaches, make remaining work and completed phases increasingly visible.
- Prefer phase-based progress over noisy percentage updates unless the percentage is accurate and useful.
- Use semantic color only. Color communicates meaning: success, warning, error, info, metadata, focus, or selection.
- Design for visual comfort in long terminal sessions. Avoid pure-white foreground, all-bold output, high-saturation status colors, and high-contrast glare.
- Never rely on color alone. Pair color with symbols, labels, layout, or typography.
- Separate human output from machine output. Machine mode must use a stable structured schema and avoid ANSI styling.
- Preserve scriptability. Keep intended data on stdout; put progress, warnings, diagnostics, and prompts on stderr.
- Make errors actionable with `Problem -> Cause -> Solution`.
- Design dangerous operations with explicit confirmation and automation-safe flags.
- Provide Unicode status symbols by default, but require ASCII fallback for CI, logs, non-UTF-8 terminals, non-TTY output, and machine mode.

## Anti-Pattern Checklist

Reject designs that:

- Print only `Processing...` for a long operation.
- Produce flat-stack output where every line has equal visual weight.
- Mix unindented metadata, progress, and result lines in one block.
- Use gerund-style counted phases such as `[1/2] trying npm`; use imperative actions such as `[1/2] Try npm install`.
- Print duplicate completion messages such as a success headline followed by `Installed opencode`.
- Leave the screen static during model inference, network calls, scans, builds, or other opaque work.
- Perform opaque work with no phase, goal, or current action.
- Hide operational facts such as `[3/12] scanning ~/.codex/skills, found 7 skills` until the command ends.
- Let a progress bar stall in the final 20% without phase, heartbeat, or explanation.
- Use decorative color or color-only meaning.
- Render most output as bright white or bold highlighted text.
- Use color palettes that feel neon, harsh, or visually tiring on dark terminals.
- Apply semantic color to entire sentences, paragraphs, or blocks instead of symbols, labels, or critical words.
- Let warning yellow dominate the screen.
- Use ANSI bright variants as the default foreground or default status palette.
- Mix progress text into stdout where scripts expect parseable results.
- Dump raw logs by default instead of summarizing what matters.
- Hide recovery instructions behind generic errors like `Error: failed`.
- Use unstable JSON fields or human prose in machine mode.
- Ask for confirmation in automation mode without an explicit non-interactive alternative.
- Treat full-screen TUI concerns as ordinary CLI output. For Ratatui, Ink, Textual, Bubbletea, or persistent terminal applications, combine this skill with a TUI design skill.
