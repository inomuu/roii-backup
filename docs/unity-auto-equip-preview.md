# Equipment preview and Auto Equip — 2026-09-18

Worktree: `idle_mmorpg-gem-ui`, branch `codex/ui-training`.

- Battle floating inventory scales with screen height and clamps to the available screen.
  Companion retains its existing user-selected scale.
- Equipment Seraphine illustration uses the user's `Sword.png`, imported separately into
  `Resources/EquipmentPortraits/Seraphine.png` (max 1024, alpha, no mipmaps). The combat HUD
  keeps its original portrait. The illustration is smaller, preserves aspect, and fades at
  all four edges using retained UI geometry. Pixel art sits in front, lower in the panel.
- Auto Equip operates on the selected character and bag only. Rank: grade first, then
  `max(graded_atk, graded_matk) + graded_def`, then item level. Equal items stay equipped.
  This is a convenience ranking, not a build/synergy optimiser; native inputs retain all
  level, weapon, and accessory placement rules. No teammate equipment is taken.
- Planning checks exact item conservation in disposable native copies; the final verified
  sequence guards against stale inventory and publishes the completed loadout once.

## Native bridge requirement

New `roi_clone` / `SimSession.Clone()` copies the complete session (including replay and
debug permission). `VerifiedInputSequence` now uses this instead of save/load, retaining its
exact hash guard. Save/load may migrate an unnormalised accessory, so it is not an identical
preview. This also avoids repeatedly parsing the catalog. No sim-core rule/schema change.

Rebuild/stage sim-ffi for every target platform before shipping these C# files. macOS release
was staged with `tools/unity_dylib.sh` and the isolated Editor fully restarted. Native plugins
cannot be updated by a play-mode restart alone.

## Validation

- sim-ffi: 7 passed, including exact clone independence, replay, inventory and debug permission.
  Deliberately changing cloned debug permission made the new test fail; restored and reran.
- sim-core lib: 467 passed, 1 ignored. Localization: 1,999 keys. `git diff --check` clean.
- Disposable Unity native fixture: 9 upgrades, item count conserved, Rapis unchanged, illegal
  weapon / level-locked armor excluded, one publication, repeated click no-op. No save/load
  normalization needed. Measured 165 ms versus the earlier 5,698 ms save/load implementation.
- Grade-first comparator checked with higher grade but weaker stats; stats break grade ties.
- Actual Game view inspected with the new Sword illustration; Unity error console empty.

Current portrait is supplied only for Seraphine. Other units retain their pixel preview until
their matching illustration is provided. No account save was used for the isolated fixture.

## Follow-up: combat stat presentation

- Illustration now zooms 2x with an aspect-correct UV crop; four-edge feather retained.
- Floating summary is a compact aligned Level / Max HP / ATK / DEF table. Amounts use 64-bit
  DTO fields, grouped exact digits and auto-sized value columns.
- MATK removed from floating full stats, Exemplar overview/equipment, and legacy CharacterWindow.
  Shared Exemplar model reads `atk_speed_x100`, replacing retired `aspd`, displayed as attacks/s.
- ATK / DEF are native effective totals; removed misleading raw-gear/base splits. UI performs
  no HP/ATK/DEF rescaling or formula reconstruction.
- Worktree still runs sim 0.0.415. Master observed at 0.0.436 changes party HP display scale;
  merge/rebuild must carry that native contract, not a compensating client-side multiplier.
- Battle management scale reduced from height/720 ×1.10 to ×0.90. Full Stats can stay open
  to the left while Forge / Training / Socket occupies the right; group width is 620/950/1280.
  Existing companion sizing reads that dynamic width. Confirmation and forge result overlays
  cover the expanded group. Bottom tool buttons highlight only the currently open tool;
  Full Stats highlights independently while its panel is visible.
- Character/inventory main panel widened from 500 to 620, with a 270-wide character section
  and five inventory columns (56-unit tiles, 62-unit pitch). Shared cells show rarity background
  and actual `item_level` at bottom-right; names/grade labels remain available in tooltips.
  Gem quantities and selection/socket marks remain. Inventory background uses one static
  gradient/star mesh with no raycast interception or per-frame animation cost.
