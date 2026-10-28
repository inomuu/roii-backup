#!/usr/bin/env bash
# Run the GAME (not the Godot editor) straight from a terminal.
#
# Why: the Rust gdext dylib is reloadable=false (editor hot-reload double-inits it → SIGABRT crash).
# Launching the GAME binary loads the dylib exactly ONCE at startup, so:
#   - no hot-reload → no crash
#   - every launch picks up the FRESHLY BUILT dylib → nothing to remember, no editor needed
#
# Usage:
#   tools/play.sh              # rebuild the bridge dylib, copy it in, kill any stale Godot, launch
#   tools/play.sh --no-build   # skip the cargo build (just relaunch the current dylib)
#   tools/play.sh --2d         # launch the flat 2D scene instead of the default HD-2D one
#   GODOT=/path/to/Godot tools/play.sh
#
# HD-2D IS THE DEFAULT (user 2026-08-20): `project.godot`'s main_scene is
# `proto/arena3d/main3d.tscn`, so a bare `play.sh` — and an export — launches it. `--2d` runs the
# flat scene, which is still the one every `views/` change should be checked against, since the 3D
# scene inherits its script and its wiring.
#
# `--3d` is kept as a no-op alias so an existing habit or script does not break.
#
# Both boot to the SAME Online/Offline picker and character-creation screen (since 2026-08-18 —
# main3d used to skip both and silently resume the active save slot).
#
# ⚠️ It writes the SAME `user://` save slot as the 2D client. Never run both: whichever
# autosaves last silently overwrites the other's progress. The single-instance lock in main.gd
# catches them running at once; two runs back to back are on you.
#
# Everything else is identical, main3d.gd being an empty subclass of main.gd.
set -euo pipefail
cd "$(dirname "$0")/.."

GODOT="${GODOT:-/Applications/Godot.app/Contents/MacOS/Godot}"
[ -x "$GODOT" ] || GODOT="$(ls /Applications/Godot*.app/Contents/MacOS/Godot 2>/dev/null | head -1 || true)"
[ -n "$GODOT" ] && [ -x "$GODOT" ] || { echo "Godot not found — set GODOT=/path/to/Godot"; exit 1; }

# macOS = .dylib, Linux = .so
case "$(uname -s)" in
  Darwin) LIB="libgodot_ext.dylib" ;;
  Linux)  LIB="libgodot_ext.so" ;;
  *) echo "unsupported OS"; exit 1 ;;
esac

# Stage the bridge library, REPLACING the file rather than overwriting it in place.
#
# `cp` onto the existing path reuses the inode, and macOS keeps the code-signature page hashes it
# already validated for that file. dyld then loads pages whose hashes no longer match and the
# kernel kills the process: CODESIGNING / "Invalid Page", SIGKILL, no message. It surfaces as the
# game refusing to start with nothing in the log but the ApplePersistence line, because it dies
# inside `dlopen` before any scene loads — and it reads as a crash in whatever was last edited.
# `rm` first gives a new inode; the ad-hoc re-sign covers the case where the build left the
# signature stale. Both are cheap and neither is conditional on having seen the failure.
stage_lib() {
  rm -f "$2"
  cp "$1" "$2"
  [ "$(uname -s)" = "Darwin" ] && codesign -f -s - "$2" >/dev/null 2>&1
  return 0
}

SCENE=""
BUILD=1
for a in "$@"; do
  case "$a" in
    --no-build) BUILD=0 ;;
    # `--3d` is now what happens anyway; kept so an old habit still works.
    --3d) SCENE="res://proto/arena3d/main3d.tscn" ;;
    --2d) SCENE="res://main.tscn" ;;
    *) echo "unknown flag: $a  (see the header for usage)"; exit 2 ;;
  esac
done

if [ "$BUILD" = "1" ]; then
  echo "== building bridge dylib =="
  # DEBUG build, deliberately — it is what an edit-run loop wants. Be aware when judging
  # performance from a play-test: the sim and `view_state`'s JSON serialisation run unoptimised,
  # and since view_state is rebuilt every TICK the cost lands in bursts, so the frame rate swings
  # rather than sitting low. A steady low frame rate is a rendering problem; a swinging one is
  # usually this. Measure perf against `cargo build --release -p godot-ext` before believing it.
  cargo build -p godot-ext
  stage_lib "target/debug/$LIB" "godot/bin/$LIB"
  echo "== copied fresh $LIB =="
fi

# Kill any running Godot (a hung/old instance holding a stale dylib) so this launch is clean.
pkill -9 -i godot 2>/dev/null || true
sleep 0.5

if [ -n "$SCENE" ]; then
  echo "== launching $SCENE =="
else
  echo "== launching the game, HD-2D (Ctrl-C or close the window to stop) =="
fi
# Run with CWD = godot/ so the Steamworks SDK finds steam_appid.txt (it reads the App ID from the
# working directory). Steam features need the Steam client running + the app owned (Dev Comp).
cd godot
# A scene path after --path runs THAT scene instead of project.godot's main_scene, so the 3D
# prototype needs no project-setting edit — which matters because project.godot is shared and a
# flipped main_scene is the kind of change that gets committed by accident.
ARGS=(--path .)
[ -n "$SCENE" ] && ARGS+=("$SCENE")
exec "$GODOT" "${ARGS[@]}"
