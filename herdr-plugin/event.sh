#!/usr/bin/env bash
# Event hook for agent state changes.
#
# Every change refreshes linear-tui's list of agents (agents.sh). With
# NOTIFY_AGENTS=1, an agent that has just started and is waiting for its
# first message is told once how to read what the user is looking at in
# linear-tui. Off by default: the notice takes the agent's first turn, and
# some agents (Claude Code) name the session after it. See docs/herdr.md.
set -uo pipefail
# shellcheck source=lib.sh
. "$(dirname "$0")/lib.sh"

bash "$(dirname "$0")/agents.sh"

[ "$NOTIFY_AGENTS" = 1 ] || exit 0
event="${HERDR_PLUGIN_EVENT_JSON:-}"
[ -n "$event" ] || exit 0
[ "$(printf '%s' "$event" | jq -r '.data.agent_status // empty')" = idle ] || exit 0
pane="$(printf '%s' "$event" | jq -r '.data.pane_id // empty')"
[ -n "$pane" ] || exit 0

agent="$("$H" agent get "$pane" 2>/dev/null)" || exit 0
session="$(printf '%s' "$agent" | jq -r '.result.agent.agent_session.value // .result.agent.terminal_id // empty')"
[ -n "$session" ] || exit 0
marker="${HERDR_PLUGIN_STATE_DIR:?}/notified/$(printf '%s' "$session" | tr -c 'A-Za-z0-9._-' '_')"
[ -e "$marker" ] && exit 0
mkdir -p "$(dirname "$marker")"
: >"$marker"

# An agent that has finished a turn is already in use: interrupting it now
# would be noise, not an introduction.
printf '%s' "$agent" | jq -e '.result.agent.completion_seq == null' >/dev/null || exit 0

"$H" agent prompt "$pane" "$(cat "$(dirname "$0")/notice.md")" >/dev/null ||
  fail "could not send the notice to $pane"
