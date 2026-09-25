# Working with a coding agent: note what you see in linear-tui, then show what
# an agent reads with `linear-tui context` and `linear-tui issue show`.
# Record it through demo/record.sh:
#
#   LINEAR_DEMO_TEAM=<team name> demo/record.sh demo/agents.tape
#
# Assumes the data from demo/seed.py, and changes nothing in the workspace:
# the note goes to the clipboard, not to Linear. Needs jq.

Output assets/agents.gif

Require linear-tui
Require jq

Set Shell "bash"
Set FontSize 15
Set Width 1440
Set Height 820
Set Padding 16
Set Theme "Catppuccin Mocha"
Set TypingSpeed 60ms
Set CursorBlink false

Hide
Type "clear && linear-tui"
Enter
Wait+Screen@20s /In Progress/
Sleep 500ms
Show

# Find an issue and open it.
Sleep 1s
Type "/"
Sleep 300ms
Type "timezone"
Sleep 800ms
Enter
Sleep 400ms
Enter
Wait+Screen@20s /Properties/
Sleep 1.5s

# A note for the agent on it, and one on the whole view.
Type "n"
Sleep 600ms
Type "Reproduce with a location saved before we stored timezones"
Sleep 800ms
Alt+Enter
Sleep 1.5s
Escape
Sleep 800ms
Type "N"
Sleep 600ms
Type "Is this the same root cause as the widget crash?"
Sleep 800ms
Alt+Enter
Sleep 1.5s

# Outside herdr, Ctrl+S copies them as one prompt; in herdr it goes to the agent.
Ctrl+S
Sleep 2.5s

# Leave linear-tui on the issue, and look at it the way an agent does.
Enter
Wait+Screen@20s /Properties/
Sleep 1s
Ctrl+C
Sleep 600ms
Type "clear"
Enter
Type "linear-tui context"
Sleep 500ms
Enter
Sleep 4s
Type "clear"
Enter
Type "linear-tui issue show $(linear-tui context --json | jq -r .snapshot.issue.identifier) | head -24"
Sleep 500ms
Enter
Sleep 5s
