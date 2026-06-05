use anyhow::Result;
use clap::Parser;
use hop::adapters;
use hop::cli::Cli;
use hop::config::{Config, UiState};
use hop::core::{ResumeCommand, SessionSummary};
use hop::engine::{Engine, Update};
use hop::enrich::gh_pr::GhPrEnricher;
use hop::enrich::service::{EnrichmentService, EnrichmentState};
use hop::enrich::{BranchEnricher, Enricher, RepoEnricher};
use hop::resume;
use hop::tui::{preview, view::RenderModel, view::StatusLine, Action, App};
use ratatui::crossterm::event::{self, Event};
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

    let pr_enabled = !config.columns.disabled.iter().any(|d| d == "pr");
    // Enrichers passed to the renderer for cell metadata. GhPrEnricher is included
    // so the Slow "pr" column can read the resolved map; its resolve() is never
    // called on the UI thread (the Slow branch only reads `resolved`).
    let mut render_enrichers: Vec<Box<dyn Enricher>> =
        vec![Box::new(RepoEnricher), Box::new(BranchEnricher)];
    if pr_enabled {
        render_enrichers.push(Box::new(GhPrEnricher));
    }
    let service = if pr_enabled {
        Some(EnrichmentService::spawn(
            vec![Box::new(GhPrEnricher)],
            enrich_cache_path(),
        ))
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
        &render_enrichers,
        service.as_ref(),
        &config,
        init_preview,
        ui_path,
    )?;

    if let Some((session, yolo)) = pending {
        let command = engine
            .resume_command_for(&session, yolo || cli.yolo)
            .unwrap_or_else(|| ResumeCommand {
                directory: session.directory.clone(),
                argv: Vec::new(),
            });
        // terminal already restored by run_tui's Drop/restore
        resume::exec_resume(&command.directory, &command.argv)?;
    }
    Ok(())
}

/// Runs the event loop. Returns Some((session, yolo)) if the user chose to resume.
fn run_tui(
    engine: &mut Engine,
    updates: std::sync::mpsc::Receiver<Update>,
    render_enrichers: &[Box<dyn Enricher>],
    service: Option<&EnrichmentService>,
    config: &Config,
    init_preview: (bool, u16),
    ui_path: std::path::PathBuf,
) -> Result<Option<(SessionSummary, bool)>> {
    let mut terminal = ratatui::init();
    let mut app = App::new();
    app.set_query(engine.query().to_string());
    app.set_preview(init_preview.0, init_preview.1);
    app.set_preview_header(config.preview.metadata_header);
    sync_results_into_app(engine, &mut app);

    let columns = hop::columns::configured_columns(
        hop::columns::default_columns(),
        &config.columns.disabled,
        &config.columns.order,
    );

    let mut enrichment = EnrichmentState::default();
    let mut preview_state = preview::PreviewState::default();
    let mut sync_status = Some("syncing".to_string());

    let outcome = (|| -> Result<Option<(SessionSummary, bool)>> {
        loop {
            let area = terminal.size()?;
            let list_rows_height = area.height.saturating_sub(3);
            let preview_height = if app.preview_visible() {
                let body_height = area.height.saturating_sub(2);
                if app.preview_header_visible() && app.results().get(app.selected()).is_some() {
                    body_height.saturating_sub(2)
                } else {
                    body_height
                }
            } else {
                1
            };
            app.set_viewport_metrics(list_rows_height, preview_height);

            let terms = engine.parsed_query().free_terms();
            let selected_for_preview = app.results().get(app.selected()).cloned();
            preview_state.update(
                &mut app,
                selected_for_preview.as_ref(),
                &terms,
                |s| engine.transcript_for(s),
                |s| engine.indexed_content(s),
            );
            let now = jiff::Timestamp::now().as_second();
            let status = StatusLine {
                sync: sync_status.clone(),
                pr_pending: enrichment.pr_pending(),
                warning: if app.preview_visible()
                    && selected_for_preview.is_some()
                    && preview_state.source_unavailable()
                {
                    Some("source unavailable".to_string())
                } else {
                    None
                },
                filters: engine.parsed_query().filter_summary(),
            };
            let modal_command = app.yolo_modal().and_then(|(index, yolo)| {
                app.results()
                    .get(index)
                    .and_then(|s| engine.resume_command_for(s, yolo))
                    .map(|command| command.argv)
            });
            terminal.draw(|f| {
                hop::tui::view::render(
                    f,
                    &app,
                    RenderModel {
                        now,
                        columns: &columns,
                        enrichers: render_enrichers,
                        resolved: &enrichment.resolved,
                        preview_lines: &preview_state.lines,
                        status: &status,
                        modal_command: modal_command.as_deref(),
                    },
                )
            })?;

            let visible = hop::tui::view::visible_result_range(
                app.results().len(),
                app.selected(),
                list_rows_height as usize,
            );
            let visible_rows = app.results().get(visible).unwrap_or_default();
            enrichment.request_visible(service, visible_rows);

            if !app.modal_open() {
                while let Ok(update) = updates.try_recv() {
                    match update {
                        Update::Refresh => {
                            engine.reload()?;
                            engine.search()?;
                            sync_results_into_app(engine, &mut app);
                            preview_state.invalidate();
                        }
                        Update::Done { report } => {
                            sync_status = Some(report.status_line());
                        }
                    }
                }
            }

            if event::poll(Duration::from_millis(50))? {
                if let Event::Key(key) = event::read()? {
                    match app.handle_key(key) {
                        Action::Quit => return Ok(None),
                        Action::Search => engine.set_query(app.query().to_string()),
                        Action::Resume { index, yolo } => {
                            if let Some(s) = app.results().get(index).cloned() {
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
                preview_state.invalidate();
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

fn sync_results_into_app(engine: &Engine, app: &mut App) {
    let results = engine.results().to_vec();
    let yolo_supported = results.iter().map(|s| engine.supports_yolo(s)).collect();
    app.set_results_with_yolo(results, yolo_supported);
}
