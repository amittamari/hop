use crate::adapters::Adapter;
use crate::core::{document_key, ResumeCommand, Session, SessionSummary, Transcript};
use crate::index::{diff_authoritative, SearchIndex};
use crate::query::{self, ParsedQuery};
use anyhow::Result;
use std::collections::{HashMap, HashSet};
use std::path::Path;
use std::sync::mpsc::{self, Receiver, Sender};
use std::time::{Duration, Instant};

pub const DEBOUNCE: Duration = Duration::from_millis(40);

/// Message pushed from the background sync thread to the UI loop.
pub enum Update {
    /// New sessions were indexed; UI should re-run its current search.
    Refresh,
    /// Sync finished with non-fatal quality/status counters.
    Done { report: SyncReport },
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct SyncReport {
    pub adapters_scanned: usize,
    pub adapters_unavailable: usize,
    pub scan_errors: usize,
    pub parse_errors: usize,
    pub empty_sessions: usize,
    pub indexed: usize,
    pub deleted: usize,
    pub fatal_errors: usize,
}

impl SyncReport {
    pub fn status_line(&self) -> String {
        if self.fatal_errors > 0 {
            return "sync failed".to_string();
        }
        let mut parts = vec!["sync complete".to_string()];
        if self.scan_errors > 0 {
            parts.push(format!("scan warnings {}", self.scan_errors));
        }
        if self.adapters_unavailable > 0 {
            parts.push(format!("unavailable {}", self.adapters_unavailable));
        }
        if self.parse_errors > 0 {
            parts.push(format!("parse errors {}", self.parse_errors));
        }
        if self.empty_sessions > 0 {
            parts.push(format!("empty sessions {}", self.empty_sessions));
        }
        parts.join("; ")
    }
}

pub struct Engine {
    index: SearchIndex,
    adapters: Vec<Box<dyn Adapter>>,
    query: String,
    parsed: ParsedQuery,
    results: Vec<SessionSummary>,
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

    pub fn results(&self) -> &[SessionSummary] {
        &self.results
    }

    pub fn query(&self) -> &str {
        &self.query
    }

    pub fn parsed_query(&self) -> &ParsedQuery {
        &self.parsed
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

    pub fn transcript_for(&self, session: &SessionSummary) -> Option<Transcript> {
        let path = session.source_path.as_deref()?;
        self.adapter_for(session.agent)
            .and_then(|adapter| adapter.transcript(path).ok())
            .map(|messages| Transcript { messages })
    }

    pub fn supports_yolo(&self, session: &SessionSummary) -> bool {
        self.adapter_for(session.agent)
            .is_some_and(|a| a.supports_yolo())
    }

    pub fn indexed_session(&self, session: &SessionSummary) -> Option<Session> {
        self.index
            .load_session(&session.document_key())
            .ok()
            .flatten()
    }

    pub fn indexed_content(&self, session: &SessionSummary) -> Option<String> {
        self.indexed_session(session).map(|s| s.content)
    }

    pub fn resume_command_for(
        &self,
        session: &SessionSummary,
        yolo: bool,
    ) -> Option<ResumeCommand> {
        let full = self.indexed_session(session)?;
        let argv = self.adapter_for(full.agent)?.resume_command(&full, yolo);
        Some(ResumeCommand {
            directory: full.directory,
            argv,
        })
    }

    #[cfg(test)]
    pub fn replace_adapters(&mut self, adapters: Vec<Box<dyn Adapter>>) {
        self.adapters = adapters;
    }

    /// Full synchronous sync pass: scan all adapters, diff, parse changed,
    /// upsert, delete removed, commit, reload. Malformed files and adapter scan
    /// problems are recorded in the report but do not abort the whole pass.
    pub fn sync_once(&mut self) -> Result<SyncReport> {
        sync_index(&self.index, &self.adapters, |_| {})
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
            let result = (|| -> Result<SyncReport> {
                let index = SearchIndex::open_or_create(&index_dir)?;
                sync_index(&index, &adapters, |_| {
                    let _ = tx.send(Update::Refresh);
                })
            })();
            let _ = tx.send(Update::Refresh);
            let _ = tx.send(Update::Done {
                report: result.unwrap_or_else(|_| SyncReport {
                    fatal_errors: 1,
                    ..SyncReport::default()
                }),
            });
        });
        (rx, handle)
    }

