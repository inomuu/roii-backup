# Server-roll DROPS — unpredictable rare/tradeable drops

> ## ⚠️ ส่วน GEAR ของเอกสารนี้ STALE แล้ว (0.0.369, 2026-09-14)
> **ของดิบไม่ roll อะไรเลย** (D1 — `gear-fusion-and-base-fix-design.md`) ⇒ เส้นทาง server-roll
> สำหรับ **gear** ถูกถอนทั้งหมด (`accrue_gear_candidate` / `roll_gear_star` / `star_eligible`
> หายไป, `CandidateKind::Gear` ไม่มีผู้สร้างแล้ว เหลือไว้เพื่อ save ที่ค้างกลางทาง)
> เหตุผล: กลไกนี้มีไว้กันไคลเอนต์แก้ค่ามินท์มูลค่าจาก ★/bonus line/rune — เมื่อไม่มีอะไรให้ roll
> ก็ไม่มีอะไรให้กัน. ส่วน **การ์ด / material / capstone** ยังใช้เส้นทางนี้อยู่ตามเดิม


DESIGN (not built). Makes VALUE-bearing drops (rare/tradeable gear) server-authored so a
client can't shadow-sim to predict or steer them. Extends the server-roll REFINE template
(`refine-server-roll-2b.md`) to the drop path. Prerequisite for a live tradeable market (a
client-rolled rare can be steered → market flood). Companion to `poe-gear-direction`
(roll-at-drop is canon), `rare-loot-bag-design`, `s3-economy-design.md`, `input-authority-
security` memory.

## Threat

The client runs the deterministic sim, so it can shadow-sim ahead and (a) predict which kill
drops a rare, (b) predict the rolled stats/stars, then STEER (kill the right monster at the
right time / only keep runs where the rare rolls godly). Replay-verify does NOT catch this —
all inputs are legal; the state verifies. Same class as refine RNG-steering, which we closed
with server-authored nonces.

## Why drops ≠ refine: FREQUENCY

Refine is an occasional deliberate action → a server round-trip per refine is fine. Drops
happen every kill (thousands/hour) → a round-trip PER kill is impossible (latency + load).
The design must move the VALUE roll server-side WITHOUT a per-kill round-trip.

## Design: decoy candidate → server roll (two-stage)

**Stage 1 — client "candidate" (decoy, frequent, predictable, worthless).** The sim rolls a
low-rate `rare_candidate` flag per kill (e.g. 1% = 1/100), deterministic. The client KNOWS its
candidates (shadow-sim-visible) — but a candidate is NOT a rare; it's just "this kill gets a
server roll".

**Stage 2 — server roll (the real value, secret, unpredictable).** Each candidate is resolved
by a server nonce → the effective rare rate is `candidate_rate × server_rate`. Example:
`1/100 × 1/1000 = 1/100,000`. The server nonce authors the FULL item (see below).

**Why the decoy works:** with a low server rate (1/1000) a candidate is ~99.9% dud → knowing
you have a candidate reveals almost nothing about rarity → shadow-sim can't steer. (If the
server rate were high, a candidate would become a strong predictive signal and the decoy
breaks — keep server_rate ≪ 1.)

**The split is a knob** (same final rate, different trade-offs):

| split | final | server-roll load | decoy quality |
|---|---|---|---|
| 1% × 0.1% | 1/100k | rolls 1% of kills | good (candidate = weak signal) |
| 0.1% × 1% | 1/100k | rolls 0.1% (10× less) | worse (candidate = 1% rare, stronger signal + rarer) |
| 10% × 0.01% | 1/100k | rolls 10% (10× more) | best decoy, heavy load |

`candidate_rate` sets server-roll LOAD; `server_rate` sets DECOY strength (lower = weaker
signal = better). 1% × 0.1% is a balanced default.

## Piggyback on the save-push — ZERO extra round-trips

Candidates are deterministic, so the server SEES them when it replays the log at `PUT /save`
verify — the client doesn't even send them. So:
1. Sim: kill → maybe a `rare_candidate` (recorded/derivable; NO item yet, rarity pending).
2. `PUT /save` (the existing ~10s push): server replays + verifies, and for each candidate
   rolls `keyed_hash(secret, account, candidate_seq)` → authors any rare → returns them in the
   save response.
3. Client applies each as a **server-authored grant event** `GrantDrop{candidate_seq,
   server_roll}` (recorded) at the next tick.
4. Next `PUT /save` includes the grant events → server replay applies them + verifies each
   nonce was its own issue → the rare is now in both states (consistent). Replay-safe, exactly
   like `StartRefine{server_roll}`.

Cost: no extra latency infra; rares materialize ~10–20s after the kill (fine for idle). Server
work = cheap hashes inside the verify pass we already run.

## Full-item server roll (NOT just "rare y/n")

The server nonce authors the COMPLETE item — star count + EVERY rolled stat/affix line — from
sub-keys `keyed_hash(secret, account, candidate_seq, "stars" | "affix_i")` (or nonce-seeded
PRNG). **This is required, not optional:** if only rarity were server-rolled and stats were
client-rolled, a player could shadow-sim to steer godly rolls (a 5★ perfect-affix item is the
high-value case). So the entire ROLL-AT-DROP moves server-side for the value-bearing path.

