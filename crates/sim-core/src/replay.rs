//! Replay & the canonical driver step (S1 — event-sourced save, GDD §4.4).
//!
//! The save is conceptually `(seed + initial state + Vec<RecordedInput>)`. Trust
//! comes from a verifier re-running the SAME sim-core over that log and getting the
//! SAME state hash (GDD §4.4 / server-plan Model C). For that to hold, the live
//! client and the verifier must advance the sim identically — so both go through
//! [`advance`], the one tick-driver step.
//!
//! Tick-triggered zone advances (a boss dying sets `pending_zone_change`) are
//! DETERMINISTIC and re-derived inside `advance`, so they are NOT recorded in the
//! input log — only free player inputs are (recorded in `apply_input_with`).

use serde::{Deserialize, Serialize};

use crate::data::{self, GameData};
use crate::events::{apply_input_with, RecordedInput};
use crate::state::GameState;

/// Default checkpoint spacing: 6000 ticks = 10 real minutes (10 ticks/s). The
/// bridge sets this on the live session; a verifier reads it from the bundle so it
/// regenerates the chain at the SAME ticks.
pub const SNAPSHOT_INTERVAL_TICKS: u64 = 6000;

/// One tamper-evident checkpoint in the snapshot chain (GDD §4.4). `state_hash` is
/// the canonical hash at `tick`; `chain_hash` binds it to the previous link
/// (`hash::chain_link`), so the sequence can't be reordered/swapped/re-timed.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Snapshot {
    pub tick: u64,
    pub state_hash: String,
    pub chain_hash: String,
}

/// Advance the sim by `n` ticks, honouring any tick-requested zone change after
/// each tick, and appending a hash-chain checkpoint every `snapshot_interval`
/// ticks. A boss death sets `pending_zone_change`; we apply it via `change_zone`
/// (which restamps `current_zone_id`), or clear it on failure so the loop can't
/// spin. This is the SINGLE driver step: the live bridge `tick()` and [`replay`]
/// both call it, so a re-run never diverges from the live session — and both
/// regenerate the SAME checkpoint chain.
///
/// (sim-cli keeps its own loop — it observes each zone change to carry CSV
/// counters — but performs the identical tick + change_zone, and it is a balance
/// tool that is never replay-verified.)
pub fn advance(state: &mut GameState, data: &GameData, n: u64) {
    for _ in 0..n {
        crate::tick::tick(state, 1);
        if let Some(next_id) = state.pending_zone_change.clone() {
            if data::change_zone(state, data, &next_id).is_err() {
                state.pending_zone_change = None;
            }
        }
        maybe_checkpoint(state);
    }
}

/// Append a checkpoint if `tick` lands on a `snapshot_interval` boundary. The
/// snapshot fields are `#[serde(skip)]`, so hashing the state here does NOT see
/// the growing chain — checkpointing can't perturb the very hashes it records.
fn maybe_checkpoint(state: &mut GameState) {
    let iv = state.snapshot_interval;
    if iv == 0 || state.tick % iv != 0 {
        return;
    }
    let state_hash = crate::hash::state_hash(state);
    let prev = state.snapshots.last().map(|s| s.chain_hash.as_str()).unwrap_or("");
    let chain_hash = crate::hash::chain_link(prev, state.tick, &state_hash);
    state.snapshots.push(Snapshot { tick: state.tick, state_hash, chain_hash });
}

/// Reconstruct the session state by replaying `log` onto a clone of `initial` (a
/// start-of-session snapshot), then advancing to `final_tick`. The server hashes
/// the result against the client's claimed state to verify it (GDD §4.4). Events
/// are applied at their recorded ticks, in order; `data` resolves data-dependent
/// events (a mismatched/older catalog makes those replay-safe no-ops, never a
/// panic — replay must survive game_data version skew).
pub fn replay(
    initial: &GameState,
    data: &GameData,
    log: &[RecordedInput],
    final_tick: u64,
) -> GameState {
    let mut s = initial.clone();
    // Don't re-record while replaying (the log is the input, not the output).
    s.record_inputs = false;
    for rec in log {
        let cur = s.tick;
        if rec.tick > cur {
            advance(&mut s, data, rec.tick - cur);
        }
        apply_input_with(&mut s, Some(data), rec.event.clone());
    }
    let cur = s.tick;
    if final_tick > cur {
        advance(&mut s, data, final_tick - cur);
    }
    s
}

