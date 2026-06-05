use anyhow::Result;
use clap::Parser;
use hop::adapters;
use hop::cli::Cli;
use hop::config::{Config, UiState};
use hop::core::Message;
use hop::engine::{Engine, Update};
use hop::enrich::gh_pr::GhPrEnricher;
use hop::enrich::service::{EnrichRequest, EnrichmentService};
use hop::enrich::{BranchEnricher, Enricher, RepoEnricher};
use hop::resume;
use hop::tui::{preview, view, Action, App};
use ratatui::crossterm::event::{self, Event};
use std::collections::HashMap;
use std::time::Duration;

fn index_dir() -> std::path::PathBuf {
    directories::ProjectDirs::from("dev", "hop", "hop")
        .map(|d| d.cache_dir().join("index"))
        .unwrap_or_else(|| std::path::PathBuf::from(".hop-index"))
}

fn enrich_cache_path() -> std::path::PathBuf {
    directories::ProjectDirs::from("dev", "hop", "hop")
        .map(|d| d.cache_dir().join("enrich").join("gh_pr.json"))
        .unwrap_or_else(|| std::path::PathBuf::from(".hop-enrich.json"))
}

fn ui_state_path() -> std::path::PathBuf {
    directories::ProjectDirs::from("dev", "hop", "hop")
        .map(|d| d.cache_dir().join("ui_state.toml"))
        .unwrap_or_else(|| std::path::PathBuf::from(".hop-ui-state.toml"))
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    let config = Config::load()?;
    let dir = index_dir();

    if cli.rebuild && dir.exists() {
        let _ = std::fs::remove_dir_all(&dir);
    }

    // Build adapters twice: one set for the foreground engine, one moved to the bg thread.
    let fg_adapters = adapters::default_adapters(&config);
    let bg_adapters = adapters::default_adapters(&config);

    let mut engine = Engine::new(&dir, fg_adapters)?;
    engine.set_query(cli.initial_query());
    engine.search()?; // immediate results from whatever is already indexed

    // background sync streams new sessions in
    let (updates, _handle) = Engine::spawn_background_sync(dir.clone(), bg_adapters);

    let fast_enrichers: Vec<Box<dyn Enricher>> = vec![Box::new(RepoEnricher), Box::new(BranchEnricher)];
    let pr_enabled = !config.columns.disabled.iter().any(|d| d == "pr");
    let service = if pr_enabled {
        Some(EnrichmentService::spawn(vec![Box::new(GhPrEnricher)], enrich_cache_path()))
    } else {
        None
    };

    let ui_path = ui_state_path();
    let init_preview = UiState::load(&ui_path)
        .map(|u| (u.preview_visible, u.preview_width_pct))
        .unwrap_or((config.preview.visible, config.preview.width_pct));

    // resume request escapes the TUI loop so we exec AFTER restoring the terminal
    let pending = run_tui(
        &mut engine,
        updates,
        &fast_enrichers,
        service.as_ref(),
        &config,
        init_preview,
        ui_path,
    )?;

    if let Some((session, yolo)) = pending {
        let agent = engine
            .adapter_for(session.agent)
            .map(|a| a.resume_command(&session, yolo || cli.yolo))
            .unwrap_or_default();
        // terminal already restored by run_tui's Drop/restore
        resume::exec_resume(&session.directory, &agent)?;
    }
    Ok(())
}

