# Simulation authority — model trade-offs (why client-sim + verify)

Reference for the core architectural choice: WHERE the game simulation runs and who is
authoritative. Settles the recurring "why not run the sim on the server?" question.
Companion to `online-robustness.md` (what we built) and `backend-arch-reconcile.md` (vs the
external handoff doc, whose §3/§7 lean toward model B below).

## The three models

- **A — Live server-sim.** The server ticks every player's world 10×/s continuously and is
  authoritative. Classic authoritative MMO.
- **B — Lazy server-sim.** Server is authoritative but computes each player's sim in a BATCH
  at check-in (every 1–10 min) or on interaction, not every tick. (The external doc's
  "lazy/checkpoint tick" idea.)
- **C — Client-sim + verify (OURS).** The client runs the sim live locally (authoritative on
  the player's machine); the server REPLAYS the recorded input log to verify, and can sample.

## Key realization: verify is NOT "fewer ticks"

At **100% verify**, the server replays the same total ticks a live sim would run — same code
(`sim-core`), same tick count → **raw sim CPU is ~equal to live sim.** The savings do NOT come
from doing less sim per tick. They come from the FREEDOMS that "later + offline + optional"
grant:

1. **Sampling** — deterrence is statistical, so verifying 10% (or 1%) still deters cheating.
   This is the 10–100× lever. A live/authoritative sim CANNOT sample: it must produce every
   player's real state every tick.
2. **Offload to clients** — in C the CLIENT's CPU runs the live sim (to play + render); the
   server only audits a sample. The bulk compute is paid by players' hardware, FREE to us.
   In A/B the server pays for all of it.
3. **Burst efficiency** — verify runs offline, full-speed, cache-hot, then evicts. Live sim
   runs thin real-time (10/s) with per-player scheduling overhead + resident state.
4. **RAM** — verify holds one world per worker, then frees it. Live/authoritative holds every
   world resident (the real MMO bottleneck → why you shard).
5. **Stateless / batchable / deferrable** — verify workers are stateless (any worker, any
   log, retry on crash, cheap spot machines). Live sim is stateful + pinned + needs failover.
6. **Skip the uneventful** — cheap hash-chain PIN (`/ingest`) for quiet stretches, full replay
   only on sample/trigger.

Measured: `sim-core` replays **~128k ticks/s/core** (release, full 5-party, incl. late-game;
`sim-cli --ticks 500000` in 3.9s). So even 100% verify is cheap: at 100k CCU (1M ticks/s of
play) ≈ 10 cores; 10% sampling ≈ 1 core. See the cost model in `online-robustness.md` notes.

## B vs C — the real trade-off

Both drop the "live" obligation (B batches, C offloads), so both beat A. The fork is who is
authoritative:

| | A: live server-sim | B: lazy server-sim | C: client-sim + verify (ours) |
|---|---|---|---|
| Server sim work | 100% continuous | 100% batched | **sample-able (e.g. 10%)** |
| Who pays sim CPU | server | server | **clients (free)** + server sample |
| CPU efficiency | poor (thin real-time) | good (burst/evict) | good |
| RAM | all worlds resident | on-demand | per-worker |
| Server egress | streams state down | pushes state down | **~0 (client has its own state)** |
| Interactivity | round-trip | round-trip + catch-up | **instant (local)** |
| Cheating possible? | no | **no** | yes, but CAUGHT (replay + sample) |
| Anti-cheat machinery | none | **none** | required (replay / input-authority / fast-forward cap / taint) |

**Precise cost comparison (important nuance):**

- **Server SIM CPU: at 100% verify, C ≈ B** — both run the same total ticks server-side (C's
  replay = a sim). C's "offload to clients" saving is REALIZED ONLY WHEN SAMPLING: at 10% the
  server does 0.1× and the client does the free 1×; at 100% the server redoes the whole sim as
  replay, so it ties B. So C's *compute* win over B is really the OPTION to sample (B, being
  authoritative, is locked at 100% forever); it is not an automatic win at 100%.
