# Unity Home menu — graphics worktree

เริ่มงานต่อ / current handoff: [Unity UI — Luna 5.6](unity-ui-luna-handoff.md) (2026-09-14).

Implemented on `codex/unity-graphics-ui` in the isolated
`idle_mmorpg-unity-graphics` worktree. Preview Unity instance: `ROI@ae78bbd46c76d37e`.
Do not operate the shared checkout or the JRPG Editor when previewing this work.

## Layout and routes

The menu uses generated library and castle artwork, ivory cards, gold/ivory
emblems and Barlow Condensed semibold headings. The Store/Excavation row has
blue/teal faces and white labels. Barlow also supplies utility labels and the mode
switch and wallet balances; profile and legacy windows retain their existing typography.
The existing Live2D model fills the left side. Hair, cape and stars may extend
past screen edges; there is no narrow portrait rectangle clipping them.
A single upper-right button switches Menu/Combat.

The main deck has four staggered rows: Story with an overlapping Event inset,
Mission/Workshop/Inventory, Exemplars/Party, then Store/Excavation. The main deck
extends from anchor y = 0.055 to 0.885, using nearly the full height below the header.

- The main label is Story, with a dark Coming Soon band across the bottom. The band
  is presentation only; the existing Story route remains accessible for development.
  Starting a mission through that existing window returns to Combat.
- Event is a small landscape card overlapping Story's upper-right area; it opens Events.
- Mission groups Daily, Weekly, Pass and Events.
- Exemplars / Party occupy the same row.
- Exemplars opens the dedicated character screen described in `unity-exemplar-prototype.md`: Attributes, Regalia, Equipment, Potential, Apex and Resonance. The existing legacy Exemplar equipment collection is not used as the playable roster.
- Party groups Party setup, Commander and Paths. Paths is party-wide and is no longer in Exemplars. Provisions remains on Combat.
- Workshop is beside Mission. Inventory is a dark horizontal tab overlapping
  beneath its lower-right edge; Workshop draws in front of it. Both have moved out
  of the bottom utility row, which now contains Achievement only.
- Inventory groups Inventory, Cards, Stash and Album.
- Workshop groups Blacksmith, Enchant, Apothecary and Kitchen.
- Store occupies the left side of the fourth main row and groups Shop, Trade and
  Crystal. It replaces the former bottom-row Shop button.
- Achievement groups Bestiary and Record.
- Excavation occupies the right side of the fourth main row and opens Summon.
  A floating black Recruitment strip sits above its title as a child of the same
  Excavation button; it does not introduce a separate recruitment mechanic or route.
- Options and Mail are small upper-left controls.

Cards and submenus route to working legacy windows. Their internal visual redesign
is still pending. Profile, Gil and Soul read the active session rather than mock values.
Gil / Soul / Gem occupy three separate chips with vector emblems and large balances.
Their entire `HomeWallet` root is parented to `MainMenuDeck`, so labels, symbols and
click targets share the cards' perspective camera, yaw and pointer motion. The
authoring anchors are (.475, .905)–(.884, .987). Currency groups pack against the
right edge with 12-unit gaps and fit the displayed number width. Icon frames are
64 reference units, balances use up to 46, and action discs are 36. Long balances
shrink within bounded per-currency widths. Profile and Combat stay flat. Gem is the existing server Crystal wallet (`tradeable + bound`),
not a new currency. Unknown/unconnected balances display an em dash. Gil opens
Store; Soul uses an arrow to Paths; Gem opens Crystal. These controls only navigate.
`HomeWalletBar` reads the sim purse every 0.5 seconds and refreshes the server wallet
on Home activation/connection change and every 30 seconds while visible. Requests
are cancelled on disable; stale responses are rejected by client/account/session
and generation checks. Premium decimal strings are parsed as UInt64, summed exactly
as decimal and formatted for display.
Escape closes Home groups or open windows; combat shortcuts are gated on Home.
Localization source is `godot/i18n/strings.csv`, mirrored into Unity StreamingAssets.
`UI_HOME_STORE`, `UI_HOME_RECRUITMENT`, `UI_HOME_GEM` and
`UI_HOME_COMING_SOON` supply the new English/Thai labels; `UI_HOME_STORY` now reads Story.

