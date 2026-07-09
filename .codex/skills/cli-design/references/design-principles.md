# CLI Design Principles

Use these principles to turn a command into a trustworthy user experience.

## HCI Foundations

- System status visibility: always show whether the program is idle, running, waiting, blocked, failed, or complete.
- Idle aversion: users become anxious when the terminal is static during uncertain work. Show meaningful activity through spinner frames, streamed tokens, phase updates, heartbeat messages, or progress events.
- Operational transparency: expose what the command is doing, why it is doing it, and what target it is operating on.
- Goal-gradient effect: as work nears completion, make progress feel stronger by confirming completed phases and narrowing the visible remaining work.
- Recognition over recall: show current target, config source, selected workspace, active profile, and next action when they affect decisions.
- Error prevention: preview destructive changes, validate inputs early, and surface risky defaults before execution.
- User control: support cancellation, dry-run, retry, and explicit confirmation for irreversible operations.

## Unix CLI Principles

- Preserve composability. Output intended data to stdout and operational feedback to stderr.
- Prefer quiet success for tiny one-shot commands, but never make long-running work silent.
- Make commands scriptable with stable flags, exit codes, and machine output.
- Respect pipes and redirects. Disable ANSI styling, spinners, live redraw, and prompts when output is not a TTY unless explicitly forced.
- Return meaningful exit codes: `0` success, non-zero failure, and documented special codes when useful.

## Developer Tool UX

- Optimize the fast path while keeping inspection available.
- Explain consequences before mutation, not after.
- Make state inspectable: workspace, config, target, environment, auth, and version should be easy to reveal.
- Prefer reversible workflows. If an operation is not reversible, require stronger confirmation.
- Make feedback actionable. A user should know the next command or decision after every warning or failure.

## Information Architecture

Layer output by decision value:

| Layer | Purpose | Examples |
| --- | --- | --- |
| Primary | Current task, key state, final result | install started, validation failed, deployment complete |
| Secondary | Parameters, counts, versions, resource details | 42 packages, profile `prod`, node `v22.0.0` |
| Metadata | Time, IDs, debug, trace details | request id, duration, cache path, stack trace |

Default output should mostly contain Primary information with selective Secondary details. Verbose output may include Secondary and Metadata. Machine output should include all fields needed by automation with stable names and types.

## Progressive Disclosure

Design three modes:

| Mode | Audience | Contract |
| --- | --- | --- |
| Default | Humans making decisions | concise, high signal, no raw logs unless needed |
| Verbose | Debugging humans | lifecycle events, config sources, retries, timing, IDs |
| Machine | Automation | JSON or structured output, stable schema, no ANSI, no prose-only status |

Default mode is not "less correct"; it is curated. Verbose mode is not a log firehose; it is structured detail. Machine mode is not human output with braces; it is an API.

## Trust Questions

For every command, answer:

- What is the tool trying to do?
- What target is being changed or inspected?
- Why is the tool performing the current action?
- What is happening now?
- What has already completed?
- What remains?
- What decision does the user need to make?
- What should the user do if it fails?
