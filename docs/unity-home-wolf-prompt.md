# Tartarus wolf watermark

Date: 2026-09-12  
Mode: built-in `image_gen__imagegen`, one generation using the previous fantasy Cerberus as a style reference.  
User direction: replace Cerberus with one side-facing fantasy wolf.

## Asset and provenance

- Project output: `unity/ROI/Assets/Resources/HomeUI/TartarusWolfEmblem.png`
- Generated source: `/Users/lunalieneray/.codex/generated_images/01a092db-e5f5-7211-9d8f-bab9f4013f36/exec-25490757-7448-4034-b5fb-8783ab278998.png`
- Style reference: `unity/ROI/Assets/Resources/HomeUI/CerberusEmblem-Fantasy.png`
- Source copied unchanged: 1254 × 1254 RGBA PNG.
- Import metadata: new GUID, same uncompressed default platform / max 1024 / trilinear / clamp / mipmaps / alpha-transparency settings as previous watermark.
- Read-only alpha inspection: range 0–255; 69.1% alpha-zero pixels; common interior alpha 253–254; corner alpha zero. The single long muzzle points right.
- Source RGB is black. The alpha-only `HomeEmblem.shader` supplies the rendered tint.
- Original Cerberus assets are retained. No manual pixel editing, generation CLI, or script-based background removal.

## Full prompt

```text
Use case: logo-brand
Asset type: original fantasy game UI alpha-mask watermark for Tartarus Excavation.
Input image: STYLE REFERENCE ONLY for flowing ancient-fantasy heraldic shapes. Do not preserve its three heads or frontal arrangement.
Primary request: create ONE SINGLE WOLF HEAD IN STRICT SIDE PROFILE, FACING RIGHT. One muzzle pointing to the right, one visible profile eye, long pointed ear, proud elegant neck and flowing mane sweeping down and left. Clearly a wolf, not a fox, dragon, lion, three-headed creature or human. No frontal head and no extra head.
Art direction: ancient fantasy kingdom heraldry; a noble mythical guardian carved as a graceful sigil. Use two or three broad flowing flame-shaped mane locks with a restrained decorative ancient collar curve integrated into the lower neck. Curves should feel hand-designed and regal rather than an aggressive modern sports logo.
Composition: centered isolated emblem, profile head in upper right and flowing tapered mane in lower left, filling about 82 percent of square with transparent margin. Cohesive bold silhouette, strong clean negative-space cuts for eye, inner ear, jaw and the large mane divisions. Legible when small as a 200-pixel low-opacity UI watermark.
Rendering: perfectly solid BLACK flat shapes on a genuinely transparent background. The UI shader uses only the alpha channel and supplies a white tint. All background and interior negative spaces must be alpha zero. Smooth crisp antialiased edges.
Avoid: circle or crest border, shield, frame, rectangular tile, lettering, runes, numbers, floating stars, tiny filigree, stipple, furry microtexture, metallic gradients, grey shading, shadows, glow or distressed scratches.
Output: actual RGBA transparent PNG. No painted white, grey, black or checkerboard backdrop. Exactly one right-facing wolf head.
```
