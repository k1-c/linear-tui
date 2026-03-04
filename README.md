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

- **Issue management** — Browse, search, filter, and mutate issues (status, priority, assignee, comments)
- **Multiple views** — Issues, My Issues, Projects, Cycles with tab-based navigation
- **Project & Cycle detail** — Drill into projects/cycles to see their issues
- **Vim-style keybindings** — `j`/`k` navigation, `/` search, `?` help
- **OAuth2 + PKCE authentication** — Secure login via browser, or use a personal API key
- **Theme support** — Default (dark), Light, and Ocean color schemes
- **Pagination** — Cursor-based infinite scrolling for large issue lists

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

| Key | Action |
| --- | --- |
| `j` / `k` | Move cursor down / up |
| `g` / `G` | Jump to first / last item |
| `Enter` | Open detail view |
| `Esc` / `q` | Back / quit |
| `1`-`4` | Switch tabs (Issues / My Issues / Projects / Cycles) |
| `s` | Change status |
| `p` | Change priority |
| `a` | Change assignee |
| `c` | Add comment (`Ctrl+Enter` to send) |
| `t` | Switch team |
| `f` / `F` | Filter / clear filters |
| `/` | Search issues |
| `r` | Reload data |
| `?` | Toggle help |

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
