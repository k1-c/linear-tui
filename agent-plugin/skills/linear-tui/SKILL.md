---
name: linear-tui
description: Resolve what the user is looking at in linear-tui, act on Linear through the linear-tui CLI, and work the running TUI itself. Use when the user refers to an issue or list on their screen without naming it ("this issue", "the one I have open", "the top three", "この issue", "今見てるやつ", "上の 3 つ"), asks to comment on, move, or file a Linear issue, or pastes notes that came from linear-tui.
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

## Work the TUI itself

When the user wants to see something, or you want to check linear-tui by
using it, drive the running instance the way a person does:

| Task | Command |
| --- | --- |
| Read the screen: focus, what is open, keys and commands here | `linear-tui tui screen` |
| Press keys | `linear-tui tui press "j <Enter>"` (`<Esc>`, `<C-k>`, `<S-Tab>`, …) |
| Run a palette command | `linear-tui tui run "Change status"` |
| Type into the focused field | `linear-tui tui type "In Review"` |
| Show an issue | `linear-tui tui open ENG-42` |

Each answers with the screen it leads to; read it before the next step. A
chord is two keys (`g m`). With no linear-tui open, `linear-tui --headless`
runs one only you can see. Keys and commands change issues exactly as the
user's would, so the rules below hold here too.

## Rules

- Without an instruction to change an issue, only comment on it.
- Move an issue — to Done or anywhere else — only when the user asks.
- Quote the identifier (`ENG-42`) when you report what you did.

The full output formats are in `docs/cli.md` of the linear-tui repository.
