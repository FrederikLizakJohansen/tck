mod anim;
mod app;
mod cli;
mod input;
mod model;
mod storage;
mod theme;
mod ui;

use std::{io, path::PathBuf};

use anyhow::{Context, Result};
use clap::Parser;
use crossterm::{
    event::{DisableBracketedPaste, DisableMouseCapture, EnableBracketedPaste, EnableMouseCapture},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{backend::CrosstermBackend, Terminal};

use crate::{app::App, cli::{Cli, Commands}, model::Project};

fn main() -> Result<()> {
    let cli = Cli::parse();
    let (project, path) = match cli.command {
        Commands::New { project_name } => {
            let project = Project::new(&project_name);
            let path = storage::default_project_path(&project_name);
            storage::save_project(&path, &project)
                .with_context(|| format!("failed to initialize {}", path.display()))?;
            (project, path)
        }
        Commands::Open { path } => {
            let project = storage::load_project(&path)?;
            (project, path)
        }
    };

    run_tui(App::new(project, canonicalize_project_path(path)?))
}

fn canonicalize_project_path(path: PathBuf) -> Result<PathBuf> {
    if path.exists() {
        std::fs::canonicalize(&path).with_context(|| format!("failed to resolve {}", path.display()))
    } else {
        Ok(path)
    }
}

fn run_tui(mut app: App) -> Result<()> {
    enable_raw_mode().context("failed to enable raw mode")?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableBracketedPaste, EnableMouseCapture)
        .context("failed to enter alternate screen")?;

    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend).context("failed to create terminal")?;
    terminal.clear().ok();

    let result = run_event_loop(&mut terminal, &mut app);

    disable_raw_mode().ok();
    execute!(
        terminal.backend_mut(),
        DisableBracketedPaste,
        LeaveAlternateScreen,
        DisableMouseCapture
    )
    .ok();
    terminal.show_cursor().ok();

    result
}

fn run_event_loop(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    app: &mut App,
) -> Result<()> {
    loop {
        terminal.draw(|frame| ui::draw(frame, &mut *app))?;
        if app.should_quit {
            break;
        }

        if let Some(event) = input::next_event(App::tick_rate())? {
            app.on_event(event)?;
        }
        app.tick();
    }

    Ok(())
}
