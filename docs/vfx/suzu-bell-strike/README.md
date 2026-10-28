# Suzu Bell Strike Preview

Single-target downward fire slash followed by a contact explosion. Editor-only
presentation study, not live skill integration or an area-damage change.
The caster still uses the existing swordsman placeholder attack animation.

[Thai pipeline comparison with rendered images](/Users/lunalieneray/idle_mmorpg/docs/vfx/suzu-bell-strike/pipeline-comparison-th.md)
explains the original pack, custom half arc, and latest white-tinted pack separately.

## Vendor White Core (Current)

The user prefers the actual pack over the custom Bell carrier and requested
white in that effect. This preview changes ONLY the main particle system's
start-color tint, from the source orange `(1, 0.35174727, 0, 1)` to warm white
`(1, 0.97, 0.91, 1)` on the temporary instance. Other particle layers keep their
source colors. No extra white strip, duplicated carrier, replacement shader,
UV adapter, opacity change or retiming is applied. The original FBX, shader,
texture masks and particle curves are used directly.

- [Real time](bell-vendor-white.mp4), [portrait](bell-vendor-white-portrait.mp4).
- [Original pack / white-core comparison, one-third speed](bell-vendor-white-comparison-slow.mp4).
- [White-core frame](bell-vendor-white-peak.png).

Entry point: `MaykerFireSlashPreview.CaptureBellVendorWhite` /
`ROI/Art/Capture Vendor Fire With White Core`. Raw frames:
`/tmp/roi-bell-vendor-white/`; log: `/tmp/roi-bell-vendor-white.log`.
The source-material identity checks remain active. Visible/hidden,
restart, changing frames, surviving tail and expiry passed at both viewports.
An on/off tint check resimulates the same age and measures near-white pixels:
0 -> 30,894 landscape and 0 -> 21,145 portrait. Disabled-tint and unchanged-image
negative controls fail as intended. The native high HDR gain is retained, so
the head reads strongly white-hot. This is still an editor preview, not a live
skill replacement or user approval of the final white intensity.
At the landscape peak sample, switching the tint off reproduces the direct
vendor baseline pixel-for-pixel (FFmpeg PSNR infinite).

## Direct Vendor Comparison

Previous request: use the actual imported pack and compare it against our version,
without further tail tuning. Run `MaykerFireSlashPreview.CaptureBellVendorComparison`
or `ROI/Art/Compare Vendor Fire Prefab And Bell Strike`. The source-only entry is
`CaptureBellVendorDirect`.

- [Pack on left / Bell on right](bell-vendor-vs-custom.mp4).
- [Same comparison at one-third speed](bell-vendor-vs-custom-slow.mp4).
- [Current custom half arc, taper still rejected](bell-half-arc-taper-review.mp4).

The left side instantiates the actual `Slash Fire VFX.prefab`: original imported
FBX/UVs, materials, textures, particle curves, native playback clock, distortion,
impact and sparks. It does not construct `MaykerFireOrbitStudy` or replace any
source material. Assertions check the renderer mesh/material identity against
the prefab and require all referenced materials to come from the vendor folder.
Uniform scale fits its outer diameter to 4.4 actor heights. One spawn rotation
exposes its broad face and points its head toward the enemy; source particle
rotation is left intact. Placement uses the same arc center as our half arc.
Fixed seeds, camera color/depth textures and the existing point-light expiry fade
are capture adaptations, not edits to the source prefab. See the source handoff
for Unity's earlier material migration.

The right side is the latest custom static half arc with the restored vendor
graph, UV travel, narrowed coverage, solid orange/white layers, aura and Hovl
explosion. Both use the same scene, camera, exposure, cast start and frame rate.
Their native durations, geometry, materials and supporting layers intentionally
differ: this compares complete effects, not an isolated shader-only A/B.
Raw source frames: `/tmp/roi-bell-vendor-direct/`; custom frames:
`/tmp/roi-suzu-bell-strike-half-arc/`. No effect has been accepted or wired live.
Both captures passed visibility, restart, changing frames, tail and expiry checks
at 1280x720 and 540x960. The source's large impact spokes can extend beyond the
preview frame at this unretuned scale; this is not a particle-framing pass.
The comparison movies use full landscape frames at 960x540 per side (1920x540
total), 168 frames: 60 fps / 2.8 seconds and 20 fps / 8.4 seconds. Logs:
`/tmp/roi-bell-vendor-direct-facing.log` and
`/tmp/roi-bell-vendor-direct-comparison.log`.

