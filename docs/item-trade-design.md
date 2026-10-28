# Item trade — the per-instance equipment market

DESIGN (not built), 2026-07-10. The ITEM half of the AH split (`auction-house-design` memory):
COMMODITY (fungible, order-book) is L1 — BUILT + live; this is the per-instance EQUIPMENT market
for unique gear (each piece has its own stats / refine / stars / sockets / cards). Reuses the L1
settlement backbone; the ledger, matching, and search are new. Companions: `s3-economy-design.md`,
`economy-invariants.md`, `monetary-policy.md` (the fee sink), `server-roll-drops-design.md` (the
rare tier, deferred), `craft-and-offline-design.md` (crafted gear trades here too).

## Why it is NOT the commodity market

Commodity items are FUNGIBLE → one order book per `def_id`, one aggregate price, auto-matched
cheapest-first. Equipment is UNIQUE per instance → there is no "price of X". So:

- **Per-item listings** (eBay/classifieds), NOT an order book. Each listing is one specific
  instance at a seller-set price. Buyers browse + buy a chosen listing.
- **No auto-matching.** The buyer picks a listing; the server transfers that exact item.
- **Search is the centerpiece.** With unique items + seller-set prices, buyers can only find what
  they want by FILTERING (slot / type / level / rarity / refine / main-stat / SUBSTAT). The filter
  is the hardest and most important part of this system, not a nicety.

## What reuses L1 vs what's new

| Piece | Reuse? |
|---|---|
| Settlement backbone — escrow via verified save, mail delivery, fee burn, `market_ops`, reconcile, the settlement invariant | ✅ reuse |
| Ledger structure | ❌ NEW — `listings by id (full item payload + price + seller)`, not `orders by def_id (aggregate)` |
| Matching | ❌ none — browse-and-buy-a-specific-listing |
| Mail payload | ⚠️ EXTEND — must carry a full ITEM instance (commodity mail carries only `def_id + qty`) |
| Search / filter + UI | ❌ NEW — the centerpiece |

## The trust model — anti-dupe by replay-verify

The whole system rests on the item being AUTHENTIC (real, unforged stats) and existing in exactly
ONE place. This comes for free from the existing replay-verify model for COMMON (client-rolled)
gear:

1. **List = the item leaves the seller's verified inventory.** A `MarketOp::DepositItem` removes
   the specific item from `GameState` in-sim and records the op. On the seller's verified save the
   server creates the listing + escrows the item server-side. Because the save is replay-verified,
   the server knows the seller REALLY held that item with THOSE stats — the sim's deposit handler
   no-ops a deposit of an item the player doesn't hold, so a forged/duped/stat-edited deposit makes
   the replay diverge → hash-mismatch → rejected (never listed). Same mechanism that makes a
   commodity deposit safe, with a richer payload.
2. **One place only.** The item is removed from the seller's inventory the moment it's listed and
   held in the item ledger (escrow) — it can't be equipped, sold twice, or duped.
3. **Delivered intact.** On sale the escrowed (authoritative) item is delivered to the buyer by
   mail with its full stats — the buyer receives the exact instance the server held.

**One system, not two.** Once a server-rolled rare is GRANTED (via a Server-authority event the
replay re-applies with the authoritative stats), it is a normal item in the verified GameState —
replay-verify protects its stats exactly like a common item's (edit the stats after grant → replay
re-applies the original → diverge → reject). So the trade codepath is IDENTICAL for both. Rares are
deferred only because they don't EXIST until `server-roll-drops-design.md` is built (a dependency),
and to prove the one system on low-value items first (rollout) — NOT because they need a separate
system.

## Data model (sketch)

```
ItemListing { id: u64, seller: u64, item: Item, price: u64 }     // full Item payload
ItemLedger  { listings: BTreeMap<u64, ItemListing>, next_id: u64, fees_burned: u64 }

MarketOp::DepositItem      { op_id, item, price }     // list (item leaves inventory)
MarketOp::BuyItem          { op_id, listing_id, max_price }
MarketOp::CancelItemListing{ op_id, listing_id }
```

`Item` already carries def_id / stats / refine_level / stars / sockets / cards / substats — the
listing serializes the whole thing. Persisted in `market_kv` like the commodity ledger.

## Flows (reusing L1 settlement)