## Presentation lifecycle

`ModeTabs` changes presentation only. SimDriver and ArenaView stay enabled.
`HomePortrait` uses an independent transparent full-screen render target, preserves
aspect when resized, and excludes scene postprocessing. Its serialized framing values
control zoom and horizontal placement. The current camera offset lowers the face.
Only the instantiated Home model's Animator uses `animationSpeed = 0.5`. Shared
prefabs, clips, controllers and the cut-in player are unchanged; cut-in remains 1×.
The Home portrait suspends the arena camera; leaving Home restores its prior state.
Companion is closed before Home acquires the camera, preventing competing camera owners.

Combat clear/death/DPS panels are grouped under `CombatPopups`. Their CanvasGroup is
hidden and noninteractive on Home, while their handlers continue to receive sim state
(including automatic respawn). Home windows remain above the menu; the switch stays
above windows. The arena debug strip is hidden on Home.

Stop Play before recompiling scripts, then restart the graphics preview: hot reloading
can invalidate the native simulation handle. Local preview uses the existing dev boot
path (`ROI_BOOT=dev`, `ROI_STEAM_PIPE=0`) and does not write the online account.

## Reusable assets

Everything consumed by the menu is inside `Assets/Resources/HomeUI`:

- `LibraryBackground.png`, `CastleBanner.png`: generated background artwork.
- `MenuEmblems-Clean.png`: generated transparent 4 x 3 sprite atlas, sliced by
  `HomeIconSet` without modifying its source. Trilinear filtering and mipmaps reduce
  aliasing when displayed small. `HomeEmblem.shader` uses the source alpha and UI tint
  for gold on ivory and ivory on dark buttons, avoiding noisy RGB fringe pixels.
- `Portrait.prefab` and `PortraitLoop.controller`: existing model 67 and its loop.
- `BarlowCondensed-SemiBold.ttf`, `HomeHeading.asset`: active condensed display
  typography, also used by utility labels and the Menu/Combat switch.
  `BarlowCondensed-OFL.txt` contains the OFL license.
- `CormorantGaramond-Semibold.ttf`, `HomeDisplay.asset`: retained as an unused
  typography reference, instantiated at weight 600; `Cormorant-OFL.txt` is its license.
- `HomeUIRenderer.asset`: clean URP Forward renderer, registered at renderer slot 1
  in both PC and Mobile pipeline assets; no Cubism or SSAO renderer features.
- `HomePerspectiveComposite.shader`: composites the transparent card render target
  with premultiplied-alpha blending. `HomePanelShadow.shader` supplies separate soft
  and contact shadows. `HomePanelSurface.shader` adds broad right-side tonal facets
  and subtle lower-right dots, with derivative filtering to limit pattern aliasing.
  Each card owns a surface material so its pattern uses that card's aspect ratio.
- Earlier Crimson/engraved icon assets are retained for comparison and are not the
  active menu artwork.

`HomeIconSet` measures each atlas cell's visible alpha bounds, retaining a 4 px
filtering gutter. Main cards now use larger titles with a quiet, oversized watermark
emblem behind the text; they no longer reserve a separate leading icon column.
The watermark extends beyond the right/top/bottom boundaries of the face, with
subdued alpha so the title remains dominant. A child `WatermarkFrame` uses a uGUI
`Mask` with `showMaskGraphic = false` to stencil-clip the oversized print to the
projected card. Keep this stencil mask rather than substituting `RectMask2D`. Utility buttons retain small leading icons, fixed padding and
TMP cap-line alignment. The Inventory tab uses one simple vector storage-crate icon beside its label; its
watermark is omitted to avoid a second competing symbol.
`HomeCurrencyIcon` supplies retained vector currency and navigation symbols without
reusing an unrelated menu emblem.
The earlier aligned flat layout is captured in
`docs/previews/unity-home-menu-aligned.png`; it predates the current title and
watermark treatment.

