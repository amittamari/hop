# Popular TUI Research

**Date:** 2026-06-05
**Status:** Research note for future TUI improvements
**Scope:** Popular terminal applications and the interaction patterns worth
borrowing for `hop`.

## Summary

The best-loved TUIs are not loved because they draw more boxes. They are loved
because they make a narrow, repetitive workflow feel immediate, inspectable, and
keyboard-native.

For `hop`, the strongest comparison set is `fzf`, `lazygit`, `k9s`, `yazi`, and
`btop`. Full editors and multiplexers such as Neovim, Helix, tmux, and Zellij
are larger products, but they reinforce the same lessons: users tolerate
learning dense keymaps when the current mode is visible, help is close, state is
stable, and the tool rewards muscle memory.

## Popularity Snapshot

GitHub stars are an imperfect proxy, but they are useful for choosing reference
tools as of this review date:

| Tool | Role | GitHub stars observed | Relevance to hop |
| --- | --- | ---: | --- |
| Neovim | editor/platform | ~100k | Extensibility, modal muscle memory, embedded terminal workflows |
| fzf | fuzzy finder/toolkit | ~80.9k | Search-first interaction, live filtering, preview split |
| lazygit | git workflow TUI | ~79k | Multi-pane task flow, contextual actions, readable command mapping |
| Helix | editor | ~44.8k | Visible modal editing, batteries-included defaults |
| Yazi | file manager | ~39.1k | Async previews, fast navigation, rich but responsive panes |
| k9s | Kubernetes dashboard | ~33.9k | Command palette, filters, aliases, stateful resource navigation |
| Zellij | terminal workspace | ~33.5k | Beginner and power-user bridge, visible modes/layouts |
| btop | resource monitor | ~32.7k | Dense status display, responsiveness, mouse fallback, themeability |

Sources:

- fzf: https://github.com/junegunn/fzf
- lazygit: https://github.com/jesseduffield/lazygit
- k9s: https://github.com/derailed/k9s
- yazi: https://github.com/sxyazi/yazi
- btop: https://github.com/aristocratos/btop
- Neovim: https://github.com/neovim/neovim
- Helix: https://github.com/helix-editor/helix
- Zellij: https://github.com/zellij-org/zellij

## Why They Are Loved

### 1. They Compress Real Workflows

`lazygit` turns hard-to-remember or awkward Git operations into direct actions:
stage selected lines, rebase from a commit list, cherry-pick, filter, custom
commands, undo, and worktree operations. Users love it because the UI removes
ceremony from tasks they already perform.

`k9s` does the same for Kubernetes: navigate resources, filter, jump contexts and
namespaces, view logs/YAML, shell into pods, port-forward, and refresh from a
single surface.

`hop` implication: the core experience should not stop at search + Enter. It
should make the next likely actions cheap:

- Resume normally.
- Resume with yolo through an explicit confirmation.
- Copy session directory.
- Copy a compact session reference.
- Open the session JSONL/source path.
- Toggle/copy the current query or filters.
- Narrow by current row's repo, branch, agent, or date.

### 2. Search Is Always Live, But Commands Are Still Discoverable

`fzf` is a benchmark for search-first terminal UX. It is fast, programmable,
integrates into shells/editors, supports dynamic reloads, and has a preview pane
with configurable position, size, and bindings.

`hop` already has the right high-level shape: type to search, move the
selection, preview the focused result. The main gap is polish around text input.
If a search box is always live, printable characters should type unless the user
has intentionally entered a command/navigation mode.

### 3. Preview Panes Carry Trust

`fzf`, `yazi`, `k9s`, and `lazygit` all use detail panes to make selection safe.
Users act quickly because the focused row explains itself before they commit.

`hop` implication: the transcript preview should continue to be the credibility
anchor. Improvements should focus on:

- A stable header with agent, repo, branch, directory, time, and source status.
- Match count and current match position.
- One-key jump between matches.
- Better scroll position feedback.
- Clear fallback when the source file is gone.

