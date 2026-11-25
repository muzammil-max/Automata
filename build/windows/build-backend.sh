#!/usr/bin/env bash
# ============================================================
#  Automata — backend ko Windows ke liye cross-compile karo
#  (Linux / macOS par chalta hai, MSVC ki zarurat nahi)
#
#  Ye script "zig cc/c++" use karti hai. Zig apne saath poora
#  C++ runtime + Windows headers le kar aata hai, is liye alag se
#  MinGW install karne ki zarurat nahi hoti.
#
#  Output: build/windows/artifacts/backend/api_server.exe
# ============================================================
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
BACKEND="$ROOT/backend"
OUT_DIR="$ROOT/build/windows/artifacts/backend"
TOOLS_DIR="$ROOT/build/windows/.tools"
ZIG_VERSION="${ZIG_VERSION:-0.17.1}"

mkdir -p "$OUT_DIR" "$TOOLS_DIR"

# ---- 1. Zig dhoondo (ya download karo) ----------------------------------
case "$(uname -s)" in
    Linux)  ZIG_OS="linux" ;;
    Darwin) ZIG_OS="macos" ;;
    *) echo "error: is OS par cross-build support nahi hai ($(uname -s))"; exit 1 ;;
esac

case "$(uname -m)" in
    x86_64|amd64) ZIG_ARCH="x86_64" ;;
    arm64|aarch64) ZIG_ARCH="aarch64" ;;
    *) echo "error: unknown CPU $(uname -m)"; exit 1 ;;
esac

ZIG_DIR="$TOOLS_DIR/zig-$ZIG_ARCH-$ZIG_OS-$ZIG_VERSION"
ZIG="$ZIG_DIR/zig"

if [ ! -x "$ZIG" ]; then
    echo "==> Zig $ZIG_VERSION download ho raha hai..."
    TARBALL="$TOOLS_DIR/zig-$ZIG_VERSION.tar.xz"
    URL="https://ziglang.org/download/$ZIG_VERSION/zig-$ZIG_ARCH-$ZIG_OS-$ZIG_VERSION.tar.xz"
    curl -fL --retry 3 -o "$TARBALL" "$URL"
    tar -xf "$TARBALL" -C "$TOOLS_DIR"
    rm -f "$TARBALL"
fi

echo "==> Zig: $("$ZIG" version)"

# ---- 2. Backend compile karo -------------------------------------------
SOURCES=(
    main.cpp
    api/api_server.cpp
    api/nvidia_client.cpp
    api/http_client.cpp
    agent/agent.cpp
    browser/browser_controller.cpp
    parser/parser.cpp
    storage/session_storage.cpp
    instructions/instruction.cpp
)

echo "==> api_server.exe build ho raha hai (x86_64-windows-gnu)..."
cd "$BACKEND"

"$ZIG" c++ \
    -target x86_64-windows-gnu \
    -std=c++17 \
    -O2 \
    -Wall -Wextra \
    "${SOURCES[@]}" \
    -lwinhttp -lws2_32 -luser32 \
    -o "$OUT_DIR/api_server.exe"

echo
# Zig debug symbols (.pdb) install mein nahi chahiye.
rm -f "$OUT_DIR"/*.pdb

echo "✅ Tayyar: $OUT_DIR/api_server.exe"
ls -lh "$OUT_DIR/api_server.exe"
