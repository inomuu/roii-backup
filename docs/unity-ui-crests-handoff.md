# Unity Party slots, Crests and Disenchant — 2026-09-19

Branch `codex/ui-crests`, isolated worktree `/Users/lunalieneray/idle_mmorpg-gem-ui`,
based on master `a818dbc7`. No master edits, backend deployment or sim rule changes.
Read alongside `ui-handoff-2026-09-19.md`.

## Merge checkpoint — 2026-09-20

Implementation is committed through `91386781` on `codex/ui-crests`; merge this branch
from the master session. This session has not merged, pushed or deployed it.
The final two sections below supersede earlier pending-art notes and palette choices.
Recent commits: `afa64b7d` (slot symbols/palette), `fec09d61` (lower-resolution imports,
required-level badges, bag capacity/new-item dots), `91386781` (inventory scrollbar).

Latest validation: clean Unity compile/console; actual GameView with ten grades;
read-receipt checks and scrollbar top/bottom/auto-hide checks passed. The 87-check native
Companion smoke predates these final UI revisions; rerun it after integration if needed.
New-item dots reset on session load and currently cover inventory, not ledger gems.
Stage master's current native library/data and restart Editor after merging.

Local-only leftovers are six regenerated TMP font/HomeUI assets (including
LiberationSans SDF - Fallback) plus untracked `docs/previews/`. They are intentionally
excluded; do not blanket-add them when merging. Runtime art and metadata are committed.

## Implemented

- Combat/member strips retain five positions. The next locked position buys a party slot;
  later positions explain that the preceding slot must be unlocked. Prices, affordability,
  slot limit and ticket count come from `party_meta`. Purchased empty slots indicate deployment
  through Party. PartyWindow uses the same purchase handler and respects purchased capacity.
- Ticket callout explicitly says the reward arrives after a verified save. The UI does not
  grant tickets or claim to have verified server delivery.
- **Separate pages**, per user correction: Crests upgrades party bonuses; Disenchant selects
  an inventory item, previews Essence, then requires destructive confirmation.
- Bottom row: Blacksmith / Training / Socket Gem / Crests / Disenchant. Both new pages use
  the shared floating host for Battle and Companion, with mutually exclusive right panels.
- Crests now has five Tier tabs and four category tabs, with one large selected Crest. Names, current bonus,
  per-level effect, held shards and next cost are native queries. Maxed/unaffordable disabled.
- Disenchant shortcut beside Discard opens its page; the conversion button is in that page.
  Right-click while that page is open selects the item instead of equipping it. Worn items
  cannot be disenchanted from this page. Invalid items remain inspectable with disabled action.
- Legacy ItemGridWindow also has native-preview Disenchant and confirmation beside Discard.
- Frozen session, inventory and preview guards cancel confirmations after inventory mutation
  (including Auto Fusion). Actions preview on an exact native clone and verify state changes
  before applying. A parse-successful native no-op is not reported as success.
- Essence counts preserve fractional materials: e.g. `1/10`, `2 + 3/10`. No float arithmetic
  or truncation to whole materials. Formatting accepts u64; native crest purses saturate u32.
  Long amount labels auto-size; standard currency formatting is unchanged.
- English/Thai localization added; mirrored CSV kept aligned, no Godot feature work.
- Companion damage pool fix discovered in testing: skip inactive pane cameras; resolve TMP's
  lazy CanvasRenderer and assign font material before outline setters. Previously an inactive
  pane repeatedly threw and allocated untracked Damage objects. Updates obsolete wrapping.

## Host blocker: ledger gems cannot currently be disenchanted

This is a concrete mismatch in the existing handoff, not a new client rule.

- Catalog gem definitions have `ledger: true`. Granting `gem_strife_g2` produces a Rare gem
  in `materials`, while `inventory` remains empty.
- `disenchant_preview:<idx>` in `crates/sim-host/src/session.rs` reads only `st.inventory`.
- `InputEvent::Disenchant { inv_idx }` / `crest::disenchant` also consumes only inventory.
- `crest::material_of` recognizes gems, but normal ledger gems never reach that container.

