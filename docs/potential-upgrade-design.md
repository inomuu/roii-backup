# Potential — how a node is bought (design, 2026-09-11)

The Potential constellation (code `potential`, screen name **Potential**) shipped as a prototype
that spent Job skill points behind a Job-tier gate. This replaces both, per the user's decisions
today. Refund decision (b) still holds: an XP refund never touches Potential.

## 2026-09-30 — Sigil has no element

User: "เอาธาตุออก ให้ sigil ไม่ต้องมีธาตุ". Three Sigils (`sigil_1..3`, grade I-III) replace the
eighteen element Sigils; every unit spends the same ones. Drops keep their odds and their gate (a
monster whose first weakness is in the chart), so the total is unchanged. Old saves fold every
element Sigil into the generic one of its grade on load. The "Sigil follows the unit's ELEMENT"
decision below is superseded.

## Decisions (user, 2026-09-11)

1. **Job level leaves the game.** Nothing on the board reads it; its removal is a separate step.
2. **Rank cap by BASE level, and only past 50.** From level 1 every node may be raised to
   rank 5 — skill nodes and the rest alike — if the player has the items. Above 50 the cap is
   `level / 10`: Lv60 → 6, Lv70 → 7, Lv80 → 8, Lv99 → 9, Lv100 → 10. The +10-per-update cap
   cadence therefore unlocks a rank per update on its own; rank 10 belongs to a future Lv100.
3. **No reset.** A rank bought stays bought. `ResetPotential` becomes a tombstone.

## Cost — items only, one material family in three grades

- **Potential Shard I / II / III** (`potential_shard_1..3`, stackable Materials, bound: not
  marketable, no sell value). Not doctrine-specific on purpose — a per-doctrine shard would
  produce the "dropped for the wrong unit" trap the stat design already avoided.
- Rank `n` costs **`n` shards** of the grade for its band: ranks 1-3 grade I, 4-6 grade II,
  7-10 grade III. No Gil in v1 (a sink can be added later without touching the rule).
- Stat nodes and Majors are rank-1 nodes: one shard of grade I (they are the constellation's
  connective tissue, not its cost).
- **Sources** (a later stage; the mechanics ship first): band-scaled monster drops (grade by
  band), boss bonus, daily/mail rewards. Until drops are authored, the dev grant fills the bag.

## The rule in one place

`potential::level_tier(level)` is the only cap. `TierCaps.level` replaces `.job`;
`LockReason::LevelTier` replaces `JobTier`. The Commander tier gate is unchanged.
Purchase, batch preview (`MaxPotential`) and the host view all run one `Purse` — the shard
counts snapshotted from the bag — so what the screen quotes is what the commit spends.

## Stages

1. sim: cap rule, shard cost, purse, tombstone reset, item defs, tests.
2. host + Unity screen: shard counts, cost per node, no reset button, lock text.
3. drops: author the shard rows per band (moves goldens — its own commit).
4. ~~job level removal~~ **DONE 0.0.352** — and class with it (user 2026-09-12: "class job ทิ้งหมดแล้ว"):
   no advancement, no tiers, no skill tree gates, no skill points. `player_class` defs remain as the
   stat block a unit is built from, not an identity.

## Naming (2026-09-12)

The template a unit is built from is called a **unit** everywhere now: catalog `units`, `UnitDef`,
entity/roster field `unit_def`, `data.unit(id)`. `unit_id` (a number) is a roster/entity id;
`unit_def` (a string) is the template. "class" survives only in the retired 10 legacy entries'
ids (`swordsman_t1` …), which remain as stat blocks for Exemplar lines and test fixtures.


---

# v2 — the gacha-style cost ladder (user decisions, 2026-09-12)

Supersedes the single-shard cost above. Shape borrowed from Genshin/HSR/NIKKE after a survey
(see the chat record): three tiers of basic materials, an ELEMENT-locked family and a generic one,
escalating Gil, rare capstones on the last ranks — and, because ROI has NO energy system, pacing
comes from drop rates over idle time, crowd-scaled server rolls, time-gated rewards and the market,
never from a guaranteed drop.

## Decisions

- **No level gate.** Cost is the only pacing. (`level_tier` is retired.)
- **Skill nodes rank 1 → 20.** Rank 1 = the free kit. **Soft cap 12** — reachable by farming alone
  in ~1-2 weeks of idle per unit; Boss Core enters at rank 7, Track at 11-12 (Genshin's weekly-boss
  + crown shape). **Hard cap 20** — ranks 13-20 use Core and Track in rising amounts and Gil that
  climbs to seven figures; the whale / long-haul sink.
- **Ranks 11-20 are OVERRANK:** the skill's own formula stops at level 10; each rank past it adds
  **+2% to the skill's numbers** (one hook for every skill) — rank 20 = +20%, felt in a boss race,
  never enough to put the baseline out of scale ("baseline ต้องรอด").
