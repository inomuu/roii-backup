# Handoff to the UI session — 2026-09-20 (after the class-removal merge)

master `fa38c387` = origin/master = the deployed server (`0.0.446-alpha` / catalog `0.0.189`,
DB wiped 2026-09-19 — every old save is gone on purpose). The worktree
`/Users/lunalieneray/idle_mmorpg-gem-ui` (`codex/ui-crests`) is fast-forwarded to the same
commit; its two regenerated font `.asset` files sit in that worktree's `git stash` (Unity
regenerates them anyway). Read with `docs/ui-handoff-2026-09-19.md` (still valid) and
`docs/unity-ui-crests-handoff.md` (what shipped from the UI side).

**First thing:** `tools/unity_dylib.sh` + `tools/unity_data.sh`, restart the Editor. The
native the UI session validated was `0.0.438`; the server refuses anything but `0.0.446`.

## What changed underneath the client since 2026-09-19

1. **The class system is gone (0.0.445).** No `swordsman_t1` … `ranger_t2` anywhere in the
   catalog, sim or host. A character is a unit (`seraphine / rapis / lucielle / aveline / nyx`).
   - `Bootstrap.ClassId`, `StoryWindow` casts already switched to unit ids (merged).
   - Sprite-sheet keys in `ArenaView` (`Class("rapis") → "mage_t1"`) are ART keys and still
     work — nothing to do, but a new unit needs its sheet mapped there.
   - `GameText.cs:152-156` still has the five class blurbs (`"swordsman_t1" => "Frontline
     tank …"`) — dead strings, remove when convenient.
   - `overlay_classes` on an Exemplar item is now always `[]` (kept for payload shape).
   - `CLASS_NAME_<unit>` loc keys stay for the five units (only the ten class rows were removed).
2. **Off-hand kinds (0.0.446, catalog 0.0.189).** The `Shield` slot now holds one of four kinds:
   Shield (Seraphine), **Tome** (Lapis, Lucielle), **Quiver** (Aveline), **Hidden Knife** (Raplus)
   — gated like weapons, so a shield no longer fits everyone.
   - Member payload: `offhand_family: ["Tome"]` (same precedence as `weapon_family`) — grey the
     Off-Hand cell / bag tile the way weapons are greyed.
   - Item payload: `offhand_type: "Shield"|"Tome"|"Quiver"|"HiddenKnife"|"None"` (always
     present) and `wieldable` filled for off-hand pieces.
   - New defs: `tome_s1-7`, `quiver_s1-7`, `hidden_knife_s1-7` (nums 571-591), loc
     `ITEM_tome_s1` … (EN + TH). Art: `art/equipment/future-offhands/` atlases were committed by
     the UI session for exactly this — wire them to the three kinds.
3. **Monsters have no element (0.0.444).** Bestiary payload carries `weaknesses` / `resists`;
   `element` on a bestiary entry is the first weakness (or "Neutral") — label it "Weak to".
4. **Godot is cancelled** (`CLAUDE.md` top). Unity is the only client; ignore the "Godot dylib
   stale" line from `tools/check_online_sync.sh`. `godot/i18n/strings.csv` is still the loc
   SOURCE that `unity_data.sh` copies — keep editing it there.

5. **Gems are bag items (0.0.448, catalog 0.0.191)** — the ledger-gem Disenchant blocker is
   closed the other way round: gems now drop, stack and sit in `inventory` like any material
   (they take a slot; `is_stackable`). They no longer appear in the `materials` query at all.
   - Every `Model.Materials.Where(IsGem)` / `Materials.FirstOrDefault(m => m.def_id == gem)`
     in `ForgeModel.cs`, `GemSocketPanel.cs`, `FloatingInventoryHost*.cs` (≈6 sites) must read
     `Bag` instead. `gem_plan.held[]` already reflects the bag.
   - `disenchant_preview:<idx>` / `Disenchant { inv_idx }` now reach gems with a plain bag
     index — the Disenchant page can drop its "cannot disenchant gems" branch.
   - `SocketGem { target: {inventory:{idx}} }` still addresses the PIECE by bag index; the sim
     re-aims it if the gem stack that empties sits before the piece, so a stale index is not
     the client's problem in that one case — everything else about index staleness is unchanged.
6. **Training has three flat slots at the top (0.0.447)**: `brawn` / `stamina` / `guard` — the
   page already renders from `training` rules + `stat_mod_text`, so only the loc rows
   (`TRAIN_brawn` …) and the new "−N Damage Taken" line text are new. Nothing to wire.