The implemented UI therefore supports inventory equipment (and any native eligible inventory
entry), but **does not claim to support disenchanting current ledger gems**. Gem Crests are
visible and upgradeable when their native materials are held. Master should supply a ledger-gem
preview/action with explicit quantity and native affordability. Then extend `CrestModel` and
the dedicated Disenchant page; do not manufacture inventory indices or shard yields in UI.

## Validation and reproduction

- Rebuilt/staged dylib + data using `tools/unity_dylib.sh` and `tools/unity_data.sh`; restarted
  only the isolated Editor. Native version read from Unity: `0.0.438-alpha`.
- `Roi.Perf.CrestUiSmoke.Run()` in Editor, outside Play mode: disposable session; four slot
  purchases, insufficient funds/max guards, duplicate-click protection, 20 Crests, T1 rejection,
  ten native disenchant credits/removals, upgrade exact spending, stale inventory cancellation,
  u64 formatting. Does not use player saves or server sessions.
- Actual Play-mode Disenchant button + confirmation tested against a disposable UI fixture:
  inventory went from one T3 sword to empty and weapon_t3 shards increased to one.
- Inspected actual Game View screenshots for separate pages and Companion in-editor layout.
  Dedicated Disenchant text reported no overflow. Companion native OS transparency/topmost/
  resize is **not** validated by Editor: `WindowMode.Supported` is deliberately false there.
- After the damage-pool fix, reopened Companion with live combat: zero console errors,
  four pooled Damage objects instead of the prior runaway allocation; switched both pages.
- Localization guard: 2,059 keys; `git diff --check` clean.

## Remaining / merge notes

- Test real standalone Companion window and an online verified-save ticket reward.
- Existing handoff Regalia passive text, new status icons, bestiary/Dungeon work remain separate.
- Existing font SDF/HomeUI Unity regeneration and untracked `docs/previews/` are excluded.
- New scripts include Unity metas. No generated icon art, scene changes or native binaries
  are committed; existing staging scripts rebuild/stage native artifacts.
- Never merge this branch by overwriting the newer master localization CSV; retain both sets
  of unique keys, then run the localization guard and stage current native data/plugin.

## Follow-up: bulk Disenchant and Auto Fusion (same branch)

- Dedicated Disenchant page supports multiple selected bag items, click again/remove from the
  selected-items grid to deselect, and Clear. Both selection grid and aggregated Essence totals
  scroll. Confirmation text also scrolls so large mixed-material batches remain reviewable.
- Auto Select uses exact grade checkboxes (not item level), defaults Common/Fine, remembers
  the mask in local PlayerPrefs. It replaces the selection, selects only native-eligible items,
  and never executes Disenchant automatically. Manual selection can include any eligible grade.
- Batch conversion preflights the complete sequence on a native clone, removes descending
  indices, verifies exact surviving rows and per-material credits, and publishes once. A stale
  confirmation consumes nothing. No new sim event or reward calculation was added.
- Auto Fusion now checks **once per 10 seconds** (first batch also waits). It displays the normal
  success/failure Result, including Gil, and keeps the current side panel/Disenchant selection.
  Result retains its ordinary auto-close behavior. No new batch starts over a visible Result,
  confirmation, drag, or manual Forge draft. Closing Forge does not disable Auto Fusion.
- Selected Disenchant entries, the inspected bag item and dragged gear are reserved from Auto
  Fusion. Other eligible items can still fuse, including identical duplicates. Known consumed
  indices rebase surviving selections before UI refresh. Unknown ambiguous removal of identical
  rows clears those selections safely; appended loot does not clear the draft. Leaving the page,
  Clear, deselection, or closing the inventory releases the reservation.
- `Roi.Perf.DisenchantBatchSmoke.Run()` passed: exact grade masks, native-ineligible exclusion,
  multiple material yields, exact survivors, stale atomic rejection, reserved duplicate skipped
  while six identical unselected items fuse, reindex/release, 10s constant.
