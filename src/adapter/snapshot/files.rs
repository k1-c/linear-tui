//! Where snapshots live on disk, and which one each reader takes.
//!
//! Each running instance owns one file, `<pid>.json`, in its workspace's
//! directory under the state dir:
//!
//! ```text
//! ~/.local/state/linear-tui/workspaces/<name>-<hash>/<pid>.json
//! ```
//!
//! Two instances in one repository therefore never overwrite each other. A
//! launch restores the one closed most recently; `context` reads the one
//! still running that moved most recently.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Duration, Instant};

use anyhow::{Context, Result};
use directories::ProjectDirs;

use super::timestamp_now;
use crate::entity::snapshot::{VERSION, ViewSnapshot};
use crate::entity::{Instance, InstanceId};

/// Overrides the state directory, for tests and for tools that want to read
/// the snapshots without knowing the platform's conventions.
pub const STATE_DIR_ENV: &str = "LINEAR_TUI_STATE_DIR";

/// How long the view must rest before it is written, so holding `j` down
/// writes once rather than once per row.
const DEBOUNCE: Duration = Duration::from_millis(500);

/// How many snapshots a workspace keeps besides the live instances' own.
const KEEP: usize = 4;

/// Where linear-tui keeps state that is neither config nor cache:
/// `$XDG_STATE_HOME/linear-tui` on Linux, the local data directory elsewhere.
pub fn state_dir() -> Result<PathBuf> {
    if let Some(dir) = std::env::var_os(STATE_DIR_ENV).filter(|d| !d.is_empty()) {
        return Ok(PathBuf::from(dir));
    }
    let dirs =
        ProjectDirs::from("", "", "linear-tui").context("Failed to determine state directory")?;
    Ok(dirs
        .state_dir()
        .unwrap_or_else(|| dirs.data_local_dir())
        .to_path_buf())
}

/// The workspace `cwd` belongs to: the repository it is checked out from,
/// so every worktree of one repository shares its snapshots — or, outside
/// git, the directory itself.
pub fn workspace_of(cwd: &Path) -> PathBuf {
    let common = Command::new("git")
        .arg("-C")
        .arg(cwd)
        .args(["rev-parse", "--path-format=absolute", "--git-common-dir"])
        .output()
        .ok()
        .filter(|out| out.status.success())
        .and_then(|out| String::from_utf8(out.stdout).ok())
        .map(|text| PathBuf::from(text.trim_end()));
    let dir = match common {
        // A normal checkout keeps its repository in `<root>/.git`; a bare
        // repository is its own common dir.
        Some(dir) if dir.file_name().is_some_and(|n| n == ".git") => {
            dir.parent().map_or(dir.clone(), Path::to_path_buf)
        }
        Some(dir) => dir,
        None => cwd.to_path_buf(),
    };
    // One spelling for one directory: git writes `C:/…` on Windows where the
    // file system says `\\?\C:\…`, and either may go through a symlink.
    fs::canonicalize(&dir).unwrap_or(dir)
}

/// Whether process `pid` still exists. A reused PID reads as running; the
/// snapshot's age says how much to trust it.
pub fn is_running(pid: u32) -> bool {
    #[cfg(target_os = "linux")]
    {
        Path::new(&format!("/proc/{pid}")).exists()
    }
    #[cfg(all(unix, not(target_os = "linux")))]
    {
        Command::new("kill")
            .args(["-0", &pid.to_string()])
            .stderr(std::process::Stdio::null())
            .status()
            .is_ok_and(|s| s.success())
    }
    #[cfg(not(unix))]
    {
        let _ = pid;
        true
    }
}

/// One workspace's snapshots.
#[derive(Debug, Clone)]
pub struct Shelf {
    dir: PathBuf,
}

impl Shelf {
    pub fn new(state_dir: &Path, workspace: &Path) -> Self {
        let name: String = workspace
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default()
            .chars()
            .map(|c| {
                if c.is_alphanumeric() || c == '-' || c == '_' {
                    c
                } else {
                    '_'
                }
            })
            .take(40)
            .collect();
        let hash = fnv1a(workspace.as_os_str().as_encoded_bytes());
        Self {
            dir: state_dir
                .join("workspaces")
                .join(format!("{name}-{hash:016x}")),
        }
    }

