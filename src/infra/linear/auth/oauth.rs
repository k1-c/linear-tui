use std::time::Duration;

use anyhow::{Context, Result};
use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use rand::Rng;
use sha2::{Digest, Sha256};

use super::server::{CALLBACK_PORTS, start_callback_server};
use super::token::OAuthTokens;

const AUTHORIZE_URL: &str = "https://linear.app/oauth/authorize";
const TOKEN_URL: &str = "https://api.linear.app/oauth/token";

/// linear-tui's own Linear application, registered as `k1-c/tui`.
///
/// Linear's PKCE flow makes `client_secret` optional, so this ships as a public
/// client: only the id is baked in, and nothing secret travels in the binary.
/// Anyone who would rather authorize against their own application can override
/// it with `LINEAR_CLIENT_ID` or `auth.oauth_client_id`.
const DEFAULT_CLIENT_ID: &str = "10a4dc40b91ad9015b7b95cf703a54e3";

const SCOPES: &str = "read,write";

/// Show the consent screen every time. Without it, Linear skips the screen
/// once the application has been approved and authorizes whichever workspace
/// the browser happens to have open, leaving no chance to pick another.
const PROMPT: &str = "consent";

/// How long `auth login` waits for the browser before giving up. Without a
/// limit, a tab closed without answering would leave the command hanging.
const LOGIN_TIMEOUT: Duration = Duration::from_secs(5 * 60);

/// Upper bound on one call to the token endpoint.
const TOKEN_TIMEOUT: Duration = Duration::from_secs(30);

fn client_id() -> Result<String> {
    if let Ok(id) = std::env::var("LINEAR_CLIENT_ID") {
        return Ok(id);
    }
    let config = crate::config::Config::load()?;
    Ok(config
        .auth
        .oauth_client_id
        .unwrap_or_else(|| DEFAULT_CLIENT_ID.to_string()))
}

/// A secret is only involved when the user brought their own application —
/// the bundled client is public and sends none.
fn client_secret() -> Result<Option<String>> {
    if let Ok(secret) = std::env::var("LINEAR_CLIENT_SECRET") {
        return Ok(Some(secret));
    }
    Ok(crate::config::Config::load()?.auth.oauth_client_secret)
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

/// Percent-encode everything outside the unreserved set, so a value survives
/// being embedded in the authorize URL's query string.
fn percent_encode(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    for byte in value.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'.' | b'_' | b'~' => {
                out.push(byte as char);
            }
            _ => out.push_str(&format!("%{byte:02X}")),
        }
    }
    out
}

fn authorize_url(client_id: &str, redirect_uri: &str, state: &str, code_challenge: &str) -> String {
    format!(
        "{AUTHORIZE_URL}?client_id={}&response_type=code&redirect_uri={}&scope={}&prompt={}&state={}&code_challenge={}&code_challenge_method=S256",
        percent_encode(client_id),
        percent_encode(redirect_uri),
        percent_encode(SCOPES),
        percent_encode(PROMPT),
        percent_encode(state),
        percent_encode(code_challenge),
    )
}

/// Run the full OAuth2 + PKCE login flow, returning the token Linear issued.
/// Which workspace it belongs to is only known by asking the API with it
/// ([`super::add_account`]).
pub async fn login() -> Result<OAuthTokens> {
    let client_id = client_id()?;
    let code_verifier = generate_code_verifier();
    let code_challenge = generate_code_challenge(&code_verifier);
    let state = generate_state();

    // Bind the callback port first: Linear matches the redirect URI exactly, so
    // the authorize URL cannot be built until the port is known.
    let (port, code_rx) = start_callback_server(state.clone()).await?;
    let redirect_uri = format!("http://localhost:{port}/callback");

    let auth_url = authorize_url(&client_id, &redirect_uri, &state, &code_challenge);

    tracing::info!(redirect_uri = %redirect_uri, "starting OAuth login flow");
    match open::that(&auth_url) {
        Ok(()) => {
            println!("Opening your browser to authorize linear-tui...");
            println!("Check the workspace on the consent screen before you approve.");
        }
        Err(e) => {
            tracing::warn!(error = %e, "could not open a browser");
            println!("Could not open a browser. Visit this URL to authorize:\n\n{auth_url}\n");
        }
    }
    println!("Waiting for the response on {redirect_uri}");

    let code = tokio::time::timeout(LOGIN_TIMEOUT, code_rx)
        .await
        .map_err(|_| {
            anyhow::anyhow!(
                "No answer from the browser within {} minutes. Run the login again.",
                LOGIN_TIMEOUT.as_secs() / 60
            )
        })?
        .context("Authorization finished without returning a code")?
        .map_err(|reason| anyhow::anyhow!("Authorization was cancelled ({reason})"))?;

    tracing::debug!("exchanging authorization code for tokens");
    let tokens = exchange_code(&code, &code_verifier, &redirect_uri).await?;
    tracing::info!("OAuth login successful");
    Ok(tokens)
}