- Timed Play-mode probe passed: no fusion before cadence, first batch after 10s, Result visible
  without side-panel switch, confirmation blocks next cycle, selected duplicate remains/reindexes.
  Zero console errors. Updated standalone auto-fusion smoke to expect the new cadence/Result.
- Actual Auto Select + confirmation buttons converted eight selected items into the previewed
  weapon_t3=7 and armor_t3=2 shards, leaving the fixture bag empty. Page reported no TMP overflow.
  Localization guard passes 2,068 keys. Tests use disposable native sessions, not online saves.
- Ledger-gem API blocker above remains; this follow-up does not pretend Materials indices are
  inventory indices. No backend/schema/catalog changes. Native standalone OS-window checks remain.

## Follow-up: illustrated Crest detail page / Fragment terminology

- Replaced the scrolling Crest list with Tier T3–T7 tabs, then Weapon / Armor / Accessory /
  Gem selectors. One native Crest per selection, 20 total. Separate Disenchant page remains.
- Large T3 ivory and T4 silver-blue generated artwork imported into Resources/Crests with
  genuine alpha, no color-key shader. Import settings preserve full resolution at UI scales.
  T5–T7 currently reuse T4 and visibly say the artwork is temporary; their host stats are real.
- Crest moves ±3.5 layout pixels over ~8 seconds. Thin static-mesh orbit rotates slowly and
  the existing CelestialMotion shader supplies restrained stars/haze. Nested stage canvas
  isolates artwork changes from inventory text/grid. No particles, lights or render textures;
  animation updates stop when the page is inactive; its material is destroyed with the stage.
- UI terminology is now Fragment (Thai: ชิ้นส่วน), including Disenchant previews and Crests.
  Existing MAT_* keys, native material IDs and shard counters are unchanged. Both localization
  copies remain synchronized; this does not reintroduce Godot development.
- Current total remains the host text. Next-level display uses the host's linear text_per_level
  with decimal formatting (including negative Cooldown / ranged damage taken), never a float.
  Unknown text formats fall back safely; if future Crests become nonlinear, expose next_text
  from the host and replace CrestPresentation. Price/eligibility/spend still native-authoritative.
- Existing exact integer/fraction formatting is retained. Long labels/counters autosize.

Validation in the isolated UI Editor (sim 0.0.438-alpha):
- Compile and console: zero errors; CrestUiSmoke passed (native spend, stale guard, u64 etc.).
- Actual Play-mode buttons: all 20 combinations showed the correct native Crest name;
  affordable upgrade advanced crest_w3 to Lv1 and deducted exactly 10 shards.
- No TMP overflow in battle or companion preview. T5–T7 temporary-art caption checked.
- Floating Y and shader motion time advanced; both T3 and T4 alpha visually inspected.
- Editor companion mode opened the same panel above its gameplay pane. Native standalone
  transparency/topmost behavior was not retested. Fixture used a disposable SimSession;
  user online saves were not modified. Editor stopped after preview.
- check_loc_keys: 2,076 keys, git diff --check clean.
- Real captures: /private/tmp/roi-crests-page-t3.png, /private/tmp/roi-crests-page-t4.png,
  /private/tmp/roi-crests-companion-layout.png (local only, not source assets).

## Follow-up: white glowing magic circle

- User rejected the free reference circles and requested a custom generated alternative.
  Added original `Resources/Crests/MagicCircle.png` line mask; reviewed CC0 OGA files stayed
  in /tmp and were not imported. Source/prompt documented in the Crests asset README.
- `WhiteCircleGlow.shader`: white core + two sampled halo radii (17 samples on one small UI
  quad). Source is a linear luminance mask; low black-field noise is removed with smoothstep
  to prevent a rotating rectangular haze. No scene bloom or HDR dependency. Normal alpha
  blend and standard UI stencil/clip support preserve the companion compositing path.
- Circle slowly rotates at 1.6 degrees/second with a small brightness pulse. Existing floating
  Crest and background stars remain. Crest image reduced slightly to leave room for runes.
  Fallback thin orbit remains if the new texture/shader is missing or unsupported.
