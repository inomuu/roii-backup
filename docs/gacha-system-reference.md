# Gacha — System Reference (as built)

> **Status: BUILT and TESTED, NOT DEPLOYED. Snapshot of 2026-07-31.**
> This documents what the code actually does today. Expect it to change — the rate table and the
> price are explicitly live-tunable, and several pieces below are stubs waiting on content or on
> the Steam transaction slice.
>
> Companion docs, do not duplicate them here:
> - **`gacha-economy-design.md`** — the numbers and WHY they are those numbers (rates, pricing,
>   F2P pacing, competitor calibration). If you are changing a number, start there.
> - **`economy-invariants.md`** — I1–I12, the "money can't do X" spine. I11 (crystal buckets) and
>   I12 (mileage) are the two that constrain this system.

---

## 1. What exists, what does not

| Piece | State |
|---|---|
| Roll core (rates, 50/50, x10 SR floor, payout resolution) | ✅ built, 19 unit tests |
| Mileage / Summon Shards (earn + exchange) | ✅ built |
| Crystal wallet, two buckets, ledger | ✅ built |
| `POST /gacha/roll`, `POST /gacha/exchange`, `GET /gacha/state` | ✅ built, idempotent, pg-tested |
| Delivery into `owned_exemplars` / `owned_regalia` | ✅ built (sim-core `bank_collectible`) |
| Banner screen (`gacha_view.gd`) with shard exchange | ✅ built |
| `POST /dev/gacha-crystal` (dev grant) | ✅ built — **the only way crystal enter today** |
| **`POST /gacha/purchase`** | ❌ **not built.** No Steam InitTxn/FinalizeTxn anywhere; `mtx_txns` is DDL-only. This is the whole revenue path. |
| **SR content** | ❌ `sr_pool` is empty on both banners. An SR pull is RECORDED as SR (the x10 floor counts it) but PAYS `regalia_fodder`. |
| Deploy | ❌ not deployed; the live server still runs the old soft-pity table |

**Content inventory (the real constraint):** 2 Exemplars (`exemplar_seraphine`, `exemplar_lucielle`),
2 real Regalia (`regalia_aurora_edge`, `regalia_litany_of_dawn`) + `regalia_fodder`, 0 SR.

---

## 2. Where the code lives, and why it is split that way

```
crates/verify-server/src/gacha.rs   PURE core — rates, roll, wallet, mileage. No axum, no sqlx.
crates/verify-server/src/main.rs    handlers + SQL + the transaction. Tables in migrate().
crates/sim-core/src/data/build/items.rs   bank_collectible() — where a delivered unit lands
godot/net/server_client.gd          HTTP
godot/sim_driver.gd                 gacha_state / gacha_roll / gacha_exchange (+ auto-claim mail)
godot/views/gacha_view.gd           the screen
godot/i18n/strings.csv              UI_GACHA_*
```

`gacha.rs` holding zero HTTP and zero SQL is the house convention (`bp.rs`, `commodity.rs`,
`mailbox.rs` are the same shape). It is what lets the banner maths be unit-tested without a
database — 19 of the 21 gacha tests need no Postgres.

**The roll is server-side and never in the sim.** A pull is money: a client-side draw could be
save-scummed, and adding a draw to the sim's single PCG32 shifts every downstream outcome in the
game (the butterfly rule in CLAUDE.md). The sim only ever sees the *delivered item*, via mail.

---

## 3. The roll

### Rates (`gacha::CHARACTER` / `gacha::WEAPON`)

Both banners: **`base_bp: 400` = 4% SSR flat. `soft`/`hard`/`ramp_bp` all 0 = no pity of any
kind.** `sr_bp` 900 (character) / 1200 (weapon) on a non-SSR pull; the remainder is R.

| | `fifty_fifty` | `carry_guarantee` | effect |
|---|---|---|---|
| Character | `true` | `false` | an SSR is 50/50 featured; **losing arms nothing** |
| Weapon | `false` | `false` | every SSR IS the featured weapon |

