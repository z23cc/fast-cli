mod app;
mod events;
mod persist;
mod strings;
mod terminal;
mod theme;
mod ui;

use anyhow::Result;
use terminal::TerminalGuard;

fn main() -> Result<()> {
    let mut app = app::App::new();
    let mut term = TerminalGuard::new()?;
    events::run(&mut term.terminal, &mut app)
}
