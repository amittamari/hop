use crate::core::{AgentId, DocumentKey, ScanEntry, Session, SessionSummary};
use crate::query::ParsedQuery;
use anyhow::{Context, Result};
use std::collections::{HashMap, HashSet};
use std::ops::Bound;
use std::path::Path;
use tantivy::collector::{Count, TopDocs};
use tantivy::query::{
    AllQuery, BooleanQuery, BoostQuery, FuzzyTermQuery, Occur, Query, QueryParser, RangeQuery,
    TermQuery,
};
use tantivy::schema::{
    Field, IndexRecordOption, Schema, Value, FAST, INDEXED, STORED, STRING, TEXT,
};
use tantivy::{Index, IndexReader, IndexWriter, TantivyDocument, Term};

pub const SCHEMA_VERSION: u32 = 4;
const EXACT_BOOST: f32 = 5.0;
const FETCH_PAGE: usize = 1_000;
const SCORE_BUCKET_SCALE: f32 = 10.0;
const RECENCY_BOOST_MAX: f32 = 3.0;
const RECENCY_HALF_LIFE_SECS: f64 = 604_800.0;
const WRITER_HEAP: usize = 50_000_000;

struct Fields {
    doc_key: Field,
    id: Field,
    agent: Field,
    title: Field,
    content: Field,
    directory: Field,
    timestamp: Field,
    mtime: Field,
    message_count: Field,
    yolo: Field,
    branch: Field,
    repo_url: Field,
    source_path: Field,
    archived: Field,
    worktree: Field,
    permission_mode: Field,
    sidecar_stamp: Field,
}

pub struct SearchIndex {
    index: Index,
    reader: IndexReader,
    f: Fields,
}

fn build_schema() -> (Schema, Fields) {
    let mut b = Schema::builder();
    let f = Fields {
        doc_key: b.add_text_field("doc_key", STRING | STORED),
        id: b.add_text_field("id", STRING | STORED),
        agent: b.add_text_field("agent", STRING | STORED),
        title: b.add_text_field("title", TEXT | STORED),
        content: b.add_text_field("content", TEXT | STORED),
        directory: b.add_text_field("directory", STRING | STORED),
        timestamp: b.add_u64_field("timestamp", INDEXED | FAST | STORED),
        mtime: b.add_u64_field("mtime", STORED),
        message_count: b.add_u64_field("message_count", STORED),
        yolo: b.add_u64_field("yolo", STORED),
        branch: b.add_text_field("branch", STRING | STORED),
        repo_url: b.add_text_field("repo_url", STRING | STORED),
        source_path: b.add_text_field("source_path", STRING | STORED),
        archived: b.add_u64_field("archived", STORED),
        worktree: b.add_text_field("worktree", STRING | STORED),
        permission_mode: b.add_text_field("permission_mode", STRING | STORED),
        sidecar_stamp: b.add_text_field("sidecar_stamp", STRING | STORED),
    };
    (b.build(), f)
}

impl SearchIndex {
    /// Open the index at `dir`, creating it if absent. If the on-disk schema
    /// version marker mismatches, the index is dropped and rebuilt empty.
    pub fn open_or_create(dir: &Path) -> Result<Self> {
        std::fs::create_dir_all(dir).with_context(|| format!("creating {}", dir.display()))?;
        let marker = dir.join(".schema_version");
        let version_ok = std::fs::read_to_string(&marker)
            .ok()
            .and_then(|s| s.trim().parse::<u32>().ok())
            == Some(SCHEMA_VERSION);

        if !version_ok && dir.join("meta.json").exists() {
            // schema changed: drop the whole index dir and recreate it empty
            std::fs::remove_dir_all(dir)
                .with_context(|| format!("wiping stale index at {}", dir.display()))?;
            std::fs::create_dir_all(dir)?;
        }

        let (schema, f) = build_schema();
        let index = match Index::open_in_dir(dir) {
            Ok(i) => i,
            Err(_) => Index::create_in_dir(dir, schema.clone())
                .with_context(|| format!("creating index in {}", dir.display()))?,
        };
        std::fs::write(&marker, SCHEMA_VERSION.to_string())?;

        let reader = index.reader()?;
        Ok(Self { index, reader, f })
    }