⇒ ~25 pulls per SSR, ~50 per featured character copy, ~25 per weapon copy, from rolls alone.

**Zero means disabled, and that is deliberate.** `ssr_chance_bp()` guards on `r.hard > 0` and
`r.soft > 0`, so the entire soft-pity ladder is still live code that simply is not engaged.
Reverting to the old 1%/74/90 ladder is an edit to two `Rates` constants — no logic changes —
and `the_disabled_pity_ladder_still_works_when_re_enabled` /
`the_carry_guarantee_works_when_re_enabled` exist specifically to keep that path from rotting.

### Mileage — read this before touching the economics

**1 ticket per pull, on every banner, into ONE shared account-wide pool, minted regardless of
what the pull produced. Never expires. `MILEAGE_COST` = 200 buys any `EXCHANGE_POOL` unit.**

⚠️ **Mileage is NOT pity.** Read as a ceiling ("you get the unit if you reach 200 without it") it
pays only the unlucky. Read correctly — a flat faucet paying *every* player — the two sources
**add**: 1/50 + 1/200 = **1 character copy per 40 pulls**; 1/25 + 1/200 = 1 weapon copy per 22.
Getting this wrong overstates the revenue ceiling by 45%. Pinned by
`mileage_pays_lucky_players_too_so_the_sources_add`, which asserts both the combined ~40 AND the
rolls-only ~50 so the two can never be conflated again.

`EXCHANGE_POOL` deliberately includes `regalia_litany_of_dawn`, which is on **no banner** — being
able to buy an off-rotation unit is the point of the pool, not an oversight.

### Payout resolution (`payout_def`)

SSR → featured, or uniformly from `ssr_off` on a lost 50/50. SR → `sr_pool`, **falling through to
`r_item` while that pool is empty**. R → `r_item` (`regalia_fodder`). An empty `ssr_off` falls
back to the featured unit: *a player must never be charged for an empty result.*

`gacha_item()` (main.rs) turns a def_id into an `Item`: catalog lookup first, then a code-only
`regalia_*` construction (Regalia carriers have no catalog defs by design — see
`items::regalia_def`).

### RNG

`gacha_seed()` = blake3 over `"gacha-roll:v1"` + server secret + account_id + length-prefixed
idem_key → `Pcg32`. Keyed by the secret so outcomes are unguessable; domain-separated from the
refine and drop nonces; **deterministic in the idem_key**, so a retry that reaches the roll before
the idempotency row committed still produces identical results.

---

## 4. Money and safety

### Buckets (invariant I11)

`Wallet { paid, bound }` (`paid` was `tradeable` until 2026-10-01 — crystal never trade). Only crystal bought with money are `paid`; faucet / pass /
bonus crystal are `bound`. **They never convert in either direction and are never summed into one
displayed balance.** `spend()` drains `bound` FIRST (matching
`refine::consume_n_stackable_bound_first`), is all-or-nothing, and reports the split so the ledger
can record it. Fuzzed by `fuzz_bucket_separation_and_conservation` over every intermediate state,
not just the endpoint.

### Mileage separation (invariant I12)

Its own table, never a column on `crystal_wallet`. Cannot be bought; one way in (1/pull), one way out
(exchange at 200). If shards could become crystal that is an RMT back door; if crystal could become
shards you would be buying the backstop directly, which defeats its purpose.

### Transaction shape

Both mutating endpoints follow `bp_claim`'s pattern exactly:

1. fast path — if `idem_key` already exists, replay the stored result, charge nothing
2. take `market_persist_lock`, `db.begin()`
3. claim the idem row `ON CONFLICT DO NOTHING`; `rows_affected() != 1` → **409, already in flight**
4. `SELECT … FOR UPDATE` on wallet / pity / mileage — no double-spend under concurrency
5. roll (or exchange), mint into the in-memory `Mailbox` **in a sync scope** (the guard must not
   cross an `await`)
