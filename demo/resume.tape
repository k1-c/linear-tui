# "Picks up where you left off": quit a few pages deep, launch again, and
# linear-tui reopens there. Record it through demo/record.sh:
#
#   LINEAR_DEMO_TEAM=<team name> demo/record.sh demo/resume.tape
#
# Assumes the data from demo/seed.py, and changes nothing in the workspace.
# record.sh gives the run its own state directory, so the relaunch finds only
# the view this recording left.

Output assets/resume.gif

Require linear-tui

Set Shell "bash"
Set FontSize 15
Set Width 1440
Set Height 820
Set Padding 16
Set Theme "Catppuccin Mocha"
Set TypingSpeed 90ms
Set CursorBlink false

Hide
Type "clear && linear-tui"
Enter
Wait+Screen@20s /In Progress/
Sleep 500ms
Show

# A saved view, grouped by priority, and an issue in it.
Sleep 1s
Type "gv"
Wait+Screen@20s /Mobile polish/
Sleep 800ms
Enter
Wait+Screen@20s /In Progress/
Sleep 800ms
Type "D"
Sleep 600ms
Type "D"
Sleep 1s
Down@350ms 2
Sleep 600ms
Enter
Wait+Screen@20s /Properties/
Sleep 2s

# Quit, and start it again.
Ctrl+C
Sleep 1s
Type "linear-tui"
Sleep 400ms
Enter
Wait+Screen@20s /Properties/
Sleep 2.5s

# The way back is there too: the view, still grouped by priority.
Escape
Sleep 2.5s
Ctrl+C
Sleep 500ms
