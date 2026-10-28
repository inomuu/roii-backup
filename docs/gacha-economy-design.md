# Gacha Economy Design

> **Status: CALIBRATION DRAFT · second-launch scope · err merciful.**
> All numbers are starting points to tune LIVE post-launch. Server/monetization
> layer — does NOT change the sim (Resonance used as built 0.0.199, flat 1
> dupe/star). Core ships first. Living doc — keep editing.
>
> **This doc is the NUMBERS and the WHY.** For what is actually built — endpoints, tables,
> transaction shape, tests, how to change things — see **`gacha-system-reference.md`**.

## Model in one line

Exemplars (characters) via **gacha**, **bound**, and **productive assets** — buy one,
farm sellable items, recoup part of the cost. P2W embraced (`p2w-defensible-framing`):
real money buys power, kept fair by PvE-beatable / PvP-power-matched / F2P free-via-market.
The player MARKET (tradeable premium currency, Warframe-style) is the whale-depth engine
AND — critically — the farm-to-sell economy is the acquisition HOOK that drives the base to
100k+ active (RO/OSRS/PoE genre). No market = no hook = no players.

Terminology (never conflate): **Rarity = R / SR / SSR** · **Resonance = S0–S5** (dupe/"stars") ·
**Enhancement Level** (fed by R-rarity FODDER, weapon banner — see below). ✅ RESOLVED 2026-07-30: the Resonance
track is **S0–S5** (HSR-style Superimposition). Rarity keeps the genre-standard R/SR/SSR ladder.
The code field is `resonance_stars` and needs no change.

## Currency — token ("crystal")

- **1 pull = 300 token** (NIKKE-style big denomination → income "feels" generous, big daily numbers).
- **Effective ~$1.85/pull** at sticker, **~$0.86/pull** across the first-purchase bonuses
  (range ~$1.75 whale-pack → ~$2.20 small-pack). **PRICED 2026-07-31 AT HALF OF NIKKE'S REAL
  IN-GAME RATE** (฿3,000 → 7,000 crystal = ฿129 ≈ $3.67/pull, verified against a live store price —
  NOT the cheaper third-party top-up resellers). Superseded the earlier ~$0.65, which the NIKKE
  rate change would otherwise have left 5.7× below the genre.
- **Two buckets, no trading (2026-10-01):** crystal never trade. `paid` = bought with money;
  pass / free-faucet / first-buy-bonus crystal are `bound`. Kept apart for refunds and revenue
  (invariant I11). Currency never cashes OUT to real money.

## Crystal packs (direct purchase = paid)

Bigger = better rate (whale bulk discount). Non-round amounts (leftover → nudge re-purchase).

| Price | Crystal | ≈ pulls | eff. $/pull |
|---|---|---|---|
| $1.99 | 300 | **1.0** | ~$1.99 |
| $4.99 | 790 | ~2.6 | ~$1.89 |
| $9.99 | 1,620 | ~5.4 | ~$1.85 |
| $19.99 | 3,330 | ~11.1 | ~$1.80 |
| $49.99 | 8,570 | ~28.6 | ~$1.75 |
| $99.99 | 17,850 | ~59.5 | ~$1.68 |

⚠️ **The rate must decrease monotonically down this table.** A first draft had the $1.99 entry at
$1.99/pull and the $4.99 tier at $2.20 — i.e. the entry SKU was the BEST rate, so the rational
play was to buy nothing but $1.99 packs and the whole ladder collapsed. Whenever these numbers
are re-tuned, re-check that column top to bottom.

**The $0.99 tier is gone and the entry SKU is $1.99 = exactly one pull.** At the new rate $0.99
buys 0.5 pulls — an entry pack you cannot spend is worse than no entry pack. Exactly-one-pull is
also the cleanest possible first purchase: the player converts and immediately gets the thing
they converted for.

**+ First-purchase 2× per tier (one-time)** — the biggest F2P→payer conversion lever, and at this
price level it is doing much more work than before: buying every tier once costs **$186.94 and
yields 64,920 crystal = 216 pulls ≈ $0.86/pull**. The on-ramp therefore runs at roughly half sticker
for the first ~216 pulls, which is why a first character copy is CHEAPER here than under the old
$0.65 ladder despite the sticker being 2.8× higher. First-buy bonus crystal are bound.

## Storefront shape — decide the ARCHITECTURE now, the SKU count later

The genre's storefronts are dense: rotating bundles, step-up packs, growth funds, monthly cards,
and a claim button on every surface. That density is not decoration — **the claim touchpoints are
the retention engine**, and they matter MORE for an idle game than an active one, because a game
that plays itself has a weaker built-in reason to open it.

Two different decisions live inside "should we have all that", and they carry different risk:

| | verdict |
|---|---|
| **Free claim touchpoints** (login calendar, dailies, mail gifts, mission/achievement claims, event exchange shops) | **Build these.** Cheap, high retention, no storefront risk. We are UNDER-built: the faucet is one undifferentiated 350/day number with no surface to interact with. |
| **Paid SKU proliferation** | **Ship Steam lean.** Steam has no IAP-normalised audience and reviews are our discovery channel; a dense cash shop is punished there in a way it is not on mobile. The doc's "two paid products only" stands FOR THE STEAM LAUNCH. |

