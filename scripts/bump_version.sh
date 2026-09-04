#!/usr/bin/env bash
# Audiover Version Bumper
# Üç dosyadaki sürümü tek komutla senkron yükseltir:
#   - src/tauri.conf.json  ("version")
#   - src/Cargo.toml       ([package] version)
#   - ui/package.json      ("version")
# Ayrıca kilit dosyaları varsa senkron tutar:
#   - src/Cargo.lock            (audiover paketi)
#   - ui/package-lock.json      (version + packages[""].version)
#
# Kullanım:
#   ./scripts/bump_version.sh patch          # 1.0.1 -> 1.0.2
#   ./scripts/bump_version.sh minor          # 1.0.1 -> 1.1.0
#   ./scripts/bump_version.sh major          # 1.0.1 -> 2.0.0
#   ./scripts/bump_version.sh 1.2.3          # doğrudan sürüme ayarla
#   ./scripts/bump_version.sh --set 1.2.3    # doğrudan sürüme ayarla
#
# Makefile kısayolları:
#   make bump-patch / make bump-minor / make bump-major
#   make bump TYPE=minor
#   make set-version VERSION=1.2.3
set -euo pipefail

PROJECT_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." >/dev/null 2>&1 && pwd)"
TAURI_CONF="$PROJECT_ROOT/src/tauri.conf.json"
CARGO_TOML="$PROJECT_ROOT/src/Cargo.toml"
PACKAGE_JSON="$PROJECT_ROOT/ui/package.json"
CARGO_LOCK="$PROJECT_ROOT/src/Cargo.lock"
PACKAGE_LOCK="$PROJECT_ROOT/ui/package-lock.json"

usage() {
    echo "Kullanim: $0 {patch|minor|major|X.Y.Z|--set X.Y.Z}" >&2
    echo "Ornek:  $0 patch | $0 minor | $0 major | $0 1.2.3 | $0 --set 1.2.3" >&2
}

# --- Argüman ayrıştırma ---
BUMP=""
if [ $# -eq 0 ]; then
    usage; exit 1
elif [ "$1" = "--set" ]; then
    [ $# -eq 2 ] || { usage; exit 1; }
    BUMP="$2"
elif [ $# -eq 1 ]; then
    BUMP="$1"
else
    usage; exit 1
fi

# --- Mevcut sürümü oku (tek doğruluk kaynağı: tauri.conf.json) ---
if [ ! -f "$TAURI_CONF" ] || [ ! -f "$CARGO_TOML" ] || [ ! -f "$PACKAGE_JSON" ]; then
    echo "[-] Hata: sürüm dosyalarından biri bulunamadı." >&2
    exit 1
fi

CURRENT="$(python3 -c "import json;print(json.load(open('$TAURI_CONF'))['version'])")"
CARGO_VER="$(python3 -c "import re;print(re.search(r'^version\s*=\s*\"([^\"]+)\"', open('$CARGO_TOML').read(), re.M).group(1))")"
UI_VER="$(python3 -c "import json;print(json.load(open('$PACKAGE_JSON'))['version'])")"

if [ "$CURRENT" != "$CARGO_VER" ] || [ "$CURRENT" != "$UI_VER" ]; then
    echo "[!] Uyari: sürümler senkron değil:" >&2
    echo "    tauri.conf.json: $CURRENT" >&2
    echo "    Cargo.toml:      $CARGO_VER" >&2
    echo "    package.json:    $UI_VER" >&2
    echo "[*] Baz olarak tauri.conf.json ($CURRENT) kullanılacak." >&2
fi

SEMVER_RE='^[0-9]+\.[0-9]+\.[0-9]+$'
NEW=""

case "$BUMP" in
    patch|minor|major)
        IFS='.' read -r MA MI PA <<< "$CURRENT"
        if ! [[ "$CURRENT" =~ $SEMVER_RE ]]; then
            echo "[-] Hata: mevcut sürüm semver değil: $CURRENT" >&2
            exit 1
        fi
        case "$BUMP" in
            patch) PA=$((PA + 1)) ;;
            minor) MI=$((MI + 1)); PA=0 ;;
            major) MA=$((MA + 1)); MI=0; PA=0 ;;
        esac
        NEW="$MA.$MI.$PA"
        ;;
    *)
        if [[ "$BUMP" =~ $SEMVER_RE ]]; then
            NEW="$BUMP"
        else
            echo "[-] Hata: geçersiz argüman '$BUMP'." >&2
            usage; exit 1
        fi
        ;;