- Verified live Game View: shader supported with zero compile errors, visible white halo,
  no square backdrop, circle rotation advances, art does not intercept clicks. Companion
  Editor layout checked; standalone native transparent-window build not rerun. Console zero
  errors. Editor stopped after preview; no native/backend changes.
- Captures: /private/tmp/roi-crest-white-circle-final.png and
  /private/tmp/roi-crest-white-circle-companion.png.

## Follow-up: centered artwork, atmosphere and Tier glow

- Supersedes the white-only circle/motion settings above. Circle glow now follows Crest Tier:
  T3 pearl, T4 ice blue, T5 violet, T6 gold, T7 rose, consistent across all four categories.
  Pale line core retains legibility; halo and a restrained mist tint receive the Tier hue.
- Uneven transparent atlas margins caused the apparent offset. Crop UVs to measured alpha
  bounds, preserve aspect ratio and use a shared centered parent for art and circle. Both
  float ±2.5px together; only the circle rotates, at 0.9 degrees/second (~400s/revolution).
- New CrestAtmosphere shader: two opposing slow mist layers, faint diamond pattern, fine
  grain and sparse stars. Small nested stage canvas; no scene bloom, lights or particles.
  Materials are instance-owned and disposed on destruction. Native/gameplay unchanged.
- Live isolated Editor check: all five Tier buttons set the expected glow color; all four
  categories preserve that color within a Tier. Game View captures verified centered T4/T7
  art, colored circle, mist/pattern and rotation; console had zero errors. T5–T7 art still
  uses the explicitly labeled T4 placeholder. Standalone native window was not rebuilt.
- Captures (local only): /private/tmp/roi-crests-tier4-atmosphere.png,
  /private/tmp/roi-crests-tier7-atmosphere.png, /private/tmp/roi-crests-tier-companion.png.

## Follow-up: smaller Crest and framed stage

- User requested more space around the centerpiece: max art dimension 190 → 158px;
  magic-circle quad 228 → 248px, exposing the ring's glyph band. Tier colors/motion retained.
- Added a stationary double hairline border, four muted-gold corner accents and central
  diamond clasps. All frame graphics disable raycasts and use existing retained UI helpers.
- Darkened the atmospheric gradient; mist strength .45 → .23 and pattern .28 → .18.
- Isolated Unity Editor compiled and Game View T4 preview inspected; zero console errors.
  Screenshot: /private/tmp/roi-crests-framed-t4.png. Stopped Play after checking.
  Native companion build was not rerun; this is a presentation-only stage change.

## Follow-up: shaped mist, independent motion and inventory grid

- Mist uses two broken, curved ribbons with dark gaps, tinted by Tier. Alpha-style color
  blending replaces additive neutral haze. Center is restrained behind the illustration.
- Motion correction: only Crest art bobs ±2.5px. Shared mount and magic-circle center stay
  fixed; circle rotates only. Live timed probe verified all three conditions.
- Inventory shows at least 30 cells (5 columns × 6 rows), with inert empty slots. Larger bags
  grow to complete rows and scroll. These are visual placeholders, not a native capacity limit.
- Removed the selected-item description area and its Equip/Discard/Disenchant action buttons.
  Hover details, right-click equip, drag/drop, Auto Equip and bottom Disenchant navigation remain.
  Brief action notices retain a small dedicated area below the grid, normally blank.
- Reused empty cells clear click/hover/drag data and compatibility tint. Tested with a disposable
  native session: a 35-cell overflow grid shrank back to 30 blanks after removing fixture gear;
  all blank payloads, previews and descriptions were null. No player inventory was modified.
- Game View inspected and Console zero errors. Screenshot /private/tmp/roi-crests-inventory-final.png.
  Play stopped afterwards; native companion standalone build was not rerun.

## Follow-up: floating UI typography and celestial ornament

- Reuses licensed existing HomeUI font assets: Crimson Text for window/section/Crest names,
  Barlow Condensed for action buttons and prominent Crest values. Body stats/descriptions keep
  Noto Sans. Each font switch also switches the SDF material to avoid sampling the old atlas.
  Button text autosizes; inventory section labels gained height for serif line metrics.
