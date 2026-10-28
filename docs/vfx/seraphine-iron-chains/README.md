# Seraphine Iron Chains: linked-mesh study

Folder map and integration status: [VFX handoff](../README.md).
Runtime code lives in `Assets/Scripts/Arena/Vfx/Seraphine/IronChains/`;
capture runner, fixture and checks live in `Assets/Editor/Vfx/Seraphine/Chains/`.

One- and two-target studies rendered in the authored Unity battlefield. This is an
editor presentation fixture, not a live skill-event integration. It does not change
the Rust simulation, damage, targeting, taunt, DEF buff, or shared hit-flash behavior.

## Clips

- `iron-chains-two-targets.mp4`: current two-target preview, real time.
- `iron-chains-two-targets-slow.mp4`: current two-target sequence at one-third speed.
- `iron-chains-two-targets.png`: two-target tension pose.
- `reach-retract-sheet.png`: enlarged source rows 3 and 4 (zero-based), three frames each, original facing.

Superseded preview media and temporary PNG capture directories were removed with
user approval on 2026-09-24. Keep the current two-target outputs and the pose-source
reference above. Historical verification notes below describe earlier revisions,
not retained media. Capture commands recreate temporary frames when needed.

## Construction

- `IronChainsEffect` builds chunky rounded-rectangle links into one dynamic mesh. Adjacent links
  alternate their plane by 90 degrees, preserving visible holes instead of reading
  as a solid beam. Buffers support up to 80 links, 24 ring segments and 6 tube sides.
- Links follow approximately uniform arc-length samples along a slack curve.
  The curve straightens after contact; externally supplied endpoints can move.
- The shader uses stepped cartoon shading, pale highlights, and deep blue contour
  edges. The current wider contour band is near-black navy. The links have straight sides and small rounded corners, with twice the
  stroke thickness of the initial oval metal study. It is not physically based lighting.
- A second renderer shares the dynamic mesh and draws a front-culled, expanded
  black hull before the blue fill. The expansion is 0.013 actor heights. Its opacity,
  cancellation, and lifetime follow the fill, including explicit material cleanup.
- The caster aura reuses `BuffAura`, `BuffGroundGlow`, and the existing generated
  `SeraphineAuraFlow-v1` texture. A blue curtain rises around the actor, one partial
  ribbon orbit wraps it, and a restrained blue pool with white center sits at the feet.
  It fades in over 0.09s, has one brightness accent at yank +0.04s, and fades out
  from 0.58 to 1.04s. It follows the actor root, never the moving chain tip or target.
  Shared aura shaders and other buff consumers are unchanged.
- The caller supplies effect age, source and target world positions. The component
  does not find targets, decide contact, or move gameplay objects.
- The prototype assumes its parent has unit scale, as the actor roots here do.
  At very long endpoint distances the capped link count can leave gaps; production
  integration must match link capacity to actual arena-to-world range conversion.

## Preview Timing

The 2.1-second fixture starts the chain at 0.25 seconds:

| Effect age | Stage |
| --- | --- |
| 0.00-0.22 s | Fast throw from behind the caster to target |
| 0.22-0.36 s | Contact accent and short tension beat |
| 0.36-0.58 s | Strong initial yank, then ease into stop (editor fixture only) |
| 0.82-1.22 s | Smooth opacity fade |

The origin is at torso height, only 0.05 actor heights behind the center along the
facing axis, and behind the sprite's camera plane. The previous 0.85-height rear
offset read as a floating chain and was removed. The synthetic pull uses quartic ease-out, a small preload away from the caster,
and a brief target lift/tilt. These are presentation timing experiments only.

The caster now uses existing sheet row 3 for an extended arm, row 4 for arm
retraction at the yank, and row 6 to settle before returning to idle. Each contains
three frames. These are a visual interpretation, not confirmed author animation
names: `arena_tables.json` labels them hurt, guard, and guard-alt. No shared
metadata or gameplay animation mapping has been changed. Earlier clips used row 2
basic slash. The target is a stationary sprite translated by the fixture. Actual
Iron Chains uses `special_1` and supports up to two targets. The two-target fixture
demonstrates that count, but authoritative targeting and animation events are not wired here.
The blue/white contact accent reuses `SwordHitImpact` without its optional outlines.

## Reproduce

Unity menu: `ROI > Art > Capture Seraphine Iron Chains`, or batch entry point
`SeraphineChainsPreview.Capture`. Output: `/tmp/roi-seraphine-chains`.
For two targets: `SeraphineChainsPreview.CaptureTwoTargets`, output
`/tmp/roi-seraphine-chains-two-targets`. The extra chain is created with
`casterAura=false`, so both lines share one visible caster aura rather than doubling
its brightness. Each target has its own chain endpoint and contact accent. Targets
are staged at different ground depths and pulled together along separate lanes;
these remain synthetic preview positions, not sim displacement or collision logic.
Captures 126 frames at 60 fps in 1280x720 and 540x960, in an additive temporary scene.
The authored scene is not saved. No sustained GPU/mobile performance claim is made.

