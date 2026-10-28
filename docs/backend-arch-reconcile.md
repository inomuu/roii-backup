# Backend architecture — reconcile with the external handoff doc

Cross-checks the external `idle-mmo-backend-architecture.md` (a decisions handoff) against
what this codebase actually is (`online-robustness.md`, `s2-server-build-plan.md`,
`economy-invariants.md`). The external doc is high quality and ~80% aligned; this note
records where it matches, where to adopt, and the ONE structural tension to settle.

## The one structural tension: SIMULATE vs VERIFY

The external doc §3 says *"hot state lives in server sim-core memory; checkpoint is a cache;
idle income = f(elapsed); on crash, recompute from `now - last_checkpoint`."* That is a
**server-authoritative simulation** model.

What we built is **client-sim + server-VERIFY**: the client runs the deterministic sim in
real time and records a `Vec<InputEvent>`; the server is a stateless verifier that REPLAYS
the pushed log and matches the hash. It does not hold a live per-player sim.

Two reasons the doc's §3 mechanism doesn't port as-written:
1. **Our combat is RNG-per-tick** (monsters/skills/drops via one PCG32 in a fixed draw
   order). It can't collapse to `income = f(elapsed)`; reproducing state means running the
   ticks — which is exactly what replay does.
2. **No offline earnings** — the game accrues only while the client is open (idle-on-
   taskbar). So "recompute idle income from elapsed while disconnected" isn't our model.

The doc is internally inconsistent here too: §1 lists "hash-chain replay" (verify) but §3
describes recompute (simulate). **Decision needed before anything downstream.** Current
codebase = VERIFY. Recommendation: keep VERIFY (it's built, proven, and scales — the server
does no per-tick work), and read every §3 principle as "replay the log" instead of
"recompute from elapsed". Everything else in the doc survives that swap.

**Full model comparison (live server-sim A / lazy server-sim B / client-sim+verify C), why
verify isn't magically cheaper at 100%, and why C fits a watch-the-battle idle game:
`sim-authority-tradeoffs.md`.**

## Point-by-point

### Already done / aligned — no action
- **Fly `sin` + Neon + sqlx direct-TCP pooled + standard-Postgres-only** (§2) — exactly our
  `verify-server`. No Neon-specific API. `pg_dump`-portable.
- **Single active session** (§4) — built: `Sessions.active` supersedes the old token → old
  session 401s.
- **Two write paths: recomputable=batch / economy=immediate-synchronous** (§3) — our design:
  the verified save is the "checkpoint"; economy (mint/trade/market) writes immediately with
  dedup — see `economy-invariants.md`, `s3-economy-design.md`.
- **Server = time authority, never trust client clock** (§3) — our sim is integer-only, no
  wall-clock; seed is server-chosen. (See fast-forward note below for the tick-rate half.)
- **Idempotent economy handlers** (§4) — refine reconcile is idempotent; `instance_id` dedup.
- **Don't over-engineer for 100k; keep it shardable** (§6) — matches `mass-100k-strategy`.

### Adopt (good, not yet done) — real backlog items
- **Jitter the checkpoint/push cadence** (§4) — at scale, ~10s pushes on a shared wall-clock
  = a write spike. Spread with `hash(account) % interval`. *Do when concurrency warrants.*
- **Max CU cap on Neon** (§4) — set autoscaling min/max to cap the bill against a spike.
- **Binary + delta protocol** (§5) — bandwidth (WS egress) is the real cost driver at scale;
  MessagePack/protobuf + send-only-deltas. *Pairs with the future push channel, not now.*
- **Graceful behavior on deploy** (§4) — see the deploy-kick gap below.

### Adapt (principle yes, mechanism differs)
- **§3 checkpoint/recompute** → for us the "checkpoint" is the verified pushed save and
  "recompute" is replay; interval is our ~10s autosave (chosen for small verify logs + low
  lost-progress, not DB-write batching). Our scaling bottleneck is replay CPU + write
  frequency, not per-row vs bulk upsert.

### Real gaps the doc surfaced

- **Fast-forward via an oversized tick log — NOW CLOSED.** Replay-verify proves a log is
  *deterministic*, not that it fits *real time*. A client could push a full `MAX_REPLAY_SPAN`
  log in 2s and gain that time's farming instantly (that constant only bounds CPU, not rate).
  Fixed by a **wall-clock tick-rate cap**: a per-account session anchor `(tick, server_ms)`
  set at each `GET /save`; a save's tick advance since the anchor may not exceed
  `elapsed × 10 t/s × margin + grace`. Anchored per-session (not per-push) so request-spam
  can't inflate it; seeded at startup from stored saves so a scale-to-zero + one huge push
  is bounded too. `within_tick_rate()` + 2 unit tests. This is the tick-rate half of the
  doc's "server time authority".

- **Deploy kicks every active player** — `by_token` is in-memory, so a Fly redeploy wipes
  it → every connected client's next request 401s → they all hit the "LOGGED IN ELSEWHERE"
  modal. The doc's "graceful drain" (§4) names the class. Our fix is likely client-side:
  on a 401 that is NOT a genuine supersede, transparently re-login + resume rather than
  showing the kicked modal. *Not yet done — track it.*

## Net
Fold the doc's ops discipline in (jitter, CU cap, binary/delta, deploy grace) as scale
backlog. Keep VERIFY over SIMULATE. The fast-forward hole it helped surface is closed
(2026-07-09). Settle simulate-vs-verify explicitly if a future feature (e.g. authoritative
server-side sim for anti-cheat-free trading) reopens it.
