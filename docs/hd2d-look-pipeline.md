# HD-2D Look Pipeline (arena3d)

How the 3D arena went from a flat, evenly-lit checker-plane look to the graded HD-2D scene it is
now — the tools, the techniques, and the decisions (including the ones that were tried and
reverted). Written after the 2026-08-24/25 look pass so the next person (or the next me) does not
re-derive it.

**Scope:** `godot/proto/arena3d/` renderer only. The sim is untouched — everything here is
presentation. Nothing crosses the sim/renderer boundary.

---

## 0. The tool that made it possible: the look probe

`godot/proto/arena3d/_look_probe.gd` — a **throwaway** SceneTree script that renders ONE frame of
the real arena (`arena3d.tscn` driven by `fake_sim`, so no login/save/network) to a PNG.

This is the whole reason the pass was fast. The arena is **code-built** (camera, lights, floor,
backdrop all created in `_ready`), so opening `main3d.tscn` in the Godot editor shows an empty
node tree, not the scene. The probe renders the actual lit result headless-ish (a window opens
for ~2 s — NOT `--headless`, which has no rendering driver and captures black).

```
godot --path godot --script proto/arena3d/_look_probe.gd -- \
    --out=/tmp/look.png --tod=0.2 --set=CONTRAST=1.2 --set=VIGNETTE=0.4
```

