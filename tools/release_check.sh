#!/usr/bin/env bash
# RELEASE GATE for a Unity player build — run it on the BUILT player before uploading to Steam.
#
# The failure it stops: a client whose sim library is not the exact code the production server
# replays with. Every save that client pushes fails replay (`hash-mismatch`) and is flagged, for
# every player on that build at once. `tools/deploy.sh` cannot catch it — the server never sees
# which build a player runs — so the check has to sit where a build leaves our hands.
#
# BLOCKS (exit 1) when any sim library in the build:
#   - carries no SIMGIT stamp, or a `-dirty` one (built over uncommitted sim changes);
#   - reports a sim version or git SHA different from the server's /version;
# or when the build's catalog (StreamingAssets/game_data.json) differs from the server's version,
# or when the game code still carries the dev doors (a "Development Build").
#
#   tools/release_check.sh <built-player-dir-or-.app> [server-url]
set -u
BUILD="${1:-}"
URL="${2:-https://renrei-idle-server.fly.dev}"
if [ -z "$BUILD" ] || [ ! -e "$BUILD" ]; then
    echo "usage: tools/release_check.sh <built-player-dir-or-.app> [server-url]"
    exit 2
fi

resp=$(curl -s -m 25 "$URL/version")
server_ver=$(echo "$resp" | grep -oE '"sim_version":"[^"]+"' | cut -d'"' -f4)
server_sha=$(echo "$resp" | grep -oE '"git_sha":"[^"]+"' | cut -d'"' -f4)
server_data=$(echo "$resp" | grep -oE '"game_data_version":"[^"]+"' | cut -d'"' -f4)
# The replay identity the save gate compares (`sim_core::SIM_BUILD`). Empty from a server built
# before it existed — then the git SHA is the only identity and must match.
server_build=$(echo "$resp" | grep -oE '"sim_build":"[0-9a-f]+"' | cut -d'"' -f4)
if [ -z "$server_ver" ] || [ -z "$server_sha" ]; then
    echo "✗ could not read $URL/version — refusing to pass a build against an unknown server"
    exit 1
fi
echo "server : $server_ver @ $server_sha, catalog $server_data"

fail=0
libs=$(find "$BUILD" -type f \( -name 'libsim_ffi.dylib' -o -name 'sim_ffi.dll' -o -name 'libsim_ffi.so' \) 2>/dev/null)
if [ -z "$libs" ]; then
    echo "✗ no sim library (libsim_ffi.dylib / sim_ffi.dll / libsim_ffi.so) found under $BUILD"
    exit 1
fi
while IFS= read -r lib; do
    ver=$(strings "$lib" 2>/dev/null | grep -oE '0\.0\.[0-9]+-alpha' | sort -V | tail -1)
    sha=$(strings "$lib" 2>/dev/null | grep -m1 -oE 'SIMGIT:[a-f0-9]{12}(-dirty)?' | cut -d: -f2)
    echo "lib    : ${ver:-?} @ ${sha:-no-stamp}  ($lib)"
    if [ -z "$sha" ]; then
        echo "  ✗ no SIMGIT stamp — cannot prove which code this is"
        fail=1
    fi
    case "$sha" in *dirty*)
        echo "  ✗ built over UNCOMMITTED sim changes (-dirty) — commit, rebuild the library, rebuild the player"
        fail=1
    ;; esac
    if [ "$ver" != "$server_ver" ]; then
        echo "  ✗ sim version $ver != server $server_ver — every save would be refused as version skew"
        fail=1
    fi
    if [ -n "$server_build" ]; then
        # A server-only redeploy moves the SHA, not the sim: what must match is SIM_BUILD.
        if strings "$lib" 2>/dev/null | grep -q "$server_build"; then
            [ -n "$sha" ] && [ "${sha%-dirty}" != "$server_sha" ] && \
                echo "  · library $sha vs server $server_sha: different commits, same sim build $server_build"
        else
            echo "  ✗ SIM BUILD MISMATCH: library lacks the server's $server_build — every save would be refused"
            fail=1
        fi
    elif [ -n "$sha" ] && [ "${sha%-dirty}" != "$server_sha" ]; then
        echo "  ✗ SOURCE MISMATCH: library $sha vs server $server_sha — every save would fail replay and be FLAGGED"
        fail=1
    fi
