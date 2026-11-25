#!/usr/bin/env bash
# ============================================================
#  Automata — Linux/macOS se poora Windows app banao
#
#  Ye ek hi command mein:
#    1. C++ backend ko api_server.exe bana kar Windows ke liye compile
#    2. chromedriver.exe download
#    3. Electron UI ko Windows .exe (installer + portable) mein pack
#
#  Output: build/windows/dist/
#      Automata-Setup-1.0.0.exe      -> installer
#      Automata-Portable-1.0.0.exe   -> bina install, double-click chalao
# ============================================================
set -euo pipefail

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT="$(cd "$HERE/../.." && pwd)"

echo "============================================================"
echo " 1/3  Backend (api_server.exe)"
echo "============================================================"
"$HERE/build-backend.sh"

echo
echo "============================================================"
echo " 2/3  ChromeDriver"
echo "============================================================"
if [ -f "$HERE/artifacts/driver/chromedriver.exe" ]; then
    echo "==> Pehle se mojood hai, skip."
else
    # Driver na mile to bhi app build ho jaye — sirf browser wale kaam
    # (navigate/click/draw) nahi chalenge, chat phir bhi chalegi.
    "$HERE/get-chromedriver.sh" || echo "WARNING: chromedriver download fail hua, app phir bhi banegi."
fi

echo
echo "============================================================"
echo " 3/3  Electron Windows package"
echo "============================================================"
cd "$ROOT/frontend"

if [ ! -d node_modules ]; then
    npm install
else
    npm install --no-audit --no-fund
fi

npx electron-builder --win --x64 --publish never

echo
echo "============================================================"
echo " ✅ TAYYAR"
echo "============================================================"
ls -lh "$HERE/dist"/*.exe 2>/dev/null || true
echo
echo "Windows par:"
echo "  Automata-Setup-*.exe     -> install karo, Start Menu se chalao"
echo "  Automata-Portable-*.exe  -> seedha double-click (backend khud start hota hai)"
