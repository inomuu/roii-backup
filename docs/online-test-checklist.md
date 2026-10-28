# Online robustness — in-game test checklist

What to verify once ready. Everything below is DEPLOYED (server 0.0.183 on Fly) +
committed; none has had a live visual/functional test. Architecture = `online-robustness.md`.

**Safety:** the "never launch on a real save" rule is about *Claude* not auto-clobbering a
load-failed local slot — YOU running the game normally is fine. Still, do these with a
**throwaway online name** (e.g. `TestBot`), not your real character. Online saves live per-
account on Neon and don't touch the local slot.

## 0. Pre-flight (do first, every time)

- [ ] `cargo build -p godot-ext && cp target/debug/libgodot_ext.dylib godot/bin/` (dylib
      must match the deployed server version, or the LOGIN version-gate false-blocks you).
- [ ] `tools/check_online_sync.sh` → must print `✓ in sync` (tree == dylib == server).
      If not, rebuild/redeploy the odd one out.
- [ ] Client points at Fly: `ONLINE_URL` in `sim_driver.gd` = `https://renrei-idle-server.fly.dev`.

## 1. Mode-select + i18n (EN)

- [ ] Launch → **CHOOSE MODE** screen appears (title, name field, ONLINE / OFFLINE (DEBUG)
      buttons, hint line at bottom). All English, laid out cleanly.
- [ ] Type a name, press Enter (or click ONLINE) → screen collapses to a small top-left
      **● ONLINE (name)** green badge.
- [ ] Relaunch, pick OFFLINE → **● OFFLINE (DEBUG)** amber badge.

## 2. i18n (Thai)

- [ ] Options → switch language to ไทย. Relaunch (or however the switch applies).
- [ ] Mode-select shows Thai: **เลือกโหมด**, ปุ่ม ออนไลน์/ออฟไลน์, hint ภาษาไทย, placeholder
      ชื่อผู้เล่น. Check nothing overflows / mojibake.
- [ ] (The session-notice overlays in tests 5–8 should also render Thai — spot-check one.)

## 3. Fresh online character → save → resume (the core loop)

- [ ] Online, new name → character creation shows → pick class + confirm → play a bit.
- [ ] Wait ~10s+ (an autosave cycle). No "not syncing" banner = it saved.
- [ ] Neon check (server-side): the account has a row in `saves` with a growing `final_tick`.
- [ ] Quit + relaunch online SAME name → **resumes** the saved state (not a fresh char),
      tick continues from where it saved.

## 4. Normal risky refine online (server-roll path)

- [ ] Refine a piece of gear at a **risky** (<100%) level while online → it refines (no
      "needs a server connection" flash). Success/fail as normal.
- [ ] Neon: a row appears in `refine_consumed` for that account; `refine_outstanding` does
      NOT keep growing across several refines (issued rolls get consumed).

## 5. Version-block at LOGIN  (needs a synthetic skew)

- [ ] Temporarily make the client version differ from the server: bump `sim_core::VERSION`
      locally + rebuild the dylib ONLY (don't redeploy the server), OR the reverse.
- [ ] Launch online → **UPDATE REQUIRED** blocking modal appears BEFORE any play; Reload /
      Quit buttons. You cannot start a session.
- [ ] Revert the version + rebuild so sync is restored afterward.

## 6. Single active session (concurrent-login kick)

- [ ] Log in online as `TestBot`, start playing (instance A).
- [ ] Launch a SECOND instance, log in online as the SAME `TestBot` (instance B).
- [ ] Instance A: on its next autosave push (~10s) → **LOGGED IN ELSEWHERE** modal + the sim
      PAUSES (no more idle progress accumulates). Instance B plays normally.

## 7. Auto-revert on divergence  (hardest to trigger naturally)

> A legit deterministic session never diverges, so this needs a synthetic bad state. Options:
> tamper the local in-memory state via the debug menu (a Debug-authority event the server
> rejects), or hand-edit a pushed bundle. If too fiddly, defer — the taxonomy is unit-tested
> server-side; this test is the client-UX half.

- [ ] Cause a push to be rejected with `hash mismatch` / `not anchored` / `forged`.
- [ ] Client **auto-reverts**: a banner/toast appears ("Reverted to your last save — <reason>"),
      the state snaps back to the last verified save, and play continues (session NOT frozen).
- [ ] It does NOT silently keep piling up unsaved progress.

## 8. Refine blocked while tainted

- [ ] Right after a divergence-reject (test 7) — before the session recovers — try a risky
      refine → it is REFUSED (client shows the "needs a server connection" flash; server
      returned 409 `session out of sync`).
- [ ] After the next successful save (auto-revert recovered) → refine works again.

## 8b. Fast-forward rejected (synthetic)

- [ ] Craft a bundle whose `final_tick` jumps far beyond real elapsed since the session's
      `GET /save` (e.g. hand-push a 500k-tick log seconds after login) → server rejects with
      `tick advance exceeds real elapsed time (fast-forward)`. Normal ~10s pushes are unaffected.
      (Unit-tested server-side via `within_tick_rate`; this is the live-path confirmation.)

## 8c. Re-anchor keeps the log short (track A)

- [ ] Play online continuously through several autosaves. Server-side, the stored save's
      `final_tick` keeps rising but each pushed bundle's replay SPAN stays small (~one
      interval), not the whole session — i.e. a session left running for many hours keeps
      saving (never hits the MAX_REPLAY_SPAN "checkpoint/resume to reset" rejection).
      (Hard to eyeball; the sim-core `reanchor_yields_a_short_equivalent_bundle` test is the
      real proof. In-game: just confirm a long uninterrupted online session keeps syncing.)

