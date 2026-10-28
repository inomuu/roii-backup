# Testing notes — what the suite covers, and how to write one here

ย้ายออกมาจาก `CLAUDE.md` (2026-08-13) เพื่อลดขนาดคำสั่งที่โหลดทุกเซสชัน เนื้อหาไม่ถูกแก้
อ่านไฟล์นี้ก่อนจะเชื่อผลเทสต์ที่ผ่าน หรือก่อนเขียนเทสต์ใหม่

### What the tests do NOT cover (read before trusting a green run)

A green `cargo test` says the SIM is consistent with its pins. It says nothing about these, so
they need eyes or a launch:

1. **`godot/views/` — almost no tests, and nothing automated runs the ones that exist.** The only
   `.gd` coverage is seven hand-run `SceneTree` selftests in `godot/tests/` (`story_driver`,
   `story_trial`, `story_mission`, `dialogue`, `mode_layer_ownership`, `player_node_geometry`,
   `save_write_guards`), each launched on its own:

   ```bash
   godot --headless --path godot --script res://tests/story_driver_selftest.gd
   ```

   **`tools/smoke.sh` does NOT run them** — a green smoke says nothing about the story layer, and
   nothing at all about the other ~187 files in `views/`. (Note their own lesson, written into
   `story_driver_selftest.gd`: an empty failure list is not evidence of success — the first run
   printed PASS while every check died on a parse error, which is why they count `_done`.)

   Two of the six fail on `master` as of 2026-08-13 (`story_trial`, `dialogue`) — confirmed
   pre-existing against a clean worktree, so check what a failure did BEFORE on this file before
   blaming your change. `story_trial` and `story_mission` also need `godot/data/game_data.json`,
   which is gitignored: a fresh checkout reports "no game_data.json" rather than a real result.

   The worst shape is silent: the bridge ships keys (`card_lines`, `bind_lines`, `special_vals`,
   `slot`, …) and GDScript reads them with `.get(key, default)`, so a RENAMED or DROPPED key
   doesn't error — the tooltip just renders empty. Every FFI-shape change must be seen in a
   running game.
2. **The FFI boundary — now PARTLY covered (2026-07-31).** `godot-ext/src/view_state.rs`
   pins the emitted key set (`contract_keys_are_stable`), so a DROPPED or RENAMED key in
   `view_state` now fails `cargo test -p godot-ext` instead of silently blanking a panel. It
   guards LOSS only — adding a key is free. Still uncovered: the VALUES behind those keys, every
   other bridge method (`stash_json`, `item_view`, the stat helpers), and the GDScript side of the
   contract. A key can still exist and carry the wrong thing.
3. **Performance.** Nothing measures tick cost or allocation, and no benchmark is committed.
   `stat_mods::for_each_mod` re-resolves Exemplar/Regalia defs (a `format!` + a def build) and
   rebuilds each card's mod Vec on EVERY reader call, several times per attack.
   **RE-MEASURED 2026-07-31** (release, 5-member party, Exemplar + Regalia + a 5★ socketed card
   on every member — the 2026-07-30 figure was taken with NO overlay, so the expensive path was
   never entered):

   | | µs/tick |
   |---|---|
   | no overlay | 3.67 |
   | full overlay | 7.00 (**1.91×**) |

   So the path really does roughly double tick cost — and it does not matter: at 10 ticks/s that
   is 70 µs of work per second of play, and an 8-hour offline catch-up (288k ticks) goes from
   ~1.1 s to ~2.0 s. Not worth caching until something changes the order of magnitude. Re-measure
   if more overlay sources land, since the multiplier is per-source.
