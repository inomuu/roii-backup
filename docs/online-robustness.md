# Online session robustness — current architecture (2026-07-09)

Reference for the online session lifecycle + the robustness/anti-cheat guards as they
stand. Companion to `refine-server-roll-2b.md` (the refine-nonce protocol),
`s2-server-build-plan.md` (the verify-server slices), `server-plan-phases.md` (S0–S4
roadmap). This doc is the "what is live NOW" snapshot.

## The two halves

- **`crates/verify-server`** (Rust/axum) — the authoritative cloud server. Deployed on
  Fly (`renrei-idle-server`) + Neon Postgres. Runs the SAME deterministic `sim-core` the
  client runs, so it can replay-verify any client save. Placeholder dev auth (`/auth/dev`,
  name→account); Steam auth swaps that one handler later.
- **`godot/` client** — `SimDriver` owns the one `SimBridge` + the `ServerClient`
  (`godot/net/server_client.gd`, thin HTTP). Online mode is chosen at startup
  (`ModeSelectView`); the client plays locally and pushes verified saves.

**Trust model:** the save file is conceptually `(server seed + genesis start + Vec<InputEvent>)`.
The server re-runs it and only stores the result if the replay hash matches. Cheating is
caught by *math* (a legit deterministic session always reproduces its hash), NOT by
client surveillance — so no invasive anti-cheat. See `anti-cheat-and-server-model.md`.

## Online session lifecycle

1. **Mode select** (`mode_select_view.gd`) — startup picks Online/Offline + a name.
   Spawned by `SimDriver` (NOT in `main.tscn`), collapses to a corner badge after a pick.
2. **Login** (`_begin_online` → `_online_handshake`) — `POST /auth/dev {name}` →
   `AuthResp { token, account_id, name, seed (STRING), sim_version }`.
   - `seed` and every refine `nonce` cross the FFI as **decimal STRINGS** — values >2^53
     corrupt through Godot's float64 JSON parser. serde_json (server) handles u64 fine;
     only the Godot boundary needs strings. (This was the root-cause bug of the original
     "online save never works".)
