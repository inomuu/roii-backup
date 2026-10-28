# Competition integrity — anti-cheat for level races / leaderboards

For any COMPETITIVE mode (level race, event leaderboard, [[pvpve-boss-race]]-style contest)
even a ~5% speed edge (running the sim at 1.05x) decides winners unfairly. Replay-verify
does NOT catch this — a 1.05x run uses only legal inputs, just advanced faster than real
time; the state verifies. This is the plan for catching it. Companion to
`online-robustness.md` (the live per-session fast-forward cap = layer 1) and
`sim-authority-tradeoffs.md` (why the client runs the sim at all).

## Two layers

### Layer 1 — per-session fast-forward cap (LIVE, in `online-robustness.md`)
A save's tick-advance since the session anchor (set at `GET /save`) may not exceed
`elapsed_ms/10ms-per-tick + SLACK` (additive slack, SLACK = 300 ticks = 30s). Legit play is
≤1.0× real time (the sim ticks at 10/s, driven by real time — it can't outrun it), so this
never false-positives (only clock-rate skew is positive excess; a realistic 0.01% skew needs
an ~83h continuous session to reach 300); a cheat trips once its excess exceeds SLACK:
detection = `SLACK / ((rate−1)×10)` seconds → 2× in ~30s, 1.5× in ~60s, **1.05× in ~10min**.

**Limitation — the anchor RESETS every `GET /save` (login/resume).** That reset is required
to close the offline-fast-forward hole (an 8h-idle resume must not be granted 8h of advance).
But it means a cheater who **re-logs-in before their session accumulates SLACK excess evades
layer 1** — at SLACK=300 they must cycle login every <10 min (very disruptive), but it's
possible. Layer 1 catches gross fast-forward + any cheater who sits in one session past the
detection time; it does NOT catch a fast-cycling re-login cheater.

**Leak:** the cheat accepted-under-slack before the trip (≤SLACK = 30s) lives in the last
stored save; the live auto-revert can't undo it (revert = recovery to that stored save, not
leak-removal). Otherwise layer 2 accounts for it (→ manual rollback).

**2-SNAPSHOT FF-rollback (a cheap way to undo the leak LIVE, build with competitions).**
Keep TWO verified saves per account instead of one: `recent` (~10s, for normal recovery) and
`old` (≥SLACK = ≥30s, promoted from `recent` when it ages past 30s). On a **fast-forward** flag,
revert to `old`. Why it lands the cheater at ≤ true-legit (leak fully removed): every accepted
save obeys `stored_tick ≤ legit + SLACK` (the ceiling); the 30s-old save therefore has
`stored_tick_old ≤ L(t−30s) + SLACK = (L(t) − 300) + 300 = L(t)` — its own banked excess is
exactly cancelled by the 300 ticks of legit that elapsed since. Trade-offs: (a) storage 2×
(bounded, not full history); (b) OVER-revert bounded to ~30–40s (if the cheat started recently
it also drops that much legit pre-cheat play) — acceptable for a cheater, and for the rare FP
(pathological clock) it loses ≤~40s, NOT the whole session (far better than revert-to-anchor);
(c) **MUST gate to fast-forward only** — hash-mismatch/divergence could be OUR bug, so those
keep the last-good (~10s) recovery revert, never the 30s one; (d) small promotion bookkeeping.
It only fixes WITHIN-session live leak — a re-login cheater still needs layer 2. Pairs with,
not replaces, layer 2.

### Layer 2 — cumulative excess (LIFETIME / event window, re-login-immune) — **BUILT 2026-07-31**
Accumulate the per-session excess into a DURABLE per-account counter:

    session_excess = (final_tick − anchor_tick) − (now_ms − anchor_ms)/10ms
    lifetime_excess += (session_excess − already_recorded_this_session)   // at each save
    FLAG when lifetime_excess > LIFETIME_THRESHOLD

- **Re-login-immune** — the counter persists; a new session adds onto it.
- **Save-frequency-immune** — excess is measured from the session ANCHOR (spans the whole
  session), not per-save-interval, so saving rarely to dodge a per-interval check doesn't help.