Verification includes isolated rendered-pixel contribution, decreasing fade energy,
zero pixels after expiry, endpoint/tip movement, and cancel/restart. Negative controls
hide the renderer, force opaque fading, freeze effect time, and offset the endpoint.
Visual inspection is still needed for link readability and attachment placement.

Previous front-spawn capture passed on 2026-09-24. Isolated chain contribution: 7,924 pixels at
1280x720 and 3,371 pixels at 540x960. Fade energy decreased from 1,205,722 to
234,581 and from 503,956 to 98,828 respectively before reaching zero at expiry.
The listed negative controls were caught. Both viewport tension frames and
throw/pull poses were visually inspected. This does not validate live skill timing.

Rear-spawn/yank revision also passed on 2026-09-24: isolated contribution was 8,732
pixels (landscape) and 3,908 (portrait). Fade energy was 2,002,988 -> 395,221 and
876,844 -> 170,749, then zero at expiry. The target travels 1.527 actor heights in
the first 0.1 seconds of the pull and only 0.148 in the remaining 0.12 seconds.
The new snap-distance guard rejects a frozen pull clock. Rear emergence, dark
contours, and framing were visually inspected in both viewport sizes.

Torso-anchored/ink revision passed: the expanded black silhouette changes 4,258
pixels in landscape and 2,110 in portrait against the fill-only render. Disabling
the ink renderer trips the contour guard. Combined fade energy decreases from
1,154,596 to 225,972 and from 510,582 to 99,331, then zero at expiry. Cancellation
disables both renderers. The throw-start and tension frames were visually inspected
to confirm the source no longer floats behind the caster. Fast-yank timing is unchanged.

Reach/retract pose revision passed in both viewports. The sampled sprite at yank
age +0.04s is measured against row 4's source rectangle; substituting the held reach
sprite trips the guard. The existing outline, fade, expiry, cancel/restart, and yank
checks remain green. Reach and retract captures were visually inspected. Only the
editor fixture's sprite selection changed; the sheet and global animation metadata
are untouched, and no claim is made that these are the artist's intended chain poses.

Caster-aura revision passed in both viewports. Isolated curtain/ribbon/ground pixel
counts: 18,903 / 4,198 / 26,027 landscape; 7,300 / 1,638 / 10,988 portrait.
Hiding each layer trips its contribution guard. Aura fade energy drops from
7,156,114 to 861,959 and 2,975,238 to 359,203; a frozen aura clock is rejected.
Chain fade is tested separately so bright aura pixels cannot mask a chain fade bug.
Combined expiry and cancellation leave all renderers off. Peak and recovery frames
were inspected. No chain pixelation post-process has been added in this revision.

Two-target capture passed in landscape and portrait. The second chain contributes
8,042 and 3,404 isolated pixels respectively. Hidden-renderer, wrong-target endpoint,
and frozen-expiry negative controls are caught. Existing pose, aura, fade, and yank
checks also pass. Isolated caster-aura layer counts and fade energy match the
single-target revision, confirming that adding a target did not double the aura.
Tension and post-pull frames were inspected for separation and framing.

## Folder And Ownership Refactor Verification

Verified on 2026-09-24 after moving runtime/editor scripts with their original
`.meta` files and extracting the caster aura, preview fixture, and checks. Public
capture entry points, the chain API, namespaces, and Resources paths are unchanged.

- Both `Capture` and `CaptureTwoTargets` completed successfully in Unity batch mode,
  including landscape and portrait checks and their deliberate negative controls.
- All 19 `IRON CHAINS` report lines from the two-target baseline matched the
  post-refactor run exactly. Timing and check thresholds were not relaxed.
- Compared all 126 motion frames per viewport. Full landscape SSIM and portrait
  cast-region SSIM rounded to 1.000000 for every frame. Images are not byte-identical:
  full portrait average SSIM was 0.998361, with visible differences confined to
  background foliage in the inspected difference image. The portrait cast region
  was the 540x400 rectangle at (0, 280), containing the actors, chains, and aura.
- As a comparison negative control, horizontally flipping portrait frame 034's
  cast region reduced SSIM to 0.278198. After-refactor tension imagery was inspected.
- The 21 moved/new C# files have metadata, and all 57 local Markdown links under
  `docs/vfx` resolve. Raw baseline and verification PNG sequences were removed
  after comparison; retained accepted preview media were not regenerated.

This validates the presentation refactor, not live skill-event integration or
sustained GPU/mobile performance. One initial single-target run failed during
Unity Package Manager startup; a fresh run completed successfully.