Icon atlas order: Story, Mission, Exemplars, Party; Excavation, Inventory,
Workshop, Shop; Achievement, Options, Mail, Combat.

Active font source: https://github.com/google/fonts/tree/main/ofl/barlowcondensed
Earlier font source: https://github.com/google/fonts/tree/main/ofl/cormorantgaramond
Image generation used the built-in imagegen tool. Final prompt is recorded in
`unity-home-icon-prompt.md`.

## Validation

The prior presentation baseline was verified in the isolated Unity Editor:
compilation, pointer clicks switching both directions and restoring the arena camera,
sim ticks advancing on Home, and Exemplars → Character opening the existing window.
The earlier localization guard and `git diff --check` passed. Those checks predate
the latest perspective and typography changes; current validation status is recorded
below rather than treating earlier results as final validation of the new layout.

Capture caveat: `ScreenCapture.CaptureScreenshot(path, 2)` crashed this macOS Metal
Editor during final capture. Reopened the same isolated worktree and returned to
normal-size captures. Avoid supersize capture with this Live2D/URP preview; the exact
engine-side cause has not been isolated.

This is a menu graphics/presentation change. It does not change combat formulas,
account saves, server data or backend deployment.

## Perspective menu cards

`HomeMenuDepth` renders the cards through a real Perspective camera. The old
Screen Space Overlay rotation made every row slope in the same direction; that
orthographic treatment and its thick offset edge plates have been replaced.

The right-hand deck has a positive 24° Y rotation, a 35° camera field of view and
no Z roll. Its right edge is closer to the camera, so upper rows rise toward the
right and lower rows fall toward the right around the central eye level. Cards
have deliberately staggered positions instead of one uniformly tilted column.
The deck is scaled to 0.96 and shifted slightly left to fit the viewport. Bottom
utility buttons occupy a separate plane with a shallow −5° Y rotation and small
alternating vertical offsets.

The main deck now extends farther left: Mission begins at anchor x = 0.548 and
Story at x = 0.582; the other rows retain their deliberate stagger. Main title sizes
in canvas reference units are Story 100, Mission 78, Workshop 56, Inventory 36,
Exemplars 68, Party/Store 72 and Excavation 76. The inset Event caption uses 34,
Recruitment and Achievement use 32, the Coming Soon caption uses 28, and the
Menu/Combat switch uses 27.
TMP can shrink card text to 65% of the requested size when needed. Story reserves
its right side for the overlapping Event inset. Large, ghosted watermark icons
replace the small leading icons on main cards; the Event inset instead uses
landscape art with a dark caption band. Store and Excavation add blue/teal accents
to the fantasy library and ivory/gold palette. Regular ivory card faces have alpha
0.84 and cyan/dark faces 0.88, leaving restrained background visibility through
the printed surfaces. Watermark alpha is 0.21 on ivory faces and 0.26 on cyan faces. Existing Live2D framing is retained.

`HomeMenuVeil` adds a continuous dark backdrop gradient above the portrait but below
the card composite, spanning x = 0.46–1. Its alpha grows from zero at the left edge
to 0.48 at the far right, leaving the face/left side bright while separating the menu
from the cape and stars behind it. This custom `MaskableGraphic` requires a
`CanvasRenderer`; creation supplies one explicitly and the class declares that
requirement. It is noninteractive and does not alter scene lighting or exposure.

### Rendering and input

A separate `HomePerspectiveSpace` root contains a World Space Canvas on layer 29
and its camera. The canvas copies HomeLayout's reference dimensions. Camera distance
is derived from canvas height and field of view; the transparent ARGB32 render
target follows the actual Screen width and height. A noninteractive, full-screen
RawImage composites that target above the portrait and below the account header
and open windows. Cards are real uGUI Buttons, with a GraphicRaycaster using the
perspective camera, so input follows the projected faces rather than an approximation
of their old overlay rectangles.

