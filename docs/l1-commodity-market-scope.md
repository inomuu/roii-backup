# L1 Commodity Market — build scope

Status: **SCOPE (not built)** · 2026-07-09 · target = the Gil-based, server-authoritative
order-book market for **fungible/stackable** items (craft materials, ore, potions, junk).
Companion to [economy-invariants.md](economy-invariants.md) (the safety spine) and
[s3-economy-design.md](s3-economy-design.md) (item lifecycle). See memory `auction-house-design`.

This is **L1** (in-game Gil economy, a real core feature) — NOT the L2 Steam real-money
layer. Item/gear auction (per-instance) is a SEPARATE surface (already partly built via
`/register` + `/market/*`); this scope is only the **commodity** half.

---

## What already exists (do not rebuild)

- **Item-side (per-instance) economy**, in-memory: `/register` `/market/list` `/market` `/market/buy`
  `/ship/load` `/ship/mint`, ledger keyed by `instance_id`, 5% fee burn sink, ownership transfer.
  `crates/verify-server/src/main.rs`. **`/register` explicitly REJECTS fungible kinds** — commodity
  is a NEW path, not this one.
- **Auth + account keying**: Bearer token → `account_of()` → `account_id`; Postgres `accounts`/`saves`/
  `trusted`/`refine_outstanding`/`refine_consumed`/`active_sessions`/`cheat_flags`; in-memory overlay +
  write-through (`load_from_db`); single-active-session; taint-on-divergence; dev-key gate.
- **Mock wallets** (`wallets: HashMap<u64,u64>` + `/dev/grant-gil`) — a placeholder, **NOT** the
  authoritative Gil. Real Gil = `GameState.gil` (`state.rs:181`), moved only by `award_gil()` and
  sell/buy InputEvents, verified on `/save`.
- **Sim gil/inventory**: `GameState.gil`/`gil_millis`; `award_gil()`; stackable = Material/Junk/
  Consumable/Card (`Item::is_stackable()`); `BuyItem`/`SellAllJunk`/`SellInventoryItem` InputEvents.
- **Client sync**: `_push_online()` posts a SaveBundle every ~10s; server replay-verifies + stores.

---

## Terminology

**"Market hold" (a.k.a. listing custody)** = commodity units the market holds while they are listed
for sale (they leave the seller's `GameState.inventory` at list time so they can't be double-spent).
This is a standard AH mechanic (WoW/RO list = item leaves your bag) and is **in-game only** — it is
**distinct from the Steam `ESCROWED` lifecycle stage** (`/ship/load`, an item committed for export to
Steam before minting). Where this doc said "escrow" for L1 below, read "market hold".

## The crux (the hard, risky part — read before slicing)

Commodities live **inside the verified `GameState.inventory`** (def_id + quantity), and Gil lives in
`GameState.gil`. The server **cannot edit a player's GameState directly** — it is client-sim-authoritative
and only mutated by InputEvents that survive replay-verify. So a sale must move value across the
**GameState ↔ server-ledger boundary** via **server-granted InputEvents + an escrow pool** (the
"trade = server-granted InputEvent" model, s3-economy-design). The mock `wallets` do not solve this;
they must be replaced by GameState-derived balances.

Concretely, one commodity sale spans two accounts and two save cycles:
1. **Seller deposits** N×`def_id` into the market → units LEAVE `GameState.inventory` (a
   `DepositCommodity` InputEvent) → server holds them in an escrow pool once the seller's save verifies.
2. **Match** against buy demand (server order book) → **buyer pays** Gil, **fee burned** (sink),
   **seller credited** Gil, **buyer receives** N units — each delivered as a **server-granted
   InputEvent** injected into that account's stream, landing on their next verified save.

Escrow conservation across this boundary IS the economy-invariant problem. Everything below is
sequenced to de-risk it: **harness first, matching engine in isolation, then the GameState bridge.**

---

## Decisions needed before Slice 2 (flag to user)

1. **Market-hold model (recommend A):**
   - **A — deposit-first hold:** units leave GameState into market custody WHEN listed (one place at a
     time = clean conservation). Delivery = server-granted grant. ✅ safer.
   - **B — list-from-holdings (lazy):** units stay in GameState while listed; removed only on match →
     seller could move/spend them between list and match → must re-verify at match (racey).
