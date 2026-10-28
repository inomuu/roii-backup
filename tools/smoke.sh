#!/usr/bin/env bash
# smoke.sh - end-to-end "is it broken?" check (macOS / Linux).
#
# POSIX twin of smoke.ps1. Same four steps:
#   1. cargo test --workspace         (sim-core unit + golden tests)
#   2. cargo build -p godot-ext       (bridge cdylib)
#   3. copy the built library into godot/bin/ under Godot's expected name
#      (libgodot_ext.dylib on macOS, libgodot_ext.so on Linux)
#   4. godot --headless --path godot --quit-after N
#      (catches GDScript parse errors, scene wiring, runtime errors from
#       at least the first N frames of sim ticks)
#
# Unlike Windows, a freshly-cloned project has no .godot/ cache, so the
# GDExtension is not yet registered and SimBridge resolves as an unknown
# type. We detect that (missing .godot/extension_list.cfg) and run one
# editor import pass first.
#
# Exits non-zero on any failure so this can wire into CI / pre-commit.
#
# Usage:
#   tools/smoke.sh                 # full run, 5 sim-seconds of headless
#   tools/smoke.sh --skip-godot    # rust only
#   tools/smoke.sh --godot-only    # GODOT only — for a pure .gd change, where cargo test /
#                                  # build / copy have nothing to do with the edit. The headless
#                                  # run is the only step that catches a GDScript RUNTIME error
#                                  # (--check-only catches parse errors, not runtime ones).
#   tools/smoke.sh --godot /path/to/godot
#   tools/smoke.sh --seconds 10
set -uo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$REPO_ROOT"

SKIP_GODOT=0
GODOT_ONLY=0
GODOT_BIN=""
RUN_SECONDS=5

while [ $# -gt 0 ]; do
    case "$1" in
        --skip-godot) SKIP_GODOT=1; shift ;;
        --godot-only) GODOT_ONLY=1; shift ;;
        --godot) GODOT_BIN="$2"; shift 2 ;;
        --seconds) RUN_SECONDS="$2"; shift 2 ;;
        *) echo "unknown arg: $1" >&2; exit 2 ;;
    esac
done

# Colors (fall back to empty when not a tty).
if [ -t 1 ]; then
    C_CYAN=$'\033[36m'; C_GREEN=$'\033[32m'; C_RED=$'\033[31m'
    C_YELLOW=$'\033[33m'; C_GRAY=$'\033[90m'; C_RESET=$'\033[0m'
else
    C_CYAN=""; C_GREEN=""; C_RED=""; C_YELLOW=""; C_GRAY=""; C_RESET=""
fi

section() { printf '\n%s==== %s ====%s\n' "$C_CYAN" "$1" "$C_RESET"; }
pass()    { printf '%s[PASS]%s %s\n' "$C_GREEN" "$C_RESET" "$1"; }
fail()    { printf '%s[FAIL]%s %s\n' "$C_RED" "$C_RESET" "$1"; }

ALL_PASS=1
PG_RAN=unknown   # money-path SQL tests: ran / skipped

# Make cargo reachable even when invoked from a non-login shell.
if ! command -v cargo >/dev/null 2>&1; then
    # shellcheck disable=SC1091
    [ -f "$HOME/.cargo/env" ] && . "$HOME/.cargo/env"
fi
if ! command -v cargo >/dev/null 2>&1; then
    echo "cargo not found - install rustup first." >&2
    exit 2
fi

# --- platform-specific library name --------------------------------------
case "$(uname -s)" in
    Darwin) LIB_SRC="libgodot_ext.dylib"; LIB_DST="libgodot_ext.dylib" ;;
    Linux)  LIB_SRC="libgodot_ext.so";    LIB_DST="libgodot_ext.so" ;;
    *)      echo "unsupported OS $(uname -s); use smoke.ps1 on Windows" >&2; exit 2 ;;
esac

find_godot() {
    if [ -n "$GODOT_BIN" ] && [ -x "$GODOT_BIN" ]; then echo "$GODOT_BIN"; return; fi
    command -v godot 2>/dev/null && return
    for c in \
        "/Applications/Godot.app/Contents/MacOS/Godot" \
        "$HOME/Applications/Godot.app/Contents/MacOS/Godot"; do
        [ -x "$c" ] && { echo "$c"; return; }
    done
}