## Restored Orbit Pipeline (Current)

Review status: the user still rejects the tail silhouette as insufficiently
tapered. Tail tuning is paused for the direct-pack comparison above, not accepted.
The latest tapered PNG capture is `/tmp/roi-bell-tapered-arcs-retry.log` with
frames in the directories below. The MP4s linked here still show the preceding
uniform-tail capture and must not be presented as the latest taper revision.
The slow comparison has not yet been encoded.

The preceding request compared 90-degree and 180-degree carriers for a longer
curved tail. Run `MaykerFireSlashPreview.CaptureBellArcs` /
`ROI/Art/Compare Bell Quarter And Half Arc Fire`. It captures both bare carriers,
then both animated variants at 1280x720 and 540x960. Raw frames go to
`/tmp/roi-suzu-bell-strike-quarter-arc/` and `/tmp/roi-suzu-bell-strike-half-arc/`.
Both use the same camera framing, shifted left by 0.8 actor heights before spawn
to include the full half arc in portrait. No camera or carrier moves during playback.

- [90 / 180-degree comparison, pre-taper](bell-long-tail-arcs.mp4).
- Quarter arc: [real time](bell-quarter-arc-vendor-flow.mp4),
  [portrait](bell-quarter-arc-vendor-flow-portrait.mp4).
- Half arc: [real time](bell-half-arc-vendor-flow.mp4),
  [portrait](bell-half-arc-vendor-flow-portrait.mp4).

These are new review candidates, not an accepted final shape or live skill.
The earlier `bell-*-fire.mp4` clips were rejected: the carrier moved and the
tail stayed too uniform. The subsequent standalone `BellSlashFlow.shader`
experiment also lost the accepted orbit's fire detail. It is no longer used.

Both use the shared `SuzuBellMeshPreview.BuildCarrier` geometry: width 1.35 actor
heights, original top at 2 heights, bottom extended to -0.2 to pass the feet.
The half arc keeps the same radii, 1.35-height width and impact endpoint as the
quarter arc. It extends the angular path, not the strip width or scale. It has
32 length segments versus 16 for the quarter arc, keeping the same tessellation
density. No nonuniform scaling or animation-time vertex deformation is used.
The carrier faces the camera and stays fixed for its whole lifetime. Its bottom
cross-section is centered below the enemy's foot line, with a camera-ray
offset to keep the below-foot extension in front of the battlefield surface.

The particle renderers use these meshes instead of the vendor FBX. The original
vendor mesh asset remains untouched. Disable shape emission and source X rotation;
set particle size to one so the accepted mesh dimensions are not multiplied by
the prefab's old size/orientation. Orange, golden rim and broad white highlight
reuse owned material copies from `MaykerFireOrbitStudy`, including the vendor
textures, scrolling masks, particle custom data and dissolve. Bell no longer
replaces the orbit shader, colors, opacity or render queues. Its only material
controls are `_BellStrip`, `_Head`, `_Tail` and `_Fade`.

`FireSlashUV` maps the moving head/tail interval on the static carrier into the
vendor's lower UV island: U 0.16-1, V 0.0048-0.7985. The original graph then
samples and combines its textures unchanged. Only unconnected UV0 inputs are
adapted; UV1 particle custom data is untouched. `FireSlashWindow` clips the
original final alpha to the traveling interval and applies the cast fade.
`_BellStrip=1` selects strip-specific coverage in `FireOrbitCoverage.hlsl`:
the leading cross-section stays broad and flat. Coverage starts narrowing at
vendor U=0.20 and reaches a tip at U=0.82, with the outer cutting edge anchored.
Orange coverage and the white band's width both contract toward the tail; the
white band uses the same normalized 0.57-0.96 cross-strip range as the orbit.
The earlier U=0.78-1 taper started too late: the native mask erased that area,
leaving the visible body and highlight almost constant-width. The mesh is not
deformed to correct it. `_Taper=0` can disable this coverage for diagnosis.
`_BellStrip` defaults to zero: UV adaptation and
travel masking become passthroughs, retaining the previous orbit coverage branch.
`tools/build_fire_orbit_graph.py` reproduces the graph and its added control.

