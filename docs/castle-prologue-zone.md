# The Prologue castle — a story-only ruined-castle zone

Status (2026-09-03): **floor strip + dressing scene BUILT and verified in the pipeline; renderer
resolver/override + the game_data half NOT yet wired.** The proto
(`godot/proto/arena3d/castle_preview.tscn`, F6) renders the approved look to
`~/Desktop/ROI/castle_scene.png`. This doc is the recipe to turn that proto into a real
zone. Read `docs/hd2d-look-pipeline.md` §8 (the grassland zone recipe) first — this is the
same pipeline with a castle skin and two deltas (a back WALL instead of cliffs; a dark
night interior instead of a sunny biome).

## What it is / what it is NOT

- **Story-only.** Entered exclusively through story mode (`StoryDriver`, `@ battle <zone_id>`),
  never the band picker, the world map, or a `next_zone_id` chain (user 2026-09-03). So it needs
  **no** band/region-name/world-map/`SelectBand` wiring — none of the `region 0` traps in
  §"region 0" of the survey apply, because the zone is never addressed as a band.
- It is the **prologue combat zone**: a pre-made cast (`@ roster preset`) fights an **authored,
  fixed-tick defeat** (`@ battle zone_story_prologue 600`), like today's `zone_story_trial`.
  The sim runs ONE zone for the tick budget; it does not sweep-and-advance three rooms.
- **"Long like Band 2"** therefore means the ART is a long strip the camera sits inside, giving
  the corridor a sense of length — not an actual 3-room sim walkthrough (that system, in
  `arena3d.gd`, only fires for `band >= 1` zones and is irrelevant here).

## The look, decided in the proto (2026-09-03)

- Aged stone FLOOR, **tiled 128px** aged look (grout AO, broken tiles, hairline cracks, moss) —
  the user picked this over a baked-strip floor and over a damage overlay. Generator:
  `tools/gen_castle_tex.py` (deterministic; reproduces every `godot/assets/castle/*.png`
  byte-for-byte). uv is NON-uniform so a ~2.6-unit tile stays square on a non-square plane.
- Back WALL (not cliffs): a long stone slab with a per-room arched doorway (CSG box − cylinder)
  and a couple of collapse breaks open to the night sky; columns, wall torches (warm point light
  + additive flame billboard), hung tattered banners, scattered rubble, a red carpet runner.
- MOOD: cool moonlight key + warm torch pools = the Octopath night-castle split. **No sun cues**
  (god rays / wet ground / motes) — it is indoors at night.

## Reserved look-region (the one architectural decision)

The renderer keys ALL art off a `region` int derived from the zone id (`_bg_region()` →
`bg_r<n>.png`, `zone_dressing_r<n>.tscn`, `BG_Z_SHIFT[n]`, `SUNNY_REGIONS[n]`, `FOG_REGIONS[n]`).
Story zones parse to band 0 → region 0 → the "no art" sentinel. To give the castle art WITHOUT
refactoring every int-keyed table, **reserve a look-region int for the castle** (proposed
`CASTLE := 90` — far above the real 1..20 region range, so it can never collide with a band's
region). Then:

1. **`arena3d_scene.gd:_bg_region()`** — before the `band < 1 → 0` return, add: if the zone id
   begins with the story-prologue prefix (`zone_story_prologue`), return `CASTLE`. One line, and
   the 2D mirror `arena_draw.gd:_bg_region()` gets the same (or is left on the checker — 2D is
   frozen, so only keep it parsing/booting).
2. **Floor** — DONE. `tools/gen_castle_floor.py` bakes a long castle strip → `godot/assets/bg/bg_r90.png` (+`_n`, +`_s`).
   Bake the approved tiled-floor algorithm across a long canvas with per-tile variation + damage
   so repeats are not obvious (the same reason `bg_r2` is one baked strip, not a tile). Target
   `bg_r2`-class width (≥ 2308px to trip `_bg_is_strip`; `bg_r2` is 3456×1472). `BG_Z_SHIFT[90]`
   set by eye in the preview (no grass back-pad rows here — the wall, not cliffs, hides the back).
