# ROI: Reign of Idlers

*(repo `minimal-idle-mmo`; old working title `taskbar-arpg`)*

An idle ARPG inspired by Ragnarok Online — an auto-fighting party with build-making depth, a
risky refine system, and a desktop companion mode. **Online-required**: the client runs the
simulation, and a server replays every save to check it.

## Status — 2026-08-13

- **`sim_core::VERSION`:** `0.0.294-alpha` (the changelog in `crates/sim-core/src/lib.rs` is the
  canonical history of every schema and balance change — read its tail to see recent work)
- **Server:** live on Fly.io (`renrei-idle-server.fly.dev`), Postgres on Neon
- **Tests:** ~639 — 470 `sim-core` unit, 38 in the integration suite (11 of them goldens pinning
  a state hash), 128 `verify-server`, 3 `godot-ext`. See *What the tests don't cover* in
  `CLAUDE.md` before trusting a green run; the gaps are large and deliberate.
- **Next:** first demo — playable to Base Lv70 plus one job advancement. `TODO.md` is the
  priority-tagged plan and holds the current P0.

## The one idea worth knowing

The simulation is a **deterministic, integer-only Rust core** and Godot is a **pure read-only
renderer** on top of it. That boundary is the defining feature of this codebase and the reason
the rest of it is shaped the way it is:

- a save is conceptually `(seed + initial state + Vec<InputEvent>)`, so the server can re-run it
  and check the result rather than trusting the client;
- balance is tuned headlessly in `sim-cli` without opening the editor;
- no float, no I/O, no wall-clock and no hash-map iteration order may enter `sim-core`, because
  any of them would break replay across machines.

`CLAUDE.md` documents the invariants that protect this, and they are not negotiable. Read it
before changing anything in `crates/sim-core` or `godot/`.

## Layout

```
crates/
  sim-core/        the entire simulation — deterministic, integer-only, no I/O
  godot-ext/       thin gdext bridge; exposes ONE class (SimBridge); JSON-only FFI
  sim-cli/         headless balance/determinism runner → CSV progression curve
  verify-server/   axum backend: replay-verify, auth, economy, market, chat, mail
godot/             Godot 4.6 client (GL Compatibility). Presentation only.
  views/           one file per screen region or modal flow
  net/             server_client.gd (HTTP/WS) + steam_auth.gd
  tests/           five hand-run headless SceneTree selftests
  bin/             the compiled bridge dylib, copied from target/
data/game_data.json   hand-written catalog: items / monsters / zones / classes / skills
docs/              design docs; the GDD is the source of truth
tools/             build, deploy, balance and asset scripts
```

**Gotcha:** `godot/data/game_data.json` is a *copy* of `data/game_data.json` — a Godot export
cannot read outside the project directory. Edit the one in `data/`; the tooling syncs the copy.

## Toolchain

- Rust stable, 1.80+ (`rustup`) — developed on `aarch64-apple-darwin`
- Godot **4.6**
- Python 3 + Pillow, for the asset pipeline (`python-pptx` is needed only by the pitch-deck
  tooling under `docs/`)
- Optional, for shipping: `cargo-xwin` + LLVM + Godot export templates to cross-build Windows;
  `flyctl` to deploy the server

## Build & run

```bash
# Fast inner loop — unit tests only, ~1s. This is what you run per change.
cargo test -p sim-core --lib

# Full check, including the golden determinism tests.
cargo test --workspace

# Play-test after editing sim-core or godot-ext: builds the dylib, stages it, launches the GAME.
tools/play.sh

# End-to-end "is it broken?" — cargo test + clippy + build + copy + headless Godot.
tools/smoke.sh

# Tune balance without opening Godot — emits a CSV progression curve (1 tick = 100 ms).
cargo run -p sim-cli -- --seed 42 --ticks 6000 --sample 50
```

**Never hot-reload the bridge dylib in the Godot editor** — it double-inits and SIGABRTs, which
is why `reloadable=false` and why `tools/play.sh` launches the game directly rather than the
editor. A new dylib always means a full relaunch.

`CLAUDE.md` has the complete command list, including the golden-test blessing workflow, the
server deploy path (`tools/deploy.sh` is the only supported one) and the i18n key guard.

## Where to read next

| | |
|---|---|
| `CLAUDE.md` | **start here** — architecture, invariants, commands, and the rules that keep the sim/render split intact. Written for coding agents; equally the fastest orientation for a human. |
| `docs/taskbar-arpg-gdd-phase0.md` | the design + technical source of truth |
| `TODO.md` | what is built, what is next, and why each deferral was made |
| `docs/combat-formulas.md`, `docs/skills-catalog.md` | the live formulas and skills |
| `docs/server-plan-phases.md`, `docs/economy-invariants.md` | the online model and the economy's safety spine |

Instructions live in `CLAUDE.md` only — this file deliberately points at it instead of restating
it. `AGENTS.md` was a copy once; it silently drifted into telling agents to run commands that no
longer exist, so it is now a pointer too.
