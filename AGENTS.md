# linear-tui

Linear.app TUI client written in Rust, built on ratatui, crossterm, tokio, and
the Linear GraphQL API.

## Build & Check Commands

```bash
cargo fmt --all                            # format
cargo clippy --all-targets -- -D warnings  # lint
cargo test                                 # test
cargo build                                # build
cargo run                                  # run (requires auth setup first)
```

Run all four, in that order, after any implementation task.

## Project Structure

- `src/main.rs` — entry point, CLI subcommands, TUI main loop, request dispatch
- `src/message.rs` — `Request` / `Message` / `Page`, the boundary between UI and I/O
- `src/app.rs` — app state (Model) and every state transition
- `src/keys.rs` — keybindings (Controller)
- `src/event.rs` — terminal event polling
- `src/ui/` — rendering (View): `issue_list`, `issue_detail`, `project_list`,
  `project_detail`, `cycle_list`, `cycle_detail`, `popup`, `new_issue`
- `src/api/` — Linear GraphQL client and types (see `docs/api-type-guide.md`)
- `tests/fixtures/` — API response fixtures for deserialization tests
- `src/auth/` — OAuth2 + PKCE, token storage, API key fallback
- `src/config.rs` — config file + theme (`~/.config/linear-tui/config.toml`)

## Architecture

**The UI thread never awaits.** Network work goes through a message loop:

```
keys/app  →  App::request(Request)  →  main loop spawns onto tokio
                                              ↓
App::handle_message(Message)  ←  mpsc channel  ←  execute_request
```

Adding an API call means adding a `Request` variant, a `Message` variant, and an
arm in `execute_request` — never an `.await` inside the main loop, `ui/`, or
`keys.rs`. An inline await freezes input and animation for the whole request.

Other invariants:

- `ui::draw` takes `&mut App` so renderers can write back measurements
  (`detail_lines`, `list_viewport`, table offsets). Rendering must not do I/O.
- Mutations are optimistic. `App::patch_issue` updates every copy of an issue,
  including `current_issue`, and the request only confirms it. Do not trigger a
  full list reload to reflect a single-field change — it costs a round trip and
  throws away the cursor position.
- A refetch restores the selection by issue id, not by row index.
- Every paginated list has a `*_page_info` field and a `maybe_prefetch_*` helper;
  wire new lists into `App::move_selection` so they scroll infinitely too.

## Keybindings

Shortcuts mirror [Linear's own](https://linear.app/docs). Before adding or
changing one, check what Linear binds that key to.

- Where a terminal cannot deliver Linear's key (`Ctrl`+punctuation, `Ctrl+M`),
  support the original under the kitty keyboard protocol — enabled automatically
  in `TerminalGuard::enter` — and add a plain-key alias that collides with
  nothing in Linear.
- Record every intentional deviation in the README's "Differences from Linear"
  table, with the reason.

## API Types

Follow `docs/api-type-guide.md`. Linear's introspection endpoint answers without
authentication, so schema questions can be settled directly:

```bash
curl -s https://api.linear.app/graphql -H 'Content-Type: application/json' \
  -d '{"query":"{ __type(name: \"Issue\") { fields { name } } }"}'
```

- Fields shared by every issue query live in `ISSUE_FIELDS` in
  `src/api/client.rs`. Extend that constant rather than one query's selection.
- Always add or update a fixture in `tests/fixtures/` plus a deserialization
  test, including a case where the new field is absent.

## Commits, PRs, and Releases

- Branch off `main`; never commit directly to `main`.
- Use [Conventional Commits](https://www.conventionalcommits.org/). PR titles are
  validated by `.github/workflows/pr-title.yml`, so the **PR title must be
  conventional too**.
- No AI attribution in commit messages or PR bodies — no `Co-Authored-By`
  trailer, no "Generated with ..." line.

Releases are automated by [release-plz](https://release-plz.dev/):

1. Merge a PR into `main`.
2. release-plz opens a `chore: release vX.Y.Z` PR that bumps `Cargo.toml`,
   `Cargo.lock`, and `CHANGELOG.md`.
3. Merging *that* PR creates the tag, publishes to crates.io, and uploads
   binaries for linux-gnu, apple-darwin (x86_64 + aarch64), and windows-msvc.

**Never edit the version in `Cargo.toml` by hand** — release-plz owns it, and a
manual bump collides with its PR. The bump is derived from commit types: `fix` →
patch, `feat` → minor, and `!` / `BREAKING CHANGE` → minor while the crate is
still `0.x`.

A crates.io publish cannot be undone, only yanked. Run the app against a real
Linear workspace before merging a release PR.
