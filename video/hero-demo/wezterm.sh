#!/usr/bin/env bash
# Open the recording terminal for scene 2 on Windows: WezTerm with wezterm.lua (same look as the
# VHS scenes), inside WSL, already in the demo shell (`dev@build-01`, demo project).
#
#   video/hero-demo/wezterm.sh
#
# Needs WezTerm on Windows (winget install wez.wezterm) and, in WSL, the fonts Geist Mono and
# Noto Sans Symbols 2 under ~/.local/share/fonts (copied next to the config, WezTerm loads them
# from there, nothing is installed on Windows). Run `sandbox.sh up` first.
set -euo pipefail

here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
wez="${WEZTERM_EXE:-/mnt/c/Program Files/WezTerm/wezterm-gui.exe}"
[ -x "$wez" ] || { echo "wezterm.sh: WezTerm not found at $wez (set WEZTERM_EXE)" >&2; exit 1; }

mkdir -p "$here/fonts"
find "$HOME/.local/share/fonts" -iname 'GeistMono-*.ttf' -o -iname 'NotoSansSymbols2-*.ttf' | while read -r f; do
  cp -u "$f" "$here/fonts/"
done
ls "$here/fonts" | grep -q GeistMono || { echo "wezterm.sh: Geist Mono not found in ~/.local/share/fonts" >&2; exit 1; }

config="$(wslpath -w "$here/wezterm.lua")"
# The window starts a non-interactive bash, which has not read ~/.bashrc, so claude and mdview
# are not on PATH yet (and sandbox.sh would exit at once): start it interactive instead.
"$wez" --config-file "$config" start -- bash -ic "exec bash '$here/sandbox.sh' shell" >/dev/null 2>&1 &
disown
echo "WezTerm opened (1920x1080, demo shell). Close it when done."
