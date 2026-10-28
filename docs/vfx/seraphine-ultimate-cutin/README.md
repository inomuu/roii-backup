# Seraphine Aegis Cut-in Prototype

Real Unity UI/shader rendering, using the existing Seraphine illustration.
The source portrait is unchanged. Aura detail uses a generated grayscale VFX mask.

![Unity-rendered cut-in, single aura burst](aura-burst.png)

[Video](aura-burst.mp4) | [Ultrawide](aura-burst-ultrawide.png) |
[Portrait](aura-burst-portrait.png) | [Previous repeating waves](aura-textured.mp4)

## Direction

A 1.8-second overlay: diagonal reveal, large character art sliding in from fully
outside the left edge with a cubic ease-out, a small push-in, one broad blue light
burst behind the art, neon shield contours, two slow-moving arcs and a translucent lower foreground wash,
then a diagonal exit. The arena stays visible beneath a dark veil. Glow is
local shader falloff; the gameplay camera, exposure and bloom are unchanged.
The battlefield veil opacity is now 0.96 (previously 0.78). This affects the
background composition, not the opaque portrait colors or the scene lighting.
Entry settles over about 0.53 seconds; the art remains readable until the exit
starts at about 1.52 seconds. Artwork orientation and size are unchanged; the
settled illustration is centered again in all viewports. The sim hold follows the
full 1.8-second clip duration.

The illustration is `Resources/EquipmentPortraits/Seraphine.png`. The original
blue brushwork remains baked into that illustration. One textured wave releases
at about 0.53 seconds, expands outward with cubic ease-out until 1.28 seconds,
then holds its shape as the light fades completely by about 1.69 seconds. There
is no wave recycling. The subtle flare peaks at about 0.63 seconds. The two neon
arcs turn only about 25 degrees during the release and then stop. Crisp cyan-white
accents trace a double shield contour and the illustration's alpha edge. Spray streaks stay
removed. Most aura is behind the portrait; only a low-opacity lower wash and
the lower portions of the orbiting trails cross in front, keeping the face clear. Cyan transition
light matches the artwork. This is a visual palette, not a gameplay element change. The baked
brushwork itself is not deformed, and this is not a Live2D rig.

`Resources/Cutin/SeraphineAuraFlow-v1.png` is an AI-generated grayscale intensity
mask, sampled twice with distorted Cartesian UVs that stretch with the burst and
stop when expansion finishes, rather than continuously scrolling. It breaks up the
wave sheet, adds an uneven crest and varies its contour. It does not affect the
portrait or neon geometry. The import uses linear intensity, mipmaps and mirrored
wrapping; seamless tiling is not assumed from the generation prompt alone.
Set shader `_AuraDetail` to zero to compare against the procedural-only version.
See [generation record and exact prompt](aura-texture-generation.md).

## Try in Unity

While playing, use `ROI > Cut-in > Seraphine Aegis > Play once`. Escape cancels.
The runtime entry point is `SeraphineUltimateCutin.Play()`.
It refuses another simultaneous cut-in, holds the current SimDriver, and
releases its own hold on completion, cancellation, disable or destruction.
The overlay blocks pointer clicks through it while playing.

SimDriver presentation holds are independent of its existing `Paused` setter:
Break ending cannot release this cut-in's hold, and the cut-in cannot release
an existing manual pause or another owner's hold. Paused time is not added to
the tick accumulator. Neither `Time.timeScale` nor Rust gameplay is changed.
Cosmetic scene animations are not globally frozen by this prototype.

## Validation and Limits

- Unity 6000.0.83f1 compiled the scripts and shader. Capture length follows the
  clip duration, with 0.3 seconds before and 0.6 seconds after (81 samples at 30 fps
  for this 1.8-second iteration).
- Checked 1280x720, 1680x720 and 540x960 output visually.
- Before/after renders match exactly; the overlay leaves no residual pixels.
- Texture import and runtime binding checks pass; the capture checks that toggling
  texture detail visibly changes the 1280x720 hero (RGB difference threshold 12).
- Pause ownership checks cover another caller clearing `Paused`, an existing
  manual pause, component disable, explicit stop and an overlapping owner.
- The first check run caught an Editor lifecycle failure on disable. The
  corrected ExecuteAlways component passed the same checks.
- Localization key validation and `git diff --check` passed.

The supplied video composites the real cut-in over an EXISTING STILL screenshot
of the arena. It is not a live battle recording or proof that live sim ticks
paused. Pause checks use an isolated driver without an attached native session.
Actual arena playback, Escape input and native tick behavior still need a live
play check. This visual iteration does not exercise the separate gameplay
ultimate trigger or its frequency policy. Audio and shield gameplay VFX are not
part of this prototype.

Capture with `ROI > Cut-in > Seraphine Aegis > Capture preview`, or batch entry
point `SeraphineCutinPreview.Capture`. Output is `/tmp/roi-seraphine-cutin`.
This uses a temporary preview scene and does not open or save the gameplay scene.
