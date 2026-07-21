## Why

Search snippets currently straddle multiple messages because `flatten_messages` joins all blocks from all messages with a single `\n` — no message boundaries. Tantivy's `SnippetGenerator` picks a text window around the match term without knowing where one message ends and another begins, producing snippets that mash user and assistant text together into unreadable fragments.

## What Changes

- **Replace Tantivy `SnippetGenerator` with a custom single-message snippet builder.** At search time, the stored content is split back into per-message chunks, the chunk with the best term match is selected, and a KWIC window is extracted within that single message.
- **Add a message separator to `flatten_messages`.** Use the ASCII Record Separator (`\x1E`) between messages so they can be split back at search time. Blocks within a message remain `\n`-separated.
- **Requires index rebuild** (`--rebuild`) since the stored content format changes.

## Capabilities

### New Capabilities

_(none)_

### Modified Capabilities

- `search-snippets`: Snippets are scoped to a single message instead of spanning arbitrary text windows across the full conversation. Generation switches from Tantivy `SnippetGenerator` to a custom builder.
- `search-index`: The indexed `content` field uses `\x1E` as the message separator instead of `\n`. KWIC snippet generation uses the custom builder instead of `SnippetGenerator`.

## Impact

- **`src/core.rs`**: `flatten_messages` changes separator between messages.
- **`src/index.rs`**: Replace `SnippetGenerator` usage with custom snippet builder; bump `SCHEMA_VERSION` to trigger auto-rebuild.
- **`tests/index_sync.rs`**: Update snippet-related tests for new format.
- **No TUI changes**: The snippet output stays in `<b>term</b>` HTML format — the existing `snippet_line` renderer works as-is.
