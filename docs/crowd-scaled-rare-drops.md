# Crowd-scaled RARE drops — an elastic supply cap that defangs bots

DESIGN (not built). Scales the SERVER-ROLL rare-drop probability inversely with how many
accounts are farming a source, updated hourly. The point is NOT to redistribute players — it
is to **bound aggregate rare supply per source so a bot farm can't flood the tradeable
market**. Rides entirely on the server-roll layer (`server-roll-drops-design.md`), so it never
touches the deterministic client sim. Companion to `competition-integrity.md` (the per-account
lifetime cap it pairs with), `economy-invariants.md`, `s3-economy-design.md`, `steam-market-
banana-vision` + `economy-antibot-brief` memories.

## Goal / threat

Very-rare, tradeable drops are the high-value tail of the economy. A bot farm's whole play is
to run N accounts 24/7 on the best rare source and dump the output on the market → price
collapses, the scarcity that makes rares meaningful evaporates, and legit players are
out-competed on supply. Detection/ban is a cat-and-mouse arms race. This design removes the
**economic incentive** instead: make botting yield ~nothing extra per added account.

Explicit NON-goals: this does not touch common drops (the market price already balances those —
see "Scope"), and it is not a bot DETECTOR (it's crowd-selective, not bot-selective — see
"Why crowd-selective is enough").

## Core mechanism

- Only items that already go through the **server roll** (`server-roll-drops-design.md`, the
  very-rare / tradeable tail) are affected. Commons roll client-side, deterministically, and are
  untouched.
- The server tracks, per rare SOURCE, how many accounts farmed it in the last hour (`N`).
- Once an hour it recomputes a per-source multiplier `m(N)` and applies it to the server roll's
  effective rate: `effective_rare_rate = base_server_rate × m(N)`.
