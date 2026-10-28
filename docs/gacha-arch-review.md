# Gacha — architecture review (external-standards pass)

**What this is:** a read-only review of the gacha backend against how commercial gacha
systems are built. No source was changed; everything here is a proposal for whoever owns
`gacha.rs` / `commodity.rs` / the handlers in `main.rs`.

**What it is not:** the economics (`gacha-economy-design.md`) or the as-built description
and its own gap list (`gacha-system-reference.md` §10). This file adds only the "how does it
compare to industry practice" axis.

Symbol names are the durable reference; line numbers are as of `c15b6d0` and will drift.

---

## What already matches commercial practice

The findings below are narrow, and they sit on a structure that is sound. Worth stating
first so the list reads as a punch list, not a verdict:

- **Pure core split from I/O.** `gacha.rs` is data + integer math — no HTTP, no Postgres —
  and the transaction lives in the handler. The standard separation: the layer that DECIDES
  an outcome is testable without the layer that RECORDS it, which is why the rate tests are
  exact rather than fixture-driven.
- **Transaction ordering is right, step for step** (`gacha_roll`): claim the idempotency row
  first → `SELECT … FOR UPDATE` on wallet / pity / mileage → debit → roll → resolve EVERY
  payout before minting the first one. That last rule is stricter than most implementations:
  an unresolvable `def_id` aborts the whole pull instead of being skipped while the crystal stay
  debited.
- **The wallet is a ledger.** `crystal_ledger` holds signed deltas with a reason and a reference;
  the balance is the sum, not the source of truth.
- **Retry-safe rolls without state.** `gacha_seed` = `blake3(domain ‖ secret ‖ account_id ‖
  idem_key)`, so a retry that reaches the roll before the row committed reproduces the SAME
  result. Making the roll a pure function is a cleaner answer to replay than seeding from OS
  entropy and hoping the idempotency row always wins the race.

---

## P0 — the roll secret has a hardcoded dev fallback

`main.rs` (~566): `SERVER_ROLL_SECRET` → `REFINE_SECRET` → the literal `"dev-refine-secret"`.

The seed is a deterministic function of a value the CLIENT chooses (`idem_key`). That is fine
while the secret is unknown, and stops being fine the moment it is known — leaked, or simply
an environment where nobody set the variable. The attack is not guessing an outcome, it is
**selecting** one: enumerate `idem_key`s offline against the known secret, find one that
yields an SSR, send that key.

Two fixes, both worth doing:

1. **Fail closed on boot.** In production, refuse to start without `SERVER_ROLL_SECRET`
   instead of silently falling back to a constant that is published in the repository.
2. **Add a server-chosen nonce to the seed.** Generate a random nonce inside the transaction,
   store it on the idempotency row, hash it in alongside the secret. A retry re-reads the
   nonce from the row, so idempotency is preserved exactly — but the outcome stops being a
   function of anything the client controls, even against a leaked secret.

## P0 — the market layer shares one global consistency boundary  (mail half CLOSED 2026-07-31)

Gacha delivery mints into the in-memory `Mailbox`, which is persisted as one JSON value in
`market_kv`. The roll handler therefore holds the process-wide `market_persist_lock`
(`main.rs` ~1921/1963). Consequences:

- **Every pull, for every account, serializes through one mutex** — alongside unrelated
  market traffic. Not a correctness bug; a throughput ceiling, and an odd one to accept on
  the one path that takes real money.
- **A paid delivery inherits `market_poisoned`.** The market's failure state can hold a
  purchased item hostage.
- **Write amplification.** Appending one mail re-serializes the whole structure; cost scales
  with accumulated data, not with the size of the change.
- **Compensating code that a transaction would make unnecessary.** A failure path reloads the
  blob to undo an in-memory mint, and sets a poisoned flag when that cannot be confirmed.

### The unit of work is the market layer, not the mailbox

`commodity::reconcile_market(ledger, item_ledger, mailbox, consumed_ops, account, ops)` is a
pure synchronous function that mutates the commodity ledger, the item ledger AND the mailbox
together, and the caller commits **four** blob keys — `ledger`, `item_ledger`, `mailbox`,
`consumed_ops` — in one transaction. The mailbox is one participant in a single atomic
reconcile, not a component that happens to live in a blob.

