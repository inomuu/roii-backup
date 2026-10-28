# Scene brief — grassland starting zone (concept target)

> For handing a zone to an outside image/design tool. Reusable: change §2 for another zone.
> Written 2026-09-08 for a GPT-6 Astra run. **Paste this whole file. Attach the two references
> described in §7.**
>
> The output of this brief is a CONCEPT TARGET — an image we aim at. Shipping art is authored
> afterwards from material we own; nothing generated here goes into the build as-is.

## 1. What we want back, in order

1. **One generated scene image.** The look, the light, the layering. This is the thing we judge.
2. **The lighting you used, written down** — key direction and colour, ambient/shade colour,
   contrast, fog, depth of field, vignette. We rebuild the grade in-engine from your numbers,
   so state them rather than leaving them inside the picture. **Propose what the scene needs;
   do not feel bound by what we run today (§6).**
3. **A composition plan** — what sits where on the 20 × 10 play grid to get that image:
   foreground framing, edge dressing, background. Coordinates.
4. **A prop list** — every distinct object the scene needs, with a one-line description each.
   We have a stock library and can generate the rest (§7), so do not restrict yourself to
   what exists.

Do not produce tilesets, sprite sheets, or engine scene files. They cannot drop into this
pipeline (fixed grid, base-pivot billboards) and we would only rebuild them.

## 2. The scene

The **first grassland zone a new player sees**. Daylight. This is the opening of the game, so it
should read as inviting and legible before it reads as impressive.

Tone: **hold the Octopath Traveler tone, exactly.** Warm and inviting, but RICH — deep shadow,
high contrast, restrained saturation. It is not a bright flat sunny picture: Octopath's daylight
scenes still have dark corners, and the colour sits in a narrow, controlled range rather than
being turned up.

Two things this rules out, and both are the failure mode to avoid:

- **Do not push saturation.** "Colourful" here means a well-chosen limited palette, not vivid.
  An oversaturated grassland is the single most likely way to miss this target.
- **Do not flatten the light** into an evenly-lit cheerful field. The mood comes from the
  contrast between warm light and cool shade (§4), and an evenly bright scene throws it away.

The game's world overall is warm and charming rather than grim — but that is the world's
character, not a licence to raise brightness or saturation in the render.

## 3. Style

**Octopath Traveler HD-2D.** Pixel-art sprites and props standing as billboards on real 3D
ground, lit with real 3D lighting, heavy depth of field, strong vignette, warm key light against
cool shadow. The pixels stay crisp; everything around them is soft.

The three-layer read matters more than any single element:

- **Foreground** — out-of-focus framing elements at the very front (overhanging leaves, tall
  grass), darkened by the vignette.
- **Play layer** — the sharp, readable middle where the fight happens.
- **Background** — receding, desaturated by fog, soft from depth of field.

## 4. Cinematography and colour discipline

This is not decoration on top of the scene — it is how the scene is judged. A picture that gets
the props right and this wrong is not usable.

**One grade governs everything.** The look is authored once, at the scene level, in the values in
§6. Individual objects are never recoloured to "fix" how they sit. If foliage looks wrong, it is
tinted INTO the grade; the grade does not bend around it. Per-object colour correction is how a
scene ends up with fifty local decisions and no overall look, and it is explicitly not how this
project works.

**Warm against cool is the backbone.** A warm key light and a cool blue shadow side, held across
the whole frame. That complementary split is what makes a flat pixel prop read as a solid object
standing in real light, and it is the single most recognisable thing about the target style.

**Value structure decides readability.** Read the composition in greyscale: the play layer must
be the highest-contrast band in the picture. Foreground framing goes dark, background goes pale
and compressed. A character standing anywhere on the field must separate from what is behind
them by VALUE, not only by hue — a player squinting at a busy fight is reading light and dark.

**Saturation is a budget, not a default.** The ground and dressing stay slightly restrained so
that characters, damage numbers and spell effects are the most saturated things on screen. If the
grass competes with a fireball, the grass is wrong.

**Depth is built from stacked cues, not one.** Fog, depth of field, value compression and
diminishing detail all push in the same direction, so distance reads even in a still frame.

**The frame leads the eye to the centre.** The vignette darkens the corners; foreground framing
elements close in from the edges. Both exist to point at the open middle where the fight happens.
Composition should reinforce that, not fight it.

**Tonemapping is neutral on purpose.** The look is authored in the lighting and the grade, not
baked in by an aggressive tonemapper, so it stays adjustable in one place.

## 5. Hard constraints

These are not preferences. An image that breaks them cannot be built.

**Camera**
- Perspective, **fov 42°**, pitched **42° downward**, fixed — it never rotates or follows.
- **NO SKY IN FRAME.** The camera looks down at the ground plane. The horizon is above the top
  edge of the picture. The far distance is more ground, fading into fog — not a skyline.

**Where the play field sits in the frame** (measured from the live camera, 16:9)

The grid numbers below mean nothing on their own, so here is what the picture actually looks
like. Ground fills **100% of the frame** — top edge to bottom edge, no sky anywhere.

