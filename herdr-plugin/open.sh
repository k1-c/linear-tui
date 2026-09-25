#!/usr/bin/env bash
# Open linear-tui in a pane.
#
#   open.sh open   focus the workspace's linear-tui pane, or open one in the
#                  focused pane's directory
#   open.sh link   open a new linear-tui pane on the Linear issue URL that was
#                  Ctrl+clicked (a link handler)
set -uo pipefail
# shellcheck source=lib.sh
. "$(dirname "$0")/lib.sh"

mode="${1:-open}"
open_args=()
ws="${HERDR_WORKSPACE_ID:-}"
[ -n "$ws" ] || fail "no workspace context (invoke from inside herdr)"
bin="$(linear_tui_bin)" || fail "linear-tui is not installed; set LINEAR_TUI_BIN in $HERDR_PLUGIN_CONFIG_DIR/config.env"

case "$mode" in
open)
  existing="$(linear_tui_panes "$ws" | head -n 1)"
  if [ -n "$existing" ]; then
    "$H" plugin pane focus "$existing" >/dev/null 2>&1 ||
      "$H" agent focus "$existing" >/dev/null 2>&1 ||
      fail "could not focus $existing"
    exit 0
  fi
  ;;
link)
  url="${HERDR_PLUGIN_CLICKED_URL:-}"
  [ -n "$url" ] || url="$(context_json | jq -r '.clicked_url // empty')"
  [ -n "$url" ] || fail "no clicked URL"
  open_args=(--env "LINEAR_TUI_OPEN=$url")
  ;;
*) fail "unknown mode '$mode' (open | link)" ;;
esac

cwd="$(context_cwd)"
[ -n "$cwd" ] || cwd="$HOME"

case "$PLACEMENT" in
split)
  target="${HERDR_PANE_ID:-}"
  [ -n "$target" ] || fail "no pane to split"
  set -- --placement split --target-pane "$target" --direction "$DIRECTION"
  ;;
zoomed)
  target="${HERDR_PANE_ID:-}"
  [ -n "$target" ] || fail "no pane to zoom"
  set -- --placement zoomed --target-pane "$target"
  ;;
tab) set -- --placement tab --workspace "$ws" ;;
overlay) set -- --placement overlay ;;
*) fail "PLACEMENT must be overlay, split, tab or zoomed, not '$PLACEMENT'" ;;
esac

"$H" plugin pane open --plugin "$PLUGIN_ID" --entrypoint tui "$@" \
  --cwd "$cwd" --env "LINEAR_TUI_BIN=$bin" "${open_args[@]}" --focus >/dev/null ||
  fail "herdr plugin pane open failed"