The camera uses `HomeUIRenderer` at renderer slot 1, with HDR, postprocessing,
shadows, depth texture and opaque texture requests disabled. Its depth is −20 so
it renders before the final screen camera and Overlay UI. The PC pipeline's MSAA4
and the Forward renderer allow the target's requested four-sample geometry edge
antialiasing. The Mobile pipeline currently has MSAA1 and render scale 0.8: URP
requires pipeline MSAA to be enabled before honoring the target's AA4 request,
and its render scale also applies to this camera. Consequently, Mobile does not
currently provide the same card antialiasing or resolution as PC.

The composite uses premultiplied-alpha blending. The cards now occupy distinct
local Z positions rather than one coplanar sheet. Positive elevation below means
movement toward the camera (negative local Z):

| Face | Resting elevation | Side thickness |
| --- | ---: | ---: |
| Story / Excavation / utilities | 0 | 6 |
| Exemplars | 8 | 6 |
| Mission | 12 | 6 |
| Party / Workshop | 22 | 6 |
| Inventory | 16 | 6 |
| Store | 34 | 6 |
| Event | 55 | 8 |
| Recruitment header | 28 relative to Excavation | 5 |

`HomePanelThickness` creates four shaded side quads and then appends the original
front triangles. It must append using `AddVert` / `AddTriangle`:
`AddUIVertexTriangleStream` replaces the native buffers and would erase the sides.
A plain card Image therefore has 22 vertices and 30 indices after the effect.

Faces use transparent UI shaders with `ZWrite Off`, so local Z alone does not resolve
overlapping painter order. `HomeMenuDepth` deliberately adds Excavation before Store,
allowing the nearer Store slab to cover their shared edge. Event follows Story for the
same reason. Inventory is added before Workshop, allowing Workshop and its shadow
to cover the Inventory tab's upper edge. Keep each card's shadow/contact/face together
when changing draw order.

Soft shadows remain on the deck plane, enlarged by 14 reference units on each edge;
ordinary cards use a (9, −14) offset and Event uses (14, −20). Translucent cards use
soft/contact alpha 0.26/0.30. Dark cards use 0.50/0.46; Event uses 0.66/0.58
for its raised inset. This increases edge separation after the face opacity adjustment.
A second, tighter
contact shadow uses feather 0.018, offset (5, −8), and follows its face at local
Z + 6. The floating Recruitment header is shifted (12, 16), sits 28 units in front
of Excavation, and has its own soft shadow on Excavation. Its label is a child of
the header and therefore follows the header's projected plane.

`HomeCardLift` stores each face's base Z and moves a hovered face another 18 units
toward the camera. The contact shadow follows that movement; the soft shadow moves
by up to (6, −9). Pressing or leaving the button returns it to its own resting
elevation. On disable, hover state resets without losing that base elevation.
Gentle pointer motion varies the deck's yaw/pitch and position using unscaled time,
while leaving its Z rotation at zero.

Decorative Overlay graphics, including MenuPage and the composite, must keep
`raycastTarget = false`; an Overlay raycaster otherwise takes priority over the
world-space card raycaster. Group submenus and legacy windows stay flat on the
Overlay Canvas and intentionally receive input above the cards. The portrait,
profile and mode switch remain independent of the perspective deck. The currency
header belongs to the deck and uses the same world-space GraphicRaycaster. Wallet
Images and vector Graphics use `HomeEmblem` for correct offscreen alpha blending;
TMP labels retain their own font shader/material.

### Lifecycle and comparison

Home enable/disable activates/deactivates the separate perspective root and resets
pointer/hover motion. Resizing replaces the render target and defers destruction
of the previous target; destruction also cleans up the camera root and owned
materials. Combat simulation and state handlers remain running while Home is open;
this camera and the card transforms affect presentation only.

For a front-facing comparison, toggle `Depth Enabled` on HomeLayout's
HomeMenuDepth component, or call `SetDepthEnabled(false)` / `SetDepthEnabled(true)`.
Disabling depth restores identity deck rotations and sets the main card/hover Z
positions to zero while retaining the perspective renderer, current typography,
surface materials, side meshes and the Recruitment header's child offset. It is a
front-facing comparison, not restoration of the old flat implementation. The older aligned
flat source checkpoint is `docs/design-checkpoints/home-flat-layout.zip`.

