# Painterly semi-cel study

Alternative scene: `unity/ROI/Assets/Scenes/Zone_r1_Painterly.unity`.
Original: `unity/ROI/Assets/Scenes/Zone_r1.unity`, retained unchanged. Additional archive:
`art-review-r1/painterly-study/original-scene-assets.tar.gz` (scene, r1 assets, atmosphere and shaders).

New terrain data, newly painted road/grass masks, four generated textures, two reusable foliage
prefabs, independent materials and volume profile live in `Assets/Environment/PainterlyMeadow`.
The grass and dirt use broad painted marks rather than photo detail. Stone uses a soft stepped
diffuse shader blended with continuous lighting. Foliage remains wind-animated alpha cards;
the shrine uses Lit shading so its lantern illuminates the stonework.

This rebuild reuses structural wall, fence, shrine and boulder meshes and the atmosphere assets.
It does not claim all-new models. Old dressing is disabled in the alternative; original assets
are not edited. `R1PainterlyStudy.cs` is an Edit Mode initial authoring helper, not a runtime
component or an exact replay of the final manual tuning. The saved scene is the final result.

Final grade: exposure +.85, contrast 10, saturation -5, white filter; split shadows (.4,.54,.6),
highlights (.55,.53,.48). Sun 1.65, warm (1,.94,.8), shadow strength .68; canopy fill .85,
cool (.58,.76,.86). Existing fixed camera retained. Meadow tiling 7/10.1 m, dirt 4.6 m.
The middle-distance wall and foliage band remain above the open combat floor.

Validation: scene validation zero missing scripts/broken prefabs; Play Mode tick 345,
EnvBaked=true, zero console errors. Final foreground boulders were subsequently reduced in
Edit Mode. Initial active dressing bounds did not overlap the combat rectangle.
Original files were compared against the archive with SHA-256 (excluding archive AppleDouble metadata).

[Edit Mode capture](art-review-r1/painterly-study/unity-edit.png)
and [Play test capture before final boulder reduction](art-review-r1/painterly-study/unity-play.png).

## Image generation

Built-in imagegen was used; generated alpha preserved, Unity import capped at 1024 pixels.
Prompts used the following production specifications:

- Meadow.png: Square seamless repeating meadow albedo, orthographic top down. Painterly realistic
  semi cel-shaded fantasy, broad angular brush dabs suggesting overlapping low grass/leaves,
  restrained sage olive base, muted yellow green facets, tiny earthy gaps. Uniform diffuse light,
  moderate contrast, no outlines, cast shadows, perspective, trees, bushes, flowers, path, text or
  border. Full bleed tileable edges, dabs 1–2% of width, intended coverage 5 metres.
- Trail.png: Square seamless dry sandstone ochre dirt albedo with scattered small flat angular
  pale stones embedded flush. Hand-painted realistic semi cel-shaded fantasy RPG, broad angular
  brush dabs, simplified three-tone facets, sandy beige cream warm earth. Orthographic top down,
  uniform diffuse lighting, restrained contrast. No grass, objects, crossing paths, perspective,
  directional shadows, vignette or text; tileable edges, intended coverage 5 metres.
- Hazel.png: One broad lush asymmetric hazel shrub billboard, overlapping branches, base pivot,
  actual transparent background and holes, elevated 42-degree three-quarter view. Hand-painted
  realistic semi cel-shaded fantasy, angular sage olive/yellow green leaves in three-tone masses,
  teal shade, soft warm upper-left light, slender brown branches, small white blossom at base.
  Whole shrub with margin, wide 3:2, no outlines, photo grain, ground patch, rock, landscape or text.
- Oak.png: Single mature upright fantasy oak billboard, full base-to-canopy tree, transparent
  background and margin. Natural tapered trunk and spreading branches, irregular broad canopy,
  angular small leaves in light/shadow masses. Elevated 42-degree three-quarter view, painterly
  realistic semi cel-shaded RPG, sage olive/yellow green foliage, teal shade, upper-left warm light,
  flat brush facets, natural anatomy. No outlines, photo grain, ground slab, rocks, backdrop or text;
  centered landscape 3:2 with crisp alpha cutout and holes.

Remaining art differences: grass still reads as a repeating surface rather than varied authored
clumps; foliage uses two recurring silhouettes; reference brushwork and stone moss are richer.

## V2: painted ground and full-bush cards

This section supersedes the initial study's tuning and dressing notes above. The original
`Zone_r1.unity` remains retained; the previous alternative is archived at
`art-review-r1/painterly-v2/previous-study.tar.gz`. V2 is authored and saved in
`Zone_r1_Painterly.unity`. Its saved values and active objects supersede the editor helper trials.

The saved composition uses **10 back and 6 foreground full-bush image cards** through the reusable
`BrushThicket.prefab` and shared subdivided breeze mesh. `ROI/Painterly/BrushFoliageBreeze` retains
the existing foliage wind and Lit rendering; the cards animate in Edit/Play Mode, contain painted
lighting, and do not cast shadows. Foreground depth of field uses 195 mm focal length, aperture
3.2, and focus distance 20.5 m.
Native stock Bush/Plant instances and the sparse `PaintedBranchShrub` study remain retained
inactive. Trees, structural props, and the open combat floor keep their separate scene geometry.

New assets live in `Assets/Environment/PainterlyMeadow/V2`. Asset brief summaries:

- `ShortMeadow.png`: painted short grass, irregular sage/yellow-green brush marks and quiet
  patches of exposed earth. The two terrain layers use 15 m and 21.3 m tiling with an offset.
- `PaleEarth.png`: pale cream/ochre dirt with sparse small gray stones and broad low-contrast
  brushwork; the road layer uses 7 m tiling.
- `PaintedBranch.png`: a transparent branching stem with distinct broad painted leaves;
  created for the branch-mesh study rather than a dense stock leaf atlas.
- `PaintedStone.png`: painted limestone, warm light planes, cool gray planes, restrained cracks
  and moss marks, intended for the existing stone geometry.
- `PaintedThicket.png`, material, and prefab: the initial V2 whole-bush pass with exposed branches,
  golden leaf tips and white flowers, preserved unused after the user's simplification request.
- `BrushThicket.png`, material, and prefab: the active simpler shrub for both foliage bands.
  The generation brief requests 8–12 broad connected colour masses, roughly 45–70 decisive
  brush dabs, three to four sage/olive/golden/blue-green value steps, sparse visible branches,
  and no tiny leaves, veins, stippling or flowers. See the
  [exact selected prompt and output caveat](art-review-r1/painterly-v2/brush-thicket-prompt.txt).

**`BrushThicket.png` has an opaque neutral checkerboard, not native alpha transparency.**
`BrushFoliageBreeze` removes low-saturation pixels during rendering in both ForwardLit and
DepthOnly, before material tint, lighting, fog or grading. The saved normalized-saturation cutoff
is .32; material base colour is (.82,.87,.84). Reuse the prefab with this shader for the cutout;
the PNG alone still contains the checkerboard.

The painted whole-bush shape still repeats, and the native tree silhouettes still differ from
the reference. The image cards depend on the fixed camera angle.

Final V2 validation: 16 cards, zero combat-bounds overlaps, zero shader errors, zero scene
validation issues; final Play Mode capture at tick 586 with `EnvBaked=true` and zero console
errors. Actors remain visible and the card cutouts show no visible checkerboard. Play Mode was
stopped and the editor scene is saved clean. SHA-256 comparison verified all 182 original
archived files unchanged.
[Edit Mode capture](art-review-r1/painterly-v2/unity-edit.png) and
[Play Mode capture](art-review-r1/painterly-v2/unity-play.png).

## Stone sheen, rear palette and silhouette variations

This pass refines the V2 alternative's stone highlights and rear foliage colours to follow the
hand-painted reference. The pre-pass state is retained in
`art-review-r1/painterly-materials/before-assets.tar.gz` and
[before.png](art-review-r1/painterly-materials/before.png).

The new `V2/PaintedStoneSheen.mat` uses `ROI/Painterly/PaintedStoneSheen` on 25 stone renderers.
Continuous broad highlights and smaller glints replace the former narrowly gated reflection;
both respect light direction, attenuation and shadows. Final material values are smoothness
.55, sheen strength .70, albedo saturation .85, sheen colour (1,.87,.65), base colour
(1.04,.99,.92), and cel blend .35. The shadow tint is retained. Forward+ directional and
clustered light loops also include the existing bounce and spot lights. Painted albedo and
the structural stone meshes remain in use.

`ROI/Painterly/BrushPaletteBreeze` remaps foliage albedo before standard URP lighting. The two
rear materials, `RearThicketGold` and `RearThicketOlive`, use the following shadow/midtone/tip
palettes, with blend strength .92 on the original 10 rear cards:

| Material | Shadow | Midtone | Lit tips |
| --- | --- | --- | --- |
| Gold | (.21,.27,.21) | (.49,.48,.275) | (.86,.72,.38) |
| Olive | (.18,.25,.20) | (.42,.47,.29) | (.71,.69,.41) |

The foreground retains its prior material. Alpha cutoff, wind and depth rendering remain
consistent with the foliage silhouette. The original `BrushThicket.png` still requires chroma
removal; changing its palette does not turn the source into an alpha image.

Two additional generated silhouettes, `V2/BrushLow.png` (1536×1024) and
`V2/BrushUpright.png` (1086×1448), add low spreading and taller upright forms to the existing
thicket. Both contain actual alpha transparency; their palette materials use chroma cutoff 0
and ordinary alpha cutoff. Generation prompts are recorded in
`art-review-r1/painterly-materials/variation-prompts.txt`.

The final composition contains 19 active cards: 13 rear and 6 foreground. The rear band uses
4 original thickets, 6 low bushes (including 3 companions beneath upright bushes), and 3
upright bushes. New variants use palette strength .94 and `_PaletteValueScale` 1.55 before
the luminance remap; original Gold/Olive thickets retain strength .92 and value scale 1.
New-variant alpha cutoff is .42 with chroma cutoff 0; Unity imports are capped at 1024 pixels.
Reuse `V2/BrushLow.prefab` and `V2/BrushUpright.prefab`; the upright shape uses the portrait
`UprightBreezeGrid.asset` mesh.

Final validation: zero shader compile errors, zero scene validation issues, and zero card
bounds overlapping the combat rectangle. Play Mode reached tick 338 with `EnvBaked=true` and
zero console errors. The scene is saved. See the
[final Edit Mode capture](art-review-r1/painterly-materials/after.png) and
[Play Mode capture](art-review-r1/painterly-materials/play.png).
