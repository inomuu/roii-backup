# Region 1 — selected EnvRef direction

The user selected the EnvRef-inspired concept on 2026-09-08 and authorized implementation in Unity.
The saved scene is `unity/ROI/Assets/Scenes/Zone_r1.unity`. All dressing is authored in Edit Mode;
no runtime, actor shader, gameplay, Rust or Godot code changed.

## Reference and preservation

- [Selected concept](art-review-r1/selected-envref-concept.png) is generated concept art.
- [Unity scene capture](art-review-r1/scene-envref-pass.png) and [Play Mode capture](art-review-r1/play-envref-pass.png) are actual engine renders.
- `art-review-r1/pre-envref-scene-backup.tar.gz` preserves the preceding scene, its meta,
  the complete region-1 asset directory and the two existing editor helpers, with repository-relative paths.
  This is a local art checkpoint, not a Git commit. Restore only the needed files deliberately.

## Changes

Three new images, stored under `Assets/Zones/r1/QualityProps/EnvRef/`, supply detailed meadow,
limestone gravel and a fresh oak/moss foreground cutout. Previous images remain intact.
The oak texture has real alpha and is imported with transparency and mipmaps.
New textures use a 2048 import limit and anisotropic filtering.

The terrain now blends the earlier meadow layer with SelectedMeadow (approximately 48–68% of
the grassy surface), retaining low-frequency variation. SelectedMeadow tiles over 4.3 m,
with a subtle existing grass normal at strength .08. Its diffuse remap max is
(1.35, 1.45, 1.50, 1), min (.015, .02, .015, 0).
SelectedTrail tiles over 4.2 m with the owned dirt/stone normal at .25.
The path centre follows z = -1.5 + .4 sin(.22x) + .1 sin(.71x); its width and feathered edges
vary gently, with a restrained worn-earth contribution elsewhere. Combat heights are unchanged.

`EnvRef_EdgeGrowth` contains 310 small grass and ivory-flower prefab instances along the wall
and foreground boundary. Wall-foot placements were tightened and reduced in scale after visual review.
Four additional oak/rock banks break up the background foliage. Six foreground renderers use
the new SelectedOak material as saved prefab overrides; the oak cutouts do not receive or cast shadows.
They remain fixed-camera 2.5D props, not volumetric trees for an orbiting camera.

## Lighting

The camera and actor-readable Sun rotation (50, 225, 0) are retained.
Sun shadow strength is .78; the cool directional bounce intensity is .62.
The warm sun-break spot is 165 with colour (1, .86, .65).
The shared grade uses exposure +.48, contrast 21, saturation -10 and filter (1, .99, .965).
DOF retains focus distance 20.5, with focal length 195 and aperture 3.2.
Bloom is .34 at threshold 1.1. Linear fog is (.53, .64, .66), start 27, end 50.
Existing daylight ownership of sun colour/intensity during play remains intact.

## Verification

- Play Mode used the baked region (EnvBaked=true), advanced to visible combat, and rendered readable sprites.
- Console returned zero errors.
- Scene validation: zero missing scripts, broken prefabs or other issues.
- 494 active scenery MeshRenderer bounds: no intersections with combat x[-10,10], z[-7,3].
- Combat terrain sampled on a .5 m grid: min=max=0.
- Saved scene was closed and reopened after serialization whitespace cleanup.
- These are Unity art checks, not a performance benchmark; Rust/Godot suites were not run.

The saved scene follows the chosen material, foliage and lighting direction. It is not a pixel-exact
reproduction of the concept; geometry and fixed-camera cutout foliage still differ.

## Image provenance

## Colour follow-up

The user requested richer colour after comparing the engine capture with the selected concept.
Only the shared region volume changed: saturation -10 → +8, exposure .48 → .65,
contrast 21 → 20, filter (.97, 1, .98). Split toning now uses shadows (.46, .54, .59),
highlights (.60, .56, .48), balance 0, reducing the preceding strong brown cast.
An initial saturation +16 trial was reduced after visual review. No placement or actor material changed.
Play Mode reached tick 55, sprites remained readable, and console returned zero errors.
[Updated engine capture](art-review-r1/scene-envref-colour.png).

### Generation method

Built-in image_gen through the imagegen skill; no external API or Meshy call.
Sources:
- SelectedMeadow: exec-d66e7e0c-3ba4-4b51-b076-1c3351bc6b8e.png
- SelectedTrail: exec-39961d0e-088b-4feb-9c93-6de2a0f64982.png
- SelectedOak: exec-6eec8bd5-5baf-413c-ad16-b3436f31057a.png

### SelectedMeadow

Production game terrain diffuse albedo texture, seamlessly tileable square2048x2048, STRICT orthographic top down, no perspective. Dense short natural countryside meadow grass with distinct fine overlapping bent grass blades, tiny clover leaves and moss, roughly90percentcoverage with10percent tiny glimpses of neutral brown soil. Fresh but restrained sage and forest greens with olive accents, NOT lime yellow, not grey or dead grass. Realistic HD2D miniature fine botanical texture, organic directional blade clusters varied across image, subtle broad variation and rich fine detail, medium contrast. Flat diffuse neutral illumination, no baked cast shadows, no sun pools, no large patches, no objects, no flowers, no road, no rocks, no text, no border, no blur. Edge-to-edge seamless albedo only, no alpha.

### SelectedTrail

Production game terrain albedo seamless tileable square2048x2048. Strict orthographic overhead of pale grey-beige compacted limestone dirt footpath surface. Many tiny embedded irregular rounded weathered limestone pebbles and gritty fine dust, small natural scuffs, subtle shallow depressions. Pebbles mostly small with occasional 2cm stones, no big rocks. Realistic detailed natural material for premium HD2D miniature scene, neutral warm beige and pale grey, not orange, no harsh contrast. Flat neutral diffuse lighting with no directional cast shadow, no sunlight patches, no blur, no vegetation, no road edges, no perspective, no text, no objects. Full bleed tileable all4edges texture.

### SelectedOak

One isolated production HD2D foliage prop on genuine transparent alpha background. Broad low asymmetrical young wild oak shrub surrounding a weathered grey mossy limestone boulder, airy irregular branching canopy, delicate distinct small oak leaves and a few tiny ivory flowers at base. Broad horizontal silhouette about twice wide as high, no prominent tree trunk, no round topiary. Front and above view at42degree camera downward pitch, for base pivot fixed camera cutout prop. Rich natural fresh sage green leaves, olive sunward tips and cool forest green shaded interiors, natural neutral grey stone and finely detailed moss. Realistic miniature botanical craftsmanship. Soft diffuse neutral light with gently brighter upper-left leaf tips, shadow interiors remain clearly readable not black. Crisp fine details everywhere, NO baked blur, bloom, vignette, or hard shadows. Entire prop fully visible with transparent margin, base near bottom, no ground rectangle, no scene or backdrop, no text. Landscape1536x1024.
