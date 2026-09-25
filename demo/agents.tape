# Working with a coding agent: linear-tui on the left, the agent's shell on
# the right. Note what you read, then see what the agent reads with
# `linear-tui context` and `linear-tui issue show`. Record it through
# demo/record.sh:
#
#   LINEAR_DEMO_TEAM=<team name> demo/record.sh demo/agents.tape
#
# Assumes the data from demo/seed.py, where the timezone crash is LIN-12, and
# changes nothing in the workspace: the notes go to the clipboard, not to
# Linear. Needs tmux.

Output assets/agents.gif

Require linear-tui
Require tmux

Set Shell "bash"
Set FontSize 14
Set Width 1780
Set Height 860
Set Padding 12
Set Theme "Catppuccin Mocha"
Set TypingSpeed 55ms
Set CursorBlink false

Hide
Type "tmux -f demo/tmux.conf -L lt-demo new-session \; split-window -h -l 72 \; select-pane -L"
Enter
Sleep 1s
Type "clear && linear-tui"
Enter
Wait+Screen@20s /In Progress/
Ctrl+B
Type "o"
Type "PS1='agent \$ ' && clear"
Enter
Ctrl+B
Type "o"
Sleep 500ms
Show

# In linear-tui: find an issue and open it.
Sleep 1s
Type "/"
Sleep 300ms
Type "timezone"
Sleep 700ms
Enter
Sleep 300ms
Enter
Wait+Screen@20s /Activity/
Sleep 1.2s

# A note on the issue, one on the view, and Ctrl+S: outside herdr they are
# copied as one prompt to paste into the agent; inside herdr they go to it.
Type "n"
Sleep 500ms
Type "Reproduce with a location saved before we stored timezones"
Sleep 600ms
Alt+Enter
Sleep 1s
Escape
Sleep 600ms
Type "N"
Sleep 500ms
Type "Same root cause as the widget crash?"
Sleep 600ms
Alt+Enter
Sleep 800ms
Ctrl+S
Sleep 2s
Enter
Wait+Screen@20s /Activity/
Sleep 1s

# In the agent's shell: what is the user looking at?
Ctrl+B
Type "o"
Sleep 600ms
Type "linear-tui context"
Sleep 400ms
Enter
Sleep 4s
Type "clear && linear-tui issue show LIN-12 | head -18"
Sleep 400ms
Enter
Sleep 5s

Hide
Type "tmux -L lt-demo kill-server"
Enter
