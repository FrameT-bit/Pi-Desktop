#!/usr/bin/env bash
# Pi Desktop installer: builds Pi Desktop from source and installs it into the
# current user's home.
#
#   curl -fsSL https://framet-bit.github.io/Pi-Desktop/install.sh | bash
#   curl -fsSL ... | bash -s -- --update
#   curl -fsSL ... | bash -s -- --uninstall
#
# Options:
#   --version X.Y.Z   install a specific release instead of the latest one
#   --update          same as a plain run: rebuild the latest release
#   --from PATH       build a local checkout instead of downloading a release
#   --container       build inside a rootless podman container, never touching
#                     the host packages
#   --deps-only       install the build dependencies and stop
#   --jobs N          compiler parallelism (default: half the cores, 2 to 8)
#   --no-sudo         never call sudo; print the commands instead
#   --background      detach the build and print the log path
#   --dry-run         print the plan and write nothing
#   --yes             accept defaults; never prompt
#   --uninstall       remove the app, keeping configuration and cache
#   --purge           with --uninstall: also remove the build cache
#   --status          report what is installed
#   --self-test       check the installer's own helpers
#   --verbose         stream the complete compiler output
#
# Installation is user-level. Only the GTK4 development packages need root,
# and --container avoids even that.
set -Eeuo pipefail

# Child processes must not inherit translated libc messages. The application
# does the same in src/main.rs. LC_ALL is left alone so UTF-8 paths still work.
export LC_MESSAGES=C
export LANGUAGE=C

readonly APP=pi-desktop
readonly APP_LABEL="Pi Desktop"
readonly REPO=FrameT-bit/Pi-Desktop
readonly PROJECT_URL="https://github.com/$REPO"
readonly MIN_GTK=4.16
readonly MIN_LIBADWAITA=1.5
readonly MIN_RUST=1.83
readonly NEED_KB=3145728
readonly CONTAINER_IMAGE=${PI_DESKTOP_CONTAINER_IMAGE:-docker.io/library/fedora:42}
readonly CONTAINER_PACKAGES="gtk4-devel libadwaita-devel gcc pkgconf-pkg-config patchelf file desktop-file-utils curl tar findutils rust cargo"

BIN_DIR=$HOME/.local/bin
DATA_HOME=${XDG_DATA_HOME:-$HOME/.local/share}
STATE_HOME=${XDG_STATE_HOME:-$HOME/.local/state}
CACHE_HOME=${XDG_CACHE_HOME:-$HOME/.cache}
APP_DATA=$DATA_HOME/pi-desktop
TOOLCHAIN=$APP_DATA/rust
CACHE=$CACHE_HOME/pi-desktop
SRC_DIR=$CACHE/src
TARGET_DIR=$CACHE/target
BUILD_LOG=$CACHE/build.log
STATE_FILE=$STATE_HOME/pi-desktop/install.json
DESKTOP_FILE=$DATA_HOME/applications/io.github.FrameT.PiDesktop.desktop
ICON_DIR=$DATA_HOME/icons/hicolor

VERSION=""
FROM_DIR=""
JOBS=""
ACTION=install
ASSUME_YES=0
DRY_RUN=0
NO_SUDO=0
DEPS_ONLY=0
BACKGROUND=0
VERBOSE=0
WANT_CONTAINER=0
PURGE=0
SELF_TEST=0
METHOD=native
PACKAGE_MANAGER=unknown
DISTRO_ID=""
DISTRO_LIKE=""
DEPS_ATTEMPTED=0
GTK_VERSION=""
LIBADWAITA_VERSION=""
ARCH=$(uname -m)
SUDO=""
RUST_OWNED=0
COMMIT=""
SOURCE_DIR=""
BUILT_BINARY=""
BUILT_IMAGE=""
INSTALLED_PATH=""
APP_DIR=""

if [ -t 1 ] && [ -z "${NO_COLOR:-}" ]; then
    C_RESET=$'\033[0m'; C_BOLD=$'\033[1m'; C_DIM=$'\033[2m'
    C_OK=$'\033[32m'; C_WARN=$'\033[33m'; C_ERR=$'\033[31m'
else
    C_RESET=""; C_BOLD=""; C_DIM=""; C_OK=""; C_WARN=""; C_ERR=""
fi

say() { printf '%s\n' "$*"; }
dim() { printf '%s%s%s\n' "$C_DIM" "$*" "$C_RESET"; }
step() { printf '\n%s==> %s%s\n' "$C_BOLD" "$*" "$C_RESET"; }
ok() { printf '  %s+%s %s\n' "$C_OK" "$C_RESET" "$*"; }
warn() { printf '  %s!%s %s\n' "$C_WARN" "$C_RESET" "$*"; }
die() { printf '\nerror: %s%s%s\n' "$C_ERR" "$*" "$C_RESET" >&2; exit 1; }
have() { command -v "$1" >/dev/null 2>&1; }

trap_trap() {
    local status=$1 line=$2
    printf '\nerror: the installer stopped at line %s (exit %s).\n' "$line" "$status" >&2
    [ -f "$BUILD_LOG" ] && printf 'build log: %s\n' "$BUILD_LOG" >&2
    printf 'please report it at %s/issues\n' "$PROJECT_URL" >&2
    exit "$status"
}
trap 'trap_trap $? $LINENO' ERR