An orange shared `BuffAura` precedes the strike. The existing Hovl explosion and
shared contact star trigger at 0.16 seconds, on one enemy only. The UV head reaches
the bottom at 0.2 seconds; the tail follows after a 0.18-second delay and reaches
the bottom at 0.62 seconds. Fade runs from 0.4 to 0.68 seconds. The native shader/particle
clock runs to 0.07 in the first 0.1 seconds, then to 0.17 at 0.52 seconds, followed
by 1.8x erosion. This preserves the native broad-blade phase through the strike.
The caster aura opts into the shared static-carrier mode.
All timing is staged editor presentation, not gameplay damage timing.

The capture measures fixed transform, changing UV head/tail, unit particle size, camera-facing
orientation, bare-mesh body overlap, blade-only visibility, white highlight,
combined impact visibility, repeatable restart, changing frames, tail and expiry.
Hidden, frozen, sideways and edge-on negative controls must fail. No live skill
event, sim rule, global hit flash, source prefab or authored scene is changed.

Rendered trail length is measured by projecting changed pixels back into the
carrier's along-strip coordinate, not screen-space bounding-box height (which
also measures strip width at the top of the quarter arc). Tests require growth,
travel toward the impact endpoint, contraction and expiry, with the effect lights disabled for
isolation. The graph guard `python3 tools/test_fire_orbit_graph.py` checks that
every vendor node/property/slot and original connection survives generation,
and deliberately broken copies must fail. These checks do not certify art quality.
The accepted orbit also passed a fresh capture after adding the adapters. Its
landscape white-on and portrait peak images were pixel-identical to the saved
pre-change images (FFmpeg PSNR infinite), confirming the default passthrough
for those sampled frames. Its legacy rotating mesh is not migrated by this work.

## Wide Mesh Review

The user requested two alternative carriers, then clarified that the strike must
overlap the enemy from head to feet, not stop above its head. Run
`SuzuBellMeshPreview.CaptureShapes` / `ROI/Art/Compare Bell Rectangle And Quarter Arc`.
Raw frames: `/tmp/roi-bell-mesh-shapes/`.

- [Rectangle, double width](rectangle-wide-landscape.png)
- [Quarter arc, double width](quarter-arc-wide-landscape.png)
- [Rectangle, portrait](rectangle-wide-portrait.png)
- [Quarter arc, portrait](quarter-arc-wide-portrait.png)
- Previous narrow carriers: [rectangle](rectangle-overlap-landscape.png),
  [quarter arc](quarter-arc-overlap-landscape.png).

These are separate temporary generated meshes, not edits to the licensed source.
Both have a height of two actor heights, strip width 0.9 actor heights (twice the
previous 0.45), 16 length
segments and a shallow 0.025-height center ridge. The curved option spans exactly
90 degrees. Both expose their wide face to the camera. Their bottom cross-section
is centered at the visible target's foot line, inferred from the existing fixture's
0.85-height target sprite. A small camera-ray offset places them in front of the
target without shifting screen alignment. UVs run along/across the strip, but no
texture, glow, impact or motion is applied in this comparison.

Unity 6000.0.83f1 capture passes both viewports, including visibility, contact
alignment, framing, and sideways/edge-on negative controls. Pixel differences in
the enemy body region measure overlap: rectangle 3,643 landscape / 2,600 portrait;
quarter arc 3,577 / 2,534. Every generated row measures a 0.9-height cross-strip
distance; the rectangle projects 95.78 pixels wide, up from 47.89, at unchanged
213.45-pixel height in landscape. These are body-region measurements, not sprite-mask
intersection counts. Visual review confirms both carriers cover the enemy.
These images record the accepted width before the later below-foot extension.
Both shapes were approved for effect trials; neither is the final selected shape.

## Earlier Mesh Placement Reviews

