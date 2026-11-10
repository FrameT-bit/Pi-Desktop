#!/usr/bin/env bash
# Install the bundled fonts into the user's fontconfig.
set -euo pipefail
root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
target="${XDG_DATA_HOME:-$HOME/.local/share}/fonts/pi-desktop"
mkdir -p "$target"
install -m644 "$root"/resources/fonts/PixelifySans.ttf "$target/"
install -m644 "$root"/resources/fonts/JetBrainsMono-*.ttf "$target/"
install -m644 "$root"/resources/fonts/*-OFL.txt "$target/"
if command -v fc-cache >/dev/null 2>&1; then
  fc-cache -f "$target" >/dev/null 2>&1 || true
fi
echo "fonts:    $target"