async fn exchange_code(code: &str, code_verifier: &str, redirect_uri: &str) -> Result<OAuthTokens> {
    let mut form = vec![
        ("grant_type", "authorization_code".to_string()),
        ("client_id", client_id()?),
        ("code", code.to_string()),
        ("redirect_uri", redirect_uri.to_string()),
        ("code_verifier", code_verifier.to_string()),
    ];
    if let Some(secret) = client_secret()? {
        form.push(("client_secret", secret));
    }
    post_token(&form).await
}

pub async fn refresh_token(refresh_token: &str) -> Result<OAuthTokens> {
    tracing::debug!("refreshing OAuth token");
    let mut form = vec![
        ("grant_type", "refresh_token".to_string()),
        ("client_id", client_id()?),
        ("refresh_token", refresh_token.to_string()),
    ];
    if let Some(secret) = client_secret()? {
        form.push(("client_secret", secret));
    }

    let mut tokens = post_token(&form).await?;
    // A refresh response may leave the refresh token out, which means the old
    // one stays valid. Losing it here would force a full re-login next time.
    if tokens.refresh_token.is_empty() {
        tokens.refresh_token = refresh_token.to_string();
    }
    Ok(tokens)
}

async fn post_token(form: &[(&str, String)]) -> Result<OAuthTokens> {
    let client = reqwest::Client::builder()
        .timeout(TOKEN_TIMEOUT)
        .build()
        .context("Could not build the HTTP client")?;
    let resp = client
        .post(TOKEN_URL)
        .form(form)
        .send()
        .await
        .context("Could not reach Linear's token endpoint")?;

    let status = resp.status();
    if !status.is_success() {
        let body = resp.text().await.unwrap_or_default();
        tracing::error!(%status, body = %body, "token request failed");
        anyhow::bail!("Linear rejected the token request ({status}): {body}");
    }

    let token_resp: TokenResponse = resp
        .json()
        .await
        .context("Linear returned a token response that could not be parsed")?;
    Ok(OAuthTokens::from_response(token_resp))
}

/// Human-readable summary of which application the flow will authorize against,
/// for `auth status`.
pub fn application_summary() -> Result<String> {
    let id = client_id()?;
    let origin = if std::env::var("LINEAR_CLIENT_ID").is_ok() {
        "from LINEAR_CLIENT_ID"
    } else if id == DEFAULT_CLIENT_ID {
        "k1-c/tui, bundled"
    } else {
        "from config.toml"
    };
    Ok(format!("{id} ({origin})"))
}

/// The ports the callback server may listen on, for error messages and `auth status`.
pub fn callback_ports() -> &'static [u16] {
    &CALLBACK_PORTS
}

#[derive(serde::Deserialize)]
pub struct TokenResponse {
    pub access_token: String,
    pub refresh_token: Option<String>,
    pub expires_in: u64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn percent_encode_escapes_a_redirect_uri() {
        assert_eq!(
            percent_encode("http://localhost:53681/callback"),
            "http%3A%2F%2Flocalhost%3A53681%2Fcallback"
        );
    }

    #[test]
    fn the_authorize_url_always_asks_for_consent() {
        // Without the consent screen there is no chance to pick the workspace.
        let url = authorize_url("id", "http://localhost:1/callback", "st", "ch");
        assert!(url.contains("&prompt=consent&"), "{url}");
        assert!(url.contains("&code_challenge=ch&code_challenge_method=S256"));
    }

    #[test]
    fn percent_encode_leaves_unreserved_characters_alone() {
        assert_eq!(percent_encode("aZ0-._~"), "aZ0-._~");
    }

    #[test]
    fn code_challenge_matches_the_rfc_7636_example() {
        // RFC 7636 appendix B: this verifier hashes to this challenge.
        assert_eq!(
            generate_code_challenge("dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk"),
            "E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM"
        );
    }

    #[test]
    fn verifier_and_state_are_url_safe() {
        for value in [generate_code_verifier(), generate_state()] {
            assert!(
                value
                    .chars()
                    .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_'),
                "not URL-safe: {value}"
            );
        }
    }
}
