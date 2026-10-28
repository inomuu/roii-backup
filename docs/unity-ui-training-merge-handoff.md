# Unity UI merge handoff — 2026-09-18

Source: `codex/ui-training`, worktree `/Users/lunalieneray/idle_mmorpg-gem-ui`.
The master session should perform the merge. This branch has not merged/rebased master.

## Included work

- Floating Training panel beside Character/Inventory, bottom navigation next to Blacksmith.
  All 23 native training slots, three groups, native gates/prices, white icon atlas, and
  `Upgradeable First` checkbox. Uses `training_rules` and `Train`; no client balance rules.
  Large currency values use unsigned 64-bit parsing and exact formatted values.
- Combat HUD portraits, HP bars, ultimate gauge/crest, bottom fade, top-right management
  buttons, smooth stage percentage and layered boss bars. These replace the prior HUD styling.
- Arena sparks fix: radial transparency and explicit blend settings prevent black VFX quads.
- Wider floating Character/Inventory (620 units), five-column bag, rarity surfaces, actual
  item level lower-right, no tile name/T label, static gradient/star backdrop. Equipment
  accessory positions stay two lower slots on each side. Hover/drag/socket interactions stay.
- Seraphine PNG equipment preview with feathered edges and separate combat portrait;
  pixel character remains in front. User-supplied artwork and its Unity `.meta` files included.
- Auto Equip, effective-stat presentation, and exact native preview cloning. Full Stats and
  right-side tool can stay open together; only the active tool highlights in bottom navigation.
- Localization CSV mirrors updated together; no new Godot feature work.

## Merge requirements

1. **Rebuild sim-ffi for each target and restart the Editor.** C# now calls the new `roi_clone`
   symbol. Do not use a previously staged dylib/DLL. Native binaries/catalog staging are not
   included in Git; use the merged checkout's staging tools and latest catalog.
2. Preserve master gameplay/schema changes. This branch's native runtime is sim 0.0.415.
   Master had advanced to 0.0.436 during testing. This branch only adds `Session: Clone`,
   `roi_clone`, and its regression test to Rust; it does not change sim-core balance/version.
3. Resolve shared Unity HUD/read-model and localization edits by behavior, not wholesale
   replacement. Master HP display scaling belongs in the host; do not multiply it in UI.
   UI now reads effective ATK/DEF/HP and `atk_speed_x100`; no visible MATK field.
4. Preserve the two matching localization CSVs and check duplicate keys after merge.
5. TMP generated glyph/atlas caches and `docs/previews/` are deliberately excluded.
   No external equipment icon pack was imported; Ravenmore/Flare were recommendations only.

## Open issues / do not mark resolved without verification

- **Intermittent equip/drag rejection reported by user:** all equipment targets turned red and
  right-click/drag would not equip, then recovered by itself in Game View. Exact-clone previews
  fix one confirmed source of rejection (save/load migrating accessory data), but the original
  intermittent incident was not captured and replayed. Keep it open until reproduced/verified;
  collect selected entity, inventory revision, compatibility reason and native preview result.
- **Companion standalone follow-up:** earlier user reports included low FPS/crash, UI stalls,
  freeze on exiting Companion and death not restarting. This commit does not establish that
  all those historical reports are fixed. Recheck in the merged native standalone build;
  Editor Game View cannot validate OS transparency/window lifecycle or exit behavior.
- **Forge label truncation:** latest Game View still shows the auto-fill button as
  `Auto-fill one b...` at current side-panel width. Widen/reflow the label in a polish pass.
- **Placeholder equipment art:** occupied tiles still use white equipment symbols, not unique
  colored item artwork. No downloaded icon pack is integrated yet. Only Seraphine currently has
  the new equipment illustration; other characters fall back to the pixel preview.
- **Native binary mismatch risk:** stale plugins missing `roi_clone` will fail when a verified
  action/Auto Equip needs a preview. Rebuild/restart is required, not an optional UI workaround.

## Validation and limits

- Earlier implementation checks: sim-ffi 7 passed; sim-core lib 467 passed, 1 ignored.
  Exact-clone test covers inventory/hash/replay/debug permissions and independent ownership.
- Auto Equip disposable native fixture: 9 upgrades, item conservation, no teammate changes,
  illegal gear rejected, single publication, repeated click no-op; 165 ms measured locally.
- Latest Game View: 23 bag cells checked for five columns, actual level and absent name label.
  Main + Full Stats + Forge fit the 2560×1440 canvas; Unity error console empty.
- Before commit: localization check passed with 1,999 keys and `git diff --check` passed.
- Latest layout changes were checked in Editor Game View. Repeat native transparent Companion
  smoke after merge/rebuild, including scaling, hover/drag, Training purchase, gem/fusion result,
  Auto Equip, and background combat. Do not infer standalone macOS/Windows coverage from Editor.

Details: `docs/unity-auto-equip-preview.md`. Original Training design/test checklist:
`docs/unity-training-ui-agent-plan.md` (planning dimensions are superseded by this implementation).

## Master session follow-up (2026-09-18, after the merge)

- **Intermittent equip rejection — cause found and fixed (client):** `PrepareDrag` previewed the drop
  against the 0.4 s timer's `_equipment` snapshot while `PreviewDropSlot` cloned the LIVE sim; any bag
  change in between (a drop, a stack merge, auto-fusion firing every frame) failed the identity
  multiset check for every slot → all targets red until a quiet refresh. Drag start and right-click
  now re-read the snapshot from the sim first (`FreshSnapshot`) and the payload's fingerprints follow
  it. Still to verify in the Editor under an auto-fusion streak.
- `Session` now shares its catalog between a live session and its clones (`Arc<GameData>`) and a clone
  carries no pending anchor; the redundant `StateHash` equality check per Auto Equip candidate is gone
  (each clone deep-copied ~800 KB of catalog and hashed two full states — up to hundreds of times per click).
- Forge auto-fill label shortened to fit its button.
- `overlay_classes` is empty for good (class system retired); the Regalia page shows `overlay_doctrine`
  through `UI_DOCTRINE_*`.
- Companion standalone reports (FPS, freeze on exit, death not restarting) were not reproduced by
  reading: `DeathBanner` keeps receiving views with the HUD hidden and the focus pause is released in
  `OnDisable`/`EndFocus`. Needs the standalone build.
