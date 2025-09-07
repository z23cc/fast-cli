mod app;
mod events;
mod persist;
mod strings;
mod terminal;
mod theme;
mod ui;

use anyhow::Result;
use terminal::TerminalGuard;
use tracing_subscriber::{fmt, EnvFilter};
use std::path::PathBuf;
use std::fs;
use directories::BaseDirs;

fn main() -> Result<()> {
    init_logging();
    let mut app = app::App::new();
    let mut term = TerminalGuard::new()?;
    events::run(&mut term.terminal, &mut app)
}

fn init_logging() {
    let log_path: PathBuf = if let Some(base) = BaseDirs::new() {
        if cfg!(windows) {
            base.home_dir().join(".fast").join("log")
        } else {
            base.config_dir().join("fast").join("log")
        }
    } else {
        PathBuf::from("./log")
    };
    let _ = fs::create_dir_all(&log_path);
    let file_appender = tracing_appender::rolling::never(&log_path, "fast-tui.log");
    let (nb, _guard) = tracing_appender::non_blocking(file_appender);
    let env_filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new("info,providers=info,fast_core=info,tui=info"));
    let subscriber = fmt()
        .with_env_filter(env_filter)
        .with_writer(nb)
        .with_ansi(false)
        .finish();
    let _ = tracing::subscriber::set_global_default(subscriber);
    tracing::info!("fast-tui logging initialized at {:?}", log_path);
}
