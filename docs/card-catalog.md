# Card Catalog

> Auto-summarized from `data/game_data.json` + `godot/i18n/strings.csv` (2026-07-20). Regenerate when cards change.


## Rules
- **Star scaling (0.0.218):** the **Base** *and* **Foil** stat lines scale **+20%/star, floored** — 0★ = the value shown, 5★ = ×2. Values below are the **0★** (authored) numbers.
- **Foil** is a *separate* authored layer added on top of Base (per-card, not a fixed ×2). A foil copy grants Base + Foil, both star-scaled.
- **Special** effects live in `combat::card_specials` (not stored on the item); the value shown is the **0★→5★** range. Percent stats (Crit / HP% / ATK%) are basis points rendered as %.
- **Activation:** a panel card applies only while an equipped piece feeding its slot has a free socket (`min(equipped, sockets)`).

## All cards (0★ values)

| Card | Slot | Elem | Rarity | Base | Foil (extra) | Special (0★→5★) |
|---|---|---|---|---|---|---|
| Pinkling Card | Armor | Water | Mythical | ATK +5, STR +1 | ATK +5 | — |
| Greenling Card | Armor | Earth | Mythical | HP +200, VIT +1 | HP +200 | — |
| Yellowling Card | Armor | Fire | Mythical | FLEE +10, AGI +1 | FLEE +10 | — |
| Chomb Card | Weapon | Earth | Mythical | ATK +4, Crit +2% | Crit +2% | — |
| Toxic Chom Card | Garment | Poison | Mythical | FLEE +10 | FLEE +10 | — |
| Burst Chom Card | Accessory1 | Fire | Mythical | ATK +12, DEF -3 | ATK +6 | — |
| Rechum Card | Shoes | Earth | Mythical | HP +3%, INT +1 | HP +2% | — |
| Zombie Card | Armor | Shadow | Mythical | HP +500 | HP +500 | — |
| Twinling Card | Garment | Spirit | Mythical | FLEE +15, LUK +2 | FLEE +15 | Damage dealt to Spirit-element foes +10→20%. |
| Gloomtooth Card | Head | Shadow | Mythical | ATK +6, Crit +2% | Crit +2% | Critical damage +10→20%. |
| Banshee Card | Accessory2 | Spirit | Mythical | — | MATK +5 | Basic attacks have a 3% chance to cast Spirit Strike Lv 5→10. Spirit Strike damage +50→100%. |
| Dustelle Card | Accessory1 | Neutral | Mythical | FLEE +10, AGI +2 | AGI +2 | Basic attacks have a 5→10% chance to cast Heal Lv 1 on the wearer. |
| Ashelle Card | Accessory2 | Shadow | Mythical | ATK +5, MATK +5 | MATK +5 | When healed by a skill, gain 2→4% attack speed for 5 seconds. Stacks up to 3 times. |
| Marionette Card | Garment | Shadow | Mythical | FLEE +10 | FLEE +10 | Basic attacks have a 10→20% chance to inflict Bleed (100% ATK over 5 seconds). |
| Scarlette Card | Weapon | Neutral | Mythical | — | Crit +2% | Critical damage +20→40%. |
| Crimsonne Card | Accessory1 | Shadow | Mythical | — | ATK +3 | Basic attacks have a 5% chance to drain 5→10% of the damage dealt as HP. |
| Brutus Card | Head | Shadow | Mythical | STR +5 | STR +2 | Bash deals bonus damage equal to 50→100% of the wearer's STR. |
| Gnarloak Card | Armor | Earth | Mythical | DEF +5, HP +3% | HP +2% | Reflects 5→10% of BASIC attacks taken back at the attacker. |
| Broomhilda Card | Weapon | Shadow | Mythical | MATK +5 | MATK +5 | Fire-element attacks deal +10→20% damage. |

## Notes
- **Special-only cards** (Base = —): Banshee, Scarlette, Crimsonne — their stat contribution comes entirely from the Foil layer + the Special proc.
- IDs: Pinkling Card = `card_pinkling`, Greenling Card = `card_greenling`, Yellowling Card = `card_yellowling`, Chomb Card = `card_chomb`, Toxic Chom Card = `card_toxic_chom`, Burst Chom Card = `card_burst_chom`, Rechum Card = `card_rechum`, Zombie Card = `card_zombie`, Twinling Card = `card_twinling`, Gloomtooth Card = `card_gloomtooth`, Banshee Card = `card_banshee`, Dustelle Card = `card_dustelle`, Ashelle Card = `card_ashelle`, Marionette Card = `card_marionette`, Scarlette Card = `card_scarlette`, Crimsonne Card = `card_crimsonne`, Brutus Card = `card_brutus`, Gnarloak Card = `card_gnarloak`, Broomhilda Card = `card_broomhilda`.
