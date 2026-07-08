# Risk Scan Concurrency Design

## Goal

Risk scanning should use bounded multi-thread-safe concurrency for independent scan work while preserving the existing per-input checker semantics.

## Current Behavior

`RiskChecker::scan` scans `CheckInput` values serially. The CLI scan command scans targets serially. The code is async, but the scheduling is sequential, so multiple LLM requests or online checks do not overlap.

## Design

Use bounded concurrency at two independent levels:

- In `sentra-lib`, `RiskChecker::scan` runs independent inputs concurrently.
- In `sentra-cli`, scan targets concurrently and restore input order before output.

Per-input checker order remains serial: hash, Yara, optional LLM review, then threat intel. This keeps hash whitelist/blacklist short-circuit behavior and the Yara/LLM final-review semantics intact.

## Thread Safety

`RiskChecker` becomes safe to share across concurrent tasks. Immutable scanning state is read through shared references. The cache is protected by a lock, so cache lookup and insert are serialized without serializing the scan itself. Scanner ownership changes from `Rc<RefCell<RiskChecker>>` to `Arc<RiskChecker>`.

Checker implementations must be `Send + Sync`. The trait object becomes `Box<dyn Checker + Send + Sync>`. Existing checker state is already immutable or uses thread-safe atomics/locks.

## Configuration

Add `concurrency` to `ScanOptions`. A missing, zero, or invalid value falls back to `4`. The same option is used by lib input scanning and CLI target scanning.

## Output And Progress

CLI output order remains stable by carrying each target index through the concurrent work and sorting finished records by index before rendering. Progress reports completed target count rather than assuming completion order.

## Error Handling

Each concurrent task returns its own scan result or error. The first fatal task error stops the command. Per-checker errors already stored in reports remain report data and do not abort scanning.

## Tests

Add a delayed LLM HTTP test proving multiple inputs run faster than serial execution. Add a CLI scan test proving JSON output order is stable under concurrent completion. Keep existing Yara/LLM tests passing.