3. **Dressing** — DONE. `zone_dressing_r90.tscn` whose root script (`castle_dressing.gd`) builds wall +
   columns + torches + banners + rubble in `_ready()` — a straight port of `castle_preview.gd`'s
   `_wall`/`_column`/`_torch`/`_banner` helpers, minus the camera/env/party/floor (those come from
   the arena). The torches are OmniLights **inside the dressing scene**, so no per-region light
   table is needed for them. Follow the strip dressing rule (props at front edge z ≤ −5.3 so they
   never land in the combat field).
4. **Ground normal_depth override (REQUIRED)**: the global `GROUND_NORMAL` and `GROUND_BUMP` are
   both 0 — grass leans on the baked SHADE map, not relief — so `_refresh_background` passes
   `normal_depth = 0`, which makes the shader skip the normal map entirely (`if normal_depth > 0.0`)
   and the floor reads BLURRY (no grout relief). The castle floor NEEDS its baked normal: give the
   look-region a `normal_depth` override (~2.2, verified in the preview) so the grout channels +
   flagstone bevels catch the raking torch/moon light.
4. **Dark override**: `SUNNY_REGIONS` leaves 90 out (→ no god rays/wet/motes, already handled).
   The one thing still global is the KEY (sun) energy/colour; a night interior wants it dimmer and
   cooler. Add a minimal per-region override read in `arena3d_look.gd:_apply_look` (e.g. a
   `KEY_SCALE`/ambient entry for `_look_region == CASTLE`) — seed from the proto's moon
   (energy ≈ 1.15, cool blue) + low ambient. **This is the eyeball-tuned part — do it in the
   preview with the user.**
5. **Preview** — DONE. `proto/arena3d/castle_zone_preview.tscn` (F6) renders the real dressing_r90
   over bg_r90 under the night candidate lighting → `~/Desktop/ROI/castle_zone_{room1,room2,over}.png`.
   This is where the wall placement / framing / lighting get iterated with the user.

**Still to wire (renderer, one slice with the data so it is reachable — see "systems built but
unreachable"): steps 1 (the `_bg_region` resolver + `CASTLE := 90`), 4 (the dark override in
`arena3d_look._apply_look`), and `BG_Z_SHIFT[90] = -3.0`. Deferred deliberately: with no story zone
id in the catalog yet, a resolver would map nothing, so it is built together with the data.**

## Data half — DEPLOY-GATED, do not ship alone

Adding the zone rows to `data/game_data.json` (id `zone_story_prologue` or `_1/_2/_3`, a
`spawn_pool` of castle monsters + a boss, `level` set to the prologue's intended power, mirroring
`zone_story_trial`'s shape) trips, in order:

- `the_catalog_cannot_change_without_its_version_changing` → bump `version` in the JSON + re-pin
  the blake3 `CONTENT_DIGEST` (`crates/sim-core/src/data/mod.rs`).
- Copy to `godot/data/game_data.json` (the bundled export fallback — nothing keeps the two in
  sync, and no test compares them; an edit to only one silently ships the old catalog).
- `every_story_battle_names_a_zone_that_exists` → the `@ battle` id in `godot/story/*.dialogue`
  must name a real zone. Point `prologue.dialogue` at the new id (it currently borrows
  `zone_story_trial` as a placeholder).
- **verify-server**: its `game_data.json` is baked into the image (`Dockerfile`), and it rejects a
  client whose `game_data_version` differs (409). So a catalog bump **requires a coordinated
  `tools/deploy.sh`** or every online save is rejected. The prologue battle itself runs on a
  sandbox bridge (no save/server), but the version bump affects ALL online play. **This is why the
  data half waits for a deploy window — it is the user's call, not an autonomous step.**

Goldens: only `golden_alpha6` loads the catalog (seeded from `zone_b1_1`); adding a *new* zone
does not move its hash as long as `zone_b1_1`, its monsters/drops, `level_base`, and the `ZoneDef`
shape are untouched. No golden re-pin needed for a pure zone add.

## Open (story design, not engineering — TODO.md "The Prologue")

- Is the sacrificed Exemplar ever obtainable? (gacha hook vs. story weight — unsettled).
- The real roster/cast (proto uses the `trial_three` preset placeholder).
- Reward + `% title`/`% brief`/`% requires` meta on `prologue.dialogue` (currently a bare
  `% id prologue` placeholder).
