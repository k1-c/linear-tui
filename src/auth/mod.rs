pub mod oauth;
pub mod server;
pub mod token;

use anyhow::Result;
use token::TokenStore;

pub enum AuthMethod {
    OAuth { access_token: String },
    ApiKey(String),
}

impl AuthMethod {
    pub fn bearer_token(&self) -> &str {
        match self {
            AuthMethod::OAuth { access_token } => access_token,
            AuthMethod::ApiKey(key) => key,
        }
    }
}

/// Resolve the auth method: try stored OAuth token first, then API key from config.
pub async fn resolve_auth(token_store: &TokenStore, api_key: Option<&str>) -> Result<AuthMethod> {
    // Try OAuth token first
    if let Some(mut tokens) = token_store.load()? {
        if tokens.is_expired() {
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

    anyhow::bail!("Not authenticated. Run `linear-tui auth login` or set api_key in config.toml")
}
