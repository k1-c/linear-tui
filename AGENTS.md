# linear-tui

Linear.app TUI client written in Rust, built on ratatui, crossterm, tokio, and
the Linear GraphQL API.

## Build & Check Commands

The toolchain is pinned in `mise.toml`, so a checkout needs nothing but
[mise](https://mise.jdx.dev/):

```bash
mise install       # fetch the pinned Rust toolchain
mise run verify    # fmt, lint, test, build — run after any implementation task
mise run dev       # run the TUI (requires auth setup first)
```

`verify` is the gate before every commit. The individual steps are also
available as `mise run fmt|lint|test|build`, and underneath they are:

```bash
cargo fmt --all                            # format
cargo clippy --all-targets -- -D warnings  # lint
cargo test                                 # test
cargo build                                # build
```

Run all four, in that order, after any implementation task.

Nothing outside the toolchain is required — TLS goes through rustls, so there is
no system OpenSSL or `pkg-config` to install. Keep it that way: a dependency that
drags in `openssl-sys` puts a C toolchain back in every contributor's path.

Without mise, a rustup stable toolchain works the same; `mise.toml` only governs
local work, and CI installs its own through `dtolnay/rust-toolchain` with an
explicit `toolchain:` (`stable`, or `rust-version` from `Cargo.toml` for the
MSRV job). Every `uses:` in `.github/workflows/` is pinned to a full commit SHA
with the version in a trailing comment (`@<sha> # v4.4.0`); Dependabot keeps
the pins current, so never add one by tag or branch.

## Project Structure

- `src/main.rs` — entry point, terminal setup, TUI main loop
- `src/cli.rs` — the `linear-tui auth …` subcommands
- `src/dispatch.rs` — `execute_request`: runs one `Request` against the API
- `src/message.rs` — `Request` / `Message` / `Page`, the boundary between UI and I/O
- `src/store/` — `Store`: everything Linear has told us (teams, team
  contexts, viewer, views, favorites, projects, cycles, each issue list's
  rows, the open issue) and the rules that keep it consistent (`patch_issue`,
  `refresh_issue`, page merging)
- `src/usecase/` — what the user can do, as functions over `&mut Store` with
  explicit arguments that apply the optimistic change and return the
  `Request`; `issue.rs` holds status, priority, assignee, comment, create
- `src/app/` — session state and every state transition. `App` in `mod.rs`
  is `store` + `nav` (`navigation.rs`: screen, destination, team, way back)
  + `view` (`view.rs`: cursors, list shapes, popup, forms, sidebar, status)
  + `frame` (`frame.rs`: what the last frame drew) + `outbox` (`outbox.rs`:
  queued requests). Transitions live by concern: `messages.rs`
  (`handle_message`), `navigation.rs`, `popups.rs`, `actions.rs` (intents that
  resolve the target and call a use case), `cursor.rs`, `lists.rs` (filtering,
  grouping, prefetch), `sidebar.rs`, `mouse.rs` (`App::click`), `input.rs`;
  `tests.rs` the state tests
- `src/grouping.rs` — Active/Backlog/All presets and grouping of issue lists
- `src/keys.rs` — keybindings (Controller): the `BINDINGS` table
- `src/palette.rs` — the `Ctrl+K` command palette (Controller): lists the
  `BINDINGS` rows with a `Command` and runs them; `src/fuzzy.rs` its matcher
- `src/event.rs` — terminal event polling
- `src/ui/` — rendering (View): `sidebar`, `issue_list`, `issue_detail`,
  `view_list`, `project_list`, `project_detail`, `cycle_list`, `cycle_detail`,
  `popup`, `palette`, `new_issue`, plus `markdown` (wrapping Markdown renderer) and
  `widgets` (glyphs, chips, width-aware truncation)
- `src/api/` — Linear GraphQL client and types (see `docs/api-type-guide.md`)
- `tests/fixtures/` — API response fixtures for deserialization tests
- `src/auth/` — OAuth2 + PKCE, token storage, API key fallback
- `src/config.rs` — config file + theme (`~/.config/linear-tui/config.toml`)
- `demo/` — the README GIF: `seed.py` fills a throwaway workspace, `demo.tape`
  is the VHS script, `record.sh` records it into `assets/demo.gif`

## Architecture

**The UI thread never awaits.** Network work goes through a message loop:

```
keys/app  →  App::request(Request)  →  main loop spawns onto tokio
                                              ↓
App::handle_message(Message)  ←  mpsc channel  ←  execute_request
```

Adding an API call means adding a `Request` variant, a `Message` variant, and an
arm in `dispatch::run_request` — never an `.await` inside the main loop, `ui/`, or
`keys.rs`. An inline await freezes input and animation for the whole request.

The layers only depend downwards:

```
keys.rs · palette.rs · App::click · (cli)   input: what the user asked for
app/                                    intents resolve which issue / value
usecase/                                the operation, with explicit arguments
store/                                  the data, kept consistent
dispatch · api                          Linear
```

- A use case never reads a cursor, a popup row, or a screen. Resolving "the
  issue under the cursor" is an intent on `App` (`actions.rs`, `popups.rs`);
  the intent passes ids and values to `usecase::…`, so a key, a click, a
  palette entry and a subcommand run the same code.
- `store/` and `usecase/` import neither `app` nor `ui`, and no ratatui.
- `app` does not import `ui`. A cache only the renderer needs lives in
  `ui::Cache`, owned by the main loop.

Other invariants:

- `ui::draw` takes `&mut App` so renderers can write back measurements
  (`app.frame`: `detail_lines`, `list_viewport`, scroll offsets). Rendering
  must not do I/O.
- Mutations are optimistic. `Store::patch_issue` updates every copy of an
  issue, including `current_issue`, and the request only confirms it. Do not trigger a
  full list reload to reflect a single-field change — it costs a round trip and
  throws away the cursor position.
- A refetch restores the selection by issue id, not by row index.
- Every paginated list is a `store::Rows` (items, `page_info`, `loaded`) and
  is prefetched from `App::move_selection`; a new issue list becomes an
  `IssueSource` variant and gets grouping, prefetch, and selection for free.
  A cursor is requested at most once (`outbox.prefetched`), because the next
  page is still in flight while the user keeps scrolling.
- Renderers record what they drew in `app.frame` (`list_rows`, `row_targets`,
  `sidebar_rows`, `chip_areas`, `popup_area`) so a click is hit-tested against
  the last frame.
  A new clickable element records its area the same way and is routed in
  `App::click`.
- Measure text by display width (`unicode-width`), never by `len()` or char
  count: CJK characters take two cells.

## Keybindings

Shortcuts mirror [Linear's own](https://linear.app/docs). Before adding or
changing one, check what Linear binds that key to.

A binding is one row of `BINDINGS` in `src/keys.rs`: its keys, the contexts it
applies in, its action, its help-overlay row, its status-bar hint, and its
command-palette entry (`.command(title, keywords)`). Dispatch, the status bar,
the help overlay, and the palette all read that table, so a new binding, hint,
or palette entry goes there, not into `ui/`. Tests reject two bindings claiming
one key in the same context, and an action in the help overlay without a
palette entry.

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
