# Risk Scan Concurrency Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add bounded, thread-safe concurrency to sentra-lib risk input scanning and sentra-cli target scanning.

**Architecture:** Preserve serial checker order inside one input, but run independent inputs and targets concurrently. Use `Arc<RiskChecker>` and a locked cache to remove `Rc<RefCell<_>>` from scanner sharing.

**Tech Stack:** Rust 2024, Tokio, futures stream combinators, sentra-lib, sentra-cli.

## Global Constraints

- Default concurrency is `4`.
- `ScanOptions.concurrency` values of `None` or `0` resolve to `4`.
- Output order must match input target order.
- No hardcoded user-visible CLI text changes are required for this feature.
- Keep hash/Yara/LLM/TI order unchanged inside a single input.

---

### Task 1: sentra-lib Input Concurrency

**Files:**
- Modify: `sentra-lib/src/risks/types.rs`
- Modify: `sentra-lib/src/interfaces.rs`
- Modify: `sentra-lib/src/risks/checkers/unified.rs`
- Test: `sentra-lib/tests/risk_llm_http_checker.rs`

**Interfaces:**
- Produces: `ScanOptions { concurrency: Option<usize>, ... }`
- Produces: `RiskChecker::concurrency(&self) -> usize`
- Produces: `Checker: Send + Sync`

- [ ] Write a failing delayed LLM test with three inputs and 300ms server delay.
- [ ] Run the test and confirm elapsed time is serial.
- [ ] Add `ScanOptions.concurrency`.
- [ ] Make `Checker` objects `Send + Sync`.
- [ ] Move `CheckResultCache` behind `std::sync::Mutex`.
- [ ] Use `futures::stream::iter(...).buffer_unordered(concurrency)` in `RiskChecker::scan`.
- [ ] Preserve cache behavior by checking cache before scheduling and inserting after each task completes.
- [ ] Run `cargo test --test risk_llm_http_checker delayed_llm_inputs_are_scanned_concurrently`.

### Task 2: Thread-Safe RiskScanner Sharing

**Files:**
- Modify: `sentra-lib/src/risks/scanners/unified.rs`
- Modify: `sentra-lib/src/risks/scanners/skill.rs`
- Modify: `sentra-lib/src/risks/scanners/cron.rs`
- Modify: `sentra-lib/src/risks/scanners/memory.rs`
- Modify: `sentra-lib/src/risks/scanners/provider.rs`

**Interfaces:**
- Consumes: `Arc<RiskChecker>`
- Produces: `RiskScanner::concurrency(&self) -> usize`

- [ ] Replace `Rc<RefCell<RiskChecker>>` with `Arc<RiskChecker>`.
- [ ] Change scanner methods to call `checker.scan(...)` through shared references.
- [ ] Keep rule loading methods requiring `&mut self` and update `Arc::get_mut` before sharing.
- [ ] Run `cargo test --test risk_skill_scanner`.

### Task 3: sentra-cli Target Concurrency

**Files:**
- Modify: `src/cli/scan.rs`
- Test: `tests/cli_list.rs`

**Interfaces:**
- Consumes: `RiskScanner::concurrency(&self) -> usize`
- Produces: stable ordered `Vec<ScanRecord>` after concurrent target scans.

- [ ] Add a CLI/order regression test if an existing fixture can expose ordering.
- [ ] Use bounded concurrent target scanning.
- [ ] Carry target index through each future and sort results before output.
- [ ] Update progress to count completions.
- [ ] Run scan-related CLI tests.

### Task 4: Verification And Commits

**Files:**
- Verify all modified files.

- [ ] Run `rustfmt --edition 2024 --check` for modified Rust files.
- [ ] Run `cargo test --manifest-path sentra-lib/Cargo.toml --test risk_llm_http_checker`.
- [ ] Run `cargo test --manifest-path sentra-lib/Cargo.toml --test risk_skill_scanner`.
- [ ] Run `cargo check` in `sentra-lib`.
- [ ] Run `cargo check` in `sentra-cli`.
- [ ] Commit `sentra-lib` with a Chinese message.
- [ ] Commit the top-level submodule pointer and CLI/doc changes with a Chinese message.