/// A replayable, self-verifying save bundle (GDD §4.4:
/// `save = {header, seed, game_data_version, event_log[], snapshots[]}`).
///
/// This S1b form carries a SINGLE `start` snapshot (the session's checkpoint) plus
/// the log since it — enough to `verify` the end state. A periodic mid-session
/// snapshot CHAIN (so a server resumes from the latest valid snapshot instead of
/// replaying from tick 0) is the S1c refinement; the trust model is unchanged: a
/// snapshot is trusted only if it was itself verified when produced.
///
/// `start`'s own `input_log` / `record_inputs` are `#[serde(skip)]`, so serializing
/// the bundle stores the state snapshot once and the log once (no duplication).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SaveBundle {
    /// `sim_core::VERSION` at capture — a verifier must run a compatible sim-core.
    pub sim_version: String,
    /// `GameData.version` at capture — replay across a catalog bump re-validates.
    pub game_data_version: String,
    /// Session checkpoint the log replays onto (a fresh new-game state, or the
    /// snapshot a save was loaded from).
    pub start: GameState,
    /// Free player inputs recorded since `start` (tick-driven zone advances are
    /// deterministic and re-derived, so they are NOT here).
    pub log: Vec<RecordedInput>,
    /// Tick the live session had reached (replay advances to it).
    pub final_tick: u64,
    /// Claimed end-state hash — the verify target.
    pub final_hash: String,
    /// Checkpoint spacing (ticks) used to build `snapshots`; a verifier regenerates
    /// the chain at the same boundaries.
    pub snapshot_interval: u64,
    /// The tamper-evident checkpoint chain (GDD §4.4). Empty when checkpointing was
    /// off (`snapshot_interval == 0`).
    pub snapshots: Vec<Snapshot>,
    /// `SIM_BUILD` of the sim that captured this bundle — a digest of sim-core's source. `sim_version`
    /// alone let two builds that both say "0.0.490" pass the version gate and then diverge on
    /// replay, flagging an honest player (audit 2026-10-02). Empty from an older client or a
    /// hand-built bundle: not compared then.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub sim_build: String,
}

impl SaveBundle {
    /// Capture a bundle from a `start` checkpoint + the `live` session (its log +
    /// end tick + end hash become the verify target).
    pub fn capture(
        sim_version: &str,
        game_data_version: &str,
        start: GameState,
        live: &GameState,
    ) -> Self {
        Self {
            sim_version: sim_version.to_string(),
            game_data_version: game_data_version.to_string(),
            start,
            log: live.input_log.clone(),
            final_tick: live.tick,
            final_hash: crate::hash::state_hash(live),
            snapshot_interval: live.snapshot_interval,
            snapshots: live.snapshots.clone(),
            sim_build: crate::SIM_BUILD.to_string(),
        }
    }

    /// Replay the log over `start` and, IF it checks out, return the reconstructed
    /// final state — the AUTHORITATIVE state a server stores after proving a save
    /// (cloud-save only ever persists states it verified). `None` = tampered /
    /// diverged / incompatible catalog. Checks BOTH the end-state hash AND the full
    /// checkpoint chain (regenerated at the same interval), so a forged MID-session
    /// state that leaves the endpoints intact is caught. Total (never panics).
    pub fn verified_state(&self, data: &GameData) -> Option<GameState> {
        let mut base = self.start.clone();
        base.snapshot_interval = self.snapshot_interval;
        base.snapshots.clear();
        let end = replay(&base, data, &self.log, self.final_tick);
        if crate::hash::state_hash(&end) == self.final_hash && end.snapshots == self.snapshots {
            Some(end)
        } else {
            None
        }
    }

