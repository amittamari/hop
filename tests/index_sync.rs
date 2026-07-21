use hop::core::{AgentId, ScanEntry, Session, SessionSummary};
use hop::index::{SearchIndex, diff, diff_authoritative};
use hop::query;
use std::collections::HashMap;
use std::path::PathBuf;

fn sess(id: &str, title: &str, content: &str, agent: AgentId, ts: i64, mtime: i64) -> Session {
    Session {
        meta: SessionSummary {
            id: id.into(),
            agent,
            title: title.into(),
            directory: "/work/api".into(),
            timestamp: ts,
            message_count: 1,
            ..Default::default()
        },
        content: content.into(),
        mtime,
    }
}

fn sess_in_dir(
    id: &str,
    title: &str,
    content: &str,
    agent: AgentId,
    directory: &str,
    ts: i64,
    mtime: i64,
) -> Session {
    let mut s = sess(id, title, content, agent, ts, mtime);
    s.meta.directory = directory.into();
    s
}

#[test]
fn build_search_and_reconstruct() {
    let dir = tempfile::tempdir().unwrap();
    let idx = SearchIndex::open_or_create(dir.path()).unwrap();

    idx.upsert(&sess("a", "auth refresh", "token bug", AgentId::Claude, 100, 1)).unwrap();
    idx.upsert(&sess("b", "unrelated", "nothing here", AgentId::Codex, 90, 1)).unwrap();
    idx.commit().unwrap();
    idx.reload().unwrap();

    let q = query::parse("auth");
    let results = idx.search(&q, query::SortOrder::Relevance, 1000, 50).unwrap();
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].id, "a");
    assert_eq!(results[0].title, "auth refresh");
    assert_eq!(results[0].agent, AgentId::Claude);
}

#[test]
fn exact_ranks_above_fuzzy() {
    let dir = tempfile::tempdir().unwrap();
    let idx = SearchIndex::open_or_create(dir.path()).unwrap();
    idx.upsert(&sess("exact", "refactor", "refactor", AgentId::Claude, 100, 1)).unwrap();
    idx.upsert(&sess("fuzzy", "refacter", "refacter", AgentId::Claude, 200, 1)).unwrap();
    idx.commit().unwrap();
    idx.reload().unwrap();

    let q = query::parse("refactor");
    let results = idx.search(&q, query::SortOrder::Relevance, 1000, 50).unwrap();
    assert_eq!(results[0].id, "exact"); // exact boosted above edit-distance-1
}

#[test]
fn text_search_breaks_equal_scores_by_recency() {
    let dir = tempfile::tempdir().unwrap();
    let idx = SearchIndex::open_or_create(dir.path()).unwrap();
    idx.upsert(&sess("old", "shared topic", "shared topic", AgentId::Claude, 100, 1)).unwrap();
    idx.upsert(&sess("new", "shared topic", "shared topic", AgentId::Claude, 200, 1)).unwrap();
    idx.commit().unwrap();
    idx.reload().unwrap();

    let q = query::parse("shared");
    let results = idx.search(&q, query::SortOrder::Relevance, 1000, 50).unwrap();
    assert_eq!(results.len(), 2);
    assert_eq!(results[0].id, "new");
    assert_eq!(results[1].id, "old");
}

#[test]
fn agent_filter_applies() {
    let dir = tempfile::tempdir().unwrap();
    let idx = SearchIndex::open_or_create(dir.path()).unwrap();
    idx.upsert(&sess("c", "deploy", "ship it", AgentId::Claude, 100, 1)).unwrap();
    idx.upsert(&sess("x", "deploy", "ship it", AgentId::Codex, 100, 1)).unwrap();
    idx.commit().unwrap();
    idx.reload().unwrap();

    let q = query::parse("deploy agent:codex");
    let results = idx.search(&q, query::SortOrder::Relevance, 1000, 50).unwrap();
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].agent, AgentId::Codex);
}

#[test]
fn incremental_diff_detects_changes_and_deletions() {
    let mut known: HashMap<String, i64> = HashMap::new();
    known.insert("keep".into(), 100);
    known.insert("changed".into(), 100);
    known.insert("deleted".into(), 100);

    let mut scanned: HashMap<String, ScanEntry> = HashMap::new();
    scanned.insert("keep".into(), ScanEntry { path: PathBuf::from("k"), mtime: 100 });
    scanned.insert("changed".into(), ScanEntry { path: PathBuf::from("c"), mtime: 500 });
    scanned.insert("new".into(), ScanEntry { path: PathBuf::from("n"), mtime: 10 });

    let (changed, deleted) = diff(&known, &scanned);
    let mut changed_ids: Vec<&String> = changed.iter().map(|(id, _)| id).collect();
    changed_ids.sort();
    assert_eq!(changed_ids, vec![&"changed".to_string(), &"new".to_string()]);
    assert_eq!(deleted, vec!["deleted".to_string()]);
}

