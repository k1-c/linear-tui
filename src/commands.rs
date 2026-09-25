//! The subcommands, assembled: `main` hands them here, and they run against
//! the infra — each on its own, so each builds what it needs, as `main` does
//! for the TUI.
//!
//! [`Infra`] is the [`Host`] the subcommands in `crate::interface::cli` run
//! against. `linear-tui auth …` is handled here in full: signing in sets up
//! the infra itself, so there is nothing for a subcommand to ask of it.

use std::path::{Path, PathBuf};

use anyhow::{Result, bail};

use crate::config::Config;
use crate::core::entity::{Instance, Organization};
use crate::core::message::Message;
use crate::core::usecase::Request;
use crate::infra::disk::snapshot::{self, Shelf};
use crate::infra::dispatch;
use crate::infra::linear::auth;
use crate::infra::linear::auth::token::TokenStore;
use crate::infra::linear::client::LinearClient;
use crate::interface::cli::{self, Host, Linear, USAGE};

/// Run the subcommand `args` names.
pub async fn run(args: &[String]) -> Result<()> {
    match args[0].as_str() {
        "auth" => auth(&args[1..]).await,
        _ => cli::handle_subcommand(args, &Infra).await,
    }
}

/// What the subcommands run against: the credentials `linear-tui auth` set
/// up, and the view snapshots on disk.
struct Infra;

impl Host for Infra {
    type Linear = SignedIn;

    async fn linear(&self) -> Result<SignedIn> {
        let config = Config::load()?;
        let token_store = TokenStore::new()?;
        if !auth::has_credentials(&token_store, &config)? {
            bail!(
                "Not signed in. Run `linear-tui auth login` (or `linear-tui auth token <key>`) first."
            );
        }
        let method = auth::resolve_auth(&token_store, config.auth.api_key.as_deref()).await?;
        Ok(SignedIn {
            client: LinearClient::new(method.into_credentials(token_store)),
            per_page: config.ui.items_per_page,
        })
    }

    fn workspace_of(&self, cwd: &Path) -> PathBuf {
        snapshot::workspace_of(cwd)
    }

    fn instances(&self, workspace: &Path) -> Result<Vec<Instance>> {
        Ok(Shelf::new(&snapshot::state_dir()?, workspace).instances())
    }

    fn snapshot_file(&self, workspace: &Path, pid: u32) -> Result<PathBuf> {
        Ok(Shelf::new(&snapshot::state_dir()?, workspace).path_for(pid))
    }

    fn state_dir(&self) -> Result<PathBuf> {
        snapshot::state_dir()
    }

    fn acting_organization(&self) -> Option<Organization> {
        TokenStore::new()
            .ok()?
            .load()
            .ok()?
            .current_account()?
            .organization
            .clone()
    }

    fn now(&self) -> u64 {
        snapshot::unix_now()
    }
}

/// Linear, through the client the TUI uses and its request code.
struct SignedIn {
    client: LinearClient,
    per_page: u32,
}

impl Linear for SignedIn {
    async fn execute(&self, request: Request) -> Message {
        dispatch::execute_request(&self.client, request, self.per_page).await
    }
}

/// `linear-tui auth …`: signing in, and seeing which credentials are used.
async fn auth(args: &[String]) -> Result<()> {
    let token_store = TokenStore::new()?;

    match args.first().map(String::as_str) {
        Some("login") => {
            let tokens = auth::oauth::login().await?;
            let viewer = auth::add_account(&token_store, tokens).await?;
            println!("{}", auth::signed_in(&viewer));
            Ok(())
        }

        Some("status") => auth_status(&token_store).await,

        Some("list") => {
            list(&token_store)?;
            Ok(())
        }

        Some("switch") => {
            let Some(name) = args.get(1) else {
                list(&token_store)?;
                anyhow::bail!("Usage: linear-tui auth switch <workspace>");
            };
            let account = switch(&token_store, name)?;
            println!("Switched to {}.", account.label());
            Ok(())
        }

        Some("token") => {
            let Some(key) = args.get(1) else {
                anyhow::bail!("Usage: linear-tui auth token <api-key>");
            };
            // Verified before it is written, so a typo fails here and not at
            // the next launch.
            let viewer = auth::setup::save_api_key(key).await?;
            println!("API key saved. {}", auth::signed_in(&viewer));
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
            let name = args[1..].iter().find(|arg| !arg.starts_with("--"));
            let mut config = Config::load()?;

            if clear_key {
                token_store.clear()?;
                config.auth.api_key = None;
                config.save()?;
                println!("Logged out of every workspace. Stored tokens and API key removed.");
                return Ok(());
            }

            let removed = token_store.update(|accounts| {
                let key = match name {
                    Some(name) => accounts.find(name).map(|a| a.key().cloned()),
                    None => accounts.current_account().map(|a| a.key().cloned()),
                };
                let removed = key.and_then(|key| accounts.remove(key.as_ref()));
                let next = accounts.current_account().map(|a| a.label());
                (removed, next)
            })?;
            match removed {
                (Some(account), next) => {
                    println!("Logged out of {}.", account.label());
                    if let Some(next) = next {
                        println!("Now using {next}.");
                    }
                }
                (None, _) => match name {
                    Some(name) => anyhow::bail!("No workspace named '{name}' is signed in."),
                    None => println!("No OAuth token was stored."),
                },
            }
            if config.auth.api_key.is_some() && token_store.load()?.is_empty() {
                println!(
                    "An API key is still set in config.toml — \
                     `linear-tui auth logout --all` removes that too."
                );
            }
            Ok(())
        }

        _ => {
            print!("{USAGE}");
            Ok(())
        }
    }
}

/// Print every signed-in workspace, the current one marked.
fn list(token_store: &TokenStore) -> Result<()> {
    let accounts = token_store.load()?;
    if accounts.is_empty() {
        println!("No workspace is signed in through the browser. Run `linear-tui auth login`.");
        return Ok(());
    }
    for account in &accounts.accounts {
        let mark = if accounts.is_current(account) {
            "*"
        } else {
            " "
        };
        match &account.user {
            Some(user) => println!("{mark} {}  as {user}", account.label()),
            None => println!("{mark} {}", account.label()),
        }
    }
    Ok(())
}

/// Make the workspace `name` names the current one.
fn switch(token_store: &TokenStore, name: &str) -> Result<auth::token::Account> {
    token_store
        .update(|accounts| {
            let account = accounts.find(name).cloned()?;
            accounts.current = account.key().cloned();
            Some(account)
        })?
        .ok_or_else(|| {
            anyhow::anyhow!(
                "No workspace named '{name}' is signed in. \
                 `linear-tui auth list` shows them; `linear-tui auth login` adds one."
            )
        })
}

async fn auth_status(token_store: &TokenStore) -> Result<()> {
    let config = Config::load()?;
    let accounts = token_store.load()?;

    println!("Application  {}", auth::oauth::application_summary()?);
    match accounts.current_account() {
        Some(account) => {
            println!("Workspace    {}", account.label());
            let remaining = account.tokens.seconds_until_expiry();
            if remaining > 0 {
                println!("OAuth token  valid for {}", format_duration(remaining));
            } else {
                println!(
                    "OAuth token  expired {} ago, refreshes on next use",
                    format_duration(-remaining)
                );
            }
            if accounts.accounts.len() > 1 {
                println!(
                    "Workspaces   {} signed in (`linear-tui auth list`)",
                    accounts.accounts.len()
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
            Ok(viewer) => println!("{} Via {}.", auth::signed_in(&viewer), method.label()),
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
