# Castle scene — prop wishlist + Meshy prompt kit

For the prologue ruined-castle tower-climb (`proto/arena3d/castle_climb_proto.tscn`, see
`docs/castle-prologue-zone.md`). The plan (2026-09-04): a bounded ~1-month Meshy subscription to
batch the 3D hero props, re-textured in-engine to match the castle (triplanar stone/steel/wood,
nearest filter). 3D-model props FIT the castle because the castle is already 3D geometry (boxes/CSG),
unlike the billboard grassland.

## How to use this kit

1. In Meshy: **text-to-3D, LOW-POLY / game topology mode**, export **glTF (GLB)**.
2. Each prompt = a shared STYLE string + the prop subject. Keep the STYLE identical across every
   generation so the whole set reads as one cohesive style.
3. **Validate the pipeline on 2-3 props first** (armour + statue + one boss) — import to Godot,
   re-texture, check scale/tris/look — BEFORE mass-generating, so credits aren't burned on props
   that don't fit.
4. Re-texture in-engine (the AI texture is a preview only; we override it).
5. Tri budget: clutter 100-400, standard prop 400-1,200, feature 1,000-2,500, boss 3-4k. Decimate
   anything that comes in much higher.

### The shared STYLE string (paste into every prompt)

```
low-poly game asset, chibi proportions, Final Fantasy fantasy style,
clean simple geometry, flat cel-shaded solid colors, symmetrical,
neutral pose, isolated on plain white background, no ground, no base
```

### Negative prompt (if supported)

```
realistic, photorealistic, high poly, dense mesh, PBR, heavy normal maps,
noisy, scene, ground plane, pedestal, motion blur, text, watermark
```

## Priority 🔴 — HERO pieces (gen first; primitives can't do these)

Prompt = `<subject>, ` + the STYLE string above.

| Prop | Subject to put in the prompt |
|---|---|
| Knight armour stand | `an empty suit of ornate fantasy plate armour standing upright, crested helm, tall pointed pauldrons, layered chestplate with gold trim, short tabard` |
| Statue — knight | `a stone statue of a heroic knight holding a sword downward, weathered, chibi` |
| Statue — king/gargoyle | `a stone gargoyle statue perched, folded wings, weathered crypt stone` |
| Throne | `an ornate fantasy throne, tall carved backrest, gold and red cushion, chibi` |
| Chandelier | `a hanging iron chandelier with lit candles, circular fantasy design` |
| Ornate column | `an ornate carved stone column with a decorative capital and base, fluted` |
| Broken column | `a toppled broken stone column in pieces, ruined, cracked` |
| Weapon rack | `a wooden weapon rack holding swords and spears, medieval` |
| Sarcophagus / coffin | `a stone sarcophagus with a carved lid, crypt, weathered` |
| Standing brazier | `an ornate iron standing fire brazier bowl on a pedestal` |
| Banner / tapestry | `a large hanging heraldic banner, tattered crimson cloth with a gold emblem` |

## Priority 🟡 — atmosphere clutter (some already exist as pixel/primitive; lower priority)

`chains and shackles` · `a cracked stone urn / vase` · `an iron candelabra with candles` ·
`a pile of skulls and bones` · `a broken wooden table and chairs` · `a cobweb corner piece`.
(Rubble, crates, barrels already exist — skip in Meshy.)

## Priority 🟢 — foreground framing (near camera)

`a broken stone arch fragment` · `creeping vines and moss overgrowth clump` — placed at the front
strip like the grassland canopy hedge.

## Priority 🔵 — gameplay / story (later)

`treasure chest` · `stone altar / pedestal` · `glowing rune circle` · `iron portcullis gate`.

## Per-landing theming (gives the climb a narrative build)

- **Landing 1 (entry):** armour guards + weapon rack + banners
- **Landing 2 (mid, crypt):** statues + chandelier + sarcophagus
- **Landing 3 (top, throne room):** throne + a grand statue + altar — the peak / boss arena

## Boss (separate list — 3D + rig)

The castle's final boss (giant knight / living gargoyle / dragon). Generate WITH rigging (Meshy
Animate supports biped/quadruped/**winged** — dragons/griffins), A/T-pose, then re-texture. Keep
~3-4k tris for the mobile-shaped target.

## Integration (Claude does this per batch)

Godot imports glTF natively. Per prop: instance the MeshInstance, `material_override` a triplanar
castle material (stone/steel/wood, nearest filter), match scale + the scene lighting, place in the
back/foreground strip (out of the combat lane, |z| >= 5.3). A `_model(path, material, scale)` helper
will be prepped so a fresh glTF drops straight into the scene.
