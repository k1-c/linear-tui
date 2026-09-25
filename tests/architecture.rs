//! The layers only depend inwards, checked against the source under `src/`,
//! where each layer is a directory:
//!
//! ```text
//! src/core/         entity · store · usecase · message   what linear-tui is: no I/O
//! src/interface/    tui · control · cli                  the ways in: a person, an agent, a script
//! src/infra/        linear · herdr · disk · dispatch     the systems it calls on
//! src/runtime.rs, src/main.rs                            wire the ways in to the systems
//! ```
//!
//! Inside `core`, `entity` is innermost, then `store`, then `usecase`.
//! `interface` and `infra` depend on `core` and not on each other — except
//! `interface/cli`, whose subcommands each run on their own and assemble
//! the infra they need, as `main` does for the TUI. `control` reads what
//! `tui` draws. The core stays clear of the crates that draw, read the
//! terminal, or do I/O. `config`, `logging`, and `private_file` sit at the
//! root, beside `main`, for every layer outside the core. See AGENTS.md
//! ("Architecture").

use std::path::{Path, PathBuf};

/// A layer: its directory (or file) under `src/`, the crate modules it may
/// name — `core::entity` allows `crate::core::entity::…` — and the external
/// crates it must not use.
struct Layer {
    path: &'static str,
    allowed: &'static [&'static str],
    forbidden_crates: &'static [&'static str],
}

/// Crates for drawing, the terminal, the network, and the runtime.
const OUTER_CRATES: &[&str] = &["ratatui", "crossterm", "reqwest", "tokio", "open"];

const LAYERS: &[Layer] = &[
    Layer {
        path: "core/entity",
        allowed: &["core::entity"],
        forbidden_crates: OUTER_CRATES,
    },
    Layer {
        path: "core/store",
        allowed: &["core::entity", "core::store"],
        forbidden_crates: OUTER_CRATES,
    },
    Layer {
        path: "core/usecase",
        allowed: &["core::entity", "core::store", "core::usecase"],
        forbidden_crates: OUTER_CRATES,
    },
    Layer {
        path: "core/message.rs",
        allowed: &["core::entity", "core::usecase", "core::message"],
        forbidden_crates: OUTER_CRATES,
    },
    Layer {
        path: "interface/tui",
        allowed: &["core", "interface::tui", "config"],
        forbidden_crates: &["reqwest", "tokio", "open"],
    },
    Layer {
        path: "interface/control",
        allowed: &[
            "core",
            "interface::tui",
            "interface::control",
            "config",
            "private_file",
        ],
        forbidden_crates: &["reqwest"],
    },
    Layer {
        path: "interface/cli",
        allowed: &["core", "interface", "infra", "config", "private_file"],
        forbidden_crates: &[],
    },
    Layer {
        path: "infra",
        allowed: &["core", "infra", "config", "private_file"],
        forbidden_crates: &["ratatui", "crossterm"],
    },
];

/// Every module path a line names through `crate::`, outside comments, with
/// a `{…}` group spread into one path per item: `crate::core::{entity,
/// store}` names `core::entity` and `core::store`.
fn crate_paths(line: &str) -> Vec<String> {
    let code = line.split("//").next().unwrap_or_default();
    let is_path = |c: char| c.is_alphanumeric() || c == '_' || c == ':';
    let mut paths = Vec::new();
    for (at, _) in code.match_indices("crate::") {
        let rest = &code[at + "crate::".len()..];
        let end = rest.find(|c: char| !is_path(c)).unwrap_or(rest.len());
        let path = &rest[..end];
        if rest[end..].starts_with('{') && path.ends_with("::") {
            let group = &rest[end + 1..];
            let group = &group[..group.find('}').unwrap_or(group.len())];
            for item in group.split(',') {
                let item = item.trim();
                let item_end = item.find(|c: char| !is_path(c)).unwrap_or(item.len());
                paths.push(format!("{path}{}", &item[..item_end]));
            }
        } else {
            paths.push(path.trim_end_matches(':').to_string());
        }
    }
    paths
}

/// Whether `path` is `module` or inside it.
fn within(path: &str, module: &str) -> bool {
    path == module
        || path
            .strip_prefix(module)
            .is_some_and(|rest| rest.starts_with("::"))
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

fn src() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("src")
}

