# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## What this is

**ROI: Reign of Idlers** (repo `minimal-idle-mmo`, old working title `taskbar-arpg`) — an idle
ARPG inspired by Ragnarok Online: an auto-fighting party with build-making depth.
The simulation is a deterministic, integer-only Rust core; the client is a pure
read-only renderer on top of it.

### THE CLIENT IS UNITY — GODOT IS CANCELLED (user, 2026-09-19)

`unity/ROI/` driven through `crates/sim-ffi` is the only client. The `godot/` tree stays in the
repo as history but gets **no work**: no `tools/play.sh`, no `libgodot_ext.dylib` staging, no
Godot-side fixes when the view JSON changes, no parity. `tools/check_online_sync.sh` checks Unity's
`sim-ffi` dylib against the server (2026-09-29; `ROI_CLIENT=godot` keeps the old check).
`godot/i18n/strings.csv` is still the loc SOURCE that `tools/unity_data.sh` copies into
`unity/ROI/Assets/StreamingAssets/` — keep editing it there. Do not delete `godot/` without asking. Direction is semi-MMO / online-required: the client runs the
sim, a verify server replays saves to check them (client-sim + server-verify, NOT
server-authoritative — see `docs/server-plan-phases.md`, `docs/sim-authority-tradeoffs.md`).

### THERE IS NO OFFLINE VERSION (user, repeatedly — 2026-07-03, 2026-08-02)

The shipped game requires a login and a server. **Never design around, ask about, or hedge for
an offline build.** This gets proposed wrongly often enough that it is pinned here rather than
three docs deep.

- The GDD's pillar #1 "fun offline first" was **Phase-0 prototype scope only**. Anywhere a doc
  still argues from it (`docs/anti-cheat-and-server-model.md`, GDD §9) is stale, not policy.
- The Unity dev save slots (`DevSaveStore`, and the old Godot `sim_driver_core.gd::_begin_offline`)
  are a **dev/test path**, not a shipping mode. Do not add features for it, and do not weaken a
  server-authored mechanic so it can work there — gate the feature off instead (see
  the old `craft_view`, which refuses server-rolled recipes when not online).
- Don't confuse this with **offline PROGRESSION** (earning while the game is closed). SETTLED
  (user 2026-10-02): there IS offline progression — EXP only, 80% of the online rate at the latest
  band reached, ≤10 h per gap, recharging every time the player is back
  (`sim_core::tick::offline_xp_per_member`, verify-server `pay_offline_at_login`). It is paid AT
  THE LOGIN (`GET /save`), in one transaction, for the time since max(latest save, last payout).
  `docs/backend-arch-reconcile.md`'s "no offline earnings" is superseded.
- The client is MEANT to keep simulating through a network blip only briefly: `SPAN_STALL_TICKS`
  (~5 min unsaved) freezes the sim until a save lands. The Godot client did this; the Unity client
  does NOT yet (handoff `docs/unity-onboarding-gear-store-handoff.md` §16.1) — until it does, a
  long outage makes every later save fail its span limit (~83 min) and the server must not assume
  the stall.

The single source of truth for design + technical rules is `docs/taskbar-arpg-gdd-phase0.md`.
`docs/combat-formulas.md` and `docs/skills-catalog.md` document the live formulas/skills.

`TODO.md` (repo root) is the other half: the GDD says what the game IS, `TODO.md` says what is
BUILT and what is next — "Road to 1st Demo", priority-tagged P0…P3, with the reasoning for each
deferral inline. Read the relevant block BEFORE implementing or tuning anything; several entries
carry a gate ("do this only after X ships") that is not inferable from the code.

**Note `README.md` is stale** — it still describes Phase-0 offline MVP, `sim-core 0.0.11-alpha`,
and a Windows-msvc toolchain. Where it and this file disagree, this file wins.

## Architecture

Six Rust crates (workspace) + the Unity client (`unity/ROI/`). The hard boundary between sim
and renderer is the defining feature of this codebase. Data flow:
`sim-core` ← `sim-host` (shared session + view JSON) ← `sim-ffi` (C ABI) ← Unity `SimFfi.cs` / `SimDriver.cs`.

- **`crates/sim-core`** — the entire game simulation. Deterministic, integer-only, no I/O.
  Owns `GameState`, `tick()`, `apply_input()`, the data catalog, combat, skills, drops, refine.
- **`crates/sim-host`** — the renderer-agnostic half of every bridge, `forbid(unsafe_code)`.
  `view_state` / `item_view` are the ONE read contract the client draws from (key-set pin test
  lives there); `session` is the start / load / tick / apply-input / export-bundle / re-anchor
  lifecycle. A change to what the client sees usually belongs here, not in `sim-ffi`.