# --- 1. cargo test -------------------------------------------------------
if [ "$GODOT_ONLY" -eq 0 ]; then
section "0. loc key guard"
if python3 tools/check_loc_keys.py; then pass "loc keys"; else fail "loc keys (missing from strings.csv or legacy 2-arg Loc.ui)"; ALL_PASS=0; fi
if python3 tools/check_vfx_tables.py >/dev/null; then pass "vfx tables"; else fail "vfx tables (missing scene, unknown skill/element/paint, or a colour past the glow threshold)"; ALL_PASS=0; fi
if python3 tools/check_telemetry_keys.py >/dev/null; then pass "telemetry keys"; else fail "telemetry keys (a client name the server rejects, or an allowed name nothing sends and nothing marks // UNWIRED)"; ALL_PASS=0; fi

section "1. cargo test --workspace"
# Status captured BEFORE the grep (audit 2026-10-02): `${PIPESTATUS[0]}` after an `if … then :; fi`
# held the status of `:`, so a failing run (which always prints a "test result" line) passed.
TEST_OUT="$(cargo test --workspace 2>&1)"; TEST_RC=$?
echo "$TEST_OUT" | grep -E "test result:|error\[|panicked" || true
if [ "$TEST_RC" -eq 0 ]; then pass "cargo test"; else fail "cargo test"; ALL_PASS=0; fi

# --- 1a. the money-path SQL tests ----------------------------------------
# `cargo test --workspace` above ALREADY runs these — but they SKIP when no Postgres is reachable
# and still report ok, which is the "green because UNTESTED" shape this file exists to prevent.
# They are the only coverage the market/wallet/gacha SQL has: everything else about those paths is
# type-checked, so a wrong column or a bad ON CONFLICT target compiles and fails in production.
# So say out loud which of the two happened, and refuse to call the whole run a PASS when they did
# not run.
section "1a. pg_ money-path tests (Postgres)"
# `--nocapture` is REQUIRED, not tidiness: the harness swallows a passing test's stderr, and a
# skipped pg test PASSES. Without it the "[pg] SKIPPED" line never appears and this step reports
# the skip as a clean run — the exact thing it was added to stop.
PG_OUT="$(cargo test -q -p verify-server pg_ -- --nocapture 2>&1 || true)"
if echo "$PG_OUT" | grep -q "\[pg\] SKIPPED"; then
    PG_RAN=skipped
    printf '%s  ! SKIPPED — no local Postgres. The money-path SQL was NOT tested.%s\n' "$C_RED" "$C_RESET"
    echo "$PG_OUT" | grep "^\[pg\]" | head -4
elif echo "$PG_OUT" | grep -qE "^test result: ok"; then
    PG_RAN=ran
    pass "pg_ tests ($(echo "$PG_OUT" | grep -oE '[0-9]+ passed' | head -1))"
else
    PG_RAN=failed
    fail "pg_ tests"; ALL_PASS=0
    echo "$PG_OUT" | grep -E "test result:|panicked at|^\[pg\]" | head -6
fi

# --- 1b. determinism lints -----------------------------------------------
# The sim's invariants (no floats, no HashMap, no clock) are DENY lints, and a deny lint only
# fires under clippy — `cargo build` and `cargo test` skip it as an unknown attribute. So until
# this step existed the flagship rule of the codebase was checked by nobody.
section "1b. cargo clippy -p sim-core (determinism invariants)"
# --lib only: the test target costs the whole golden suite to compile, and test code may use a
# HashMap for its own bookkeeping without any of it reaching the sim.
# On clippy's EXIT CODE (audit 2026-10-02): under pipefail, `clippy | grep` took clippy's failure
# status into the else branch, so a broken lint reported PASS.
if cargo clippy -q -p sim-core --lib; then
    pass "clippy invariants"
else
    fail "clippy: a determinism invariant was broken (float / HashMap / clock)"; ALL_PASS=0
