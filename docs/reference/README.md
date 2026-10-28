# RO pre-renewal monster reference

Reference data for balancing band monsters against classic RO **pre-renewal** scale.

## Files
- **`ro_prere_monsters.csv`** — every monster (1004), all scalar stat fields, sorted
  by Level then Id. Easiest to query/filter (e.g. monsters near a target level).
- **`ro_prere_monsters.json`** — full entries including `Drops` / `MvpDrops` / `Modes`.
- Regenerate both with: `python3.12 tools/fetch_ro_mobdb.py`

## Source & credit
Derived from **rAthena**'s `db/pre-re/mob_db.yml`
(<https://github.com/rathena/rathena>, GPL-licensed). We store only the derived
stat tables (the numbers are factual game data) + the fetch/parse script, crediting
the source — not rAthena's raw file. Pre-renewal dataset (confirmed: Poring = 50 HP).

## Fields (CSV columns)
`Id, AegisName, Name, Level, Hp, Sp, BaseExp, JobExp, MvpExp,
Attack, Attack2` (ATK min–max), `Defense` (hard DEF), `MagicDefense` (MDEF),
`Resistance, MagicResistance, Str, Agi, Vit, Int, Dex, Luk,
Hit, Flee` (DERIVED — not stored in the DB; pre-re mob engine values
`Hit = Level+Dex`, `Flee = Level+Agi`), `AttackRange, SkillRange, ChaseRange,
Size, Race, Element, ElementLevel,
WalkSpeed, AttackDelay, AttackMotion, DamageMotion, DamageTaken,
Ai, Class, IsMvp, IsBoss, DropCount`.

## Notes for balance use
- **HP** scales smoothly with level (≈ `1.4·Lv^2.18` for normal mobs) — a good
  driver for predictable pacing.
- **ATK** is per-monster tuned (noisy, non-monotonic across levels) — treat as a
  per-band knob, not a formula.
- **DEF** is often 0 on pre-re mobs; **MDEF** is a per-mob "magic-resistant?" flag
  (most 0–15; casters/demons/undead spike to 40–60). Don't derive either from level.
- Filter out inert egg-state mobs (Pupa, Ant Egg — inflated HP) when picking
  representatives.
- **Hit/Flee** are the mob's RAW engine stats (`Level+Dex` / `Level+Agi`). These
  are NOT ratemyserver's `Hit(100%)` / `Flee(95%)` — those are PLAYER-facing
  derived numbers (the player Hit needed to 100%-hit the mob ≈ its Flee; the
  player Flee needed to 95%-dodge it ≈ its Hit + ~95), and RMS often shows the
  RENEWAL calc. Use the raw values here as the balance reference.