- **List** — client `DepositItem{item, price}` → item leaves inventory (replay-verified) → on the
  verified save, reconcile creates `ItemListing` + escrows the item. (Client force-pushes so it
  appears immediately, like commodity list.)
- **Buy** — client `BuyItem{listing_id, max_price}` holds `price` Gil → on the buyer's verified
  save, reconcile: if the listing still exists and `price ≤ max_price`, transfer: **item → buyer**
  (Purchase mail, full item), **price − fee → seller** (SaleProceeds mail), **fee burned**, listing
  removed. If the listing was already taken (race — two buyers, one unique item) → **full refund**
  to the buyer (Refund mail). Buyer claims the item from mail into inventory.
- **Cancel** — seller cancels → the escrowed item is returned via Return mail (like commodity
  cancel). A "My Listings" view (as built for commodity) shows the seller's active item listings.

Fee: same % burn as commodity → a Gil sink (`monetary-policy.md`). All cross-account value still
moves ONLY via mail on a verified save (settlement invariant, `economy-invariants.md`).

## Mailbox extension

Commodity mail carries `{gil, def_id, qty}`. Item delivery needs a full ITEM instance, so
`MailItem` gains an optional `item: Option<Item>` payload and `ClaimMail` grants that item into the
buyer's inventory (with a full-inventory guard). This is the one structural change to the shared
mailbox; commodity mail is unaffected.

## Search / filter — the centerpiece

`GET /item-market?<filters>` returns matching listings (seller-set price, sorted by price by
default). Filters:

- **slot / item type** (weapon, armor slot, accessory, …)
- **level / rarity / refine** ranges
- **main stat** (has ATK ≥ X, MATK ≥ X, …)
- **SUBSTAT** — filter by substat presence AND value threshold (e.g. "has +crit ≥ 5%"). This is a
  hard requirement (user decision) and drives the whole search UX: unique + seller-priced items
  are only discoverable by substat filtering.

Server-side filter over the listing set (linear scan is fine at expected volume; index by slot +
substat later if needed). No aggregate price / no comparable-sales helper — sellers set price,
buyers compare visible listings (user decision).

## Sequencing (build order — ONE system)

The trade system is ONE codepath for any inventory item; "commons vs rares" is a DEPENDENCY +
rollout order, NOT a fork:

1. **Slice 1 — backbone.** `DepositItem` / list / buy / cancel / deliver-by-mail + the mailbox
   item-payload extension + basic search (slot / rarity). Reuse all L1 settlement. Works for ANY
   item; proven on common (client-rolled) gear where a dupe bug is cheap.
2. **Slice 2 — rich search/filter + UI.** Main-stat + substat + refine ranges, the full filter UI,
   item "My Listings".

Server-rolled RARES need NO new trade code — they flow through the SAME system automatically once
`server-roll-drops-design.md` grants them (they're verified items like any other). Optional later:
an extra provenance guard on high-value items (economy integrity / Steam cash-out) — an add-on, not
a system.

## Determinism / architecture

- `DepositItem` / `BuyItem` / `CancelItemListing` are `GameState.market_ops` (integer, deterministic,
  replay-recorded), extending the L1 `MarketOp` enum. `DepositItem`/`BuyItem` are Client-authority
  (self-cost); settlement is server-side on the verified save.
- The item ledger + escrow live server-side (verify-server), persisted in `market_kv`, exactly like
  `CommodityLedger`.
- No floats. The item payload crossing FFI/mail is the existing serde `Item` — no new float paths.

## Open decisions

1. Item identity for `DepositItem`: carry the full `Item` payload (replay verifies it matches the
   held item) vs an instance-id reference (needs a stable per-item id in `GameState`). Payload is
   simpler + matches the commodity pattern; decide during Slice 1.
2. Default sort + pagination for search results (price asc? recency?) at volume.
3. Substat filter granularity: presence only, value threshold, or exact roll ranges.
4. Listing duration / expiry (do listings persist forever, or expire + auto-return like some AHs?).
5. Per-account listing cap (anti-spam / anti-flood) — a light cap, echoing `crowd-scaled-rare-drops`
   supply thinking.

## Status

DESIGN, not built. Slice 1 (backbone + common gear) is the immediate target and mirrors the L1
commodity build closely; the genuinely new work is the item-payload mail extension and the substat
search. sim_version bump only when the `MarketOp` / `MailItem` serialized shapes change.
