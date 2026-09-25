#!/usr/bin/env bash
# The `deliver` action: hand over what linear-tui left in its outbox.
#
# linear-tui writes one JSON file per request to $STATE/herdr/outbox/ and then
# invokes this action (the format is in docs/herdr.md). Each file is claimed by
# renaming it, so two deliveries running at once never send one twice.
set -uo pipefail
# shellcheck source=lib.sh
. "$(dirname "$0")/lib.sh"

state="$(linear_tui_state)" || fail "linear-tui is not installed; set LINEAR_TUI_BIN in config.env"
outbox="$state/herdr/outbox"
[ -d "$outbox" ] || exit 0

# Keep a prompt that could not be sent, and say where it is.
keep() {
  local file="$1" why="$2" kept
  mkdir -p "$outbox/undelivered"
  kept="$outbox/undelivered/$(basename "${file%.sending}").md"
  jq -r '.text' "$file" >"$kept"
  notify "linear-tui: notes not sent" "$why — saved to $kept"
  printf 'linear-tui plugin: %s (saved to %s)\n' "$why" "$kept" >&2
}

# The agent a prompt from pane $1 in workspace $2, started in $3, goes to:
# an agent in the same workspace, one in the same directory first, then
# one that is not busy, then the one that changed state last.
pick_agent() {
  "$H" agent list 2>/dev/null | jq -r --arg from "$1" --arg ws "$2" --arg cwd "$3" '
    [.result.agents[]?
     | select(.workspace_id == $ws and .pane_id != $from)
     | . + { rank: [
         (if (.foreground_cwd // .cwd) == $cwd then 0 else 1 end),
         (if .agent_status == "working" or .agent_status == "blocked" then 1 else 0 end),
         -(.state_change_seq // 0)
       ] }]
    | sort_by(.rank) | first | .pane_id // empty'
}

render() {
  local file="$1" template="${HERDR_PLUGIN_CONFIG_DIR:-}/prompt.md"
  if [ -f "$template" ]; then
    jq -r --rawfile t "$template" '
      . as $r | $t
      | split("{{notes}}") | join($r.notes)
      | split("{{view}}") | join($r.view)
      | split("{{hint}}") | join($r.hint)' "$file"
  else
    jq -r '.text' "$file"
  fi
}

deliver_prompt() {
  local file="$1" from ws cwd target text out
  from="$(jq -r '.from.pane // empty' "$file")"
  ws="$(jq -r '.from.workspace // empty' "$file")"
  cwd="$(jq -r '.from.cwd // empty' "$file")"
  [ -n "$ws" ] || ws="${HERDR_WORKSPACE_ID:-}"
  target="$(pick_agent "$from" "$ws" "$cwd")"
  if [ -z "$target" ]; then
    keep "$file" "no agent in this workspace"
    return
  fi
  text="$(render "$file")"
  if out="$("$H" agent prompt "$target" "$text" 2>&1)"; then
    notify "linear-tui" "Notes sent to the agent in $target"
  else
    keep "$file" "the agent in $target did not take them ($(printf '%s' "$out" | jq -r '.error.code // empty' 2>/dev/null || true))"
  fi
}

for file in "$outbox"/*.json; do
  [ -e "$file" ] || continue
  claimed="${file%.json}.sending"
  mv "$file" "$claimed" 2>/dev/null || continue
  case "$(jq -r '.kind // empty' "$claimed")" in
  prompt) deliver_prompt "$claimed" ;;
  focus)
    pane="$(jq -r '.pane // empty' "$claimed")"
    "$H" agent focus "$pane" >/dev/null 2>&1 || notify "linear-tui" "Could not switch to $pane"
    ;;
  *) printf 'linear-tui plugin: unknown request in %s\n' "$claimed" >&2 ;;
  esac
  rm -f "$claimed"
done