So "move the mailbox to rows" does not buy what it looks like it buys:

- **Rows, but still a `BTreeMap` in memory** (storage swap only): the lock stays, because the
  in-memory copy must remain consistent with the ledgers across the same critical section.
  Wins write amplification, nothing else.
- **DB-only mail** (no in-memory copy): `reconcile_market` must become `async` and DB-aware —
  a rewrite of a pure, well-tested function that pulls the whole market layer with it.

The wallet, pity and mileage tables already refused the blob on the grounds that "this is real
money and must not sit behind a blob rewrite". The same reasoning applies here; it just
applies to four things at once.

### Two real paths

**(A) Move the market layer to rows** — commodity + item + mailbox + consumed-ops together.
Fixes every point above. Large, and it touches the save-verify path: its own project.

**(B) Split `reconcile_market` into decide / apply** — `decide` stays pure and keeps its
existing tests; `apply` performs the writes. Storage and behaviour unchanged.

**(B) is not an alternative to (A); it is (A)'s first commit.** A row migration needs an
apply layer regardless, so (B) is never wasted — and the system is WHOLE when it lands: same
storage, same behaviour, same tests, one extra seam. (A) stopped halfway leaves rows and blob
coexisting on a money path, which is the state worth avoiding.

Do (B) before the purchase path lands: once `/gacha/purchase` + Steam InitTxn/FinalizeTxn +
a refund webhook exist, this becomes a change to a payment path.

### (B) — DONE 2026-07-31

`reconcile_market` now takes `mailbox: &Mailbox` and returns its mailbox effects as data:

```rust
pub struct MailPlan { pub send: Vec<MailIntent>, pub consume: Vec<u64> }
pub fn apply_mail(mb: &mut Mailbox, plan: MailPlan)
```

DECIDE cannot touch the mailbox — the borrow checker enforces that, so the seam cannot
silently regress. Storage, behaviour and mail ids are unchanged.

Batching the effects after the op loop rather than interleaving them is provably equivalent:
`consume` only names mail that existed BEFORE the batch (phase 1 verifies against the
pre-state), `send` only allocates ids above every existing one, and `consume_claim` never
touches the id counter — so the two lists cannot overlap.

The commodity and item ledgers are deliberately still `&mut`. They are not the store being
moved, and giving them the same treatment is (A)'s work, not a precondition for it.

`reconcile_decides_mail_without_touching_the_mailbox_then_apply_lands_it` pins the seam and
covers a save that both claims old mail and mints new mail in one batch — a combination no
prior test exercised. Verified by sabotage: stubbing `apply_mail` to a no-op fails it and
`reconcile_market_full_loop_deposit_list_buy_via_mail`.

### Mail is rows — DONE 2026-07-31, steps 1–3 of the swap

Done as a clean cutover rather than a migration: the user confirmed the market and mailbox had
no live users, so the four `market_kv` keys were wiped instead of backfilled.

- `mailbox` table (`BIGSERIAL` id, `claimed_at` nullable) + `mailbox_store.rs`, the Postgres
  adapter. `mailbox.rs` stays a closed core so its fuzz harness keeps proving claim-once and
  conservation over random sequences; the model defines the rules, the adapter makes Postgres
  obey them, `pg_mailbox_rows_honour_the_model` checks that it does.
- All three mint paths (BP claim, gacha roll, gacha exchange) deliver as INSERTs in the same
  transaction that debits, so **none of them takes `market_persist_lock` any more** and none can
  inherit `market_poisoned`. That was the entire point of the finding.
- Claim-once is the UPDATE's own predicate (`WHERE claimed_at IS NULL` + `rows_affected`), not
  read-then-write, so two concurrent claims cannot both succeed.
- The reconcile still runs on the PURE `&Mailbox` it got in (B) — `snapshot_for_claims` hydrates
  just the mails a save names, so DECIDE never learned about Postgres.
- `MailIntent` gained a `reason`, because delivery used to hardcode `"market"` / `"item-market"`
  and a cancel-return would otherwise have lost its provenance (economy-invariants I6).

