#!/usr/bin/env bash
# Pi Desktop setup.
#
#   ./setup.sh                 open the graphical setup wizard
#   ./setup.sh --demo          walk through the wizard without changing the system
#   ./setup.sh --dry-run       show the plan, write nothing
#   ./setup.sh --no-gui        install from the terminal
#   ./setup.sh --uninstall     remove the app, shortcut and bundled fonts
#   ./setup.sh --status        report what is installed
#
# Installation is user-level. Nothing here needs root.
set -Eeuo pipefail

SCRIPT_DIR=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)
BACKEND=$SCRIPT_DIR/installer-backend.sh
WIZARD=$SCRIPT_DIR/installer.py
SYSTEM_PYTHON=/usr/bin/python3

DEMO=0
DRY_RUN=0
NO_GUI=0
ACTION=install
EXTRA=()

usage() {
    sed -n '2,11p' "${BASH_SOURCE[0]}" | sed 's/^# \{0,1\}//'
}

wizard_available() {
    [[ -x $SYSTEM_PYTHON ]] || return 1
    "$SYSTEM_PYTHON" -c \
        'import gi; gi.require_version("Gtk", "4.0"); gi.require_version("Adw", "1"); from gi.repository import Gtk, Adw' \
        2>/dev/null
}

while [[ $# -gt 0 ]]; do
    case $1 in
    --demo) DEMO=1 ;;
    --dry-run) DRY_RUN=1 ;;
    --no-gui) NO_GUI=1 ;;
    --uninstall|--remove) ACTION=uninstall ;;
    --status) ACTION=status ;;
    -h|--help) usage; exit 0 ;;
    *) EXTRA+=("$1") ;;
    esac
    shift
done

[[ -x $BACKEND || -f $BACKEND ]] || {
    printf 'installer-backend.sh is missing next to setup.sh.\n' >&2
    exit 1
}

case $ACTION in
status)
    "$BACKEND" status "${EXTRA[@]+"${EXTRA[@]}"}"
    exit $?
    ;;
uninstall)
    "$BACKEND" uninstall "${EXTRA[@]+"${EXTRA[@]}"}"
    exit $?
    ;;
esac

if [[ $DRY_RUN == 1 ]]; then
    if [[ $NO_GUI == 1 ]] || ! wizard_available; then
        "$BACKEND" plan "${EXTRA[@]+"${EXTRA[@]}"}"
        exit $?
    fi
    exec "$SYSTEM_PYTHON" "$WIZARD" --dry-run "${EXTRA[@]+"${EXTRA[@]}"}"
fi

if [[ $NO_GUI == 1 ]]; then
    "$BACKEND" install "${EXTRA[@]+"${EXTRA[@]}"}"
    exit $?
fi

if [[ $DEMO == 1 ]]; then
    if ! wizard_available; then
        printf 'The demo needs GTK4 and libadwaita for %s.\n' "$SYSTEM_PYTHON" >&2
        exit 1
    fi
    exec "$SYSTEM_PYTHON" "$WIZARD" --demo "${EXTRA[@]+"${EXTRA[@]}"}"
fi

if wizard_available; then
    exec "$SYSTEM_PYTHON" "$WIZARD" "${EXTRA[@]+"${EXTRA[@]}"}"
fi

printf 'GTK4/libadwaita are not available for %s; falling back to terminal install.\n' "$SYSTEM_PYTHON" >&2
"$BACKEND" install "${EXTRA[@]+"${EXTRA[@]}"}"
