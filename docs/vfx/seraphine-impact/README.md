# Seraphine sword impact prototype

## Selected direction and scope

User selected the original impact without the added outline, with a red target
flash, for this circular-slash skill only. Do not change the game's default hit
flash, other skills, ordinary attacks or all of Seraphine's hits to red.

The current implementation remains a scoped preview (`CaptureRedImpact`); neither
the circular slash nor its new impact is bound to a confirmed live skill event yet.
When that integration is requested, opt into the red flash at this skill's confirmed
hit only, and let unrelated hits retain their existing flash. Do not select it from
attacker character identity alone or change `ArenaView.FlashTint` / `ActorHitFlash`
defaults. This decision does not authorize changing general hit reactions.

The selected impact capture is `impact-red-flash.mp4`. A subsequent user request
softens the scoped red flash to `(1.3, 0.42, 0.36, 1)` at 65% strength while adding
radial four-point stars; current combined captures are in
`../seraphine-radial-stars/README.md`. The outline and larger-particle
captures below are historical comparisons, not the selected design.

Impact-only iteration after the approved textured circular slash and recovery fade.
The slash, its support layers, camera lighting and bloom are unchanged. A stationary
enemy sprite is added only to the capture fixture; neither actor is repositioned
during the attack. No gameplay damage, collision, hit stop or sim changes.

## Shape and timing

- Thin white diamond along the projected swing tangent, with a shorter cross glint.
- Soft saturated-blue halo behind the sharp core.
- Ten native Unity particles emitted in a narrow cone along the same tangent.
- Core fades over 190 ms, cross glint 130 ms, halo 270 ms; particles live 170-290 ms.
- Component is completely hidden at 380 ms. Its caller supplies contact, direction
  and age; there is no independent Update clock or damage decision.
- The preview schedules a demonstration hit at 0.55 seconds against the dummy.
  This is not connected to confirmed combat events yet.

The slender flash and directed fragments follow the impact discussion in
`../anatomy-slash-study/README.md`. No support pass is included in this iteration.

## Capture

Unity menu: `ROI/Art/Capture Seraphine Sword Impact`.
Batch entry point: `SeraphineCircularSlashPreview.CaptureImpact`.
Raw frames: `/private/tmp/roi-seraphine-impact`.

- `impact.mp4`: landscape, real time (1.4 seconds).
- `impact-portrait.mp4`: portrait, real time.
- `comparison-slow.mp4`: left without impact, right with impact, one-third speed
  (4.2 seconds). The dummy is present on both sides.
- `impact.png` and `impact-portrait.png`: hit at 0.583 seconds.

The fixture uses the authored arena and actor shader, rendering 84 frames at 60 fps
at 1280x720 and 540x960. Comparison captures hide only impact: target, slash, pose,
camera and scene stay identical. It does not save modifications to the scene.

## Verification

The capture measures isolated image contributions for all four renderers, direction
changes, decreasing tail energy, cancellation, restart and expiry. Negative controls
hide each renderer, freeze direction/age and reverse the particle cone. Native
particle positions are inspected to check forward travel. Existing slash layer,
texture-mask and recovery-tail checks also run.

Capture passed on 2026-09-24 in Unity 6000.0.83f1 for both viewport sizes. Isolated
RGB-difference energy at 45/140/250 ms was 1,039,774 / 310,466 / 897 (landscape),
and 611,371 / 180,976 / 177 (portrait). Ten particles emitted, mean local forward
travel at 45 ms was 0.0781. All four isolated layers contributed visible pixels.
Encoded clips contain all 84 frames; the batch exited successfully.

This is an editor-rendered visual prototype, not a play-mode combat integration or
a mobile performance measurement.

## Shared hit flash

The next iteration connects the dummy to the existing monster hit-flash rendering
code through `ActorHitFlash.Apply`. `ArenaView.TickJuice` now calls the same helper.
The live `recent_hits` trigger and timer remain unchanged: 300 ms, HDR white tint 6,
decay exponent 4. The helper preserves other material properties. No second combat
trigger, new damage calculation or global lighting change is introduced.

The preview starts flash and impact at the same scheduled contact time. The target
briefly becomes a bright white silhouette with the current live settings, then
returns to its original appearance. This deliberately does not retune the flash.
The new sword impact itself is still a preview, not a live combat event consumer.

New captures preserve the earlier impact-only files:

- `impact-flash.mp4`: impact with the existing hit flash, real time.
- `impact-flash-portrait.mp4`: the same in portrait.
- `comparison-flash-slow.mp4`: left impact only, right impact plus hit flash,
  one-third speed. Both sides retain the same target, slash and impact.
- `impact-flash.png`: combined peak frame.

Additional checks isolate the target sprite, measure flash at 45 and 230 ms,
verify the shader at half the duration yields `_Flash = 0.0625`, and require an exact
return to the baseline image after 300 ms. Disabled-flash and frozen-age negative
controls must fail the same visibility, curve and fade guards.

Both viewport checks passed in Unity 6000.0.83f1 on 2026-09-24. Target-only image
energy at 45/230 ms was 7,079,999 / 1,246,033 in landscape and 4,163,095 / 731,690
in portrait. The batch exited successfully. This checks the shared rendering path
and capture, not an end-to-end live combat session.

## Outer blue halo

