# Anti-Cheat + Economy Strategy v2 (improved)

> Builds on `~/Downloads/anti-cheat-economy-strategy.md`. The original spine
> is sound — this version keeps it and **hardens the 5 weak points** that
> matter specifically for an **idle** game with a real-money-ish market.
> Unchanged core: win at the cash-out gate; reversibility decides verify
> depth; ship in-game economy first; rarity-cap the free tier; License =
> costed + rate-limited + revocable; deterministic core → cheap batch
> verify. Those stand. Below = the deltas.

---

## Weak point 1 — "proof-of-play as bot friction" is weak in IDLE games
**Problem:** in an idle/auto-battler, a bot that just leaves the client
running *is* "playing." Account-age/playtime gates are cheap for idle bots
to grind. So gameplay-as-friction can't be the load-bearing defense.

**Fix A — Move the Turing test to the cash-out door, not the farm.**
Stop trying to prove a *human is playing* (lost cause in idle). Instead,
prove a *human is withdrawing*: the **Merchant License** (the only path to
real-money-ish value) requires periodic human/trust signals — Steam account
standing + age + non-trivial Steam purchase history, light liveness/KYC at
license issuance and re-attested on a schedule. Farming stays open; the
**exit is human-gated**.

**Fix B — Exploit determinism for behavioral fingerprinting (free signal).**
The sim is deterministic + event-sourced, so the server has the **entire
input/decision stream** per account. Bot farms are statistically obvious in
it: near-identical input-timing entropy, lockstep progression curves,
low build-diversity across "different" accounts, synchronized session
windows, identical loot-reaction patterns. Cluster on these → flag farm
rings. This is **detection telemetry most games don't have** — the
deterministic design hands it to you for free. Use it to drive License
revocation + escrow clawback (below), not to block play.

**Fix C — Per-account marginal cost that doesn't amortize.**
Sybil dies when each extra account costs real, non-recoverable resource.
Stack: Valve's $5 trade-unlock (one-time, but real) + License issuance cost
+ a **stake** (refundable deposit slashed on fraud). N burner accounts now
cost N × (>$5 + stake) up front, recovered only by clean behavior.

---

## Weak point 2 — License economics + detection LATENCY is fragile
**Problem:** "revocable" only helps if you catch them *before* they extract.
Bots race to withdraw before revocation.

**Fix A — Settlement escrow (defeats latency directly).**
Promoting an item across the irreversible boundary doesn't pay out
instantly. It enters an **escrow / holding window** (e.g. 7–14 days) during
which fraud detection (WP1-B signals) runs. Value **settles** only after the
window. Detected fraud during escrow → **clawback** (the item/credit is
reversible *until settled*). This converts "detection must be real-time"
into "detection must be faster than the escrow window" — a far easier bar.

**Fix B — Make the License break-even slower than the bubble window.**
Design so a license can't pay for itself before it's likely caught:

```
expected extractable value before settlement
  = daily_cap × avg_settled_value × escrow_days
must be <  license_cost + stake + farm_cost
```

Tune `daily_cap`, `escrow_days`, `license_cost`, `stake` so the inequality
holds even at *zero* detection. Detection then becomes upside, not the only
wall. (This is the load-bearing formula — pin it and stress-test it.)

**Fix C — Stake slashing aligns incentives.**
License holders post a stake; confirmed fraud slashes it (funds the
clawback + a bounty pool). Honest sellers get it back. Burner-license
farming now bleeds capital on every caught ring.

---

## Weak point 3 — Steam Wallet ≠ withdrawable cash (be honest, then weaponize it)
**Reality:** Steam Market proceeds are **Steam Wallet credit** — spendable
only inside Steam, **not** cashable to a bank for normal users. The original
doc treats market value ≈ real money; it's softer than that.

