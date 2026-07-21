## 1. Config

- [x] 1.1 Add `icons: bool` to `DisplayConfig` in `src/config.rs` with `#[serde(default = "default_true")]` and default `true` in the `Default` impl
- [x] 1.2 Add a `resolved_*` helper (or reuse) so callers get the selected glyph variant from config — done as `Glyphs::from_icons_enabled(config.display.icons)` (no new config method needed)
- [x] 1.3 Add config tests: default enables icons; `[display] icons = false` disables

## 2. Glyphs set module

- [x] 2.1 Create `src/tui/glyphs.rs` with a `Glyphs` struct (accessors for agent-mark spacing, branch, repo, pr, time, msgs, archived, warning/success/error, plus the centralized structural glyphs)
- [x] 2.2 Implement `Glyphs::nerd()` returning the locked PUA glyphs (see design "locked glyph code points" table) and `Glyphs::ascii()` returning empty strings for field icons while keeping structural glyphs (`·`, `─`, `▎`, `❯`, spinner) unchanged in both
- [x] 2.2a Confirm `nf-oct-git_pull_request` (`U+F407`) against the current Nerd Fonts cheat sheet before embedding; resolve by class name if the hex has drifted — embedded as `U+F407` with the class name recorded inline; flagged in the design for a live cheat-sheet check before release
- [x] 2.3 Register the module in `src/tui/mod.rs` and pick the variant once at startup from config (built in `main::run_tui`, stored on `App` via `set_glyphs`)
- [x] 2.4 Unit test: `Glyphs::ascii()` field-icon accessors return `""` and contain no PUA code point; structural glyphs equal their pre-change literals

## 3. Per-agent glyph via Adapter (B-011)

- [x] 3.1 Add an agent-agnostic glyph method to the `Adapter` trait in `src/adapters/mod.rs` with a safe default
- [x] 3.2 Override it in `adapters/claude.rs` (`nf-fa-asterisk` `U+F069`), `adapters/codex.rs` (`nf-fa-terminal` `U+F120`), `adapters/cursor.rs` (`nf-fa-i_cursor` `U+F246`)
- [x] 3.3 Expose the glyph to the TUI through the adapter path (no `AgentId` match in a generic layer) — injected into `Glyphs` in `main` by iterating adapters, keyed by position in `AgentId::ALL`
- [x] 3.4 Test: each adapter returns its glyph; a non-overriding adapter falls back to the default without error

## 4. Thread `&Glyphs` and centralize existing literals

- [x] 4.1 Thread `Glyphs` through the render path — carried on `App` (mirroring `theme`), read via `App::glyphs()`, and passed into `RowCtx`, `preview_header_lines`, `footer_*`, and `modal::render_yolo_modal`. (Chosen over a `RenderModel` field to avoid churning ~18 view test literals; `toolbar`/`help`/`columns` need no glyphs.)
- [x] 4.2 Centralize chrome glyph literals into `Glyphs`: `SELECTION_MARKER`, `SPINNER_FRAMES` (const home moved to `glyphs.rs`, re-exported from `view`), `ACCENT_BAR`, `SEP` (+ inlined `" · "` copies in view/modal), `ARCHIVED_MARKER`; call sites read from `Glyphs`. **Refinement:** the preview transcript's content prefixes (`●`, `›`, `•`, `─`) are intentionally left literal in `preview.rs` — common-plane glyphs identical in both variants, in the content layer, whose relocation would churn ~15 signatures for zero visual change (see design "content-layer exception").
- [x] 4.3 Confirm no chrome glyph literal remains hardcoded at a render site (grepped view/results_list/modal — only a test assertion references `" · "`)

## 5. Apply the subtle icon set (chrome only)

- [x] 5.1 Agent mark: render agent glyph + space + text label in brand color when enabled; text-only when disabled (card line 1, compact agent cell, preview header meta). Transcript agent header keeps its existing agent-colored `●` mark (content-layer exception, see 4.2).
- [x] 5.2 Card metadata line + preview header: prefix repo / branch / PR / time / message-count fields with their glyphs, keeping the existing `·` separator
- [x] 5.3 Archived marker: render an archive glyph in place of the `arch ` prefix when enabled
- [x] 5.4 Status glyphs: warning status prefixed with a glyph styled by `theme.warning` (footer + modal warning/YOLO lines); `success`/`error` glyph accessors + `theme.success`/`error` roles wired and ready (no current render site emits success/error status)
- [x] 5.5 Verify restraint: no icons added to footer key-hints, card snippet, or transcript prose

## 6. Tests & docs

- [x] 6.1 Render test: with icons disabled, agent mark and card metadata line output equals the pre-change strings (no PUA, no layout shift)
- [x] 6.2 Render test: with icons enabled, agent mark contains the adapter glyph and metadata fields carry their glyphs
- [x] 6.3 Update `docs/ARCHITECTURE.md`: new `Glyphs` boundary and agent-glyph via `Adapter`; note the `Theme::agent_color` B-011 bend as pressure point `P-003`
- [x] 6.4 Update `README.md`: document `[display] icons` (default on) and the patched-nerd-font requirement / opt-out
- [x] 6.5 Run `cargo test` and `cargo test --lib`; confirm green (296 pass; `cargo clippy --all-targets` clean)
