<div align="center">

<img src="https://raw.githubusercontent.com/k1-c/linear-tui/main/assets/icon.png" alt="" width="120" height="120">

# linear-tui

**[Linear](https://linear.app) in your terminal, with Linear's shortcuts, and context for your coding agent.**

[![Crates.io](https://img.shields.io/crates/v/linear-tui.svg)](https://crates.io/crates/linear-tui)
[![License: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)
[![CI](https://github.com/k1-c/linear-tui/actions/workflows/ci.yml/badge.svg)](https://github.com/k1-c/linear-tui/actions/workflows/ci.yml)
[![Rust](https://img.shields.io/badge/rust-1.88%2B-orange.svg)](https://www.rust-lang.org)

<img src="https://raw.githubusercontent.com/k1-c/linear-tui/main/assets/demo.gif" alt="linear-tui browsing issues grouped by status, opening an issue with Markdown and comments, then a project, a cycle, and a saved view">

</div>

---

## Why use linear-tui

- **It works like Linear.** The sidebar, Favorites, saved
  views, grouped and collapsible lists, the issue page with rendered Markdown
  and threaded comments, plus Linear's keys: `c` to create, `s` / `p` / `a` /
  `i` to update, `g` then a letter to go places.
- **Network work stays off the UI thread.** Requests run in the background, and
  changes appear as you make them, so moving around stays responsive on a slow
  connection.
- **It shares terminal context with your coding agent.** `linear-tui context`
  tells an agent which issue you have open and which row your cursor is on;
  notes you jot while reading go to it as one prompt.
- **It is a single binary.** No application to register, no client secret, no
  OpenSSL: `linear-tui auth login` and you are in.

## Highlights

### Command palette

The command palette lists the available actions for where you are, with each
shortcut, and finds any issue, project, cycle, view, or team page by name or ID.

<img src="https://raw.githubusercontent.com/k1-c/linear-tui/main/assets/palette.gif" alt="The command palette listing commands with their shortcuts, finding an issue by a word of its title, changing status and grouping from it, and opening a favorite project by name">

### Reopens where you left off

Each repository, including each worktree of it, reopens on the page, list
settings, and issue you last had open there. Pages are remembered by Linear ID,
so a reordered sidebar does not reopen the wrong one.

<img src="https://raw.githubusercontent.com/k1-c/linear-tui/main/assets/resume.gif" alt="Opening an issue in a saved view grouped by priority, quitting, and launching linear-tui again straight back on that issue, with Esc returning to the view">

### Agent context

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
[More on agents](docs/agents.md)

### herdr integration

In [herdr](https://herdr.dev/), linear-tui opens in a pane beside your agents:
notes go to the agent next to it, issue rows show which agent is working on
them, `g w` jumps to that agent, and Ctrl+clicking a Linear link opens it in
linear-tui. [The herdr plugin](docs/herdr.md)

### Additional features

Favorites and saved views as Linear evaluates them · sub-issues nested under
their parent · Active / Backlog / All presets · mouse support · CJK-aware layout
and Markdown line breaking · copy the ID, URL, or branch name over SSH · dark,
light, and ocean themes.

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

The first run asks how to connect: through the browser, or with a personal API
key, and opens your issues. [Signing in](docs/authentication.md) covers SSH,
admin-approved workspaces, and your own OAuth application.

A few keys to begin with:

| Key | |
| --- | --- |
| `j` / `k`, `Enter`, `Esc` | Move, open, go back |
| `Ctrl+K` | Other actions, with shortcuts |
| `c` · `s` · `p` · `a` · `m` | Create · status · priority · assignee · comment |
| `/` then `Ctrl+G` | Filter the list · search all of Linear |
| `g` `m` / `g` `p` / `g` `c` / `g` `v` | My Issues / Projects / Cycles / Views |
| `?` | Shortcut reference |

## Documentation

| | |
| --- | --- |
| [Keybindings](docs/keybindings.md) | Shortcuts, the palette, the mouse, differences from Linear |
| [Configuration](docs/configuration.md) | `config.toml`, per-directory settings, themes, files |
| [Working with coding agents](docs/agents.md) | `context`, notes, working the TUI, the agent plugin, herdr |
| [Headless commands](docs/cli.md) | The CLI contract for agents and scripts, `linear-tui tui …`, `--headless` |
| [Troubleshooting](docs/troubleshooting.md) | Sign-in, clipboard, shortcuts, restores, agents |
| [Development](docs/development.md) | Building, testing, recording the demos |

The full index is [docs/README.md](docs/README.md).

## License

[MIT](LICENSE)