3. **Version gate (at LOGIN)** — client compares `auth.sim_version` vs `bridge.sim_version()`.
   Mismatch → `UPDATE REQUIRED` block *before* playing (a skewed client can never sync, so
   don't let it start a doomed session). Backstop: the save-time skew check still exists in
   case the server redeploys mid-session.
4. **Resume or create** — `GET /save`: if a stored verified state exists, `load_json` it and
   leave creation; else show creation for a fresh online character. A freshly created
   character's party NAME is a recorded `SetPartyName` *event* (not baked into `start`), so
   `start` stays a reproducible genesis (see anchor below).
5. **Play + autosave** — every ~10s `_push_online` sends the growing `SaveBundle`
   (`export_log()`) to `PUT /save`.
6. **Server verifies** `PUT /save` (in order): policy/limits → **genesis anchor** →
   **replay-verify** (`bundle.verified_state`) → **refine bind-check** → **refine reconcile**
   → store. Any failure = rejected + NOT stored.

## Server-side verify pipeline (`put_save`)

| Step | Guard | Reject reason |
|---|---|---|
| Anchor | `bundle.start` hash ∈ `genesis_candidates(seed)` (bare `new_game_in_zone` + `new_game_as` per creatable class) OR a previously-confirmed end-state | `start snapshot is not anchored` |
| Replay | re-run the log, hash must match the claimed final | `replay hash mismatch — not stored` |
| Refine bind | full-replay: each risky refine carries the exact nonce bound to (account, instance_id, level, attempts); a safe/max refine needs none | `refine bind-check failed` |
| Refine reconcile | every used nonce was server-ISSUED (unforgeable); idempotent for growing-log re-saves | `forged refine nonce(s)` |

On success the confirmed final hash becomes a **trusted anchor** for the next resume.

## Robustness layer (built 2026-07-08/09)

### Save-reject handling (client, `_push_online`)
The client categorizes a rejected push and responds — an idle game must never silently
pile up progress that evaporates on resume:

| Reject signal | Client response |
|---|---|
| `401` (token gone) = **superseded** (logged in elsewhere) | `LOGGED IN ELSEWHERE` blocking modal; session PAUSED |
| reason `version skew` | `UPDATE REQUIRED` modal (Reload retries) |
| reason `hash mismatch` / `not anchored` / `forged` = **divergence** | `_auto_revert`: `GET /save` → `load_json` last verified state → toast the reason → keep playing |
| network / transient (streak ≥3) | non-blocking "not syncing" banner; keep the good state + keep retrying (reverting on a blip = the TBH mistake) |

A **doomed** session (can't ever save) is PAUSED (`_session_invalid` gates `_process`) so it
stops accumulating unsaved idle time. Auto-revert notifies (never silent). Modals =
`session_ended_view.gd`; banner = `sync_banner_view.gd` (Control nodes, no `_draw`).

### Single active session (server)
`Sessions.active: account_id → current token`. A new login mints a fresh random token and
DROPS the account's previous token from `by_token` → the old session's next request 401s.
Prevents concurrent play (dup saves / economy dup). Steam auth gives this for free.

### Taint-on-divergence (server, #2-lite)
An integrity-rejected save (the 4 reasons above) marks the account **TAINTED**
(`Sessions.tainted`, in-memory). While tainted, `POST /refine/roll` is refused (409) so a
doomed/malicious session can't keep harvesting nonces or spamming `outstanding` rows. The
token stays valid (so the client can still pull-and-revert). Taint CLEARS on the next
verified save (it recovered) or on re-login. Not persisted — a restart clears it; the next
save re-taints if still broken. *Why refine-only and not a full token revoke:* the
client's own auto-revert reuses the token to `GET`/`PUT /save`; a full revoke would break
recovery. The real refine guarantee is at save-verify (a roll only counts when a verified
save consumes it) — taint is anti-DoS + clean invalidation, not the primary gate.

### Anti-fast-forward: wall-clock tick-rate cap (server)
Replay-verify proves a log is *deterministic*, NOT that it fits *real time* — a client could
push a long (up-to-`MAX_REPLAY_SPAN`) log in 2s and gain that time's farming instantly
(`MAX_REPLAY_SPAN` only bounds replay CPU, not rate). Guard: a per-account **session anchor** `(tick, server_ms)` set
at each `GET /save` (handshake/resume). A save's tick advance since the anchor may not exceed
`elapsed_ms/100 + FASTFWD_SLACK_TICKS(300 = 30s)` — **additive** slack, not a multiplicative
margin: legit play is ≤1.0× real time (the sim can't outrun the wall clock), so this never
false-positives (the only positive excess is clock RATE skew; a realistic 0.01% skew needs an
~83h CONTINUOUS session to reach 300) AND the detectable-cheat margin shrinks as the session
runs (a cheat trips once its excess passes the slack: 2× ~30s, 1.5× ~60s, 1.05× ~10min — a ×2
margin, by contrast, let any rate ≤2× farm forever). SLACK also bounds the per-detection LEAK
(cheat that got accepted-under-slack, which the auto-revert can't undo — it lives in the last
stored save) to ~SLACK = 30s; removing that residual needs a history rollback or the LIFETIME
layer (`competition-integrity.md`). Anchored per-SESSION (not per-push) so request-spam can't
inflate it; seeded at startup from stored saves so a scale-to-zero + one huge push stays
bounded. Pure `within_tick_rate()` + unit tests. This is the tick-rate half of "server is the
time authority" (`backend-arch-reconcile.md`). A patient re-login cheater who never accrues the
slack in one session is caught by the LIFETIME layer instead — see `competition-integrity.md`.

### Orphan-roll prune (server, #3)
`refine_outstanding` rows are issued-but-unconsumed rolls. A legit roll is consumed within
one autosave (~10s); one still outstanding past **`OUTSTANDING_TTL` = 1h** is an
abandoned/doomed-session orphan. Pruned by: a `created_at` column + a **startup DELETE
sweep** (so orphans don't reload) + **prune-on-issue** (GC an active account's stale rolls
under the roll lock, write-through the delete). TTL ≫ any real issue→consume gap, so
pruning can't false-forge a slow-but-legit refine.

### Re-anchor: keep the replay log short (client)
After a CONFIRMED save, the client moves its start checkpoint to the just-verified state
(`bridge.commit_anchor()` → `sim_core::replay::reanchor`): drop the events the bundle
covered and rebase the snapshot hash-chain onto the new anchor (keep post-anchor snapshots,
re-link `chain_hash` from empty — state hashes are anchor-independent). Confirm-gated (a
rejected save leaves it; the log keeps growing until one succeeds). Without this the log +
chain grow for the whole session → verify replays more each time AND a session past
`MAX_REPLAY_SPAN` (50k ticks ≈ 83 min) can no longer save — which is why the client freezes
the sim at `SPAN_STALL_TICKS` (45k) rather than play on unsaveably. This is the lever that
keeps verify cheap (see `sim-authority-tradeoffs.md` — the growing-log ×30-80 multiplier).

## Persistence (Neon Postgres, write-through)

`accounts` · `saves` (one authoritative state/account) · `trusted` (anchor hashes) ·
`refine_outstanding` (+`created_at`) · `refine_consumed` · `active_sessions` (the current
token/account — restored at startup so a redeploy doesn't 401 connected clients into the
"logged in elsewhere" modal; single-session semantics survive the restart). In-memory maps
are the working set; the DB is loaded at startup and written-through on change. No
`DATABASE_URL` → in-memory only (never crash-loops on a bad URL). Survives Fly scale-to-zero.
(`tainted` + `tick_ref` are intentionally in-memory — a restart clears them; they self-heal
on the next save.)

## i18n

The online overlays (mode-select, session notices, sync banner) route through
`Loc.ui(key, fallback)` — keys `UI_MODE_*` / `UI_SES_*` / `UI_ONL_*` in
`godot/i18n/strings.csv` (EN + TH filled). Multi-line bodies store a literal `\n` in the
CSV (a real newline breaks `get_csv_line`) and convert via `SimDriver._L()`.

## Version-skew guard (dev workflow)

Two sessions bumping `sim_core::VERSION` / editing `game_data` causes client↔server skew →
silent save rejects. `tools/check_online_sync.sh` compares tree(lib.rs) vs client-dylib vs
deployed-server (`GET /version`). Run before any online test. Deploy = `fly deploy` (tars
the working tree, so rebuild the dylib to match after a sim-core change).

## NOT done (intentionally deferred)

- **#2 promotion enforcement** (reuse `verify_refine_binding` at market register/ship) —
  NOT needed until the trade market is live: today no path promotes a refined item out of
  its verified-save context, and the save-verify already makes the stored refine level
  trustworthy. Build it WITH the market (its shape depends on how promotion sources the
  item). Recorded so it isn't forgotten.
- **Real-time push channel** (WebSocket/SSE) → the foundation for chat / friends+presence /
  +10 global announce. One build unlocks the whole social layer.
- **Server-roll DROPS** (rare/tradeable + cards) — `instance_id` at drop time (currently
  lazy-at-refine). The next valuable RNG to make server-authoritative.
- Fold the refine bind-check into the single `verified_state` replay (currently a second
  pass). Prune `consumed` on anchor-advance.

## In-game verification still pending (USER runs the game — never launch on a real save)

None of the new `.gd` flows have had a live visual/functional test: mode-select (EN+TH),
version-block, single-session kick, auto-revert + toast, refine-while-tainted, the
localized overlays. Parse-verified only (Godot `--import`).

**Step-by-step test plan = `online-test-checklist.md`** (pre-flight + each flow with expected
results; core loop 1-4/6 is naturally triggerable, 5/7/8/9 need synthetic conditions).
