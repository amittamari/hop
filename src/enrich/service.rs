//! Background resolution of slow enrichers with a disk cache.
//!
//! The UI sends `EnrichRequest`s (a session + which enricher); a worker thread
//! resolves them (checking/populating the on-disk cache) and returns
//! `EnrichResult`s the UI folds into its render state.

use super::{EnrichValue, Enricher};
use crate::core::SessionSummary;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::mpsc::{Receiver, Sender};
use std::time::{SystemTime, UNIX_EPOCH};

pub struct EnrichRequest {
    pub session: SessionSummary,
    pub enricher: &'static str,
}

pub struct EnrichResult {
    pub session_key: String,
    pub enricher: &'static str,
    /// None = resolved-but-absent (render as "—"); Some = a value.
    pub value: Option<EnrichValue>,
}

#[derive(Serialize, Deserialize, Default)]
struct CacheFile {
    /// cache_key -> (text-or-empty, fetched_at_unix_secs)
    entries: HashMap<String, (String, u64)>,
}

fn now_secs() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
}

/// Pure cache-hit check used by the worker and by tests.
fn cache_lookup(cache: &CacheFile, key: &str, ttl_secs: u64) -> Option<Option<EnrichValue>> {
    let (text, fetched) = cache.entries.get(key)?;
    if now_secs().saturating_sub(*fetched) > ttl_secs {
        return None; // stale
    }
    if text.is_empty() { Some(None) } else { Some(Some(EnrichValue { text: text.clone() })) }
}

pub struct EnrichmentService {
    pub req_tx: Sender<EnrichRequest>,
    pub res_rx: Receiver<EnrichResult>,
    _handle: std::thread::JoinHandle<()>,
}

#[derive(Default)]
pub struct EnrichmentState {
    pub resolved: HashMap<(String, &'static str), Option<String>>,
    requested: std::collections::HashSet<(String, &'static str)>,
}

impl EnrichmentState {
    pub fn pr_pending(&self) -> usize {
        self.requested.iter().filter(|key| !self.resolved.contains_key(*key)).count()
    }

    pub fn request_visible(
        &mut self,
        service: Option<&EnrichmentService>,
        rows: &[SessionSummary],
    ) {
        let Some(service) = service else {
            return;
        };
        for session in rows {
            let key = (session.document_key(), "pr");
            if !self.requested.contains(&key) {
                self.requested.insert(key.clone());
                let _ =
                    service.req_tx.send(EnrichRequest { session: session.clone(), enricher: "pr" });
            }
        }
        self.drain(service);
    }

    pub fn drain(&mut self, service: &EnrichmentService) {
        while let Ok(r) = service.res_rx.try_recv() {
            self.resolved.insert((r.session_key, r.enricher), r.value.map(|v| v.text));
        }
    }
}

impl EnrichmentService {
    /// Spawn the worker. `enrichers` are the slow ones to service; `cache_path`
    /// is the JSON cache file (created/loaded lazily).
    pub fn spawn(enrichers: Vec<Box<dyn Enricher>>, cache_path: PathBuf) -> EnrichmentService {
        let (req_tx, req_rx) = std::sync::mpsc::channel::<EnrichRequest>();
        let (res_tx, res_rx) = std::sync::mpsc::channel::<EnrichResult>();
        let handle = std::thread::spawn(move || {
            let mut cache: CacheFile = std::fs::read_to_string(&cache_path)
                .ok()
                .and_then(|s| serde_json::from_str(&s).ok())
                .unwrap_or_default();

            while let Ok(req) = req_rx.recv() {
                let Some(enr) = enrichers.iter().find(|e| e.id() == req.enricher) else {
                    let _ = res_tx.send(EnrichResult {
                        session_key: req.session.document_key(),
                        enricher: req.enricher,
                        value: None,
                    });
                    continue;
                };
                let key = enr.cache_key(&req.session);
                let ttl = enr.ttl().as_secs();
                let value = match cache_lookup(&cache, &key, ttl) {
                    Some(hit) => hit,
                    None => {
                        let resolved = enr.resolve(&req.session);
                        cache.entries.insert(
                            key.clone(),
                            (
                                resolved.as_ref().map(|v| v.text.clone()).unwrap_or_default(),
                                now_secs(),
                            ),
                        );
                        if let Some(parent) = cache_path.parent() {
                            let _ = std::fs::create_dir_all(parent);
                        }
                        if let Ok(s) = serde_json::to_string(&cache) {
                            let _ = std::fs::write(&cache_path, s);
                        }
                        resolved
                    }
                };
                let _ = res_tx.send(EnrichResult {
                    session_key: req.session.document_key(),
                    enricher: req.enricher,
                    value,
                });
            }
        });
        EnrichmentService { req_tx, res_rx, _handle: handle }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::{AgentId, SessionSummary};
    use crate::enrich::{EnrichKind, Enricher};
    use std::time::Duration;

    struct FakeEnricher;
    impl Enricher for FakeEnricher {
        fn id(&self) -> &'static str {
            "fake"
        }
        fn kind(&self) -> EnrichKind {
            EnrichKind::Slow
        }
        fn resolve(&self, s: &SessionSummary) -> Option<EnrichValue> {
            Some(EnrichValue { text: format!("v:{}", s.id) })
        }
        fn cache_key(&self, s: &SessionSummary) -> String {
            s.id.clone()
        }
        fn ttl(&self) -> Duration {
            Duration::from_secs(3600)
        }
    }

    fn sess(id: &str) -> SessionSummary {
        SessionSummary {
            id: id.into(),
            agent: AgentId::Claude,
            title: "t".into(),
            directory: "/w".into(),
            timestamp: 1,
            ..Default::default()
        }
    }

    #[test]
    fn resolves_and_caches_to_disk() {
        let tmp = tempfile::tempdir().unwrap();
        let cache = tmp.path().join("gh_pr.json");
        let svc = EnrichmentService::spawn(vec![Box::new(FakeEnricher)], cache.clone());
        svc.req_tx.send(EnrichRequest { session: sess("a"), enricher: "fake" }).unwrap();
        let r = svc.res_rx.recv_timeout(Duration::from_secs(2)).unwrap();
        assert_eq!(r.session_key, "claude:a");
        assert_eq!(r.value.unwrap().text, "v:a");
        // cache file written
        assert!(cache.exists());
    }
}
