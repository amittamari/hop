use crate::core::{AgentId, DocumentKey, ScanEntry, Session};
use crate::query::ParsedQuery;
use anyhow::{Context, Result};
use std::collections::HashMap;
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

pub const SCHEMA_VERSION: u32 = 5;
const EXACT_BOOST: f32 = 5.0;
const FETCH_PAGE: usize = 1_000;
const SCORE_BUCKET_SCALE: f32 = 1_000.0;
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
        let doc_key = s.document_key();
        w.delete_term(Term::from_field_text(self.f.doc_key, &doc_key));
        let mut doc = TantivyDocument::default();
        doc.add_text(self.f.doc_key, &doc_key);
        doc.add_text(self.f.id, &s.id);
        doc.add_text(self.f.agent, s.agent.slug());
        doc.add_text(self.f.title, &s.title);
        doc.add_text(self.f.content, &s.content);
        doc.add_text(self.f.directory, &s.directory);
        doc.add_u64(self.f.timestamp, s.timestamp.max(0) as u64);
        doc.add_u64(self.f.mtime, s.mtime.max(0) as u64);
        doc.add_u64(self.f.message_count, s.message_count as u64);
        doc.add_u64(self.f.yolo, s.yolo as u64);
        if let Some(b) = &s.branch {
            doc.add_text(self.f.branch, b);
        }
        if let Some(r) = &s.repo_url {
            doc.add_text(self.f.repo_url, r);
        }
        if let Some(path) = &s.source_path {
            doc.add_text(self.f.source_path, &path.to_string_lossy());
        }
        let _ = w.add_document(doc);
    }

    pub fn delete(&self, w: &mut IndexWriter, doc_key: &str) {
        w.delete_term(Term::from_field_text(self.f.doc_key, doc_key));
    }

    /// document_key -> mtime for every indexed session (drives incremental diff).
    pub fn known_mtimes(&self) -> Result<HashMap<DocumentKey, i64>> {
        let searcher = self.reader.searcher();
        let n = searcher.num_docs().max(1) as usize;
        let hits = searcher.search(
            &AllQuery,
            &TopDocs::with_limit(n).order_by_u64_field("timestamp", tantivy::Order::Desc),
        )?;
        let mut map = HashMap::new();
        for (_, addr) in hits {
            let doc: TantivyDocument = searcher.doc(addr)?;
            let doc_key = doc.get_first(self.f.doc_key).and_then(|v| v.as_str());
            let mtime = doc.get_first(self.f.mtime).and_then(|v| v.as_u64());
            if let (Some(doc_key), Some(m)) = (doc_key, mtime) {
                map.insert(doc_key.to_string(), m as i64);
            }
        }
        Ok(map)
    }

    /// Run a parsed query. `now` is unix seconds (for date filtering).
    pub fn search(&self, q: &ParsedQuery, now: i64, limit: usize) -> Result<Vec<Session>> {
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
                searcher
                    .search(
                        &query,
                        &TopDocs::with_limit(page_limit)
                            .and_offset(offset)
                            .tweak_score(|segment_reader: &tantivy::SegmentReader| {
                                let timestamps = segment_reader
                                    .fast_fields()
                                    .u64("timestamp")
                                    .unwrap()
                                    .first_or_default_col(0);
                                move |doc: tantivy::DocId, score: tantivy::Score| {
                                    (score_bucket(score), timestamps.get_val(doc))
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
                let s = self.to_session(&doc);
                if !dir_ok(&s.directory, q) {
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

    fn to_session(&self, doc: &TantivyDocument) -> Session {
        let get_str = |f: Field| {
            doc.get_first(f)
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string()
        };
        let get_u64 = |f: Field| doc.get_first(f).and_then(|v| v.as_u64()).unwrap_or(0);
        Session {
            id: get_str(self.f.id),
            agent: AgentId::from_slug(&get_str(self.f.agent)).unwrap_or(AgentId::Claude),
            title: get_str(self.f.title),
            directory: get_str(self.f.directory),
            timestamp: get_u64(self.f.timestamp) as i64,
            content: get_str(self.f.content),
            message_count: get_u64(self.f.message_count) as u32,
            mtime: get_u64(self.f.mtime) as i64,
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
        }
    }
}

fn dir_ok(directory: &str, q: &ParsedQuery) -> bool {
    let d = directory.to_lowercase();
    q.dirs.include.iter().all(|i| d.contains(&i.to_lowercase()))
        && !q.dirs.exclude.iter().any(|e| d.contains(&e.to_lowercase()))
}

fn score_bucket(score: tantivy::Score) -> i64 {
    if !score.is_finite() {
        return 0;
    }
    (score * SCORE_BUCKET_SCALE).round() as i64
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
        .cloned()
        .collect();
    (changed, deleted)
}
