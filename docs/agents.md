# Working with coding agents

More and more Linear issues are read, filed, and fixed by coding agents. The
human's part becomes reading what is on the board, pointing at it, and saying
what to do. linear-tui lets you do that without spelling out IDs: the agent can
see what you are looking at, act on Linear with your credentials, and take your
notes on it as one prompt.

None of this needs a Linear MCP server or a second sign-in, and all of it works
without [herdr](herdr.md) — herdr only adds the parts that need a workspace
manager.

## 1. The agent sees your screen

```sh
linear-tui context
```

prints what linear-tui is showing in the current repository — the page, the
issue open in the detail view, and the list on screen, numbered, with the row
under your cursor marked:

```markdown
- Showing: Engineering › Issues › ENG-42
- Updated: 2 minutes ago (2026-09-25T06:01:02Z); linear-tui is open (pid 48213)

## The list it was opened from (2)

Preset: Active · Priority: High

1. ENG-40 Retry payment webhooks — In Progress · me · High
2. ENG-42 Checkout fails on empty cart — Todo · High  ← cursor
```

So "fix this one" and "look at the top three" resolve without IDs. Inside a
worktree whose branch names an issue (`me/eng-42-…`), that issue comes first.

## 2. The agent acts on Linear

```sh
linear-tui issue show ENG-42               # fields, description, sub-issues, comments
linear-tui issue comment ENG-42 -          # a comment read from stdin
linear-tui issue status ENG-42 "In Review" # a state of the issue's own team
linear-tui issue create --team ENG --title "Retry payment webhooks"
```

Every command takes `--json`, prints Markdown otherwise, and fails on stderr
with a non-zero exit. The contract is [cli.md](cli.md).

## 3. The agent knows all this without being told

Install the agent plugin once:

```sh
# Claude Code
claude plugin marketplace add k1-c/linear-tui
claude plugin install linear-tui@linear-tui

# Codex
codex plugin marketplace add k1-c/linear-tui
codex plugin add linear-tui@linear-tui
```

Every new session in a repository where you use linear-tui then starts with a
few lines on the commands above, through a SessionStart hook — no prompt is
sent, no turn is spent, and repositories where you do not use linear-tui get
nothing. A `linear-tui` skill covers "this issue" and "the top three". See
[agent-plugin.md](agent-plugin.md).

For another agent, add a line like this to its instructions (`AGENTS.md`,
`CLAUDE.md`, …):

```markdown
The user browses Linear in linear-tui. `linear-tui context` shows what they
are looking at (open issue, list, cursor); `linear-tui issue show|comment|status|create`
act on Linear with their credentials. Only move an issue to Done when told to.
```

## 4. You send notes as you read

While reading in linear-tui, `n` notes the issue under the cursor and
`Shift+N` the whole view. `Ctrl+S` sends every note as one prompt, each naming
its issue:

```markdown
My notes on what I am looking at in linear-tui (Engineering › Issues):

- **ENG-42** Checkout fails on empty cart: reproduce with an expired coupon first
- **This view**: the top three are one bug — merge them

(`linear-tui context` shows this view with the row under my cursor; …)
```

Outside herdr the prompt is copied to the clipboard, to paste into whichever
agent you use. Inside herdr it goes straight to the agent in the same
workspace.

## 5. In herdr

The [herdr plugin](herdr.md) puts linear-tui in a pane next to your agents, and
adds what only a workspace manager can:

- `Ctrl+S` delivers your notes to the agent beside linear-tui;
- issue rows show the state of the agent working on them (`▲` waiting for you,
  `●` working), and `g w` jumps to that agent's pane;
- Ctrl+click on a Linear issue URL in any pane opens it in linear-tui;
- after herdr restarts, linear-tui comes back in its panes, on the page it was
  showing.

## Ground rules

These are the defaults the agent plugin and the notice teach, and the ones the
commands are designed around:

- The agent acts with **your** credentials; issues it files are yours.
- Without an instruction to change an issue, an agent only comments on it.
- It moves an issue — to Done or anywhere else — only when you ask.