/// Runs the event loop. Returns Some((session, yolo)) if the user chose to resume.
fn run_tui(
    engine: &mut Engine,
    updates: std::sync::mpsc::Receiver<Update>,
    fast_enrichers: &[Box<dyn Enricher>],
    service: Option<&EnrichmentService>,
    config: &Config,
    init_preview: (bool, u16),
    ui_path: std::path::PathBuf,
) -> Result<Option<(hop::core::Session, bool)>> {
    let mut terminal = ratatui::init();
    let mut app = App::new();
    app.set_query(engine.query().to_string());
    app.set_keymap(hop::tui::keymap::Preset::from_str(&config.keymap));
    app.set_preview(init_preview.0, init_preview.1);
    sync_results_into_app(engine, &mut app);

    // slow-enrichment state
    let mut resolved: HashMap<(String, &'static str), Option<String>> = HashMap::new();
    let mut requested: std::collections::HashSet<(String, &'static str)> = Default::default();

    // memoized preview state, rebuilt only when (selection, query) changes
    let mut transcript: Vec<Message> = Vec::new();
    let mut transcript_for: Option<String> = None;
    let mut preview_lines: Vec<ratatui::text::Line<'static>> = Vec::new();
    let mut preview_key: Option<(String, String)> = None;
    let mut preview_base: u16 = 0;

    let outcome = (|| -> Result<Option<(hop::core::Session, bool)>> {
        loop {
            // re-parse the selected session's transcript on selection change
            let sel_id = engine.results().get(app.selected()).map(|s| s.id.clone());
            if app.preview_visible() && sel_id != transcript_for {
                transcript = match engine.results().get(app.selected()) {
                    Some(s) => engine
                        .adapter_for(s.agent)
                        .and_then(|a| latest_transcript(a, s))
                        .unwrap_or_default(),
                    None => Vec::new(),
                };
                transcript_for = sel_id.clone();
            }

            // rebuild memoized preview lines when selection or query changes
            let pkey = (sel_id.clone().unwrap_or_default(), app.query().to_string());
            if app.preview_visible() && preview_key.as_ref() != Some(&pkey) {
                let agent = engine
                    .results()
                    .get(app.selected())
                    .map(|s| s.agent)
                    .unwrap_or(hop::core::AgentId::Claude);
                preview_lines = preview::render_transcript(&transcript, app.query(), agent);
                preview_base = preview::first_match_line(&preview_lines, app.query())
                    .map(|i| i as u16)
                    .unwrap_or(0);
                preview_key = Some(pkey);
            }

            let now = jiff::Timestamp::now().as_second();
            terminal.draw(|f| {
                view::render(f, &app, now, fast_enrichers, &resolved, &preview_lines, preview_base)
            })?;

            // request PR enrichment for visible rows (cap to first ~200), dedup'd.
            // Clear `content` before sending — PR resolution doesn't need it and
            // it can be large.
            if let Some(svc) = service {
                for s in engine.results().iter().take(200) {
                    let key = (s.id.clone(), "pr");
                    if !requested.contains(&key) {
                        requested.insert(key);
                        let mut slim = s.clone();
                        slim.content = String::new();
                        let _ = svc.req_tx.send(EnrichRequest { session: slim, enricher: "pr" });
                    }
                }
                while let Ok(r) = svc.res_rx.try_recv() {
                    resolved.insert((r.session_id, r.enricher), r.value.map(|v| v.text));
                }
            }

            if !app.modal_open() {
                while let Ok(update) = updates.try_recv() {
                    if let Update::Refresh = update {
                        engine.reload()?;
                        engine.search()?;
                        sync_results_into_app(engine, &mut app);
                        transcript_for = None; // force re-parse next frame
                        preview_key = None;
                    }
                }
            }

            if event::poll(Duration::from_millis(50))? {
                if let Event::Key(key) = event::read()? {
                    match app.handle_key(key) {
                        Action::Quit => return Ok(None),
                        Action::Search => engine.set_query(app.query().to_string()),
                        Action::Resume { index, yolo } => {
                            if let Some(s) = engine.results().get(index).cloned() {
                                return Ok(Some((s, yolo)));
                            }
                        }
                        _ => {}
                    }
                }
            }

            if !app.modal_open() && engine.search_due() {
                engine.search()?;
                sync_results_into_app(engine, &mut app);
                transcript_for = None;
                preview_key = None;
            }
        }
    })();

    ratatui::restore();
    let _ = UiState {
        preview_visible: app.preview_visible(),
        preview_width_pct: app.preview_width_pct(),
    }
    .save(&ui_path);
    outcome
}

/// Re-parse the on-disk file for `s` into a transcript. The path isn't stored on
/// the Session, so re-scan the adapter to find it by id (cheap stat-level scan).
fn latest_transcript(
    adapter: &dyn hop::adapters::Adapter,
    s: &hop::core::Session,
) -> Option<Vec<Message>> {
    let scanned = adapter.scan().ok()?;
    let entry = scanned.get(&s.id)?;
    adapter.transcript(&entry.path).ok()
}

fn sync_results_into_app(engine: &Engine, app: &mut App) {
    app.set_results(engine.results().to_vec());
}
