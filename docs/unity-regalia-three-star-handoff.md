# Unity: Regalia 3★ set — backend done (handoff 2026-09-27)

Answer to `docs/regalia-three-star-backend-handoff-2026-09-27.md` (UI worktree). Sim 0.0.485 /
catalog 0.0.216. The ten cards, their Mods and passives are in the sim; the server gacha pays them.

## Names (user 2026-09-27)

A card has three parts: **Primary Mod** (the flat ATK), **Secondary Mod** (the advanced stat) and
the **Passive**. Use these words in the UI.

## Host contract (new keys on every Regalia item view)

| Key | Meaning |
|---|---|
| `primary_mod` | Primary Mod text at THIS card's level, display scale (e.g. `"+10 ATK"` at Lv1) |
| `primary_mod_max` | the same at Lv60 (`"+150 ATK"`) |
| `secondary_mod` | Secondary Mod text at this card's level |
| `secondary_mod_max` | the same at Lv60 |
| `overlay_passive` | unchanged: `[{key, args}]`, loc key `REGALIA_PASSIVE_*`, `{0}` now, `{1}` at S5, `{2}` extra |
| `overlay_doctrine` | unchanged — Rouge Thread's R card now reads `catalyst` |
| `passive_active` | **`owned_regalia` only**: true when a member wears it AND that member's Doctrine matches |

Print these; never run the level curve or divide in Unity. The older `overlay_lines` /
`overlay_level_lines` stay for existing screens.

## New loc keys (in `godot/i18n/strings.csv`, sync with `tools/unity_data.sh`)

- `ITEM_regalia_<id>` for all ten cards.
- `REGALIA_PASSIVE_taunt_guard_self`, `_low_hp_def`, `_ult_party_atk`, `_toughness_dmg`,
  `_debuff_applied_atk_once` (Rouge Thread: `{2}` = seconds).

## Gacha

An R pull now pays one of the ten cards (equal odds), never `regalia_fodder`. A 3★ card is also
Enhance feed: `EnhanceRegalia { target_idx, feeder_idx }` accepts one as the feeder (+5 levels),
refuses a feeder a member wears, and a 3★ card can be the target. Existing fodder still feeds.

## 0.0.486 — Resonance S1..S5

- A Regalia's Resonance now reads **S1..S5**: `resonance_stars` 0..4, a fresh card is S1, four
  duplicates reach S5. Print the new keys **`resonance_s`** (1..5) and **`resonance_s_max`** (5)
  — never `resonance_stars` itself.
- `overlay_passive` `{1}` is now the S5 value; every passive doubles S1 → S5.
- Kestrel / Meridian: `REGALIA_PASSIVE_ranged_dmg` no longer has a range (`{2}` unused) — any
  ranged attacker, point-blank included.

## 0.0.487 — Nemu's SSR renamed

`regalia_crimson_mirror` / Crimson Mirror is now **`regalia_lycoris_lullaby` / Lycoris Lullaby**
(named for its art: Nemu asleep among red spider lilies). `RegaliaCardArt.Load` must key the Nemu
signature image on the new id. Lore key: `ITEM_LORE_regalia_lycoris_lullaby`.

## Catalog 0.0.218 — the player calls them "Memory Fragment"

User 2026-09-27: every player-facing "Regalia" is now **Memory Fragment** (Thai text keeps the
English term). The ten `UI_REGALIA_*` / `UI_EXEMPLAR_REGALIA_*` / `UI_SHOP_TAB_REGALIA` /
`UI_FLOAT_OVERLAY_EFFECTS` lines are updated in `strings.csv`; the fodder items are now named
Memory Dust / Radiant Memory Dust. Code identifiers, item ids and loc KEYS keep `regalia` — only
change C# literals that SHOW the word to the player (there should be none; all go through Loc).

## 0.0.488 — SSR passives reworked (new loc keys)

Five SSR passives changed shape; the host still sends `overlay_passive` as `{key, args}`. New keys
(all in `strings.csv`): `REGALIA_PASSIVE_on_break_dmg`, `_on_break_crit_dmg`, `_on_break_crit_rate`,
`_debuffed_target_crit_dmg`, `_debuffed_target_crit_rate`, `_multi_debuff_target_dmg` (`{2}` = the
number of different statuses), `_healed_ally_dmg`. `{2}` is seconds for the timed ones.

## 0.0.489 — Enhance takes Memory Dust only; cards are salvaged first

User 2026-09-28: a Memory Fragment is no longer fed to another. The player salvages it into Dust,
and Enhance spends Dust.

- **Dust is a count, not a card.** Memory Dust (`regalia_fodder`) and Radiant Memory Dust
  (`regalia_sr_fodder`) are no longer rows of `owned_regalia` — read them from the `materials`
  list (one row per def, `quantity` = count). Old saves move automatically on load.