tty_available() { { true >/dev/tty; } 2>/dev/null; }

ask() {
    local prompt=$1 default=${2:-}
    if [ "$ASSUME_YES" = 1 ] || ! tty_available; then
        printf '%s\n' "$default"
        return 0
    fi
    local answer=""
    printf '%s' "$prompt" >/dev/tty 2>/dev/null || { printf '%s\n' "$default"; return 0; }
    IFS= read -r answer </dev/tty 2>/dev/null || true
    printf '%s\n' "${answer:-$default}"
}

numbers_of() {
    local value=${1:-0} out="" part
    local IFS=.
    for part in $value; do
        part=${part%%[!0-9]*}
        out=$out.${part:-0}
    done
    printf '%s\n' "${out#.}" | cut -d. -f1-3
}

version_ge() {
    local have_version want_version idx have_part want_part
    have_version=$(numbers_of "$1")
    want_version=$(numbers_of "$2")
    for idx in 1 2 3; do
        have_part=$(printf '%s' "$have_version" | cut -d. -f"$idx")
        want_part=$(printf '%s' "$want_version" | cut -d. -f"$idx")
        have_part=${have_part:-0}
        want_part=${want_part:-0}
        if [ "$have_part" -gt "$want_part" ]; then return 0; fi
        if [ "$have_part" -lt "$want_part" ]; then return 1; fi
    done
    return 0
}

pkg_version() {
    have pkg-config || return 0
    pkg-config --modversion "$1" 2>/dev/null | head -1 || true
}

read_gtk_versions() {
    GTK_VERSION=$(pkg_version gtk4)
    LIBADWAITA_VERSION=$(pkg_version libadwaita-1)
    if [ -n "${PD_INSTALLER_FAKE_GTK:-}" ]; then
        GTK_VERSION=$PD_INSTALLER_FAKE_GTK
        LIBADWAITA_VERSION=${PD_INSTALLER_FAKE_LIBADWAITA:-$MIN_LIBADWAITA}
    fi
}

gtk_sufficient() {
    [ -n "$GTK_VERSION" ] && version_ge "$GTK_VERSION" "$MIN_GTK" || return 1
    [ -n "$LIBADWAITA_VERSION" ] && version_ge "$LIBADWAITA_VERSION" "$MIN_LIBADWAITA" || return 1
    return 0
}

detect_package_manager() {
    if have dnf; then printf 'dnf\n'
    elif have apt-get; then printf 'apt\n'
    elif have pacman; then printf 'pacman\n'
    elif have zypper; then printf 'zypper\n'
    else printf 'unknown\n'
    fi
}

read_distro() {
    DISTRO_ID=unknown
    DISTRO_LIKE=""
    if [ -r /etc/os-release ]; then
        DISTRO_ID=$(. /etc/os-release; printf '%s' "${ID:-unknown}")
        DISTRO_LIKE=$(. /etc/os-release; printf '%s' "${ID_LIKE:-}")
    fi
}

distro_name() {
    local id=""
    if [ -r /etc/os-release ]; then
        id=$(. /etc/os-release; printf '%s %s' "${NAME:-Linux}" "${VERSION_ID:-}")
    fi
    printf '%s\n' "${id:-Linux}"
}

default_jobs() {
    if [ -n "$JOBS" ]; then printf '%s\n' "$JOBS"; return 0; fi
    local cores jobs
    cores=$(getconf _NPROCESSORS_ONLN 2>/dev/null || printf '2\n')
    jobs=$((cores / 2))
    [ "$jobs" -lt 2 ] && jobs=2
    [ "$jobs" -gt 8 ] && jobs=8
    printf '%s\n' "$jobs"
}

sudo_prepare() {
    if [ "$(id -u)" = 0 ]; then SUDO=""; return 0; fi
    if [ "$NO_SUDO" = 1 ]; then return 0; fi
    if [ "$ASSUME_YES" = 1 ]; then
        sudo -n true 2>/dev/null || die "sudo needs a password; rerun without --yes, or install the development packages yourself."
        SUDO="sudo -n"
        return 0
    fi
    sudo -v || die "sudo authentication failed."
    SUDO="sudo"
}

dependency_hint() {
    case $PACKAGE_MANAGER in
        dnf) printf 'sudo dnf install gtk4-devel libadwaita-devel gcc pkgconf-pkg-config\n' ;;
        apt) printf 'sudo apt-get install libgtk-4-dev libadwaita-1-dev build-essential pkg-config\n' ;;
        pacman) printf 'sudo pacman -S --needed gtk4 libadwaita gcc pkgconf\n' ;;
        zypper) printf 'sudo zypper install gtk4-devel libadwaita-devel gcc pkgconf-pkg-config\n' ;;
        *) printf 'install the GTK4 (>= %s) and libadwaita (>= %s) development packages\n' "$MIN_GTK" "$MIN_LIBADWAITA" ;;
    esac
}

install_dependencies() {
    case $PACKAGE_MANAGER in
        dnf) $SUDO dnf -y install gtk4-devel libadwaita-devel gcc pkgconf-pkg-config ;;
        apt)
            $SUDO env DEBIAN_FRONTEND=noninteractive apt-get update -qq
            $SUDO env DEBIAN_FRONTEND=noninteractive apt-get install -y libgtk-4-dev libadwaita-1-dev build-essential pkg-config
            ;;
        pacman) $SUDO pacman -S --needed --noconfirm gtk4 libadwaita gcc pkgconf ;;
        zypper) $SUDO zypper -n install gtk4-devel libadwaita-devel gcc pkgconf-pkg-config ;;
        *) die "I do not know how to install packages on this system. Run:\n  $(dependency_hint)" ;;
    esac
}

