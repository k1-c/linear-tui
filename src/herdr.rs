//! The hand-off to herdr, for the few actions that exist only inside it.
//!
//! linear-tui does not drive herdr. It leaves a request in its own state
//! directory and asks the plugin to pick it up with
//! `herdr plugin action invoke k1-c.linear-tui.deliver`; the plugin's scripts
//! do the herdr work. Outside herdr (`HERDR_BIN_PATH` unset) none of this is
//! reachable. The file formats are in `docs/herdr.md`.

use std::path::PathBuf;

use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};

use crate::snapshot;

/// The plugin action that delivers what is in the outbox.
pub const DELIVER: &str = "k1-c.linear-tui.deliver";

/// The running herdr, when linear-tui is inside one.
pub fn bin() -> Option<PathBuf> {
    std::env::var_os("HERDR_BIN_PATH")
        .filter(|p| !p.is_empty())
        .map(PathBuf::from)
}

pub fn available() -> bool {
    bin().is_some()
}

/// Where requests for the plugin wait: `$STATE/herdr/outbox/`.
pub fn outbox_dir() -> Result<PathBuf> {
    Ok(snapshot::state_dir()?.join("herdr").join("outbox"))
}

/// What linear-tui asks the plugin to do.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Handoff {
    /// Send the user's notes to the agent working next to linear-tui.
    Prompt {
        /// The whole prompt, ready to send as it is.
        text: String,
        /// Its parts, for a prompt template: the notes as a Markdown list,
        /// where the user is, and how to read more.
        notes: String,
        view: String,
        hint: String,
    },
}

impl Handoff {
    pub fn done(&self) -> &'static str {
        match self {
            Self::Prompt { .. } => "Notes handed to herdr for your agent",
        }
    }
}

/// A request as it is written for the plugin.
#[derive(Debug, Serialize)]
struct Envelope<'a> {
    version: u32,
    /// The pane, workspace, and directory it came from, so the plugin can
    /// pick the agent next to it.
    from: From,
    #[serde(flatten)]
    handoff: &'a Handoff,
}

#[derive(Debug, Serialize)]
struct From {
    pane: Option<String>,
    workspace: Option<String>,
    cwd: Option<PathBuf>,
}

/// Leave `handoff` for the plugin and ask it to deliver.
pub async fn deliver(handoff: &Handoff) -> Result<()> {
    let bin = bin().context("not running inside herdr")?;
    let env = |name: &str| std::env::var(name).ok().filter(|v| !v.is_empty());
    let envelope = Envelope {
        version: 1,
        from: From {
            pane: env("HERDR_PANE_ID"),
            workspace: env("HERDR_WORKSPACE_ID"),
            cwd: std::env::current_dir().ok(),
        },
        handoff,
    };
    let name = format!(
        "{}-{}.json",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.as_millis()),
        std::process::id()
    );
    let path = outbox_dir()?.join(name);
    crate::private_file::write(&path, &serde_json::to_vec_pretty(&envelope)?)?;

    let out = tokio::process::Command::new(&bin)
        .args(["plugin", "action", "invoke", DELIVER])
        .output()
        .await
        .with_context(|| format!("could not run {}", bin.display()))?;
    if !out.status.success() {
        let _ = std::fs::remove_file(&path);
        let stderr = String::from_utf8_lossy(&out.stderr);
        let stdout = String::from_utf8_lossy(&out.stdout);
        let reason = if stderr.trim().is_empty() {
            stdout
        } else {
            stderr
        };
        bail!("the linear-tui herdr plugin did not run: {}", reason.trim());
    }
    Ok(())
}
