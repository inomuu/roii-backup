# Reference Base Mesh: Geometry Only

This is the base-mesh study requested after reviewing Anatomy Of A Slash
at 03:02-03:06 and the user's screenshots. The asset itself has no material.
On 2026-09-25 the user identified the missing center thickness: the previous
carrier was flat, while the intended section is lens-shaped. The center rows
now rise above and below the plane, tapering to sharp inner/outer edges.
Geometry approval is pending. Dimensions, subdivision count and UV mirror axis
remain locally authored, not recovered values from the video. No additional
effect tuning or gameplay integration was performed during this correction.

## Shape And UVs

- Open 270-degree arc in local XZ, with a 90-degree gap and square ends.
- Constant inner/outer radii 0.85 / 1.7, 64 angular segments and two radial bands.
- Three radial vertex rows: inner, middle, outer. No time-varying deformation or tilt.
- Separate upper/lower surfaces: center ridges at Y = +0.16 / -0.16, with both
  inner and outer rows at Y = 0. Maximum thickness is 0.32 local units.
- The two radial bands on each surface make a faceted lens/diamond cross section.
  The 270-degree angular ends are uncapped. This is not a closed manifold solid.
- Front UVs: U follows the arc from 0 to 1; radial V is 0, 0.5, 1.
- Back UVs: U is mirrored to 1 minus front U; V is unchanged. Both islands occupy
  the same rectangular UV domain. Rendering both sides does not depend on Cull Off.

The clip states that the UV layout is mirrored for each side. We implement that as
front/back U mirroring; the video does not supply its original mesh file or exact
vertex correspondence. Dimensions and segment count above are local authored values,
not values recovered from the author's project.

## Files

- Native mesh: `Assets/Resources/Effects/Suzu/SuzuSlashBaseMesh.asset`.
- Generator: `Assets/Scripts/Arena/Vfx/Suzu/SuzuSlashBaseMesh.cs`.
- Capture/checks: `Assets/Editor/Vfx/Suzu/SuzuSlashBasePreview.cs`.
- [Current lens-section preview](base-mesh-lens.png): angled view on the left;
  low-angle view above right; measured radial cross section below right.
- [Earlier flat mesh and UV overview](base-mesh-overview.png), retained as history.

Capture menu: `ROI > Art > Capture Suzu Slash Base Mesh`.
Batch entry: `SuzuSlashBasePreview.Capture`.
Temporary images: `/tmp/roi-suzu-base-mesh`.
The generator updates the native asset in place, retaining its GUID. Rendering uses
an isolated Unity preview scene and does not discard or save an open scene.

The overview's wire lines, grid and UV colors are diagnostics only. The native mesh
has no material or texture. UV colors identify corresponding angular columns, so
their order reverses in the back chart; they are not the effect's final palette.

The earlier flat overview orders top view, angled view, front UV and back UV.
The lens preview's section outline is sampled from actual inner/ridge/outer
vertices, not an illustration of a proposed shape.

## Verification

Lens correction passed Unity batch capture on 2026-09-25. Flattening all Y
coordinates tripped the geometry guard. An edge-on rendered comparison against
a separately instantiated flat mesh changed 42,662 pixels; substituting the
flat control again tripped the volume guard. UV and hidden-mesh guards also
passed. Topology remains 390 vertices and 512 triangles. This checks the base
mesh only; the previous static-effect movie still shows the flat carrier.

The earlier flat version passed Unity batch capture on 2026-09-24. It contained 390 vertices and
512 triangles total (195 vertices / 256 triangles per side). Geometry checks
measured the radii, flatness, 270-degree opening, rectangular UV coordinates and
opposite face winding. Deliberate displaced-vertex, closed-gap, warped-UV,
unmirrored-back and reversed-face mutations were all rejected.

The mesh contributed 236,280 pixels in the isolated top-view check; hiding its
renderers was caught. Mirrored UV correspondence changed 368,002 pixels between
the two charts; substituting the front UVs in the back chart was caught. This is
a geometry-stage check, not confirmation of texture motion or live skill integration.
