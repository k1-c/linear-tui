//! Keeping view snapshots on disk: one file per instance, under the state
//! directory, filed by workspace — and the clock and process checks that
//! go with them. The snapshot itself is `entity::snapshot`; which one to
//! reopen or report is `usecase::instance`.

use std::path::PathBuf;

use crate::entity::Origin;

mod files;
#[cfg(test)]
mod tests;
mod time;

pub use files::*;
pub use time::*;

/// Where this process runs, started in `cwd`.
pub fn this_process(cwd: PathBuf) -> Origin {
    Origin {
        workspace: workspace_of(&cwd),
        cwd,
        pid: std::process::id(),
        herdr_pane: std::env::var("HERDR_PANE_ID")
            .ok()
            .filter(|p| !p.is_empty()),
    }
}
