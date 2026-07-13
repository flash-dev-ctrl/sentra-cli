# Color System

Use color as semantic feedback, never as decoration. Optimize for long-session readability: calm contrast, low saturation, and clear hierarchy without glare.

## Design Tokens

Define tokens by purpose:

| Token | Purpose |
| --- | --- |
| `background` | terminal or app background |
| `foreground` | primary readable text |
| `primary` | active command, current action, link, or accent |
| `focus` | keyboard focus, active cursor, or current choice |
| `selection` | selected row or highlighted option background |
| `secondary` | supporting labels and structure |
| `muted` | metadata, timestamps, IDs, low-priority details |
| `border` | separators, table rules, low-emphasis structure |
| `success` | completed, safe, added, passed |
| `warning` | caution, partial success, recoverable issue |
| `error` | failure, destructive, blocked |
| `info` | neutral status, explanation, discovery |

## Aesthetic Direction

The default look should feel calm, precise, and native to a developer terminal. Use a cool neutral base, restrained semantic accents, and enough contrast to read comfortably for long sessions.

- Prefer quiet hierarchy over a branded color wash. The palette should not make routine output feel like an alert.
- Do not force a custom background for ordinary line-oriented CLI output; let the terminal theme carry the page unless the app owns a full-screen interface.
- Use warm colors only when they carry meaning, such as warning or error. A warm-heavy screen quickly feels noisy.
- Keep active state distinct from semantic state: focus is not warning, and selection is not success.
- Design the monochrome version first. The colored version should feel clearer, not like a different interface.

## Dark Terminal Native Theme

Default to a low-saturation dark terminal palette inspired by VS Code Dark, Catppuccin, and Tokyo Night. Avoid pure white (`#ffffff`) as the default foreground; it creates glare on dark terminals and makes every line feel equally loud.

| Token | Semantic color | Example truecolor |
| --- | --- | --- |
| `background` | deep neutral | `#111318` |
| `foreground` | softened text | `#b8beca` |
| `primary` | calm blue | `#7b9fc8` |
| `focus` | brighter calm blue | `#9eb7d8` |
| `selection` | blue-gray surface | `#263241` |
| `secondary` | quiet slate | `#8e98a8` |
| `muted` | dim gray | `#646d7a` |
| `border` | dark separator | `#2a303a` |
| `success` | muted green | `#84b38a` |
| `warning` | muted amber | `#caa75f` |
| `error` | softened red | `#cc7070` |
| `info` | muted cyan-blue | `#6f9fbd` |

These values are defaults, not hardcoded requirements. Implementations should map tokens to the host color library and terminal capabilities.

Against the default dark background, the palette is intentionally readable without glare: `foreground` is about 10:1 contrast, `secondary` about 6.4:1, `muted` about 3.6:1, and semantic accents about 5.4-8.1:1. `muted` is acceptable only for low-priority metadata, not instructions or error recovery text.

## Light Terminal Native Theme

When the app controls its colors and can confidently target a light terminal, use a darker, lower-saturation companion palette. Do not apply dark-theme muted colors on a light background.

| Token | Semantic color | Example truecolor |
| --- | --- | --- |
| `background` | soft neutral | `#f7f8fb` |
| `foreground` | deep neutral | `#252a33` |
| `primary` | calm blue | `#2f6fa3` |
| `focus` | deep focus blue | `#215f8f` |
| `selection` | pale blue surface | `#e7eef7` |
| `secondary` | cool gray | `#4f5968` |
| `muted` | readable gray | `#687386` |
| `border` | light separator | `#d7dce5` |
| `success` | grounded green | `#2f7a45` |
| `warning` | grounded amber | `#8a6200` |
| `error` | grounded red | `#b04444` |
| `info` | grounded cyan-blue | `#24708d` |

The light palette favors slightly darker accents so status symbols remain readable. Avoid pastel semantic colors for text on light backgrounds; they often look pleasant in isolation but fail in real terminal output.

## Luminance Ladder

Build hierarchy through a luminance ladder, not by making everything bright. On the default dark background:

| Layer | Token | Target contrast | Use |
| --- | --- | --- | --- |
| Normal text | `foreground` | about 8-10:1 | task/result phrases, action text |
| Supporting text | `secondary` | about 5.5-7:1 | values, completed phase text, less-important labels |
| Quiet metadata | `muted` | about 3.5-4.5:1 | timestamps, counters, IDs, pending items |
| Semantic accents | `success`, `warning`, `error`, `info` | readable but restrained | symbols, short labels, critical words |

