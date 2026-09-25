//! The use case layer reads as the specification, checked against the source
//! under `src/core/usecase/`. The rules are in `docs/development.md` ("The use
//! case layer"); in short:
//!
//! - `mod.rs` lists every aggregate module in its table.
//! - Each module opens with a `//!` summary that links every use case in it.
//! - A use case is a top-level `pub fn`. Its doc comment opens with its name
//!   in bold (`/// **Change an issue's status** (`s`).`), then its rules.
//! - Each use case is exercised by a test in its module.
//! - Each test states one rule: a `///` sentence above it, and a name that
//!   reads as that rule (`a_status_change_shows_everywhere_at_once`).

use std::path::Path;

/// A source file of the use case layer.
struct Module {
    name: String,
    text: String,
}

impl Module {
    /// The part before `#[cfg(test)]`, and the tests.
    fn split(&self) -> (&str, &str) {
        match self.text.find("#[cfg(test)]") {
            Some(at) => self.text.split_at(at),
            None => (&self.text, ""),
        }
    }

    /// The module's `//!` summary.
    fn summary(&self) -> String {
        self.text
            .lines()
            .take_while(|l| l.starts_with("//!"))
            .collect::<Vec<_>>()
            .join("\n")
    }
}

fn modules() -> Vec<Module> {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("src/core/usecase");
    let mut modules: Vec<Module> = std::fs::read_dir(dir)
        .unwrap()
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| path.extension().is_some_and(|e| e == "rs"))
        .map(|path| Module {
            name: path.file_stem().unwrap().to_string_lossy().into_owned(),
            text: std::fs::read_to_string(&path).unwrap(),
        })
        .collect();
    modules.sort_by(|a, b| a.name.cmp(&b.name));
    modules
}

/// The doc comment lines right above line `at`, top to bottom.
fn doc_above<'a>(lines: &[&'a str], at: usize) -> Vec<&'a str> {
    let mut doc: Vec<&str> = lines[..at]
        .iter()
        .rev()
        .take_while(|l| l.trim_start().starts_with("///") || l.trim_start().starts_with("#["))
        .filter(|l| l.trim_start().starts_with("///"))
        .copied()
        .collect();
    doc.reverse();
    doc
}

/// The top-level `pub fn`s of a module's code: its use cases.
fn use_cases(code: &str) -> Vec<(String, Vec<String>)> {
    let lines: Vec<&str> = code.lines().collect();
    lines
        .iter()
        .enumerate()
        .filter_map(|(n, line)| {
            let rest = line.strip_prefix("pub fn ")?;
            let name = rest.split(['(', '<']).next()?.to_string();
            let doc = doc_above(&lines, n).iter().map(|l| l.to_string()).collect();
            Some((name, doc))
        })
        .collect()
}

/// The `#[test]` functions of a module's tests, with their doc comments.
fn tests(tests: &str) -> Vec<(String, Vec<String>)> {
    let lines: Vec<&str> = tests.lines().collect();
    lines
        .iter()
        .enumerate()
        .filter(|(_, l)| l.trim() == "#[test]")
        .filter_map(|(n, _)| {
            let signature = lines.get(n + 1)?.trim();
            let name = signature
                .strip_prefix("fn ")?
                .split('(')
                .next()?
                .to_string();
            let doc = doc_above(&lines, n).iter().map(|l| l.to_string()).collect();
            Some((name, doc))
        })
        .collect()
}

/// `mod.rs` names every aggregate module in its table.
#[test]
fn the_layers_table_lists_every_module() {
    let modules = modules();
    let root = modules.iter().find(|m| m.name == "mod").unwrap();
    let missing: Vec<&str> = modules
        .iter()
        .filter(|m| m.name != "mod")
        .filter(|m| !root.text.contains(&format!("| [`{}`] |", m.name)))
        .map(|m| m.name.as_str())
        .collect();
    assert!(
        missing.is_empty(),
        "modules missing from the table in src/core/usecase/mod.rs: {missing:?}"
    );
}

/// Each module opens with a summary that links every use case in it.
#[test]
fn each_module_summary_links_its_use_cases() {
    let mut problems = Vec::new();
    for module in modules().iter().filter(|m| m.name != "mod") {
        let summary = module.summary();
        if summary.is_empty() {
            problems.push(format!("{}.rs: no //! summary", module.name));
            continue;
        }
        let (code, _) = module.split();
        for (name, _) in use_cases(code) {
            if !summary.contains(&format!("[`{name}`]")) {
                problems.push(format!(
                    "{}.rs: summary does not link [`{name}`]",
                    module.name
                ));
            }
        }
    }
    assert!(problems.is_empty(), "{}", problems.join("\n"));
}

