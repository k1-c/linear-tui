//! The hand-off to herdr, for the few actions that exist only inside it.
//!
//! linear-tui does not drive herdr. It leaves a request in its own state
//! directory and asks the plugin to pick it up with
//! `herdr plugin action invoke k1-c.linear-tui.deliver`; the plugin's scripts
//! do the herdr work. Outside herdr (`HERDR_BIN_PATH` unset) none of this is
//! reachable. The file formats are in `docs/herdr.md`.

use std::path::PathBuf;
use std::time::{Duration, Instant, SystemTime};

use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};

use crate::core::entity::{AgentLink, Handoff};
use crate::infra::disk::snapshot;

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

/// The plugin's list of herdr agents: `$STATE/herdr/agents.json`.
pub fn agents_file() -> Result<PathBuf> {
    Ok(snapshot::state_dir()?.join("herdr").join("agents.json"))
}

#[derive(Debug, Deserialize)]
struct AgentsFile {
    version: u32,
    #[serde(default)]
    agents: Vec<AgentLink>,
}

/// Parse the plugin's agents file; one from a newer plugin reads as empty.
pub fn parse_agents(text: &str) -> Result<Vec<AgentLink>> {
    let file: AgentsFile = serde_json::from_str(text)?;
    Ok(if file.version == 1 {
        file.agents
    } else {
        Vec::new()
    })
}

/// Watches the agents file, reading it again only when it changes.
#[derive(Debug)]
pub struct AgentWatch {
    path: PathBuf,
    modified: Option<SystemTime>,
    next: Instant,
}

impl AgentWatch {
    /// How often the file is looked at.
    const EVERY: Duration = Duration::from_secs(1);

    pub fn new() -> Option<Self> {
        Some(Self {
            path: agents_file().ok()?,
            modified: None,
            next: Instant::now(),
        })
    }

    /// The agents, when the file changed since the last look.
    pub fn poll(&mut self, now: Instant) -> Option<Vec<AgentLink>> {
        if now < self.next {
            return None;
        }
        self.next = now + Self::EVERY;
        let modified = std::fs::metadata(&self.path)
            .and_then(|m| m.modified())
            .ok();
        if modified == self.modified {
            return None;
        }
        self.modified = modified;
        if modified.is_none() {
            return Some(Vec::new());
        }
        let text = std::fs::read_to_string(&self.path).ok()?;
        parse_agents(&text)
            .inspect_err(|e| tracing::warn!("unreadable {}: {e:#}", self.path.display()))
            .ok()
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_agents_file_parses_and_tolerates_new_states() {
        let agents = parse_agents(
            r#"{"version":1,"updated_at":"2026-09-25T00:00:00Z","agents":[
                {"pane":"w2:p1","agent":"claude","status":"working","issue":"ENG-42"},
                {"pane":"w3:p1","agent":"codex","status":"thinking"}]}"#,
        )
        .unwrap();
        assert_eq!(agents[0].issue.as_deref(), Some("ENG-42"));
        assert_eq!(agents[1].status, crate::core::entity::AgentStatus::Unknown);
        assert!(
            parse_agents(r#"{"version":2,"agents":[]}"#)
                .unwrap()
                .is_empty()
        );
    }
}
