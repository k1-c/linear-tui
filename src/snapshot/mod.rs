//! Keeping view snapshots on disk: one file per instance, under the state
//! directory, filed by workspace — and the clock and process checks that
//! go with them. The snapshot itself is `entity::snapshot`; which one to
//! reopen or report is `usecase::instance`.

use std::path::PathBuf;

mod files;
#[cfg(test)]
mod tests;
mod time;

pub use files::*;
pub use time::*;

/// Who is writing a snapshot, and from where.
#[derive(Debug, Clone)]
pub struct Origin {
    pub workspace: PathBuf,
    pub cwd: PathBuf,
    pub pid: u32,
    pub herdr_pane: Option<String>,
}

impl Origin {
    /// This process, started in `cwd`.
    pub fn current(cwd: PathBuf) -> Self {
        Self {
            workspace: workspace_of(&cwd),
            cwd,
            pid: std::process::id(),
            herdr_pane: std::env::var("HERDR_PANE_ID")
                .ok()
                .filter(|p| !p.is_empty()),
        }
    }
}
