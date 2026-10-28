# Exemplar management prototype

เริ่มงานต่อ / current handoff: [Unity UI — Luna 5.6](unity-ui-luna-handoff.md) (2026-09-14).

Date: 2026-09-12. Branch: `codex/unity-graphics-ui`.

## Accepted direction

Home remains the accepted Arknights-inspired prototype. Its complete source/art checkpoint is
`docs/design-checkpoints/home-perspective-prototype.zip`; its accepted capture is
`docs/previews/unity-home-menu-wolf-management.png`.

Home → Exemplars now opens a dedicated character screen. The new composition follows the
Wuthering Waves Attributes reference: navigation at the left edge, identity and attributes on
the left, a large central character, and a vertical owned-character selector on the right.
The accepted art direction combines WuWa, Epic Seven and HSR references: medium-brightness
slate surfaces, ivory text and restrained champagne accents over a quiet starfield. It should
support colorful characters from multiple artists while remaining calm and mature, without
turning the menu into a bright, cheerful theme.

The user explicitly requested the same existing Live2D in the center. Model 67 is therefore
shared prototype artwork for every selected character, not an assertion that it depicts
Seraphine or Rapis. The roster uses the existing swordsman/mage sprite aliases.

Reference inspected: [Wuthering Waves resonator progression / Attributes screenshot](https://www.gameleap.com/articles/wuthering-waves-resonator-progression-tips-and-tricks).
The active celestial background is the original generated
`unity/ROI/Assets/Resources/ExemplarUI/CelestialBackground-Medium.png`; the earlier dark plate
is preserved. Tool, full generation/edit prompts, sources and import settings are recorded in
[the art prompt document](unity-exemplar-art-prompt.md#medium-celestial-variant--approved-wuwa--epic-seven--hsr-blend).

## Layout and implementation

- `ExemplarScreen`: full-page navigation, persistent inspected ID, character list, page lifecycle.
- `ExemplarUi`: retained uGUI helpers and the Attributes view; regular Noto UI SDF text.
- `ExemplarReadModel`: read-only session projection combining deployed members and the benched roster.
- `ExemplarPortrait`: same Live2D by default, reusable fallback sprite and cached roster portraits.
- `ModeTabs`: routes the Home button, switches to Combat, and returns from Exemplars to Home.

Attributes displays real level/cap, job level, XP, HP, ATK, magic ATK, DEF, critical chance,
critical damage, dodge, attack speed and mitigation. Basis-point values are formatted as
percentages without calculating gameplay outcomes. XP has no percentage bar because the
current projection does not provide its threshold. Missing values are not fabricated.

The right-hand selector owns the inspected character. Switching inspection does not recruit,
deploy, bench or otherwise change simulation state. Legacy per-character pages synchronize
`PartySelection` to this inspected ID when reopening, including after Combat changes the old
window selection. Benched members remain inspectable; stats and deployed-only actions are
unavailable because the host does not currently project their effective stats.

Navigation routes reuse working existing widgets:

| New navigation | Existing page | Scope |
| --- | --- | --- |
| Attributes | New `ExemplarOverview` | Selected character |
| Regalia | `ExemplarRegaliaPage` | Selected deployed character |
| Equipment | `ExemplarEquipmentPage` | Selected deployed character |
| Potential | `PotentialWindow` with Exemplar presentation | Selected deployed character |
| Apex | `ExemplarApexPage` | Selected deployed character |
| Resonance | `ExemplarResonancePage` | Selected character, only actual linked data |

Regalia, Equipment, Apex and Resonance now have dedicated presentations. Potential retains its
functional graph with a larger graph area, restyled node surfaces, clearer details and pale main
action. `ExemplarWidgetScale` preserves only the graph's existing 720-unit geometry.

The agreed navigation order is Attributes → Regalia → Equipment → Potential → Apex → Resonance.
Advance is removed from the Exemplar shell. Paths moved to Home → Party → Paths, alongside
Party and Commander; the existing Soul wallet shortcut remains available. The user clarified
that Resonance stays and only Advance is obsolete. Potential no longer displays Board in
its menu title; internal Skills IDs remain intact.
The legacy `owned_exemplars` equipment collection is not used as the playable character roster.

Back/Escape returns from an inner page to Attributes, then Home. Menu/Combat remains at the
top right. `UiConfirm` owns its Escape cancellation and blocks mode navigation while open,
including the same dismissal frame, so a cancellation cannot also back out of the page.

## Art and lifecycle

The Exemplar portrait uses the Home renderer with its own framing `.57` and horizontal anchor
`.65` in Attributes. Management pages now use framing `.60` and horizontal anchor `.51`
beside the side panels. Home retains its existing `.31` / `.31` framing. Only these portrait instances run their
animation at half speed; shared clips and combat cut-in speed remain unchanged. Potential replaces the center portrait with its graph to keep nodes legible. Other pages retain
the same Live2D. Scene lighting and postprocessing do not affect portrait color. The arena camera is restored when returning
to Combat. SimDriver remains enabled and continues ticking while the screen is open.

The shared `ExemplarUi` palette now uses medium slate `Surface`, darker slate `Deep`, lifted
`Card` and `Selected` surfaces, a faint cool `Line`, ivory `White`, blue-gray `Muted`, and
champagne `Gold`. Navigation and native management pages use these shared values. Regalia,
Equipment and Potential received corresponding presentation refinements. Apex keeps its
host-driven two-column scrolling cards, with a single exact level-gate caption per substat
section and an individual lock mark on each locked card. Resonance separates rank and effects
into quiet side surfaces. The management surfaces preserve a clear center for Live2D; these
presentation changes do not add progression, currency or gameplay rules.

`CelestialMenuMotion` and `CelestialMotion.shader` animate the background with two slow layers
of drifting, twinkling stars and low-amplitude haze. The component is reusable on an image and
owns a separate material for each instance; serialized `motionSpeed`, `starStrength` and
`hazeStrength` control the effect. Its local animation stops when hidden. It does not change
global time, character grading, Home artwork or cut-in playback.

Stop Play before script edits to avoid invalidating the native sim handle during domain reload.
Use normal-resolution `ScreenCapture.CaptureScreenshot`, not supersize capture, on this
Metal/Cubism preview. The earlier supersize crash remains an engine-side capture limitation.

## Initial shell validation (2026-09-12, before the management-page pass)

Historical checks for the original seven-page shell, using the existing dev preview boot:

- Unity compiled with zero console errors; all seven navigation pages opened.
- Native EventSystem raycasts and pointer-click dispatch hit Character_2, Nav_Skills, Back,
  the projected Home Exemplars card, and the Menu/Combat button.
- Inspecting Seraphine/Rapis and traversing management pages left the simulation StateHash
  unchanged within the same frame.
- Reopening Rapis Potential Board after Combat changed PartySelection to Seraphine correctly
  restored PotentialWindow's target to Rapis (ID 2).
- A non-mutating confirmation test blocked mode switching and back navigation, retained Apex,
  preserved StateHash and retained the cancellation guard for the dismissal frame.
- SimDriver advanced from tick 2709 to 2931 while Attributes remained open. Combat camera
  restoration and Exemplar camera hiding were checked during the round trip.
- Localization guard passed with 1930 keys, and `git diff --check` passed.

No Rust, backend, balance, catalog or online save changes. No deployment or commit performed.
Localization keys were added to `godot/i18n/strings.csv` and mirrored into Unity StreamingAssets.
Final capture: `docs/previews/unity-exemplar-prototype.png`.


## Management-page pass (2026-09-12)

- Regalia: equipped summary, owned picker, actual enhancement/resonance, resolved worn effects
  versus separately labeled base/per-level/first-rank previews. Equip/Unequip use SimDriver;
  transfer asks confirmation. Target member, session and collection fingerprint are checked
  again before applying. Results come from the refreshed ownership projection.
- Equipment: ten retained slot cards, real item details and tooltips, and Open Inventory.
  Unity did not yet have gear EquipContextMenu or a gear Equip/Unequip action route; this pass
  preserves the existing Inventory route and does not claim to add that missing gear workflow.
- Apex: metadata-driven allocation cards and points. The preview native library reports
  `0.0.346-alpha` and nine buckets with `loc`/`per_pt`/`bp`, all post-cap. Repo host source
  already has the newer explicit core/substat `tier` schema. The page detects the projection:
  absent tiers means the existing nine-bucket post-cap view with Core labels hidden; explicit
  tiers exposes both real pools. No source-host constants are assumed for the older binary.
  Allocation and refund bind the selected ID and session; refund callbacks cannot retarget.
- Resonance: the new playable starter units do not currently expose their own Resonance
  progression. The only existing query is legacy `owned_exemplars` carriers. The page uses
  only a unique carrier whose `equipped_by` matches the inspected ID; it never chooses a
  carrier by name, class alias or first list entry. With no linked data it shows an em dash
  and an explicit unavailable message. There are no invented ranks, costs or upgrade nodes.
  Linked items show actual stars and separately labeled base effects; first-star authored
  modifiers are not represented as resolved effects at the current rank. New per-unit
  Resonance integration remains outside this presentation pass.

Live2D retains its original colors and half-speed menu animation. More opaque side surfaces
keep its bright cape from washing out the text. The accepted Home source/art checkpoint is
unchanged. Source for the old legacy Advance window is retained, but the Exemplar route is gone.


## Management-page checkpoint (2026-09-12, before medium celestial polish)

Verified in the isolated graphics Unity Editor for the revised six-page prototype:

- Navigation order: Attributes → Regalia → Equipment → Potential → Apex → Resonance.
  Advance is absent from this shell, and Paths remains party-shared outside it.
- Unity compiled with zero compiler errors. The localization key guard passed with 1,980 keys.
- Switching pages and inspecting character IDs 1 and 2 left StateHash unchanged in the
  same-frame read-only probes. The page bindings remained independent of party composition.
- Native EventSystem navigation clicks and equipment-slot selection were confirmed; the
  Equipment page's Open Inventory action opened the existing Inventory route.
- The loaded native DLL reports `0.0.346-alpha`. Its actual Apex projection has nine post-cap
  buckets, so the page renders those nine cards and hides the unsupported Core pool. The
  newer source-host tier metadata is supported without pretending it exists in this binary.
- Equip, allocation and refund writes were **not exercised** in this checkpoint. Their action
  paths exist, but this validation does not claim successful mutations or online verification.
- Starter-character Resonance remains explicitly unavailable where no linked carrier data
  exists; the prototype does not invent progression, ranks or costs.

Saved captures:

- [Equipment](previews/unity-exemplar-equipment.png)
- [Regalia](previews/unity-exemplar-regalia.png)
- [Potential](previews/unity-exemplar-potential.png)
- [Apex](previews/unity-exemplar-apex.png)

The Regalia capture from this checkpoint was visually inspected: its selected-character identity, empty
owned collection and unavailable equip state remain legible alongside the shared Live2D.
The four images are historical review artifacts from this management-page checkpoint; they
do not show the later medium celestial polish. No commit,
deployment, native rebuild or gameplay change was performed as part of this checkpoint.


## Medium celestial polish validation (2026-09-12, final recompile pending)

Completed checks reported by the graphics preview owner before the subsequent background
aspect/mask fix:

- The prior Unity compile completed; `CelestialMotion.shader` was supported and reported zero
  shader messages.
- Read-only navigation and inspection of character IDs 1 and 2 left StateHash unchanged.
- The localization guard passed with 1,980 keys, and `git diff --check` passed.

The preview owner is performing the final recompile after the aspect/mask fix. This section
does not claim that revision has finished compilation or final visual validation. No capture
files were replaced during this documentation update; final images will be recorded separately
by the preview owner. Equip, allocation and refund writes remain unexercised by the recorded
checks. Starter Resonance stays unavailable when no exact linked carrier is projected.


### Final starfield animation validation

The aspect-ratio and masked-material corrections have now compiled. Unity console contains
zero errors after all six pages rendered. Generated starfield shader is supported on this
Metal preview with zero shader compiler messages.

Runtime probes confirmed the motion clock stops while the Exemplar screen is hidden,
Combat's camera is restored, and simulation ticks advance during the Combat round trip.
Returning to Exemplars resumes the existing material and motion. Cover-cropped UVs use the
corrected aspect (1.776833 for this plate); masked uGUI render-material clones receive the same
local animation properties. A static review covered material ownership and disable/destroy.

Six actual Unity captures are saved as `docs/previews/unity-exemplar-medium-{attributes,
equipment,regalia,potential,apex,resonance}.png`. Earlier prototype captures remain intact.
The animated preview is [a seven-second Unity capture](previews/unity-exemplar-stars-motion.mp4).
It preserves actual capture timestamps; PNG capture cadence is limited and is not a runtime
frame-rate measurement. No gameplay writes, backend/native changes, deployment or commit.

## Equipment paper-doll and gear actions (2026-09-13)

The accepted Equipment direction is Epic Seven's character-centered gear arrangement inside
our WuWa-style shell. Ten category slots flank the existing Live2D, effective attributes sit
on the left, and selected-item details and a replacement picker share the right panel.
A separate eleventh cell appears if the host projects both Shield and WeaponLeft. The current
art uses crisp vector category glyphs (`ExemplarEquipmentIcon`), not finished item illustrations;
actual item names, item levels, refinement and rarity come from the native projection.

`ExemplarEquipmentActions` reads the inspected deployed character and real inventory. Equip
and Unequip send the host's existing inputs with the selected entity ID and original inventory
or equipped-array index. A frozen selection retains session, class and full raw collection
fingerprints; save switches and inventory/loadout changes require selecting again. Display
refreshes do not silently authorize an old choice. A parsed native input is not reported as a
successful equip unless the refreshed equipped collection actually changed.

`ExemplarGearDetails` compares host-projected ATK, magic ATK and DEF item contributions,
including their projected refinement/star fields. It explicitly labels this as item values;
it does not predict total character attributes. Rolled effects, cards, overlays and bindings
remain host-authored strings. Native rules still own dual wield, two-hand mastery and automatic
accessory placement. The picker only shows Weapon/Armor gear and uses the projected class and
level restrictions as preliminary checks.

### Existing starter-weapon limitation

The currently loaded native library reports `0.0.346-alpha`. A disposable starter-session test
confirmed that Seraphine starts with Training Saber, but its projected `wieldable` class list
still contains legacy classes. Removing it succeeds; re-equipping it is refused by that host.
The new UI therefore disables removing an item when the same class cannot restore it and
explains why. It does not fabricate a Seraphine-to-swordsman class alias or patch the catalog.
This native/catalog inconsistency remains follow-up work, separate from the graphics branch.

### Validation

The actual 346 native library was exercised through a disabled test SimDriver owning a
separate NewStarter session. Armor equipped and unequipped successfully; a ring equipped to
Rapis. Changed inspected identity and changed inventory produced Stale without modifying the
loadout. Starter-weapon removal was blocked. Test sessions were disposed and never attached
to the user's gameplay/save driver. No backend, catalog, DLL, online save or gameplay rules
were changed, and no deployment or commit was performed.

UI regression also confirmed that ordinary refresh retains the same frozen choice, changed
bag contents clear and disable it, and replacing the session with identical starter identity
and matching item data resets the selection. Actual retained-button callbacks equipped and
removed armor on the disposable session; the gameplay session StateHash was unchanged during
the same-frame probe. Native EventSystem raycasts/click dispatch reached Armor, Change,
Cancel and Weapon. The detail panel has an independent scroll viewport and visible draggable
scrollbar for long comparisons; the action area has a slate backdrop for readable notices.

Unity compiled with zero console errors after the final UI pass. Localization guard passed
with 2,000 keys; whitespace validation passed. The graphics preview is left on Equipment.
Current captures: [Equipment](previews/unity-exemplar-equipment-paperdoll.png) and
[replacement comparison using a disposable starter fixture](previews/unity-exemplar-equipment-comparison-fixture.png).
The second capture is test inventory, not the user's save. Earlier captures remain historical.


### Compact Equipment layout (2026-09-13)

User feedback: the paper-doll view felt too dense. The shared left navigation now places
centered labels under icons and uses 7.7% of screen width instead of 13.5%. Management
content starts at 8.5%; Attributes shifts left while preserving its panel width. The roster
and top-right Combat route retain their positions. Equipment side-panel widths are
compensated for the wider content area, and gear tiles shrink from roughly 162×115 to
97×97 pixels at 1920×1080 after the user requested square slots. Each tile uses a
width-driven 1:1 AspectRatioFitter, preserving its square shape across window sizes.
This opens the central character area without shrinking the
stat values or item-detail text. Gear actions and frozen-selection safeguards are unchanged.

Current capture: [compact Equipment](previews/unity-exemplar-equipment-compact.png).

Verified all ten visible slots remain square in Free Aspect and FullHD (97.2×97.2).
Native pointer dispatch reached the labels under Regalia/Equipment icons and the smaller
Weapon/Accessory II buttons; same-frame navigation preserved the gameplay StateHash.
Unity compiled with zero errors; whitespace checks passed.

### Compact roster (2026-09-13)

The right roster now occupies 7.1% of screen width (previously 9.7%). Character cards
are 128×104 at FullHD, down from 174×137, with a shared 114-unit row stride used for
placement and scroll extent. Names stay at 18pt and levels at 13pt with local card
anchors adjusted for the smaller portraits; the owned count follows the narrower rail.
The management panels retain their width, leaving more clear space before the roster.
Native pointer clicks selected Rapis and Seraphine correctly, and same-frame inspection
preserved StateHash. Unity compiled with zero errors. Updated preview:
[Equipment with compact roster](previews/unity-exemplar-equipment-compact-roster.png).

### Item-data panel position (2026-09-13)

The Equipment inspector moves 79.5px right at FullHD into the gutter freed by the
compact roster, preserving its 316.6px width and all child text/action layouts. Its
right edge is 1746.1px, leaving 22.2px before the roster rail. Ancestors are unmasked;
only the inspector’s internal scrolling regions clip their own content. Change/Cancel
pointer clicks were verified at the new position, and Unity compiled without errors.
Current capture: [item-data panel shifted right](previews/unity-exemplar-equipment-item-data-right.png).


### White slot symbols and Equipment polish (2026-09-13)

The user clarified that generated icons should be white slot symbols, not colored item
illustrations. Ten transparent generated symbols now live in
`unity/ROI/Assets/Resources/ExemplarUI/GearSymbols/`: Sword, Staff, Shield, Armor, Cloak,
Boots, Crown, Ring, Mask and Aura. Equipment slots and item detail headers share the same
category resolver. Other weapon categories retain their existing vector fallback. The
unused colored iteration is archived under `docs/design-checkpoints/gear-painted-unused/`,
outside Unity Resources. Exact built-in imagegen prompts and original output paths are in
[slot symbol provenance](unity-equipment-slot-symbol-prompts.md).

Original RGBA files remain unchanged at 1254 square. Unity imports use a 512-pixel maximum,
uncompressed color/alpha, mipmaps, trilinear filtering and clamp wrap. Source cutouts contain
some irregular residue, documented in the prompt record; at the actual 97-pixel tile size
the selected set reads as white symbols without a baked background.

The stat panel uses larger values, quieter labels and sparse group rules instead of zebra
bands. Slot labels sit above the square tiles; selection uses a complete champagne frame.
`EquipmentSurface` adds a restrained vertical gradient and layered shadow to simple panels.
The item inspector is one continuous slate panel with a larger symbol/title header, separate
stat values and deltas, and muted lore below a rule. A local 0.15-second unscaled reveal runs
only when inspected content changes. No gameplay time or equipment authority changes.

Validation: Unity reports zero console errors; localization guard passes with 2,000 keys;
whitespace check passes. All visible categories resolve to project textures, including
Rapis's Staff. EventSystem pointer dispatch reaches Armor, Change equipment and Weapon.
The local reveal settles at alpha 1 and disables its Update. A disposable starter comparison
fixture confirmed the DEF comparison row remains within its scroll viewport; it was disposed
and the normal gameplay driver restored. No backend or save migration was performed.

Current actual Unity capture:
[Equipment with white slot symbols](previews/unity-exemplar-equipment-white-symbols.png).


### Stronger slot contrast (2026-09-13)

User requested darker boxes. Empty slot opacity increases from 0.58 to 0.90 with a
darker navy fill; occupied slots use 0.95 and selected slots 0.98. White symbol tint
and panel layout are unchanged. Unity compiled without errors and the actual FullHD
preview was inspected: [darker slots](previews/unity-exemplar-equipment-darker-slots.png).


### Slot label backplates and right-column spacing (2026-09-13)

All slot labels now have a near-black 94%-opaque strip with inset ivory text, including
the conditional second-weapon slot. The right slot column moves from local x 0.70 to
0.74 (58.9px at FullHD), leaving 48.6px before the detail panel. Tile dimensions and
left-column positions are unchanged. The backplates are decorative and do not capture
raycasts. Unity compiled without errors; FullHD inspection found zero overflowing
slot labels and no panel overlap. [Updated preview](previews/unity-exemplar-equipment-label-backplates.png).


### Change Equipment browser and owned-gear transfer (2026-09-13)

The accepted flow is a window, not fullscreen. It has the current item on the left, a
four-column scrollable collection in the middle, and selected item contributions with
green/red differences on the right. Clicking a card previews only; Equip is a separate
action. Same-slot bag items and all deployed characters' worn items appear in the list,
including currently equipped pieces. Worn cards carry the owner's avatar and name.
Selecting one's own worn item is allowed for inspection but cannot equip it twice.

Clicking outside closes only this window; it cannot activate navigation, a character,
or Combat behind it. A full-screen sibling scrim owns that first click, while the window
itself consumes empty-area clicks. The visible Combat switch temporarily yields raycasts
to the dismiss scrim and restores input on close. Back/Escape cancels the picker before
leaving Equipment. Item, gallery and lore scrolling remain independent.

Equipping another character's item opens a confirmation naming the item, donor and
recipient. The callback retains the exact frozen choice and page lifecycle token; closing,
switching character/session, or changes to relevant equipment invalidate it. Confirmed
moves use existing UnequipItem then EquipItem inputs, never synthetic inventory edits.
`SimDriver.ApplyVerifiedInputs` first runs that sequence against an identical disposable
native state, verifies the destination slot and exact item payload, checks source removal
and complete equipment conservation, and only then runs the same inputs synchronously on
the original session with one final publication. The live input log retains both inputs.
No clone is created during card previews or ordinary refresh. No backend/catalog/native
version changes, deployment or commit were made.

Limitations of the current host: comparison is of native item contributions, not a forecast
of total character stats. Bench equipment lacks an enriched query and the native equip
handlers require a deployed entity; when bench gear exists the window explicitly reports
that it is unavailable. The prior starter-weapon class restriction remains guarded.

New files: `ExemplarEquipmentPicker.cs`, `ExemplarEquipmentTransfer.cs`,
`Sim/VerifiedInputSequence.cs`, and Editor-only `EquipmentTransferVerification.cs`.
`ExemplarGearDetails.Create(parent, standalone:true)` supports the two independent panels.
The generic confirmation body now measures its wrapped height instead of assuming 44px.
English/Thai localization guard passes with 2,011 keys; Unity console has zero errors;
whitespace checks pass.

Validation on shipped native 0.0.346-alpha: `EquipmentTransferVerification.Run()` passed
10/10 scenarios and 260 assertions (bag equip, donor transfer and displaced-item recovery,
exact copy selection, wrong-slot refusal, own-item protection, stale bag/source/target,
identity/session changes, one complete publication, and both real log inputs). All runners
own and dispose their disabled fixture drivers. UI checks confirmed preview and Cancel
emit no inputs; confirming moves Rapis's Iron Mail to Seraphine and returns Bronze Mail.
An actual EventSystem click at Combat hit OutsideDismiss first and stayed on Equipment;
the second click switched Combat. Wheel scrolling reached the bottom and retained its
position across refresh. Final comparison has no overflowing text.

[Window preview](previews/unity-change-equipment-window.png) shows an isolated fixture with
18 armor pieces to demonstrate four columns, scrolling, ownership and stat comparison;
[confirmation preview](previews/unity-change-equipment-confirm.png) is also fixture data.
Those fixtures were disposed and the ordinary gameplay driver restored before handoff.
