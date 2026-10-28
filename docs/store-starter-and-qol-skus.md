# Store — starter pack + non-crystal SKUs

Fills two gaps `gacha-economy-design.md` leaves open. **That doc owns crystal, passes and every
pull-price number; nothing here changes them.** This one covers only:

1. a **one-time starter pack** (the doc has none — the $0.99 ladder rung was removed on purpose)
2. **SKUs that are not crystal** (permanent utility, cosmetics)

Written 2026-07-31 by a session that does not own the economy — proposals, not decisions.

---

## Constraints these products must not break

Checked against the code and the invariants doc, because three obvious ideas are illegal here:

| rule | consequence for a store product |
|---|---|
| **I12** — Summon Shards cannot be bought | a pack may **never** contain shards. Selling pity progress directly is the thing the invariant exists to stop. |
| **I11** — crystal buckets never convert | promo crystal are **bound**, like every faucet/pass crystal. Only a direct crystal purchase yields tradeable crystal; a discounted pack of tradeable crystal is an RMT on-ramp at a discount. |
| Characters are gacha-only | no SR/SSR Exemplar in any pack, at any price. |
| Ladder rate must fall monotonically | a starter pack sits **outside** the ladder and must be flagged as such in the catalog, so a later audit of "rate per tier" does not try to "fix" it. |
| BP already grants Inventory Expander (tiers 5, 15) | do not sell what a pass gives — see §3. |

---

## 1. Starter pack — "Regent's Kit"

**$1.99 · once per account, ever · never discounted, never re-run.**

| contents | why it is allowed |
|---|---|
| 300 **bound** crystal | exactly one pull — the same legibility rule that set the $1.99 ladder rung ("an entry pack you cannot spend is worse than no entry pack"). Bound, so it is not a cheap route to tradeable currency. |
| Refine ore bundle | pure progression, no market value |
| Refine Coupon I ×2 | already a BP-tier item, small quantity — a taste, not a substitute |
| One cosmetic (hat) | costs nothing to grant, carries the "I bought something" signal |

**Its job is conversion, not revenue.** The expensive step in a F2P game is the *first* payment —
card details, the decision to be a person who pays for this game. Everything after that is
cheaper to sell. Price it so the answer is reflexive and the value is obviously lopsided.

**Rules that keep it from damaging the rest of the store**
- **Outside the crystal ladder.** It is not the bottom rung; it is a different product that happens
  to contain crystal. Flag it in the catalog so the monotonic-rate check skips it.
- **Never re-run, never discounted.** A "one-time" offer that returns teaches players that every
  price on the store is provisional — the same reasoning the economy doc uses for never
  re-running a limited unit.
- **Do not show it against the $2.99 gacha pass.** They compete for the same first-yes. Sequence
  them: starter early (one-time, small), pass surfaced *after* the player has a reason to log in
  daily. Two "best first purchase" offers on one screen sell neither.
- Visible from the first session, but not a pop-up before the player knows whether the game is
  fun.

**Why not $0.99:** at 300 crystal/pull that buys half a pull, which is exactly why the ladder's
$0.99 rung was deleted. $1.99 is the cheapest price at which the pack contains something the
player can actually use on arrival.

---

## 2. Non-crystal SKUs — permanent utility

These matter more than their price suggests: content SKUs cost ~$2,000 per SSR to produce,
while a slot upgrade costs nothing to make. They lift blended margin and fill price points the
crystal ladder does not cover.

| SKU | price | shape |
|---|---|---|
| **Rare-bag +3 slots** | $4.99 | permanent, one purchase (already designed — memory `rare-slot-purchase-design`) |
| **Dispatch slot +1** | $4.99 → $7.99 → $9.99 | escalating per slot, permanent. Pairs with the dispatch system in M1.6 |
| **Second party slot** | $19.99 | the premium one; needs the Account/Session split first (memory `multi-party-account-session-split`) |
| **Cosmetics / hats** | $1.99–$4.99 | fills the price band between the starter and the $9.99 crystal rung |

**Pricing rules for permanent goods (different from crystal)**
- **One price, forever. No sales, no bundles that make it cheaper.** A player who paid full price
  and then sees it discounted does not buy the next permanent item. Crystal can go on promotion;
  permanent capability cannot.
- **Escalate within a family** (dispatch slots) rather than discounting — the same shape the
  in-game recruit cost already uses, so it reads as consistent rather than greedy.
- **The value must be computable by the player.** "+1 dispatch slot = +1 expedition running at
  all times, forever" is a calculation the buyer can do, which is what makes a utility SKU an
  easy yes and a power SKU an argument.

### Deliberately NOT sold

- **Bag / inventory slots.** The BP already grants Inventory Expander at tiers 5 and 15. Selling
  the same capability for cash devalues two BP tiers and gives the pass one less reason to exist.
  Either sell it and remove it from the BP, or leave it earned — **leave it earned** is the
  recommendation: the BP needs concrete utility more than the store needs another $4.99 line.
- **Anything on the pity axis.** Shards, guaranteed pulls, rate-up: I12, and the reason for it.
- **Power that is not obtainable in game.** The standing position is pay-for-power via gear that
  free players can also reach; a store-only stat line breaks that framing.

---

## 3. One contradiction to resolve before building

`gacha-economy-design.md` §Income says **"Two paid products only"**. That is true of
*subscriptions* (gacha pass, BP) but the storefront section of the same doc anticipates bundles
and a dense catalogue, and everything in §2 above is a third category: **one-off, non-recurring,
non-crystal**.

Scope that sentence — "two RECURRING products" — or this doc contradicts it on sight. The
architectural consequence is already agreed either way: the catalogue is **server-side data**
(`gacha-system-reference.md` §10.2), so adding a SKU must never mean shipping a new handler.
