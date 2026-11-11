#!/usr/bin/env bash
# User-level installer for Pi Desktop. No root is required: everything goes
# into the current user's XDG directories.
#
# Usage:
#   installer-backend.sh check
#   installer-backend.sh plan
#   installer-backend.sh install [--source DIR] [--no-build] [--no-fonts] [--no-shortcut] [--dry-run]
#   installer-backend.sh uninstall [--dry-run]
#   installer-backend.sh status
#
# Records are line-oriented:
#   STEP|percent|phase|detail
#   CHECK|ok|fail|name|detail
#   STATUS|present|missing|path|detail
#   LOG|text
#   RESULT|ok|fail|detail
set -Eeuo pipefail

SCRIPT_DIR=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)
SOURCE_DIR=$SCRIPT_DIR
MODE=
DRY_RUN=0
WITH_BUILD=1
WITH_FONTS=1
WITH_SHORTCUT=1

DATA_HOME=${XDG_DATA_HOME:-$HOME/.local/share}
BIN_DIR=$HOME/.local/bin
EXEC_PATH=$BIN_DIR/pi-desktop
APPS_DIR=$DATA_HOME/applications
DESKTOP_FILE=$APPS_DIR/io.github.FrameT.PiDesktop.desktop
ICON_THEME=$DATA_HOME/icons/hicolor
FONT_DIR=$DATA_HOME/fonts/pi-desktop
CONFIG_PATH=${XDG_CONFIG_HOME:-$HOME/.config}/pi-desktop/config.ini
TARGET_BIN=$SOURCE_DIR/target/release/pi-desktop

step() { printf 'STEP|%s|%s|%s\n' "$1" "$2" "$3"; }
log() { printf 'LOG|%s\n' "$*"; }
check_ok() { printf 'CHECK|ok|%s|%s\n' "$1" "$2"; }
check_fail() { printf 'CHECK|fail|%s|%s\n' "$1" "$2"; }
status_line() { printf 'STATUS|%s|%s|%s|%s\n' "$1" "$2" "$3" "$4"; }
finish() { printf 'RESULT|%s|%s\n' "$1" "$2"; }
die() { finish fail "$*"; exit 1; }

run() {
    if [[ $DRY_RUN == 1 ]]; then
        log "would run: $*"
        return 0
    fi
    "$@"
}

usage() {
    sed -n '2,18p' "${BASH_SOURCE[0]}" | sed 's/^# \{0,1\}//'
}

args=()
while [[ $# -gt 0 ]]; do
    case $1 in
    --source) SOURCE_DIR=${2:?--source needs a directory}; shift ;;
    --source=*) SOURCE_DIR=${1#*=} ;;
    --dry-run) DRY_RUN=1 ;;
    --no-build) WITH_BUILD=0 ;;
    --no-fonts) WITH_FONTS=0 ;;
    --no-shortcut) WITH_SHORTCUT=0 ;;
    -h|--help) usage; exit 0 ;;
    check|plan|install|uninstall|status) MODE=$1 ;;
    *) die "unknown argument: $1" ;;
    esac
    shift
done
[[ -n $MODE ]] || { usage; exit 2; }
SOURCE_DIR=$(cd -- "$SOURCE_DIR" && pwd)