### Prior perspective baseline validation

The prior typography/watermark/stagger pass compiled without Unity Console errors.
These results predate the four-row layout, Barlow font, Store/Excavation colors and
Recruitment strip; they are historical evidence, not validation of those changes.
All 10 cards passed center and two inset-corner native EventSystem raycasts (30/30)
in the actual Free Aspect GameView. All 10 centers also passed after resizing to
Full HD, and the PC render target reports AA4. The 1920 × 1080 capture was inspected
for converging perspective, edge quality, readable labels and clipping.

Clicking Exemplars opened its flat Group submenu. The mode switch was exercised
through EventSystem raycast hits in both directions: Combat re-enabled ArenaCamera
and disabled the perspective root; returning Home restored the cards. The sim tick
advanced from 1080 to 1499 while Home was open, with Paused false.

Use the actual GameView viewport for pointer validation. Fixed-resolution Editor
captures can report a temporary Screen/event-coordinate mismatch during MCP calls.
Prior perspective capture: `docs/previews/unity-home-menu-perspective.png`.
`docs/previews/unity-home-menu-depth.png` records the superseded orthographic variant.

### Prior four-row pass validation

Unity compilation and Console checks passed without errors. All 10 buttons passed
center and two inset-corner native EventSystem raycasts (30/30) in Free Aspect.
The overlapping Event card won its raycast over Story and emitted the Events route.
Store opened Shop / Trade / Crystal; clicking the black Recruitment header hit its
parent Excavation button and emitted the Summon route. No purchases or draws were
performed. The shared localization key guard also passed.

The final 1920 × 1080 screenshot was inspected for the four-row composition,
readable Barlow headings, watermark contrast, clipping and the separate soft shadows.
Historical capture: `docs/previews/unity-home-menu-four-rows.png`. These results
predate the layered material/depth pass above.


### Prior layered pass validation

The final translucent/polka-dot pass compiled without Unity Console errors and
passed all 10 buttons at center and two visible inset corners through native
EventSystem raycasts (30/30). Story's actual side/front mesh contains 22 vertices
and 30 indices. Its fill alpha was verified at 0.92; the right-side veil has its
CanvasRenderer and a 50-vertex gradient mesh. Recruitment and Excavation's first
glyphs both resolve to the intended Excavation hit target despite Store's overlap.

The mode switch was clicked through native raycast hits in both directions.
Combat restored ArenaCamera and deactivated the perspective root; Home returned
correctly. The sim tick advanced from 634 to 1258 across these checks. The shared
localization key guard and whitespace checks passed.

The final 1920 × 1080 screenshot was inspected for the larger composition, local
background contrast, clipped watermarks, separate Event/header shadows, thin side
faces, transparency and faint lower-right dots. Final capture:
`docs/previews/unity-home-menu-layered.png`. Older captures remain comparisons.


### Prior stronger translucency and dot pass

Reduced ivory/cyan face alpha to 0.68/0.78, ordinary card shadow opacity to
0.18/0.10 and the maximum backdrop veil alpha to 0.48. These layers compound,
so reducing only the front face alpha did not make the background sufficiently
visible. Dark utilities and the raised Event inset retain their existing shadows.

The procedural lower-right dots now use 18 rows, radius 0.18 and tonal strength
0.22. Their fade covers a broader corner region while keeping the left label area
quiet. Derivative-based antialiasing remains enabled for the projected surface.

Unity compilation and shader checks passed with no Console errors. Mission's
runtime face alpha was verified at 0.68. Both the smaller GameView and the final
1920 × 1080 capture were visually inspected for visible background, legible labels
and stronger dots. Whitespace checks passed. Layout and input were unchanged;
the raycast and switching results above belong to the prior validation pass.

Historical capture: `docs/previews/unity-home-menu-translucent.png`.


### Prior taller deck, wallet and utility relocation validation

The right deck now has four rows: Story/Event, Mission/Workshop/Inventory,
Exemplars/Party and Store/Excavation. Workshop opens the existing Blacksmith,
Enchant, Apothecary and Kitchen group; Inventory opens Inventory, Cards, Stash
and Album. Its narrow dark face and simple storage-crate icon follow the Depot reference.
Achievement remains at the bottom left. Transparency and polka-dot materials stay
at the values established in the previous pass.

