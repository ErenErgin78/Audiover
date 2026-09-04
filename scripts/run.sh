#!/usr/bin/env bash
# Audiover Launcher Script (Rust / Tauri)
set -e

DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." >/dev/null 2>&1 && pwd)"
cd "$DIR"

# 1. Ensure UI node_modules are installed
if [ ! -d "ui/node_modules" ]; then
    echo "[!] UI dependencies not found. Running setup_env.sh first..."
    bash "$DIR/scripts/setup_env.sh"
fi

# 2. Check for Rust / Cargo
if ! command -v cargo &>/dev/null; then
    echo "[-] Error: 'cargo' not found in PATH. Please install Rust or run ./scripts/setup_env.sh"
    exit 1
fi

# 3. Preflight: fail fast if the dev port is already taken (e.g. a stale
# `vite` from a previous session). The dev server uses strictPort, so it
# will not silently move to 5174 while Tauri polls 5173.
if (echo > /dev/tcp/127.0.0.1/5173) 2>/dev/null; then
    echo "[-] Error: port 5173 is already in use (stale dev server?)."
    ss -tlnp 2>/dev/null | grep ':5173' || true
    echo "    Stop the old process (e.g. 'pkill -f \"vite --\"') and retry."
    exit 1
fi

# 4. Launch Tauri Dev Server
echo "[+] Launching Audiover (Tauri + React)..."
exec npx --prefix ui tauri dev "$@"
