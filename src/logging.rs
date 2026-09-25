use tracing_appender::non_blocking::WorkerGuard;
use tracing_subscriber::{EnvFilter, fmt, prelude::*};

/// Initialize file-based logging.
///
/// Logs are written to `~/.config/linear-tui/debug.log`.
/// The default level is `info`; override with `RUST_LOG` env var.
///
/// Returns a `WorkerGuard` that **must** be held for the lifetime of the program
/// to ensure all buffered logs are flushed on exit.
pub fn init() -> WorkerGuard {
    let config_dir = directories::ProjectDirs::from("", "", "linear-tui")
        .map(|d| d.config_dir().to_path_buf())
        .unwrap_or_else(|| std::env::temp_dir().join("linear-tui"));

    std::fs::create_dir_all(&config_dir).ok();

    // Request logs name issues and people in the workspace, so the file is
    // owner-only like the credentials beside it. Logging is best-effort: if
    // the file cannot be opened, the app runs without it.
    let (non_blocking, guard) =
        match crate::private_file::open_append(&config_dir.join("debug.log")) {
            Ok(file) => tracing_appender::non_blocking(file),
            Err(_) => tracing_appender::non_blocking(std::io::sink()),
        };

    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));

    tracing_subscriber::registry()
        .with(filter)
        .with(fmt::layer().with_writer(non_blocking).with_ansi(false))
        .init();

    guard
}