candidate_gtk_version() {
    if [ -n "${PD_INSTALLER_FAKE_CANDIDATE:-}" ]; then
        printf '%s\n' "$PD_INSTALLER_FAKE_CANDIDATE"
        return 0
    fi
    case $PACKAGE_MANAGER in
        dnf) timeout 30 dnf -q info --available gtk4-devel 2>/dev/null | sed -n 's/^Version *: *//p' | head -1 ;;
        apt) timeout 30 apt-cache policy libgtk-4-dev 2>/dev/null | sed -n 's/^ *Candidate: *//p' | head -1 ;;
        pacman) timeout 30 pacman -Si gtk4 2>/dev/null | sed -n 's/^Version *: *//p' | head -1 ;;
        zypper) timeout 30 zypper -q info gtk4-devel 2>/dev/null | sed -n 's/^Version *: *//p' | head -1 ;;
    esac
}

upgrade_hint() {
    case $DISTRO_ID in
        ubuntu) say "  Ubuntu: sudo do-release-upgrade    (24.04 -> 24.10 -> 25.04)" ;;
        debian) say "  Debian: upgrade to 13 (trixie), which ships GTK4 4.18" ;;
        fedora) say "  Fedora: sudo dnf system-upgrade download --releasever=42 && sudo dnf system-upgrade reboot" ;;
        arch) say "  Arch:   sudo pacman -Syu" ;;
        opensuse*|suse*) say "  openSUSE: sudo zypper dup" ;;
        *)
            case $PACKAGE_MANAGER in
                dnf) say "  Fedora: sudo dnf system-upgrade download --releasever=42 && sudo dnf system-upgrade reboot" ;;
                apt) say "  Use Debian 13 or newer, or Ubuntu 25.04 or newer." ;;
                pacman) say "  Arch: sudo pacman -Syu" ;;
                *) say "  Use a distribution that ships GTK4 $MIN_GTK or newer." ;;
            esac
            ;;
    esac
    say "  Supported without extra work: Fedora 41+, Ubuntu 25.04+, Debian 13+, Arch, openSUSE Tumbleweed."
}

require_podman() {
    have podman || die "podman is not installed. Install podman, or upgrade to a supported release."
    ok "podman $(podman --version 2>/dev/null | awk '{print $3}')"
}

decide_without_gtk() {
    local current=${GTK_VERSION:-not installed}
    local candidate
    candidate=$(candidate_gtk_version)
    if [ -n "$candidate" ] && version_ge "$candidate" "$MIN_GTK"; then
        METHOD=native
        return 0
    fi
    if [ -z "$candidate" ] && [ "$DEPS_ATTEMPTED" = 0 ]; then
        METHOD=native
        return 0
    fi
    step "GTK4 $MIN_GTK or newer is required"
    if [ "$current" = "not installed" ]; then
        say "  This system has no GTK4 development files, and its packages are older than $MIN_GTK."
    else
        say "  This system provides GTK4 $current, libadwaita ${LIBADWAITA_VERSION:-none}."
    fi
    say "  $APP_LABEL needs GTK4 $MIN_GTK or newer, and that is a distribution package."
    say ""
    if ! tty_available || [ "$ASSUME_YES" = 1 ]; then
        upgrade_hint
        die "cannot build on this system. Upgrade it, or pass --container to build in podman."
    fi
    say "  ${C_BOLD}1)${C_RESET} Upgrade to a supported release ${C_DIM}(recommended, nothing is written)${C_RESET}"
    say "  ${C_BOLD}2)${C_RESET} Build inside a rootless podman container, leaving this system untouched"
    say ""
    local answer
    answer=$(ask "  Choose [1]: " 1)
    case $answer in
        1) upgrade_hint; say ""; say "Nothing was installed."; exit 0 ;;
        2) require_podman; METHOD=container; ok "using the container build" ;;
        *) die "cancelled." ;;
    esac
}

ensure_dependencies() {
    read_gtk_versions
    if gtk_sufficient; then
        ok "GTK4 $GTK_VERSION, libadwaita $LIBADWAITA_VERSION"
        return 0
    fi
    if [ "$NO_SUDO" = 1 ] && [ "$(id -u)" != 0 ]; then
        warn "development packages are missing and --no-sudo was given"
        say ""
        say "  $(dependency_hint)"
        die "install them and run the installer again."
    fi
    step "Installing the GTK4 development packages"
    DEPS_ATTEMPTED=1
    sudo_prepare
    install_dependencies
    read_gtk_versions
    gtk_sufficient || decide_without_gtk
    ok "GTK4 $GTK_VERSION, libadwaita $LIBADWAITA_VERSION"
}