7. **Sealed Cache (0.0.449, catalog 0.0.192)** — item `lootbox` ("Sealed Cache", bound, rare
   server-rolled drop). The bag needs an **Open** action → `{"OpenLootbox":{"inv_idx":N}}`; the
   sim only consumes the box. What was inside lands **after the next verified save** (bound
   crystal in the wallet, or Summon coupons in `bound_items`) — the same "after save" pattern as
   the party-slot tickets, so reuse that callout. A ×10 / ×100 win arrives as a system chat line
   (`from: ""`) "… opened a Sealed Cache — Summon ×10 · #3 of 50 · 2026-09"; a cut-in /
   screenshot frame for it is the "แคปไปอวด" moment (`docs/gacha-economy-design.md` BUILT block).
   No odds are shown anywhere in-game.

8. **Gacha is four banners at 3% / 97% SR** (server-only change, `docs/gacha-economy-design.md`
   "Banners as built"): `/gacha/state.banners[]` now has `newcomer` (kind `Newcomer`,
   `remaining` pulls left — 0 means closed, hide or grey it; `multi_coupons` 8), `standard`,
   `limited_nyx` (pick-up, 50/50), `weapon_aurora_edge`; the old `char_seraphine` id is gone.
   `pool` lists the SSRs a pool banner can pay. No R tier exists any more — every non-SSR is SR.
   The x10 price on Newcomer is 8 coupons; price everything off `multi_coupons`, not a constant.
9. **"Claim instantly"** (user): reuse `SessionBoot.RequestPrioritySave` (your commit
   `6dd68531`) after every action whose reward the server pays on save — `OpenLootbox`, party
   slot buy (already), and a pull's `ClaimMail` — so nothing waits for the 10 s autosave.

10. **Combat Score (0.0.450)** — `party_meta.combat_score` (party) and `stats:<id>.combat_score`
    (member). Display only; show it on the party header and each character card the way gacha
    games show CP (big number, up-arrow delta when it rises after an equip / star / train). No
    stage recommendation yet — that comes with the stage gate.

