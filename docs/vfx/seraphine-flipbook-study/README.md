# Seraphine: generated flipbook vs mesh

An isolated Unity presentation study, not a change to combat, skill events or the approved
mesh. Uses the same Seraphine sprite animation, stationary anchor, scene, camera, exposure,
post-processing and 0.4-second strike as the mesh preview. Both capture streams have identical
pose timestamps. No AI-generated character frames are involved.

## Pipeline

1. Use the supplied Sword of Convallaria screenshots as visual references: white hooked cutting
   edges, inset cyan ribbons, blue wakes and breakup. Do not infer their rendering implementation.
2. Generate one chronological 4x4 atlas with the built-in image_gen tool. Request top-down art so
   Unity provides the oblique world projection, rather than baking camera perspective twice.
3. Inspect the complete sheet before import. Generation 1 overfilled several cells. A second
   imagegen edit requested padding correction; this only partially followed the instructions.
   `generated-v1.png` preserves the rejected first attempt. Full prompts are in `PROMPTS.md`.
4. Import the unretouched second PNG from `Assets/Resources/Effects/SeraphineSpinFlipbook-v2.png`.
   Preserve source resolution and alpha. Use sRGB, bilinear filtering, clamp, no mipmaps and no
   compression for this quality study. The request was 2048 square, but the returned asset is
   1254 square, approximately 313 pixels per cell. Normalized UVs accommodate fractional cell
   boundaries; do not claim this is a 512-pixel-per-frame sheet.
5. `CircularSwordFlipbook` samples frames in row-major order, using the attack's normalized
   strike phase. A four-vertex horizontal quad at the same height as the mesh provides depth
   occlusion against the alpha-clipped character. The 4-actor-height quad puts the peak blade
   circumference near the mesh's radius, without normalizing individual frames or hiding drift.
6. `SwordFlipbook.shader` multiplies RGB by the preserved alpha and local emission (3), then
   blends additively. A 2.5%-6% cell-edge fade suppresses faint generated grid artifacts. This
   is an explicit runtime mitigation, not evidence the source sheet is clean. There is no
   frame interpolation, optical flow, shader rotation or procedural replacement artwork.
7. Render both effects separately at matching timestamps. Compare normal speed and quarter
   speed, inspect all 16 isolated cells, and inspect landscape plus portrait output.

## Art Review

The generated texture produces richer hooked tips, overlapping streaks and fading fragments
than the current deliberately smooth mesh. It is useful for testing the look of a painted
slash. The central empty space and scale are not perfectly stable across frames; several peak
frames change silhouette without following the requested consistent clockwise travel. Padding
is still imperfect despite the edit. These are source-art limitations, not solved by a green
rendering test. At reduced speed the difference between coherent mesh rotation and changing
painted silhouettes becomes easier to inspect.

The sheet is not yet a production-quality replacement for the approved mesh. A useful next
iteration would keep the mesh as the stable cutting silhouette and use generated painted
frames for the subordinate blue wake, or correct the flipbook's pivot/shape progression by
hand. This study does not implement that hybrid and does not claim parity with the reference.

The mesh uses opaque-looking alpha-blended blades plus an additive wake; the flipbook is all
additive and includes painted glow. Equal emission settings are not a luminance-matched test.
No background darkening or camera flash was added to favor either version. The different
breakup timing is deliberately visible rather than retimed away.

## Reproduce

Unity editor menu: `ROI > Art > Compare Seraphine Mesh and Flipbook`.

Batch entry point: `SeraphineCircularSlashPreview.CaptureComparison`.
Temporary frame output: `/tmp/roi-seraphine-flipbook-study`.

The original `Capture` method still renders the mesh alone. Comparison mode checks all 16
cells for visible pixel contribution and adjacent-frame differences, then exercises negative
controls by disabling the renderer and freezing its frame clock. Recovery must hide the
flipbook after a visible strike. The existing mesh checks still run; every captured pose also
checks that the actor did not move. These guards validate playback plumbing, not artistic
continuity, online battle integration or mobile performance.

The saved A/B media predates the subsequently requested mesh aura and foot glow. The user
preferred the mesh; later captures use its current settings, including those added layers.
The original comparison files and generated atlas remain unchanged for reference.

## Deliverables

- `comparison.mp4`: mesh on the left, generated flipbook on the right, normal speed.
- `comparison-slow.mp4`: the same comparison at quarter speed, no synthesized in-between frames.
- `comparison.png`: matching peak-phase stills, same left/right order.
- `flipbook-portrait.png`: portrait render in the authored arena.
- `generated-v1.png`: first generated sheet, before the imagegen padding edit.
- `PROMPTS.md`: exact generation and refinement prompts, tool mode and source caveats.

The final source atlas and runtime shader/component live in Unity Resources and Scripts, not
only in the image-generation cache. No existing approved mesh media was overwritten.
