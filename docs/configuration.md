# Configuration

linear-tui works without configuration. Settings live in
`~/.config/linear-tui/config.toml` (on macOS,
`~/Library/Application Support/linear-tui/config.toml`); `linear-tui paths`
prints where. A key the file does not understand, or a value out of range, is
reported in a popup when linear-tui starts rather than silently ignored.

```toml
[auth]
# OAuth tokens are managed automatically via `linear-tui auth login`.
# api_key = "lin_api_xxxxx"            # a personal API key instead
# oauth_client_id = "..."              # your own Linear application
# oauth_client_secret = "..."          # optional — PKCE does not require one

[ui]
default_team = "Core"
items_per_page = 50
theme = "default"
sidebar = true
sidebar_width = 26
group_by = "status"

[agent]
control = true                          # let agents work a running linear-tui

# Always open on a team in one directory, instead of where you left off there.
[workspaces."~/dev/storefront"]
team = "WEB"
```

## `[ui]`

| Key | Default | |
| --- | --- | --- |
| `default_team` | the first team | Team to open on, by name or key. |
| `items_per_page` | `50` | Issues fetched per page, `1`–`250` (Linear's maximum). |
| `theme` | `"default"` | `"default"`, `"light"`, or `"ocean"`. |
| `sidebar` | `true` | Show the sidebar. It hides itself on terminals narrower than 100 columns regardless. |
| `sidebar_width` | `26` | Sidebar width in columns, `18`–`48`. |
| `group_by` | `"status"` | How issue lists are grouped at first: `"status"`, `"assignee"`, `"priority"`, `"project"`, or `"none"`. `D` changes it while running. |

### Themes

| Theme | |
| --- | --- |
| `default` | Dark, with cyan accents |
| `light` | Light background, with blue accents |
| `ocean` | Dark blue, with soft colours |

Workflow states, labels, and projects keep the colours your workspace gave them
for each theme.

## `[agent]`

| Key | Default | |
| --- | --- | --- |
| `control` | `true` | Whether a running linear-tui takes commands from agents (`linear-tui tui …`, see [cli.md](cli.md)). It listens on a loopback port only, and a command must carry the token in a file only you can read. `linear-tui --headless` needs it on. |

## `[auth]`

Written by the `linear-tui auth` commands; see
[authentication.md](authentication.md). You rarely edit it by hand.

## `[workspaces."<path>"]`

Settings for one directory. The path may start with `~/`; it matches the
directory linear-tui is started in, or the repository that directory belongs
to, so one entry covers each worktree.

| Key | |
| --- | --- |
| `team` | Always open on this team, by name or key. |

An entry here wins over the view remembered for that repository.

## Remembered view

linear-tui remembers what it is showing: the page, the list's preset, filters
and grouping, and the issue under the cursor, separately for each repository,
and reopens it on the next launch there. Worktrees of one repository share it;
outside git, the directory itself is the key.

Pages are remembered by their Linear ID, so reordering teams or views does not
reopen the wrong one, and a page that has since been deleted opens its parent
instead. Pressing a key before the page is back cancels the rest of the
restore. `linear-tui open <ID>` opens an issue instead of the remembered view.

The record is a small JSON file per running instance under
`~/.local/state/linear-tui/workspaces/`. It is also what
`linear-tui context` shows your coding agent; its format is in
[view-snapshot.md](view-snapshot.md).

## Files and environment

| Path | |
| --- | --- |
| `~/.config/linear-tui/config.toml` | Settings, and an API key if you use one |
| `~/.config/linear-tui/tokens.json` | The OAuth token |
| `~/.config/linear-tui/debug.log` | The log |
| `~/.local/state/linear-tui/workspaces/` | Remembered views ([view-snapshot.md](view-snapshot.md)) |
| `~/.local/state/linear-tui/herdr/` | Hand-off files for the herdr plugin ([herdr.md](herdr.md)) |

On macOS the config directory is `~/Library/Application Support/linear-tui`
and the state directory is the same one.

| Variable | |
| --- | --- |
| `LINEAR_TUI_STATE_DIR` | Use this directory instead of the state directory. |
| `LINEAR_CLIENT_ID`, `LINEAR_CLIENT_SECRET` | Authorize against this Linear application, over `config.toml`. |
| `RUST_LOG` | Log level for `debug.log`: `info` by default; `debug` or `linear_tui=trace` to investigate. |
| `XDG_CONFIG_HOME`, `XDG_STATE_HOME` | Move the config and state directories (Linux). |
