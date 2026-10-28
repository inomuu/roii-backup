# Free Slash VFX: Inspected Source

Inspected 2026-09-25, following the user's Add to My Assets action. The cache
archive is readable and contains 181 pathname entries. The initial source review
was followed by an Editor-only fire-prefab render. Gameplay, saved scenes and
project settings are unchanged.

## Locations

- Cache: `/Users/lunalieneray/Library/Unity/Asset Store-5.x/MaykerStudio/Shaders/Free Slash VFX.unitypackage`.
- Reconstructed source: `/private/tmp/roi-free-slash-study/source/Assets/Free Slash VFX/`.
- Parsed graph / prefab evidence: `/private/tmp/roi-free-slash-study/inspection.json`.
- FBX polygon / UV data: `/private/tmp/roi-free-slash-study/meshes.json`.

The full extraction stays outside the repository. The fire prefab's 32-asset
dependency closure is staged under
`unity/ROI/Assets/Editor/Vfx/References/MaykerStudio/FreeSlashVFX/` with original
GUIDs. `source-manifest.json` records source hashes. No demo scripts, scenes,
URP settings or unrelated prefabs were imported. These are licensed vendor
assets, not our original work; do not redistribute as a standalone asset pack.
The mesh diagram below is an analytical projection, not a Unity render.

## Fire Render

![Fire prefab in the battlefield](mayker-fire-peak.png)

- [Original speed, 60 fps / 2.8 seconds](mayker-fire-real-time.mp4).
- [One-third speed, 20 fps / 8.4 seconds](mayker-fire-slow.mp4).
- [Portrait, 540 x 960 / 60 fps](mayker-fire-portrait.mp4).

Entry point: `MaykerFireSlashPreview.Capture`, or the menu
`ROI/Art/Capture Mayker Fire Slash Reference`. Raw PNGs go to
`/tmp/roi-mayker-fire-slash/`. This instantiates the actual
`Prefabs/Slash Fire VFX.prefab`, including its original materials, textures,
meshes, sparks and distortion. It does not port the effect into our shader.

Preview-only adaptations:

- Uniform actor-relative scale and root rotation toward the two targets.
- Existing bright Suzu battlefield fixture, with the camera pulled back to fit
  the long sparks. Actor attack art is the existing swordsman placeholder.
- URP camera color/depth textures enabled for the vendor distortion shader.
- Fixed particle seeds and explicit cast-age simulation. Temporary render
  passes pin shader time before shadows, opaques and transparents so background
  foliage and vendor noise use the same sampled clock. Vendor shaders are not edited.
- The prefab's point light fades out between ages 0.08 and 0.5 seconds so it
  does not remain lit after particle expiry.
- No custom Suzu slash, hit flash or added hit particles in this capture.

This is a full-prefab integration study, not an isolated main-mesh A/B. Its
original HDR color reads yellow-hot under our exposure; no fire recoloring was
applied. It does not change the live skill.

Validation on Unity 6000.0.83f1: both viewports pass visible-vs-hidden pixel
energy, same-age restart, movement, surviving tail and full expiry checks.
RGB absolute-difference energy at peak: 10,381,925 landscape; 7,085,943 portrait.
Hidden-effect, frozen-frame and changed-age restart negative controls are rejected.
Restart comparison allows a mean error below 0.1 8-bit level per channel and a
maximum local error of 8; HDR readback is not claimed to be bit-identical.
Two camera renders per sample settle the newly enabled light before readback.
The 32 source hashes are recorded in the manifest. A subsequent post-exit audit
found 25 byte-identical assets and 7 materials reserialized by Unity/URP. Original
palette and texture bindings are preserved; the flying-fire material's existing
`_DstBlendAlpha` migrates from 0 to 10, with additional importer defaults elsewhere.
Do not treat Unity-reserialized `.mat` files as byte-identical package originals.

```sh
"/Applications/Unity/Hub/Editor/6000.0.83f1/Unity.app/Contents/MacOS/Unity" \
  -batchmode -projectPath "$PWD/unity/ROI" \
  -executeMethod MaykerFireSlashPreview.Capture -quit \
  -logFile /tmp/roi-mayker-fire.log
ffmpeg -y -framerate 60 -i /tmp/roi-mayker-fire-slash/motion-landscape-%03d.png \
  -c:v libx264 -crf 18 -pix_fmt yuv420p -movflags +faststart \
  docs/vfx/suzu-sunder/free-slash-pack/mayker-fire-real-time.mp4
```

