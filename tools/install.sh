#!/usr/bin/env bash
# Pi Desktop installer.
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
data="${XDG_DATA_HOME:-$HOME/.local/share}"
bin_dir="$HOME/.local/bin"
exec_path="$bin_dir/pi-desktop"
app_id_desktop="$data/applications/io.github.FrameT.PiDesktop.desktop"

say() { printf '  %-10s %s\n' "$1" "$2"; }

if [ "${1:-}" = "--uninstall" ]; then
  rm -f "$exec_path" "$app_id_desktop" "$data/applications/pi-desktop.desktop"
  for size in 32 64 128 256; do
    rm -f "$data/icons/hicolor/${size}x${size}/apps/pi-desktop.png"
  done
  command -v update-desktop-database >/dev/null 2>&1 && update-desktop-database "$data/applications" >/dev/null 2>&1 || true
  say removed "$exec_path, launcher and icons"
  say kept "${XDG_CONFIG_HOME:-$HOME/.config}/pi-desktop/config.ini"
  exit 0
fi

profile=release
cargo_flags=(--release)
if [ "${1:-}" = "--debug" ]; then
  profile=debug
  cargo_flags=()
fi

if ! command -v cargo >/dev/null 2>&1; then
  echo "error: cargo is not on PATH. Install the Rust toolchain first." >&2
  exit 1
fi

if ! pkg-config --exists gtk4 2>/dev/null; then
  echo "error: gtk4-devel not found (pkg-config gtk4)." >&2
  echo "       sudo dnf install gtk4-devel libadwaita-devel" >&2
  exit 1
fi

echo "building ($profile)..."
( cd "$root" && cargo build "${cargo_flags[@]}" )

bin="$root/target/$profile/pi-desktop"
[ -x "$bin" ] || { echo "error: $bin was not generated" >&2; exit 1; }

echo "installing..."
"$root/tools/install-icons.sh" "$bin"

say binary "$exec_path"
say version "$("$exec_path" --version 2>/dev/null || grep -m1 '^version' "$root/Cargo.toml" | cut -d'"' -f2)"
say config "untouched: ${XDG_CONFIG_HOME:-$HOME/.config}/pi-desktop/config.ini"
echo
echo "open from the menu (Pi icon) or run: pi-desktop"
