//! `linear-tui auth …`: signing in, and seeing which credentials are used.

use anyhow::Result;

use crate::auth;
use crate::auth::token::TokenStore;
use crate::config::Config;

use super::USAGE;

pub async fn run(args: &[String]) -> Result<()> {
    let token_store = TokenStore::new()?;

    match args.first().map(String::as_str) {
        Some("login") => {
            auth::oauth::login(&token_store).await?;
            let method = auth::resolve_auth(&token_store, None).await?;
            println!("Signed in as {}.", auth::identify(&method).await?.name);
            Ok(())
        }

        Some("status") => auth_status(&token_store).await,

        Some("token") => {
            let Some(key) = args.get(1) else {
                anyhow::bail!("Usage: linear-tui auth token <api-key>");
            };
            // Verified before it is written, so a typo fails here and not at
            // the next launch.
            let name = auth::setup::save_api_key(key).await?;
            println!("API key saved. Signed in as {name}.");
            Ok(())
        }

        Some("set-oauth") => {
            let Some(client_id) = args.get(1) else {
                anyhow::bail!("Usage: linear-tui auth set-oauth <client-id> [client-secret]");
            };
            let mut config = Config::load()?;
            config.auth.oauth_client_id = Some(client_id.clone());
            config.auth.oauth_client_secret = args.get(2).cloned();
            config.save()?;

            println!("Logins will now use your application ({client_id}).");
            println!("Register these redirect URIs on it:");
            for port in auth::oauth::callback_ports() {
                println!("  http://localhost:{port}/callback");
            }
            Ok(())
        }

        Some("logout") => {
            let clear_key = args.iter().any(|arg| arg == "--all");
            token_store.clear()?;

            let mut config = Config::load()?;
            if clear_key {
                config.auth.api_key = None;
                config.save()?;
                println!("Logged out. Stored token and API key removed.");
            } else if config.auth.api_key.is_some() {
                println!(
                    "Signed out of OAuth. An API key is still set in config.toml — \
                     `linear-tui auth logout --all` removes that too."
                );
            } else {
                println!("Logged out.");
            }
            Ok(())
        }

        _ => {
            print!("{USAGE}");
            Ok(())
        }
    }
}

async fn auth_status(token_store: &TokenStore) -> Result<()> {
    let config = Config::load()?;

    println!("Application  {}", auth::oauth::application_summary()?);
    match token_store.load()? {
        Some(tokens) => {
            let remaining = tokens.seconds_until_expiry();
            if remaining > 0 {
                println!("OAuth token  valid for {}", format_duration(remaining));
            } else {
                println!(
                    "OAuth token  expired {} ago, refreshes on next use",
                    format_duration(-remaining)
                );
            }
            println!("Stored in    {}", token_store.path().display());
        }
        None => println!("OAuth token  none"),
    }
    println!(
        "API key      {}",
        match config.auth.api_key {
            Some(_) => "set in config.toml",
            None => "none",
        }
    );

    println!();
    match auth::resolve_auth(token_store, config.auth.api_key.as_deref()).await {
        Ok(method) => match auth::identify(&method).await {
            Ok(viewer) => println!("Signed in as {} via {}.", viewer.name, method.label()),
            Err(e) => println!("Stored credentials were rejected by the API: {e}"),
        },
        Err(e) => println!("{e}"),
    }
    Ok(())
}

fn format_duration(seconds: i64) -> String {
    match seconds {
        s if s >= 3600 => format!("{}h {}m", s / 3600, (s % 3600) / 60),
        s if s >= 60 => format!("{}m", s / 60),
        s => format!("{s}s"),
    }
}
