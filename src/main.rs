use anyhow::Result;
use clap::Parser;
use hop::adapters;
use hop::cli::Cli;
use hop::config::Config;
use hop::engine::{Engine, Update};
use hop::resume;
use hop::tui::{view, Action, App};
use ratatui::crossterm::event::{self, Event};
use std::time::Duration;

fn index_dir() -> std::path::PathBuf {
    directories::ProjectDirs::from("dev", "hop", "hop")
        .map(|d| d.cache_dir().join("index"))
        .unwrap_or_else(|| std::path::PathBuf::from(".hop-index"))
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

    // resume request escapes the TUI loop so we exec AFTER restoring the terminal
    let pending = run_tui(&mut engine, updates)?;

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
) -> Result<Option<(hop::core::Session, bool)>> {
    let mut terminal = ratatui::init();
    let mut app = App::new();
    app.set_query(engine.query().to_string());
    sync_results_into_app(engine, &mut app);

    let outcome = (|| -> Result<Option<(hop::core::Session, bool)>> {
        loop {
            let now = jiff::Timestamp::now().as_second();
            terminal.draw(|f| view::render(f, &app, now))?;

            // fold in any streamed sessions
            while let Ok(update) = updates.try_recv() {
                if let Update::Refresh = update {
                    engine.reload()?;
                    engine.search()?;
                    sync_results_into_app(engine, &mut app);
                }
            }

            if event::poll(Duration::from_millis(50))? {
                if let Event::Key(key) = event::read()? {
                    match app.handle_key(key) {
                        Action::Quit => return Ok(None),
                        Action::Search => {
                            engine.set_query(app.query().to_string());
                        }
                        Action::Resume { index, yolo } => {
                            if let Some(s) = engine.results().get(index).cloned() {
                                return Ok(Some((s, yolo)));
                            }
                        }
                        Action::None => {}
                    }
                }
            }

            // run the debounced search once the keystroke stream goes quiet
            if engine.search_due() {
                engine.search()?;
                sync_results_into_app(engine, &mut app);
            }
        }
    })();

    ratatui::restore();
    outcome
}

fn sync_results_into_app(engine: &Engine, app: &mut App) {
    app.set_results(engine.results().to_vec());
}
