//! The layers only depend inwards, checked against the source under `src/`,
//! where each layer is a directory:
//!
//! ```text
//! src/adapter/     api · auth · cli · dispatch · herdr · snapshot   the edges
//! src/interface/   app · ui · keys · palette · event · …            what the user sees and does
//! src/usecase/     one module per aggregate; usecase::Request        what the user can do
//! src/entity/      entities and value objects                        what it is all about
//! src/store/       what Linear has told us, kept consistent
//! ```
//!
//! A layer may name its own modules and those further in, never those
//! further out; the inner layers also stay clear of the crates that draw,
//! read the terminal, or do I/O. `config` (settings handed to every layer at
//! startup) sits beside them. See AGENTS.md ("Architecture").

use std::path::{Path, PathBuf};

/// A layer: its directory under `src/`, the crate modules it may name, and
/// the external crates it must not use.
struct Layer {
    name: &'static str,
    allowed: &'static [&'static str],
    forbidden_crates: &'static [&'static str],
}

/// Crates for drawing, the terminal, the network, and the runtime.
const OUTER_CRATES: &[&str] = &["ratatui", "crossterm", "reqwest", "tokio", "open"];

const LAYERS: &[Layer] = &[
    Layer {
        name: "entity",
        allowed: &["entity"],
        forbidden_crates: OUTER_CRATES,
    },
    Layer {
        name: "store",
        allowed: &["entity", "store"],
        forbidden_crates: OUTER_CRATES,
    },
    Layer {
        name: "usecase",
        allowed: &["entity", "store", "usecase"],
        forbidden_crates: OUTER_CRATES,
    },
    Layer {
        name: "interface",
        allowed: &["entity", "store", "usecase", "interface", "config"],
        forbidden_crates: &["reqwest", "tokio", "open"],
    },
];

/// Every `crate::<module>` a line names, outside comments.
fn crate_modules(line: &str) -> Vec<&str> {
    let code = line.split("//").next().unwrap_or_default();
    code.match_indices("crate::")
        .map(|(at, _)| {
            let rest = &code[at + "crate::".len()..];
            let end = rest
                .find(|c: char| !(c.is_alphanumeric() || c == '_'))
                .unwrap_or(rest.len());
            &rest[..end]
        })
        .collect()
}

/// Whether a line uses the external crate `name`, outside comments.
fn uses_crate(line: &str, name: &str) -> bool {
    let code = line.split("//").next().unwrap_or_default();
    let path = format!("{name}::");
    code.match_indices(&path).any(|(at, _)| {
        // `ratatui::` on its own, not `crate::ratatui_thing::` or `my_tokio::`.
        code[..at]
            .chars()
            .last()
            .is_none_or(|c| !(c.is_alphanumeric() || c == '_' || c == ':'))
    })
}

fn rust_files(path: &Path, out: &mut Vec<PathBuf>) {
    if path.is_dir() {
        for entry in std::fs::read_dir(path).unwrap().flatten() {
            rust_files(&entry.path(), out);
        }
    } else if path.extension().is_some_and(|e| e == "rs") {
        out.push(path.to_path_buf());
    }
}

fn layer_files(layer: &Layer) -> Vec<PathBuf> {
    let mut files = Vec::new();
    rust_files(
        &Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("src")
            .join(layer.name),
        &mut files,
    );
    assert!(!files.is_empty(), "no sources for layer {}", layer.name);
    files
}

/// Each layer names only itself and the layers further in, and uses none of
/// the crates it is kept clear of.
#[test]
fn every_layer_depends_only_inwards() {
    let mut violations = Vec::new();
    for layer in LAYERS {
        for file in layer_files(layer) {
            let text = std::fs::read_to_string(&file).unwrap();
            let shown = file
                .strip_prefix(env!("CARGO_MANIFEST_DIR"))
                .unwrap_or(&file)
                .display()
                .to_string();
            for (n, line) in text.lines().enumerate() {
                for module in crate_modules(line) {
                    if !layer.allowed.contains(&module) {
                        violations.push(format!(
                            "{shown}:{}: {} names crate::{module}",
                            n + 1,
                            layer.name
                        ));
                    }
                }
                for name in layer.forbidden_crates {
                    if uses_crate(line, name) {
                        violations.push(format!(
                            "{shown}:{}: {} uses the {name} crate",
                            n + 1,
                            layer.name
                        ));
                    }
                }
            }
        }
    }
    assert!(
        violations.is_empty(),
        "dependencies pointing outwards:\n{}",
        violations.join("\n")
    );
}

/// The checker itself: it sees a module path and a crate path, and not ones
/// in comments.
#[test]
fn the_checker_reads_paths_and_skips_comments() {
    assert_eq!(
        crate_modules("use crate::entity::Issue; // crate::ui"),
        ["entity"]
    );
    assert!(uses_crate("use ratatui::style::Color;", "ratatui"));
    assert!(!uses_crate("// ratatui::style::Color", "ratatui"));
    assert!(!uses_crate("use crate::my_tokio::x;", "tokio"));
}
