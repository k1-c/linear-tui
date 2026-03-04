use anyhow::{Context, Result};
use directories::ProjectDirs;
use ratatui::style::Color;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct Config {
    #[serde(default)]
    pub auth: AuthConfig,
    #[serde(default)]
    pub ui: UiConfig,
}

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct AuthConfig {
    pub api_key: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct UiConfig {
    pub default_team: Option<String>,
    #[serde(default = "default_items_per_page")]
    pub items_per_page: u32,
    #[serde(default)]
    pub theme: ThemeName,
}

impl Default for UiConfig {
    fn default() -> Self {
        Self {
            default_team: None,
            items_per_page: default_items_per_page(),
            theme: ThemeName::default(),
        }
    }
}

fn default_items_per_page() -> u32 {
    50
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
#[derive(Debug, Clone, Copy)]
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
            accent: Color::Cyan,
            secondary: Color::Magenta,
            muted: Color::DarkGray,
            warning: Color::Yellow,
            error: Color::Red,
            success: Color::Green,
            text: Color::White,
            text_dim: Color::Gray,
            border: Color::White,
            highlight_fg: Color::White,
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
            border: Color::DarkGray,
            highlight_fg: Color::Black,
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
            border: Color::Rgb(80, 100, 130),
            highlight_fg: Color::Rgb(240, 245, 255),
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
            toml::from_str(&contents).with_context(|| "Failed to parse config")
        } else {
            Ok(Self::default())
        }
    }

    pub fn save(&self) -> Result<()> {
        let path = Self::config_path()?;
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let contents = toml::to_string_pretty(self)?;
        fs::write(&path, contents)?;
        Ok(())
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