- [x] **Span stall — PASSED 2026-08-01 (local server, SPAN_STALL_TICKS temporarily 300).**
      Killed the server at tick 510; ticks ran on to 800 and froze there — span exactly 300 —
      with `unsaved span reached 300 ticks — sim paused until a save confirms`. Restarted it:
      `synced — resuming from the span stall`, and the stored save jumped to tick 999, so the
      300 ticks played while disconnected were SYNCED, not discarded. No auto-revert, no cheat
      flag. Method worth reusing: point the client at a local server via `VERIFY_SERVER_URL` and
      kill it, rather than touching the real network — the outage is exact and production is
      untouched. Note the constant is read by
      `the_client_stalls_before_the_server_refuses_its_bundle`, which parses the .gd line, so a
      trailing comment on it fails the test (it did).

- [x] **The backstop — PASSED 2026-08-02.** The run above proved the guard works; this proves
      what happens when it does not. Rather than raise `SPAN_STALL_TICKS` past 50k (which would
      mean 83 minutes of waiting), LOWER the local server's `MAX_REPLAY_SPAN` to 500 and leave
      the client at 3000 — the server then rejects before the client would ever stall, which is
      the same condition in a minute instead of an hour.

      Observed: `POST /save -> 400  replay span 900 ticks > max 500`, then
      `span guard was bypassed — reverting`, then `GET /save -> 200` and saves resumed. It does
      NOT sit banner-up forever, which is the whole claim.

- [x] **No cheat flag from any of it, and layer 2 reads NEGATIVE.** Checked before wiping the
      test database (the previous run truncated first and lost the evidence): `cheat_flags` was
      empty across a mid-session server restart — so `boot_anchor_ms` + the restart grace hold on
      real traffic — and `integrity` showed `lifetime_excess = -60` for the account that played.
      Negative is the design: against a 1.000× reference that same honest session reads slightly
      POSITIVE and would ratchet toward the threshold over days. This is the false-positive class
      `TOLERATED_RATE_BP` exists to remove, seen on live data rather than in a unit test.

## 8d. Redeploy doesn't kick you (track A)

- [ ] While playing online, redeploy the server (`fly deploy`). Your session should keep
      syncing after the new machine is up (a transient "not syncing" blip is fine) — you
      should NOT get the "LOGGED IN ELSEWHERE" modal from the restart. (Single-session still
      works: a real second login on the same name still kicks the first — test 6.)

- [ ] **And no cheat flag.** Check `GET /flags` after the redeploy: the account must NOT have
      picked up a `fast-forward` entry. The backlog a client accumulates while the machine is
      down used to be measured against a `now`-anchored clock, so a deploy longer than ~30 s
      flagged honest players and auto-reverted their progress; `RESTART_GRACE_MS` backdates the
      startup anchor 15 min to cover it. Worth eyeballing on a real deploy because the SEEDING
      line itself is only reachable from `main()` — the tests pin the constants, not the boot.

## 9. Network-loss banner (transient, keep-playing)

- [ ] Kill wifi mid-session (server unreachable). After ~3 failed autosaves → a non-blocking
      top **"Not syncing — your progress isn't being saved (retrying...)"** banner (game keeps
      running, state preserved).
- [ ] Restore network → next autosave succeeds → banner clears (no revert of good progress).

---

### Priority if short on time
Core value = **1, 2, 3, 4, 6** (mode-select + i18n + the save/resume/refine loop + the
concurrent-login kick — all naturally triggerable). **5, 7, 8, 9** need synthetic conditions;
do them when convenient. Report anything that renders wrong or behaves off → I fix.
