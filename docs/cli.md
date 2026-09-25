# Headless commands

`linear-tui context` and `linear-tui issue …` run without the TUI. They are how
a coding agent reads what you are looking at in linear-tui and acts on Linear
itself, using the credentials `linear-tui auth` set up and the same request
code as the TUI, so there is no second client and no second sign-in.

This document defines the contract. Output is meant for agents first:

- **Markdown by default**: plain, stable, and short enough to paste into a
  prompt. Headings and the `- Name: value` lines below stay as documented.
- **`--json`** for scripts. Fields are only ever added.
- **Errors** go to stderr, prefixed `Error:`, with exit status 1. Nothing
  prompts: the commands do not wait for a person at the keyboard.

An `<ID>` is an identifier in any case (`ENG-123`, `eng-123`), an issue URL
(`https://linear.app/<org>/issue/ENG-123/…`), or an issue UUID. An argument
given as `-` is read from stdin, so a multi-line body needs no quoting.

## `linear-tui open <ID>`

Not headless: starts the TUI on that issue, over the team's issue list,
instead of reopening the remembered view. The herdr plugin's link handler uses
it.

## `linear-tui paths [--json]`

Where linear-tui keeps its files: `config` (`config.toml`) and `state` (view
snapshots, and the herdr plugin's outbox). With `--json`:
`{ "config": "…", "state": "…" }`. Honors `LINEAR_TUI_STATE_DIR`.

## `linear-tui context [--json] [--workspace <path>]`

What linear-tui is showing in the repository of the current directory (or of
`--workspace`), read from its [view snapshot](view-snapshot.md): the page, the
issue open on it, and the list on screen with the row under the cursor. It
reads the snapshot of a running instance first, and says when the only one
left is from an instance that has quit.

Inside a linked worktree whose branch names an issue (`me/eng-42-…`, as Linear
suggests branch names), that issue is reported first because it is what the
worktree is for.

```markdown
# linear-tui context

This worktree is for **ENG-42** — `linear-tui issue show ENG-42` for the issue itself.

- Showing: Engineering › Issues › ENG-42
- Updated: 2 minutes ago (2026-09-25T06:01:02Z); linear-tui is open (pid 48213)

## Open issue

ENG-42 Checkout fails on empty cart — `linear-tui issue show ENG-42` for its description and comments.

## The list it was opened from (2)

Preset: Active · Priority: High

1. ENG-40 Retry payment webhooks — In Progress · me · High
2. ENG-42 Checkout fails on empty cart — Todo · High  ← cursor

Act on these with `linear-tui issue show|comment|status <ID>` and `linear-tui issue create --team <key> --title <text>`.
```

- `- Showing:` is the path back: destination › project or cycle › issue.
- `- Search results for:` appears when the list is a workspace search.
- A closed instance reads `linear-tui is **closed — this is the last view
  before it quit, and may be stale**`.
- The list heading is `## List (n)` or, under an open issue, `## The list it
  was opened from (n)`; `n` becomes `first 100 of n` for a longer list. Rows are
  numbered in display order, and `← cursor` marks the selected one.

With `--json`:

```json
{
  "workspace": "/home/me/dev/shop",
  "running": true,
  "worktree_issue": "ENG-42",
  "snapshot": { "version": 1, "…": "the view snapshot, as documented" }
}
```

`running` and `snapshot` are `null` when no view is recorded. With neither a
view nor a worktree issue, the command fails.

## `linear-tui issue show <ID> [--json]`

```markdown
# ENG-42 Checkout fails on empty cart

- State: Todo
- Priority: High
- Assignee: me
- Creator: someone
- Labels: bug, checkout
- Project: Storefront
- Cycle: Cycle 12
- Parent: ENG-30 Payments rework
- Branch: me/eng-42-checkout-fails-on-empty-cart
- URL: https://linear.app/shop/issue/ENG-42/checkout-fails-on-empty-cart

## Description

…

## Sub-issues

- ENG-43 Guard the empty cart (Done)

## Comments (2)

### someone — 2026-09-24T10:00:00.000Z

…

#### ↳ me — 2026-09-24T11:00:00.000Z

…
```

A field without a value is left out. Comments run oldest first, each reply
(`#### ↳`) right after the comment it answers.

`--json` gives `id`, `identifier`, `title`, `url`, `state` (`{ name, type }`,
`type` being Linear's category: `triage`, `backlog`, `unstarted`, `started`,
`completed`, `canceled`, `duplicate`), `priority` (label), `assignee`,
`creator`, `team_id`, `labels` (names), `project` and `cycle` (`{ id, name }`),
`parent` (`{ identifier, title }`), `children` (`{ identifier, title, state }`),
`estimate`, `due_date`, `branch_name`, `created_at`, `updated_at`,
`description`, and `comments` (`{ id, author, created_at, parent_id, body }`,
in the order above).

## `linear-tui issue create --team <key> --title <text> [--description <text>] [--priority <level>] [--json]`

`--team` is a team key or name, in any case. `--priority` is `urgent`, `high`,
`medium`, `low`, or `none` (the default). Prints `Created ENG-44 <title>` and
the URL on the next line; `--json` prints the new issue as `issue show --json`
does.

## `linear-tui issue comment <ID> <body> [--json]`

Posts `<body>` (Markdown) as a comment. Prints `Commented on ENG-42`; `--json`
prints `{ "issue": "ENG-42", "commented": true }`.

## `linear-tui issue status <ID> <state> [--json]`

Moves the issue to the workflow state named `<state>`, in any case. The state
is looked up in the **issue's own team**: two teams can both have a "Done",
and only the issue's is valid for it. An unknown name fails and lists the
team's states. Prints `ENG-42: Todo → Done`; `--json` prints
`{ "issue": "ENG-42", "from": "Todo", "to": "Done" }`.