- **The strong property:** `lifetime_excess` only grows if the account's NET rate > 1.0×.
  To stay under the threshold forever a cheater must average ≤1.0× = no real advantage. So
  ANY sustained speed edge is eventually caught — faster the more they cheat (2× in minutes,
  1.05× in a few hours of cumulative play). A 1.05× cheater cycling login every 8 min to dodge
  layer 1 still accrues +240 excess/session → crosses a ~3000 threshold in ~13 sessions
  (~1.7h connected). The counter doesn't care about the cycle cadence — only the net rate.

#### As built — three deliberate departures from the design above

**1. The reference rate is 1.001×, not 1.000×** (`TOLERATED_RATE_BP = 10_010`). Comparing to
exactly real time does not survive contact with THIS game. A client counts ticks off an
undisciplined monotonic clock while the server measures wall time, and an idle game accrues
connected time around the clock, so ordinary crystal drift accumulates forever against a counter
that never forgets:

| client drift | excess/s | hits a 3 000 threshold after |
|---|---|---|
|  50 ppm (typical) | +0.0005 | 69 days connected |
| 200 ppm (in spec) | +0.002  | 17 days connected |
| 500 ppm (out of spec) | +0.005 | 7 days connected |

The "~1000h to inch over" estimate below assumed a player who logs in for sessions; at 24/7 that
is 42 days, not years. Tolerating 0.1% makes anything under it accumulate NEGATIVE at any
duration, and costs nothing in detection — the gap to a real cheat is ~250×, so 1.05× still trips
in ~1.7 h and the re-login loop is untouched (its `elapsed` is ~0, so the tolerance grants it
nothing). `a_fast_client_clock_can_never_accumulate` pins both directions.

**2. The counter is floored at −3 000** (`EXCESS_FLOOR`). "Don't floor" is right for a floor at
ZERO — positive jitter would ratchet against it. Once the drift tolerance lives in the rate, an
honest account instead runs negative forever, and an unbounded negative is a BANKED cheat budget:
a year of honest play would buy days of undetected 10×. The floor's job is now to bound the bank,
so it is both safe and necessary. Measured: 500 honest hour-long sessions reach only −28 000
unfloored, i.e. the clamp binds after roughly two days of play.

**3. It REJECTS at the threshold, not only flags.** A flag taints the session — which blocks
refine and market settlement but lets the save through, so a 20× re-login loop would keep minting
until a human intervened. Rejecting is not an auto-ban: the client auto-reverts to its last
verified save exactly as it does for layer 1, and banning stays manual. This is only safe because
of departure 1 — an honest account sits pinned at the floor, nowhere near the threshold.

Constants and code: `TOLERATED_RATE_BP` / `EXCESS_THRESHOLD` / `EXCESS_FLOOR`, `session_excess()`
+ `fold_excess()` (pure, unit-tested), the `integrity` table, and the `by_excess` block in
`GET /flags`. `TickAnchor.recorded` carries the per-session already-counted amount so the counter
cannot be moved by changing how often a client saves; `TickAnchor.from_handshake` suppresses
accumulation against a boot-synthesized anchor, which is backdated and carries no history.

## Measuring PLAY TIME (the input to layer 2) — derive it, don't track presence

Do NOT build a separate presence/heartbeat system. Play time falls out of the save protocol:

- **Session duration = `last_save_server_ms − session_anchor_server_ms`.** The anchor
  (`tick_ref`) is stamped at `GET /save` in SERVER time; every verified save carries a server
  timestamp. Both are server-clock → the client can't fake the duration.
- **Offline is excluded for free** — it's the gap BETWEEN one session's last save and the next
  session's anchor, i.e. outside every anchor→last-save window. No offline earnings, so no
  ticks accrue there anyway.
- **Lifetime connected time = Σ session durations.** For an EVENT, use the event's fixed
  real-time window and the connected slices within it.

## False positives (layer 2)
Legit `session_excess ≈ 0 or slightly NEGATIVE` (the sim can't outrun real time; load-delay
after `GET` + network latency push `final_tick` behind the server clock). Use a SIGNED
accumulator (don't floor) so ± clock jitter cancels and only a consistent positive bias (a
cheater) accumulates → legit `lifetime_excess` stays ≤0 and never trips. Residual: a client
with a persistently FAST clock (~0.05%, rare; NTP corrects wall clocks) over ~1000h could
inch over — and it's a FLAG for human review, never an auto-ban ([[input-authority-security]]:
a flag spike across accounts = OUR bug, not a cheat wave).

