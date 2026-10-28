# Monster Skills — implementation spec

Status: **DESIGN, ready to implement.** Blocked only by a clean tree (another session has
uncommitted sim-core work at 0.0.231 — let it land first). Principle: **reuse existing FX,
zero new primitives** (audited — see [[reuse-existing-sim-fx]] / `skills/mod.rs` SkillDef).
Values below are placeholders — tune in sim-cli.

## What already EXISTS — do NOT rebuild or modify

- **Monster on-death skills** (`skills/catalog/monster.rs`): `self_destruct` (3×3 blast+knock),
  `burst_blast` (5×5), `mending_burst` (heal ally monsters 5×5), `toxic_burst` (3×3 +poison).
  Assigned to chomb/chomber/chomboom/rechum/toxic_chom/burst_chom in game_data.json. **Leave them.**
- **Cast machinery** (`tick/combat_step/mod.rs::pick_active_skill`, `cast.rs`, `entities[i].active_skills`)
  is per-entity/generic — NOT player-gated. So a monster with populated `active_skills` should flow
  through the same cast pipeline. **VERIFY** this actually runs for Team::Monster in the tick loop;
  if the loop only invokes casting for players, add the (small) enable for monsters.
- **FX / SkillDef fields to reuse:** `ground_field_radius`/`field_hits`/`field_duration_ticks`
  (+ Quicksand debuff-field), `aoe_radius`/`self_aoe_radius`/`knockback`, `cast_buff[]`/`cast_buff_party`,
  `on_hit_debuff` (BuffKind incl. **Bleed**, Poison, PoisonDot, Stun, Freeze, Blind, DefDownPct,
  AgiSlowAdd, AtkPct, DefPct…), `cast_element`, `teleport`, `target_lowest_hp`/`target_furthest`,
  `heals_ally`, **`charges_left`**, entity **`untargetable`** flag, boss-minion spawn (`spawn.rs`),
  traps (`combat_step/trap.rs`).

## Genuinely NEW work (small — NOT FX)

1. **Enemy active-cast enable** — confirm/wire monsters through `pick_active_skill` + cast in the
   tick loop (machinery is generic; may already work — verify first).
2. **id-hash desync** — at spawn, init a skill's cooldown to `(entity_id * 2654435761) % cd_max`
   so a pack casts out of sync. **NO PRNG draw** (uses entity id) — keeps the loot RNG stream
   byte-identical; only the skill-cast logic shifts goldens.
3. **charges-from-id (weighted)** — at spawn set `charges_left` from `h = (id*2654435761) % 100`:
   `h<50→0, h<80→1, else→2` (50/30/20). Field exists; just the spawn-init. No PRNG draw.
4. **Twinling vanish→appear sequence** — AI: set `untargetable` (vanish) for N ticks, then
   `teleport` adjacent to a party member and attack. Rides existing `untargetable` + `teleport`.

Everything else = authoring **new SkillDef consts** (reusing fields) + assigning `skills: [...]`
on monster defs in `game_data.json`. Golden hashes shift → VERSION bump + re-bless.

## Scope / sequencing

- **Slice 1 (proves the pipeline):** `zombie` + `gnarloak` only. Small, low balance risk.
- **Slice 2:** Duskmourn gothic cast (10) — mid-tier (bands 5-10, Lv46-50), stats still placeholder
  (hp=1) → author skills now, tune power/cd when stats land.
- **Untouched:** early mobs (slimes, chom) stay basic / on-death; band bosses + king_yellowling
  stay stat-stick (no skills yet, by decision).

## Skill specs

### Slice 1
| monster | new skill id | SkillDef (reused fields) | notes |
|---|---|---|---|
| **zombie** | `zombie_poison` | power ~150%, `on_hit_debuff: Poison`, `cooldown ~300t (30s)`, `charges_left` init 0/1/2 from id | id-hash desync on cd; 50/30/20 charges |
| **gnarloak** | `umbral_roots` | `ground_field_radius:1` (3×3), `cast_element: Shadow`, `field_hits ~10` @25%, `field_duration_ticks ~50`, + slow debuff (Quicksand-style) | active-cast ground field |

### Slice 2 — Duskmourn cast (all reuse; power/cd = placeholder)
| monster (id) | skill | SkillDef (reused fields) |
|---|---|---|
| Twinling (mini_ghost) | Blink Ambush | `untargetable` (vanish N ticks) → `teleport` adjacent → hit |
| Gloomtooth (ghost_rabbit) | Shadow Nibble | melee + `on_hit_debuff` (small curse: DefDownPct/AgiSlowAdd) |
| Banshee (banshee) | Wail | `aoe_radius` + `on_hit_debuff` (−ATK/−ASPD: AtkPct-down / Blind) |
| Dustelle (maid) | Mend | `heals_ally` (heal ally monsters) |
| Ashelle (maid_horn) | Dark Rite | `cast_buff_party` (StrAdd/AtkPct to ally monsters) |
| Marionette (marionette) | Puppet Lunge | `teleport` backstab burst (= Shadow Strike mechanic; high power) |
| Scarlette (red_hood) | Quick Step | `cast_buff` self (AgiPct/AspdPct) |
| Crimsonne (red_hood_crimson) | Blood Slash | `cast_element: Shadow` + `on_hit_debuff: Bleed` |
| Brutus (werewolf) | Transform + Rend | `cast_buff` self (DefPct/AtkPct — the "grow") + `on_hit_debuff: Bleed` |
| Broomhilda (witch, elite) | Hex · Cauldron · Familiars | `cast_element`+`on_hit_debuff` · `ground_field` · boss-minion summon |

## Determinism

- All new randomness = **id-hash** (desync, charges) — no PRNG draws → loot/refine RNG stream
  byte-identical; goldens shift only from new skill-cast logic. Re-bless intentionally.
- Bleed/Poison land on the 1s regen interval (existing DoT path). Ground fields tick likewise.
- VERSION bump per the changelog convention in `lib.rs`; note which goldens moved and why.

## Verify-first checklist (before coding)
1. Does the tick loop run `pick_active_skill` + cast for Team::Monster? (enable if not)
2. Can a ground_field carry BOTH damage AND a slow debuff, or is that two fields? (Vermilion=dmg,
   Quicksand=slow-only) — combine for `umbral_roots`.
3. `charges_left` spawn-init hook — where monster entities are built (`spawn.rs` / `data/build`).
