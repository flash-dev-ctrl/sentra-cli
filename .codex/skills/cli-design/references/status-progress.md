# Status and Progress

Use this reference when designing system status visibility and long-running task feedback.

## Status Iconography

Use a consistent status language:

| Symbol | Meaning | ASCII fallback |
| --- | --- | --- |
| `✓` | Success, completed | `[OK]` |
| `✗` | Error, failed | `[ERR]` |
| `⚠` | Warning, caution | `[WARN]` |
| `●` | Info, context | `[INFO]` |
| `◌` | Running, active | `[*]` |
| `○` | Pending, queued | `[ ]` |
| `▶` | Action, command, next step | `>` |

Use Unicode in interactive human output when supported. Use ASCII fallback for CI, logs, non-UTF-8 terminals, non-TTY output, and machine mode.

## Long-Running Work

Long-running commands must show:

- current phase
- current action
- target or goal
- why the action is happening when the reason is not obvious
- heartbeat or progress
- completion confirmation for each phase
- clear remaining work

Example:

```text
● Generate report
  Goal: analyze dependency risk

  ✓ Discover workspace
  ✓ Load configuration
  ◌ Analyze dependencies
  ○ Render report
  ○ Write summary
```

## Progress Types

| Type | Use when | Pattern |
| --- | --- | --- |
| Spinner | duration is unknown | `◌ Analyzing dependencies` |
| Phase progress | workflow has known stages | completed/running/pending phase list |
| Progress bar | accurate total exists | bytes, files, tests, rows |
| Heartbeat | opaque task may take time | periodic "still running" status with current target |
| Streaming events | meaningful events arrive over time | concise event list with bounded detail |
| Token or text streaming | model output is being generated | stream partial content or show inference status |

Do not show fake precision. A truthful phase list is better than a misleading percentage.

## Counted Progress

Use counted progress when the operation has a small known sequence. Counted phases must use imperative action text:

```text
● Install agent
  Agent: opencode

  [1/2] Try npm install
  [2/2] Verify executable

✓ Agent installed
  Name: opencode
```

Rules:

- Use `[i/N] <imperative action>`.
- Prefer `Try npm install` over `trying npm`.
- Use `✓` for final completed outcomes, not every counted phase unless phases remain visible as a checklist.
- Do not combine counted progress and final result into redundant lines.
- Keep counted phase lines indented under the task context.

## Goal Gradient Effect

As the command nears completion:

- increase specificity about remaining tasks
- confirm each completed phase
- avoid a long silent final phase
- avoid letting the final 20% of a progress bar appear frozen; switch to named final phases or heartbeat updates when completion cannot be estimated
- show final verification explicitly
- end with a clear result and next step

Example:

```text
  ✓ Compile project
  ✓ Run tests
  ◌ Verify package
  ○ Publish release

  Remaining: verify package, publish release
```

Then:

```text
  ✓ Compile project
  ✓ Run tests
  ✓ Verify package
  ◌ Publish release

  Remaining: publish release
```

## Timing Guidance

- Avoid showing a spinner for very fast operations; delay spinner display briefly to prevent flicker.
- For tasks longer than a few seconds, show phase status or heartbeat.
- For tasks longer than a minute, include enough context for the user to know what is still happening.
- For model inference, show a spinner, streamed text, or state updates such as `Planning`, `Calling tool`, `Reading results`, and `Verifying`.
- On cancellation, show what was canceled and whether partial changes were made.

## Avoid

```text
Processing...
```

```text
Downloading 1%
Downloading 2%
Downloading 3%
```

Prefer:

```text
  ✓ Initialize
  ✓ Validate manifest
  ◌ Download packages
  ○ Verify checksums
  ○ Link dependencies
```
