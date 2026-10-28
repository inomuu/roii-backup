#!/usr/bin/env bash
# Build sim-ffi and stage it into the Unity project.
#
# The copy is rm + cp + codesign, never a plain `cp`: writing over a dylib that a running process
# has already dlopen'd corrupts the mapped image, and the symptom is not an error — the host dies
# at load with an empty log. Ad-hoc signing after the copy is what macOS needs to load it at all.
#
# A native plugin is NOT hot-reloadable: the Unity Editor holds it open for the whole session, so
# a new build only takes effect after the Editor is restarted. This script says so rather than
# leaving the next person to wonder why their new entry point is missing.
#
#   tools/unity_dylib.sh            # release (what a build ships)
#   tools/unity_dylib.sh --debug
set -euo pipefail
cd "$(dirname "$0")/.."

PROFILE=release
[ "${1:-}" = "--debug" ] && PROFILE=debug

if [ "$PROFILE" = release ]; then
  cargo build -p sim-ffi --release
else
  cargo build -p sim-ffi
fi

SRC="target/$PROFILE/libsim_ffi.dylib"
DEST="unity/ROI/Assets/Plugins/macOS/libsim_ffi.dylib"
[ -f "$SRC" ] || { echo "no $SRC"; exit 1; }

if pgrep -x Unity >/dev/null; then
  echo "⚠  the Unity Editor is running — it holds the OLD plugin until you restart it."
fi

mkdir -p "$(dirname "$DEST")"
rm -f "$DEST"
cp "$SRC" "$DEST"
codesign -f -s - "$DEST"
echo "staged $PROFILE -> $DEST"
nm -gU "$DEST" | grep -c ' _roi_' | xargs -I{} echo "exported entry points: {}"