done <<< "$libs"

catalogs=$(find "$BUILD" -type f -name 'game_data.json' -path '*StreamingAssets*' 2>/dev/null)
if [ -z "$catalogs" ]; then
    echo "✗ no StreamingAssets/game_data.json found under $BUILD"
    fail=1
fi
while IFS= read -r cat; do
    [ -z "$cat" ] && continue
    cver=$(grep -m1 -oE '"version": "[^"]+"' "$cat" | cut -d'"' -f4)
    echo "catalog: ${cver:-?}  ($cat)"
    if [ "$cver" != "$server_data" ]; then
        echo "  ✗ catalog $cver != server $server_data — run tools/unity_data.sh and rebuild the player"
        fail=1
    fi
done <<< "$catalogs"

# Commander reward track: the server pays from data/commander_rewards.json (baked into its image),
# the client renders the track from its StreamingAssets copy — byte drift shows one Crystal total
# and pays another. No version string, so the files must be identical (as check_online_sync.sh).
root=$(cd "$(dirname "$0")/.." && pwd)
# The SERVER's copy is the one at the SHA it runs, not this working tree's (audit 2026-10-03: an
# edited-but-undeployed file passed a build the live server pays differently).
server_rewards=$(mktemp)
trap 'rm -f "$server_rewards"' EXIT
if ! git -C "$root" show "$server_sha:data/commander_rewards.json" > "$server_rewards" 2>/dev/null; then
    echo "✗ cannot read data/commander_rewards.json at server SHA $server_sha (git fetch?)"
    fail=1
fi
rewards=$(find "$BUILD" -type f -name 'commander_rewards.json' -path '*StreamingAssets*' 2>/dev/null)
if [ -z "$rewards" ]; then
    echo "✗ no StreamingAssets/commander_rewards.json found under $BUILD"
    fail=1
fi
while IFS= read -r rw; do
    [ -z "$rw" ] && continue
    if [ -s "$server_rewards" ] && cmp -s "$server_rewards" "$rw"; then
        echo "commander rewards: match  ($rw)"
    else
        echo "  ✗ $rw differs from data/commander_rewards.json at $server_sha — sync to the deployed copy and rebuild the player"
        fail=1
    fi
done <<< "$rewards"

# Dev doors: TitleScreen.DevDoors / SessionBoot's ROI_BOOT override are compiled in only under
# UNITY_EDITOR || DEVELOPMENT_BUILD. A player built with "Development Build" ticked ships a local
# cheat session behind a title-screen button. The button's loc key is a literal inside that #if,
# so its presence in the compiled game code means the doors are in the build. Mono keeps C#
# literals as UTF-16LE (Assembly-CSharp.dll), IL2CPP as UTF-8 (global-metadata.dat).
code=$(find "$BUILD" -type f \( -name 'Assembly-CSharp.dll' -o -name 'global-metadata.dat' \) 2>/dev/null)
if [ -z "$code" ]; then
    echo "✗ no game code (Assembly-CSharp.dll / global-metadata.dat) found under $BUILD — cannot check for dev doors"
    fail=1
fi
while IFS= read -r f; do
    [ -z "$f" ] && continue
    if python3 - "$f" <<'PY'
import sys
b = open(sys.argv[1], "rb").read()
k = "UI_TITLE_LOCAL_SESSION_DEV"
sys.exit(0 if (k.encode() in b or k.encode("utf-16-le") in b) else 1)
PY
    then
        echo "  ✗ DEV DOORS PRESENT in $f — this is a Development Build; untick it and rebuild"
        fail=1
    else
        echo "code   : no dev doors  ($f)"
    fi
done <<< "$code"

if [ "$fail" = 0 ]; then
    echo "✓ release OK — the build runs exactly the server's sim ($server_ver @ $server_sha) and catalog ($server_data)"
else
    echo "✗ RELEASE BLOCKED — do not upload this build"
fi
exit $fail