## Known residuals (2026-07-31 — both since CLOSED)

**1. The handshake re-login loop — CLOSED by layer 2 the same day.** `FASTFWD_SLACK_TICKS` (300)
is a per-SESSION cushion and `GET /save` re-anchors unconditionally, so
`auth → GET /save → POST /save (+300 ticks)` minted 300 ticks per pass with nothing counting
across passes. Every layer-1 gate accepted it: span 300 ≪ `MAX_REPLAY_SPAN`, the tick advance sat
exactly on `0 + SLACK`, the start state was one the server itself had confirmed, and the ticks
were genuinely simulated so replay verified. The only ceiling was the per-IP rate limit
(120 req/min ÷ 3 requests ≈ 40 passes/min ≈ **12 000 ticks/min, ~20×**).

Layer 2 sees it because the counter does not reset: each pass posts +300, so it crosses the
threshold in 11 passes (~16 s at the rate-limit ceiling) and the save is then rejected.
`the_relogin_loop_trips_the_lifetime_counter` asserts both halves — that layer 1 accepts every
pass, and that the counter climbs anyway.

Recorded for whoever revisits this: the naive fix (stop re-anchoring on handshake) does NOT work.
The budget would then accrue while a player is legitimately logged off, turning an 8 h absence
into 288 k claimable ticks. The reset has to stay; the accumulator is what carries across it.

**2. The farmable restart grace — CLOSED 2026-07-31.** The startup anchor used to be backdated a
flat `RESTART_GRACE_MS` for every account, and `fly.toml` runs `min_machines_running = 0`, so
anyone alone on the server could idle the machine to sleep, wake it, and collect the grace as
free progress. At the original 15 min that paid ~3× the sleep it cost and was worth farming.

Fixed by `saved_at_ms` on `saves` + `boot_anchor_ms(saved_at_ms, now − RESTART_GRACE_MS)`: the
budget is now the AGE of that account's own last save. That is the same quantity as the backlog
the client is about to present — a backlog IS the play since the last confirmed save — so an
honest client always fits exactly, and a woken machine can only pay out wall time that genuinely
elapsed. No profit; the constant survives purely as a ceiling on the offline-claim, which is what
the `max` term still stops. `waking_a_slept_machine_pays_only_the_time_that_actually_passed` pins
both ends.

⚠️ That test only discriminates when the idle time is well UNDER the clamp — at exactly
`RESTART_GRACE_MS` the save-age and the flat backdate are the same number and it passes against
either implementation. It was written that way first and proved nothing; the sabotage run is what
caught it.

## Ranking
Rank a race by SERVER-VERIFIED, real-time-normalized progress (level/ticks per connected
real-second within the event window). A 1.05× cheater's rate exceeds the physical 1.0× ceiling
→ both loses rank credibility (cap the counted rate at 1.0×) AND accumulates `lifetime_excess`
→ flagged.

## Built / still to build

Done 2026-07-31 (brought forward from "with the competition system" because the re-login loop
was live in the base game, not only in a race):
- ✅ `integrity` table + the in-memory counter, loaded at boot and written through per save.
- ✅ `TickAnchor { tick, ms, recorded, from_handshake }` — the anchor grew the per-session
  scratch instead of a separate map.
- ✅ Accumulate at each save, reusing layer 1's excess (`session_excess` / `fold_excess`).
- ✅ `GET /flags` → `by_excess`, ranked worst-first, with the threshold and floor reported.
- ✅ Reject at the threshold (see departure 3), manual ban only — no auto-ban anywhere.
- ✅ `saved_at_ms` on `saves` + `boot_anchor_ms` — closes residual 2 (see below).

Still to build, and genuinely event-scoped this time:
- Event-window variant: reset a per-event excess counter at event start; rank + flag on it.
- 2-snapshot FF-rollback (above) to remove the within-session leak live.
- Ranking normalization (below).

Layer 1 ships with the base game (gross fast-forward defense + auto-revert UX). Layer 2 now
ships with it too; the event-window variant remains a prerequisite for the FIRST competitive
mode.
