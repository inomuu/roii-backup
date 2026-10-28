> **RETIRED 0.0.370 — refine no longer exists.** The whole +N ladder (and this server-roll
> protocol with it) was deleted when gear moved to the GRADE/tier model: with power carried by
> the piece's grade, nobody sinks ore into an item they are about to fuse away
> (user 2026-09-15). `/refine/roll`, the nonce, the outstanding/consumed reconcile and the
> bind-check are gone from `verify-server`; the `refine_outstanding` / `refine_consumed` tables
> are left undropped on purpose. Kept as the reference for how a SERVER-ROLLED, instance-bound
> outcome is built — the same shape any future high-value roll (fusion, crystal cutting) would use.
> Current gear design: `docs/gear-fusion-and-base-fix-design.md`.

# Server-roll refine — Phase 2b design (verify-server)

Implementation design for the server side of server-rolled refine. Reads on top of
`docs/s3-economy-design.md` §6.4 / §6.4.1 (the decided model) and the `server-roll-refine`
memory. Phase 1 (sim carries `server_roll`) and Phase 2a (`Item.instance_id`, stamped on
first refine) are DONE + committed (0.0.175 / 0.0.176). This doc is 2b — NOT yet built.

## Goal

The server issues refine rolls that are **bound** (to one item+level+attempt) and **final**
(can't be un-asked or reassigned), stores only a small self-clearing set, reconciles cheaply
on sync, and fully enforces at promotion — so refined gear can't be cheated into the tradeable
economy. Keep the server LIGHT (approach A): it rolls + records; the **sim still applies** and
replay stays the source of truth. The server never holds live item state.

## 0. When is a server roll even required?

Only for a refine whose published success is **< 100%**. A 100%-success refine (every curve's
low levels — e.g. iLv 1-2 is 100% through +7) is **deterministic**: it can't be steered, so the
sim applies it with `server_roll = 0` and the verifier accepts that. Server rolls are issued
**only for risky levels** (`odds_for(...).success_bp < 10_000`).

This also resolves the **chicken-and-egg** of the lazy `instance_id`: an item is stamped with
its id on its FIRST refine (a safe, 100% level), so by the time it reaches a RISKY level (the
first one needing a server roll) it already has a stable id to bind to. Bonus: safe early
refines need no server round-trip at all.

> Legacy note: a pre-2a save may hold an already-refined item with no id. `migrate_item` should
> stamp an id for any refined-but-idless gear so its next (possibly risky) refine can bind.

## 1. Nonce derivation (deterministic, secret-keyed)

```
server_roll(nonce) = keyed_hash(server_secret,
                                account_id ‖ seq ‖ instance_id ‖ refine_level) : u64
```
- **server_secret** — per-server, env-only, NEVER shipped. The one true secret (Kerckhoffs:
  everything else — sim, odds, draw order — can be public).
- **seq** — the account's cumulative RISKY-refine counter. Makes the SAME (item, level) rolled
  twice (after a downgrade) produce a DIFFERENT nonce → no deterministic re-roll of a lucky level.
- **instance_id + refine_level** — binds the roll to THIS exact item at THIS level → a revealed
  roll is useless on any other item/level (closes target-selection).

The sim reads the outcome as `roll_outcome(nonce % 10_000, odds_for(ilv, level))` — the SAME
published table (odds honesty intrinsic; verifier reproduces it in replay).

## 2. Server state (small, durable, self-clearing)

Per account: a **RISKY-refine sequence counter** + an **outstanding-roll set**:
```
outstanding[account] : Vec<{ seq, instance_id, refine_level, nonce }>   # issued, not-yet-consumed
verified_refine_count[account] : u64                                     # seq of the last VERIFIED refine
```
- Issue a roll → `seq = verified_refine_count + outstanding.len() + 1`; append to `outstanding`.
- Reconcile a synced bundle → drop the consumed entries from `outstanding`, bump
  `verified_refine_count`.
- **Durable** (survives disconnect): the `(seq → instance_id, level, nonce)` binding must persist,
  so abandon+resume re-serves the SAME binding (no re-roll, no rebind — see §6).
- NOT stateless (a stateless server can't catch peek-and-skip); NOT live item state (that's B).

## 3. `POST /refine/roll` (the commit-and-reveal endpoint)

Request (authenticated): `{ instance_id, refine_level }` (the item + its CURRENT level).
For a FIRST-refine-at-a-risky-level item the client sends the id it is about to stamp — but by
§0 that can't happen (risky ⇒ already refined ⇒ already has an id), so `instance_id` is always a
real, existing id.

Server:
1. If `odds_for(ilv, level).success_bp == 10_000` → reject (safe refines need no roll; client
   applies deterministically).
2. `seq = next seq`; `nonce = keyed_hash(secret, account‖seq‖instance_id‖level)`.
3. **Record** `{seq, instance_id, level, nonce}` in `outstanding` (durable) — issuing IS committing.
4. Return `{ seq, nonce }`.

**Roll-is-final:** once issued, `seq` is bound to `(instance_id, level)` forever until consumed.
Re-requesting the same `(instance_id, level)` while it's the head outstanding returns the SAME
`(seq, nonce)` (idempotent) — the client cannot re-roll or rebind.

## 4. Client flow (sim side — mostly done)

1. Player refines item X at a risky level → `sim_driver` calls `/refine/roll {X.instance_id, level}`.
2. Server returns `nonce`. Client injects `StartRefine{…, server_roll: nonce}` (Phase 1 already
   carries the field); the sim rolls from it (Phase 1) and applies.
3. Safe (100%) refines skip the server entirely (`server_roll: 0`, deterministic).
4. Offline / no connection → the Refine button is disabled ("need connection") for risky refines;
   safe refines still work. (Client UI — coordinate with the UI session; `.gd`.)

## 5. Verifier enforcement

**Sync-time (cheap, every bundle — log-scan, NOT replay):** walk the bundle's `input_log`, pick
out `StartRefine{,Inventory}` events in order. For each RISKY one, it must consume the next
`outstanding` entry in order: same `instance_id` + `refine_level` + `server_roll == nonce`. A
gap, reorder, rebind (wrong item), forged nonce, or a risky refine with `server_roll == 0` →
**reject / flag**. Consumed entries drop from `outstanding`; `verified_refine_count` advances.

**Promotion-time (100% replay, rare — the trade edge):** the full replay recomputes each risky
refine's expected nonce from `(secret, account, seq, item.instance_id, level_before)` and the
outcome from `odds_for`, and confirms the item's provenance. Enforces the same consume-all-in-order
+ bind + no-forgery, now against the authoritative replay. A fake/peeked item can't be promoted.

Peek-and-skip is caught because a skipped risky refine leaves an **outstanding entry that never
gets consumed** — visible to the server even though the client's log shows no trace.

## 6. Abandon / resume

If the client abandons without syncing, `outstanding` keeps the issued bindings. On resume the
server re-serves the SAME `(seq → instance_id, level, nonce)` for any re-request → the outcome is
unchanged (no re-roll) and can't be rebound to another item. `verified_refine_count` only advances
on a VERIFIED sync, so the sequence can't be rolled back to grind.

## 7. Adversarial test matrix (must pass before "done")

- **peek-and-skip** — issue a fail roll, drop the refine from the log → outstanding-unconsumed → reject.
- **target-selection** — apply a roll issued for item X to item Y → bind mismatch → reject.
- **abandon-retry (same item)** — re-request after abandon → identical nonce → same outcome.
- **abandon-retry (different item)** — re-request seq for a different item → server refuses to rebind.
- **reorder** — consume issued rolls out of order → sequence mismatch → reject.
- **forge** — arbitrary `server_roll` on a risky refine → nonce mismatch → reject.
- **safe-forgery** — claim `server_roll = 0` on a <100% refine → reject (risky needs a bound roll).
- **replay-of-nonce** — reuse a consumed nonce for a second refine → not outstanding → reject.
- **determinism** — same secret + inputs → identical nonce + outcome across server + replay.

## 8. Build order (2b)

1. `server_secret` (env) + `keyed_hash` nonce fn (blake3-keyed) mirroring the sim's `nonce % 10_000`
   read. Unit-test determinism + sim agreement.
2. `outstanding` store + `verified_refine_count` (in-memory first, Postgres when S2 storage lands).
3. `POST /refine/roll` (issue + commit + idempotent re-request).
4. Sync-time log-scan reconcile in `/save` /`/submit` (consume-all-in-order + bind).
5. Promotion-time full enforcement in the promotion/replay path.
6. The adversarial test matrix (§7).
7. Client wiring (`sim_driver` fetch + inject; risky-refine gating) — coordinate with the UI session.

## Status

- ✅ Phase 1 (0.0.175) — `StartRefine{,Inventory}.server_roll`, roll-from-nonce, authority.
- ✅ Phase 2a (0.0.176) — `Item.instance_id`, stamped on first refine, stable + replayable.
- ✅ 2b slice 1 (b017c88) — `POST /refine/roll` + `refine_nonce` + outstanding store.
- ✅ 2b slice 2 (c756789, 0.0.177) — sim `Item.refine_attempts` (monotonic → fresh roll).
- ✅ 2b slice 3 (f171c04) — `reconcile_refine` (consume-all + forgery, nonce-set).
- ✅ 2b slice 4 (4658c35) — `verify_refine_binding[s]` (target-selection / risky-no-roll) +
  reconcile, both wired into `/save`; reconcile idempotent via a `consumed` set.
- ✅ 2b slice 5 (0a38b91) — CLIENT wiring (`.gd`): `server_client.refine_roll` +
  `sim_driver.server_refine_roll`; `start_refine{,_inventory}` carry the nonce;
  `refine_view` fetches it for RISKY refines when ONLINE (blocks if none issued), and
  client-rolls "0" for safe refines + the whole offline alpha build. `instance_id` +
  `refine_attempts` already ride the item view (serde). NEEDS an in-game test vs a running
  verify-server (a real-save launch is forbidden → the USER runs it).
- ✅ 2b persistence (Neon Postgres) — `refine_outstanding` + `refine_consumed` tables +
  write-through (issue / reconcile) + startup reload. Survives scale-to-zero: PROVEN live on
  Fly (issue → machine restart → same request returns "idempotent" = the roll reloaded from
  Postgres, no false-forge). The user chose the per-refine record (auditable, low volume) over
  a lighter counter/seq-in-nonce scheme.
- ⏳ later — promotion-path (register/ship) can reuse `verify_refine_binding`; prune the
  `consumed` set on anchor-advance; fold the bind-check into the single `verified_state`
  replay (currently a second pass).
- Deferred — +10 global announce (needs a push channel; server-roll makes it trustworthy).