    pub fn writer(&self) -> Result<IndexWriter> {
        Ok(self.index.writer(WRITER_HEAP)?)
    }

    pub fn reload(&self) -> Result<()> {
        self.reader.reload()?;
        Ok(())
    }

    pub fn upsert(&self, w: &mut IndexWriter, s: &Session) {
        self.upsert_with_sidecar_stamp(w, s, None);
    }

    pub fn upsert_with_sidecar_stamp(
        &self,
        w: &mut IndexWriter,
        s: &Session,
        sidecar_stamp: Option<&str>,
    ) {
        let m = &s.meta;
        let doc_key = m.document_key();
        w.delete_term(Term::from_field_text(self.f.doc_key, &doc_key));
        let mut doc = TantivyDocument::default();
        doc.add_text(self.f.doc_key, &doc_key);
        doc.add_text(self.f.id, &m.id);
        doc.add_text(self.f.agent, m.agent.slug());
        doc.add_text(self.f.title, &m.title);
        doc.add_text(self.f.content, &s.content);
        doc.add_text(self.f.directory, &m.directory);
        doc.add_u64(self.f.timestamp, m.timestamp.max(0) as u64);
        doc.add_u64(self.f.mtime, s.mtime.max(0) as u64);
        doc.add_u64(self.f.message_count, m.message_count as u64);
        doc.add_u64(self.f.yolo, m.yolo as u64);
        doc.add_u64(self.f.archived, m.archived as u64);
        if let Some(b) = &m.branch {
            doc.add_text(self.f.branch, b);
        }
        if let Some(r) = &m.repo_url {
            doc.add_text(self.f.repo_url, r);
        }
        if let Some(path) = &m.source_path {
            doc.add_text(self.f.source_path, path.to_string_lossy());
        }
        if let Some(w) = &m.worktree {
            doc.add_text(self.f.worktree, w);
        }
        if let Some(pm) = &m.permission_mode {
            doc.add_text(self.f.permission_mode, pm);
        }
        if let Some(stamp) = sidecar_stamp {
            doc.add_text(self.f.sidecar_stamp, stamp);
        }
        let _ = w.add_document(doc);
    }

    pub fn delete(&self, w: &mut IndexWriter, doc_key: &str) {
        w.delete_term(Term::from_field_text(self.f.doc_key, doc_key));
    }

    /// document_key -> mtime for every indexed session (drives incremental diff).
    pub fn known_mtimes(&self) -> Result<HashMap<DocumentKey, i64>> {
        Ok(self.known_sync_state()?.0)
    }

    /// Indexed source mtimes and sidecar file stamps, keyed by document key.
    /// Keeping these signals separate lets metadata-only hook writes trigger a
    /// reparse without distorting the source-file mtime.
    pub fn known_sync_state(
        &self,
    ) -> Result<(HashMap<DocumentKey, i64>, HashMap<DocumentKey, String>)> {
        let searcher = self.reader.searcher();
        let n = searcher.num_docs().max(1) as usize;
        let hits = searcher.search(
            &AllQuery,
            &TopDocs::with_limit(n).order_by_u64_field("timestamp", tantivy::Order::Desc),
        )?;
        let mut mtimes = HashMap::new();
        let mut sidecars = HashMap::new();
        for (_, addr) in hits {
            let doc: TantivyDocument = searcher.doc(addr)?;
            let doc_key = doc.get_first(self.f.doc_key).and_then(|v| v.as_str());
            let mtime = doc.get_first(self.f.mtime).and_then(|v| v.as_u64());
            if let (Some(doc_key), Some(m)) = (doc_key, mtime) {
                mtimes.insert(doc_key.to_string(), m as i64);
                if let Some(stamp) = doc.get_first(self.f.sidecar_stamp).and_then(|v| v.as_str()) {
                    sidecars.insert(doc_key.to_string(), stamp.to_string());
                }
            }
        }
        Ok((mtimes, sidecars))
    }

