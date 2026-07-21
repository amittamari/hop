## 1. Channel plumbing

- [x] 1.1 Add `Update::UpgradeAvailable { latest: String }` variant to `engine::Update` enum in `src/engine.rs`
- [x] 1.2 In `run()` (`src/main.rs`): clone the `Sender<Update>` and pass it to the update-check background thread instead of using a `JoinHandle`
- [x] 1.3 In the background thread: call `check_for_update()`, and if `Some`, send `Update::UpgradeAvailable { latest: info.latest.to_string() }` through the sender
- [x] 1.4 Remove the post-exit `JoinHandle::join()` + `eprintln!` block (lines 245-247)

## 2. TUI state

- [x] 2.1 Add `update_available: Option<String>` field to `LoopState` in `src/main.rs`
- [x] 2.2 Handle `Update::UpgradeAvailable` in `process_sync()`: set `self.update_available` if `None` (write-once)
- [x] 2.3 Add `update: Option<String>` field to `StatusLine` in `src/tui/view/mod.rs`
- [x] 2.4 Wire `build_status()` to pass `self.update_available.clone()` into `StatusLine::update`

## 3. Footer rendering

- [x] 3.1 In `footer_status_line()` (`src/tui/view/footer.rs`): render `status.update` as `↑ v<version>` styled with `theme.accent`

## 4. Docs and cleanup

- [x] 4.1 Update `docs/ARCHITECTURE.md` line about stderr upgrade notice to reflect in-app display
- [x] 4.2 Verify `cargo test` passes (existing update-checker tests should be unaffected since `check_for_update` itself is unchanged)
