# Seraphine radial four-point stars

Adds a subordinate support layer around the circular slash. The selected original
impact without outline, blade, texture erosion, aura and foot glow are unchanged.
The scoped red target flash is softened as described below. This remains an editor
preview, not a new live skill integration.

Eighteen native Unity particles spawn on a radius-1.4 circle in the slash plane, with
even angular distribution. They move radially outward at 1.1-2 actor heights/s.
Current sizes are 0.18-0.26 actor heights; lifetimes 280-420 ms, with a brief fade-in,
shrink and fade-out. Billboards use an anti-aliased four-point star shader, a white
centre and cool-blue tips. No generated image, global bloom or flash-default change.

After reviewing the first 12-star capture, the user requested more/larger stars
and a softer red flash. The revised scoped preview uses flash colour
`(1.3, 0.42, 0.36, 1)` and 65% of the existing opacity curve, retaining sprite detail.
The duration and live combat defaults are unchanged. The star shader, radial speed
and lifetimes are unchanged from the first star capture.

The star clock advances through recovery, so stars continue drifting instead of
freezing after the sweep. Explicit cancellation hides the renderer and clears the
system. Native particles use a fixed visual seed, independent of gameplay RNG.

Capture with `SeraphineCircularSlashPreview.CaptureRedImpact` (Unity menu:
`ROI/Art/Capture Seraphine Impact With Red Flash`). Images render at 1280x720 and
540x960. Existing isolated layer, fade, cancellation and red-flash checks run.
Additional checks measure star pixels in small projected particle regions within
the full composite and inspect mean radial velocity. Hiding the renderer and
reversing particle speed must fail the same guards. The image region excludes
unrelated animated scenery from the negative control.

Artifacts:

- `radial-stars.mp4`: real time, 84 frames at 60 fps.
- `comparison-slow.mp4`: previous stronger red-flash version without stars left;
  revised stars and softer red flash right, one-third speed.
- `radial-stars.png`: peak frame.

The revised capture passed in Unity 6000.0.83f1 at both sizes. Stars contributed
1,068 changed pixels in landscape and 616 in portrait within projected particle
regions. All 18 particles were present; mean outward speed was 1.510662. Hidden
renderer and reversed-speed negative controls failed as expected. Existing slash
and impact lifecycle/fade checks passed. The batch exited successfully; comparison
contains 84 frames over 4.2 seconds.

## Larger white-core revision

User found the previous stars too small and too pale blue. Size increased another
roughly 50%, from 0.12-0.17 to 0.18-0.26. The white region now fills most of the
four-point shape (mask transition 0.75-0.96 instead of 0.35-0.85), with local white
emission 4.5 instead of 2.5. Saturated blue `(0.005, 0.2, 1)` remains only at the
tips/edge with separate gain 1.5, avoiding an all-over pastel tint. Count, emission
timing, radial movement, lifetime, soft red flash and the rest of the effect stay
unchanged. Scene bloom remains untouched.

The full-composite check now also measures newly bright white pixels at projected
star positions; it must fail when the stars are hidden, so the white slash itself
cannot satisfy the test.

Latest artifacts: `radial-stars-bright.mp4` (real time),
`comparison-bright-slow.mp4` (previous small pale-blue stars left, new larger
white-core stars right, one-third speed), and `radial-stars-bright.png`.

This revision passed both Unity captures. At the sampled peak, star contributions
were 2,168 changed / 890 new bright-white pixels in landscape, and 1,247 / 478 in
portrait. Radial speed remained 1.510662. Hidden-renderer and reversed-speed checks
caught their negative controls; existing fade/cancellation checks passed; exit 0.