    /// Run a parsed query. `now` is unix seconds (for date filtering).
    pub fn search(&self, q: &ParsedQuery, now: i64, limit: usize) -> Result<Vec<SessionSummary>> {
        let searcher = self.reader.searcher();

        // --- build the text/all query plus agent constraints ---
        let mut clauses: Vec<(Occur, Box<dyn Query>)> = Vec::new();

        if q.free_text.trim().is_empty() {
            clauses.push((Occur::Must, Box::new(AllQuery)));
        } else {
            let qp = QueryParser::for_index(&self.index, vec![self.f.title, self.f.content]);
            let exact = qp
                .parse_query(&sanitize(&q.free_text))
                .unwrap_or_else(|_| Box::new(AllQuery));
            let mut should: Vec<(Occur, Box<dyn Query>)> =
                vec![(Occur::Should, Box::new(BoostQuery::new(exact, EXACT_BOOST)))];
            for word in q.free_text.split_whitespace() {
                let w = word.to_lowercase();
                for field in [self.f.title, self.f.content] {
                    let fz = FuzzyTermQuery::new_prefix(Term::from_field_text(field, &w), 1, true);
                    should.push((Occur::Should, Box::new(fz)));
                }
            }
            clauses.push((Occur::Must, Box::new(BooleanQuery::new(should))));
        }

        // agent include (any-of) / exclude
        if !q.agents.include.is_empty() {
            let any: Vec<(Occur, Box<dyn Query>)> = q
                .agents
                .include
                .iter()
                .map(|a| {
                    (
                        Occur::Should,
                        Box::new(TermQuery::new(
                            Term::from_field_text(self.f.agent, a.slug()),
                            IndexRecordOption::Basic,
                        )) as Box<dyn Query>,
                    )
                })
                .collect();
            clauses.push((Occur::Must, Box::new(BooleanQuery::new(any))));
        }
        for a in &q.agents.exclude {
            clauses.push((
                Occur::MustNot,
                Box::new(TermQuery::new(
                    Term::from_field_text(self.f.agent, a.slug()),
                    IndexRecordOption::Basic,
                )),
            ));
        }

        if let Some(date) = q.date {
            let (lo, hi) = date.range(now);
            if hi.is_some_and(|hi| hi < 0) {
                return Ok(Vec::new());
            }
            if let (Some(lo), Some(hi)) = (lo, hi) {
                if lo > hi {
                    return Ok(Vec::new());
                }
            }
            let lower = lo
                .map(|lo| Bound::Included(Term::from_field_u64(self.f.timestamp, lo.max(0) as u64)))
                .unwrap_or(Bound::Unbounded);
            let upper = hi
                .map(|hi| Bound::Included(Term::from_field_u64(self.f.timestamp, hi as u64)))
                .unwrap_or(Bound::Unbounded);
            clauses.push((Occur::Must, Box::new(RangeQuery::new(lower, upper))));
        }

        let query = BooleanQuery::new(clauses);

        // --- collect pages until post-filters produce enough rows or hits are exhausted ---
        let total_hits = searcher.search(&query, &Count)?;
        let mut out = Vec::new();
        let mut offset = 0usize;
        while out.len() < limit && offset < total_hits {
            let page_limit = FETCH_PAGE.min(total_hits - offset);
            let addrs: Vec<tantivy::DocAddress> = if q.free_text.trim().is_empty() {
                searcher
                    .search(
                        &query,
                        &TopDocs::with_limit(page_limit)
                            .and_offset(offset)
                            .order_by_u64_field("timestamp", tantivy::Order::Desc),
                    )?
                    .into_iter()
                    .map(|(_, a)| a)
                    .collect()
            } else {
                let now_ts = now.max(0) as u64;
                searcher
                    .search(
                        &query,
                        &TopDocs::with_limit(page_limit)
                            .and_offset(offset)
                            .tweak_score(move |segment_reader: &tantivy::SegmentReader| {
                                let timestamps = segment_reader
                                    .fast_fields()
                                    .u64("timestamp")
                                    .unwrap()
                                    .first_or_default_col(0);
                                move |doc: tantivy::DocId, score: tantivy::Score| {
                                    let ts = timestamps.get_val(doc);
                                    let age = now_ts.saturating_sub(ts);
                                    let combined = score + recency_boost(age);
                                    (score_bucket(combined), ts)
                                }
                            }),
                    )?
                    .into_iter()
                    .map(|(_, a)| a)
                    .collect()
            };
            if addrs.is_empty() {
                break;
            }
            offset += addrs.len();

            for addr in addrs {
                let doc: TantivyDocument = searcher.doc(addr)?;
                let s = self.to_summary(&doc);
                if !dir_ok(&s.directory, q) || !repo_ok(s.repo_url.as_deref(), q) {
                    continue;
                }
                out.push(s);
                if out.len() >= limit {
                    break;
                }
            }
        }
        Ok(out)
    }

