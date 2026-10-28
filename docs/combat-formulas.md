# Combat & Stat Formulas (living doc)

Every formula the sim uses. **Source of truth = the Rust code**
(`stats.rs`, `combat.rs`, `tick.rs`); update this when they change.

Direction (decided 2026-06-16): **RO Renewal-flavoured** — formulas are
**linear** (no pre-renewal `⌊stat/10⌋²` spikes) and **BaseLevel feeds
power** (`⌊Lvl/4⌋`), because linear is far easier to balance long-term,
especially for future PvP / guild war. Plan: PvE retention via endless
level/prestige; PvP balanced via normalised brackets (so endless PvE
power doesn't break PvP). All formulas are **integer-only & deterministic**
— physical hits keep RO's weapon-level damage variance via a SINGLE pinned
PRNG draw (magic takes none), and all conditional draws have a fixed order,
so golden hashes stay pinned. See [[combat-damage-design]].

- **Tick = 100 ms** → 10 ticks/s. bp = basis points (10000 = 100%).

## Stat → derived (`stats.rs`)
| From | Derived | Formula |
|---|---|---|
| STR | melee StatusATK | `⌊Lvl/4⌋ + STR + ⌊DEX/5⌋ + ⌊LUK/3⌋` |
| DEX | ranged StatusATK | `⌊Lvl/4⌋ + ⌊STR/5⌋ + DEX + ⌊LUK/3⌋` |
| INT | StatusMATK | `⌊Lvl/4⌋ + INT + ⌊INT/2⌋ + ⌊DEX/5⌋ + ⌊LUK/3⌋` |
| VIT | soft DEF (flat) | `⌊VIT/2 + AGI/5 + Lvl/2⌋` |
| INT | soft MDEF (flat) | `⌊INT/5⌋` |
| AGI/DEX | ASPD | RO model — see effective_atk_speed_milli below |
| VIT | max HP | `+1%` each (multiplier — see effective_max_hp) |
| INT | max SP | `+1%` each (multiplier — see effective_max_sp) |
| LUK | crit chance | `(LUK − 1) × 30 bp` = 0.3%/pt above baseline |
| — | **HIT** | `175 + BaseLvl + DEX + ⌊LUK/3⌋` |
| — | **FLEE** | `100 + BaseLvl + AGI + ⌊LUK/5⌋` (before mobbed penalty) |

DEF is split into **hard** (base + equip → %-reduction) and
**soft** (the VIT/AGI/Lvl flat above → subtracted after). MDEF is still
soft-only (flat) — hard/equipment MDEF % is future.

## effective_* (`combat.rs`) — how it all combines
- **effective_atk** = base `atk` + Σ(equip atk_bonus) + StatusATK
  (melee if `proj_travel_ticks==0` else ranged) + `passive_atk_bonus`
  (Keen Eye) + `aura_atk` (Blessing). **Class base `atk` is 0** — damage
  comes from the equipped WEAPON (RO rule). Each class spawns with a
  `starting_weapon` (sword/bow/dagger = atk_bonus; staff/mace = matk_bonus).
- **effective_matk** = base `atk` + StatusMATK + Σ(equip `matk_bonus`).
  Physical `atk_bonus` does NOT raise MATK and vice-versa (separate pools).
- **effective_def** = base `def` + Σ(equip def_bonus) + soft DEF.
- **effective_mdef** = soft MDEF.
- **effective_max_hp** = `(max_hp + hp_per_level × BaseLvl) × (1 + VIT/100)`
  — RO Renewal job growth. `max_hp` = class level-1 base, `hp_per_level`
  per-class (warriors high). Monsters: hp_per_level 0 → flat HP.
- **effective_max_sp** = `(sp_base + sp_per_level × BaseLvl) × (1 + INT/100)`
  — per-class base + growth (casters ≫ warriors). Monsters: sp_base 0.
- **effective_atk_speed_milli** — **attack speed, LoL/Dota model** (0.0.367).
  One formula for every entity, player and monster:
  ```
  rate = base_rate × (1 + atk_speed_bonus) × (1 − slow)   clamped to [0.20, 10.00] atk/s
  ```
  `base_rate` is the template's own `atk_speed_milli` — milli-swings per
  100 ms tick, and the accumulator spends 1000 per swing, so **`milli / 100`
  is attacks per second** and that is exactly what the UI shows
  (`atk_speed_x100` in the stats payload → "1.06 /s"). Per-unit rates today:
  seraphine **1.06**, rapis **0.92**, monsters ~**0.70**.
  - **atk_speed_bonus** = every speed source summed ADDITIVELY in bp: `AspdPct`
    (Sword Quicken, monster pack-buffs), `AspdPotionPct`, `CardAspdStack`,
    each equipped item's `atk_speed_bp` (unique speed lines; ordinary gear 0),
    minus the **shield** weight (−10%, shaved 1 pt/level by Sword & Shield
    Mastery, gone at Lv10) and **dual-wield** (−18%, the off-hand's own
    `atk_speed_bp` excluded so this is its whole cost). Capped at **+200%**
    (`ATK_SPEED_BONUS_CAP_BP`) — that is the balance bound.
  - **slow** (`AspdSlowPct`, Aspd Dampening) stays a FINAL multiplier rather
    than a negative bonus, so it bites hardest on an already-sped-up target, and
    it works on monsters and in PvP because it reads no attribute.
  - The rate clamp is the tick's physical ceiling (1000 milli = one swing per
    tick), not a balance dial; a `base_rate` of 0 means "no attack rate
    authored" and stays 0.
  - Level and attributes do NOT touch attack speed (AGI/DEX went in 0.0.87),
    and neither does weapon type (familiarity went in 0.0.366).

  **What this replaced** (and why, so nobody re-derives it): RO's ASPD-point
  model — a 146-207 scale through a hyperbola `50/(200−ASPD)` with a knee at
  180, per-weapon familiarity tables, and a `%` that folded as
  `200 − (200−ASPD)·(1−pct)`. It existed to serve AGI/DEX and familiarity,
  both retired. What it left was a number no player could read and a "+30%"
  that actually gave +43% — and, because the fold is non-linear in the base,
  one "+ASPD" affix was worth different amounts on different units, which is
  poison for a market where every buyer should price a line the same.
  `aspd-table-iro.md` is kept as the reference those base numbers came from.

