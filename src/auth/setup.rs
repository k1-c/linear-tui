use anyhow::Result;
use std::io::{self, Write};

use super::oauth;
use super::token::TokenStore;
use crate::api::types::Viewer;
use crate::config::Config;

const API_KEY_SETTINGS_URL: &str = "https://linear.app/settings/account/security";

/// First-run setup, shown when nothing is stored yet.
///
/// Runs before the alternate screen is entered, so it is plain stdin/stdout —
/// the TUI has no business starting until there is a workspace to show.
/// Returns `false` when the user backed out, which is a quit and not an error.
pub async fn run(token_store: &TokenStore) -> Result<bool> {
    println!("linear-tui is not connected to Linear yet.");
    println!();
    println!("  1  Log in through your browser  (recommended)");
    println!("  2  Paste a personal API key");
    println!("  q  Quit");
    println!();

    loop {
        match prompt("Choose 1, 2 or q: ")?.as_str() {
            "1" | "" => {
                browser_login(token_store).await?;
                return Ok(true);
            }
            "2" => return api_key_login().await,
            "q" | "quit" => {
                println!("Nothing was saved.");
                return Ok(false);
            }
            other => println!("'{other}' is not one of the options."),
        }
    }
}

async fn browser_login(token_store: &TokenStore) -> Result<()> {
    let tokens = oauth::login().await?;
    let viewer = super::add_account(token_store, tokens).await?;
    println!("{}", super::signed_in(&viewer));
    Ok(())
}

async fn api_key_login() -> Result<bool> {
    println!();
    println!("Create a key under Settings > Account > Security & access:");
    println!("  {API_KEY_SETTINGS_URL}");
    if open::that(API_KEY_SETTINGS_URL).is_ok() {
        println!("(opened in your browser)");
    }
    println!();

    loop {
        let key = prompt("Paste your API key (or q to quit): ")?;
        if key == "q" {
            println!("Nothing was saved.");
            return Ok(false);
        }
        if key.is_empty() {
            continue;
        }

        match save_api_key(&key).await {
            Ok(viewer) => {
                println!("{}", super::signed_in(&viewer));
                return Ok(true);
            }
            Err(e) => println!("That key did not work: {e}"),
        }
    }
}

/// Verify an API key against the API, then write it to the config file.
/// Returns who it belongs to.
pub async fn save_api_key(key: &str) -> Result<Viewer> {
    let viewer = super::identify(&super::AuthMethod::ApiKey(key.to_string())).await?;

    let mut config = Config::load()?;
    config.auth.api_key = Some(key.to_string());
    config.save()?;

    Ok(viewer)
}

fn prompt(label: &str) -> Result<String> {
    print!("{label}");
    io::stdout().flush()?;

    let mut line = String::new();
    if io::stdin().read_line(&mut line)? == 0 {
        anyhow::bail!("Setup cancelled — no input available.");
    }
    Ok(line.trim().to_string())
}
