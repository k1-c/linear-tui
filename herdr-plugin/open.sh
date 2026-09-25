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
# The pane a split or zoom attaches to: the one the action was invoked from.
target="${HERDR_PANE_ID:-}"
[ -n "$target" ] || target="$(context_json | jq -r '.focused_pane_id // empty')"

case "$PLACEMENT" in
split)
  [ -n "$target" ] || fail "no pane to split"
  case "$DIRECTION" in
  right | down) ;;
  *) fail "DIRECTION must be right or down, not '$DIRECTION'" ;;
  esac
  set -- --placement split --target-pane "$target" --direction "$DIRECTION"
  ;;
zoomed)
  [ -n "$target" ] || fail "no pane to zoom"
  set -- --placement zoomed --target-pane "$target"
  ;;
tab) set -- --placement tab --workspace "$ws" ;;
overlay) set -- --placement overlay ;;
*) fail "PLACEMENT must be overlay, split, tab or zoomed, not '$PLACEMENT'" ;;
esac

opened="$("$H" plugin pane open --plugin "$PLUGIN_ID" --entrypoint tui "$@" \
  --cwd "$cwd" --env "LINEAR_TUI_BIN=$bin" "${open_args[@]}" --focus)" ||
  fail "herdr plugin pane open failed"

# A split opens at half the pane; SIZE (a percentage) moves the divider so
# linear-tui gets that share. `pane resize --amount` moves it by a fraction.
if [ "$PLACEMENT" = split ] && [ -n "$SIZE" ] && [ "$SIZE" != 50 ]; then
  case "$SIZE" in
  [1-9] | [1-9][0-9]) ;;
  *) fail "SIZE must be a percentage from 1 to 99, not '$SIZE'" ;;
  esac
  pane="$(printf '%s' "$opened" | jq -r '.result.plugin_pane.pane.pane_id // empty')"
  [ -n "$pane" ] || exit 0
  # Growing the new pane pushes its divider away from it: left for a pane
  # on the right, up for one below.
  if [ "$SIZE" -gt 50 ]; then
    towards="$([ "$DIRECTION" = down ] && echo up || echo left)"
    amount="$((SIZE - 50))"
  else
    towards="$([ "$DIRECTION" = down ] && echo down || echo right)"
    amount="$((50 - SIZE))"
  fi
  "$H" pane resize --pane "$pane" --direction "$towards" \
    --amount "$(printf '0.%02d' "$amount")" >/dev/null ||
    printf 'linear-tui plugin: could not resize %s to %s%%\n' "$pane" "$SIZE" >&2
fi
