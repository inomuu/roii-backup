# Battle Pass — Secure Implementation Plan

**Status:** plan v1, 2026-07-22. **Design:** `battle-pass-design.md`. **Safety spine:**
`economy-invariants.md` (I1–I10), quest 3-layer split, server-roll-drops (GrantDrop/mail),
refine keyed-nonce idempotency, competition-integrity fast-forward cap.

Principle: **the sim never knows the pass exists.** The sim carries counters; the SERVER
owns season state, XP, levels, claims, entitlements. Every reward enters the game through
the already-verified mail/grant path. This is the same split every shipped online system
here uses — nothing novel = nothing newly exploitable.

---

## 1. Invariants (BP1–BP8)

| # | Invariant | Enforced where |
|---|-----------|----------------|
| BP1 | BP XP derives ONLY from replay-verified progress: tick deltas + measurement-counter deltas of ACCEPTED saves. No client-claimed XP input exists. | server, on /save verify |
| BP2 | Online-time XP inherits the **additive-slack fast-forward cap** — BP can never accrue faster than wall-clock, even with a hacked client pushing fat saves. | existing cap in /save |
| BP3 | `level = f(xp)` computed server-side only; **claims are idempotent** per (account, season, track, slot) — the refine-nonce pattern. Replay/retry never double-grants. | server DB unique key |
| BP4 | Premium claims require an **active entitlement at claim time**; retroactive unlock re-checks entitlement, never caches it client-side. | server |
| BP5 | Rewards enter the game ONLY via the verified **mail / GrantDrop** path (provenance I6: every granted item traces to a server grant record). | existing mail infra |
| BP6 | FREE-track items mint with **`bound = true`**: Gil-market list = sim no-op + verify reject, Steam promote = reject, vendor `sell_value = 0`. | sim + verify + promote |
| BP7 | **Refund clawback**: pass refunded → entitlement revoked → unclaimed premium locks; already-claimed hero items are still inside their trade-lock (lock ≫ Steam's 14-day refund window) → revoke/void the instance cleanly before it can ever trade. | server + trade-lock |
| BP8 | The day's daily set is **dealt deterministically** (season_seed + date + account_id) and logged — auditable, replayable, no reroll-fishing (reroll consumes the day's single reroll credit server-side). | server |

Bot economics recap (design layer): dailies award BP only, free track is bound-unsellable
everywhere → a free bot account can extract NOTHING liquid from the pass. The only liquid
rewards (SR Exemplar/Regalia) cost a paid pass per account per season.

## 2. Phases (each lands green + testable before the next)

### P0 — sim substrate (smallest, offline-testable)
- Audit measurement counters; add the few missing ones (refine_attempts, feed counts,
  craft count, market-list count already exist or are trivial additive fields).
- **`Item.bound` flag**: skip-nothing serde field; guards in sell/list/deposit paths as
  silent no-ops (replay-safe); vendor path returns 0. Golden + `anchor_roundtrip` cover.
- InputEvent audit: rewards arrive via the existing server-authored grant events
  (InputAuthority::Server) — likely zero new events for the pass itself
  (UseExpTicket etc. come with their items, separately classified).
- VERSION bump + bless. **No pass logic in the sim.**

### P1 — server season core
- Tables: `seasons` (id, start/end, seed), `bp_state` (account, season, xp, PK),
  `bp_claims` (account, season, track, slot, PK — BP3), `bp_entitlements`
  (account, season, tier, source, active_until, revoked).
- XP accrual inside the EXISTING /save verify hook: accepted save → Δticks (BP2-capped)
  → time-XP; Δcounters vs the dealt dailies → daily/weekly XP (BP1).
- Daily deal endpoint `GET /bp/today` (BP8) + reroll (one credit/day, server-side).
- Unit + property tests: xp monotonic, cap holds under fat-save attack, deal determinism.

### P2 — claims + free track E2E
- `POST /bp/claim {season, track, slot}` → validates level ≥ slot + idempotent insert →
  emits MailIntents (existing mailbox) → client claims from the mailbox as usual (BP5).
  Free-track mints carry `bound=true` (BP6).
- Property tests: double-claim, claim-below-level, mail-claim idempotency (existing suite
  pattern); invariant harness run like economy-invariants.
- **Milestone: free track fully live without any money code.**

### P3 — monetization
- Purchase: Steam MTX (InitTxn/FinalizeTxn server-side) → entitlement row. Tier-skip SKU
  = +N levels (server adds XP, capped at slot 50 — never past content).
- Premium claims: BP4 check + retroactive (client may batch-claim earned slots).
- **Refund webhook** → revoke entitlement → BP7 clawback path; hero-item instances carry
  their trade-lock so revocation precedes any tradability.
- SR grant plumbing: SR Exemplar/Regalia = the existing Exemplar-entitlement/grant shape
  with a `tier` field; selector = claim param (validated against the season's pool of 5).

### P4 — client UI (pure read + claim calls)
- BP screen as a UIWindow (Controls, CanvasLayer 10, key-only Loc, resolve-on-display):
  track grid ×50, XP bar, daily panel (3 + reroll), claim buttons, season timer
  (server time). HudBar badge when claims are waiting. The client renders server truth;
  it computes nothing.

### P5 — content + season ops
- Season-1 catalog: the 50-slot table as a server-side season config (JSON), SR pool of 5,
  Regalia art-gate fallback wired. Ops: season rollover job, unclaimed auto-claim at end.

## 3. Attack checklist (must all fail before ship)

1. Fat save / offline-forged ticks → BP2 cap eats it.
2. Replay same save N times → same XP once (verify dedup) ; claim N times → one grant (BP3).
3. Claim premium without pass / after refund → BP4/BP7 reject.
4. Free-track item → Gil market / Steam / vendor → all three paths refuse (BP6).
5. Reroll-fish the daily deal → one credit, deterministic redeal (BP8).
6. Multi-account free bots → nothing liquid exists to extract (design layer).
7. Buy pass → claim SR → refund → instance still trade-locked → revoked (BP7).
8. Tier-skip overflow (buy 99 skips) → XP clamped at slot-50 requirement.

## 4. Dependencies / order

quest-measurement counters (P0) ← already mostly present · mailbox (live) · Steam MTX
(needs App ID setup — exists) · game→player ops mail (TODO.md) shares the P2 mail path —
build P2 generically and the ops-mail backlog item falls out of it for free.