    /// [`verified_state`], and also whether the replay passed through state `via_hash` at tick
    /// `via_tick` (every input recorded at or before that tick applied). A server uses it to tell a
    /// CONTINUATION — a bundle re-sent from an older base after its first save landed but the reply
    /// was lost — from a rewind, which leaves that save's path. `None` = not verified;
    /// `Some((state, passed))` otherwise. Total (never panics).
    pub fn verified_state_via(&self, data: &GameData, via_tick: u64, via_hash: &str) -> Option<(GameState, bool)> {
        if via_tick < self.start.tick || via_tick > self.final_tick {
            return self.verified_state(data).map(|s| (s, false));
        }
        let mut s = self.start.clone();
        s.snapshot_interval = self.snapshot_interval;
        s.snapshots.clear();
        s.record_inputs = false;
        let before = self.log.partition_point(|r| r.tick < via_tick);
        let after = self.log.partition_point(|r| r.tick <= via_tick);
        for rec in self.log.get(..before).unwrap_or(&[]) {
            let cur = s.tick;
            if rec.tick > cur {
                advance(&mut s, data, rec.tick - cur);
            }
            apply_input_with(&mut s, Some(data), rec.event.clone());
        }
        let cur = s.tick;
        if via_tick > cur {
            advance(&mut s, data, via_tick - cur);
        }
        // Inputs recorded AT `via_tick` may sit on either side of the earlier capture (one landed
        // after it, before the next tick): the state is checked before each and after the last.
        let mut passed = crate::hash::state_hash(&s) == via_hash;
        for rec in self.log.get(before..after).unwrap_or(&[]) {
            apply_input_with(&mut s, Some(data), rec.event.clone());
            passed |= crate::hash::state_hash(&s) == via_hash;
        }
        let end = replay(&s, data, self.log.get(after..).unwrap_or(&[]), self.final_tick);
        if crate::hash::state_hash(&end) == self.final_hash && end.snapshots == self.snapshots {
            Some((end, passed))
        } else {
            None
        }
    }

    /// Boolean form of [`verified_state`] — the anti-cheat verdict (server-plan
    /// Model C). `false` = tampered / diverged / incompatible catalog.
    pub fn verify(&self, data: &GameData) -> bool {
        self.verified_state(data).is_some()
    }
}

/// Re-anchor a live session onto a just-CONFIRMED checkpoint (tick `anchor_tick`), so the
/// next exported bundle replays from THERE, not from genesis. Without this the input log +
/// snapshot chain grow for the whole session → each verify replays more ticks (cost) and a
/// session past `MAX_REPLAY_SPAN` (~14h) can no longer save at all. The caller sets its
/// session-start checkpoint to the confirmed state; this fixes up the LIVE state:
///   - drop the `confirmed_len` input events the bundle already covered (keep any recorded
///     during the async confirm gap),
///   - rebase the snapshot chain: keep only post-anchor snapshots and re-link their
///     `chain_hash` from an empty prev, exactly as a verifier replaying from the new anchor
///     regenerates it (state_hash per tick is anchor-independent; only the chain link moves).
/// MUST be called only after the server VERIFIED the bundle whose final == the anchor (so
/// the anchor's hash is a trusted base); otherwise the next save can't anchor.
pub fn reanchor(live: &mut GameState, anchor_tick: u64, confirmed_len: usize) {
    let n = confirmed_len.min(live.input_log.len());
    live.input_log.drain(0..n);
    live.snapshots.retain(|s| s.tick > anchor_tick);
    let mut prev = String::new();
    for s in live.snapshots.iter_mut() {
        s.chain_hash = crate::hash::chain_link(&prev, s.tick, &s.state_hash);
        prev = s.chain_hash.clone();
    }
}