Sabotage-checked: stubbing `apply_plan` fails `pg_atomic_commit_and_roundtrip`; dropping
`AND claimed_at IS NULL` fails the claim-once assertion with a literal double grant.

### `consumed_ops` is rows — DONE 2026-07-31

Taken next because it was the only blob key that grew **without bound**: the client's
`GameState.market_ops` is append-only and is never trimmed anywhere in the codebase, so a client
re-presents every op it has ever made on every save, and the server-side set has to remember all
of them forever. As one JSON value that meant every market write re-serialized every player's
entire trading history.

As `consumed_market_ops(account_id, op_id)` the write is one INSERT per NEW op, and the read is a
lookup of just the ids the current save presents — O(ops in the bundle), not O(history), which is
better than the blob was even before the growth. Same shape as the mailbox: read a minimal
snapshot before the sync locks, hand the pure reconcile the `&HashSet` it always took, write the
marks inside the settlement's own transaction.

`MarketWrite` groups blobs + mail + consumed so the transactional writer does not grow a
parameter per store that leaves the blob.

⚠️ **This does not fix the underlying growth, only where it lands.** `market_ops` itself still
grows forever inside `GameState`, so it is inside the state hash and inside every save blob and
every replay. Postgres does not care about a few million narrow rows; the save does. Trimming it
needs a sim-core change (drop ops the server has confirmed settled, at re-anchor) and a `VERSION`
bump — worth doing before the market sees real volume, and NOT done here.

**Step 4 — the commodity + item ledgers — is deliberately NOT done.** They are a
different kind of thing: a matching engine whose correctness rests on `commodity.rs` being a
closed system that `fuzz_conservation_and_atomicity` can drive, which `docs/economy-invariants.md`
leans on for I1/I3/I5/I7. Moving them to SQL trades that harness for row storage on a ledger that
is Gil-only (not the real-money path) and whose blob is rewritten only when someone trades. The
lock that remains guards those blobs alone.

## P1 — banner tables are compile-time constants

`gacha::CHARACTER` / `gacha::WEAPON` are `const`, so rotating a banner or correcting a rate is
a deploy. Commercial systems keep this as server-side configuration with a version and an
active window — and, the part that matters more than the convenience, **record which config
version produced each pull**.

`gacha_rolls.results_json` stores what a pull produced but not the rates in force at the time,
so reconstructing "what were the odds that day" depends on git archaeology lining up with
deploy times. A config version (or the rate block itself) on the roll row makes each pull
self-describing.

## P1 — nothing measures actual rates against published ones

The audit trail exists (`results_json`); the check does not. In code this is one aggregate
query plus a test that runs it against a production snapshot: pull counts by rarity vs the
published table, with a tolerance band.

It is the only artifact that can demonstrate the code pays what the UI advertises — the
cheapest item on this list, and the most awkward to build after someone asks for it.

## P2 — PCG32 decides money outcomes

`Pcg32` is not a CSPRNG; given enough output its stream is reconstructible. Today that is
covered — the stream is keyed through blake3 with a secret and raw draws are never exposed.
Listed to make the dependency explicit: **the whole guarantee rests on one secret**, which is
why the first P0 matters more than it looks.

---

## Suggested order

1. Fail-closed boot on `SERVER_ROLL_SECRET` — minutes, removes a silent production hazard
2. Server nonce in the seed — small, closes outcome-selection for good
3. Rate-verification query + test — cheap, and it is the evidence artifact
4. ~~`reconcile_market` → decide / apply, then mail to rows~~ — **done 2026-07-31**. The money
   path is off the blob and off the lock; only the Gil ledgers still use `market_kv`, on purpose
   (see P0)
5. Banner config as versioned data — with the first real rotation, not before

---

## Provenance

Reviewed by a session that does not own this code, against external practice rather than
against the design docs. One correction has already been folded in: the first draft scoped
P0's second item as "move the mailbox out of the blob"; the owning session pointed out
`reconcile_market`'s three-way atomic mutation, which makes the market layer the real unit of
work. That correction is integrated above rather than appended — the lesson worth keeping is
that an architecture review has to trace **who mutates this data**, not only **how it is
stored**.
