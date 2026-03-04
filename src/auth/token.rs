use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use super::oauth::TokenResponse;
use crate::config::Config;

#[derive(Debug, Serialize, Deserialize)]
pub struct OAuthTokens {
    pub access_token: String,
    pub refresh_token: String,
    pub expires_at: u64,
}

impl OAuthTokens {
    pub fn from_response(resp: TokenResponse) -> Self {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();
        Self {
            access_token: resp.access_token,
            refresh_token: resp.refresh_token.unwrap_or_default(),
            // Subtract 60 seconds as buffer
            expires_at: now + resp.expires_in.saturating_sub(60),
        }
    }

    pub fn is_expired(&self) -> bool {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();
        now >= self.expires_at
    }
}

pub struct TokenStore {
    path: PathBuf,
}

impl TokenStore {
    pub fn new() -> Result<Self> {
        let path = Config::config_dir()?.join("tokens.json");
        Ok(Self { path })
    }

    pub fn load(&self) -> Result<Option<OAuthTokens>> {
        if !self.path.exists() {
            return Ok(None);
        }
        let contents = fs::read_to_string(&self.path)
            .with_context(|| format!("Failed to read tokens: {}", self.path.display()))?;
        let tokens: OAuthTokens =
            serde_json::from_str(&contents).context("Failed to parse tokens")?;
        Ok(Some(tokens))
    }

    pub fn save(&self, tokens: &OAuthTokens) -> Result<()> {
        if let Some(parent) = self.path.parent() {
            fs::create_dir_all(parent)?;
        }
        let contents = serde_json::to_string_pretty(tokens)?;
        fs::write(&self.path, contents)?;
        // Restrict file permissions on Unix
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&self.path, fs::Permissions::from_mode(0o600))?;
        }
        Ok(())
    }

    pub fn clear(&self) -> Result<()> {
        if self.path.exists() {
            fs::remove_file(&self.path)?;
        }
        Ok(())
    }
}