### 4. The Best TUIs Have a Command Vocabulary

`k9s` uses `:` commands and `/` filters. `fzf` has a programmable binding model.
`lazygit` exposes custom commands. These tools do not rely only on a footer with
every possible key. They have a small command grammar that scales.

`hop` already has query filters (`agent:`, `dir:`, `date:`). A future command
layer could stay separate from search:

- `/` edits the search query in modal mode.
- `:` opens command entry in modal mode.
- Commands could include `copy-dir`, `copy-id`, `filter-repo`, `filter-branch`,
  `rebuild`, `toggle-pr`, `open-source`, and `theme`.

This should be added only if the command surface solves real repeated actions;
otherwise it will just add keymap burden.

### 5. Mode Visibility Matters More Than Modal Power

Neovim, Helix, k9s, and Zellij can be dense because the user can build muscle
memory around visible states and stable semantics. Hidden mode changes are what
make TUIs feel brittle.

`hop` already tracks modal navigation state, but the current footer does not
make the sub-mode obvious. This matches `P-006 TUI Mode Visibility Gap` in
`docs/ARCHITECTURE.md`.

Recommended direction:

- Default search preset footer: `SEARCH  type query | ↑↓ move | Enter resume | ? help | Esc quit`
- Modal search sub-mode: `SEARCH  Esc nav | Tab complete | Enter resume | Ctrl+C quit`
- Modal nav sub-mode: `NAV  j/k move | / search | Enter resume | ? help | Esc quit`

### 6. Responsive Rendering Is Product Quality, Not Just Performance

`yazi` emphasizes non-blocking async I/O, task cancellation, previews, and
cross-terminal media support. `btop` emphasizes a responsive UI, filtering,
sorting, selectable graph symbols, process detail, mouse fallback, and themes.
Ratatui's own guidance warns that computationally demanding or I/O-intensive
work in the update path can make the app hang.

`hop` already aligns well here:

- Background sync.
- Background PR enrichment.
- Viewport-bounded row rendering.
- On-demand selected-row preview.
- No broad work in the render path.

Future TUI work should preserve these invariants. New features should be modeled
as explicit effects/messages, not as direct work during render.

## Recommended Improvements For hop

### Highest Leverage

1. **Fix search-input command conflicts.** Plain `?`, `[`, and `]` should type
   when the query is non-empty, or move those commands to modified chords. This
   keeps the default preset true to the search-palette model.
2. **Make mode state visible.** Add a compact `SEARCH` / `NAV` indicator and
   mode-specific footer text.
3. **Make `Ctrl+C` global.** It should exit even when help is open.
4. **Add real query editing.** Support cursor position, Left/Right, Home/End,
   Delete, `Ctrl+A`, `Ctrl+E`, and `Ctrl+W`.
5. **Add match navigation in preview.** Users need `n`/`N` or equivalent to move
   through matches inside the selected transcript.

### Next Layer

6. **Add row-context commands.** Copy directory/source/session id; filter by
   selected repo/branch/agent/date; open source JSONL.
7. **Add sorting/toggle affordances.** Sort by time, repo, agent, message count,
   PR; toggle hidden/visible columns without editing config.
8. **Make scroll/paging viewport-aware.** Page movement should follow rendered
   height, not fixed constants.
9. **Improve status density.** Show sync state, result count, filters, PR
   pending count, and preview match position in predictable slots rather than a
   long appended footer string.
10. **Theme and accessibility polish.** Implement the reserved theme config and
    offer at least default, no-icons, high-contrast, and low-color profiles.

## Design Principles To Keep

- Keep the default search-first. `hop` is closer to `fzf` than to an editor.
- Preserve direct keyboard activation: type, select, Enter.
- Use preview as the trust surface.
- Keep slow work out of rendering.
- Make modal behavior opt-in, visible, and internally consistent.
- Let advanced actions grow through a command vocabulary instead of filling the
  footer with more one-off shortcuts.

