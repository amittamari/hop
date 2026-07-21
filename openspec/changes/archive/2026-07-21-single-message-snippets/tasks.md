## 1. Message separator in flatten_messages

- [x] 1.1 Add a `MSG_SEP` constant (`'\x1E'`) to `src/core.rs`
- [x] 1.2 Change `flatten_messages` to insert `MSG_SEP` between messages (keep `\n` between blocks within a message)
- [x] 1.3 Update the `flatten_messages_joins_prose_and_code` test to verify the new separator

## 2. Custom snippet builder

- [x] 2.1 Implement `build_snippet(content: &str, terms: &[String], max_len: usize) -> Option<String>` in `src/index.rs`: split by `MSG_SEP`, score chunks by term hits, extract KWIC window from best chunk, wrap matches in `<b>` tags
- [x] 2.2 Replace `SnippetGenerator` usage in `SearchIndex::search()` with `build_snippet`, passing `free_terms()` from the parsed query
- [x] 2.3 Remove the `tantivy::snippet::SnippetGenerator` import

## 3. Schema version bump

- [x] 3.1 Increment `SCHEMA_VERSION` in `src/index.rs` to trigger automatic index rebuild

## 4. Tests

- [x] 4.1 Add unit tests for `build_snippet`: single message match, multi-message best-pick, term wrapping, max-length windowing, short message fits entirely, no matches returns None
- [x] 4.2 Update `search_with_query_produces_snippets` in `tests/index_sync.rs` to verify snippets are scoped to a single message
