#!/usr/bin/env bash
# Pre-flight: is the ONLINE stack in sync? Catches the client-dylib vs deployed-server
# skew that silently rejects every online save ("... version skew" in-game).
#
# There are TWO gates, not one — `verify-server` rejects on `sim_version` AND on
# `game_data_version` (main.rs:3205-3213) — and this script only ever checked the first. The
# catalog is the easier one to get wrong: it is a hand-typed string, it sat at 0.0.68-alpha across
# six catalog changes, and the server bakes its copy in at IMAGE BUILD (Dockerfile:43), so editing
# data and restarting is not enough. sim-core's `the_catalog_cannot_change_without_its_version_
# changing` stops the string going stale; this stops the deploy going stale.
#
# Two sessions bumping sim_core::VERSION + deploy lag means the client's dylib and the
# Fly server can drift apart. Run this before an online play test.
#
#   tools/check_online_sync.sh [server-url]
set -u
URL="${1:-https://renrei-idle-server.fly.dev}"
root="$(cd "$(dirname "$0")/.." && pwd)"

# Unity is THE client (Godot cancelled 2026-09-19); ROI_CLIENT=godot keeps the historical check.
case "${ROI_CLIENT:-unity}" in
    unity)
        client_lib="$root/unity/ROI/Assets/Plugins/macOS/libsim_ffi.dylib"
        client_catalog="$root/unity/ROI/Assets/StreamingAssets/game_data.json"
        client_rewards="$root/unity/ROI/Assets/StreamingAssets/commander_rewards.json"
        ;;
    godot)
        client_lib="$root/godot/bin/libgodot_ext.dylib"
        client_catalog="$root/godot/data/game_data.json"
        client_rewards="$root/godot/data/commander_rewards.json"
        ;;
    *) echo "unknown ROI_CLIENT (expected unity or godot)"; exit 1 ;;
esac

tree_ver=$(grep 'pub const VERSION' "$root/crates/sim-core/src/lib.rs" | grep -oE '0\.0\.[0-9]+-alpha')
dylib_ver=$(strings "$client_lib" 2>/dev/null | grep -oE '0\.0\.[0-9]+-alpha' | sort -V | tail -1)
resp=$(curl -s -m 25 "$URL/version")
server_ver=$(echo "$resp" | grep -oE '"sim_version":"[^"]+"' | cut -d'"' -f4)

tree_data=$(grep -m1 -oE '"version": "[^"]+"' "$root/data/game_data.json" | cut -d'"' -f4)
server_data=$(echo "$resp" | grep -oE '"game_data_version":"[^"]+"' | cut -d'"' -f4)
# The client ships its OWN copy (exports cannot read the repo's data/), so a catalog edit that
# never reached godot/data/ is a third way to be out of sync — and one no server check can see.
client_data=$(grep -m1 -oE '"version": "[^"]+"' "$client_catalog" | cut -d'"' -f4)

# SOURCE IDENTITY: compare git SHAs, not just the hand-bumped version string —
# two builds can both say "0.0.X-alpha" while running different code (the
# knockback-diff incident). Stamps are baked by sim-core's build.rs.
head_sha=$(git -C "$root" rev-parse --short=12 HEAD 2>/dev/null || echo "?")
dylib_sha=$(strings "$client_lib" 2>/dev/null | grep -m1 -oE 'SIMGIT:[a-f0-9]{12}(-dirty)?' | cut -d: -f2)
server_sha=$(echo "$resp" | grep -oE '"git_sha":"[^"]+"' | cut -d'"' -f4)
server_build=$(echo "$resp" | grep -oE '"sim_build":"[0-9a-f]+"' | cut -d'"' -f4)
server_prod=$(echo "$resp" | grep -oE '"prod":(true|false)' | cut -d: -f2)
server_money=$(echo "$resp" | grep -oE '"money_live":(true|false)' | cut -d: -f2)

echo "tree  (lib.rs)  : ${tree_ver:-?}  @ ${head_sha}"
echo "dylib (client)  : ${dylib_ver:-?}  @ ${dylib_sha:-no-stamp}"
echo "server (fly)    : ${server_ver:-?}  @ ${server_sha:-no-stamp}"
echo "game_data       : tree ${tree_data:-?} / client ${client_data:-?} / server ${server_data:-?}"
echo "server mode     : prod=${server_prod:-?} money_live=${server_money:-?}"
echo

fail=0
# The server refuses to BOOT in this state (live_money_misconfig); seeing it here means a build
# from before that guard is running with real money on.
if [ "${server_money:-}" = "true" ] && [ "${server_prod:-}" != "true" ]; then
    echo "✗ REAL MONEY WITHOUT PROD MODE — the test fixtures (/auth/dev) are being served"
    fail=1
fi
if [ "$dylib_ver" != "$tree_ver" ]; then
    echo "✗ CLIENT dylib is stale → rebuild and stage ${ROI_CLIENT:-unity}: $client_lib"
    fail=1
fi
if [ "$server_ver" != "$tree_ver" ]; then
    echo "✗ SERVER is stale → tools/deploy.sh"
    fail=1
fi
if [ -n "${client_data:-}" ] && [ "$client_data" != "$tree_data" ]; then
    echo "✗ CLIENT CATALOG is stale → update $client_catalog from data/game_data.json"
    fail=1
fi
# Commander reward config is STATIC and duplicated the same way (source in data/, gitignored client
# copy under godot/): the server credits from data/, the client renders the track from godot/data/, so a
# byte drift means a player sees one Crystal total and is paid another. No version string to compare —
# the files must be byte-identical.
if [ -f "$client_rewards" ] \
   && ! cmp -s "$root/data/commander_rewards.json" "$client_rewards"; then
    echo "✗ CLIENT REWARD FILE is stale → update $client_rewards from data/commander_rewards.json"
    fail=1
fi
if [ -n "${server_data:-}" ] && [ "$server_data" != "$tree_data" ]; then
    echo "✗ SERVER CATALOG is stale (${server_data} vs ${tree_data}) → tools/deploy.sh — it is baked"
    echo "  into the image at build, so restarting the server does NOT pick up a data edit."
    fail=1
fi
# The save gate compares SIM_BUILD (a digest of the sim's code), not the git SHA: a server-only
# deploy moves the SHA and must not fail this check. A server without `sim_build` in /version
# predates it — then the SHA is the identity (both sides stamped).
if [ -n "${server_build:-}" ]; then
    if strings "$client_lib" 2>/dev/null | grep -q "$server_build"; then
        echo "sim build       : $server_build (client and server)"
    else
        echo "✗ SIM BUILD MISMATCH: the client library lacks the server's $server_build — every online save would be refused"
        fail=1
    fi
elif [ -n "${dylib_sha:-}" ] && [ -n "${server_sha:-}" ] && [ "$dylib_sha" != "$server_sha" ]; then
    echo "✗ SOURCE MISMATCH: dylib=$dylib_sha vs server=$server_sha — same version string, DIFFERENT code"
    fail=1
fi
case "${dylib_sha:-}${server_sha:-}" in *dirty*)
    echo "⚠ a build carries -dirty (built over uncommitted sim changes) — commit + rebuild for a clean identity"
;; esac
if [ "$fail" = 0 ]; then
    echo "✓ in sync ($tree_ver @ ${server_sha:-no-stamp}, game_data ${tree_data:-?}) — client and server run the same sim source and the same catalog"
fi
exit $fail
