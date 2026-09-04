#!/usr/bin/env bash
# Audiover Package Builder (Tauri CLI)
# Builds Linux desktop packages (.rpm, .deb, .AppImage) using Tauri bundler.
set -euo pipefail

PROJECT_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." >/dev/null 2>&1 && pwd)"
DIST_DIR="$PROJECT_ROOT/dist"
BUNDLE_DIR="$PROJECT_ROOT/src/target/release/bundle"

# Bundles to build:
# Defaults to all targets configured in tauri.conf.json ("deb", "appimage", "rpm")
# or accepts target name(s) passed as argument (e.g. ./scripts/build_packages.sh rpm)
TARGET_BUNDLES="${1:-}"

# Ensure environment compatibility (prevents linuxdeploy DT_RELR strip error on modern Linux distributions like Fedora)
export NO_STRIP=true

echo "========================================================"
echo "          Audiover Package Builder (Tauri CLI)"
echo "========================================================"

# 1. System dependency checks & auto-installation
ensure_system_deps() {
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
            echo "[+] Installing missing packages with sudo dnf..."
            sudo dnf install -y "${missing[@]}"
        else
            echo "[✓] System build dependencies are already satisfied."
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
            echo "[+] Installing missing packages with sudo apt..."
            sudo apt-get update -qq
            sudo apt-get install -y "${missing[@]}"
        else
            echo "[✓] System build dependencies are already satisfied."
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
            echo "[+] Installing missing packages with sudo pacman..."
            sudo pacman -S --needed --noconfirm "${missing[@]}"
        else
            echo "[✓] System build dependencies are already satisfied."
        fi
    fi
}

ensure_system_deps

# 2. Toolchain checks
for cmd in node npm cargo; do
    if ! command -v "$cmd" &>/dev/null; then
        echo "[-] Error: Required tool '$cmd' is not installed." >&2
        exit 1
    fi
done

# 2. Ensure frontend dependencies are installed
if [ ! -d "$PROJECT_ROOT/ui/node_modules" ]; then
    echo "[+] Installing frontend dependencies..."
    (cd "$PROJECT_ROOT/ui" && npm install)
fi

# 3. Ensure Tauri icons exist
if [ ! -f "$PROJECT_ROOT/src/icons/32x32.png" ]; then
    echo "[+] Generating application icons from assets..."
    mkdir -p "$PROJECT_ROOT/src/icons"
    if [ -f "$PROJECT_ROOT/assets/icons/audiover.png" ]; then
        npx --prefix "$PROJECT_ROOT/ui" tauri icon "$PROJECT_ROOT/assets/icons/audiover.png" -o "$PROJECT_ROOT/src/icons"
    elif [ -f "$PROJECT_ROOT/assets/icons/audiover.svg" ]; then
        npx --prefix "$PROJECT_ROOT/ui" tauri icon "$PROJECT_ROOT/assets/icons/audiover.svg" -o "$PROJECT_ROOT/src/icons"
    fi
    rm -rf "$PROJECT_ROOT/src/icons/android" "$PROJECT_ROOT/src/icons/ios"
fi

# 4. Clean stale bundle outputs and execute Tauri build
mkdir -p "$DIST_DIR"
if [ -n "$TARGET_BUNDLES" ]; then
    IFS=',' read -ra BUNDLE_ARR <<< "$TARGET_BUNDLES"
    for b in "${BUNDLE_ARR[@]}"; do
        b_clean="$(echo "$b" | xargs)"
        if [ -n "$b_clean" ]; then
            rm -rf "$BUNDLE_DIR/$b_clean"
            case "$b_clean" in
                rpm) rm -f "$DIST_DIR"/*.rpm ;;
                deb) rm -f "$DIST_DIR"/*.deb ;;
                appimage) rm -f "$DIST_DIR"/*.AppImage ;;
            esac
        fi
    done
else
    rm -rf "$BUNDLE_DIR"
    rm -f "$DIST_DIR"/*.rpm "$DIST_DIR"/*.deb "$DIST_DIR"/*.AppImage 2>/dev/null || true
fi

echo "[+] Running Tauri build..."
BUILD_ARGS=()
if [ -n "$TARGET_BUNDLES" ]; then
    echo "    Target bundles: $TARGET_BUNDLES"
    BUILD_ARGS+=(--bundles "$TARGET_BUNDLES")
else
    echo "    Target bundles: (default from tauri.conf.json)"
fi

npx --prefix "$PROJECT_ROOT/ui" tauri build "${BUILD_ARGS[@]}"

# 5. Collect artifacts into dist/
echo ""
echo "[+] Collecting built packages into $DIST_DIR..."

FOUND_PACKAGES=0
if [ -d "$BUNDLE_DIR" ]; then
    while IFS= read -r -d '' file; do
        cp -p "$file" "$DIST_DIR/"
        FOUND_PACKAGES=$((FOUND_PACKAGES + 1))
    done < <(find "$BUNDLE_DIR" -type f \( -name "*.rpm" -o -name "*.deb" -o -name "*.AppImage" \) -print0)
fi

echo "========================================================"
if [ "$FOUND_PACKAGES" -gt 0 ]; then
    echo "  Package Build Complete! Available in: $DIST_DIR"
    echo "--------------------------------------------------------"
    ls -lh "$DIST_DIR"/*.{rpm,deb,AppImage} 2>/dev/null || true
else
    echo "[-] Warning: No package files were found in $BUNDLE_DIR"
fi
echo "========================================================"