Key flags / modes learned the hard way:
- `--set NAME=VALUE` pokes a `LookSettings` (or `ArenaRenderData`) static before the scene builds.
- `--tod` sets time of day (0 = noon, 0.25 = golden hour, 0.5 = night).
- `--compare=KNOB=v1,v2,v3` renders several values **in ONE run from the same frozen frame**.
  This exists because `fake_sim` walks its monsters on a wall-clock delta, so two *separate* runs
  never share a layout — a fair A/B of a fine detail (ground normal, grass sat) is impossible
  across runs. Compare mode freezes `FakeSim` AFTER the warmup (freezing before it loses the bg
  art, which loads lazily off the sim's zone id) and shoots each value from the identical frame.
- Perf mode uncaps `Engine.max_fps` AND disables vsync before timing — with vsync on, every
  configuration reports one refresh interval and the A/B says everything costs the same.

**The probe cannot judge motion** (wind, cloud drift) — those need the running game. And its
layout differs run-to-run unless frozen. Both cost real time before they were understood.

Viewing: images are `cp`'d to `~/Desktop/ROI/` and `open`'d — SendUserFile does not render inline
in the Claude Code terminal.

---

## 1. Lighting & grade — `views/look_settings.gd` + `arena3d_scene.gd`

All the live knobs moved out of `const`s in `arena3d_scene.gd` into `LookSettings` (statics) so
they can be dialled in `debug_view`'s **"World look"** panel (backtick) against a running fight,
then printed ("Print values") and pasted back. A grade is judged by eye on motion; the slow loop
caps the search.

`_apply_look()` (arena3d_scene) polls a `LookSettings.snapshot()` array each frame and pushes on
change — one guard drives time-of-day AND every look knob.

### Current values (2026-08-25)

```
# sun / fill
KEY_ENERGY 1.35   AMBIENT_ENERGY 0.44   SHADOW_OPACITY 0.93
SUN_PITCH -34   SUN_YAW -38   NIGHT_DIM 0.35
SUN_COLOR (1.0, 0.97, 0.91)      # arena3d_scene const
FILL_ENERGY 3.0   FILL_HEIGHT 9.0   FILL_RANGE 26.0
# grade
CONTRAST 1.16   SATURATION 0.98   BRIGHTNESS 1.05
GLOW_INTENSITY 0.95   GLOW_BLOOM 0.04   GLOW_THRESHOLD 0.78
# depth of field
DOF_FAR_DISTANCE 26   DOF_FAR_TRANSITION 10   DOF_NEAR_DISTANCE 9
DOF_NEAR_TRANSITION 5   DOF_AMOUNT 0.12
# atmosphere / misc
VIGNETTE 0.48   ACTOR_EXPOSURE 0.82
CLOUD 0.45   CLOUD_SCALE 0.13   CLOUD_SPEED 1.2
GRID_ALPHA 0.0 (off)   GRASS_WIND 0.0 (off)   GROUND_NORMAL 0.0 (off)   GROUND_BUMP 0.0 (off)
# ground shader (ground_recolour.gdshader)
GROUND_SHADE 0.40   GROUND_SHADE_CEIL 0.60
```

### Techniques

**Luma compensation on the sun** (the single most important lighting fix). `daylight_color(t)`
can only *subtract* (it's a multiply), so a golden-hour tint takes most of the blue and a third
of the green — "warmer" and "darker" were the same knob, and a scene tuned bright at dusk blew
out at noon (white tint subtracts nothing). Fix: `KEY_ENERGY / luma` where `luma` is the Rec.709
luma of the tint. So `KEY_ENERGY` is the illumination at ANY hour and the tint decides only the
colour. The day/night dimming survives deliberately as `NIGHT_DIM`, applied after. Ambient gets
the same `/luma`. See `_apply_look`.

**Sun colour whitened** (0.93/0.80 → 0.97/0.91) so midday reads neutral-white, not orange; the
tint still warms dusk because it multiplies on top.

**Depth of field** — `CameraAttributesPractical` on the camera (Forward+ only; the project runs
`forward_plus`, NOT `gl_compatibility` as an old memory claimed). Distances are measured **in
view distance along the camera axis, not Z**: the play area sits at view-distance 16.5–24, so the
far band starts past 24 and the near band ends before 16.5 — no actor is ever out of focus, only
scenery. `DOF_*_DISTANCE 0` disables a band and skips building the pass (it is the most expensive
thing in the file).

**Vignette** — a quad parented to the camera (`vignette.gdshader`), fitted to the frustum, falloff
in UV so it follows the 16:9 frame. Not a CanvasLayer: the UI layers are numbered and there is no
slot between world and chat; a camera-child quad sidesteps ordering entirely.

**Ground fill light** — an `OmniLight3D` over the play area gives the flat floor plane light that
VARIES across it (a directional light hits every coplanar pixel identically — the sun cannot pool).
Shadows off (an omni shadow is a 6-face cubemap for a soft gradient). **Culled from the actor
sprites** via `light_cull_mask = 1` + sprites on render layer 2 — otherwise a character near the
pool centre caught an omni at point-blank and blew out.

**Actor exposure** — `actor_exposure` global uniform dims the actor sprites ONLY (in
`sprite_flash.gdshader`, which the player sprite now also uses). The sun tuned for the ground
over-lit pixel-art characters, which carry their own painted shading. Splitting the sun into two
lights was the alternative; a single dim uniform is cheaper and does not double the shadows.

**Grade** — `Environment.adjustment_*` contrast/saturation/brightness. The surprise: **saturation
goes DOWN** (0.98). A warm-lit reference frame is *less* saturated than the raw art — the light
supplies colour, the pigment only carries detail. Raising it turned grass acid-green.

---

## 2. Ground shader — `proto/arena3d/ground_recolour.gdshader`

The floor's art plane runs through this (the checker fallback is a plain StandardMaterial3D). It
accreted several passes, each a uniform gated at 0:

- **`recolour_on`** — the original one-colour live palette preview (pre-existing).
- **`shade_crush` / `shade_ceiling`** — shadows-only darkening of the ART before lighting. The
  reference's value range comes from its *darks* (brick joints, shade under foliage); ambient
  lifts every pixel equally and the painted shadows have the least to spare, so they wash out
  first. This multiplies dark pixels darker, leaves bright ones. Applied here (ground only) not
  as a full-screen pass so characters stay readable.
- **`normal_depth` / baked normal** and **`bump`** — relief. **Both currently OFF** (0): the
  raking relief read as the road going *darker* (facets tilt from the light), and flat-clean
  scored better by eye. The machinery stays for a future try. See §5.
- **cloud shadow** — `cloud_shade(v_world.xz)`, see §3.
- **`grass_wind`** — painted-grass sway, **currently OFF**. See §6.

`v_world` varying (world position from the vertex stage) is the shared key for cloud + wind.

---

## 3. Cloud shadow — `proto/arena3d/cloud_shadow.gdshaderinc`

Drifting dark patches across the WHOLE scene — ground, sprites, props alike. A cloud shadow that
only darkened the ground reads as a stain, not a cloud; the moment a character walks under one it
has to dim too.

- **Shared `#include`** — `cloud_shade(world_xz)` + value noise, included by `ground_recolour`,
  `sprite_flash`, and `prop_sway`. One function, every surface.
- **Global shader uniforms** (`cloud_strength/scale/speed`) declared in `project.godot`
  `[shader_globals]` and pushed once from `_apply_look` via
  `RenderingServer.global_shader_parameter_set` — a hundred materials read one value, no per-node
  wiring. Gated to 0 when `ArenaRenderData.FX_ENABLED` is false (per-frame animated → droppable).
- **World-space key**, so the patch on the ground and the darkening on a sprite standing there
  match, and the pattern stays put as the camera moves. Ground samples per-pixel (`v_world`),
  sprites per-node (`MODEL_MATRIX[3]`).
- Two octaves at unrelated speeds, biased so most ground is in sun and shadow is the passing
  exception.

⚠️ **`global_shader_parameter_get` / `_get_list` are editor-only** and return null/[] at runtime —
they lied during debugging and made a working feature look dead. `_set` works fine at runtime;
`_add` is editor-only and errors. Verify globals by pixel-diff, not by reading them back.

---

## 4. Colour / palette

**Per-colour palette table, never an HSL curve** (`tools/apply_palette.py` + a `src,dst` CSV).
The grass went teal → olive → vivid green across the pass; each retint was an explicit table
generated from the ORIGINAL `bg_r1.png` (backed up in scratchpad) so it never compounds. An HSL
curve bends different greens by different amounts and moves dirt and rock in opposite directions —
`ground_recolour.gdshader`'s header and the tool's header both warn about this. The green family
is isolated by hue+sat so road/dirt are simply not in the table.

Final grass hue landed as **vivid yellow-green** (H≈70–90, sat boosted) — the art-director note
was "cozy colourful, not dark"; the moody/olive direction was a wrong turn corrected back.

---

## 5. Grass (billboard clumps) — the biggest sub-pass

Region 1 grass lives in `zone_dressing_r1.tscn`. The clump asset is `grass_clump.tscn`
(`grass_clump.gd`, a MultiMesh of quads); the pack shader is
`assets/BinbunGrass/src/shader/grass.gdshader`.

**Placement: procedural → baked to hand-placed.** `grass_field.gd` (still in the repo) scatters
clumps procedurally with a green-mask (grows only on grass pixels of the bg, keyed on **hue+sat** —
the olive retint made grass and road both have r≈g, so the old `g>r` ratio test planted grass on
the road). It was then **baked** into 28 individual `GrassClump` nodes in the scene (a headless
script read the scattered positions/radii and wrote them as node instances) so they can be
dragged, deleted, and added by hand in the editor. `grass_field.gd` is kept for baking future
zones.

**Colour matches the ground.** The gradient stops were `sample`d directly from the retinted
`bg_r1.png`'s dominant grass greens, so the clumps read as the ground standing up, not a brighter
foreign object.

**Grass now follows time of day.** The pack's `void light()` shipped as `DIFFUSE_LIGHT += ndotl` —
a bare scalar that throws `LIGHT_COLOR` away, so the grass took brightness but never the sun's
tint and stayed vivid green while the scene turned blue at night. Fixed to
`DIFFUSE_LIGHT += LIGHT_COLOR * ndotl * 0.35` (the ×0.35 tames the full sun energy the pack scalar
never carried).

**Deep saturated roots.** `root_shade` (0.28) darkens the blade base via UV.y, and `root_sat`
(1.4) re-saturates it so the root reads as deep RICH green, not muddy grey (plain multiply drains
colour as it darkens).

**Grounding shadow — the billboard trade-off.** Documented because it cost several rounds:

| approach | shadow | grass look |
|---|---|---|
| full billboard + directional cast_shadow | **floats** (blade faces camera incl. pitch → mass lifts off ground) | full & lush |
| Y-billboard + directional | sits on ground | **flattens** (pitched camera sees the blade edge-on) |
| **full billboard + blob (chosen)** | anchored to base | full & lush |

Chosen: `cast_shadow OFF` + a **shade-pool blob** per clump — a flat radial-gradient dark quad on
the ground under the base, offset toward the shadow direction and stretched along it so it reads
as a cast shadow but is anchored to the foot by construction. `alpha 0.85`, shared material across
all clumps. This is what real HD-2D billboard grass does; a true directional shadow and a
billboard are incompatible without losing either grounding or lushness.

---

## 6. Tried and REVERTED (kept off, machinery retained)

These are off but wired, so re-enabling is a one-line knob change, not a re-implementation.

- **Ground normal / bump relief** (`GROUND_NORMAL`, `GROUND_BUMP`, `tools/build_ground_normal.py`,
  `bg_r1_n.png`). Baked a two-scale normal from the displacement/luminance. It gave the road
  per-stone relief but darkened the whole surface (facets tilt from the light) and flat-clean won
  by eye. Baked normal beats the in-shader `bump` when on. Off at the user's call.
- **Painted-grass wind** (`GRASS_WIND` in `ground_recolour`). Sways the bg's green pixels by
  offsetting their sample UV, **quantized to whole texels** (`round(wave*n)*texel`) so pixel-art
  grass jumps a pixel at a time instead of smearing, driven by `_cloud_vnoise` (sine waves read as
  marching diagonal bands). Turned off — even subtle it did not read well. The billboard clumps
  and props (`prop_sway.gdshader`) still sway; only the flat bg wind is off.
- **Cell grid overlay** (`GRID_ALPHA` 0). Removed because the regular ruled lines flatten the
  image — straight parallel lines tell the eye "flat plane". Kept as a knob; genuinely useful for
  laying out dressing by cell in the editor.
- **PBR / pixelated-PBR ground materials** (probe `--pbr`, scratchpad only). Real tiled textures
  (raw and downres+palette-quantized) were A/B'd against the painted road. The pixelated set with
  a normal derived from the displacement map read plausibly and, quantized to the road's own
  palette, matched — but it was a probe experiment, never wired into the scene. Nothing committed.

---

## 7. Cut-in mockups (not in-engine)

`ffmpeg` composites of splash-art (acolyte/gothic/knight) over a blurred scene render, with a
coloured accent band and the character bleeding off the right edge — concept art for a skill
cut-in. **Mockups only**, in `~/Desktop/ROI/`. A real cut-in would be a Godot CanvasLayer +
AnimationPlayer slide-in, not built.

---

## Files touched this pass

**New:** `views/look_settings.gd`, `proto/arena3d/vignette.gdshader`,
`proto/arena3d/cloud_shadow.gdshaderinc`, `proto/arena3d/grass_field.gd`,
`proto/arena3d/_look_probe.gd` (throwaway), `tools/build_ground_normal.py`, `assets/bg/bg_r1_n.png`.

**Edited:** `proto/arena3d/arena3d_scene.gd`, `arena3d.gd`, `ground_recolour.gdshader`,
`prop_sway.gdshader` (per-prop tint + cloud), `sprite_flash.gdshader` (cloud + actor_exposure,
now used by the player too), `player_node_3d.gd`, `base_monster_3d.gd` (layer 2 + shader),
`grass_clump.gd` (shade pool, cast_shadow off), `zone_dressing_r1.tscn` (baked grass, tree tint),
`views/debug_view.gd` (World-look panel), `views/render_data.gd`, `fake_sim.gd`,
`assets/BinbunGrass/src/shader/grass.gdshader` (LIGHT_COLOR, root shade/sat),
`grass_roi.tres` (ground-matched gradient), `project.godot` ([shader_globals]).

⚠️ `views/render_data.gd` was also touched by another session in the shared worktree (`@tool` +
`UI_CMDR_SMITE`) — stage carefully, do not `git commit -a`.

---

## 8. Building a whole zone — the recipe (2026-09)

Sections 0-7 above derive the base LOOK on a single screen. This is the layer on top: the
end-to-end steps to dress a whole ZONE, added in the 2026-09 pass. Studied against official
Octopath Traveler II shots — and the lesson from that study is the spine of this section:
**their "full, contrasty" frame is occlusion that is PAINTED AND PLACED, not a lighting ratio.**
A flat ground plane shrugs off key/ambient changes (measured — see §1); the depth comes from
baked shade, contact AO, and framing. ROI stays **cozy colourful pixel + bright anime**, NOT
Octopath's moody look, so every intensity below is deliberately ~half theirs. Borrow the
structure, not the darkness. (Related: the `[[continuous-walkthrough-rooms]]` memory — the
3-room strip + march this dresses.)

### The pipeline, in order

1. **Ground strip** — `tools/gen_grassland.py --seed N --region R --tmx ~/out.tmx`
   - Procedural grassland for a whole BAND (3 rooms), 108×46 tiles. Wangset corner auto-tiling
     from the ERW tileset; pick the wang **colour** deliberately (`WS_DIRT=("dirt",2)` = "to main
     grass"; colour 1 rings patches with dark grass). Features keep a ≥2-tile gap (butted
     transitions read as a hard seam).
   - `--back-pad 10`: extra plain rows BEHIND the cliffs so the camera never sees past the art.
   - Writes `art/bg_src/tiles_rR.png` (per-material tone-matched to `bg_r1` — a global transfer
     turned the road pink) AND `godot/assets/bg/bg_rR_s.png` (the SHADE MAP, step 4).
   - Then bake: `tools/build_bg.py R --raw` → `bg_rR.png`, and
     `tools/build_ground_normal.py R` → `bg_rR_n.png` (two-scale relief).
   - Flowers / tall grass are NOT baked into the ground — they are props (step 3).

2. **Camera + strip layout** (renderer, already built): a band's 3 rooms lay out left-to-right
   one field-width apart; a strip bg (width ≥ 3×`_gw`) is planted once at the band centre and the
   party marches between rooms. `BG_Z_SHIFT[R]` recentres the plane for the back-pad rows
   (r2 = -4.16 for 10 rows). Rooms scroll, camera pans, fade only at the band boundary.

3. **Dressing** — `zone_dressing_rR.tscn`, hand-placed, ONE instance, never moved/copied at
   runtime. Edit it in the Godot editor against the `EditorGround` strip + the three red/orange
   combat-field frames (freed at runtime).
   - STRIP RULE: the dressing repeats per room, so any prop whose **z is in [-5.2, 5.2] lands in a
     neighbour room's combat field**. Keep cliffs/scenery in the back row (front edge z ≤ -5.3);
     small plants may sit in the field.
   - Props: `cliff_box.tscn` (octagon-prism cliffs, live-tunable size), billboards with
     `prop_3d.gd` (tree/bush/barrel/well/flowers/tufts), `grass_clump.tscn` (reactive). Foliage
     colour is palette-mapped onto band 1's green ramp so borrowed art matches.
   - Foreground canopy hedge: ~25 crisp leaf-crown sprites along the bottom frame edge, jittered/
     overlapped, 3 shapes × 5 green tints, `offset ~20` so only the crown pokes in. Blurred by
     the near-DoF (step 5), NOT pre-blurred.

4. **Baked occlusion** (`shade_pass` in gen_grassland → `bg_rR_s.png`, multiplied in
   `ground_recolour.gdshader`, blend = `LookSettings.BAKED_SHADE`): ridge shadow cast INTO the
   field from the cliff line, far-row falloff, a dark foreground band, and low-frequency light/
   shade dapple (may brighten — sun pools). A separate map, so it tunes/toggles live (debug
   panel "Contrast kit" 0..1). This is the single biggest "fullness" lever — and the thing §1's
   lighting knobs alone could NOT give the flat plane.

5. **Lighting + optics** — values in §1's table; the zone-pass changes were the Octopath
   contrast tune (KEY 1.55 / AMBIENT 0.30 cool-blue / SHADOW_OPACITY 0.97 ≈ 5:1, half Octopath),
   vignette 0.65 / inner 0.38, and the near-field DoF for the foreground canopy: DOF_NEAR_DISTANCE
   15.5 / TRANSITION 1.5 / AMOUNT 0.13. **DoF distances are along the view ray** — the camera is
   13.5 up + 13 back, so a z~8.8 prop is ~14 units out, the field front ~17, the party ~20; the
   near band is tuned to blur only the canopy. Forward+ only.

6. **Contact shadows**: cliff foot-darkening + a ground skirt (`cliff_box`, intensities MATCHED so
   the junction blends, not steps); prop shadow pools (`prop_3d`, glow-soft); actor discs under
   every party member + monster (firm core, soft rim — `player_node_3d` / `base_monster_3d`).
   All built once, shared material, child of the node → zero per-frame cost.

### How to SEE a zone while building it
`proto/arena3d/dressing_preview.tscn` renders the dressed band under game lighting → one PNG per
room + overview in `~/Desktop/ROI/dress_rR_*.png`. Edit → run → read the images. NOTE: the preview
has NO post-FX — DoF/vignette/glow show only in the running game (§0's look-probe is the
single-screen equivalent that DOES include them). `LOOK_BEFORE=1` env → the pre-contrast lighting,
for before/after strips.

---

## 9. NOT done yet (the zone queue)
- **Region 1** is still single-room art (dressing teleports with the floor on a room swap). Give
  it a strip (`gen_grassland --region 1`) + a permanent dressing to move it onto this model.
- **Regions 3+** have no art at all (checker floor) — each needs the full §8 pipeline.
- Per-room dressing variety within a band (currently one scene spans all 3 rooms identically).
- Waterwell prop is ungraded; cliff far-shade side is a touch heavy.
- Reactive grass clumps only run in the room-1 third of a strip band.
- 2D fallback renderer lost mid-band fades (cosmetic, accepted while HD-2D is the direction).
- Day/night cycle (`views/day_cycle.gd`, another session) tints the whole thing — check a new
  zone's look at more than one TIME_OF_DAY (debug slider) before calling it done.

---

## 10. Sun-cue atmosphere + per-region look (2026-09-02)

Four atmosphere systems added toward the Octopath look, all **global systems** (not per-zone
dressing) so any zone gets them for free, and all **gated per region** so a zone opts in rather
than inheriting them everywhere. Each has a live debug slider in the "World look" panel and its
default in `look_settings.gd`. The dressing side got NOTHING here — these are code.

**The per-region gate.** `look_settings.gd` holds `SUNNY_REGIONS = {1: true, 2: true}` — regions
that get the SUN cues (god rays + wet ground + motes). `arena3d_scene.gd::_refresh_background`
computes the region on a zone change, sets `_sunny`/`_look_region`, and clears `_look_applied`
so `_apply_look` (which early-returns on an unchanged LookSettings snapshot) re-evaluates the
gate. Add a region to the set when its outdoor art is ready; a gothic/indoor region left off gets
none of it automatically. **Fog is separate** (see below) — it is opt-in per region via its own
table and is NOT tied to `_sunny`, because a non-sunny map can still want haze.

1. **God rays** — `godray_world.gdshader` + `_build_godrays`/`_fit_godrays` in `arena3d_look.gd`.
   A SPATIAL PORT of the cut-in's `views/cutin/cutin_godrays.gdshader` (same two-noise-band
   algorithm), rehomed onto a **camera-parented quad** exactly like the vignette — a CanvasLayer
   has no slot between the 3D world and the UI (the vignette header explains why). Sorts one
   render_priority UNDER the vignette so the frame darkens over the shafts and keeps focus centred.
   Knobs: `GODRAY` (strength), `GODRAY_ANGLE`, `GODRAY_SHARP` (a `pow()` that crushes the diffuse
   noise into standing shafts — without it the rays read as haze, not beams). FX-gated.

2. **Wet ground** — the wet block in `ground_recolour.gdshader`, pushed from `_apply_look`. Masks
   the road/cobble by **low saturation** (grass is green + saturated, stays matte), drops those
   pixels' ROUGHNESS, darkens them slightly, replaces the stone relief with a rippled water normal,
   and — the part that actually reads — adds an **explicit sun glint** to EMISSION (Blinn + a
   fresnel grazing term tinted with the sun colour). The BRDF alone shows nothing: a high sun
   reflects off flat ground back into the SKY, not down into a pitched camera, so the glint is
   built by hand and the grazing (fresnel) half is what reads from this angle — brightest on the
   far edge of a wet patch, the wet-road-receding look. Knobs: `WET`, `WET_ROUGH`, `WET_SAT`
   (mask cutoff), `WET_RIPPLE` (FX-gated shimmer), `WET_GAIN`.

3. **Ambient motes** — `_build_motes` in `arena3d_look.gd`, a `GPUParticles3D` on the camera. The
   floating dust/pollen that gives the air volume. Camera-parented so the cloud fills the view on a
   pan, but `local_coords = false` so a spawned mote drifts in WORLD space and parallaxes against
   the ground. Additive + warm so the sun catches them and glow blooms them. Slow (motion at ~20%
   of the first try — user found it too fast — with a long 16 s lifetime so they linger). Knob:
   `MOTES` (density, via `amount_ratio`). FX-gated.

4. **Atmospheric fog** — `_apply_look`, `EXPONENTIAL` env fog. **Per-region opt-in** via
   `FOG_REGIONS = {}` (region -> strength; empty, so no map fogs yet — region 2 was tried and did
   not suit it); `FOG` is a live slider override for dialling. Density = amount * 0.02, warm-cool
   pale colour, `fog_sky_affect = 0`. See the bug below for why it is exponential, not depth.

**`dressing_preview.gd` now renders the REAL `ground_recolour` shader** (it used a plain
StandardMaterial3D and so showed none of the shade map or wet sheen) and applies the same fog —
faithful to the game, and the only way the wet glint could be judged from a screenshot at all.

### Three silent bugs found this pass (all cost a render to catch, none errored)
- **`smoothstep(edge0, edge1, x)` with edge0 > edge1 is UNDEFINED in GLSL and returns 0 on Metal
  (macOS).** A descending-edge mask (`smoothstep(hi, lo, x)`) silently zeroed the entire wet mask.
  Fix: `1.0 - smoothstep(lo, hi, x)`, ascending edges only.
- **A `source_color` sampler decodes sRGB -> LINEAR, so saturation measured in the shader is not
  the saturation of the 8-bit art.** The road's sat was 0.13 in sRGB but **0.25 in linear**, above
  a 0.22 cutoff — the wet mask missed the whole road. Measure/choose shader thresholds in linear.
- **DEPTH-mode fog holds a flat level past `fog_end`**, and on a tilted camera the ground runs far
  past any end, so the far ground became a uniform pale sheet ("หมอกเต็มพื้นเท่ากัน"). EXPONENTIAL
  fog is asymptotic — always a smooth gradient, never a flat wall.

### Still not done (atmosphere queue, ranked)
- **In-scene warm point lights** (lantern / brazier / glowing well) — real light pools, big for
  evening/night with the day cycle. Medium effort.
- **Animated reflective water** on ponds/streams (the wet sheen is a taste; the well water is a
  flat blue disc). Medium.
- Fog wants a real zone to land in (region 2 declined it) — bake a `FOG_REGIONS` entry when a
  misty map exists.
- Everything in §9 still stands (regions 3+ art is the big one).