- Star count = rarity = routing (2★+ → rare bag / tradeable; 5★ → Immortal — per
  `poe-gear-direction` / `rare-loot-bag-design`). The server's star roll decides rare-ness.
- The item MODEL doesn't change (roll-at-drop stays canon) — only the RNG SOURCE moves for
  high-value rolls.

**Division of labor:**

| drop | who rolls | notes |
|---|---|---|
| common / low-star (the bulk) | client (in-sim) | predictable but worthless → no need to protect |
| candidate → rare (high-star) | **server** (nonce) | stars + all affixes unpredictable |

## Register into the ledger (tradeable-safe)

A server-authored rare is born with a server-assigned `instance_id` and registered in the
economy ledger (instance_id → item, owner) → dedup / no-forge / mint-on-verify at trade
promotion (`s3-economy-design.md`, `economy-invariants.md`). A forged `GrantDrop` (nonce not
issued for that candidate) → save rejected + `cheat_flag` (like refine).

## Guard: candidate INFLATION

Candidates are client-predictable, so a cheater could PRNG-grind (shadow-sim orderings that
yield >expected candidates) → proportionally more rares. Bounded by:
- **Total kills ≈ real-time-capped** (the fast-forward tick-rate cap — `online-robustness.md`)
  → the absolute candidate count has a ceiling.
