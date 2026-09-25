# Troubleshooting

When something goes wrong, the log usually says why:
`~/.config/linear-tui/debug.log` (on macOS,
`~/Library/Application Support/linear-tui/debug.log`). Run with
`RUST_LOG=debug linear-tui` for more detail. The log names issues and people in
your workspace, so check it before you paste it anywhere.

## Signing in

**The browser opens, but linear-tui does not hear back.** The callback goes to
`http://localhost:53681` (or 53682, 53683) on the machine linear-tui runs on.
Over SSH, or in a container, the browser's `localhost` is a different machine.
Use a personal API key instead: `linear-tui auth token <key>`
([authentication.md](authentication.md#with-a-personal-api-key)).

**Your workspace does not allow the application.** Some workspaces require an
admin to approve third-party applications. Register your own and use it:
`linear-tui auth set-oauth <client-id>`
([authentication.md](authentication.md#using-your-own-linear-application)).

**Which credentials are in use?** `linear-tui auth status` shows the token or
key, how long the token has left, and who Linear says you are.

## Copying does nothing

`y`, `Y`, and `b` copy through the terminal (OSC 52), so they work over SSH if
the terminal allows it.

- **tmux:** `set -g set-clipboard on` in `~/.tmux.conf`.
- **GNU screen** does not pass OSC 52 through.
- Some terminals ask for permission, or have it off by default (for example
  iTerm2: *Settings → General → Selection → Applications in terminal may access
  clipboard*).

The same applies to `Ctrl+S` outside herdr, which copies your notes.

## A shortcut does nothing

**`Ctrl+.`, `Ctrl+Shift+,`, `Ctrl+Shift+.`, `Ctrl+M`.** Most terminals cannot
tell these apart from other keys. Use their aliases `y`, `Y`, `b`, `m`, or a
terminal with the kitty keyboard protocol (kitty, Ghostty, WezTerm, foot,
Alacritty), which linear-tui enables automatically.

**Inside tmux or a multiplexer,** the multiplexer may take the key first. For
example, `Ctrl+b` is tmux's and herdr's prefix. The sidebar can also be
toggled from the command palette (`Ctrl+K`, *Toggle sidebar*).

**`Ctrl+S` freezes the terminal** in some setups with flow control on. linear-tui
turns it off while it runs; if your terminal emulator intercepts `Ctrl+S`
itself, use the palette's *Send notes to your agent*.

## The sidebar is gone

It hides itself on terminals narrower than 100 columns, whatever the setting;
widen the window. On a wide terminal, `Ctrl+b` shows it again if it was hidden,
and `sidebar = false` in `config.toml` keeps it hidden at start
([configuration.md](configuration.md)).

## linear-tui does not reopen where I left off

- The view is remembered per repository. Starting in another repository, or
  outside git in another directory, opens that one's view.
- A `[workspaces."<path>"]` entry in `config.toml` wins over the remembered
  view.
- `linear-tui open <ID>` opens that issue instead.
- A page that was deleted opens its parent; a project or cycle is looked for in
  the first page of its list only.

## An agent does not know about linear-tui

- `linear-tui context` in that repository should print a view. If it says none
  is recorded, open linear-tui there once.
- The agent plugin needs linear-tui 0.7.0 or newer on the agent's `PATH`
  (`linear-tui --version`). Check it is enabled: `claude plugin list`.
- Codex asks you to trust the plugin's hook before running it.

See [agents.md](agents.md) and [agent-plugin.md](agent-plugin.md).

## herdr

The plugin reports failures to herdr's plugin log:

```sh
herdr plugin log list --plugin k1-c.linear-tui
```

- **"linear-tui is not installed":** herdr starts plugins with a short `PATH`.
  Set `LINEAR_TUI_BIN` in the plugin's `config.env`.
- **Notes were not sent:** no agent was in the workspace, or it did not take
  the prompt. The prompt is saved under `~/.local/state/linear-tui/herdr/outbox/undelivered/`.
- **Agent states do not show:** the plugin needs `jq`, and the agent's branch
  must name the issue (`me/eng-42-…`).

See [herdr.md](herdr.md).

## Still stuck

[Open an issue](https://github.com/k1-c/linear-tui/issues) with what you did,
what you expected, `linear-tui --version`, your terminal, and the relevant lines
of `debug.log`.
