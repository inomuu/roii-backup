# Cerberus watermark asset

Date: 2026-09-12  
Purpose: Home menu Tartarus / Excavation card watermark.  
Mode: built-in `image_gen__imagegen`; no fallback CLI and no manual pixel editing.

## Selected asset

- Project asset: `unity/ROI/Assets/Resources/HomeUI/CerberusEmblem.png`
- Source output: `/Users/lunalieneray/.codex/generated_images/01a092db-e5f5-7211-9d8f-bab9f4013f36/exec-afc6afff-16cf-44b8-99ea-08a838f1c821.png`
- Original size: 1254 × 1254, RGBA PNG, copied without modifying pixels.
- Unity import: maximum size 1024, uncompressed default platform, mipmaps enabled, clamp, trilinear, alpha is transparency.
- The selected image is black with transparent negative spaces. `HomeEmblem.shader` reads alpha only and supplies the UI tint, so source RGB does not determine the rendered color.
- Alpha verified read-only: range 0–255; 64.5% fully transparent; 0.6% fully opaque; most nonzero interior values are 253–254. Corner alpha is zero. No opaque rectangle behind the emblem.
- Initial white mask contained noisy alpha around cuts. A cleanup produced clean artwork with an unwanted baked checkerboard; that intermediate was rejected. The selected black mask edit restores actual transparency and cleaner broad shapes.
- This is an original generated emblem, not an extracted branded game asset.

## Initial generation prompt

```text
Use case: logo-brand
Asset type: original game UI watermark emblem, a square 1024x1024 transparent PNG to place behind the Tartarus Excavation title on a cyan card.
Primary request: a cool, confident, minimalist heraldic Cerberus three-headed hound emblem.
Subject: exactly three canine heads sharing one unified neck-and-chest silhouette. The middle head looks directly forward, noble and intimidating, with pointed ears and a broad strong muzzle. A left head and a right head face outward in crisp side profiles. Clearly canine, no human faces, no dragon. Balanced near-symmetry and elegant angular geometry.
Style/medium: flat graphic ink silhouette with sharp, clean, generously sized white shapes and transparent negative-space cuts for eyes, jaw separation and neck structure. Sophisticated fantasy heraldry, restrained and bold; broad forms designed to remain legible when displayed at 200 pixels as a subtle watermark.
Composition/framing: centered isolated emblem fills about 85 percent of the square. Leave clean transparent margin around the emblem.
Color palette: solid pure white emblem shapes, genuine transparent background. Every area outside the white emblem and all interior negative spaces must have alpha zero.
Constraints: create an original cohesive mark; do not copy any existing branded Cerberus emblem. No text, lettering, numbers, crest border, circle, frame, stars, ribbons, glow, shadows, lighting gradients, fur texture, metallic rendering or illustration shading. Not an app icon tile.
Output requirement: REAL transparent PNG with alpha channel. No white, black, grey or checkerboard background rendered into the image. The ONLY visible pixels are the white flat emblem with clean antialiased edges.
```

Initial output: `exec-9d2ea6d5-1a12-4621-ba78-f55cf370f6b4.png` in the source output directory above.

## Shape cleanup prompt

```text
Use case: precise-object-edit
Edit target: the supplied white Cerberus emblem PNG.
Primary request: clean this SAME emblem into a pristine flat solid-white UI symbol. Retain its three heads and overall heraldic arrangement: center head frontal, side heads looking outward.
Change only: REMOVE ALL distressed speckles, grunge, crumbs, thin scratchy hairlines and detached white artifacts. Simplify the interior cuts into a few broad, confident negative spaces. Make the white areas completely opaque, homogeneous, and pure white. Make the outside region and interior negative spaces completely transparent with alpha zero. Edges must be smooth, sharp and clean with only a narrow antialias edge.
Design quality: refined modern angular fantasy heraldry, thick readable geometry, a strong central muzzle and cheek planes, recognizable at 200 pixels.
Preserve: overall three-headed hound composition and centered square format. No decorative additions.
Output: actual transparent PNG, not a white or black or grey or checkerboard background. No text, frame, texture, shadow, glow or gradients. Transparent cutouts and solid white silhouette only.
```

Input: initial output. Output: `exec-59123523-eb7e-4d34-a7cf-34f2c502cfcb.png`. Not used directly because it has an RGB checkerboard.

## Rejected white extraction prompt