4. **Cross-platform determinism.** Goldens pin arm64 hashes produced on this machine; nothing runs
   them on x86, so a float-free-but-platform-divergent bug would only surface in the wild (this is
   exactly what `diag_band4`'s comment worries about).
5. **The online replay path.** verify-server's replay is covered by its own tests, not the golden
   suite; client-dylib vs deployed-server VERSION skew is caught by `tools/check_online_sync.sh`
   (a script someone must remember to run), not by a test.
6. **Real old saves — MOSTLY COVERED since 2026-08-16, with two edges left.**
   `every_real_save_on_this_machine_still_loads` (`tests/suite/save_shape.rs`) reads the live
   `user://` directory, deserializes every `save_N.json` into a `GameState`, and runs
   `migrate_save_items` over each — the real load path, against saves written ~130 VERSION bumps
   ago. Renaming a field without `#[serde(default)]`/`#[serde(alias)]`, retyping one, or renaming
   an `ItemKind`/`Rarity`/`WeaponType` variant now fails at `cargo test` instead of when a player
   opens the game. It FAILS rather than skips when there is nothing to read, unlike the `pg_`
   suite. What is still open:
   - **Machine-local.** The corpus is whatever saves exist here, so a fresh clone has none and
     the test goes red. That is deliberate — committed fixtures would only cover the shapes
     somebody remembered to build, which is what this gap was complaining about — but it does
     mean the coverage is not portable.
   - **The corpus exercises what it happens to contain.** Measured 2026-08-15: 15 items with
     `quantity > 1` and ZERO with `refine_level`, `refine_attempts` or `bound`, so those fields
     ride along untested here (they ARE covered synthetically by
     `migrate_save_items_preserves_every_per_instance_field`). It becomes live the moment a
     refined or bound item is saved.

   Not covered at all: `savegame.json` (pre-0.0.168) genuinely does not load, and cannot be made
   to — the party bag was renamed `stash` → `inventory` and `stash` was REUSED for the deposit
   warehouse, so `alias = "stash"` collides and made saves unloadable for 24 versions
   (0.0.169–0.0.193, see the note on `GameState::inventory`). Reading it needs a versioned
   migration nobody has written.
7. **Process-level death.** Since the 17 test crates became one binary, an abort / stack overflow
   (NOT a normal panic — the harness still isolates those) takes the remaining results with it.

### Two rules for writing a test here (learned the hard way, 2026-08-02/03)

Three bugs in one day got through a green suite for the same reason: **the tests asserted that
the code does what it does, in a state the test itself constructed.** A craft cap bug survived
twelve passing tests because every one of them started from an empty candidate list while a
real account sits at the cap. An inverted drop pipeline ran for three months because nothing
ever *measured* how fast candidates accrue. A clamp "test" re-implemented the clamp in its own
body, so it tested `u32::clamp` rather than any code in this repo.

**1. A CAP and the LOAD that consumes it are ONE decision — test them in ONE test.**
`RARE_CANDIDATE_CAP` was sized against one accrual rate; a data commit raised that rate 62x and
nobody re-derived the cap, because the two numbers live in different files with nothing binding
them. Any budget, capacity, TTL or queue bound needs a test that computes the real load from
the real catalog and asserts they still fit — see `candidate_load_fits_the_offline_window`,
`gear_tickets_fit_the_unsynced_window`, `the_bag_fills_at_the_pace_the_loop_is_designed_around`.
Both numbers then fail together, which is the only time either failing is useful.

**2. For anything numeric, MEASURE — don't assert a boolean.**
A test that pushes a candidate in and checks it is there proves nothing about the rate. A test
that computes candidates-per-kill from `data/game_data.json` and compares it to the design
intent fails the moment the data drifts. Same for byte budgets: serialize the real struct and
count, don't estimate.

Corollary, and it is not optional: **sabotage every new test before believing it.** Break the
line the test is supposed to protect and confirm it goes red. Half the guards written on
2026-08-03 survived their own sabotage — two were unreachable given their input types, so they
were decoration, and one test never touched production code at all. Sabotage is what told the
difference; nothing else did.

Coverage is by hand-written scenario, not by measurement — there is no coverage tooling. A stat
family can be authored, plumbed, and consumed by nothing without any test noticing.

### Golden tests workflow (IMPORTANT)

`crates/sim-core/tests/suite/golden_*.rs` each pin a 64-hex state hash of a fixed scenario. They
exist to catch UNINTENDED determinism drift. When you make an INTENTIONAL change that shifts a hash:

1. Run `cargo test -p sim-core --tests` and read which goldens moved and WHY — confirm the drift
   is expected (a new field, a tuned formula), not an accidental draw-order or logic change.
2. Re-pin with `tools/bless_goldens.sh` (all) or `tools/bless_goldens.sh beta9` (one). Blessing
   is you asserting "this change was intentional."

The version changelog entries in `lib.rs` always note which goldens were re-blessed and why —
follow that convention when you bump `VERSION`.

**ONE test binary — no process-global state.** Every scenario under `crates/sim-core/tests/suite/`
is a `mod` of a SINGLE test target (`tests/suite/main.rs`), not its own executable: each file used
to be its own crate that static-linked sim-core, so one lib edit relinked all 17 and that link —
not the tests — was most of `cargo test` wall-clock. The cost of merging them is that all tests now
share ONE process and run on parallel THREADS, so **a test may never touch process-global state**
(env vars, current dir, `static mut` / `OnceLock`, working directory). sim-core is pure — no I/O,
no globals — so this holds today; keep it that way or tests become flaky in ways that are very hard
to reproduce. Build each scenario's `GameState` locally and share nothing.
Run one scenario with `cargo test -p sim-core --test suite golden_beta9` (a NAME filter — the old
per-file `--test golden_beta9` target no longer exists). Adding a scenario = drop the file in
`tests/suite/` and add its `mod` line to `main.rs`.

### The Unity probe suite: how often, and what is KNOWN RED (2026-09-09)

`tools/unity_probes.sh` is a MILESTONE check — a merge, a release, something structural moving —
not a per-change one (user, 2026-09-09: "อาจจะรันนานๆที ก็พอ รันวันละหลายรอบเสียเวลาเกินไป").
Per change, run `cargo test -p sim-core --lib`, `python3 tools/check_loc_keys.py`, a Unity
`refresh` compile, and open the game. On the day this was written the suite cost over an hour
across several runs and found no real bug; every genuine bug that day came from playing or from
a `SceneDump` diff.

**BEFORE RUNNING IT, if `sim_core::VERSION` was bumped, restart the local verify-server.** The
online version gate refuses a sign-in on a mismatch, the game then sits at the login screen, and
every probe that needs a session times out — five at once, with error text that blames whatever
each probe happened to be pressing ("the bag may be full"). `tools/check_online_sync.sh` exists
to catch exactly this and is worth the two seconds.

**A probe that fails DIFFERENTLY on each run is state-dependent, not a regression.** The screen
probes share one mutable dev save and the party keeps fighting and looting while they measure.
The fix for that is always the same and it is not to relax the assertion: the probe sets up
everything it asserts on, and asserts on something it owns rather than on a shared total.

**KNOWN RED — AlbumProbe, the ★ Up (feed) leg.** Its deposit and withdraw legs were repaired on
2026-09-09; the feed leg needs rewriting, not patching, and here is what the next person is
walking into:

- `AlbumWindow.FeedCandidates` needs a target under the 5-star cap. The dev album accumulates, so
  every earlier run of this probe has fed its rows to the cap and the button opens no dialog at
  all — the probe reports "the feed was never confirmed", which names nothing.
- Choosing an eligible row out of `card_album` and tapping that index does NOT work: the drawn
  cell order is not the sim's list order.
- The assertion after the feed (`total1 == qty0 - 1`) assumes the album holds ONE row.

So the leg needs an album it controls — a card def nothing else uses, deposited fresh, with the
target chosen by pressing until the bench offers the move (the way EnchantProbe now picks its
piece) rather than by index.