```
 ── top of frame ────────────────────────────────
   0–8%    distant ground beyond the field, washing out into fog   ← background
   8–25%   the far end of the play field; a MID-GROUND band belongs here (see below)
  25–75%   the play field proper — where characters stand and fight
  ~59%     the middle of the field is about here
  75–100%  near ground, out of focus, darkened by the vignette      ← foreground framing
 ── bottom of frame ─────────────────────────────
```

- The frame is **a window that pans horizontally**, not the whole field at once. Do not compose
  a picture that only works if all 20 cells are visible together.
- Perspective spread: the far end of the field is roughly **twice as wide** in world units as the
  near end, so edge dressing thins out fast as it comes toward the camera.
- **KNOWN GAP, and the most valuable thing you could solve.** There is currently nothing in the
  MIDDLE DISTANCE — the field ends and the backdrop begins. Tilting the camera up to show more
  was tried twice and reverted both times, because a shallower angle only bought blank ground:
  there is nothing up there to reveal. What the scene needs is a band of scenery between the
  play field and the backdrop that gives the eye somewhere to go. If your composition answers
  that, say so explicitly.

**Play field**
- The fight happens on a **20 × 10 cell** field — twenty wide, only **ten deep**. It is a shallow
  stage, not an open landscape; depth has to be built in the layers around it, not inside it.
  Every zone in the game is this size.
- The party enters from the **LEFT**, monsters walk in from the **RIGHT**.
- **The centre must stay open.** It is the combat space. No prop may sit in it, and nothing may
  obscure a character standing anywhere on the grid. Dressing lives at the edges, in the
  foreground frame, and in the background.
- Ground plane is **49.92 × 29.952** world units.

**Sprites**
- Characters and most props are **2D billboards standing on 3D ground**.
- A prop's pivot is at the **BASE** of its image, always — it stands on the floor, it does not
  float.

## 6. Lighting and grade — what we run today, OPEN to a better proposal

These are the live values, given so you know where we are standing — **not a specification to
match.** If the scene wants a different key angle, a different shade colour, more or less fog,
say so and give the numbers (§1.2); we would rather re-grade the engine than force your image
into an older look.

What is NOT open: one grade governs the whole scene (§4), and per-object colour correction is
not how this project works.

| | value |
|---|---|
| Sun | pitch **−34°**, yaw **−38°**, energy **1.55**, colour `(1.00, 0.97, 0.91)` — warm |
| Ambient (shadow side) | energy **0.30**, colour `(0.26, 0.33, 0.52)` — cool blue |
| Fill light | energy 3.0, 9.0 units above the field, range 26.0 |
| Shadows | opacity **0.97** — near-solid, not soft |
| Tonemap | contrast **1.16**, saturation **0.98**, brightness **1.05** |
| Fog | exponential, colour `(0.74, 0.79, 0.84)` — pale, faintly cool, reads as distance |
| Glow | intensity 0.95, threshold 0.78 |
| Depth of field | blurs past **26** units and nearer than **15.5**, amount 0.13 (pixel art smears fast, so it is deliberately restrained) |
| Vignette | 0.65 — Octopath-style framing |
| God rays | 0.22, tilt −0.35 rad |

In words: **warm sun, cool blue shade, high contrast, dark corners, distance washing out pale.**

## 7. Props — not a constraint

We already own a stock library (bushes, crates, barrels, trees, signs, buckets, planks, flowers,
grass tufts, water wells, lamp posts, foreground canopy leaves), plus real 3D grass clumps and a
wind-sway shader. It is there if it fits.

**But do not design around it.** Anything the scene needs that we do not have, we can make —
2D props can be generated, and hero props go through a 3D model pipeline we already use. So list
what the scene NEEDS (§1.4) and let the sourcing be our problem.

Two shapes we can take a new prop in:

- **2D billboard** — transparent PNG, pivot at the BASE of the image, lit to match the scene's
  key direction. This is what most dressing is.
- **3D model** — low-poly, for a hero prop that has to hold up close or catch real light.

## 8. The two references attached

1. **A screenshot of our game as it stands.** This is the ground truth for scale, camera and what
   the play field actually looks like. Match its framing, not its polish.
   ⚠️ Do **not** sample colour from it: the current ground texture was built from an ungraded
   source set and does not match §6. §6 is the truth.
2. **An Octopath Traveler reference.** This is for the STYLE — layering, light, depth, framing —
   and nothing else. Do not reproduce its content, its assets, or any recognisable location.
   What we want from it is the treatment; the scene must be our own.

## 9. Acceptance, once the plan comes back

- The placement plan opens in `dressing_preview.tscn`
  (`Godot --path godot proto/arena3d/dressing_preview.tscn`) and renders.
- Nothing blocks the lanes a character walks.
- Any new 2D asset: transparent PNG, pivot at the base, size checked with `sprite_cells()` (not
  guessed from `CELLS_PER_ART_PX`), texture downsized before it is committed. A 3D model: low-poly,
  textures downsized, checked under the scene's own lighting rather than a neutral viewer.
- If the grade moved, the new values land in `look_settings.gd` — one place, not per object.
- `tools/smoke.sh` passes — both the 2D and the HD-2D scenes must still parse and boot headless.