- ForgeDecoration is a reusable static retained mesh: inset corners, header rule, narrow side
  engravings and diamond clasps. Applied to inventory, Forge, Socket, Training, Crests and
  Disenchant. No raycasts or per-frame geometry updates. InventoryBackdrop adds subtle celestial
  arcs behind the portrait and a constellation under the bag grid.
- Actual Game View inspected; all six active page states checked for TMP overflow (none with
  current English data). Console zero errors. Locale-wide and native standalone QA not rerun.
  Snapshot: /private/tmp/roi-ui-ornaments-final.png. Editor Play stopped after verification.
- Font atlas regeneration remains uncommitted, as before; no font source/license changes.

## Follow-up: remove ornament pass, use illustrated nebula

- User reversed the new outer gold corners/clasps and connected constellations. Removed
  ForgeDecoration and the InventoryBackdrop constellation/portrait arcs. Typography retained;
  the earlier Crest-stage mount and standard functional panel borders remain.
- The procedural nebula still looked like purple haze. Generated original NebulaBackdrop.png
  with defined dark dust lanes, gas pillars and blue/amethyst filaments. Source/prompt/import
  settings recorded in Resources/Crests/README.md. Asset is checked into Resources, not /tmp.
- InventoryBackdrop now draws one quad with an instance-owned material; both it and CrestArtwork
  use the image through CrestAtmosphere. Aspect-preserving slow image drift and larger twinkling
  stars; bounded UV overscan avoids edges. Procedural clouds remain only as missing-art fallback.
  No new particles, lights or render textures. Material destroyed with component.
- Isolated Game View inspected: image loads, shader supported, motion time advances, decorative
  objects absent, no input interception. Console zero errors; native companion build not rerun.
  Screenshot: /private/tmp/roi-nebula-art-ui.png. Play stopped after verification.

## Follow-up: subdued nebula / expanding star pulses

- Artwork mixes at 55% over midnight navy. Detail remains visible with less contrast behind UI.
- Stars use independent 4–8s phases, expanding/fading halos and restrained rays. Separate
  unscaled sparkle time avoids tying pulse speed to slow background drift. No extra geometry.
- Actual Game View checked; both materials' sparkle time advances, shader messages zero.
  Screenshot /private/tmp/roi-nebula-soft-sparkle.png. Play stopped after inspection.

## Follow-up: final selected T3 artwork

- Replaced T3 with the user's final selected ivory/charcoal/gold sheet
  (`codex-clipboard-c515361e-41df-46f6-8631-68da50ec59a4.png`). Its original organic
  crystal motifs are retained. Intermediate literal equipment-shadow variants are superseded.
- Built-in imagegen extracted alpha; project copy is Resources/Crests/T3-Atlas.png.
  Source and full extraction prompt are in Resources/Crests/README.md. Updated measured
  alpha bounds for all four categories, retaining centered aspect-preserving display.
- Actual isolated Unity Game View checked (weapon and accessory screenshots); switched all
  four categories and verified texture/UV bounds. No console errors. Play stopped afterwards.
  Screenshots: /private/tmp/roi-t3-approved.png and /private/tmp/roi-t3-approved-accessory.png.
- Remaining broader artwork request: T5–T7 Crests and T1–T7 equipment sets are not yet delivered.
  Actor foot shadows / lower overhead bars are a separate uncommitted work in progress;
  not included in the T3 artwork commit or claimed as visually verified here.

## Follow-up: approved jade T5 set

- User selected jade / charcoal / champagne gold after reviewing purple and jade concepts.
  Resources/Crests/T5-Atlas.png now supplies all four T5 categories. Small decorative stones
  are consistently jade; orange/teal/amber/violet central cores keep category identity.
- Added T5 alpha bounds, jade circle glow and ambient accent. T5 no longer shows the temporary
  artwork caption; T6/T7 still use T4 as a labeled placeholder. No native rules/stat changes.
- Texture uses actual alpha, full 1254px size, uncompressed sRGB, clamp, bilinear, no mipmaps.
  Complete built-in imagegen prompt and source filename recorded in Resources/Crests/README.md.
