# herdr plugin

[herdr](https://herdr.dev/) is a terminal workspace manager for coding agents.
The plugin in `herdr-plugin/` puts linear-tui in a herdr pane and connects it
to the agents running next to it.

linear-tui does not depend on herdr: it builds, runs, and passes its tests
without it. Everything herdr-specific lives in the plugin's scripts.

## Install

```sh
herdr plugin install k1-c/linear-tui/herdr-plugin
```

It needs herdr 0.9.1 or newer, `jq`, and `linear-tui` on your `PATH` (or set
`LINEAR_TUI_BIN`, below). For a local checkout, link it instead:
`herdr plugin link ./herdr-plugin`.

Bind the action to a key in herdr's `config.toml`:

```toml
[[keys.command]]
key = "prefix+l"
type = "plugin_action"
command = "k1-c.linear-tui.open"
description = "linear-tui"
```

## Actions

| Action | |
| --- | --- |
| `k1-c.linear-tui.open` | Focus the workspace's linear-tui pane, or open one in the focused pane's directory. |

linear-tui opens where it was last left in that repository (see
[view-snapshot.md](view-snapshot.md)), so an agent in the same workspace can run
`linear-tui context` and see what you see.

## Configuration

`$(herdr plugin config-dir k1-c.linear-tui)/config.env`, plain `KEY=value`
lines:

| Key | Default | |
| --- | --- | --- |
| `PLACEMENT` | `overlay` | How `open` places the pane: `overlay`, `split`, `tab`, or `zoomed`. |
| `DIRECTION` | `right` | Where a `split` goes: `right` or `down`. |
| `LINEAR_TUI_BIN` | `linear-tui` on `PATH` | The binary to run. herdr starts plugins with a short `PATH`; the plugin adds `~/.cargo/bin`, `~/.local/bin`, Nix and Homebrew locations. |
| `NOTIFY_AGENTS` | `0` | `1` tells each newly started agent, once, how to use `linear-tui context` and `linear-tui issue …`. |

## How agents learn about linear-tui

An agent has to know `linear-tui context` exists before you say "fix this
one". The plugin offers two ways, and you can add a third:

- **`NOTIFY_AGENTS=1`** — when an agent has started and is waiting for its first
  message, the plugin sends it `herdr-plugin/notice.md` once per agent session.
  It is off by default: the notice takes the agent's first turn, and Claude Code
  names the session after it.
- **Your agent's instructions** — add a line like this to `AGENTS.md` or
  `CLAUDE.md`, which works with or without herdr:

  ```markdown
  The user browses Linear in linear-tui. `linear-tui context` shows what they
  are looking at (open issue, list, cursor); `linear-tui issue show|comment|status|create`
  act on Linear with their credentials. Only move an issue to Done when told to.
  ```

### Why this design (#47)

Checked against herdr 0.9.1 and Claude Code 2.1:

- `[[events]] on` accepts every event of the socket API's `events.subscribe`
  (`pane.agent_detected`, `pane.agent_status_changed`, `pane.created`, …);
  herdr warns only about names it does not know. The payload arrives in
  `HERDR_PLUGIN_EVENT_JSON` as `{ "event": "pane_agent_status_changed", "data":
  { "pane_id", "workspace_id", "agent", "agent_status" } }`. herdr-hunk-diff
  learns about agent states the same way, from `pane.agent_status_changed`.
- `pane.agent_detected` fires as soon as the agent's process is recognised —
  often while it still shows a startup dialog (a trust prompt), reported as
  `blocked`. `herdr agent prompt` refuses a blocked agent, and typing into the
  dialog would answer it. The right moment is the agent's first `idle`, before
  it has completed any turn (`completion_seq` absent in `herdr agent get`).
- A prompt sent then arrives before the user's first message and is answered
  in a few seconds. It is not free: it is the agent's first turn, and Claude
  Code titles the session after it. Hence opt-in.
- The other options were dropped: an MCP server needs registering per agent
  and contradicts "no dependency on Linear MCP"; a note inside each prompt
  linear-tui sends covers only the human-to-agent direction (it is still done,
  once prompts are sent from linear-tui).

## Files

- `herdr-plugin.toml` — the manifest.
- `lib.sh` — shared helpers: config, `PATH`, the `linear-tui` binary.
- `open.sh` — the `open` action.
- `event.sh`, `notice.md` — the opt-in agent notice.

Failures go to herdr's plugin log: `herdr plugin log list --plugin k1-c.linear-tui`.
