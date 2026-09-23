pub mod oauth;
pub mod server;
pub mod setup;
pub mod token;

use anyhow::Result;
use token::TokenStore;

use crate::api::client::LinearClient;
use crate::api::types::Viewer;
use crate::config::Config;

pub enum AuthMethod {
    OAuth { access_token: String },
    ApiKey(String),
}

impl AuthMethod {
    /// Returns the value for the Authorization header.
    /// OAuth tokens use "Bearer <token>", API keys are sent directly.
    pub fn authorization_header(&self) -> String {
        match self {
            AuthMethod::OAuth { access_token } => format!("Bearer {access_token}"),
            AuthMethod::ApiKey(key) => key.clone(),
        }
    }

    pub fn label(&self) -> &'static str {
        match self {
            AuthMethod::OAuth { .. } => "OAuth",
            AuthMethod::ApiKey(_) => "API key",
        }
    }
}

/// Whether anything is stored to authenticate with, without touching the network.
pub fn has_credentials(token_store: &TokenStore, config: &Config) -> Result<bool> {
    Ok(token_store.load()?.is_some() || config.auth.api_key.is_some())
}

/// Resolve the auth method: stored OAuth token first, then the API key from config.
pub async fn resolve_auth(token_store: &TokenStore, api_key: Option<&str>) -> Result<AuthMethod> {
    // Try OAuth token first
    if let Some(mut tokens) = token_store.load()? {
        if tokens.is_expired() {
            if tokens.refresh_token.is_empty() {
                anyhow::bail!(
                    "Your session expired and no refresh token was stored. \
                     Run `linear-tui auth login` to sign in again."
                );
            }
            tokens = oauth::refresh_token(&tokens.refresh_token).await?;
            token_store.save(&tokens)?;
        }
        return Ok(AuthMethod::OAuth {
            access_token: tokens.access_token,
        });
    }

    // Fall back to API key
    if let Some(key) = api_key {
        return Ok(AuthMethod::ApiKey(key.to_string()));
    }

    anyhow::bail!("Not authenticated. Run `linear-tui auth login` to sign in.")
}

/// Ask the API who the credentials belong to — the only way to tell a live
/// credential from a revoked one.
pub async fn identify(auth: &AuthMethod) -> Result<Viewer> {
    LinearClient::new(auth.authorization_header())
        .viewer()
        .await
}
