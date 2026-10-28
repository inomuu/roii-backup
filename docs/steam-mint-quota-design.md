# Steam Market — Mint Quota System (redesign)

**Status:** design v1, 2026-07-22 (decisions locked with user; ปรับได้)

> **2026-09-24 — D1 CANCELLED (user: "ยกเลิกแล้ว ถ้าทำแบบนั้นคนไม่เล่น").** Selling is NOT
> pass-gated: every account may mint/sell (Valve's own $5 / authenticator wall still applies).
> Everything below that assumes pass-only minting (D1, the pass point budgets in §3.2, D7, §3.4's
> "a bot must buy a pass") is superseded; the Farm Banner box (`farm-banner-design.md`) is the
> first mintable item and relies on this. What replaces the point budgets is open.
> **2026-10-03 — MINT COUPONS replace the point budgets (user; name not final). BUILT (server).**
> One coupon accrues every 4 h of REAL time from the first login (no need to be online), up to 5
> uncollected (the clock stops while full). A claim moves them into the held balance, max 10;
> what does not fit stays uncollected (the client warns first via `room`). Sending ONE item to
> Steam costs ONE coupon, whatever the item. Free only, untradeable (bound). Spent by the mint
> worker when it sends the item to Steam; refunded if Steam refuses or nothing was sent; an
> item queued without a coupon waits until one is held. A second use (upgrades, for players who
> do not sell) is planned, not designed. Code: `crates/verify-server/src/mint_coupon.rs`,
> `GET /mint-coupons`, `POST /mint-coupons/claim`, `/badges.mint_coupons`.
**Supersedes:** the "pure drop → anyone promotes" assumption in the original Banana-vision.
**Companions:** `steam-market-requirements.md` (Valve rules), `economy-invariants.md` (I1–I10,
esp. I9), `server-roll-drops-design.md` (provenance source), `monetization-economy.md` (SKUs),
`ROI-Financial-Model.md` (user's Downloads — revenue model this serves).

---

## 1. What changed

The financial model makes **sell quota a product**, not just an anti-bot valve:

- **Merchant Pass (250฿/mo)** = extra sell quota + one 4★ Exemplar per month.
- **Regal Pass (550฿/mo)** = Merchant rights + second party screen + battle-pass track.
- Market fee 10% dev (+5% Valve) grows with GMV → quota design directly shapes revenue.
- Anti-bot line item: "โควตาถ่วงน้ำหนักตามความหายาก + global daily cap".

## 2. Decisions (locked 2026-07-22)

| # | Decision | Choice |
|---|----------|--------|
| D1 | Who can sell (mint) at launch | **Pass holders only.** A free base quota is DEFERRED — revisit after launch data (bot risk + open issues). Consciously accepted: this bends the "earn everything free-via-market" defensibility line for now; free players still BUY freely with wallet funds. |
| D2 | Mintable set at market launch | **Rare cards + uniques** — exactly the classes that are already server-rolled (0.0.207: all sub-1% drops). Every candidate item has server-side provenance from day one. No gear, no Exemplar resale in v1. |
| D3 | Quota unit | **Rarity-weighted points.** Minting an item spends points by rarity tier; passes grant a monthly point budget. Controls VALUE inflow, not item count. |
| D4 | Reset cadence | **Monthly budget (matches pass billing) + a per-day burst limit** — no single-day dumps; smooth inflow without daily-grind UX. |
| D5 | Global daily cap | **NONE.** Personal (pass) limits only — global mint volume is already upper-bounded by global DROP volume, which crowd-scaling holds constant (∝1/N); a market-side global cap would be redundant, and every additional quota-holder is pass revenue. |
| D6 | Purchased Exemplar/Regalia resale (v2, post trade-lock) | **Quota-EXEMPT.** Paid items resell without spending points (hand-change, not new supply) — keeps the "you can always sell it back" pitch line intact. |
| D7 | Leftover points on pass lapse | **Expire with the pass.** Points are a membership right, not a stockpile — prevents banking months of quota for one dump and keeps renewal incentive. |

## 3. System shape

### 3.1 Mint pipeline (extends mint-on-verify)

```
drop (server-rolled, keyed nonce)            ← provenance exists already (I6)
  → player initiates PROMOTE (item → Steam)  ← client intent, server decides
  → server checks, atomically:
      a. provenance: item traces to an attested server roll   (I6)
      b. entitlement: account has an active Merchant/Regal Pass (D1)
      c. quota: monthly points ≥ cost(rarity)                  (D3)
      d. burst: today's spent points + cost ≤ DAY_BURST_LIMIT  (D4)
  → deduct points + mint via IInventoryService (idempotent, I4)
  → item leaves the game inventory permanently (burned game-side; no dual existence)
```

- Quota spend is **atomic with the mint** on the ledger (I9): a failed Steam grant refunds
  the points; a retried grant must not double-mint (dedup key per promote, I4).
- Enforcement is entirely server-side; the client only renders remaining points.

### 3.2 Point budgets (numbers = placeholders, tune later)

| Tier | Point cost | Note |
|------|-----------|------|
| Rare card | 10 | volume commodity of the market |
| Mythical card / unique | 40 | |
| Legendary+ / special | 100 | scarcity showcase |

| Source | Points | Cadence |
|--------|--------|---------|
| Merchant Pass | P_m (e.g. 100) | per month, expire with the pass (D7) |
| Regal Pass | P_r ≥ P_m | per month, same |
| Free account | 0 (v1) | revisit post-launch (D1) |

- **Per-day burst limit** (D4): spendable points per day capped (e.g. P_m / 10) so a
  month's budget can't hit the market in one dump.
- No global cap (D5) — the binding global limit is the crowd-scaled DROP rate; personal
  quotas shape per-seller flow, and quota supply scales only with paying passes.
- Purchased Exemplar/Regalia resales bypass points entirely (D6) — separate, always-open lane.

### 3.3 Layering vs existing supply controls

Two independent valves, both stay:

1. **Drop-side** — crowd-scaled rare drops (rate ∝ 1/N farmers): keeps IN-GAME supply of
   rares constant per hour regardless of population. Bots dilute each other before items
   even exist.
2. **Mint-side** — this system: keeps MARKET value-inflow bounded and paid-for
   (pass-gated). A bot that wants to sell must buy a pass every month + pass Valve's
   $5/authenticator wall → bot pressure converts into revenue (Banana model).

### 3.4 Pass mechanics (server)

- Pass = server entitlement with an expiry (like Exemplar entitlements; enforce
  server-side per the multi-party/IAP rule — client is UX only).
- Monthly 4★ Exemplar grant (Merchant) + BP rewards (Regal) deliver via the
  **game→player (ops) mail** path — the TODO.md backlog item is now a dependency.
- Regal's "second screen" = the premium 2-party session split (existing future design;
  enforced server-side at session level).

## 4. Valve constraints that bind this design

- Marketable items must be **per-instance, non-stacking** (restriction dates stack to the
  newest — a fresh mint merged into a stack would lock the whole stack).
- Price band $0.01–$100 → point costs should track expected price tiers so the global cap
  approximates a $ value cap.
- Market needs a **released** game; grant/consume/inventory testable pre-release with
  itemdefs Private. Fee: set 10% game-specific.
- No store-page RMT marketing at submit; market update announces later (second launch).

## 5. Open questions (next design pass)

1. **Point numbers**: per-rarity costs + monthly budgets (P_m/P_r) + DAY_BURST_LIMIT —
   model against the financial doc's GMV/engaged (120฿/mo) once attach-rate data exists.
2. **Free tier revisit** (D1): criteria to introduce a small free quota later (e.g. after
   bot telemetry + the $5 wall proves sufficient) — restores full "earn everything" framing.
3. **Quota UI**: where the player sees points/burst state (market screen header; the new
   Steam-market screen is being rebuilt anyway).
4. **Pass upgrade mid-month** (Merchant→Regal): prorate points or grant the difference?
5. **Itemdef mapping**: per-rarity itemdefid plan (< 1,000,000, permanent identity) for the
   card/unique classes + icon pipeline (commissioned art needs IP transfer on file first).


---

## BUILT (server side, 2026-09-25 — branch `steam-market`, sim 0.0.478)

- **Catalog**: `ItemDef.steam_itemdef` is an item's permanent Steam itemdef id (itemdef JSON must
  use the same number). Only an item that has one can be sent. `farm_box` = **1001**.
- **Sim**: `InputEvent::SendToSteam { inv_idx }` removes one (unbound) from the bag and counts it in
  `GameState.steam_sent[def]`. The game copy is gone BEFORE any mint — nothing exists twice.
- **Server** (`verify-server/src/steam_inv.rs`): a verified save's counts past the watermark
  become `steam_mints` rows (`pending`, keyed account/def/seq — a repeated save queues nothing).
  A worker in the minute loop claims up to 20 as `inflight`, calls `IInventoryService/AddItem`,
  and records `done` (+ Steam item id) / back to `pending` (Steam said no) / `uncertain` (no
  trustworthy answer — never retried blind, reconciled by hand).
- **Switch**: the worker runs only with `STEAM_INVENTORY_LIVE=1` and `STEAM_PUBLISHER_KEY`.
- **Needs the partner site (user)**: enable Inventory Service for app 4958230, upload the itemdef
  (1001, icon, `tradable`/`marketable` false until the market update), visibility Private.
- Not built: reverse flow (Steam → game), a status endpoint for the client, the ⚠ AddItem response
  shape is from the docs and unverified against live Steam — confirm on the first Private grant.