11. **Nemu (SSR · Striker · Dark · staff, succubus tone) — VFX asks** (sim landing as 0.0.451):
    - Ultimate **Abyssal Embrace** = Enigma's Black Hole (Dota 2), user's reference: every enemy on
      the board is drawn toward one point GRADUALLY — one cell every 0.5 s for 3 s (user: "ค่อยๆ
      pull เข้าตรงกลาง"), held and swirling as they arrive — and the big burst lands when the
      hole CLOSES, not when it opens (sim: a fused 3×3 field that pulls 1 cell / 5 ticks + Root
      + DoT, then a 5×5 detonation on expiry). The swirl / distortion / darkness is the VFX. One
      cut-in-worthy moment; the camera can hold on the hole.
    - **Velvet Bog** — a 3×3 dark viscous pool on the ground (slow 50%, DoT every 0.5 s).
    - **Heartstring** — a visible tether line Nemu ↔ target for 6 s (drains toward her).
    - **Sweet Poison** stacks (1-10) on enemies — needs a stack counter on the enemy bar.
    - Basic **Kiss of Ruin** — a dark bolt, range 6 (mage art key until she has a sheet).

12. **2026-09-22 batch — server is now `0.0.458-alpha` @ `08ce4ceb` / catalog `0.0.199`**
    (`tools/unity_dylib.sh` + `tools/unity_data.sh` already run in the main tree; restart the
    Editor). Everything from 0.0.450 to 0.0.458 is live at once:
    - **Elements are `Light` / `Dark`** in every payload (were `Holy` / `Shadow`). `CardArt` and
      `ArenaProjectiles` accept both spellings already; grep the rest of the UI for the old
      strings (`"Holy"`, `"Shadow"` as element keys) — loc `TRAIN_attune_*` / `TRAIN_ward_*`
      text now reads Light / Dark too.
    - **Two more SR units, 0.0.465**: `suzu` (Destroyer · Fire · sword+shield, art `swordsman`) and `mirei` (Catalyst · Thunder · staff+tome, art `mage`), both in the SR tier of the pool banners, one Regalia each (`emberbell`, `stormglass_pin`). New status icons: **Sunder** (−DEF), **Static Spark** (a 1-5 stack counter on the enemy — anyone's hit pops one), **Break Fever** (a self buff on Suzu).
    - **Raplus is LAPLUS** (0.0.464): display name only — id `nyx`, `limited_nyx`, art key `thief` unchanged; `CLASS_NAME_nyx` = Raplus. Her S1 (`veil_slash`) is now **Pickpocket** (no damage, applies buff kind `Dazzle` — a new status icon — and pays Gil at once); her ult applies `Dazzle` instead of `Darkness`. Skill display names come from the registry as always.
    - **Four new units** (`charon` came 0.0.463 — SSR · Destroyer · Dark · MACE + TRINKET off-hand (new `offhand_type: "Trinket"`, items `trinket_s1-7`, art `art/equipment/future-offhands/` has no trinket yet), art key `swordsman`; Blackwater Mark = buff kind `BlackwaterMark` on ONE enemy; Regalia `ferrymans_lantern`; in the SSR pool banners; `airi` came 0.0.462 — SR · Striker · Thunder · bow+quiver, art key `archer`; Static stacks 1-3 on enemies, buff kind `StaticMark`; one Regalia `thunderhead` (SR rung only — new units get ONE piece at their own rarity; Lumi's `snowdrop_charm` / `frostbell` are gone, `hoarfrost_crown` is her SR piece), loc `REGALIA_PASSIVE_atk`; in the SR tier of every pool banner): `nemu` (SSR · Striker · Dark · staff, see item 11) and `lumi` (SR ·
      Sentinel · Water/Ice · sword+shield, art key `swordsman` until she has a sheet). Both are in
      `/gacha/state` pools (`limited_nemu`; Lumi in the SR tier of every pool banner beside
      Aveline). `CLASS_NAME_lumi` loc row exists. Lumi's kit for VFX: Chill stacks (1-3) on
      enemies (same counter as Sweet Poison / Umbral Mark), Frost Taunt 5×5 ice burst, Ice Mend
      self-heal, **Frost Aura** party buff 30 s (a cold shimmer on every member; enemies that hit
      a member get Chill).
    - **Aveline is SR** (`exemplar_aveline` Mythical). Any UI that assumed five SSR exemplars.
    - **Combat Score** (item 10) and **new Regalia lines**: Sentinel #2 `snowdrop_charm /
      frostbell / hoarfrost_crown`, Striker #2 `rouge_thread / velvet_mask / lycoris_lullaby`
      (loc `ITEM_LORE_regalia_*` rows exist). New passive loc keys:
      `REGALIA_PASSIVE_debuff_applied_atk`, `REGALIA_PASSIVE_ult_heal_party`,
      `REGALIA_PASSIVE_def`.
    - **Loot**: gear odds are flat per stage, tiers climb with the stage (T6 ≈ weekly, T7 ≈
      monthly at 232+); **monsters drop Gil** now (the kill toast should show it — read
      `GilEarn::Kill` in the ledger / `gil` delta); fusion Gil return is 50% (T1-T2) / 20% (T3+)
      × a grade multiplier — the fusion PREVIEW (`fusion::preview`, 4th tuple field) already
      returns the exact payout, keep reading it rather than computing.
    - New `SkillDef` fields the skill-inspect panel may want: `heal_self_only`,
      `heal_maxhp_per_level_bp` (Ice Mend: "heals herself 10% +1%/lv max HP").
    - Status icons to add to item "Still open": **Chill** (Lumi — buff kind `FrostMark` + the
      `MarkSlow` / `MarkAspdSlow` pair, 0.0.459), **Sweet Poison** (Nemu — `UmbralMark` + the same
      pair), **Frost Aura** (party buff kind `FrostAura`, on members).

## Asked of the sim side (open, not started)
- **Early-game DEF curve**: proposal `K = 500 + 100·Lv` (today `5000 + 50·Lv`) so tier gear
  mitigates ~25-30% from Lv1 instead of ~8%; monster ATK anchors for stages 1-50 would be
  raised to compensate. Decision pending with the user. If it lands, the stat panel's
  `def_mitigation_bp` moves — nothing for the UI to change, just expect different numbers.

## Still open on the UI side (carried from 2026-09-19)

- Regalia passive / Resonance rank text (`overlay_passive`, `overlay_ranks`, `overlay_doctrine`
  — loc `REGALIA_PASSIVE_*`, `RANK_*`, `UI_DOCTRINE_*`).
- Status icons (Darkness, Bleed/Rend, Umbral Mark, Burn, Freeze …) on enemy bars.
- Real standalone Companion window + the online verified-save slot-ticket reward test.
- Bestiary "Weak to" list (item 3 above).
- Off-hand grey-out + icons (item 2 above).