check_sources() {
    local missing=()
    for file in Cargo.toml src/main.rs resources/fonts/PixelifySans.ttf \
                resources/fonts/JetBrainsMono-Regular.ttf resources/brand/logo-32.png; do
        [[ -r $SOURCE_DIR/$file ]] || missing+=("$file")
    done
    if ((${#missing[@]})); then
        check_fail "Source checkout" "missing: ${missing[*]}"
        return 1
    fi
    check_ok "Source checkout" "$SOURCE_DIR"
}

check_tools() {
    if command -v cargo >/dev/null 2>&1; then
        check_ok "Rust toolchain" "$(cargo --version 2>/dev/null | head -1)"
    else
        check_fail "Rust toolchain" "cargo is not on PATH; install Rust from https://rustup.rs"
        return 1
    fi
    if command -v pkg-config >/dev/null 2>&1 && pkg-config --exists gtk4 2>/dev/null; then
        check_ok "GTK4 development files" "$(pkg-config --modversion gtk4 2>/dev/null)"
    else
        check_fail "GTK4 development files" "install gtk4-devel (Fedora) or libgtk-4-dev (Debian/Ubuntu)"
        return 1
    fi
    if pkg-config --exists libadwaita-1 2>/dev/null; then
        check_ok "libadwaita development files" "$(pkg-config --modversion libadwaita-1 2>/dev/null)"
    else
        check_fail "libadwaita development files" "install libadwaita-devel (Fedora) or libadwaita-1-dev (Debian/Ubuntu)"
        return 1
    fi
}

check_requirements() {
    local ok=0
    check_sources || ok=1
    check_tools || ok=1
    return "$ok"
}

do_check() {
    if check_requirements; then
        finish ok "ready to install"
    else
        finish fail "required dependencies are missing"
        return 1
    fi
}

plan_line() { printf 'STEP|%s|%s|%s\n' "$1" "$2" "$3"; }

do_plan() {
    local ok=0
    check_requirements >/dev/null || ok=1
    plan_line 5 "check" "source, Rust and GTK4/libadwaita"
    if [[ $WITH_BUILD == 1 ]]; then plan_line 30 "build" "cargo build --release"; fi
    plan_line 55 "binary" "$EXEC_PATH"
    if [[ $WITH_FONTS == 1 ]]; then plan_line 70 "fonts" "$FONT_DIR"; fi
    if [[ $WITH_SHORTCUT == 1 ]]; then plan_line 80 "desktop" "$DESKTOP_FILE + icons"; fi
    plan_line 100 "done" "no changes were made"
    if [[ $ok == 0 ]]; then finish ok "plan ready"; else finish fail "required dependencies are missing"; fi
    return "$ok"
}

build_binary() {
    [[ $WITH_BUILD == 1 ]] || { [[ -x $TARGET_BIN ]] || die "no release binary at $TARGET_BIN"; return 0; }
    if [[ $DRY_RUN == 1 ]]; then
        log "would build: cargo build --release"
        return 0
    fi
    log "building release binary (this can take a few minutes)"
    local build_log
    build_log=$(mktemp)
    if ! (cd "$SOURCE_DIR" && cargo build --release >"$build_log" 2>&1); then
        tail -n 20 "$build_log" | while IFS= read -r line; do log "$line"; done
        rm -f "$build_log"
        die "cargo build --release failed"
    fi
    tail -n 5 "$build_log" | while IFS= read -r line; do log "$line"; done
    rm -f "$build_log"
    [[ -x $TARGET_BIN ]] || die "cargo did not produce $TARGET_BIN"
}

install_binary() {
    step 55 "binary" "$EXEC_PATH"
    run install -Dm755 "$TARGET_BIN" "$EXEC_PATH"
}

install_fonts() {
    [[ $WITH_FONTS == 1 ]] || return 0
    step 70 "fonts" "$FONT_DIR"
    run mkdir -p "$FONT_DIR"
    run install -m644 "$SOURCE_DIR"/resources/fonts/PixelifySans.ttf "$FONT_DIR/"
    run install -m644 "$SOURCE_DIR"/resources/fonts/JetBrainsMono-*.ttf "$FONT_DIR/"
    run install -m644 "$SOURCE_DIR"/resources/fonts/*-OFL.txt "$FONT_DIR/"
    if [[ $DRY_RUN == 0 ]] && command -v fc-cache >/dev/null 2>&1; then
        fc-cache -f "$FONT_DIR" >/dev/null 2>&1 || true
    fi
}

install_shortcut() {
    [[ $WITH_SHORTCUT == 1 ]] || return 0
    step 82 "desktop" "$DESKTOP_FILE + icons"
    run mkdir -p "$APPS_DIR"
    if [[ $DRY_RUN == 0 ]]; then
        cat > "$DESKTOP_FILE" <<EOF
[Desktop Entry]
Type=Application
Name=Pi Desktop
Comment=Talk to Pi over RPC
Exec=$EXEC_PATH
Icon=pi-desktop
Terminal=false
Categories=Development;
StartupNotify=true
StartupWMClass=io.github.FrameT.PiDesktop
EOF
        chmod 644 "$DESKTOP_FILE"
    else
        log "would write $DESKTOP_FILE"
    fi
    for size in 32 64 128 256; do
        run install -Dm644 "$SOURCE_DIR/resources/brand/logo-$size.png" \
            "$ICON_THEME/${size}x${size}/apps/pi-desktop.png"
    done
    if [[ $DRY_RUN == 0 ]]; then
        if command -v gtk-update-icon-cache >/dev/null 2>&1; then
            gtk-update-icon-cache -f -t "$ICON_THEME" >/dev/null 2>&1 || true
        fi
        if command -v update-desktop-database >/dev/null 2>&1; then
            update-desktop-database "$APPS_DIR" >/dev/null 2>&1 || true
        fi
    fi
}

do_install() {
    check_requirements >/dev/null || die "required dependencies are missing"
    build_binary
    install_binary
    install_fonts
    install_shortcut
    step 100 "done" "$EXEC_PATH"
    finish ok "Pi Desktop installed at $EXEC_PATH"
}

do_uninstall() {
    step 20 "binary" "$EXEC_PATH"
    run rm -f "$EXEC_PATH"
    step 55 "desktop" "$DESKTOP_FILE"
    run rm -f "$DESKTOP_FILE"
    for size in 32 64 128 256; do
        run rm -f "$ICON_THEME/${size}x${size}/apps/pi-desktop.png"
    done
    step 75 "fonts" "$FONT_DIR"
    run rm -rf "$FONT_DIR"
    if [[ $DRY_RUN == 0 ]] && command -v update-desktop-database >/dev/null 2>&1; then
        update-desktop-database "$APPS_DIR" >/dev/null 2>&1 || true
    fi
    step 100 "done" "configuration kept at $CONFIG_PATH"
    finish ok "Pi Desktop removed; user data and configuration were kept"
}

do_status() {
    status_line "$([[ -x $EXEC_PATH ]] && echo present || echo missing)" "$EXEC_PATH" "" ""
    status_line "$([[ -f $DESKTOP_FILE ]] && echo present || echo missing)" "$DESKTOP_FILE" "" ""
    for size in 32 64 128 256; do
        status_line "$([[ -f $ICON_THEME/${size}x${size}/apps/pi-desktop.png ]] && echo present || echo missing)" \
            "$ICON_THEME/${size}x${size}/apps/pi-desktop.png" "" ""
    done
    status_line "$([[ -d $FONT_DIR ]] && echo present || echo missing)" "$FONT_DIR" "" ""
    status_line "$([[ -f $CONFIG_PATH ]] && echo present || echo missing)" "$CONFIG_PATH" "" "kept on uninstall"
    finish ok "status complete"
}

case $MODE in
check) do_check ;;
plan) do_plan ;;
install) do_install ;;
uninstall) do_uninstall ;;
status) do_status ;;
esac