    pub fn path_for(&self, pid: u32) -> PathBuf {
        self.dir.join(format!("{pid}.json"))
    }

    /// Every readable snapshot, newest first. A file that does not parse, or
    /// was written by a newer version, is skipped rather than guessed at.
    pub fn read_all(&self) -> Vec<(PathBuf, ViewSnapshot)> {
        let Ok(entries) = fs::read_dir(&self.dir) else {
            return Vec::new();
        };
        let mut snapshots: Vec<_> = entries
            .filter_map(|entry| {
                let path = entry.ok()?.path();
                if path.extension()? != "json" {
                    return None;
                }
                let snapshot: ViewSnapshot = serde_json::from_slice(&fs::read(&path).ok()?)
                    .inspect_err(
                        |e| tracing::warn!(path = %path.display(), %e, "unreadable snapshot"),
                    )
                    .ok()?;
                (snapshot.version <= VERSION).then_some((path, snapshot))
            })
            .collect();
        snapshots.sort_by(|a, b| b.1.updated_at.cmp(&a.1.updated_at));
        snapshots
    }

    /// Every instance with a readable record here, as its record and the
    /// system describe it. Which to reopen or report is
    /// `usecase::instance`'s to decide.
    pub fn instances(&self) -> Vec<Instance> {
        self.read_all()
            .into_iter()
            .map(|(_, view)| Instance {
                id: InstanceId {
                    workspace: view.workspace.clone(),
                    pid: view.pid,
                },
                running: is_running(view.pid),
                view,
            })
            .collect()
    }

    /// Delete all but the newest few snapshots of instances that are gone.
    pub fn prune(&self) {
        let gone = self
            .read_all()
            .into_iter()
            .filter(|(_, s)| s.closed_at.is_some() || !is_running(s.pid));
        for (path, _) in gone.skip(KEEP) {
            let _ = fs::remove_file(path);
        }
    }
}

/// Writes one instance's snapshot as the view changes, never more often than
/// [`DEBOUNCE`] and never when nothing on it changed.
#[derive(Debug)]
pub struct Recorder {
    path: PathBuf,
    last: Option<ViewSnapshot>,
    due: Option<Instant>,
}

impl Recorder {
    pub fn new(shelf: &Shelf, pid: u32) -> Self {
        Self {
            path: shelf.path_for(pid),
            last: None,
            due: None,
        }
    }

    /// Something happened that may have changed the view.
    pub fn touch(&mut self, now: Instant) {
        self.due.get_or_insert(now + DEBOUNCE);
    }

    /// Whether the view has rested long enough to be written.
    pub fn is_due(&self, now: Instant) -> bool {
        self.due.is_some_and(|due| now >= due)
    }

    /// Write `snapshot` unless it shows what the file already does.
    pub fn record(&mut self, snapshot: ViewSnapshot) {
        self.due = None;
        if self
            .last
            .as_ref()
            .is_some_and(|last| last.same_view(&snapshot))
        {
            return;
        }
        self.write(snapshot);
    }

    /// The instance is quitting: write the view one last time, marked closed.
    pub fn close(&mut self, mut snapshot: ViewSnapshot) {
        snapshot.closed_at = Some(timestamp_now());
        self.write(snapshot);
    }

    fn write(&mut self, snapshot: ViewSnapshot) {
        // A snapshot is a convenience: failing to write one is logged, never
        // allowed to take the TUI down.
        let result = serde_json::to_vec_pretty(&snapshot)
            .map_err(anyhow::Error::from)
            .and_then(|bytes| crate::adapter::private_file::write(&self.path, &bytes));
        match result {
            Ok(()) => self.last = Some(snapshot),
            Err(e) => {
                tracing::warn!(path = %self.path.display(), "failed to write snapshot: {e:#}")
            }
        }
    }
}

/// 64-bit FNV-1a: stable across Rust versions and platforms, unlike the
/// standard library's hasher, so a directory name survives an upgrade.
fn fnv1a(bytes: &[u8]) -> u64 {
    bytes.iter().fold(0xcbf2_9ce4_8422_2325, |hash, byte| {
        (hash ^ u64::from(*byte)).wrapping_mul(0x0100_0000_01b3)
    })
}