- **Server BANDWIDTH: C wins at ANY sample rate — and this is the bigger lever**, because
  egress is the #1 server cost. Clouds (Fly/AWS/GCP/Azure) bill **egress** (server→client,
  out) but **ingress** (client→server, in) is **free**. In C the bulk traffic is the client
  UPLOADING its log (ingress = free) and the server returns a tiny ack (egress ≈ 0). In B the
  client has no state of its own, so the server must STREAM state DOWN (egress = billed) — or
  B degrades into C. C flows the free direction; B flows the paid one. So C's billable
  bandwidth is structurally ~0 for the save loop; B pays to ship state.

So at 100% verify: **C ≈ B on sim CPU, C ≪ B on bandwidth, and C keeps the option to sample
down (10×) while B cannot.** C's compute win is a future option (sampling); its bandwidth win
is immediate. The only real future egress in C is the social push channel (server→clients
broadcast) — that, not the save loop, is what binary/delta/idle-silence optimize.

**B's payoff:** server-authoritative → cheating is impossible by construction → you DELETE the
entire verify/anti-cheat apparatus (replay-verify, input-authority taxonomy, fast-forward cap,
taint). Simpler trust. Worth considering when economy integrity dominates and you'll pay
server compute for simplicity.

## Why C fits THIS genre specifically (the decisive points)

Beyond raw cost, B has genre-fatal problems for a watch-the-auto-battle idle game:

1. **Watch-the-battle forces local sim anyway.** The appeal is watching the party fight in
   real time. Between B's checkpoints the client must render SOMETHING: either a frozen
   snapshot (bad UX) or it runs the sim locally to animate — which IS model C. So B forces
   either frozen UX or **double simulation** (client render + server truth) with constant
   reconciliation jank (the view "snaps" to server truth each checkpoint).
2. **Reconciliation is redundant here.** Each player is a single-player PvE world → the client
   holds ALL inputs → client and server compute BIT-IDENTICALLY (determinism). The server
   running it produces no new information — the minimal, non-redundant server role is just
   "check they match" = **verify**.
3. **Every mutating action would round-trip.** Under B, equip/refine/stat/skill all mutate the
   authoritative state → server catch-up-sim + round-trip → laggy. Under C the client is
   local-authoritative → instant; only the few money/RNG-value actions call the server.
4. **Switch-device is comparable-or-better under C.** New device: login → pull verified save →
   resume local → instant thereafter (single-session kicks the old device). B also pays a
   one-time catch-up pull but then STILL round-trips every action. (Progress lost on switch: C
   ≤ one push interval (10–60s); B ≤ one checkpoint (1–10 min) unless it catches up.)

## The principle this yields

> Call the server ONLY for what MUST be authoritative — money / value-bearing RNG — which
> happens rarely. Keep the continuous 99% (combat, movement, ordinary play) local and instant.

This is exactly our **hybrid**: client-sim + server-VERIFY for state + server-AUTHORITATIVE
for the few value-bearing RNG points (refine rolls now; drops next). We pay simulate-grade
trust only where value is at stake, without paying simulate cost for all of combat. B fails to
localize — it makes everything authoritative, so everything pays the latency + server cost.

## Decision

**C (client-sim + verify + hybrid server-authoritative RNG).** Chosen for the "100k CCU on
cheap infra" thesis (bulk compute offloaded to clients; server samples) and because a
watch-the-battle idle game must run the sim client-side anyway. B is not a foolish
alternative — it trades server compute + egress + interactivity for ironclad trust and the
deletion of anti-cheat machinery — but it loses on cost and on the genre's core UX.

Revisit only if a future feature needs real-time cross-player authority (e.g. live shared-world
combat) that C's independent-worlds model can't express — then a targeted authoritative service
for that feature, not a wholesale switch to B.