    /// Re-open the reader (after a background commit) so subsequent searches see new docs.
    pub fn reload(&self) -> Result<()> {
        self.index.reload()
    }
}

fn sync_index(
    index: &SearchIndex,
    adapters: &[Box<dyn Adapter>],
    mut on_batch_commit: impl FnMut(&SearchIndex),
) -> Result<SyncReport> {
    let known = index.known_mtimes()?;
    let mut writer = index.writer()?;
    let mut report = SyncReport::default();
    let mut all_scanned = HashMap::new();
    let mut owner = HashMap::new();
    let mut authoritative_agents = HashSet::new();

    for (ai, adapter) in adapters.iter().enumerate() {
        if !adapter.is_available() {
            report.adapters_unavailable += 1;
            continue;
        }
        match adapter.scan() {
            Ok(scanned) => {
                report.adapters_scanned += 1;
                authoritative_agents.insert(adapter.id());
                for (id, entry) in scanned {
                    let key = document_key(adapter.id(), &id);
                    owner.insert(key.clone(), ai);
                    all_scanned.insert(key, entry);
                }
            }
            Err(_) => {
                report.scan_errors += 1;
            }
        }
    }

    let (changed, deleted) = diff_authoritative(&known, &all_scanned, &authoritative_agents);
    report.deleted = deleted.len();
    for key in &deleted {
        index.delete(&mut writer, key);
    }

    let mut since_commit = 0usize;
    for (key, entry) in &changed {
        let Some(&ai) = owner.get(key) else {
            continue;
        };
        match adapters[ai].parse(&entry.path) {
            Ok(mut s) => {
                s.mtime = entry.mtime;
                if s.source_path.is_none() {
                    s.source_path = Some(entry.path.clone());
                }
                if s.message_count == 0 || s.content.trim().is_empty() {
                    report.empty_sessions += 1;
                }
                index.upsert(&mut writer, &s);
                report.indexed += 1;
                since_commit += 1;
            }
            Err(_) => report.parse_errors += 1,
        }
        if since_commit >= 200 {
            writer.commit()?;
            index.reload()?;
            on_batch_commit(index);
            writer = index.writer()?;
            since_commit = 0;
        }
    }
    writer.commit()?;
    index.reload()?;
    Ok(report)
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
        available: bool,
        scan_fails: bool,
        parse_fails: bool,
    }

    impl Adapter for FakeAdapter {
        fn id(&self) -> AgentId {
            self.agent
        }
        fn is_available(&self) -> bool {
            self.available
        }
        fn scan(&self) -> anyhow::Result<HashMap<SessionId, ScanEntry>> {
            if self.scan_fails {
                return Err(anyhow::anyhow!("scan failed"));
            }
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
            if self.parse_fails {
                return Err(anyhow::anyhow!("parse failed"));
            }
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

    fn adapter(agent: AgentId, sessions: Vec<Session>) -> Box<dyn Adapter> {
        Box::new(FakeAdapter {
            agent,
            sessions,
            available: true,
            scan_fails: false,
            parse_fails: false,
        })
    }

    fn unavailable_adapter(agent: AgentId) -> Box<dyn Adapter> {
        Box::new(FakeAdapter {
            agent,
            sessions: Vec::new(),
            available: false,
            scan_fails: false,
            parse_fails: false,
        })
    }

    fn failing_scan_adapter(agent: AgentId) -> Box<dyn Adapter> {
        Box::new(FakeAdapter {
            agent,
            sessions: Vec::new(),
            available: true,
            scan_fails: true,
            parse_fails: false,
        })
    }

    fn failing_parse_adapter(agent: AgentId, sessions: Vec<Session>) -> Box<dyn Adapter> {
        Box::new(FakeAdapter {
            agent,
            sessions,
            available: true,
            scan_fails: false,
            parse_fails: true,
        })
    }

    #[test]
    fn sync_then_search_finds_indexed_sessions() {
        let dir = tempfile::tempdir().unwrap();
        let adapters: Vec<Box<dyn Adapter>> = vec![adapter(
            AgentId::Claude,
            vec![sess("a", "auth bug"), sess("b", "deploy")],
        )];
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
        let adapters: Vec<Box<dyn Adapter>> =
            vec![adapter(AgentId::Claude, vec![sess("a", "auth")])];
        let mut engine = Engine::new(dir.path(), adapters).unwrap();
        engine.sync_once().unwrap();
        engine.set_query("");
        engine.search().unwrap();
        assert_eq!(engine.results().len(), 1);

        // adapter now returns nothing -> session pruned
        let empty: Vec<Box<dyn Adapter>> = vec![adapter(AgentId::Claude, vec![])];
        engine.replace_adapters(empty);
        engine.sync_once().unwrap();
        engine.search().unwrap();
        assert_eq!(engine.results().len(), 0);
    }

    #[test]
    fn sync_namespaces_overlapping_raw_ids_by_agent() {
        let dir = tempfile::tempdir().unwrap();
        let adapters: Vec<Box<dyn Adapter>> = vec![
            adapter(
                AgentId::Claude,
                vec![sess_for(AgentId::Claude, "same", "auth claude")],
            ),
            adapter(
                AgentId::Codex,
                vec![sess_for(AgentId::Codex, "same", "auth codex")],
            ),
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

    #[test]
    fn unavailable_adapter_does_not_delete_existing_rows() {
        let dir = tempfile::tempdir().unwrap();
        let mut engine = Engine::new(
            dir.path(),
            vec![
                adapter(
                    AgentId::Claude,
                    vec![sess_for(AgentId::Claude, "a", "auth")],
                ),
                adapter(
                    AgentId::Codex,
                    vec![sess_for(AgentId::Codex, "b", "deploy")],
                ),
            ],
        )
        .unwrap();
        engine.sync_once().unwrap();

        engine.replace_adapters(vec![
            unavailable_adapter(AgentId::Claude),
            adapter(
                AgentId::Codex,
                vec![sess_for(AgentId::Codex, "b", "deploy")],
            ),
        ]);
        let report = engine.sync_once().unwrap();
        assert_eq!(report.adapters_unavailable, 1);

        engine.set_query("");
        engine.search().unwrap();
        assert_eq!(engine.results().len(), 2);
        assert!(engine
            .results()
            .iter()
            .any(|s| s.id == "a" && s.agent == AgentId::Claude));
    }

    #[test]
    fn scan_failure_does_not_delete_existing_rows() {
        let dir = tempfile::tempdir().unwrap();
        let mut engine = Engine::new(
            dir.path(),
            vec![adapter(
                AgentId::Claude,
                vec![sess_for(AgentId::Claude, "a", "auth")],
            )],
        )
        .unwrap();
        engine.sync_once().unwrap();

        engine.replace_adapters(vec![failing_scan_adapter(AgentId::Claude)]);
        let report = engine.sync_once().unwrap();
        assert_eq!(report.scan_errors, 1);

        engine.set_query("");
        engine.search().unwrap();
        assert_eq!(engine.results().len(), 1);
        assert_eq!(engine.results()[0].id, "a");
    }

    #[test]
    fn parse_failures_and_empty_sessions_are_reported() {
        let dir = tempfile::tempdir().unwrap();
        let mut empty = sess_for(AgentId::Claude, "empty", "");
        empty.content.clear();
        empty.message_count = 0;
        let mut engine = Engine::new(
            dir.path(),
            vec![
                adapter(AgentId::Claude, vec![empty]),
                failing_parse_adapter(
                    AgentId::Codex,
                    vec![sess_for(AgentId::Codex, "bad", "bad parse")],
                ),
            ],
        )
        .unwrap();

        let report = engine.sync_once().unwrap();
        assert_eq!(report.parse_errors, 1);
        assert_eq!(report.empty_sessions, 1);
        assert!(report.status_line().contains("parse errors 1"));
        assert!(report.status_line().contains("empty sessions 1"));
    }
}
