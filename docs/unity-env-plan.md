# Unity arena — the ENVIRONMENT cluster (E1…E4)

## Why this cluster, and why now

Everything ported so far is LOGIC — the sim, the swing, the glide, the facing. That is the half
Godot never made hard. The open question (`docs/engine-decision-unity-spike.md`) is whether Unity
makes the VISUAL half easier, and no line of the port so far touches it. The environment is the
first work that actually tests the hypothesis, so it is worth doing before the decision, not after.

It is also the biggest gap: 38 of the ledger's 101 `missing` rows, plus 20 more in lighting.

**Record how long each step takes.** That number, not the screenshot, is what the engine decision
turns on.

## Ground rules

- **Generate, never type** (`tools/export_arena_tables.gd`). Region art, z-shifts, look values and
  prop placements come out of the Godot scene; a number typed into C# is a number that will drift.
- **Build it scroll-ready.** The room SCROLL (20 more `missing` rows) moves the ground per room and
  keeps the previous room's floor alive during the march. Ignoring that in E1 means rebuilding it.
- The ledger row is the definition of done: each step ends with rows flipping `missing` → `ported`
  with a note saying where, and `tools/port_ledger.py` still exiting 0.

## Steps

**E1 — ground + background.** One PNG per region (`assets/bg/bg_r<N>.png`) laid on a quad sized by
the SHARED pixel scale (`sprite_cells(w) × sprite_cells(h)`), nearest-filtered, offset by
`FIELD_Z_SHIFT` + the per-region `BG_Z_SHIFT`. Art ≥ 3 rooms wide is a band STRIP (centred on the
middle room, never moved by the scroll); anything narrower is a single-room floor. No art for a
region → the checker fallback, which is what Unity effectively has now.
Rows: `_floor _bg_zone _bg_cache _bg_is_strip _build_floor _refresh_background _bg_region BG_Z_SHIFT`.

**E2 — the frame around the field.** Outer ground plane (`OUTER_*`), cell grid overlay, backdrop
sky quad (`SKY_TOP/MID/HORIZON`). Cheap, and without it the field floats on the clear colour.

**E3 — lighting slice.** Sun pitch/yaw/energy, ambient, the fill pool (`FILL_*`) — the four that
decide whether a sprite reads as standing in the scene. Everything else in `LookSettings` (glow,
DoF, vignette, god rays, wet, clouds, motes, contrast kit) stays `deferred` WITH A GATE, not
quietly skipped: they are a look pass, and a look pass on an engine nobody has chosen is waste.

**E4 — dressing props.** The hardest and the most informative: Godot instances a hand-placed
per-region scene. The port is an EXPORT of that scene (position, scale, texture, react radius) into
the same generated table, then billboards on the Unity side — the same shape as `arena_tables`.
If this step is slow in Unity, that is the answer to the engine question.

## Log

**E1 — ground + background: DONE.** The region PNG, its size in cells, the per-region z-shift and
the strip flag are exported like everything else; Unity lays it on a quad with a NEGATIVE x scale,
because this scene mirrors world x to cancel its 180-yaw camera and a texture does not survive that
the way a point does.

Verified as numbers, not by eye: the dump now carries a `ground` and a `camera` row, and both
engines report ground `(x 0, z -6, 49.92 x 29.952)` and camera `(0, 13.5, 13) pitch -42 fov 42` —
identical. That mattered: the screenshots still look different, and the numbers say the difference
is NOT the background. It is the 3D grass clumps and hand-placed props (E4) plus the lighting (E3),
which is what the eye was actually reading. A picture would have sent this to the wrong fix.

**What the numbers could NOT catch (E1).** The ground row matched exactly while the art was still
UPSIDE DOWN: position and size are the same whichever way the UV runs. The user saw it in one
glance. So the acceptance pair is not ceremony — a measured value proves the geometry, and only the
picture proves the mapping. Flip in UV space, never with a second negative scale: two mirrored axes
turn the normal back down and the quad vanishes.

**E3 — lighting: DONE (a/world, b/actors).** Godot's model is three layers and only the middle one
was obvious: baked into the art (normal + occlusion maps, the ground shader's derived shade, wet,
cloud), the world at runtime (sun, a ground fill culled OFF the actors, ambient as colour x energy,
FILMIC tonemap + contrast/saturation/brightness + gated glow), and the actors at runtime (lit,
then dimmed alone by ACTOR_EXPOSURE). Porting only the world layer leaves every sprite unlit,
because a SpriteRenderer's stock material ignores lights — so E3b is a shader, which is exactly
what Godot needed too (`sprite_flash.gdshader`).

