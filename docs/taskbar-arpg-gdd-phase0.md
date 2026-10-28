# Taskbar Idle ARPG — Phase 0 GDD / Implementation Spec

**Audience:** Claude Code (implementation agent) + solo developer (Rust beginner, Godot/GDScript experienced)
**Status:** Phase 0 only — offline, local MVP (single player's own session; party of N controlled characters). Phases 1–2 are referenced ONLY where they constrain Phase 0 architecture.
**Working title:** TBD (use `taskbar-arpg` as repo/project name)

---

## 1. One-paragraph pitch

A Ragnarok Online–inspired idle ARPG that lives in a thin always-on-top strip at the bottom of the screen, above the Windows taskbar. The world is a 2-D grid viewed RO/Diablo-style; the player's party auto-fights monsters while the player works. Core loop: farm → loot → build (gear/cards/stats) → refine (risky upgrade) → progression wall → repeat. Phase 0 ships a fun offline game; the architecture must be ready for a Phase 2 server-verified, Steam Market–integrated economy without rewrites.

**Design pillars (in priority order):**
1. **Must be fun offline first.** No economy feature justifies a boring game.
2. **Deterministic to the bit.** The entire simulation is replayable from a seed + input log.
3. **Window-first game, strip is the signature mode.** The game is a normal windowed idle ARPG (where all depth and management live); the taskbar strip is a toggleable signature presentation mode — the marketing wedge (the "RO party farming above your taskbar" hook), not a gameplay requirement. Build windowed-first but **strip-aware**: the main arena + core HUD must always be able to collapse into a short strip. Strip is cosmetic to *gameplay* but essential to *marketing* — these are different axes.
4. **Few systems, one deep core (depth budget).** Positioned between Task Bar Hero (everything shallow → exhausted in days) and IdleOn (everything deep → overwhelming). The single designated deep system is **build-making**: stat allocation × elements × gear × socketed cards × refine. All mastery lives there. Everything else stays deliberately shallow. Acquisition variety (zone-specific drops, timed bosses, low-rate chase items) is CONTENT inside the one loot system — new rows in CSV, not new mechanics — and is encouraged. A new MECHANIC is acceptable only if it is itself shallow AND feeds build decisions (daily boss dropping refine mats = OK; a pet system with its own leveling = rejected; a fishing minigame gating items = rejected). Litmus test: a player should explain the whole game in one minute, but argue builds for an hour.

---

## 2. Scope

### 2.1 In scope (Phase 0)

- Windows desktop only. Godot 4.x.
- Windowed game + toggleable taskbar strip mode (borderless, always-on-top, no-focus). Windowed-first, strip-aware (pillar 3).
- Grid-based 2-D world (integer cells; see §3.2). Sprites face left/right only with horizontal flip (Vampire Survivors-style) — no 4/8-direction art needed.
- Party: sim supports N controlled characters from day 1 (`party_size` in data, symmetric entity rule §4.2.2). MVP content starts with 1 character; final MVP party count is an open question (§8). Full 4-character party ("เปิด 4 จอ") is Phase 1.
- Monster spawning by zone; ~20–30 monster types at MVP.
- Loot: equipment + materials + currency (offline currency, NOT real money).
- Refine system (RO-style risky upgrade) — the emotional core of the loop.
- Zone progression gated by power walls.
- Save = event log + snapshots + hash chain (see §4.4).
- Headless sim runner CLI for balance + determinism testing.
- Data pipeline: CSV → `game_data.json`.
- Asset normalization pipeline (downscale + palette quantize + outline).
- itch.io playtest build at end of Phase 0.

### 2.2 Explicitly OUT of scope (do not build, do not stub beyond what §5 requires)

- ❌ Any networking, server code, accounts, cloud saves.
- ❌ Steamworks integration of any kind (Inventory Service, Market, achievements).
- ❌ Monetization, Merchant License, real-money anything.
- ❌ Cross-player ally / snapshot / PvP / guild systems (local party of controlled characters per §2.1 is fine; networked ally snapshots are not).
- ❌ Content editor GUIs (Google Sheets/CSV is the editor).
- ❌ Localization framework (hardcode English strings; keep them in one constants file).
- ❌ macOS/Linux client support (Linux is CI-only for determinism checks).

If a task seems to require any of the above, STOP and ask the developer instead of building it.

---

## 3. Core gameplay spec

### 3.1 Presentation modes (window + strip)

The game runs in two modes that are two viewports onto the **same sim** (same grid, same state) — never two games. A toggle switches between them at runtime.

**Window mode (primary home):**
- A normal resizable window. Shows the full arena, full HUD, and is where all management UI lives (party, inventory, refine, build/cards, stat allocation, zone select, battle replay viewing later).
- This is where Phase 0 development happens first (easier to build and debug than the strip).

**Strip mode (signature / marketing mode):**
- Height ~80–120 px, width = usable screen width, docked to the bottom of the usable screen rect (above the OS taskbar) via `DisplayServer.screen_get_usable_rect()`. Borderless, always-on-top, no-focus.
- Shows a compact slice of the arena (the ambient "fish tank"). Clicking opens the same management panels as window mode (compact popup window).
- Must NEVER take keyboard focus. Mouse-only.

**Strip-aware constraint (hold from day 1, costs ~nothing now, expensive to retrofit):**
- The main arena and core HUD must always be able to collapse into a short strip (~3 grid rows tall). Never design a screen or HUD element that cannot shrink into the strip.
- Minimum readable cell size (the smallest cell at which sprites stay distinguishable — see M0 "squint test") sets the strip row height and therefore the grid cell budget for the whole game. Lock it early.

**Camera/perspective (both modes):** RO/Diablo-style top-down (slightly angled ¾ view) onto the grid. The render is a window onto the grid sim: entity positions come from sim cell coordinates; GDScript interpolates movement between cells smoothly (presentation-only) so motion reads MOBA/RTS-fluid while the sim stays auditable. Arena dimensions per zone come from data.

### 3.2 World model & combat (auto)

**Grid world (the spatial model — deliberately discrete, NOT continuous 2-D):**
- Positions are integer cell coordinates `(x: i32, y: i32)` on a per-zone grid. Arena dimensions per zone in data.
- One entity per cell (occupancy map). No collision physics.
- Movement: an entity moves one cell per move-interval (accumulator pattern, speed from stats) along the shortest path to its target — BFS on the occupancy grid, deterministic tie-breaking (prefer lower y, then lower x; document the order once and never change it casually).
- Range = Chebyshev cell distance (melee = 1; ranged values in data). Ranged attacks may declare travel time in ticks (data); damage applies as a delayed sim event, never as a renderer effect.
- Targeting: nearest enemy by cell distance, tie-break by entity spawn index. Fixed, published (legibility rule 5).
- Arena size is the ONLY thing that changes between contexts: strip farming (~16×3, a few entities) and future large battle viewers (bigger grids, more entities) use the same rules, same code, same balance. Never fork the combat model per mode.

**Combat:**
- Damage model: `damage = max(1, ATK - DEF)` modified by a 3-element triangle (each element deals bonus % vs one element, reduced % vs another; exact multipliers in `game_data.json`).
- Death: party respawns after a short delay at zone start with no item loss (Phase 0 is forgiving; harsh penalties are an economy-era decision).
- All combat math is INTEGER ONLY and lives in sim-core, never in GDScript.

### 3.2.1 Legibility rules (anti-TFM2; as important as the math itself)

Cautionary tale: Teamfight Manager 2 launched to Mixed reviews largely because backend-resolved outcomes were choreographed by the frontend ("champions walking back into enemies to die") and players couldn't connect decisions to results. These rules prevent that failure class:

1. **The render IS the calculation.** GDScript only visualizes sim state. The renderer NEVER invents gameplay-meaningful motion, never plays a death/retreat/engage animation that isn't a direct depiction of sim events. (Presentation-only flair — inter-cell movement interpolation, hit flash, screen shake, particles — must never imply false causality.)
2. **Never resolve outcome-first.** No code path may compute "who wins" abstractly and then animate toward it. Battles exist only as tick-by-tick simulation.
3. **Every visible event maps to a sim event; every sim event maps to a published rule.** The combat log is generated from the sim event stream verbatim.
4. **Damage is decomposable.** Any damage number can be expanded to its factors (base × element × cards × refine × stats) in the log/inspector, so pre-battle decisions are *visibly* doing work.
5. **Zero hidden randomness in AI decisions.** Target selection, movement, and skill usage follow fixed, published, deterministic rules. PRNG appears only in declared-odds procs (crit, drops, refine). A player auditing the log must always be able to answer "why did that happen" with a rule, never with "the AI felt like it."

### 3.3 Loot & items

- Monsters roll on weighted drop tables (defined in data).
- Item categories: weapon, armor, accessory, **card**, material, currency. Weapons/armor/accessories may declare socket slots in data.
- Rarity tiers: common / uncommon / rare / legendary (names tunable in data).
- Every item definition carries fields that Phase 2 will need even though Phase 0 ignores them: `marketable: bool`, `emission_group: string`, `quota_class: string`. Populate them in data now; sim-core stores but does not enforce them in Phase 0.

### 3.4 Refine (the hook)

- Any equipment piece can be refined +1 … +N using materials + currency.
- Success/fail chances per level come from data. Failure outcomes per level (also data-driven): nothing / downgrade / break (item destroyed → becomes scrap material).
- Refine results MUST come from the sim-core PRNG inside the event-sourced state machine (a refine attempt is an input event). No client-side rolls.
- UI must communicate risk honestly: show exact success % before confirming. No gacha-style obfuscation — deterministic, published odds are a brand value.

### 3.5 Builds (the deep core)

Build identity emerges from four multiplicative layers, each individually simple:

- **Stats:** RO-style point allocation on level-up (e.g. STR/AGI/VIT/DEX; final set in data). One screen, free respec in Phase 0 (respec cost is an economy-era decision).
- **Elements:** weapon/armor element vs zone monster elements (§3.2 triangle).
- **Gear:** slot-based equipment with stat lines from data.
- **Cards:** monsters rarely drop their card; equipment has socket slots (count per item in data); socketing a card grants its modifier (flat stats, element conversion, % vs monster element/family, refine-interaction bonuses). Cards are THE chase items and the future backbone of the Phase 2 market — every card definition must carry the §3.3 marketable/emission fields.
- MVP target: 10–15 cards + 4 stats must support at least 2–3 genuinely different viable builds per zone tier, verified via sim-cli build comparison reports.
- Acquisition variety rule (from Pillar 4): new ways to GET build pieces should be data rows (zone drops, timed boss spawns, low-rate chase drops), not new mechanics.

### 3.6 Progression loop

- Zones unlock sequentially. Each zone has a soft power wall (monster DEF/HP outpaces un-refined gear).
- Intended rhythm: a zone is comfortable → farm drops → refine → push next zone. Target: first wall felt within day 1–2 of idle play; numbers tuned via headless runner, not in code.

### 3.7 Offline catch-up

- On launch, the sim fast-forwards elapsed wall-clock time as ticks (capped, cap in data). This is just `tick()` in a loop — no special offline math. The headless runner and offline catch-up are literally the same code path.

---

## 4. Technical architecture (NON-NEGOTIABLE RULES)

### 4.1 Language split

- **sim-core (Rust):** all game state, all rules, all RNG, all math. Pure library. No Godot, no I/O, no threads, no wall-clock, no floats.
- **godot-ext (Rust, gdext):** thin bridge only. Target API surface ≤ ~6 functions: `init(seed, game_data)`, `apply_input(event)`, `tick(n)`, `view_state() -> json/packed struct`, `export_log()`, `state_hash()`.
- **Godot project (GDScript):** window management, rendering, animation/juice, UI panels, audio. GDScript may NEVER compute a gameplay outcome. If GDScript needs a number, it asks `view_state()`.

### 4.2 Determinism rules (sim-core)

1. `#![forbid(unsafe_code)]` in sim-core.
2. No `f32`/`f64` anywhere in sim-core. Percentages are integer basis points (10000 = 100%).
3. Single PRNG: PCG32, owned by `GameState`, seeded at new-game. Every random draw goes through it in a defined order.
4. No iteration over `HashMap`/`HashSet` for any gameplay-affecting logic. Use `Vec` or `BTreeMap`.
5. Time = `u64` tick counter. 1 tick = 100 ms of game time (tunable constant, but fixed per save).
6. `tick()` and `apply_input()` are pure with respect to state: same state + same calls = same state, on every OS and architecture.
7. CI runs the golden replay suite on BOTH Windows and Linux; hashes must match.

### 4.2.1 Fixed-point conventions (how every stat is represented)

| Stat kind | Representation | Example |
|---|---|---|
| HP / ATK / DEF / currency / counts | plain `i64`/`u64` | `atk = 47` |
| Any percentage (crit, dodge, drop, refine odds, dmg reduction) | basis points, `0..=10000` | 12.5% = `1250`; roll PRNG `0..9999` against it |
| Rates per time (attack speed, regen, DoT) | accumulator pattern: rate in milli-units per tick, accumulate, fire on ≥ 1000 | 1.5 attacks/sec @ 100 ms tick = rate `150` |
| Position | integer grid cells `(x, y)` as `i32` | movement between cells is sim-discrete; smoothness is renderer interpolation |
| Movement speed | accumulator: milli-moves per tick, move one cell on ≥ 1000 | speed 0.4 cells/tick = rate `400` |

Rules:
- Every data column declares its scale in the CSV header comment and in `build_data.py` validation (e.g. `crit_bp`, `speed_milli`).
- Multiply before divide: `a * b / 10000`, never `a / 10000 * b`.
- Integer division floors. Any formula needing different rounding must say so explicitly in a comment AND in this spec. Rounding direction is part of replay-critical behavior.
- Use `i64`/`u64` widths generously; debug builds use checked arithmetic (overflow panics are determinism bugs caught early).

### 4.2.2 Symmetric entity rule (future-proofing, costs nothing now)

Combat code operates on generic entities: `fn resolve_attack(attacker: &Entity, defender: &Entity, ...)`. The player is just an entity with a controller attached. Never hardcode player-vs-monster asymmetry into combat math, targeting, buffs, or death handling. This keeps the sim extensible to NvN battles (async arena / guild battles, far-future) without restructuring. Phase 0 still ships ONLY player-vs-monsters content — this rule is about code shape, not scope.

### 4.3 Repo layout (Cargo workspace + Godot)

```
taskbar-arpg/
├── crates/
│   ├── sim-core/
│   │   ├── src/{state,tick,rng,grid,combat,drops,refine,events,hash}.rs
│   │   └── tests/golden/        # *.replay.json + expected hashes
│   ├── sim-cli/                 # headless runner
│   └── godot-ext/               # gdext bridge
├── data/
│   ├── sheets/*.csv             # source of truth for content
│   └── build_data.py            # validate + emit game_data.json
├── tools/
│   └── normalize_sprites.py     # asset pipeline (see §6)
├── godot/                       # Godot 4 project
└── .github/workflows/ci.yml
```

### 4.4 Event sourcing & saves

- Every player-initiated action is an `InputEvent` (enum): `StartRefine{item_id}`, `EquipItem{..}`, `ChangeZone{..}`, etc. Idle progress is implicit in tick count.
- Save file = `{header, seed, game_data_version, event_log[], snapshots[]}`.
- Snapshot every N game-minutes: serialized state + `state_hash` (blake3 or xxh3 of canonical serialization) chained to the previous snapshot hash.
- Loading: restore latest valid snapshot, replay events after it. If hash mismatch → fall back to previous snapshot and report.
- This format IS the Phase 2 anti-cheat artifact. Do not "simplify" it away.

### 4.5 Data pipeline

- Content lives in CSVs (`items.csv`, `monsters.csv`, `zones.csv`, `drop_tables.csv`, `refine_table.csv`, `constants.csv`).
- `build_data.py` validates (unique stable string IDs, dangling references, drop weights sane, refine rows complete) and emits one `game_data.json`, embedded-versioned.
- sim-core, sim-cli, and the Godot project all consume the SAME `game_data.json`. Balance numbers never live in code.

### 4.6 Performance budget (client)

- ≤ 2% CPU on a mid-range laptop while idling, ≤ 200 MB RAM.
- FPS cap 30, Low Processor Mode on. Sim ticks are cheap; rendering is the budget.

---

## 5. Milestones & acceptance criteria

Work strictly in order. Each milestone ends with criteria the developer verifies manually.

### M0 — Strip de-risk spike + readability lock (GDScript only, ~2–3 days)
Goal: prove the *risky* strip-specific unknowns before investing in window mode, and lock the art/grid resolution. Bare scene, placeholder sprites on a strip-height window.
- [ ] Borderless, always-on-top, no-focus window docked above taskbar via `DisplayServer.screen_get_usable_rect()`
- [ ] No focus steals while typing in another app; survives sleep/wake, resolution change, monitor plug/unplug; clickable without blocking the real taskbar
- [ ] CPU ≤ 2%, RAM ≤ 200 MB (Task Manager) over an 8-hour beside-work session
- [ ] **Squint test:** place candidate sprites at strip height and confirm 3–4 monster types stay distinguishable at a glance from normal working distance (silhouette + color, not detail). Record the smallest readable cell size in px.
- [ ] **Lock outcome:** strip row height + grid cell pixel size + normalize target resolution (§6) are fixed from this result and feed §8 open questions.
- [ ] If a strip unknown proves unworkable, the game still ships as windowed-only — note the issue and proceed; this spike is no longer existential (pillar 3).

### M0.5 — Window shell + workspace bootstrap
- [ ] Normal resizable window renders a placeholder grid arena (window mode is the primary dev surface)
- [ ] Runtime toggle between window mode and strip mode proven with a placeholder scene
- [ ] Cargo workspace builds; gdext loads in Godot (call `state_hash()` from GDScript and print it)

### M1 — sim-core skeleton + CI
- [ ] PCG32 implemented with reference test vectors
- [ ] `GameState` + `tick()` advancing a counter; first golden test passes
- [ ] CI: clippy -D warnings, rustfmt check, tests on windows-latest AND ubuntu-latest, hashes compared

### M2 — Data pipeline
- [ ] `build_data.py` validates and emits `game_data.json` from CSVs
- [ ] 10 items, 5 monsters, 2 zones defined in sheets
- [ ] sim-core loads game_data; bad data fails loudly at build time, never at runtime

### M3 — Combat vertical slice
- [ ] Grid world: occupancy, BFS movement with documented tie-breaks, deterministic targeting
- [ ] Auto-combat with element triangle, integer math, in sim-core
- [ ] Drops rolling from data-defined tables through sim PRNG
- [ ] Godot renders sim state on the strip grid: party + monsters, smooth inter-cell interpolation, HP, hit juice
- [ ] sim-cli runs "30 game-days" in < 10 s and emits a CSV progression report
- [ ] Golden tests cover: pathing sequence, combat sequence, drop sequence, full 1-day replay

### M4 — Builds, refine + wall
- [ ] Stat allocation (free respec) as input events
- [ ] Sockets + 10–15 cards dropping from data-defined tables; socketing as input events
- [ ] Refine state machine as input events; published odds shown in UI
- [ ] Failure outcomes (nothing/downgrade/break) per data table
- [ ] sim-cli build comparison report proves ≥ 2 distinct viable builds for zone 2
- [ ] Zone 2 wall tuned via sim-cli so refining + build adjustment is the intended answer
- [ ] Offline catch-up working (capped fast-forward)

### M5 — Playtest build
- [ ] Save/load via snapshot+replay robust across game_data version bumps (migration = re-validate, refuse incompatible)
- [ ] Normalized asset set integrated (§6)
- [ ] Windows export uploaded to itch.io (private), basic local analytics counter (sessions, days retained — local file only, shown to user, nothing transmitted)

---

## 6. Asset pipeline

`tools/normalize_sprites.py` — every sprite (purchased packs, AI packs, future commissioned art) passes through before import:
1. Downscale to target grid (default 48×48, config per category) with nearest-neighbor.
2. Quantize to the project master palette (single `.pal`/hex list checked into repo, ~32 colors, chosen from lospec).
3. Apply uniform 1 px outline (palette's darkest tone) + normalize contrast.
4. Emit to `godot/assets/normalized/` with stable filenames matching item/monster IDs in `game_data.json`.

Rule: raw pack files never enter the Godot project directly. Art is referenced by ID; swapping art later must require zero code changes.

---

## 7. Working agreements for Claude Code

1. **Money-zone code requires teaching.** Anything in `drops.rs`, `refine.rs`, `rng.rs`, `events.rs`/`hash.rs`: include generous comments written for a Rust beginner, and summarize *why* each design choice was made when presenting the diff. The developer must be able to explain these files line by line.
2. **Never change sim semantics silently.** If a refactor would alter any golden-test hash, stop, explain the behavioral change, and get explicit approval before updating expected hashes.
3. **Tests first in sim-core.** New gameplay rule → golden or unit test in the same change.
4. **Ask, don't assume, on scope.** Anything touching §2.2 (out of scope) or new dependencies in sim-core: ask first. Prefer zero-dependency or tiny, audited crates (serde, blake3/xxh3 are pre-approved).
5. **Balance numbers go in CSV/JSON, never in Rust/GDScript.** If a constant feels gameplay-ish, it belongs in `constants.csv`.
6. **Keep the bridge thin.** Resist adding godot-ext API surface; prefer enriching `view_state()`.

---

## 8. Open questions (do not resolve unilaterally)

- Final tick rate (100 ms assumed) and offline cap value.
- Master palette choice.
- Strip arena grid dimensions (16×3 assumed) and cell pixel size at strip height.
- MVP party size: 1 character only, or unlock a 2nd slot within Phase 0 to test party synergy (recommended) — affects M4 scope.
- Player character: single fixed class for MVP vs. weapon-defines-style.
- Whether the popup panel is a second OS window or an expanding strip overlay (test both in M0 if cheap).

## 9. Future-enabled by design (NOT Phase 0 scope — do not build)

Recorded so future decisions don't accidentally close these doors. None of this is built in Phase 0; it only explains *why* the Phase 0 architecture rules exist.

- **Three-tier authority model (the shape of the whole future game):**
  - *Private sim* — runs on the player's machine: farming, refine, builds. Deterministic, offline-capable, ~zero server cost. This is where wealth is created and must be verifiable.
  - *Major events* — server runs the sim **once per event** and broadcasts a small input bundle: guild war (weekly), world boss. Cost is spiky and trivial, not continuous.
  - *Social/economy* — async shared via a light server: market, leaderboards, watching others' replays. No real-time shared world.
- **Sim is always session-local; there is NO shared-world real-time combat.** Shared-world MMO farming would require an always-on authoritative server (~10–50× the cost, ~$6k–20k/mo at 10k users vs hundreds–low-thousands for session-based), destroy offline play and determinism, and turn a solo-maintainable game into a team-run ops business. The RO "others are present" feeling is delivered by the social/economy tier (towns, market, guild, leaderboards, and optionally static avatar snapshots of other players), not by shared simulation.
- **One binary, two homes (the reason sim-core is Rust + integer-only).** The SAME compiled sim-core runs on the client (via gdext) and headless on the server. Trust comes from players re-running identical code and getting identical results — not from believing the server. This is only possible because gameplay logic lives exclusively in sim-core (§4.1) and is bit-deterministic across OSes (§4.2), verified by the Windows+Linux golden suite (M1). If any gameplay outcome were ever computed in GDScript, client and server would diverge — hence that hard rule.
- **Replay = stored inputs, never recorded video.** "Watch live", "watch later", and "scrub" are all the same activity: load an input bundle (seed + both sides' build snapshots, a few KB) and run the sim to draw frames. Live = everyone runs it starting at a server-announced start time (synchronized broadcast at near-zero server cost — no video streaming). Scrub/replay = run it again, fast-forwarding via the same code path as offline catch-up. Never add a separate recording/streaming system: a second source of truth would reintroduce the TFM2 desync failure and break verifiability.
- **One combat model, many arenas.** The grid sim (§3.2) is the single combat model for everything, forever. Future modes change arena size and entity count via data — never the rules. Camera/window is per-mode; battle viewers are separate scenes reading the same sim.
- **World boss (async, embarrassingly parallel).** Not "10k players hit one boss in real time" (a netcode nightmare). Each player submits a build; the server runs each build-vs-boss as an independent deterministic sim, sums the damage/results against the boss HP, and ranks. No shared state to sync; every run independently verifiable.
- **Guild War (the flagship far-future mode):** weekly cadence (WoE rhythm). Guilds lock member builds before a deadline; the server runs one deterministic 16v16 (size TBD) grid battle per pairing and publishes the input bundle; everyone watches TFT/MOBA-replay style (live at a set time, or later). Compute cost is trivial by design. The build-lock deadline doubles as a weekly economy heartbeat in Phase 2+ (pre-war market rush). **Core value — respect player time:** contribution = build quality accumulated asynchronously all week (farm, market, or comp cleverness); the war itself is ~20 minutes of *optional* watching (it's always replayable, so live attendance is a bonus, never an obligation). No synchronized-attendance labor, ever (anti-GBF-GvG principle).
- **Async deterministic battles (general form):** any PvP is `result = sim(seed, side_A_snapshots, side_B_snapshots)`. The symmetric entity rule (§4.2.2) plus headless snapshot-vs-snapshot invocation in sim-cli are the only Phase 0 obligations.
- **Battle design direction (when the time comes):** all intelligence lives pre-battle (formation on the grid, comp, builds, cards); in-battle AI stays simple, fixed, and published — modeled on TFT's transparent targeting/movement rules (nearest enemy, chase one cell then retarget, published tie-break priority). Battles are decided by build-making — consistent with Pillar 4, the legibility rules (§3.2.1), and the transparent-odds brand.
