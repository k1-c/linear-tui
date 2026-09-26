//! Writing text in the user's own editor: `$VISUAL`, else `$EDITOR`, else
//! `vi`, on a temporary Markdown file only the user can read. The caller
//! lends it the terminal.

use std::path::PathBuf;
use std::process::Command;
use std::sync::atomic::{AtomicU32, Ordering};

use anyhow::{Context, Result, bail};

use crate::private_file;

/// Open `text` in the editor and return what it was saved as, or `None`
/// when the editor exited with an error (quitting Vim with `:cq`, say).
pub fn edit(text: &str) -> Result<Option<String>> {
    let command = editor_command(
        std::env::var("VISUAL").ok().as_deref(),
        std::env::var("EDITOR").ok().as_deref(),
    );
    let path = temp_path();
    private_file::write(&path, text.as_bytes())?;
    let outcome = run(&command, &path);
    let _ = std::fs::remove_file(&path);
    outcome
}

fn run(command: &[String], path: &PathBuf) -> Result<Option<String>> {
    let Some((program, args)) = command.split_first() else {
        bail!("no editor: set $EDITOR");
    };
    let status = Command::new(program)
        .args(args)
        .arg(path)
        .status()
        .with_context(|| format!("could not run {program}"))?;
    if !status.success() {
        return Ok(None);
    }
    let saved = std::fs::read_to_string(path)
        .with_context(|| format!("could not read back {}", path.display()))?;
    Ok(Some(saved.trim_end_matches('\n').to_string()))
}

/// The editor to run and its arguments: `$VISUAL`, then `$EDITOR`, then
/// `vi`. A value may carry arguments (`code --wait`); an empty one is
/// skipped.
fn editor_command(visual: Option<&str>, editor: Option<&str>) -> Vec<String> {
    [visual, editor]
        .into_iter()
        .flatten()
        .map(|v| v.split_whitespace().map(String::from).collect::<Vec<_>>())
        .find(|words| !words.is_empty())
        .unwrap_or_else(|| vec!["vi".to_string()])
}

/// A file name no other edit, in this process or another, is using.
fn temp_path() -> PathBuf {
    static NEXT: AtomicU32 = AtomicU32::new(0);
    std::env::temp_dir().join(format!(
        "linear-tui-{}-{}.md",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn visual_wins_then_editor_then_vi() {
        assert_eq!(editor_command(Some("nvim"), Some("nano")), ["nvim"]);
        assert_eq!(
            editor_command(Some(" "), Some("code --wait")),
            ["code", "--wait"]
        );
        assert_eq!(editor_command(None, None), ["vi"]);
    }

    #[cfg(unix)]
    #[test]
    fn what_the_editor_saves_comes_back_and_a_failed_editor_is_none() {
        let path = temp_path();
        std::fs::write(&path, "old").unwrap();
        let append = vec!["sh".into(), "-c".into(), "printf 'new\\n' > \"$0\"".into()];
        assert_eq!(run(&append, &path).unwrap().as_deref(), Some("new"));
        let fail = vec!["false".to_string()];
        assert_eq!(run(&fail, &path).unwrap(), None);
        let _ = std::fs::remove_file(&path);
    }
}
