## 1. Snippet rendering changes

- [x] 1.1 In `snippet_line()` (`src/tui/results_list.rs`), prepend a `…` span in muted style before parsing the HTML content, deducting 1 column from the width budget
- [x] 1.2 After the HTML parsing loop, append a `…` span in muted style (if width budget remains)
- [x] 1.3 Normalize Tantivy's inter-fragment `...` (three ASCII dots) to ` … ` (spaced Unicode ellipsis) before parsing — use a string replacement on the input HTML before the rendering loop

## 2. Tests

- [x] 2.1 Add a test that `snippet_line` output starts with `…` and ends with `…` for a typical snippet
- [x] 2.2 Add a test that inter-fragment `...` is rendered as ` … ` in the output spans
- [x] 2.3 Add a test that ellipsis characters are included within the width budget on narrow widths (no overflow)
