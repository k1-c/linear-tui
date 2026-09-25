# Command palette demo. Record it through demo/record.sh like the main demo:
#
#   LINEAR_DEMO_TEAM=<team name> demo/record.sh demo/palette.tape
#
# Assumes the data from demo/seed.py, and changes nothing in the workspace:
# the status picker is closed again without choosing.

Output assets/palette.gif

Require linear-tui

Set Shell "bash"
Set FontSize 15
Set Width 1440
Set Height 820
Set Padding 16
Set Theme "Catppuccin Mocha"
Set TypingSpeed 120ms
Set CursorBlink false

# Launch off camera so the recording opens on a loaded list.
Hide
Type "clear && linear-tui"
Enter
Wait+Screen@20s /In Progress/
Sleep 500ms
Show

# Every command available here, each with its shortcut.
Sleep 1s
Ctrl+K
Sleep 1.5s
Down@300ms 5
Sleep 800ms

# Issues too: loaded ones match at once, Linear is searched once typing stops.
Ctrl+U
Type "timezone"
Wait+Screen@10s /Crash when a saved location/
Sleep 1.5s
Enter
Sleep 2.5s

# Letters match in order: "cs" finds Change status, which asks for a value.
# Step back without choosing, so the demo data stays as seeded.
Ctrl+K
Sleep 600ms
Type "cs"
Sleep 1.2s
Enter
Sleep 1.5s
Down@400ms 2
Sleep 800ms
Escape
Sleep 1.2s
Escape
Sleep 800ms
Escape
Sleep 1s

# Group by, picked straight from the palette.
Ctrl+K
Sleep 600ms
Type "gb"
Sleep 1s
Enter
Sleep 1.2s
Type "as"
Sleep 800ms
Enter
Sleep 2s

# Places: a favorite project by name.
Ctrl+K
Sleep 600ms
Type "widgets"
Sleep 1.5s
Enter
Wait+Screen@20s /Lock screen widget/
Sleep 3s
Type "q"
Sleep 500ms
