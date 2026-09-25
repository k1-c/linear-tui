use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use super::oauth::TokenResponse;
use crate::config::Config;
use crate::entity::Organization;
use crate::entity::OrganizationId;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OAuthTokens {
    pub access_token: String,
    pub refresh_token: String,
    pub expires_at: u64,
}

impl OAuthTokens {
    pub fn from_response(resp: TokenResponse) -> Self {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();
        Self {
            access_token: resp.access_token,
            refresh_token: resp.refresh_token.unwrap_or_default(),
            // Subtract 60 seconds as buffer
            expires_at: now + resp.expires_in.saturating_sub(60),
        }
    }

    pub fn is_expired(&self) -> bool {
        self.seconds_until_expiry() <= 0
    }

    /// Seconds left on the access token; negative once it has lapsed.
    pub fn seconds_until_expiry(&self) -> i64 {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();
        self.expires_at as i64 - now as i64
    }
}

/// One workspace's credentials. A token belongs to exactly one workspace,
/// so signing in to another adds an account rather than replacing this one.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Account {
    /// `None` only for a token stored before accounts existed, until the
    /// next launch asks Linear which workspace it belongs to.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub organization: Option<Organization>,
    /// Who signed in, for `auth list`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub user: Option<String>,
    pub tokens: OAuthTokens,
}

impl Account {
    pub fn key(&self) -> Option<&OrganizationId> {
        self.organization.as_ref().map(|org| &org.id)
    }

    /// "Acme (acme)", or a placeholder for a token not yet identified.
    pub fn label(&self) -> String {
        match &self.organization {
            Some(org) => format!("{} ({})", org.name, org.url_key),
            None => "an unidentified workspace".to_string(),
        }
    }

    /// Whether `query` names this account: its URL key, name, or id.
    fn is_named(&self, query: &str) -> bool {
        self.organization.as_ref().is_some_and(|org| {
            org.url_key.eq_ignore_ascii_case(query)
                || org.name.eq_ignore_ascii_case(query)
                || org.id == query
        })
    }
}

/// Every signed-in workspace, and the one in use.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Accounts {
    /// The workspace the TUI opens and the headless commands act in.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub current: Option<OrganizationId>,
    #[serde(default)]
    pub accounts: Vec<Account>,
}

impl Accounts {
    pub fn is_empty(&self) -> bool {
        self.accounts.is_empty()
    }

    /// The account in use: the current one, or the first when the current
    /// one has been removed or was never chosen.
    pub fn current_account(&self) -> Option<&Account> {
        self.accounts
            .iter()
            .find(|a| self.current.is_some() && a.key() == self.current.as_ref())
            .or_else(|| self.accounts.first())
    }

    pub fn is_current(&self, account: &Account) -> bool {
        self.current_account()
            .is_some_and(|current| current.key() == account.key())
    }

    pub fn find(&self, query: &str) -> Option<&Account> {
        self.accounts.iter().find(|a| a.is_named(query))
    }

    /// Store a signed-in account, replacing any earlier one for the same
    /// workspace. The first account stored becomes the current one.
    pub fn add(&mut self, account: Account) {
        let key = account.key().cloned();
        match self.accounts.iter_mut().find(|a| a.key() == key.as_ref()) {
            Some(slot) => *slot = account,
            None => self.accounts.push(account),
        }
        if self.current.is_none() {
            self.current = key;
        }
    }

    /// Store what an unidentified token turned out to be.
    pub fn identify(&mut self, account: Account) {
        self.accounts.retain(|a| a.key().is_some());
        self.add(account);
    }

    /// Replace the tokens of one account, leaving the others as they are.
    pub fn set_tokens(&mut self, key: Option<&OrganizationId>, tokens: OAuthTokens) {
        if let Some(account) = self.accounts.iter_mut().find(|a| a.key() == key) {
            account.tokens = tokens;
        }
    }

    /// Forget one account. Returns it, or `None` when there was no such one.
    pub fn remove(&mut self, key: Option<&OrganizationId>) -> Option<Account> {
        let index = self.accounts.iter().position(|a| a.key() == key)?;
        let removed = self.accounts.remove(index);
        if self.current.as_ref() == key {
            self.current = self.accounts.first().and_then(|a| a.key().cloned());
        }
        Some(removed)
    }
}

/// What `tokens.json` may hold. Before accounts it was one bare token pair,
/// which reads as a single account whose workspace is not yet known.
#[derive(Deserialize)]
#[serde(untagged)]
enum OnDisk {
    // First: every field of `Accounts` has a default, so it matches anything.
    Legacy(OAuthTokens),
    Accounts(Accounts),
}

#[derive(Clone)]
pub struct TokenStore {
    path: PathBuf,
}

impl TokenStore {
    pub fn new() -> Result<Self> {
        let path = Config::config_dir()?.join("tokens.json");
        Ok(Self { path })
    }

    /// A store at an explicit path, for tests.
    #[cfg(test)]
    pub fn at(path: PathBuf) -> Self {
        Self { path }
    }

