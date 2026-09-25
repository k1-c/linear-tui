//! A linear-tui instance: one run of the TUI in one repository, and the view
//! it last recorded. An entity: it is the same instance from launch to quit
//! while what it shows changes.

use std::path::PathBuf;

use super::snapshot::ViewSnapshot;

/// Which instance: the repository it runs in (every worktree of one
/// repository shares it) and its process.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct InstanceId {
    pub workspace: PathBuf,
    pub pid: u32,
}

/// Where an instance runs: what it records its view under.
#[derive(Debug, Clone)]
pub struct Origin {
    /// The repository root, shared by all of its worktrees — or the
    /// directory itself outside git.
    pub workspace: PathBuf,
    /// Where it was started.
    pub cwd: PathBuf,
    pub pid: u32,
    /// The herdr pane it runs in, when it runs inside herdr.
    pub herdr_pane: Option<String>,
}

impl Origin {
    /// The instance it is.
    pub fn id(&self) -> InstanceId {
        InstanceId {
            workspace: self.workspace.clone(),
            pid: self.pid,
        }
    }
}

/// An instance as its record on disk and the operating system describe it.
#[derive(Debug, Clone, PartialEq)]
pub struct Instance {
    pub id: InstanceId,
    /// What it showed when it last recorded its view.
    pub view: ViewSnapshot,
    /// Whether its process is still there. A reused process id reads as
    /// running; the record's age says how far to trust it.
    pub running: bool,
}

impl Instance {
    /// Whether it quit normally. One that did not and whose process is gone
    /// was ended by a crash or a closed terminal.
    pub fn closed(&self) -> bool {
        self.view.closed_at.is_some()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// An instance is known by its workspace and its process, wherever in
    /// the workspace it was started.
    #[test]
    fn an_instance_is_its_workspace_and_process() {
        let origin = Origin {
            workspace: PathBuf::from("/repo"),
            cwd: PathBuf::from("/repo/sub"),
            pid: 42,
            herdr_pane: Some("p".into()),
        };
        assert_eq!(
            origin.id(),
            InstanceId {
                workspace: PathBuf::from("/repo"),
                pid: 42,
            }
        );
    }
}
