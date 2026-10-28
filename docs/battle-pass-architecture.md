# Battle Pass — Architecture

**Status:** v1, 2026-07-22. Companions: `battle-pass-design.md` (what), 
`battle-pass-implementation-plan.md` (BP1–BP8 + phases), `economy-invariants.md`.

Core stance: **BP is a server feature that observes the sim.** The sim ships counters;
the server turns verified counter/tick deltas into XP, levels, and mail. The client is a
dumb renderer of server truth. No BP state ever lives in `GameState`.

```
┌────────────── Godot client ──────────────┐
│ bp_view (UIWindow)   hud badge           │
│   │ read state/deal     │                │
│   ▼                     ▼                │
│ sim_driver ── server_client (HTTP) ──────┼──►┌───────────── verify-server ─────────────┐
│   │                                      │   │ /save (existing verify)                 │
│   │ save push (existing)                 │   │   └─ bp::accrue(Δticks, Δcounters)      │
│   ▼                                      │   │ /bp/today   /bp/state                   │
│ SimBridge → sim-core                     │   │ /bp/claim ──► MailIntents ──► mailbox   │
│   (counters only; no BP knowledge)       │   │ /bp/buy (Steam MTX) /webhook/refund     │
└──────────────────────────────────────────┘   │ season job (rollover, auto-claim)       │
                                               │ Postgres: seasons/bp_* tables           │
                                               └─────────────────────────────────────────┘
```

## 1. Placement

| Piece | Lives in | Notes |
|---|---|---|
| Measurement counters | `sim-core` (`GameState` additive fields) | kills, rooms, refine_attempts, feeds, crafts, listings… monotonic u64s; already mostly exist |
| `Item.bound` | `sim-core` | serde default-false; silent no-op guards in list/deposit/vendor paths |
| Season state, XP, claims, entitlements, deals | `verify-server::bp` (new module) | Postgres tables, no in-memory-only state |
| Reward delivery | existing `mailbox` + GrantDrop path | BP emits MailIntents; zero new delivery code |
| UI | `godot/views/bp_view.gd` (UIWindow, layer 10) | renders server truth; claim = HTTP call then normal mail claim |

## 2. Data model (Postgres)

```sql
seasons        (season_id PK, starts_at, ends_at, seed BIGINT, config_json TEXT)
bp_state       (account_id, season_id, xp BIGINT, PK(account_id, season_id))
bp_daily       (account_id, season_id, day DATE, deal_json TEXT,
                done_mask SMALLINT, reroll_used BOOL, PK(account_id, season_id, day))
bp_claims      (account_id, season_id, track SMALLINT, slot SMALLINT,
                claimed_at, voided_at NULL, void_reason NULL,
                PK(account_id, season_id, track, slot))
                -- BP3 idempotency + permanent claim HISTORY: rows are NEVER deleted
                -- (deleting breaks idempotency + audit); refund clawback marks
                -- voided_at instead. Support trail: bp_claims -> mail row -> ClaimMail.
bp_entitlements(entitle_id PK, account_id, season_id, tier, source,     -- mtx | ops
                txn_id, active BOOL, revoked_at NULL)
mtx_txns       (txn_id PK, account_id, sku, status, order_id)           -- Init/Finalize dedup
```

All tables live in the SAME Postgres (Neon) as the verify-server — REQUIRED, not a
preference: BP1 accrual must share the /save commit transaction (separate DB would need
2PC). Growth is trivial (bp_daily ≈ accounts × days). Scale-to-zero already proven by
the refine tables.

`config_json` = the season catalog: `tracks[2][50]` ({def_id, qty, bound, kind}),
`sr_pool[5]`, xp curve (slot cost), day-burst values, week themes, daily catalog.
Content changes = new season row, no code deploy.

## 3. Flows