**Reframe the hook honestly:** *"earn tradeable value you can spend across
Steam,"* not *"withdraw cash."* This is both honest and a **defense
multiplier**: the built-in non-cashability already throttles pure
cash-extraction bots and keeps you clear of money-transmitter / gambling
regulatory traps. Lean into it as a *feature* (safety + legitimacy).

**For real studio revenue** (separate from the player economy): sell
**cosmetics / season passes / the Merchant License itself** for real money.
Keep the *tradeable-item* economy Wallet-bound (safer); make studio income a
clean, conventional storefront. Don't entangle the two ledgers.

---

## Weak point 4 — rarity-cap forces a near-zero free floor (design it in)
**Constraint (accept it):** bots optimize the highest free-tradable tier, so
that tier's market value **must stay ~negligible** or it leaks at scale
(value × thousands of accounts).

**Fix — sinks pull value UP and OUT of the floor.**
- **Crafting/upgrade sinks consume commons/uncommons** (combine → chance at
  rare). This drains the free-tradable floor as raw supply and converts it
  into a sink, keeping floor value pinned near zero *by design*, not by hope.
- Rare value is then **sink-supported** (rares are inputs to refine/enchant/
  socket), not pure speculation — gives the market a utility backbone.

---

## Weak point 5 — economies bubble even WITH gameplay (CS:GO skins)
**Problem:** real gameplay fixes "no-gameplay speculation," but a tradeable-
rare market can still boom/bust on collectible dynamics.

**Fix — faucet/sink balance + emission throttle as first-class systems.**
- **Mint-rate caps on rares** (deterministic, server-seeded): supply can't be
  botted to infinity; emission is a tunable tap, not a function of bot count.
- **Persistent sinks** (gear destruction on refine fail, consumables,
  re-roll costs) so supply has a drain matching the faucet.
- **Track faucet:sink ratio as a live ops metric**; throttle emission /
  adjust sinks when it drifts. Treat the economy like a central bank, not a
  fire-and-forget drop table.

---

## What's already DONE in this project (positioning)
The hard "spine" the strategy depends on is largely **already built or
chosen**: deterministic, event-sourced **Rust sim-core** (the future
verifier), replay-on-demand server model (`docs/anti-cheat-and-server-
model.md`), integer-only golden-tested combat. The economy/anti-cheat layer
(run-chain, checkpoints, ledger, License, escrow) is **correctly deferred to
Phase 2–3** — *after* the single-player game proves fun. None of this is in
the depa S1 scope (SP slice only).

## Updated decision checklist (v2 additions in **bold**)
- [ ] sim-core deterministic + loot RNG server-seeded in the same sim
- [ ] run-chain + checkpoint + single transactional ledger
- [ ] compiled verifier, separate from any scripting/economy sim
- [ ] DB writes batched, never per-run
- [ ] in-game economy first; Steam Market closed/narrow at launch
- [ ] Merchant License = costed + rate-limited + revocable **+ staked**
- [ ] free tier capped by **rarity** (minted, deterministic), rare+ → License
- [ ] cap lives on the Steam-promotion gate only (in-game stays reversible)
- [ ] **Turing test at cash-out, NOT at play (idle = play is passive)**
- [ ] **settlement escrow + clawback window > detection latency**
- [ ] **License break-even formula holds at zero detection (pin & stress-test)**
- [ ] **behavioral fingerprinting off the deterministic input stream**
- [ ] **crafting sinks consume the free-tradable floor → floor value ~0**
- [ ] **rare mint-rate cap + persistent sinks; track faucet:sink live**
- [ ] **value prop honest: "tradeable Steam value", not "cash"; real revenue = cosmetics/pass/License**

---

### One-line summary of the upgrade
*Don't gate **play** (impossible in idle) — gate the **exit**: human-verified
+ staked License, a settlement **escrow** that makes detection-latency
survivable, and behavioral detection the deterministic engine gives you for
free; keep the floor worthless by **sinking** it and the rares
**utility-backed**, with emission throttled like a central bank.*
