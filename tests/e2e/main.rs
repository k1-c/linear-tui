//! End-to-end: the real linear-tui, headless, against two real Linear test
//! workspaces, worked through `linear-tui tui …` as an agent works it.
//!
//! Each run empties workspace A and fills it with known data (`linear.rs`),
//! then every scenario launches its own instance and checks both what it
//! shows and what Linear ends up holding. Workspace B is there for the
//! scenarios about more than one workspace. Setup, and the secrets CI needs,
//! are in `docs/development.md` ("End-to-end tests").
//!
//! The scenarios are `#[ignore]`d so a plain `cargo test` never touches
//! Linear; run them with
//!
//! ```sh
//! cargo test --test e2e -- --ignored --test-threads=1
//! ```
//!
//! Every use case in `src/core/usecase/` is covered by at least one scenario,
//! named on its `Covers:` line; `tests/usecase_spec.rs` checks that. These
//! are not, for the reason given:
//!
//! Not end to end:
//! - `agent::agent_for`, `agent::jump` — need herdr and its agents
//! - `notes::salvage` — needs herdr to refuse a hand-off
//! - `issue::change_refused` — needs Linear to refuse a change linear-tui allows
//! - `team::context_failed` — needs Linear to fail a request
//! - `project::next_page`, `cycle::next_page` — need more than 100 projects or cycles
//! - `workspace::signed_in`, `workspace::open_switcher`, `workspace::pick`,
//!   `workspace::leave`, `workspace::view_on_return` — the switcher offers
//!   OAuth sign-ins, which need a browser; an API key is one workspace

mod agents;
mod browse;
mod change;
mod find;
mod linear;
mod session;
mod tui;
mod workspaces;

use std::sync::OnceLock;

use linear::{Account, Linear, Seeded};

/// Workspace A, emptied and seeded once per run.
pub fn a() -> (Account, &'static Seeded) {
    static SEEDED: OnceLock<Result<Seeded, String>> = OnceLock::new();
    let account = account("A");
    let seeded = SEEDED
        .get_or_init(|| linear::seed_a(&Linear::new(&account), &account))
        .as_ref()
        .unwrap_or_else(|e| panic!("seeding workspace A: {e}"));
    (account, seeded)
}

/// Workspace B, emptied and seeded once per run.
pub fn b() -> (Account, &'static Seeded) {
    static SEEDED: OnceLock<Result<Seeded, String>> = OnceLock::new();
    let account = account("B");
    let seeded = SEEDED
        .get_or_init(|| linear::seed_b(&Linear::new(&account), &account))
        .as_ref()
        .unwrap_or_else(|e| panic!("seeding workspace B: {e}"));
    (account, seeded)
}

fn account(name: &'static str) -> Account {
    Account::from_env(name).unwrap_or_else(|| {
        panic!("set LINEAR_E2E_API_KEY_{name} and LINEAR_E2E_WORKSPACE_{name} to run the end-to-end tests")
    })
}
