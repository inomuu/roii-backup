# White Slash Attack Variants

Five editor-only VFX studies reuse the same lens-section mesh, wedge mask and
white base shader. The character still uses the placeholder swordsman's original
attack frames; these are NOT five new sprite animations or live skills.

## Previews

Latest: shallow-ripple v3 mask with brighter preview lighting:

- [Bright comparison](comparison-v3-bright.mp4) / [one-third speed](comparison-v3-bright-slow.mp4).
- [Bright representative frames](comparison-v3-bright.png).
- [Lighting before / after](lighting-comparison.png), same idle pose with no slash.

The files below preserve the earlier rounded v1 mask and disabled-volume preview:

- [Comparison, real time](comparison.mp4), 1920x720, 60 fps, 2.8 seconds.
- [Comparison, one-third speed](comparison-slow.mp4), 20 fps, 8.4 seconds.
- [Representative frames](comparison.png).
- [Portrait framing example](whirlwind-portrait.png).

Comparison layout, left to right: top row = vertical, diagonal, horizontal;
bottom row = circular, whirlwind, source wedge mask. Representative stills use
different cast ages to show each shape, while the movie uses one shared clock.

| Variant | Individual Clip | Placement And Timing |
| --- | --- | --- |
| Vertical | [vertical.mp4](vertical.mp4) | Upright narrow ellipse in front of caster; 216-degree carrier window. |
| Diagonal | [diagonal.mp4](diagonal.mp4) | Upright plane tilted 42 degrees; same 216-degree window. |
| Horizontal | [horizontal.mp4](horizontal.mp4) | Waist-height horizontal 162-degree sweep toward the targets. |
| Circular | [circular.mp4](circular.mp4) | Two fixed 180-degree windows, opposite orientations, 0.12s stagger. |
| Whirlwind | [whirlwind.mp4](whirlwind.mp4) | Three two-part circles at heights 0.28 / 0.65 / 1.02 actor heights, starting at 0.32 / 0.62 / 0.92s. |

Directional sweeps play the base clock at 1.4x. Circular plays at 1.55x;
whirlwind at 2.4x. Whirlwind repeats existing attack poses to accompany the pulses.
Every mask passes through its own UV window once and exits. No per-frame mesh
rotation/deformation, texture repeat, damage calculation or extra hit event is used.
No fire, ground glow, impact or target flash is enabled in this study.

## Implementation

- `Assets/Editor/Vfx/Suzu/SuzuSlashAttackStudy.cs`: placements, arc windows and
  pulse schedules. One imported mesh and one texture are shared by all instances.
- `Assets/Editor/Vfx/Suzu/SuzuSlashAttackPreview.cs`: capture and isolated checks.
- `SuzuSlashBaseEffect.SetPlacement`: instance placement and material parameters.
  Defaults preserve the original single-base preview.
- `SuzuSlashBase.shader`: optional carrier-U window and underside U alignment.
  In this study only, underside sampling reverses the authored mirrored U back
  to the same physical travel direction so both sides remain aligned when tilted.
  The mesh's UV data is not modified. Both new controls default to legacy behavior.

Unity menu: `ROI > Art > Capture Suzu Slash Attack Variants`.
Batch entry: `SuzuSlashAttackPreview.Capture`.
Temporary output: `/tmp/roi-suzu-slash-attacks`.
Each variant produces 168 motion frames at 960x540/60fps plus stills and checks
at 540x960. No scene or render-pipeline settings are saved.

`SuzuSlashAttackPreview.CaptureDirectional` captures all five variants, then the
standalone base. The current mask is `SuzuBaseWedgeMask-v3.png`: a 2D contour with
shallow asymmetric waves and one dominant swept cusp. V2's three long hooked
tips were rejected as strange-looking ripples; its source bitmap is retained,
but the base effect now loads v3. The generator checks contour crossings with a
deliberately self-intersecting negative control.

### Preview Lighting

`SuzuSunderFixture` freezes gameplay behaviours but preserves `Volume` and URP
camera/light components. The old blanket disable also turned off the scene's
post-processing volumes. Each preview volume now receives an owned temporary
copy of its profile and components: exposure is increased by 0.8 EV (the current
arena profile goes from 0.65 to 1.45), and depth of field is disabled because its
authored focus distance does not match this close-up camera. Bloom and grading
remain authored. Profiles are destroyed when the additive fixture closes; the
game scene, shared profile assets and live actor exposure are not changed.

The capture also renders an idle before/after pair with volumes disabled/enabled,
measures image luminance, and repeats the disabled-volume fault to verify the
brightness guard rejects the old behaviour.

## Verification

V3 and the brighter fixture passed on 2026-09-25: all five styles in both
viewports, plus the standalone base. Idle-frame mean luminance measured
129.87 -> 155.24 (about 19.5% brighter); disabling volumes again tripped the
brightness guard. The initial +0.35 EV adjustment failed that guard, so the
preview was raised to +0.8 EV rather than weakening the threshold. The authored
scene and source volume profile have no diff. Portrait framing and ripple
silhouettes were visually inspected. These are revised studies, not user approval.

All five variants passed Unity capture in landscape and portrait on 2026-09-25.
Every pulse was rendered in isolation and contributed visible pixels; all five
landscape composite fingerprints differ. Motion, viewport borders, expiration,
cancellation, repeatable restart and mirrored actor visibility passed. Deliberate
hidden pulses, duplicate fingerprints, frozen clocks, frozen expiry and a clipped
border pixel were rejected. This is neither a sustained performance measurement
nor a gameplay integration test.