Unity compilation and Console checks passed with no errors. Native EventSystem
raycasts passed all 10 card centers, two additional inset corners on each of
Mission/Workshop/Inventory, and all 3 currency chips (19 checks). Pointer click
events opened both relocated groups with the expected destinations. Home/Combat
switching disabled and restored the wallet component correctly. The localization
key guard and whitespace checks passed.

The 1920 × 1080 screenshot was inspected for clipping, expanded height, readable
headings, currency spacing and the three-button row. Gil/Soul showed the dev
session's values; Gem correctly showed an em dash without an online wallet. Live
server wallet refresh/purchases were not exercised in this local visual preview.
Historical capture: `docs/previews/unity-home-menu-tall-wallet.png`.

The Inventory icon was subsequently simplified from the generated money bag to
one vector storage crate, with no duplicate watermark. Compilation and Console
checks remained clean; the final capture includes that icon refinement.


### Prior opacity, perspective wallet and Home motion validation

Increased ivory opacity to 0.84 and cyan/dark opacity to 0.88, with watermark
alpha 0.21/0.26 on ivory/cyan respectively. Lower-right dot strength remains
unchanged. Moved the wallet root to the real menu plane without adding another
camera or changing balance sources and navigation routes.

Compilation and Console checks passed with no errors. Runtime Mission/Store/
Inventory alpha was verified at 0.84/0.88/0.88. All six wallet raycasts passed
(three chip centers and their action icons) using the perspective event camera.
The wallet disables on Combat and returns on Home.

The Home Animator was verified at 0.5; an actual dev cut-in was spawned and its
Animator verified at 1. The cut-in completed normally before the final capture.
No global time-scale or shared animation assets were changed. A 1920 × 1080 capture
was inspected for projected currency baselines, top-edge clearance, stronger
watermarks and readable labels. Whitespace checks passed.

Historical capture: `docs/previews/unity-home-menu-perspective-wallet.png`.


### Prior Story status, wallet scale and Inventory overlap validation

Story now has a larger heading and a full-width dark Coming Soon strip. The Event
inset is raised to clear that strip. The Energy-equivalent field is not fabricated;
Chapter/progress was proposed for when Story content is ready. Character artwork,
brightness, colors and framing are retained as requested. Home stays at 0.5× and
cut-in stays at 1×.

Wallet icons and values are larger; filled white action discs sit directly after
the numbers. The retained layout updates only on balance text or rect-size changes.
Inventory now sits at (.806, .451)–(.985, .546), behind the lower-right edge of
Workshop at (.742, .521)–(.967, .642). Inventory's icon/label occupy the exposed area.

Compilation, Console and localization checks passed with no errors. Seventeen
native EventSystem raycasts covered changed cards, the Story strip, currency icons
and action discs. Workshop correctly wins the shared overlap; the exposed lower
tab resolves to Inventory. A temporary display-only 9,999,999 test on all three
currency labels reported no overflow (font 37.05, text width 129.76 at Full HD);
original values were restored in the same tool call without changing balances.
The 1920 × 1080 capture was inspected for text clearance, stacking, stronger
shadows and spacing.

Historical capture: `docs/previews/unity-home-menu-story-status.png`.


### Tartarus Excavation identity

Excavation's heading now starts 64 reference units from the left edge (previously
30), with a small localized Tartarus caption directly above it. Both labels share
the same left alignment. The black Recruitment header keeps its existing depth.

The old star watermark is replaced by an original three-headed Cerberus emblem
loaded from `Resources/HomeUI/CerberusEmblem.png`. The reusable sprite is cached
by HomeIconSet, tinted by the existing HomeEmblem material and stencil-clipped
to the right side of the actual perspective card. The material samples only alpha,
so the PNG must retain transparent negative spaces; its source RGB color does not
affect the white watermark tint. The image generation record is stored separately
in `docs/unity-home-cerberus-prompt.md`. Excavation still opens the existing Summon
screen; this is a presentation change.

