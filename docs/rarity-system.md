# Rarity system (DESIGN SPEC — draft)

Status: **DESIGN — draft, alpha, provisional.** Ties together how rarity is used across
[[drop-system]] (gear roll) and [[card-system]] (Normal/Rare). The enum + UI colors +
threshold routing already exist in code; this doc pins the SEMANTICS.

## Core principle: rarity is a LABEL, not a rate — EXCEPT for equipment
Rarity (`Common / Uncommon / Rare / Legendary`) is primarily a **descriptive tag**: a
name + colour that tells the player an item's value at a glance. In general the **drop
rate does NOT derive from the rarity tier** — each item/table sets its rate EXPLICITLY
(cards 1/20k + daily cap; a Rare-variant card 1/20 of its Normal; signature weights on
Axis A). The rarity word just labels the result.

**The one exception — EQUIPMENT (the Diablo gear roll, [[drop-system]] Axis B):** here
rarity IS the mechanic. The gear roll's rarity-weight profile (normal / boss) literally
decides how often each rarity drops. For gear, "drop rate follows rarity" is true by
construction; everywhere else it's coincidental / authored.

## What rarity is actually USED for (mechanically)
1. **Gear drop odds — equipment ONLY.** Axis-B roll weights per rarity, per profile
   (normal vs boss). This is the only place the tier drives the RATE. [[drop-system]]
2. **Loot routing — the rare bag.** `item.rarity >= rare_threshold` (player-set, default
   Rare) routes a NON-STACKABLE drop to the rare bag (`drops.rs`). Label-driven, all item
   kinds — but stackables (incl. cards) don't use this path; cards have the Card Pocket.
3. **UI colour + label.** One shared scheme, already in code (`RARITY_COLORS`):
   Common grey `(.72,.72,.78)` · Uncommon green `(.35,.85,.45)` · Rare blue · **Epic
   purple (NEW)** · Legendary gold `(.98,.72,.20)`. Same palette everywhere (gear, cards,
   materials). Foil cards add a shiny/animated treatment ON TOP of the Legendary gold.
4. **Sort / vendor-value hints** (descriptive).

## Per item-type: what a rarity tag MEANS
- **Equipment** — MECHANICAL tier. Common = the `_sN` base gear; Uncommon/Rare/Legendary
  (`unique_*`) = the upgrade tail in the gear roll. Rarity = both the odds AND the power.
- **Cards** — a LABEL. A card's real "rarity" is its **Normal vs Rare VARIANT**
  ([[card-system]]), which has its OWN rate (Rare ≈ 1/20 of Normal) — NOT the enum. The
  enum tag on a card is only for routing + colour.
- **Materials / ore / loot consumables** — a LABEL for colour/sort; the rate is the item's
  authored Axis-A weight.

## Cards → the enum: Epic (normal) / Foil = Legendary
Card power is read mostly by its STAR ([[card-system]]); the variant is the flavor tier
that STAMPS an enum value (one enum → uniform colour + routing):
- **Epic card (normal)** → `rarity: Epic`.
- **Foil card (the ~1/20 special variant)** → `rarity: Legendary`, plus a shiny/foil UI
  treatment layered on the gold.
- The RATE stays the card rules (1/20k, Foil 1/20 of Epic), NOT the tier — consistent with
  the core principle. (Cards tag `rarity: Rare` today → migrate to Epic/Legendary.)

## Tiers — now FIVE (Epic added for cards)
`Common / Uncommon / Rare / Epic / Legendary` (Epic slots between Rare and Legendary,
standard ARPG order). Adding `Epic` to the `Rarity` enum is a shape change → VERSION bump
+ goldens re-bless if any golden carries a rarity value (check).
- Gear uses Common..Legendary (Epic optional for gear). Cards live at Epic (normal) /
  Legendary (Foil). Don't add MORE tiers speculatively — the LABEL is cheap, the content
  behind each tier is the real cost.

## Non-goals / guardrails
- Don't let a rarity tag silently change a DROP RATE outside the gear roll — rates are
  authored per table so balance stays legible (a card's rate lives with the card rules,
  not implied by "it's Legendary").
- Keep ONE colour scheme; never per-screen palettes.

## Open decisions
- Exact colour values for Rare (blue) + the NEW Epic (purple) — the others are in code.
- Foil UI treatment (static gold-with-sheen vs animated shimmer).
- Does `rare_threshold` routing want a card-specific analogue, or is the Card Pocket the
  card path (so cards ignore `rare_threshold`)? (Leaning: Card Pocket owns cards.)
- Boss gear profile's Legendary weight vs the signature `unique_*` on Axis A — keep both
  feeding Legendary, per [[drop-system]].
