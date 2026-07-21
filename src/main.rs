use anyhow::Result;
use clap::Parser;
use hop::adapters;
use hop::cli::Cli;
use hop::config::Config;
use hop::core::{ResumeCommand, SessionSummary};
use hop::engine::{Engine, Update};
use hop::enrich::gh_pr::GhPrEnricher;
use hop::enrich::service::{EnrichmentService, EnrichmentState};
use hop::enrich::{BranchEnricher, Enricher, RepoEnricher};
use hop::resume;
use hop::tui::toolbar::Scope;
use hop::tui::{Action, App, SearchMode, preview, view::RenderModel, view::StatusLine};
use ratatui::crossterm::event::{self, Event};
use std::time::Duration;

fn hop_dirs() -> Option<directories::ProjectDirs> {
    directories::ProjectDirs::from("dev", "hop", "hop")
}

fn index_dir() -> std::path::PathBuf {
    hop_dirs()
        .map(|d| d.cache_dir().join("index"))
        .unwrap_or_else(|| std::path::PathBuf::from(".hop-index"))
}

fn enrich_cache_path() -> std::path::PathBuf {
    hop_dirs()
        .map(|d| d.cache_dir().join("enrich").join("gh_pr.json"))
        .unwrap_or_else(|| std::path::PathBuf::from(".hop-enrich.json"))
}