    pub fn load(&self) -> Result<Accounts> {
        if !self.path.exists() {
            return Ok(Accounts::default());
        }
        let contents = fs::read_to_string(&self.path)
            .with_context(|| format!("Failed to read tokens: {}", self.path.display()))?;
        Ok(
            match serde_json::from_str(&contents).context("Failed to parse tokens")? {
                OnDisk::Accounts(accounts) => accounts,
                OnDisk::Legacy(tokens) => Accounts {
                    current: None,
                    accounts: vec![Account {
                        organization: None,
                        user: None,
                        tokens,
                    }],
                },
            },
        )
    }

    pub fn save(&self, accounts: &Accounts) -> Result<()> {
        let contents = serde_json::to_string_pretty(accounts)?;
        crate::adapter::private_file::write(&self.path, contents.as_bytes())
    }

    /// Read, change, and write back. Several instances share the file, so a
    /// change is applied to what is on disk now, not to a copy read earlier.
    pub fn update<T>(&self, change: impl FnOnce(&mut Accounts) -> T) -> Result<T> {
        let mut accounts = self.load()?;
        let out = change(&mut accounts);
        self.save(&accounts)?;
        Ok(out)
    }

    pub fn clear(&self) -> Result<()> {
        if self.path.exists() {
            fs::remove_file(&self.path)?;
        }
        Ok(())
    }

    pub fn path(&self) -> &std::path::Path {
        &self.path
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tokens(access: &str) -> OAuthTokens {
        OAuthTokens {
            access_token: access.into(),
            refresh_token: "r".into(),
            expires_at: 0,
        }
    }

    fn account(key: &str, access: &str) -> Account {
        Account {
            organization: Some(Organization {
                id: key.into(),
                name: key.to_uppercase(),
                url_key: key.into(),
            }),
            user: None,
            tokens: tokens(access),
        }
    }

    fn store(name: &str) -> TokenStore {
        let dir =
            std::env::temp_dir().join(format!("linear-tui-tokens-{}-{name}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        TokenStore::at(dir.join("tokens.json"))
    }

    #[test]
    fn a_token_stored_before_accounts_reads_as_one_unidentified_account() {
        let store = store("legacy");
        fs::write(
            store.path(),
            r#"{ "access_token": "a", "refresh_token": "r", "expires_at": 1 }"#,
        )
        .unwrap();
        let accounts = store.load().unwrap();
        assert_eq!(accounts.accounts.len(), 1);
        let current = accounts.current_account().unwrap();
        assert!(current.organization.is_none());
        assert_eq!(current.tokens.access_token, "a");
    }

    #[test]
    fn accounts_survive_a_round_trip() {
        let store = store("round-trip");
        let mut accounts = Accounts::default();
        accounts.add(account("acme", "a1"));
        accounts.add(account("globex", "g1"));
        store.save(&accounts).unwrap();
        let loaded = store.load().unwrap();
        assert_eq!(loaded.accounts.len(), 2);
        assert_eq!(loaded.current_account().unwrap().label(), "ACME (acme)");
    }

    #[test]
    fn identifying_an_old_token_replaces_it_instead_of_adding_one() {
        let mut accounts = Accounts {
            current: None,
            accounts: vec![Account {
                organization: None,
                user: None,
                tokens: tokens("a"),
            }],
        };
        accounts.identify(account("acme", "a"));
        assert_eq!(accounts.accounts.len(), 1);
        assert_eq!(accounts.current, Some("acme".into()));
    }

    #[test]
    fn a_new_sign_in_keeps_a_token_not_yet_identified() {
        let mut accounts = Accounts {
            current: None,
            accounts: vec![Account {
                organization: None,
                user: None,
                tokens: tokens("old"),
            }],
        };
        accounts.add(account("acme", "a1"));
        assert_eq!(accounts.accounts.len(), 2);
    }

    #[test]
    fn signing_in_again_replaces_that_workspace_and_keeps_the_current_one() {
        let mut accounts = Accounts::default();
        accounts.add(account("acme", "a1"));
        accounts.add(account("globex", "g1"));
        accounts.add(account("globex", "g2"));
        assert_eq!(accounts.accounts.len(), 2);
        assert_eq!(accounts.find("GLOBEX").unwrap().tokens.access_token, "g2");
        assert_eq!(accounts.current, Some("acme".into()));
    }

    #[test]
    fn a_refresh_only_touches_its_own_account() {
        let mut accounts = Accounts::default();
        accounts.add(account("acme", "a1"));
        accounts.add(account("globex", "g1"));
        accounts.set_tokens(Some(&"globex".into()), tokens("g2"));
        assert_eq!(accounts.find("acme").unwrap().tokens.access_token, "a1");
        assert_eq!(accounts.find("globex").unwrap().tokens.access_token, "g2");
    }

    #[test]
    fn removing_the_current_account_falls_back_to_another() {
        let mut accounts = Accounts::default();
        accounts.add(account("acme", "a1"));
        accounts.add(account("globex", "g1"));
        assert!(accounts.remove(Some(&"acme".into())).is_some());
        assert_eq!(accounts.current, Some("globex".into()));
        assert_eq!(
            accounts.current_account().unwrap().label(),
            "GLOBEX (globex)"
        );
        assert!(accounts.remove(Some(&"nowhere".into())).is_none());
    }

    #[test]
    fn a_current_account_that_is_gone_falls_back_to_the_first() {
        let mut accounts = Accounts::default();
        accounts.add(account("acme", "a1"));
        accounts.current = Some("deleted".into());
        assert_eq!(accounts.current_account().unwrap().label(), "ACME (acme)");
    }
}
