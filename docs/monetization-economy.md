# Monetization & Economy — one-pager

Status: **PARTLY SUPERSEDED** (reviewed 2026-08-01). Written 2026-07-14; two of its
foundations changed afterwards and the affected passages are corrected in place below:

1. **Exemplars are BOUND.** `gil_tradeable: false, steam_tradeable: false` in `game_data.json`
   for every Exemplar — there is no resale of collectibles, so the trade-lock lifecycle and the
   ~10% resale-fee revenue tier described here no longer exist.
2. **The model sells random rolls.** A gacha with banners and a 4% SSR rate is built
   (`crates/verify-server/src/gacha.rs`, `docs/gacha-economy-design.md`), which is the opposite
   of the "never sells a random roll" line this doc opened with.

**`docs/gacha-economy-design.md` is the authority on the current model** (crystal ladder, passes,
pull prices, banner rates). What is still live here: the audience/positioning framing, the
pay-for-power stance, and the revenue funnel — see the note under it for which slice moved.

Related: [[exemplar-resonance-system]], [[store-copy-positioning]], [[steam-market-banana-vision]],
[[auction-house-design]], [[monetary-policy]], `docs/economy-invariants.md` (safety spine).

## Model

F2P live-service for the RO/SEA audience. Players may buy meaningful power and progression
directly; this is an intentional pay-to-win / pay-to-progress model, not cosmetic-only framing.

~~Every paid SKU is a known, fixed outcome: monetization never sells a random roll.~~
**Superseded 2026-08-01** — the primary channel is now a GACHA (banners, 4% SSR flat, Summon
Shards as a parallel faucet). Direct-purchase SKUs still exist beside it. The judge/store-facing
LANGUAGE rule is unchanged and separate from this (see `store-copy-positioning`): avoid gambling
vocabulary in outward copy, but internal docs must describe the model as it is.

Revenue comes from gacha pulls, power-bearing collectibles, season passes, and cosmetics.
A player-owned secondary market (Steam Market) still exists for FARMED/dropped goods — gated by
an active pass on the sell side (`gacha-economy-design.md`) — but **collectibles no longer enter
it**.

### Priced SKUs
- **Exemplar** — $8.99. Collectible character with a skin, stat overlay, and skill overlay;
  server entitlement.
- **Signature Regalia** — $4.99. Premium gear with gameplay stats and a signature attack
  animation.
- **Season Pass** — $9.99 × ~6/year.
- Bundles / cheaper cosmetics fill lower price points.

### Lifecycle of a limited drop (per SKU)
1. **Sell window** — 1st-party sale, unlimited supply, fixed price, **trade-LOCKED**.
2. **Supply freezes** when the window closes (no rerun — see below).
3. ~~**2-month trade-lock** (per-purchase, from purchase date).~~
4. ~~**Secondary market opens** — players resell on Steam Market; we take ~10% dev fee on resales.~~

   **Superseded 2026-08-01: steps 3-4 do not happen.** Exemplars and Regalia are bound on
   acquisition and never become tradeable, so there is no trade-lock to wait out and no resale
   fee to collect. A purchase is terminal: the buyer keeps it, nobody resells it.

- ~~**No rerun, ever.**~~ ~~Player-facing phrasing: *"Limited means limited — we never
  re-release, so your collection keeps its value."*~~

  **DEAD RULE — superseded 2026-08-01.** Both of its justifications are gone and the current
  model actively depends on the opposite:
  - the original reason (protecting secondary-market prices) died with tradeability;
  - the replacement reason ("a promise to the buyer") does not survive contact with
    `gacha-economy-design.md`, which states that expensive SSRs **must become PERMANENT
    eventually** — "rerun tail amortizes $2,000 over years" — that the ~20-day banner rhythm is
    **rerun-padded**, and that the queued Limited SSRs are explicitly marked *"rerun later"*.

  Reruns are not a concession here, they are how a $2,000 unit pays for itself and how cadence
  is sustained without shipping 18 new SSRs a year.

  **Limited now means a limited WINDOW, not permanent exclusivity.** What a buyer gets is early
  access and the window, not "never again".

  ⚠️ **Do not ship the old phrasing.** "We never re-release" is a promise this model cannot keep;
  publishing it and then running a rerun is a broken commitment to paying customers, not a
  marketing tweak. Any store copy on scarcity must describe the WINDOW.
- **Judge-facing framing:** avoid "gambling / loot-box / gacha / FOMO" — not to hide anything, but
  because they are **factually inaccurate** for this model (see "Why it's not gambling" below). Use
  precise terms: *direct-sale seasonal collectibles, tradeable on Steam's standard marketplace* — a
  **common, mainstream model** (CS:GO, Dota 2, TF2, and thousands of Steam titles). Keep the internal
  scarcity/"value-retention" language OUT of judge-facing materials.
- Trade-lock also amplifies scarcity (item is unobtainable on the market during the lock).
- Dupe-proof: enforced server-side + 100% replay-verify (inventory→escrow→mail intact). See
  [[item-trade-design]].

## Store layout — tiered (decided 2026-07-14)

