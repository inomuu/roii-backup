# iRO Renewal ASPD — Formula & Table (reference)

Captured from **`FJK_iRO_ASPD_Calc_1.11.xls`** (the "FJK iRO Renewal ASPD
Calc"), so the data survives in git even if the spreadsheet is lost. The
calc improves the original WIND/iRowiki formula by adding an
`ASPD_Correction` term; its author collected ASPD 151→186 from a live Arc
Bishop to fit it. Original thread:
`http://forums.irowiki.org/showthread.php?t=52590`.

> **Retired as a live rule (0.0.366).** The per-job WEAPON FAMILIARITY table
> (§4) no longer runs: classes are gone, so each unit has ONE base ASPD and
> every weapon type swings at it. This file stays as the reference the base
> numbers were derived from, not as a description of the sim.

This is where the base numbers came from for
`combat::effective_atk_speed_milli`. The living formula doc is
[`combat-formulas.md`](combat-formulas.md); the source of truth is always
the Rust code.

---

## 1. Raw spreadsheet formula (verbatim)

```
Aspd_penalty
  = IF(Job_Base_Aspd > 144, 1 − (Job_Base_Aspd − 144)/50, 1)

ASPD_Correction
  = ROUNDUP( (SQRT(205) − SQRT(AGI)) / 7.15, 3 )

Base_Aspd
  = ROUNDDOWN(
      200 − (200 − (Job_Base_Aspd + Shield_Penalty − ASPD_Correction
                    + SQRT(AGI·9.999 + DEX·0.19212) · Aspd_penalty))
            · (1 − ASPD_Potion_% − ASPD_Skill_%),
      2 )

%_ASPD_Equipment = ROUNDDOWN((195 − Base_Aspd) · ASPD_Equipment_%, 1)
Final_Aspd       = Base_Aspd + %_ASPD_Equipment + fix_ASPD_increase_equipment

ATTACKS/SECOND   = 50 / (200 − Final_Aspd)
```

- `Job_Base_Aspd` = **class base ASPD + weapon modifier** (e.g. Arc Bishop
  151 + Claw −5 = 146). See the table in §4.
- **Shield_Penalty** is a NEGATIVE value (see the Shield column).
- Max ASPD in iRO is **193**.
- Potion %: Concentration 0.10 / Awakening 0.15 / Berserk 0.20.
- Skill %: e.g. Two-Handed Quicken 0.30, Berserk 0.30, Swing Dance 0.07.

### Worked reference (matches the spreadsheet cell exactly)
`Job_Base_Aspd 151, AGI 1, DEX 1`, no shield/pot/skill/equip:
- `Aspd_penalty = 1 − (151−144)/50 = 0.86`
- `ASPD_Correction = ceil((√205 − √1)/7.15, 3) = 1.863`
- `√(1·9.999 + 1·0.19212) = √10.19112 = 3.19235`
- `Base_Aspd = ⌊151 − 1.863 + 3.19235·0.86⌋₂ = ⌊151.882⌋₂ = 151.88` ✓
- `attacks/sec = 50/(200 − 151.88) = 1.039`

---

## 2. Our adaptation (integer fixed-point)

sim-core is **float-free & deterministic** (golden hashes pin the PRNG
stream and all state). We drop the optional terms we don't model yet —
**shield penalty, potion %, skill %, equip %** are all 0 today — which
collapses `Base_Aspd` to `JB − correction + √(…)·penalty`, then implement
it in integers:

```
JB         = base_aspd + Σ equipped[i].aspd_base        (penalty ≤ 0)
penalty    = clamp(194 − JB, 0, 50) / 50                    (kept as /50)
correction = ceil((√205 − √AGI) / 7.15, 3 dp)               (subtracted)
ASPD       = JB − correction + √(AGI·9.999 + DEX·0.19212)·penalty   → floor to centi
swing_milli (per 100 ms tick) = 500000 / (20000 − ASPD_centi)   capped [50.00, 193.00]
```

- √ uses `u64::isqrt`. IEEE-754 **requires `sqrt` be correctly rounded**,
  so it is bit-stable across platforms (unlike `sin`/`exp`).
- "Bare" (no weapon) and armor carry modifier 0, so unarmed sits at the
  full job base and is the fastest baseline.
- Entities with `base_aspd ≤ 0` (monsters + hand-built test entities)
  bypass this and keep their flat `atk_speed_milli`.

Deferred (add when the systems exist): shield penalty, ASPD potions, ASPD
skills (e.g. Two-Handed Quicken), %-ASPD equipment.

---

## 3. Mapping to our classes

| class | base ASPD | starting weapon | weapon mod | item `aspd_base` |
|-------|-----------|-----------------|-----------:|-----------------:|
| swordsman | 156 | 1hSword (`starting_sword`) | −7 | −7 |
| archer | 156 | Bow (`starting_bow`) | −10 | −10 |
| thief | 156 | Dagger (`starting_dagger`) | −8 | −8 |
| acolyte | 156 | Mace (`starting_mace`) | −5 | −5 |
| **mage** | **146** | 1hStaff (`starting_staff`) | −5 | −5 |
| (drop) | — | 1hSword (`sword_short`) | −7 | −7 |
| (drop) | — | 2hSword (`sword_long`) | −14 | −14 |

> The sim stores **one modifier per item** (1-D), whereas the real table
> is job×weapon (2-D). Each weapon's `aspd_base` is taken from its
> *intended* class's row; generic drop swords use the Swordsman row.
> Mage is the only class with a non-156 base. Class bases other than Mage
> rely on `default_job_aspd() = 156`.

---

## 4. Full job base + weapon modifiers (from the spreadsheet)

`base` = the "Bare" column (job base ASPD). Each weapon entry is the
modifier ADDED to the base. Empty = class can't wield that weapon.

| Job | base | weapon modifiers |
|-----|-----:|------------------|
| Novice | 156 | Dagger −15, 1hSword −17, 1hAxe −10, Mace −10, 1hStaff −25 |
| Swordsman | 156 | Dagger −7, 1hSword −7, 2hSword −14, 1hAxe −15, 2hAxe −20, Mace −10, 1hSpear −17, 2hSpear −25 |
| Knight | 156 | Dagger −9, 1hSword −5, 2hSword −12, 1hAxe −10, 2hAxe −15, Mace −5, 1hSpear −15, 2hSpear −25 |
| Rune Knight | 156 | Dagger −10, 1hSword −12, 2hSword −15, 1hAxe −18, 2hAxe −20, Mace −5, 1hSpear −8, 2hSpear −12 |
| Acolyte | 156 | Mace −5, 1hStaff −20 |
| Priest | 156 | Mace −3, 1hStaff −20, 2hStaff −20, Book −4, Knuckle −20 |
| Arc Bishop | 151 | Mace 0, 1hStaff −15, 2hStaff −10, Book +1, Knuckle −5 |
| Mage | 146 | Dagger 0, 1hStaff −5, 2hStaff −5 |
| Wizard | 146 | Dagger −4, 1hStaff −3, 2hStaff −3 |
| Warlock | 151 | Dagger −7, 1hStaff −5, 2hStaff −5 ¹ |
| Merchant | 156 | Dagger −12, 1hSword −12, 1hAxe −8, 2hAxe −15, Mace −10 |
| Blacksmith | 156 | Dagger −10, 1hSword −10, 1hAxe −6, 2hAxe −13, Mace −8 |
| Mechanic | 156 | Dagger −20, 1hSword −25, 1hAxe −5, 2hAxe −8, Mace −8 |
| Archer | 156 | Dagger −15, Bow −10 |
| Hunter | 156 | Dagger −13, Bow −8 |
| Ranger | 156 | Dagger −10, Bow −9 |
| Thief | 156 | Dagger −8, 1hSword −10, 1hAxe −20, Bow −13 |
| Assassin | 156 | Dagger −2, 1hSword −10, 1hAxe −11, Katar −2, dual Dagger −10, dual Sword −12, dual Axe −12 |
| Guillotine Cross | 156 | Dagger −2, 1hSword −25, 1hAxe −40, Katar −2, dual Dagger −10, dual Sword −16, dual Axe −20 |
| Crusader | 156 | Dagger −8, 1hSword −3, 2hSword −15, 1hAxe −10, 2hAxe −15, Mace −5, 1hSpear −13, 2hSpear −12 |
| Royal Guard | 156 | Dagger −7, 1hSword −5, 2hSword −9, 1hAxe −8, 2hAxe −12, Mace −4, 1hSpear −10, 2hSpear −10 |
| Bard/Dancer | 156 | Dagger −13, Bow −8, Instrument −5 |
| Minstrel/Wanderer | 156 | Dagger −12, Bow −10, Instrument −5 |
| Sage | 151 | Dagger −8, 1hStaff −10, 2hStaff −10, Book +2 |
| Sorcerer | 156 | Dagger −10, 1hStaff −5, 2hStaff −5, Book −5 |
| Monk | 156 | Mace −3, 1hStaff −20, 2hStaff −18, Knuckle 0 |
| Sura | 158 | Mace −5, 1hStaff −10, 2hStaff −12, Knuckle −1 |
| Rogue | 156 | Dagger −5, 1hSword −10, Bow −10 ² |
| Shadow Chaser | 156 | Dagger −3, 1hSword −7, Bow −7 ² |
| Alchemist | 156 | Dagger −10, 1hSword −5, 1hAxe −5, 2hAxe −12, Mace −5 |
| Genetic | 156 | Dagger −10, 1hSword −4, 1hAxe −11, 2hAxe −11, Mace −5 |
| Taekwon Kid | 156 | (bare-hand) |
| Taekwon Master | 156 | Book −10 |
| Soul Linker | 146 | Dagger 0, 1hStaff −3, 2hStaff −3 |
| Ninja | 156 | Dagger −3, Huuma −15 |
| Gunslinger | 146 | Handgun +5, Rifle −5, Shotgun −40, Gatling 0, Grenade −50 |

**Shield penalty** (separate, applied via `Shield_Penalty`, NEGATIVE):
most jobs −5; Acolyte −7, Archer/Hunter −9, Ranger −8, Guillotine
Cross −9, Sage/Mage −10, Wizard −8, Mechanic/Sura/Soul Linker −6/−5/−8,
Alchemist/Genetic −4. (See the Shield column of the source for the full
per-job list.)

¹ Warlock Staff of Destruction uses −15 instead of −5.
² Rogue/Shadow Chaser 1hAxe is listed −159 in the sheet = "cannot wield";
omitted here.
