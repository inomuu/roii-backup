# ROI: Reign of Idlers — project handoff

Written 2026-09-09 for someone (or some model) picking this up cold. It is a map, not a spec:
every section says where the real answer lives, because this repo keeps its decisions in files
rather than in anyone's head.

**User decision 2026-09-10:** the class system is cancelled; the starting
characters are **Seraphine + Rapis**. An old Swordsman dev save is historical
data, not the intended starter design. Do not add a Swordsman Potential Board.
Fresh Unity creation now asks only for a party name and starts both characters
through the shared deterministic starter constructor (sim 0.0.345-alpha). The
verifier recognizes this genesis; existing saves and historical constructors are
preserved. Recruit/advancement still contain legacy class flows to revisit.
Deployed 2026-09-10: verifier and Unity native plugin both report sim
0.0.345-alpha at commit 865ada6d6e6c; catalog 0.0.101-alpha matches.
Linux unit/golden checks and post-deploy source-identity checks passed.

**Update 2026-09-10 — Potential Board:** the functional Unity node graph and
Rust purchasing/combat/save path now exist on `master`, using **Skill Points**
temporarily (items later). Current scope and validation are in
[potential-board-implementation.md](potential-board-implementation.md).
The boards support Seraphine/Rapis; incomplete nodes remain previewable and
unavailable for purchase. Both were checked together in the idle Unity Editor.
The sections below describe the earlier handoff snapshot. A separate JRPG Unity
instance may also be open: verify the project path before editor operations.

**Read these three first, in this order.** Nothing below replaces them.

| file | what it is |
|---|---|
| `CLAUDE.md` | the rules. Architecture, the sim invariants, the Godot/Unity discipline, the commands. If this handoff and `CLAUDE.md` disagree, `CLAUDE.md` wins. |
| `TODO.md` | what is BUILT and what is NEXT, priority-tagged, with the reasoning for each deferral inline. Several entries carry a gate ("only after X ships") that is not inferable from the code. |
| `crates/sim-core/src/lib.rs` (the changelog) | 4,800 lines of dated entries, one per schema or balance change, each with the measurement that justified it. The tail is the most reliable account of recent work anywhere in the project. |

`docs/taskbar-arpg-gdd-phase0.md` is the design source of truth; `docs/combat-formulas.md` and
`docs/skills-catalog.md` document the live formulas. `README.md` is STALE — it still describes a
Phase-0 offline MVP.

---

## 1. What the game is

An idle ARPG in the lineage of Ragnarok Online: an auto-fighting party of up to five, where the
depth is in the BUILD and the party composition rather than in per-second input. Cozy, colourful
pixel art with bright anime characters — not dark. The player is the **Commander**.

**It is online-required.** There is no offline build, and this gets proposed wrongly often enough
that `CLAUDE.md` pins it in a section of its own. The offline entry in Godot's login picker is a
dev path. Do not design around it, and do not weaken a server-authored mechanic so it can work
there — gate the feature off instead.

Targeted at Steam (App ID 4958230, "Coming Soon" not yet posted), with mobile as a plausible
second platform — the client is already mobile-shaped, and SKUs/claimables are server-side data
for that reason.

---

## 2. The architecture, and the one rule that explains all of it

**A deterministic, integer-only Rust simulation. Every renderer is read-only.**

That single boundary is the reason the project can be worked on at all: gameplay is verified in
Rust tests and a headless CLI, not by opening a game and looking at it. It is also the reason a
second renderer (Unity) could be added at all without touching gameplay.

```
crates/sim-core     the entire simulation. GameState, tick(), apply_input(), catalog,
                    combat, skills, drops, refine. No I/O, no floats, no threads.
crates/sim-host     the ONE read interface: view_state.rs builds the JSON both engines read,
                    plus the debug/query surface. The widest untyped surface in the project.
crates/godot-ext    gdext bridge. Exposes one class, SimBridge. JSON is the only type that crosses.
crates/sim-ffi      plain C-ABI cdylib for engines that are not Godot. This is what Unity loads.
crates/sim-cli      headless balance runner. Auto-plays a party, emits a CSV progression curve.
crates/verify-server axum + Postgres. Steam auth, session pinning, server-rolled drops and refine,
                    economy, market, chat, mail. Verifies uploaded saves BY REPLAY.
```

**sim-core invariants (non-negotiable, enforced by lints and goldens):** no floats anywhere
(percentages are basis points); no I/O, no threads, no wall-clock, no hash-order iteration; ONE
PCG32 PRNG whose draw ORDER is part of the contract; `tick()` and `apply_input()` pure w.r.t.
`GameState`; `apply_input` for invalid input is a SILENT no-op and must never panic, because
replay has to survive version skew. `sim_core::VERSION` is bumped whenever the serialized shape
of `GameState` changes.

