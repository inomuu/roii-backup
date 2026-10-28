# Region 1: material relief study

The user requested another implementation pass after comparing rock highlights and ground depth
with the selected EnvRef concept. Camera, lights, shared colour grade, composition, wind and
atmosphere were retained so the material response can be compared directly.

## Stone

`QualityProps/EnvRef/WeatheredLimestone_Relief.mat` is a sibling material, preserving the previous
material. It uses the stone albedo's companion normal map from the owned nature pack,
normal strength .38, smoothness .34, metallic 0. Applied to the wall and shrine stonework.

Inspection exposed invalid tangent values on the existing wall mesh (approximately -431602100
in each component). A nonzero tangent-array length did not prove valid data. Both stone meshes
were copied to `_Relief` assets and their tangents recalculated before final review. Scene instances
reference the repaired meshes through saved prefab overrides; previous meshes remain available.

## Ground

Two normal-source PNG copies are imported using Unity's Convert to Normal Map, derived from
the corresponding SelectedMeadow and SelectedTrail albedos. These are **brightness-derived
relief approximations**, not artist-authored physical height maps. They replace the unrelated
ground normal maps used previously. No new image-generation call was made.

- Meadow source height scale .16, terrain normal strength .32, smoothness .11, tiling 6.1 m.
- Trail source height scale .22, terrain normal strength .24, smoothness .16, tiling remains 4.2 m.
- Meadow blend varies at two spatial frequencies; the earlier quiet meadow underlayer remains.
- The road mask and all terrain heights remain unchanged. No gameplay collisions were added.

Stronger trial normals made the gravel excessively rough; the final strengths were reduced
after inspecting the camera render. Rock specular response is currently controlled by a constant
smoothness value, not a separately painted roughness/moss mask.

[Before](art-review-r1/material-before.png) · [After](art-review-r1/material-after.png)

These are Unity engine captures. Their foliage phases can differ because the wind remains active.

Validation: Play Mode reached tick 724 with EnvBaked=true and no console errors; characters
remained readable. Scene validation reported zero issues, missing scripts, or broken prefabs.
The scene was saved in Edit Mode. [Play capture](art-review-r1/material-play.png).
