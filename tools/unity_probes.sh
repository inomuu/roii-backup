#!/usr/bin/env bash
# Run the Unity client's probes, in order, and exit non-zero if any of them failed.
#
# This is the client's whole safety net in one command. `cargo test` covers the SIM; it says
# nothing about the C# — a renamed field, a dead button, a screen that draws an empty list because
# nobody answers its query. Only these probes press the real controls and assert the sim moved.
#
# IT DRIVES THE OPEN EDITOR, and that is not a shortcut. `-batchmode` cannot attach to a project
# the Editor already holds, and the Editor is what holds the native plugin; a second instance is
# refused by the project lock. So the runner writes `.roi_cmd` (the same bridge a human uses) and
# reads the verdict back out of the Editor log.
#
# The VERDICT is computed in the Editor, not here — see `RemoteControl.ProbeWatch`. Each probe
# reports in its own vocabulary ("BAG done", "PRESS DONE", and four announce nothing at all), so a
# shell script that knew each of those would be a second list to keep in step with the probes.
#
# DO NOT EDIT C# WHILE IT RUNS. The Editor picks up a changed script on its own and recompiles,
# which unloads the assembly the probe in flight is running inside — the run then times out with
# nothing wrong with the probe. One run cost exactly that.
#
#   tools/unity_probes.sh                 # all of them
#   tools/unity_probes.sh BagProbe ApexProbe
#   tools/unity_probes.sh --list
set -uo pipefail
cd "$(dirname "$0")/.."

LOG="$HOME/Library/Logs/Unity/Editor.log"
CMD="unity/ROI/.roi_cmd"
TIMEOUT=${ROI_PROBE_TIMEOUT:-600}     # seconds per probe; the story one really does take minutes

# ORDER MATTERS at the front: login proves the session anchors before anything asks a screen to
# act on one. The rest are grouped by what they touch.
ALL=(
  LoginProbe OnlineProbe SteamAuthProbe RouteProbe
  UiPress ScreenProbe OverlayProbe ContractProbe
  BagProbe CarrierProbe AlbumProbe
  ShopProbe TradeProbe GearProbe SummonProbe
  SoundProbe NoticeProbe StoryProbe
  CommanderSkillProbe ZoneFadeProbe TelemetryProbe ProjectileProbe MarchProbe DevSaveProbe
)

if [ "${1:-}" = "--list" ]; then printf '%s\n' "${ALL[@]}"; exit 0; fi
PROBES=("$@"); [ $# -eq 0 ] && PROBES=("${ALL[@]}")

if ! pgrep -x Unity >/dev/null; then
  echo "✗ the Unity Editor is not running — open unity/ROI first (the probes need it)" >&2
  exit 2
fi
[ -f "$LOG" ] || { echo "✗ no Editor log at $LOG" >&2; exit 2; }

# The online probes talk to a LOCAL verify-server, never production. Say so up front rather than
# letting four probes fail with a network error apiece.
if ! curl -s -m 2 http://127.0.0.1:8787/health >/dev/null 2>&1; then
  echo "!  nothing is listening on 127.0.0.1:8787 — the online probes will fail."
  echo "   start it against roi_dev (never roi_test, never prod):"
  echo "     tools/local_server.sh"
  echo
fi

# One refresh up front so a probe never runs against a stale compile. A compile ERROR breaks the
# bridge itself (Assembly-CSharp fails, so RemoteControl never registers), which is why the wait
# below reports it as "the Editor stopped answering" rather than hanging forever.
mark=$(wc -l < "$LOG")
echo refresh > "$CMD"
for _ in $(seq 1 40); do
  awk -v n="$mark" 'NR>n && /REMOTE cmd/' "$LOG" | grep -q . && break
  sleep 1
done

fails=(); passes=()
for probe in "${PROBES[@]}"; do
  mark=$(wc -l < "$LOG")
  printf '  %-14s ' "$probe"
  echo "probe:$probe" > "$CMD"
  line=""
  for _ in $(seq 1 "$TIMEOUT"); do
    line=$(awk -v n="$mark" 'NR>n && /^PROBE END /' "$LOG" | head -1)
    [ -n "$line" ] && break
    sleep 1
  done
  if [ -z "$line" ]; then
    echo "TIMEOUT after ${TIMEOUT}s (the Editor may have stopped answering — check for a compile error)"
    fails+=("$probe (timeout)")
    # Leave play mode so the next probe starts from a known state.
    echo stop > "$CMD"; sleep 3
    continue
  fi
  if [[ "$line" == *" ok" ]]; then
    echo "ok"
    passes+=("$probe")
  else
    echo "FAIL"
    echo "      ${line#PROBE END $probe fail — }"
    fails+=("$probe")
  fi
done

echo
if [ ${#fails[@]} -eq 0 ]; then
  echo "✓ ${#passes[@]}/${#PROBES[@]} probes passed"
  exit 0
fi
echo "✗ ${#fails[@]} failed: ${fails[*]}"
exit 1
