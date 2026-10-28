# Item Naming Spec

Status: **BUILT (phase 1 + phase 2 core, 2026-07-10)**. Source of truth for how a
dropped weapon/armor gets its *display name*. Nothing here touches sim-core.

- Phase 1 (base names): DONE — 273 weapon/armor SKUs named Material+Noun in
  strings.csv (`fixed`).
- Phase 2 (affix layer): DONE — `godot/views/item_naming.gd` composes
  `[Prefix] Base [of Suffix]` from rolled bonuses; wired into item_slot / modals /
  sidebar (this also migrated item-name display onto `Loc.item`). 41 affix words
  seeded EN. Verified end-to-end (8 cases).
- Not yet done: affix-word translations (EN only so far); per-locale
  `NAME_TEMPLATE` overrides; steam_market_view still reads raw `item.name`; the
  open decisions in §9.

## 0. Core principle

The item in `GameState` is **`id + rolled stats`, exactly as today — no name is
stored**. The display name is a **pure front-end derivation** from data that is
already identical on every machine (the item's `def_id` + its ordered rolled bonus
list). Because the inputs are canonical state and the derivation is a pure
function, every client computes the **same name for the same item** without any
display-time RNG — the same determinism guarantee the sim itself relies on.

Consequences:
- **Never store the name** (state bloat + un-translatable).
- **sim stays locale-agnostic** — it ships `def_id` + mod ids over the FFI, never
  a composed sentence.
- Naming lives in a **versioned front-end util** + the loc layer. Zero
  determinism/golden/market-file risk (it is presentation only).

## 1. Name structure

```
[Prefix]   [Base = Material + Noun]   [of Suffix]
  ↑ from first PREFIX-category rolled mod
             ↑ carries TIER + weapon kind (fixed, translatable)
                                        ↑ from first SUFFIX-category rolled mod
```

Example: `Blazing  Steel Saber  of the Ox`