/// Each use case's doc comment opens with its name in bold, then says more.
#[test]
fn each_use_case_is_named_and_described() {
    let mut problems = Vec::new();
    for module in modules() {
        let (code, _) = module.split();
        for (name, doc) in use_cases(code) {
            let opens_in_bold = doc
                .first()
                .is_some_and(|l| l.trim_start().starts_with("/// **"));
            if !opens_in_bold {
                problems.push(format!(
                    "{}.rs: {name} does not open with its name in bold",
                    module.name
                ));
            }
        }
    }
    assert!(problems.is_empty(), "{}", problems.join("\n"));
}

/// Each use case is exercised by a test in its module.
#[test]
fn each_use_case_has_a_test() {
    let mut problems = Vec::new();
    for module in modules() {
        let (code, tests) = module.split();
        for (name, _) in use_cases(code) {
            if !tests.contains(&format!("{name}(")) {
                problems.push(format!("{}.rs: no test calls {name}", module.name));
            }
        }
    }
    assert!(problems.is_empty(), "{}", problems.join("\n"));
}

/// Every `module::function` named in `text`.
fn named_use_cases(text: &str) -> Vec<String> {
    text.split(|c: char| !(c.is_alphanumeric() || c == '_' || c == ':'))
        .filter(|word| {
            let mut parts = word.split("::");
            matches!((parts.next(), parts.next(), parts.next()), (Some(m), Some(f), None) if !m.is_empty() && !f.is_empty())
        })
        .map(String::from)
        .collect()
}

/// Each use case is run end to end by at least one scenario in
/// `tests/e2e/` (on its `Covers:` line), or is listed there under "Not end
/// to end" with the reason.
#[test]
fn each_use_case_has_an_end_to_end_scenario() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/e2e");
    let mut covered = Vec::new();
    for entry in std::fs::read_dir(&dir).unwrap().flatten() {
        let text = std::fs::read_to_string(entry.path()).unwrap();
        // A `Covers:` line and the doc lines continuing it.
        let mut in_covers = false;
        for line in text.lines() {
            let doc = line
                .trim_start()
                .trim_start_matches("///")
                .trim_start_matches("//!");
            let is_doc =
                line.trim_start().starts_with("///") || line.trim_start().starts_with("//!");
            if let Some(rest) = doc.trim_start().strip_prefix("Covers:") {
                in_covers = true;
                covered.extend(named_use_cases(rest));
            } else if in_covers && is_doc && !doc.trim().is_empty() {
                covered.extend(named_use_cases(doc));
            } else {
                in_covers = false;
            }
        }
        if entry.file_name() == "main.rs" {
            let exempt = text
                .split("Not end to end:")
                .nth(1)
                .expect("tests/e2e/main.rs lists what is not end to end");
            covered.extend(named_use_cases(
                exempt.split("\n\n").next().unwrap_or_default(),
            ));
        }
    }
    let mut missing = Vec::new();
    for module in modules().iter().filter(|m| m.name != "mod") {
        let (code, _) = module.split();
        for (name, _) in use_cases(code) {
            let full = format!("{}::{name}", module.name);
            if !covered.contains(&full) {
                missing.push(full);
            }
        }
    }
    assert!(
        missing.is_empty(),
        "use cases no end-to-end scenario covers (add one, or list the reason under \
         \"Not end to end\" in tests/e2e/main.rs): {missing:?}"
    );
}

/// Each test states one rule: a sentence above it, and a name that reads as
/// that rule.
#[test]
fn each_test_states_its_rule() {
    let mut problems = Vec::new();
    for module in modules() {
        let (_, tests_part) = module.split();
        for (name, doc) in tests(tests_part) {
            if doc.is_empty() {
                problems.push(format!(
                    "{}.rs: {name} has no /// rule above it",
                    module.name
                ));
            }
            if name.starts_with("test_") || name.split('_').count() < 3 {
                problems.push(format!(
                    "{}.rs: {name} does not read as a rule (a sentence, not test_…)",
                    module.name
                ));
            }
        }
    }
    assert!(problems.is_empty(), "{}", problems.join("\n"));
}
