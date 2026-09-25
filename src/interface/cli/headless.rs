//! Talking to Linear from a subcommand: the TUI's credentials, and its
//! request code — every call goes through [`dispatch::execute_request`], so a
//! subcommand and a key press send exactly the same query.

use anyhow::{Result, bail};

use crate::config::Config;
use crate::core::message::Message;
use crate::core::usecase::Request;
use crate::infra::dispatch;
use crate::infra::linear::auth;
use crate::infra::linear::auth::token::TokenStore;
use crate::infra::linear::client::LinearClient;

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
    pub async fn run(&self, request: impl Into<Request>) -> Result<Message> {
        match dispatch::execute_request(&self.client, request.into(), self.per_page).await {
            Message::Failed { request, error } => {
                bail!("{}: {error}", crate::core::message::failure(&request))
            }
            message => Ok(message),
        }
    }
}
