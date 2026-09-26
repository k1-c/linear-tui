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

`src/` is three directories, one per layer (see [Architecture](#architecture)):
`core/` (what linear-tui is), `interface/` (the ways in), and `infra/` (the
systems it calls on). Beside them sit the entry point, the main loop that
wires the two outer layers together, and what every outer layer is handed.

- `src/main.rs` — entry point, terminal setup, the modes (terminal,
  headless) and workspace switching; `src/runtime.rs` — the main loop's
  work, step by step, against the terminal or — `--headless` — an in-memory
  screen; `src/commands.rs` — the subcommands, assembled: `Infra`, the
  `cli::Host` they run against, and `linear-tui auth …` in full (signing in
  sets up the infra itself); `src/config.rs` — config file + theme
  (`~/.config/linear-tui/config.toml`); `src/logging.rs`;
  `src/private_file.rs` — files only the user may read
- `src/core/` — no terminal, no network, no files:
  - `entity/` — what linear-tui is about: entities (with an id: `Issue`,
    `Team`, `User`, `Project`, `Cycle`, `CustomView`, `Favorite`,
    `Instance`) and value objects (`Preset`, `Priority`, `IssueFilter`,
    `Page`, `Note`, `Handoff`, the view snapshot in `snapshot.rs`), one file
    per aggregate, `ids.rs` the typed ids. They keep serde derives with
    Linear's field names, so the API decodes straight into them (see
    `docs/api-type-guide.md`)
  - `store/` — `Store`: everything Linear has told us (teams, team
    contexts, viewer, views, favorites, projects, cycles, each list's rows,
    the open issue) and the rules that keep it consistent (`patch_issue`,
    `refresh_issue`); `lists.rs` is how every paginated list is opened,
    paged, reloaded, and kept clear of stale pages (`ListOf` says what a
    list lists)
  - `usecase/` — what the user can do, one module per aggregate (`issue`,
    `project`, `cycle`, `team`, `view`, `favorite`, `user`, `notes`,
    `agent`, `instance`, `workspace`), each holding its queries and its
    changes alike and its own `Request`; `mod.rs` indexes them and combines
    the requests into `usecase::Request`, the output port. **This layer is the
    specification** — see `docs/development.md` ("The use case layer")
  - `message.rs` — `Message`, Linear's answers coming back, and how a
    failed request is worded
- `src/interface/` — the ways in:
  - `tui/` — a person in the terminal:
    - `app/` — session state and every state transition. `App` in
      `mod.rs` is `store` + `nav` (`navigation.rs`: screen, destination,
      team, way back) + `view` (`view.rs`: cursors, list shapes, popup,
      forms, sidebar, status) + `frame` (`frame.rs`: what the last frame
      drew) + `outbox` (`outbox.rs`: queued requests) + `notes`.
      Transitions live by concern: `messages.rs` (`handle_message`),
      `navigation.rs`, `popups.rs`, `actions.rs` (intents that resolve the
      target and call a use case), `cursor.rs`, `lists.rs` (list shapes,
      grouping, prefetch margins), `sidebar.rs`, `mouse.rs` (`App::click`),
      `input.rs`, `snapshot.rs` (capture the view) and `restore.rs` (reopen
      it on launch, or open the issue `linear-tui open` names), `notes.rs`,
      `agents.rs` (herdr agents on issues, `g w`), `edits.rs` (renaming
      an issue, its description in `$EDITOR`, replying to, editing, and
      deleting comments); `tests.rs` the state tests
    - `keys.rs` — keybindings (Controller): the `BINDINGS` table;
      `palette.rs` — the `Ctrl+K` command palette (Controller), with
      `fuzzy.rs` its matcher; `event.rs` — terminal events, routed
    - `ui/` — rendering (View): `sidebar`, `issue_list`, `issue_detail`,
      `view_list`, `project_list`, `project_detail`, `cycle_list`,
      `cycle_detail`, `popup`, `palette`, `new_issue`, `note`, plus
      `markdown` (wrapping Markdown renderer) and `widgets` (chips,
      width-aware truncation); `look.rs` — glyphs and colours;
      `grouping.rs` — grouping of issue lists
  - `control/` — an agent working a running TUI: `mod.rs` the channel (a
    loopback port, a token in `<pid>.control`); `screen.rs` — the screen as
    an agent reads it (`linear-tui tui screen`): the frame as text, focus,
    what is open, the keys and commands here; `notation.rs` — the key
    notation agents press keys in
  - `cli/` — the headless commands for agents (`docs/cli.md`), against
    the `Host` port in `mod.rs` (Linear, the recorded views, the clock) —
    `context.rs` renders the view snapshot, `tui.rs` works the running TUI
    (`linear-tui tui …`), `issue.rs` shows, creates, updates, comments on,
    and moves issues, and edits and deletes comments, through
    `headless.rs`'s `Linear` port, which `commands.rs` backs with
    `dispatch::execute_request`; `resolve.rs` turns the names typed into
    members, labels, projects, cycles, and parents; `args.rs` parses their
    arguments
- `src/infra/` — the systems linear-tui calls on:
  - `linear/` — Linear's GraphQL client (`client.rs`, `error.rs`;
    `decode_tests.rs` checks decoding against `tests/fixtures/`) and
    `auth/` — OAuth2 + PKCE, token storage, API key fallback
  - `dispatch.rs` — `execute_request`: carries out one `usecase::Request`
  - `editor.rs` — `$VISUAL` / `$EDITOR` on a private temporary file, for
    writing a description; `main` lends it the terminal
  - `disk/snapshot/` — view snapshots on disk (`docs/view-snapshot.md`):
    the per-workspace files under the state dir (`Shelf`, which reads them
    as `Instance`s) and the debounced writer (`Recorder`) the main loop
    drives
  - `herdr.rs` — the hand-off to the herdr plugin: writes a request to the
    outbox and invokes the plugin's `deliver` action. The only place that
    runs `HERDR_BIN_PATH`; `main` sets `App::herdr` from it. Also reads the
    plugin's `agents.json` (`AgentWatch`, polled by the main loop). A
    binding marked `.herdr_only()` neither answers nor is listed unless
    `App::herdr` is set