fn files_under(path: &str) -> Vec<PathBuf> {
    let mut files = Vec::new();
    rust_files(&src().join(path), &mut files);
    assert!(!files.is_empty(), "no sources under src/{path}");
    files
}

/// Each line of every file under `path`, with where it is.
fn lines_under(path: &str) -> Vec<(String, String)> {
    let mut out = Vec::new();
    for file in files_under(path) {
        let text = std::fs::read_to_string(&file).unwrap();
        let shown = file
            .strip_prefix(env!("CARGO_MANIFEST_DIR"))
            .unwrap_or(&file)
            .display()
            .to_string();
        for (n, line) in text.lines().enumerate() {
            out.push((format!("{shown}:{}", n + 1), line.to_string()));
        }
    }
    out
}

/// Each layer names only what it is allowed to, and uses none of the crates
/// it is kept clear of.
#[test]
fn every_layer_depends_only_inwards() {
    let mut violations = Vec::new();
    for layer in LAYERS {
        for (at, line) in lines_under(layer.path) {
            for path in crate_paths(&line) {
                if !layer.allowed.iter().any(|module| within(&path, module)) {
                    violations.push(format!("{at}: {} names crate::{path}", layer.path));
                }
            }
            for name in layer.forbidden_crates {
                if uses_crate(&line, name) {
                    violations.push(format!("{at}: {} uses the {name} crate", layer.path));
                }
            }
        }
    }
    assert!(
        violations.is_empty(),
        "dependencies pointing the wrong way:\n{}",
        violations.join("\n")
    );
}

/// The core is always `crate::core`: a bare `core::` is Rust's own crate,
/// and reads as if it were ours.
#[test]
fn the_core_is_always_named_through_crate() {
    let bare: Vec<String> = lines_under("")
        .into_iter()
        .filter(|(_, line)| uses_crate(line, "core"))
        .map(|(at, line)| format!("{at}: {}", line.trim()))
        .collect();
    assert!(bare.is_empty(), "bare core:: paths:\n{}", bare.join("\n"));
}

/// The layers table covers every directory under `src/`, so a new one is
/// placed on purpose.
#[test]
fn every_source_directory_is_in_a_layer() {
    let mut dirs = Vec::new();
    for top in ["core", "interface", "infra"] {
        for entry in std::fs::read_dir(src().join(top)).unwrap().flatten() {
            let name = entry.file_name().to_string_lossy().to_string();
            if name != "mod.rs" {
                dirs.push(format!("{top}/{name}"));
            }
        }
    }
    let unplaced: Vec<&String> = dirs
        .iter()
        .filter(|dir| {
            !LAYERS
                .iter()
                .any(|layer| dir.as_str() == layer.path || within_dir(dir, layer.path))
        })
        .collect();
    assert!(unplaced.is_empty(), "not in any layer: {unplaced:?}");
    for entry in std::fs::read_dir(src()).unwrap().flatten() {
        let name = entry.file_name().to_string_lossy().to_string();
        assert!(
            ["core", "interface", "infra"].contains(&name.as_str()) || name.ends_with(".rs"),
            "src/{name} is a directory outside the layers"
        );
    }
}

/// Whether `dir` lies inside the layer directory `layer`.
fn within_dir(dir: &str, layer: &str) -> bool {
    dir.strip_prefix(layer)
        .is_some_and(|rest| rest.starts_with('/'))
}

/// The checker itself: it sees module paths, groups, and crate paths, and
/// not ones in comments.
#[test]
fn the_checker_reads_paths_and_skips_comments() {
    assert_eq!(
        crate_paths("use crate::core::entity::Issue; // crate::ui"),
        ["core::entity::Issue"]
    );
    assert_eq!(
        crate_paths("use crate::core::{entity, store::Store};"),
        ["core::entity", "core::store::Store"]
    );
    assert!(within("core::entity::Issue", "core::entity"));
    assert!(!within("core::entityish", "core::entity"));
    assert!(uses_crate("use ratatui::style::Color;", "ratatui"));
    assert!(!uses_crate("// ratatui::style::Color", "ratatui"));
    assert!(!uses_crate("use crate::my_tokio::x;", "tokio"));
    assert!(!uses_crate("use crate::core::entity;", "core"));
}