6. every write + the serialized mailbox blob commit in the SAME transaction
7. on failure: `load_market(...)` to undo the in-memory mint; if that cannot be confirmed, set
   `market_poisoned` and fail closed

Consequence worth knowing: mail is persisted through the `market_kv` JSON blob, so a gacha mint
inherits the `market_poisoned` flag. The crystal **wallet** is its own per-row table — only the
delivery half rides the blob.

### Delivery chain

```
server rolls → mailbox.send_item(Reward) → client sim_driver auto-claims
    → ClaimMail (InputAuthority::Server, exactly-once, all-or-nothing)
        → bank_collectible():  Exemplar → owned_exemplars
                               Regalia  → owned_regalia
                               anything else → false (falls through to inventory)
```

`bank_collectible` is **uncapped and skips the room check** — a paid pull must never fail because
a bag was full. It is one helper called by both the shop path and `try_grant_item`; the shop had
this rule and the grant path did not, which is exactly the bug class this consolidation prevents.

---

## 5. Tables (`migrate()` in main.rs)

Real per-row tables, **not** the `market_kv` JSON snapshot: this is real money and must not sit
behind a blob rewrite.

| table | key | holds |
|---|---|---|
| `crystal_wallet` | `account_id` | `paid`, `bound` |
| `crystal_ledger` | serial | signed `d_paid` / `d_bound`, `reason`, `ref`, `at` — the I1/I7 audit trail |
| `gacha_pity` | `(account_id, banner_type)` | `count`, `pulls_since_sr`, `lost_5050`. Per TYPE so a rotation never resets. **`count`/`lost_5050` are inert under the flat rate**; `pulls_since_sr` still drives the x10 floor. |
| `gacha_rolls` | `idem_key` | idempotency + `results_json` audit |
| `gacha_mileage` | `account_id` | `tickets` |
| `gacha_exchanges` | `idem_key` | idempotency + `def_id`, `spent` |

---

## 6. HTTP API

All bodies carry `ok: bool` + `reason: String`. `401` unauthorized · `503 "gacha needs a
database"` · `409` business conflict · `500 "persist failed — retry"`.

⚠️ **u64 crosses as a decimal STRING.** Godot's JSON parser is float-based and loses precision
past 2^53. Balances and ticket counts are strings on the wire; the client does
`int(String(x))`.

### `GET /gacha/state`
```
{ ok, paid, bound, cost_per_pull,          // strings; + legacy `tradeable` (= paid) until Unity reads `paid`
  mileage, mileage_cost, mileage_per_pull,      // strings
  exchange_pool: [def_id],
  pity: { character: {count, pulls_since_sr, lost_5050}, weapon: {…} },
  banners: [{ id, kind, featured, base_bp, soft, hard, ramp_bp,
              fifty_fifty, has_pity, carry_guarantee }] }
```
`has_pity` exists so the UI can tell "no pity" from "a counter at zero" — rendering `0 / 0` reads
as a broken meter. Odds are disclosed on every banner: a design-doc guardrail and a store
requirement in several markets.

### `POST /gacha/roll` → `{banner_id, count: 1|10, idem_key}`
Returns `{ok, reason, pulls: [{def_id, rarity, featured}], mails, paid, bound, pity}`.
**Rarity comes from the PULL, not the delivered item** — an SR pays fodder today, so reading the
item would show "Rare" and hide the hit.

### `POST /gacha/exchange` → `{def_id, idem_key}`
Returns `{ok, reason, def_id, mails, mileage}`. Rejects non-pool defs with 400 **before** any
charge; resolves the carrier item before opening the transaction.

### `POST /dev/gacha-crystal` → `{account_id, paid, bound}` (`tradeable` accepted as an alias)
Dev-only faucet. `GREATEST(…, 0)` guards a negative grant.

---

## 7. Client

`gacha_view.gd` is a `UIWindow` (**`Node2D`, not `Control`** — a `Control` node silently never
instantiates and the hotkey does nothing). It renders server truth only; it decides no outcome.

