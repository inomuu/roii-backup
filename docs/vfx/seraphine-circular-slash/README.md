# Seraphine: circular sword mesh prototype

Uses the existing seven basic-attack frames in row 2 of `seraphine.png`. No new sprite art,
character rotation, root movement, gameplay damage, AoE range or sim timing changes.

`CircularSwordSlash` is a manually sampled presentation component. `Show(frame, frameProgress,
visible)` sweeps one world-space curve through 360 degrees during frames 2-4. A growing trail
reaches 330 degrees during the first 40% of the strike. The main ribbon is now 60% thicker than
the approved wide-curve study (width coefficient 0.10 -> 0.16), retaining its smooth geometric
edges and pointed ends. A shorter cyan inset blade, a soft additive blue wake, and 18 short-lived
native Unity particles layer around it. The wake reuses the generated Seraphine water texture;
its texture never displaces the solid blade's silhouette. A fourth ribbon adds a smooth white
halo immediately around the main blade. A fifth, texture-masked ribbon adds three sharp white
streaks outside the primary blade. These five ribbons total 970 vertices and 960 triangles,
excluding the separate body aura, foot plane and particles. The earlier jagged/morphing
experiment remains removed.
The two blades use per-instance SwordEdge materials (emission 4.5 / 3.5); the new
CircularSlashGlow shader handles the textured wake and radial spark mask. The basic attack
material, shared SwordEdge shader and global bloom settings are unchanged.

After the flipbook comparison, the user chose the mesh and requested a blue aura plus glowing
feet. The slash now reuses `BuffAura` (textured rising curtain and helical ribbon) and
`BuffGroundGlow` at a separate foot anchor. The surrounding blue wake is wider, while the
approved main blade geometry is unchanged. The aura's white-core mix is reduced locally;
the ground has a blue tint and a bright white center. Per the follow-up direction, the blade
and inset stay white/cool-white rather than blue. The white halo hugs the main radius 1.35;
the saturated blue wake has moved outward to radius 1.6 and widened to coefficient 0.52,
so its soft texture surrounds the cut instead of filling the inset blade with blue.
These fade with the strike envelope.
Shared buff defaults retain the prior Aegis appearance: core whiteness 0.7, ground tint zero,
and no extra ground core. There are no global lighting or gameplay changes.

The sprite writes alpha-clipped depth, so rear mesh segments disappear behind its opaque pixels.
Mirroring the sprite mirrors the mesh about its local X axis. Mesh data is updated in preallocated
arrays; generated meshes and materials are destroyed with the component. The anchor is read-only.
Particles are manually sampled with a fixed visual seed, not gameplay randomness. All layers
are hidden and particles cleared when the strike is suppressed or the component disabled.

This is an isolated prototype, not yet attached to an authoritative skill event or every basic hit.
In particular it does not change Steel Break or Aegis. Existing basic sword-edge playback remains
unchanged. A gameplay integration should drive the component from the owning animation's strike
phase and suppress it when that animation is interrupted, without changing simulation outcomes.

## Preview

UV-motion alternative: `../seraphine-uv-study/README.md` contains a controlled A/B with fixed
ring carriers and scrolling masks. The moving-mesh implementation remains the default.
The latest `*-tail-v2.mp4` captures there include a gradual recovery fade in both modes:
the sweep still finishes at frame 5, but its light fades through frames 5-6 and is hidden
at pose 7. This supersedes the strike-end cutoff in the earlier clips below. Explicit
visibility suppression still cancels immediately; no simulation timing is changed.

Latest: `texture-mask-slash.png`, `texture-mask-slash-portrait.png` and `texture-mask-slash.mp4`.
`texture-mask-comparison.mp4` shows the unchanged effect on the left and the textured version
on the right, at half speed for inspection. Both sides come from the same capture and pose;
only the new detail renderer is disabled on the left. The normal-speed clip remains 1.4 seconds.