**Authority model: client-sim + server-VERIFY, not server-authoritative.** The client runs the
sim; the server replays the save (seed + inputs must reproduce the claimed hash) and owns only
what the client must not be trusted with. The practical consequence, and it bites: an input
marked `InputAuthority::Client` CANNOT be caught by replay-verify — the server runs the same code
and agrees with whatever the sim accepts. A rule about such an input has to be enforced inside
`apply_input` or nowhere. `SelectBand`'s band gate is the worked example.

---

## 3. Two renderers, and where each one stands

**Godot 4.6 (`godot/`) — the live client.** Complete and playable. HD-2D (the Octopath look,
adopted 2026-08-16) is the direction; the flat 2D scene is FROZEN — kept as a fallback, no parity
work, but it must still parse and boot headless (`tools/smoke.sh` checks it).

**Unity 6 (`unity/ROI/`) — a port, roughly 95% of the way.** Started as a two-week evaluation
that was never formally closed; it now has the arena, the HUD, most screens, the online layer,
Steam auth, the walk-through band, and a probe suite. 87 C# scripts, all UI built from code.
Deliberately NOT ported: chat, ambient birds, and Skills (which waits on the Potential Board).

Both read the same view JSON from `sim-host`. **Numbers do not cross engines.** Camera positions,
grade values, bloom settings, easing curves — every one of these had to be re-derived for Unity;
copying Godot's values washed the scene out or moved the party off the art. What crosses is the
GATE and the STRENGTH, never the value.

---

## 4. Where it actually is, in numbers (2026-09-09)

| | |
|---|---|
| `sim_core::VERSION` | `0.0.343-alpha` |
| `data/game_data.json` | `0.0.101-alpha` |
| sim-core tests | 502 unit + 22 golden scenarios, green |
| verify-server | ~241 test attributes, its own suite — `cargo test -p sim-core` does NOT run them |
| Unity probes | 27, driven through a file bridge; see §7 |
| docs/ | 126 files |
| deployed | `renrei-idle-server.fly.dev` (Fly.io, Neon Postgres), healthy |
| master vs origin | **88 commits unpushed** |

**Branches:** `master` is the trunk and carries both the Unity port and the merged gacha pivot.
`feature/gacha-pivot` and `feature/leveling` are local-only. `feature/zone-art-astra` holds the
authored three-room zone art and has been merged. The `worktree-wf_*` branches are agent
scratch and can be pruned.

**Nothing is pushed** because `gh auth` is logged into the wrong GitHub account. That is the
first thing to fix if continuity matters.

---

## 5. What is built, what is not

**Built and working:** the whole combat sim (5 classes with 2-1 advancement, elements, skills,
runes, Apex post-cap, soul paths); drops, refine, enchant, craft; the market (commodity order book
+ per-instance item market, migrated to rows); the schedule/claim machine; battle pass; mail;
Steam auth end to end; save replay verification; telemetry; the walk-through three-room band.

**Designed and NOT built — this is the list that matters:**

1. **Quest / objective system.** Top of the code list, and Commander level waits on it. The
   measurement half already exists (`Measures` on `GameState`); what is missing is the catalog
   and the server-side quest state. `docs/quest-system-design.md`.
2. **Potential Board.** The gacha pivot DELETED skill points and SP and put a PoE-style
   constellation in their place — and the constellation was never built. This is the largest
   functional hole in the game right now. Shape decided in the pivot doc §5.3.
3. **The story prose.** The dialogue machine is built; nobody has written the words. `TODO.md`
   is blunt that this is the single largest line in the estimate and that no amount of
   engineering shortens it.
4. Packs, the rest of bound items, Commander level, tutorial.

**Deliberate test fixtures that must not ship:** 26 catalog items (Exemplar / Regalia / Cards)
carry a 10 Gil `buy_price` from an old test. Kept on purpose; the place to gate them is
`tools/export_windows.sh`, not the catalog.

---

## 6. The commands that matter

```bash
cargo test -p sim-core --lib          # 0.6 s. This is the per-change check.
cargo test -p sim-core --tests        # + the goldens
cargo test -p verify-server           # NOT covered by the above
cargo clippy -p verify-server --all-targets   # availability lints; only clippy runs them
python3 tools/check_loc_keys.py       # every literal Loc key must exist in the CSV

cargo run -p sim-cli -- --seed 42 --ticks 6000 --sample 50   # balance, headless
tools/play.sh                         # build the dylib + launch the Godot game
tools/smoke.sh                        # end-to-end "is it broken"
tools/deploy.sh                       # the ONLY way to deploy verify-server
tools/local_server.sh                 # a local verify-server on roi_dev
tools/check_online_sync.sh            # client/server sim-version skew — run it before online tests
tools/bless_goldens.sh                # re-pin a golden after an INTENTIONAL change
tools/unity_dylib.sh                  # build sim-ffi and stage it into Unity
```

**Unity is driven through a file bridge**, because `-batchmode` cannot attach to a project the
Editor already holds: write a line into `unity/ROI/.roi_cmd` and read the verdict out of
`~/Library/Logs/Unity/Editor.log`. `refresh` recompiles, `stop` leaves play mode,
`exec:Class.Method` runs an editor entry point, `probe:Name` runs one probe.