Two traps worth keeping: light a camera-facing quad by its own normal and sprites brighten as the
camera turns (pin the normal UP), and let a billboard receive its own cast shadow and half the
slimes render black (cast yes, receive no).

Still missing in this cluster: the daylight tint (TIME_OF_DAY x NIGHT_DIM), which drives the sun
and fill colour by the hour, and everything baked into the art.

**Oracle after E1+E3:** 22 mismatches over 117 rows — the 11 actor rows are unchanged from before
the environment work (no regression), and the other 11 are the new `ground`/`camera` rows going red
from tick 300 on, where Godot pans to the next room and Unity has no scroll. That is the same
cluster as parity defect 6, now visible directly instead of only through its side effects.

**E4a — dressing props: DONE.** Placement is exported from `zone_dressing_r<N>.tscn` through
`PackedScene.get_state()` (instancing a region hangs a headless run — the grass clumps build noise
textures and MultiMeshes on load and their @tool rebuilds are deferred). Props are lit billboards
with the authored tint, sized by Godot's `texture px x pixel_size` rule and pivoted at the foot.
Prop positions are in the dump now: both engines report Tree1 (-7,-6), Barrel1 (5,-5), Bush1 (2,1).

Three faults in a row, all the same shape — **a value with a spatial meaning cannot be copied
between these engines**: the background UV (upside down), the sun's Euler angles (mirrored in z as
well as x, so shadows fell the wrong way), and the dressing root's `FIELD_Z_SHIFT` (props two cells
out, which reads as "the map is offset"). Plain numbers copy; directions, pivots and anchors have
to be derived. All three now have a row in the dump.

**E4b — grass: DONE, and it is a REPLACEMENT, not a port.** BinbunGrass is a `.gdshader` plus
MultiMesh clumps that part where a walker passes; none of that crosses. The art comes from the
Idyllic Fantasy Nature pack (paid, gitignored) and only the LAYOUT is ported — the same 28 clump
positions, radii and seeds. Each clump is scaled to its exported radius by MEASURING the prefab's
bounds, because a prefab from another pack has no reason to be the size Godot's clump was and a
hand-picked scale would quietly stop matching the moment the prefab changed.

The ledger says `deferred: replaced by a Unity-native pack`, never `ported`. The two clients now
show DIFFERENT grass, and that is the honest entry.

**Brightness: measured, not judged.** The scene read as clearly darker than Godot's, and it was —
mean luminance over the same crop: **169.0 (Godot) vs 107.8 (Unity), 36% down**. Two causes, both
of them a value that cannot be copied between engines:

- **Tonemapper.** Godot uses FILMIC; ACES (the obvious pick) crushes the midtones. URP's *Neutral*
  is the match.
- **Light falloff.** Godot's omni fades as `1 - d/range`; URP's point light is inverse-square. At
  the pool's own height that is 0.65 against 0.012 — a factor of ~54. With `fill_energy` copied
  across, the ground fill contributed essentially nothing, which is exactly why the floor read flat
  as well as dark. The intensity is now converted through both attenuation models at the height the
  pool is FOR.

After both: **190.2 vs 169.0**. The remaining 12% is not lighting — the Godot frame has
BinbunGrass (dark blue-green) where Unity has the Idyllic pack (lighter, yellower), so the two
frames no longer show the same art. Tune it on the grass material, not on the lights.

`ARENA_SHOT_TICK=<n>` now grabs the Unity frame at a SIM TICK, matching the Godot shotter, so the
pair being compared is the same moment rather than two different fights.

**Why the sprites came out duller than the art (measured on the pink slime, same tick):**

| | sat | rgb |
|---|---|---|
| source PNG | 0.441 | 189, 123, 220 |
| Godot render | 0.436 | 255, 144, 255 |
| Unity, Neutral tonemap | 0.317 | 197, 139, 204 |
| Unity, tonemap off | 0.384 | 233, 150, 244 |

Lighting multiplies the albedo, so a bright painted pixel lands at or above 1.0. A tonemapper's
highlight roll-off then compresses the HIGHEST channel hardest — which is precisely the channel
that defines the hue — so `(max-min)/max` shrinks and the colour both dulls and shifts. Godot's
FILMIC at white 1.0 CLIPS instead: the pink's red and blue both peg at 255 and the artist's
saturation survives. Pixel art is authored at final saturation, so clipping is the behaviour that
respects it.

## Acceptance

The numeric oracle cannot see environment art — it compares actors. So E1–E4 are accepted on
(a) the ledger rows, and (b) a same-tick screenshot pair from both engines (`ARENA_SHOT_TICKS` /
`ArenaShot.Run`). Where a value CAN be measured — ground plane size and position, light angles —
add it to the dump rather than trusting the picture.
