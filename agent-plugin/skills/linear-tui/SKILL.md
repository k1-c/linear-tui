---
name: linear-tui
description: Resolve what the user is looking at in linear-tui and act on Linear through the linear-tui CLI. Use when the user refers to an issue or list on their screen without naming it ("this issue", "the one I have open", "the top three", "この issue", "今見てるやつ", "上の 3 つ"), asks to comment on, move, or file a Linear issue, or pastes notes that came from linear-tui.
---

# linear-tui

The user reads and triages Linear in linear-tui, a terminal UI. Its CLI shows
you what is on their screen and acts on Linear with their credentials — no
Linear MCP server or separate sign-in is needed.

## Find out what "this" is

Run `linear-tui context` before guessing. It prints, as Markdown:

- `- Showing:` — the page: team or view, then project or cycle, then issue;
- `## Open issue` — the issue open in the detail view, if any;
- the list on screen, numbered in display order, with `← cursor` on the row the
  user has selected (on an issue page, the list it was opened from);
- whether linear-tui is still open. A closed instance's view may be stale — say
  so if you rely on it.

"The top three" means rows 1–3 of that list; "this one" is the open issue, or
else the row under the cursor. Inside a worktree whose branch names an issue,
the context starts with that issue. Use `--json` when you need exact fields.

## Read and act

| Task | Command |
| --- | --- |
| Read an issue | `linear-tui issue show ENG-42` |
| Comment | `linear-tui issue comment ENG-42 -` (body on stdin) |
| Change its state | `linear-tui issue status ENG-42 "In Review"` |
| File an issue | `linear-tui issue create --team ENG --title "…" --description -` |

`<ID>` may be an identifier in any case or an issue URL. Errors go to stderr
with a non-zero exit; an unknown state lists the team's states.

## Rules

- Without an instruction to change an issue, only comment on it.
- Move an issue — to Done or anywhere else — only when the user asks.
- Quote the identifier (`ENG-42`) when you report what you did.

The full output formats are in `docs/cli.md` of the linear-tui repository.