```text
Use case: background-extraction.
Remove the entire grey checkerboard backdrop from this white Cerberus emblem and deliver an actual RGBA transparent PNG cutout. The checkerboard is an unwanted baked-in background in the source; it must not appear at all in the result.
Preserve exactly the clean white three-headed hound shape. Keep the white shape opaque. Make every grey checkerboard area outside and inside the emblem into fully transparent alpha-zero pixels, including gaps between heads, eyes and neck ribbons. No redraw, no shading, no additional objects, no text. Actual transparency is essential because this is a game UI mask.
```

Input: shape cleanup output. Output: `exec-5d9daa7d-0c05-4e90-8cb2-c6effafd0c77.png`. Not selected because the white alpha extraction produced speckled interior cuts.

## Final black alpha-mask prompt

```text
Use case: precise-object-edit.
Make the attached three-headed Cerberus emblem a clean solid BLACK silhouette on a genuinely TRANSPARENT background.
The input's grey checkerboard is a baked-in unwanted background. Remove it completely; use actual PNG alpha transparency. All the source white shapes become perfectly opaque solid black. Every source grey/checkerboard area outside and inside the emblem becomes empty transparent alpha-zero space.
Keep the same composition: large central front-facing hound head, two outward profile hound heads, shared pointed chest. Preserve the clean broad negative spaces in the source. No speckles, distressed texture, stray pixels, wisps, soft edges or gradients. Only flat smooth solid-black emblem shapes with crisp antialiased boundaries, and actual transparent empty spaces.
This is an alpha-mask asset for a game UI, so rendering a checkerboard or any colored/white/black background invalidates it. Output a PNG with actual transparency.
```

Input: shape cleanup output. Output: selected source above.


## Fantasy heraldry revision — 2026-09-12

User asked for a more fantasy Cerberus mark. This sibling revision introduces flowing flame-like mane forms, a more ornate ancient collar and integrated curved flourishes while retaining exactly three canine heads.

- Mode: built-in `image_gen__imagegen` edit.
- Edit target: project `CerberusEmblem.png`.
- Output source: `/Users/lunalieneray/.codex/generated_images/01a092db-e5f5-7211-9d8f-bab9f4013f36/exec-e1a90953-2c79-4611-9e53-a81459439632.png`
- New project asset: `unity/ROI/Assets/Resources/HomeUI/CerberusEmblem-Fantasy.png`
- Original preserved unchanged.
- Output is copied unchanged at 1254 × 1254 RGBA; same Unity import settings with a new GUID.
- Read-only alpha validation: 66.1% fully transparent; common solid interior alpha 253–254; corner alpha zero. Source RGB is black, rendered color comes from the existing alpha-only UI shader.
- No pixel editing, background removal scripts or CLI generation were used.

### Fantasy edit prompt

```text
Use case: style-transfer
Asset type: fantasy game UI watermark emblem, transparent PNG alpha mask.
Edit target: the supplied three-headed Cerberus silhouette. Evolve its art direction into ancient fantasy underworld heraldry for a Tartarus excavation banner.
Primary request: a more mythical, regal Cerberus with exactly three elegant canine heads, one central frontal head and two outward-facing profile heads. Add generous flowing flame-shaped mane locks, an ornamental ancient collar with a single simple diamond clasp, and restrained sweeping arcane flourishes integrated into the shared neck silhouette. Long graceful tapered curves should make it feel like an antique fantasy sigil engraved for an underworld kingdom.
Composition: centered balanced triangular crest silhouette; all three heads unmistakable. Heads occupy upper two-thirds; tapered flowing mane and broad collar occupy lower third. No separate floating ornaments. Clear broad negative-space cuts around jaws, eyes, necks and flame mane. Keep design readable as a 200-pixel UI watermark.
Style: rich fantasy heraldry expressed through clean solid graphic shapes. Elegant organic sweep and deliberate ornament, with bold weight; avoid a modern sports-team mascot or aggressive e-sports angular badge. No circle, border, shield outline, rectangle, lettering, runes, tiny stipple, hair texture or microdetail. Do not add a fourth head or human face.
Color: perfectly opaque solid BLACK emblem shapes with true TRANSPARENT alpha-zero empty spaces. No tonal rendering, no gradient, no grey fill, no lighting, no shadow, no glowing eyes. The UI will recolor the alpha mask.
Output: a genuine transparent PNG. The entire backdrop and interior negative spaces must be alpha zero. No checkerboard or white or black backdrop painted into the image. Preserve a clean margin around the full emblem.
```
