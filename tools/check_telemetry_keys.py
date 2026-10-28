#!/usr/bin/env python3
"""Guard the client/server telemetry contract. Exit 1 on violation.

Two failures, both silent, both exactly the failure `metrics.rs` was written to prevent:

1. **The client sends a name the server does not know.** It lands on `client.unknown` and that
   metric reads zero forever. Nothing logs, nothing 500s — the counter simply never moves, which
   is indistinguishable from a feature nobody uses.
2. **The server allows a name no client sends.** Same reading, opposite cause: a flat zero that
   looks like a dead system. This one cannot be a build failure, because a name is usually added
   before its emitter — so it is required to say so in the list, with a `// UNWIRED` marker. Then
   every zero on the dashboard is explainable from the source rather than from memory.

Runs in well under a second. Wired into tools/smoke.sh beside check_loc_keys.py.
"""
import pathlib
import re
import sys

ROOT = pathlib.Path(__file__).resolve().parent.parent
METRICS = ROOT / "crates/verify-server/src/metrics.rs"
GODOT = ROOT / "godot"
UNITY = ROOT / "unity" / "ROI" / "Assets" / "Scripts"


def allowed_names() -> dict[str, bool]:
    """name -> is it marked UNWIRED."""
    src = METRICS.read_text()
    m = re.search(r"pub const ALLOWED: &\[&str\] = &\[(.*?)\n    \];", src, re.S)
    if not m:
        print("could not find ALLOWED in metrics.rs — the guard would pass vacuously",
              file=sys.stderr)
        sys.exit(1)
    out: dict[str, bool] = {}
    for line in m.group(1).splitlines():
        hit = re.match(r'\s*"([^"]+)",\s*(//\s*UNWIRED)?', line)
        if hit:
            out[hit.group(1)] = bool(hit.group(2))
    return out


def emitted_names(allowed: dict[str, bool]) -> dict[str, list[str]]:
    """name -> the CLIENT files containing it as a literal — both clients.

    DELIBERATELY A LITERAL SCAN, not a scan of `note(...)` call sites. A first attempt matched the
    argument of `note(` and missed two shapes already in the tree: `d.note("fx.off" if on else
    "fx.on")` puts the second name in a ternary, and `telemetry.gd` dispatches the fps buckets out
    of a table, so `note(String(b[1]))` has no literal in it at all. A stricter pattern would have
    reported both as unwired and been wrong.

    What this proves is that the name EXISTS in the client, not that a code path reaches it. That
    is the honest limit and it is the right trade: the failure this guards against is a name that
    does not exist on the other side, which a literal scan catches exactly.
    """
    # BOTH CLIENTS. The Unity port emits the same names through its own `Telemetry.Note`, and
    # while this guard only read `.gd` it reported those as unwired — a marker that says "no client
    # sends this" when one does is worse than no marker, because it is believed.
    sources = sorted(GODOT.rglob("*.gd")) + sorted(UNITY.rglob("*.cs"))

    out: dict[str, list[str]] = {}
    for src in sources:
        text = src.read_text(errors="ignore")
        for name in allowed:
            if f'"{name}"' in text:
                out.setdefault(name, []).append(src.relative_to(ROOT).as_posix())
    # A name the client sends that the SERVER does not allow cannot be found by asking about
    # names the server allows — so the call sites are scanned separately for that direction.
    for src in sources:
        text = src.read_text(errors="ignore")
        # `note(` in GDScript, but `Telemetry.Note(` in C#. The bare `Note(` was tried first and
        # is wrong: three screens have a status-line helper of that name, so it reported
        # "Nobody is benched." as an unknown counter. The C# side has to be QUALIFIED.
        call_pat = r'\bnote\(([^)\n]*)' if src.suffix == ".gd" else r'\bTelemetry\.Note\(([^)\n]*)'
        for call in re.findall(call_pat, text):
            for name in re.findall(r'"([^"]*)"', call):
                out.setdefault(name, []).append(src.relative_to(ROOT).as_posix())
        for name in re.findall(r'^\s*telemetry_key\s*=\s*"([^"]*)"', text, re.M):
            out.setdefault(name, []).append(src.relative_to(ROOT).as_posix())
    return out


def main() -> int:
    allowed = allowed_names()
    if not allowed:
        print("ALLOWED parsed as empty — the guard would pass vacuously", file=sys.stderr)
        return 1
    emitted = emitted_names(allowed)
    problems: list[str] = []

    for name, files in sorted(emitted.items()):
        if name not in allowed:
            problems.append(
                f'{files[0]} sends "{name}" — not in metrics::client::ALLOWED, so it would '
                f"count as client.unknown forever")

    for name, unwired in sorted(allowed.items()):
        if name in emitted and unwired:
            problems.append(
                f'"{name}" is marked // UNWIRED but {emitted[name][0]} sends it — '
                f"drop the marker or the list lies about which zeros are expected")
        if name not in emitted and not unwired:
            problems.append(
                f'"{name}" is allowed but nothing in godot/ sends it. Wire it, or mark it '
                f"`// UNWIRED` so a zero on the dashboard is explainable")

    for p in problems:
        print(f"TELEMETRY ERROR: {p}", file=sys.stderr)
    if problems:
        print(f"\n{len(problems)} problem(s) in the telemetry contract.", file=sys.stderr)
        return 1
    wired = sum(1 for n, u in allowed.items() if not u)
    print(f"telemetry contract ok — {len(allowed)} names allowed, {wired} wired, "
          f"{len(allowed) - wired} declared unwired")
    return 0


if __name__ == "__main__":
    sys.exit(main())