- **Enhance:** `{"EnhanceRegaliaDust": {"target_idx": N, "dust_id": "regalia_fodder", "count": K}}`.
  5 levels per Dust, 15 per Radiant; the sim spends only what reaching Lv60 needs and only what
  the player holds, so a "use max" button can just send a large `count`. The old
  `EnhanceRegalia {target_idx, feeder_idx}` does nothing now — remove the pick-a-card-to-feed flow.
- **Salvage one:** `{"SalvageRegalia": {"owned_idx": N}}`. **Salvage all:**
  `{"SalvageRegaliaAll": {}}` — takes only UNTOUCHED 3★ cards (Enhance Lv0, S1, unworn,
  unlocked; user 2026-09-28). Each row has `bulk_salvage: bool` — show the preview list and the
  Dust total from the rows where it is true. SR / SSR / any invested 3★ go one at a time.
  Each `owned_regalia` row now has `salvage`: `{"regalia_fodder": n, "regalia_sr_fodder": m}` —
  exactly what salvaging it pays — or `null` when the sim would refuse (worn, locked, or an SSR
  whose other copy is not yet S5). Show the button only when `salvage` is non-null and print those
  numbers; never compute them.
- **Lock:** `{"LockRegalia": {"owned_idx": N, "locked": true}}`; each row has `locked`. A locked
  card is skipped by salvage (one and all) and cannot be a Resonance feeder.
- Resonance (`FeedRegalia`, same card → S+1) is unchanged.
- New UI strings (salvage / salvage all / lock / the confirm text) need keys in
  `godot/i18n/strings.csv` as usual.

## 0.0.489 (server) — the gacha pays every Memory Fragment

- Rates: SSR 3% · SR 10% · **R 87%** (a guaranteed pull after 9 without SR: SSR 3% / SR 97%) (the R tier is back; an R pull = a 3★ card). `/gacha/state`
  banners now carry `sr_bp` and `sr_rate_bp` — print the odds from those, never hard-code 97%.
- SR on a character banner = SR unit or SR card (50/50); SR on the Memory Fragment banner = SR card.
- No Dust from the gacha any more.
- `exchange_pool` (mileage, 200 tickets) now lists all nine SSR Memory Fragments beside the six units.
- **Six SR cards are deleted**: `regalia_sunbreaker`, `_nightjar`, `_farsight`, `_velvet_mask`,
  `_stormglass`, `_choirlight` (stat-only, no passive). A save holding one gets Radiant Dust on
  load. Drop any art / sort / filter mapping that names them. 24 cards remain: 10 3★, 5 SR
  (Hoarfrost Crown, Static Charm, Emberbell, Stormglass Pin, Thunderhead), 9 SSR.

## 0.0.490 — Enhance is XP; three Dust tiers; drops by stage

- **Three Dust tiers** in `materials`: Memory Dust `regalia_fodder` = **1,000 XP**, Radiant
  `regalia_sr_fodder` = **10,000 XP**, NEW Memory Crystal `regalia_crystal_fodder` = **100,000 XP**
  (catalog 0.0.220; ends in `fodder`, so `ItemNaming.is_regalia_fodder` already covers it). The
  Upgrade page must offer all three.
- **Cap is Lv70** (was 60; `overlay_level_cap` says 70, and `*_max` / `overlay_level_lines` are
  now the Lv70 numbers). XP to Lv60: 3★ 1.2M · SR 1.6M · SSR 2M (early cheap, SSR Lv59→60 ≈ 92k);
  Lv60→70 is a steep late sink: 3★ 12M · SR 16M · SSR 20M more, each level ×1.4 the previous.
  Lv70 stats are authored per tier (SSR ATK 3,600 at Lv70 vs 3,005 at Lv60).
- Each `owned_regalia` row: `enhance_xp` (into the current level), `enhance_xp_next` (this level's bar;
  0 at Lv60), `enhance_xp_to_max`. Draw the bar from these; preview a feed by running
  `EnhanceRegaliaDust` on the native clone — never re-derive the curve. "Use max" may send the whole
  held count; the sim spends only what the cap needs.
- `salvage` now has up to three keys (`regalia_fodder`, `regalia_sr_fodder`, `regalia_crystal_fodder`);
  print every non-zero one. 3★ = 5 Radiant; SSR spare = 6 Crystals; invested XP comes back.
- Dust DROPS, more and larger the deeper the stage (first zone ≈ 65 Dust a day, stage 168 ≈ 160 Dust
  + ~10 Radiant + a Crystal every few days). They land in `materials` and the loot feed like Sigils.

## 0.0.490 — cards are held back from the first release

`QueryJson("features")` → `{"cards": false}`. While false, hide the card album, the enchant screen and
card-socket interaction (pieces still SHOW their empty card sockets — the user wants them on items).
Cards no longer drop and the shop list no longer contains them. Flip is catalog-only later.
