# AGENTS.md

**Read `CLAUDE.md` in this same directory. It is the only copy of these instructions.**

Everything that used to live here — what this project is, the four-crate + Godot architecture,
the sim-core determinism invariants, the commands, the golden-test workflow, the Godot
presentation-only rules — is in `CLAUDE.md`, and it applies to every agent working in this repo,
not just Claude Code.

## Why this file is a pointer and not a copy

It *was* a copy, forked 2026-07-30, and it silently rotted within two weeks. By 2026-08-13 the
two files disagreed on facts an agent acts on:

- it pointed at `crates/sim-core/tests/golden_*.rs`, a path that no longer exists — the 17 test
  crates were merged into one binary under `crates/sim-core/tests/suite/`;
- it told you to run one scenario with `cargo test -p sim-core --test golden_beta9`, which now
  fails outright with `error: no test target named golden_beta9`. The command is
  `cargo test -p sim-core --test suite golden_beta9` — a NAME filter, not a target.

Nothing flagged either one. Duplicated instructions have no test, so the stale copy stays
confident and wrong, and an agent reading it burns a cycle discovering the repo moved.

So: do not restore the content here. If a rule needs to change, change it in `CLAUDE.md`.