fi

# --- 2-3. Godot library: only when Godot runs (Godot is cancelled — CLAUDE.md: no dylib staging)
if [ "$SKIP_GODOT" -eq 0 ]; then
# --- 2. cargo build ------------------------------------------------------
section "2. cargo build -p godot-ext"
if cargo build -p godot-ext; then pass "cargo build"; else fail "cargo build"; ALL_PASS=0; fi

# --- 3. copy library -----------------------------------------------------
section "3. copy $LIB_DST"
SRC="$REPO_ROOT/target/debug/$LIB_SRC"
DST="$REPO_ROOT/godot/bin/$LIB_DST"
if [ -f "$SRC" ]; then
    mkdir -p "$REPO_ROOT/godot/bin"
    # REPLACE, never overwrite in place. `cp` onto the existing path keeps the inode, and macOS
    # keeps the code-signature page hashes already validated for it — dyld then sees pages whose
    # hashes do not match and the kernel SIGKILLs the game inside `dlopen`, before any scene
    # loads. It looks exactly like a crash in whatever was edited last. See tools/play.sh.
    rm -f "$DST"
    cp "$SRC" "$DST"
    [ "$(uname -s)" = "Darwin" ] && codesign -f -s - "$DST" >/dev/null 2>&1
    printf '%s       %s  (%s)%s\n' "$C_GRAY" "godot/bin/$LIB_DST" "$(du -h "$DST" | cut -f1)" "$C_RESET"
    pass "copy library"
else
    fail "copy library (no source at $SRC)"; ALL_PASS=0
fi
fi   # SKIP_GODOT

fi   # GODOT_ONLY

# --- 4. godot --headless -------------------------------------------------
section "4. godot --headless"
if [ "$SKIP_GODOT" -eq 1 ]; then
    printf '%s       SKIPPED (--skip-godot)%s\n' "$C_GRAY" "$C_RESET"
else
    GODOT_EXE="$(find_godot)"
    if [ -z "$GODOT_EXE" ]; then
        printf '%s       no Godot binary found; pass --godot or --skip-godot%s\n' "$C_GRAY" "$C_RESET"
    else
        printf '%s       binary: %s%s\n' "$C_GRAY" "$GODOT_EXE" "$C_RESET"
        # First-clone bootstrap: register the GDExtension so SimBridge resolves.
        if [ ! -f "$REPO_ROOT/godot/.godot/extension_list.cfg" ]; then
            printf '%s       no .godot cache - running one editor import pass%s\n' "$C_GRAY" "$C_RESET"
            "$GODOT_EXE" --headless --editor --path godot --quit >/dev/null 2>&1 || true
        fi
        FRAMES=$(( RUN_SECONDS * 60 ))
        OUT="$("$GODOT_EXE" --headless --path godot --quit-after "$FRAMES" 2>&1)"
        echo "$OUT" | grep -E "BETA|ALPHA|ERROR|Parse Error|Initialize godot-rust" | sed 's/^/       /'
        if echo "$OUT" | grep -qE "SCRIPT ERROR|Parse Error"; then
            fail "godot smoke run (script errors above)"; ALL_PASS=0
        else
            pass "godot smoke run"
        fi
    fi
fi

echo ""
if [ "$ALL_PASS" -eq 1 ] && [ "$PG_RAN" = "skipped" ]; then
    # Deliberately NOT "PASS". Everything that ran was green, but the money path was not exercised
    # at all, and a run that says PASS is read as "the money path is fine". Exit 0 — this is a
    # legitimate state on a machine without Postgres, it just may not claim more than it did.
    printf '%s==== smoke: PARTIAL (pg_ skipped — money-path SQL untested) ====%s\n' "$C_YELLOW" "$C_RESET"
    printf '  start Postgres and re-run to turn this into a PASS.\n'
    exit 0
elif [ "$ALL_PASS" -eq 1 ]; then
    printf '%s==== smoke: PASS ====%s\n' "$C_GREEN" "$C_RESET"; exit 0
else
    printf '%s==== smoke: FAIL ====%s\n' "$C_RED" "$C_RESET"; exit 1
fi
