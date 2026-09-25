<div align="center">

<img src="https://raw.githubusercontent.com/k1-c/linear-tui/main/assets/icon.png" alt="" width="120" height="120">

# linear-tui

**[Linear](https://linear.app) in your terminal — with Linear's own shortcuts, and a coding agent that sees what you see.**

[![Crates.io](https://img.shields.io/crates/v/linear-tui.svg)](https://crates.io/crates/linear-tui)
[![License: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)
[![CI](https://github.com/k1-c/linear-tui/actions/workflows/ci.yml/badge.svg)](https://github.com/k1-c/linear-tui/actions/workflows/ci.yml)
[![Rust](https://img.shields.io/badge/rust-1.88%2B-orange.svg)](https://www.rust-lang.org)

<img src="https://raw.githubusercontent.com/k1-c/linear-tui/main/assets/demo.gif" alt="linear-tui browsing issues grouped by status, opening an issue with Markdown and comments, then a project, a cycle, and a saved view">

</div>

---

## Why linear-tui

- **It feels like Linear.** The sidebar, Favorites, saved views, grouped and
  collapsible lists, the issue page with rendered Markdown and threaded
  comments — and Linear's keys: `c` to create, `s` / `p` / `a` / `i` to update,
  `g` then a letter to go places.
- **It never waits on the network.** Every request runs off the UI thread, and
  every change shows at once, so moving around stays instant on a slow
  connection.
- **It shares your screen with your coding agent.** `linear-tui context` tells
  an agent which issue you have open and which row your cursor is on; notes you
  jot while reading go to it as one prompt.
- **It is one binary.** No application to register, no client secret, no
  OpenSSL: `linear-tui auth login` and you are in.

## Highlights

### Everything behind `Ctrl+K`

The command palette lists every action for where you are, with its shortcut, and
finds any issue, project, cycle, view, or team page by name or ID.

<img src="https://raw.githubusercontent.com/k1-c/linear-tui/main/assets/palette.gif" alt="The command palette listing commands with their shortcuts, finding an issue by a word of its title, changing status and grouping from it, and opening a favorite project by name">

### Picks up where you left off

Each repository — and every worktree of it — reopens on the page, list
settings, and issue you last had open there. Pages are remembered by Linear ID,
so a reordered sidebar never reopens the wrong one.

<img src="https://raw.githubusercontent.com/k1-c/linear-tui/main/assets/resume.gif" alt="Opening an issue in a saved view grouped by priority, quitting, and launching linear-tui again straight back on that issue, with Esc returning to the view">

### Your agent sees what you see

<img src="https://raw.githubusercontent.com/k1-c/linear-tui/main/assets/agents.gif" alt="linear-tui on the left, an agent's shell on the right: two notes jotted on an issue and the view are copied with Ctrl+S, then linear-tui context shows the open issue with the cursor, and linear-tui issue show prints it">

```sh
linear-tui context                       # the page, the open issue, the list with your cursor
linear-tui issue show ENG-42             # description, fields, and comments as Markdown
linear-tui issue comment ENG-42 -        # comment, body from stdin
linear-tui issue status ENG-42 "In Review"
```

Say "fix this one" or "look at the top three" and the agent knows which you
mean. With the [agent plugin](docs/agent-plugin.md), Claude Code and Codex learn
these commands on their own, without a prompt being sent. While you read, `n`
notes an issue and `Ctrl+S` sends your notes to the agent as one prompt.
[More on agents →](docs/agents.md)

### At home in herdr

In [herdr](https://herdr.dev/), linear-tui opens in a pane beside your agents:
notes go straight to the agent next to it, issue rows show which agent is
working on them, `g w` jumps to that agent, and Ctrl+clicking a Linear link
opens it in linear-tui. [The herdr plugin →](docs/herdr.md)

### And the rest

Favorites and saved views exactly as Linear evaluates them · sub-issues nested
under their parent · Active / Backlog / All presets · mouse support · CJK-aware
layout and Markdown line breaking · copy the ID, URL, or branch name over SSH ·
dark, light, and ocean themes.

## Install

```sh
cargo install linear-tui
```

Pre-built binaries for Linux, macOS (Intel and Apple Silicon), and Windows are on
the [Releases](https://github.com/k1-c/linear-tui/releases) page. To build from
a checkout, `cargo install --path .` needs only a Rust toolchain.

## Get started

```sh
linear-tui
```

The first run asks how to connect — through the browser, or with a personal API
key — and opens your issues. [Signing in](docs/authentication.md) covers SSH,
admin-approved workspaces, and your own OAuth application.

A few keys to begin with:

| Key | |
| --- | --- |
| `j` / `k`, `Enter`, `Esc` | Move, open, go back |
| `Ctrl+K` | Everything else, with its shortcut |
| `c` · `s` · `p` · `a` · `m` | Create · status · priority · assignee · comment |
| `/` then `Ctrl+G` | Filter the list · search all of Linear |
| `g` `m` / `g` `p` / `g` `c` / `g` `v` | My Issues / Projects / Cycles / Views |
| `?` | Every shortcut |

## Documentation

| | |
| --- | --- |
| [Keybindings](docs/keybindings.md) | Every shortcut, the palette, the mouse, differences from Linear |
| [Configuration](docs/configuration.md) | `config.toml`, per-directory settings, themes, files |
| [Working with coding agents](docs/agents.md) | `context`, notes, the agent plugin, herdr |
| [Headless commands](docs/cli.md) | The CLI contract for agents and scripts |
| [Troubleshooting](docs/troubleshooting.md) | Sign-in, clipboard, shortcuts, restores |
| [Development](docs/development.md) | Building, testing, recording the demos |

The full index is [docs/README.md](docs/README.md).

## License

[MIT](LICENSE)