/// DIAGNOSTIC for a rejected save: replay the client's own bundle and report which FIELD of the
/// resulting state disagrees with the client's.
///
/// `#[ignore]` — it reads a dump the game writes, so it only runs when asked:
///
/// ```text
/// MISMATCH_DUMP="$HOME/Library/Application Support/reign_of_idlers/mismatch_dump.json" \
///   cargo test -p sim-core --lib diag_mismatch -- --ignored --nocapture
/// ```
///
/// A rejection says only that two hashes differ, which is where the 2026-08-03 investigation
/// stalled: the state was reproducible from its own snapshot, serde lost nothing, sim and
/// catalog matched on both sides, and there was still nothing to point at. Naming the field is
/// the whole job.
#[cfg(test)]
mod diag_mismatch {
    /// Where the two states disagree, walked one level into `grid` because "grid differs" on a
    /// 24x12 board with a party and a spawn pool is not an answer.
    fn report(a: &serde_json::Value, b: &serde_json::Value) -> usize {
        let (Some(ao), Some(bo)) = (a.as_object(), b.as_object()) else {
            return usize::from(a != b);
        };
        let mut n = 0;
        for (k, av) in ao {
            let bv = bo.get(k).unwrap_or(&serde_json::Value::Null);
            if av == bv {
                continue;
            }
            n += 1;
            if k == "grid" {
                eprintln!("  grid:");
                let ents = |v: &serde_json::Value| -> Vec<serde_json::Value> {
                    v.get("entities").and_then(|e| e.as_array()).cloned().unwrap_or_default()
                };
                let (ea, eb) = (ents(av), ents(bv));
                if ea.len() != eb.len() {
                    eprintln!("    entity COUNT {} vs {}", ea.len(), eb.len());
                }
                for (i, (x, y)) in ea.iter().zip(eb.iter()).enumerate() {
                    if x == y {
                        continue;
                    }
                    let id = x.get("id").cloned().unwrap_or(serde_json::Value::Null);
                    let (Some(xo), Some(yo)) = (x.as_object(), y.as_object()) else { continue };
                    let diffs: Vec<&String> = xo.keys()
                        .filter(|f| yo.get(*f) != xo.get(*f)).collect();
                    eprintln!("    entity[{i}] id={id}: {diffs:?}");
                }
            } else {
                eprintln!("  `{k}` differs");
            }
        }
        n
    }