Two things in it are load-bearing:
- **Rarity badges read `pull.rarity`**, never the delivered item — see above.
- **Exchange buttons are built ONCE and thereafter only have `disabled` updated.** The view
  refreshes every 15 s, and free+recreate of Buttons inside a periodic refresh is this codebase's
  recurring "button stops responding" bug. Banner rows still rebuild per refresh — pre-existing,
  worth migrating.

Loc keys: `UI_GACHA_*` in `strings.csv`. `UI_GACHA_ODDS` (flat) and `UI_GACHA_ODDS_LADDER`
(pity) both exist so the revert path keeps working, as do `UI_GACHA_PITY` /
`UI_GACHA_GUARANTEED`. `Loc.ui()` has **no EN fallback** — a missing key renders as the key.

---

## 8. Test coverage — and what it does not cover

**19 unit tests** in `gacha.rs` (no DB) + **2 pg tests** in `main.rs` that execute the real SQL in
the real statement order. Run:

```bash
cargo test -p verify-server gacha::                       # pure core, fast
TEST_DATABASE_URL="postgres://$(whoami)@localhost/roi_test" \
  cargo test -p verify-server -- --test-threads=1         # incl. pg
```

The pg tests exist because the SQL had only ever been type-checked: a wrong column, a bad
`ON CONFLICT` target or an overflowing cast all compile and only fail in production.

**NOT covered:**
- The whole Godot client. No `.gd` file is exercised by anything, and the FFI/JSON boundary is
  asserted on neither side — a renamed key renders empty rather than erroring. Every shape change
  must be seen in a running game.
- The handlers themselves end-to-end. The pg tests replay the handler's *statements*, not the
  handler.
- `market_poisoned` / rollback behaviour under a real mid-transaction failure.
- Anything about price. Those numbers live only in the design doc.

---

## 9. Recipes

**Retune rates** → edit `CHARACTER` / `WEAPON` in `gacha.rs`. The three average-pull tests will
fail with the new figure in the message; update them and the design doc's tables together.

**Revert to soft pity** → set `soft`/`hard`/`ramp_bp` back (e.g. 74/90/600) and
`carry_guarantee: true`. No other code changes; the ladder tests already prove the path works.

**Add a banner** → append to `BANNERS`. It is data: *"Content, not logic — extend this list, not
the roll."* Add a `UI_GACHA_KIND_*` loc key if the type is new.

**Add an exchangeable unit** → append to `EXCHANGE_POOL`. `the_exchange_pool_spans_off_banner_units…`
asserts every banner's featured unit is present.

**Fill the SR tier** → put def_ids in `sr_pool`. Explicitly "no code change needed". Consider
raising `sr_bp` at the same time — it is low today partly because an SR currently pays fodder,
and a high SR rate that hands out fodder reads as a lie.

**Change the shard threshold** → `MILEAGE_COST`. Remember it changes the *combined* acquisition
rate (§3), not just the worst case.

---

## 10. Known gaps and likely future changes

1. **No purchase path.** Everything above is exercised by dev grants. `/gacha/purchase` + Steam
   InitTxn/FinalizeTxn + a refund webhook is the biggest remaining slice, and it is the first
   real-money transaction in the codebase — treat it as its own project.
2. **Storefront/claimables must be server-side DATA when built.** The client is mobile-shaped and
   mobile is a plausible future platform; Steam wants a lean store and mobile wants a dense one.
   Deciding this is free today and a rewrite after the first hardcoded purchase handler ships.
3. **Empty SR tier** is the largest felt gap — it is what makes a pull feel like nothing happened.
4. **`gacha_pity.count` / `lost_5050` are dead columns** under the flat rate. Left in place
   deliberately for the revert path; drop them only if the flat rate is made permanent.
5. **Hover-only tooltips** affect this screen like every other. Mobile/touch has no hover.
6. **Content cadence, not rates, is the real economy risk.** The pacing figures in the design doc
   assume a banner rotation. With 2 Exemplars there is no rotation, and shard portability
   "across banners" has nothing to be portable between yet.
