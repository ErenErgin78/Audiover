#!/usr/bin/env bash
# Audiover - Fedora Voice & Soundboard Engine Setup Script
set -e

echo "========================================================"
echo "  Fedora Voice & Soundboard Engine (Audiover) Setup"
echo "========================================================"

# Check Python version
PYTHON_BIN=$(which python3 2>/dev/null || true)
if [ -z "$PYTHON_BIN" ]; then
    echo "[-] Python3 is not installed. Please install it using: sudo dnf install python3"
    exit 1
fi
echo "[+] Detected Python: $($PYTHON_BIN --version)"

# Check pipewire / pactl tools
if ! command -v pactl &> /dev/null; then
    echo "[!] pactl not found. Installing pulseaudio-utils..."
    sudo dnf install -y pulseaudio-utils
fi

if ! command -v ffmpeg &> /dev/null; then
    echo "[!] ffmpeg not found. It is recommended for media decoding."
fi

# Create virtual environment if not exists
if [ ! -d ".venv" ]; then
    echo "[+] Creating virtual environment in .venv..."
    python3 -m venv .venv
fi

# Activate virtual environment and install requirements
echo "[+] Installing Python dependencies..."
source .venv/bin/activate
pip install --upgrade pip
pip install -r requirements.txt

# Create necessary runtime directories
mkdir -p config assets/sounds

# Check user group for Wayland global hotkeys (/dev/input access)
if groups | grep -q '\binput\b'; then
    echo "[+] User is already in the 'input' group. Global evdev hotkeys are enabled!"
else
    echo "[!] Notice: For global hotkeys outside the app window on Wayland,"
    echo "    you can add your user to the 'input' group with:"
    echo "    sudo usermod -aG input \$USER"
    echo "    (Requires re-login to take effect)."
fi

echo "========================================================"
echo "  Setup completed successfully!"
echo "  Run the application with: ./run.sh or source .venv/bin/activate && python3 src/main.py"
echo "========================================================"
