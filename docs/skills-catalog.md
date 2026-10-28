# Skills Catalog (living doc)

> **SUPERSEDED — the element triangle below is the CLASS era.** The live rule is six elements
> (Physical, Fire, Thunder, Water, Light, Dark) and per-monster weakness LISTS, no triangle:
> `docs/element-weakness-system.md`. Earth / Wind / Poison / Spirit are cancelled (2026-09-23).


> **2026-09-19 — read this first.** The class system is retired; the five UNIT kits that ship
> (Seraphine / Lapis / Lucielle / Aveline / Raplus) are catalogued in `docs/exemplar-roster.md` §0
> with their Resonance ladders (`crates/sim-core/src/unit_rank.rs`) and SSR Regalia passives
> (`crates/sim-core/src/regalia_passive.rs`); live numbers are in
> `crates/sim-core/src/skills/catalog/{seraphine,rapis,exemplar}.rs`. There is ONE attack stat
> (MATK is gone — "% of MATK" below means % of ATK), monsters have WEAKNESSES not an element,
> and the element triangle below no longer applies (`docs/element-weakness-system.md`). The
> class tables below describe the `*_t1` trees that are still in `skills/tree.rs` as remnants
> (Lapis still borrows `fireball` / `flame_shield` from the mage set); the engine-mechanics
> and how-to-add sections are still current.

Quick reference for every skill currently in the game, matching the code.
**Source of truth = `crates/sim-core/src/skills/catalog.rs`** (the hardcoded
`SkillDef` table + `skill_display` names) and `crates/sim-core/src/skills/tree.rs`
(per-class gates). Update this file whenever those change.

Design stance (see `taskbar-arpg-skills-jobs-design.md`): an idle auto-battler.
Active skills **auto-cast by priority** in place of a basic attack; passives are
always-on; buffs are **auto-cast** (no manual timing) — either active casts on a
cooldown cycle or always-on auras. Skills are **learned** (not innate): you spend
skill points (1 per Job Level) to learn (Lv 0→1) and raise nodes, gated by Job
Level + any prerequisite (see the Skill Tree window, hotkey **K**).

## How the numbers work

- **Tick = 100 ms** → 10 ticks/sec. Cooldowns/durations show ticks (+ seconds).
- **power** = damage as % of effective ATK (physical) or MATK (magic) at Lv 1;
  `+/lvl` adds per skill level. Max level 10 for all (Taunt 7).
- An active cast needs all of: **off cooldown** + **enough SP** + a **valid
  target in range** — else it falls back to the basic attack.
- **Auto-cast priority** = skill-list order, EXCEPT the mage's elemental bolts,
  which are picked by **element matchup** vs the current target (see Mage).
- Pure **buff casts** (no damage) auto-fire on their cooldown even with no enemy
  present (like Heal), refreshing the buff (cooldown ≈ duration → ~100% uptime).
- Regen (HP/SP) ticks once per second (every 10 ticks). Baseline SP regen = 3/s.
- `DefPct` buffs raise **hard DEF** (diminishing-returns mitigation), so their
  real value scales with the wearer's base DEF.
- Element triangle: Fire > Earth > Water > Fire (1.5× / 0.5×); Neutral = 100%.

---

## Swordsman (`swordsman_t1`) — melee, Physical

| Skill | Type | Job | Effect | Power | SP | Cooldown |
|---|---|---|---|---|---|---|
| **Bash** | Active | 1 | Single-target nuke (range 1) | 180% +20%/lvl | 12 | 12t (1.2s) |
| **HP Recovery** | Passive | 1 | Heal/sec = `5·lvl + MaxHP·0.2%·lvl` | — | — | — |
| **Endure** | Passive | 3 | Soft DEF = `ceil(0.5·lvl) × (enemies within 1 cell)`, cap 100 | — | — | — |
| **Sword Mastery** | Passive | 5 | +4 ATK/lvl — only with a 1H sword or dagger | — | — | — |
| **2H Sword Mastery** | Passive | 5* | +4 ATK/lvl — only with a 2H sword | — | — | — |
| **Ignite Break** | Active | 7 | **Point-blank Fire AoE** (3×3 around caster) + **knockback 2**; sets weapon element to Fire for **10s** (cd 5s → near-permanent Fire weapon once rolling) | 150% +15%/lvl | 18 | 50t (5s) |
| **Taunt** | Passive (auto) | 10 | Free: locks every enemy in a 5×5 onto the caster for a **fixed 4s**; leveling cuts the **cooldown** so the lock gaps less (Lv1 ~6s gap → Lv7 continuous) | — | 0 | 100t→40t (−10t/lvl, Lv1–7) |

*2H Sword Mastery requires **Sword Mastery Lv 1** first.