---

## 7. The traps. Every one of these cost real time

- **A bumped `sim_core::VERSION` needs the local server rebuilt.** The online version gate refuses
  the sign-in, the game sits at the login screen, and every probe that needs a session times out
  reporting whatever it happened to be pressing. `tools/check_online_sync.sh` exists for this.
- **A native plugin is held for the Editor's whole session.** A new `libsim_ffi.dylib` only takes
  effect after the Editor is restarted. Never `cp` onto a loaded dylib — rm + cp + codesign.
- **Unity's domain reload is disabled for the capture paths, so STATICS SURVIVE play mode.** A
  cached material or a cached `Available` flag outlives the session that made it.
- **`JsonUtility` silently drops fields a DTO does not declare and zero-fills ones it does.** A
  renamed or forgotten key renders as 0 or empty and nothing errors. `view_state.rs` trims
  `zone_spawn` to a HAND-WRITTEN key list for the same reason — a key you forget to add there is
  silent on the far side.
- **`godot/` has zero tests.** A renamed FFI key renders an empty panel instead of erroring.
- **A green `cargo test` proves the SIM only.** Read `docs/testing-notes.md` before trusting one.
- **Two sessions can share this worktree.** `git commit -a` will absorb someone else's work.
- **Generated files are not editable.** `unity/.../arena_tables.json` and the Unity copy of
  `strings.csv` come from `tools/unity_data.sh`; edit the source under `godot/` and re-copy.
- **On macOS there is no `timeout`/`gtimeout`** — exit 127 reads as a pass.

---

## 8. How this project prefers to work

These are the user's rules, learned by being corrected. They are worth more than any of the code
above.

- **Copy then adapt, never rewrite.** When porting, `cp` the original and change only what is
  structurally required. The renderer is years of invisible tuning — attack anticipation synced to
  attack speed, contact frames, lunge surge, flinch queues. Read the config TABLES; do not guess.
- **Measure, do not guess.** The recurring failure mode in this project is patching from a plausible
  story about a symptom. What has worked, every time: capture two states and diff them. A scene
  hierarchy dumped at two moments found a bug that three reasoned-about fixes had missed.
- **Fix the code, not the test.** A guard going red on your change is the guard working.
- **Build the live control before tuning by eye.** The slow loop caps how far the search goes.
- **The user sees ground truth.** Their diagnosis is a hint worth verifying, not an instruction —
  but their observation of a symptom is a fact.
- **Reply in Thai; never translate technical terms.** "raycast" stays "raycast".
- **Do not justify a mechanic with lore**, and do not use pace-deferring sign-offs.

The most reliable long-form record of decisions is the changelog in `sim-core/src/lib.rs`. When
you change balance, MEASURE it with `sim-cli` and put the numbers in the entry — every recent
entry does, and that is what makes the file trustworthy a year later.

---

## 9. Known-red, so it is not re-investigated

`AlbumProbe`'s ★ Up (feed) leg. Its deposit and withdraw legs were repaired 2026-09-09; the feed
leg needs rewriting rather than patching, and `docs/testing-notes.md` records exactly what the
next person is walking into. Everything else was green on 2026-09-09.

Unity connection update (2026-09-10): normal login now targets
`https://renrei-idle-server.fly.dev`. Editor/development tests can explicitly set
`ROI_SERVER=http://127.0.0.1:8787`; ScreenProbe refuses a non-loopback target.
The idle local dev save was backed up and cleared at user request. The user's Steam account 13 cloud save was backed up and reset on request; a
subsequent Steam login confirmed no stored save. Account identity was retained.

Local debug now has five save slots; see [unity-local-save-slots.md](unity-local-save-slots.md).
Seraphine/Rapis temporarily use Swordsman/Mage sprites (presentation only).

Fixed-kit correction in sim 0.0.346-alpha: free level-1 kit for Seraphine/Rapis,
including shared trusted-save migration. See potential-board-implementation.md.

Deployment status for 0.0.346 (2026-09-11): successfully deployed through
`ROI_CLIENT=unity tools/deploy.sh` after the earlier builder failures. Fly server,
staged Unity native library, and source all report 0.0.346-alpha / c813514edb6a;
catalog matches at 0.0.101-alpha. Linux release tests passed: 527 core (1 ignored)
and 39 integration/golden tests. Local validation also passed: 6 FFI, 15 host,
242 server; automatic Iron Chains/Fireball casts and migration/refund idempotence
verified in tests. Existing saves receive the free level-1 kit via migration;
no save reset is needed.

Unity was fully restarted with the combat-skill HUD native query and Steam login
resumed account 13 from the server checkpoint. Final save-push verification is
still pending: an earlier push reported an unanchored start snapshot, and the
explicit follow-up push was blocked by automatic approval review. See
[unity-combat-skill-hud.md](unity-combat-skill-hud.md) for validation and recovery details.

Area 6+ temporarily reuses the authored Area 1–5 map; see
[unity-area-map-placeholder.md](unity-area-map-placeholder.md).
