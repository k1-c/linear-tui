//! Talking to Linear from a subcommand: the TUI's credentials, and its
//! request code — every call goes through [`dispatch::execute_request`], so a
//! subcommand and a key press send exactly the same query.

use anyhow::{Result, bail};

use crate::api::client::LinearClient;
use crate::auth;
use crate::auth::token::TokenStore;
use crate::config::Config;
use crate::dispatch;
use crate::message::{Message, Request};

/// A client signed in the way `linear-tui auth` set up. Never prompts: a
/// subcommand is often run by an agent with nobody at the keyboard.
pub async fn client() -> Result<Session> {
    let config = Config::load()?;
    let token_store = TokenStore::new()?;
    if !auth::has_credentials(&token_store, &config)? {
        bail!(
            "Not signed in. Run `linear-tui auth login` (or `linear-tui auth token <key>`) first."
        );
    }
    let method = auth::resolve_auth(&token_store, config.auth.api_key.as_deref()).await?;
    Ok(Session {
        client: LinearClient::new(method.into_credentials(token_store)),
        per_page: config.ui.items_per_page,
    })
}

pub struct Session {
    client: LinearClient,
    per_page: u32,
}

impl Session {
    #[cfg(test)]
    pub fn new(client: LinearClient) -> Self {
        Self {
            client,
            per_page: 50,
        }
    }

    /// Run one request; a failure becomes an error that says what failed.
    pub async fn run(&self, request: Request) -> Result<Message> {
        match dispatch::execute_request(&self.client, request, self.per_page).await {
            Message::Failed { request, error } => bail!("{}: {error}", request.failure()),
            message => Ok(message),
        }
    }
}
