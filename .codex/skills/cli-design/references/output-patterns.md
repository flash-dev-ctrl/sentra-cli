# Output Patterns

Use this reference to design human and machine output contracts.

## Canonical Flow

```text
Context
Action
Progress
Result
Next Step
```

Every long-running command should expose at least context, current action, and result. Include next step when the result requires a human decision.

Operationally transparent output should state the phase, action, goal, and essential context while work is happening:

```text
[3/12] Scan skills
  Path:  ~/.codex/skills
  Found: 7 skills
```

## Output Composition Grammar

Compose human output from line roles before writing final text:

| Role | Purpose | Visual form |
| --- | --- | --- |
| `headline` | task start or major state | unindented status symbol plus short noun/verb phrase |
| `metadata` | supporting context | two-space indent, `Label: value` |
| `phase` | progress step | two-space indent, `[i/N] <Imperative action>` or status symbol |
| `result` | completed outcome | unindented `✓`, `✗`, or `⚠` headline |
| `next_step` | user action or follow-up | two-space indent, starts with `Next:` or prompt text |
| `diagnostic` | debug or verbose detail | two-space indent, muted when styled |

Use blank lines between context, progress, result, and next-step groups. Do not create flat-stack output where every line has equal visual weight.

## Layout Templates

Short operation:

```text
● <Task>
  <Label>: <value>

✓ <Result>
  <Label>: <value>
```

Counted operation:

```text
● <Task>
  <Label>: <value>

  [1/N] <Imperative action>
  [2/N] <Imperative action>

✓ <Result>
  <Label>: <value>
```

Long-running operation:

```text
● <Task>
  Goal:   <goal>
  Target: <target>

  ✓ <Completed phase>
  ◌ <Current phase>
  ○ <Pending phase>

  <Heartbeat or current action>
```

Failure:

```text
✗ <Failure title>

  Problem:
  <What failed>

  Cause:
  <Why it likely failed>

  Solution:
  Run:
    <command>
```

Polished compact output:

```text
● Install agent
  Agent: opencode

  [1/2] Try npm install
  [2/2] Verify executable

✓ Agent installed
  Name: opencode
```

## Microcopy Rules

- Phase lines use imperative verb phrases: `Try npm install`, `Verify executable`.
- Results use completed state: `Agent installed`, `Report generated`.
- Avoid awkward internal phrasing like `install finished` when a direct result exists.
- Avoid duplicate completion. A success headline plus metadata is enough.
- Use sentence case consistently for actions and labels.
- Remove trailing spaces.

## Key-Value Layout

- With one metadata line, do not align artificially.
- With two or more metadata lines in the same block, align values after the longest label by rendered terminal cell width.
- Keep labels short and stable.
- Do not mix unindented metadata with progress lines.
- Put the colon immediately after the label, then pad after the colon. Avoid padding before the colon, because `Agent   :` and `模型      :` look uneven in mixed-language output.
- Strip ANSI escape sequences before measuring width. Color and style codes are zero-width.
- Measure Unicode by terminal display width, not bytes or code points. CJK characters are usually 2 cells; combining marks and ANSI escapes are 0 cells.
- If a metadata block mixes languages, prefer one label language when possible. Mixed labels are acceptable, but they must still align by display width.

Example:

```text
● Install agent
  Agent:  opencode
  Method: npm
  Path:   ~/.local/bin
```

Mixed English and CJK labels:

```text
✓ 模型供应商已更新
  Agent:    sentra
  Base URL: https://ai-api-gateway.app.baizhi.cloud/api/openai
  模型:     dev/gpt-5.5
  协议:     chat_completions
```

Prefer fully localized labels when the surrounding output is localized:

```text
✓ 模型供应商已更新
  代理:     sentra
  基础 URL: https://ai-api-gateway.app.baizhi.cloud/api/openai
  模型:     dev/gpt-5.5
  协议:     chat_completions
```

## stdout and stderr

| Stream | Use for |
| --- | --- |
| stdout | Intended result, generated data, machine JSON, values intended for pipes |
| stderr | Progress, spinners, warnings, diagnostics, prompts, debug logs |

When stdout is redirected, do not write progress to stdout. When stderr is redirected, avoid live redraw unless explicitly enabled.

## Default Mode Patterns

Task start:

```text
● Install dependencies
  Workspace: sentra-cli
  Target:    42 packages
```

Phase transition:

```text
✓ Discover workspace
✓ Load configuration
◌ Analyze dependencies
○ Install packages
○ Verify lockfile
```

Running action:

```text
▶ Fetch package metadata
  Registry: https://registry.npmjs.org
```

Counted action:

```text
  [3/12] Scan ~/.codex/skills
  Found: 7 skills
```

Successful result:

```text
✓ Dependencies installed
  Added:    18
  Updated:  4
  Duration: 12.4s
```

Warning with continuation:

```text
⚠ Lockfile was updated
  Cause: package constraints resolved to newer patch versions
  Next:  review the diff before committing
```

Failed result:

```text
✗ Installation failed

Problem:
  Permission denied while writing node_modules

Cause:
  Current user cannot access the target directory

Solution:
  Run:
    cli repair permissions
```

Next-step prompt:

```text
▶ Continue deployment?
  Target:  production
  Changes: 3 services

  Press Enter to continue, Esc to cancel.
```

## Verbose Mode

Verbose mode may include:

- config file paths and precedence
- environment and version details
- retries and backoff timing
- request IDs and correlation IDs
- cache hit/miss information
- command lifecycle events
- expanded tool output

Keep verbose mode structured and scannable:

```text
● Build project
  Workspace: sentra-cli
  Profile:   release

Verbose:
  Config:    E:\ct\sentra-cli\sentra.toml
  Cache:     C:\Users\user\AppData\Local\sentra\cache
  Toolchain: rustc 1.88.0

  ✓ Load config 18ms
  ✓ Resolve workspace 42ms
  ◌ Compile crates
```

## Machine Mode

Machine mode must:

- emit stable JSON or another documented structured format
- use stdout for data
- avoid ANSI styling, spinners, prompts, and prose-only messages
- include machine-readable status and errors
- preserve schema compatibility or version the schema

Example:

```json
{
  "status": "failed",
  "command": "install",
  "workspace": "sentra-cli",
  "error": {
    "code": "permission_denied",
    "problem": "Cannot write node_modules",
    "cause": "Current user cannot access the target directory",
    "solution": "Run: cli repair permissions"
  }
}
```

## Avoid

- `Processing...` with no phase or heartbeat.
- Silent waiting followed by a surprising final result.
- Printing every percent when the number has no decision value.
- Progress bars that appear stuck near completion without explaining the active final phase.
- Flat-stack output with no indentation, grouping, or visual hierarchy.
- Duplicate completion lines that say the same success twice.
- Mixing result data and progress on stdout.
- Human phrases in fields that automation must parse.
- Dumping raw logs before a concise summary.
