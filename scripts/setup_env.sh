#!/usr/bin/env bash
# Audiover Development & Environment Setup Script
# Sets up Rust toolchain, Node.js dependencies, and system libraries for Audiover
set -e

PROJECT_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." >/dev/null 2>&1 && pwd)"
cd "$PROJECT_ROOT"

echo "========================================================"
echo "         Audiover Environment Setup (Rust & Tauri)"
echo "========================================================"

# 1. Detect Package Manager and Distro
detect_and_install_deps() {
    if command -v dnf &>/dev/null; then
        echo "[+] Fedora/RHEL detected (dnf)"
        echo "[*] Recommended system dependencies: webkit2gtk4.1-devel openssl-devel alsa-lib-devel pulseaudio-utils ffmpeg ImageMagick rpm-build"
        read -p "[?] Would you like to attempt installing system dependencies with sudo dnf? (y/N) " -n 1 -r
        echo
        if [[ $REPLY =~ ^[Yy]$ ]]; then
            sudo dnf install -y webkit2gtk4.1-devel openssl-devel alsa-lib-devel pulseaudio-utils ffmpeg ImageMagick rpm-build
        fi
    elif command -v apt-get &>/dev/null; then
        echo "[+] Debian/Ubuntu detected (apt)"
        echo "[*] Recommended system dependencies: libwebkit2gtk-4.1-dev libssl-dev libasound2-dev pulseaudio-utils ffmpeg imagemagick"
        read -p "[?] Would you like to attempt installing system dependencies with sudo apt? (y/N) " -n 1 -r
        echo
        if [[ $REPLY =~ ^[Yy]$ ]]; then
            sudo apt-get update && sudo apt-get install -y libwebkit2gtk-4.1-dev libssl-dev libasound2-dev pulseaudio-utils ffmpeg imagemagick
        fi
    elif command -v pacman &>/dev/null; then
        echo "[+] Arch Linux detected (pacman)"
        echo "[*] Recommended system dependencies: webkit2gtk-4.1 openssl alsa-lib libpulse ffmpeg imagemagick"
        read -p "[?] Would you like to attempt installing system dependencies with sudo pacman? (y/N) " -n 1 -r
        echo
        if [[ $REPLY =~ ^[Yy]$ ]]; then
            sudo pacman -S --needed webkit2gtk-4.1 openssl alsa-lib libpulse ffmpeg imagemagick
        fi
    fi
}

if [[ "${1:-}" == "--install-deps" ]]; then
    detect_and_install_deps
fi

# 2. Check Rust & Cargo
echo "[+] Checking Rust toolchain..."
if ! command -v cargo &>/dev/null || ! command -v rustc &>/dev/null; then
    echo "[-] Rust is not installed or not in PATH."
    echo "    Please install Rust via rustup: https://rustup.rs"
    echo "    curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh"
    exit 1
fi
echo "    Rust: $(rustc --version)"
echo "    Cargo: $(cargo --version)"

# 3. Check Node.js & npm
echo "[+] Checking Node.js & npm..."
if ! command -v node &>/dev/null || ! command -v npm &>/dev/null; then
    echo "[-] Node.js / npm is not installed."
    echo "    Please install Node.js 18+ (e.g. from your package manager or nvm)."
    exit 1
fi
echo "    Node: $(node --version)"
echo "    npm: v$(npm --version)"

# 4. Install Frontend UI Dependencies
echo "[+] Installing UI dependencies in ui/..."
cd "$PROJECT_ROOT/ui"
npm install
cd "$PROJECT_ROOT"

# 5. Check Global Hotkey Permissions (/dev/input)
echo "[+] Checking input group membership for global hotkeys..."
if ! id -nG "$USER" 2>/dev/null | grep -qw "input"; then
    echo "[!] User '$USER' is not in the 'input' group (required for global hotkeys)."
    if [[ "${1:-}" == "--install-deps" ]]; then
        echo "[+] Adding '$USER' to 'input' group..."
        sudo usermod -aG input "$USER"
        echo "[*] Added to 'input' group. (Note: Log out and log back in for changes to take effect)."
    else
        read -p "[?] Would you like to add '$USER' to the 'input' group with sudo? (y/N) " -n 1 -r
        echo
        if [[ $REPLY =~ ^[Yy]$ ]]; then
            sudo usermod -aG input "$USER"
            echo "[*] Added to 'input' group. (Note: Log out and log back in for changes to take effect)."
        else
            echo "[!] Skipped. You can add manually later: sudo usermod -aG input $USER"
        fi
    fi
else
    echo "[✓] User '$USER' is already in 'input' group."
fi

# 6. Verify Rust Build
echo "[+] Checking Rust backend build..."
cd "$PROJECT_ROOT/src"
cargo check
cd "$PROJECT_ROOT"

echo "========================================================"
echo "  Setup Complete! Ready to launch Audiover."
echo "  Run 'make dev' to start development server."
echo "========================================================"
