#!/usr/bin/env bash
# Start verify-server on 127.0.0.1:8787 against the LOCAL roi_dev database — the server the Unity
# probes (tools/unity_probes.sh) and a local play-test talk to. Never roi_test (`cargo test`
# TRUNCATEs it) and never production.
#
# Secrets are read BY REFERENCE out of .secrets/ and passed as environment, never as arguments:
# argv is world-readable in `ps`, a process's environment is not. Nothing here echoes a value.
set -euo pipefail
cd "$(dirname "$0")/.."

BUILD=1
[ "${1:-}" = "--no-build" ] && BUILD=0

DB="${DATABASE_URL:-postgres://$(whoami)@localhost/roi_dev}"
# The HOST part of the URL, not a substring anywhere in it (audit 2026-10-02: a remote URL with
# `?host=localhost` passed). Empty host = a local unix socket. The URL is never echoed (it may
# carry a password).
DB_HOST="$(printf '%s' "$DB" | sed -E 's#^[a-zA-Z0-9+.-]+://([^@/]*@)?(\[[^]]*\]|[^/:?]*).*#\2#')"
case "$DB_HOST" in
  localhost|127.0.0.1|"[::1]"|"") ;;
  *) echo "✗ DATABASE_URL host '$DB_HOST' is not local — refusing to start (this script is dev-only)" >&2; exit 2 ;;
esac
# A query-string host overrides the URL's (`?host=` / `?hostaddr=`, libpq): only a unix socket
# directory (an absolute path) is local.
case "$DB" in
  *\?*host=*|*\&host=*|*hostaddr=*)
    q_host="$(printf '%s' "$DB" | sed -nE 's#.*[?&](host|hostaddr)=([^&]*).*#\2#p')"
    case "$q_host" in
      /*|%2F*|%2f*) ;;
      *) echo "✗ DATABASE_URL carries a non-socket host parameter — refusing to start (this script is dev-only)" >&2; exit 2 ;;
    esac ;;
esac
case "$DB" in
  *roi_test*) echo "✗ roi_test is the test database — \`cargo test\` TRUNCATEs it. Use roi_dev." >&2; exit 2 ;;
esac

# Stable across restarts: a fresh random secret would invalidate every server-signed roll already
# sitting in roi_dev from the last session.
ROLL=.secrets/roll_secret
if [ ! -f "$ROLL" ]; then
  mkdir -p .secrets
  openssl rand -hex 32 > "$ROLL"
  chmod 600 "$ROLL"
  echo "·  generated $ROLL (local only, gitignored)"
fi

STEAM=.secrets/steam_publisher_key
if [ -s "$STEAM" ]; then
  # Whitespace-stripped: the safe way to write this file is `cat > file` + Ctrl-D, which leaves a
  # trailing newline. Carrying it into the query string makes Steam answer "Invalid parameter" for
  # a key that is perfectly correct — an hour lost to an invisible character.
  export STEAM_PUBLISHER_KEY="$(tr -d '[:space:]' < "$STEAM")"
  echo "·  Steam auth ENABLED — /auth/steam will validate against the real Steam Web API"
else
  echo "!  no $STEAM — /auth/steam will answer \"steam auth not configured\"."
  echo "   write the publisher key there yourself, without it passing through argv or shell history:"
  echo "     cat > $STEAM        # paste the key, Enter, then Ctrl-D"
  echo "     chmod 600 $STEAM"
fi

[ "$BUILD" = 1 ] && cargo build -q -p verify-server

echo "·  roi_dev @ 127.0.0.1:8787"
DATABASE_URL="$DB" \
DEV_KEY="${DEV_KEY:-roi-local-dev}" \
SERVER_ROLL_SECRET="$(cat "$ROLL")" \
BIND="${BIND:-127.0.0.1:8787}" \
GAME_DATA_PATH="${GAME_DATA_PATH:-data/game_data.json}" \
exec ./target/debug/verify-server
