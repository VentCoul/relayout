#!/usr/bin/env bash
# Builds Relayout on Linux (Debian/Ubuntu, Fedora or Arch based).
#   ./build-linux.sh        build the .deb, .rpm and AppImage packages
#   ./build-linux.sh dev    run in development mode (errors show in the terminal)
# Set RELAYOUT_YES=1 to install missing tools without asking.
set -euo pipefail
cd "$(dirname "$0")"

echo
echo "=== Relayout: Linux build ==="
echo

SUDO=""
if [ "$(id -u)" -ne 0 ]; then SUDO="sudo"; fi

confirm() {
    if [ "${RELAYOUT_YES:-0}" = "1" ]; then return 0; fi
    read -r -p "        $1 [y/N] " answer
    [[ "$answer" =~ ^[YyТт] ]]
}

install_system_packages() {
    if command -v apt-get >/dev/null; then
        $SUDO apt-get update
        $SUDO env DEBIAN_FRONTEND=noninteractive apt-get install -y \
            build-essential curl wget file pkg-config libssl-dev \
            libwebkit2gtk-4.1-dev libayatana-appindicator3-dev librsvg2-dev libxdo-dev \
            nodejs npm
    elif command -v dnf >/dev/null; then
        $SUDO dnf install -y \
            gcc gcc-c++ make curl wget file pkgconf-pkg-config openssl-devel \
            webkit2gtk4.1-devel libappindicator-gtk3-devel librsvg2-devel libxdo-devel \
            nodejs npm rpm-build
    elif command -v pacman >/dev/null; then
        $SUDO pacman -S --needed --noconfirm \
            base-devel curl wget file openssl \
            webkit2gtk-4.1 libappindicator-gtk3 librsvg xdotool \
            nodejs npm
    else
        echo "Unknown distribution. Install the Tauri prerequisites by hand:"
        echo "  https://v2.tauri.app/start/prerequisites/#linux"
        exit 1
    fi
}

if ! pkg-config --exists webkit2gtk-4.1 2>/dev/null || ! command -v node >/dev/null; then
    echo "[setup] System libraries for building (WebKitGTK, GTK, Node.js) are missing."
    if confirm "Install them with the system package manager?"; then
        install_system_packages
    else
        echo "Cannot build without them."
        exit 1
    fi
fi

if [ -x "$HOME/.cargo/bin/cargo" ]; then export PATH="$HOME/.cargo/bin:$PATH"; fi
if ! command -v cargo >/dev/null; then
    echo "[setup] Rust is not installed."
    if confirm "Install it for this user with the official installer from rustup.rs?"; then
        curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --profile minimal
        export PATH="$HOME/.cargo/bin:$PATH"
    else
        echo "Cannot build without Rust: https://rustup.rs"
        exit 1
    fi
fi

if command -v pnpm >/dev/null; then
    PNPM=(pnpm)
else
    PNPM=(npx --yes pnpm@12)
fi

echo "[1/2] Installing JavaScript dependencies..."
"${PNPM[@]}" install

if [ "${1:-}" = "dev" ]; then
    echo "[2/2] Starting in development mode. Press Ctrl+C to stop."
    exec "${PNPM[@]}" tauri dev
fi

echo "[2/2] Building the packages. The first build takes 5-15 minutes..."
# linuxdeploy (used for the AppImage) is itself an AppImage; this lets it run
# where FUSE is unavailable, such as in containers.
export APPIMAGE_EXTRACT_AND_RUN=1
BUNDLES="deb,appimage"
if command -v rpmbuild >/dev/null; then BUNDLES="deb,rpm,appimage"; fi
"${PNPM[@]}" tauri build --bundles "$BUNDLES"

echo
echo "Done. Packages:"
find target/release/bundle -maxdepth 2 -type f \( -name '*.deb' -o -name '*.rpm' -o -name '*.AppImage' \) -print
