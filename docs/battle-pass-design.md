# Battle Pass — 30-day / 50-slot design

**Status:** design v1, 2026-07-22 (decisions locked with user; ตัวเลขทั้งหมด = placeholder)
**Companions:** `steam-mint-quota-design.md` (pass SKUs), `monetization-economy.md`,
quest-system design (memory: 3-layer catalog/measurement/quest-state — BP is its first
instance), `economy-invariants.md`.

> **LIVE CONTENT (2026-10-01, user-approved) — supersedes §1 length, §2 XP and §4 contents below.**
> Built in `crates/verify-server/src/bp.rs` (`default_config`).
>
> - **Season 42 days** (`bp::SEASON_DAYS`, the genre's 6-week version cadence), 50 slots × 100 XP.
> - **XP only from commissions, finish at HALF season** (user: HSR finishes by mid-season, and a
>   player who arrives mid-season must still finish): daily rung 35 (175 a day), weekly rung 90
>   (450 a week). Both → slot 50 on day 21; start on day 21 and still finish by day 42; dailies
>   alone ≈ day 29.
> - **Only items the game still spends; everything bound.**
>
> | | Free | Premium ($9.99) |
> |---|---|---|
> | 10/20/30/40/50 | Standard ticket ×1 | Limited ticket ×2 |
> | 5/15/35 | Sigil I ×6 | Boss Core ×1 |
> | 25/45 | Sigil I ×6 | Track ×1 |
> | other slots, cycling | Memory Dust ×5 · Gil 20,000 · EXP Elixir ×1 · gem g0 ×1 (8 sets rotate) | Radiant Memory Dust ×2 · Sigil II ×3 · EXP Elixir ×2 · bound crystal 100 |
> | season total | 5 Std tickets, 30 Sigil I, 50 Dust, 200k Gil, 10 Elixir, 10 gems | 10 Limited tickets, 3 Boss Core, 2 Track, 20 Radiant Dust, 30 Sigil II, 20 Elixir, 1,000 crystal |
>
> Meals, Phials and Craft materials stay out until Meal / Craft are decided.

---

## 1. Shape

- Season = **30 days**, tracks have **50 slots** each: FREE track (everyone) +
  PREMIUM track (**Merchant AND Regal pass holders** — deviation from the financial
  model's Regal-only BP; Regal's extra differentiation is NOT final yet, see §6).
- Premium unlock is **retroactive**: buying a pass mid-season instantly opens the premium
  rewards for every level already earned.

## 2. BP XP (rides the quest-measurement layer)

Primary = **verified ONLINE time** (~70%), secondary = daily objectives (~30%):

| Source | BP | Note |
|--------|----|------|
| Online hour (server-verified ticks) | ~15/hr, capped ~8 h/day | idle-friendly; drives the DAU/CCU KPI; offline time earns NOTHING (matches "money real-time-only" model; single-session enforced) |
| Daily objectives ×3 (kills / rooms / craft / market action) | ~20 each | from sim measurement counters, quest-state on server |
| Weekly milestone | ~150 | bigger arc, forgiving window |

Pacing target: normal idle player (online most days + some dailies) finishes slot 50 around
**day 24–26**; hardcore ~day 18. Never tune it tight — the pass exists to make opening the
game daily feel right, not to punish a missed day.

## 2.5 Daily system (decided 2026-07-22)

Philosophy: in an idle game a daily is **a reason to LOOK at the game once a day**, not a
chore list — the game plays itself; the daily is the check-in moment.

- **3 dailies/day = 2 auto + 1 action.**
  - *Auto tier* (completes from idle play alone): kill N / clear N rooms / be online 2 h —
    the player opens the game and finds green checks waiting. "The game worked for me."
  - *Action tier* (ONE intentional click, teaches a system): refine once / feed a card or
    Exemplar / craft once / use a Star Stone / list an item (online). Rotates with the BP
    week theme (refine week surfaces refine dailies).
- **No dead ends: 1 free reroll/day** — an action daily you can't do (no ore) is swapped,
  cheaper than guaranteeing every action is always possible.
- **Rewards = BP ONLY** (~20 each, +20 all-3 bonus). No Gil, no items — **the pass track
  IS the reward table**: a full day (~120 online + 60 dailies + 20 bonus ≈ 200 BP ≈ 2
  slots) pops 1–2 track slots, so dailies "give items" every day, paid through the one
  supply-controlled channel. Single economy tap; nothing for bots; pacing tunes in one
  number. Missing a day loses nothing (5-day slack), it's just slower.
- **Weekly milestone = a byproduct**, not extra work: "complete dailies on 5 days this
  week" (~150 BP) — progresses itself.
- **Claim is manual in the BP screen** (the daily dopamine moment: claim ×3, bar moves,
  slot pops) + auto-claim at season end so nothing is lost to forgetfulness.
- **Deliberately absent:** login streaks (FOMO + punishes a missed day — online-time BP is
  the no-punishment attendance already); long-play dailies (time lives in the BP hour
  track); item/Gil rewards. After finishing slot 50, dailies simply stop mattering —
  "done, rest" fits the no-pressure identity.
- **Mechanics (SERVER-TIME-SPLIT, per the quest 3-layer design):** the sim only carries
  counters (kills/refines/crafts/feeds — mostly existing); the SERVER deals the day's set
  (deterministic from date+account → replay-verifiable) and stamps day boundaries; the
  sim never knows what a "day" is.

## 3. Economy rules (hard)

1. **FREE track = account-bound, every slot — unsellable EVERYWHERE** (tightened
   2026-07-22): no Gil-market listing, no Steam promotion, and **no NPC vendoring**
   (`sell_value = 0` — otherwise the free track becomes a small Gil faucet). Free BP
   rewards are use-only. A free account can never convert the pass into Gil or market
   supply (anti-bot: farming BP on free accounts yields nothing liquid).
   *Implementation: a `bound` flag on the item instance, enforced at Gil-market listing
   (sim + server verify), at the Steam promote endpoint, and in the vendor-sell path.*
2. **EVERY reward on BOTH tracks is account-bound — the pass grants nothing sellable**
   (revised 2026-07-30; supersedes the 2026-07-22 "PREMIUM tradeables = SR Exemplar +
   SR Regalia" rule, which is CANCELLED). Rationale: **only farmed loot may ever enter
   the market.** Nothing a player pays for is resellable, in the pass or anywhere else.
   That keeps a single clean line — cash buys power and convenience, never a real-money
   exit — which is also what makes a mobile build defensible at store review (no
   pay-to-extract path exists to review). See §4.4.
3. **No Gil, no raw EXP — both tracks.** Cash must never raise Gil/hour (monetary policy).
   Boosts are time-limited %, online-only, recorded input events, always bound.
4. Power-adjacent FREE rewards are **materials for existing sinks only** (refine ores,
   Star Stones, orbs) — accelerate personal sinks, don't mint new power.

## 4. Track contents (v1 sketch)

Season-1 constraint: **art-light** — cosmetics limited to code-only pieces (titles, chat
name color, damage-number styles, badge); the full cosmetic set waits for the art pipeline.
Every reward on both tracks is BOUND (§3 rule 2) — there is no tradeable tag any more.
Quantities are placeholders.

The 50 slots run as **four themed weeks + finale** — each week's rewards teach/feed one
system, dailies can sync with the theme. Selector nodes (pick-your-reward) add agency.

**Refine Coupon I / II** (bound, built 2026-07-24 — `refine_ticket_minor` / `refine_ticket_major`):
a discount voucher spent on ONE refine that knocks a fixed Gil amount off the cost —
**I = 500 Gil, II = 1000 Gil** — free when the cost is ≤ the value. Everything else about the
refine is normal (same ore, odds, break risk). Early-game Gil relief. Grant plan: **Coupon I ×5
in week 1** (early) + **Coupon II ×5 in week 2** (mid). Placement below is provisional.

**สัปดาห์ 1 — จุดประกายตีบวก (1–13)** *(activation: refine starters early, Enriched debut)*

| # | FREE (bound) | PREMIUM (adds — also bound) |
|---|--------------------------|----------------|
| 1 | Red Phial ×500 | **SR Exemplar — เลือก 1 จาก 5** |
| 2 | **Refine Coupon I ×3** | EXP boost +50% (8 h) |
| 3 | Valkirion ×1 | **Enriched Ore ×2** |
| 4 | Red Phial ×500 | Valkirion ×5 |
| 5 | Orange Phial ×500 | **Inventory Expander ×1 (+1 bag slot, ถาวร)** |
| 6 | **Refine Coupon I ×2** | drop boost +30% (8 h) |
| 7 | EXP Ticket S ×10 · **Unbroken Seal ×1** | **Aegis Seal ×1** |
| 8 | Aegirilium ×1 | EXP Ticket M ×10 |
| 9 | quiver pack | Orange Phial ×500 |
| 10 | ⭐ Blue Star Stone ×1 | **Purple Star Stone ×1** |
| 11 | Field Ration S ×10 (+5 all stat, 2h) | **Field Ration M ×10 (+10 all stat, 2h)** |
| 12 | Speed Potion I ×50 | Enriched Ore ×2 |
| 13 | Orange Phial ×500 | Enriched Ore ×2 |

**สัปดาห์ 2 — นักล่า (14–26)** *(hunt: star stones + refine mats; lures/trials removed 2026-07-24)*

| # | FREE | PREMIUM (adds) |
|---|------|----------------|
| 14 | Blue Star Stone ×1 | Valkirion ×5 |
| 15 | Max Potion ×50 | **Inventory Expander ×1 (+1 bag slot, ถาวร)** |
| 16 | Valkirion ×1 · **Refine Coupon II ×5** | Blue Star Stone ×1 |
| 17 | Field Ration S ×10 (+5 all stat, 2h) | **Field Ration M ×10 (+10 all stat, 2h)** |
| 18 | Monster Accelerator ×1 | **ore crate — PICK Valkirion or Aegirilium ×5** |
| 19 | quiver pack | drop boost (8 h) |
| 20 | ⭐ Blue Star Stone ×1 | **Enriched Ore ×3** |
| 21 | EXP Ticket S ×10 | EXP Ticket M ×10 |
| 22 | Field Ration S ×10 (+5 all stat, 2h) | **Field Ration M ×10 (+10 all stat, 2h)** |
| 23 | Speed Potion I ×50 · **Unbroken Seal ×1** | **Aegis Seal ×1** |
| 24 | Aegirilium ×1 | Purple Star Stone ×1 |
| 25 | EXP Ticket S ×10 | **EXP Ticket L ×10** |
| 26 | Field Ration S ×10 (+5 all stat, 2h) | **Field Ration M ×10 (+10 all stat, 2h)** |

**สัปดาห์ 3 — ช่างธาตุ (27–38)** *(element/craft: SR Regalia, Element Infusion debut, boss tickets)*

| # | FREE | PREMIUM (adds) |
|---|------|----------------|
| 27 | Element Infusion Stone ×1 | Purple Star Stone ×1 |
| 28 | Element Infusion Stone ×1 | Element Infusion Stone ×5 |
| 29 | Valkirion ×1 | Enriched Ore ×2 |
| 30 | ⭐ Purple Star Stone ×1 | **SR Regalia** *(art-gated; fallback: Chaos Stone)* |
| 31 | Yellow Phial ×500 | Aegirilium ×5 |
| 32 | Speed Potion II ×50 | Speed Potion II ×50 |
| 33 | EXP Ticket S ×10 | **Boss Ticket ×2** |
| 34 | Field Ration S ×10 (+5 all stat, 2h) | **Field Ration M ×10 (+10 all stat, 2h)** |
| 35 | **Inventory Expander +1 (ถาวร)** | EXP Ticket M ×10 |
| 36 | Aegirilium ×1 | **Field Ration M ×10 (+10 all stat, 2h)** |
| 37 | Max Potion ×50 · **Unbroken Seal ×1** | **Aegis Seal ×1** |
| 38 | quiver pack | drop boost (8 h) |

**สัปดาห์ 4 — ปิดฤดูกาล (39–50)** *(market week + finale)*

| # | FREE | PREMIUM (adds) |
|---|------|----------------|
| 39 | White Phial ×500 | Purple Star Stone ×2 |
| 40 | ⭐ **เลือก: EXP Ticket S ×10 / Blue Star Stone** | Enriched Ore ×3 |
| 41 | White Phial ×500 | White Phial ×500 |
| 42 | Valkirion ×1 | Enriched Ore ×2 |
| 43 | Field Ration S ×10 (+5 all stat, 2h) | **Field Ration M ×10 (+10 all stat, 2h)** |
| 44 | **Boss Ticket ×1** (free moment) | Boss Ticket ×3 |
| 45 | Purple Star Stone ×1 | Valkirion ×5 + Aegirilium ×5 |
| 46 | Speed Potion II ×50 | EXP boost (8 h) |
| 47 | EXP Ticket S ×10 | EXP Ticket M ×10 |
| 48 | Rejuvenating ×50 | Shiny Star Stone ×1 |
| 49 | Orange Phial ×500 | **Enriched Ore ×3** |
| 50 | ⭐ Chaos Star Stone ×1 | **Red Star Stone ×1** + Shiny ×2 + Inventory Expander +2 |

Rhythm notes: free "moments" = starter tickets (2/6, activation), EXP tickets (7/21/33/47),
Inventory Expander +1 (35), Boss Ticket (44), Chaos Star Stone (50 free). The hero items are
1: SR Exemplar and 30: SR Regalia — bound like everything else (§3 rule 2);
Protection at 7/23/37 (Aegis paid / Unbroken free); Enriched at 3/13/29/40/42/49; selectors at 1/18/40.
Premium finale (50) = **Red Star Stone** (apex, otherwise farm-only). NO cosmetics (art not ready).
Storage expanders (season trials removed 2026-07-24 — expiry too hard; permanent entitlements are
simpler): **stash +1 at 5 & 15** (stash_cap already upgradeable), **Inventory Expander +1 at 35 free /
+2 at 50 premium**. Consumables scaled for idle burn (2026-07-24): Phials ×500, Speed/Max/Rejuvenating
×50, Field Ration S/M ×10 (2 h buff). EXP + drop boosts = 8 h. Star Stones: Blue (early) / Purple (mid);
Red 5★ farmed only.

- **The Exemplar grant sits at SLOT 1** (decided 2026-07-22): buying the pass delivers the
  headline item INSTANTLY — industry-standard conversion shape; retroactive unlock makes a
  mid-season purchase identical. Player PICKS 1 of a 5-strong pool; the copy is **bound**
  (§3 rule 2), so no trade-lock question arises.
- **Exemplar TIERS (decided 2026-07-22, names provisional): SSR = the 300฿ shop-purchased
  Exemplars; SR = the tier granted through pass tracks** (this slot + the Merchant monthly
  grant). **SR and SSR are DIFFERENT CHARACTERS (separate rosters), and an SSR is a clear
  power step above an SR** — the pass never hands out what the shop sells, so grants can't
  cannibalize SSR copy sales, and SSR keeps its premium identity. NOT called "4★": ★ is
  Resonance (per-instance dupe-feed progression) and applies to both tiers as usual.
- **The tier split covers Regalia too**: slot 30 (moved from 27, 2026-07-24) = **SR Regalia** (the pass's own weapon
  line — the Genshin BP-exclusive-weapon analog); SSR Regalia stays a shop SKU ($4.99).
  The SR line (Exemplar + Regalia) is the PASS product family; the SSR line is the SHOP's.
- Free track never grants an Exemplar.
- With the Merchant monthly grant that's up to 2 copies/month for an engaged holder —
  the copy engine behind Resonance star-ups and the 1.81 copies/buyer assumption.
- **ALL cosmetics REMOVED 2026-07-24 (art not ready — add back later):** chat color (10), dmg-number
  style (30), season badge (40), exclusive style + season title (50). Replaced with star stones —
  Purple ×1 at 10 & 27, **Red Star Stone ×1 as the slot-50 finale** (the apex stone, otherwise
  farm-only). Retention now lives in Regalia (30) + the Red finale, not a cosmetic set.

## 4.4 Monetization add-ons (decided 2026-07-22)

- **Tier skip: ~$5 per 10 levels.** Late buyers / last-week finishers buy the remaining
  distance (retroactive premium makes every skipped level pay out instantly). Standard
  industry lever; no gameplay effect beyond reaching rewards sooner.
- **Why buy the pass (revised 2026-07-30).** The old answer was the *self-paying pass* —
  sell the premium tradeables to recoup the price. That died with the bound-everything rule
  (§3 rule 2). The pass now sells on the standard industry shape: **more crystal than buying
  the same dollar amount directly, plus the item track on top.** Same price, strictly more
  value, and the crystal are the pull currency the player already wants. Pass crystal stay
  **non-tradeable** and drip daily (see `gacha-economy-design.md` §Currency) — a generous
  headline gated behind logging in, which caps the real margin cost.
- **No repeatable tail slots (50+).** Considered and rejected. Post-completion retention
  rides normal idle play and the next season, not filler levels.
- Regal deluxe differentiation: still deferred (§6).

## 4.5 Reward idea pool (brainstorm 2026-07-22 — pick per season)

> Revision 2026-07-30: BP-granted copies of everything below are **bound, without exception**
> (§3 rule 2 — the SR Exemplar / SR Regalia carve-out is cancelled). `[T]` tags below refer to
> NON-BP channels
> (gameplay drops, craft, shop) where a tradeable version may still make sense.

- **EXP Ticket (catch-up)** [F small / P+T packs]: grants EXP to a party member but NEVER
  past the account's highest member level (minus a small margin) — pure catch-up, can't
  push the frontier → the cleanest "paying saves time" instance. Fixes recruit friction
  that grows with account age and makes buying a new Exemplar/class immediately usable.
  Sim: new `UseExpTicket` input event (classify per input-authority; item needs server
  provenance; cap checked deterministically in-sim). Tradeable packs = an onboarding
  commodity veterans sell to newcomers.
- **Protection Stone (กันแตก)** [P+T, slots 7/23/37]: consumed on a refine attempt to
  protect against the BREAK outcome. NEW item + refine-flow change — to design: does a
  protected break become "no change" or "downgrade"? auto-consume when held vs explicit
  toggle in the refine UI? Interacts with server-rolled refine (the roll stays server-side;
  protection modifies outcome APPLICATION deterministically). Tradeable = prime market
  commodity + a real Gil-market/refine-economy lever (softens the downgrade-tension noted
  in the anti-bot brief). Free track gets NONE (ore ×1/slot only) — protection is a paid
  economy good.
- **Buff consumables (ยา buff / อาหาร / ยาเร่ง ASPD)** [F/P]: timed stat buffs through the
  existing auto-use consumable bar — **Speed Potion I–III (aspd_pot_1..3) are ALREADY in the
  catalog**; add RO-style stat FOODS (+STR/+AGI/+INT… for N minutes, one food buff active).
  Long game: foods become CRAFT recipes → commodity-market goods → recurring Gil-economy
  demand (RO food-economy loop). BP grants small packs; crafting is the sustained source.
- Loot items already BUILT but not in drop tables (Lure, Accelerator, Field Ration, Max
  Potion, Phoenix Tail, Rejuvenating) — zero-work fillers [F].
- Sink materials: ore packs, Star Stones; anchors carry Chaos/Shiny [F]; big packs [P+T].
- QoL (always bound): offline-cap +2 h for the season (check pacing math), EXP/drop %
  boosts (online-only, recorded), TRIAL funnels — rare-bag +1 slot / stash +10 for the
  season (taste of the permanent cash upsells; expiry must be loud).
- Cosmetics [P]: desktop-companion window themes (identity item!), damage-number styles /
  hit-FX colors, chat name color + badge, party-frame skins, seasonal card-back foil [T],
  seasonal cosmetic SET across anchors finishing at 50 [T].
- Mid-track anchor [P]: **SR Regalia** at slot ~25-30 — the second headline pull of the track (the pass's own weapon line; SSR Regalia = shop SKU). Bound, like every BP grant.
- **Reroll Orb** [P+T]: rerolls a gear's substat/enchant lines (PoE-chaos-style) — sits
  exactly on the locked "real-money = orb + unique narrow rolls" direction; likely the
  strongest-selling tradeable in the pool.
- **Foil Polish** [P+T, very rare]: converts an owned card to its FOIL version — chase
  item for the card market (foil system 0.0.223).
- **Album Wildcard** [P+T, rare]: counts as any card for BIND COSTS only (not equippable)
  — closes a missing-last-copy bind; time-saver, not new power.
- **Typed Lures** [F/P]: race/element-targeted lures ("Plant lure", "Undead lure") —
  targeted card/mat farming, deepens the planned lure line.
- **Boss Ticket** [P]: summons the band boss immediately (skip the kill counter) —
  economy-safe: rare drops stay server-rolled + crowd-scaled, so kill frequency ≠ supply.
- **Inventory Expander +1 / +2** [F +1 rare / P +2]: PERMANENT inventory (or stash) slots
  as small consumable items — the RO "Gym Pass" pattern. Tiny drips (a few per season,
  hard season cap) so the big permanent capacity upsell keeps its value; bound always.
  **BUILT 2026-07-25:** item `inventory_expander` (bound); use it (＋ corner in the inventory
  → confirm) → `ExpandInventory` event → +1 BAG slot via `inventory_cap_bonus` (capped +50).
  "+2" = grant qty 2. Granted on the pass free track (slots 5/15 in the dev config).
- **Enriched Ore** [P+T]: refine ore with BONUS success odds — complements Protection
  Stone (odds-up vs break-guard); RO's most iconic paid refine item (Enriched Elunium).
- **Refine Starter Ticket** [F/P, ALWAYS bound]: flat **−500 to −1,000 Gil** off ONE refine
  attempt (floor 0). Purpose = activation ("makes you want to START refining"): at +1–+3
  it halves the cost, by +8 it's ~10% — **self-limiting**, the exponential high-level sink
  stays intact. Bound on BOTH tracks: a tradeable Gil-discount ticket would be indirect
  Gil trading. A few per season. (The expected-cost "discount" at high levels stays the
  job of Enriched Ore + Protection Stone — items, not price cuts.)
- **Mercenary Scroll** [P]: temporary 6th party member for a limited time — a live trial
  funnel for the party-slot / second-party upsell.
- **Specialized stash tabs** (PoE pattern, shop upsell more than BP): Card Tab / Material
  Tab — ethical-monetization gold standard.
- **Auto junk-seller** (GW2 salvage-o-matic pattern, shop upsell): permanent tool that
  vendors junk automatically — the Sell Junk button already exists; this automates it.
- **Dead-Branch-style random summon**: fun chaos item — MUST be craft/Gil-acquired only,
  never cash-sold (cash + RNG + marketable = the NL/BE loot-box triangle).
- ⚠️ **Reroll Orb legal flag**: paid + random outcome + marketable gear = the same
  triangle → the orb must be earnable/Gil-tradeable as the primary path (PoE model), not
  direct-cash-sold; BP-premium placement is borderline — legal check before shipping.
- Social/QoL code-only: Party Rename Scroll, Pet Food (Ranger pet buff), Bestiary
  Research Notes (reveal drop/element info early).
- ❌ TRAPS — never make: Resonance Catalyst (substitutes Exemplar copies = eats the core
  revenue engine), Gil-sink discounts (refine cost / market fee cuts = inflationary +
  cuts our own revenue).
- Titles: one free [F], richer + frame [P].
- Daily objective examples (all existing sim counters): kill N / clear N rooms / refine
  once / craft once / feed a card or Exemplar / list an item.
- **NEVER: soul** (permanent Paths power through wipes), raw Gil, uncapped EXP.

## 5. Open details

1. **The 5-Exemplar pool composition** — current season + which past/launch ones? Rotates
   per season? (Selector value depends on pool freshness.)
2. **SR tier spec** — SR/SSR are separate characters, SSR one power step above (locked);
   still open: the exact gap (overlay stat/skill numbers, star-rank table depth, art
   budget) + final tier names (SR/SSR provisional). Also re-read the financial model's
   "Exemplar 4 ดาว/เดือน" line as an SR grant.
3. ~~**Other premium slots' tradeability**~~ — CLOSED 2026-07-30: nothing the pass grants is
   tradeable, on either track (§3 rule 2). No per-slot flagging needed.
4. ~~**Do pass-reward mints spend quota points?**~~ — CLOSED 2026-07-30: pass rewards never
   mint, because they are never sellable. Quota applies to farmed loot only.
5. Trade-lock on premium rewards (mirror the 2-month Exemplar lock? shorter?).
6. Numbers: BP/hr, daily/weekly values, slot cost curve — tune after attach data.
7. Reward art/icon pipeline for the seasonal cosmetic set (commission + IP transfer).

## 6. Regal differentiation (NOT final)

Regal (550฿) currently = Merchant rights + second party screen + BP premium (same as
Merchant now). Its extra BP layer is undecided — candidates: bonus slots (51–60), a
parallel "Regal lane" of cosmetics, boost multipliers, or extra quota points. Design later;
nothing in this doc blocks it.
