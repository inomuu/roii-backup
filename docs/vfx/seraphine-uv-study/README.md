# Seraphine: Moving Mesh vs UV Motion

An isolated presentation A/B, not a gameplay integration. The approved moving-mesh version
remains the default. Enable the alternative with `CircularSwordSlash.SetUvMotion(true)`, then
sample the same `Show(frame, progress, visible)` API. No character movement, simulation,
attack cadence, global bloom, lighting, sprite art or generated texture changes.

## Comparison

Unless stated otherwise, side-by-side media use **left = moving mesh, right = fixed mesh with UV motion**.

Latest recovery-tail revision:

- `before-after-tail-v2.mp4`: **left = old ending, right = new gradual fade**, both moving-mesh
  mode at one-third speed. The left side is cropped from the preserved original comparison.
- `comparison-tail-v2.mp4`: updated mesh/UV A/B at normal speed.
- `comparison-tail-v2-slow.mp4`: updated mesh/UV A/B at one-third speed.
- `comparison-tail-v2-portrait.mp4`: updated portrait mesh/UV A/B at one-third speed.

Earlier clips below are preserved, including their short fade and strike-end cutoff:

- `comparison.mp4`: normal speed, 84 frames at 60 fps, 1.4 seconds.
- `comparison-slow.mp4`: the same frames at 20 fps, one-third speed, 4.2 seconds.
- `comparison-portrait.mp4`: portrait A/B at one-third speed.
- `opening.png`: identical early strike pose, where the different reveal behavior is visible.
- `peak.png`: identical fully extended strike pose; the silhouettes are intentionally similar.

The A/B is captured sequentially at identical presentation phases, from the same actor and
authored scene. The same body aura, foot glow and fixed-seed spark system are reused; only
the five slash ribbons are exchanged. Camera, sprite pose and post-processing are unchanged.

## Implementation

`CircularSwordUvSweep` builds five full-ring carriers once. Their vertices, UVs and transforms
do not rotate or grow with strike phase. `SwordSlashUv` uses the head's angular UV offset and
periodic distance around the ring to transport the mask continuously through the 0/1 seam.
The material's angular span is constant at the approved maximum for each layer.

The main blade's existing width curve is baked into a 256-sample linear float texture, not
redesigned. Radius 1.35 and width coefficient 0.16 remain unchanged. The shader reconstructs
the same tapered cross-section and inner/outer colors from this profile. The inset, white
halo, blue wake and sharp detail mask retain their original dimensions, texture, emission,
blend mode, angular offsets and opacity envelopes. The original generated mask is reused.

The deliberate difference during startup: the original compresses the full longitudinal
texture into a growing arc; UV motion keeps its longitudinal scale fixed and reveals the
leading portion, with a narrow softened trailing reveal edge. Both use the same head motion
and reach the same maximum arc at phase 0.4. Detail erosion has the same timing in both.
After startup the images are close by design, not evidence that the UV mode is inactive.

Rasterization is not pixel-identical: the new carriers use 192 full-circle segments and
shader edge antialiasing, whereas the original uses 96 moving segments and geometric edges.
No performance advantage is claimed; fixed CPU geometry trades against full-ring transparent
fragment coverage, and neither mobile GPU load nor fill-rate was benchmarked.

## Gradual Recovery Tail

Previously the opacity envelope occupied only strike phase 0.84-1.0, about 64 ms in this
preview, and all layers were disabled at attack frame 5. With HDR-white materials the bright
core remained visually saturated until very late, making the ending read like a hard cut.

Both modes now retain the final sweep pose and fade from animation pose 4 through pose 7,
including recovery frames 5 and 6. At the unchanged preview cadence this is about 333 ms,
ending at 0.95 seconds. The smooth tail is squared so brightness leaves saturation earlier.
The common opacity drives the blades, halos, aura and foot glow. Detail erosion keeps its
previous timing. No extra spin, new silhouette, shader or global-lighting change was added.

`Show(..., false)` remains immediate cancellation. Sampling an inactive/disabled component
also hides it immediately. Completed pose 7 disables all renderers and clears particles;
the next strike is sampled independently with no retained fade state. Owners must continue
supplying recovery-frame progress for a natural tail rather than reporting cancellation.

## Validation

Run `ROI > Art > Compare Seraphine Mesh and UV Motion` or batch entry point
`SeraphineCircularSlashPreview.CaptureUvComparison`.
Temporary frames: `/tmp/roi-seraphine-uv-study`.

Both 1280x720 and 540x960 captures completed in Unity. Existing moving-mesh, detail erosion,
ground-center, per-layer visibility, facing and recovery checks still passed. New checks:

- All five UV carriers contribute rendered pixels independently and reject hidden-renderer
  sabotage. Landscape counts: 21,232 / 5,214 / 25,759 / 55,745 / 12,856 for main, inset,
  white halo, blue wake and details. Portrait: 11,493 / 3,109 / 13,324 / 28,465 / 6,780.
- Changing only the main blade's UV head moves 16,575 / 8,867 pixels. Freezing that property
  makes the movement guard fail; no other layers, post effects or moving geometry can pass it.
- Every carrier's vertex positions and world transform remain unchanged across strike phases.
  Moving a vertex deliberately causes the fixed-mesh guard to fail.
- Crossing head 0.9999 to 1.0001 changes only 7 / 4 pixels out of 17,898 / 10,512 visible
  pixels. A deliberately discontinuous head jump fails the continuity guard.
- Mirroring changes the UV render; recovery hides all original and UV renderers and clears
  particles. The actor stays at its original position throughout both motion captures.

Tail checks additionally render each mode on an isolated background without post-processing
at poses 5, 5.5, 6, 6.5 and 6.95. Sum of absolute RGB differences from the empty frame:

| Mode / viewport | 5 | 5.5 | 6 | 6.5 | 6.95 |
| --- | ---: | ---: | ---: | ---: | ---: |
| Mesh / landscape | 30601515 | 21930663 | 6660969 | 538903 | 0 |
| UV / landscape | 31014889 | 21944151 | 6629032 | 535791 | 0 |
| Mesh / portrait | 17285882 | 12370814 | 3755923 | 304090 | 0 |
| UV / portrait | 17627761 | 12413869 | 3742330 | 302532 | 0 |

Immediate-cutoff and frozen-pose sabotage each fail their corresponding fade guard.
Cancellation, next-strike brightness restoration, disabled-component sampling and exact
zero-pixel completion pass. An initial test incorrectly expected a play-mode OnDisable
callback from the edit-mode capture; the corrected test explicitly samples the disabled
component's public API. It does not claim play-mode lifecycle callback coverage.

This adapts the tutorial's UV-motion principle, not its exact unpublished shader. Caption
context and earlier interpretation corrections are in `../anatomy-slash-study/README.md`.
