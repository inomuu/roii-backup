# Anatomy Of A Slash: reference study

Source: the user-provided `Anatomy Of A Slash - onetupthree (720p, h264).mp4` in Downloads.
Measured duration 638.960 seconds; 1280x720, 60 fps, with an audio track.
Source video link located via an index: https://www.youtube.com/watch?v=QyI8ZS-G9nw

The initial study used extracted frames and on-screen text only. Follow-up on 2026-09-24:
downloaded the English `en-US` caption track using yt-dlp and read it in full, then checked
the UV, support-comparison and ice-highlight frames again. The local MP4 has video and audio
but no subtitle stream. The downloaded captions remain in
`/private/tmp/roi-anatomy-slash-study/anatomy.en-US.vtt`; they are not republished in this repo.
The ordinary web-page fetch failed, but subtitle retrieval succeeded before a redundant
translated track hit HTTP 429. No repeated requests were made after that rate limit.
This is a caption-assisted review, not a claim that the audio was independently transcribed.
Timestamps below are positions in the supplied file. Exact shader code, numerical bloom
settings and the timing of the original unedited effect cannot be established from these
screenshots. The later texture experiment changes presentation only; see
`../seraphine-circular-slash/README.md` and its saved comparison capture.

## Caption Review And Correction

- 02:01-02:51: choose complexity from the weapon's identity and the attack's role first.
- 02:53-03:37: the base establishes the silhouette. A separate arc mesh allows art direction,
  but its placement must be aligned manually with the weapon animation.
- 03:37-04:17: UV offset can drive motion; follow-through, fading, erosion and flipbooks are
  alternative dissipation choices, not requirements to stack together.
- 04:27-05:16: the highlight gives focus through contrast and shape, generally covering less
  area than the base. Its motion can follow the base or deliberately differ.
- 05:55-06:03: supporting detail must not overpower the highlight. **The brighter right-hand
  image at 06:00 is not an unconditional "better glow" target.** My first visual-only reading
  missed this warning's context.
- 06:21-07:12: impact should match the slash's visual language and reinforce its direction;
  the example uses a thin element moving roughly along the tangent at contact.
- 10:01-10:20: these are flexible design guides; the montage deliberately departs from some.

The ice montage's large highlight at 08:40 should therefore not be mistaken for a universal
surface-area rule. Its specific breakdown remains on-screen text rather than narration.

### Application To Our Prototype

Treat our approved white blade as the primary highlight, not as a base that needs another
equally dominant white layer. The new masked streaks are subordinate accents. Keep the
requested primary width unchanged; compare negative space and readability before increasing
brightness or detail density. Our new carrier has constant width because the texture itself
already tapers its tips; geometric tapering pinched those details twice in the first render.

Our implementation rotates/grows geometry and uses a static silhouette mask plus phase-driven
erosion. It does **not** yet use UV offset to transport the silhouette along a stationary mesh.
MaterialPropertyBlock supplies presentation phase; a particle custom-stream implementation is
not necessary for this manually sampled prototype. These are local engineering choices, not
shader code recovered from the tutorial. Global bloom and gameplay remain unchanged.

## What Is Visible

| File time | Observation | Local evidence |
| --- | --- | --- |
| Around 00:20 | A curved strip/ring-sector mesh is edited in Blender. | Overview extraction, temporary folder |
| 03:35-03:45 | A flat irregular slash silhouette is discussed alongside a normalized UV-x span. The shape does not need every small feature modeled into mesh vertices. | `uv-03m45s.png` |
| 04:00-05:15 | Four presentations of the trailing/dissipating slash are compared: continued motion, opacity fade, erosion and flipbook playback. | `decay-05m15s.png` |
| 06:00 | A white/gray layered cut is compared with much brighter cyan supporting bands and specks. Captions warn here against letting support overpower the highlight. | `glow-comparison-06m00s.png` |
| Around 07:00 | An impact direction is illustrated relative to the tangent of the curved slash. | Overview extraction, temporary folder |
| Around 07:40-08:00 | The fire example is decomposed into a darker base, brighter highlight, supporting elements and impact. | Detailed extraction, temporary folder |
| Around 08:30 | The ice example first displays subdued blue body strips without the brilliant white layer. | Dense ice-section extraction, temporary folder |
| 08:40 | The ice highlight adds bright blue/white sharp fragments across multiple layers, with erosion and a broad silhouette. | `ice-highlight-08m40s.png` |
| 08:42.5-08:43.3 | The paused ice explanation resumes; the broad highlight breaks into narrower disconnected streaks and vanishes. | `ice-motion-08m42s.png` |
| Around 08:50-09:00 | Supporting wind/ice details and a separate impact treatment are shown. | Dense ice-section extraction, temporary folder |