**⚠️ BUT the client is mobile-shaped and mobile is a plausible future platform, so the constraint
is architectural, not numeric:** storefront entries and claimables must be SERVER-SIDE DATA, not
hand-written handlers. Then expanding to mobile-normal density is adding rows; hardcoding now
makes it a rewrite.

The codebase already has the pattern to copy — `gacha::BANNERS` is a const table whose comment
reads *"Content, not logic — extend this list, not the roll."* Crystal packs, bundles and claimables
should follow the same shape. **This is free to decide today** (packs exist only in this doc and
`/gacha/purchase` is unbuilt) and expensive to retrofit once the first purchase handler ships.

The one paid pattern worth taking early is the **drip pass** below: it produces the same
open-the-game-daily behaviour as a dense storefront while costing exactly one SKU.

## Rarity & production tiers

| Rarity | Production | ~Cost/unit | Role |
|---|---|---|---|
| **SSR** | Live2D animated + pixel + VFX + minimal voice | ~$2,000 | premium chase, animated reveal = the pull payoff |
| **SR** | **NO Live2D** — quality static art + pixel + light VFX | ~$400–700 | accessible tier, BP/free reward, cheap cadence filler |

SR art must still be GOOD (Azur Lane model). Production twin = **Arknights / Azur Lane** (not Genshin/full-3D-voice).

## Banner flow — DECIDED 2026-09-30, BUILT server-side (verify-server `gacha.rs` / `/gacha/*`)

User: "starter ก็ 50 roll สุ่ม SSR / Standard 100 roll เลือก SSR (ครั้งเดียว) + ทุก roll ได้ mileage -> แลก Memory Fragment"