The primary blade retains width 0.16, the white halo, saturated outer blue wake, body aura and
white-centered ground glow. `SwordSlashDetail` samples the generated static grayscale
`SeraphineSlashMask-v1` for silhouette coverage on one extra ribbon (radius 1.48, half-width 0.4).
Unlike the solid blade, this carrier does not geometrically pinch its width: the painted mask
already tapers the tips. Linear mask sampling with derivative-smoothed threshold edges keeps
the cutouts sharp. HDR emission is local (3.8), not a change to global lighting or bloom.
From strike phase 0.52 to 0.98, a threshold erodes the trailing end first; the existing water
flow texture breaks up this threshold with phase-driven UV sampling. The primary blade never
uses this mask or erosion. See `TEXTURE-PROMPT.md` for the built-in generation prompt and asset.

Validation passed all nine renderers in landscape and portrait. Total changed pixels versus
no slash: 109,458 / 58,488. Isolated textured streak coverage fell from 12,815 / 6,747 pixels
to 3,031 / 1,481 at erosion 0.78 with geometry, opacity, camera and post fixed. Replacing the
mask with white and freezing erosion each caused their corresponding pixel guard to fail.
Every layer also rejected disabled-renderer sabotage; all layers hid on recovery and particles
cleared. Main blade counts remain 19,808 / 10,421, unchanged from the prior capture.
No full-battle integration or mobile performance test was performed.

Previous: `aura-ground.png`, `aura-ground-portrait.png` and `aura-ground.mp4` show the mesh with
the rising blue aura and white-centered blue foot glow. Capture checks automatically include
all eight renderers, testing each layer's visible contribution and hidden-renderer sabotage.
The latest capture passed both viewports with 108,276 / 58,058 changed pixels in landscape /
portrait. The white blade halo alone contributed 25,948 / 13,417 pixels on the isolated background.
The ground-center guard also removes the core color to confirm it rejects a missing white
center. The shared buff shader additions are opt-in defaults; Aegis was not recaptured in
this tuning pass. No mobile performance or online battle claim is made.

Previous: `layered-slash.png`, `layered-slash-portrait.png` and `layered-slash.mp4` show the thicker
layered version. `wide-curve.*` retains the approved single wide curve; `single-curve.*` retains
the earlier thin-line study. The layered capture passed both viewports with 33,016 / 18,300
changed comparison pixels in landscape / portrait. On an isolated background, individual
layer contributions (main, inset, wake, sparks) measured 19,808 / 4,214 / 25,099 / 124 pixels
in landscape and 10,421 / 2,369 / 13,200 / 64 in portrait. Each layer's pixel guard also ran
against its disabled renderer and rejected that sabotage. Recovery hides all four renderers
and leaves zero live particles. These are rendered-pixel counts, not performance measurements.

Editor menu: `ROI > Art > Capture Seraphine Circular Slash`.

Batch entry point: `SeraphineCircularSlashPreview.Capture`.
Output: `/tmp/roi-seraphine-circular-slash`.

The capture opens the authored arena additively without saving. It reuses its lighting and
post-processing, with temporary frozen foliage materials, and brings the camera closer for review.
Landscape (1280x720) and portrait (540x960) captures show the same 1.4-second preview:
0.15 s idle, 0.20 s anticipation, 0.40 s circular sweep, 0.20 s recovery, then idle.
These are preview-only pacing choices; no runtime attack cadence is changed.

## Reference Reading

The supplied Sword of Convallaria screenshots (desktop captures 2026-09-24, 07:36:59 through
07:37:23) show a broad white-cyan flare, a layered hooked edge, then separated white fragments
over a lingering blue body. Several captures show nearly the same video frame; desktop capture
timestamps are not animation-frame timings. The player reads roughly 1:19-1:20. These images do
not establish whether the source uses Live2D, Spine, a flipbook, meshes, or a combination.
The latest prototype layers effects around the approved smooth curve without silhouette noise;
it is not a claim about the original game's implementation. It does not reproduce the reference's full-screen
radial blur, exposure flash, chromatic fringes or debris layers.

Checks cover visible strike mesh, pixel contribution versus the unchanged sprite, finite geometry,
facing flip, unchanged actor position, per-layer contribution and recovery suppression. A no-post capture exposes the mesh
edge without bloom. `curve-only-*` isolates the curve on a dark background with all post-processing
disabled, to judge its geometric edge without scene lighting. This is not an online battle integration
or mobile performance benchmark.