## Mage (`mage_t1`) — ranged, Magic

| Skill | Type | Job | Effect | Power | SP | Cooldown |
|---|---|---|---|---|---|---|
| **Fireball** | Active | 1 | Magic **AoE** bolt, 3×3 on impact (range 6) | 200% +20%/lvl | 20 | 18t (1.8s) |
| **Mana Spring** | Passive | 1 | +2 SP/sec per level (on top of baseline) | — | — | — |
| **Fire Bolt** | Active | 1 | Single nuke, **Fire** (strong vs Earth), range 6 | 180% +20%/lvl | 14 | 8t (0.8s) |
| **Lightning Bolt** | Active | 3 | Single nuke, **Earth** (strong vs Water), range 6 | 180% +20%/lvl | 14 | 8t (0.8s) |
| **Frost Bolt** | Active | 5 | Single nuke, **Water** (strong vs Fire) + **slows** the target, range 6 | 180% +20%/lvl | 14 | 8t (0.8s) |
| **Arcane Ward** | Active (self-buff) | 7 | **Mana shield**: cut all incoming damage by **2.5%/lvl** (Lv10 = 25%) for 12s; each hit drains **2% max SP**, ward collapses when SP runs out | — (buff) | 12 | 100t (10s) |

**3-bolt auto-pick:** when more than one bolt is ready, the mage casts the one
whose element gives the **1.5× advantage** vs the current target (Fire→Earth
foes, Lightning/Earth→Water foes, Frost/Water→Fire foes). No advantage anywhere
(Neutral targets) → list order, so **Fireball** (AoE) wins the tie. The three
bolts are **separate nodes, leveled independently**.

## Archer (`archer_t1`) — ranged, Physical

Base attack range is **3** (lowered from 5); Hawk Sight extends it back up to 8.