2. **Async delivery = MAILBOX (DECIDED 2026-07-09, WoW-style).** Sale proceeds (Gil) and
   expired/cancelled returns land in a server-side per-account **mailbox** (Neon, persisted);
   the player CLAIMS them via a `ClaimMail {mail_id}` server-gated InputEvent (verify the mail is
   real + unclaimed → apply ONCE → mark consumed; the `refine_outstanding`/`consumed` template
   exactly). This is the scope's `outstanding_grants` given a player-facing UX + a discrete,
   idempotent, forgery-checkable CLAIM point — which is WHY it de-risks Slice 2 (value sits safe
   in the server mailbox until an explicit claim, instead of the server injecting grants into the
   client stream at arbitrary times). Asymmetry: the **buyer** (online, initiating) can receive
   goods directly as part of the buy; the **seller** (absent when it sells) gets proceeds/returns
   by mail. (Optionally route everything through mail for uniformity — safer, slightly slower UX.)
   Needs: `mailbox(account_id, mail_id, kind, def_id, qty, gil, created_at)` table + `ClaimMail`
   event + a mailbox UI (Control). This ALSO resolves #5 below (cancel/expiry → mail).
   **EXACTLY-ONCE (loss-safe) RULE for 2b — consume a mail ONLY on a VERIFIED save, NOT at the
   claim request.** Flow: player claims → `ClaimMail {mail_id, payload}` enters the input log →
   sim applies gil/item to GameState (optimistic) → save-verify: if VERIFIED, mark the mail
   consumed (the value is durably in-game); if REJECTED/lost, the mail stays claimable → no loss.
   That gives at-least-once (no loss on a dropped/rejected save) + at-most-once (the consumed set
   blocks a re-grant) = exactly-once. The client carries the payload in the event; the server
   verifies it against its mailbox record (forged/mismatched payload → reject + taint) — the SAME
   consume-on-verified-save + server-checked-payload pattern as server-roll refine. ⚠️ Slice 2a's
   HTTP `/mail/claim` consumes at request time (isolated stand-in) → has a theoretical lost-response
   window; 2b's ClaimMail path closes it.
   **GENERAL PRIMITIVE (user 2026-07-09): build the mailbox as a server→player DELIVERY channel,
   not AH-specific.** Same system serves: AH proceeds/returns, **compensation** (downtime/bug/
   rollback gifts), rewards (event/login/milestone/redeem/founder), refunds, player gifts. `kind`
   enumerates the source. THREE guards because it's a server-authored MINT channel: (a) strict
   authority gate — a client-forged `ClaimMail` is rejected at verify (entitlement, like Exemplar/
   refine nonce); (b) **mint provenance/audit** — every mailed item entering the economy is a MINT
   event, logged (who/what/why) so conservation still audits (economy-invariants I6); (c) an ops
   send path (`send_mail(account, payload, reason)`) + **global/broadcast mail as a COUPON** (one
   global reward row + per-account consumed marker — do NOT fan out 100k rows).
3. **Partial fills:** market-buy N units may match several sell orders at different prices → allow
   partial fills + price-time priority? (Standard order book. Recommend yes.)
4. **Anti-manipulation:** price bands (reject listings wildly off the moving market price?), per-account
   rate/quota (I9), min fee. Confirm scope for the first cut (recommend: fee sink + quota now, price
   bands later).
5. **Cancel/expiry:** un-matched sell orders — cancel returns units via a grant; TTL expiry (reuse the
   refine-orphan GC pattern).

---

## SETTLEMENT INVARIANT (money-safety — non-negotiable, user 2026-07-09)

**Cross-account value moves ONLY when the mover's save is VERIFIED. Never settle on a request.**

The attack it defends (fraud-transfer / laundering race): a cheater forges 1,000,000 Gil in
their GameState, immediately market-buys, and the Gil flows to a (colluding) SELLER's account —
"cashing out" fake money to a real account before the forger's save is caught and reverted. If
settlement happened on the buy REQUEST (optimistically), the seller keeps laundered value even
after the forger's save is rejected.

Defense (inherent in the design if the discipline holds): the reconcile runs ONLY after
`verified_state` succeeds (same gate as the refine reconcile). A forged-Gil buy → the save fails
replay verification → the reconcile never runs → NO seller credit, NO order match, NO mail. The
forged value is caught at verification BEFORE any settlement; there is NO async window because
settlement is bound to verification, not to the buy request. The cheater "becoming invalid" is a
side effect, not the defense — the defense is that unverified saves settle NOTHING.

Consequences: (a) the Slice-1 direct `/commodity/buy` `/commodity/sell` (which settle on request)
are DEV-ONLY — production settlement is EXCLUSIVELY via the verified-save reconcile; gate/remove
the direct-mutation endpoints for prod. (b) The only cross-account value path is reconcile-
generated mail, which only fires on verified saves → forged value never reaches another account →
cross-account conservation holds. (c) A seller cannot fabricate their own proceeds — they receive
value only from a real, verified buyer.

## Vertical slices