- PRNG-grinding gain is small (short-window variance) and costs optimal play.
- **If tighter is needed: server rate-limit rares per account per REAL time** (final rate not
  tied to the client's candidate count) — the strongest guard, at the cost of decoupling drops
  from effort.

## Bandwidth — ~zero

- **What the server returns** (in the `PUT /save` RESPONSE = server→client = EGRESS = the paid
  direction): ONLY actual rares — `instance_id` + rolled stars/affixes + the `server_roll`
  nonce (~100–300 bytes each). A DUD candidate returns nothing — convention: a candidate not
  granted in the response IS a dud (zero bytes, no "dud" message).
- **Egress is negligible:** rares are 1/100,000 kills → ~1 per player per ~3h → at 10k
  concurrent ≈ ~200 bytes/s total. Smaller than the heartbeat.
- **No extra ingress:** candidates aren't sent (the server recomputes them from the replay);
  the `GrantDrop` events ride the next bundle (client→server = ingress = FREE) and are tiny.
- Net: server-roll drops ride the existing save cycle → no extra round-trips AND ~no extra
  bill (egress only for the actual rare, which is rare by definition). The real egress cost
  remains the future social push channel, not this.

## Online-required

Rares need the server to author them → require online. The game is online-required already;
offline/alpha can client-roll non-tradeable placeholders (like refine's offline "0").

## Reuses the refine template

Nonce = `keyed_hash(secret, account, bound-key)`; injected into the sim as a server_roll /
grant event; verified at `PUT /save` (issued? bound? not forged?); forgery → reject +
`cheat_flag`; `authority()` = Server when server_roll ≠ 0. The drop path adds: the decoy
candidate filter, the piggyback-at-verify resolution, the full-item roll, and ledger register.

## Build order (when the tradeable market approaches — NOT before)
1. `rare_candidate` roll in the sim (tunable rate) + a pending-candidate representation.
2. Server candidate resolution at `PUT /save` verify → `GrantDrop` events + save-response.
3. Full-item server roll (stars + affixes) from the nonce.
4. `GrantDrop` apply + verify + reconcile (extend the refine reconcile).
5. Ledger register + the inflate rate-limit.
Gate: must ship BEFORE rare gear has LIVE tradeable value (same gate as server-roll refine).

## Second user of the same machinery: CRAFT (built 0.0.254, 2026-08-02)

The 12 Duskmourn weapon recipes need a star + bonus lines on their output, and a 4★ is
Legendary — so the roll belongs to the server for exactly the reason above. Rather than a
second pipeline, a crafted weapon is a second KIND of pending candidate
(`CandidateKind::Craft`) travelling the same accrue → resolve-at-save → grant-event path.

Two rules invert, and both are load-bearing:

| | DROP candidate | CRAFT candidate |
|---|---|---|
| rate | ~0.1% (`drop_hit_den`); duds are never sent | **none** — it was paid for, all are granted |
| TTL | expires after 8 h (duds must not accumulate) | **exempt** — ageing it out would delete a paid item |
| cap at accrual | silently skipped (a free ticket costs nothing) | **refuses the craft** rather than eating 30 materials |
| banks as | loot (rare bag routing) | a purchase (straight to the stash) |

Because the rules invert, the two must not share an event. `GrantCraft` has its own nonce
domain (`craft-roll:v1`) and its own validator with no hit test, and both handlers guard on
the candidate's kind. Without that separation a client always holds a legitimately-issued
craft nonce for some seq N, and could replay it against a forged history where N is a card
candidate — minting a 1-in-200,000 card out of a 30-material craft. The domain tag makes the
nonce invalid as a drop grant; the kind guard makes it a no-op even if it were not.

The offline caveat above applies with teeth here: a drop candidate offline is a dud that
expires harmlessly, but a craft candidate offline is materials spent on an item nobody will
ever deliver. The craft screen therefore refuses GEAR recipes while offline
(`server_rolled` on the recipe payload, from the sim's own `craft::output_is_rolled_gear`);
potions, ores and materials are unaffected and still craft instantly.


## Regression + repair, 0.0.256 (2026-08-02)

The pipeline above ran INVERTED from 0.0.207 until 0.0.256, and it is worth recording how,
because the failure was invisible in every way a failure can be.

`d5c682d` moved 380 rows (star stones, rune stones, ore, legacy cards) onto server-roll and
hit the confirmed final rates with `candidate_bp = 10^7/den` against the flat 0.1% server
rate. That is arithmetically exact — and it puts per-item rarity in STAGE 1, which is the one
place §Design says it must not go. The form inverts with rarity: the MORE common the item, the
HIGHER its candidate rate (1/2,500 needs 40%). Rolled per drop-table ENTRY rather than per
kill, a 12-entry monster accrued 1.3 candidates per kill.

Measured across the catalog: **0.62 candidates/kill against the 0.01 this design budgets** —
62x, where the table above already calls 10x "heavy load".

Consequences, in the order they bite:

1. `RARE_CANDIDATE_CAP` (500) was sized for the documented rate. It is overrun after ~800
   kills, about 15 minutes.
2. Past the cap `drop_on_death` stopped accruing — so every server-roll item stopped, cards
   included, even though cards at 0.5%/kill would have taken 27 hours to fill the list alone.
   They were collateral damage from the orb and ore traffic sharing one list.
3. It stops SILENTLY. Final rates stay correct in the data, the client logs nothing, the
   server logs nothing, and no test measures accrual. Live for three months.

The repair keeps every authored number: stage 1 is one gated draw per kill at
`CANDIDATE_RATE_BP` plus a weighted pick of WHICH item, and stage 2 moves onto the ticket as
`RareCandidate.hit_ppm`, derived by `stage2_hit_ppm(W) = 10*W` so that
`1% x bp/W x 10W/10^6 == bp/10^7` exactly. `server_bp` in game_data.json keeps its meaning as
relative frequency within a monster; nothing was rebalanced.

**The rate and the cap are one decision, not two.** `candidate_load_fits_the_offline_window`
now checks them together against a measured 8 h offline window (11,759 kills), demanding 2x
headroom — offline there is no server to resolve anything and the TTL is the only drain, so
the whole window has to fit. Change either number and that test is where you find out.


## Stage 3 SHIPPED, 0.0.257 (2026-08-02)

The "full-item server roll" in the build order above is live for DROPS. The client picks which
item drops off the weighted table; the server decides what it is worth.

Why it could not wait for the market: no Legendary BASE item drops anywhere in the catalog, so
every Legendary in the game is minted by the STAR roll (4★ → Legendary) or by the skill-rune
roll (which floors rarity at Legendary). Both were client-side, along with `roll_bonuses`,
which is the item's stats. An honest player sees a Legendary about once per 1.3 M trash kills;
a modified client sees one whenever it likes.

| roll | before | now |
|---|---|---|
| which item dropped | client (weighted table) | client — unchanged, low value alone |
| star (0-4) | client PRNG | **server nonce** |
| bonus lines | client PRNG | **server nonce** |
| skill rune | client PRNG | **server nonce** |

`CandidateKind::Gear { boss }` carries the piece; `boss` rides along because the generous star
table is the boss's third reward axis and the server cannot know what killed what. `GrantGear`
replays star → bonuses → rune off `Pcg32::new(server_roll)`, the same order the client used, so
nothing about the item changed except who rolled it. It banks as LOOT, so rare-bag routing
still applies — unlike a craft, which banks as a purchase.

Costs, both accepted:
- A piece appears one save cycle after the kill rather than instantly. §"Piggyback on the
  save-push" already budgets for this ("~10-20 s after the kill (fine for idle)").
- The run summary tallies at grant time, not drop time — tallying earlier would list an item
  the player had not been given.

`GEAR_CANDIDATE_CAP = 128` is sized against `SPAN_STALL_TICKS`, not against hours: the client
freezes its own sim after ~5 min unsynced, which is ~122 kills, of which ~1 is gear. At the cap
the piece is dropped rather than banked un-starred — banking it would be a way for a client to
mint gear the server never valued, which is the entire thing this stage exists to prevent.

**Three kinds now share one candidate list, and each grant must match its kind EXACTLY.**
`grant_craft` briefly matched "anything that is not a Drop", which the moment Gear existed let
a craft nonce resolve a gear ticket. Separate nonce domains stop the same trick across a forged
history; the kind guard stops it within one. Both locks, always — and no
`always_pays()`-shaped helper, because that predicate is what made the loose match read
correctly.
