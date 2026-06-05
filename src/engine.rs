use crate::adapters::Adapter;
use crate::core::{document_key, Message, Session};
use crate::index::{diff, SearchIndex};
use crate::query::{self, ParsedQuery};
use anyhow::Result;
use std::path::Path;
use std::sync::mpsc::{self, Receiver, Sender};
use std::time::{Duration, Instant};

pub const DEBOUNCE: Duration = Duration::from_millis(40);

/// Message pushed from the background sync thread to the UI loop.
pub enum Update {
    /// New sessions were indexed; UI should re-run its current search.
    Refresh,
    /// Sync finished; carries count of files that failed to parse.
    Done { parse_errors: usize },
}

pub struct Engine {
    index: SearchIndex,
    adapters: Vec<Box<dyn Adapter>>,
    query: String,
    parsed: ParsedQuery,
    results: Vec<Session>,
    limit: usize,
    last_keystroke: Option<Instant>,
}

impl Engine {
    pub fn new(index_dir: &Path, adapters: Vec<Box<dyn Adapter>>) -> Result<Self> {
        let index = SearchIndex::open_or_create(index_dir)?;
        Ok(Self {
            index,
            adapters,
            query: String::new(),
            parsed: ParsedQuery::default(),
            results: Vec::new(),
            limit: 500,
            last_keystroke: None,
        })
    }

    pub fn results(&self) -> &[Session] {
        &self.results
    }

    pub fn query(&self) -> &str {
        &self.query
    }

    pub fn set_query(&mut self, q: impl Into<String>) {
        self.query = q.into();
        self.parsed = query::parse(&self.query);
        self.last_keystroke = Some(Instant::now());
    }

    /// True when a query change is pending and its debounce interval has elapsed,
    /// i.e. it's time to actually run the search. Returns false once `search()`
    /// has consumed the pending change (it clears `last_keystroke`).
    pub fn search_due(&self) -> bool {
        self.last_keystroke.is_some_and(|t| t.elapsed() >= DEBOUNCE)
    }

    pub fn search(&mut self) -> Result<()> {
        let now = jiff::Timestamp::now().as_second();
        self.results = self.index.search(&self.parsed, now, self.limit)?;
        self.last_keystroke = None;
        Ok(())
    }

    pub fn adapter_for(&self, agent: crate::core::AgentId) -> Option<&dyn Adapter> {
        self.adapters
            .iter()
            .find(|a| a.id() == agent)
            .map(|b| b.as_ref())
    }

    pub fn transcript_for(&self, session: &Session) -> Option<Vec<Message>> {
        let path = session.source_path.as_deref()?;
        self.adapter_for(session.agent)?.transcript(path).ok()
    }

    pub fn supports_yolo(&self, session: &Session) -> bool {
        self.adapter_for(session.agent)
            .is_some_and(|a| a.supports_yolo())
    }

    #[cfg(test)]
    pub fn replace_adapters(&mut self, adapters: Vec<Box<dyn Adapter>>) {
        self.adapters = adapters;
    }

    /// Full synchronous sync pass: scan all adapters, diff, parse changed,
    /// upsert, delete removed, commit, reload. Returns parse-error count.
    pub fn sync_once(&mut self) -> Result<usize> {
        let known = self.index.known_mtimes()?;
        let mut writer = self.index.writer()?;
        let mut parse_errors = 0usize;

        // gather scans, keyed across all adapters by namespaced document key
        let mut all_scanned = std::collections::HashMap::new();
        let mut owner = std::collections::HashMap::new(); // document key -> adapter index
        for (ai, adapter) in self.adapters.iter().enumerate() {
            if !adapter.is_available() {
                continue;
            }
            for (id, entry) in adapter.scan()? {
                let key = document_key(adapter.id(), &id);
                owner.insert(key.clone(), ai);
                all_scanned.insert(key, entry);
            }
        }

        let (changed, deleted) = diff(&known, &all_scanned);
        for key in &deleted {
            self.index.delete(&mut writer, key);
        }
        for (key, entry) in &changed {
            let ai = owner[key];
            match self.adapters[ai].parse(&entry.path) {
                Ok(mut s) => {
                    s.mtime = entry.mtime;
                    if s.source_path.is_none() {
                        s.source_path = Some(entry.path.clone());
                    }
                    self.index.upsert(&mut writer, &s);
                }
                Err(_) => parse_errors += 1,
            }
        }
        writer.commit()?;
        self.index.reload()?;
        Ok(parse_errors)
    }

    /// Spawn the background sync on its own thread. The thread sends `Refresh`
    /// then `Done` over the returned receiver. Uses a fresh index handle so the
    /// UI's engine keeps serving searches meanwhile.
    pub fn spawn_background_sync(
        index_dir: std::path::PathBuf,
        adapters: Vec<Box<dyn Adapter>>,
    ) -> (Receiver<Update>, std::thread::JoinHandle<()>) {
        let (tx, rx): (Sender<Update>, Receiver<Update>) = mpsc::channel();
        let handle = std::thread::spawn(move || {
            let result = (|| -> Result<usize> {
                let index = SearchIndex::open_or_create(&index_dir)?;
                let known = index.known_mtimes()?;
                let mut writer = index.writer()?;
                let mut parse_errors = 0usize;

                let mut all_scanned = std::collections::HashMap::new();
                let mut owner = std::collections::HashMap::new();
                for (ai, adapter) in adapters.iter().enumerate() {
                    if !adapter.is_available() {
                        continue;
                    }
                    for (id, entry) in adapter.scan()? {
                        let key = document_key(adapter.id(), &id);
                        owner.insert(key.clone(), ai);
                        all_scanned.insert(key, entry);
                    }
                }
                let (changed, deleted) = diff(&known, &all_scanned);
                for key in &deleted {
                    index.delete(&mut writer, key);
                }
                // batch commits every 200 upserts so rows stream in
                let mut since_commit = 0;
                for (key, entry) in &changed {
                    let ai = owner[key];
                    match adapters[ai].parse(&entry.path) {
                        Ok(mut s) => {
                            s.mtime = entry.mtime;
                            if s.source_path.is_none() {
                                s.source_path = Some(entry.path.clone());
                            }
                            index.upsert(&mut writer, &s);
                            since_commit += 1;
                        }
                        Err(_) => parse_errors += 1,
                    }
                    if since_commit >= 200 {
                        let _ = writer.commit();
                        let _ = index.reload();
                        let _ = tx.send(Update::Refresh);
                        since_commit = 0;
                    }
                }
                let _ = writer.commit();
                let _ = index.reload();
                Ok(parse_errors)
            })();
            let _ = tx.send(Update::Refresh);
            let _ = tx.send(Update::Done {
                parse_errors: result.unwrap_or(0),
            });
        });
        (rx, handle)
    }