Semantic colors are accents. They should not replace the text hierarchy or color whole lines by default.

## Contrast Acceptance

Use contrast as a quality gate, not as the whole aesthetic:

- Normal text should target at least 7:1 contrast and must stay at or above 4.5:1.
- Help text, recovery instructions, prompts, and error details must stay at or above 4.5:1.
- Metadata may sit around 3:1 only when it is genuinely nonessential and paired with layout, labels, or proximity.
- Semantic text accents should stay at or above 4.5:1 against the active background.
- Selection and focus states must preserve readable selected text at or above 4.5:1.
- Disabled or pending states may be dimmer, but they must remain legible when the terminal is used in a bright room or copied into logs.

## Visual Comfort Rules

- Do not use pure white (`#ffffff` or ANSI bright white) for normal text. Reserve the brightest foreground only for rare focus or selected text.
- Do not make all headlines bold. Use symbol, position, spacing, and wording before text weight.
- Keep most text in `foreground`; use `primary` sparingly for the current action or focused control.
- Keep metadata in `muted` or dim styling. Metadata should be readable but never compete with the result.
- Keep status colors medium-bright and low-saturation. Avoid neon green, pure red, and intense yellow on dark backgrounds.
- Prefer one colored element per line: usually the status symbol or short status label, not the whole sentence.
- Avoid stacking color plus bold plus underline unless the user must act immediately.
- Design the unstyled version first. Color should reinforce hierarchy, not create it.

## Color Density Budget

Keep color sparse enough that state changes are easy to notice:

- In a typical screen, only a small minority of visible glyphs should be colored accents.
- Use one semantic accent per output group unless multiple independent states are being compared.
- Keep routine progress mostly `foreground`, `secondary`, and `muted`; reserve semantic colors for status symbols and final states.
- Do not color every row in a table by status. Color the status word or symbol, and leave the row readable.
- If warnings or errors repeat, color the first symbol or label and summarize the rest in normal text.

## Color Application Grammar

Apply color in this order:

1. Color the status symbol first.
2. Keep the main phrase in `foreground`.
3. Style metadata labels in `muted`.
4. Style metadata values in `secondary` or `foreground`.
5. Style counters, timestamps, IDs, and pending states in `muted`.
6. Use semantic colors only for symbols, short labels, or critical words.
7. Use bold only for rare active focus or destructive confirmation, not normal headings.

Do not color full paragraphs, full error blocks, or all progress rows. If a line needs emphasis, color the symbol or label and keep the sentence readable.

## ANSI and Alignment

Color must never break terminal alignment:

- Calculate padding, truncation, and table widths from unstyled text using rendered terminal cell width.
- Add ANSI styling after layout has been computed, or use an ANSI-aware width library.
- Treat ANSI escape sequences as zero-width. Never count them as bytes, code points, or display columns.
- Keep padding spaces unstyled unless the terminal library requires otherwise for a selected row background.
- Style the label, symbol, or status word independently; do not wrap a whole padded line in color if it makes resets and spacing hard to reason about.
- Test mixed Latin/CJK examples and colored examples together. Output that aligns without color but shifts when styled is not acceptable.

Recommended emphasis budget:

| Element | Style |
| --- | --- |
| Task headline | symbol in `info` or `primary`, text in `foreground` |
| Current phase | symbol in `primary`, action text in `foreground` |
| Completed phase | symbol in `success`, text in `secondary` or `foreground` |
| Pending phase | symbol and text in `muted` |
| Metadata label | `muted` or dim |
| Metadata value | `secondary` or `foreground` |
| Final success | symbol in `success`, result text in `foreground` |
| Warning | symbol in `warning`, text in `foreground` |
| Error | symbol in `error`, title in `foreground`; details stay unbolded |

## Semantic Rules

- Success is muted green and paired with `✓` or a success label.
- Warning is muted amber/yellow and paired with `⚠` or a warning label.
- Error is softened red and paired with `✗` or an error label.
- Info is muted blue/cyan and paired with `●` or an info label.
- Metadata is gray/dim and never competes with the primary message.
- Primary/accent color is for focus, current action, links, and active selections.

Do not use color to make output "prettier." If color does not add meaning, remove it.