- **Stat nodes and Majors stay rank-1** (3 × Sigil I + Gil).
- **Sigil follows the unit's ELEMENT, not its doctrine** (user 2026-09-12: "ตาม element ไม่ใช่
  doctrine"). A Fire unit spends Fire Sigil; ten elements (Neutral shown as Physical, Fire, Earth,
  Water, Wind, Holy, Shadow, Poison, Thunder, Spirit).
- **Everything is TRADEABLE** — Sigil, Essence, Core and Track (user: exchange is the pillar). A
  drop in the wrong element is a market good, not a trap. Pacing of the rare tiers is supply
  (server-rolled, crowd-scaled) and time (weekly/pass/mileage), which the market prices.

## Material families (Essence CUT 2026-09-13 — user: "ตัด Essence ออกก่อน ใช้แค่ Sigil"; the table below and
## the cost ladder keep their Sigil / Core / Track / Gil columns, the Essence column is gone in code)

| family | ids | source | roll |
|---|---|---|---|
| **Element Sigil I/II/III** | `sigil_<element>_1..3` (10 × 3) | every monster, grade by its level, **element random** (one row per element) | sim |
| ~~Essence I/II/III~~ | — | CUT 0.0.360; was the generic second basic family | — |
| **Boss Core** | `boss_core` | bosses, a CHANCE (crowd-scaled); weekly reward | server (`server_bp`) |
| **Track** (placeholder name) | `track` | high-band bosses, very low (crowd-scaled); weekly / pass / mileage | server |

## Cost table — one skill node (Sigil = the unit's element)

| rank | Sigil (×2 since 0.0.363) | ~~Essence~~ | Core | Track | Gil |
|---|---|---|---|---|---|
| 2 | I ×4 | – | – | – | 1k |
| 3 | I ×6 | – | – | – | 2k |
| 4 | I ×8 | – | – | – | 4k |
| 5 | II ×6 | – | – | – | 8k |
| 6 | II ×8 | – | – | – | 12k |
| 7 | II ×10 | – | 1 | – | 20k |
| 8 | II ×12 | – | 1 | – | 30k |
| 9 | III ×8 | – | 2 | – | 50k |
| 10 | III ×10 | – | 2 | – | 80k |
| 11 | III ×12 | – | 3 | 1 | 120k |
| **12 (soft cap)** | III ×16 | – | 3 | 1 | 160k |
| 13 | III ×20 | – | 4 | 1 | 200k |
| 14 | III ×24 | – | 5 | 1 | 260k |
| 15 | III ×28 | – | 6 | 2 | 330k |
| 16 | III ×32 | – | 7 | 2 | 420k |
| 17 | III ×36 | – | 8 | 2 | 520k |
| 18 | III ×40 | – | 9 | 3 | 650k |
| 19 | III ×48 | – | 10 | 3 | 800k |
| **20 (hard cap)** | III ×60 | – | 12 | 4 | 1.0M |

Per skill node to 12: Sigil 9 I / 18 II / 23 III, Essence the same, Core 12, Track 2, Gil 487k.
Four skill nodes to 12: ×4. To 20 adds Sigil/Essence III ×144 each, Core 61, Track 18, Gil 4.2M.

## Where the materials live and how they drop (2026-09-12, built 0.0.354-355)

- **Materials are a LEDGER, not bag items** (user: the 35 stacks would clutter the bag). `GameState.materials`
  is `def_id → count`: no slot, no cap, never bound, party-shared like Gil. The commodity market lists from
  and delivers into it by def id, so everything stays tradeable; the shape is what a Steam Inventory
  itemdef mint would need (fungible id × qty) if that is ever wanted.
- **Loot table by MONSTER LEVEL, not per-monster rows** (user: "ควรทำ loot table ตามเลเวลมอนสเตอร์").
  `potential::LOOT_TIERS`: ≤25 → grade I, 26-50 → II, 51+ → III, with absolute per-kill odds (field / boss).
  One extra PRNG draw per kill. Grade following the monster's level is what ties II and III to the high
  bands; grade I stays wanted forever (every stat node), so the low bands keep a reason to be visited.
- **The Sigil a monster drops = the element it is WEAK to** (its first weakness). Monsters keep an
  `element` only as their ATTACK element; the player side has no weakness (100% always, gear RES still
  applies). Bring the unit that hits the monster hardest and it farms its own material — "find where to
  farm" is real without an element map. The element farm map is a QoL layer for later (user), and when
  built it is just a spawn pool that gathers monsters sharing a first weakness.
- **Coverage (done 0.0.358, ratcheted in `tests/suite/potential_coverage.rs`):** every chart element
  (6) is the first weakness of ≥1 FIELD monster in each grade tier (18 cells), and all six appear in the
  zones up to level 20.
- **Element areas unlock at level 20** (user 2026-09-13): the first four areas teach every element on
  ordinary monsters; from Lv20 a dedicated element area per chart element becomes the main Sigil source.
  NOT built — when it is, an element area is a zone whose spawn pool gathers monsters sharing a first
  weakness (same drop path, nothing new in the sim beyond the zones and their gate).