    pub fn load_session(&self, doc_key: &str) -> Result<Option<Session>> {
        let searcher = self.reader.searcher();
        let query = TermQuery::new(
            Term::from_field_text(self.f.doc_key, doc_key),
            IndexRecordOption::Basic,
        );
        let hits = searcher.search(&query, &TopDocs::with_limit(1).order_by_score())?;
        let Some((_, addr)) = hits.into_iter().next() else {
            return Ok(None);
        };
        let doc: TantivyDocument = searcher.doc(addr)?;
        Ok(Some(self.to_session(&doc)))
    }

    fn to_session(&self, doc: &TantivyDocument) -> Session {
        let meta = self.to_summary(doc);
        let content = doc
            .get_first(self.f.content)
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        let mtime = doc
            .get_first(self.f.mtime)
            .and_then(|v| v.as_u64())
            .unwrap_or(0) as i64;
        Session {
            meta,
            content,
            mtime,
        }
    }

    fn to_summary(&self, doc: &TantivyDocument) -> SessionSummary {
        let get_str = |f: Field| {
            doc.get_first(f)
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string()
        };
        let get_u64 = |f: Field| doc.get_first(f).and_then(|v| v.as_u64()).unwrap_or(0);
        SessionSummary {
            id: get_str(self.f.id),
            agent: AgentId::from_slug(&get_str(self.f.agent)).unwrap_or(AgentId::Claude),
            title: get_str(self.f.title),
            directory: get_str(self.f.directory),
            timestamp: get_u64(self.f.timestamp) as i64,
            message_count: get_u64(self.f.message_count) as u32,
            yolo: get_u64(self.f.yolo) != 0,
            branch: {
                let b = get_str(self.f.branch);
                if b.is_empty() {
                    None
                } else {
                    Some(b)
                }
            },
            repo_url: {
                let r = get_str(self.f.repo_url);
                if r.is_empty() {
                    None
                } else {
                    Some(r)
                }
            },
            source_path: {
                let p = get_str(self.f.source_path);
                if p.is_empty() {
                    None
                } else {
                    Some(p.into())
                }
            },
            archived: get_u64(self.f.archived) != 0,
            worktree: {
                let w = get_str(self.f.worktree);
                if w.is_empty() {
                    None
                } else {
                    Some(w)
                }
            },
            permission_mode: {
                let pm = get_str(self.f.permission_mode);
                if pm.is_empty() {
                    None
                } else {
                    Some(pm)
                }
            },
        }
    }
}

fn dir_ok(directory: &str, q: &ParsedQuery) -> bool {
    let d = directory.to_lowercase();
    q.dirs.include.iter().all(|i| d.contains(&i.to_lowercase()))
        && !q.dirs.exclude.iter().any(|e| d.contains(&e.to_lowercase()))
}

