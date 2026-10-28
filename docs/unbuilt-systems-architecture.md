# Architecture plan — the systems that are not built yet

Rough shapes for the M1.5 / M1.6 systems, written 2026-07-31. The point of doing them together
is that four of them share the same three primitives; planned separately they would each grow
their own copy of "grant value exactly once", which is the class of bug worth spending planning
on.

Every plan follows the conventions this codebase already proved, so nothing here is a new style:
`gacha.rs` (pure core, no I/O), `mailbox.rs` + `mailbox_grant.rs` (server decides, sim applies),
`crystal_wallet` / `gacha_pity` (per-row tables for anything valuable), `hp_gate_open` (one rule in
one function).

---

## 0. Shared foundations — build these once

### F1. The grant path (already exists — do not build a second one)

```
server decides → mailbox → sim `ClaimMail` (InputAuthority::Server) → GameState
```

**Every system below that hands the player something uses this.** Quest rewards, login rewards,
dispatch yields, pack contents, first-clear rewards: none of them may write to GameState
directly, because the server cannot mutate a save — the replay verifier would reject it. Adding
value has exactly one door, and it is already built and exactly-once.

Consequence for planning: a new system's "reward" work is usually just *deciding* the reward.
Delivery is free.

### F2. Claimable — the schedule + claim machine (M1.6, to build)

One server-side table of *"this account may claim this thing, once, if a condition holds inside
a window"*. Daily login, event missions, quest rewards, pack availability and first-clear rewards
are all rows in it with different conditions.

```rust
// verify-server/src/claimable.rs  — pure core, no I/O (gacha.rs shape)
pub struct ClaimableDef {
    pub id: &'static str,          // stable; the claim key
    pub window: Window,            // Always | Calendar{from,to} | SinceSignup{day}
    pub condition: Condition,      // Immediate | Counter{measure, n} | Purchased{sku}
    pub reward: RewardSpec,        // resolved to mail via F1
    pub repeat: Repeat,            // Once | Daily | Weekly | Monthly
}
pub fn claimable_now(def: &ClaimableDef, st: &AccountClaimState, now: Ts, m: &Measures) -> bool;
```

Table: `claims(account_id, def_id, period_key, claimed_at)` — `period_key` is what makes
"daily" work without a cron: the row for 2026-07-31 is a different row from 2026-08-01, and the
primary key does the deduplication. **No reset job, no scheduled task, no clock skew.**

Reused by: daily login (7-day and monthly), event missions, quest rewards, pack unlocks.

> **Avoids:** four systems each inventing claim-once + reset; a nightly reset job that
> double-grants when it runs twice or skips a day when it fails.

### F3. Measurement — counters live in the sim, judging lives on the server

`state::Measures` already counts kills, rooms cleared, refines, orbs, card feeds. Quests and
event missions **read** it; they never add their own counters to the sim.

Rule: when a new objective needs a number the sim does not track, add the counter to `Measures`
(one `u64`, skip-if-zero) — not a quest-specific field. The sim must not know what a quest is.

> **Avoids:** quest logic leaking into the deterministic core, where every change is a golden
> re-bless and a replay risk.

### F4. Catalog-as-data, versioned

Definitions (quests, claimables, stages, packs, banners) are **server-side data with a version**,
not `const` tables. Anything that grants value records **which version granted it**.

> **Avoids:** the banner problem already noted in `gacha-system-reference.md` §10 — rotating
> content means a deploy, and you cannot reconstruct afterwards what the rules were that day.

---

## 1. Telemetry (M1.5, ⛔ before content)

**Owns:** recording what happened, for querying across players.
**Does not own:** anything gameplay. Nothing may read telemetry back into the game — it is
write-only from the game's point of view. That single rule keeps it un-coupled forever.

```
client → POST /telemetry/batch  [{ts, name, fields}]  → events(account_id, ts, name, fields JSONB)
```

- Batched (say every 30 s or 20 events) — never one request per event.
- **Fire-and-forget:** a failed telemetry post must never block, retry hard, or surface an error.
  If the endpoint is down the game does not notice.
- Keyed by `account_id`; no names, no chat text, no free-form player input.
- Event names are a closed `enum` in one file, not strings scattered at call sites.

> **Avoids:** the analytics tail wagging the game (a laggy endpoint stuttering play), PII
> creeping into a table nobody audits, and typo'd event names that silently break a funnel.

## 2. Quest / objective layer (M1.5)

**Owns:** the catalog of objectives and per-account progress.
**Does not own:** counting (F3), rewarding (F2 → F1), or anything inside the sim.

```rust
// verify-server/src/quest.rs — pure core
pub struct QuestDef { id, kind: Daily|Weekly|Achievement|Event, goal: Goal, reward_ref: ClaimableId }
pub enum Goal { Measure { key: MeasureKey, n: u64 }, ClearStage { stage_id }, ... }
```

Progress is **derived, not stored**: `progress = measures_now[key] − measures_at_period_start[key]`.
Store only the snapshot taken when a period began. One row per account per period.

> **Avoids:** an increment path that can double-count on a re-save (the save log is replayed —
> anything that *increments* on receipt is a duplication bug waiting for a retry).

Daily / weekly / achievement / event missions are all this one machine with a different `kind`
and `period_key`.

## 3. Dispatch (M1.6)