- The level-gap loot penalty does not apply to this draw (grade is the level lever).
- Band scaling to the player's level was considered and dropped in favour of the level-keyed table.

## Node tiers, every-grade drops, conversion (2026-09-13, built 0.0.362)

- **Target (user): soft cap 12 on ONE unit in 3-4 weeks of farming alone** (solo, no crowd scale).
- **Stat / Major nodes are tiered** (user: "stat เต็มไวไป ต้องมี node ยากด้วย") — authored `tier` per node,
  priced by `NODE_TIER_COST`: T1 (s1 s2 s5 s6) Sigil I ×3 + 1k · T2 (s3 s4 s7) II ×3 + 5k · T3 (s9 s10)
  III ×3 + Core 1 + 20k · T4 (s8) III ×6 + Core 2 + Track 1 + 60k · M1 (A B) III ×4 + Core 2 + 40k ·
  M2 (C) III ×8 + Core 4 + Track 1 + 120k — **every Sigil count ×2 since 0.0.363** (T1 I ×6 … M2 III ×16).
  Per unit to rank 12 + every node: Sigil 96 I / 162 II / 240 III, Core 60, Track 10, Gil ≈ 2.3M.
- **A zone pays every grade up to its own** (`GRADE_BP` I 10 / II 4 / III 2 bp since 0.0.363, boss ×7.5; one draw,
  highest grade checked first): a 7-day run had both starters stuck on day 1 with 0 grade I and 20-30
  grade II they could not spend — grade I must flow everywhere because every unit keeps buying it.
- **Sigil converts both ways at 3:1, lossless** (`MergeSigil` / `SplitSigil`, user: "ควรให้ผสมหรือแตกออก
  ได้ด้วย"): grades are denominations of one material; the drop table, not the exchange, is what makes a
  higher band worth standing in. Unity: four buttons under the purse. sim-cli converts toward
  affordability automatically.
- **Core 20 bp / Track 3 bp** on the boss rows (were 2000 / 200 placeholders) — at ~1.5k boss kills a day
  that is ~3 Core and ~0.45 Track a day solo, i.e. ~20 days for the 60 / 10 a unit needs. sim-cli prints
  the expected value; the real rate is crowd-scaled below this.

## Measured (sim-cli, seed 42, 4-member party, solo)

21 days at the 0.0.362 rates (20 / 8 / 3 bp, old requirements): pooled across all elements 292% of one
unit's whole board; own element Rapis 68%, Seraphine 29% (zone choice — the party sat in b45-52 where
Physical-first monsters are few); Core / Track expected 55 / 8 of 60 / 10 (on target). User's fix
(0.0.363): odds halved + every Sigil requirement doubled → ~4× slower, pooled ≈ 75% at 3 weeks =
the 3-4 week target. The pooled figure is the one to tune against: Sigils are sim-rolled (no crowd
scale), every element trades, and the Lv20 element areas make own-element ≈ pooled without a market.
Rerun after any change: `sim-cli --ticks 18144000 --sample 1728000 --recruit seraphine,rapis,seraphine`
(≈1 sim day per 1.5-2.5 min wall; a live progress line prints on stderr).

## Drop rates (measured in sim-cli against the time targets)

Targets: **rank 12 on one unit ≈ 10-14 days of idle by farming alone; rank 20 ≈ 2 months.**
Sigil 2% / boss 15% (one element per monster — its first weakness); Essence 4% / boss 30%;
Boss Core server-rolled ~20% per boss kill base + 2 per weekly; Track server-rolled ~2% on band 10+
bosses + 1 per weekly + pass + mileage.

## Build

A. sim: catalog (30 Sigil + 3 Essence + Core + Track), `Purse` over all families keyed by element,
   the cost table, overrank hook, `level_tier` retired, drops for Sigil/Essence, server_bp rows for
   Core/Track. B. host + Unity: per-family counts, per-rank recipe on a node. C. sim-cli: spend +
   measure the time targets; tune rates.

## Daily soft cap (user 2026-09-12 — DEFERRED: "ค่อยคิดระบบ cap ทำระบบอัพเกรดก่อน"; a sketch, not a decision)

Sigil and Essence are sim-rolled, so their supply grows with accounts × hours with no ceiling.
A per-account DAILY cap makes total supply computable (N accounts × cap) and blunts multi-client
farming. Because the game is sold on being left open 24/7, the cap is SOFT: past it the family's
drop rate falls to ×0.25 rather than zero. Set it at ~2× what a normal 24 h idle yields at the
target band, so ordinary play never meets it. Implemented in the sim on a 864,000-tick window
(one day at 10 ticks/s) — deterministic, replayable, server-verifiable, no wall clock. Core/Track
are already crowd-scaled server rolls and need no cap. Needs the measured yield first.
