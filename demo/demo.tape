# Demo recording for the README. Run through demo/record.sh, which points
# linear-tui at the demo workspace and builds the binary first:
#
#   LINEAR_DEMO_TEAM=<team name> demo/record.sh
#
# The script assumes the data from demo/seed.py. Each page change waits for
# text only the loaded page shows, since API latency varies from run to run.

Output assets/demo.gif

Require linear-tui

Set Shell "bash"
Set FontSize 15
Set Width 1440
Set Height 820
Set Padding 16
Set Theme "Catppuccin Mocha"
Set TypingSpeed 90ms
Set CursorBlink false

# Launch off camera so the recording opens on a loaded list.
Hide
Type "clear && linear-tui"
Enter
Wait+Screen@20s /In Progress/
Sleep 500ms
Show

# Team issues, grouped by status.
Sleep 1.5s
Down@350ms 5
Sleep 800ms

# Regroup by assignee and priority, then back to status.
Type "D"
Sleep 1.5s
Type "D"
Sleep 1.5s
Hide
Type "DDD"
Sleep 500ms
Show

# Backlog preset, then back to Active.
Type "gb"
Sleep 2s
Type "ga"
Sleep 1.5s

# Filter as you type, then open the match.
Type "/"
Sleep 400ms
Type "cache"
Sleep 800ms
Enter
Sleep 600ms
Enter
Sleep 2.5s

# Scroll the rendered Markdown down to the comments.
Down@300ms 12
Sleep 1.5s

# Status picker (closed again, so the demo data stays as seeded).
Type "s"
Sleep 1s
Down@400ms 2
Sleep 800ms
Escape
Sleep 500ms

# Back to the list and clear the filter.
Escape
Sleep 500ms
Escape
Sleep 1s

# Projects, and the in-progress one's page (third row).
Type "gp"
Wait+Screen@20s /Radar Performance/
Sleep 1s
Down@400ms 2
Sleep 600ms
Enter
Wait+Screen@20s /In Progress/
Sleep 1.5s
Down@350ms 4
Sleep 1s
Escape
Sleep 800ms

# Cycles, and the current cycle (listed under the upcoming one).
Type "gc"
Wait+Screen@20s /Current/
Sleep 1s
Down
Sleep 600ms
Enter
Wait+Screen@20s /In Progress/
Sleep 2s
Escape
Sleep 800ms

# Saved views, and one of them.
Type "gv"
Wait+Screen@20s /Mobile polish/
Sleep 1s
Enter
Wait+Screen@20s /In Progress/
Sleep 2s
Escape
Sleep 800ms

# Sidebar: walk down to the Favorites.
Tab
Sleep 500ms
Down@350ms 6
Sleep 1s
Tab
Sleep 500ms

# Every shortcut, then quit.
Type "?"
Sleep 2.5s
Escape
Sleep 500ms
Type "q"
Sleep 500ms
