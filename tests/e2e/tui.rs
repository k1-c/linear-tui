//! A linear-tui for one scenario: the real binary, headless, signed in to a
//! test workspace with an API key, in a directory of its own — worked
//! through `linear-tui tui …` exactly as an agent works one.

use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::{Duration, Instant};

use serde_json::Value;

use super::linear::Account;

/// How long an instance may take to come up and load its first page.
const STARTUP: Duration = Duration::from_secs(40);

pub struct Tui {
    /// The running process, until it quits.
    child: Option<Child>,
    /// Everything the instance owns: its config, its state, its workspace.
    dir: PathBuf,
}

/// The screen as `linear-tui tui … --json` reports it.
#[derive(Debug, Clone)]
pub struct Screen(pub Value);

impl Tui {
    /// Launch on `account`'s workspace. `config` is added to the
    /// `config.toml` it runs with (the API key is always there).
    pub fn start(account: &Account, config: &str) -> Self {
        static NEXT: AtomicU32 = AtomicU32::new(0);
        let dir = std::env::temp_dir().join(format!(
            "linear-tui-e2e-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        let _ = std::fs::remove_dir_all(&dir);
        for sub in ["config/linear-tui", "state", "workspace"] {
            std::fs::create_dir_all(dir.join(sub)).expect("the temp dir is writable");
        }
        std::fs::write(
            dir.join("config/linear-tui/config.toml"),
            format!("[auth]\napi_key = {:?}\n\n{config}\n", account.api_key),
        )
        .expect("the temp dir is writable");
        let mut tui = Self { child: None, dir };
        tui.launch();
        tui
    }

    /// Start the process in the instance's directory and wait for it.
    fn launch(&mut self) {
        let child = Command::new(bin())
            .args(["--headless", "--size", "120x40"])
            .current_dir(self.dir.join("workspace"))
            .envs(env(&self.dir))
            .stdout(Stdio::null())
            .stderr(Stdio::inherit())
            .spawn()
            .expect("linear-tui starts");
        self.child = Some(child);
        self.wait_until_ready();
    }

    /// Wait until the instance answers and has loaded its first page.
    fn wait_until_ready(&self) {
        let deadline = Instant::now() + STARTUP;
        loop {
            if let Ok(screen) = self.try_tui(&["screen"])
                && !screen.loading()
                && !screen.text().contains("Loading")
            {
                return;
            }
            assert!(
                Instant::now() < deadline,
                "linear-tui did not come up within {STARTUP:?}"
            );
            std::thread::sleep(Duration::from_millis(250));
        }
    }

    fn try_tui(&self, args: &[&str]) -> Result<Screen, String> {
        let mut all = vec!["tui"];
        all.extend_from_slice(args);
        all.push("--json");
        let text = self.cli(&all)?;
        serde_json::from_str(&text)
            .map(Screen)
            .map_err(|e| format!("{e}: {text}"))
    }

    fn tui(&self, args: &[&str]) -> Screen {
        self.try_tui(args)
            .unwrap_or_else(|e| panic!("linear-tui tui {}: {e}", args.join(" ")))
    }

    pub fn screen(&self) -> Screen {
        self.tui(&["screen"])
    }

    /// Press keys, in linear-tui's notation (`g m`, `<Enter>`, `<C-k>`).
    pub fn press(&self, keys: &str) -> Screen {
        self.tui(&["press", keys])
    }

    pub fn type_text(&self, text: &str) -> Screen {
        self.tui(&["type", text])
    }

    /// Run a command palette entry by its title.
    pub fn run(&self, title: &str) -> Screen {
        self.tui(&["run", title])
    }

    /// `tui run` that is expected to fail; returns the error.
    pub fn run_fails(&self, title: &str) -> String {
        self.try_tui(&["run", title])
            .expect_err("the command should fail")
    }

    pub fn open(&self, issue: &str) -> Screen {
        self.tui(&["open", issue])
    }

    /// Any linear-tui command, as an agent in this workspace runs it.
    pub fn cli(&self, args: &[&str]) -> Result<String, String> {
        let out = Command::new(bin())
            .args(args)
            .current_dir(self.dir.join("workspace"))
            .envs(env(&self.dir))
            .output()
            .map_err(|e| e.to_string())?;
        if out.status.success() {
            Ok(String::from_utf8_lossy(&out.stdout).into_owned())
        } else {
            Err(String::from_utf8_lossy(&out.stderr).trim().to_string())
        }
    }

    /// An issue as Linear holds it now (`linear-tui issue show --json`).
    pub fn issue(&self, id: &str) -> Value {
        let text = self
            .cli(&["issue", "show", id, "--json"])
            .unwrap_or_else(|e| panic!("issue show {id}: {e}"));
        serde_json::from_str(&text).expect("issue show prints JSON")
    }

    /// Quit, and launch again in the same place.
    pub fn relaunch(&mut self) {
        self.quit();
        self.launch();
    }

    /// Quit and wait for the process to end.
    pub fn quit(&mut self) {
        let Some(mut child) = self.child.take() else {
            return;
        };
        let _ = self.cli(&["tui", "quit"]);
        let deadline = Instant::now() + Duration::from_secs(10);
        while Instant::now() < deadline {
            if let Ok(Some(_)) = child.try_wait() {
                return;
            }
            std::thread::sleep(Duration::from_millis(100));
        }
        let _ = child.kill();
    }
}

impl Drop for Tui {
    fn drop(&mut self) {
        self.quit();
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

/// Try `attempt` until it gives something, for up to `SEARCH_INDEX_LAG`:
/// Linear's search finds a new issue only once it has indexed it, a few
/// seconds after it was filed.
pub fn eventually<T>(what: &str, mut attempt: impl FnMut() -> Option<T>) -> T {
    let deadline = Instant::now() + SEARCH_INDEX_LAG;
    loop {
        if let Some(found) = attempt() {
            return found;
        }
        assert!(
            Instant::now() < deadline,
            "{what}: not within {SEARCH_INDEX_LAG:?}"
        );
        std::thread::sleep(Duration::from_secs(2));
    }
}

/// How long Linear may take to index a new issue for search.
const SEARCH_INDEX_LAG: Duration = Duration::from_secs(60);

/// The binary under test.
fn bin() -> &'static str {
    env!("CARGO_BIN_EXE_linear-tui")
}

/// The environment an instance and its commands share: its own config and
/// state, and nothing of herdr's.
fn env(dir: &Path) -> Vec<(&'static str, PathBuf)> {
    vec![
        ("XDG_CONFIG_HOME", dir.join("config")),
        ("LINEAR_TUI_STATE_DIR", dir.join("state")),
        ("HERDR_BIN_PATH", PathBuf::new()),
        ("HERDR_PANE_ID", PathBuf::new()),
    ]
}

impl Screen {
    /// The frame, one line per row.
    pub fn text(&self) -> String {
        self.0["lines"]
            .as_array()
            .map(|lines| {
                lines
                    .iter()
                    .filter_map(Value::as_str)
                    .collect::<Vec<_>>()
                    .join("\n")
            })
            .unwrap_or_default()
    }

    fn field(&self, name: &str) -> Option<&str> {
        self.0[name].as_str()
    }

    pub fn showing(&self) -> &str {
        self.field("showing").unwrap_or_default()
    }

    pub fn focus(&self) -> &str {
        self.field("focus").unwrap_or_default()
    }

    pub fn overlay(&self) -> Option<&str> {
        self.field("overlay")
    }

    pub fn issue(&self) -> Option<&str> {
        self.field("issue")
    }

    pub fn status(&self) -> Option<&str> {
        self.field("status")
    }

    pub fn loading(&self) -> bool {
        self.0["loading"] == Value::Bool(true)
    }

    /// What a headless instance held back from the desktop.
    pub fn held(&self) -> Vec<String> {
        self.0["held"]
            .as_array()
            .map(|h| {
                h.iter()
                    .filter_map(Value::as_str)
                    .map(String::from)
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Fail, showing the screen, unless `text` is on it.
    #[track_caller]
    pub fn expect(&self, text: &str) -> &Self {
        assert!(
            self.text().contains(text),
            "expected {text:?} on screen:\n{}",
            self.text()
        );
        self
    }

    /// Fail, showing the screen, if `text` is on it.
    #[track_caller]
    pub fn expect_not(&self, text: &str) -> &Self {
        assert!(
            !self.text().contains(text),
            "expected no {text:?} on screen:\n{}",
            self.text()
        );
        self
    }
}