- Isolated Unity Game View: switched all four T5 categories, verified correct texture and
  aspect-preserving sizes, empty temporary-art caption, inspected accessory image and jade
  glow. Console zero errors; git diff --check clean. Play stopped. Native companion executable
  not rebuilt. Screenshot: /private/tmp/roi-t5-jade-approved.png.
- Still pending: T6/T7 artwork, T1–T7 equipment set artwork, separate actor shadow/bar WIP.
## Final artwork pass — 2026-09-20 (supersedes pending-art notes above)

Worktree `idle_mmorpg-gem-ui`, branch `codex/ui-crests`. No master edits, merge,
push, deployment, catalog or sim-rule changes in this pass.

Delivered:
- Correct equipment art: **105 images, 15 types × T1–T7**, in seven transparent atlases.
  Uses the live ten slots: Weapon / Shield / Helmet / Gloves / Armor / Shoes /
  Earring / Necklace / Ring / Bracer. Six Weapon illustrations account for the extra five.
  Legacy emission groups `mask_tN` and `garment_tN` are NOT artwork identities:
  they now mean Gloves and Earring. Rejected mask/cloak concept sheets were not imported.
- Distinct visual themes: T1 honey wood/cream linen, T2 iron/crimson, T3 silver/blue wings,
  T4 obsidian/amethyst crescents, T5 jade leaves, T6 gold/black sun, T7 pearl/pink petals.
  Theme materials are presentation only; do not rename catalog material names or stats.
- Shared `EquipmentArtwork` exact-ID lookup and alpha-trimmed UVs, consumed by
  `ExemplarEquipmentIcon` (floating bag, worn slots, picker, Forge result) and legacy
  `ItemSlot`. Full RGB preserved; no per-item texture copy. Resources cached per used Tier.
  Art uses Tier from ID, not grade or item level; unknown IDs fall back to category glyphs.
  Reset UV/art mode on recycled empty cells; preserve binding over disable/enable.
- Empty glyphs corrected for Helmet, Gloves, Earring, Necklace, Bracer. No mask/cloak glyph
  remains mapped to a live slot. Old symbol assets kept for unrelated consumers.
- Quiver / Tome / Hidden Knife: **21 future illustrations only**, two source sheets in
  `art/equipment/future-offhands/`, outside Unity Assets. User explicitly says their system
  does not exist yet. **Do not wire catalog IDs, equip rules or drops from these pictures.**
- Crest T6 and T7: four real categories each; no more T4 placeholder. Gold/rose circle
  glow remains Tier-specific. Only illustration bobs; circle rotates about fixed center.
- Actor contact-shadow quad: soft ellipse above optional wet-sheen ground plane, follows
  actor, smooth width changes, reduced opacity during lift/death. Independent of directional
  sprite cast shadows, no extra camera/RT. Instance mesh/material released on destroy.
- Overhead HP anchor uses alpha-trimmed sprite height plus small gap and smooth movement.
  Existing VFX `HeadY` kept unchanged, avoiding unintended Break/cut-in framing changes.
- Native Forge smoke had stale `UI_FORGE_TITLE` lookup after Blacksmith rename and hardcoded
  old 830/500 panel widths. Updated locator and references to current layout constants.

Validation:
- Unity C# compile and macOS Development player build succeeded, zero build errors.
- Exact artwork IDs checked against current catalog: **105/105 present**.
- Live shared renderer exercised for 105 bindings, grade/level independence, UV validity,
  recycled empty/unknown cells, disable/enable, and five corrected empty glyphs: no failures.
- Actual Game View inspected: mixed-tier inventory + equipped starter weapon; T6 weapon,
  T7 accessory/gem; all 20 Crest texture/UV bindings valid. No Unity console errors.
- Visual references / generation prompts / source filenames and import settings are recorded
  in Resources/EquipmentArtwork/README.md, Resources/Crests/README.md and future-offhands README.
