# Scan Output Design Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Render `sentra scan` terminal output with a calmer Codex-style hierarchy while preserving JSON output.

**Architecture:** Keep scan data collection unchanged. Improve only terminal formatting in `src/cli/output.rs`, using existing ANSI helpers and table helpers where possible.

**Tech Stack:** Rust, Cargo tests, existing `serde_json` terminal formatter, existing ANSI escape helpers.

## Global Constraints

- Keep `--format json` structure unchanged.
- Keep stdout as final command output and stderr as progress/log output.
- Honor existing `NO_COLOR`, TTY, and `--output` color disabling behavior.
- Avoid bright white or broad high-contrast styling; default text stays terminal default.
- Do not add dependencies.
- Do not hardcode new user-visible strings outside the existing CLI output layer in this task.

---

### Task 1: Summary And Severity Index

**Files:**
- Modify: `src/cli/output.rs`

**Interfaces:**
- Consumes: existing `format_scan_results(value: &serde_json::Value, color: bool) -> String`
- Produces: terminal output that starts with a scan summary and table rows that show severity distribution.

- [ ] **Step 1: Write the failing test**

Add a unit test in `src/cli/output.rs`:

```rust
#[test]
fn scan_terminal_output_summarizes_findings_by_severity() {
    let value = serde_json::json!([
        {
            "source": "path",
            "data": {"name": "math-calculator"},
            "report": {
                "findings": [
                    {"severity": "CRITICAL", "title": "Reverse shell", "category": "MALICIOUS_EXECUTION", "checker": "llm-checker", "file": "SKILL.md", "location": {"line": 1}},
                    {"severity": "HIGH", "title": "Prompt injection", "category": "PROMPT_INJECTION", "checker": "llm-checker", "file": "SKILL.md", "location": {"line": 2}}
                ],
                "errors": []
            }
        }
    ]);

    let output = format_scan_results(&value, false);

    assert!(output.starts_with("Scan complete: 1 target, 2 findings, 0 errors\n"));
    assert!(output.contains("Risk: critical=1 high=1 medium=0 low=0 info=0"));
    assert!(output.contains("TARGET"));
    assert!(output.contains("RISK"));
    assert!(output.contains("critical=1 high=1"));
}
```

- [ ] **Step 2: Verify red**

Run: `cargo test scan_terminal_output_summarizes_findings_by_severity`

Expected: FAIL because the summary and `RISK` table column do not exist.

- [ ] **Step 3: Implement minimal formatter support**

Add small helpers in `src/cli/output.rs`:

```rust
#[derive(Default, Clone, Copy)]
struct SeverityCounts {
    critical: usize,
    high: usize,
    medium: usize,
    low: usize,
    info: usize,
}
```

Implement helpers to count severities from each report's `findings`, sum totals, and format labels as `critical=1 high=1 medium=0 low=0 info=0`. Update `format_scan_results` to print:

```text
Scan complete: N target(s), M finding(s), E error(s)
Risk: critical=C high=H medium=M low=L info=I

Scan Results (N)
...
```

Use singular/plural for `target`, `finding`, and `error`.

- [ ] **Step 4: Verify green**

Run: `cargo test scan_terminal_output_summarizes_findings_by_severity`

Expected: PASS.

### Task 2: Risk-First Detail Layout

**Files:**
- Modify: `src/cli/output.rs`

**Interfaces:**
- Consumes: existing `append_scan_details` and `append_finding_detail`
- Produces: details sorted by severity and headed by severity, target, and title.

- [ ] **Step 1: Write the failing test**

Add a unit test in `src/cli/output.rs`:

