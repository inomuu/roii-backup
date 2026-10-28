# Trigger Vocabulary — the design primitives for kits / Regalia / cards

**Status:** reference v1, 2026-07-28. The list of AUTOMATIC combat events an effect (a character
skill, a Regalia passive, a card special) can hook into. Our game is an **idle auto-battler** — no
manual dash / dodge / charge / ultimate-press like ZZZ / Genshin / HSR — so every trigger is an
event the sim already produces on its own. Design effects by picking a trigger + a payoff.

**How to read the "Hook" column:** ✅ = a proc-point already exists in the sim (cheap to attach a
new effect); ➕ = a small new hook/primitive to build; 🔮 = future mechanic (not built, don't design
against it yet). Grep anchors are in `crates/sim-core/src/tick/combat_step/` unless noted.

## A. Attack triggers  (the on-hit family — richest, all hooked)

| Trigger | Fires when | Hook | Gacha analog |
|---|---|---|---|
| **on-hit** | a basic attack deals damage | ✅ `try_card_procs` (mod.rs ~1313) | Normal Attack |
| **on-crit** | that hit was a crit | ✅ `is_crit` flag on the packet | CRIT hit |
| **by element** | the hit's element = Holy/Fire/Water/… | ✅ `atk_element` / `cast_element` | Elemental DMG |
| **by damage-type** | Physical / Magic / True | ✅ `damage_type` | phys vs anomaly |
| **by delivery** | melee / ranged / projectile | ✅ `is_ranged`, basic profile | — |
| **multi-hit / bolt** | Nth bolt / display-hit | ✅ `display_hits`, bolt count | multi-hit |

## B. Skill / cast triggers  (the "special/ultimate" analog — skills auto-fire on cooldown)

| Trigger | Fires when | Hook |
|---|---|---|
| **on-cast** | any skill resolves | ✅ `skill_fx` push + `try_cast_*` (cast.rs) |
| **on-cast-buff** | a party-buff cast lands | ✅ `resolve_buff_cast` |
| **on-ground-field** | a ground field is placed | ✅ `try_cast_ground_field` |
| **on-summon** | a familiar is summoned | ✅ `try_cast_summon` |

## C. Kill / death triggers  (snowball engine)

| Trigger | Fires when | Hook |
|---|---|---|
| **on-kill** | an enemy dies | ✅ `.alive = false` sites; death hooks exist |
| **on-death-with-debuff** | a debuffed enemy dies (→ blast) | ✅ `maybe_schedule_lumen`, `maybe_poison_explosion` |

## D. Debuff / DoT triggers  =  OUR "ANOMALY" analog  (the deepest design space)

Our DoT/debuff system is the equivalent of Genshin reactions / HSR-ZZZ anomaly — the richest
place to build layered kits (spreaders, detonators, amplifiers).

| Trigger | Fires when | Hook |
|---|---|---|
| **on-debuff-applied** | poison/bleed/burn/freeze/stun/vuln lands | ✅ `apply_on_hit_debuff` (mod.rs 461) |
| **DoT tick** | a DoT ticks its interval | ✅ poison/bleed/burn tick |
| **mark / vuln** | target carries a mark (e.g. `LumenVuln`) | ✅ predicate exists (Lumen) |

## E. Defensive / state triggers

| Trigger | Fires when | Hook |
|---|---|---|
| **on-hurt** | the wearer takes a hit (→ reflect/counter) | ✅ `apply_card_reflect` (mod.rs 1439) |
| **HP-threshold** | HP crosses below X% | ✅ `ConsumeTrigger::HpBelowPct` (items.rs 346) |
| **ally-down** | a party member is downed | ✅ `ConsumeTrigger::AllyDown` |

## F. Passive / periodic

| Trigger | Fires when | Hook |
|---|---|---|
| **periodic** | every tick / regen interval | ✅ per-tick aura/regen |
| **stacks** | a buff stacks on repeated action | ✅ `CardAspdStack` (BuffKind) |
| **while-in-field** | an enemy stands in a ground field | ✅ Sanctus Terra / field ticks |

## ➕ To build (small, high value)

- **Follow-up attack** (追加攻撃, HSR-style) — an EXTRA auto-attack fired BY another trigger
  (on-crit / on-Nth-hit / when an ally attacks). It is itself a tagged attack that OTHER effects
  can trigger ON → a **trigger-of-trigger = combo engine**. Best idle fit: auto-chains are
  watchable (the "juice" with no input) and enable cross-character synergy (A's follow-up feeds
  B's follow-up trigger). Reuses the attack pipeline (light vs a whole new system).
- **Counter / retaliation** — extend `apply_card_reflect` from flat reflect into a real
  counter-ATTACK that itself procs on-hit.
- **Ramp / time-in-combat** & **kill-streak / momentum** — idle-native: reward long farms + high
  kill counts with stacking buffs (a `CardAspdStack`-style stack on a new trigger).
- **Wave / room-clear** — an idle-progression event → a bonus (heal/buff/loot) on clearing a room.

## 🔮 Future mechanics (NOT built — do NOT design against yet)

- **Break / Stagger gauge** (HSR Break / ZZZ Daze) — a toughness gauge → break → burst window.
  Adds a build→break→burst rhythm + team-loop that reads great in auto-combat, but needs a whole
  new toughness state + break mechanic. Deferred (user 2026-07-28: "น่าสนใจแต่ยังไม่มี mechanic").
- **Mark-detonation as a general system** — Lumen already marks; generalizing marks + detonators
  into a shared primitive is future.
- **Energy / ultimate charge** — we are cooldown-based, not energy-based; an ult-charge resource is
  a future option.

## Archetype menu (trigger → identity)

on-hit stacker · crit-burst · element-caster · **anomaly (DoT) spreader / detonator / amplifier** ·
on-kill snowball · support-on-cast (team buff) · bruiser-on-hurt (counter) · **follow-up combo** ·
ramp/kill-streak farmer.
