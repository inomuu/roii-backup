# Seraphine Basic Sword Edge

Prototype: two ivory/gold mesh edges follow the existing seven-frame basic
attack in `seraphine.png`. The sprite sheet is unchanged. Frame numbers below
are zero-based: frame 2 sweeps, frame 3 contacts, frame 4 leaves a short fading
tail; all other frames hide the edge.

![Previous twin edges on the left, brighter glow on the right](glow-compare.gif)

[Video](glow-compare.mp4) | [Contact frame](glow-compare.png) |
[Without bloom](glow-no-bloom.png) | [Light background](glow-light-background.png) |
[Previous twin-edge prototype](twin-edge.gif) |
[Previous single-edge prototype](comparison.gif)

The outer ribbon is wider, with a thinner inset ribbon and visible space between
them. Both taper and fade with the source poses. The material's `_EmissionGain`
is 6 (previously 2.5): brightness comes from the unlit HDR shader, not increased scene lighting.
Bloom supplies the surrounding glow; this shader does not light nearby surfaces.
No gameplay lighting or volume assets were changed.
The brighter iteration changes no mesh geometry or timing. Its comparison renders
use the same scene and bloom, with a temporary 2.5-gain material on the left and
the current material on the right. The wider halo intentionally brightens the
area around the stroke while preserving the narrow, separated cores.

The curves are authored in the source sprite's 120 x 100 pixel coordinates and
render as tapered triangle ribbons. They inherit the body's scale, billboard,
lunge and horizontal flip. Meshes are built once per actor; opacity uses a
MaterialPropertyBlock. The shared material is in Resources so its shader is
included in builds.

ArenaView enables this only for Seraphine's melee basic animation. It latches
`pending_skill` at strike start, suppresses it during casting, and cancels the
edge if an actual `skill_fx` arrives. It does not generate a hit or change sim
timing. Animation and ASPD behavior remain owned by the existing animator.

This basic prototype uses coplanar curves. For a future attack that circles
behind the actor, author a 3D ribbon path crossing the sprite plane and retain
depth testing against the actor's alpha-clipped depth. Flipbooks can supply
animated texture detail or separate impact layers. A foreground billboard
alone cannot provide that front/back relationship. Because the sword is baked
into the character sprite, its 3D path must be authored rather than read from
a weapon bone.

Validation: Unity 6000.0.83f1 compiled the runtime and editor scripts and rendered
60 animation samples, each with original/enhanced and previous/current comparisons.
Checked finite mesh vertices, hidden preparation/recovery,
explicit suppression, shader support, both facings and light/dark backgrounds.
The original/enhanced contact comparison changed 184,436 pixels between the two
halves with the broader glow (this is a render-presence check, not a quality score).
The HDR preview copies bloom threshold (1.4), intensity (3.5) and scatter
(1) from the existing R1 volume, with Neutral tonemapping. It does not reproduce
the arena's full grading. Bloom-on/off renders were checked visually; the two
curves remain distinct in both.
The preview uses the 14 fps source poses; it is not a live arena combat capture.
High-ASPD and event timing in the running arena have not been visually verified.

To reproduce, run the Unity menu `ROI/Art/Capture Seraphine Basic Slash`.
It renders a temporary preview scene into `/tmp/roi-seraphine-slash` without
opening or saving a gameplay scene. The batch entry point is
`SeraphineSlashPreview.Capture`.