```rust
#[test]
fn scan_terminal_output_lists_findings_by_risk_before_target_grouping() {
    let value = serde_json::json!([
        {
            "source": "path",
            "data": {"name": "low-skill"},
            "report": {
                "findings": [
                    {"severity": "LOW", "title": "Low risk", "category": "SUPPLY_CHAIN", "checker": "hash-checker", "file": "LOW.md", "location": {"line": 1}}
                ],
                "errors": []
            }
        },
        {
            "source": "path",
            "data": {"name": "critical-skill"},
            "report": {
                "findings": [
                    {"severity": "CRITICAL", "title": "Critical risk", "category": "MALICIOUS_EXECUTION", "checker": "llm-checker", "file": "CRITICAL.md", "location": {"line": 1}}
                ],
                "errors": []
            }
        }
    ]);

    let output = format_scan_results(&value, false);

    let critical = output.find("CRITICAL  critical-skill  Critical risk").unwrap();
    let low = output.find("LOW       low-skill       Low risk").unwrap();
    assert!(critical < low);
    assert!(output.contains("Location"));
    assert!(output.contains("Checker"));
}
```

- [ ] **Step 2: Verify red**

Run: `cargo test scan_terminal_output_lists_findings_by_risk_before_target_grouping`

Expected: FAIL because current details are grouped by target order.

- [ ] **Step 3: Implement minimal detail ordering**

Create a local detail item struct inside the output module or helper function:

```rust
struct FindingRenderItem<'a> {
    target: String,
    finding: &'a serde_json::Value,
}
```

Collect all findings across scan items, sort by severity rank `CRITICAL`, `HIGH`, `MEDIUM`, `LOW`, `INFO`, unknown last, then render each heading as:

```text
  CRITICAL  target-name  Finding title
```

Keep existing detail fields and context rendering.

- [ ] **Step 4: Verify green**

Run: `cargo test scan_terminal_output_lists_findings_by_risk_before_target_grouping`

Expected: PASS.

### Task 3: Low-Stimulus Color Contract

**Files:**
- Modify: `src/cli/output.rs`

**Interfaces:**
- Consumes: existing `styled(value, style, color)` helper
- Produces: no bright white ANSI styling and no color when `color=false`.

- [ ] **Step 1: Write the failing test**

Add a unit test in `src/cli/output.rs`:

```rust
#[test]
fn scan_terminal_output_color_avoids_bright_white_and_disables_cleanly() {
    let value = serde_json::json!([
        {
            "source": "path",
            "data": {"name": "critical-skill"},
            "report": {
                "findings": [
                    {"severity": "CRITICAL", "title": "Critical risk", "category": "MALICIOUS_EXECUTION", "checker": "llm-checker", "file": "CRITICAL.md", "location": {"line": 1}, "evidence": "danger"}
                ],
                "errors": []
            }
        }
    ]);

    let colored = format_scan_results(&value, true);
    assert!(!colored.contains("\u{1b}[97m"));
    assert!(!colored.contains("\u{1b}[1;97m"));

    let plain = format_scan_results(&value, false);
    assert!(!plain.contains("\u{1b}["));
}
```

- [ ] **Step 2: Verify red or existing-pass**

Run: `cargo test scan_terminal_output_color_avoids_bright_white_and_disables_cleanly`

Expected: PASS is acceptable if the current code already avoids bright white; keep the test as a regression guard.

- [ ] **Step 3: Adjust styles only if needed**

If the test fails, remove bright white style codes from `AnsiStyle` and keep only red, yellow, magenta, dim, and bold/default-safe styles.

- [ ] **Step 4: Verify scan output tests**

Run: `cargo test --lib output`

Expected: PASS for output module tests.

### Task 4: CLI Regression

**Files:**
- Modify: `tests/cli_list.rs` only if existing assertions require the previous exact words.

**Interfaces:**
- Consumes: existing scan CLI behavior.
- Produces: passing CLI tests with the new terminal rendering.

- [ ] **Step 1: Run existing CLI scan tests**

Run: `cargo test scan_skill`

Expected: PASS or failures only where assertions refer to old terminal labels.

- [ ] **Step 2: Update assertions only if needed**

If tests fail because of expected terminal labels, update assertions to check stable semantic text:

```rust
assert!(stdout.contains("Scan complete:"));
assert!(stdout.contains("Risk:"));
assert!(stdout.contains("CRITICAL"));
assert!(stdout.contains("Remediation"));
```

- [ ] **Step 3: Run final verification**

Run:

```bash
cargo test
cargo run -- scan skill .\fixtures\skill\ --with-llm --no-cache
```

Expected: tests pass; manual output shows calm summary, risk table, risk-first details, and no bright white styling.
