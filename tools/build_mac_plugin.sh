#!/usr/bin/env bash
# Build the macOS window plugin into the Unity project.
#
# A .bundle is a directory with a Mach-O inside; Unity loads it as a native plugin and P/Invoke
# finds the symbols by the BUNDLE's name. Built as a universal binary so one file serves Apple
# Silicon and Intel — a plugin that is arm64-only silently fails to load on an x86_64 player, and
# the symptom is a DllNotFoundException at the first call rather than at load.
#
# Re-run after editing native/macos/roi_window.m. The Editor holds a loaded plugin for the life of
# the process, so restart it afterwards (the same rule as libsim_ffi.dylib).
set -euo pipefail
root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
src="$root/unity/ROI/native/macos/roi_window.m"
out="$root/unity/ROI/Assets/Plugins/macOS/roi_window.bundle"
bin="$out/Contents/MacOS/roi_window"

mkdir -p "$(dirname "$bin")"
cat > "$out/Contents/Info.plist" <<PLIST
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>CFBundleDevelopmentRegion</key><string>en</string>
  <key>CFBundleExecutable</key><string>roi_window</string>
  <key>CFBundleIdentifier</key><string>com.roi.window</string>
  <key>CFBundleInfoDictionaryVersion</key><string>6.0</string>
  <key>CFBundleName</key><string>roi_window</string>
  <key>CFBundlePackageType</key><string>BNDL</string>
  <key>CFBundleShortVersionString</key><string>1.0</string>
  <key>CFBundleVersion</key><string>1</string>
</dict>
</plist>
PLIST

clang -bundle -fobjc-arc -framework AppKit -framework Foundation -framework QuartzCore \
      -arch arm64 -arch x86_64 \
      -mmacosx-version-min=11.0 \
      -o "$bin" "$src"

# SIGNED AD-HOC here so the Editor and a local build will load it; the real signature comes from
# signing the whole .app at export time, which re-signs everything inside it.
codesign --force --sign - "$out" >/dev/null 2>&1 || true

echo "built $out"
lipo -archs "$bin"
