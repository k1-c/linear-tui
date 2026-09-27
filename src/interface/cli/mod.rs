//! The `linear-tui <command>` subcommands, which run without the TUI.
//!
//! Besides signing in, these are how an agent reads what the user is looking
//! at (`context`) and does, without a TUI open, whatever a person can do in
//! one: list, read, change, and comment on issues (`issue …`), and read and
//! change teams, projects and their milestones, cycles, saved views, and
//! favorites. They use the same credentials and the same request code as
//! the TUI. Output is meant for agents first: plain, stable Markdown by
//! default, JSON with `--json`, and errors on stderr with a non-zero exit.
//! The contract is `docs/cli.md`.
//!
//! A subcommand never reaches for the infra: what it needs from outside —
//! Linear, the recorded views, the clock — comes in through a [`Host`],
//! which `crate::commands` assembles. `linear-tui auth …` is handled there
//! too: signing in sets up the infra itself.
//!
//! Every use case can be carried out from here: each command names the
//! use cases it serves on a `Covers:` line, and `tests/usecase_spec.rs`
//! fails for one that none covers and that is not listed below. These are
//! what the TUI does on screen, with nothing left for a command to do:
//!
//! Not from the command line:
//! - `issue::quick_search`, `issue::take_search_results` — the palette's and
//!   a list's search as you type; `issue search` is the command line's
//! - `issue::ensure_thread` — keeps the open issue's thread on screen;
//!   `issue show` reads it whole every time
//! - `issue::created` — puts a new issue at the top of the list on screen;
//!   `issue create` prints it
//! - `issue::change_refused` — takes back on screen a change Linear refused;
//!   a command reports the refusal
//! - `team::pick_initial`, `team::switch` — which team the TUI shows; a
//!   command names its team with `--team`
//! - `team::context_failed` — lets the TUI ask again; a command reports the
//!   failure
//! - `notes::add`, `notes::prompt`, `notes::send`, `notes::discard`,
//!   `notes::salvage` — notes are what a person writes for the agent in the
//!   TUI; the command line is the agent's side of them
//! - `agent::agent_for`, `agent::jump` — herdr panes on the person's desktop
//! - `instance::to_reopen` — which view a TUI launch reopens
//! - `workspace::leave`, `workspace::view_on_return` — which page the TUI
//!   comes back to after a workspace switch

use std::path::{Path, PathBuf};

use anyhow::Result;

use crate::core::entity::{Instance, Organization};

mod args;
mod browse;
mod context;
mod headless;
mod issue;
mod paths;
mod project;
mod resolve;
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
  linear-tui issue list (--team <key> [--preset active|backlog|all] | --mine
                        | --view <name> | --project <name> | --cycle <name|current> --team <key>)
                        [--status <name>] [--priority <level>] [--query <text>] [--limit <n>]
                                 A list of issues, as the TUI shows it
  linear-tui issue search <text> [--team <key>]
                                 Search all of Linear, or one team
  linear-tui issue show <ID> [--json]
                                 An issue with its description and comments
  linear-tui issue create --team <key> --title <text> [--description <text>]
                          [--priority <urgent|high|medium|low|none>]
                          [--assignee <me|name|email>] [--estimate <n>]
                          [--label <name>]... [--project <name>] [--milestone <name>]
                          [--cycle <name|current>] [--parent <ID>] [--json]
  linear-tui issue update <ID> [--title <text>] [--description <text>]
                          [--priority <level>] [--assignee <me|name|email|none>]
                          [--estimate <n|none>] [--label <name>]... [--unlabel <name>]...
                          [--project <name|none>] [--milestone <name|none>]
                          [--cycle <name|current|none>] [--parent <ID|none>] [--json]
  linear-tui issue comment <ID> <body> [--reply-to <comment-id>] [--json]
  linear-tui issue comment edit <ID> <comment-id> <body> [--json]
  linear-tui issue comment delete <ID> <comment-id> [--json]
  linear-tui issue status <ID> <state> [--json]
  linear-tui project list (--team <key> | --view <name>)
  linear-tui project show <project>
  linear-tui project create --team <key>... --name <text> [--description <text>]
                            [--content <markdown>] [--lead <who>] [--status <name>]
                            [--priority <level>] [--start <date>] [--target <date>]
  linear-tui project update <project> [--name] [--description] [--content]
                            [--lead|none] [--status] [--priority] [--start|none]
                            [--target|none]
  linear-tui project delete <project>
  linear-tui milestone list <project>
  linear-tui milestone create <project> --name <text> [--description] [--target <date>]
  linear-tui milestone update <project> <milestone> [--name] [--description] [--target|none]
  linear-tui milestone delete <project> <milestone>
  linear-tui team list | team show <key>
                                 Teams; a team's states, members, and labels
  linear-tui cycle list --team <key>
  linear-tui view list [--team <key>]
  linear-tui favorite list       Each of these takes --json too
  linear-tui tui screen [--json]
                                 The screen of the linear-tui running here
  linear-tui tui press <keys>    Press keys in it (`g m`, `<Enter>`, `<C-k>`)
  linear-tui tui type <text>     Type into the field that has focus
  linear-tui tui run <title>     Run a command palette entry by its title
  linear-tui tui open <ID>       Open an issue in it
  linear-tui tui quit            Quit it
  linear-tui paths [--json]      Where the config and the state (view snapshots) live

<ID> is an identifier (ENG-123) or an issue URL. A <body> or <text> of `-`
is read from stdin, by one argument at most.
";

/// Run the subcommand `args` names — all but `auth`, which
/// `crate::commands` handles.
pub async fn handle_subcommand(args: &[String], host: &impl Host) -> Result<()> {
    match args[0].as_str() {
        "context" => context::run(&args[1..], host),
        "tui" => tui::run(&args[1..], host).await,
        "issue" => issue::run(&args[1..], host).await,
        "project" | "milestone" | "team" | "cycle" | "view" | "favorite" => {
            let linear = host.linear().await?;
            let rest = &args[1..];
            let out = match args[0].as_str() {
                "project" => project::run(rest, &linear).await?,
                "milestone" => project::run_milestone(rest, &linear).await?,
                "team" => browse::team(rest, &linear).await?,
                "cycle" => browse::cycles(rest, &linear, host.now()).await?,
                "view" => browse::views(rest, &linear).await?,
                _ => browse::favorites(rest, &linear).await?,
            };
            println!("{}", out.trim_end());
            Ok(())
        }
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
