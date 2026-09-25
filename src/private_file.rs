//! Writing files that hold credentials.
//!
//! `config.toml` can carry an API key and a client secret, and `tokens.json`
//! an OAuth token, so both have to be readable by their owner alone — from the
//! moment they exist, not after a `chmod` that follows the write.

use std::fs::{self, File, OpenOptions};
use std::io::Write;
use std::path::Path;

use anyhow::{Context, Result};

/// Replace `path` with `contents`, readable only by the current user.
///
/// The data goes to a sibling temporary file created owner-only, which is then
/// renamed over the target, so a crash mid-write leaves the old file intact
/// rather than a truncated one, and no reader ever sees looser permissions.
pub fn write(path: &Path, contents: &[u8]) -> Result<()> {
    let dir = path
        .parent()
        .with_context(|| format!("{} has no parent directory", path.display()))?;
    fs::create_dir_all(dir)?;

    let name = path
        .file_name()
        .with_context(|| format!("{} has no file name", path.display()))?;
    let tmp = dir.join(format!(".{}.tmp", name.to_string_lossy()));

    let result = (|| {
        let mut file = create_private(&tmp)?;
        file.write_all(contents)?;
        file.sync_all()?;
        fs::rename(&tmp, path)
    })();
    if result.is_err() {
        let _ = fs::remove_file(&tmp);
    }
    result.with_context(|| format!("Failed to write {}", path.display()))
}

/// Open `path` for appending, creating it owner-only, and tighten an existing
/// file that was created before this rule existed.
pub fn open_append(path: &Path) -> std::io::Result<File> {
    let mut options = OpenOptions::new();
    options.create(true).append(true);
    restrict(&mut options);
    let file = options.open(path)?;
    tighten(&file)?;
    Ok(file)
}

fn create_private(path: &Path) -> std::io::Result<File> {
    let mut options = OpenOptions::new();
    options.create(true).write(true).truncate(true);
    restrict(&mut options);
    let file = options.open(path)?;
    // `mode` only applies when the file is created; a stale temporary file
    // left by an earlier crash keeps whatever it had.
    tighten(&file)?;
    Ok(file)
}

#[cfg(unix)]
fn restrict(options: &mut OpenOptions) {
    use std::os::unix::fs::OpenOptionsExt;
    options.mode(0o600);
}

#[cfg(not(unix))]
fn restrict(_: &mut OpenOptions) {}

#[cfg(unix)]
fn tighten(file: &File) -> std::io::Result<()> {
    use std::os::unix::fs::PermissionsExt;
    file.set_permissions(fs::Permissions::from_mode(0o600))
}

#[cfg(not(unix))]
fn tighten(_: &File) -> std::io::Result<()> {
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch(name: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "linear-tui-private-file-{}-{name}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&dir);
        dir.join("secret.json")
    }

    #[test]
    fn writes_and_replaces_the_contents() {
        let path = scratch("replace");
        write(&path, b"first").unwrap();
        write(&path, b"second").unwrap();
        assert_eq!(fs::read_to_string(&path).unwrap(), "second");
        let leftovers: Vec<_> = fs::read_dir(path.parent().unwrap())
            .unwrap()
            .map(|e| e.unwrap().file_name())
            .collect();
        assert_eq!(leftovers, ["secret.json"], "no temporary file is left");
    }

    #[cfg(unix)]
    #[test]
    fn the_file_is_readable_by_its_owner_only() {
        use std::os::unix::fs::PermissionsExt;
        let path = scratch("mode");
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        // A file left world-readable by an older version is tightened too.
        fs::write(&path, "old").unwrap();
        fs::set_permissions(&path, fs::Permissions::from_mode(0o644)).unwrap();

        write(&path, b"new").unwrap();
        let mode = fs::metadata(&path).unwrap().permissions().mode();
        assert_eq!(mode & 0o777, 0o600);

        let log = path.with_file_name("debug.log");
        fs::write(&log, "").unwrap();
        fs::set_permissions(&log, fs::Permissions::from_mode(0o644)).unwrap();
        open_append(&log).unwrap();
        let mode = fs::metadata(&log).unwrap().permissions().mode();
        assert_eq!(mode & 0o777, 0o600);
    }
}