    #[test]
    #[ignore]
    fn diag_mismatch_names_the_field() {
        let path = std::env::var("MISMATCH_DUMP").expect("set MISMATCH_DUMP to the dump path");
        let raw = std::fs::read_to_string(&path).expect("dump readable");
        let v: serde_json::Value = serde_json::from_str(&raw).expect("dump parses");
        // The dump grew a wrapper on 2026-08-03; accept both shapes.
        let (bundle, client) = match v.get("bundle") {
            Some(b) => (b.clone(), v.get("client_final_state").cloned()),
            None => (v.clone(), None),
        };
        let gd_path = std::env::var("GAME_DATA")
            .unwrap_or_else(|_| concat!(env!("CARGO_MANIFEST_DIR"), "/../../data/game_data.json").into());
        let d = crate::data::GameData::from_json_str(&std::fs::read_to_string(gd_path).unwrap())
            .expect("catalog parses");

        let start: crate::state::GameState =
            serde_json::from_value(bundle["start"].clone()).expect("start parses");
        let log: Vec<crate::events::RecordedInput> =
            serde_json::from_value(bundle["log"].clone()).unwrap_or_default();
        let final_tick = bundle["final_tick"].as_u64().unwrap();
        eprintln!("start {} -> {}  ({} logged events)", start.tick, final_tick, log.len());

        let mine = super::replay(&start, &d, &log, final_tick);
        let claimed = bundle["final_hash"].as_str().unwrap_or("");
        eprintln!("replay hash : {}", crate::hash::state_hash(&mine));
        eprintln!("client hash : {claimed}");

        let Some(client) = client else {
            eprintln!("\nNo `client_final_state` in this dump — it predates the wrapper, so the \
                       field cannot be named. Reproduce once more with a current build.");
            return;
        };
        let n = report(&serde_json::to_value(&mine).unwrap(), &client);
        eprintln!("\n{n} top-level field(s) differ (left = local replay, right = client)");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::events::InputEvent;
    use crate::hash::state_hash;

    fn game_data() -> GameData {
        GameData::from_json_str(include_str!("../../../data/game_data.json"))
            .expect("game_data.json must parse")
    }

    /// The end-to-end payoff of the S0 hole-closing + S1 recording: a session
    /// played through `advance` + recorded events replays bit-for-bit from its
    /// start snapshot. Exercises deterministic ticks (PRNG combat/drops), a
    /// data-dependent zone event (SelectBand → change_zone), a data-free event
    /// (SetFarmMode), and another data-dependent event (RespawnInZone). If ANY
    /// mutation escaped the event log (a reopened S0 hole) or `advance` diverged,
    /// the hashes would differ.
    /// The anchor a verifier receives is DESERIALIZED, not cloned — and that is a
    /// strictly stronger requirement than `replay_reproduces_a_recorded_session`
    /// checks, because `clone()` copies `#[serde(skip)]` fields that the wire drops.
    ///
    /// Four `Projectile` fields the sim READS at impact (`is_basic`, `burn_tier`,
    /// `focus_mult_bp`, `pin_slow_pct`) were skipped on exactly that reasoning — each
    /// documented as harmless because "a reload mid-flight just drops it for that one
    /// shot". True for a player reloading; false for replay-verify, where the client
    /// keeps the value and the server does not. A shot airborne across the anchor then
    /// burns/slows/crits on one side only, and the save is rejected with an EMPTY input
    /// log — nothing to point at. That shipped and rejected real saves.
    ///
    /// So this asserts the general contract, not the four fields: freeze the state
    /// mid-flight, continue from the wire form, and the hashes must not part.
    #[test]
    fn a_deserialized_anchor_continues_identically() {
        let d = game_data();
        // Rapis: a ranged kit, so a projectile is in the air within a few seconds.
        let mut live = data::new_game_as(42, &d, "zone_b1_1", "lapis").unwrap();

        let mut spun = 0;
        while live.projectiles.is_empty() && spun < 3_000 {
            advance(&mut live, &d, 1);
            spun += 1;
        }
        // Guard against a vacuous pass: with no projectile airborne this test proves nothing.
        assert!(!live.projectiles.is_empty(), "no projectile in flight after {spun} ticks");

        let p = &mut live.projectiles[0];
        p.is_basic = true;
        p.burn_tier = 3;
        p.focus_mult_bp = 30_000;
        p.pin_slow_pct = 25;

        let anchor: GameState =
            serde_json::from_str(&serde_json::to_string(&live).unwrap()).expect("anchor round-trips");
        let q = &anchor.projectiles[0];
        assert_eq!(
            (q.is_basic, q.burn_tier, q.focus_mult_bp, q.pin_slow_pct),
            (true, 3, 30_000, 25),
            "a transient the sim reads at impact did not survive the wire"
        );

        let mut from_anchor = anchor;
        advance(&mut live, &d, 200);
        advance(&mut from_anchor, &d, 200);
        assert_eq!(state_hash(&live), state_hash(&from_anchor));
    }

    #[test]
    fn replay_reproduces_a_recorded_session() {
        let d = game_data();
        let mut live = data::new_game_in_zone(42, &d, "zone_b1_1").unwrap();
        let initial = live.clone(); // pristine tick-0 base (record_inputs = false)
        live.record_inputs = true; // start recording the live session

        advance(&mut live, &d, 120);
        apply_input_with(&mut live, Some(&d), InputEvent::SelectBand { band: 2 });
        advance(&mut live, &d, 80);
        apply_input_with(&mut live, Some(&d), InputEvent::SetFarmMode { on: true });
        advance(&mut live, &d, 60);
        apply_input_with(&mut live, Some(&d), InputEvent::RespawnInZone);
        advance(&mut live, &d, 60);

        assert!(!live.input_log.is_empty(), "the session recorded inputs");
        let replayed = replay(&initial, &d, &live.input_log, live.tick);

        assert_eq!(replayed.tick, live.tick, "replay reached the same tick");
        assert_eq!(
            state_hash(&replayed),
            state_hash(&live),
            "replay(seed + initial + log) must reproduce the live state bit-for-bit"
        );
    }

    /// After a CONFIRMED save, `reanchor` produces a SHORT next bundle (start = the
    /// confirmed state, log/chain rebased from there) that still verifies and reconstructs
    /// the true current state — even with events + ticks in the async confirm gap. This is
    /// what keeps a long idle session's replay span bounded (never grows to MAX_REPLAY_SPAN).
    #[test]
    fn reanchor_yields_a_short_equivalent_bundle() {
        let d = game_data();
        let mut live = data::new_game_in_zone(42, &d, "zone_b1_1").unwrap();
        live.record_inputs = true;
        live.snapshot_interval = 50;

        // Play a long stretch (this would be the whole session under the old model).
        advance(&mut live, &d, 137);
        apply_input_with(&mut live, Some(&d), InputEvent::SelectBand { band: 2 });
        advance(&mut live, &d, 213);

        // PUSH point: snapshot the confirmed state B + its log length.
        let anchor = live.clone();
        let confirmed_len = live.input_log.len();

        // Async confirm gap: more ticks AND an event land before the ok arrives.
        advance(&mut live, &d, 40);
        apply_input_with(&mut live, Some(&d), InputEvent::SetFarmMode { on: true });
        advance(&mut live, &d, 25);
        let truth = state_hash(&live);

        // Re-anchor onto B, then export the next bundle from the cleaned start.
        reanchor(&mut live, anchor.tick, confirmed_len);
        let mut base = anchor.clone();
        base.input_log.clear();
        base.snapshots.clear();
        base.record_inputs = false;
        let bundle = SaveBundle::capture(crate::VERSION, &d.version, base, &live);

        assert!(bundle.verify(&d), "re-anchored short bundle verifies (log + chain rebased)");
        assert_eq!(
            state_hash(&bundle.verified_state(&d).unwrap()),
            truth,
            "it reconstructs the TRUE current state, gap events included"
        );
        assert_eq!(bundle.start.tick, anchor.tick, "start moved to the confirmed anchor");
        assert!(
            bundle.final_tick - bundle.start.tick < 100,
            "span is just the gap (~65t), not the whole session (~350t)"
        );
    }

    /// A bundle re-sent from an OLDER base after its first save landed passes through that save's
    /// state; a bundle that diverged before it does not.
    #[test]
    fn verified_state_via_tells_a_continuation_from_a_rewind() {
        let d = game_data();
        let mut live = data::new_game_in_zone(7, &d, "zone_b1_1").unwrap();
        let start = live.clone();
        live.record_inputs = true;
        live.snapshot_interval = 50;
        advance(&mut live, &d, 120);
        apply_input_with(&mut live, Some(&d), InputEvent::SelectBand { band: 2 });
        advance(&mut live, &d, 30);
        let (mid_tick, mid_hash) = (live.tick, state_hash(&live));
        advance(&mut live, &d, 70);
        apply_input_with(&mut live, Some(&d), InputEvent::SetFarmMode { on: true });
        advance(&mut live, &d, 40);
        let bundle = SaveBundle::capture(crate::VERSION, &d.version, start.clone(), &live);
        let (end, passed) = bundle.verified_state_via(&d, mid_tick, &mid_hash).expect("verifies");
        assert!(passed, "an honest continuation passes the earlier save");
        assert_eq!(state_hash(&end), bundle.final_hash);
        assert!(!bundle.verified_state_via(&d, mid_tick, &"0".repeat(64)).unwrap().1, "another path does not");

        // An input recorded AT the earlier save's tick, after it was captured, still continues it.
        let mut live2 = data::new_game_in_zone(7, &d, "zone_b1_1").unwrap();
        let start2 = live2.clone();
        live2.record_inputs = true;
        live2.snapshot_interval = 50;
        advance(&mut live2, &d, 150);
        let (t2, h2) = (live2.tick, state_hash(&live2));
        apply_input_with(&mut live2, Some(&d), InputEvent::SetFarmMode { on: true });
        advance(&mut live2, &d, 30);
        let b2 = SaveBundle::capture(crate::VERSION, &d.version, start2, &live2);
        assert!(b2.verified_state_via(&d, t2, &h2).expect("verifies").1, "same-tick input after the capture");

        // ...and one recorded at that tick BEFORE the capture: the capture holds its effect.
        let mut live3 = data::new_game_in_zone(7, &d, "zone_b1_1").unwrap();
        let start3 = live3.clone();
        live3.record_inputs = true;
        live3.snapshot_interval = 50;
        advance(&mut live3, &d, 150);
        apply_input_with(&mut live3, Some(&d), InputEvent::SetFarmMode { on: true });
        let (t3, h3) = (live3.tick, state_hash(&live3));
        advance(&mut live3, &d, 30);
        let b3 = SaveBundle::capture(crate::VERSION, &d.version, start3, &live3);
        assert!(b3.verified_state_via(&d, t3, &h3).expect("verifies").1, "same-tick input before the capture");

        // A rewind: same base, different choices before mid_tick.
        let mut alt = start.clone();
        alt.record_inputs = true;
        alt.snapshot_interval = 50;
        advance(&mut alt, &d, 60);
        apply_input_with(&mut alt, Some(&d), InputEvent::SetFarmMode { on: true });
        advance(&mut alt, &d, 200);
        let rewind = SaveBundle::capture(crate::VERSION, &d.version, start, &alt);
        assert!(!rewind.verified_state_via(&d, mid_tick, &mid_hash).unwrap().1);
    }

    /// A SaveBundle with a hash-chain checkpoint sequence survives JSON round-trip
    /// and self-verifies; forging the end state, a MID-session checkpoint, or the
    /// chain length all make verify() fail (the server-side anti-cheat check).
    #[test]
    fn save_bundle_snapshot_chain_verifies_and_detects_tampering() {
        let d = game_data();
        let mut live = data::new_game_in_zone(7, &d, "zone_b1_1").unwrap();
        let start = live.clone();
        live.record_inputs = true;
        live.snapshot_interval = 50; // small interval → several checkpoints

        advance(&mut live, &d, 90);
        apply_input_with(&mut live, Some(&d), InputEvent::SelectBand { band: 2 });
        advance(&mut live, &d, 90);
        assert!(live.snapshots.len() >= 2, "several checkpoints accumulated");

        let bundle = SaveBundle::capture(crate::VERSION, &d.version, start, &live);
        let json = serde_json::to_string(&bundle).unwrap();
        let restored: SaveBundle = serde_json::from_str(&json).unwrap();
        assert!(restored.verify(&d), "honest bundle (chain + final) verifies after round-trip");

        // Forge the claimed end state → reject.
        let mut f1 = restored.clone();
        f1.final_hash = "0".repeat(64);
        assert!(!f1.verify(&d), "tampered end hash fails");

        // Forge a MID-session checkpoint while leaving the endpoints intact → the
        // regenerated chain won't match → reject.
        let mut f2 = restored.clone();
        f2.snapshots[0].state_hash = "0".repeat(64);
        assert!(!f2.verify(&d), "forged mid-session checkpoint fails");

        // Drop a checkpoint (chain length mismatch) → reject.
        let mut f3 = restored.clone();
        f3.snapshots.pop();
        assert!(!f3.verify(&d), "missing checkpoint fails");
    }

    /// A verifier with NO catalog can't resolve data-dependent events, so replaying
    /// a log full of them onto the start snapshot must NOT panic (idempotent no-op)
    /// — the replay-survives-version-skew rule. (This uses apply_input directly.)
    #[test]
    fn replay_without_catalog_is_a_safe_noop() {
        let d = game_data();
        let mut s = data::new_game_in_zone(1, &d, "zone_b1_1").unwrap();
        let before = state_hash(&s);
        crate::events::apply_input(&mut s, InputEvent::RespawnInZone);
        crate::events::apply_input(&mut s, InputEvent::SelectBand { band: 3 });
        assert_eq!(state_hash(&s), before, "no catalog → data-dependent events no-op");
    }
}