The **base** always shows (it is the item's identity + tier). The prefix/suffix
are added from the rolled bonus lines per §4.

## 2. Base name — Material ladder × Weapon noun

The base carries the tier so a player reads power at a glance. It **scales to
s20-s30+** by extending the material ladder — adding a future tier is appending
one material word, never renaming old gear (critical for the +10-cap Update Kit
cadence, see [[endgame-economy-vision]]).

### Tier → band → material (current s1-s7, ladder pre-extended to s20)

Each band drops exactly ONE gear tier (verified from drop tables):

| Tier | Drops in bands | Material |
|------|----------------|----------|
| s1 | b1–b3 | Bronze |
| s2 | b4–b5 | Iron |
| s3 | b6–b10 | Steel |
| s4 | b11–b15 | Cobalt |
| s5 | b16–b17 | Silver |
| s6 | b18–b19 | Mithril |
| s7 | b20 | Adamant |
| s8 | (future) | Obsidian |
| s9 | | Meteoric |
| s10 | | Runed |
| s11 | | Dragonbone |
| s12 | | Wyrmsteel |
| s13 | | Astral |
| s14 | | Celestial |
| s15 | | Void |
| s16 | | Empyrean |
| s17 | | Godsteel |
| s18 | | Aetherium |
| s19 | | Primordial |
| s20 | | Eternal |

s21+ extends the same list (Starforged, Ascendant, Mythic, Transcendent, …).

### Weapon nouns (6 types)

Default = a fixed noun per type. Optional richness = the noun **climbs every ~5
tiers** so the whole line isn't monotone; the material stays the primary tier
signal. (Decision open — default to fixed nouns for launch.)

| weapon_type (id) | Fixed noun | Climbing option (tiers 1-5 / 6-10 / 11-15 / 16-20) |
|---|---|---|
| Sword1H (`sword`) | Saber | Saber → Falchion → Broadsword → Warblade |
| Sword2H (`greatsword`) | Greatsword | Greatsword → Claymore → Zweibrand → Warcleaver |
| Dagger (`dagger`) | Dirk | Dirk → Kris → Stiletto → Fang |
| Bow (`bow`) | Longbow | Shortbow → Longbow → Warbow → Siegebow |
| Staff (`rod`) | Rod | Rod → Scepter → Staff → Runestaff |
| Mace (`mace`) | Warmace | Cudgel → Warmace → Maul → Skullcrusher |

### Armor nouns (by equip slot)

Same material ladder (metal = tier, even on soft slots, for at-a-glance tier
legibility). Family id → noun:

| Family (slot) | Noun |
|---|---|
| `armor` (Armor/body) | Mail |
| `shield` (Shield) | Shield |
| `garment` (Garment) | Cloak |
| `shoes` (Shoes) | Boots |
| `helm` (HeadTop) | Helm |
| `mask` (HeadMid) | Mask |
| `charm` (HeadLow) | Charm |
| `ring` (Accessory1) | Ring |
| `pendant` (Accessory2) | Pendant |

e.g. `Steel Shield`, `Cobalt Helm`, `Silver Cloak`, `Adamant Ring`. Legacy
`leather_armor`/`chain_armor` → `Leather Armor`/`Chain Mail`. Elemental quivers
(`arrow_*`, Shield slot) and `unique_plate` keep their own names.

Base names live as `fixed`-flag `ITEM_<id>` rows in `strings.csv` (kept EN across
locales, like all item names). They **replace the current placeholder** rows
(`ITEM_sword_s3_1s = "Sword s3 [1]"` → `"Steel Saber"`). Socket count is NOT in
the name (the tooltip already shows `Sockets: N/M`).

## 3. Uniques are exempt

Hand-authored uniques (Legendary: Dawnbreaker, Ruincleaver, …) keep their **fixed
`ITEM_<id>` name** and get NO affix generation. Affix naming applies only to
rolled gear (Common → Rare). This is the "signature name" tier.

## 4. Rarity (star count) → affix rule

Stars = rolled bonus-line count ([[poe-gear-direction]], roll-at-drop). The name
adds **at most one prefix + one suffix**, chosen as the FIRST mod of each category
walking the bonus list in its stored order:

| Stars | Mods | Name |
|-------|------|------|
| 0★ | 0 | base only — `Steel Saber` |
| 1★ | 1 | base + the one affix — `Blazing Steel Saber` **or** `Steel Saber of the Ox` |
| 2★ | 2 | first prefix + first suffix — `Blazing Steel Saber of the Ox` |
| 3–5★ | 3–5 | still first-prefix + first-suffix; extra mods show in the tooltip only |

Selection rules:
- Walk the ordered bonus list once. First `PREFIX`-category mod → prefix word;
  first `SUFFIX`-category mod → suffix word.
- **Cap 1 + 1.** A second prefix (e.g. two offense mods, no attribute) is ignored
  for the name — result is prefix-only (`Blazing Steel Saber`).
- **Empty category → omit that slot.** All-attribute rolls → suffix-only
  (`Steel Saber of the Ox`).
- Everything not in the name is already visible in the tooltip's rule-text lines.

This is deterministic with **no hash needed** — the bonus list is ordered state,
"first of each category" is pure, so all clients agree.

### Reroll behaviour (Star Stones) — falls out for free

The name is recomputed from current state, so a reroll updates it with no special
handling, deterministically (the reroll itself draws PRNG in fixed order / is a
server-roll for rare outcomes, [[server-roll-refine]]):

- **RerollValues** (keeps line types + order, rerolls numbers) → affix words
  **unchanged** (fire is still `Blazing`), only tooltip numbers move.
- **RerollAll** (rerolls the lines) → affix words **change** (`Blazing` →
  `Frostbound`) as the first-of-category mods change.

## 5. Mod → affix category map (fixed)

Every `GearMod` variant is classified once. Drives §4's "first of each category".

### PREFIX (offense / power)

| GearMod | Prefix word (weak → strong roll) |
|---|---|
| `AtkBp` / `AtkFlat` | Honed · Vicious · Brutal · Merciless |
| `MatkBp` / `MatkFlat` | Charged · Arcane · Eldritch |
| `CritBp` | Keen · Deadly · Fatal |
| `IgnoreDefBp` | Piercing · Sundering |
| `DmgVsElement` (word keys on the element) | Fire: Smoldering/Flaming/Blazing · Water: Chilling/Frostbound/Glacial · Wind: Gusting/Storming/Tempest · Earth: Craggy/Stonebound/Tectonic · Thunder: Sparking/Shocking/Thunderstruck · Holy: Blessed/Hallowed/Radiant · Shadow: Shaded/Umbral/Dreadbound · Poison: Tainted/Venomous/Virulent |
| `DmgVsRace` / `DmgVsSize` | Beastbane · Giantsbane · Slayer's |

### SUFFIX (attribute / defense / utility)

| GearMod | Suffix "of ~" |
|---|---|
| `Str` | of Might · of the Bear |
| `Agi` | of Alacrity · of the Fox |
| `Vit` | of Vigor · of the Ox |
| `Int` | of the Mind · of the Owl |
| `Dex` | of Precision · of the Hawk |
| `Luk` | of Fortune |
| `MaxHp` / `MaxHpBp` | of Vitality · of the Colossus |
| `Flee` | of Evasion |
| `Hit` | of Accuracy |
| `MoveSpeedBp` | of Swiftness · of the Gale |
| `ResistElement` (keys on element) | of Warding (of Fire Ward, of Frostward, …) |
| `ResistStatusBp` | of Steadfastness · of the Unshaken |
| `DrVsRace` / `DrVsSize` | of the Bulwark |
| `DrVsRanged` | of Deflection |
| `CooldownBp` | of Quickening |
| `CastBp` | of Focus |
| `HealBp` | of Mending |

"weak → strong" = optional **tiered words** (bigger roll magnitude → grander
word). Adds flavor at ~3× vocabulary. Decision open; default to ONE word per mod
for launch, add tiers later.

## 6. i18n

Bounded vocabulary (~60–80 affix words + ~20 materials + 6 nouns) = translate
once, not per item.

- Base: `ITEM_<id>` (`fixed`, kept EN) — already the mechanism.
- Affix words: one key per category value, e.g. `AFFIX_PRE_fire3 = "Blazing"`,
  `AFFIX_SUF_vit = "of the Ox"` (translatable, blank flag).
- **Word-order template per language** (NOT per item): EN `"{pre} {base} {suf}"`;
  Thai/CJK reorder + drop spaces, e.g. `"{pre}{base}{suf}"`. One template row per
  locale (e.g. key `NAME_TEMPLATE`). This is the only grammar surface — a fixed,
  bounded cost, unlike composing a unique sentence per item.
- The front-end resolves: pick affix keys (deterministic, §4) → `Loc` each word in
  the viewer's locale → assemble via that locale's template. A traded item shows in
  each viewer's language but is the SAME item (identity = `instance_id`, never the
  name).

## 7. Where it lives

- `godot/views/item_naming.gd` (new, static util — like `item_desc.gd`): takes an
  item view dict, returns the composed display name. Pure, versioned.
- Wired wherever a name is shown: `item_slot.gd` (grid), `modals_view` tooltip,
  sidebar, shop, etc. — all currently read `item.name`; they instead call the util.
- sim / `game_data.json` untouched.

## 8. Implementation phases

1. **Base names now** (independent, shippable): replace placeholder `ITEM_<id>`
   rows with `Material + Noun` for all rolled gear (s1-s7 × 6 types × socket
   variants). Pure loc edit, no code. Translatable immediately.
2. **Affix layer** (with enchant depth / market): the `item_naming.gd` util +
   affix keys + per-locale template + category map. Gate behind a feature check so
   it can land without disturbing base names.
3. **Later polish**: tiered affix words; optional 5★ "signature title" layer;
   climbing weapon nouns.

## 9. Open decisions

- Fixed noun vs climbing noun per type (§2). Default: fixed.
- One affix word per mod vs tiered by roll magnitude (§5). Default: one.
- 3–5★ signature-title layer (§4) — deferred; uniform rule for launch.
