# Drop system — what actually happens when something dies

Status: **AS IMPLEMENTED, sim 0.0.364 / game_data 0.0.115 (2026-09-13; first written for 0.0.260).**
The map: the ordered path a kill takes, who rolls what, and where every number lives. It
deliberately does not restate the specialised docs — see [Where the detail lives](#where-the-detail-lives).

## Where a drop row LIVES (2026-09-13, user: "ต้องเคลียร์ว่าอะไรอยู่ row ไหนบ้าง")

Three layers, one home per item family. A kill still takes the same draws as before (below);
only the SOURCE of the rows changed.

| layer | home | what | who edits |
|---|---|---|---|
| **B monster rows** | `monsters[].drops` in `game_data.json` — short | the monster's **junk** (1 per monster — its identity drop, user: "junk อยู่กับ monster"), its **cards** and **uniques** (server rows), gear/ore/materials until pass 2 | content |
| **C rules** | `game_data.loot_rules` — one block | **Star Stones** (field / boss, `server_bp`), **Boss Core / Track** on zone bosses (`min_level`); generated into the table at spawn by `build_drop_table`, appended after the authored rows | balance |
| **C rule (sim)** | `potential::LOOT_TIERS` / `GRADE_BP` + the monster's FIRST weakness | the **Sigil** — a separate absolute-odds draw, never in the weighted table | balance |
| **C rule: gear** (pass 2, 0.0.365) | `loot_rules.gear` — bands by level, absolute bp per class, `dagger_mult`, `exclude` | every non-unique Weapon / Armor def of the monster's tier; 1-slot everywhere, 2-slot on bosses; weights taken out of `nothing` so authored rows keep their odds | balance |
| *(still authored rows)* | `monsters[].drops` | ore (42 rows on 31 monsters), craft materials (79) — candidates for a rule when their pattern is confirmed | content |

Rules that keep it clean: an item lives in ONE layer (a def in a rule never appears as a row;
the ratchets in `potential_coverage.rs`, `recipe_supply.rs` and `orbs::star_stone_effects_match_the_catalog`
hold their pieces); rates in a rule are absolute bp, never weights, so adding one never dilutes
another (the 0.0.107 Sigil rows did — `recipe_supply` caught it); `nothing` is the filler the
weighted table computes, not something to author.

Storage: Sigil / Boss Core / Track / Star Stones / **ore and ore shards** are **ledger** materials
(`ItemDef::ledger`, `GameState.materials`) — a count, no bag slot, tradeable through the same
commodity seams where the def is `marketable`; craft draws on the ledger after bound
bag stacks and before unbound ones.

> ⚠️ Everything above the "TWO AXES" heading used to be a 2026-06-29 draft of a system that was
> then built differently. Its wrong claims are recorded under
> [What the June draft got wrong](#what-the-june-draft-got-wrong) rather than deleted, because
> two of its ideas were never built and are still worth having. But none of it described the
> shipped game, and a doc that contradicts the code is worse than no doc — that is how
> "offline-first" survived three months past being true.

## The path a kill takes

`drops::drop_on_death`, in order. Every PRNG draw is called out, because draw ORDER is the
determinism contract (GDD §7.2): changing it reorders every drop in every save ever made.

1. **Level-gap penalty** — `loot_gap_mult_bp`. No draw. Grace 15 levels, then −20% and −5% per
   further level, floored at **10%**. Scales the weighted roll, the Gil reward AND the candidate
   gate below, so an over-levelled farmer gets 10x fewer rare tickets too.
2. **Stage-1 candidate gate** — ONE draw at a flat **1%** (`CANDIDATE_RATE_BP`), the same rate
   for every item and every monster, times the gap penalty. On a hit, a SECOND draw picks WHICH
   item the ticket is for, weighted by that entry's `server_bp`. Monsters with no server-roll
   entries take no draw at all.
3. **The weighted table roll** — ONE draw (`roll_drop_scaled`): the ordinary loot path — junk,
   materials, gear. Most kills land on the `nothing` sentinel, and the draw is taken anyway,
   which is what keeps the stream position-stable.
3b. **The Sigil draw** — ONE draw (`roll_potential_material`, 0.0.355), every kill, absolute odds
   by the monster's level tier and grade; the element is the monster's first weakness. Taken
   before the foil draw so the stream has one fixed shape.
4. **Gear leaves for the server.** A star-eligible piece (Weapon/Armor, non-unique) does NOT
   bank here — it accrues a `Gear` ticket and returns. Star, bonus lines and rune are rolled by
   the server (stage 3, 0.0.257).
5. **Foil** — ONE draw, cards only, 5%.
6. **Bank** — straight to the stash, no escrow. 3★+ non-stackables route to the rare bag.

## Who rolls what, and why

This split is the security model. The client may decide things that are low-value on their own;
the server decides anything that determines what a thing is WORTH.

| roll | rolled by | why |
|---|---|---|
| which item drops | **client**, weighted table | a base item is cheap; forging one gains little |
| gear star 0–4 | **server** | 4★ = Legendary, and no Legendary BASE item drops anywhere |
| gear bonus lines | **server** | they ARE the item's stats |
| skill rune | **server** | floors rarity at Legendary by itself |
| crafted gear star + lines | **server** | same, and 4★ is 4x likelier than off a trash mob |
| card / orb / ore payout | **server** | stage 2 — the actual rarity |
| foil bit | **server** | derived from the grant nonce |
| Gil, XP, junk | client | no scarcity to protect |

Server-rolled results arrive one save cycle later: ~1 s for a craft (which pushes immediately),
up to ~10 s for a drop (which waits for the autosave). The design accepts that explicitly.

## Pending tickets — three kinds, one list

`GameState.rare_candidates`. Each grant event resolves ONLY its own kind, and each kind has its
own nonce domain, so a nonce issued for one can never resolve another's ticket.

| | `Drop` | `Craft` | `Gear` |
|---|---|---|---|
| what it is | a lottery ticket | a paid-for craft | a piece that already dropped |
| pays out | at its `hit_ppm` | **always** | **always** |
| TTL expiry | yes, 8 h of play | never | never |
| at the cap | skipped silently | **the craft is refused** | the piece is dropped |
| grant event | `GrantDrop` | `GrantCraft` | `GrantGear` |
| nonce domain | `drop-roll:v1` | `craft-roll:v1` | `gear-roll:v1` |
| banks as | loot | a purchase | loot |

## The numbers, and where they come from

Measured, not assumed — `sim-cli --ticks 288000 --recruit …`, 2026-08-02.

| | |
|---|---|
| kills in 8 h | **11,759** — a 3-member party to band 10, so a FLOOR not a ceiling |
| gear drop | ~1 in **130** kills |
| candidate gate | **1%** per kill, x the gap penalty |
| card final rate | **1/200,000** (~136 h solo — cards are a market item, not a farm) |
| orb / ore final | 1/2,500 – 1/4,000 |
| 4★ off trash gear | 1/10,000; the boss table 1/1,000 |
| Legendary, honest player | ~1 per 1.3 M trash kills |

The final rate for any server-roll item is `bp x 1e-7`, where `bp` is its authored `server_bp`.
The monster's total **cancels out** of the formula — so changing one item's bp changes only that
item, which is what lets a card event exist without quietly halving orb drops.

## Bounds and drains

Three caps bound the list; two mechanisms drain it.

- **Prune** (0.0.260) — the server judges every ticket on every save and reports how far
  (`resolved_seq`); the client drops the duds via `PruneCandidates`. Steady state ~1.
- **TTL** — 288,000 **ticks**, and ticks only advance while the game is open. Wall-clock does
  nothing: close the game and nothing drains. This is the fallback if prune never arrives (~118).
- **Caps** — drop 1,000 / craft 64 / gear 128.

> **A bound must be unreachable or loud — never quietly reachable.**
>
> 0.0.255 is what the third option looks like. The 500-ticket cap sat at an operating level, was
> reached after ~15 minutes once the load drifted, and then stopped every server-roll drop in the
> game **silently**, for three months. The caps above are set where ordinary play cannot approach
> them, and `pending_drops` / `pending_crafts` / `pending_gear` ride `view_state` so an approach
> is visible.

`RARE_RATE_MULT_BP` (verify-server env, default x1) scales every stage-2 rate live without a sim
change — the dial for a low-CCU launch. The crowd-scaling policy that would drive it
automatically is designed, not built.

## Code map

| where | owns |
|---|---|
| `sim-core/src/drops.rs` | `drop_on_death` (the whole path), `accrue_candidate`, the caps + TTL, `card_boosted_bp`, `loot_gap_mult_bp`, banking + overflow |
| `sim-core/src/gear_roll.rs` | `roll_bonuses`, `roll_skill_rune` — called from the GRANT handlers now, not from the drop path |
| `sim-core/src/events/handlers.rs` | `grant_drop` / `grant_craft` / `grant_gear` — the server-authored half |
| `sim-core/src/events/mod.rs` | the event variants + `authority()` + dispatch |
| `sim-core/src/state.rs` | `RareCandidate`, `CandidateKind` |
| `verify-server/src/main.rs` | `keyed_nonce` + the three domains, `candidate_hits`, grant issuance, `resolved_seq`, `RARE_RATE_MULT_BP` |
| `godot/sim_driver_core.gd` | applies grants **then** the prune, in that order |
| `godot-ext/src/view_state.rs` | `pending_drops` / `pending_crafts` / `pending_gear` |
| `data/game_data.json` | `DropEntry { template, weight, server_bp }` per monster |

`DropEntry` has two mutually exclusive halves. **`weight > 0` = client-rolled** off the weighted
table. **`server_bp > 0` = server-rolled, and `weight` MUST be 0** — an entry with both is
rollable twice, by two different authorities. `server_bp` is the item's final rate as
`bp x 1e-7` (so `bp = 1e7 / desired_denominator`).

## Making a change without breaking something

**Add or retune a server-roll drop.** Set `server_bp = 1e7/den` on the entry, leave `weight` at
0. Nothing else — the rate is per-item and independent (W cancels). Run
`cargo test -p sim-core --lib`; `candidate_load_is_one_gated_roll_per_kill_whatever_the_table_holds`
will fail if the new entry pushes stage 1 off its global rate, which is the 0.0.207 mistake.
game_data version bump + deploy; no sim change, no VERSION bump, no golden move.

**Change the stage-1 gate (`CANDIDATE_RATE_BP`).** This is a global lever on server-roll LOAD,
not on any item's rarity — every final rate moves with it. `candidate_load_fits_the_offline_window`
pairs it against the cap and will fail if they drift apart. Draw counts do not change, but the
stream shifts: **goldens move, VERSION bumps.**

**Change a cap.** Both byte-budget assertions in `a_save_does_not_fill_up_with_pending_tickets`
apply. Remember the rule above: a cap must stay unreachable in ordinary play. **VERSION bumps** —
a client and server disagreeing about where accrual stops will diverge on replay.

**Touch the drop path's draws at all.** Adding, removing or reordering a `gen_range_u32` in
`drop_on_death` reorders the PRNG stream for every kill: **all kill-touching goldens re-bless,
VERSION bumps.** Read `drops.rs`'s header contract first.

**Add a fourth candidate kind.** Give it its OWN nonce domain and its own grant event, and match
the kind EXACTLY in every handler. `each_grant_resolves_only_its_own_kind` is a 3x3 matrix that
caught a real cross-kind hole the moment `Gear` landed — extend it to 4x4.

## Invariants

1. **Grants before prune, always.** The same save response carries both; pruning first deletes a
   winning ticket before its grant can find it. `a_prune_before_its_grant_would_destroy_the_win`
   pins both orders.
2. **A grant resolves only its own kind**, and each kind has its own nonce domain. Two locks,
   because the domain stops replay across a forged history and the kind guard stops it within one.
3. **The client never decides value.** If a new roll determines what an item is worth, it belongs
   on a grant nonce, not in the sim's PRNG.
4. **`weight` and `server_bp` are exclusive.** Both set = rollable by two authorities.
5. **No `.max(1)` on a rate.** An entry with no rate is not on that path; inventing one for it is
   how a card event nearly became a client minting hole (0.0.259).

## Verifying a change

```bash
cargo test -p sim-core --lib          # the iterate loop — 0.6 s
cargo test -p sim-core --tests        # + goldens, before a commit
cargo run --release -p sim-cli -- --seed 42 --ticks 288000 --sample 288000 \
    --recruit swordsman_t1,acolyte_t1,mage_t1,archer_t1   # 8 h of kills, for rate work
```

Sabotage anything new: break the line the test protects and confirm it goes red. On 2026-08-03
half the guards written for this system survived their own sabotage — two were unreachable given
their input types, one test never touched production code.

## When loot is not appearing

The failure that costs the most time here is silence: a ticket that never accrues and a ticket
that never pays look identical from the game. Walk it in this order.

1. **Client log — did the sim accrue anything?** `[CRAFT] … pending_crafts A -> B (pending_drops N)`
   prints on any craft and is the cheapest read of all three counters.
2. **Client log — did a grant come back?** `[ONLINE] server-roll gear grants applied: N`,
   `… craft grants …`, `… drop grants …`.
3. **Server log — did the server ever see the ticket?** `fly logs -a renrei-idle-server --no-tail
   | grep -E 'gear-grants|craft-grants|drop-grants'`. Nothing there means the candidate never
   reached a verified save, so the problem is client-side accrual, not the roll.
4. **Is a save even landing?** `POST /save -> 200` in the server log. A `sim version skew` line
   means the client is older than the server; the client pauses and asks for an update.
5. **Is a cap full?** `pending_drops` near its cap blocks accrual. This was 0.0.255.

Rates cannot be judged by watching. An orb is ~1 per 2 hours of play and a card is ~1 per 136
hours; seeing none for an evening proves nothing either way. Use the tests and sim-cli for rates,
and reserve play-testing for whether the PATH works at all.

## Where the detail lives

| doc | owns |
|---|---|
| `server-roll-drops-design.md` | the two-stage pipeline, its 0.0.207 regression, stage 3 |
| `server-roll-drop-table.md` | the per-item table of what goes through it |
| `gear-roll-rules.md` | how bonus lines are rolled (`gear_roll.rs`) |
| `rarity-system.md` | the rarity ladder and what promotes it |
| `card-system.md` | what cards DO once you have them |
| `crowd-scaled-rare-drops.md` | scaling rates against the active population |
| `economy-invariants.md` | the money-can't-do-X spine |

## What the June draft got wrong

Recorded so nobody rebuilds from it:

- **"~1/20,000 per kill" for cards** — shipped at 1/200,000, ten times rarer.
- **"Daily cap: 2 cards/day, reset every 24 h"** — never built. Nothing has a daily cap; the
  rate alone is the control. (Its wall-clock-at-the-boundary reasoning was sound and would still
  apply if a cap is ever wanted.)
- **"No level-gap penalty needed — tiered value is self-balancing"** — one was built anyway in
  0.0.208 and it is load-bearing. Tiered value does make low-band loot worthless, but it does
  not stop an over-levelled party farming a trivial band at full RARE rate. The penalty does.
- **"~907 kills/hr"** — remeasured at ~1,470/hr for a 3-member party at band 10.

Never built, still worth having:

- **Card Pocket ($2.99)** — a dedicated auto-catch pocket so a card can never be lost to a full
  bag, with a point-of-need upsell at the moment one drops into a full bag. Partly overtaken:
  cards are stackable and route to the album, and the rare bag covers 3★+ gear, so the loss risk
  is much smaller than it looked in June.
- **Dungeon / event rates above the normal line**, safe because access is already gated. The
  Duskmourn dungeon already runs doubled trash rates on exactly this reasoning.

---

# Table structure — TWO AXES (2026-07-02 design)

Status: **DESIGN — agreed, not built.** Restructures HOW tables are authored/rolled;
the rate/cap/economy rules above are unchanged (they layer on top of Axis B / cards).

## The problem
Today each monster's `drops` is ONE flat weighted list that JAMS TWO different concerns
together: (1) the monster's signature loot (its card, its material) and (2) the generic
band-scaled gear (`armor_s2`, `sword_s2`, … — Diablo-style, identical for every monster
in a band). Result: ~50 verbose lines per monster, most of it copy-pasted gear, and the
two knobs (what's THIS monster about vs what tier is this band) are tangled.

## The split
Every kill rolls TWO independent weighted tables, **fixed order, one PRNG draw each**:

### Axis A — Signature drops (per monster) → `MonsterDef.drops`, repurposed
- Holds ONLY the monster's own loot: its **card**, unique **materials**, **ore**,
  special **loot_** consumables, + a `nothing` pad. ~4-6 rows.
- This is where the **card rate (~1/20k) + daily cap** rules (above) apply — cards live
  on Axis A, governed by the boundary layer.
- Example (yellowling): `card_yellowling 51 / ore_primocon 250 / nothing <rest>`.
- **SIGNATURE UNIQUES (2026-07-02):** a monster's (usually a BOSS's) Axis-A table MAY
  list a specific `unique_*` at a low weight — the "this boss drops THAT blade" chase,
  RO MVP-style. This is separate from the Axis-B random-legendary luck: signature =
  targeted farming a known boss for a known unique; Axis-B = rare surprise on any kill.
  A unique may appear in BOTH (a guaranteed-ish target on its boss + a rare fluke
  elsewhere). Keeping uniques data-authored per monster means no code change — Axis A is
  already a free-form weighted table.

### Axis B — Gear roll (Diablo, band-scaled, SHARED — not per monster)
- One roll for a random gear piece scaled to the band. This is the "Common loot —
  band-tiered VALUE" class from the section above, now given a concrete structure.
- **Tier from a central `band_gear_tiers` map** (top-level data), because band→tier is
  IRREGULAR and hand-tuned (NOT a formula):
  `{1:1,2:1,3:1, 4:2,5:2, 6:3,7:3,8:3,9:3, 10:4,11:4,12:4,13:4,14:4, 15:5,16:5, 17:6,18:6, 19:7}`
  All monsters in a band share the band's tier (verified: each band drops exactly one
  `_sN` tier today).
- **Pool auto-composed from the item catalog** — every slot already has one `_sN` item
  per tier, so the Common pool for tier N = "all `*_sN`". No per-monster gear authoring.
- **Rarity inside this roll (chosen: rare/legendary live on Axis B, Diablo-style):**
  one draw yields nothing / Common `_sN` / an Uncommon / a Rare / a Legendary
  (`unique_*`), weighted mostly-nothing → mostly-Common → rare upgrades.
- **BOSS vs NORMAL profile (2026-07-02):** the rarity distribution is chosen by
  `is_boss` — two profiles under `gear_pool`. A boss kill is the "big hit": thinner
  `nothing`, fatter Rare/Legendary tail (and optionally a GUARANTEED gear drop, or N
  rolls). Same band tier — the boss is in the band; only its odds are better.
  Provisional (tune in sim-cli):
  - normal: `nothing ~75% · Common ~24% · Uncommon ~0.8% · Rare ~0.18% · Legendary ~0.02%`
  - boss:   `nothing ~0% (guaranteed) · Common ~60% · Uncommon ~30% · Rare ~8% · Legendary ~2%`
  - If a boss rolls the gear axis MULTIPLE times (Diablo burst), each extra roll is an
    extra PRNG draw → account for it in the draw contract below.
- **Legendary `unique_*` are a FIXED set, not `_sN`-tiered.** Two sources feed them
  (both allowed — see Axis A note): the Axis-B random-luck tail (band-gated subset so a
  b1 kill can't cough up the top unique) AND monster-specific SIGNATURE uniques on Axis A.

## Before / after (authoring)
```
BEFORE (one tangled table, ~50 lines):
  drops: [ dagger_s2 500, sword_s2 250, …14 gear rows…, card_redling 51,
           nothing 760000, junk_b3 250000, ore_primocon 250 ]

AFTER:
  Axis A  drops: [ card_yellowling 51, ore_primocon 250, nothing <rest> ]   # ~4 rows
  Axis B  (nothing on the monster — pulled from band_gear_tiers[band] + the shared pool)
```
Adding a monster = write its signature list only; gear comes free from the band pool.

## Roll order & determinism (CONTRACT)
Per kill, in this FIXED order:
1. **Draw 1 — Axis A**: `roll_drop(monster.drops)` (unchanged path; always draws).
2. **Draw 2..(1+N) — Axis B**: `roll_gear(band_tier, profile)` — the shared tier pool
   with the normal/boss profile. Always draws (even on `nothing`) so the stream is
   position-stable. A normal kill does N=1; a boss doing a Diablo burst does N=fixed
   boss-roll-count (each an extra draw). N MUST be deterministic (a constant / data
   field), never data-dependent, or replay diverges.
Going from 1→(1+N) draws per kill REORDERS the PRNG stream → **all kill-touching goldens
re-bless** (intentional, one time). New field(s) (`band_gear_tiers`, gear-pool config
incl. the two profiles + boss roll-count) bump `sim_core::VERSION`.

## Open decisions (before build)
- **Gear drop rate**: fold "does gear drop?" into the Axis-B weighted `nothing`, or a
  separate up-front chance? (Leaning: fold in → 1 draw for the whole axis.)
- **Slot fairness**: uniform slot pick, or weight toward the player's needs / away from
  dupes? (Leaning: uniform first, revisit.)
- **Legendary pool**: RESOLVED as a hybrid — signature uniques on a boss's Axis A +
  a band-gated random tail on Axis B. Still to pin: which specific `unique_*` binds to
  which boss, and the exact band-gate for the Axis-B tail.
- **Boss burst**: does a boss roll Axis B once (better profile) or N times (a loot
  spray)? If N>1, fix N as a constant/data field (determinism).
- **junk**: keep `junk_bN` (vendor trash) on Axis A, or its own axis?
- **sim-cli**: add a drop-rate report column so the new weights are tuned headlessly
  before touching goldens.
