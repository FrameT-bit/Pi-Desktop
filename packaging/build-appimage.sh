#!/usr/bin/env bash
# Build a self-contained Pi Desktop AppImage for x86_64.
set -Eeuo pipefail

root="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)"
version="$(sed -n 's/^version = "\(.*\)"/\1/p' "$root/Cargo.toml" | head -1)"
tools="$root/.tools"
appdir="$root/build/AppDir"
dist="$root/dist"
appimage="$dist/Pi-Desktop-${version}-x86_64.AppImage"

mkdir -p "$tools" "$dist" "$appdir"
rm -rf "$appdir"
mkdir -p "$appdir"/usr/bin "$appdir"/usr/share/applications \
    "$appdir"/usr/share/icons/hicolor/256x256/apps "$appdir"/usr/share/fonts/pi-desktop

fetch() {
    local url=$1 out=$2
    [[ -x $out ]] && return 0
    echo "downloading $(basename "$out")..."
    curl -fsSL --max-time 120 "$url" -o "$out"
    chmod +x "$out"
}

fetch "https://github.com/AppImage/AppImageKit/releases/download/continuous/appimagetool-x86_64.AppImage" "$tools/appimagetool"
fetch "https://github.com/linuxdeploy/linuxdeploy/releases/download/continuous/linuxdeploy-x86_64.AppImage" "$tools/linuxdeploy"
fetch "https://raw.githubusercontent.com/linuxdeploy/linuxdeploy-plugin-gtk/master/linuxdeploy-plugin-gtk.sh" "$tools/linuxdeploy-plugin-gtk.sh"

echo "building release..."
( cd "$root" && cargo build --release )

cp "$root/target/release/pi-desktop" "$appdir/usr/bin/pi-desktop"
cp "$root/resources/brand/logo-256.png" "$appdir/usr/share/icons/hicolor/256x256/apps/pi-desktop.png"
cp "$root/resources/brand/logo-256.png" "$appdir/pi-desktop.png"
cp "$root"/resources/fonts/*.ttf "$root"/resources/fonts/*-OFL.txt "$appdir/usr/share/fonts/pi-desktop/"

cat > "$appdir/usr/share/applications/io.github.FrameT.PiDesktop.desktop" <<EOF
[Desktop Entry]
Type=Application
Name=Pi Desktop
Comment=Talk to Pi over RPC
Exec=pi-desktop
Icon=pi-desktop
Terminal=false
Categories=Development;
StartupNotify=true
StartupWMClass=io.github.FrameT.PiDesktop
EOF

PATH="$tools:$PATH" APPIMAGE_EXTRACT_AND_RUN=1 LINUXDEPLOY_OUTPUT_VERSION="$version" \
    "$tools/linuxdeploy" --appdir "$appdir" \
    --executable "$appdir/usr/bin/pi-desktop" \
    --desktop-file "$appdir/usr/share/applications/io.github.FrameT.PiDesktop.desktop" \
    --icon-file "$appdir/usr/share/icons/hicolor/256x256/apps/pi-desktop.png" \
    --plugin gtk

cat > "$appdir/AppRun" <<'EOF'
#!/bin/sh
set -e
this_dir="$(readlink -f "$(dirname "$0")")"
if [ -r "$this_dir/apprun-hooks/linuxdeploy-plugin-gtk.sh" ]; then
    . "$this_dir/apprun-hooks/linuxdeploy-plugin-gtk.sh"
fi
font_dir="${XDG_DATA_HOME:-$HOME/.local/share}/fonts/pi-desktop"
if [ ! -f "$font_dir/PixelifySans.ttf" ]; then
    mkdir -p "$font_dir"
    cp "$this_dir"/usr/share/fonts/pi-desktop/*.ttf "$font_dir"/
    cp "$this_dir"/usr/share/fonts/pi-desktop/*-OFL.txt "$font_dir"/ 2>/dev/null || true
    if command -v fc-cache >/dev/null 2>&1; then
        fc-cache -f "$font_dir" >/dev/null 2>&1 || true
    fi
fi
exec "$this_dir"/AppRun.wrapped "$@"
EOF
chmod +x "$appdir/AppRun"

echo "packing $appimage..."
APPIMAGE_EXTRACT_AND_RUN=1 ARCH=x86_64 "$tools/appimagetool" "$appdir" "$appimage"
echo "$appimage"
