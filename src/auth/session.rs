//! OAuth credentials that stay valid for as long as the TUI runs.

use tokio::sync::Mutex;

use super::oauth;
use super::token::{OAuthTokens, TokenStore};
use crate::api::client::{BoxFuture, Credentials};
use crate::api::error::ApiError;
use crate::api::ids::OrganizationId;

/// An OAuth token pair that renews itself.
///
/// The access token is renewed ahead of its expiry, and again whenever Linear
/// refuses it — revoked early, or a clock that disagrees with Linear's. Each
/// renewal is written back to this workspace's account in the token store,
/// so the next launch starts from the newest pair.
pub struct OAuthSession {
    store: TokenStore,
    /// The account the pair belongs to. The current workspace may change
    /// while this session runs; the pair must still land in its own account.
    account: Option<OrganizationId>,
    tokens: Mutex<OAuthTokens>,
}

impl OAuthSession {
    pub fn new(store: TokenStore, account: Option<OrganizationId>, tokens: OAuthTokens) -> Self {
        Self {
            store,
            account,
            tokens: Mutex::new(tokens),
        }
    }

    async fn renew(&self, tokens: &mut OAuthTokens) -> Result<(), ApiError> {
        let fresh = oauth::refresh_token(&tokens.refresh_token)
            .await
            .map_err(|e| ApiError::Refresh(format!("{e:#}")))?;
        // The new pair works for this session whether or not it reaches the
        // disk; failing to save only costs a refresh at the next launch.
        let saved = fresh.clone();
        if let Err(e) = self
            .store
            .update(|accounts| accounts.set_tokens(self.account.as_ref(), saved))
        {
            tracing::warn!(error = %e, "could not store the refreshed token");
        }
        tracing::info!("OAuth token refreshed");
        *tokens = fresh;
        Ok(())
    }
}

fn bearer(tokens: &OAuthTokens) -> String {
    format!("Bearer {}", tokens.access_token)
}

impl Credentials for OAuthSession {
    fn authorization(&self) -> BoxFuture<'_, Result<String, ApiError>> {
        Box::pin(async move {
            let mut tokens = self.tokens.lock().await;
            if tokens.is_expired() && !tokens.refresh_token.is_empty() {
                self.renew(&mut tokens).await?;
            }
            Ok(bearer(&tokens))
        })
    }

    fn refresh<'a>(&'a self, rejected: &'a str) -> BoxFuture<'a, Result<bool, ApiError>> {
        Box::pin(async move {
            let mut tokens = self.tokens.lock().await;
            // Requests run concurrently, so several can be refused by the
            // same expired token. The first renews it; the rest only need to
            // notice it has changed. Renewing again would spend the refresh
            // token a second time.
            if bearer(&tokens) != rejected {
                return Ok(true);
            }
            if tokens.refresh_token.is_empty() {
                return Ok(false);
            }
            self.renew(&mut tokens).await?;
            Ok(true)
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn session(access: &str, refresh: &str) -> OAuthSession {
        let store = TokenStore::at(std::env::temp_dir().join(format!(
            "linear-tui-session-{}-{access}.json",
            std::process::id()
        )));
        OAuthSession::new(
            store,
            None,
            OAuthTokens {
                access_token: access.into(),
                refresh_token: refresh.into(),
                expires_at: u64::MAX / 2,
            },
        )
    }

    #[tokio::test]
    async fn a_live_token_is_sent_as_a_bearer_header() {
        assert_eq!(
            session("a1", "r1").authorization().await.unwrap(),
            "Bearer a1"
        );
    }

    #[tokio::test]
    async fn a_token_someone_else_already_renewed_is_not_renewed_again() {
        // The refused header is not the current one, so another request has
        // renewed it in the meantime — no network call is made.
        assert!(session("a2", "r1").refresh("Bearer a1").await.unwrap());
    }

    #[tokio::test]
    async fn without_a_refresh_token_there_is_nothing_to_retry_with() {
        assert!(!session("a1", "").refresh("Bearer a1").await.unwrap());
    }
}
