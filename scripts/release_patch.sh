#!/usr/bin/env bash
# Audiover Release & Tag Script
#
# 1. bump_version.sh kullanarak sürüm numarasını yükseltir (varsayılan: patch)
# 2. Değişen sürüm dosyalarını git commit yapar
# 3. 'v[yeni versiyon]' git etiketini oluşturur
# 4. 'git push origin [dal]' ve 'git push origin v[yeni versiyon]' komutlarını çalıştırır
#
# Kullanım:
#   ./scripts/release_patch.sh              # patch sürüm yükselt, commit'le, etiketle ve push et
#   ./scripts/release_patch.sh patch        # aynı şekilde patch
#   ./scripts/release_patch.sh minor        # minor sürüm için
#   ./scripts/release_patch.sh major        # major sürüm için
#   ./scripts/release_patch.sh --dry-run    # sadece yapılacak işlemleri simüle et
#
# Makefile kısayolları:
#   make release-patch
#   make release TYPE=patch
set -euo pipefail

PROJECT_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." >/dev/null 2>&1 && pwd)"
TAURI_CONF="$PROJECT_ROOT/src/tauri.conf.json"
CARGO_TOML="$PROJECT_ROOT/src/Cargo.toml"
PACKAGE_JSON="$PROJECT_ROOT/ui/package.json"
CARGO_LOCK="$PROJECT_ROOT/src/Cargo.lock"
PACKAGE_LOCK="$PROJECT_ROOT/ui/package-lock.json"
BUMP_SCRIPT="$PROJECT_ROOT/scripts/bump_version.sh"

usage() {
    echo "Kullanım: $0 [patch|minor|major] [--dry-run]" >&2
    echo "Örnekler:" >&2
    echo "  $0                  # Varsayılan: patch sürüm yükselt, etiketle ve push et" >&2
    echo "  $0 patch            # Patch sürüm (1.0.4 -> 1.0.5)" >&2
    echo "  $0 minor            # Minor sürüm (1.0.4 -> 1.1.0)" >&2
    echo "  $0 --dry-run        # Değişiklik yapmadan simüle et" >&2
}

BUMP_TYPE="patch"
DRY_RUN=0

for arg in "$@"; do
    case "$arg" in
        patch|minor|major)
            BUMP_TYPE="$arg"
            ;;
        --dry-run|-n)
            DRY_RUN=1
            ;;
        -h|--help)
            usage
            exit 0
            ;;
        *)
            echo "[-] Hata: Bilinmeyen argüman '$arg'" >&2
            usage
            exit 1
            ;;
    esac
done

if [ ! -f "$TAURI_CONF" ]; then
    echo "[-] Hata: $TAURI_CONF bulunamadı." >&2
    exit 1
fi

CURRENT_VERSION="$(python3 -c "import json; print(json.load(open('$TAURI_CONF'))['version'])")"

if [ "$DRY_RUN" -eq 1 ]; then
    echo "[*] DRY-RUN MODU AKTİF - Gerçekte hiçbir dosya değiştirilmeyecek ve push yapılmayacaktır."
    echo "    Mevcut sürüm : $CURRENT_VERSION"
    echo "    Artış türü   : $BUMP_TYPE"
    IFS='.' read -r MA MI PA <<< "$CURRENT_VERSION"
    case "$BUMP_TYPE" in
        patch) PA=$((PA + 1)) ;;
        minor) MI=$((MI + 1)); PA=0 ;;
        major) MA=$((MA + 1)); MI=0; PA=0 ;;
    esac
    SIMULATED_VERSION="$MA.$MI.$PA"
    SIMULATED_TAG="v$SIMULATED_VERSION"
    echo "    Yeni sürüm   : $SIMULATED_VERSION"
    echo "    Hedef etiket : $SIMULATED_TAG"
    echo ""
    echo "[*] Çalıştırılacak adımlar:"
    echo "    1. bash $BUMP_SCRIPT $BUMP_TYPE"
    echo "    2. git add sürüm dosyaları"
    echo "    3. git commit -m 'chore(release): $SIMULATED_TAG'"
    echo "    4. git push origin \$(mevcut dal)"
    echo "    5. git tag $SIMULATED_TAG"
    echo "    6. git push origin $SIMULATED_TAG"
    exit 0
