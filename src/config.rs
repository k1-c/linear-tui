use anyhow::{Context, Result};
use directories::ProjectDirs;
use ratatui::style::Color;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct Config {
    #[serde(default)]
    pub auth: AuthConfig,
    #[serde(default)]
    pub ui: UiConfig,
    #[serde(default)]
    pub agent: AgentConfig,
    /// Settings for one directory, keyed by its path (`~` allowed).
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub workspaces: BTreeMap<String, WorkspaceConfig>,
    /// What was wrong with the file as loaded — unknown keys, values out of
    /// range — for the app to surface. Never written back.
    #[serde(skip)]
    pub warnings: Vec<String>,
}

/// The keys each table of `config.toml` understands. A key outside these is
/// almost always a typo, which serde would otherwise ignore without a word.
const KNOWN_KEYS: &[(&str, &[&str])] = &[
    ("", &["auth", "ui", "agent", "workspaces"]),
    ("agent", &["control"]),
    (
        "auth",
        &["api_key", "oauth_client_id", "oauth_client_secret"],
    ),
    (
        "ui",
        &[
            "default_team",
            "items_per_page",
            "theme",
            "sidebar",
            "sidebar_width",
            "group_by",
        ],
    ),
];

/// Linear's largest page. A bigger `first` is refused by the API.
pub const MAX_ITEMS_PER_PAGE: u32 = 250;
pub const SIDEBAR_WIDTH: std::ops::RangeInclusive<u16> = 18..=48;

/// The keys a `[workspaces."<path>"]` table understands.
const WORKSPACE_KEYS: &[&str] = &["team"];

/// How linear-tui opens in one directory. An entry here wins over the view
/// remembered for the directory.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct WorkspaceConfig {
    /// The team to open on, by name or key.
    pub team: Option<String>,
}

/// How coding agents may work with linear-tui.
#[derive(Debug, Serialize, Deserialize)]
pub struct AgentConfig {
    /// Whether a running instance takes commands from agents on its control
    /// channel (`linear-tui tui …`). On unless turned off.
    #[serde(default = "default_true")]
    pub control: bool,
}