fn update_cache_path() -> std::path::PathBuf {
    hop_dirs()
        .map(|d| d.cache_dir().join("update_check.json"))
        .unwrap_or_else(|| std::path::PathBuf::from(".hop-update-check.json"))
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    if let Some(cmd) = &cli.command {
        return match cmd {
            hop::cli::Command::Meta { action } => match action {
                hop::cli::MetaAction::Capture { agent, event } => {
                    let result: anyhow::Result<()> = (|| {
                        let agent = hop::core::AgentId::from_slug(agent)
                            .ok_or_else(|| anyhow::anyhow!("unknown agent: {agent}"))?;
                        let event = match event.as_str() {
                            "start" => hop::hooks::sidecar::HookEvent::Start,
                            "stop" => hop::hooks::sidecar::HookEvent::Stop,
                            _ => anyhow::bail!("unknown event: {event}"),
                        };
                        let mut stdin = String::new();
                        std::io::Read::read_to_string(&mut std::io::stdin(), &mut stdin)?;
                        hop::hooks::capture::capture(agent, event, &stdin)
                    })();
                    let _ = result;
                    Ok(())
                }
            },
            hop::cli::Command::Hooks { action } => {
                let home = hop::hooks::providers::home_dir();
                match action {
                    hop::cli::HooksAction::Install { all, provider } => {
                        let providers = hop::hooks::providers::detect_providers();
                        let targets: Vec<_> = if let Some(name) = provider {
                            let agent = hop::core::AgentId::from_slug(name)
                                .ok_or_else(|| anyhow::anyhow!("unknown provider: {name}"))?;
                            providers.into_iter().filter(|p| p.agent == agent).collect()
                        } else if *all {
                            providers.into_iter().filter(|p| p.detected).collect()
                        } else {
                            // Interactive: show detected, ask user
                            let detected: Vec<_> =
                                providers.into_iter().filter(|p| p.detected).collect();
                            if detected.is_empty() {
                                eprintln!("No providers detected.");
                                return Ok(());
                            }
                            eprintln!("Detected providers:");
                            for (i, p) in detected.iter().enumerate() {
                                let effort = if p.best_effort { " [best-effort]" } else { "" };
                                let status = if p.installed { " (already installed)" } else { "" };
                                eprintln!("  {}. {}{}{}", i + 1, p.agent.badge(), effort, status);
                            }
                            eprint!("Install for all? [Y/n] ");
                            let mut input = String::new();
                            std::io::stdin().read_line(&mut input)?;
                            if input.trim().eq_ignore_ascii_case("n") {
                                return Ok(());
                            }
                            detected
                        };
                        for p in &targets {
                            match hop::hooks::providers::install_provider(p.agent, &home) {
                                Ok(msg) => eprintln!("{msg}"),
                                Err(e) => {
                                    eprintln!("Failed to install for {}: {e}", p.agent.badge())
                                }
                            }
                        }
                        Ok(())
                    }
                    hop::cli::HooksAction::Uninstall { all: _, provider } => {
                        let providers = hop::hooks::providers::detect_providers();
                        let targets: Vec<_> = if let Some(name) = provider {
                            let agent = hop::core::AgentId::from_slug(name)
                                .ok_or_else(|| anyhow::anyhow!("unknown provider: {name}"))?;
                            providers.into_iter().filter(|p| p.agent == agent).collect()
                        } else {
                            providers.into_iter().filter(|p| p.installed).collect()
                        };
                        for p in &targets {
                            match hop::hooks::providers::uninstall_provider(p.agent, &home) {
                                Ok(msg) => eprintln!("{msg}"),
                                Err(e) => {
                                    eprintln!("Failed to uninstall for {}: {e}", p.agent.badge())
                                }
                            }
                        }
                        Ok(())
                    }
                    hop::cli::HooksAction::Status => {
                        let providers = hop::hooks::providers::detect_providers();
                        for p in &providers {
                            let detected = if p.detected { "detected" } else { "not found" };
                            let installed = if p.installed { "installed" } else { "not installed" };
                            let effort = if p.best_effort { " [best-effort]" } else { "" };
                            eprintln!(
                                "{}: {} / {}{}",
                                p.agent.badge(),
                                detected,
                                installed,
                                effort
                            );
                        }
                        Ok(())
                    }
                }
            }
        };
    }

    if cli.version {
        println!("hop {}", env!("CARGO_PKG_VERSION"));
        match hop::update::check_for_update(&update_cache_path()) {
            Some(info) => eprintln!("{}", hop::update::upgrade_message(&info)),
            None => println!("(up to date)"),
        }
        return Ok(());
    }

    let config = Config::load()?;
    let dir = index_dir();

    if cli.rebuild && dir.exists() {
        let _ = std::fs::remove_dir_all(&dir);
    }

    // Build adapters twice: one set for the foreground engine, one moved to the bg thread.
    let fg_adapters = adapters::default_adapters(&config);
    let bg_adapters = adapters::default_adapters(&config);

    let mut engine = Engine::new(&dir, fg_adapters)?;
    // Auto-scope to the current repo unless the user opted out or set an explicit
    // repo filter. Resolves the cwd's `origin` remote into an `owner/name` slug.
    let auto_repo = cli
        .wants_auto_repo()
        .then(|| adapters::git_remote_url("."))
        .flatten()
        .and_then(|url| hop::enrich::repo_slug_from_url(&url));

    // Resolve the initial search mode. Simple (guided toolbar) is the default, but
    // CLI-typed DSL or filter flags the toolbar can't represent start in raw mode
    // so nothing is silently dropped.
    let configured_mode = SearchMode::from_config(&config.search_mode);
    let mode = if cli.has_unsupported_simple_flags() || cli.query_has_dsl() {
        SearchMode::Raw
    } else {
        configured_mode
    };
    // Slug for the simple-mode "This repo" scope: explicit --repo wins, else auto.
    let scope_slug = cli.repo.clone().or_else(|| auto_repo.clone());
    let scope = if cli.all || scope_slug.is_none() { Scope::All } else { Scope::ThisRepo };
    let initial = match mode {
        SearchMode::Raw => {
            let q = cli.initial_query(auto_repo.as_deref());
            engine.set_query(q.clone());
            InitialSearch { mode, scope, repo_slug: scope_slug, input: q }
        }
        SearchMode::Simple => {
            let free = cli.query.clone().unwrap_or_default();
            let repo = matches!(scope, Scope::ThisRepo).then(|| scope_slug.as_deref()).flatten();
            engine.set_query(hop::query::compose_simple(&free, repo));
            InitialSearch { mode, scope, repo_slug: scope_slug, input: free }
        }
    };
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
        Some(EnrichmentService::spawn(vec![Box::new(GhPrEnricher)], enrich_cache_path()))
    } else {
        None
    };

    let init_preview = (config.display.visible, config.display.width_pct);

    let update_cache = update_cache_path();
    let update_handle = std::thread::spawn(move || hop::update::check_for_update(&update_cache));

    // resume request escapes the TUI loop so we exec AFTER restoring the terminal
    let pending = run_tui(
        &mut engine,
        updates,
        &render_enrichers,
        service.as_ref(),
        &config,
        init_preview,
        initial,
    )?;

    if let Ok(Some(info)) = update_handle.join() {
        eprintln!("{}", hop::update::upgrade_message(&info));
    }

    if let Some((session, yolo)) = pending {
        let command = engine
            .resume_command_for(&session, yolo || cli.yolo, &config.launcher)
            .unwrap_or_else(|| ResumeCommand {
                directory: session.directory.clone(),
                argv: Vec::new(),
                prepare: None,
            });
        // terminal already restored by run_tui's Drop/restore
        // Run any prep step (e.g. `codex unarchive <id>`) before exec-resuming.
        if let Some(prepare) = &command.prepare {
            resume::run_prepare(prepare)?;
        }
        resume::exec_resume(&command.directory, &command.argv)?;
    }
    Ok(())
}

