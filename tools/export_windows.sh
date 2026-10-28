#!/usr/bin/env bash
# export_windows.sh — cross-build the Rust GDExtension to a Windows DLL and
# export a standalone Windows build (for testers) from macOS.
#
# Prerequisites (one-time):
#   rustup target add x86_64-pc-windows-msvc
#   cargo install cargo-xwin
#   brew install llvm lld        # llvm-lib + clang-cl from llvm, lld-link from lld
#   Godot 4.6 with the Windows export templates installed.
#
# Output: dist/windows/{MinimalIdleMMO.exe, .pck, godot_ext.dll}
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$REPO_ROOT"

GODOT="${GODOT:-/opt/homebrew/bin/godot}"
# BOTH kegs are needed and they are separate formulae. LLVM 22 split `lld` out of the `llvm`
# bottle, which silently broke this script between the 2026-06-21 export and 2026-07-31 —
# `llvm-lib` was still present, only `lld-link` vanished. Both are keg-only, hence the explicit
# PATH rather than relying on the shell.
export PATH="/opt/homebrew/opt/llvm/bin:/opt/homebrew/opt/lld/bin:$PATH"

# Fail LOUDLY and specifically: cargo-xwin's own error for a missing linker is opaque.
for tool in llvm-lib lld-link; do
	command -v "$tool" >/dev/null || {
		echo "error: '$tool' not found. Run: brew install llvm lld" >&2
		exit 1
	}
done

echo "==> cross-building godot_ext.dll (x86_64-pc-windows-msvc, release)"
cargo xwin build -p godot-ext --target x86_64-pc-windows-msvc --release

echo "==> staging the Windows DLL + a bundled copy of game_data.json"
cp target/x86_64-pc-windows-msvc/release/godot_ext.dll godot/bin/godot_ext.dll
mkdir -p godot/data
cp data/game_data.json godot/data/game_data.json   # exports can't read repo/data
cp data/commander_rewards.json godot/data/commander_rewards.json   # static reward config the client renders

echo "==> exporting the Windows build"
mkdir -p dist/windows
"$GODOT" --headless --path godot --export-release "Windows Desktop" \
	../dist/windows/MinimalIdleMMO.exe

echo "==> done: dist/windows/"
ls -la dist/windows/
