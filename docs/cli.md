# Headless commands

`linear-tui context`, `linear-tui issue …`, and the commands for projects,
milestones, teams, cycles, views, and favorites run without the TUI. They are
how a coding agent reads what you are looking at in linear-tui and does
whatever you can do in the TUI, on Linear itself, using the credentials `linear-tui auth` set up and the same request
code as the TUI, so there is no second client and no second sign-in. They act
in the workspace in use ([authentication.md](authentication.md#several-workspaces)).
`linear-tui tui …` goes further and works the running TUI itself, and
`linear-tui --headless` runs one for agents alone.

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

## `linear-tui tui <command> [--json] [--workspace <path>]`

Works the linear-tui running in this repository (or in `--workspace`) the way
a person does: read the screen, press keys, run a command. Each command waits
until Linear has answered what it asked for — up to 15 seconds, after which
the screen is reported still loading — and prints the screen it leads to.

| Command | Does |
| --- | --- |
| `tui screen` | Nothing; reports the screen |
| `tui press <keys>` | Presses keys: plain characters one by one (`gm` is `g`, `m`), named keys in brackets — `<Enter>` `<Esc>` `<Tab>` `<S-Tab>` `<C-k>` `<A-Enter>` `<F5>` `<Space>` `<Up>` … |
| `tui type <text>` | Types into the field that has focus (`-` reads stdin) |
| `tui run <title>` | Runs the command palette entry of that title, in any case, `…` optional; part of a title will do when it names one entry |
| `tui open <ID>` | Opens an issue over whatever is on screen |
| `tui quit` | Quits linear-tui; prints `linear-tui quit` |

````markdown
# linear-tui screen

- Showing: Engineering › Issues
- Focus: issue_list
- Issue: ENG-42 Checkout fails on empty cart

```text
<the frame, one line per row>
```

Keys here: `Enter` open · `^K` commands · `s/p/a` status/priority/assignee · …

## Commands here

- Change status… (`s`)
- Go to my issues (`g m`)
- …
````

- `- Focus:` is where keys go: a screen (`issue_list`, `issue_detail`,
  `project_list`, …), the `sidebar`, a text field (`comment field`,
  `new issue: title`, …), or `go_to` while a `g` chord waits for its key.
- `- Open:` names what is open over the screen: a picker (`status picker`),
  the command palette, the help, or an error — which any key dismisses.
- `- Issue:` is the issue under the cursor, or open.
- `- Status:`, `- Error:`, `- Loading:`, and `- Notes:` appear when there is
  something to say. `- Held:` lists what a headless instance did not do at
  the desktop: a page it would have opened in a browser, text it would have
  copied, a hand-off to herdr.
- `## Commands here` lists the palette entries that apply, with the keys
  that run them too.

With `--json`: `{ showing, lines, width, height, focus, overlay, issue,
status, error, loading, notes, keys: [{ keys, does }], commands: [{ title,
keys }], held }`; fields are only ever added. A command that cannot be carried
out — an unknown key, a title that names no single entry — fails on stderr
and leaves the screen as it was.

The instance listens on a loopback port, and writes where with a token to
`<pid>.control` beside its view snapshot, readable only by you. Set
`[agent] control = false` ([configuration.md](configuration.md#agent)) to
turn it off.

## `linear-tui --headless [--size <W>x<H>]`

Runs linear-tui with no terminal — for an agent, a script, or CI to drive
with `linear-tui tui` in the same repository. It draws a screen of the given
size (120×40 by default) that only `tui screen` shows, records its view as a
running instance does (so `linear-tui context` reads it), opens nothing in a
browser and copies nothing (see `- Held:`), and runs until `tui quit` or an
interrupt. It needs credentials already set up, and its control channel on.

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
- Linear workspace: Shop (shop)
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
- `- Linear workspace:` is the workspace the view is of. When `linear-tui
  issue …` would act in another one (`linear-tui auth switch`), the output ends
  with a bold warning naming the `auth switch` that fixes it.
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

## `linear-tui issue list (source) [narrowing] [--limit <n>] [--json]`

One of the lists the TUI shows, read afresh. The source is one of:

| Source | Lists |
| --- | --- |
| `--team <key> [--preset active\|backlog\|all]` | a team's issues in one of Linear's slices (`active` by default) |
| `--mine` | the issues assigned to you, across every team |
| `--view <name>` | a saved issue view's issues, as Linear evaluates it |
| `--project <name>` | a project's issues |
| `--team <key> --cycle <name\|current>` | a cycle's issues |

`--status <name>`, `--priority <level>`, and `--query <text>` (in the title or
identifier, any case) narrow it, as the TUI's filter and search do; `--preset`
narrows the other sources too. `--limit` (50 by default) caps the rows; pages
are read until it is reached, ten at most.

```markdown
# Weather issues, Active (3)

- WX-12 Crash when the forecast is empty — In Progress · Urgent · me
- WX-9 Retry failed forecast fetches — In Progress · High
- WX-4 Hourly chart overlaps on narrow screens — Todo · High
```

`(n)` becomes `(first n)` when there is more. `--json` prints `{ "list",
"issues": [...], "more" }`, each issue as `issue show --json` prints it.

## `linear-tui issue search <text> [--team <key>] [--json]`

Linear's full-text search, done or not, in one team or the whole workspace.
Prints the results as `issue list` does; `--json` prints `{ "search",
"issues" }`.

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

- Id: 3f2c…

…

#### ↳ me — 2026-09-24T11:00:00.000Z

- Id: 9a41…

…
```

A field without a value is left out. Comments run oldest first, each reply
(`#### ↳`) right after the comment it answers. Each comment's `- Id:` is what
`issue comment --reply-to`, `comment edit`, and `comment delete` take.

`--json` gives `id`, `identifier`, `title`, `url`, `state` (`{ name, type }`,
`type` being Linear's category: `triage`, `backlog`, `unstarted`, `started`,
`completed`, `canceled`, `duplicate`), `priority` (label), `assignee`,
`creator`, `team_id`, `labels` (names), `project` and `cycle` (`{ id, name }`),
`parent` (`{ identifier, title }`), `children` (`{ identifier, title, state }`),
`estimate`, `due_date`, `branch_name`, `created_at`, `updated_at`,
`description`, and `comments` (`{ id, author, created_at, parent_id, body }`,
in the order above).

## `linear-tui issue create --team <key> --title <text> [fields] [--json]`

`--team` is a team key or name, in any case. The fields are those of
`issue update` below, without `--unlabel` and `none`: `--description`,
`--priority` (`none` by default), `--assignee`, `--estimate`, `--label`
(repeatable), `--project`, `--milestone` (of that project), `--cycle`,
`--parent`, each resolved in that team.
Prints `Created ENG-44 <title>` and the URL on the next line; `--json` prints
the new issue as `issue show --json` does.

## `linear-tui issue update <ID> [fields] [--json]`

Changes any of the issue's fields at once:

| Option | Value |
| --- | --- |
| `--title <text>` | the new title |
| `--description <text>` | the new description, Markdown; `-` reads the whole body from stdin |
| `--priority <level>` | `urgent`, `high`, `medium`, `low`, or `none` |
| `--assignee <who>` | `me`, a member of the team by name, display name, or email, or `none` |
| `--estimate <n>` | a whole number, or `none` |
| `--label <name>` | a label to add; repeatable |
| `--unlabel <name>` | a label to remove; repeatable |
| `--project <name>` | a project of the team, or `none` |
| `--milestone <name>` | a milestone of the project the issue is moved to, or else of its own; `none` takes it out of one. Moving the issue to another project takes it out of its milestone |
| `--cycle <name>` | a cycle of the team by name or number, `current` for the one under way, or `none` |
| `--parent <ID>` | the parent issue, or `none` |

At least one is required. Names resolve in the **issue's own team** (labels:
the team's and the workspace's), in any case. A name that matches nothing
fails and lists what is valid; one that matches more than one thing fails and
lists the candidates. An issue cannot be its own parent, nor a label both added
and removed.

Prints one line per field changed, in the order `issue show` lists them:

```text
ENG-42: priority High → Low
ENG-42: labels UI → UI, Bug
ENG-42: description rewritten
```

`--json` prints `{ "issue": "ENG-42", "changes": [{ "field": "priority",
"from": "High", "to": "Low" }, …] }`; `from` and `to` are names (labels: a
list of names), or `null` for none.

## `linear-tui issue comment <ID> <body> [--reply-to <comment-id>] [--json]`

Posts `<body>` (Markdown) as a comment, or with `--reply-to` as a reply in
that comment's thread; Linear's threads are one level deep, so a reply to a
reply goes to the thread's first comment. Prints `Commented on ENG-42 (comment
<id>)`, or `Replied on ENG-42 (comment <id>, in the thread of <id>)`; `--json`
prints `{ "issue": "ENG-42", "commented": true, "id", "url", "parent_id" }`.

## `linear-tui issue comment edit <ID> <comment-id> <body> [--json]`

Replaces the comment's body. Prints `Edited comment <id> on ENG-42`; `--json`
prints `{ "issue": "ENG-42", "comment": "<id>", "edited": true }`.

## `linear-tui issue comment delete <ID> <comment-id> [--json]`

Deletes the comment. Prints `Deleted comment <id> on ENG-42`; `--json` prints
`{ "issue": "ENG-42", "comment": "<id>", "deleted": true }`.

Linear lets only a comment's author edit or delete it, so someone else's fails
before anything is sent, naming who wrote it; so does a comment id that is not
on the issue.

## `linear-tui issue status <ID> <state> [--json]`

Moves the issue to the workflow state named `<state>`, in any case. The state
is looked up in the **issue's own team**: two teams can both have a "Done",
and only the issue's is valid for it. An unknown name fails and lists the
team's states. Prints `ENG-42: Todo → Done`; `--json` prints
`{ "issue": "ENG-42", "from": "Todo", "to": "Done" }`.


## `linear-tui project list (--team <key> | --view <name>) [--json]`

A team's projects, or a saved project view's:

```markdown
# Weather projects (2)

- Forecast v2 — In Progress · High · target 2026-12-01
- Radar — Planned
```

`--json` prints `{ "list", "projects": [...] }`, each as `project show --json`
prints it without its milestones.

A `<project>` below is a project's name, in any case, anywhere in the
workspace, or its id. Two projects of one name fail with both listed, ids
and teams included; name one by its id then.

## `linear-tui project show <project> [--json]`

```markdown
# Forecast v2

- Id: 3f2c…
- Status: In Progress
- Priority: High
- Lead: me
- Teams: WX
- Start: 2026-09-01
- Target: 2026-12-01
- Progress: 40%
- URL: https://linear.app/…

## Description

…

## Milestones (2)

- Beta — target 2026-11-01 (id 9a41…)
- Launch (id 77b0…)
```

A field without a value is left out. `--json` gives `id`, `name`, `url`,
`status` (`{ name, type }`), `priority`, `lead`, `teams` (keys),
`start_date`, `target_date`, `progress` (0 to 1), `description`, and
`milestones` (`{ id, name, target_date, description }`).

## `linear-tui project create --team <key>... --name <text> [fields] [--json]`

## `linear-tui project update <project> [--name <text>] [fields] [--json]`

| Option | Value |
| --- | --- |
| `--team <key>` | create only: a team it belongs to; repeatable, at least one |
| `--description <text>` | Markdown; `-` reads stdin |
| `--lead <who>` | `me`, or a member of its (first) team by name, display name, or email; `none` on update |
| `--status <name>` | one of the workspace's project statuses (`Backlog`, `Planned`, `In Progress`, …) |
| `--priority <level>` | `urgent`, `high`, `medium`, `low`, or `none` |
| `--start <date>`, `--target <date>` | `YYYY-MM-DD`; `none` on update |

A target date before the start date is refused. `create` prints `Created
project <name> (id <id>)` and the URL; `--json` prints the project. `update`
prints one line per field changed (`Forecast v2: status Planned → In
Progress`), and `--json` `{ "project", "id", "changes": [{ "field", "from",
"to" }] }`.

## `linear-tui project delete <project> [--json]`

Moves the project to Linear's trash; its issues stay, without a project.
Prints `Deleted project <name> (id <id>); it is in Linear's trash`.

## `linear-tui milestone list <project> [--json]`

The project's milestones, as `project show` lists them.

## `linear-tui milestone create <project> --name <text> [--description <text>] [--target <date>] [--json]`

## `linear-tui milestone update <project> <milestone> [--name <text>] [--description <text>] [--target <date|none>] [--json]`

## `linear-tui milestone delete <project> <milestone> [--json]`

A `<milestone>` is its name, in any case, or its id, within the project.
`create` prints `Created milestone Beta — target 2026-11-01 (id …) in
<project>`; `update` one line per change; `delete` `Deleted milestone <name>
of <project>`. A deleted milestone's issues stay in the project.

## `linear-tui team list [--json]`, `linear-tui team show <key> [--json]`

The teams you are in; and one team's workflow states (in board order, with
their category), members (with email), and labels (the team's and the
workspace's) — what `issue update` and `issue status` resolve names against.

## `linear-tui cycle list --team <key> [--json]`

A team's cycles with their dates and progress, `← current` on the one under
way.

## `linear-tui view list [--team <key>] [--json]`

The saved views on the workspace's Views page, or a team's: issue views and
project views, each marked shared or personal.

## `linear-tui favorite list [--json]`

Your Favorites in Linear's order, indented inside a folder, each with what
it leads to and the command that reads it:

```markdown
- Forecast v2 (project) — `linear-tui project show "Forecast v2"`
- Open bugs (view) — `linear-tui issue list --view "Open bugs"`
```