Validation: Unity compiled without Console errors. The live material loaded the
1024 × 1024 Cerberus texture at watermark alpha 0.26. Both Tartarus (28 units) and
Excavation (76 units) reported no text overflow. Three native EventSystem raycasts
over the title, caption and emblem all resolved to Excavation; decorations do not
intercept input. Localization key and whitespace checks passed. The actual Full HD
render was inspected for readable text, three-head silhouette, transparent interior
cuts and clean stencil clipping.

Historical capture: `docs/previews/unity-home-menu-tartarus.png`.


### Fantasy wolf, Exemplar subtitle and text edge tuning

The current Excavation watermark uses `HomeUI/TartarusWolfEmblem.png`: a single
fantasy wolf head in profile. The same alpha-only tint and clipped card placement
are retained. Prior Cerberus assets remain available as alternatives. The active
asset's prompt and provenance are in `docs/unity-home-wolf-prompt.md`.

Exemplars now has a 28-unit gray Management subtitle beneath the 68-unit heading.
Both share a 30-unit left inset. `UI_HOME_MANAGEMENT` supplies EN/TH localization.

HomeHeading's dedicated TMP material now uses Sharpness 0.25 and PerspectiveFilter
0.25 (previously 0 and 0.875). The prior heading material bypassed the existing
UiTheme edge tuning; this change narrows the SDF transition specifically for Home
headings, wallet values and the mode switch. Face dilation and outline/underlay
softness remain zero. The atlas remains uncompressed with bilinear sampling, and
the perspective target remains native resolution with 4× MSAA. No render-target
supersampling or postprocess sharpening was added.

Validation: compilation and Console checks are clear. After a fresh Play start,
the active headings retained Sharpness 0.25 / PerspectiveFilter 0.25. Management,
Exemplars, Tartarus and Excavation all reported no text overflow at Full HD. The
active watermark loaded TartarusWolfEmblem with alpha 0.26 and no input interception.
Native EventSystem raycasts over Management and the wolf resolved to their parent
Exemplars/Excavation buttons. The localization guard (1906 keys) and whitespace
checks passed. Actual 1920 × 1080 captures before/after material tuning were
inspected; the final screenshot includes the single wolf profile. GameView preview
is left at 16:9 aspect. Game logic and menu destinations remain unchanged.

Current capture: `docs/previews/unity-home-menu-wolf-management.png`.

## Accepted Home checkpoint / Exemplar continuation

Home was retained as the accepted prototype before the new Exemplar screen pass.
`docs/design-checkpoints/home-perspective-prototype.zip` preserves that Home source and art.
The new route and its validation are documented in `unity-exemplar-prototype.md`.


### Navigation cleanup · 2026-10-14
Kitchen is removed from the player menu and gallery by request. Workshop now exposes only Apothecary; Blacksmith and Enchant had no connected Unity destination and are no longer offered as dead links. Historical screenshots are retained as files only. Album populated previews use a disposable editor session, never a player save.


### Shared Details buttons · 2026-10-14
Menu action buttons now reuse the excavation Details reference frame through UiDetailButton. HudBar, ExemplarUi and ForgeUi factories retain their callbacks, hotkeys and disabled states. Compact icon controls and gameplay card/map targets remain distinct. UiWindow plays a finite entrance on actual open, not on resize; Details frames use event-driven hover/press motion and respect reduced motion. Album cards no longer repeat orbit diagrams; only a few static faint stars decorate the background.


### Revert shared Details rollout · 2026-10-14
User reverted the shared Details button rollout and its new menu/button animation changes. Restored the seven UI files from the pre-rollout backup. Original gacha Details and pull buttons remain unchanged. Album keeps the simplified cards and sparse background stars from the preceding cleanup.


### UX readability and transitions · 2026-10-14
Kept pre-Details button design. Increased secondary-text luminance and captions from 12 to 13 reference units. Ghost actions now retain a low-contrast resting surface and border, while selected controls retain their existing indicators. Corrected Album guidance to describe selection before actions. UiPageTransition adds a 200 ms opacity-only dissolve to page opens, Exemplar tabs, home construction, and inventory opening. No position/scale changes or delayed gameplay callbacks; reduced motion and companion mode suppress it. Editor motion fixture checks alpha completion, layout stability and reduced motion and records actual frames.


