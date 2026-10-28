# Region 1 — authored Unity scene, 2026-09-08

The user's later instructions supersede the Godot deliverable in `zone-art-astra-task.md`:
work in Unity, author real scene objects, and use the supplied grassland image as the colour
reference. The saved deliverable is `unity/ROI/Assets/Scenes/Zone_r1.unity` on
`feature/zone-art-astra`. No runtime scene-generation or gameplay code was changed.

The initial, quality and reference fidelity passes below are historical snapshots. Their
placement, lighting and verification values describe preceding revisions; the latest work is
recorded under **Actor readability, continuous wall, foliage and landmark** at the end.

## Scene changes — initial and quality passes

- `Arena/Field/R1_AuthoredScenery` retains the original 88 authored stock prop placements, with superseded
  foreground props and selected middle rocks disabled. The quality revision adds nine oak
  cutout props, eleven 3D wall segments and two 3D wooden fences. There are 169 active
  MeshRenderers under this group.
- The former `Dressing`, `GrassField` and `Flora` groups remain in the scene, inactive.
  They put large props and grass inside the combat field. Nearby scattered surround plants
  are inactive; more distant surround trees remain.
- Painted the existing TerrainData alphamap into a gently bending horizontal path, replacing
  the T junction. Existing grass, cobblestone and dirt layers are reused. The old WetSheen
  overlay is inactive because its original road mask no longer matches the terrain.
- `Arena/CanopyBounce` is a saved, shadowless directional fill light.

## Placement — quality pass, in Unity world units

The combat rectangle is x = -10…10, z = -7…3. Camera remains at (0, 13.5, 13),
Euler (42, 180, 0), perspective FOV 42 degrees.

| Layer | Placement | Purpose |
| --- | --- | --- |
| Small edge details | x roughly -24…24, z about -9…-10.5 | Graduated transition from clear field to larger props |
| Rocks and hedges | x roughly -26…26, z about -11…-14 | A visible middle-distance band, occupying the upper portion of the camera frame |
| Back trees | x -26…24.4, z -13.8…-16.2 | Taller silhouettes behind the low band |
| Foreground oak and moss rocks | (8.4, 0, 8.3), (-9.3, 0, 8.7); uniform scales 5.6 and 6 | Natural silhouettes at lower corners; no axis stretching |
| Path | z centred on -2.8 + 0.7 sin(0.22x) | Soft-edged beige route across an open meadow |

The middle-distance band now includes a low dry-stone wall with two split-rail fence gaps,
small oak/moss clusters, flowers, hedges and taller trees. Stock back trees and terrain still
limit fidelity to the supplied reference; this is an authored game scene, not a match of all
the reference's fine environmental detail.

## Lighting and grade — previous quality pass

This table applies to the previous quality pass, not the latest reference fidelity revision.

One shared region profile governs the scene; existing prop materials and textures were not
individually recoloured.

| Setting | Previous quality-pass value |
| --- | --- |
| Sun Euler rotation | (50, 265, 0) degrees |
| Sun energy / colour | Existing 1.55 / (1, .97, .91) at the saved daylight setting |
| Cool bounce | Directional, intensity .38, colour (.64, .73, .88), rotation (48, 205, 0), no shadows |
| Colour adjustment | exposure +.30 stops, contrast +22, saturation -35, hue -8 degrees |
| Colour filter | (1, .97, .85) |
| Bloom | intensity .30, threshold 1.05 |
| Tonemapping | Existing Neutral |
| Fog | Linear, colour (.58, .65, .64), start 23, end 60 |
| DOF | Bokeh, focus distance 20.5, focal length 180, aperture 2.8 |
| Vignette / split toning | Existing region profile values retained |

An earlier backlight trial made actor billboards black. Testing in Play Mode exposed this;
the quality-pass sun angle restored actor illumination without shader or runtime changes. The existing
daylight system still controls sun intensity/colour and ambient during play.

## Verification — previous quality pass

- Unity 6000.0.83f1 compiled the editor capture helper without errors.
- MCP scene validation: zero missing scripts, zero broken prefabs, zero reported issues.
- All 169 active scenery meshes checked by **MeshRenderer** bounds: no intersections with the combat
  rectangle. ParticleSystemRenderer bounds are excluded because inactive particle bounds
  at world origin create false positives. New prop colliders are disabled.
- Play Mode loaded the baked environment, advanced the simulation, and rendered visible
  actors with the final light direction. Console returned zero errors; the quality revision test reached tick 374 with `EnvBaked = true`.
- Camera captures inspected at centre and x = -4/+4; camera restored afterwards.
- This was a Unity scene/asset pass. Rust and Godot smoke suites were not run.

Images: [before](art-review-r1/before.png), [first pass](art-review-r1/scene.png),
[quality revision](art-review-r1/scene-quality.png), [quality Play Mode](art-review-r1/play-quality.png). These are engine captures, not generated concept art.

