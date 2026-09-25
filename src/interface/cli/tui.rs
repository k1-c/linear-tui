//! `linear-tui tui screen / press / type / run / open / quit`: work the
//! linear-tui running in this repository the way a person does, through
//! its control channel (`crate::interface::control`).
//!
//! Each command prints the screen it leads to — Markdown by default, JSON
//! with `--json` — once Linear has answered what it asked for.

use std::path::PathBuf;

use anyhow::{Result, bail};

use super::args::{Args, text_or_stdin};
use crate::infra::disk::snapshot::{self, Shelf};
use crate::interface::control::screen::ScreenReport;
use crate::interface::control::{self, Command};

const USAGE: &str = "linear-tui tui <screen|press <keys>|type <text>|run <title>|open <ID>|quit> [--json] [--workspace <path>]";

pub async fn run(args: &[String]) -> Result<()> {
    let args = Args::parse(args, &["json"], &["workspace"])?;
    let command = command(&args.positional)?;
    let cwd = match args.value("workspace") {
        Some(path) => PathBuf::from(path),
        None => std::env::current_dir()?,
    };
    let quitting = command == Command::Quit;
    let reply = control::send(&control_file(&cwd)?, command).await?;
    if let Some(error) = reply.error {
        bail!("{error}");
    }
    if quitting {
        println!("linear-tui quit");
        return Ok(());
    }
    let Some(screen) = reply.screen else {
        bail!("linear-tui answered without a screen");
    };
    if args.flag("json") {
        println!("{}", serde_json::to_string_pretty(&screen)?);
    } else {
        let report: ScreenReport = serde_json::from_value(screen)?;
        println!("{}", report.markdown().trim_end());
    }
    Ok(())
}

/// The command the positionals spell.
fn command(positional: &[String]) -> Result<Command> {
    let joined = |rest: &[String]| rest.join(" ");
    Ok(match positional {
        [verb] if verb == "screen" => Command::Screen,
        [verb] if verb == "quit" => Command::Quit,
        [verb, rest @ ..] if verb == "press" && !rest.is_empty() => {
            Command::Press { keys: joined(rest) }
        }
        [verb, text] if verb == "type" => Command::Type {
            text: text_or_stdin(text)?,
        },
        [verb, rest @ ..] if verb == "run" && !rest.is_empty() => Command::Run {
            title: joined(rest),
        },
        [verb, issue] if verb == "open" => Command::Open {
            issue: issue.clone(),
        },
        _ => bail!("Usage: {USAGE}"),
    })
}

/// The control file of the linear-tui running for `cwd`'s repository.
fn control_file(cwd: &std::path::Path) -> Result<PathBuf> {
    let workspace = snapshot::workspace_of(cwd);
    let shelf = Shelf::new(&snapshot::state_dir()?, &workspace);
    let instances = shelf.instances();
    match crate::core::usecase::instance::to_report(&instances) {
        Some((instance, true)) => Ok(control::endpoint_path(&shelf.path_for(instance.id.pid))),
        _ => bail!(
            "No linear-tui is running for {} — start one there (`linear-tui`, or `linear-tui --headless`).",
            workspace.display()
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn words(text: &str) -> Vec<String> {
        text.split_whitespace().map(String::from).collect()
    }

    #[test]
    fn each_verb_makes_its_command() {
        assert_eq!(command(&words("screen")).unwrap(), Command::Screen);
        assert_eq!(
            command(&words("press g m")).unwrap(),
            Command::Press { keys: "g m".into() }
        );
        assert_eq!(
            command(&["type".into(), "In Review".into()]).unwrap(),
            Command::Type {
                text: "In Review".into()
            }
        );
        assert_eq!(
            command(&words("run Change status")).unwrap(),
            Command::Run {
                title: "Change status".into()
            }
        );
        assert_eq!(
            command(&words("open ENG-42")).unwrap(),
            Command::Open {
                issue: "ENG-42".into()
            }
        );
        assert_eq!(command(&words("quit")).unwrap(), Command::Quit);
    }

    #[test]
    fn a_verb_without_what_it_needs_shows_the_usage() {
        for bad in ["", "press", "run", "open", "type", "fly"] {
            let error = command(&words(bad)).unwrap_err();
            assert!(error.to_string().starts_with("Usage:"), "{bad}: {error}");
        }
    }
}
