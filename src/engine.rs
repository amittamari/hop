// File-size note: ~560 of these lines are inline #[cfg(test)] tests exercising
// private sync fns (sync_index, sync_index_with_sidecar_dir); the module itself
// is ~170 lines. Splitting to satisfy the ~500-line soft limit would be ceremony.
use crate::adapters::Adapter;
use crate::core::{ResumeCommand, Session, SessionSummary, Transcript, document_key};
use crate::index::{SearchIndex, diff_authoritative};
use crate::query::{self, ParsedQuery, SortOrder};
use anyhow::Result;
use std::collections::{HashMap, HashSet};
use std::path::Path;
use std::sync::mpsc::{self, Receiver, Sender};
use std::time::{Duration, Instant};

pub const DEBOUNCE: Duration = Duration::from_millis(40);

/// Message pushed from background threads to the UI loop.
pub enum Update {
    /// New sessions were indexed; UI should re-run its current search.
    Refresh,
    /// Sync finished with non-fatal quality/status counters.
    Done { report: SyncReport },
    /// A newer hop version is available.
    UpgradeAvailable { latest: String },
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct SyncReport {
    pub adapters_scanned: usize,
    pub adapters_unavailable: usize,
    pub scan_errors: usize,
    pub parse_errors: usize,
    pub empty_sessions: usize,
    pub non_interactive_sessions: usize,
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
        if self.non_interactive_sessions > 0 {
            parts.push(format!("non-interactive {}", self.non_interactive_sessions));
        }
        parts.join("; ")
    }
}

pub struct Engine {
    index: SearchIndex,
    adapters: Vec<Box<dyn Adapter>>,
    query: String,
    parsed: ParsedQuery,
    sort: SortOrder,
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
            sort: SortOrder::default(),
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

    pub fn sort(&self) -> SortOrder {
        self.sort
    }

    /// Set the result ordering. Marks a search as pending (like a query change)
    /// so the loop re-runs the search after the debounce interval.
    pub fn set_sort(&mut self, sort: SortOrder) {
        if self.sort != sort {
            self.sort = sort;
            self.last_keystroke = Some(Instant::now());
        }
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
        self.results = self.index.search(&self.parsed, self.sort, now, self.limit)?;
        self.last_keystroke = None;
        Ok(())
    }

    pub fn adapter_for(&self, agent: crate::core::AgentId) -> Option<&dyn Adapter> {
        self.adapters.iter().find(|a| a.id() == agent).map(|b| b.as_ref())
    }

    pub fn transcript_for(&self, session: &SessionSummary) -> Option<Transcript> {
        let path = session.source_path.as_deref()?;
        self.adapter_for(session.agent)
            .and_then(|adapter| adapter.transcript(path).ok())
            .map(|messages| Transcript { messages })
    }

    pub fn supports_yolo(&self, session: &SessionSummary) -> bool {
        self.adapter_for(session.agent).is_some_and(|a| a.supports_yolo())
    }

    pub fn indexed_session(&self, session: &SessionSummary) -> Option<Session> {
        self.index.load_session(&session.document_key()).ok().flatten()
    }

    pub fn indexed_content(&self, session: &SessionSummary) -> Option<String> {
        self.indexed_session(session).map(|s| s.content)
    }

