# View snapshot

A view snapshot records what one linear-tui instance is showing. linear-tui
writes it as you move around and reads it back on the next launch in the same
repository; other programs, such as an agent or a herdr plugin, can read it to
learn what you are looking at.

This document describes the contract. Fields are only added; a change that
removes or reinterprets one bumps `version`, and readers must refuse a version
they do not know.

## Where it lives

```text
$STATE/workspaces/<name>-<hash>/<pid>.json
```

- `$STATE` is `$XDG_STATE_HOME/linear-tui` (default `~/.local/state/linear-tui`)
  on Linux and the local data directory elsewhere
  (`~/Library/Application Support/linear-tui` on macOS). The environment
  variable `LINEAR_TUI_STATE_DIR` overrides it.
- `<name>-<hash>` identifies the **workspace**: the repository the working
  directory belongs to, found with `git rev-parse --git-common-dir`, so every
  worktree of one repository shares a directory. Outside git it is the
  directory itself. `<name>` is the directory's name, for humans; `<hash>` is the
  64-bit FNV-1a of the absolute path, in hex.
- `<pid>` is the process that owns the file. Each running instance writes only
  its own, so two instances in one repository do not overwrite each other.

The file is written owner-only (`0600`) and atomically, so a reader does not see
a half-written one. It is rewritten at most every half second while the view
changes, and a last time when linear-tui quits, with `closed_at` set. The newest
four files of instances that are gone are kept; older ones are deleted.

### Which file to read

- **Reopening** (what linear-tui does on launch): the file with the latest
  `closed_at`; if none has one, because every earlier instance crashed or is
  still running, the one with the latest `updated_at`.
- **Reporting** (what `linear-tui context` does): the file of a running instance
  (`closed_at` absent and process `pid` alive) with the latest `updated_at`;
  failing that, the newest file of all, reported as stale.

## Example

```json
{
  "version": 1,
  "workspace": "/home/me/dev/shop",
  "cwd": "/home/me/dev/shop-worktrees/eng-42-checkout",
  "pid": 48213,
  "updated_at": "2026-09-25T06:01:02Z",
  "herdr_pane": "w2:p3",
  "team": { "id": "3f1c…", "key": "ENG", "name": "Engineering" },
  "destination": {
    "kind": "team",
    "team": { "id": "3f1c…", "key": "ENG", "name": "Engineering" },
    "section": "issues"
  },
  "screen": "issue_detail",
  "issue": { "id": "9a0e…", "identifier": "ENG-42", "title": "Checkout fails on empty cart" },
  "group_by": "status",
  "lists": [
    {
      "source": "team",
      "preset": "active",
      "priority": "High",
      "selected": { "id": "9a0e…", "identifier": "ENG-42", "title": "Checkout fails on empty cart" }
    }
  ],
  "rows": [
    { "kind": "issue", "id": "77b2…", "identifier": "ENG-40", "title": "Retry payment webhooks",
      "state": "In Progress", "assignee": "me", "priority": "High",
      "url": "https://linear.app/shop/issue/ENG-40/retry-payment-webhooks" },
    { "kind": "issue", "id": "9a0e…", "identifier": "ENG-42", "title": "Checkout fails on empty cart",
      "state": "Todo", "priority": "High",
      "url": "https://linear.app/shop/issue/ENG-42/checkout-fails-on-empty-cart" }
  ],
  "rows_total": 2,
  "selected_row": 1
}
```

The same example lives in `tests/fixtures/view_snapshot.json`, where a test
keeps it parseable.

## Fields

Every ID is a Linear ID. Pages are recorded by ID, not by their position in
the sidebar, so a reordered sidebar still names the same page.

| Field | Type | |
| --- | --- | --- |
| `version` | number | `1`. |
| `workspace` | path | The repository root (or directory) the file is filed under. |
| `cwd` | path | Where the instance was started (a worktree or a subdirectory, for example). |
| `pid` | number | The instance's process. |
| `updated_at` | timestamp | When the view last changed. |
| `closed_at` | timestamp? | When the instance quit normally. |
| `herdr_pane` | string? | The herdr pane it runs in (`HERDR_PANE_ID`). |
| `team` | team? | The selected team. |
| `destination` | destination | The sidebar destination, see below. |
| `screen` | screen | What the content pane shows. |
| `project` | ref? | The project page that is open, or is behind the open issue. |
| `cycle` | ref? | The cycle page that is open, or is behind the open issue. |
| `issue` | issue? | The issue open on the `issue_detail` screen. |
| `search` | string? | Set while the team list shows workspace search results for this term. |
| `group_by` | string | `status`, `assignee`, `priority`, `project`, or `none`. |
| `lists` | list settings[] | How each issue list is shaped; a list left as it opens is omitted. |
| `rows` | row[] | The first 100 rows of the list on screen, in display order. On `issue_detail`, the list the issue was opened from. |
| `rows_total` | number | How many rows that list has loaded in all. |
| `selected_row` | number? | The row under the cursor, as an index into `rows`. |

Timestamps are RFC 3339 in UTC, to the second: `2026-09-25T06:01:02Z`. A `?`
marks a field that may be absent.

**team**: `{ "id", "key", "name" }`. **ref**: `{ "id", "name" }`.
**issue**: `{ "id", "identifier", "title" }`, where `identifier` is `ENG-42`.

**destination** is tagged by `kind`:

| `kind` | Other fields | |
| --- | --- | --- |
| `my_issues` | | My Issues. |
| `views` | | The workspace's saved views. |
| `view` | `id`, `name` | One saved view. |
| `team` | `team`, `section` | A team page; `section` is `issues`, `cycles`, `projects`, or `views`. |
| `favorite` | `id`, `title` | A favorite project, cycle, or issue, opened in place. |

**screen** is one of `issue_list`, `issue_detail`, `project_list`,
`project_detail`, `cycle_list`, `cycle_detail`, `view_list`. The screens form
the path back: an `issue_detail` over a `project` goes back to that project's
page, which goes back to the destination's list.

**list settings**: `{ "source", "preset", "status"?, "priority"?, "selected"? }`.
`source` is which list: `team`, `my`, `view`, `project`, or `cycle`. `preset`
is `active`, `backlog`, or `all`. `status` is a workflow state name, `priority`
a label (`Urgent`, `High`, `Medium`, `Low`, `None`), `selected` the issue under
that list's cursor. The search box is not recorded.

**row**: `{ "kind", "id", "identifier"?, "title", "state"?, "assignee"?,
"priority"?, "url"? }`. `kind` is `issue`, `project`, `cycle`, or `view`.
`state` is an issue's workflow state or a project's state; `assignee` an
issue's assignee or a project's lead.

## Reopening

On launch linear-tui follows the recorded path as Linear answers: the
team, then the destination, then the project or cycle page, then the issue, and
puts each list's cursor back on its issue. Whatever no longer exists opens its
parent instead:

| Gone | Opens |
| --- | --- |
| the team | the first team's issues |
| the saved view | the Views page |
| the favorite | My Issues |
| the project or cycle | the list it was on |
| the issue | the list it was opened from |

A project or cycle is looked for in the first page of its list. Pressing a key
before the page is back stops the restore where it is.

A `[workspaces."<path>"]` entry in `config.toml` for the repository or the
directory wins over the snapshot: that directory opens as configured.
