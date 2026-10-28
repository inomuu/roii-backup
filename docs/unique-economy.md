# Unique Item Economy (design — build at second launch)

Status: **DESIGN ONLY.** Build alongside server-roll-drops (full gear) + Steam Market +
steam-mint-quota. Calibrate against a LIVE playerbase — the price/quota numbers here are
starting points, not final. Depends on: `server-roll-drops-design.md`,
`crowd-scaled-rare-drops.md`, `steam-mint-quota-design.md`, `economy-invariants.md`.

## Core principle

Supply of a unique is a **controlled output**, never a side-effect of a fixed drop rate.
An idle game kills at huge volume, so any flat per-kill rate floods at scale. Uniques are
**server-rolled + crowd-scaled + supply-capped**, reset every ~3 months (set rotation).

Empirical anchor (TBH, Steam app 3678970): market **price ≈ 1 / listed-supply**; rarity
label is noise (same grade varies 55×). Value-holding rares sit at **<100 listed even with
100k+ players** (<0.5% ownership). Radiant Shield Divine $222 → ฿7.97 once supply unlocked —
the $200 era was artificial-scarcity (locked-from-sale), not real value.

## What is tradeable, and where

- **Base UNIQUE (special ability, fixed identity, craftable-further) = real money on Steam.**
  Steam Market groups by `market_hash_name` (fungible-by-name, discrete buckets only). So a
  unique on Steam = its ability name + at most a few discrete quality buckets (e.g. grade ×
  durability tier, CS:GO-style ~5–15 names). Continuous per-instance value earns ZERO Steam
  premium (that's why CSFloat/Buff exist off-Steam).
- **Crafted / substat god-roll = bound / Gil in-game.** Once orbs roll substats onto the
  unique it becomes many-axis → cannot bucket on Steam → in-game Gil market, which searches
  all axes natively. God-rolls never hit Steam → cannot flood it. See
  [[base-item-real-money-core]].
- Non-unique rolled gear (many substats) = Gil-only in-game (per `poe-gear-direction`).

## Per-instance quality → a price LADDER (not one price)

Each dropped unique rolls **base_dmg float (immutable)** + **max-durability**. This turns one
fungible unique into a CS:GO-style continuous quality market.

- **The special ability is a shared FLOOR** — every copy is worth ≥ ability value, so no copy
  drops to ฿1 (unlike a plain base, which floors when common). float/durability add a premium
  tail on top.
- Price model: `price = floor × (1 + k · Qᵃ)`, Q = quality 0..1 (≈0.7·float + 0.3·dura).
  - **k** = ceiling: god-roll = floor × (1+k). Realistic **k ≈ 2–3** (float ±15–20% power →
    ~2–3× swing, like CS:GO's typical 2–5×). Higher k if float matters more / PvP demand.
  - **a** = convexity / accessibility knob: high a → most copies cluster near the affordable
    floor, god-rolls are the thin expensive tail. **a ≈ 2** recommended.
- Result: one unique = a market from floor-roll (accessible entry) to god-roll (whale target).
  This SOLVES the thin ultra-rare market — buyers who just want *a* copy pay the floor.
- **Bucketing for Steam:** base_dmg tier (5) × max-durability tier (3) ≈ 15 names/base. Only
  MAX durability matters for resale (current durability repairs to full). Fine-grain float
  premium surfaces via an in-game "float-finder" overlay, never Steam.

## Durability = decay curve + Gil sink + check-in hook

- Durability is a **rolled max** governing a **damage-output decay curve** over farm time
  (e.g. low-dura: −20% @4h, −50% @8h; high-dura: 100% for 8h++). NOT permanent consumption.
- **Repair = pay Gil only** (fully restores). So gear is permanent; durability's jobs are:
  (1) **auto-scaling Gil sink** (scales with farm time → counters idle faucet, per
  `monetary-policy`), (2) minor market quality axis (uptime convenience).
- **Deliberate check-in hook** — the user WANTS players to return periodically, not idle for
  days. So NO default auto-repair; repair on return = one-click. Tune decay to target
  check-in cadence; keep the penalty at the *expected* cadence mild (avoid a leash). Still
  compatible with the idle-CCU flywheel (client stays open, player tabs back).
- Auto-repair / slower-decay / higher-durability = candidate PREMIUM convenience (Pass perk,
  "paying saves time"). Anti-pattern to avoid: don't make the base loop feel bad to sell the
  fix.
- base_dmg = float ceiling (immutable); effective dmg = base_dmg × durability_mult.

## Break-repair sink (proposed, on top of refine)

Optional stronger sink: refining (+bonus) can BREAK; a break **consumes 1 more copy** to
repair. Monte-Carlo findings:
- **MUST be spare-safe** — a break may never destroy a player's last/only copy. If it can
  ("risk-main"), ownership collapses (13% own at break 0.3) — catastrophic for a wanted item.
- A spare-safe break sink lets a **~5× more generous drop hold the same price** (buys fun
  without flooding).
- **Dead at ultra-rarity** — if everyone owns exactly 1, spare-safe means nobody risks
  refining → the break sink (and unique refine) never fires. So: chase-tier uniques get a
  SAFE upgrade path; break-refine belongs to commoner uniques/bases where players hold spares.
- Even where break-refine is dead, durability variance keeps a Gil-sink alive (low-dura floor
  copies wear faster → repair more).

## Rate / rarity tiers (per-capita, anchored to TBH listed/player)

`r` = mints per demand-player per day. Demand base = **active players in the tier that drops
it**, and for class-locked uniques = **players of that class** (10k end-game / 5 classes ≈ 2k).

| tier            | listed/player | r (per demand-player/day) | ~%own | price band |
|-----------------|---------------|---------------------------|-------|------------|
| accessible      | ~0.01+        | ~0.01                     | ~45%  | ฿25–60     |
| prestige        | ~0.002        | ~0.001                    | ~10%  | ฿120–250   |
| TBH-rare (chase)| ~0.0005       | ~0.00005                  | <0.5% | ฿250–500+  |

Split structure: any-unique = 5 classes × 2 = 10 types; specific = any / 10. Mix tiers across
the 10 (a couple ultra-rare chase w/ safe-upgrade; the rest prestige/accessible w/ sink).

**Playerbase–liquidity tension:** ultra-rare (<0.5% own) + a liquid market (~50–100 listed)
only coexist at LARGE N. At small launch N you pick thin-but-prestigious OR liquid-but-more-
common. Items grow MORE exclusive as the game grows (good).

## Seasonal price curve (~90-day set)

drop 0.01/player/day + spare-safe break 0.3 (illustrative): opens prestige (thin supply) →
plateaus reward → next set resets. Early adopters / lucky drops get high value; late buyers
get the accessible plateau; reset before it fully floors → no "junk" phase. Absolute ฿ is a
weak guess (needs live demand); the CURVE SHAPE and dynamics are the robust output.

## Quota system (the control loop)

Set the **setpoint = target LISTED count** (not %own), because listed→price is the
TBH-anchored relation and stays meaningful as N grows.

```
quota_today(u)  = r_u × avg_legit_active_in_tier(7d)         # feed-forward, per-capita
r_u calibrated  from observed season price (err SCARCE first — un-flooding is impossible)
grant           = crowd-scaled server-roll: p_u = quota / expected_candidates  (p ∝ 1/N)
                  spread across the day (hourly buckets) — no timezone quota-sniping
correction      = server knows exact circulating N_u; nudge r_u by (target − actual) slowly
points cap      = global daily mint-point ceiling × A (steam-mint-quota) as blow-out guard
season reset    = new set → controllers reset; old set mint→0 (freeze) → soft price decay
```

- **"yesterday's active count → today's quota" WORKS** and is simple; 3-month reset bounds
  drift so no heavy continuous controller is required. Confirmed by sim to hold the price band
  through population swings + event spikes, where a fixed quota wobbles.
- **Count LEGIT active only** (replay-verified, session-valid, anti-taint) — bots inflating A
  inflate the quota. Bots are otherwise starved: capped supply → each bot gets ~0.
- Demand can't be estimated a priori (not everyone is a try-hard) → the MARKET reveals it;
  start scarce, loosen from observed price season-over-season.

## Build / determinism notes (when implemented)

- base_dmg float + durability = per-instance fields on the item instance → MUST be in the
  serialized state + state hash + replay, and MUST survive the load path (see
  [[migrate-item-must-not-rebuild]]). Adding them shifts golden hashes → VERSION bump +
  re-bless.
- Money-value rolls (which unique, float, durability) = server-rolled (server-roll pipeline),
  not client, to prevent forgery. High-volume common bases can stay client-sim + replay-
  verified (like star roll today).
- No paid random pulls, ever — free drop + known-price purchase = the legal shield (loot-box
  / gambling laws target paid randomization). Disclose philosophy, not dynamic numbers.

## Unique item ABILITY design (Pharaoh Set v1 — launch)

Design language: **additive & easy to read, NO trade-offs** (variety comes from WHICH
unique you center, not from downsides). Two reward shapes:
- **Linear** — skill power **+X% per TOTAL refine** across all worn items; this item's OWN
  refine cuts the skill's **CD/cast**. (dual-hook: own-refine = speed, total-refine = power)
- **Breakpoint** — a **mechanic unlock at a refine threshold** (RO-style +7/+9), e.g. more
  targets / bigger AoE / a fire zone / an on-cast buff.

All 10 anchor **class-1 skills**. **Awaken to 2nd-class skills = post-launch** (naturally
gated by 2nd-class shipping; dupe-fusion 5× → awakened version, reuses Resonance
consume-dupes tech; the 5→1 fusion is also a self-consuming supply sink — see
[[base-item-real-money-core]] economy). Effect **rides item id** through ONE shared
`skill-mod table` hook (per [[skills-decoupled-from-anim]]).

5 classes × 2 = one season's set. Set drops from the Pyramid dungeon (boss Queen
Nefari / "Cleopatra"). Cleopatra's Diadem (her crown) = the natural chase piece.

| class | unique (slot) | shape | effect |
|---|---|---|---|
| Swordsman | **Khopesh of Ember** (1H sword) | Linear | Bash 150% **+2%/total-refine**; own-refine **−2% Bash CD** |
| Swordsman | **Dragonbone Greatsword** (2H) | Breakpoint | Ignite Break **200% flat, Neutral (no element convert)**; **@+9 → 5×5 AoE** |
| Mage | **Ankh of Cinders** (staff) | Linear+ | Fireball 200% **+2%/total-refine**; own-refine **−2% CD**; **Fireball leaves a fire zone (ground DoT)** |
| Mage | **Sceptre of the Storm** (rod) | dual-Linear | bolt **cast −20%, −3%/own-refine** (bolts have cast time); bolt **dmg +1%/total-refine** |
| Archer | **Windserpent Bow** (bow) | Breakpoint | Twin Strafe **+20% dmg, hits 2 targets**; **@+7 → 3, @+9 → 4** |
| Archer | **Sandstorm Quiver** (bow) | conditional | Arrow Shower **×3 vs a SINGLE target** (160×3 = **480%**); **+2%/total-refine added BEFORE the ×3** (refine tripled when isolated) |
| Thief | **Fang of the Jackal** (dagger) | Linear | Shadow Strike **+2%/total-refine**; own-refine **−2% CD**; **@+9 → +10% ASPD for 2s after Shadow Strike** |
| Thief | **Apep's Sting** (dagger) | Linear | Venom Edge poison **×2, +3%/total-refine** (higher rate — poison is weak baseline) |
| Acolyte | **Staff of the Nile** (rod) | Linear+ | Heal **+2%/total-refine**; own-refine **−2% CD**; **@+9 → heals 2 targets** |
| Acolyte | **Cleopatra's Diadem** (headgear) | Breakpoint | Blessing **duration 32s → 60s**; **refine >7 → party +5 STR/INT/DEX**; **refine >9 → party +10 ATK/MATK** |

**Sim hook (the one foundation to build):** a per-item `skill-mod table` keyed by item id
with fields — `skill_target`, `power_add_per_total_refine`, `cdcast_reduce_per_own_refine`,
plus refine-gated overrides: `target_count` (Windserpent/Sceptre-pierce),
`self_aoe_radius` (Dragonbone), `spawn_zone` (Ankh), `on_cast_buff` (Fang ASPD),
`single_target_multiplier` (Sandstorm), `party_buff_add` (Diadem), `element_override`
(Dragonbone Neutral), `duration_add` (Diadem). All integer/deterministic, folds through
existing combat/derived + tick paths. "total refine" = sum of refine across equipped items
(a new summed read); "own refine" = this item's refine. Cast reduction needs a floor
(no 0-cast). Tune %s in sim-cli.

## Open decisions

1. integral gain / whether to add the slow correction vs rely on season reset alone.
2. grant delivery: probability-throttle vs lottery-draw (draw = exact supply).
3. quota allocation across the 10 uniques in a shared point pool (equal vs demand-weighted).
4. launch playerbase target → thin-prestige vs liquid-common at launch.
5. which uniques are chase-tier (safe-upgrade) vs commoner (break-refine sink).
6. check-in cadence target → fixes the durability decay curve.