- `tests/` — `architecture.rs` (the layers depend inwards),
  `usecase_spec.rs` (the use case layer reads as the specification, and
  every use case has an end-to-end scenario), `e2e/` (the real binary,
  headless, against two real Linear test workspaces it empties and seeds;
  `#[ignore]`d — `mise run e2e`, and `.github/workflows/e2e.yml`; see
  `docs/development.md`), and `fixtures/` — API responses for the decoding
  tests
- `agent-plugin/` — the Claude Code / Codex plugin (`docs/agent-plugin.md`):
  a SessionStart hook and a skill, listed by `.claude-plugin/marketplace.json`
  and `.agents/plugins/marketplace.json`. Bump its `version` in both
  `plugin.json` files when it changes; release-plz does not
- `herdr-plugin/` — the herdr plugin (`docs/herdr.md`): a manifest and shell
  scripts; every herdr call lives here, never in the Rust code
- `demo/` — the GIFs in `assets/`: `seed.py` fills a throwaway workspace, one
  VHS script per feature (`demo.tape`, `palette.tape`, `resume.tape`,
  `agents.tape`), `record.sh` records them (`docs/development.md#demos`)
- `docs/` — user and contributor documentation, indexed by `docs/README.md`;
  the README is the landing page and links into it

## Architecture

**The UI thread never awaits.** Network work goes through a message loop:

```
keys/app  →  App::request(Request)  →  main loop spawns onto tokio
                                              ↓
App::handle_message(Message)  ←  mpsc channel  ←  execute_request
```

Adding an API call means adding a variant to the aggregate's `Request` in
`core/usecase/`, a `Message` variant, and an arm in `dispatch` — never an
`.await` inside the main loop, `interface/tui/`, or a use case. An inline await freezes
input and animation for the whole request.

**The layers only depend inwards** (Clean Architecture). Each is a directory
under `src/`, and `tests/architecture.rs` fails when one names a layer it
may not, or when the core uses a crate that draws, reads the terminal, or
does I/O:

```
interface/   tui · control · cli                   the ways in       ─┐ both depend on core,
infra/       linear · herdr · disk · dispatch      what it calls on  ─┘ not on each other
core/        usecase → store → entity; message     what linear-tui is: no I/O
main.rs, runtime.rs, commands.rs                  wire the ways in to what it calls on
```

- `interface/` and `infra/` do not know each other; `main.rs`,
  `runtime.rs`, and `commands.rs` connect them. What a way in needs from a
  system, it declares as a port — the subcommands' `cli::Host` and
  `cli::Linear` — and the root fills it. Inside `interface/`, `control`
  reads what `tui` draws.
- The core is always named `crate::core::…`; a bare `core::` is Rust's own
  crate.
- A use case never reads a cursor, a popup row, or a screen. Resolving "the
  issue under the cursor" is an intent on `App` (`actions.rs`, `popups.rs`);
  the intent passes ids and values to `usecase::…`, so a key, a click, a
  palette entry and a subcommand run the same code.
- A use case never performs I/O either: it returns its aggregate's
  `Request`, which `infra::dispatch` carries out.
- Use cases are grouped by the aggregate they act on — finding, reading,
  and changing an issue all live in `core/usecase/issue.rs` — never by kind of
  operation.
- `app` does not import `ui`. A cache only the renderer needs lives in
  `ui::Cache`, owned by the main loop.

Other invariants:

- `ui::draw` takes `&mut App` so renderers can write back measurements
  (`app.frame`: `detail_lines`, `list_viewport`, scroll offsets). Rendering
  must not do I/O.
- The view snapshot is written from the main loop only, after input or an
  answer has been handled and the view has rested. It holds pages by Linear ID,
  never by sidebar position; its format is a contract with other programs, so
  fields are only added, and a breaking change bumps `snapshot::VERSION`.
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

A binding is one row of `BINDINGS` in `src/interface/tui/keys.rs`: its keys, the contexts it
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
- Document the binding in `docs/keybindings.md`, and record every intentional
  deviation in its "Differences from Linear" table, with the reason.

## API Types

Follow `docs/api-type-guide.md`. Linear's introspection endpoint answers without
authentication, so schema questions can be settled directly:

```bash
curl -s https://api.linear.app/graphql -H 'Content-Type: application/json' \
  -d '{"query":"{ __type(name: \"Issue\") { fields { name } } }"}'
```

- Fields shared by every issue query live in `ISSUE_FIELDS` in
  `src/infra/linear/client.rs`. Extend that constant rather than one query's selection.
- Always add or update a fixture in `tests/fixtures/` plus a deserialization
  test, including a case where the new field is absent.

## Commits, PRs, and Releases

- Branch off `main`; never commit directly to `main`.
- Use [Conventional Commits](https://www.conventionalcommits.org/). PR titles are
  validated by `.github/workflows/pr-title.yml`, so the **PR title must be
  conventional too**.
- **Squash-merge** every PR; the repository allows nothing else. The squash
  commit takes the PR title, so each PR lands on `main` as one conventional
  commit and one CHANGELOG entry. Merge commits made release-plz drop the
  commits behind them when several PRs were merged in a row.
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

A crates.io publish cannot be undone, only yanked. The end-to-end tests run on
the release PR against the real Linear test workspaces; merge it only once
they pass.