**Problem:** 1 item/month → store feels empty + narrow taste coverage (a player who dislikes this
month's character has nothing to buy).

**Constraint:** **art capacity is the bottleneck** for a small team — the store's size must match
what art can produce per cycle. "No rerun" means variety must come from NEW items only (can't do
the Fortnite rotate-back model).

**Chosen model — tiered store, always 2-3 items live:**
- **1 "Spotlight"** on a SHORT window = the FOMO hero that drives the cycle's revenue.
- **+ 1-2 "still available this season"** on longer/overlapping windows = variety, keeps the store
  full. (Staggered windows let 1-new-per-month still show 2-3 at any time — fuller store at
  1/month art load.)
- Spread across **archetypes** (knight / mage / rogue), power roles, and **price points**
  (Exemplar $8.99 / Regalia $4.99 / bundle / cheap cosmetic) → something for every taste,
  build, and budget.

Tradeoff to manage: short window = strong FOMO but sparse store; long/staggered = full store but
weaker per-item FOMO. The tiered split gets both (spotlight keeps urgency, season items keep it
full).

## Revenue model (SEA-adjusted, 3 tiers — matches deck slide 13)
Player base & paying % held constant across tiers; only ARPPU varies (SEA-adjusted, modeled
monthly). Figures gross, before Steam's 30% cut.

| Tier | Players | Paying | Payers | ARPPU/yr (· /mo) | Year-1 gross | vs prev |
|---|---|---|---|---|---|---|
| Conservative | 200k | 1% | 2,000 | ฿2,600 · ฿217/mo | ~฿5.2M | 5.2× |
| **Base (target)** | 500k | 2% | 10,000 | ฿2,600 · ฿217/mo | **~฿26M** | 4.3× |
| Optimistic | 1M | 3% | 30,000 | ฿2,600 · ฿217/mo | ~฿78M | 4.3× |

**ARPPU is FLAT across tiers** (we vary only reach + conversion, not per-payer spend) — this kills
the "ARPPU rising with scale is backwards / double-counting optimism" critique. Real F2P ARPPU tends
to *fall* with scale, so a flat ฿2,600 is already slightly optimistic, not aggressive.

- ARPPU is a **weighted blend**, not typical spend: ~70% pay just the battle pass (~฿125/mo),
  ~25% buy a few collectibles, ~5% collectors buy multiples — the tail lifts the average. Still
  **below gacha benchmarks** ($150–400/payer/yr).
- Uplift vs the prev pass-only model = a **new revenue tier** (collectibles + gacha), NOT a
  rosier forecast of the same tier. ~~+ ~10% market fee~~ — removed 2026-08-01 (collectibles are
  bound). The ฿26M figure is ARPPU-driven and never had the fee as a separate line, so the total
  stands; what changed is that one of the two justifications for the tier is gone and the gacha
  replaces it.
- Pitch anchors **Base ฿26M**; optimistic = clearly-labeled upside.
- ⚠️ Known soft spots (raised in review): player-base is the riskiest assumption. ~~collectible
  value + 10% fee both need a liquid secondary market…~~ — **no longer applies to collectibles**
  (bound since 2026-08-01), so the chicken-and-egg risk now only covers the FARMED-goods market,
  which was never in the ฿26M figure. Net effect: this risk shrank; the player-base assumption
  did not.

## Revenue funnel (Base — bottom-up, derived not asserted)
Year 1, **post-launch** (not within the 6-month grant):
1. Cumulative F2P installs: **~500,000** ← the single variable revenue is most sensitive to (linear).
2. Paying conversion **2%** → **10,000 payers** (PC F2P norm is 1–5%).
3. Blended ARPPU/yr **฿2,600** = 70%×฿1,500 (pass-only) + 25%×฿4,200 + 5%×฿10,000 (collectors).
4. **10,000 × ฿2,600 = ~฿26M**, gross (before Steam's 30%).

Own the sensitivity out loud: revenue scales **linearly with reach**; reach comes from the GTM
flywheel (influencer + Steam charts + F2P low friction). Say plainly which number it hinges on
(installs) — that reads as founders who know their model, not a top-down guess.

## Why it's not gambling / a loot box (the real, legal distinction)
- **No randomness.** Loot boxes = pay for a RANDOM outcome — *that* is the mechanic regulators treat
  as gambling. ROI collectibles are a **direct purchase of a known item at a fixed price** — you see
  exactly what you get. **Zero RNG purchases anywhere.**
- Third-party validated: **Rated Teen** + passed Steam's content survey (Steam flags RNG/loot-box +
  gambling; passing = clean).
- The **Steam Market is Valve's own standard system**, used by thousands of games — not a novel
  mechanism we invented.
- We deliberately avoid gacha/RNG monetization — a design choice that keeps us compliant (Teen) and
  trusted by an RO/PoE audience that dislikes gacha. → **This is a strength to state, not a risk to hide.**

## Pitch discipline
For the 5-min depa pitch: revenue model + Exemplar/Regalia concept is enough. If asked "how does
the store work / how do you keep players spending" → *tiered store: a spotlight plus seasonal
variety, diverse archetypes, power roles, and price points.* Use precise terms: *direct-sale
seasonal power collectibles on Steam's marketplace*. Do not describe the model as cosmetic-only or
claim that power is not sold. Avoid "gambling/loot-box/gacha" because those labels are factually
inaccurate for a no-RNG purchase model, not because they are taboo.