`ROI/Art/Capture Region 1` runs the editor-only `R1ArtPreview` helper and writes a 1600×900
camera capture to `/tmp/roi-r1-art.png` plus a placement inventory. It does not generate props.

## Worktree dependencies

The initial worktree lacked local Cubism SDK, native plugins, and the owned nature/VFX packs.
Existing copies were restored from the user's other local checkout, preserving metadata.
This resolved Safe Mode. These local dependency copies are not new authored art. Incidental
Live2D importer and Steam define changes were restored rather than included in the art change.

## New quality props

Saved in `unity/ROI/Assets/Zones/r1/QualityProps/`:

- `OakMossForeground.png`: 1024 × 683 RGBA, preserved transparent alpha, mipmapped.
  Lit cutout material and base-pivot mesh, plus reusable prefab. Designed for the fixed
  42-degree camera; it is a detailed 2.5D prop, not a fully volumetric tree for orbiting.
- `DryStoneWall_4m`: mesh and prefab, 720 triangles per segment, three staggered courses
  of individually shaped stones, smoothed normals, neutral limestone material using the
  owned pack's rock albedo. Eleven saved scene instances.
- `SplitRailFence_4m`: mesh and prefab, three posts and four rails with slight natural lean,
  using the owned branch material. Two saved scene instances.

All construction happened in Edit Mode and was serialized into meshes, prefabs and scene
objects. No environment generator, billboard-follow behaviour or Update loop was added.
The former stretched foreground shrubs remain inactive for comparison.

### Image generation provenance

Method: built-in `image_gen`, through the imagegen skill; no external API or Meshy call.
Generated source: `exec-3339e74f-700f-4b84-9038-9bd52de1f2c0.png`, originally 1536 × 1024,
downscaled with Pillow while retaining RGBA. Output is the oak texture above.

Prompt:

> Create ONE production game environment prop as a genuinely transparent PNG with alpha:
> a naturally sprawling low wild oak shrub growing around one broad weathered mossy
> limestone rock, a few subtle fern fronds and tiny ivory wildflowers at the base. Isolated
> complete object, no background, no ground rectangle, no scenery, no frame, no text, no cast
> shadow outside silhouette. Designed as a foreground HD-2D billboard in a Unity diorama
> viewed from a camera pitched 42 degrees down, looking at the shrub from its front and above.
> Broad low irregular silhouette, twice as wide as tall, NOT a spherical topiary, NOT a tree,
> no exaggerated trunk. Natural believable branching and small detailed oak leaves, rich
> finely textured moss and chipped stone. Premium Octopath-inspired realistic miniature
> craftsmanship, tactile natural materials with subtle pixel-art-inspired detail, not low-poly
> flat shapes. Restrained desaturated olive greens, muted cool forest-green underside,
> grey-beige limestone. Soft broad warm key from upper left, subtle cool ambient in the dark
> recesses; no bright neon green or yellow. Balanced albedo, enough visible shade detail for
> scene lighting to grade. Keep object sharp, DO NOT bake blur, vignette or bloom. Subject
> occupies 88% width and 82% height; entire opaque base aligned near bottom centre,
> transparent margin on all sides. Output landscape 1536x1024, preserve actual transparency.

## Reference fidelity pass — previous revision

This revision brings the saved Unity scene closer to the supplied grassland reference by
changing the ground materials, breaking up the scenery boundary, adding canopy shade, and
extending the foreground framing. The camera remains at (0, 13.5, 13), Euler (42, 180, 0),
perspective FOV 42 degrees. Work remains authored in scene and asset files, with no runtime
environment generation or gameplay changes.

### Ground and composition

- Added 1024-pixel `MeadowGround.png` and `LimestoneTrail.png` textures with terrain layers,
  blended with the existing grass. The terrain alphamap is now 512 × 512. The worn route was
  corrected and shifted 1.2 m toward the camera to better match its position in the reference.
- Added a shallow background bank, reaching up to 1.2 m in height only where z < -10.9.
  The combat ground retains its existing height.
- Rearranged the wall with uniform scales from .76 to 1, varied depth and partially buried
  segments. Rocks and vegetation interrupt the masonry instead of leaving a constant border.
- Added white meadow-flower clusters, ferns and the pack's native green `Bush_01` prefabs.
  Broadleaf trees are grouped with overlapping silhouettes; real canopy shadows provide
  variation across the edge of the field. Owned foliage was selected by its native material
  rather than individually recoloured.
- Four additional foreground undergrowth clumps connect the lower edge into an irregular
  U-shaped frame. The larger corner props retain their proportions.

### Lighting and grade — previous reference fidelity pass