/// Resolved initial search state handed to the TUI: how the query line is
/// interpreted plus the toolbar scope and the launch repo slug.
struct InitialSearch {
    mode: SearchMode,
    scope: Scope,
    repo_slug: Option<String>,
    input: String,
}

/// Runs the event loop. Returns Some((session, yolo)) if the user chose to resume.
#[allow(clippy::too_many_arguments)]
fn run_tui(
    engine: &mut Engine,
    updates: std::sync::mpsc::Receiver<Update>,
    render_enrichers: &[Box<dyn Enricher>],
    service: Option<&EnrichmentService>,
    config: &Config,
    init_preview: (bool, u16),
    initial: InitialSearch,
) -> Result<Option<(SessionSummary, bool)>> {
    // Resolve keybindings before entering the alternate screen so any config
    // warnings land on the normal terminal rather than being clobbered.
    let (keymap, keymap_warnings) = hop::tui::keymap::Keymap::from_config(&config.keybindings);
    for warning in &keymap_warnings {
        eprintln!("hop: {warning}");
    }

    // Build the glyph set once: pick the variant from config, then inject each
    // agent's mark glyph from its adapter (B-011: the tui layer never names an
    // agent-specific glyph literal). A fresh, cheap adapter set is used purely to
    // read the glyphs.
    let mut glyphs = hop::tui::glyphs::Glyphs::from_icons_enabled(config.display.icons);
    for adapter in adapters::default_adapters(config) {
        glyphs.set_agent_glyph(adapter.id(), adapter.agent_glyph());
    }

    let mut terminal = ratatui::init();
    let mut app = App::new();
    app.set_glyphs(glyphs);
    app.set_keymap(keymap);
    app.init_search(initial.mode, initial.scope, initial.repo_slug, initial.input);
    app.set_preview(init_preview.0, init_preview.1);
    app.set_preview_header(config.display.metadata_header);
    sync_results_into_app(engine, &mut app);

    let row_style = config.display.resolved_row_style();
    let columns = hop::tui::columns::configured_columns(
        hop::tui::columns::default_columns(),
        &config.columns.disabled,
        &config.columns.order,
    );

    let mut state = LoopState::new();

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
            let effective_list_rows = if row_style == hop::config::RowStyle::Card {
                list_rows_height / 3
            } else {
                list_rows_height
            };
            app.set_viewport_metrics(effective_list_rows, preview_height);
            app.tick();

            let terms = engine.parsed_query().free_terms();
            let selected_for_preview = app.results().get(app.selected()).cloned();
            let preview_w = if app.preview_visible() {
                let pct = app.preview_width_pct() as u32;
                (area.width as u32 * pct / 100).saturating_sub(3) as u16
            } else {
                area.width
            };
            state.preview.update(
                &mut app,
                selected_for_preview.as_ref(),
                &terms,
                |s| engine.transcript_for(s),
                |s| engine.indexed_content(s),
                row_style == hop::config::RowStyle::Card,
                preview_w,
            );
            let now = jiff::Timestamp::now().as_second();
            let status = state.build_status(&app, selected_for_preview.is_some());
            let modal_command = app.yolo_modal().and_then(|(index, yolo)| {
                app.results()
                    .get(index)
                    .and_then(|s| engine.resume_command_for(s, yolo, &config.launcher))
                    .map(|command| command.argv)
            });
            app.set_indexing(if state.sync_done { None } else { Some(app.results().len()) });
            terminal.draw(|f| {
                hop::tui::view::render(
                    f,
                    &app,
                    RenderModel {
                        now,
                        columns: &columns,
                        enrichers: render_enrichers,
                        resolved: &state.enrichment.resolved,
                        query_terms: &terms,
                        preview_lines: &state.preview.lines,
                        status: &status,
                        modal_command: modal_command.as_deref(),
                        theme: *app.theme(),
                        row_style,
                    },
                )
            })?;

            let visible = hop::tui::view::visible_result_range(
                app.results().len(),
                app.selected(),
                list_rows_height as usize,
            );
            let visible_rows = app.results().get(visible).unwrap_or_default();
            state.enrichment.request_visible(service, visible_rows);

            if !app.modal_open() {
                state.process_sync(&updates, engine, &mut app)?;
            }

            if event::poll(Duration::from_millis(50))?
                && let Event::Key(key) = event::read()?
            {
                match app.handle_key(key) {
                    Action::Quit => return Ok(None),
                    Action::Search => {
                        engine.set_query(app.effective_query());
                        engine.set_sort(app.sort());
                    }
                    Action::Resume { index, yolo } => {
                        if let Some(s) = app.results().get(index).cloned() {
                            return Ok(Some((s, yolo)));
                        }
                    }
                    Action::OpenPr { index } => {
                        if let Some(s) = app.results().get(index)
                            && let Some(Some(pr)) =
                                state.enrichment.resolved.get(&(s.document_key(), "pr"))
                        {
                            hop::enrich::gh_pr::open_pr_in_browser(
                                pr,
                                s.repo_url.as_deref(),
                                &s.directory,
                            );
                        }
                    }
                    _ => {}
                }
            }

            if !app.modal_open() && engine.search_due() {
                engine.search()?;
                sync_results_into_app(engine, &mut app);
                state.preview.invalidate();
            }
        }
    })();

    ratatui::restore();
    outcome
}

