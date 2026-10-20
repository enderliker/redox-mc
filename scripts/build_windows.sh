#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

# Add user-space mingw-w64 toolchain to PATH if present
if [ -d "${HOME}/.local/mingw-w64/usr/bin" ]; then
    export PATH="${HOME}/.local/mingw-w64/usr/bin:${PATH}"
fi

echo "=== Building Redox-MC for Windows x86_64 (x86_64-pc-windows-gnu) ==="
cd "${ROOT_DIR}"

cargo build --target x86_64-pc-windows-gnu -p redox-mc-loader -p test-mod

LOADER_DLL="${ROOT_DIR}/target/x86_64-pc-windows-gnu/debug/redox_mc_loader.dll"
MOD_DLL="${ROOT_DIR}/target/x86_64-pc-windows-gnu/debug/test_mod.dll"

if [ -f "${LOADER_DLL}" ] && [ -f "${MOD_DLL}" ]; then
    echo "SUCCESS: Built Windows DLLs:"
    ls -lh "${LOADER_DLL}" "${MOD_DLL}"
else
    echo "ERROR: Expected DLL binaries were not found!"
    exit 1
fi
