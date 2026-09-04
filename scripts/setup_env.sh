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
    local missing=()
    if command -v dnf &>/dev/null; then
        local deps=(webkit2gtk4.1-devel openssl-devel alsa-lib-devel pulseaudio-utils ImageMagick rpm-build)
        for pkg in "${deps[@]}"; do
            if ! rpm -q "$pkg" &>/dev/null; then
                missing+=("$pkg")
            fi
        done
        if [ ${#missing[@]} -gt 0 ]; then
            echo "[!] Missing system dependencies detected: ${missing[*]}"
            read -p "[?] Would you like to install missing dependencies with sudo dnf? (y/N) " -n 1 -r
            echo
            if [[ $REPLY =~ ^[Yy]$ ]]; then
                sudo dnf install -y "${missing[@]}"
            fi
        else
            echo "[✓] All system dependencies are already installed (dnf)."
        fi
    elif command -v apt-get &>/dev/null; then
        local deps=(libwebkit2gtk-4.1-dev libssl-dev libasound2-dev pulseaudio-utils imagemagick)
        for pkg in "${deps[@]}"; do
            if ! dpkg -s "$pkg" 2>/dev/null | grep -q "Status: install ok installed"; then
                missing+=("$pkg")
            fi
        done
        if [ ${#missing[@]} -gt 0 ]; then
            echo "[!] Missing system dependencies detected: ${missing[*]}"
            read -p "[?] Would you like to install missing dependencies with sudo apt? (y/N) " -n 1 -r
            echo
            if [[ $REPLY =~ ^[Yy]$ ]]; then
                sudo apt-get update && sudo apt-get install -y "${missing[@]}"
            fi
        else
            echo "[✓] All system dependencies are already installed (apt)."
        fi
    elif command -v pacman &>/dev/null; then
        local deps=(webkit2gtk-4.1 openssl alsa-lib libpulse imagemagick)
        for pkg in "${deps[@]}"; do
            if ! pacman -Q "$pkg" &>/dev/null; then
                missing+=("$pkg")
            fi
        done
        if [ ${#missing[@]} -gt 0 ]; then
            echo "[!] Missing system dependencies detected: ${missing[*]}"
            read -p "[?] Would you like to install missing dependencies with sudo pacman? (y/N) " -n 1 -r
            echo
            if [[ $REPLY =~ ^[Yy]$ ]]; then
                sudo pacman -S --needed "${missing[@]}"
            fi
        else
            echo "[✓] All system dependencies are already installed (pacman)."
        fi
    fi
}

detect_and_install_deps

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
    echo "    Please install Node.js 22+ (LTS) (e.g. from your package manager or nvm)."
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
