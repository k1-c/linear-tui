#!/usr/bin/env bash
# Record the demo GIFs in assets/ against the demo workspace.
#
#   demo/record.sh [tape ...]                          # use the linear-tui login
#   demo/record.sh all                                 # every demo/*.tape
#   LINEAR_DEMO_API_KEY=lin_api_... demo/record.sh     # or a demo API key
#
# Without a tape it records demo/demo.tape, the README's main GIF.
#
# linear-tui runs with throwaway config and state directories, so nothing from
# your own config.toml (theme, default team, sidebar width) or a view it
# remembered for this checkout leaks into the recording, and the recording
# leaves no remembered view behind.
# Without LINEAR_DEMO_API_KEY the OAuth login is copied in, which means the
# workspace linear-tui is signed in to is the one that gets recorded.
#
# Set LINEAR_DEMO_TEAM to the team name to open on. Needs vhs, ttyd, and
# ffmpeg on PATH.
set -euo pipefail

cd "$(dirname "$0")/.."
if [[ $# -eq 0 ]]; then
  tapes=(demo/demo.tape)
elif [[ "$1" == all ]]; then
  tapes=(demo/*.tape)
else
  tapes=("$@")
fi

for tool in vhs ttyd ffmpeg; do
  command -v "$tool" >/dev/null || { echo "$tool is not installed" >&2; exit 1; }
done

# directories::ProjectDirs honours XDG_CONFIG_HOME on Linux only; on macOS it
# would read ~/Library/Application Support, so record on Linux.
if [[ "$(uname)" != Linux ]]; then
  echo "record.sh relies on XDG_CONFIG_HOME, which linear-tui only honours on Linux" >&2
  exit 1
fi

own_tokens="${XDG_CONFIG_HOME:-$HOME/.config}/linear-tui/tokens.json"
if [[ -z "${LINEAR_DEMO_API_KEY:-}" && ! -f "$own_tokens" ]]; then
  echo "Set LINEAR_DEMO_API_KEY, or sign in with \`linear-tui auth login\`" >&2
  exit 1
fi

cargo build --release

config_home="$(mktemp -d)"
demo_config="$config_home/linear-tui"
mkdir -p "$demo_config"

if [[ -n "${LINEAR_DEMO_API_KEY:-}" ]]; then
  trap 'rm -rf "$config_home"' EXIT
  auth_line="api_key = \"$LINEAR_DEMO_API_KEY\""
else
  cp -p "$own_tokens" "$demo_config/tokens.json"
  # linear-tui refreshes an expiring token in place, and Linear rotates the
  # refresh token when it does. Copy a refreshed login back so the stored one
  # is not left holding a revoked refresh token.
  trap 'cmp -s "$demo_config/tokens.json" "$own_tokens" ||
          cp -p "$demo_config/tokens.json" "$own_tokens"
        rm -rf "$config_home"' EXIT
  auth_line=""
fi

cat >"$demo_config/config.toml" <<EOF
[auth]
$auth_line

[ui]
${LINEAR_DEMO_TEAM:+default_team = \"$LINEAR_DEMO_TEAM\"}
theme = "default"
EOF

# Each tape starts with no remembered view of its own, so one recording
# cannot reopen where another left off. The recording is not inside herdr even
# when this script is: without HERDR_* the notes go to the clipboard, as they
# do for anyone watching, rather than to the herdr plugin.
for tape in "${tapes[@]}"; do
  state="$config_home/state-$(basename "$tape" .tape)"
  env -u HERDR_BIN_PATH -u HERDR_SOCKET_PATH -u HERDR_ENV -u HERDR_PANE_ID \
    -u HERDR_TAB_ID -u HERDR_WORKSPACE_ID \
    LINEAR_TUI_STATE_DIR="$state" XDG_CONFIG_HOME="$config_home" \
    PATH="$PWD/target/release:$PATH" vhs "$tape"
  echo "Wrote $(grep -m1 '^Output' "$tape" | cut -d' ' -f2)"
done