resolve_version() {
    if [ -n "$FROM_DIR" ]; then
        [ -f "$FROM_DIR/Cargo.toml" ] || die "$FROM_DIR does not look like the $APP_LABEL source tree."
        FROM_DIR=$(cd "$FROM_DIR" && pwd)
        VERSION=$(sed -n 's/^version = "\(.*\)"/\1/p' "$FROM_DIR/Cargo.toml" | head -1)
        [ -n "$VERSION" ] || die "cannot read the version from $FROM_DIR/Cargo.toml"
        COMMIT=$(git -C "$FROM_DIR" rev-parse --short HEAD 2>/dev/null || printf 'local')
        ok "building $VERSION from $FROM_DIR ($COMMIT)"
        return 0
    fi
    if [ -n "$VERSION" ]; then
        ok "release $VERSION requested"
        return 0
    fi
    step "Resolving the latest release"
    local response tags
    response=$(curl -fsSL --max-time 20 -H 'User-Agent: Pi-Desktop-Installer' \
        "https://api.github.com/repos/$REPO/releases?per_page=100") ||
        die "cannot reach GitHub. Check the network, or pass --version X.Y.Z."
    tags=$(printf '%s' "$response" | grep -oE '"tag_name":[[:space:]]*"v[0-9][0-9.]*"' | sed 's/.*"v//; s/"//' | sort -V)
    VERSION=$(printf '%s\n' "$tags" | tail -1)
    [ -n "$VERSION" ] || die "no published release found for $REPO."
    ok "latest release is $VERSION"
}

fetch_tag_with_git() {
    local tag=$1 dir=$2
    git init --quiet "$dir" || return 1
    git -C "$dir" remote add origin "$PROJECT_URL.git" || return 1
    git -C "$dir" -c advice.detachedHead=false fetch --quiet --depth 1 origin "refs/tags/$tag" || return 1
    git -C "$dir" -c advice.detachedHead=false checkout --quiet FETCH_HEAD || return 1
}

ensure_source() {
    if [ -n "$FROM_DIR" ]; then
        SOURCE_DIR=$FROM_DIR
        return 0
    fi
    SOURCE_DIR=$SRC_DIR/v$VERSION
    if [ -d "$SOURCE_DIR/.git" ] && git -C "$SOURCE_DIR" rev-parse -q --verify HEAD >/dev/null 2>&1; then
        COMMIT=$(git -C "$SOURCE_DIR" rev-parse --short HEAD)
        ok "source already cached at $SOURCE_DIR"
        return 0
    fi
    step "Downloading the $VERSION source"
    mkdir -p "$SRC_DIR"
    rm -rf "$SOURCE_DIR"
    if have git; then
        fetch_tag_with_git "v$VERSION" "$SOURCE_DIR" ||
            die "cannot fetch tag v$VERSION from $PROJECT_URL."
        COMMIT=$(git -C "$SOURCE_DIR" rev-parse --short HEAD)
    else
        curl -fsSL --max-time 120 "https://codeload.github.com/$REPO/tar.gz/refs/tags/v$VERSION" |
            tar xz -C "$SRC_DIR" || die "cannot download the v$VERSION source archive."
        mv "$SRC_DIR/Pi-Desktop-$VERSION" "$SOURCE_DIR"
        COMMIT="tarball"
    fi
    ok "source in $SOURCE_DIR ($COMMIT)"
}

ensure_rust() {
    local current=""
    if have rustc; then
        current=$(rustc --version 2>/dev/null | awk '{print $2}')
    fi
    if [ -n "$current" ] && version_ge "$current" "$MIN_RUST"; then
        ok "rustc $current"
        return 0
    fi
    step "Installing a Rust toolchain (${MIN_RUST}+)"
    say "  It goes to $TOOLCHAIN and is removed again by --uninstall."
    mkdir -p "$TOOLCHAIN" "$CACHE"
    local bootstrap=$CACHE/rustup-init.sh
    curl --proto '=https' --tlsv1.2 -fsSL --max-time 120 https://sh.rustup.rs -o "$bootstrap" ||
        die "cannot download rustup. Install Rust $MIN_RUST+ yourself and rerun."
    env RUSTUP_HOME=$TOOLCHAIN/rustup CARGO_HOME=$TOOLCHAIN/cargo \
        sh "$bootstrap" -y --profile minimal --no-modify-path --default-toolchain stable >/dev/null ||
        die "rustup failed; see $BUILD_LOG"
    RUST_OWNED=1
    export RUSTUP_HOME=$TOOLCHAIN/rustup
    export CARGO_HOME=$TOOLCHAIN/cargo
    export PATH=$TOOLCHAIN/cargo/bin:$PATH
    ok "rustc $(cargo --version | awk '{print $2}') in $TOOLCHAIN"
}

low_priority() {
    local -a prefix=()
    have nice && prefix+=(nice -n 15)
    have ionice && prefix+=(ionice -c 3)
    printf '%s\n' "${prefix[@]+"${prefix[@]}"}"
}

filter_build_output() {
    grep --line-buffered -E '^ *(Compiling) |^ *(Finished) |^error|error\[' || true
}

build_native() {
    step "Compiling $APP_LABEL (this is the slow part)"
    mkdir -p "$TARGET_DIR" "$(dirname "$BUILD_LOG")"
    local jobs prefix_line rc=0
    jobs=$(default_jobs)
    prefix_line=$(low_priority)
    say "  $jobs jobs at low priority; log: $BUILD_LOG"
    say "  Ctrl-C is safe: rerunning this command continues from here."
    set +e
    (
        cd "$SOURCE_DIR" || exit 1
        # shellcheck disable=SC2086
        CARGO_TARGET_DIR=$TARGET_DIR env $prefix_line cargo build --release --jobs "$jobs"
    ) 2>&1 | tee -a "$BUILD_LOG" | {
        if [ "$VERBOSE" = 1 ]; then cat; else filter_build_output; fi
    }
    rc=${PIPESTATUS[0]}
    set -e
    [ "$rc" = 0 ] || die "the build failed. Full output: $BUILD_LOG"
    BUILT_BINARY=$TARGET_DIR/release/$APP
    [ -x "$BUILT_BINARY" ] || die "the build produced no binary at $BUILT_BINARY"
    ok "binary: $BUILT_BINARY"
}