/// Substring match on the git remote URL. A session with no `repo_url` matches
/// the empty string, so it satisfies an exclude filter but never an include one
/// (non-git directories correctly drop out of any `repo:` query).
fn repo_ok(repo_url: Option<&str>, q: &ParsedQuery) -> bool {
    let r = repo_url.unwrap_or_default().to_lowercase();
    q.repos
        .include
        .iter()
        .all(|i| r.contains(&i.to_lowercase()))
        && !q
            .repos
            .exclude
            .iter()
            .any(|e| r.contains(&e.to_lowercase()))
}

fn score_bucket(score: tantivy::Score) -> i64 {
    if !score.is_finite() {
        return 0;
    }
    (score * SCORE_BUCKET_SCALE).round() as i64
}

fn recency_boost(age_secs: u64) -> f32 {
    let decay = (-std::f64::consts::LN_2 * age_secs as f64 / RECENCY_HALF_LIFE_SECS).exp();
    RECENCY_BOOST_MAX * decay as f32
}

/// Escape characters that would make tantivy's QueryParser error out.
fn sanitize(s: &str) -> String {
    s.replace(
        [
            '+', '-', '!', '^', '~', '*', '?', ':', '(', ')', '[', ']', '{', '}', '"',
        ],
        " ",
    )
}

/// Pure incremental diff. Returns (changed[(id, entry)], deleted[id]).
/// Changed = scanned mtime > known + 1ms, or id absent from known.
pub fn diff(
    known: &HashMap<DocumentKey, i64>,
    scanned: &HashMap<DocumentKey, ScanEntry>,
) -> (Vec<(DocumentKey, ScanEntry)>, Vec<DocumentKey>) {
    diff_with_delete_scope(known, scanned, None)
}

/// Incremental diff where deletions are authoritative only for the supplied
/// agents. Changed rows still come from every scanned row.
pub fn diff_authoritative(
    known: &HashMap<DocumentKey, i64>,
    scanned: &HashMap<DocumentKey, ScanEntry>,
    authoritative_agents: &HashSet<AgentId>,
) -> (Vec<(DocumentKey, ScanEntry)>, Vec<DocumentKey>) {
    diff_with_delete_scope(known, scanned, Some(authoritative_agents))
}

fn diff_with_delete_scope(
    known: &HashMap<DocumentKey, i64>,
    scanned: &HashMap<DocumentKey, ScanEntry>,
    authoritative_agents: Option<&HashSet<AgentId>>,
) -> (Vec<(DocumentKey, ScanEntry)>, Vec<DocumentKey>) {
    let mut changed = Vec::new();
    for (id, entry) in scanned {
        match known.get(id) {
            Some(&m) if entry.mtime <= m + 1 => {}
            _ => changed.push((id.clone(), entry.clone())),
        }
    }
    let deleted: Vec<DocumentKey> = known
        .keys()
        .filter(|id| !scanned.contains_key(*id))
        .filter(|id| {
            authoritative_agents.is_none_or(|agents| {
                id.split_once(':')
                    .and_then(|(agent, _)| AgentId::from_slug(agent))
                    .is_some_and(|agent| agents.contains(&agent))
            })
        })
        .cloned()
        .collect();
    (changed, deleted)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::query;

    #[test]
    fn repo_filter_matches_remote_substring_case_insensitively() {
        let q = query::parse("repo:HOP");
        assert!(repo_ok(Some("git@github.com:ofirg/hop.git"), &q));
        assert!(!repo_ok(Some("git@github.com:other/repo.git"), &q));
        // non-git sessions never satisfy an include filter
        assert!(!repo_ok(None, &q));
    }

    #[test]
    fn repo_exclude_keeps_non_git_and_drops_matches() {
        let q = query::parse("-repo:vendor");
        assert!(repo_ok(Some("git@github.com:me/app.git"), &q));
        assert!(!repo_ok(Some("https://example.com/vendor/lib.git"), &q));
        // no repo_url -> nothing to exclude, stays in
        assert!(repo_ok(None, &q));
    }
}
