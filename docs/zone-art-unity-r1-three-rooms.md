# Painterly meadow: three-room layout

Saved scene: `unity/ROI/Assets/Scenes/Zone_r1_Painterly_3Rooms.unity`.
The approved single-room `Zone_r1_Painterly.unity` remains available.

Three authored rooms have centers at Unity X = 0, -52, -104 metres. Terrain spans
156 metres with matching edge heights and terrain blend weights. Each room has
independent terrain data and scenery; materials remain shared. The first Ground
stays directly under Arena/Field for existing baked-environment adoption.

Removed the old Surround terrain and six Canopy_Shade trees from this alternative
scene to clear the connecting views. Removed two duplicated BackBrushThicket_9
cards at room joins and moved the preceding BackBrushThicket_0 cards by
(-0.8, 0, -0.25) metres to separate overlapping foliage planes.

Approved appearance remains exposure 0 with color grading disabled. The earlier
front-wall fern removal and enlarged white flower bushes are retained.

Validation: both joins captured after the foliage fix; scene validation reports
zero missing scripts or broken prefabs, and Unity console reports no errors.
These are Edit Mode checks, not a runtime traversal test. Camera and gameplay
room transitions have not been implemented by this layout change.

- [Join 1](art-review-r1/three-rooms/seam-1.png)
- [Join 2](art-review-r1/three-rooms/seam-2.png)

Editor authoring helper: `Assets/Editor/R1ThreeRoomLayout.cs`. Its Build command
refuses to overwrite an existing alternative scene. View Room 1–3 menu commands
move the editor scene camera for inspection.
