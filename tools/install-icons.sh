#!/usr/bin/env bash
# Install the Pi Desktop icon and desktop entry.
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
icons="$root/resources/icons"
data="${XDG_DATA_HOME:-$HOME/.local/share}"
apps="$data/applications"
hicolor="$data/icons/hicolor"
bin_dir="$HOME/.local/bin"
exec_path="$bin_dir/pi-desktop"

src="${1:-}"
if [ -z "$src" ]; then
  for candidate in "$root/target/release/pi-desktop" "$root/target/debug/pi-desktop" "$exec_path"; do
    if [ -x "$candidate" ]; then
      src="$candidate"
      break
    fi
  done
fi
if [ -z "$src" ] || [ ! -x "$src" ]; then
  echo "error: binary not found; run 'cargo build' first." >&2
  exit 1
fi

install -Dm755 "$src" "$exec_path"
"$root/tools/install-fonts.sh" >/dev/null

for size in 32 64 128 256; do
  file="$root/resources/brand/logo-$size.png"
  if [ ! -f "$file" ]; then
    echo "error: missing $file" >&2
    exit 1
  fi
  install -Dm644 "$file" "$hicolor/${size}x${size}/apps/pi-desktop.png"
done

mkdir -p "$apps"
desktop="$apps/io.github.FrameT.PiDesktop.desktop"
rm -f "$apps/pi-desktop.desktop"
cat > "$desktop" <<EOF
[Desktop Entry]
Type=Application
Name=Pi Desktop
Comment=Pi Desktop - talk to Pi over RPC
Exec=$exec_path
Icon=pi-desktop
Terminal=false
Categories=Development;
StartupNotify=true
StartupWMClass=io.github.FrameT.PiDesktop
EOF
chmod 644 "$desktop"

if command -v gtk-update-icon-cache >/dev/null 2>&1; then
  gtk-update-icon-cache -f -t "$hicolor" >/dev/null 2>&1 || true
fi
if command -v update-desktop-database >/dev/null 2>&1; then
  update-desktop-database "$apps" >/dev/null 2>&1 || true
fi

echo "icon:     $hicolor/{32,64,128,256}x*/apps/pi-desktop.png"
echo "launcher: $desktop"
echo "exec:     $exec_path"