Use input framerate 20 for the slow clip; use `motion-portrait-%03d.png` for
portrait. Do not run a second Editor on this project while it is already open.

Re-stage from the locally licensed archive with
`python3 tools/import_free_slash_reference.py "<cache path>.unitypackage"`.
The importer refuses to overwrite changed files. Unity may migrate imported
metadata, so re-staging into an already imported folder is intentionally not
an automatic overwrite workflow.

## Solid Fire Orbit

Separate adaptation requested after reviewing the original prefab render:

- [Real time](mayker-fire-orbit-real-time.mp4).
- [One-third speed](mayker-fire-orbit-slow.mp4).
- [Portrait](mayker-fire-orbit-portrait.mp4).
- [Contact frame](mayker-fire-orbit-peak.png).

Use `MaykerFireSlashPreview.CaptureOrbit`, menu
`ROI/Art/Capture Mayker Solid Fire Orbit`. PNG output:
`/tmp/roi-mayker-fire-orbit/`. The same fixture, exposure, bloom and camera framing
are retained; no global lighting or gameplay changes are made.

`MaykerFireOrbitStudy.cs` owns the temporary variant:

- Fixed pivot at caster position plus 0.43 actor heights; one clockwise world-Y
  revolution over 0.78 seconds, then a stationary dissolving tail.