### Album Tactical Codex · 2026-10-14
User selected concept C's two-pane collection with the smaller selected-card inspector/action treatment from the second reference. AlbumWindow now has Album / In the bag tabs, a two-column large-card grid, quantity/rarity cues, an explicit selected state, and a compact monster/effect/bind/action inspector. AlbumCodexSurface draws local vector chamfers and the gold Level Up treatment; other screens and gacha button assets are unchanged. Bag selection has an explicit Deposit action. Mutations keep the appropriate card selected by re-reading the sim; feeding selects the row whose quantity increased, including merges into an existing upgraded stack. Withdraw/feed/bind inputs and costs remain simulation-owned.
Grid and inspector have independent clamped scrolling; tab switching preserves grid scroll. Selected portraits use the authored idle frames and stop under reduced motion/companion settings. Reopening recreates the idle portrait so it resumes correctly. Tab/selection changes reuse the short opacity-only transition. Grid portraits remain static and cache their cropped sprites. Localization includes EN/TH instructions, costs, disabled reasons and confirmations; source and runtime CSV remain synchronized.
Validation: disposable native Album walkthrough at 1440×900 and 1024×768, including empty/bag/foil states, deposit/withdraw, cancellation, feed consumption and result selection, bind consumption/progress, tab scroll, reopen, reduced motion, and Thai rendering. The withdrawal quantity check also detects a deliberately disconnected UI listener before restoring it. No player account/save changes. Gallery photos and the selectable idle preview come from native Unity captures, not a browser simulation.


### Shared Codex controls and spacing · 2026-10-14 (42)
The approved Album frame geometry now lives in CodexFrameMesh. Album keeps its existing layout and action semantics. Shared HudBar and Forge controls use CodexButtonImage (an Image subclass, preserving selected-state setters and Button tinting), with dark primary fills, gold outlines and legible ivory/gold text. Menu / Inventory are matching quiet navigation controls with filled icons and a subdued shortcut hint. GameMenuSurface carries the same chamfered treatment across notices, panels, Exemplar screens and other menus, while preserving opaque sheets. Gacha's authored one/ten-pull button assets remain intact.

Character & Inventory removes only the redundant Gil/Soul header; currency mechanics and relevant costs elsewhere remain unchanged. The character switcher, equipped slots, bag caption, stat rows and footer actions have revised gutters and sizing. Item art is inset, its level label has a dark caption strip, empty borders are quieter and rarity fills are more restrained. Existing authored item identities, rarity cues, drag/equip interactions and filters remain in use.

Apex uses a smaller summary, wider single-column allocation rows, visible disabled-action text and scrollable lower attributes; the character framing leaves room for both panes. Regalia, Resonance, equipment inspectors and Bestiary use the same frame language. Debug is removed from player entries, shortcut routing and menu construction; editor-only diagnostic source is retained. Historical Debug and Kitchen screenshots were archived outside the gallery. Album now defers size-only reflow until LateUpdate so its inspector stays inside the window on first opening, without refetching simulation data during resize.

Validation: native 1440×900 and 1024×768 full-screen sweeps (55 captures each), plus Album (14 each), Bestiary (19 each), and Excavation (15 each) action/state walkthroughs. These include modal cancel/confirm ownership, scrolling, back navigation, selected state, card consumption/result selection, Thai text, authored idle frames and reduced-motion checks. All final walkthroughs reported zero runtime errors. Online service pages show their real unavailable state in the isolated fixture; no account, purchase or save mutation was performed.

The capture fixture uses a temporary copy of InputSystem settings so synthetic key events continue when the editor loses focus. It preserves a copy of temporary original settings (Unity destroys temporary settings on replacement), restores the original behavior on teardown and never saves a project settings asset. Source/runtime localization copies match at 2314 unique keys. Gallery version: codex-42.
