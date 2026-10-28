# Free Slash VFX: Supplementary Reference

User supplied this reference on 2026-09-25, alongside Anatomy Of A Slash.
The package became available in the local Asset Store cache on 2026-09-25.
It was extracted and inspected outside the Unity project, then its fire-prefab
dependencies were staged for a real Editor-only render. The earlier publisher-only
findings below are retained separately from source evidence.

## Local Source Review

See [the inspected package comparison](free-slash-pack/README.md) and
[mesh / UV diagram](free-slash-pack/mesh-uv-comparison.png).
The fire prefab's 32 dependencies are under `Assets/Editor/Vfx/References/` in
`unity/ROI`, separate from runtime assets. No demo settings or scenes were imported;
the other open Unity workspace was not modified.

## Confirmed From Publisher Pages

- [Unity Asset Store listing](https://assetstore.unity.com/packages/vfx/shaders/free-slash-vfx-309498):
  MaykerStudio, version 1.0, 13.1 MB; lists URP compatibility for Unity 6000.0.34f1
  and 2021.3.45f1. This is not a runtime test in our 6000.0.83f1 project.
- [Publisher demo page](https://maykerstudio.itch.io/free-slash-vfx-demo):
  describes custom slash shaders, glow/color/intensity controls, distortion,
  prefabs, and adjustable speed/shapes. The WebGL demo has not been played here.
- During the initial publisher-only review, package source was not yet available.
  It is now present in the local cache and staged for Editor-only preview.

## Render Validation Still To Do

1. Unity resolved the fire root to `Circle.005` (125 vertices, shallow bounds).
   Compare its cross section, normals and end caps in the same camera as our mesh.
2. Confirm the inspected UV islands with a numbered checker render.
3. Texture coverage versus geometry silhouette; clamp/repeat and UV offset curves.
4. Shader blending, HDR output, erosion, custom particle streams and distortion inputs.
5. Prefab timing and layer separation: base, highlight, soft support and impact.

Do not infer a lens-shaped mesh, a particular UV scheme, or the exact Anatomy
technique from promotional images. Do not import demo project settings or change
the shared URP pipeline merely to reproduce this reference. First inspect the
package in isolation, then adapt only the relevant presentation assets.
