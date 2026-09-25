//! The `linear-tui <command>` subcommands, which run without the TUI.
//!
//! Besides signing in, these are how an agent reads what the user is looking
//! at (`context`) and acts on Linear itself (`issue …`), with the same
//! credentials and the same request code as the TUI. Output is meant for
//! agents first: plain, stable Markdown by default, JSON with `--json`, and
//! errors on stderr with a non-zero exit. The contract is `docs/cli.md`.

use anyhow::Result;

mod args;
mod auth;
mod context;
mod headless;
mod issue;
mod paths;
mod tui;

pub use issue::issue_key;

pub const USAGE: &str = "\
linear-tui — a terminal UI for Linear

Usage:
  linear-tui                     Open the TUI (sets up credentials on first run)
  linear-tui open <ID>           Open the TUI on an issue (ENG-123 or its URL)
  linear-tui --headless [--size <W>x<H>]
                                 Run the TUI with no terminal, for agents to drive
  linear-tui auth login          Sign in through the browser
  linear-tui auth status         Show which credentials are in use
  linear-tui auth token <key>    Sign in with a personal API key
  linear-tui auth logout [--all] Forget the stored token (--all: the API key too)
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

pub async fn handle_subcommand(args: &[String]) -> Result<()> {
    match args[0].as_str() {
        "auth" => auth::run(&args[1..]).await,
        "context" => context::run(&args[1..]),
        "tui" => tui::run(&args[1..]).await,
        "issue" => issue::run(&args[1..]).await,
        "paths" => paths::run(&args[1..]),
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