- Original vendor mesh, UVs, texture bindings and Shader Graphs. Material copies
  are owned by the study and destroyed on completion; reference assets receive no
  variant tuning (Unity's material migration is described above).
- Higher main opacity (5.32 to 16), lower HDR color and saturated orange body.
  Refraction strength is zero; the root shader's existing scene-color modulation
  remains, so this is more solid-colored, not a newly authored opaque shader.
- A 1.025x copy of the original main carrier renders below the orange body with
  a golden material, leaving a narrow exposed bright edge. It uses the same seed,
  particle age and textures, not a separately generated mask.
- Point-light intensity reduced to 12 percent, warm tint; smaller flying embers.
  The source's large single-contact impact star is disabled for this orbit.
- Source particle age progresses from 0 to 0.20 during the sweep, then advances
  1.8 seconds per second through the original dissolve and particle expiry.
  This is a deliberately retimed skill study, not original prefab timing.

The capture measures a full -360-degree accumulated yaw with a fixed pivot and
rejects a sabotaged frozen rotation. Existing image checks cover visibility,
same-age restart, changing silhouette, tail and expiry at both viewport sizes.
Both viewport checks passed on Unity 6000.0.83f1, with peak RGB difference energy
4,755,258 (landscape) and 3,429,526 (portrait), and hidden/frozen negative controls
rejected. No scene, pipeline, sim or live skill asset was saved by this study.
The caster still uses the existing placeholder attack poses; no new character
animation or live damage/hit timing is implied.

## Highlight And Accents

Latest iteration, retaining the mesh and adjusting coverage through UVs:

- [Real time, faster/brighter v2](mayker-fire-orbit-fx-v2-real-time.mp4).
- [One-third speed](mayker-fire-orbit-fx-v2-slow.mp4).
- [Portrait](mayker-fire-orbit-fx-v2-portrait.mp4).
- [Contact frame](mayker-fire-orbit-fx-v2-peak.png).
- [Previous speed and ember size](mayker-fire-orbit-fx-real-time.mp4).

Entry: `MaykerFireSlashPreview.CaptureOrbitAccents`; raw frames:
`/tmp/roi-mayker-fire-orbit-accents/`. The original solid-orange orbit route and
movies above are preserved for comparison.

V2 runs the sweep, texture clock, contacts and accents together at 1.18x, taking
the revolution from 0.78 to approximately 0.661 real seconds. Anticipation and the
placeholder sprite animation are unchanged. Ages below refer to the effect clock.

The white upper highlight uses the **same imported mesh** as the orange layer,
not a narrowed/deformed mesh. `Shaders/FireOrbitCoverage.hlsl` multiplies the
vendor alpha by a UV coverage profile, accounting for the broad lower V island
and the raised surface's narrow upper island. It retains the vendor texture
sampling, custom particle data and dissolve. The white layer selects a narrower
radial region of this mask. No vertices, triangles or source UV coordinates change.

Only the final 42% of U tapers to a point. The beginning and middle retain full
coverage; the initial two-ended taper experiment was rejected by the user.
Source textures can still erode their own silhouettes independently of this mask.

`tools/build_fire_orbit_graph.py` deterministically derives the Editor-only
`Shaders/FireOrbitCoverage.shadergraph` from the vendor graph using JSON parsing.
It adds a fragment-alpha custom-function node and two controls (`_WhiteBand`,
`_Taper`), without changing vertex outputs or the licensed source graph. Regenerate
the derivative with `python3 tools/build_fire_orbit_graph.py`; edit the HLSL for
coverage tuning. This is a vendor-derived study, not an independently authored pack.

`MaykerFireOrbitAccents.cs` adds separately sampled layers:

- Warm ground pool and a local point light, fading completely by age 1.2 seconds.
- Existing `SwordHitImpact` on each fixture target at ages 0.09 and 0.19 seconds,
  using the fixture's orange/gold accent. No global hit-flash changes.
- Nine staggered ember fans (eight particles each), emitted just beyond the sweep
  radius with fixed seeds; they drift outward/up and shrink/fade over 0.36-0.64
  effect-clock seconds. V2 sizes are 0.09-0.14 actor heights (about 1.64x the
  previous size), with gold-white color and emission gain 5.2 instead of 3.8.
  Lifetime compensates for the faster clock so the embers remain readable.

These contact times are staged preview events, not damage calculation or runtime
collision detection. The sim and live skill wiring remain untouched.

Capture checks isolate white, light, hit and ember layers and measure their pixel
contribution against a disabled-layer render; hidden-layer negative controls must
fail. Existing same-age restart, movement, tail, expiry and full-revolution checks
also run in landscape and portrait. Source FBX, textures and original shader graph
hashes remain byte-identical to the manifest.

The first tail-only capture (before v2) passed on Unity 6000.0.83f1.
Isolated RGB difference sums:

| View | White Band | Ground/Light | Hits | Embers |
| --- | ---: | ---: | ---: | ---: |
| 1280x720 | 5,153,539 | 2,728,915 | 2,461,019 | 22,371 |
| 540x960 | 3,676,626 | 1,810,499 | 1,607,038 | 17,362 |

V2 passes the same capture and negative-control checks at both viewports.
Isolated ember difference sums are 113,264 (landscape) and 83,380 (portrait).
These samples also advance on the faster clock, so they are visibility evidence,
not a same-particle-age brightness ratio against v1.

## Caster Aura (V3)

- [Real time](mayker-fire-orbit-fx-v3-real-time.mp4)
- [One-third speed](mayker-fire-orbit-fx-v3-slow.mp4)
- [Portrait](mayker-fire-orbit-fx-v3-portrait.mp4)

V3 preserves the accepted v2 slash mesh, UV coverage, speed, white highlight and
embers. `MaykerFireOrbitAccents` now also owns the shared `BuffAura` helper with
the existing `SeraphineAuraFlow-v1` texture, recolored orange/gold. It rises during
anticipation and fades after the sweep. No new texture or global lighting edit.
The accepted v2 videos remain available above.

The aura is included in separately disabled-layer capture checks. Isolated RGB
difference sums are 446,590 landscape and 313,875 portrait. Both viewports pass
restart, tail, expiry and all hidden/frozen negative controls. This remains an
editor fixture, not live Sunder wiring. The next single-target study is
[Bell Strike](../../suzu-bell-strike/README.md).

## What The Base Prefab Actually Uses

`Prefabs/Slash VFX.prefab` root renderer references `Materials/Slash.mat` and
`Model/Slash/Slash .fbx` (mesh file ID `7567532504055388576`). The root particle
has a 0.5-second lifetime and one birth burst. Its Rotation, Size and texture-sheet
UV modules are disabled. This base is not a flipbook and does not depend on
rotation-over-lifetime to move its visible pattern.

Two custom-data components ramp from 0 to 1 over normalized particle lifetime.
The Shader Graph reads UV1: its X component drives dissolve operations and its
Y component is used in the UV offset for the main and emission textures. Additional
ScrollUV subgraphs animate mask noise, dissolve sampling and distortion sampling.
Particle stream packing should still be confirmed in Unity's renderer inspector.

The prefab has four ParticleSystems: the main Slash, Smoke, Slash Distortion and
Flying. The distortion layer has a 0.2-second lifetime. A Point Light object is
also present. These support layers are separate from the main slash silhouette.

## Texture And Shader Roles

These are GUID-resolved bindings from the root `Slash.mat`, not a guess based
on filenames or the publisher's promotional screenshot:

| Slot | Actual Texture | Connected Role |
| --- | --- | --- |
| `_Main` | `Noise_GrayScale/T_Lu_Noise_31.png` | Main sampled alpha, multiplied by opacity then saturated. |
| `_MaskTex` | `Gradient 4.png` | Mask sampled through a rotated, noise-modulated UV path, multiplied into alpha. |
| `_DissolveTex` | `Noise.png` | Sampled alpha feeds One Minus, Step and Lerp controlled by UV1 X. |
| `_DistortionTex` | `Noise_GrayScale/T_Lu_Noise_29.png` | Normal From Texture offsets screen-space Scene Color sampling. |
| `_EmissionTex` | `Blue Noise_Blue Noise.png` | Optional sampled emission branch; `_Emission` is 0 in this base material. |

`Textures/Slash Noise_Slash Noise.png` is a blue painted slash also shipped in the
pack, but is NOT the root prefab's main texture. Do not present it as that shader's
base mask.

The root material uses alpha blending (`SrcAlpha`, `OneMinusSrcAlpha`), no Z write,
both faces, `_Opacity=5.32`, `_Distortion=0.009`, and HDR color RGB approximately
8.574. Alpha also includes vertex alpha and a scene-depth soft intersection factor.
These values are evidence, not recommended values to copy into our scene blindly.

## Mesh Evidence

![Source mesh and UV comparison](mesh-uv-comparison.png)

The FBX contains six named meshes, each with 72 quad faces / 144 triangles:
`Circle`, `Circle.010`, `Circle.011` have 125 control points;
`Circle.001`, `Circle.006`, `Circle.007` have 100. They fall into two position-shape
groups with local thickness-to-largest-span ratios of about 0.044 and 0.204.
The plot shows one representative from each group. The subsequent Unity import
resolved the fire prefab's root mesh to **Circle.005**, with 125 imported vertices
and local bounds extents approximately (0.73, 0.73, 0.03). This is a shallow carrier,
not the taller narrow-band representative. Unity's imported name differs from
the raw parser names, so the diagram must not be treated as an exact submesh-ID
mapping. Raw FBX control-point counts are not generally interchangeable with
Unity's imported split-vertex count.

The representative `Circle` UVs occupy a broad lower strip and a separate narrow
upper strip, whereas our two surfaces overlap the same full 0-1 strip with mirrored
U. Its raised rows sit toward the outside of the band rather than our middle row.
The `Circle.001` representative is a taller narrow-band carrier. This rules out
assuming that every mesh in the pack is the same mirrored lens layout as ours;
an exact imported-root UV checker comparison remains to be done.

Our plotted data comes from the actual `SuzuSlashBaseMesh.asset` vertex/index
buffers: 390 imported vertices / 512 triangles, a 270-degree arc, mirrored UV
strips and a central ridge. FBX positions were read using the upstream ufbx C
loader, then normalized for the diagram. An experimental Python binding returned
invalid coordinate magnitudes and was discarded; its vertex arrays are not used.

## Difference From Our Current White Base

Our `SuzuSlashBase.shader` samples one code-authored wedge, thresholds it around
0.5, slides it once through a bounded U window, and fades opacity near expiry.
It does not yet have the pack's independent moving mask-noise and dissolve paths,
screen distortion or separate support particles in the white-base fixture.

This is why copying a jagged texture outline alone does not reconstruct this
package's pipeline. It does not prove which feature makes it look better:
that needs a controlled render, not a source-code comparison.

## Next Controlled Comparison

1. Relevant fire dependencies are now staged Editor-only, preserving GUIDs;
   no demo URP settings or scripts are imported.
2. Root FBX submesh is resolved; render its geometry / checker UV against ours next.
3. Capture the package's base particle with smoke, distortion and flying particles
   disabled, with the same camera, screen size, exposure and bloom as our base.
4. Compare main coverage alone, then add mask noise and dissolve independently.
5. Restore support layers last. Adapt lifetime control to our sampled cast age
   rather than introducing free-running global time into deterministic previews.

No isolated matched-camera base A/B or live integration has been completed.
The working Suzu v3 preview remains unchanged; the vendor fire render is separate.