| Setting | Reference fidelity value |
| --- | --- |
| Sun Euler rotation | (50, 265, 0) degrees |
| Sun energy / colour | Existing daylight energy 1.55; runtime daylight colour control unchanged |
| GroundFill | Scene light component disabled |
| CanopyBounce | Directional, intensity .7, colour (.66, .76, 1), rotation (40, 150, 0) |
| Reference_SunBreak | Spot at (11, 12, -8), aimed at (5, 0, -2); range 32, outer angle 75 degrees, inner angle 40 degrees, intensity 130, colour (1, .78, .50), soft shadows |
| Colour adjustment | exposure +.4 stops, contrast +18, saturation -30, hue 0 degrees |
| Colour filter | (1, .98, .93) |
| DOF | Bokeh, focus distance 20.5, focal length 205, aperture 2.8 |
| Fog | Linear, colour (.61, .69, .72), start 26, end 48 |

The warm spot creates an upper-left light pool while the directional bounce supplies cool
shade. A 300-degree sun-yaw trial darkened the actor billboards, so the final sun restores
the verified actor-readable direction. The existing daylight system continues to control
the sun's intensity and colour during play.

### Review status

Previous engine captures: [reference fidelity scene](art-review-r1/scene-reference-pass.png),
[Play Mode](art-review-r1/play-reference-pass.png).

- Play Mode adopted the saved environment (`EnvBaked = true`) and reached simulation tick 63.
  The final light direction renders readable actors. Console returned zero errors.
- Scene validation found zero missing scripts, broken prefabs or other reported issues.
- All 304 active scenery MeshRenderer bounds are outside the combat rectangle.
- Combat terrain height remains zero; the bank affects only the scenery beyond it.
- Camera captures at x = -4 and +4 were inspected, then the centred camera was restored and saved.
- No runtime or simulation scripts changed. Rust/Godot suites are not applicable to this Unity art pass.

### New ground texture provenance

Both images were created with built-in `image_gen` and downscaled to 1024 × 1024 RGB with
Pillow. They are imported as mipmapped repeating terrain textures with anisotropic filtering.
They supply surface detail; scene lights produce the canopy shadows.

`MeadowGround.png`, source `exec-da4b0b7c-f1bb-48c4-ab25-c5c1ce57d45d.png`, prompt:

> Create a production seamless tileable diffuse/albedo texture for a Unity terrain: closely cropped natural meadow ground seen in STRICT ORTHOGRAPHIC TOP DOWN view, zero perspective. Restrained desaturated olive and sage-green short grass, tiny flat weeds and moss, irregular sparse grass blades over pale grey-beige dry soil. Grass covers about 75 percent, small interlocking worn earth patches 25 percent. Very fine realistic miniature material detail as in a premium Octopath HD-2D countryside meadow. Low contrast, broad irregular patch scale variation and fine grain, no strong repeating pattern. No path or road, no rocks larger than tiny pebbles, no flowers, no trees, no objects, no typography, no grid. Edge-to-edge surface, no alpha. Flat neutral diffuse lighting for a PBR albedo, absolutely no cast shadows, no sunlight pools, no vignette, no blur, no border. Must seamlessly tile horizontally and vertically. Square 1024x1024. Make the palette naturally muted olive-grey rather than bright lime or emerald, and the dirt softly chalky beige rather than orange or brown.

`LimestoneTrail.png`, source `exec-ec9b9107-3bd0-47a0-a41a-ed6ac446052e.png`, prompt:

> A production seamless tileable PBR diffuse/albedo texture, strict overhead orthographic view of a dry old countryside walking trail surface. Pale grey-beige compacted limestone dust and fine sandy earth, many tiny weathered stone chips, small worn flat embedded grey limestone pebbles, faint irregular shallow depressions and gritty earth grain. Warm neutral beige, muted low saturation, absolutely no orange clay. Texture only, NO scene, NO framing, NO distinct road edges, NO vegetation, NO large objects, NO text. Realistic miniature handcrafted HD-2D game ground material with natural finely detailed irregularity. Soft diffuse neutral shadowless lighting, no baked sun, no directional highlights, no cast shadows, no ambient vignette. Entire image filled edge to edge, seamless on all four edges; moderate and uniform brightness. Square 1024x1024.

## Actor readability, continuous wall, foliage and landmark

This revision addresses the user's four review points: dark actors, disconnected masonry,
unattractive round background bushes, and the need for a distinctive scene landmark. The
camera and open combat rectangle remain as documented above. All new scenery is saved in
the scene, meshes, materials and prefabs; no runtime environment component was added.

### Actor lighting

The saved Sun rotation is now **(50, 225, 0)**. Actor texture palettes, materials, shader,
global exposure and the region's colour grade were not changed in this revision.