- **`crates/sim-ffi`** — the C-ABI `cdylib` Unity P/Invokes (`roi_*` symbols), a thin wrapper over
  `sim_host::session::Session`. **JSON is the only type that crosses.** Every function is total
  (null/malformed in → null/0 out, never unwinds). Every returned `char*` is caller-owned and must
  be freed with `roi_string_free` — in C# declare it `IntPtr`, never `string` (the marshaller
  would free a Rust allocation → silent heap corruption). Ship RELEASE only (`panic = "abort"`).
  Host rules: `docs/sim-ffi-unity-gap.md`.
- **`crates/godot-ext`** — the old gdext bridge (`SimBridge`) over the same `sim-host`.
  Historical; still compiles in the workspace, gets no feature work.
- **`crates/sim-cli`** — headless balance/determinism runner. Loads `data/game_data.json`,
  auto-plays a party (`build.rs`: allocates stats, learns skills, equips gear, recruits), runs
  N ticks, emits a CSV progression curve. This is how balance is tuned without opening a client.
- **`crates/verify-server`** — the online backend (axum + tokio, Postgres via sqlx, deployed on
  Fly.io at `renrei-idle-server.fly.dev`, DB on Neon). Links `sim-core` directly and verifies
  uploaded saves by REPLAY (seed + inputs must reproduce the claimed hash). Owns everything the
  client must not be trusted with: Steam ticket auth, session pinning, server-rolled rare drops
  and refine, the economy/market, chat, mailbox. `docs/economy-invariants.md` is its safety
  spine.
- **`unity/ROI/`** — the client. Pure presentation, C# under `Assets/Scripts/` (`Sim/`, `Net/`,
  `Arena/`, `UI/`, `Story/`, `Cutin/`, …). `Sim/SimDriver.cs` is the ONE owner of the sim handle:
  it ticks at a fixed 10 Hz on an unscaled-time accumulator, publishes view JSON via the
  `StateUpdated` event and save bundles via `SaveRequested`. Views subscribe to events and never
  touch the handle or compute gameplay outcomes. `Net/` holds `ServerClient` / `OnlineService` /
  `SteamAuth` for the verify server. Editor-side probes live in `Assets/Editor/*Probe.cs`.
  Many C# files are ports of a named Godot original and cite it (`sim_driver_core.gd::…`) — the
  `.gd` is still the reference for behaviour that was tuned there.