#[test]
fn authoritative_diff_deletes_only_successfully_scanned_agents() {
    let mut known: HashMap<String, i64> = HashMap::new();
    known.insert("claude:gone".into(), 100);
    known.insert("codex:preserve".into(), 100);

    let scanned: HashMap<String, ScanEntry> = HashMap::new();
    let authoritative = [AgentId::Claude].into_iter().collect();

    let (_changed, mut deleted) = diff_authoritative(&known, &scanned, &authoritative);
    deleted.sort();
    assert_eq!(deleted, vec!["claude:gone".to_string()]);
}

#[test]
fn empty_query_returns_all_sorted_by_recency() {
    let dir = tempfile::tempdir().unwrap();
    let idx = SearchIndex::open_or_create(dir.path()).unwrap();
    idx.upsert(&sess("old", "a", "x", AgentId::Claude, 100, 1)).unwrap();
    idx.upsert(&sess("new", "b", "y", AgentId::Claude, 200, 1)).unwrap();
    idx.commit().unwrap();
    idx.reload().unwrap();

    let q = query::parse("");
    let results = idx.search(&q, query::SortOrder::Recent, 1000, 50).unwrap();
    assert_eq!(results.len(), 2);
    assert_eq!(results[0].id, "new"); // newest first
}

#[test]
fn sort_oldest_reverses_recent_order() {
    let dir = tempfile::tempdir().unwrap();
    let idx = SearchIndex::open_or_create(dir.path()).unwrap();
    idx.upsert(&sess("old", "a", "x", AgentId::Claude, 100, 1)).unwrap();
    idx.upsert(&sess("new", "b", "y", AgentId::Claude, 200, 1)).unwrap();
    idx.commit().unwrap();
    idx.reload().unwrap();

    let q = query::parse("");
    let recent = idx.search(&q, query::SortOrder::Recent, 1000, 50).unwrap();
    let oldest = idx.search(&q, query::SortOrder::Oldest, 1000, 50).unwrap();
    assert_eq!(recent[0].id, "new");
    assert_eq!(oldest[0].id, "old"); // Oldest flips the order
    assert_eq!(oldest[1].id, "new");
}

#[test]
fn known_mtimes_maps_document_key_to_mtime() {
    let dir = tempfile::tempdir().unwrap();
    let idx = SearchIndex::open_or_create(dir.path()).unwrap();
    idx.upsert(&sess("a", "t", "c", AgentId::Claude, 100, 42)).unwrap();
    idx.upsert(&sess("b", "t", "c", AgentId::Codex, 100, 7)).unwrap();
    idx.commit().unwrap();
    idx.reload().unwrap();

    let map = idx.known_mtimes().unwrap();
    assert_eq!(map.len(), 2);
    assert_eq!(map.get("claude:a"), Some(&42));
    assert_eq!(map.get("codex:b"), Some(&7));
}

#[test]
fn raw_session_id_can_overlap_between_agents() {
    let dir = tempfile::tempdir().unwrap();
    let idx = SearchIndex::open_or_create(dir.path()).unwrap();
    idx.upsert(&sess("same", "claude row", "shared", AgentId::Claude, 100, 11)).unwrap();
    idx.upsert(&sess("same", "codex row", "shared", AgentId::Codex, 90, 22)).unwrap();
    idx.commit().unwrap();
    idx.reload().unwrap();

    let results =
        idx.search(&query::parse("shared"), query::SortOrder::Relevance, 1_000, 10).unwrap();
    assert_eq!(results.len(), 2);
    assert!(results.iter().any(|s| s.id == "same" && s.agent == AgentId::Claude));
    assert!(results.iter().any(|s| s.id == "same" && s.agent == AgentId::Codex));

    idx.delete("claude:same").unwrap();
    idx.commit().unwrap();
    idx.reload().unwrap();

    let results =
        idx.search(&query::parse("shared"), query::SortOrder::Relevance, 1_000, 10).unwrap();
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].agent, AgentId::Codex);
}

#[test]
fn dir_filter_pages_past_many_filtered_out_hits() {
    let dir = tempfile::tempdir().unwrap();
    let idx = SearchIndex::open_or_create(dir.path()).unwrap();
    for i in 0..5_100 {
        idx.upsert(&sess_in_dir(
            &format!("other-{i}"),
            "recent",
            "recent",
            AgentId::Claude,
            "/work/other",
            10_000 + i as i64,
            i as i64,
        ))
        .unwrap();
    }
    idx.upsert(&sess_in_dir(
        "target",
        "target",
        "target",
        AgentId::Claude,
        "/work/target",
        1,
        9_999,
    ))
    .unwrap();
    idx.commit().unwrap();
    idx.reload().unwrap();

    let results =
        idx.search(&query::parse("dir:target"), query::SortOrder::Recent, 20_000, 1).unwrap();
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].id, "target");
}