## Focus and Selection

Interactive prompts and selection lists need stronger affordance than passive output:

- Use `focus` for the current cursor, active prompt, or currently executable choice.
- Use `selection` as a background only when the terminal library supports it reliably; otherwise use a pointer symbol, inverse video, or a clear `>` marker.
- Do not use `success`, `warning`, or `error` as selection colors unless the selected item itself has that state.
- Keep selected-row text in `foreground` or a high-contrast inverse foreground.
- In line-oriented output, avoid background fills. They create visual blocks that can look broken in copied logs.

## ANSI Mapping

When limited to 16 ANSI colors, use normal ANSI colors before bright variants. ANSI bright variants are not the default theme; reserve them for explicit user-selected high-contrast modes.

| Token | Preferred ANSI |
| --- | --- |
| `foreground` | default foreground, not bright white |
| `primary` | blue |
| `focus` | blue or reverse video |
| `selection` | reverse video or terminal selection background |
| `secondary` | default foreground or bright black/gray |
| `muted` | bright black/gray or dim |
| `border` | bright black/gray or dim |
| `success` | green |
| `warning` | yellow |
| `error` | red |
| `info` | cyan or blue |

Avoid using ANSI bold as a color-brightening hack. In many terminals, bold maps to bright colors and causes the "everything is highlighted white" problem.

## Example Styling Intent

Human-readable output should look calm in a dark terminal:

```text
● Install agent          # symbol: info, text: foreground
  Agent: opencode        # label: muted, value: secondary

  [1/2] Try npm install  # counter: muted, action: foreground
  [2/2] Verify executable

✓ Agent installed        # symbol: success, text: foreground
  Name: opencode         # label: muted, value: secondary
```

Do not style this as all bright white or all bold. The hierarchy comes from grouping, indentation, symbols, and restrained semantic color.

Styling intent:

- `●`: `info`; `Install agent`: `foreground`
- `Agent:`: `muted`; `opencode`: `secondary`
- `[1/2]`: `muted`; `Try npm install`: `foreground`
- `✓`: `success`; `Agent installed`: `foreground`
- `Name:`: `muted`; `opencode`: `secondary`

## Capability and Accessibility

Support:

- `NO_COLOR`: disable color entirely.
- non-TTY output: disable ANSI and live UI by default.
- 16-color fallback: use standard ANSI roles rather than fragile custom hex assumptions.
- truecolor: enhance readability when available.
- color-blind users: never rely on red/green alone; distinguish states with symbols, labels, and wording.
- high-contrast user themes: respect the terminal's default foreground/background unless the user selects an app theme.

Use this fallback hierarchy:

1. If `NO_COLOR` is set, no color.
2. If stdout/stderr is not a TTY, no color unless forced.
3. If a light or dark terminal background is known and truecolor is supported, use the matching theme tokens.
4. If the background is unknown, prefer terminal-native foreground plus semantic ANSI accents instead of forcing background-specific muted colors.
5. If 256-color is supported, map to nearest semantic values.
6. Otherwise, use basic ANSI colors plus symbols and labels.

If terminal theme detection is unreliable, prefer terminal-native defaults over forcing a custom palette.

## Non-Color Hierarchy

Color must be supported by:

- symbols: `✓`, `✗`, `⚠`, `●`, `◌`, `○`, `▶`
- labels: `Problem`, `Cause`, `Solution`
- spacing and grouping
- indentation
- text weight where supported
- stable order

The CLI must remain understandable in monochrome logs.

## Theme Quality Checklist

Before accepting a CLI color theme, verify:

- Normal body text is not pure white or ANSI bright white.
- Less than a small minority of visible text is bold in a typical screen.
- Metadata is visibly quieter than task/result text.
- Success, warning, error, and info are distinguishable without becoming neon.
- Semantic colors are accents, not full-line paint.
- Warning yellow does not dominate the screen.
- The same output remains readable with color disabled.
- Dark terminal output does not feel like a wall of highlighted text.
- The palette works for long sessions: no glare, no saturated color flood, no bold-only hierarchy.

Reject themes that:

- color entire paragraphs instead of status symbols or short labels
- make every headline bright white
- rely on bold to create all hierarchy
- use high-saturation red/yellow/green as large text blocks
- use ANSI bright variants by default
- override user terminal colors without an explicit theme option
