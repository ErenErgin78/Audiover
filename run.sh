#!/usr/bin/env bash
# Audiover Launcher Script
set -e
DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" >/dev/null 2>&1 && pwd)"
cd "$DIR"

if [ ! -d ".venv" ]; then
    echo "[!] Virtual environment not found. Running setup_env.sh first..."
    bash setup_env.sh
fi

source .venv/bin/activate
export PYTHONPATH="$DIR:${PYTHONPATH:-}"
exec python3 src/main.py "$@"