| Skill | Type | Job | Effect | Power | SP | Cooldown |
|---|---|---|---|---|---|---|
| **Twin Strafe** | Active | 1 | Heavy single-target bolt (range 5) | 220% +20%/lvl | 14 | 14t (1.4s) |
| **Keen Eye** | Passive | 1 | **+1 DEX/lvl** (RO Owl's Eye) — boosts ranged ATK + HIT + ASPD together | — | — | — |
| **Hawk Sight** | Passive | 3 | **+1 HIT/lvl** and **+1 attack range per 2 lvls** (Lv10 = +5 range) | — | — | — |
| **Arrow Shower** | Active | 5 | Ranged **AoE**, 3×3 on impact (range 5) | 160% +15%/lvl | 16 | 50t (5s) |
| **Sharp Focus** | Active (self-buff) | 7* | Self **AGI & DEX +(lvl+2)% of base** (Lv1 +3% → Lv10 +12%) for 15s; faster ASPD, more HIT/FLEE/ATK; no damage | — (buff) | 12 | 120t (12s) |

*Sharp Focus requires **Hawk Sight Lv 1** first (RO: Improve Concentration needs
Vulture's Eye). Its low SP cost (12) + being cast **before** the attack each
tick keep it from being starved by Twin Strafe's SP use.

## Thief (`thief_t1`) — melee, Physical

| Skill | Type | Job | Effect | Power | SP | Cooldown |
|---|---|---|---|---|---|---|
| **Shadow Strike** | Active | 1 | **Teleports behind the lowest-HP enemy** in reach (guaranteed backstab burst), range 4 | 240% +20%/lvl | 16 | 40t (4s) |
| **Double Attack** | Passive | 1 | Basic swings have a 5%/lvl chance (50% max) to strike twice | — | — | — |
| **Venom Edge** | Active | 3 | One light hit + a strong **poison** DoT; only on a **non-poisoned** target, range 1 | 100% +10%/lvl hit; poison **300% +30%/lvl over 7s** | 12 | 14t (1.4s) |
| **Nimble Mastery** | Passive | 5 | +3 FLEE/lvl (evasion) | — | — | — |

## Acolyte (`acolyte_t1`) — support, Magic

| Skill | Type | Job | Effect | Power | SP | Cooldown |
|---|---|---|---|---|---|---|
| **Heal** | Active (support) | 1 | Heals the most-injured ally in range for % of MATK (range 6); no Damage packet | 300% +30%/lvl | 18 | 16t (1.6s) |
| **Blessing** | Active (party buff) | 1 | Party **+STR/+INT/+DEX** for 32s | — (buff) | 25 | 300t (30s) |
| **Agility Up** | Active (party buff) | 3 | Party **+AGI** (lvl+2) **and +25% move speed** for 30s (faster ASPD + FLEE + movement) | — (buff) | 20 | 280t (28s) |
| **Guardian Aura** | Passive (aura) | 5 | Always-on party **+DEF% + VIT** while learned | — (aura) | — | — |
| **Holy Light** | Active | 7 | Single-target magic nuke (range 6); kept below the mage's bolts (support, not a rival nuker) | 130% +15%/lvl | 16 | 12t (1.2s) |

When no ally needs healing / no buff is due, the acolyte fights with Holy Light
or a basic magic bolt.

---

## Buff / debuff magnitudes (per level)

All buffs are transient (`Entity.buffs`), aged each tick by `tick::decay_buffs`.
Stat-add buffs flow through the single `buffed_stats` fold point, so they feed
every derived stat (ATK, MATK, DEF, HIT, FLEE, ASPD).

| Source | Kind | Magnitude (Lv 1 → +/lvl) | Duration | Target |
|---|---|---|---|---|
| **Blessing** | STR + INT + DEX | +1/lvl each (Lv1 +1 … Lv10 +10) | 32s | whole party |
| **Agility Up** | AGI + move speed | +3 AGI +1/lvl (Lv10 = +12) **and +25% move speed** | 30s | whole party |
| **Sharp Focus** | AGI + DEX **%** | +(lvl+2)% of base each (Lv10 = +12%) | 15s | self |
| **Arcane Ward** | dmg reduction % | +2.5%, +2.5%/lvl (Lv10 = 25%) | 12s (drains 2% SP/hit) | self |
| **Guardian Aura** | hard DEF % + VIT | +10% / +2, then +3% / +1 per lvl | aura (always-on) | whole party |
| **Frost Bolt** | AGI slow | −10, −2/lvl | 3s | target (always) |
| **Venom Edge** | poison DoT | 300% ATK +30%/lvl total, split over the ticks | 7s (~7 ticks) | target (only if not already poisoned) |
| **Ignite Break** | weapon element → Fire | — | 10s (cd 5s) | self (drives the blast) |

Notes
- Party buffs (Blessing, Agility Up) re-cast on their cooldown → near-100%
  uptime. Guardian Aura is re-stamped every tick (`apply_party_auras`) while the
  acolyte is alive.
- The two debuffs (Frost Bolt slow, Venom poison) always apply on a hit
  (`chance_bp = 0` → no extra RNG draw, so determinism/goldens are unaffected).
- DoT damage lands on the 1-second regen interval and routes lethal ticks
  through the normal death/drop path.

## Engine mechanics (where each effect lives)

- **AoE** — `aoe_radius` (target-centred, projectile path: Fireball, Arrow
  Shower, the bolts are single) vs `self_aoe_radius` (caster-centred, melee:
  Ignite Break). `knockback` pushes self-AoE survivors away.
- **Buffs/debuffs** — `cast_buff: &[BuffSpec]` (+ `cast_buff_party`) and
  `on_hit_debuff: Option<DebuffSpec>` on `SkillDef`; applied in
  `tick::combat_step` (self-buff / on-hit) or `try_cast_buffs` (party/self pure
  buff) or `apply_party_auras` (passive auras). Folded in `combat::derived`.
- **Element override** — `cast_element` (mage bolts) sets the damage packet's
  element and drives the element-aware auto-cast in `pick_active_skill`. Ignite
  Break instead uses a self `AtkElement` buff via `build_damage`.
- **Passive folds** — `atk_flat_per_level` (Sword/2H masteries),
  `hit_flat_per_level` (Hawk Sight), `flee_flat_per_level` (Nimble Mastery),
  plus the id-keyed `passive_dex_bonus` (Keen Eye → `buffed_stats`) and
  `passive_range_bonus` (Hawk Sight → `effective_atk_range`),
  `sp_regen_flat` (Mana Spring), `double_atk_chance_bp` (Double Attack),
  `hp_regen_*` (HP Recovery). Taunt/Endure run in `tick::apply_taunt`/`_endure`.
- **Targeting** — `teleport` + `target_lowest_hp` (Shadow Strike);
  `heals_ally` (Heal, via `try_cast_support`).

## How to add a skill (checklist)
1. Add a `SkillDef` const in `skills/catalog.rs` + register it in `skill_def()`,
   and add a display name + blurb in `skill_display()`.
2. Add a tree node (class + Job-Level gate + optional prereq) in `skills/tree.rs`.
3. If it needs a new mechanic, wire the `SkillDef` field through the engine
   (`combat/derived.rs`, `tick/combat_step.rs`, `tick/upkeep.rs`).
4. Add unit tests; run `cargo test -p sim-core` (goldens stay byte-identical
   unless a golden scenario actually learns the skill).
5. The Skill Tree UI is data-driven from `skill_tree_json` (name/blurb/kind), so
   no client change is needed. Rebuild the dylib (`tools/smoke.sh`).
6. Update this doc.
