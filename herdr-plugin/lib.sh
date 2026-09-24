# Shared by the linear-tui plugin's scripts. Sourced, never run.
#
# Every herdr call goes through "$H" (HERDR_BIN_PATH), which works whatever
# socket or session herdr is using. Failures are reported on stderr, which
# herdr keeps in the plugin log (`herdr plugin log list --plugin k1-c.linear-tui`).

# herdr runs plugin commands with a minimal PATH; add the usual install
# locations of linear-tui and jq.
PATH="${HOME}/.cargo/bin:${HOME}/.local/bin:${HOME}/.nix-profile/bin:/etc/profiles/per-user/${USER:-}/bin:/run/current-system/sw/bin:/opt/homebrew/bin:/usr/local/bin:${PATH:-/usr/bin:/bin}"
export PATH

H="${HERDR_BIN_PATH:-herdr}"
PLUGIN_ID="${HERDR_PLUGIN_ID:-k1-c.linear-tui}"

# User settings: $HERDR_PLUGIN_CONFIG_DIR/config.env, plain KEY=value lines.
# See docs/herdr.md for the keys.
PLACEMENT=overlay
DIRECTION=right
NOTIFY_AGENTS=0
LINEAR_TUI_BIN="${LINEAR_TUI_BIN:-}"
if [ -n "${HERDR_PLUGIN_CONFIG_DIR:-}" ] && [ -f "$HERDR_PLUGIN_CONFIG_DIR/config.env" ]; then
  # shellcheck disable=SC1091
  . "$HERDR_PLUGIN_CONFIG_DIR/config.env"
fi

fail() {
  printf 'linear-tui plugin: %s\n' "$*" >&2
  exit 1
}

command -v jq >/dev/null 2>&1 || fail "jq is required but not on PATH"

# The linear-tui binary: LINEAR_TUI_BIN, else the first on PATH.
linear_tui_bin() {
  if [ -n "$LINEAR_TUI_BIN" ]; then
    printf '%s\n' "$LINEAR_TUI_BIN"
  else
    command -v linear-tui 2>/dev/null
  fi
}

# The directory the invocation is about: the focused pane's live directory,
# else the workspace's.
context_cwd() {
  local context="${HERDR_PLUGIN_CONTEXT_JSON:-}"
  [ -n "$context" ] || context='{}'
  printf '%s' "$context" | jq -r '.focused_pane_cwd // .workspace_cwd // empty'
}

# Pane ids in workspace $1 whose foreground process is linear-tui.
linear_tui_panes() {
  "$H" pane list --workspace "$1" 2>/dev/null |
    jq -r '.result.panes[]?.pane_id' |
    while IFS= read -r pane; do
      [ -n "$pane" ] || continue
      if "$H" pane process-info --pane "$pane" 2>/dev/null |
        jq -e '[.result.process_info.foreground_processes[]?
                | ((.argv0 // (.argv // [])[0] // "") | split("/") | last)]
               | index("linear-tui") != null' >/dev/null; then
        printf '%s\n' "$pane"
      fi
    done
}

# Show a herdr notification; best effort.
notify() {
  "$H" notification show "$1" --body "${2:-}" >/dev/null 2>&1 || :
}
