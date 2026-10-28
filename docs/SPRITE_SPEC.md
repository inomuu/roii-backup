# Sprite Spec — Characters & Hat Cosmetics

For the art pass. Goal: **layered hat cosmetics** — one hat sprite overlays any
character without redrawing the body. Works because the head barely moves, so a
hat can sit at a near-fixed spot.

> **The trick:** hats are painted on the **same 100×100 frame grid as the body**,
> aligned to where they sit on the head. The game draws `body` then `hat` at the
> **exact same rect** — so there's no anchor math. The artist just paints the hat
> over a copy of the character frame as a guide.

---

## 1. Character body sheets

- **Frame size: 100×100 px**, transparent PNG sprite sheet (current format — keep it).
- Existing per-class layouts are fine (archer 12×6, mage 15×10, etc.). Don't need
  to standardize the sheet grid — only the **head position** matters.
- **Bare / clean head** — NO headgear baked into the body (no helmet/cap/hat).
  Hair is OK (a hat sits over hair), but keep the **top of the head clear** so a
  hat can rest on it.
- **Head stays put.** Across ALL frames the head should sit at the **same pixel
  position** within the 100×100 frame (little/no vertical bob). A few px of bob is
  tolerable; big head movement breaks the fixed-hat overlay.
- **Same head position ACROSS ALL CLASSES** (strongly preferred): head centered
  horizontally, head-top at the same Y in every class's frame. Then **one hat fits
  every class**. If a class must differ, flag it (the dev adds a per-class offset).
- **Facing:** draw facing ONE direction (right). The renderer flips horizontally
  for left — so keep the design flip-friendly (no text/asymmetry that breaks mirrored).
- **⚠️ Archer bow:** the bow is baked into the archer sheet and currently crosses
  the head. **Re-pose the bow so it never overlaps the head/hat zone** (hold it
  lower or angled away). Otherwise a hat would clash with the bow. (Splitting the
  weapon onto its own layer is a LATER step for weapon skins — not now.)

**Deliver per class:** the clean-head body sheet. (Optional: a "with default hat"
version for reference / the default look.)

## 2. Hat sprites (cosmetics)

- **100×100 px**, transparent PNG, **same coordinate space as a body frame.**
- Paint the hat where it sits on the head — use a character frame as a backdrop
  guide, then delete the body so only the hat remains on the 100×100 canvas.
- One frame per hat is enough (head is static). If you want bob-matched hats later,
  a multi-frame hat sheet can drop in — not required for v1.
- Keep the hat within the head zone; transparent everywhere else.
- Flip-friendly (the renderer mirrors it with the body when facing left).

**Deliver:** **2–3 hat mock-ups** (e.g. a simple cap, a wizard hat, a helm) so we
can wire + test the overlay end to end.

## 3. Technical

- PNG, 32-bit (alpha), transparent background.
- **Pixel-art: hard edges, NO anti-aliasing / soft edges** (the game samples
  NEAREST and upscales — soft edges turn to mush).
- Match the existing palette / outline style so hats read as the same art.
- No padding offsets — frame content aligned to the 100×100 cell as the body is.

## 4. Deliverables checklist

- [ ] Clean-head body sheet per class (swordsman, archer, mage, thief, acolyte)
- [ ] Archer sheet re-posed so the bow clears the head zone
- [ ] 2–3 hat mock-ups (100×100, aligned to the head)
- [ ] (optional) "with default hat" reference per class

## 4b. Floor tiles (tilemap)

- **Tile size: 20×20 px** (matches the ~16×19 character → 1 tile = 1 grid cell =
  ~1 character footprint; same pixel density as the sprites).
- One **TileSet** of 20×20 tiles; the game fills each zone's grid from it
  (grids run 32×16 up to **44×20** cells — author tiles, not whole maps).
- Make **several variants per theme** (4–8) so the floor doesn't read as one
  repeated tile. Themes by band: Grassland · Forest · Cavern · Ruins · Volcano ·
  Abyss. Edge/transition tiles optional (nice-to-have).
- Same technical rules as sprites: transparent where needed, **pixel-art hard
  edges (no AA)**, **import NEAREST + no mipmaps**, matching palette/style.

## 5. How the game uses it (for context)

`player_animator` draws, per frame, in order: **body → hat (if equipped)**, both at
the same screen rect, both flipped together for facing. A `hat` cosmetic slot on the
character picks which hat sheet to overlay. (Weapon stays baked for v1; a separate
weapon layer comes with weapon skins.)
