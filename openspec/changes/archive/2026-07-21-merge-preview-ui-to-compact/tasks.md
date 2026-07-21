## 1. Unify rendering in PreviewState

- [x] 1.1 Remove `use_separators` parameter from `PreviewState::update` (`src/tui/preview.rs:426`); always call `render_transcript_with_separators` at line 460-470
- [x] 1.2 Update the `preview.update(…)` call in `src/main.rs:345-353` to drop the `row_style == RowStyle::Card` argument

## 2. Update the public render_transcript wrapper

- [x] 2.1 Add `width: u16` parameter to `render_transcript` (`src/tui/preview.rs:159`) and have it delegate to `render_transcript_with_separators` instead of `render_transcript_with_terms`
- [x] 2.2 Update callers in `src/tui/view/tests_preview.rs` (lines 32, 147, 208) and `src/tui/preview.rs` tests (lines 499, 513, 522, 529, 667) to pass a width argument

## 3. Remove dead code

- [x] 3.1 Delete `render_transcript_with_terms` (`src/tui/preview.rs:169-225`)
- [x] 3.2 Delete `prefix_first` (`src/tui/preview.rs:311-317`) and `indent` (`src/tui/preview.rs:319+`) helpers
- [x] 3.3 Verify no remaining references to deleted functions

## 4. Update tests

- [x] 4.1 Update `transcript_has_role_prefixes` test (`src/tui/preview.rs:497-508`) — replace assertions on `"› fix the auth bug"` and `"● CLAUDE"` with assertions on separator format (`"── user"` / `"── claude"`)
- [x] 4.2 Run `cargo test` and fix any remaining test failures from the signature and format changes