#[test]
fn branch_roundtrips_through_index() {
    use hop::core::{AgentId, Session, SessionSummary};
    use hop::index::SearchIndex;
    use hop::query::ParsedQuery;
    let dir = tempfile::tempdir().unwrap();
    let idx = SearchIndex::open_or_create(dir.path()).unwrap();
    let s = Session {
        meta: SessionSummary {
            id: "a".into(),
            agent: AgentId::Codex,
            title: "t".into(),
            directory: "/w".into(),
            timestamp: 1,
            message_count: 1,
            branch: Some("feat/x".into()),
            repo_url: Some("git@github.com:me/web.git".into()),
            source_path: Some(std::path::PathBuf::from("/sessions/a.jsonl")),
            ..Default::default()
        },
        content: "hello".into(),
        mtime: 1,
    };
    idx.upsert(&s).unwrap();
    idx.commit().unwrap();
    idx.reload().unwrap();
    let out = idx.search(&ParsedQuery::default(), query::SortOrder::Recent, 100, 10).unwrap();
    assert_eq!(out.len(), 1);
    assert_eq!(out[0].branch.as_deref(), Some("feat/x"));
    assert_eq!(out[0].repo_url.as_deref(), Some("git@github.com:me/web.git"));
    assert_eq!(out[0].source_path.as_deref(), Some(std::path::Path::new("/sessions/a.jsonl")));
}

#[test]
fn recency_boosts_recent_over_old_with_similar_text() {
    let dir = tempfile::tempdir().unwrap();
    let idx = SearchIndex::open_or_create(dir.path()).unwrap();
    let now = 1_700_000_000i64;
    let three_months_ago = now - 90 * 86_400;
    idx.upsert(&sess("old", "deploy api", "deploy api", AgentId::Claude, three_months_ago, 1))
        .unwrap();
    idx.upsert(&sess("new", "deploy api", "deploy api", AgentId::Claude, now, 1)).unwrap();
    idx.commit().unwrap();
    idx.reload().unwrap();

    let q = query::parse("deploy");
    let results = idx.search(&q, query::SortOrder::Relevance, now, 50).unwrap();
    assert_eq!(results.len(), 2);
    assert_eq!(results[0].id, "new");
    assert_eq!(results[1].id, "old");
}

#[test]
fn search_with_query_produces_snippets() {
    let dir = tempfile::tempdir().unwrap();
    let idx = SearchIndex::open_or_create(dir.path()).unwrap();
    idx.upsert(&sess(
        "a",
        "auth refresh",
        "the refresh token was expired so I refreshed the credential store",
        AgentId::Claude,
        100,
        1,
    ))
    .unwrap();
    idx.commit().unwrap();
    idx.reload().unwrap();

    let q = query::parse("refresh");
    let results = idx.search(&q, query::SortOrder::Relevance, 1000, 50).unwrap();
    assert_eq!(results.len(), 1);
    let snippet = results[0].snippet.as_deref().expect("snippet should be Some for a query match");
    assert!(snippet.contains("refresh"), "snippet should contain the query term, got: {snippet:?}");
}

#[test]
fn snippet_scoped_to_single_message() {
    let dir = tempfile::tempdir().unwrap();
    let idx = SearchIndex::open_or_create(dir.path()).unwrap();
    let content = "please implement the feature\x1EI will help you with that\x1Eimplement it now";
    idx.upsert(&sess("a", "implement feature", content, AgentId::Claude, 100, 1)).unwrap();
    idx.commit().unwrap();
    idx.reload().unwrap();

    let q = query::parse("implement");
    let results = idx.search(&q, query::SortOrder::Relevance, 1000, 50).unwrap();
    assert_eq!(results.len(), 1);
    let snippet = results[0].snippet.as_deref().expect("snippet present");
    assert!(snippet.contains("implement"), "snippet contains the term");
    assert!(!snippet.contains("I will help"), "snippet should not cross message boundaries");
}

#[test]
fn empty_query_produces_no_snippets() {
    let dir = tempfile::tempdir().unwrap();
    let idx = SearchIndex::open_or_create(dir.path()).unwrap();
    idx.upsert(&sess("a", "auth refresh", "token content", AgentId::Claude, 100, 1)).unwrap();
    idx.commit().unwrap();
    idx.reload().unwrap();

    let q = query::parse("");
    let results = idx.search(&q, query::SortOrder::Recent, 1000, 50).unwrap();
    assert_eq!(results.len(), 1);
    assert!(results[0].snippet.is_none(), "empty query should produce no snippet");
}
