# Keybindings

Shortcuts follow [Linear's own keyboard shortcuts](https://linear.app/docs)
wherever a terminal allows it, so muscle memory carries over from the web app.
`?` shows every shortcut inside linear-tui, and `Ctrl+K` lists what can be done
where you are, with its key. The status bar at the bottom hints at the keys that
matter on the current screen.

## Command palette

`Ctrl+K` opens the command palette, as in Linear. It lists what can be done
where you are, with the key that does the same next to each entry, so it
also teaches the shortcuts. Type to narrow it down: letters match in order,
`cs` finds **C**hange **s**tatus, then `Enter` to run, or `Esc` to close.
`↑` / `↓` (or `Ctrl+p` / `Ctrl+n`) move; recently used commands come first.

The palette also goes places. Type an issue ID or part of a title, or the name
of a team page, saved view, favorite, project, or cycle, and `Enter` opens it.
Issues already loaded match at once. Linear is searched after you pause typing,
so an issue that is not in a list on screen can turn up too. Start the query
with `>` to see commands only.

A command that needs a value, such as *Change status…*, *Assign to…*,
*Group by…*, *Switch team…*, or *Filter…*, turns the palette into a list of
choices; `Esc` (or `Backspace` on an empty query) steps back to the commands.

The same pickers open directly with `s`, `p`, `a`, `t` and `f`, and narrow as you
type there too. Until you type, they keep their single keys: `j` / `k` to move,
`1`-`9` to pick, `q` to close; an upper-case letter starts a query.

## Navigation

| Key | Action |
| --- | --- |
| `j` / `k`, `↓` / `↑` | Move cursor down / up |
| `g` `g` / `G` | Jump to first / last item |
| `Enter`, `Space` | Open (Linear: peek) |
| `Esc` | Back / close |
| `J` / `K` | Next / previous issue, in the detail view |
| `Tab` | Move focus between the sidebar and the content |
| `Ctrl+b` | Show / hide the sidebar |
| `h` / `l` | Fold / unfold a Favorites folder (while the sidebar has focus) |
| `g` `a` / `g` `b` / `g` `e` | Active / Backlog / All issues of the current team |
| `g` `m` / `g` `v` / `g` `p` / `g` `c` | Go to My Issues / Views / Projects / Cycles |
| `1`-`5` | Team issues / My Issues / Projects / Cycles / Views |
| `Ctrl+d` / `Ctrl+u` | Half page down / up |
| `PgDn` / `PgUp` | Full page down / up |

## List display

| Key | Action |
| --- | --- |
| `Shift+Tab` | Next preset: Active → Backlog → All issues (on a Views page: Issues ⇄ Projects tab) |
| `D` | Group by status → assignee → priority → project → none |
| `z` / `Z` | Fold the group under the cursor / fold or unfold every group |

## Issue actions

| Key | Action |
| --- | --- |
| `c` | Create a new issue (`Ctrl+Enter` to save) |
| `s` | Change status |
| `p` | Change priority |
| `Shift+1` … `Shift+4`, `Shift+0` | Set priority directly (Urgent → Low, None) |
| `a` | Assign to someone |
| `i` | Assign to me |
| `m` | Add a comment (`Ctrl+Enter` to send) |

## Copy and open

| Key | Action |
| --- | --- |
| `Ctrl+.` or `y` | Copy the issue ID |
| `Ctrl+Shift+,` or `Y` | Copy the issue URL |
| `Ctrl+Shift+.` or `b` | Copy the suggested git branch name |
| `o` | Open the issue (or project) on linear.app |

Copying uses the OSC 52 terminal escape, so it works over SSH. If nothing lands
on your clipboard, see [troubleshooting.md](troubleshooting.md#copying-does-nothing).

## Search and filtering

| Key | Action |
| --- | --- |
| `/` | Filter the visible list as you type |
| `Ctrl+G` | Search all of Linear for the current query (while filtering) |
| `f` / `Shift+F` | Filter by status & priority / clear filters |

## Notes for your agent

Linear has no agent beside it, so these are linear-tui's own. Jot remarks while
you read, then send them all as one prompt: inside [herdr](herdr.md) to the
agent in the same workspace, anywhere else to the clipboard. See
[agents.md](agents.md).

| Key | Action |
| --- | --- |
| `n` | Note on the issue under the cursor (`Ctrl+Enter` to add) |
| `Shift+N` | Note on the whole view |
| `Ctrl+S` | Send the notes to your agent |
| `g` `w` | Go to the herdr agent working on the issue (inside herdr only) |

Inside herdr, an issue row also shows the state of the agent working on it:
`▲` waiting for you, `●` working, `○` idle, `✓` done.

## Other

| Key | Action |
| --- | --- |
| `t` | Switch team (or Enter / click on the team row in the sidebar) |
| `Ctrl+r`, `F5` | Refresh the current view |
| `?` | Show all shortcuts |
| `q` | Quit (on a nested page: go back) |
| `Ctrl+c` | Quit from anywhere |

## Mouse

| Action | Effect |
| --- | --- |
| Click a row | Select it; click it again to open it |
| Click a sidebar entry | Go there; a folder folds, the team row opens the team switcher |
| Click a preset chip, tab, or group header | Switch preset or tab / fold the group |
| Click a popup or palette entry | Choose it; click outside to close |
| Wheel | Scroll whatever is under the pointer |

## Text fields

Search, comments, notes, and the new-issue form accept readline-style editing:
`Ctrl+w` (delete a word), `Ctrl+u` / `Ctrl+k` (delete to the start / end),
`Ctrl+a` / `Ctrl+e` (jump to the start / end), and the arrow keys. In multi-line
fields `Enter` breaks the line and `Ctrl+Enter` (or `Alt+Enter`) submits; `Esc`
cancels.

## Differences from Linear

A terminal cannot deliver every shortcut the web app uses, and a few of Linear's
actions have no meaning here. Where they differ:

| Linear | linear-tui | Why |
| --- | --- | --- |
| `Ctrl+.`, `Ctrl+Shift+.`, `Ctrl+Shift+,`, `Ctrl+M` | also `y`, `b`, `Y`, `m` | Legacy terminals cannot distinguish `Ctrl`+punctuation, and `Ctrl+M` *is* `Enter`. The originals work in terminals supporting the [kitty keyboard protocol](https://sw.kovidgoyal.net/kitty/keyboard-protocol/) (kitty, Ghostty, WezTerm, foot, Alacritty), which is enabled automatically when available. |
| `Cmd+K` (command menu) | `Ctrl+K` | A terminal never receives `Cmd`. `Ctrl+K` is what Linear uses outside macOS; inside a text field it keeps its readline meaning (delete to the end). |
| `r` (rename issue) | *(unbound)* | Renaming isn't supported yet; refreshing uses the terminal's `Ctrl+r` instead. |
| `j` / `k` (next / previous issue in the issue view) | `J` / `K` | In the detail view `j`/`k` scroll the text, which a terminal cannot do with a trackpad. |
| Display options menu (grouping, collapsing) | `D`, `z`, `Z`, `Shift+Tab` | Linear keeps these behind a menu with no shortcut; the keys are ones Linear leaves free. |
| Double-click to open | click the selected row again | Terminals do not report double-clicks. |
| `Ctrl+d` (set due date) | half page down | The scrolling convention wins in a terminal. |
| — | `o`, `t`, `q`, `Tab`, `Ctrl+b`, `1`-`5` | Open in browser, switch team, quit, and sidebar focus have no web-app equivalent. |
| — | `n`, `Shift+N`, `Ctrl+S`, `g` `w` | Notes for a coding agent, and jumping to one; Linear has no agent beside it. |