**Owns:** "these characters are away until T; the yield is Y".
**Does not own:** the sim (dispatched members stay in the roster; nothing in GameState changes
while away), or delivery (F1).

```
dispatches(account_id, slot, member_ids JSONB, mission_id, started_at, ends_at, claimed)
```

- **Server wall-clock, never sim ticks.** Two timestamps; the game being closed is irrelevant.
  This is not offline progression — no simulation runs — which is why it fits an online-100%
  game without touching that decision.
- Yield is **craft materials / upgrade stock only — never Gil, never sellable goods.** Otherwise
  it is a faucet that runs without playing, which contradicts the standing monetary rule.
- Claim goes through F1, so a full inventory cannot lose the yield.

> **Avoids:** the "reward for closing the game" shape, and a second definition of elapsed time
> competing with the sim's tick clock.

## 4. Stage + dialogue (M1.6 — campaign, event stages, affection)

The largest, and **half of it exists**: an instance chain (`instance_start_zone_id`,
`is_instance_end`, `EnterInstance`) is already a stage — Duskmourn proves the shape.

Split it in three, because the three have different owners and lifetimes:

| piece | where | owns |
|---|---|---|
| **Stage** | `game_data.json` (data) | rooms, entry, clear condition — the existing instance chain |
| **First-clear** | server (F2 claimable) | "this stage, once, ever" |
| **Script** | data + a client view | who says what, when |

The script system is a **player of a data format**, not a system that knows about story:

```
scene := [ {speaker, text_key, portrait?, sfx?} … ]
trigger := on_stage_enter | on_stage_clear | on_affection_rank | on_first_meet
```

Text lives in `strings.csv` like everything else (i18n discipline), so scripts hold **keys, not
prose**.

**Affection** is then: a per-Exemplar counter on the server + rank thresholds that unlock
`on_affection_rank` scenes. It reuses the script player wholesale and adds no new UI machine.

> **Avoids:** three separate "show some dialogue" implementations (campaign, event, bond), and
> story text baked into scripts where the translator cannot reach it.
> **Non-code precondition:** none of this is useful until the story is written.

## 5. Micro-transaction + store, then packs (M1.6)

**Owns:** turning money into entitlements.
**Does not own:** what an entitlement *is* (crystal are `crystal_wallet`; items go through F1).

```
POST /store/checkout {sku, idem_key} → Steam InitTxn → (user confirms) → FinalizeTxn
    → entitlements(account_id, sku, txn_id, granted_at)  → F1 delivery
refund webhook → reverse the entitlement, ledger a negative delta, NEVER delete the row
```

- **Product catalog is server-side data** (F4). `gacha-system-reference.md` §10.2 already flags
  this as free to decide now and a rewrite later.
- The Steam txn id is the idempotency key; a webhook that arrives twice must be a no-op.
- Refunds are **compensating ledger entries**, never deletions — the audit trail must show what
  happened, not the net result.
- **Packs** are SKUs whose contents resolve through F2 + F1. They add no new grant path.

> **Avoids:** a hardcoded purchase handler per product; a refund that leaves granted items
> behind because the grant was a direct mutation instead of a ledgered one.

## 6. Level-cap / band gate (M1.5)

Small and worth doing early because it is a *release valve*: `LEVEL_CAP` exists; what is missing
is a **server-published cap** the client honours, so raising it is a config change and not a
patch.

> **Avoids:** shipping content that cannot be pulled back.

## 7. Content authoring + validation (M1.5)

Three small tools, not a system:

1. **Target → multiplier solver.** Today's boss conversion needed a script that nudged
   `*_mult_bp` by 1 until integer division landed on the authored value. Author the *target*,
   let the tool compute the bp.
2. **Validation at build time**, not test time. `spawn_pool` with `target_population: 0` was
   caught by a unit test; it should fail at author time with the zone id in the message.
3. **Tier template.** A dungeon tier is a parameterised copy: base monster, scale bp, boss refs.

> **Avoids:** hand-editing a 25k-line JSON as the normal way to add content.

---

## Cross-cutting rules (the ones that failed today, made explicit)

1. **One rule, one function.** `cast_hp_below_pct` was implemented inline on one cast path, so
   three others silently lacked it. If a rule is consulted in more than one place, it is a
   function — not a copied condition.
2. **Per-row tables for anything valuable.** The market blob's global lock and its
   `market_poisoned` coupling are consequences of storage shape, not of the domain
   (`docs/gacha-arch-review.md`).
3. **Value enters GameState only through F1.** No second door, ever.
4. **serde-skip at defaults**, so new fields keep the goldens byte-identical — and never skip a
   field the sim reads (the 0.0.196 replay hole).
5. **Client never decides an outcome.** It reports gestures and draws what the sim/server says.
6. **A new content variant must not need a client change.** Behaviour-only monster variants
   already broke this once (the boss-room handmaids rendered as slimes until `ART_ALIAS`).

---

## Build order implied by the dependencies

```
F2 claimable ──┬─→ daily login / event windows
               ├─→ quest rewards
               ├─→ dispatch claim
               └─→ packs ──← store ──← micro-transaction
telemetry      (independent — do it first, it is the cheapest and cannot be backfilled)
stage+dialogue (independent of the above; blocked on written story)
```

Telemetry first because it cannot be reconstructed later. Then F2, because four systems are
waiting on it. Micro-transaction earlier than its priority suggests — it is the only item with a
third party in the path, and surprises should arrive while there is slack.