`ActorSprite.shader` computes the normal actor colour as
`texture × vertex tint × material tint × (ambient SH + main-light colour × cookie × N·L) × .82`.
The existing flash override follows that calculation. This shader does not accumulate
additional lights, so the scene's added bounce and warm spot improve the surroundings
without supplying corresponding actor fill. Turning the sun toward the camera increases
actor `N·L` from approximately .554 to .849; retaining its pitch preserves the direct-light
angle on horizontal ground.

Paused-frame comparisons showed that disabling post-processing made actors darker. With
the grade retained, the new sun yaw raised displayed luminance by approximately **36%** in
same-pixel player and slime interior samples. This is an approximate masked sample, not a
measurement of fully segmented actors. The cloud cookie was null during that test. A
separate sRGB-import concern in the stored cookie is therefore latent, not the observed
cause; no cookie texture or importer was changed.

### Saved scenery

- Wall segments now follow a gentle continuous curve, with only length adjusted and their
  height/depth scales kept at 1. Adjacent masonry endpoints differ by less than .000001
  world units. The two intentional wooden-fence openings remain.
- `HazelThicket` and `FernMossBank` supply detailed background foliage as 1024 × 683 RGBA
  cutouts, each with a saved material and prefab. Nineteen saved instances replace the round
  stock bushes, which remain inactive. These cutouts do not receive shadows because their
  textures already contain subtle shading; they are designed for the fixed scene camera.
- `WayfarerShrine` is a new 3D prefab with 2,192 triangles at (-5.8, 0, -11.8), yaw 180°.
  Its carved arch, copper-and-amber lantern and teal banner provide a recognizable landmark
  beyond the combat area. `R1WaysideLandmark.Build` is an editor helper; its output is
  serialized into assets and scene objects without a runtime component.

### Verification status

All 180 active scenery MeshRenderer bounds were checked and remain outside the combat
rectangle. Play Mode adopted the saved environment (`EnvBaked = true`), reached tick 51,
and rendered the brighter actors and new landmark. Console returned zero errors; scene
validation found zero missing scripts, broken prefabs or other issues.
Engine captures: [scene](art-review-r1/scene-landmark-pass.png) and
[Play Mode](art-review-r1/play-landmark-pass.png). Controlled lighting comparison:
[before](art-review-r1/actor-before.png), [after](art-review-r1/actor-after.png).

### New foliage provenance

Both cutouts were made with built-in `image_gen`, then downscaled with Pillow to
1024 × 683 while preserving RGBA alpha. Source files:

- `HazelThicket`: `exec-945c457d-107e-4d2a-8c06-325f7f6603e0.png`.
- `FernMossBank`: `exec-0acb97f3-c0f6-4c11-8361-6aaad67807b2.png`.

`HazelThicket` prompt:

> One complete isolated game environment prop on genuinely transparent alpha background: an irregular low thicket of young hazel and wild willow growing from a narrow mossy earth bank, with branching airy foliage, a few ferns and tiny ivory meadow flowers at its feet. It is a naturally spreading dense shrub group, NOT a round topiary and NOT a tree, no ball-shaped foliage, no straight trimmed hedges. Layered asymmetrical silhouette: taller sparse sprays left of center, fine twig tips, low creeping plants at right. Twice as wide as tall. Camera views front and top with 42-degree downward pitch, intended as a base-pivot HD-2D Unity billboard. Restrained cool olive and sage greens, grey-green shaded leaves, pale moss; no yellow, orange, red or purple foliage. Realistic handcrafted miniature botanical detail in the Octopath diorama spirit, delicate small individual leaves, believable branching, tactile natural textures. Diffuse soft neutral illumination, gentle warm top-left key, enough shadow detail, NO deep black areas and NO hard baked directional shadows. Crisp object details, no blur, no vignette, no bloom. Entire irregular object visible with transparent margins on every side, base near bottom, no background scenery, no rectangular ground slab, no text. Production transparent PNG, landscape 1536x1024.

`FernMossBank` prompt:

> One complete isolated game prop as genuinely transparent PNG with alpha: a low asymmetrical clump of woodland ferns, pale moss-covered stones and small wild blueberry shrubs with tiny ivory-white wildflowers. Fine delicate pinnate fern fronds reaching sideways and arching gracefully, dense lower ground cover, a few distinct rounded grey mossy rocks peeking between plants. No berries, no red or purple, no large leaves, no topiary, no spherical bushes. Broad low wedge-like silhouette about twice as wide as tall, taller fern spray on the right, low trailing foliage on left. Front/above view from a camera tilted 42 degrees downward, for a base-pivot HD-2D Unity environment billboard. Refined realistic miniature foliage detail, muted sage and cool forest green with restrained olive highlights; soft neutral studio diffuse light for integration into a warm sunlit meadow. Keep the deepest shadows readable. No background, no ground rectangle, no cast shadow beyond silhouette, no scene, no text or frame. Sharp all over, no baked blur or bloom. Entire object visible with alpha margin on every side and base aligned near lower edge. Landscape 1536x1024.
