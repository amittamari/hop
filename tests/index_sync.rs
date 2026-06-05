use hop::core::{AgentId, ScanEntry, Session};
use hop::index::{diff, SearchIndex};
use hop::query;
use std::collections::HashMap;
use std::path::PathBuf;

fn sess(id: &str, title: &str, content: &str, agent: AgentId, ts: i64, mtime: i64) -> Session {
    Session {
        id: id.into(),
        agent,
        title: title.into(),
        directory: "/work/api".into(),
        timestamp: ts,
        content: content.into(),
        message_count: 1,
        mtime,
        yolo: false,
        branch: None,
        repo_url: None,
    }
}

#[test]
fn build_search_and_reconstruct() {
    let dir = tempfile::tempdir().unwrap();
    let idx = SearchIndex::open_or_create(dir.path()).unwrap();

    let mut w = idx.writer().unwrap();
    idx.upsert(&mut w, &sess("a", "auth refresh", "token bug", AgentId::Claude, 100, 1));
    idx.upsert(&mut w, &sess("b", "unrelated", "nothing here", AgentId::Codex, 90, 1));
    w.commit().unwrap();
    idx.reload().unwrap();

    let q = query::parse("auth");
    let results = idx.search(&q, 1000, 50).unwrap();
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].id, "a");
    assert_eq!(results[0].title, "auth refresh");
    assert_eq!(results[0].agent, AgentId::Claude);
}

#[test]
fn exact_ranks_above_fuzzy() {
    let dir = tempfile::tempdir().unwrap();
    let idx = SearchIndex::open_or_create(dir.path()).unwrap();
    let mut w = idx.writer().unwrap();
    idx.upsert(&mut w, &sess("exact", "refactor", "refactor", AgentId::Claude, 100, 1));
    idx.upsert(&mut w, &sess("fuzzy", "refacter", "refacter", AgentId::Claude, 100, 1));
    w.commit().unwrap();
    idx.reload().unwrap();

    let q = query::parse("refactor");
    let results = idx.search(&q, 1000, 50).unwrap();
    assert_eq!(results[0].id, "exact"); // exact boosted above edit-distance-1
}

#[test]
fn agent_filter_applies() {
    let dir = tempfile::tempdir().unwrap();
    let idx = SearchIndex::open_or_create(dir.path()).unwrap();
    let mut w = idx.writer().unwrap();
    idx.upsert(&mut w, &sess("c", "deploy", "ship it", AgentId::Claude, 100, 1));
    idx.upsert(&mut w, &sess("x", "deploy", "ship it", AgentId::Codex, 100, 1));
    w.commit().unwrap();
    idx.reload().unwrap();

    let q = query::parse("deploy agent:codex");
    let results = idx.search(&q, 1000, 50).unwrap();
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
fn empty_query_returns_all_sorted_by_recency() {
    let dir = tempfile::tempdir().unwrap();
    let idx = SearchIndex::open_or_create(dir.path()).unwrap();
    let mut w = idx.writer().unwrap();
    idx.upsert(&mut w, &sess("old", "a", "x", AgentId::Claude, 100, 1));
    idx.upsert(&mut w, &sess("new", "b", "y", AgentId::Claude, 200, 1));
    w.commit().unwrap();
    idx.reload().unwrap();

    let q = query::parse("");
    let results = idx.search(&q, 1000, 50).unwrap();
    assert_eq!(results.len(), 2);
    assert_eq!(results[0].id, "new"); // newest first
}

#[test]
fn known_mtimes_maps_id_to_mtime() {
    let dir = tempfile::tempdir().unwrap();
    let idx = SearchIndex::open_or_create(dir.path()).unwrap();
    let mut w = idx.writer().unwrap();
    idx.upsert(&mut w, &sess("a", "t", "c", AgentId::Claude, 100, 42));
    idx.upsert(&mut w, &sess("b", "t", "c", AgentId::Codex, 100, 7));
    w.commit().unwrap();
    idx.reload().unwrap();

    let map = idx.known_mtimes().unwrap();
    assert_eq!(map.len(), 2);
    assert_eq!(map.get("a"), Some(&42));
    assert_eq!(map.get("b"), Some(&7));
}

#[test]
fn branch_roundtrips_through_index() {
    use hop::core::{AgentId, Session};
    use hop::index::SearchIndex;
    use hop::query::ParsedQuery;
    let dir = tempfile::tempdir().unwrap();
    let idx = SearchIndex::open_or_create(dir.path()).unwrap();
    let mut w = idx.writer().unwrap();
    let s = Session {
        id: "a".into(), agent: AgentId::Codex, title: "t".into(),
        directory: "/w".into(), timestamp: 1, content: "hello".into(),
        message_count: 1, mtime: 1, yolo: false,
        branch: Some("feat/x".into()), repo_url: Some("git@github.com:me/web.git".into()),
    };
    idx.upsert(&mut w, &s);
    w.commit().unwrap();
    idx.reload().unwrap();
    let out = idx.search(&ParsedQuery::default(), 100, 10).unwrap();
    assert_eq!(out.len(), 1);
    assert_eq!(out[0].branch.as_deref(), Some("feat/x"));
    assert_eq!(out[0].repo_url.as_deref(), Some("git@github.com:me/web.git"));
}