### 3.1 XP accrual (inside the existing /save verify transaction)
1. Save accepted (replay hash OK — BP1 gate).
2. `Δticks` = final_tick − last accrued tick, clamped by the additive-slack cap (BP2)
   → `time_xp = rate(Δticks)` (rate already capped ≈ 8 h/day by the cap itself).
3. `Δcounters` diffed against `bp_daily.deal_json` targets → set bits in `done_mask`
   → daily XP + all-3 bonus; weekly milestone = count of days with full mask.
4. `UPDATE bp_state SET xp = xp + …` — same DB txn as the save commit: a rejected save
   accrues nothing, a crash mid-verify accrues nothing (atomicity).

### 3.2 Daily deal (BP8)
`GET /bp/today` → `deal = pick(catalog, H(season.seed ‖ account_id ‖ date))`, insert-if-
absent into `bp_daily`. Pure function → auditable/replayable. Reroll: single UPDATE
guarded by `reroll_used = false`, redeal with a bumped hash salt.

### 3.3 Claim
`POST /bp/claim {season, track, slot}` (batch allowed):
level(xp) ≥ slot → track == premium ⇒ active entitlement (BP4) → `INSERT … ON CONFLICT
DO NOTHING` (dup = no-op, BP3) → on fresh insert: MailIntents from `config_json`
(free-track items stamped `bound = true`, BP6; SR selector slot carries `choice` param
validated against `sr_pool`). Player receives via the normal verified mailbox (BP5).

### 3.4 Purchase / refund
`POST /bp/buy {sku}` → Steam InitTxn → (overlay approve) → FinalizeTxn callback →
`mtx_txns` + `bp_entitlements` insert. Tier-skip SKU: adds XP server-side, clamped to
slot-50 cost. Refund webhook → `active = false, revoked_at = now()` → unclaimed premium
locks instantly; claimed hero items are still inside their trade-lock → instance voided
before it could ever trade (BP7).

### 3.5 Season rollover (job)
At `ends_at`: auto-claim every earned-but-unclaimed slot → mail; season closes read-only;
next season row activates. Idempotent (re-run safe).

## 4. Client (P4)

- `server_client.gd`: `bp_state()`, `bp_today()`, `bp_claim(slots)`, `bp_buy(sku)` —
  same thin await-HTTP shape as mail/market.
- `sim_driver.gd`: passthrough wrappers (views never touch server_client directly —
  existing convention).
- `bp_view.gd`: UIWindow (CanvasLayer 10, key-only Loc, resolve-on-display) — track grid
  50×2 (fingerprint-gated rebuild), XP bar (ProgressBar), daily panel (3 + reroll),
  claim-all button, season countdown from **server time** (never local clock).
- HudBar: badge dot when `claimable > 0` (data piggybacks on `bp_state()` polled on open
  + after save pushes; no new socket).
- Offline mode: BP screen shows "online only" (mailbox pattern).

## 5. Failure semantics

- The game is **always-online**: 3 failed connection confirms (~30 s) → client DCs to the
  session-ended screen, so play (and counters) STOP within seconds of losing the server.
  Consequence: verified deltas never lag more than one push interval — day-boundary
  attribution ("action at 23:59, save lands 00:01") is a seconds-wide edge, not hours.
- Server unreachable → accrual just pauses (XP derives from saves; nothing lost — the
  next accepted save carries the ticks). Client shows stale state read-only.
- Double-submit anywhere (claim/buy/refund/rollover) → idempotent by PK/txn design.
- Config error in a season row → season fails closed (no deals, no claims), previous
  season unaffected; fix = update row.

## 6. Test surface (maps to plan §3 attack checklist)

- Unit: xp curve, deal determinism, clamp math.
- Property (economy-invariants style): random interleavings of
  save/claim/buy/refund/rollover → assert BP1–BP8 never violated (esp. conservation:
  Σ granted per account ≤ config totals; no claim without level; no premium without
  active entitlement at claim time).
- E2E smoke: free track full loop on the dev server before any MTX code exists (P2 gate).