- **Starter banner** (today's `newcomer`): 50 pulls, SSR guaranteed by the 50th pull counted from
  the last SSR (an early SSR means no second guarantee). Every SR pull is an SR CHARACTER the
  account does not own yet (uniform among those); once all are owned it falls back to the normal
  SR payout. SSR is always a character. The Standard banner opens only after the 50 are spent.
- **Standard banner**: own pity counter, starting at 0. After 100 Standard pulls the account picks
  ONE SSR character from the standard pool, once per account (HSR/WuWa-style selector).
- **Mileage**: 1 per pull on every banner, in TWO currencies (user 2026-09-30: "ตู้ limit แลกตัว
  อะไรก็ได้"). Limited-banner mileage buys ANY character, the featured limited unit included.
  Starter/Standard mileage buys **Memory Fragments only**, so standard pulls are never a path to a
  limited unit. 200 per exchange in both until the user says otherwise.
- **Tickets** (user 2026-09-30: "ไม่มีตั๋วธรรมดา มีแต่ตั๋ว std กับ limit"): Standard tickets
  (`gacha_coupon_std`) spend on the Starter and Standard banners only; Limited tickets
  (`gacha_coupon_limited`) on the Limited and Weapon banners only. The pre-split universal
  `gacha_coupon` is deleted outright on boot (balances and ledger; pre-launch). A crystal shortfall
  buys the banner's own kind. Standard tickets come from the newcomer daily track (10 over its first
  7 days — a daily claim, not the tutorial), party slots 3-5, the Starter pack and Sealed Caches.
- Onboarding it serves (sim-cli `--gacha-flow`): start Suzu, Lumi from the first boss, party slots
  3/4/5 at stage 6/18/24 paying 10 Starter pulls each, no free Lucielle; tutorial supplies the other
  20. Monte Carlo (100k): after 30 pulls 99.9% own ≥5 characters; after 50, median 8, every account
  ≥1 SSR. Research behind the shape: NIKKE wishlist, HSR/ZZZ 300 selector, WuWa beginner choice.

## Pools + R tier — 0.0.489 (2026-09-28, server `gacha.rs`)

> Supersedes the "No R tier" bullet below. User: the 3★ cards exist to be pulled.

- Rates on every banner: **SSR 3% · SR 10% (`SR_BP` 1,031 of the 97%) · R 87%**. The "SR in
  every 10" floor: after 9 pulls with no SR or better, the next is SSR 3% / SR 97%. `/gacha/state` discloses `sr_bp` and `sr_rate_bp`.
- **R** = one of the ten 3★ Memory Fragments (`R_POOL`).
- The six stat-only SR cards (Sunbreaker, Nightjar, Farsight, Velvet Mask, Stormglass,
  Choirlight) are deleted; `SR_CARDS` = Hoarfrost Crown, Static Charm, Emberbell, Stormglass Pin,
  Thunderhead.
- **SR**, character banners = 50/50 between the SR units (`SR_UNITS`) and the eleven SR Memory
  Fragments (`SR_CARDS`); **Memory Fragment banner** SR = the SR cards only.
- **No Dust in any pool** — Memory Dust / Radiant come from salvaging cards (sim 0.0.489).
- **Mileage** (`EXCHANGE_POOL`, 200 tickets, pick any): the six SSR units + all nine SSR Memory
  Fragments. Tests bind `SR_CARDS` / `SSR_CARDS` to the sim's rungs, so a new card cannot ship
  unpullable.

## Banners as built — 2026-09-21 (server `gacha.rs`)

> Supersedes the "4% / two banners" tables below (kept as history). User: "ทำระบบกาชา 3 ตู้ …
> ปรับเรทเป็น 3%", "SR การันตีทุก 10 จั่ว เหมือนเดิม", "(SR 97% SSR 3%)".

| banner | id | SSR | who is the SSR | pity | price |
|---|---|---|---|---|---|
| **Newcomer** | `newcomer` | 3% | uniform pick from `STANDARD_POOL` (all 5 units) | **certain by pull 50** (counted from the last SSR); open for all 50 pulls, then closed (`gacha_newcomer`; 2026-09-30) | one Standard ticket per pull (the x10 = 8 discount was removed 2026-09-30) |
| **Standard** | `standard` | 3% | uniform pick from the pool | none | 1 coupon / pull |
| **Limited** | `limited_<unit>` | 3% | pick-up unit **50/50**, no carry; a loss = uniform pick from the other four | none (mileage backs it) | 1 coupon / pull |
| **Weapon** | `weapon_<regalia>` | 3% | always the featured Regalia | none | 1 coupon / pull |

- **No R tier**: 97% of pulls are SR — `SR_POOL` = the SR fodder ×3 + **Aveline** (the first SR
  unit, 0.0.452), so a quarter of SR pulls are her; the "SR in every 10" floor still runs.
- Mileage unchanged: 1 ticket per pull on every banner, 200 buys anything in `EXCHANGE_POOL`
  (now all five units + the two Regalia).
- Copies per pull: Limited featured 1/67 + 1/200 = **1/50**; Standard any-SSR 1/33; Weapon 1/33.
- Pity keys: Newcomer `newcomer`, Standard `standard`, Limited keeps `character`, Weapon `weapon`.
- Client: `/gacha/state` banners carry `kind`, `pool`, `remaining` (Newcomer only; 0 = closed)
  and `multi_coupons` (10 on every banner since 2026-09-30); a Newcomer press past `remaining` is refused with 409.

## Character banner (pity) — HISTORY (4%, two banners)

> **LOCKED + CORRECTED 2026-07-30.** The old table said "hard pity 90, guaranteed featured, no
> 50/50" AND "~70 pulls / $45". Those cannot both be true: with a guaranteed featured at 90 the
> mean is mathematically capped at ~61 pulls, so the money column was ~20% too high for its own
> mechanic. The user then reinstated the 50/50 (hard pity guarantees an **SSR**, not the featured
> one), which moved every figure again. Numbers below are computed from the shipped table
> (`crates/verify-server/src/gacha.rs`) and asserted by its tests.

> **SUPERSEDED 2026-07-31 — NIKKE shape.** The soft-pity ladder above was replaced wholesale: a
> flat rate with **no pity of any kind**, and a separate **Summon Shards** currency as the second
> acquisition track. The old table survives in `Rates` as data (`soft`/`hard`/`ramp_bp` = 0
> disables it) and in the tests, so reverting is a two-constant edit.

| Parameter | Value |
|---|---|
| SSR rate | **4% flat** — identical on every pull, forever |
| Soft / hard pity | **NONE** |
| Featured split | **50/50, plain** — losing it arms nothing (shards are the backstop instead) |
| Pulls per SSR | **~25** |
| Avg pulls to a FEATURED copy, rolls alone | ~50 |
| **Avg pulls to a FEATURED copy, incl. shards** | **~40** (1/50 + 1/200) |
| **Cost per copy** | 12,000 token ≈ **$35 first unit** / $74 steady state |
| Worst case | unbounded on the rolls; **200 pulls guarantees the unit via shards** |

**The generosity is the visible rate.** 4% is ~1.4 SSR/month for a F2P on our faucet against 0.64
under the old 1% ladder — that ratio, not the price, is what players perceive as merciful. The
cost is variance: with no soft pity the tail is genuinely long (p90 ≈ 114 pulls on the featured
unit), which is exactly what the shard track exists to bound.

### Summon Shards (mileage) — a parallel faucet, NOT pity

**1 shard per pull, on every banner, into one shared account-wide pool. Never expires. 200 buys
any unit in the exchange pool, including one no banner currently features.**

The modelling trap, and the reason this is called out in a box: read as a pity ceiling ("you get
the unit if you reach 200 without it") it only pays the unlucky and the banner prices at ~50
pulls/copy. Read correctly — a flat faucet that pays **every** player regardless of what their
rolls produced — the two sources ADD: 1/50 + 1/200 = **1 copy per 40 pulls**. That is a 45% cut
in the full-clear ceiling, not a rounding error. Asserted by
`mileage_pays_lucky_players_too_so_the_sources_add`.

Deliberately **more portable than NIKKE's**, which splits tickets into two non-fungible tracks.
Ours is one pool: a pull spent on a banner you only half-want is still real progress toward the
one you do.

## Weapon banner (separate slot)

Own banner + own slot, separate from normal equipment (a signature-weapon OVERLAY — sim/gameplay
design in `regalia-overlay-design.md`). **TWO power axes: (1) Enhancement LEVEL — fed by consuming
R-rarity Regalia as FODDER (+ a Gil cost) → scales the main stat/flat to full; (2) Resonance stars
(dupes) → scales the passive.** **NOT** ore-refined (the craft/refine ore stays a dropped/crafted-
gear sink only). **R-rarity Regalia are FODDER** (an R pull is never worthless = level EXP + a
material/Gil sink).

**LOCKED 2026-07-30 — and this banner is the honest one.** Genshin's weapon banner is its most
predatory (fate points on top of a 50/50); ours inverts that: **the character banner is the
gamble, the weapon banner never cheats you.** That is the positioning line replacing "no 50/50".

| Parameter | Value |
|---|---|
| SSR rate | **4% flat** — same table as the character banner |
| Soft / hard pity | **NONE** |
| Featured split | **NONE — every SSR is the featured weapon** |
| Avg pulls per copy, rolls alone | ~25 |
| **Avg pulls per copy, incl. shards** | **~22** ≈ **$19 first unit** / $41 steady state |
| Worst case | unbounded on the rolls; 200 pulls guarantees it via shards |

Note the old 3%/55/70 ladder already averaged ~28 pulls — within 3 of the flat 4%. **This banner
was effectively NIKKE-shaped before the change**; only the character banner actually moved.

Shards are ONE pool across both banners, so they cannot be spent twice — a player choosing to
bank them toward a character is choosing not to bank them toward a weapon. That is what keeps the
two axes from compounding into a much larger discount than intended.

## Resonance (S0–S5)

Flat **1 dupe/star, S0→S5** (built 0.0.199, unchanged). Depth from stacking char + weapon tracks, not escalating cost. SR maxes far cheaper (accessible dolphin goal).

**A dupe is never dead, even past S5** — an extra copy is its own carrier and can be equipped by
a DIFFERENT party member (already true in code: `equip_exemplar` only clears the carrier held by
that entity). So 6 copies is a real choice — S5 on one member, or S0 across six — which is the
"party comp is the build" pillar showing up in the monetization. This also softens the 50/50 far
more than in Genshin, where an off-banner dupe past C6 is pure dust.

## Milestone cost (SSR)

**THE ~15% REBATE IS RETIRED.** It never had a mechanism: it was "R-fodder + shards returned by
pulling", and the shard/dispenser half was itself an unresolved open item, so every "effective"
figure was discounting against something that did not exist. Shards now ARE the value-return
mechanism, and they are counted in the pull economics above rather than bolted on afterwards.
There is one column now, and it is real.

**PRICED 2026-07-31 at half of NIKKE's real in-game rate: $1.85/pull sticker.** Two regimes
matter, because the first-purchase 2× bonuses cover the first ~216 pulls at ~$0.86:

| Target | pulls | token | **first unit** (w/ first-buy 2×) | steady state ($1.85) |
|---|---|---|---|---|
| Character, 1 copy (avg) | ~40 | 12,000 | **~$35** | ~$74 |
| Weapon, 1 copy (avg) | ~22 | 6,600 | ~$19 | ~$41 |
| **Character S5** (6 copies) | ~240 | 72,000 | ~$228 | ~$444 |
| **Weapon S5** (6 copies) | ~132 | 39,600 | ~$114 | ~$244 |
| **Full max 1 SSR** (char S5 + weapon S5, shards spent optimally) | ~360 | 108,000 | **~$438** | ~$666 |

**This lands on the old ladder's economics from both ends at once:** the on-ramp gets CHEAPER
($53 → $35 for a first character copy) while the ceiling holds ($429 → $438, +2%). That is the
whole reason for pricing at half-NIKKE rather than at the $1.19 "revenue parity" figure — parity
on the ceiling alone would have left the on-ramp at $48, worse than today.

The structure this produces is deliberate: **your first unit is cheap, every unit after it is
full price** (~$666), because the first-purchase bonuses are one-time. Conversion is subsidised;
depth is not.

### Why not stay at $0.65

**NIKKE's real in-game price, ฿3,000 → 7,000 crystal (verified 2026-07-31): ฿129 ≈ $3.67 per pull.**
⚠️ An earlier revision cited $1.94–4.00 from top-up sites (bittopup / lootbar / topuplist) —
those are third-party RESELLERS discounting below store price, not what the game charges. Do not
benchmark against them again.

At that price its 11-copy full max is **~$1,980 for one unit**, which independently confirms the
$1,500–8,000 ceiling this doc has always cited (the upper end is chasing several units). The 4%
flat rate and the 200-ticket track are affordable to them *because* each pull costs 5.7× what
$0.65 would have. The generosity and the price are one design, and adopting the first without the
second was a 45% pay cut ($429 → $234).

**Sticker vs sticker we are 3.0× cheaper to fully max: $666 vs $1,980.** That factor decomposes,
and the two halves are different kinds of thing:

| source | factor |
|---|---|
| per-pull price ($1.85 vs $3.67) | **2.0×** — this is the pricing decision |
| pulls needed (360 vs 539) | **1.5×** — our ladder is shorter: 12 copies across two axes vs their 11 on one |

⚠️ Compare STICKER TO STICKER here. An earlier revision claimed 4.5× by applying OUR
first-purchase 2× bonus and not theirs — NIKKE has a first top-up bonus too, so the two roughly
cancel and including only one side inflates the gap by ~1.5×. The $438 first-unit figure above is
the right number for what a real player pays, but it is NOT the number to hold against a
competitor's sticker price.

**None of this touches F2P.** The faucet is denominated in tokens, so every F2P figure below is
identical at any price. Raising the sticker moves money from people already paying, not from the
players the generosity was for.

### Why HALF, and not parity

Half is a production-quality calibration, not a round number. Our SSR is a ~$2,000 unit on the
Arknights / Azur Lane production model (quality static + pixel + VFX, minimal voice); NIKKE's is
Live2D-animated with full voice at multiples of that. Charging half of a materially richer unit
is proportionate; charging parity would be selling a smaller product at the same price, and
undercutting further (the $0.65 we came from) was leaving money on the table for no positioning
gain — F2P generosity comes from the FAUCET, which price does not touch.

It also clears our own cost base with room: one $2,000 SSR is covered by ~27 single-copy buyers
at steady state, or ~5 players taking it to full max. Price viability here is not only a
competitor comparison.

⚠️ **This rationale is INTERNAL.** "Cheaper because we are smaller" is the correct way to
calibrate the number and the wrong way to sell it — see `store-copy-positioning` and
`credit-the-design-not-free`. Externally the price is justified by things we CHOSE: a 6-step
ladder instead of 11, two axes instead of one, and shards that never expire.

## Packs + passes shelf — BUILT 2026-09-29 (server `store.rs`)

> Supersedes "two paid products only" below. User 2026-09-29: launch sells the crystal ladder
> plus a starter, growth packs, a monthly card and the Battle Pass. No weekly bundle, no boosts.

| SKU | Price | Grants | Limit | Offered |
|---|---|---|---|---|
| `pack_starter` | $0.99 / ฿35 | 10 gacha coupons (10 pulls) | 1 per account | always |
| `pack_growth_10/15/20/25/30` | $1.99 / 4.99 / 9.99 / 19.99 / 29.99 (฿69 / 169 / 339 / 669 / 999) | 1,500 / 3,300 / 5,800 / 10,000 / 13,500 BOUND crystal (2.5× → ≥1.25× the ladder's first-buy rate) + BOUND Memory Dust/Crystal, Sigils, Boss Core, Track, EXP Elixir (Feast until Meals were cut 2026-10-02); Lv10 best value, each tier worse per dollar; no line may carry a Steam itemdef; more tiers to come | 1 each | Commander 10/15/20/25/30 (all inside the first month on progress CXP); stays on the shelf once unlocked |
| `monthly_card` | $2.99 / ฿99 | 300 PAID crystal now + 150 bound/day for 30 days, claim daily, missed day forfeited | at most 2 cards banked | always |
| `battle_pass` | $9.99 / ฿339 | premium track of the season running at checkout (`bp_claims.track = 1`) | 1 per season | while a season runs |

Pack items arrive as BOUND mail (`reason = store:<order>`). A refund withdraws unclaimed pack
mail, takes back unspent coupons, removes the card's 30 days and deactivates the pass; spent value
stays, like spent crystal, and the refund flag reviews it.

## Income & passes (drip = daily-login retention)

Two paid products only. Pass crystal **bound** and **dripped daily (must log in to claim; miss a day = forfeit that day)** → generous headline gated behind engagement, self-limiting margin.

| Source | token/day | token/month | = pulls | Notes |
|---|---|---|---|---|
| **F2P free** | 350 | 10,500 | 35 | daily/quest/idle/event |
| **Gacha pass $2.99** | **150** | 4,500 | 15 | drip-login; ~3.9× value; token-focused |
| **Battle pass $10** | 75 | 2,250 | 7.5 | over season; **< gacha pass tokens**; + refine mats + cosmetics + coupons (Refine Coupon / Inventory Expander). **NO SR/SSR character** |

**Characters are GACHA-ONLY.** Neither pass gives an SR or SSR Exemplar — protects gacha value. SR (accessible tier) sources = **gacha (high rate) + events/shop**, never the BP. (Reverses the older `battle-pass-design` "slot 50 = Exemplar" plan.) BP/passes give tokens + materials + cosmetics + utility coupons only.

**Steam Market mint gate (sell-side):** minting an item onto the real-money Steam Market requires an ACTIVE subscription (gacha pass $2.99 OR BP $10 — either grants it). Aligns with `steam-mint-quota-redesign` (sell=PASS-gated). Design:
- **Gate SELL/mint only; BUYING is open** (max demand/liquidity).
- **Free hook stays free:** farm → sell for GIL (in-game) = free; farm → sell for REAL MONEY (Steam) = the $3/mo gate. Matches Gil-market-launch-hook / Steam-market-scale-gated split.
- **Two-layer control:** pass = WHO may mint; **quota** (rarity points + daily cap, `steam-mint-quota-redesign`) = HOW MUCH → gate stays cheap, quota handles volume.
- **$3/mo well-calibrated:** low for legit sellers, non-zero anti-bot floor (1,000-acct farm = $3,000/mo just for access → kills low-margin botting; stacks with crowd-scale + replay-verify + trade limits).
- **Synergy:** pass bundles gacha tokens + market-sell access → no-brainer for the farming base → conversion + recurring revenue from the most active players.
- **Optics:** keep the mint-gate INTERNAL, never a marketing point (pay-to-earn framing; closed-loop = no real cash-out, but frame per `store-copy-positioning`).

Per tier: F2P 350/day · +$3 pass 500/day · +$13 (pass+BP) 575/day.

Months per FEATURED character copy, at 40 pulls/copy (rolls + shards):

| Tier | pulls/month | month per copy | SSR seen/month | shards full (200) |
|---|---|---|---|---|
| **F2P** (350/day) | 35 | **~1.1** | 1.40 | 5.7 mo |
| **$2.99 pass** (500/day) | 50 | ~0.8 | 2.00 | 4.0 mo |
| **$13/mo** (pass+BP) | 58 | ~0.7 | 2.30 | 3.5 mo |
| Direct (whale) | — | $35 first copy / $74 after | — | — |

**Every one of these roughly halved** against the old ladder (F2P was 2.3 months/copy and 0.64
SSR/month). F2P chasing S5 on one character went from ~14 months to **~6.9**.

**Value prop:** $2.99 pass = an easy impulse yes + ~3.9× value if you log in daily → the F2P→payer on-ramp. BP = the "progress/craft" buyer (materials + a free SR character).

Benchmarks: Limbus/NIKKE F2P ≈ 1.5–2 mo/unit — **we are now FASTER than the benchmark**, where
the old ladder was slower than it while the doc claimed alignment (that claim was computed from a
stale 70-pull figure and was wrong even before this change). Whether being faster than the
benchmark is correct depends entirely on the SSR shipping cadence, which is a content question,
not an economy one; if new units arrive slower than ~1/month, players out-run the pipeline.
NIKKE's own faucet is 27–40 pulls/month (8–12k crystal) — i.e. essentially identical to our 35, so
the difference between the two games has never been the faucet.

## Unit economics & break-even

- SSR content ~$2,000/unit (SR ~$400–700). Steam 30%.
- **Target scale = 100k+ active** (the farm-to-sell market hook is the acquisition engine, not a scale-dependent add-on). At 100k+, per-SSR content, market liquidity, whale-depth all work.
- **Passes = recurring, banner-independent baseline** covering content cost regardless of which banner hits.
- Expensive SSRs must become **PERMANENT** eventually (rerun tail amortizes $2,000 over years).

## Cadence & content buffer

- **Perceived cadence ~20 days (Genshin-like) ≠ new-content cadence.** Fast banner rhythm is RERUN-padded.
- **First ~6 months = all-new** (no back-catalog to rerun) → most expensive + most critical → a 6-month buffer is needed.
- **New-content ~8–12 SSR/year at launch**; SR (cheap/no-Live2D) + reruns/events fill to the ~20-day feel; ramp toward ~18/yr as pop grows.
- Build FULL launch roster + months 1–2; keep months 3–6 FLEXIBLE (adapt to launch feedback).

## Launch roster (6-month buffer)

- **Non-limited SSR: 5–6** (permanent backbone) · **SR: 5–6** (accessible, cheap) · **Limited SSR: 5–6 queued** (revenue/FOMO/cadence; rerun later).
- Launch playable ~10–12; queue ~6 = ~6-month runway. Content budget ~$27k upfront.
- Ongoing = mostly LIMITED SSR (permanent pool grows ~1–2/yr).

## Borrowed mechanics

- **Limbus Dispenser/shard** ⭐ — dupe → shards → buy a SPECIFIC Exemplar (incl. old). Adapt: dupe → CHOOSE Resonance (power) OR shards (toward another unit). Kills the #1 gacha frustration.
- **Limbus daily discounted pull** — cheap daily pull; fits idle daily loop.
- **NIKKE mileage carry-over** — ✅ **ADOPTED 2026-07-31 as Summon Shards**, and made MORE
  portable than the original: one shared pool instead of NIKKE's two non-fungible tracks. Note it
  is NOT "hard-pity progress carrying" as written here — that framing is the modelling trap
  documented in the character-banner section; it is a parallel faucet that pays every player.

## Comparables (wiki-verified)

- **Arknights / Azur Lane** — production twin (Live2D + minimal voice).
- **Limbus** (small studio) — merciful; dupes give NO power (→shards). Subsidized by IP/fanbase/content we lack.
- **NIKKE** (big studio) — dupes=power, flat 4% no soft pity, 200-ticket mileage, **11 copies for
  full max** (1 + 3 limit break + 7 Core Up). Its $1,500-8,000 ceiling is a function of PRICE
  (฿3,000 → 7,000 crystal ≈ $3.5-3.9/pull IN GAME — not the cheaper reseller rates) and that
  11-step ladder, NOT of stinginess — verified 2026-07-31 against a real store price.
  ⇒ **Rates + mileage adopted; the 11-step ladder and the price were not.** Ours stays 6 steps
  across two axes, and at half their per-pull price our ceiling is ~$666 sticker against their
  ~$1,980 sticker — 3.0×, of which 2.0× is price and 1.5× is the shorter ladder.
- **Warframe** — the ONLY comparable with a player economy (tradeable currency + power trading); our market models on this. D3 RMAH = cautionary.

## Guardrails & design constraints

- P2W embraced: PvE beatable F2P, PvP power-matched, F2P path via market.
- **GACHA MUST REINFORCE THE FARM HOOK, NEVER REPLACE IT** — gacha gives characters that FARM the economy better, not end-game items that skip farming. Tradeable value comes from FARMING; Exemplars stay BOUND. If gacha power trivializes farmed gear, the acquisition hook dies.
- Odds disclosed every banner. Currency closed-loop (no cash-out). Daily trade limits + gifted/starting/pass crystal untradeable.

## Item-transfer integrity (safety-critical, before real-money market)

Dupe/atomic-transaction is the highest-stakes surface (real-money dupe = printed money). Single authority per lifecycle: **sim owns untraded items; the server LEDGER owns them once listed/traded** (never both). Item transfer = a **server-authored InputEvent** in the deterministic log (same pattern as server-roll drops/refine) → determinism preserved + ledger authoritative + no dupe. Plus: escrow-on-list (verify rejects saves claiming escrowed instances), atomic ACID trade + idempotency (survive scale-to-zero), server-assigned immutable instance_id, save-verify reconciles vs ledger. Extends `economy-invariants.md` with "item can't X" tests.

## Open / to-calibrate

- Free-faucet (350/day) vs conversion cannibalization — **now the sharper question**, since the
  NIKKE shape roughly doubled what that faucet buys (F2P 2.3 → 1.1 months per copy).
- ~~Per-pull price~~ — **RESOLVED 2026-07-31: $1.85 sticker = half of NIKKE's real in-game rate.**
  On-ramp $35/first copy (cheaper than the old ladder's $53), ceiling $438 (vs $429). Still
  3.0× cheaper than NIKKE to fully max (sticker vs sticker). `/gacha/purchase` remains unbuilt, so these are paper
  prices until the Steam txn slice lands.
- Recoup % at real population (crowd-scaling).
- Weapon banner exact rate/pity for ~$22/copy.
- ~~Dispenser/shard costs~~ — **RESOLVED 2026-07-31**: Summon Shards, 1/pull, 200/exchange, one
  pool, never expires. This also retired the phantom ~15% rebate that was discounting against it.
- Liquidity threshold gating the real-money (Steam) market vs the launch Gil market (Gil = hook, day 1; Steam = scale-gated, second launch).
- **BP recoup vehicle (DEFERRED):** BP no longer grants a character; does it keep any TRADEABLE recoup item (old self-paying-pass relied on the removed SR Exemplar), or become a pure-value pass (tokens + mats + cosmetics, non-recoup)? Merchant monthly Exemplar grant also removed — what (if anything) replaces it.

## BUILT 2026-09-21 — Sealed Cache (sim 0.0.449 / server `lootbox.rs`)

The IDEA below shipped in this shape (user: "โบนัสเสริม", "ไม่จำเป็นต้องแจกแจง เพราะไม่ใช่ของที่ซื้อได้"):

- **Drop**: item `lootbox` in the server-rolled `orbs` lane — field 200 bp (0.002%/kill ≈ one
  box every ~3 days for a full-time farmer), boss 2,000 bp. **Bound on arrival**; cannot be listed,
  mailed or sold. Only `OpenLootbox` consumes it, and the sim records nothing but a count.
- **Contents** (server nonce per open; internal tuning, NOT disclosed in-game): 10 crystal
  89.445% · 50 crystal 10% · Summon ×1 0.5% · ×10 0.05% · ×100 0.005%. EV ≈ 16 crystal/box ≈ +5
  crystal/day/player — a bonus, never a faucet.
- **Server-wide monthly quota** per coupon tier: ×100 **5** · ×10 **50** · ×1 **500**
  (`lootbox_quota (month, coupons, won)`); a win past the quota pays 50 crystal instead, silently.
  Tuned for the expected launch scale (user: ~10,000 players ≈ 100,000 opens/month): every tier
  reaches its quota about once a month, so the cap works from day one; at 1,000 players it is
  ~6 ×100 a year and ~5 ×10 a month.
- ×10 / ×100 wins post a system chat line with a serial: "Summon ×10 · #3 of 50 · 2026-09" —
  the "แคปไปอวด" moment. The cut-in / screenshot frame is client work (UI handoff).
- Every payout is BOUND (crystal → `crystal_wallet.bound`, coupons → `bound_items`), each with
  a ledger row; `every_coupon_counter_write_is_accompanied_by_a_ledger_row` lists the writer.

## IDEA (2026-09-14, superseded by the block above) — ตั๋วกาชาหายากระดับเซิร์ฟเวอร์ เพื่อให้ "แคปไปอวด"

user 2026-09-14: "แจก gacha×10 ในการฟาร์ม จำกัดมาก 10 drop/วัน ทั้งเซิฟ" + "อาจจะมี 100× แจกเดือนละครั้ง
1 drop" + **"มองว่ามันเป็น idea ให้ผู้เล่น cap หน้าจอไปอวด ว่าได้อันนี้มา"**

**เป้าคือภาพที่เอาไปโพสต์ ไม่ใช่ช่องทางรางวัล** — เกณฑ์ตัดสินจึงต่างจาก free-pull ทั่วไป:

| | ตัวเลข |
|---|---|
| ต้นทุน ×10 × 10 ใบ/วัน | 3,000 pull/เดือนทั้งเซิฟ |
| ต้นทุน ×100 × 1 ใบ/เดือน | 100 pull/เดือน |
| รวม ≈ | **~89 "คน-เดือน" ของ F2P** (F2P = 35 pull/เดือน) — **เพดานสัมบูรณ์ ไม่โตตามจำนวนผู้เล่น** |
| มูลค่าต่อหัวเมื่อเซิฟโต | 1,000 คน = 3.1 pull/เดือน · 10,000 = 0.31 · 50,000 = **0.06 (มองไม่เห็น)** |

⇒ ในฐานะ**รางวัล** มันเจือจางจนหาย แต่ในฐานะ **UGC/marketing** เพดานรวมกลายเป็นข้อดี:
**~300 โพสต์/เดือนคงที่** ไม่ว่าเซิฟจะมีคนเท่าไร และยิ่งเซิฟใหญ่ "1 ใน 10 ใบของวันนี้" ยิ่งน่าอวด
(เรามีงบ marketing ฿0 ตาม `budget-plan.md` → โมเมนต์ที่คนอยากแคปเองเป็นคันโยกที่มีจริง)

**ข้อกำหนดที่มาจากเป้า "ภาพ" (ต่างจากเป้า "รางวัล")**
1. **หมายเลขซีเรียลในเฟรม** — "×100 Ticket · #1 of September" / "×10 · #3 of 10 · 14 Sep"
   คนนอกต้องเข้าใจความหายากเองจากภาพ โดยไม่ต้องมีคนอธิบาย
2. **ชื่อผู้เล่น + วันเวลา + ชื่อเกมในเฟรม** — ภาพเป็นของเขา และพาชื่อเกมออกไปด้วย
3. **โมเมนต์เต็มจอ** ไม่ใช่ไอเทมในกระเป๋า → **ผู้ใช้รายแรกของระบบ cut-in ที่สร้างไว้แล้วแต่ยังไม่มีใครเรียก**
   (`cutin-authoring.md`; Godot built ยังไม่ wired / Unity Live2D ทำงานแล้ว)
4. **ประกาศในแชททั้งเซิฟ** — social proof ในเกม ส่วนภาพพาออกไปข้างนอก

**สองข้อที่ต้องมีไม่ว่าเป้าจะเป็นอะไร**
- **ตั๋ว bound** — ตั๋วที่ขายทิ้งไม่มีเรื่องเล่า และตั๋วราคาสูงในตลาด = เป้า RMT
- **กระจาย 10 ใบเป็นถังรายชั่วโมง** (~ใบละ 2.4 ชม.) ไม่ใช่ใครถึงก่อนได้ก่อน — ไม่งั้นทั้ง 300 ภาพ/เดือน
  มาจากโซนเวลาเดียว และขัดกับเสาหลัก idle (เปิดทิ้งไว้ข้ามคืน)
- **gate ด้วย Commander level / วัน active ไม่ใช่จำนวนการฆ่า** — รางวัลเพดานรวมทั้งเซิฟเป็นแม่เหล็กบอท
  (replay-verify กันของปลอมได้ ไม่กันบัญชีจริงหลายบัญชี) และถ้าบอทได้ ×100 ภาพจะเล่าเรื่องตรงข้าม

**ยังไม่เคาะ**: ถ้าอยากให้ F2P รู้สึกว่า "ฟาร์มแล้วได้ pull" ต้องมีช่อง**ต่อหัว**แยก (เช่น 1 pull/สัปดาห์)
ซึ่งต้องหักจากงบ 35 pull/เดือนที่วางไว้ ไม่ใช่เพิ่มลอย ๆ — คนละกลไกกับตั๋วสเปกตาเคิลข้างบน
