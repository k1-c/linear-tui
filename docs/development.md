# Development

## Setup

The Rust toolchain is pinned in `mise.toml`, so [mise](https://mise.jdx.dev/)
is all you need to install:

```sh
git clone https://github.com/k1-c/linear-tui.git
cd linear-tui
mise install       # fetch the pinned toolchain
mise run dev       # run the TUI from source
mise run verify    # format, lint, test, build
```

`mise run fmt`, `lint`, `test`, and `build` run the steps individually. Without
mise, any stable rustup toolchain works; `cargo run`, `cargo test`, and the
rest behave the same. The minimum supported Rust version is in `Cargo.toml`
(`rust-version`) and checked in CI.

No separate TLS dependencies are needed: TLS goes through rustls, so no system
OpenSSL or `pkg-config` is involved.

Before changing code, read [AGENTS.md](../AGENTS.md): the project map, the
architecture invariants (the UI thread never awaits; the layers only depend
inwards), the keybinding policy, and the commit and release flow. Behaviour
is specified in the use case layer, below. API types
follow [api-type-guide.md](api-type-guide.md).

## Checking a change by using it

After a change to what linear-tui shows or does, work the real program, not
only the tests. An agent does this the way a person would, through the
control channel:

```sh
cargo build
LINEAR_TUI_STATE_DIR=$(mktemp -d) ./target/debug/linear-tui --headless &   # or `mise run dev` in another terminal
./target/debug/linear-tui tui screen
./target/debug/linear-tui tui press "g m"
./target/debug/linear-tui tui run "Change status"
./target/debug/linear-tui tui quit
```

Use the same `LINEAR_TUI_STATE_DIR` for every command, so they find the
instance and it does not replace the view your own linear-tui reopens.
The instance acts on the Linear account you are signed in to: keys and
commands change issues for real, so change nothing you did not mean to.
The commands are in [cli.md](cli.md#linear-tui-tui-command---json---workspace-path).

## The use case layer

`src/usecase/` is the specification of linear-tui: what a person or an agent
can do, and the rules each thing follows. It is written to be read as such,
by people and by agents working on the code.

**Structure.** One module per aggregate (`issue`, `project`, `cycle`, `team`,
`view`, `favorite`, `user`, `notes`, `agent`, `instance`). Everything done to
an aggregate lives in its module — finding, reading, and changing an issue
are all in `issue.rs`. Never group use cases by kind of operation ("browse",
"search", "mutations"). `usecase/mod.rs` opens with a table of the modules.

**A use case** is a top-level `pub fn` taking the `Store` and explicit
arguments — which issue, which value — and returning its aggregate's
`Request` (or `Open` for a list, `Result<_, Refusal>` when it can decline, a
plain value for a rule). It never reads a cursor, a popup, or a screen, and
never performs I/O.

**Its doc comment is the specification.** It opens with the use case's name
in bold, as the user would say it, with the key when there is one; then its
rules in plain sentences — what happens, what is refused and why, what is
left alone:

```rust
/// **Browse a team's issues** in one of Linear's slices: Active, Backlog,
/// or All.
///
/// Linear slices the list, so a team's Active issues are all of them, not
/// the active ones among the latest page. A list already holding that slice
/// is not fetched again. Moving to another slice of the same team keeps the
/// rows on screen until the new slice lands; moving to another team drops
/// them.
pub fn open_team_issues(store: &mut Store, team_id: TeamId, preset: Preset) -> Open {
```

Write for someone who knows Linear but not this code: say "the issue shows
the new state everywhere at once", not "patches every copy in the store".

**The module's `//!` summary** is its table of contents: it links every use
case in the module.

**Its tests state the rules, one per test.** A `///` sentence above each
test says the rule; the test's name says it again as a sentence
(`a_status_change_shows_everywhere_at_once`, never `test_set_status`). Cover
what the doc comment promises, including every refusal and every "left
alone". Build the store a test needs in a small helper with a doc comment
saying what it holds.

`tests/usecase_spec.rs` enforces the shape: the table lists every module,
each summary links its use cases, each use case opens with its bold name and
has a test, and each test has its rule and a sentence for a name. Whether the
words say the right thing is for review.

Around the use cases:

- `entity/` holds what they act on; `store/` keeps what Linear has told us
  consistent (including how every paginated list pages, in `lists.rs` —
  mechanism, not a use case).
- `interface/app/` resolves intents ("the issue under the cursor") and calls
  a use case; it keeps no rule of its own beyond what the screen needs.
- An adapter (`dispatch`) carries out the returned request.

## Tests

`cargo test` runs everything offline:

| Layer | What is tested | Where |
| --- | --- | --- |
| use cases | every rule of every use case: the specification | `src/usecase/*.rs` |
| store, entity | consistency (every copy patched, pages merged, stale pages dropped), value rules | `src/store/`, `src/entity/` |
| interface | intents and state transitions, rendering against ratatui's `TestBackend`, key and palette dispatch | `src/interface/` |
| adapters | API decoding against `tests/fixtures/`, requests against a `wiremock` server, the CLI's output, snapshot files | `src/adapter/` |
| architecture | the layers depend inwards | `tests/architecture.rs` |
| specification | the use case layer's shape | `tests/usecase_spec.rs` |

Spec coverage is measured on the use case layer:

```sh
mise run coverage        # line coverage by file, the use case layer first
mise run coverage:html   # a browsable report in target/llvm-cov/html
```

Keep `src/usecase/` near full coverage; a line no test reaches is a rule
nobody wrote down.

herdr-only bindings follow `App::herdr`, which `main` sets from
`HERDR_BIN_PATH`; tests build an `App` outside herdr unless they set it, so
the environment they run in does not matter.

The shell scripts of the plugins are checked with
`shellcheck -x -P SCRIPTDIR herdr-plugin/*.sh agent-plugin/hooks/*.sh`, and the
agent plugin with `claude plugin validate .` and
`claude plugin validate agent-plugin`.

## Documentation

| When you change | Update |
| --- | --- |
| what a user or an agent can do | the use case in `src/usecase/<aggregate>.rs`: its doc comment and its tests (see [The use case layer](#the-use-case-layer)) |
| a keybinding | the `BINDINGS` row in `src/interface/keys.rs`, and [keybindings.md](keybindings.md) (with the "Differences from Linear" table if it departs from Linear) |
| a `config.toml` key | `KNOWN_KEYS` in `src/config.rs`, and [configuration.md](configuration.md) |
| a subcommand or its output | [cli.md](cli.md), a contract for agents and scripts |
| the view snapshot | [view-snapshot.md](view-snapshot.md), `snapshot::VERSION` for a breaking change |
| the herdr plugin | [herdr.md](herdr.md), and `version` in `herdr-plugin/herdr-plugin.toml` |
| the agent plugin | [agent-plugin.md](agent-plugin.md), and `version` in both of its `plugin.json` |
| what users see first | the README, and a demo if the feature is worth showing |

## Demos

The GIFs in `assets/` are scripted with [VHS](https://github.com/charmbracelet/vhs),
so they can be re-recorded after a UI change. They run against a throwaway
Linear workspace filled with made-up data for a weather app, not a real one.

| Tape | GIF | Shows |
| --- | --- | --- |
| `demo/demo.tape` | `assets/demo.gif` | The tour: grouped lists, presets, an issue with Markdown and comments, a project, a cycle, a saved view, the sidebar |
| `demo/palette.tape` | `assets/palette.gif` | The command palette: commands, issue search, pickers, places |
| `demo/resume.tape` | `assets/resume.gif` | Quitting a few pages deep and relaunching where you left off |
| `demo/agents.tape` | `assets/agents.gif` | linear-tui beside an agent's shell (tmux, `demo/tmux.conf`): notes on what you read, and what the agent sees with `linear-tui context` and `issue show` |

Sign linear-tui in to the throwaway workspace (or use an API key for it), fill
it with demo data once, then record:

```sh
python3 demo/seed.py                                    # issues, projects, cycles, views
LINEAR_DEMO_TEAM=<team name> demo/record.sh             # assets/demo.gif
LINEAR_DEMO_TEAM=<team name> demo/record.sh all         # every tape
LINEAR_DEMO_API_KEY=lin_api_... demo/record.sh all      # with an API key instead
```

`record.sh` needs `vhs`, `ttyd`, and `ffmpeg` (and `tmux` for `agents.tape`),
and runs on Linux. linear-tui runs with a throwaway config and a fresh state
directory per tape, so neither your settings nor a remembered view leak into a
recording, and no tape leaves one behind. It also drops herdr's environment,
so a recording made inside herdr still shows linear-tui as it runs elsewhere. The tapes change nothing in the
workspace: pickers are closed without choosing, and notes go to the clipboard.

A new tape follows the same pattern: launch off camera (`Hide` … `Show`), wait
for text only the loaded page shows (`Wait+Screen /…/`) rather than sleeping on
API latency, and keep it under 30 seconds with one feature per GIF.

## Releases

Releases are automated by release-plz; see [AGENTS.md](../AGENTS.md#commits-prs-and-releases).
The herdr and agent plugins are installed from the repository, not from the
crate, so a change to them alone needs no release.
