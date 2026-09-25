#!/usr/bin/env bash
# Keep linear-tui's list of herdr agents current: $STATE/herdr/agents.json.
#
# Run on every agent state change, on pane and worktree lifecycle events, and
# at startup. Each agent is tied to the issue its checkout's branch names
# (`me/eng-42-…`, as Linear suggests branch names). The format is in
# docs/herdr.md.
set -uo pipefail
# shellcheck source=lib.sh
. "$(dirname "$0")/lib.sh"

state="$(linear_tui_state)" || exit 0
agents="$("$H" agent list 2>/dev/null)" || exit 0
labels="$("$H" workspace list 2>/dev/null | jq -c '[.result.workspaces[]? | {key: .workspace_id, value: .label}] | from_entries' 2>/dev/null)"
[ -n "$labels" ] || labels='{}'

# ENG-42 from a branch name, or nothing.
issue_of() {
  local branch
  branch="$(git -C "$1" branch --show-current 2>/dev/null)" || return 0
  printf '%s\n' "$branch" |
    grep -oiE '(^|/)[a-z][a-z0-9]*-[0-9]+([^0-9]|$)' | head -n 1 |
    sed -E 's#^/##; s#[^0-9]$##' | tr '[:lower:]' '[:upper:]'
}

out="$state/herdr/agents.json"
mkdir -p "$(dirname "$out")"
tmp="$(mktemp "$out.XXXXXX")" || exit 1
printf '%s' "$agents" |
  jq -r '.result.agents[]? | [.pane_id, (.foreground_cwd // .cwd // "")] | @tsv' |
  while IFS=$'\t' read -r pane cwd; do
    printf '%s\t%s\n' "$pane" "$([ -n "$cwd" ] && issue_of "$cwd")"
  done |
  jq -R -s --argjson list "$agents" --argjson labels "$labels" '
    (split("\n") | map(select(length > 0) | split("\t") | {key: .[0], value: .[1]}) | from_entries) as $issues
    | { version: 1,
        updated_at: (now | todate),
        agents: [ $list.result.agents[]?
          | { pane: .pane_id,
              workspace: .workspace_id,
              workspace_label: $labels[.workspace_id],
              agent: .agent,
              status: .agent_status,
              cwd: (.foreground_cwd // .cwd),
              issue: (($issues[.pane_id] // "") | if . == "" then null else . end) } ] }' >"$tmp"
if [ -s "$tmp" ]; then
  chmod 600 "$tmp"
  mv "$tmp" "$out"
else
  rm -f "$tmp"
fi
