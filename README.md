<div align="center">

# linear-tui

**A TUI client for [Linear.app](https://linear.app) — manage issues, projects, and cycles from your terminal.**

[![Crates.io](https://img.shields.io/crates/v/linear-tui.svg)](https://crates.io/crates/linear-tui)
[![License: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)
[![CI](https://github.com/k1-c/linear-tui/actions/workflows/ci.yml/badge.svg)](https://github.com/k1-c/linear-tui/actions/workflows/ci.yml)
[![Rust](https://img.shields.io/badge/rust-1.85%2B-orange.svg)](https://www.rust-lang.org)

Built with [ratatui](https://ratatui.rs/) and the Linear GraphQL API.

</div>

---

## Features

- **Issue management** — Browse, search, filter, create, and mutate issues (status, priority, assignee, comments)
- **Multiple views** — Issues, My Issues, Projects, Cycles with tab-based navigation
- **Project & Cycle detail** — Drill into projects/cycles to see their issues
- **Never blocks** — Every API call runs off the UI thread, so navigation and input stay responsive while data loads
- **Search** — Filter as you type locally, or `Ctrl+G` to search all of Linear
- **Open & copy** — Jump to the issue in your browser, or copy its identifier, URL, or suggested branch name
- **Linear's own keybindings** — `c` to create, `s`/`p`/`a`/`i` to update, `g`+key to switch views, plus vim-style `j`/`k` and mouse scrolling
- **OAuth2 + PKCE authentication** — Secure login via browser, or use a personal API key
- **Theme support** — Default (dark), Light, and Ocean color schemes
- **Pagination** — Cursor-based infinite scrolling across every list

## Installation

### From crates.io

```sh
cargo install linear-tui
```

### From GitHub Releases

Pre-built binaries are available for Linux, macOS (Intel/Apple Silicon), and Windows on the [Releases](https://github.com/k1-c/linear-tui/releases) page.

### From source

```sh
git clone https://github.com/k1-c/linear-tui.git
cd linear-tui
cargo install --path .
```

## Getting Started

### 1. Authenticate

**OAuth2 (recommended)**

Register an OAuth application at [Linear Settings > API](https://linear.app/settings/api), then save your credentials:

```sh
linear-tui auth set-oauth <client-id> <client-secret>
linear-tui auth login
```

Tokens are stored locally and refreshed automatically.

**Personal API Key**

Generate a key at [Linear Settings > API](https://linear.app/settings/api), then:

```sh
linear-tui auth token <your-api-key>
```

### 2. Launch

```sh
linear-tui
```

## Keybindings

Shortcuts follow [Linear's own keyboard shortcuts](https://linear.app/docs) wherever
a terminal allows it, so muscle memory carries over from the web app.

### Navigation

| Key | Action |
| --- | --- |
| `j` / `k`, `↓` / `↑` | Move cursor down / up |
| `g` `g` / `G` | Jump to first / last item |
| `Enter`, `Space` | Open the issue (Linear: peek) |
| `Esc` | Back / close |
| `g` `e` / `g` `m` / `g` `p` / `g` `c` | Go to All Issues / My Issues / Projects / Cycles |
| `1`-`4` | Same views, by tab number |
| `Ctrl+d` / `Ctrl+u` | Half page down / up |
| `PgDn` / `PgUp` | Full page down / up |
| mouse wheel | Scroll lists and issue bodies |

### Issue actions

| Key | Action |
| --- | --- |
| `c` | Create a new issue (`Ctrl+Enter` to save) |
| `s` | Change status |
| `p` | Change priority |
| `Shift+1` … `Shift+4`, `Shift+0` | Set priority directly (Urgent → Low, None) |
| `a` | Assign to someone |
| `i` | Assign to me |
| `m` | Add a comment (`Ctrl+Enter` to send) |

### Copy and open

| Key | Action |
| --- | --- |
| `Ctrl+.` or `y` | Copy the issue ID |
| `Ctrl+Shift+,` or `Y` | Copy the issue URL |
| `Ctrl+Shift+.` or `b` | Copy the suggested git branch name |
| `o` | Open the issue (or project) on linear.app |

### Search and filtering

| Key | Action |
| --- | --- |
| `/` | Filter the visible list as you type |
| `Ctrl+G` | Search all of Linear for the current query |
| `f` / `Shift+F` | Filter by status & priority / clear filters |
| `t` | Switch team |
| `Ctrl+r`, `F5` | Refresh the current view |
| `?` | Show all shortcuts |
| `q`, `Ctrl+c` | Quit |

Text fields (search, comments, the new-issue form) accept readline-style
editing: `Ctrl+w`, `Ctrl+u`, `Ctrl+k`, `Ctrl+a`, `Ctrl+e`, and arrow keys.

### Differences from Linear

A terminal cannot deliver every shortcut the web app uses, and a few of Linear's
actions have no meaning here. Where they differ:

| Linear | linear-tui | Why |
| --- | --- | --- |
| `Ctrl+.`, `Ctrl+Shift+.`, `Ctrl+Shift+,`, `Ctrl+M` | also `y`, `b`, `Y`, `m` | Legacy terminals cannot distinguish `Ctrl`+punctuation, and `Ctrl+M` *is* `Enter`. The originals work in terminals supporting the [kitty keyboard protocol](https://sw.kovidgoyal.net/kitty/keyboard-protocol/) (kitty, Ghostty, WezTerm, foot, Alacritty), which is enabled automatically when available. |
| `r` — rename issue | *(unbound)* | Renaming isn't supported yet; refreshing uses the terminal's `Ctrl+r` instead. |
| `Ctrl+d` — set due date | half page down | The scrolling convention wins in a terminal. |
| — | `o`, `t`, `q` | Open in browser, switch team, and quit have no web-app equivalent. |

Copying uses the OSC 52 terminal escape, so it works over SSH. If nothing
lands on your clipboard, enable it in your terminal — under tmux that means
`set -g set-clipboard on`.

## Configuration

Config file: `~/.config/linear-tui/config.toml`

```toml
[auth]
# OAuth tokens are managed automatically via `linear-tui auth login`
# To use a personal API key instead:
# api_key = "lin_api_xxxxx"

[ui]
default_team = "Core"       # Auto-select this team on startup
items_per_page = 50          # Issues per page (pagination)
theme = "default"            # "default" | "light" | "ocean"
```

### Themes

| Theme | Description |
| --- | --- |
| `default` | Dark theme with cyan accents |
| `light` | Light background with blue accents |
| `ocean` | Dark blue palette with soft colors |

## License

[MIT](LICENSE)