### Slice 0 — Invariant harness FIRST (safety spine, no new money code)
Encode the Gil-relevant subset of I1–I10 (economy-invariants.md) as property/fuzz + concurrency tests
against a `CommodityLedger` model (pure Rust, no HTTP): **I1 conservation** (Σ Gil + Σ escrowed units
constant across any trade sequence), **I2 no-negative**, **I3 atomic trade** (buy either fully applies
or fully rolls back), **I5 no-double-spend** (a deposited unit can't be sold twice), **I7 sink
accounting** (burned fee = Σ price − Σ seller-received). Real-money invariants (I10) = stubbed for L2.
**Accept:** `cargo test -p verify-server` fuzz run (10k random trade sequences) holds every invariant.
This slice is the reviewer for every slice after it.

### Slice 1 — Commodity order book, in-memory, DECOUPLED from GameState
New endpoints (dev-gated, auth'd), operating on a NEW `commodity` ledger `(account_id, def_id) → qty`
+ the existing mock wallet (GameState bridge comes in Slice 2):
- `POST /commodity/sell {def_id, qty, unit_price}` → post a sell order (debits the account's commodity-
  ledger balance into escrow).
- `POST /commodity/buy {def_id, qty, max_unit_price}` → market-buy, match cheapest-first (price-time),
  partial fills, 5% fee burn, atomic.
- `GET /commodity?def_id=` → the order book (asks aggregated by price) + last trade price.
- `POST /commodity/cancel {order_id}` → return unsold units to the commodity-ledger.
**Accept:** curl a full sell→book→buy→settle cycle; Slice-0 harness green against the live structs.

### Slice 2 — GameState reconciliation bridge (the crux)
Wire the commodity ledger to the verified GameState via server-granted InputEvents (escrow Model A):
- New sim InputEvents: `DepositCommodity {def_id, qty}` (Client authority, self-cost: removes from
  inventory) and `GrantCommodity {def_id, qty}` / `GrantGil {amount}` (**Server** authority — client-
  forged = rejected at verify, per input-authority-security).
- Server: on a verified save carrying a `DepositCommodity`, credit the seller's commodity-ledger
  (the deposit is only real once verified). On a match, enqueue `GrantGil`(seller) + `GrantCommodity`
  (buyer) into `outstanding_grants` (per-account, persisted); apply on their next save; reconcile like
  refine nonces (idempotent, consume-once).
- Extend Slice-0 harness to the boundary: Σ(GameState gil+units) + Σ(escrow) + Σ(burned) conserved
  across deposit→match→grant, including offline/async delivery and replayed (idempotent) saves.
**Accept:** two simulated accounts (curl + a scripted sim) complete a cross-account sale end-to-end;
boundary conservation holds; a forged `GrantCommodity` from the client is rejected.

### Slice 3 — Postgres persistence
Tables mirroring the existing pattern: `commodity_orders (order_id, account_id, def_id, qty_remaining,
unit_price, created_at)`, `commodity_balances (account_id, def_id, qty)`, `outstanding_grants
(account_id, seq, kind, def_id, qty, amount, created_at)`. Write-through + `load_from_db` overlay +
TTL GC for stale orders. **Accept:** restart (scale-to-zero) preserves open orders + pending grants
(the refine-persistence proof, repeated for commodities).

### Slice 4 — Client commodity-market UI (Control nodes)
`commodity_market_view.gd` (retained Control, like `craft_view.gd`): per-material order book (asks +
last price), a Sell panel (pick a stackable from inventory, set qty + unit price), a Buy panel (qty
from cheapest), live Gil. Wire through `sim_driver` → `server_client` (new `commodity_*` calls),
following the shop/craft pattern; hotkey + HudBar button. **Accept:** in-game (online) sell + buy a
stack of Red Herb between two accounts; Gil + inventory reconcile after the next sync.

---

## Reuse checklist (patterns already in the codebase)
- Auth: `account_of(headers)` → `account_id`; dev-key gate.
- Persistence: in-memory overlay + write-through + `load_from_db`; composite-PK tables; TTL GC
  (refine-orphan sweep is the template).
- Async server→client delivery: `outstanding`/`consumed` idempotent store (refine_outstanding pattern).
- Server-authored value: `InputAuthority::Server` grants, rejected if client-forged (input-authority-security).
- Sink: 5% fee burn (existing `/market/buy`), tracked in a `fees_collected` counter.
- Client sync: SaveBundle push carries the seller's `DepositCommodity`; grants arrive via login/pull.

## Out of scope (later)
- Item/gear auction (per-instance) polish — separate surface.
- server-roll drops gate (needed before RARE tradeables, not commodities).
- L2 Steam real-money mint (release-gated).
- Price-band anti-manipulation, merchant-license stake (anti-cheat-economy-strategy-v2).
