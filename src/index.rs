use crate::core::{AgentId, ScanEntry, Session, SessionId};
use crate::query::ParsedQuery;
use anyhow::{Context, Result};
use std::collections::HashMap;
use std::path::Path;
use tantivy::collector::TopDocs;
use tantivy::query::{
    AllQuery, BooleanQuery, BoostQuery, FuzzyTermQuery, Occur, Query, QueryParser, TermQuery,
};
use tantivy::schema::{Field, IndexRecordOption, Schema, Value, FAST, STORED, STRING, TEXT};
use tantivy::{Index, IndexReader, IndexWriter, TantivyDocument, Term};

pub const SCHEMA_VERSION: u32 = 1;
const EXACT_BOOST: f32 = 5.0;
const FETCH_CAP: usize = 5_000;
const WRITER_HEAP: usize = 50_000_000;

struct Fields {
    id: Field,
    agent: Field,
    title: Field,
    content: Field,
    directory: Field,
    timestamp: Field,
    mtime: Field,
    message_count: Field,
    yolo: Field,
}

pub struct SearchIndex {
    index: Index,
    reader: IndexReader,
    f: Fields,
}

fn build_schema() -> (Schema, Fields) {
    let mut b = Schema::builder();
    let f = Fields {
        id: b.add_text_field("id", STRING | STORED),
        agent: b.add_text_field("agent", STRING | STORED),
        title: b.add_text_field("title", TEXT | STORED),
        content: b.add_text_field("content", TEXT | STORED),
        directory: b.add_text_field("directory", STRING | STORED),
        timestamp: b.add_u64_field("timestamp", FAST | STORED),
        mtime: b.add_u64_field("mtime", STORED),
        message_count: b.add_u64_field("message_count", STORED),
        yolo: b.add_u64_field("yolo", STORED),
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
        w.delete_term(Term::from_field_text(self.f.id, &s.id));
        let mut doc = TantivyDocument::default();
        doc.add_text(self.f.id, &s.id);
        doc.add_text(self.f.agent, s.agent.slug());
        doc.add_text(self.f.title, &s.title);
        doc.add_text(self.f.content, &s.content);
        doc.add_text(self.f.directory, &s.directory);
        doc.add_u64(self.f.timestamp, s.timestamp.max(0) as u64);
        doc.add_u64(self.f.mtime, s.mtime.max(0) as u64);
        doc.add_u64(self.f.message_count, s.message_count as u64);
        doc.add_u64(self.f.yolo, s.yolo as u64);
        let _ = w.add_document(doc);
    }

    pub fn delete(&self, w: &mut IndexWriter, id: &str) {
        w.delete_term(Term::from_field_text(self.f.id, id));
    }

    /// id -> mtime for every indexed session (drives incremental diff).
    pub fn known_mtimes(&self) -> Result<HashMap<SessionId, i64>> {
        let searcher = self.reader.searcher();
        let n = searcher.num_docs().max(1) as usize;
        let hits = searcher.search(
            &AllQuery,
            &TopDocs::with_limit(n).order_by_u64_field("timestamp", tantivy::Order::Desc),
        )?;
        let mut map = HashMap::new();
        for (_, addr) in hits {
            let doc: TantivyDocument = searcher.doc(addr)?;
            let id = doc.get_first(self.f.id).and_then(|v| v.as_str());
            let mtime = doc.get_first(self.f.mtime).and_then(|v| v.as_u64());
            if let (Some(id), Some(m)) = (id, mtime) {
                map.insert(id.to_string(), m as i64);
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

        let query = BooleanQuery::new(clauses);

        // --- collect (recency order for empty free-text, else score) ---
        let addrs: Vec<tantivy::DocAddress> = if q.free_text.trim().is_empty() {
            searcher
                .search(
                    &query,
                    &TopDocs::with_limit(FETCH_CAP)
                        .order_by_u64_field("timestamp", tantivy::Order::Desc),
                )?
                .into_iter()
                .map(|(_, a)| a)
                .collect()
        } else {
            searcher
                .search(&query, &TopDocs::with_limit(FETCH_CAP).order_by_score())?
                .into_iter()
                .map(|(_, a)| a)
                .collect()
        };

        // --- reconstruct + post-filter dir & date ---
        let date_range = q.date.map(|d| d.range(now));
        let mut out = Vec::new();
        for addr in addrs {
            let doc: TantivyDocument = searcher.doc(addr)?;
            let s = self.to_session(&doc);
            if !dir_ok(&s.directory, q) {
                continue;
            }
            if let Some((lo, hi)) = date_range {
                if let Some(lo) = lo {
                    if s.timestamp < lo {
                        continue;
                    }
                }
                if let Some(hi) = hi {
                    if s.timestamp > hi {
                        continue;
                    }
                }
            }
            out.push(s);
            if out.len() >= limit {
                break;
            }
        }
        Ok(out)
    }

    fn to_session(&self, doc: &TantivyDocument) -> Session {
        let get_str =
            |f: Field| doc.get_first(f).and_then(|v| v.as_str()).unwrap_or("").to_string();
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
        }
    }
}

fn dir_ok(directory: &str, q: &ParsedQuery) -> bool {
    let d = directory.to_lowercase();
    q.dirs.include.iter().all(|i| d.contains(&i.to_lowercase()))
        && !q.dirs.exclude.iter().any(|e| d.contains(&e.to_lowercase()))
}

/// Escape characters that would make tantivy's QueryParser error out.
fn sanitize(s: &str) -> String {
    s.replace(
        ['+', '-', '!', '^', '~', '*', '?', ':', '(', ')', '[', ']', '{', '}', '"'],
        " ",
    )
}

/// Pure incremental diff. Returns (changed[(id, entry)], deleted[id]).
/// Changed = scanned mtime > known + 1ms, or id absent from known.
pub fn diff(
    known: &HashMap<SessionId, i64>,
    scanned: &HashMap<SessionId, ScanEntry>,
) -> (Vec<(SessionId, ScanEntry)>, Vec<SessionId>) {
    let mut changed = Vec::new();
    for (id, entry) in scanned {
        match known.get(id) {
            Some(&m) if entry.mtime <= m + 1 => {}
            _ => changed.push((id.clone(), entry.clone())),
        }
    }
    let deleted: Vec<SessionId> = known
        .keys()
        .filter(|id| !scanned.contains_key(*id))
        .cloned()
        .collect();
    (changed, deleted)
}
