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
mise, any stable rustup toolchain works — `cargo run`, `cargo test`, and the
rest behave the same. The minimum supported Rust version is in `Cargo.toml`
(`rust-version`) and checked in CI.

There is nothing else to install: TLS goes through rustls, so no system OpenSSL
or `pkg-config` is involved.

Before changing code, read [AGENTS.md](../AGENTS.md): the project map, the
architecture invariants (the UI thread never awaits; the layers only depend
downwards), the keybinding policy, and the commit and release flow. API types
follow [api-type-guide.md](api-type-guide.md).

## Tests

`cargo test` runs everything offline: state transitions, rendering against
ratatui's `TestBackend`, API decoding against `tests/fixtures/`, and requests
against a `wiremock` server.

Inside herdr, `HERDR_BIN_PATH` is set and herdr-only bindings are shown; CI has
no herdr. Run the tests once without it before pushing a change to bindings:

```sh
env -u HERDR_BIN_PATH cargo test
```

The shell scripts of the plugins are checked with
`shellcheck -x -P SCRIPTDIR herdr-plugin/*.sh agent-plugin/hooks/*.sh`, and the
agent plugin with `claude plugin validate .` and
`claude plugin validate agent-plugin`.

## Documentation

| When you change | Update |
| --- | --- |
| a keybinding | the `BINDINGS` row in `src/keys.rs`, and [keybindings.md](keybindings.md) (with the "Differences from Linear" table if it departs from Linear) |
| a `config.toml` key | `KNOWN_KEYS` in `src/config.rs`, and [configuration.md](configuration.md) |
| a subcommand or its output | [cli.md](cli.md) — a contract for agents and scripts |
| the view snapshot | [view-snapshot.md](view-snapshot.md), `snapshot::VERSION` for a breaking change |
| the herdr plugin | [herdr.md](herdr.md), and `version` in `herdr-plugin/herdr-plugin.toml` |
| the agent plugin | [agent-plugin.md](agent-plugin.md), and `version` in both of its `plugin.json` |
| what users see first | the README, and a demo if the feature is worth showing |

## Demos

The GIFs in `assets/` are scripted with [VHS](https://github.com/charmbracelet/vhs),
so they can be re-recorded after a UI change. They run against a throwaway
Linear workspace filled with made-up data — a weather app — never a real one.

| Tape | GIF | Shows |
| --- | --- | --- |
| `demo/demo.tape` | `assets/demo.gif` | The tour: grouped lists, presets, an issue with Markdown and comments, a project, a cycle, a saved view, the sidebar |
| `demo/palette.tape` | `assets/palette.gif` | The command palette: commands, issue search, pickers, places |
| `demo/resume.tape` | `assets/resume.gif` | Quitting a few pages deep and relaunching where you left off |
| `demo/agents.tape` | `assets/agents.gif` | Notes on what you read, and what an agent sees with `linear-tui context` and `issue show` |

Sign linear-tui in to the throwaway workspace (or use an API key for it), fill
it with demo data once, then record:

```sh
python3 demo/seed.py                                    # issues, projects, cycles, views
LINEAR_DEMO_TEAM=<team name> demo/record.sh             # assets/demo.gif
LINEAR_DEMO_TEAM=<team name> demo/record.sh all         # every tape
LINEAR_DEMO_API_KEY=lin_api_... demo/record.sh all      # with an API key instead
```

`record.sh` needs `vhs`, `ttyd`, and `ffmpeg` (and `jq` for `agents.tape`), and
runs on Linux. linear-tui runs with a throwaway config and a fresh state
directory per tape, so neither your settings nor a remembered view leak into a
recording, and no tape leaves one behind. The tapes change nothing in the
workspace: pickers are closed without choosing, and notes go to the clipboard.

A new tape follows the same pattern: launch off camera (`Hide` … `Show`), wait
for text only the loaded page shows (`Wait+Screen /…/`) rather than sleeping on
API latency, and keep it under 30 seconds — one feature per GIF.

## Releases

Releases are automated by release-plz; see [AGENTS.md](../AGENTS.md#commits-prs-and-releases).
The herdr and agent plugins are installed from the repository, not from the
crate, so a change to them alone needs no release.
