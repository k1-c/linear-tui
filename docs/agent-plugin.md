# Agent plugin

`agent-plugin/` is a plugin for Claude Code and Codex. It tells a coding agent
that you use linear-tui, so "fix this issue" or "comment on the top three" can
work without spelling out IDs: the agent runs `linear-tui context` to see what
is on your screen, `linear-tui issue …` to act, and `linear-tui tui …` to work
the running TUI itself (see [cli.md](cli.md)).

It needs linear-tui 0.7.0 or newer on the agent's `PATH`, and works with or
without herdr. `linear-tui tui …` came after 0.9.0; with an older linear-tui
the agent still has `context` and `issue`.

## Install

```sh
# Claude Code (or /plugin in a session)
claude plugin marketplace add k1-c/linear-tui
claude plugin install linear-tui@linear-tui

# Codex
codex plugin marketplace add k1-c/linear-tui
codex plugin add linear-tui@linear-tui
```

Codex asks you to review and trust the plugin's hook before it runs.

## What it adds

**A SessionStart hook** (`hooks/session-start.sh`). When a session starts,
resumes, or is cleared or compacted, it prints about a dozen lines that the
agent receives as context: when to run `linear-tui context`, the `issue`
commands, the rule that an issue is only moved when you ask, and that it can
work the running TUI — `linear-tui tui screen` to read it, `tui press`,
`tui run`, `tui type`, and `tui open` to act, each answering with the new
screen — to show you something or to check linear-tui by using it. Inside a
worktree whose branch names an issue it adds `This worktree is for ENG-42`.

It prints only when linear-tui is installed and `linear-tui context` has
something for the repository: a recorded view (you have opened linear-tui there)
or a worktree issue. In other repositories the plugin does not add extra
context.

No prompt is sent. This differs from the herdr plugin's `NOTIFY_AGENTS` notice
([herdr.md](herdr.md)): a prompt spends the agent's first turn, renames a Claude
Code session, can collide with what you are typing, and reads as something you
said. Context from a hook does none of that.

**A `linear-tui` skill** (`skills/linear-tui/SKILL.md`), which the agent loads
when you refer to something on your screen without naming it, or ask it to
comment on, move, or file an issue. It explains how to read `linear-tui context`
(numbered rows, `← cursor`, stale views), the `issue` commands, and working the
TUI: reading the screen, pressing keys, running palette commands, typing, and
`linear-tui --headless` when no linear-tui is open.

## Layout

```text
.claude-plugin/marketplace.json     the Claude Code marketplace (repository root)
.agents/plugins/marketplace.json    the Codex marketplace
agent-plugin/
  .claude-plugin/plugin.json        Claude Code manifest
  .codex-plugin/plugin.json         Codex manifest
  hooks/hooks.json                  read by both
  hooks/session-start.sh
  skills/linear-tui/SKILL.md
```

Both agents pass `CLAUDE_PLUGIN_ROOT` to plugin hooks and run them in the
session's directory. Bump `version` in both `plugin.json` files when the plugin
changes; release-plz only versions the crate.

## Checked

- Claude Code 2.1 with `--plugin-dir agent-plugin`: in a worktree on
  `me/eng-42-checkout`, a new session named `linear-tui context` and ENG-42
  without running anything; in an unrelated directory it knew neither.
- Codex 0.139: the hook format, `CLAUDE_PLUGIN_ROOT`, and plain-text output as
  context follow Codex's hook documentation, but a live session has not been
  checked yet.
