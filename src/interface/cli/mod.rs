//! The `linear-tui <command>` subcommands, which run without the TUI.
//!
//! Besides signing in, these are how an agent reads what the user is looking
//! at (`context`) and acts on Linear itself (`issue …`), with the same
//! credentials and the same request code as the TUI. Output is meant for
//! agents first: plain, stable Markdown by default, JSON with `--json`, and
//! errors on stderr with a non-zero exit. The contract is `docs/cli.md`.
//!
//! A subcommand never reaches for the infra: what it needs from outside —
//! Linear, the recorded views, the clock — comes in through a [`Host`],
//! which `crate::commands` assembles. `linear-tui auth …` is handled there
//! too: signing in sets up the infra itself.

use std::path::{Path, PathBuf};

use anyhow::Result;

use crate::core::entity::{Instance, Organization};

mod args;
mod context;
mod headless;
mod issue;
mod paths;
mod tui;

pub use headless::Linear;
pub use issue::issue_key;

/// What the subcommands need from the world outside linear-tui.
pub trait Host {
    type Linear: Linear;

    /// Linear, signed in the way `linear-tui auth` set up. Never prompts: a
    /// subcommand is often run by an agent with nobody at the keyboard.
    async fn linear(&self) -> Result<Self::Linear>;

    /// The repository `cwd` is in: the workspace linear-tui files views under.
    fn workspace_of(&self, cwd: &Path) -> PathBuf;

    /// The linear-tui instances recorded for a workspace.
    fn instances(&self, workspace: &Path) -> Result<Vec<Instance>>;

    /// Where an instance records its view; its control file sits beside it.
    fn snapshot_file(&self, workspace: &Path, pid: u32) -> Result<PathBuf>;

    /// Where the view snapshots live.
    fn state_dir(&self) -> Result<PathBuf>;

    /// The workspace `linear-tui issue …` acts in, read without the network;
    /// `None` when it cannot be told (an API key, or nothing stored).
    fn acting_organization(&self) -> Option<Organization>;

    /// Seconds since the Unix epoch, now.
    fn now(&self) -> u64;
}

pub const USAGE: &str = "\
linear-tui — a terminal UI for Linear

Usage:
  linear-tui                     Open the TUI (sets up credentials on first run)
  linear-tui open <ID>           Open the TUI on an issue (ENG-123 or its URL)
  linear-tui --headless [--size <W>x<H>]
                                 Run the TUI with no terminal, for agents to drive
  linear-tui auth login          Sign in to a workspace through the browser
  linear-tui auth list           List the signed-in workspaces
  linear-tui auth switch <workspace>
                                 Use another signed-in workspace (by URL key or name)
  linear-tui auth status         Show which credentials are in use
  linear-tui auth token <key>    Sign in with a personal API key
  linear-tui auth logout [<workspace>] [--all]
                                 Forget a workspace's token (--all: every token and the API key)
  linear-tui auth set-oauth <client-id> [client-secret]
                                 Authorize against your own Linear application

For agents and scripts (Markdown by default, --json for JSON):
  linear-tui context [--json] [--workspace <path>]
                                 What linear-tui is showing in this repository
  linear-tui issue show <ID> [--json]
                                 An issue with its description and comments
  linear-tui issue create --team <key> --title <text> [--description <text>]
                          [--priority <urgent|high|medium|low|none>] [--json]
  linear-tui issue comment <ID> <body> [--json]
  linear-tui issue status <ID> <state> [--json]
  linear-tui tui screen [--json]
                                 The screen of the linear-tui running here
  linear-tui tui press <keys>    Press keys in it (`g m`, `<Enter>`, `<C-k>`)
  linear-tui tui type <text>     Type into the field that has focus
  linear-tui tui run <title>     Run a command palette entry by its title
  linear-tui tui open <ID>       Open an issue in it
  linear-tui tui quit            Quit it
  linear-tui paths [--json]      Where the config and the state (view snapshots) live

<ID> is an identifier (ENG-123) or an issue URL. A <body> or <text> of `-`
is read from stdin.
";

/// Run the subcommand `args` names — all but `auth`, which
/// `crate::commands` handles.
pub async fn handle_subcommand(args: &[String], host: &impl Host) -> Result<()> {
    match args[0].as_str() {
        "context" => context::run(&args[1..], host),
        "tui" => tui::run(&args[1..], host).await,
        "issue" => issue::run(&args[1..], host).await,
        "paths" => paths::run(&args[1..], host),
        "help" | "--help" | "-h" => {
            print!("{USAGE}");
            Ok(())
        }
        "--version" | "-V" => {
            println!("linear-tui {}", env!("CARGO_PKG_VERSION"));
            Ok(())
        }
        other => {
            eprint!("{USAGE}");
            anyhow::bail!("unknown command: {other}")
        }
    }
}