    /// Re-open the reader (after a background commit) so subsequent searches see new docs.
    pub fn reload(&self) -> Result<()> {
        self.index.reload()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::adapters::Adapter;
    use crate::core::{AgentId, ScanEntry, Session, SessionId};
    use std::collections::HashMap;
    use std::path::{Path, PathBuf};

    struct FakeAdapter {
        agent: AgentId,
        sessions: Vec<Session>,
    }

    impl Adapter for FakeAdapter {
        fn id(&self) -> AgentId {
            self.agent
        }
        fn is_available(&self) -> bool {
            true
        }
        fn scan(&self) -> anyhow::Result<HashMap<SessionId, ScanEntry>> {
            Ok(self
                .sessions
                .iter()
                .map(|s| {
                    (
                        s.id.clone(),
                        ScanEntry {
                            path: PathBuf::from(&s.id),
                            mtime: s.mtime,
                        },
                    )
                })
                .collect())
        }
        fn parse(&self, path: &Path) -> anyhow::Result<Session> {
            let id = path.to_string_lossy().to_string();
            self.sessions
                .iter()
                .find(|s| s.id == id)
                .cloned()
                .ok_or_else(|| anyhow::anyhow!("not found"))
        }
        fn resume_command(&self, s: &Session, _yolo: bool) -> Vec<String> {
            vec!["echo".into(), s.id.clone()]
        }
        fn transcript(&self, _path: &Path) -> anyhow::Result<Vec<crate::core::Message>> {
            Ok(Vec::new())
        }
        fn supports_yolo(&self) -> bool {
            true
        }
    }

    fn sess(id: &str, title: &str) -> Session {
        sess_for(AgentId::Claude, id, title)
    }

    fn sess_for(agent: AgentId, id: &str, title: &str) -> Session {
        Session {
            id: id.into(),
            agent,
            title: title.into(),
            directory: "/d".into(),
            timestamp: 100,
            content: title.into(),
            message_count: 1,
            mtime: 10,
            yolo: false,
            branch: None,
            repo_url: None,
            source_path: None,
        }
    }

    #[test]
    fn sync_then_search_finds_indexed_sessions() {
        let dir = tempfile::tempdir().unwrap();
        let adapters: Vec<Box<dyn Adapter>> = vec![Box::new(FakeAdapter {
            agent: AgentId::Claude,
            sessions: vec![sess("a", "auth bug"), sess("b", "deploy")],
        })];
        let mut engine = Engine::new(dir.path(), adapters).unwrap();

        // synchronous full sync (the blocking core that the bg thread also calls)
        engine.sync_once().unwrap();

        engine.set_query("auth");
        engine.search().unwrap();
        assert_eq!(engine.results().len(), 1);
        assert_eq!(engine.results()[0].id, "a");
    }

    #[test]
    fn deletion_pruned_on_resync() {
        let dir = tempfile::tempdir().unwrap();
        let adapters: Vec<Box<dyn Adapter>> = vec![Box::new(FakeAdapter {
            agent: AgentId::Claude,
            sessions: vec![sess("a", "auth")],
        })];
        let mut engine = Engine::new(dir.path(), adapters).unwrap();
        engine.sync_once().unwrap();
        engine.set_query("");
        engine.search().unwrap();
        assert_eq!(engine.results().len(), 1);

        // adapter now returns nothing -> session pruned
        let empty: Vec<Box<dyn Adapter>> = vec![Box::new(FakeAdapter {
            agent: AgentId::Claude,
            sessions: vec![],
        })];
        engine.replace_adapters(empty);
        engine.sync_once().unwrap();
        engine.search().unwrap();
        assert_eq!(engine.results().len(), 0);
    }

    #[test]
    fn sync_namespaces_overlapping_raw_ids_by_agent() {
        let dir = tempfile::tempdir().unwrap();
        let adapters: Vec<Box<dyn Adapter>> = vec![
            Box::new(FakeAdapter {
                agent: AgentId::Claude,
                sessions: vec![sess_for(AgentId::Claude, "same", "auth claude")],
            }),
            Box::new(FakeAdapter {
                agent: AgentId::Codex,
                sessions: vec![sess_for(AgentId::Codex, "same", "auth codex")],
            }),
        ];
        let mut engine = Engine::new(dir.path(), adapters).unwrap();

        engine.sync_once().unwrap();
        engine.set_query("auth");
        engine.search().unwrap();

        assert_eq!(engine.results().len(), 2);
        assert!(engine
            .results()
            .iter()
            .any(|s| s.id == "same" && s.agent == AgentId::Claude));
        assert!(engine
            .results()
            .iter()
            .any(|s| s.id == "same" && s.agent == AgentId::Codex));
    }
}
