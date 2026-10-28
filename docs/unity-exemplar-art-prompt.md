# Exemplar menu background generation

Date: 2026-09-12  
Mode: built-in `image_gen__imagegen`, one original generation with no image references.

## Selected background

- Project asset: `unity/ROI/Assets/Resources/ExemplarUI/CelestialBackground.png`
- Generated source: `/Users/lunalieneray/.codex/generated_images/01a092db-e5f5-7211-9d8f-bab9f4013f36/exec-68297f35-650c-44b9-b674-382be75ce62f.png`
- Source copied unchanged: RGB PNG, 1672 × 941, approximately 16:9.
- Unity import: sRGB, maximum size 2048, no non-power-of-two rescaling, bilinear filtering, clamp, no mipmaps, uncompressed default and standalone platform settings.
- This is a background plate only: no character, UI, lettering, buttons or icons. Existing character assets are composed separately in Unity.
- Art direction: muted navy and slate celestial atmosphere, sparse distant stars, silver-blue lower horizon and subtle reflective floor. Dark left side leaves space for stats; center and right remain quiet for the character and roster.
- No manual pixel editing or image-generation CLI used.

## Full generation prompt

```text
Use case: stylized-concept
Asset type: original 16:9 landscape background plate for a fantasy RPG character-management menu. Background ONLY.
Primary request: an elegant restrained celestial chamber made of muted navy and slate-blue atmosphere. A quiet seamless dark space with gentle deep-blue haze and a faint silver-blue horizontal glow near the lower third. A subtle curved floor plane reflects the soft horizon glow, gradually fading into a slightly luminous gray-blue foreground. The upper two thirds remain a calm dark navy. Sparse very tiny distant stars only, widely separated and barely visible.
Composition: wide 16:9 with generous low-contrast negative space across the entire frame. Leave left third especially calm and dark for character stats. Center is empty for an overlaid character. Right edge stays quiet and unobtrusive for a roster selector. Horizon at about 70 percent down from the top. Depth comes from soft atmospheric falloff, not structures.
Style: refined original sci-fantasy game menu atmosphere, subtle cinematic color transitions, polished and immersive but restrained. Smooth tonal gradients with faint floor reflection; no busy detail.
Color palette: desaturated midnight navy, slate, cool gray-blue, a modest silver-blue glow. No saturated violet/cyan explosions.
Constraints: no character, figure, person, creature, silhouette, portrait, text, letters, numbers, logo, watermark, UI panel, icons, buttons, rings, circular portals, runes, ornate architecture, pillars, machinery, bright colorful nebula blobs, dense sparkles, or obvious focal object. No borders. Do not imitate a specific game's exact background artwork.
Output: a clean 16:9 full-bleed landscape image, ideally 2048x1152 or comparable widescreen resolution.
```


## Medium celestial variant — approved WuWa / Epic Seven / HSR blend

Date: 2026-09-12. Built-in imagegen edit, original dark plate preserved.

- Source: `/Users/lunalieneray/.codex/generated_images/01a07f7f-7daf-7373-8b23-4b66a2bdea71/exec-1c702385-6b13-4fa3-b565-ded7e6880862.png`
- Active project plate: `unity/ROI/Assets/Resources/ExemplarUI/CelestialBackground-Medium.png`
- The original dark plate remains available. Same uncompressed sRGB/clamp import settings.
- Approved direction: characters have varied colors and multiple artists; the UI remains restrained, neither very dark nor very bright. Medium slate and ivory surfaces support artwork without grading the character.
- Animation is added in Unity, not baked into video: `CelestialMenuMotion` owns one local material using `CelestialMotion.shader`, with two drifting/twinkling star layers and low-amplitude haze. Serialized motion speed, star strength and haze strength are adjustable. It updates only while enabled and does not alter global time, portrait colors or cut-in playback.

### Edit prompt

```text
Use case: lighting-weather. Edit target: the supplied existing celestial menu background, background plate only. Create a sibling alternate for a fantasy RPG character-management menu. Preserve wide 16:9 composition, seamless horizon at 72 percent from top, extremely subtle floor reflection, empty center and usable negative space for UI. Change the overall tonal range to medium slate-blue and soft blue-gray, NOT near-black navy and NOT bright pastel daylight. Lift the upper and left midtones substantially so the background feels quietly luminous, mature and balanced. Add clearly visible but elegant scattered fine silver stars across the upper two thirds, varied tiny sizes with only 4 or 5 subtle cross-shaped twinkles; stars remain fine pinpoints, not glitter/confetti. A very soft diffused dusty-silver celestial haze adds depth, evenly dispersed with no focal blob. Palette restrained desaturated blue-gray, gentle faint lavender-gray nuance, soft silver horizon, no cyan/violet saturation. The floor should be atmospheric and less visibly tiled, smooth with faint wide curved reflections. Intended use behind colorful anime Live2D artwork from multiple artists, so support many artwork colors without tinting them. Full-bleed clean high-quality 16:9 landscape bitmap preferably 2048x1152 or similar. No characters, objects, symbols, text, labels, typography, logo, UI, buttons, frames, portals, planets or large moon. Keep midtones controlled and details calm.
```