build_in_container() {
    step "Building inside podman ($CONTAINER_IMAGE)"
    say "  No system packages are touched. First run pulls the image and the tools."
    mkdir -p "$(dirname "$BUILD_LOG")"
    rm -rf "$SOURCE_DIR/dist" "$SOURCE_DIR/build"
    local rc=0
    set +e
    podman run --rm \
        -v "$SOURCE_DIR:/src:rw,z" -w /src \
        -v "$CACHE/container-target:/target:rw,z" \
        -e CARGO_TARGET_DIR=/target \
        -e APPIMAGE_EXTRACT_AND_RUN=1 \
        "$CONTAINER_IMAGE" bash -lc \
        "dnf -y install $CONTAINER_PACKAGES >/tmp/dnf.log 2>&1 && bash packaging/build-appimage.sh" \
        2>&1 | tee -a "$BUILD_LOG" | {
        if [ "$VERBOSE" = 1 ]; then cat; else filter_build_output; fi
    }
    rc=${PIPESTATUS[0]}
    set -e
    [ "$rc" = 0 ] || die "the container build failed. Full output: $BUILD_LOG"
    BUILT_IMAGE=$(printf '%s\n' "$SOURCE_DIR"/dist/*.AppImage 2>/dev/null | tail -1)
    [ -f "$BUILT_IMAGE" ] || die "the container build produced no AppImage."
    ok "bundle: $BUILT_IMAGE"
}

glibc_host() {
    ldd --version 2>/dev/null | sed -n '1s/.* //p' || printf '0\n'
}

glibc_required() {
    objdump -p "$1" 2>/dev/null | grep -o 'GLIBC_[0-9.]*' | sed 's/GLIBC_//' | sort -V | tail -1 || true
}

glibc_required_tree() {
    local root=$1 highest="" candidate file
    while IFS= read -r file; do
        candidate=$(glibc_required "$file")
        [ -n "$candidate" ] || continue
        if [ -z "$highest" ] || ! version_ge "$highest" "$candidate"; then
            highest=$candidate
        fi
    done < <(find "$root" -type f \( -name '*.so' -o -name '*.so.*' -o -perm -u+x \) 2>/dev/null)
    printf '%s\n' "$highest"
}

glibc_compatible() {
    local required=$1 host_version=$2
    [ -z "$required" ] && return 0
    version_ge "$host_version" "$required"
}

check_runtime_compatibility() {
    local root=$1 host required
    have objdump || { warn "objdump is missing; skipping the runtime check"; return 0; }
    host=$(glibc_host)
    required=$(glibc_required_tree "$root")
    if glibc_compatible "$required" "$host"; then
        ok "needs glibc ${required:-any}, this system has $host"
        return 0
    fi
    printf '\nerror: this bundle needs glibc %s but the system has %s.\n' "$required" "$host" >&2
    printf 'The container image (%s) is newer than this system.\n' "$CONTAINER_IMAGE" >&2
    printf 'Upgrading the distribution is the supported path here.\n' >&2
    exit 1
}

install_native() {
    step "Installing"
    local built
    built=$(version_of_binary "$BUILT_BINARY")
    [ "$built" = "$VERSION" ] || die "built version $built does not match $VERSION"
    bash "$SOURCE_DIR/tools/install-icons.sh" "$BUILT_BINARY"
    if [ -d "$APP_DATA/opt" ]; then rm -rf "$APP_DATA/opt"; ok "removed the previous container bundle"; fi
    INSTALLED_PATH=$BIN_DIR/$APP
    APP_DIR=""
}

install_bundle() {
    step "Installing from the bundle"
    APP_DIR=$APP_DATA/opt/$VERSION
    local scratch
    scratch=$(mktemp -d)
    ( cd "$scratch" && "$BUILT_IMAGE" --appimage-extract >/dev/null ) ||
        die "cannot extract $BUILT_IMAGE"
    rm -rf "$APP_DIR"
    mkdir -p "$(dirname "$APP_DIR")"
    mv "$scratch/squashfs-root" "$APP_DIR"
    rm -rf "$scratch"
    check_runtime_compatibility "$APP_DIR/usr"
    mkdir -p "$BIN_DIR"
    cat > "$BIN_DIR/$APP" <<WRAPPER
#!/bin/sh
exec "$APP_DIR/AppRun" "\$@"
WRAPPER
    chmod 755 "$BIN_DIR/$APP"
    install_icon_and_launcher
    INSTALLED_PATH=$BIN_DIR/$APP
}

install_icon_and_launcher() {
    local source_icon=$APP_DIR/$APP.png size
    [ -f "$source_icon" ] || source_icon=$APP_DIR/usr/share/icons/hicolor/256x256/apps/$APP.png
    [ -f "$source_icon" ] || return 0
    for size in 32 64 128 256; do
        install -Dm644 "$source_icon" "$ICON_DIR/${size}x${size}/apps/$APP.png"
    done
    mkdir -p "$(dirname "$DESKTOP_FILE")"
    cat > "$DESKTOP_FILE" <<DESKTOP
[Desktop Entry]
Type=Application
Name=$APP_LABEL
Comment=$APP_LABEL - talk to Pi over RPC
Exec=$INSTALLED_PATH
Icon=$APP
Terminal=false
Categories=Development;
StartupNotify=true
StartupWMClass=io.github.FrameT.PiDesktop
DESKTOP
    chmod 644 "$DESKTOP_FILE"
    have gtk-update-icon-cache && gtk-update-icon-cache -f -t "$ICON_DIR" >/dev/null 2>&1 || true
    have update-desktop-database && update-desktop-database "$(dirname "$DESKTOP_FILE")" >/dev/null 2>&1 || true
}

version_of_binary() {
    "$1" --version 2>/dev/null | awk '{print $NF}' | head -1 || true
}

report_runtime_failure() {
    local binary=$1 missing
    have ldd || return 0
    missing=$(ldd "$binary" 2>&1 | grep -E 'not found|version .* not found' | head -4 || true)
    if [ -n "$missing" ]; then
        say ""
        warn "this system cannot load the installed binary:"
        printf '    %s\n' $missing
    fi
}

write_state() {
    mkdir -p "$(dirname "$STATE_FILE")"
    cat > "$STATE_FILE" <<STATE
{
  "method": "$1",
  "version": "$VERSION",
  "commit": "$COMMIT",
  "installed_at": "$(date -u +%Y-%m-%dT%H:%M:%SZ)",
  "binary": "$INSTALLED_PATH",
  "app_dir": "${APP_DIR:-}",
  "source": "$SOURCE_DIR",
  "cache": "$CACHE",
  "toolchain_owned": $RUST_OWNED
}
STATE
}

check_pi_cli() {
    local found
    if ! found=$(command -v pi 2>/dev/null); then
        warn "the pi CLI is not on PATH; $APP_LABEL talks to it over RPC"
        say "    npm install -g @earendil-works/pi-coding-agent"
        return 0
    fi
    ok "pi CLI: $found"
    case $found in
        "$HOME/.local/bin/pi") ;;
        *) warn "the app expects ~/.local/bin/pi by default; set bin= in ~/.config/$APP/config.ini to $found" ;;
    esac
}

check_path() {
    case ":$PATH:" in
        *":$BIN_DIR:"*) ;;
        *) warn "$BIN_DIR is not on PATH; add this to your shell profile:"
           say "    export PATH=\"\$HOME/.local/bin:\$PATH\"" ;;
    esac
}

report() {
    step "Done"
    ok "version $VERSION ($1)"
    ok "command $INSTALLED_PATH"
    if [ -n "$APP_DIR" ]; then ok "bundle  $APP_DIR"; fi
    say ""
    dim "  run:        $APP"
    dim "  update:     rerun this installer with --update"
    dim "  uninstall:  rerun this installer with --uninstall"
}

print_plan() {
    step "Plan"
    say "  version     $VERSION"
    local method_label=$METHOD
    if [ "$METHOD" = container ]; then method_label="$METHOD ($CONTAINER_IMAGE)"; fi
    say "  method      $method_label"
    say "  source      ${SOURCE_DIR:-$SRC_DIR/v$VERSION}"
    say "  cache       $CACHE"
    say "  installs    $BIN_DIR/$APP"
    say "  launcher    $DESKTOP_FILE"
    say "  jobs        $(default_jobs)"
    local packages_label='none on the host'
    if [ "$METHOD" = native ]; then packages_label="GTK4 $MIN_GTK+, libadwaita $MIN_LIBADWAITA+"; fi
    say "  packages    $packages_label"
    say "  system      $(distro_name), GTK4 ${GTK_VERSION:-none}, libadwaita ${LIBADWAITA_VERSION:-none}"
    say "  estimate    4-8 minutes and about 1 GB of disk on a first install"
    say ""
    say "  dry run: nothing was written."
}

check_disk() {
    local target=$HOME available
    available=$(df -Pk "$target" 2>/dev/null | awk 'NR==2 {print $4}')
    [ -n "$available" ] || return 0
    if [ "$available" -lt "$NEED_KB" ]; then
        die "need about $((NEED_KB / 1024)) MB free in $target, found $((available / 1024)) MB."
    fi
}

check_platform() {
    [ "$(uname -s)" = Linux ] || die "$APP_LABEL builds on Linux only."
    case $ARCH in
        x86_64|aarch64) ;;
        *) warn "untested architecture: $ARCH" ;;
    esac
    have curl || die "curl is required."
    have tar || die "tar is required."
    if [ "$METHOD" = container ] && [ "$ARCH" != x86_64 ]; then
        die "the container build produces an x86_64 bundle only; build natively on $ARCH."
    fi
    check_disk
}

detach() {
    if [ "$BACKGROUND" = 0 ] || [ "$DRY_RUN" = 1 ]; then return 0; fi
    if [ ! -f "$0" ]; then
        warn "--background needs the script on disk; saving it is enough:"
        say "    curl -fsSL https://framet-bit.github.io/Pi-Desktop/install.sh -o install.sh && bash install.sh --background"
        BACKGROUND=0
        return 0
    fi
    mkdir -p "$CACHE"
    setsid bash "$0" ${ORIGINAL_ARGS[@]+"${ORIGINAL_ARGS[@]}"} --_child </dev/null >>"$BUILD_LOG" 2>&1 &
    say "building in the background (pid $!); follow it with:"
    say "    tail -f $BUILD_LOG"
    say "when it finishes: $BIN_DIR/$APP --version"
    exit 0
}

run_install() {
    step "$APP_LABEL installer"
    say "  builds from source into $HOME; only the GTK4 development packages need root"
    detach
    check_platform
    resolve_version
    SOURCE_DIR=${FROM_DIR:-$SRC_DIR/v$VERSION}
    read_gtk_versions
    if [ "$WANT_CONTAINER" = 1 ]; then
        require_podman
        METHOD=container
    elif gtk_sufficient; then
        METHOD=native
    else
        decide_without_gtk
    fi
    if [ "$DRY_RUN" = 1 ]; then print_plan; exit 0; fi
    check_platform
    if [ "$METHOD" = native ]; then
        ensure_dependencies
        if [ "$DEPS_ONLY" = 1 ]; then ok "development packages ready"; exit 0; fi
        ensure_rust
    fi
    ensure_source
    if [ "$METHOD" = container ]; then
        build_in_container
        install_bundle container
    else
        build_native
        install_native
    fi
    local method_name=source
    if [ "$METHOD" = container ]; then method_name=bundle; fi
    write_state "$method_name"
    local reported
    reported=$(version_of_binary "$INSTALLED_PATH")
    if [ "$reported" != "$VERSION" ]; then
        report_runtime_failure "$INSTALLED_PATH"
        die "the installed binary reports '${reported:-nothing}', expected $VERSION"
    fi
    check_pi_cli
    check_path
    local origin='compiled here'
    if [ "$METHOD" = container ]; then origin='built in a container'; fi
    report "$origin"
}

run_status() {
    step "$APP_LABEL status"
    if [ -f "$STATE_FILE" ]; then
        sed 's/^/  /' "$STATE_FILE"
    else
        say "  no install record in $STATE_FILE"
    fi
    say ""
    if [ -x "$BIN_DIR/$APP" ]; then ok "binary $BIN_DIR/$APP"; else warn "no binary at $BIN_DIR/$APP"; fi
    if [ -f "$DESKTOP_FILE" ]; then ok "launcher $DESKTOP_FILE"; else warn "no launcher at $DESKTOP_FILE"; fi
    if [ -d "$CACHE" ]; then ok "cache $(du -sh "$CACHE" 2>/dev/null | cut -f1) in $CACHE"; fi
}

run_uninstall() {
    step "Removing $APP_LABEL"
    local app_dir=""
    if [ -f "$STATE_FILE" ]; then
        app_dir=$(sed -n 's/.*"app_dir": *"\([^"]*\)".*/\1/p' "$STATE_FILE")
    fi
    rm -f "$BIN_DIR/$APP" && ok "removed $BIN_DIR/$APP"
    rm -f "$DESKTOP_FILE" && ok "removed the launcher"
    local size
    for size in 32 64 128 256; do rm -f "$ICON_DIR/${size}x${size}/apps/$APP.png"; done
    ok "removed the icons"
    if [ -n "$app_dir" ] && [ -d "$app_dir" ]; then rm -rf "$(dirname "$app_dir")"; ok "removed the bundle"; fi
    rm -rf "$APP_DATA/rust"
    rmdir "$APP_DATA/opt" 2>/dev/null || true
    if rmdir "$APP_DATA" 2>/dev/null; then ok "removed $APP_DATA"; fi
    rm -f "$STATE_FILE"
    if [ "$PURGE" = 1 ] && [ -d "$CACHE" ]; then
        rm -rf "$CACHE"; ok "removed the build cache"
    elif [ -d "$CACHE" ]; then
        say "  kept the build cache in $CACHE (use --purge to remove it)"
    fi
    say "  kept your settings in ${XDG_CONFIG_HOME:-$HOME/.config}/$APP"
    have update-desktop-database && update-desktop-database "$(dirname "$DESKTOP_FILE")" >/dev/null 2>&1 || true
    have gtk-update-icon-cache && gtk-update-icon-cache -f -t "$ICON_DIR" >/dev/null 2>&1 || true
    step "Removed"
}

run_self_test() {
    local failures=0
    trap - ERR
    expect() {
        local label=$1 expected=$2 actual=$3
        if [ "$expected" = "$actual" ]; then printf '  ok   %s\n' "$label"
        else printf '  FAIL %s (expected %s, got %s)\n' "$label" "$expected" "$actual"; failures=$((failures + 1)); fi
    }
    ge() { if version_ge "$1" "$2"; then printf '0\n'; else printf '1\n'; fi; }
    check() {
        local label=$1
        shift
        if "$@"; then printf '  ok   %s\n' "$label"
        else printf '  FAIL %s\n' "$label"; failures=$((failures + 1)); fi
    }
    step "self test"
    expect "version_ge 4.16 >= 4.16" 0 "$(ge 4.16 4.16)"
    expect "version_ge 4.18 >= 4.16" 0 "$(ge 4.18 4.16)"
    expect "version_ge 4.22.5 >= 4.16" 0 "$(ge 4.22.5 4.16)"
    expect "version_ge 4.14.5 < 4.16" 1 "$(ge 4.14.5 4.16)"
    expect "version_ge 4.8.3 < 4.16" 1 "$(ge 4.8.3 4.16)"
    expect "version_ge 1.5 >= 1.5" 0 "$(ge 1.5 1.5)"
    expect "version_ge 1.2.0 < 1.5" 1 "$(ge 1.2.0 1.5)"
    expect "version_ge 10.0 >= 9.9" 0 "$(ge 10.0 9.9)"
    expect "numbers_of 4.14.5+ds-0ubuntu0.4" 4.14.5 "$(numbers_of 4.14.5+ds-0ubuntu0.4)"
    expect "numbers_of 4.18.6+ds-2" 4.18.6 "$(numbers_of 4.18.6+ds-2)"
    expect "numbers_of 1.7.6-1~deb13u1" 1.7.6 "$(numbers_of 1.7.6-1~deb13u1)"
    expect "numbers_of 4.22.5" 4.22.5 "$(numbers_of 4.22.5)"
    check "gtk_sufficient false for 4.14" bash -c 'GTK_VERSION=4.14.5 LIBADWAITA_VERSION=1.5.0; ! gtk_sufficient'
    check "gtk_sufficient true for 4.18" bash -c 'GTK_VERSION=4.18.5 LIBADWAITA_VERSION=1.7.0; gtk_sufficient'
    check "gtk_sufficient false when libadwaita is old" bash -c 'GTK_VERSION=4.18.5 LIBADWAITA_VERSION=1.2; ! gtk_sufficient'
    check "gtk_sufficient false when missing" bash -c 'GTK_VERSION="" LIBADWAITA_VERSION=""; ! gtk_sufficient'
    check "default_jobs is at least 2" bash -c '[ "$(default_jobs)" -ge 2 ]'
    check "default_jobs honours --jobs" bash -c 'JOBS=3; [ "$(default_jobs)" = 3 ]'
    check "glibc 2.39 fits a 2.43 host" bash -c 'glibc_compatible 2.39 2.43'
    check "glibc 2.43 fits a 2.43 host" bash -c 'glibc_compatible 2.43 2.43'
    check "glibc 2.44 does not fit a 2.43 host" bash -c '! glibc_compatible 2.44 2.43'
    check "missing requirement always fits" bash -c 'glibc_compatible "" 2.43'
    trap 'trap_trap $? $LINENO' ERR
    say ""
    if [ "$failures" = 0 ]; then ok "all checks passed"; return 0; fi
    die "$failures check(s) failed"
}

usage() {
    cat <<'USAGE'
Pi Desktop installer: builds from source and installs into the current user's home.

  curl -fsSL https://framet-bit.github.io/Pi-Desktop/install.sh | bash
  curl -fsSL ... | bash -s -- --update
  curl -fsSL ... | bash -s -- --uninstall

Options:
  --version X.Y.Z   install a specific release instead of the latest one
  --update          same as a plain run: rebuild the latest release
  --from PATH       build a local checkout instead of downloading a release
  --container       build inside a rootless podman container
  --deps-only       install the build dependencies and stop
  --jobs N          compiler parallelism (default: half the cores, 2 to 8)
  --no-sudo         never call sudo; print the commands instead
  --background      detach the build and print the log path
  --dry-run         print the plan and write nothing
  --yes             accept defaults; never prompt
  --uninstall       remove the app, keeping configuration and cache
  --purge           with --uninstall: also remove the build cache
  --status          report what is installed
  --self-test       check the installer's own helpers
  --verbose         stream the complete compiler output
USAGE
}

export SELF_TEST MIN_GTK MIN_LIBADWAITA MIN_RUST
export -f version_ge numbers_of gtk_sufficient default_jobs glibc_compatible

ORIGINAL_ARGS=("$@")

while [ $# -gt 0 ]; do
    case $1 in
        --version) VERSION=${2:-}; shift 2 ;;
        --from) FROM_DIR=${2:-}; shift 2 ;;
        --jobs|-j) JOBS=${2:-}; shift 2 ;;
        --update) ACTION=install; shift ;;
        --container) WANT_CONTAINER=1; shift ;;
        --deps-only) DEPS_ONLY=1; shift ;;
        --no-sudo) NO_SUDO=1; shift ;;
        --background) BACKGROUND=1; shift ;;
        --dry-run) DRY_RUN=1; shift ;;
        --yes|-y) ASSUME_YES=1; shift ;;
        --uninstall|--remove) ACTION=uninstall; shift ;;
        --purge) PURGE=1; shift ;;
        --status) ACTION=status; shift ;;
        --self-test) SELF_TEST=1; shift ;;
        --verbose) VERBOSE=1; shift ;;
        --_child) BACKGROUND=0; ASSUME_YES=1; VERBOSE=0; shift ;;
        -h|--help) usage; exit 0 ;;
        *) die "unknown option: $1 (see --help)" ;;
    esac
done

if [ -n "$VERSION" ]; then VERSION=${VERSION#v}; fi
PACKAGE_MANAGER=$(detect_package_manager)
read_distro

if [ "$SELF_TEST" = 1 ]; then run_self_test; exit 0; fi

case $ACTION in
    install) run_install ;;
    status) run_status ;;
    uninstall) run_uninstall ;;
esac

exit 0
