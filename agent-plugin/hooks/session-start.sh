#!/bin/sh
# SessionStart hook for Claude Code and Codex. What it prints becomes
# context for the session — no prompt is sent, no turn is taken.
#
# It prints only when linear-tui is installed and has something for this
# repository: a recorded view, or a worktree tied to an issue. Anywhere else
# the session gets nothing. `linear-tui context` exits non-zero when there is
# nothing to report (and older linear-tui has no `context`), which is what
# keeps this quiet.

command -v linear-tui >/dev/null 2>&1 || exit 0

# The session's directory, from the hook input; hooks also run in it.
cwd="$(sed -n 's/.*"cwd"[[:space:]]*:[[:space:]]*"\([^"]*\)".*/\1/p' | head -n 1)"
[ -n "$cwd" ] && [ -d "$cwd" ] || cwd="$PWD"

context="$(linear-tui context --workspace "$cwd" 2>/dev/null)" || exit 0
issue="$(printf '%s\n' "$context" | sed -n 's/^This worktree is for \*\*\([^*]*\)\*\*.*/\1/p' | head -n 1)"

cat <<'TEXT'
linear-tui (a terminal UI for Linear) is used in this repository. When the user
points at what they are looking at in it — "this issue", "the one I have open",
"the top three", "that list" — run `linear-tui context` first: it prints the page
on their screen, the open issue, and the list with the row under their cursor.

Act on Linear through linear-tui's CLI, which uses the user's own credentials:
- `linear-tui issue show <ID>` — fields, description, sub-issues, comments
- `linear-tui issue comment <ID> <body>` — `-` as the body reads it from stdin
- `linear-tui issue status <ID> <state>` — a state name of the issue's team
- `linear-tui issue create --team <key> --title <text> [--description <text>]`
Add `--json` for JSON. Without an instruction to change an issue, only comment;
move it (to Done or anywhere else) only when asked.

To show the user something, or check linear-tui by using it, work the running
TUI: `linear-tui tui screen` reads it, `tui press <keys>`, `tui run <command>`,
`tui type <text>`, and `tui open <ID>` act, each answering with the new screen.
TEXT
if [ -n "$issue" ]; then
  printf '\nThis worktree is for %s (from its branch name).\n' "$issue"
fi