The original small, centre-weighted additive halo was lost inside the target's
white hit flash. The halo now uses its own `SwordImpactHalo` shader with a soft
annular mask: colour is weighted away from the centre, with no hard ring edge.
Its ellipse grows from 1.35 x 1.05 to 1.75 x 1.4 actor heights, uses saturated blue
and emission gain 5. It retains the original 270 ms fade. The white impact core,
cross glint, particles, live hit-flash settings and scene bloom are unchanged.

The capture additionally compares halo enabled/disabled while target hit flash
and scene post-processing are active. It requires more than 100 newly visible
blue-dominant pixels, and repeats with the halo hidden as a negative control.

- `impact-blue-halo.mp4`: combined effect at real speed.
- `comparison-blue-halo-slow.mp4`: previous halo left, expanded blue halo right,
  one-third speed, both with the same hit flash.
- `impact-blue-halo.png`: combined peak frame.

Unity capture passed for both sizes: 3,033 newly blue-dominant pixels with hit
flash in landscape and 1,424 in portrait. Hidden-halo negative controls failed as
expected. Existing layer, tail and shared-flash checks also passed; batch exit 0.

## Rejected directional-particle experiment

User review found that the halo change still did not make individual particles
readable. The old soft additive particles were too small, started inside the white
silhouette, and mostly expired before the flash cleared.

The new `SwordImpactSpark` material uses alpha-blended blue diamond slivers with a
small cyan-white core. This preserves a coloured edge over white instead of only
adding more light. Ten particles remain, with a 38-degree cone, starting 0.28 actor
heights along the strike. Speed is 3.8-6 actor heights/s, size 0.065-0.095, and life
320-460 ms. The particle tail raises the component lifetime to 500 ms; core and halo
timing, flash strength, slash and scene bloom are unchanged.

The full-composite checks now toggle ONLY the particle renderer at 83 and 250 ms
after the hit, with both slash and target flash active. They measure newly blue
pixels against the matching particle-hidden frame, and require the same guard to
fail when the renderer is hidden. Direction checks use particle velocity so the
new spawn offset cannot mask a reversed emission cone.

Captures: `impact-readable-sparks.mp4` (real time),
`comparison-readable-sparks-slow.mp4` (previous particles left, revised right,
one-third speed), and `impact-readable-sparks.png` (83 ms after contact).

Both viewport runs passed in Unity 6000.0.83f1. Full-composite particle contribution
at 83/250 ms: 1,795 / 1,543 blue pixels in landscape, 1,024 / 809 in portrait.
Hidden-particle negative controls failed as expected. Direction, flash, halo,
fade, cancellation, restart and expiry checks passed; batch exited successfully.

## Previous: original impact with a thin edge

User preferred the original impact and requested only a visible edge. The enlarged
blue-sliver experiment above is reverted: original additive particle shader,
size, speed, cone, centre spawn, 170-290 ms lifetime and 380 ms component duration
are restored. The experimental particle shader is removed. Historical clips remain
for comparison, not as the active design.

Two blue outline layers now follow the original white needle and cross glint.
They share the exact diamond silhouette, expanded 8% along its length and 70%
across its very thin width. The original white meshes and their animation are
unchanged. The coloured underlay stays solid initially so HDR white cannot wash
through it, then fades during each glint's final 35%. Existing outer halo and hit
flash settings remain unchanged.

The full-composite check now toggles only the outline layers at 33 and 83 ms after
contact, with slash, particles and target flash unchanged. Isolated-layer tests
also cover both outlines and their disappearance at expiry.

Captures: `impact-outline.mp4` (real time), `comparison-outline-slow.mp4`
(original impact without outline left, same impact with outline right, one-third
speed), and `impact-outline.png` (33 ms after contact).

Both sizes passed in Unity 6000.0.83f1: outline contribution at 33/83 ms was
201 / 628 blue pixels in landscape and 87 / 356 in portrait. Hidden-outline
negative controls failed as expected. Original particle-layer energy returned to
88,145 in landscape. Layer, fade, cancellation and expiry checks passed; batch exit 0.

## Selected red target-flash study

User proposed the no-outline version with a red target flash. The separate preview
entry point `SeraphineCircularSlashPreview.CaptureRedImpact` disables both outline
layers and overrides only the dummy's `_FlashColor` to `(1.5, 0.015, 0.01, 1)`.
The red peak is lower than the existing white HDR 6 so it retains its colour.
The shared 300 ms / exponent-4 flash envelope, original impact and particles,
outer blue halo, slash and scene lighting are unchanged. Runtime flash defaults
and combat triggers are not modified by this colour study.

Output frames use `/private/tmp/roi-seraphine-impact-red`, separate from the white
flash capture. The target-only test requires a visible red contribution and fails
deliberately when white is substituted. It also checks decay and exact sprite
restoration. Existing impact layer, fade and cancellation tests still run.

Captures: `impact-red-flash.mp4` (real time), `comparison-red-flash-slow.mp4`
(white flash without outline left, red flash without outline right, one-third
speed), and `impact-red-flash.png` (33 ms after contact).

Both viewport captures passed: 10,706 red-contribution pixels in landscape and
6,278 in portrait; white-tint negative controls failed as expected. Flash restore,
impact layers and tail checks passed; batch exit 0. Where the retained blue halo
overlaps the red sprite, the additive mix appears pink/magenta.
