#!/usr/bin/env bash
# ============================================================
#  Automata — Windows ke liye chromedriver.exe download karo
#
#  chromedriver wo pul hai jo backend ko asli Chrome chalane
#  deta hai. Iske bagair chat to chalti hai, lekin browser wale
#  kaam (navigate/click/draw) nahi chalenge.
#
#  Istemaal:
#    ./get-chromedriver.sh                 # latest stable
#    ./get-chromedriver.sh 131.0.6778.204  # apne Chrome jaisa version
#
#  Output: build/windows/artifacts/driver/chromedriver.exe
# ============================================================
set -euo pipefail

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
OUT_DIR="$HERE/artifacts/driver"
mkdir -p "$OUT_DIR"

VERSION="${1:-}"

if [ -z "$VERSION" ]; then
    echo "==> Latest stable chromedriver ka version dhoonda ja raha hai..."
    META="$(curl -fsSL --retry 3 https://googlechromelabs.github.io/chrome-for-testing/last-known-good-versions-with-downloads.json)"

    VERSION="$(printf '%s' "$META" | node -e "
        let s = '';
        process.stdin.on('data', d => s += d).on('end', () => {
            const j = JSON.parse(s);
            process.stdout.write(j.channels.Stable.version);
        });
    ")"

    URL="$(printf '%s' "$META" | node -e "
        let s = '';
        process.stdin.on('data', d => s += d).on('end', () => {
            const j = JSON.parse(s);
            const list = j.channels.Stable.downloads.chromedriver;
            const win = list.find(x => x.platform === 'win64') || list.find(x => x.platform === 'win32');
            if (!win) { console.error('win64 chromedriver nahi mila'); process.exit(1); }
            process.stdout.write(win.url);
        });
    ")"
else
    URL="https://storage.googleapis.com/chrome-for-testing-public/${VERSION}/win64/chromedriver-win64.zip"
fi

echo "==> chromedriver $VERSION download ho raha hai..."
TMP="$(mktemp -d)"
trap 'rm -rf "$TMP"' EXIT

curl -fL --retry 3 -o "$TMP/chromedriver.zip" "$URL"
unzip -q -o "$TMP/chromedriver.zip" -d "$TMP"

FOUND="$(find "$TMP" -type f -name 'chromedriver.exe' | head -n 1)"
if [ -z "$FOUND" ]; then
    echo "error: zip ke andar chromedriver.exe nahi mila" >&2
    exit 1
fi

cp "$FOUND" "$OUT_DIR/chromedriver.exe"
chmod +x "$OUT_DIR/chromedriver.exe" || true

echo
echo "✅ Tayyar: $OUT_DIR/chromedriver.exe (version $VERSION)"
echo "   Yaad rahe: iska major version aapke Chrome se match karna chahiye."
