# Tables and Lists

Use tables and lists when users need to scan, compare, or select resources.

## Table Rules

- Align columns.
- Left-align text.
- Right-align numbers.
- Compute column widths by rendered terminal cell width after stripping ANSI escape sequences.
- Treat CJK wide characters as 2 cells and combining marks as 0 cells. Do not align tables by bytes, UTF-16 length, Rust `chars().count()`, or JavaScript `string.length`.
- Keep IDs visible but muted or shortened when safe.
- Avoid heavy borders; whitespace and headers are usually enough.
- Sort by the most actionable or surprising column by default.
- Truncate predictably with an ellipsis when the terminal is narrow.
- Provide a JSON or structured alternative for automation.
- Apply color after calculating padding and truncation, or use a table renderer that is ANSI-aware.

Default table:

```text
Name              Status    Version   Size
api               ready     1.8.2      42 MB
worker            warning   1.8.1      38 MB
dashboard         ready     1.8.2      51 MB
```

Verbose table:

```text
Name              Status    Version   Size    Updated              ID
api               ready     1.8.2      42 MB   2026-07-09 10:42     svc_8f21
worker            warning   1.8.1      38 MB   2026-07-09 10:40     svc_91bc
dashboard         ready     1.8.2      51 MB   2026-07-09 10:39     svc_a81d
```

JSON alternative:

```json
{
  "services": [
    {
      "name": "api",
      "status": "ready",
      "version": "1.8.2",
      "size_bytes": 44040192,
      "id": "svc_8f21"
    }
  ]
}
```

## List Rules

Use lists for sequences, phases, choices, and summaries.

## List Rhythm

- Keep indentation stable across the whole output.
- Separate task context, progress, result, and next step with blank lines.
- Use metadata blocks for properties of one object.
- Use phase lists for workflow state.
- Use tables for comparing multiple objects with the same fields.
- Do not visually merge metadata blocks and phase lists.

Phase list:

```text
  ✓ Discover workspace
  ✓ Load configuration
  ◌ Analyze dependencies
  ○ Generate report
```

Choice list:

```text
Select environment:
  1. staging     safe test environment
  2. production  live customer traffic
```

Grouped result:

```text
✓ Audit complete

  Changed:
    package.json
    pnpm-lock.yaml

  Unchanged:
    README.md
```

Metadata block:

```text
● Install agent
  Agent:  opencode
  Method: npm
  Path:   ~/.local/bin
```

Phase block:

```text
● Install agent
  Agent: opencode

  [1/2] Try npm install
  [2/2] Verify executable

✓ Agent installed
  Name: opencode
```

## Responsive Behavior

For narrow terminals:

- hide low-priority metadata first
- shorten IDs but preserve disambiguation
- wrap descriptions below the primary row
- avoid horizontal scrolling in default mode
- expose full data in verbose or JSON mode

## Display Width Alignment

Terminal layout must align by visible cells:

- ANSI color/style sequences are zero-width.
- CJK ideographs, kana, and many full-width symbols are usually 2 cells.
- Combining marks, variation selectors, and zero-width joiners should not add padding width.
- Emoji width varies by terminal; avoid using emoji as structural table content unless the renderer handles width correctly.
- Truncate by display width and preserve valid ANSI reset sequences.

If the implementation cannot calculate display width reliably, avoid dense tables and prefer compact metadata blocks or JSON output.

## Anti-Patterns

- ASCII art boxes around every table.
- Columns that shift between rows.
- Columns that align in raw strings but not in the terminal because ANSI or CJK width was counted incorrectly.
- Flat-stack lists with no grouping or indentation.
- Metadata lines mixed into phase lists without spacing.
- Human-readable sizes only in JSON.
- JSON arrays whose field names change by command.
- Truncating the only unique identifier.