struct LoopState {
    enrichment: EnrichmentState,
    preview: preview::PreviewState,
    sync_status: Option<String>,
    sync_done: bool,
}

impl LoopState {
    fn new() -> Self {
        Self {
            enrichment: EnrichmentState::default(),
            preview: preview::PreviewState::default(),
            sync_status: Some("syncing".to_string()),
            sync_done: false,
        }
    }

    fn build_status(&self, app: &App, has_selected: bool) -> StatusLine {
        StatusLine {
            sync: self.sync_status.clone(),
            pr_pending: self.enrichment.pr_pending(),
            warning: if app.preview_visible() && has_selected && self.preview.source_unavailable() {
                Some("source unavailable".to_string())
            } else {
                None
            },
        }
    }

    fn process_sync(
        &mut self,
        updates: &std::sync::mpsc::Receiver<Update>,
        engine: &mut Engine,
        app: &mut App,
    ) -> Result<()> {
        while let Ok(update) = updates.try_recv() {
            match update {
                Update::Refresh => {
                    engine.reload()?;
                    engine.search()?;
                    sync_results_into_app(engine, app);
                    self.preview.invalidate();
                }
                Update::Done { report } => {
                    self.sync_status = Some(report.status_line());
                    self.sync_done = true;
                }
            }
        }
        Ok(())
    }
}

fn sync_results_into_app(engine: &Engine, app: &mut App) {
    let results = engine.results().to_vec();
    let yolo_supported = results.iter().map(|s| engine.supports_yolo(s)).collect();
    app.set_results_with_yolo(results, yolo_supported);
}