    pub fn resume_command_for(
        &self,
        session: &SessionSummary,
        yolo: bool,
        launcher: &crate::config::LauncherConfig,
    ) -> Option<ResumeCommand> {
        let full = self.indexed_session(session)?;
        let adapter = self.adapter_for(full.meta.agent)?;
        let argv = adapter.resume_command(&full, yolo);
        let argv = match launcher.rewrite_argv(full.meta.agent, &argv) {
            Some(Ok(rewritten)) => rewritten,
            Some(Err(_)) => return None,
            None => argv,
        };
        let prepare = full.meta.archived.then(|| adapter.unarchive_command(&full)).flatten();
        Some(ResumeCommand { directory: full.meta.directory, argv, prepare })
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
    ) -> (Receiver<Update>, Sender<Update>, std::thread::JoinHandle<()>) {
        let (tx, rx): (Sender<Update>, Receiver<Update>) = mpsc::channel();
        let tx_clone = tx.clone();
        let handle = std::thread::spawn(move || {
            let result = (|| -> Result<SyncReport> {
                let index = SearchIndex::open_or_create(&index_dir)?;
                sync_index(&index, &adapters, |_| {
                    let _ = tx.send(Update::Refresh);
                })
            })();
            let _ = tx.send(Update::Refresh);
            let _ = tx.send(Update::Done {
                report: result
                    .unwrap_or_else(|_| SyncReport { fatal_errors: 1, ..SyncReport::default() }),
            });
        });
        (rx, tx_clone, handle)
    }

    /// Re-open the reader (after a background commit) so subsequent searches see new docs.
    pub fn reload(&self) -> Result<()> {
        self.index.reload()
    }
}

fn sync_index(
    index: &SearchIndex,
    adapters: &[Box<dyn Adapter>],
    on_batch_commit: impl FnMut(&SearchIndex),
) -> Result<SyncReport> {
    sync_index_with_sidecar_dir(
        index,
        adapters,
        &crate::hooks::sidecar::sidecar_dir(),
        on_batch_commit,
    )
}

