pub mod oauth;
pub mod server;
pub mod session;
pub mod setup;
pub mod token;

use std::sync::Arc;

use anyhow::Result;
use token::{Account, OAuthTokens, TokenStore};

use crate::api::client::{Credentials, LinearClient, StaticCredentials};
use crate::api::types::{Organization, Viewer};
use crate::config::Config;

pub enum AuthMethod {
    OAuth(Account),
    ApiKey(String),
}

impl AuthMethod {
    /// Returns the value for the Authorization header.
    /// OAuth tokens use "Bearer <token>", API keys are sent directly.
    fn authorization_header(&self) -> String {
        match self {
            AuthMethod::OAuth(account) => format!("Bearer {}", account.tokens.access_token),
            AuthMethod::ApiKey(key) => key.clone(),
        }
    }

    pub fn label(&self) -> &'static str {
        match self {
            AuthMethod::OAuth(_) => "OAuth",
            AuthMethod::ApiKey(_) => "API key",
        }
    }

    /// The workspace these credentials act in, when it is known without
    /// asking Linear. An API key only says so once the viewer is loaded.
    pub fn organization(&self) -> Option<&Organization> {
        match self {
            AuthMethod::OAuth(account) => account.organization.as_ref(),
            AuthMethod::ApiKey(_) => None,
        }
    }

    /// Credentials for a long-lived client. OAuth ones renew themselves and
    /// write the renewed pair back to `store`.
    pub fn into_credentials(self, store: TokenStore) -> Arc<dyn Credentials> {
        match self {
            AuthMethod::OAuth(account) => Arc::new(session::OAuthSession::new(
                store,
                account.key().cloned(),
                account.tokens,
            )),
            AuthMethod::ApiKey(key) => Arc::new(StaticCredentials(key)),
        }
    }
}

/// Whether anything is stored to authenticate with, without touching the network.
pub fn has_credentials(token_store: &TokenStore, config: &Config) -> Result<bool> {
    Ok(!token_store.load()?.is_empty() || config.auth.api_key.is_some())
}

/// Resolve the auth method: the current workspace's OAuth token first, then
/// the API key from config.
pub async fn resolve_auth(token_store: &TokenStore, api_key: Option<&str>) -> Result<AuthMethod> {
    if let Some(stored) = token_store.load()?.current_account() {
        let previous = stored.key().cloned();
        let mut account = stored.clone();
        let mut changed = false;
        if account.tokens.is_expired() {
            if account.tokens.refresh_token.is_empty() {
                anyhow::bail!(
                    "Your session in {} expired and no refresh token was stored. \
                     Run `linear-tui auth login` to sign in again.",
                    account.label()
                );
            }
            account.tokens = oauth::refresh_token(&account.tokens.refresh_token).await?;
            changed = true;
        }
        // A token stored before accounts existed learns its workspace once.
        // Failing to ask only postpones that to the next launch.
        if account.organization.is_none() {
            match identify(&AuthMethod::OAuth(account.clone())).await {
                Ok(viewer) => {
                    account.organization = viewer.organization;
                    account.user = Some(viewer.name);
                    changed = account.organization.is_some() || changed;
                }
                Err(e) => tracing::warn!(error = %e, "could not identify the stored token"),
            }
        }
        if changed {
            let fresh = account.clone();
            token_store.update(|accounts| {
                if previous.is_none() && fresh.organization.is_some() {
                    accounts.identify(fresh);
                } else {
                    accounts.set_tokens(previous.as_ref(), fresh.tokens);
                }
            })?;
        }
        return Ok(AuthMethod::OAuth(account));
    }

    // Fall back to API key
    if let Some(key) = api_key {
        return Ok(AuthMethod::ApiKey(key.to_string()));
    }

    anyhow::bail!("Not authenticated. Run `linear-tui auth login` to sign in.")
}

/// Keep a freshly issued token as the account for its workspace, and make
/// that workspace the current one. Returns who signed in, and where.
pub async fn add_account(token_store: &TokenStore, tokens: OAuthTokens) -> Result<Viewer> {
    let account = Account {
        organization: None,
        user: None,
        tokens,
    };
    let viewer = identify(&AuthMethod::OAuth(account.clone())).await?;
    let Some(organization) = viewer.organization.clone() else {
        anyhow::bail!("Linear did not say which workspace the new token belongs to");
    };
    token_store.update(|accounts| {
        accounts.current = Some(organization.id.clone());
        accounts.add(Account {
            organization: Some(organization),
            user: Some(viewer.name.clone()),
            ..account
        });
    })?;
    Ok(viewer)
}

/// "Signed in as Ada in Acme (acme)." — which person and which workspace the
/// credentials act as.
pub fn signed_in(viewer: &Viewer) -> String {
    match &viewer.organization {
        Some(org) => format!(
            "Signed in as {} in {} ({}).",
            viewer.name, org.name, org.url_key
        ),
        None => format!("Signed in as {}.", viewer.name),
    }
}

/// Ask the API who the credentials belong to — the only way to tell a live
/// credential from a revoked one.
pub async fn identify(auth: &AuthMethod) -> Result<Viewer> {
    Ok(LinearClient::with_header(auth.authorization_header())
        .viewer()
        .await?)
}