These are observations of this tutorial, not proof of how Sword of Convallaria implements its
VFX. The tutorial shows concepts in multiple tools; it is not a reason to revive the cancelled
Godot client.

## Why This Matters For Our Slash

Before the texture experiment, our approved main cut was a smooth geometric strip with vertex
color and local emission. It had no shape texture or UV-driven erosion. Its shape grew and rotated, then every layer shared
the same late opacity envelope. Raising emission can strengthen radiance, but the filled strip
will still read as a continuous flat white band once its display values saturate.

The reference's convincing energy comes from several visible differences, not just brightness:

- Darker/less luminous colored body areas give the white highlights a local contrast reference.
- Sharp highlights are interrupted by negative space and secondary tapered slivers.
- Soft light is adjacent to sharp shapes; it does not replace all edges with blur.
- Trailing detail changes shape as it disappears, rather than only becoming transparent.
- Supporting particles and the impact have directional and temporal roles, not merely density.

The original visual interpretation below is now qualified by the caption correction above.
It does not establish the author's exposure,
bloom algorithm, material blend settings, texture channel packing or exact animation curves.

## Proposed Next Experiment

Preserve the approved nearly full-circle primary silhouette, existing sprite attack, stationary
actor, white blade interior, external saturated-blue aura and white-centered foot glow. Do not
reintroduce the rejected noisy/morphing main mesh, and do not tune global bloom first.

1. Give a secondary slash strip longitudinal/cross-strip UVs. Use a dedicated slash material;
   leave the shared basic-attack SwordEdge shader alone.
2. Author or generate a static grayscale mask with a few curved directional streaks and hooked
   tips. Map it onto the stable arc. Begin with only two or three readable accents.
3. Use a separate grayscale breakup field and a phase-driven threshold for erosion. Keep the
   newly revealed leading edge crisp while the trailing section breaks into thinner islands.
   Direction, thresholds and anti-aliasing are our implementation choices, not recovered code.
4. Retain the continuous white primary blade during the main strike. Treat the masked details
   as subordinate highlights initially; evaluate shape changes before touching the core.
5. Give the secondary blue wake, highlight accents and sparks slightly different envelopes,
   all driven by the existing presentation phase. Do not change simulation or damage timing.
6. Keep actual impact effects event-driven. A directionally correct flash belongs at an observed
   hit location, not at an invented collision computed by the renderer.
7. Compare the current effect against this mask/erosion variant on identical captured poses,
   with no camera/exposure changes. Review normal speed, slow playback and isolated no-post
   frames. Check all layers disappear on interruption and never move the actor.

For AI-assisted production, this experiment favors generating mask/flow artwork, then controlling
motion with mesh UVs and shader phase. It avoids depending on the generated 16-frame atlas to
maintain a stable pivot and coherent silhouette, problems found in our prior flipbook study.
That is a recommendation for this project based on the two studies, not a universal rejection
of flipbooks; carefully authored flipbooks remain a distinct option shown in the tutorial.

## Extraction Notes

Temporary overview/detail sheets are under `/tmp/roi-anatomy-slash-study`. The full clip was
sampled every 20 seconds, then the technical section from 03:20 onward every 10 seconds.
The ice section was additionally inspected at 2 fps, with a 30 fps detail sequence around
08:42.5. Overview and detailed grids are sampling aids, not full-frame playback transcripts.

For a single source frame:

```sh
ffmpeg -ss 520 -i 'Anatomy Of A Slash - onetupthree (720p, h264).mp4' \
  -frames:v 1 ice-highlight-08m40s.png
```

For tiled diagnostics with a sparse select filter, use `-fps_mode vfr`; otherwise the output
image sequence can duplicate the first contact sheet to satisfy the original nominal frame rate.
