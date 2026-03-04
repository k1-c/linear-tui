use anyhow::{Context, Result};
use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use rand::Rng;
use sha2::{Digest, Sha256};

use super::server::start_callback_server;
use super::token::{OAuthTokens, TokenStore};

const AUTHORIZE_URL: &str = "https://linear.app/oauth/authorize";
const TOKEN_URL: &str = "https://api.linear.app/oauth/token";

// Users must register their own OAuth app at https://linear.app/settings/api
// and set these values via environment variables or config.
fn client_id() -> Result<String> {
    std::env::var("LINEAR_CLIENT_ID")
        .context("LINEAR_CLIENT_ID environment variable not set. Register an OAuth app at https://linear.app/settings/api")
}

fn client_secret() -> Result<String> {
    std::env::var("LINEAR_CLIENT_SECRET")
        .context("LINEAR_CLIENT_SECRET environment variable not set")
}

fn generate_code_verifier() -> String {
    let random_bytes: Vec<u8> = (0..32).map(|_| rand::rng().random::<u8>()).collect();
    URL_SAFE_NO_PAD.encode(&random_bytes)
}

fn generate_code_challenge(verifier: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(verifier.as_bytes());
    URL_SAFE_NO_PAD.encode(hasher.finalize())
}

fn generate_state() -> String {
    let random_bytes: Vec<u8> = (0..16).map(|_| rand::rng().random::<u8>()).collect();
    URL_SAFE_NO_PAD.encode(&random_bytes)
}

/// Run the full OAuth2 + PKCE login flow.
pub async fn login(token_store: &TokenStore) -> Result<()> {
    let client_id = client_id()?;
    let code_verifier = generate_code_verifier();
    let code_challenge = generate_code_challenge(&code_verifier);
    let state = generate_state();

    // Start local callback server
    let (port, code_rx) = start_callback_server(state.clone()).await?;
    let redirect_uri = format!("http://localhost:{port}/callback");

    // Build authorization URL
    let auth_url = format!(
        "{AUTHORIZE_URL}?client_id={client_id}&response_type=code&redirect_uri={redirect_uri}&scope=read,write&state={state}&code_challenge={code_challenge}&code_challenge_method=S256",
    );

    println!("Opening browser for Linear authentication...");
    open::that(&auth_url).context("Failed to open browser")?;
    println!("Waiting for authorization...");

    // Wait for the callback
    let code = code_rx
        .await
        .context("Failed to receive authorization code")?;

    // Exchange code for tokens
    let tokens = exchange_code(&code, &code_verifier, &redirect_uri).await?;
    token_store.save(&tokens)?;

    println!("Authentication successful!");
    Ok(())
}

async fn exchange_code(code: &str, code_verifier: &str, redirect_uri: &str) -> Result<OAuthTokens> {
    let client = reqwest::Client::new();
    let resp = client
        .post(TOKEN_URL)
        .form(&[
            ("grant_type", "authorization_code"),
            ("client_id", &client_id()?),
            ("client_secret", &client_secret()?),
            ("code", code),
            ("redirect_uri", redirect_uri),
            ("code_verifier", code_verifier),
        ])
        .send()
        .await
        .context("Token exchange request failed")?;

    if !resp.status().is_success() {
        let body = resp.text().await.unwrap_or_default();
        anyhow::bail!("Token exchange failed: {body}");
    }

    let token_resp: TokenResponse = resp.json().await?;
    Ok(OAuthTokens::from_response(token_resp))
}

pub async fn refresh_token(refresh_token: &str) -> Result<OAuthTokens> {
    let client = reqwest::Client::new();
    let resp = client
        .post(TOKEN_URL)
        .form(&[
            ("grant_type", "refresh_token"),
            ("client_id", &client_id()?),
            ("client_secret", &client_secret()?),
            ("refresh_token", refresh_token),
        ])
        .send()
        .await
        .context("Token refresh request failed")?;

    if !resp.status().is_success() {
        let body = resp.text().await.unwrap_or_default();
        anyhow::bail!("Token refresh failed: {body}");
    }

    let token_resp: TokenResponse = resp.json().await?;
    Ok(OAuthTokens::from_response(token_resp))
}

#[derive(serde::Deserialize)]
pub struct TokenResponse {
    pub access_token: String,
    pub refresh_token: Option<String>,
    pub expires_in: u64,
}
