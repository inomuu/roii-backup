#!/usr/bin/env bash
# Deploy the verify-server with SOURCE-IDENTITY guarantees — the answer to
# "how do I know the server runs EXACTLY the sim I have locally?"
#
#   1. REFUSE a dirty tree for build-affecting paths (crates/ data/ Cargo.*
#      Dockerfile) — `fly deploy` ships the WORKING TREE, not HEAD, so an
#      uncommitted edit would ship code no commit describes. --force overrides.
#   2. Stamp the image with the exact commit (build-arg GIT_SHA → sim-core
#      build.rs → /version git_sha).
#   3. After deploy, verify the server reports THAT sha, then run the full
#      version+sha sync check against the local dylib.
#
#   tools/deploy.sh [--force]
set -eu
root="$(cd "$(dirname "$0")/.." && pwd)"
cd "$root"

dirty=$(git status --porcelain -- crates data Cargo.toml Cargo.lock Dockerfile)
if [ -n "$dirty" ] && [ "${1:-}" != "--force" ]; then
    echo "✗ build-affecting paths have UNCOMMITTED changes — commit first (or --force):"
    echo "$dirty"
    exit 1
fi

# The availability lints (crates/verify-server/clippy.toml: no `.unwrap()`, no raw `.lock()`, no
# slice indexing) fire ONLY under clippy, which `cargo test` never runs. Without this line they are
# a rule nothing enforces. Release sets `panic = "abort"`, so a panic that slips through is not a
# failed request — it is the process, and with min_machines_running = 0, a cold start after it.
echo "clippy: verify-server availability lints …"
cargo clippy -p verify-server --all-targets -q

sha=$(git rev-parse --short=12 HEAD)
[ -n "$dirty" ] && sha="$sha-dirty"
echo "deploying commit $sha …"
# An explicit fallback is useful when Depot authentication is unavailable.
case "${ROI_FLY_BUILDER:-auto}" in
    auto) fly deploy --remote-only --build-arg GIT_SHA="$sha" ;;
    buildkit) fly deploy --remote-only --depot=false --buildkit --build-arg GIT_SHA="$sha" ;;
    *) echo "unknown ROI_FLY_BUILDER (auto or buildkit)"; exit 1 ;;
esac

server_sha=$(curl -s -m 25 "https://renrei-idle-server.fly.dev/version" \
    | grep -oE '"git_sha":"[^"]+"' | cut -d'"' -f4)
echo "server git_sha : ${server_sha:-?}  (local HEAD: $sha)"
if [ "$server_sha" != "$sha" ]; then
    echo "✗ deployed server does NOT report the expected commit — investigate before playing online"
    exit 1
fi
echo "✓ server runs exactly commit $sha"
echo
exec "$root/tools/check_online_sync.sh"