The first clip's edge-on presentation was rejected. Effect tuning is paused at
the user's request while reviewing the bare carrier. Run
`SuzuBellMeshPreview.Capture` / `ROI/Art/Inspect Bell Strike Mesh Placement` for
landscape and portrait images in `/tmp/roi-bell-mesh`. It shows the unchanged
vendor `Circle.005` mesh with a plain gray material and a separate wireframe;
no slash texture, particles, effect lights, aura or impact. Source vertices/UVs
are not edited. This placement is a proposal, not an approved final slash.

The first bare-mesh placement was rejected for being off to the side. The next
edge-on placement was also rejected: downward describes the travel direction,
not a requirement to show a thin edge. The current inspection exposes the broad
face toward the camera, centered directly over the head. It retains uniform
scale and unchanged vertices/UVs, with a two-actor-height diameter.
The crown anchor is measured from the sprite's top opaque row,
including its horizontal center and sprite flip, not its transparent frame bounds.
The carrier's projected bottom is aligned to that anchor.

- [Current broad-face mesh](mesh-broad-face-landscape.png)
- [Current portrait](mesh-broad-face-portrait.png)

Capture checks measure projected width/height, horizontal head alignment, bottom
alignment and viewport containment. Deliberate sideways offsets and edge-on
rotations must fail. Unity 6000.0.83f1: projected bounds 210.94 x 210.94 pixels
in landscape, 177.89 x 177.89 in portrait; bottom/head gaps 1.58 and 1.33 pixels respectively. Rendered
visible/hidden mesh checks also pass. No new animated effect is approved or wired.

The clips below retain the rejected first version for comparison; the current
`SuzuBellStrikeStudy` uses the new carriers described above.

- [Real time, 60 fps](bell-strike-real-time.mp4)
- [One-third speed](bell-strike-slow.mp4)
- [Portrait](bell-strike-portrait.mp4)

## Sources And Ownership

`Assets/Editor/Vfx/Suzu/SuzuBellStrikeStudy.cs` owns the temporary UV playback and burst.
It uses the shared rectangle/quarter meshes and the orange/white UV coverage variant
from `MaykerFireOrbitStudy`. Placement happens once per viewport before playback;
sampling changes shader uniforms, not mesh transforms or vertices. The legacy
orbit's rotation has not been migrated in this task. This is not a new character animation.

The explosion comes from the **already imported** Magic Effects FREE pack:

`Assets/Hovl Studio/Magic effects pack/Prefabs/Hits and explosions/Explosion.prefab`

This vendor folder is gitignored and already supports Aegis via `Buff.prefab`.
No duplicate import, folder relocation, source material edit or project-settings
import was performed. A new checkout needs the locally licensed package and its
existing URP material conversion; the capture fails clearly if either is absent.

All four explosion particle systems use fixed seeds and sampled time. Only the
temporary instance changes: disable looping/autoplay, use local simulation and
hierarchical scale, fit to 0.22 actor heights. Its existing URP particle materials
are used unchanged. The fixture's shared `SwordHitImpact` adds the contact star
and scoped orange hit flash. The other fixture target is hidden.

## Capture And Checks

Run `ROI/Art/Capture Suzu Bell Strike`, or batch execute
`MaykerFireSlashPreview.CaptureBellStrike` for the rectangle. PNGs go to
`/tmp/roi-suzu-bell-strike-rectangle/`. Use `CaptureBellShapes` for both variants.
Use `CaptureBellArcs` for the current quarter/half-arc long-tail comparison.
Do not run another Editor on the same project or edit C# during capture.

```sh
ffmpeg -y -framerate 60 -i /tmp/roi-suzu-bell-strike-rectangle/motion-landscape-%03d.png \
  -c:v libx264 -crf 18 -pix_fmt yuv420p -movflags +faststart \
  docs/vfx/suzu-bell-strike/bell-rectangle-vendor-flow.mp4
```

Use 20 fps input for slow playback and `motion-portrait-%03d.png` for portrait.
Captures use Unity 6000.0.83f1 at 1280x720 and 540x960, 168 frames at 60 fps.
Keep rejected historical clips separate from the current `vendor-flow` outputs.

No sim, authored scene, global hit behavior or live skill timing changes.