impl Default for AgentConfig {
    fn default() -> Self {
        Self { control: true }
    }
}

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct AuthConfig {
    pub api_key: Option<String>,
    pub oauth_client_id: Option<String>,
    pub oauth_client_secret: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct UiConfig {
    pub default_team: Option<String>,
    #[serde(default = "default_items_per_page")]
    pub items_per_page: u32,
    #[serde(default)]
    pub theme: ThemeName,
    /// Show the navigation sidebar. It hides itself on narrow terminals
    /// regardless, so this is the preference, not the current state.
    #[serde(default = "default_true")]
    pub sidebar: bool,
    #[serde(default = "default_sidebar_width")]
    pub sidebar_width: u16,
    /// How a list groups its issues out of the box.
    #[serde(default)]
    pub group_by: GroupByName,
}

impl Default for UiConfig {
    fn default() -> Self {
        Self {
            default_team: None,
            items_per_page: default_items_per_page(),
            theme: ThemeName::default(),
            sidebar: default_true(),
            sidebar_width: default_sidebar_width(),
            group_by: GroupByName::default(),
        }
    }
}

fn unknown_keys(table: &toml::Table) -> Vec<String> {
    let mut unknown = Vec::new();
    for (section, known) in KNOWN_KEYS {
        let keys = if section.is_empty() {
            Some(table)
        } else {
            table.get(*section).and_then(toml::Value::as_table)
        };
        for key in keys.into_iter().flat_map(|t| t.keys()) {
            if !known.contains(&key.as_str()) {
                let path = if section.is_empty() {
                    key.clone()
                } else {
                    format!("{section}.{key}")
                };
                unknown.push(format!("unknown key `{path}` is ignored"));
            }
        }
    }
    let workspaces = table.get("workspaces").and_then(toml::Value::as_table);
    for (path, entry) in workspaces.into_iter().flatten() {
        let keys = entry.as_table().into_iter().flat_map(|t| t.keys());
        for key in keys.filter(|k| !WORKSPACE_KEYS.contains(&k.as_str())) {
            unknown.push(format!(
                "unknown key `workspaces.\"{path}\".{key}` is ignored"
            ));
        }
    }
    unknown
}

fn default_items_per_page() -> u32 {
    50
}

fn default_true() -> bool {
    true
}

fn default_sidebar_width() -> u16 {
    26
}

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum GroupByName {
    #[default]
    Status,
    Assignee,
    Priority,
    Project,
    None,
}

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ThemeName {
    #[default]
    Default,
    Light,
    Ocean,
}

/// Runtime color theme derived from ThemeName.
#[allow(dead_code)]
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Theme {
    pub accent: Color,
    pub secondary: Color,
    pub muted: Color,
    pub warning: Color,
    pub error: Color,
    pub success: Color,
    pub text: Color,
    pub text_dim: Color,
    pub border: Color,
    pub highlight_fg: Color,

    /// Background of the selected row. A tinted band reads far better than the
    /// reverse-video block a terminal gives you by default, and it leaves the
    /// row's own colours (status, priority, labels) legible instead of
    /// inverting them into mud.
    pub selection_bg: Color,
    /// Background of a group header band and of the sidebar's active entry.
    pub surface: Color,
    /// Background behind inline code and fenced blocks.
    pub code_bg: Color,
    /// Background of a label chip.
    pub chip_bg: Color,

    // Priority colors
    pub pri_urgent: Color,
    pub pri_high: Color,
    pub pri_medium: Color,
    pub pri_low: Color,
}

impl Theme {
    pub fn from_name(name: ThemeName) -> Self {
        match name {
            ThemeName::Default => Self::dark(),
            ThemeName::Light => Self::light(),
            ThemeName::Ocean => Self::ocean(),
        }
    }

    fn dark() -> Self {
        Self {
            accent: Color::Rgb(126, 138, 240),
            secondary: Color::Rgb(180, 140, 255),
            muted: Color::Rgb(110, 114, 126),
            warning: Color::Yellow,
            error: Color::Red,
            success: Color::Green,
            text: Color::White,
            text_dim: Color::Gray,
            border: Color::Rgb(60, 62, 70),
            highlight_fg: Color::White,
            selection_bg: Color::Rgb(40, 42, 54),
            surface: Color::Rgb(28, 28, 32),
            code_bg: Color::Rgb(38, 38, 44),
            chip_bg: Color::Rgb(45, 45, 52),
            pri_urgent: Color::Red,
            pri_high: Color::Rgb(255, 165, 0),
            pri_medium: Color::Yellow,
            pri_low: Color::Blue,
        }
    }

    fn light() -> Self {
        Self {
            accent: Color::Blue,
            secondary: Color::Magenta,
            muted: Color::Gray,
            warning: Color::Rgb(200, 150, 0),
            error: Color::Red,
            success: Color::Green,
            text: Color::Black,
            text_dim: Color::DarkGray,
            border: Color::Rgb(205, 205, 210),
            highlight_fg: Color::Black,
            selection_bg: Color::Rgb(225, 228, 240),
            surface: Color::Rgb(240, 240, 244),
            code_bg: Color::Rgb(233, 233, 238),
            chip_bg: Color::Rgb(228, 228, 234),
            pri_urgent: Color::Red,
            pri_high: Color::Rgb(200, 100, 0),
            pri_medium: Color::Rgb(180, 150, 0),
            pri_low: Color::Blue,
        }
    }

    fn ocean() -> Self {
        Self {
            accent: Color::Rgb(100, 200, 255),
            secondary: Color::Rgb(180, 140, 255),
            muted: Color::Rgb(80, 80, 100),
            warning: Color::Rgb(255, 200, 80),
            error: Color::Rgb(255, 100, 100),
            success: Color::Rgb(100, 220, 150),
            text: Color::Rgb(220, 230, 240),
            text_dim: Color::Rgb(140, 150, 170),
            border: Color::Rgb(55, 72, 96),
            highlight_fg: Color::Rgb(240, 245, 255),
            selection_bg: Color::Rgb(30, 48, 70),
            surface: Color::Rgb(21, 32, 46),
            code_bg: Color::Rgb(28, 42, 60),
            chip_bg: Color::Rgb(34, 50, 70),
            pri_urgent: Color::Rgb(255, 80, 80),
            pri_high: Color::Rgb(255, 165, 80),
            pri_medium: Color::Rgb(255, 220, 80),
            pri_low: Color::Rgb(80, 160, 255),
        }
    }
}

impl Config {
    pub fn load() -> Result<Self> {
        let path = Self::config_path()?;
        if path.exists() {
            let contents = fs::read_to_string(&path)
                .with_context(|| format!("Failed to read config: {}", path.display()))?;
            Self::parse(&contents)
        } else {
            Ok(Self::default())
        }
    }

    /// Parse `config.toml`, then check what serde lets through: unknown keys,
    /// and values the app would otherwise only trip over later.
    pub fn parse(contents: &str) -> Result<Self> {
        let mut config: Self = toml::from_str(contents).context("Failed to parse config")?;
        let table: toml::Table = toml::from_str(contents).context("Failed to parse config")?;
        config.warnings = unknown_keys(&table);
        config.validate();
        for warning in &config.warnings {
            tracing::warn!(%warning, "config.toml");
        }
        Ok(config)
    }

    fn validate(&mut self) {
        let per_page = self.ui.items_per_page;
        if !(1..=MAX_ITEMS_PER_PAGE).contains(&per_page) {
            self.ui.items_per_page = per_page.clamp(1, MAX_ITEMS_PER_PAGE);
            self.warnings.push(format!(
                "ui.items_per_page = {per_page} is outside 1–{MAX_ITEMS_PER_PAGE}; using {}",
                self.ui.items_per_page
            ));
        }
        let width = self.ui.sidebar_width;
        if !SIDEBAR_WIDTH.contains(&width) {
            self.ui.sidebar_width = width.clamp(*SIDEBAR_WIDTH.start(), *SIDEBAR_WIDTH.end());
            self.warnings.push(format!(
                "ui.sidebar_width = {width} is outside {}–{}; using {}",
                SIDEBAR_WIDTH.start(),
                SIDEBAR_WIDTH.end(),
                self.ui.sidebar_width
            ));
        }
    }

    /// The entry for `workspace` — the repository — or for `cwd` itself,
    /// the more specific one first.
    pub fn workspace(&self, workspace: &Path, cwd: &Path) -> Option<&WorkspaceConfig> {
        let home = directories::BaseDirs::new().map(|d| d.home_dir().to_path_buf());
        let expand = |key: &str| match (key.strip_prefix("~/"), &home) {
            (Some(rest), Some(home)) => home.join(rest),
            _ => PathBuf::from(key),
        };
        let matches = |dir: &Path| {
            let dir = fs::canonicalize(dir).unwrap_or_else(|_| dir.to_path_buf());
            self.workspaces.iter().find_map(|(key, entry)| {
                let path = expand(key);
                let path = fs::canonicalize(&path).unwrap_or(path);
                (path == dir).then_some(entry)
            })
        };
        matches(cwd).or_else(|| matches(workspace))
    }

    /// Write the config back. It can hold an API key and a client secret, so
    /// it is written owner-only.
    pub fn save(&self) -> Result<()> {
        let contents = toml::to_string_pretty(self)?;
        crate::adapter::private_file::write(&Self::config_path()?, contents.as_bytes())
    }

    pub fn config_dir() -> Result<PathBuf> {
        let dirs = ProjectDirs::from("", "", "linear-tui")
            .context("Failed to determine config directory")?;
        Ok(dirs.config_dir().to_path_buf())
    }

    fn config_path() -> Result<PathBuf> {
        Ok(Self::config_dir()?.join("config.toml"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_typo_is_reported_rather_than_silently_ignored() {
        let config =
            Config::parse("theem = 1\n[ui]\ntheem = \"light\"\n[auth]\napi_key = \"k\"\n").unwrap();
        assert_eq!(
            config.warnings,
            [
                "unknown key `theem` is ignored",
                "unknown key `ui.theem` is ignored"
            ]
        );
        assert_eq!(config.auth.api_key.as_deref(), Some("k"));
    }

    #[test]
    fn out_of_range_values_are_clamped_with_a_warning() {
        let config = Config::parse("[ui]\nitems_per_page = 1000\nsidebar_width = 3\n").unwrap();
        assert_eq!(config.ui.items_per_page, MAX_ITEMS_PER_PAGE);
        assert_eq!(config.ui.sidebar_width, *SIDEBAR_WIDTH.start());
        assert_eq!(config.warnings.len(), 2);
    }

    #[test]
    fn a_workspace_entry_is_found_by_path_and_checked_for_typos() {
        let config = Config::parse(
            "[workspaces.\"/tmp\"]\nteam = \"ENG\"\n[workspaces.\"/nowhere\"]\ntaem = \"X\"\n",
        )
        .unwrap();
        assert_eq!(
            config.warnings,
            ["unknown key `workspaces.\"/nowhere\".taem` is ignored"]
        );
        let entry = config.workspace(Path::new("/elsewhere"), Path::new("/tmp"));
        assert_eq!(entry.and_then(|e| e.team.as_deref()), Some("ENG"));
        assert!(config.workspace(Path::new("/a"), Path::new("/b")).is_none());
    }

    #[test]
    fn a_valid_file_has_no_warnings() {
        let config = Config::parse("[ui]\ntheme = \"light\"\nitems_per_page = 100\n").unwrap();
        assert!(config.warnings.is_empty(), "{:?}", config.warnings);
    }

    /// Every key the structs define is in `KNOWN_KEYS`, so a new setting cannot
    /// be flagged as a typo.
    #[test]
    fn known_keys_cover_every_field() {
        let mut config = Config::default();
        config.auth.api_key = Some("k".into());
        config.auth.oauth_client_id = Some("id".into());
        config.auth.oauth_client_secret = Some("secret".into());
        config.ui.default_team = Some("ENG".into());
        config.workspaces.insert(
            "~/dev/app".into(),
            WorkspaceConfig {
                team: Some("ENG".into()),
            },
        );
        let written = toml::to_string(&config).unwrap();
        assert!(Config::parse(&written).unwrap().warnings.is_empty());
        let table: toml::Table = toml::from_str(&written).unwrap();
        for (section, known) in KNOWN_KEYS {
            let keys: Vec<&str> = if section.is_empty() {
                table.keys().map(String::as_str).collect()
            } else {
                table[*section]
                    .as_table()
                    .unwrap()
                    .keys()
                    .map(String::as_str)
                    .collect()
            };
            assert_eq!(keys.len(), known.len(), "[{section}] {keys:?}");
        }
        let entry = table["workspaces"]["~/dev/app"].as_table().unwrap();
        assert_eq!(entry.len(), WORKSPACE_KEYS.len(), "{entry:?}");
    }
}