fi

# --- Git Kontrolleri ---
if ! git -C "$PROJECT_ROOT" rev-parse --is-inside-work-tree >/dev/null 2>&1; then
    echo "[-] Hata: $PROJECT_ROOT bir Git deposu değil." >&2
    exit 1
fi

if ! git -C "$PROJECT_ROOT" remote get-url origin >/dev/null 2>&1; then
    echo "[-] Hata: 'origin' adında bir git uzak sunucusu (remote) bulunamadı." >&2
    exit 1
fi

# İzlenen dosyalarda sürüm dosyaları haricinde kaydedilmemiş değişiklik var mı kontrol et
MODIFIED_NON_VERSION="$(git -C "$PROJECT_ROOT" diff --name-only HEAD 2>/dev/null | grep -vE 'src/tauri\.conf\.json|src/Cargo\.toml|ui/package\.json|src/Cargo\.lock|ui/package-lock\.json' || true)"
if [ -n "$MODIFIED_NON_VERSION" ] && [ "${ALLOW_DIRTY:-0}" != "1" ]; then
    echo "[-] Hata: Çalışma dizininde kaydedilmemiş kod değişiklikleri var:" >&2
    echo "$MODIFIED_NON_VERSION" >&2
    echo "    Lütfen önce bu değişiklikleri commit'leyin veya stash yapın (atlamak için: ALLOW_DIRTY=1 $0)." >&2
    exit 1
fi

# --- 1. Sürüm Yükseltme ---
echo "[+] 1/4: Sürüm numarası yükseltiliyor ($BUMP_TYPE)..."
bash "$BUMP_SCRIPT" "$BUMP_TYPE"

NEW_VERSION="$(python3 -c "import json; print(json.load(open('$TAURI_CONF'))['version'])")"
TAG="v$NEW_VERSION"

# Etiketin önceden var olup olmadığını kontrol et
if git -C "$PROJECT_ROOT" rev-parse "$TAG" >/dev/null 2>&1; then
    echo "[-] Hata: '$TAG' etiketi yerel depoda zaten mevcut!" >&2
    exit 1
fi

# --- 2. Sürüm Dosyalarını Commit Yap ---
echo "[+] 2/4: Sürüm dosyaları Git'e kaydediliyor..."
git -C "$PROJECT_ROOT" add "$TAURI_CONF" "$CARGO_TOML" "$PACKAGE_JSON"
[ -f "$CARGO_LOCK" ] && git -C "$PROJECT_ROOT" add "$CARGO_LOCK"
[ -f "$PACKAGE_LOCK" ] && git -C "$PROJECT_ROOT" add "$PACKAGE_LOCK"

if ! git -C "$PROJECT_ROOT" diff --cached --quiet; then
    git -C "$PROJECT_ROOT" commit -m "chore(release): $TAG"
    echo "    [✓] Commit oluşturuldu: chore(release): $TAG"
else
    echo "    [*] Değişiklik yok, commit adımı atlandı."
fi

# Dalı uzak sunucuya güncelle
CURRENT_BRANCH="$(git -C "$PROJECT_ROOT" rev-parse --abbrev-ref HEAD 2>/dev/null || echo "")"
if [ -n "$CURRENT_BRANCH" ] && [ "$CURRENT_BRANCH" != "HEAD" ]; then
    echo "    [+] Dal push ediliyor: origin $CURRENT_BRANCH..."
    git -C "$PROJECT_ROOT" push origin "$CURRENT_BRANCH"
fi

# --- 3. Git Tag Oluştur ---
echo "[+] 3/4: Git etiketi oluşturuluyor: $TAG..."
git -C "$PROJECT_ROOT" tag "$TAG"

# --- 4. Git Push Tag ---
echo "[+] 4/4: Etiket uzak sunucuya gönderiliyor: origin $TAG..."
git -C "$PROJECT_ROOT" push origin "$TAG"

echo ""
echo "========================================================"
echo "  [✓] Başarılı! $TAG yayınlandı ve origin'e push edildi."
echo "========================================================"