- **`data/game_data.json`** — hand-written catalog (items / monsters / zones / units / crests).
  Loaded once into `GameData`, lives OUTSIDE `GameState`, passed by reference. Unity reads a COPY
  in `Assets/StreamingAssets/` — re-run `tools/unity_data.sh` after editing it (a stale copy fails
  the server's version gate).

### sim-core invariants (NON-NEGOTIABLE)

These are enforced by `#![forbid(unsafe_code)]` + `#![deny(clippy::float_arithmetic)]` and by
the golden tests. Violating them breaks determinism and cross-platform save replay:

- **No floats anywhere.** All math is `i64`/`u64`. Percentages are basis points (10000 = 100%).
- **No I/O, no threads, no wall-clock, no `std::collections::HashMap` iteration order** in
  logic. Iterate entities in spawn order, not hash order. Use `BTreeMap` where a map is needed.
- **One PCG32 PRNG**, owned by `GameState`, seeded at new-game. Every random outcome takes a
  draw in a fixed order — changing draw order changes every downstream result. When adding a
  mechanic, preserve the existing draw order (e.g. `roll_drop` always takes its draw even when
  the result is the inert `nothing` item).
- **`tick()` and `apply_input()` are pure w.r.t. `GameState`.** The save file is conceptually
  `(seed + initial state + Vec<InputEvent>)` and must replay bit-identically (GDD §4.4).
  `apply_input` for invalid input is a silent no-op — NEVER panic (replay must survive
  game_data version skew).
- **`sim_core::VERSION`** (in `lib.rs`) must be bumped whenever the serialized shape of
  `GameState` changes. The long version changelog in `lib.rs` is the canonical history of every
  schema/balance change — read the tail of it to understand recent work.

### What a character is (2026-09-19)

- A **unit** (`Entity.unit_def`: seraphine / lapis / lucielle / aveline / raplus, …; ids `rapis` / `nyx` until 0.0.492) with a fixed kit
  (`skills::unit_kit_tree`), a **Doctrine** (`potential::doctrine_for_unit`: Sentinel / Striker /
  Destroyer / Catalyst / Support), an ATTACK element, and an Exemplar overlay carrying stars 0-6
  (Resonance ranks S1-S6 in `unit_rank.rs`). **The class system and MATK are gone** (0.0.445 /
  catalog 0.0.188: no class stat blocks, no class skills or trees, no `ChangeClass`,
  no `class_characters`) — never key new code on `*_t1` / `*_t2` ids or a magic-attack stat. What
  is left: the Godot / Unity renderers still name their sprite sheets by the old class ids
  (art keys, not gameplay), and engine hooks for deleted skills (traps, pets, spirit strike,
  purificatio, …) sit inert in `tick/` until a later sweep. One ATK number on items, entities,
  buffs and the host.
- **Elements**: six, and ONLY six (Earth / Wind / Poison / Spirit are cancelled, user
  2026-09-23 — dead enum variants + a few legacy cards; never propose one) — Physical (serde
  alias `Neutral`, 0.0.461), Fire, Lightning (Thunder), Ice (serde alias `Water`, 0.0.475), Light,
  Dark (serde reads the old `Holy` / `Shadow` spellings; 0.0.458). A character has an attack element only (never weak to anything); a **monster has
  no element**, only `weaknesses` / `resists` lists (`docs/element-weakness-system.md`).
- Per-character (`CharacterData`): stats, equipped gear, skills, Regalia. Party-shared: `stash`,
  `gil`, `soul`, `soul_paths`, Training, Crests, party slots.
- Game-over fires only on a full party WIPE, not a single member death; a fight nobody can end is
  felled by the stalemate breaker (`tick::break_stalemate`).

### Tuning workflow

`target/release/sim-cli --bench 70:6:legendary:168 --regalia sig:60:0 --stars N` reads a standard
party's damage in ~4 s (MEMBERS table + per-skill breakdown + `BENCH` line); `--lead X --recruit
"" --dummy` reads one unit's raw output against training dummies. Change one thing, re-run. A 7-day
`--ticks 6048000` run is for progression and wipes only (20 min) — do not reach for it to compare
numbers. Reference tables: `docs/stage-ladder-renovation.md` §8-9.

## Commands

```bash
# Iterate fast against unit tests ONLY (golden integration tests don't run here):
cargo test -p sim-core --lib

# Full check incl. golden determinism tests:
cargo test -p sim-core --tests        # unit + 11 goldens
cargo test --workspace

# Run ONE test by name:
cargo test -p sim-core --lib <test_name>
cargo test -p sim-core --test suite golden_beta9   # one scenario (all live in ONE binary)

# Headless balance run (tune pacing without a client) — emits a CSV curve:
cargo run -p sim-cli -- --seed 42 --ticks 6000 --sample 50
#   1 tick = 100 ms (10 ticks/s); 6000 ticks = 10 min. --help lists all flags.
#   Pacing numbers only count with the REAL unit party (see "Tuning workflow" above).

# Rust-only "is it broken?" check (Godot half is dead — always skip it):
tools/smoke.sh --skip-godot

# Unity client — after editing sim-core / sim-host / sim-ffi:
tools/unity_dylib.sh           # build sim-ffi (release) → rm + cp + codesign into
                               # Assets/Plugins/macOS/. NEVER plain `cp` over a loaded dylib
                               # (dies at load, empty log). The Editor holds the plugin for
                               # the whole session: new dylib ⇒ RESTART the Unity Editor.
tools/unity_data.sh            # copy game_data.json / commander_rewards.json / strings.csv /
                               # audio / story files into StreamingAssets — run after editing any
tools/unity_probes.sh          # the client's only test net: drives the OPEN Editor via
tools/unity_probes.sh BagProbe # unity/ROI/.roi_cmd, reads verdicts from Editor.log. --list shows
                               # all. Do NOT edit C# while it runs (recompile kills the probe).

# verify-server (its 128 tests are NOT in the golden suite — `cargo test -p sim-core` misses them):
cargo test -p verify-server
cargo clippy -p verify-server --all-targets   # the AVAILABILITY lints (crates/verify-server/
                               # clippy.toml: no `.unwrap()`, no raw `.lock()`, no slice indexing).
                               # These fire ONLY under clippy; `cargo test` never runs them.
                               # Release sets `panic = "abort"` — a panic here is an outage, not a
                               # failed request. tools/deploy.sh runs this; nothing else does.

# Roster reference (stats + kits + ladders + Regalia), GENERATED from the code — never hand-edit:
target/release/sim-cli --roster-web docs/roster && open docs/roster/index.html

# i18n key guard (exit 1 on violation) — every literal key in C# Loc.Get / Loc.GetOr / Loc.Fmt
# (and legacy .gd Loc.ui) must exist in godot/i18n/strings.csv. A missing key renders as the raw KEY:
python3 tools/check_loc_keys.py

# Online stack:
tools/deploy.sh                # ONLY way to deploy verify-server (source-identity: refuses a
                               # dirty tree, stamps git SHA, verifies the server reports it)
tools/check_online_sync.sh     # pre-flight before an online play test: client vs deployed server
                               # sim-version skew silently rejects every online save
tools/release_check.sh <build> # RELEASE GATE: run on a built Unity player before a Steam upload —
                               # blocks a -dirty / unstamped sim library or one that is not the
                               # prod server's exact SHA + catalog (else every save is FLAGGED)
tools/local_server.sh          # local verify-server → roi_dev DB. NEVER point it at roi_test:
                               # `cargo test -p verify-server` TRUNCATEs roi_test
```

### Golden tests

`crates/sim-core/tests/suite/golden_*.rs` pin a state hash each (`golden_roster_a/b` field the REAL five-unit party — add a kit, rank, Regalia or mark change and expect them to move). When an INTENTIONAL change
moves one, confirm the drift is expected, then re-pin with `tools/bless_goldens.sh` — blessing
is you asserting the change was deliberate, and `lib.rs`'s changelog records which moved and why.
Full workflow in `docs/testing-notes.md`.

### What a green test run does NOT prove

`cargo test` covers the SIM only. It says nothing about the Unity C# (only
`tools/unity_probes.sh` touches it — a renamed view-JSON key renders empty instead of
erroring), the FFI boundary, performance, cross-platform
determinism, the online replay path, real old saves, or process-level aborts.
**Read `docs/testing-notes.md` before trusting a green run — and before writing a test here
(two rules there were learned the hard way).**

## Client rules (Unity)

- **Visual effects must be fully switchable off (user, 2026-09-28).** This is an idle
  game: companion mode must minimize CPU/GPU work, not just reduce visual intensity.
  New HD-2D/post-processing/atmosphere experiments need a genuine off path; runtime
  integration must connect it to companion mode. Disable optional render passes and
  decorative updates/spawning, render only necessary views at a bounded cadence, and
  restore the user's graphics settings on exit. Never slow or pause SimDriver,
  progression, saving or networking to achieve this. Verify actual CPU/GPU work;
  a clean-looking screenshot is not evidence of low power. Implementation gaps and
  acceptance checks: `docs/unity-scene1-post-processing-study.md`.

- **VFX carrier meshes are static after spawn (user, 2026-09-25).** Place and orient
  the carrier ONCE, then keep its world position, rotation, scale and geometry fixed
  for the effect lifetime. Do not animate its Transform, move it via mesh-particle
  velocity, inherit motion from a moving parent, or deform vertices to fake slash
  travel. The effect must flow THROUGH the stationary mesh using UV sampling,
  textures, shader masks, independently moving head/tail windows and dissolution.
  Vary trail length, width/brightness coverage and timing in the shader, not carrier
  geometry. Verify a fixed world transform AND rendered head travel / changing trail
  length; a moving bright object is not proof of UV flow. Legacy rotating/moving
  studies are not templates for new work; migrate them explicitly, not by silently
  changing previously accepted effects during an unrelated task.

- **Sim is authoritative; views are read-only.** One `SimDriver` owns the one sim handle. Views
  read from its events and mutate only through its methods; C# never computes a gameplay outcome.
  If a screen needs a number the view JSON lacks, add it to `sim-host`'s view, don't derive it.
- **No offline shipping mode.** `DevSaveStore` / `LocalSlotStore` are local DEBUG save slots
  (separate from cloud saves), not a product feature — gate server-authored features off there,
  never weaken them.
- **i18n**: no NEW user-facing string literals in C# — go through `Loc.Get` / `Loc.GetOr` /
  `Loc.Fmt` with keys in `godot/i18n/strings.csv` (still the single source; `unity_data.sh` copies
  it). sim-core stays locale-agnostic (ids + EN across the FFI). `docs/localization-plan.md`.
- Unity port status and per-screen handoffs: `docs/unity-port-plan.md`, `docs/unity-port-parity.md`,
  `docs/unity-*.md`.

The former "Godot rules", 2D-frozen and `proto/arena3d` VFX notes are historical — see git history
of this file or `docs/vfx-notes.md` if a Godot original must be read as a porting reference.