- `m(N)` DECREASES as `N` grows (crowded → rarer) and rises toward a cap as `N → 0` (empty →
  richer, bounded so an empty map isn't a jackpot).

Because the roll is authored server-side on the verified save, `m(N)` is pure server state: it
never enters `GameState`, the client sim, or the replay log.

## Why it does NOT break determinism (the architectural fit)

The v1 instinct — scale drop rate by live crowd — breaks the core invariant when applied to
COMMON client drops: drops are a deterministic PCG32 roll (fixed draw order, replay-verified),
and a live global crowd count is server-side, time-varying state the client can't see. Feeding
it per-tick would demand real-time online sync and blow up the replay log.

This design sidesteps all of that by two choices:
1. **Server-roll only.** The value roll is ALREADY server-authoritative and off the
   deterministic path (`server-roll-drops-design.md`). Multiplying its rate by `m(N)` is a
   server-internal knob — zero client-sim / replay impact.
2. **Hourly, not per-tick.** A coarse, slow modifier means no input stream, no per-kill
   round-trip, no online-every-tick requirement. It fits the semi-MMO's periodic save-sync.

## The economic property that makes it work

Let per-account rare rate scale as `~1/N`. Then aggregate output from a source:

```
aggregate ≈ N accounts × (base / N) ≈ constant
```

**Aggregate rare supply per source per hour is ~constant regardless of headcount.** A bot farm
adding 10 accounts to one source gets ~the same total rares, now split 10 ways → per-account
yield collapses → botting stops paying. Rares stop being "supply that scales with bot count"
and become an **elastic supply cap per source**. This is congestion pricing applied to drops.

### Tuning knob: the curve

- `m(N) ∝ 1/N` — aggregate held ~constant (HARD cap; maximally bot-hostile, hardest on legit
  players sharing a hot source).
- `m(N) ∝ 1/√N` — aggregate grows sublinearly (SOFTER; legit players keep most of their rate,
  bots still hit diminishing returns).
- General form `m(N) = clamp((N0 / (N0 + N))^k, m_min, m_max)` — `N0` sets the "comfortable
  headcount" pivot, `k` the steepness, `m_max` the empty-source bonus cap, `m_min` the floor so
  a hot source never goes fully dry. Start soft (`k≈0.5`) and tighten with live data.

## Why "crowd-selective, not bot-selective" is the RIGHT target

The modifier hits everyone on a crowded source equally — a legit whale there is throttled the
same as a bot. That is fine and in fact the point: the market only cares about AGGREGATE rare
supply, not who produced it. Capping aggregate per source protects the price whether the crowd
is bots or humans. It is a supply throttle, not a detector — no false-positive bans, no arms
race.

For a very-rare, actively-chased item the usual idle objection ("players don't watch drop rates
and relocate") is weak: people DO chase rares. Surfacing the per-source bonus (see Transparency)
turns the throttle into voluntary redistribution.

## Population measurement

Hourly window, inferred from saves — no real-time presence needed:
- Count distinct accounts whose most recent verified save within the last hour had a farm
  target on that source.
- Recompute `m(N)` per source at the top of each hour; cache; apply to every server roll until
  the next recompute.

The one-hour granularity is deliberate: cheap, smooth, and immune to per-tick gaming.

## Exploit surface + mitigations

1. **Spread-sybil.** If `m` is purely PER-SOURCE, a bot farm distributes accounts across many
   sources so each stays low-`N` (high rate), dodging the penalty.
   - Mitigation A: define the crowd domain **per rare-ITEM** (aggregate `N` across every source
     that drops that item), not per physical map — spreading no longer helps.
   - Mitigation B (belt-and-suspenders): pair with the **per-account lifetime rare cap** from
     `competition-integrity.md` (layer-2 cumulative-excess). Crowd-scaling caps supply per
     source; the account cap caps supply per identity. Together they bound both axes.
2. **Hide from the count.** A bot could avoid syncing so it doesn't inflate `N` and keep the
   rate high — but a rare only becomes REAL on a verified save (settlement invariant,
   `economy-invariants.md`). No sync → no settled rare → nothing to sell. **Hiding and
   harvesting are mutually exclusive.** Closed for free.
3. **Off-peak timing.** Farming at low-CCU hours yields a higher rate. This is acceptable (it's
   real supply smoothing, not an exploit) and self-corrects: if off-peak becomes known-good,
   more farmers arrive → `N` rises → the edge closes.

## Transparency — decision needed

- **Show** the per-source (or per-item) modifier ("this source: rare ×1.4, low population") →
  legit players redistribute voluntarily, healthier market, but min-maxers chase it.
- **Hide** it → less gaming, but no voluntary spread.

Leaning SHOW: voluntary redistribution is a feature here, and it reads as an honest,
RS-Grand-Exchange-style signal rather than a hidden nerf (avoids the "why did my rate silently
drop" resentment).

## Integration

- Reuses the deployed server-roll infra (`refine-server-roll-2b.md`, `server-roll-drops-
  design.md`): `m(N)` is one more factor in the server's roll, computed from a new per-source
  hourly aggregation of recent saves.
- Pairs with `competition-integrity.md` layer-2 (per-account cumulative-excess) against
  spread-sybil.
- Fits the tradeable-market endgame (`steam-market-banana-vision`, `s3-economy-design.md`):
  bounding rare supply is a prerequisite for a healthy real-value market.

## Open decisions (before build)

1. Crowd domain: per-map vs per-rare-item aggregate (recommend per-item — kills spread-sybil).
2. Curve: hard `1/N` vs soft `1/√N` vs the general clamped form (recommend start soft, tighten
   with data).
3. `m_min` / `m_max` bounds (never fully dry; empty-source bonus ceiling).
4. Transparency: show vs hide (leaning show).
5. Whether to ship BEFORE the tradeable market opens (recommend yes — it's a supply-side
   prerequisite, cheaper to have the cap in place before rares carry real value).

## Status

DESIGN, not built. No determinism impact by construction (server-roll only, hourly). Depends on
`server-roll-drops-design.md` being live for the affected items. Build slice: (1) per-source/
item hourly aggregation of recent farm targets → `N`, (2) `m(N)` applied in the server roll,
(3) optional transparency surface in the client. Bless nothing in sim-core (no sim change).