fn sync_index_with_sidecar_dir(
    index: &SearchIndex,
    adapters: &[Box<dyn Adapter>],
    sidecar_base: &Path,
    mut on_batch_commit: impl FnMut(&SearchIndex),
) -> Result<SyncReport> {
    let known = index.known_sync_state()?;
    let mut report = SyncReport::default();
    let mut all_scanned = HashMap::new();
    let mut owner = HashMap::new();
    let mut sidecar_stamps = HashMap::new();
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
                    if let Some(stamp) =
                        crate::hooks::sidecar::sidecar_stamp_in(sidecar_base, adapter.id(), &id)
                    {
                        sidecar_stamps.insert(key.clone(), stamp);
                    }
                    owner.insert(key.clone(), ai);
                    all_scanned.insert(key, entry);
                }
            }
            Err(_) => {
                report.scan_errors += 1;
            }
        }
    }

    let (mut changed, deleted) =
        diff_authoritative(&known.mtimes, &all_scanned, &authoritative_agents);
    let mut changed_keys: HashSet<_> = changed.iter().map(|(key, _)| key.clone()).collect();
    for (key, entry) in &all_scanned {
        // Codex compression preserves the rollout mtime while replacing
        // `.jsonl` with `.jsonl.zst`, so mtime alone cannot detect this change.
        if known.source_paths.get(key) != Some(&entry.path) && changed_keys.insert(key.clone()) {
            changed.push((key.clone(), entry.clone()));
        }
        if sidecar_stamps.get(key) != known.sidecar_stamps.get(key)
            && changed_keys.insert(key.clone())
        {
            changed.push((key.clone(), entry.clone()));
        }
    }
    report.deleted = deleted.len();
    for key in &deleted {
        index.delete(key)?;
    }

    let mut since_commit = 0usize;
    for (key, entry) in &changed {
        let Some(&ai) = owner.get(key) else {
            continue;
        };
        match adapters[ai].parse(&entry.path) {
            Ok(mut s) => {
                s.mtime = entry.mtime;
                if s.meta.source_path.is_none() {
                    s.meta.source_path = Some(entry.path.clone());
                }
                crate::hooks::merge::merge_sidecar_from_dir(&mut s.meta, sidecar_base);
                if s.meta.message_count == 0 || s.content.trim().is_empty() {
                    // Nothing to search or resume (e.g. a Cursor subagent spawn
                    // the model blocked before any reply). Don't index it.
                    report.empty_sessions += 1;
                    continue;
                }
                if !adapters[ai].is_interactive(&s) {
                    // Non-interactive threads (e.g. Codex sub-agent /
                    // memory-consolidation) a user would never resume. The
                    // adapter owns that judgment; don't index them.
                    report.non_interactive_sessions += 1;
                    continue;
                }
                index.upsert_with_sidecar_stamp(&s, sidecar_stamps.get(key).map(String::as_str))?;
                report.indexed += 1;
                since_commit += 1;
            }
            Err(_) => report.parse_errors += 1,
        }
        if since_commit >= 200 {
            index.commit()?;
            index.reload()?;
            on_batch_commit(index);
            since_commit = 0;
        }
    }
    index.commit()?;
    index.reload()?;
    Ok(report)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::adapters::Adapter;
    use crate::core::{AgentId, ScanEntry, Session, SessionId, SessionSummary};
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
                        s.meta.id.clone(),
                        ScanEntry { path: PathBuf::from(&s.meta.id), mtime: s.mtime },
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
                .find(|s| s.meta.id == id)
                .cloned()
                .ok_or_else(|| anyhow::anyhow!("not found"))
        }
        fn resume_command(&self, s: &Session, _yolo: bool) -> Vec<String> {
            vec!["echo".into(), s.meta.id.clone()]
        }
        fn transcript(&self, _path: &Path) -> anyhow::Result<Vec<crate::core::Message>> {
            Ok(Vec::new())
        }
        fn supports_yolo(&self) -> bool {
            true
        }
        fn unarchive_command(&self, s: &Session) -> Option<Vec<String>> {
            Some(vec!["unarchive".into(), s.meta.id.clone()])
        }
        // Stand-in for an adapter with a non-interactive notion: a neutral marker
        // so the engine test exercises the mechanism, not Codex's vocabulary.
        fn is_interactive(&self, s: &Session) -> bool {
            s.meta.source.as_deref() != Some("non-interactive")
        }
    }

    fn sess(id: &str, title: &str) -> Session {
        sess_for(AgentId::Claude, id, title)
    }

    fn sess_for(agent: AgentId, id: &str, title: &str) -> Session {
        Session {
            meta: SessionSummary {
                id: id.into(),
                agent,
                title: title.into(),
                directory: "/d".into(),
                timestamp: 100,
                message_count: 1,
                ..Default::default()
            },
            content: title.into(),
            mtime: 10,
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
        let adapters: Vec<Box<dyn Adapter>> =
            vec![adapter(AgentId::Claude, vec![sess("a", "auth bug"), sess("b", "deploy")])];
        let mut engine = Engine::new(dir.path(), adapters).unwrap();

        // synchronous full sync (the blocking core that the bg thread also calls)
        engine.sync_once().unwrap();

        engine.set_query("auth");
        engine.search().unwrap();
        assert_eq!(engine.results().len(), 1);
        assert_eq!(engine.results()[0].id, "a");
    }

    #[test]
    fn resume_command_adds_unarchive_prepare_only_for_archived() {
        let dir = tempfile::tempdir().unwrap();
        let mut active = sess("active", "live one");
        active.meta.archived = false;
        let mut archived = sess("gone", "old one");
        archived.meta.archived = true;
        let adapters: Vec<Box<dyn Adapter>> =
            vec![adapter(AgentId::Claude, vec![active, archived])];
        let mut engine = Engine::new(dir.path(), adapters).unwrap();
        engine.sync_once().unwrap();
        engine.set_query("");
        engine.search().unwrap();

        let row = |id: &str| engine.results().iter().find(|s| s.id == id).cloned().unwrap();
        let no_launcher = crate::config::LauncherConfig::default();
        let active_cmd = engine.resume_command_for(&row("active"), false, &no_launcher).unwrap();
        assert_eq!(active_cmd.prepare, None, "active sessions need no prep");

        let archived_cmd = engine.resume_command_for(&row("gone"), false, &no_launcher).unwrap();
        assert_eq!(
            archived_cmd.prepare,
            Some(vec!["unarchive".to_string(), "gone".to_string()]),
            "archived sessions unarchive before resuming"
        );
        assert_eq!(archived_cmd.argv, vec!["echo".to_string(), "gone".to_string()]);
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
            adapter(AgentId::Claude, vec![sess_for(AgentId::Claude, "same", "auth claude")]),
            adapter(AgentId::Codex, vec![sess_for(AgentId::Codex, "same", "auth codex")]),
        ];
        let mut engine = Engine::new(dir.path(), adapters).unwrap();

        engine.sync_once().unwrap();
        engine.set_query("auth");
        engine.search().unwrap();

        assert_eq!(engine.results().len(), 2);
        assert!(engine.results().iter().any(|s| s.id == "same" && s.agent == AgentId::Claude));
        assert!(engine.results().iter().any(|s| s.id == "same" && s.agent == AgentId::Codex));
    }

    #[test]
    fn unavailable_adapter_does_not_delete_existing_rows() {
        let dir = tempfile::tempdir().unwrap();
        let mut engine = Engine::new(
            dir.path(),
            vec![
                adapter(AgentId::Claude, vec![sess_for(AgentId::Claude, "a", "auth")]),
                adapter(AgentId::Codex, vec![sess_for(AgentId::Codex, "b", "deploy")]),
            ],
        )
        .unwrap();
        engine.sync_once().unwrap();

        engine.replace_adapters(vec![
            unavailable_adapter(AgentId::Claude),
            adapter(AgentId::Codex, vec![sess_for(AgentId::Codex, "b", "deploy")]),
        ]);
        let report = engine.sync_once().unwrap();
        assert_eq!(report.adapters_unavailable, 1);

        engine.set_query("");
        engine.search().unwrap();
        assert_eq!(engine.results().len(), 2);
        assert!(engine.results().iter().any(|s| s.id == "a" && s.agent == AgentId::Claude));
    }

    #[test]
    fn scan_failure_does_not_delete_existing_rows() {
        let dir = tempfile::tempdir().unwrap();
        let mut engine = Engine::new(
            dir.path(),
            vec![adapter(AgentId::Claude, vec![sess_for(AgentId::Claude, "a", "auth")])],
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
    fn empty_sessions_are_not_indexed() {
        let dir = tempfile::tempdir().unwrap();
        let mut empty = sess_for(AgentId::Claude, "empty", "");
        empty.content.clear();
        empty.meta.message_count = 0;
        let adapters: Vec<Box<dyn Adapter>> =
            vec![adapter(AgentId::Claude, vec![empty, sess("real", "auth bug")])];
        let mut engine = Engine::new(dir.path(), adapters).unwrap();
        engine.sync_once().unwrap();
        engine.set_query("");
        engine.search().unwrap();

        // The empty session is skipped; only the real one is searchable.
        assert_eq!(engine.results().len(), 1);
        assert_eq!(engine.results()[0].id, "real");
    }

    #[test]
    fn non_interactive_sessions_are_not_indexed() {
        let dir = tempfile::tempdir().unwrap();
        let mut sub = sess_for(AgentId::Codex, "subagent", "helper thread output");
        sub.meta.source = Some("non-interactive".into());
        let adapters: Vec<Box<dyn Adapter>> =
            vec![adapter(AgentId::Codex, vec![sub, sess("real", "auth bug")])];
        let mut engine = Engine::new(dir.path(), adapters).unwrap();
        let report = engine.sync_once().unwrap();
        assert_eq!(report.non_interactive_sessions, 1);
        engine.set_query("");
        engine.search().unwrap();

        // The non-interactive session is skipped; only the real one is searchable.
        assert_eq!(engine.results().len(), 1);
        assert_eq!(engine.results()[0].id, "real");
    }

    #[test]
    fn parse_failures_and_empty_sessions_are_reported() {
        let dir = tempfile::tempdir().unwrap();
        let mut empty = sess_for(AgentId::Claude, "empty", "");
        empty.content.clear();
        empty.meta.message_count = 0;
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

    #[test]
    fn sidecar_only_change_reindexes_unchanged_session() {
        use crate::hooks::sidecar::{HookEvent, Sidecar, SidecarEvent, sidecar_path_in};

        let dir = tempfile::tempdir().unwrap();
        let sidecars = tempfile::tempdir().unwrap();
        let index = SearchIndex::open_or_create(dir.path()).unwrap();
        let adapters: Vec<Box<dyn Adapter>> =
            vec![adapter(AgentId::Claude, vec![sess("s1", "auth")])];

        let first =
            sync_index_with_sidecar_dir(&index, &adapters, sidecars.path(), |_| {}).unwrap();
        assert_eq!(first.indexed, 1);
        let initial = index.search(&ParsedQuery::default(), SortOrder::Recent, 1_000, 10).unwrap();
        assert_eq!(initial[0].branch, None);

        let sidecar = Sidecar {
            version: 1,
            session_id: "s1".into(),
            agent: AgentId::Claude,
            events: vec![SidecarEvent {
                event: HookEvent::Stop,
                timestamp: 200,
                cwd: None,
                branch: Some("feature/hooks".into()),
                repo_url: None,
                worktree: None,
                permission_mode: None,
            }],
        };
        sidecar.write(&sidecar_path_in(sidecars.path(), AgentId::Claude, "s1")).unwrap();

        let second =
            sync_index_with_sidecar_dir(&index, &adapters, sidecars.path(), |_| {}).unwrap();
        assert_eq!(second.indexed, 1);
        let updated = index.search(&ParsedQuery::default(), SortOrder::Recent, 1_000, 10).unwrap();
        assert_eq!(updated[0].branch.as_deref(), Some("feature/hooks"));
    }

    #[test]
    fn source_path_change_reindexes_when_mtime_is_preserved() {
        use crate::adapters::codex::CodexAdapter;
        use std::fs::FileTimes;

        let data = tempfile::tempdir().unwrap();
        let sessions = data.path().join("sessions/2026/07/11");
        std::fs::create_dir_all(&sessions).unwrap();
        let plain = sessions.join("rollout-2026-07-11T10-00-00-session.jsonl");
        let compressed = sessions.join("rollout-2026-07-11T10-00-00-session.jsonl.zst");
        let rollout = |message: &str| {
            format!(
                concat!(
                    r#"{{"type":"session_meta","timestamp":"2026-07-11T10:00:00Z","payload":{{"id":"session","cwd":"/w"}}}}"#,
                    "\n",
                    r#"{{"type":"event_msg","payload":{{"type":"user_message","message":"{message}"}}}}"#,
                    "\n"
                ),
                message = message
            )
        };
        std::fs::write(&plain, rollout("before compression")).unwrap();
        let original_mtime = std::fs::metadata(&plain).unwrap().modified().unwrap();

        let index_dir = tempfile::tempdir().unwrap();
        let adapters: Vec<Box<dyn Adapter>> =
            vec![Box::new(CodexAdapter::new(data.path().to_path_buf()))];
        let mut engine = Engine::new(index_dir.path(), adapters).unwrap();
        assert_eq!(engine.sync_once().unwrap().indexed, 1);

        std::fs::write(
            &compressed,
            zstd::stream::encode_all(rollout("after compression").as_bytes(), 0).unwrap(),
        )
        .unwrap();
        std::fs::File::options()
            .write(true)
            .open(&compressed)
            .unwrap()
            .set_times(FileTimes::new().set_modified(original_mtime))
            .unwrap();
        std::fs::remove_file(&plain).unwrap();

        assert_eq!(engine.sync_once().unwrap().indexed, 1);
        engine.set_query("after compression");
        engine.search().unwrap();
        assert_eq!(engine.results().len(), 1);
        assert_eq!(engine.results()[0].source_path.as_deref(), Some(compressed.as_path()));
    }
}
