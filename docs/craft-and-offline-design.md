# Craft skill + offline model

DESIGN (not built), settled 2026-07-10. Two interlocking systems: a soul-gated CRAFT skill
(the production side of the economy) and the OFFLINE catch-up model (what an idle player accrues
while away). They share one governing line — the money economy stays real-time; progression and
conversion may run offline (capped). Reads on `monetary-policy.md` (the sink/faucet framework
this serves), `auction-house-design` + `party-comp-is-the-build` + `semi-mmo-direction` memories.

## Part 1 — Craft skill

### Home: the craft screen, gated by soul

Crafting levels are bought with **soul** (the scarce, party-shared, wipe-persistent meta
currency) but the UI lives on **craft_view**, NOT inside the Soul Path web. Rationale: the path
web is already complex; keeping craft progression on its own simple screen is legible (one
system, one place). Soul is still the currency, so the opportunity cost (soul → craft vs soul →
combat paths) is preserved — that shared scarce budget is what keeps crafters a SPECIALIZATION
and sustains a finished-goods market. Levelling by *spending soul* (investment), not by *earning
craft XP* (activity), was chosen for simplicity; a craft-XP mastery layer can be added later.

### Two-layer gating (keep these separate)

| Layer | Paid with | Governs |
|---|---|---|
| **Recipe / craft LEVEL** (capability) | **soul** (+ a one-time Gil fee) | which recipe tiers you can make, yield %, success % |
| **Each craft** (output) | **materials** (+ time) | how much you actually produce |

Soul unlocks *what you can make*; materials-per-craft are *the production cost* — the conveyor /
sink from `monetary-policy.md`. Never merge them.

### Worked example — Speed Potion I

- **Learn:** 100,000 soul + 10,000 Gil (one-time). Soul is the real gate (10k Gil is trivial);
  keep tier-I learn costs ACCESSIBLE so the crafter population isn't thin, and reserve big soul
  costs for high tiers / high-yield mastery.
- **Per craft:** e.g. `3 Red Herb + 1 Glass Flask` + **10 min** craft time → 1 potion.
- **Buff:** +30% for **30 min**.

### Background queue (the craft-time model)

Materials go in, the player walks away, potions accrue at the craft rate — crafting runs in the
BACKGROUND alongside farming (no need to stop farming). So the 10-min craft time is a **rate
cap**, not a farm-time opportunity cost.

**Why the numbers balance:** a 30-min buff needs ~2 potions/hour to keep permanent uptime; a
10-min craft makes 6/hour → **~4/hour surplus to sell** + a per-account production ceiling of
6/hour (anti-flood / anti-bot mass-production). Materials are the natural limiter — the queue
stops when they run out, so no separate queue cap is needed (materials ARE the cap), and it keeps
the material market (the conveyor) flowing.

Because background crafting has no farm-time cost, the earlier "benefit must exceed craft-time"
concern relaxes to "benefit must exceed material + purchase cost" — easily met at +30%/30min.

### Specialization comes from the soul gate, not craft time

With background crafting removing the time cost, the thing that stops everyone from self-crafting
(and killing the finished-goods market) is the **soul-gated recipe**: a player who spent soul on
combat paths instead of learning the recipe cannot make it → they buy. The gate is KNOWLEDGE
(soul), not time.

### Trap-1 guard — a speed potion is a farm-speed buff

A haste potion speeds Gil farming → a faucet amplifier (`monetary-policy.md` Trap 1), and
background crafting removed the craft-time damper. Keep it clean by EITHER:
- have the potion boost a **NON-Gil** reward (drop rate / XP) instead of kill-Gil — cleanest, no
  pump; OR
- include a **Gil-cost reagent** in the recipe, tier-scaled so its cost tracks the band's Gil
  rate (a flat reagent becomes a pump at high level).

Recommendation: tier-I "Speed Potion" boosts DROP/material yield (drives the material economy,
no Gil pump). Reserve attack-speed buffs for a tier that carries the tier-scaled Gil reagent.

## Part 2 — the offline model

### The governing line

> **The money economy is real-time-only. Progression and conversion may run offline (capped).**
>
> Test for anything offline: *"does it mint Gil or create tradeable raw value?"* → YES =
> online-only. NO (personal progression / conversion of stuff you already own) = offline-OK,
> capped.

This preserves the real-time faucet cap (`monetary-policy.md`, `semi-mmo-direction`) — the reason
inflation is controllable — while still giving idle players offline progress.

| While offline (capped) | Allowed? |
|---|---|
| Gil income | ❌ no |
| Item drops (raw value) | ❌ no |
| **EXP** (personal progression) | ✅ yes — rate-based |
| **Craft** (material → potion conversion) | ✅ yes — material-limited, 8h cap |

### Offline EXP — rate-based

- On resume you receive EXP = `avg_xp_rate × min(elapsed, CAP)`, no Gil and no drops.
- `avg_xp_rate` is a **smoothed average over a recent window** (e.g. last 30–60 min), NEVER the
  instantaneous rate — otherwise a player farms a high-XP burst right before logout to inflate
  the offline payout.
- CAP: to decide (8h like craft, or its own value).
- Not inflationary (EXP isn't money / isn't tradeable). Second-order note: a higher level farms
  Gil faster on return, but the online money faucet is still real-time-bound, so the cap holds.

### Offline craft — conversion only, 8h cap

- Consumes materials you already hold → produces potions. Mints NO Gil, creates NO raw materials.
- Runs up to **8h** offline; actual output = `min(8h, materials available)` — materials are the
  real limit.
- Must advance the CRAFT subsystem only — never combat/drops/Gil (else it becomes offline
  farming). Keep craft-ticks distinct from combat-ticks.

### Verification (reuses anti-fast-forward infra)

sim-core has no wall-clock, so offline duration enters as a **Server-authority InputEvent**, a
DISCRETE catch-up (not tick-by-tick sim): `OfflineExp{ticks}` and `OfflineCraft{ticks}`. The sim
applies the closed-form result once (EXP = rate×ticks; potions = floor(ticks/craft_time) +
material debit) with NO RNG. The server verifies `ticks ≤ real_elapsed_since_last_save` (capped),
exactly like the existing `within_tick_rate` anti-fast-forward check. Replay stays deterministic
(the verified duration replays identically). This is the offline catch-up path deliberately
skipped for combat — budget for it, but the caps bound it.

## Determinism / architecture

- `craft_level` / learned recipes live in `GameState` (party-shared, like soul). Spending soul to
  learn/level = an `InputEvent` (like AllocateStatPoint / soul-path alloc) → server-verifiable.
- Background craft progress advances in `tick()` deterministically (integer time, material debit).
- Offline catch-up = the two Server-authority events above, verified server-side.
- No floats, no RNG in the offline path.

## Open decisions

1. Offline EXP cap (8h vs other) and the smoothing window length.
2. Speed Potion tier-I effect: drop/XP boost (recommended) vs attack-speed + tier-scaled Gil
   reagent.
3. Soul cost curve per recipe tier (cheap entry, expensive mastery).
4. Soul respec: does it exist? Needed so players can rebalance combat ↔ craft (soul is shared +
   wipe-persistent). If not, craft branch must be additive enough not to cripple combat.
5. Whether background craft occupies anything at all, or is purely passive alongside farming
   (current assumption: passive alongside, gated by soul + materials).

## Status

DESIGN, not built. No sim schema change is forced by the framework; building it adds
`craft_level` + recipe state + the two offline catch-up events + their server verification.
