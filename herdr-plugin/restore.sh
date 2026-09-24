#!/usr/bin/env bash
# Startup hook: after herdr restores a session, start linear-tui again in the
# panes it was running in when the server stopped.
#
# Those are the view snapshots (docs/view-snapshot.md) that name a herdr pane,
# were never closed, and whose process is gone. linear-tui then reopens its
# view from the snapshot by itself. A pane is only reused when it came back
# as a plain shell in the snapshot's directory.
set -uo pipefail
# shellcheck source=lib.sh
. "$(dirname "$0")/lib.sh"

bin="$(linear_tui_bin)" || exit 0
state="$(linear_tui_state)" || exit 0
[ -d "$state/workspaces" ] || exit 0

# pane<TAB>cwd for every snapshot of an instance that did not quit, newest
# first, at most a week old.
candidates() {
  for file in "$state"/workspaces/*/*.json; do
    [ -e "$file" ] || continue
    jq -r 'select(.herdr_pane != null and .closed_at == null
                  and (now - (.updated_at | fromdateiso8601)) < 604800)
           | [.updated_at, (.pid | tostring), .herdr_pane, .cwd] | @tsv' "$file" 2>/dev/null
  done | sort -r
}

is_shell() {
  "$H" pane process-info --pane "$1" 2>/dev/null |
    jq -e '[.result.process_info.foreground_processes[]?
            | ((.argv0 // (.argv // [])[0] // "") | split("/") | last | ltrimstr("-"))]
           | all(. == "sh" or . == "bash" or . == "zsh" or . == "fish" or . == "nu")' >/dev/null
}

seen=" "
candidates | while IFS=$'\t' read -r _ pid pane cwd; do
  case "$seen" in *" $pane "*) continue ;; esac
  seen="$seen$pane "
  kill -0 "$pid" 2>/dev/null && continue
  pane_cwd="$("$H" pane get "$pane" 2>/dev/null | jq -r '.result.pane | select(.agent == null) | .cwd // empty')"
  [ "$pane_cwd" = "$cwd" ] || continue
  is_shell "$pane" || continue
  "$H" pane run "$pane" "$(printf '%q' "$bin")" >/dev/null ||
    printf 'linear-tui plugin: could not restart linear-tui in %s\n' "$pane" >&2
done
