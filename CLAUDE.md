# linear-tui

Linear.app TUI client written in Rust.

## Build & Check Commands

```bash
cargo fmt --all          # format
cargo clippy --all-targets -- -D warnings  # lint
cargo build              # build
cargo run                # run (requires auth setup first)
```

## Project Structure

- `src/main.rs` - Entry point, CLI subcommands, TUI main loop
- `src/app.rs` - App state (Model)
- `src/keys.rs` - Keybindings (Controller)
- `src/event.rs` - Event polling
- `src/ui/` - Rendering (View): issue_list, issue_detail, popup, project_list, project_detail, cycle_list, cycle_detail
- `src/api/` - Linear GraphQL client and types
- `src/auth/` - OAuth2 + PKCE, token storage, API key fallback
- `src/config.rs` - Config file + theme (~/.config/linear-tui/config.toml)

## Rules

- After completing any implementation task, ALWAYS run `/verify` before marking it done.
- When committing, ALWAYS use the `conventional-commit` skill. Never create commits manually with `git commit`.
- When you notice recurring patterns, user corrections, or conventions worth codifying, use `/skill-creator` to create a new skill.
- For ownership/borrow/lifetime errors (E0382, E0597 etc.), use the `m01-ownership` skill.
- For concurrency/async issues (Send/Sync, tokio, deadlock), use the `m07-concurrency` skill.
- For error handling design (Result, anyhow, thiserror), use the `m06-error-handling` skill.
- For performance concerns, use the `m10-performance` skill.
- For unsafe code or FFI, use the `unsafe-checker` skill.
- For Rust version/crate info lookups, use the `rust-learner` skill.
- For code navigation (go-to-definition, find references), use the `rust-code-navigator` skill.
- For refactoring (rename, extract, move), use the `rust-refactor-helper` skill.
- For code style and naming conventions, use the `coding-guidelines` skill.
