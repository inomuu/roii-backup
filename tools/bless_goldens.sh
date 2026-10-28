#!/usr/bin/env bash
# bless_goldens.sh — re-pin every golden's expected hash to the CURRENT
# sim output, in one command, instead of hand-editing each test file.
#
# Goldens exist to catch UNINTENDED determinism changes. Blessing them is
# you asserting "this change was intentional" — so eyeball the diff first
# (run `cargo test -p sim-core --test suite` and read what moved), THEN bless.
# Day-to-day, iterate against `cargo test -p sim-core --lib` (unit tests
# only — the golden integration tests don't run), and bless at the end.
#
# Each tests/suite/golden_*.rs AND tests/suite/diag_*.rs holds exactly one 64-hex pinned hash
# (`*_EXPECTED_HASH` / `ARM64_HASH`). The diags were added to this sweep after being
# re-pinned by hand twice (0.0.243, 0.0.245): they pin a hash exactly like a golden,
# so leaving them out just moved the work onto someone remembering.
# We run the test; on a hash-mismatch failure the panic prints
# `left: "<actual>"`; we splice that back into the file.
#
# Usage:
#   tools/bless_goldens.sh            # bless all goldens
#   tools/bless_goldens.sh beta9      # bless only golden_beta9
#   tools/bless_goldens.sh diag       # bless only the diag_* pins
set -uo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$REPO_ROOT"
command -v cargo >/dev/null 2>&1 || { [ -f "$HOME/.cargo/env" ] && . "$HOME/.cargo/env"; }

# sed -i needs an arg on BSD/macOS, none on GNU/Linux.
if sed --version >/dev/null 2>&1; then SED_INPLACE=(-i); else SED_INPLACE=(-i ''); fi

filter="${1:-}"
changed=0
for f in crates/sim-core/tests/suite/golden_*.rs crates/sim-core/tests/suite/diag_*.rs; do
    name="$(basename "$f" .rs)"          # e.g. golden_beta9 / diag_band4
    [ -n "$filter" ] && [[ "$name" != *"$filter"* ]] && continue

    # One binary now (tests/suite/main.rs): select a scenario by NAME-filter, not by target.
    out="$(cargo test -p sim-core --test suite "$name" 2>&1)"
    actual="$(printf '%s' "$out" | grep -oE 'left: "[0-9a-f]{64}"' | head -1 | grep -oE '[0-9a-f]{64}')"
    if [ -z "$actual" ]; then
        echo "  ok   $name (unchanged)"
        continue
    fi
    old="$(grep -oE '"[0-9a-f]{64}"' "$f" | head -1 | tr -d '"')"
    if [ "$old" = "$actual" ]; then
        echo "  ok   $name"
        continue
    fi
    sed "${SED_INPLACE[@]}" "s/$old/$actual/" "$f"
    echo "  BLESS $name"
    echo "         $old"
    echo "      -> $actual"
    changed=$((changed + 1))
done

echo ""
echo "re-pinned $changed pin(s); verifying…"
cargo test -p sim-core --tests 2>&1 | grep -E "test result:|FAILED" | tail -20
