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

# 1. Dependency checks
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

# 4. Execute Tauri build
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
mkdir -p "$DIST_DIR"
echo ""
echo "[+] Collecting built packages into $DIST_DIR..."

FOUND_PACKAGES=0
if [ -d "$BUNDLE_DIR" ]; then
    while IFS= read -r -d '' file; do
        cp "$file" "$DIST_DIR/"
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
