#!/usr/bin/env bash
# Audiover RPM Package Builder for Fedora / RHEL
# Builds pure native Rust & React Tauri RPM package
set -e

PROJECT_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")" >/dev/null 2>&1 && pwd)"
BUILD_DIR="$PROJECT_ROOT/build/rpmbuild"
DIST_DIR="$PROJECT_ROOT/dist/rpm"

echo "========================================================"
echo "         Building Audiover RPM (Rust Native)"
echo "========================================================"

# 1. Check requirements
for cmd in rpmbuild cargo node npm; do
    if ! command -v "$cmd" &>/dev/null; then
        echo "[-] Error: Required tool '$cmd' is not installed."
        exit 1
    fi
done

# 2. Build Frontend UI
echo "[+] Step 1: Building React frontend..."
cd "$PROJECT_ROOT/ui"
if [ ! -d "node_modules" ]; then
    npm install
fi
npm run build
cd "$PROJECT_ROOT"

# 3. Build Rust Native Application
echo "[+] Step 2: Compiling Rust release binary with embedded UI (custom-protocol)..."
cd "$PROJECT_ROOT/src"
cargo build --release --features custom-protocol
cd "$PROJECT_ROOT"

# 4. Generate Icons if needed
echo "[+] Step 3: Ensuring application icons are generated..."
mkdir -p "$PROJECT_ROOT/assets/icons"
if [ ! -f "$PROJECT_ROOT/assets/icons/audiover.png" ] && command -v magick &>/dev/null; then
    magick "$PROJECT_ROOT/assets/icons/audiover.svg" -background none -resize 256x256 "$PROJECT_ROOT/assets/icons/audiover.png"
fi

# 5. Prepare RPM build environment
echo "[+] Step 4: Staging RPM filesystem tree..."
rm -rf "$BUILD_DIR"
mkdir -p "$BUILD_DIR"/{BUILD,RPMS,SOURCES,SPECS,SRPMS}

STAGING_OPT="$BUILD_DIR/SOURCES/opt/audiover"
STAGING_BIN="$BUILD_DIR/SOURCES/usr/bin"
STAGING_DESKTOP="$BUILD_DIR/SOURCES/usr/share/applications"
STAGING_ICONS_SVG="$BUILD_DIR/SOURCES/usr/share/icons/hicolor/scalable/apps"
STAGING_ICONS_PNG="$BUILD_DIR/SOURCES/usr/share/icons/hicolor/256x256/apps"
STAGING_PIXMAPS="$BUILD_DIR/SOURCES/usr/share/pixmaps"

mkdir -p "$STAGING_OPT" "$STAGING_BIN" "$STAGING_DESKTOP" "$STAGING_ICONS_SVG" "$STAGING_ICONS_PNG" "$STAGING_PIXMAPS"

# Copy compiled Rust binary and assets to /opt/audiover
cp "$PROJECT_ROOT/src/target/release/audiover" "$STAGING_OPT/audiover"
chmod +x "$STAGING_OPT/audiover"

if [ -d "$PROJECT_ROOT/assets" ]; then
    cp -r "$PROJECT_ROOT/assets" "$STAGING_OPT/"
fi

if [ -d "$PROJECT_ROOT/config" ]; then
    cp -r "$PROJECT_ROOT/config" "$STAGING_OPT/"
fi

# Copy desktop launcher and integration assets
cp "$PROJECT_ROOT/packaging/bin/audiover" "$STAGING_BIN/audiover"
chmod +x "$STAGING_BIN/audiover"

cp "$PROJECT_ROOT/packaging/desktop/audiover.desktop" "$STAGING_DESKTOP/audiover.desktop"
if [ -f "$PROJECT_ROOT/assets/icons/audiover.svg" ]; then
    cp "$PROJECT_ROOT/assets/icons/audiover.svg" "$STAGING_ICONS_SVG/audiover.svg"
fi
if [ -f "$PROJECT_ROOT/assets/icons/audiover.png" ]; then
    cp "$PROJECT_ROOT/assets/icons/audiover.png" "$STAGING_ICONS_PNG/audiover.png"
    cp "$PROJECT_ROOT/assets/icons/audiover.png" "$STAGING_PIXMAPS/audiover.png"
fi

# Copy spec file
cp "$PROJECT_ROOT/packaging/rpm/audiover.spec" "$BUILD_DIR/SPECS/audiover.spec"

# 6. Execute rpmbuild
echo "[+] Step 5: Building RPM package with rpmbuild..."
rpmbuild -bb \
    --define "_topdir $BUILD_DIR" \
    "$BUILD_DIR/SPECS/audiover.spec"

# 7. Collect output package
mkdir -p "$DIST_DIR"
cp "$BUILD_DIR"/RPMS/*/*.rpm "$DIST_DIR/"
RPM_FILE=$(ls -t "$DIST_DIR"/*.rpm | head -n 1)

echo "========================================================"
echo "  RPM Package Built Successfully!"
echo "  Output: $RPM_FILE"
echo "========================================================"

# 8. Reinstall on local system (remove previous, install new)
echo "[+] Step 6: Removing old application version and installing newly built package..."
if rpm -q audiover &>/dev/null || rpm -q audiover_rust &>/dev/null; then
    echo "[*] Removing previous version..."
    sudo dnf remove -y audiover audiover_rust 2>/dev/null || true
fi

echo "[*] Installing fresh RPM package: $RPM_FILE..."
sudo dnf install -y "$RPM_FILE"

echo "========================================================"
echo "  Audiover has been successfully reinstalled!"
echo "========================================================"
