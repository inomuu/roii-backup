# Suzu: Sunder Preview

First fire-sweep study for SR Suzu. This is an editor presentation fixture, not
live skill-event integration. The sim, damage, targeting, Toughness, DEF reduction,
and global hit flash are unchanged. Start with the [VFX handoff](../README.md).

## Current Preview

The latest fire experiment adds a
[white highlight, UV-tapered tail and accents](free-slash-pack/README.md#highlight-and-accents)
to the solid-color self-centered orbit: one full revolution, an orange body,
white upper band, golden rim, foot light, contact particles and embers. This remains an
Editor-only study; the original vendor render and white-base comparisons below
are preserved separately.

The latest comparison renders [five attack variants](attack-variants/README.md):
vertical, diagonal, horizontal, circular and whirlwind, reusing the white base.
The current [bright comparison](attack-variants/comparison-v3-bright.mp4) uses the
shallow-ripple v3 contour and a preview-only lighting profile with close-up blur disabled.

The latest isolated step is the [white wedge base texture study](base-texture.md):
the thick-center lens mesh, a broad grayscale mask and one non-repeating UV pass.
Fire, foot glow, embers, impacts and target flashes are suppressed in this fixture.
[Current real time](suzu-base-white-v3-bright.mp4).
Use `SuzuSlashBaseTexturePreview.Capture` for this version; the original Sunder
capture remains a separate comparison route.

## Earlier Layered Study

The layered Sunder code uses the imported [base mesh](base-mesh/README.md) with a scoped
`SuzuSlashArc` shader: fixed geometry, two UV passes, texture-controlled core and
fire, then erosion/fade. [Experimental capture](suzu-sunder-static-study.mp4).
On 2026-09-25 the user flagged that this carrier does not match the reference.
Further texture tuning is paused pending geometry review; this is NOT an accepted
reference reconstruction. Its 0.5 inner/outer radius ratio and 64 subdivisions
were authored guesses, as was the coincident front/back interpretation of mirrored UVs.

The user then identified the missing center thickness. The imported base now has
upper/lower center ridges and sharp radial edges; see the
[lens-section preview](base-mesh/base-mesh-lens.png). The experimental movie above
predates that correction and still shows the flat mesh. Additional reference:
[Free Slash VFX study](free-slash-vfx-study.md); package source is now staged
under the Editor-only reference folder and has been rendered in our battlefield.

The previous moving-mesh version is retained below for visual comparison:

- [Real time](suzu-sunder-double.mp4), 2.8 seconds at 60 fps.
- [One-third speed](suzu-sunder-double-slow.mp4), 8.4 seconds at 20 fps.
- [Portrait](suzu-sunder-double-portrait.mp4), 540x960 at 60 fps.
- [Contact frame](suzu-sunder-double.png).

## Art And Timing (Earlier Moving-Mesh Version)

Suzu has no dedicated sprite sheet in the current Unity art tables. The fixture
explicitly uses `swordman_sheet.png` and its six-frame basic swing as a temporary
stand-in. This is not final Suzu art or a global character-art mapping change.

The current Rust `SUNDER` definition is a Fire, self-centered radius-1 sweep (3x3)
with `motion_delay_ticks: 5` and `anim_slot: special_1`. The preview schedules its
contact at 0.5 seconds after cast start to match that delay; it does not calculate
the hit set. Two stationary demonstration targets show independent impact flashes.
Their restrained warm hit tint is fixture-only; the shared hit-flash default is unchanged.

- Warm foot light builds during anticipation.
- The sweep begins at 0.36 seconds and reaches full extension at 0.63 seconds,
  then travels another full 360 degrees through 1.18 seconds.
- One 295-degree ribbon carries a sharp HDR cream-white core, saturated orange
  edge, soft red-orange wake, and eroding textured flame streaks.
- The white highlight's mesh half-width is 0.39 actor heights, three times the
  first version's 0.13. The rim and fire carrier move outward to retain separation.
- Contact at 0.5 seconds adds warm directional impacts and a radial ember burst.
- The sweep stays bright for both revolutions, then fades from 1.2 to 1.7 seconds.
  Fire erosion is delayed with it. The extra revolution does not repeat the hit,
  target flash or damage; it is a visual continuation of the same cast.

The dedicated `Resources/Effects/Suzu/SuzuFireTrailMask-v1.png` is an AI-generated
grayscale fire ribbon with hooked tongues and open gaps. Its red channel controls
coverage; the shader adds yellow-to-red color, emission, and tail erosion. The
carrier is not tapered a second time, preserving the painted flame silhouette.
The existing flow texture is reused for erosion, with instance-owned materials.
Generation used the built-in image tool; [the exact prompt](fire-mask-prompt.md)
is retained with this handoff. The original generated bitmap is imported unchanged
as a linear, clamp-wrapped, uncompressed texture with mipmaps.

## Ownership

- `Assets/Scripts/Arena/Vfx/Suzu/SuzuSunderEffect.cs`: geometry, sampled timing,
  fire palette, ground light, embers, cancellation and resource cleanup.
- `Assets/Resources/Effects/Suzu/SuzuSlashArc.shader`: experimental static-carrier
  UV transport. The core samples the middle ribbon of `SeraphineSlashMask-v1`;
  support layers sample `SuzuFireTrailMask-v1`. Back-face culling prevents the
  two surfaces from blending twice through each other. Shared shaders are unmodified;
  the base asset is maintained separately by `SuzuSlashBasePreview`.
- `Assets/Scripts/Arena/Vfx/Common/SwordHitImpact.cs`: existing impact geometry;
  the new instance-level `SetAccent` changes only the caller's halo, outline and
  spark colors. Existing callers keep the original blue defaults.
- `Assets/Editor/Vfx/Suzu/SuzuSunderFixture.cs`: placeholder art, authored arena,
  scheduled contact, two targets, camera and temporary resource ownership.
- `Assets/Editor/Vfx/Suzu/SuzuSunderPreview.cs`: menu/batch capture entry point.
- `Assets/Editor/Vfx/Suzu/SuzuSunderChecks.cs`: isolated rendered-layer and
  palette measurements, fade/expiry, cancel/restart, and per-target impacts.

## Reproduce

Unity menu: `ROI > Art > Capture Suzu Sunder`.
Batch entry point: `SuzuSunderPreview.Capture`.
Temporary output: `/tmp/roi-suzu-sunder`; 168 frames at 60 fps in 1280x720 and
540x960. The authored scene is opened additively and never saved.

Keep the current preview clips and representative image here. Raw frame sequences
belong in `/tmp` and can be removed after verification. Capture checks include
deliberately hidden layers, a frozen fade clock, hidden impacts, and frozen expiry.
This fixture does not measure sustained GPU/mobile performance or validate the
live skill-event route.

## Extra-Revolution Verification: 2026-09-24

The current version passed Unity batch capture in both viewports. Sampling the
actual mesh tip through the second sweep measured exactly 360 additional degrees.
The core's maximum sampled band width was 0.5483277 actor heights at age 0.55;
its half-width parameter increased exactly 3x, from 0.13 to 0.39. Compressing the
rendered geometry back to one-third width tripped the thickness guard. Freezing
the second sweep clock and hiding its renderers tripped the rotation and visibility
guards. No impact renderer remains enabled during the second revolution.

All six layer checks, generated-mask/erosion checks, palette, fade, expiry,
cancellation, repeatable restart and disabled-sampling checks passed. Tail RGB
energy at ages 1.30 / 1.42 / 1.54 / 1.62 was 40,093,118 / 29,333,865 / 6,774,736 /
534,355 in landscape and 23,604,270 / 17,587,103 / 4,034,528 / 317,907 in portrait.
The 1.67s pre-cutoff sample was below 1% of peak; expiry left zero effect pixels.
Contact and second-revolution frames were inspected in both viewport sizes.
Current clips contain 168 frames; superseded one-revolution media were removed.

## Initial Fire-Mask Verification: 2026-09-24

The following measurements describe the initial one-revolution version, before
the user's request for another revolution and a three-times-thicker highlight.

Unity batch capture completed successfully in both viewports with the generated
fire mask. Six effect layers each made a measured contribution in isolation;
hiding each renderer tripped its guard. Warm-edge / bright-core palette checks
passed, as did expiry, cancellation, deterministic restart, and disabled sampling.

| Measurement | Landscape | Portrait |
| --- | ---: | ---: |
| Fire-mask difference vs solid white mask (RGB energy) | 67,484,171 | 33,944,934 |
| Fire energy before erosion | 25,283,006 | 13,469,086 |
| Fire energy at erosion 0.9 | 52,124 | 31,819 |
| Full effect tail, age 0.70 | 28,030,858 | 16,462,516 |
| Full effect tail, age 0.82 | 16,784,478 | 10,141,765 |
| Full effect tail, age 0.94 | 3,252,258 | 1,936,888 |
| Full effect tail, age 1.02 | 242,900 | 144,434 |

White-mask, frozen-erosion, frozen-fade, frozen-expiry and hidden-impact negative
controls were caught. Both targets' impacts contributed independently. Contact
and recovery images were inspected; the portrait composition keeps the flame tips
inside the viewport. No full-game, sim-event or performance claim is made.

The first run exposed a real draw-order defect: the orange alpha-blended ribbon
covered the white core. Explicit per-layer render queues fixed it. A subsequent
tail check was corrected to account for tiny HDR values quantizing to black in an
RGB24 capture near expiry: positive decreasing samples are measured through 1.02s,
then the 1.07s sample must be below 1% of peak before the 1.1s lifetime cutoff.
