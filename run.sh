#!/usr/bin/env bash
# Audiover Launcher Script (Rust / Tauri)
set -e

DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" >/dev/null 2>&1 && pwd)"
cd "$DIR"

# 1. Ensure UI node_modules are installed
if [ ! -d "ui/node_modules" ]; then
    echo "[!] UI dependencies not found. Running setup_env.sh first..."
    bash setup_env.sh
fi

# 2. Check for Rust / Cargo
if ! command -v cargo &>/dev/null; then
    echo "[-] Error: 'cargo' not found in PATH. Please install Rust or run ./setup_env.sh"
    exit 1
fi

# 3. Launch Tauri Dev Server
echo "[+] Launching Audiover (Tauri + React)..."
exec npx --prefix ui tauri dev "$@"
