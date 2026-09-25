//! A small argument parser: positionals, boolean flags, and options that
//! take a value (`--team ENG` or `--team=ENG`). Anything it does not know is
//! an error, so a typo fails loudly instead of being ignored.

use std::io::Read;

use anyhow::{Result, bail};

#[derive(Debug, Default, PartialEq)]
pub struct Args {
    pub positional: Vec<String>,
    flags: Vec<String>,
    values: Vec<(String, String)>,
}

impl Args {
    /// Parse `args` against the flags and valued options a command accepts,
    /// given without their leading `--`. After `--` everything is positional.
    pub fn parse(args: &[String], flags: &[&str], valued: &[&str]) -> Result<Self> {
        let mut parsed = Self::default();
        let mut rest = args.iter();
        while let Some(arg) = rest.next() {
            let Some(name) = arg.strip_prefix("--") else {
                parsed.positional.push(arg.clone());
                continue;
            };
            if name.is_empty() {
                parsed.positional.extend(rest.cloned());
                break;
            }
            let (name, inline) = match name.split_once('=') {
                Some((name, value)) => (name, Some(value.to_string())),
                None => (name, None),
            };
            if flags.contains(&name) && inline.is_none() {
                parsed.flags.push(name.to_string());
            } else if valued.contains(&name) {
                let Some(value) = inline.or_else(|| rest.next().cloned()) else {
                    bail!("--{name} needs a value");
                };
                parsed.values.push((name.to_string(), value));
            } else {
                bail!("unknown option --{name}");
            }
        }
        Ok(parsed)
    }

    pub fn flag(&self, name: &str) -> bool {
        self.flags.iter().any(|f| f == name)
    }

    /// The last value given for `name`.
    pub fn value(&self, name: &str) -> Option<&str> {
        self.values
            .iter()
            .rev()
            .find(|(n, _)| n == name)
            .map(|(_, v)| v.as_str())
    }

    /// Exactly `N` positionals, or the usage line.
    pub fn positionals<const N: usize>(&self, usage: &str) -> Result<[&str; N]> {
        if self.positional.len() != N {
            bail!("Usage: {usage}");
        }
        Ok(std::array::from_fn(|i| self.positional[i].as_str()))
    }
}

/// `text` itself, or stdin when it is `-` — so an agent can pass a
/// multi-line body without quoting it into one argument.
pub fn text_or_stdin(text: &str) -> Result<String> {
    if text != "-" {
        return Ok(text.to_string());
    }
    let mut body = String::new();
    std::io::stdin().read_to_string(&mut body)?;
    Ok(body.trim_end_matches('\n').to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(list: &[&str]) -> Vec<String> {
        list.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn flags_values_and_positionals_are_told_apart() {
        let parsed = Args::parse(
            &args(&["ENG-1", "--json", "--team", "ENG", "--title=A b", "Done"]),
            &["json"],
            &["team", "title"],
        )
        .unwrap();
        assert_eq!(parsed.positional, ["ENG-1", "Done"]);
        assert!(parsed.flag("json"));
        assert_eq!(parsed.value("team"), Some("ENG"));
        assert_eq!(parsed.value("title"), Some("A b"));
        assert_eq!(parsed.value("description"), None);
    }

    #[test]
    fn everything_after_a_double_dash_is_positional() {
        let parsed = Args::parse(&args(&["ENG-1", "--", "--json", "-"]), &["json"], &[]).unwrap();
        assert_eq!(parsed.positional, ["ENG-1", "--json", "-"]);
        assert!(!parsed.flag("json"));
    }

    #[test]
    fn a_typo_or_a_missing_value_is_an_error() {
        let err = Args::parse(&args(&["--jsn"]), &["json"], &[]).unwrap_err();
        assert_eq!(err.to_string(), "unknown option --jsn");
        let err = Args::parse(&args(&["--team"]), &[], &["team"]).unwrap_err();
        assert_eq!(err.to_string(), "--team needs a value");
    }

    #[test]
    fn positionals_are_counted() {
        let parsed = Args::parse(&args(&["ENG-1"]), &[], &[]).unwrap();
        let [id] = parsed.positionals::<1>("x <ID>").unwrap();
        assert_eq!(id, "ENG-1");
        let err = parsed.positionals::<2>("x <ID> <state>").unwrap_err();
        assert_eq!(err.to_string(), "Usage: x <ID> <state>");
    }
}