- `tools/check_loc_keys.py`: 2,076 keys pass. `git diff --check` clean.
- No sim tests rerun for artwork-only changes; no sim behavior changed.
- Standalone companion smoke **87 checks PASS, exit 0** after fixing the stale probe
  locator/width expectations. Covers native transparency, game/UI scale, shared inventory,
  gear/gem fusion + actual reward, socket confirmation, combat continuing under Result,
  and returning to Battle. New reward artwork visually inspected in native screenshot.
  Log: `/private/tmp/roi-artwork-native-smoke-v3.log`; screenshots and check list:
  `/private/tmp/roi-floating-forge-native/`. Test executable exited; our Editor Play stopped.

Evidence on this machine (not runtime dependencies):
`/private/tmp/roi-artwork-runtime-t6.png`, `/private/tmp/roi-artwork-runtime-t7.png`,
`/private/tmp/roi-crest-t7-gem-final.png`, `/private/tmp/roi-all-105-equipment-unity.png`.
The atlas-grid diagnostic screenshot clips the final row at this Game View aspect;
individual T7 item bindings and the real inventory were also checked.

Merge notes:
- Branch native library is still sim **0.0.438-alpha**. Do not infer it is the newest master
  dylib. Merge side must stage its own current dylib/data and restart Editor afterward.
- Existing five dynamic font/HomeUI regeneration diffs and `docs/previews/` stay outside
  these commits, as previously agreed. They are not needed for the artwork mapping.
- Generated PNGs and .meta are committed in the worktree; no dependencies on Codex cache.
# Equipment palette / stylish slot revision — 2026-09-20

Follow-up: all seven equipment tier atlases import at max 512 to reduce visible
fine-detail noise; source PNGs remain intact. Runtime sizes verified as 512×307 in Unity.
Pure-white slot symbols keep their original resolution.
Inventory header now shows used / sim-provided inventory_cap, with no hardcoded cap.
Inventory also has a draggable right-hand scrollbar in its reserved gutter; five
columns remain intact. Verified top/bottom movement and automatic hiding for short lists.
New inventory loot gets a red top-right dot; hover tooltip, click, context-click or
drag acknowledges it. Tracking continues while the panel is closed and is shared
by Battle/Companion. Existing items form the baseline on session load; unread state
is presentation-only and currently lasts for that session (not persisted to a save).
Owned counts include equipped items to avoid flagging unequip as loot. Legacy items
without instance IDs are grouped by definition/grade/star/foil; identical copies are
marked from the end of the bag. Gems in the separate materials list are not marked.
Validation: seven runtime read-receipt checks passed (baseline, gain, read, duplicate,
old copy, unequip and capacity), all seven atlases report 512×307, and GameView labels
show requirements Lv1/10/20/30/40/50/60. Debug preview uses ten grades in an isolated save.
ForgeCell corner labels now use sim-provided `level_req`, not `item_level`:
T4 is Lv30 and T7 is Lv60. Rarity gradients remain independent of artwork tier.

- Equipment T4 is now green Jade Grove; T5 is purple Amethyst Crescent. This is artwork only: IDs, gameplay, rarity backgrounds and crest art are unchanged.
- Green surfaces received restrained cleanup; T7 pearl/pink surfaces were repainted to reduce tiny facets and avoid the thick outlines of the rejected intermediate version.
- Added ten generated pure-white equipment slot pictograms with tapered fantasy silhouettes and clear negative space. Shared ExemplarEquipmentIcon rendering uses alpha-trimmed atlas UVs and correctly resets them when a cell changes from an item to an empty slot.
- Future off-hand atlas filenames now indicate actual tier rows after the green/purple swap. Quiver/Tome/Hidden Knife remain art-only and outside Unity Assets.
- Sources, exact prompts and active tier mapping: Assets/Resources/EquipmentArtwork/README.md. No placeholder or retired mask/cloak slots were introduced.
- Validation: Unity compile and console clean; actual GameView inspected with 45 debug-granted T4/T5/T7 items in an isolated temporary dev-save directory. The earlier 87-check native smoke run predates this artwork-only revision and was not rerun.
- No merge, push or deployment. Pre-existing regenerated font/HomeUI assets and docs/previews remain excluded.
