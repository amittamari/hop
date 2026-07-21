## Context

`flatten_messages` concatenates all message blocks with `\n`, producing a flat string stored in Tantivy's `content` field. The `SnippetGenerator` extracts a text window around matched terms from this flat string. Because there are no message boundaries, snippets routinely span multiple messages — mixing user prompts and assistant responses into a single unreadable fragment.

## Goals / Non-Goals

**Goals:**
- Snippets are always scoped to a single message
- The existing `snippet_line` renderer works unchanged (same `<b>term</b>` HTML format)
- Searchability is unaffected — the same terms match the same sessions

**Non-Goals:**
- Role-aware snippets (showing "User:" / "Agent:" prefixes) — out of scope for now
- Multi-fragment snippets (showing multiple match locations) — single best match is enough
- Changing the tokenizer or query parser

## Decisions

### 1. Message separator: ASCII Record Separator (`\x1E`)

**Choice**: Insert `\x1E` between messages in `flatten_messages`. Blocks within a message stay `\n`-separated.

**Why not `\n\n`?** Prose blocks can contain paragraph breaks (`\n\n`), making it unreliable as a message boundary.

**Why `\x1E`?** It's the ASCII control character designed for record separation. It won't appear in conversation text. Tantivy's tokenizer treats it as a non-alphanumeric boundary, so it doesn't affect token indexing.

### 2. Custom snippet builder instead of SnippetGenerator

**Choice**: Replace `SnippetGenerator` with a function that splits stored content by `\x1E`, scores each chunk by term hits, and extracts a KWIC window from the best chunk.

**Why not post-process SnippetGenerator output?** The generator picks its own window before we can intervene. If the window center falls on a message boundary, truncating to one side loses the match context. Building our own gives full control.

**Why not store messages as separate Tantivy fields?** Tantivy doesn't support variable-length repeated fields well. A single text field with a known separator is simpler and already works with the existing schema.

### 3. Snippet builder algorithm

```
build_snippet(content, terms, max_len) → Option<String>
  1. Split content by '\x1E' → chunks
  2. For each chunk, count case-insensitive term occurrences → pick highest
  3. In the winning chunk, find the byte position of the first term match
  4. Center a window of max_len chars around that position
  5. Wrap all term occurrences within the window in <b>…</b>
  6. Return HTML string (same format as SnippetGenerator)
```

Terms come from `query::parse().free_terms()` which is already used in the search path.

### 4. Schema version bump

Bump `SCHEMA_VERSION` so existing indexes auto-wipe and rebuild with the new separator. This is the established migration pattern — no manual `--rebuild` needed after upgrading.

## Risks / Trade-offs

- **Rebuild required** → Mitigated by schema version bump triggering automatic rebuild on next launch.
- **Single-message window may be narrow for short messages** → The snippet will still show the match in context; a short message is itself good context. If the best-matching message is very short, the snippet is just that message (no padding from neighbors).
- **Case-insensitive matching in snippet builder must align with Tantivy's analyzer** → Use simple lowercasing (same as Tantivy's `SimpleTokenizer`). No stemming to worry about since we don't use a stemmer.