esac

if [ "$NEW" = "$CURRENT" ]; then
    echo "[=] Sürüm zaten $NEW, yapılacak işlem yok."
    exit 0
fi

echo "[+] Sürüm yükseltiliyor: $CURRENT -> $NEW"

# --- Dosyaları güncelle (python ile güvenli JSON/TOML dokunuşu) ---
python3 - "$TAURI_CONF" "$CARGO_TOML" "$PACKAGE_JSON" "$CARGO_LOCK" "$PACKAGE_LOCK" "$NEW" <<'EOF'
import json, re, sys

tauri_conf, cargo_toml, package_json, cargo_lock, package_lock, new = sys.argv[1:7]

# 1. tauri.conf.json
with open(tauri_conf) as f:
    data = json.load(f)
data["version"] = new
with open(tauri_conf, "w") as f:
    json.dump(data, f, indent=2, ensure_ascii=False)
    f.write("\n")

# 2. Cargo.toml -> yalnızca [package] bloğundaki ilk version satırı
with open(cargo_toml) as f:
    content = f.read()
in_package, done, out = False, False, []
for line in content.splitlines(keepends=True):
    s = line.strip()
    if s.startswith("["):
        in_package = (s == "[package]")
    if in_package and not done and re.match(r'^version\s*=', s):
        line = re.sub(r'^(\s*version\s*=\s*")[^"]+(".*)$', rf'\g<1>{new}\g<2>', line)
        done = True
    out.append(line)
assert done, "Cargo.toml içinde [package] version bulunamadı"
with open(cargo_toml, "w") as f:
    f.writelines(out)

# 3. ui/package.json
with open(package_json) as f:
    pkg = json.load(f)
pkg["version"] = new
with open(package_json, "w") as f:
    json.dump(pkg, f, indent=2, ensure_ascii=False)
    f.write("\n")

# 4. src/Cargo.lock (varsa) -> yalnızca name = "audiover" paket bloğu
try:
    with open(cargo_lock) as f:
        lines = f.readlines()
    out, i, changed = [], 0, False
    while i < len(lines):
        out.append(lines[i])
        if lines[i].strip() == 'name = "audiover"':
            j = i + 1
            while j < len(lines) and lines[j].strip() != "":
                if re.match(r'^version\s*=', lines[j].strip()):
                    out.append(re.sub(r'"[^"]+"', f'"{new}"', lines[j], count=1))
                    changed = True
                else:
                    out.append(lines[j])
                j += 1
            i = j - 1
        i += 1
    if changed:
        with open(cargo_lock, "w") as f:
            f.writelines(out)
        print("    [~] src/Cargo.lock senkronize edildi")
except FileNotFoundError:
    pass

# 5. ui/package-lock.json (varsa)
try:
    with open(package_lock) as f:
        lock = json.load(f)
    lock["version"] = new
    if isinstance(lock.get("packages"), dict) and "" in lock["packages"]:
        lock["packages"][""]["version"] = new
    with open(package_lock, "w") as f:
        json.dump(lock, f, indent=2, ensure_ascii=False)
        f.write("\n")
    print("    [~] ui/package-lock.json senkronize edildi")
except FileNotFoundError:
    pass
EOF

echo "    [~] src/tauri.conf.json -> $NEW"
echo "    [~] src/Cargo.toml       -> $NEW"
echo "    [~] ui/package.json      -> $NEW"
echo "[✓] Tüm sürümler $NEW olarak senkronize edildi."
