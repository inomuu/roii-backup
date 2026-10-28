# White Base Texture Study

This is the next isolated step after correcting the lens-section mesh. It follows
the broad wedge-mask idea shown at 03:26-03:37 and one-shot UV movement at
03:43-03:49 in the user's local Anatomy Of A Slash video. It is a local
reconstruction of that technique, not the author's original texture or shader.

Latest: [shallow-ripple v3, brighter preview](suzu-base-white-v3-bright.mp4)
and [full-mask frame](suzu-base-white-v3-bright.png).

Earlier v1 captures:

- [Real time](suzu-base-white.mp4): 168 frames, 60 fps, 2.8 seconds.
- [One-third speed](suzu-base-white-slow.mp4): 20 fps, 8.4 seconds.
- [Portrait](suzu-base-white-portrait.mp4) and [full-mask frame](suzu-base-white.png).

## Assets And Mapping

- `Assets/Resources/Effects/Suzu/SuzuBaseWedgeMask-v3.png`: 512x512 linear grayscale,
  white coverage on black, straight upper edge, shallow asymmetric waves and
  one dominant swept cusp. The earlier rounded `v1` and hooked `v2` bitmaps remain
  for comparison; v2's long evenly emphasized tips were rejected by the user.
- `Assets/Editor/Vfx/Suzu/SuzuBaseWedgeTexture.cs`: editable 2D Bezier contour with
  a crisp central cusp and 4x4 supersampled scan conversion. The outline keeps
  broad continuous curves instead of v2's long triangular cuts. This mask is code-authored, not an
  AI-generated bitmap, a screenshot crop, or a paintover of an existing texture.
- The PNG has a 2% black border. The shader maps its interior onto radial V;
  the straight upper edge lies along the outer radial boundary. That orientation
  is our explicit local mapping, not recovered source UV correspondence.
- `Assets/Resources/Effects/Suzu/SuzuSlashBase.shader`: sampled coverage with
  edge antialiasing, white emission, one-shot offset and explicit out-of-window
  rejection. No `frac`, UV-repeat motion, erosion noise or vertex deformation.
- `Assets/Scripts/Arena/Vfx/Suzu/SuzuSlashBaseEffect.cs`: owns its material only,
  shares the imported lens mesh, samples cast age and handles hide/restart.

The imported mesh retains its upper/lower mirrored U layout and center thickness.
No geometry, shared shaders, existing fire texture or Sunder timing was overwritten.

## Timing And Scope

The fixture uses the same placeholder swordsman and two stationary targets.
It suppresses all fire layers, ground glow, embers, impacts and target hit flash.
There is exactly one white base renderer. This is editor presentation only.

At cast age 0.36-0.64s the offset advances from -1.05 to 0. It then moves to
+1.05 through 1.55s, leaving the UV window entirely. A gentle opacity fade begins
at 1.30s; lifetime ends at 1.65s. At full mask visibility the silhouette is a
broad arc with a straight circular boundary and irregular inner taper, not the
old cropped thin ribbon. Color and support layers are intentionally deferred.

## Reproduce

Unity menu: `ROI > Art > Capture Suzu Base Texture`.
Batch entry point: `SuzuSlashBaseTexturePreview.Capture`.
The capture builds/imports the mask, opens the existing arena fixture additively,
and renders 168 frames at 60 fps in 1280x720 and 540x960.
Raw output: `/tmp/roi-suzu-base-texture`. No authored scene or project settings
are saved. The normal `SuzuSunderPreview.Capture` route is unchanged.

Verification isolates the base on black without post-processing and measures
mask contribution, UV travel, progressive exit, out-of-window transparency,
cancellation, repeatable restart, disabled sampling and mirrored actor visibility.
Negative controls hide the renderer, replace the mask with white, freeze UVs,
force an out-of-window mask back to offset zero and freeze the exit clock.

## V3 Verification: 2026-09-25

Both viewports passed with the shallow-ripple mask. Mask replacement changed
21,491,610 RGB energy units in landscape and 12,282,798 in portrait. At cast ages
0.92 / 1.05 / 1.18 / 1.28s, landscape exit energy decreased through
15,939,432 / 10,153,182 / 3,859,830 / 845,397; portrait through
9,210,987 / 5,619,657 / 2,048,814 / 435,141. Out-of-window transparency and all
existing lifecycle / sabotage checks passed. The generator also rejected a
deliberately crossing contour. Lighting changes and their measured comparison
are documented in [attack variants](attack-variants/README.md#preview-lighting).

## Earlier V1 Verification: 2026-09-25

Unity capture passed in both viewports. The white-mask replacement changed
24,292,527 RGB energy units in landscape and 14,065,590 in portrait, confirming
the imported wedge controls the rendered silhouette. Exit energy at cast ages
0.92 / 1.05 / 1.18 / 1.28s decreased through 13,005,552 / 8,470,263 / 4,217,370 /
1,622,517 in landscape and 7,590,126 / 4,806,057 / 2,282,424 / 836,991 in portrait.
Offset -1.2, +1.2 and +2.2 produced zero effect pixels with opacity forced to one.
All listed negative controls and lifecycle checks passed. Full and partial-mask
frames were inspected, including portrait framing. This is not a performance or
live skill-event integration test.