## Damage (`combat.rs`)
1. **Build** packet (`build_damage`): `amount = effective_atk` — ONE attack number for every
   damage type (**MATK is gone**, 0.0.437; a staff's number lives in `atk_bonus`). Skills scale it:
   `build_damage_scaled` → `amount × power_bp / 10000`, where `power_bp` comes through
   `unit_rank::power` (the S3/S5 level ranks read above the board's cap, then the "+X% DMG"
   add-on bucket of the reached ranks). A **basic** swing is `basic_power_bp` (a passive's
   `basic_power_bp` override — Seraphine 80%, Aveline 90% — × the basic ranks), melee and ranged
   alike (0.0.441: the melee path had skipped the override).
2. **Weapon variance** (`apply_weapon_variance`, PHYSICAL only): a
   connecting hit rolls its damage DOWN from max by `0..=weapon_level × 5%`
   (`weapon_variance_bp = weapon_level × 500`), via ONE PRNG draw placed
   AFTER the crit/hit rolls so the stream stays simple. The full damage is
   the **ceiling** — RO never rolls above max. `weapon_level` = the highest
   equipped weapon's level (no weapon / monsters = 1 → −0..5%); higher-tier
   weapons swing wider. Shared by the melee resolve + the ranged projectile
   path. Magic/True take **no** draw (zero variance — keeps casts pinned).
3. **Apply** (`apply_damage`) on the target — Renewal two-layer DEF:
   - Magic → `raw = max(1, amount − soft MDEF)`.
   - Physical, **crits included** (0.0.224) — the old "crit ignores ALL DEF"
     boss-killer rule is RETIRED: it made crit builds strictly dominant. A crit
     now MULTIPLIES (`effective_crit_dmg_bp`, applied at resolve) and is
     mitigated by DEF exactly like any other hit. Guard test:
     `crit_multiplies_and_def_applies_renewal_style`.
   - Physical → hard DEF gives a **diminishing %-reduction**, then soft
     DEF subtracts flat:
     `after_hard = amount × (K + hardDEF) / (K + FACTOR·hardDEF)`,
     `raw = max(1, after_hard − softDEF)`.
     `K = HARD_DEF_K = 4000`, `FACTOR = HARD_DEF_FACTOR = 10` → caps at
     90% reduction, never cliffs to 1. **Tuning note:** at our small DEF
     (tens) K=4000 barely reduces — lower K (or scale DEF up) to make DEF
     matter. Monsters use this same path (symmetric), but sit at level 1
     so they don't get the ⌊Lvl/4⌋ ATK/DEF growth players do.
   - `final = max(1, raw × element_mult_vs / 10000)`
4. **Weakness list** (`element_mult_vs`, `combat/damage.rs`; 0.0.444 — the matrix is GONE):
   a **monster has no element**, only `weaknesses` / `resists`: an attack of a listed weakness
   is **120%** (`WEAKNESS_BP`), a resisted one **80%**, anything else **100%**; a monster with no
   list takes 100% from everything. The player side is never weak to anything (0.0.359). Six
   elements, and only six: Physical, Fire, Lightning (Thunder), Ice (Water), Light, Dark.
   Earth / Wind / Poison / Spirit are CANCELLED (user 2026-09-23) — unused enum variants and a
   few legacy cards, never a design option. Frozen counts as weak to Ice;
   Holy Bane and `DmgVsElement` lines pay against a target weak to that element. Then, in order:
   the attacker's `ElementDmgBp` / mastery, the target's `DmgReducePct` (Arcane Ward, Regalia
   taunt guard) and `DmgTakenPct` (Break, Iron Chains / Meteor ranks, Raplus's Silent Verdict),
   Holy / Shadow vulnerability (Sanctus Terra, Lumen, Umbral Mark — stronger mark wins, never
   sums), Umbral Exploit (+bp per DISTINCT debuff kind on the target), Potential reductions.
5. **AoE** (`SkillDef.aoe_radius`, e.g. Fireball=1): the projectile hits
   every alive `victim_team` entity within Chebyshev radius of impact;
   the same pre-computed `damage` is applied to each (each kill rolls its
   own drop).

## Accuracy / crit / flee (`combat.rs`)
- **Accuracy** `= clamp(80 + HIT − FLEE, 5%, 100%)` (`HIT_RATE_BASE_PCT
  80`, floor `HIT_FLOOR_BP 500`). At ≥100% `roll_hit` takes **no** PRNG
  draw (keeps STAT_MIN goldens pinned).
- **Crit**: rolled only when `crit_chance > 0`; a crit skips the hit roll
  (always lands) but **DEF still applies** (0.0.224 — see §Apply; the
  def-ignore rule was retired for being too strong). Draw order per physical hit: crit →
  hit → weapon variance (variance only on a connecting, non-zero hit).
- **Magic**: never crits, never misses, **no** weapon variance — takes
  **zero** draws.
- **Mobbed flee** (Renewal): defender's FLEE is cut `10%` per attacker
  beyond the 2nd (`MOB_FLEE_FREE 2`, `MOB_FLEE_STEP_PCT 10`), floored at 0.
- **Double Attack** (thief passive): a BASIC swing (power 100%) has
  `500 bp × level` (5%/lvl, cap 100%) chance to strike twice — rolled only
  when the attacker owns the passive, so other entities' streams are
  untouched.

## Projectile flight (`tick.rs`)
Ranged attacks fly at a **constant visual speed**: scaled by distance so
an adjacent target resolves almost instantly:
`flight_ticks = max(1, round(proj_travel_ticks × dist / atk_range))`
(`proj_travel_ticks == 0` = melee/instant). Damage is **pre-computed at
fire time** and applied on arrival.

## Skills (`skills.rs`) — see [skills-catalog.md](skills-catalog.md)
SP has ONE meaning: the **aura reservation budget**. The `SkillKind` enum is
`Active` / `Passive`; an "aura" is a skill carrying `sp_reserve > 0`.
- **Active** — auto-cast in place of a basic swing, gated by **cooldown +
  range only**. `sp_cost` is **0** across the board: cooldown is the single
  power dial (short = spammy, long = signature). Power per level =
  `power_bp + power_per_level_bp × (lvl−1)`, applied via `build_damage_scaled`
  (% of ATK/MATK). Auto-cast priority: longest-ready cooldown first
  (signatures preempt) → element score → least-recently-fired (`last_fired`).
- **Aura** (reservation) — a persistent self/party buff that RESERVES a flat
  `sp_reserve` chunk while active, lowering the **usable** SP pool
  (`usable = max − Σ reserved`, allocated in list order until the budget runs
  out). Stacking auras genuinely trades SP for power. The ONLY SP sink
  (Blessing, Arcane Ward, Sharp Focus, Agility Up, …).
- **Passive** — always-on, no SP/cooldown/slot; a build may stack many:
  HP Recovery (regen), Keen Eye (`+ATK`), Mana Spring (`+SP regen`), Double
  Attack, Elemental Mastery (+dmg on element advantage), Steal, Hiding, …
- **Heal** = `effective_matk(caster) × power_bp / 10000` HP to the
  most-injured ally in range (no Damage struct, no mitigation; NOT auto-cast
  as an attack — triggers below an HP threshold).

## Progression (`tick.rs`, `stats.rs`)
- **Base XP to next** `= EXP_BASE × level²` (`EXP_BASE = 250`, **QUADRATIC**
  — Lv1→2 needs 250, Lv9→10 needs 81×250; cumulative to L ≈
  `EXP_BASE·(L−1)L(2L−1)/6`). Calibrated via the sim-cli headless run
  (~Lv70 target). Base level uncapped (for now).
- **Status points / level** (RO Renewal curve, increasing): leaving level
  `L` grants `⌊L/5⌋ + 3` below 100, `⌊L/10⌋ + 13` at 100+ (1→99 totals
  1225). ⚠️ With uncapped level the point budget is unbounded → eventually
  every stat maxes (no build choice / unbalanceable PvP). A level or
  point-budget cap is still **open** — needed if builds/PvP must matter.
- **Job XP to next** `= JOB_EXP_BASE × job_level²` (`JOB_EXP_BASE = 320`,
  **QUADRATIC** — same growth order as base XP, so job level stays a roughly
  constant ~0.74× of base); each job level grants 1 skill point; job cap `50`.
- **Stat raise cost** `= ⌊value/10⌋ + 2` (RO curve); stat cap `99`,
  min `1`.
- **Per-class base stats** (level-1 floor, RO recommended-build
  directions): Swordsman STR8/VIT6/DEX3, Archer DEX9/AGI5/LUK2, Mage
  INT9/DEX5, Thief AGI7/STR4/LUK3/DEX2, Acolyte INT8/DEX4/VIT3. Stored on
  `Stats.base_stats`; free (not "spent"). **Reset** returns to this base
  and refunds only points spent above it.

## Regen (`tick.rs`)
- **SP** every `REGEN_INTERVAL_TICKS = 10` ticks (≈1 s) =
  `SP_REGEN_PER_SEC(3) + Σ passive sp_regen` (Mana Spring), capped to the
  **usable** pool (`max − Σ reserved`; see Skills).
- **HP** (HP Recovery passive) every `HP_REGEN_INTERVAL_TICKS = 50` ticks
  (≈5 s) = `(flat·lvl + MaxHP·bp/10000·lvl) × ratio`, `ratio =
  HP_REGEN_INTERVAL_TICKS / REGEN_INTERVAL_TICKS = 5` — so the AVERAGE HP/s
  equals the old 1 s tick, but heals in fewer, larger (less jittery) bounces.

## Determinism rules (do not break)
- Integer-only; multiply-before-divide.
- Conditional PRNG draws (crit / hit / weapon variance / double-attack /
  on-hit debuff) are **skipped** when their chance is 0 / accuracy ≥ 100% /
  variance 0 — this is what keeps golden hashes pinned. Any new randomness
  must preserve draw order (GDD §4.2 / §7.2).


## Ultimate gauge (`skills/effects.rs`, 0.0.434)
Every ultimate needs **1,000** (`lb_max`); gains: `lb_per_second` (2 for everyone), `lb_on_basic`
(4; Aegis 2), `lb_on_skill` (10-15), `lb_on_break` (Ember Cascade 16). **No gain from damage
taken** any more. `ChargeRateBp` (Surge Crest, Potential) scales every gain at the one fold.

## Monster enrage (`tick/upkeep.rs::tick_enrage`, 0.0.430)
A monster alive on the board for 30 s gains **+20% ATK every 15 s**, uncapped (one self-sourced
`AtkPct` buff grown in place). A fight that drags gets harder; it never stalls quietly.

## Stalemate (`tick/mod.rs::break_stalemate`, 0.0.416 → 0.0.443)
A fight nobody can end fells the party so the host's wipe path (fall-back, revive) runs. Clock:
2 min with a member down (boss HP floor flat, or trash kill counter flat); **5 min at full
strength** (boss floor flat, or the room's total-HP floor flat); only against a live enemy.

## Resonance ranks (`unit_rank.rs`, 0.0.441-442)
Stars 0-6 per Exemplar unit. Every rank is an EFFECT; numbers only as +3 skill levels (S3 the
two skills, S5 the ultimate — `unit_rank::power` reads them above the board cap) or as a "+X% DMG"
add-on bucket on one skill (additive across ranks, applied once). Sized on the party bench to
S6 ≈ +60-65% over S0, S1-S3 ≈ half of it. Webs per unit in `unit_rank::ranks_of`; UI text through
`overlay_ranks` (`RANK_*` loc keys).

## SSR Regalia passives (`regalia_passive.rs`, 0.0.435-439)
Anyone equips any Regalia and takes its lump + advanced stat; the PASSIVE works only on a wearer
of the piece's doctrine (`items::regalia_doctrine`). Triggers are role conditions (taunt held,
shield broke on you, enemies on the field, a break you caused, an ally you healed, a debuffed
target, range ≥ 4, a status you applied, a target under half); `base + per_star × stars`, S5 = 2×
S0. Text through `overlay_passive` (`REGALIA_PASSIVE_*`).

## Combat Score (0.0.450, display only)

`sim_core::combat_score` — the one number a gacha player reads. Not a game input: nothing in
`tick()` uses it; the host exposes it (`party_meta.combat_score`, `stats:<id>.combat_score`)
and sim-cli `--bench` prints `CP`. It measures INVESTMENT, not damage: a support and a Striker
at the same rarity / level / stars / gear read the same number; the DPS meter says who hits.

```
unit     = base[rarity] + per_level[rarity] × (level − 1)        SR 650+65/lv · SSR 1000+100/lv
gear     = Σ tier × 350 × grade_mult + enhance × 40 + lines × 120 + gems × 200
regalia  = base[R/SR/SSR] + per_level + stars × 10% of it
shared   = (training_levels × 40 + crest_levels × 60) / 5
member   = (unit + gear + regalia + shared) × (1 + stars × star_bp[rarity]) / 30
             star_bp: SR 7.5%/star (S6 +45%) · SSR 16.67%/star (S6 +100%)
party    = Σ fielded members
```

Rule it encodes (memory `sr-vs-ssr-rule`): SR S0 = 65% of SSR S0, SR S6 ≈ 95% of SSR S0,
SSR S6 = 2× S0. Calibration on the bench: reference party (Lv70 / T6 Legendary / SSR Regalia
Lv60 / S0) ≈ **10K**; S6 ×2; no Regalia 8.4K; R Regalia 8.8K; T4/Rare 7.3K; Lv50/T4 6.4K;
Lv30/T2 2.2K; Lv10/T1 1.3K. F2P seed 1: d1 2.1K · d2 3.7K · d3-d7 4.1-4.7K (flat with the stage wall at 96). Bench dmg/h moves further than the score on the same steps (gear
compounds in a fight) — the score is a progress ladder, not a damage predictor.
