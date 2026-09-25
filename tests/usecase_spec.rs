//! The use case layer reads as the specification, checked against the source
//! under `src/usecase/`. The rules are in `docs/development.md` ("The use
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
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("src/usecase");
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
        "modules missing from the table in src/usecase/mod.rs: {missing:?}"
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
