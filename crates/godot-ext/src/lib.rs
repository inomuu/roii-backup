// godot-ext — thin gdext bridge between Godot and sim-core.
//
// API surface stays at ≤ 6 functions (GDD §4.1, working agreement §7.6):
//   1. init_game(seed, game_data_json, zone_id) -> bool
//   2. tick(n)
//   3. apply_input_json(event_json) -> bool
//   4. view_state() -> JSON string
//   5. state_hash() -> hex string
//   6. sim_version() -> string
//
// Rules at play here:
//   - GDScript may NEVER compute gameplay outcomes — only call into
//     these methods and visualize what `view_state()` returns.
//   - All Variant-friendly types: `i64`, `GString`, `bool`. We do NOT
//     leak `serde_json::Value` / Rust structs across the FFI; JSON is
//     the boundary format.
//   - Methods are total (no panics) — bad input returns `false` / the
//     last good state, so a buggy GDScript can't crash the sim.
//
// FILE LAYOUT — the `#[func]` methods are split across modules to keep each
// file under 500 lines. gdext (0.2.4) supports ONE primary `#[godot_api]`
// impl block plus any number of `#[godot_api(secondary)]` blocks on the same
// `#[derive(GodotClass)]` type, in any module of the crate. The primary block
// (game lifecycle: init/tick/view/save/load/hash/version) lives here; the
// rest live in sibling modules:
//   - bridge_world.rs — class / zone / party methods
//   - bridge_items.rs — shop / stash / consumable / refine methods
//   - bridge_stats.rs — derived-stat / skill / buff / soul-path + input
//   - item_view.rs    — the `item_to_view` JSON helper (free fn)

use godot::prelude::*;
use sim_core::{data, state::GameState};

mod bridge_items;
mod bridge_stats;
mod bridge_world;

// The view JSON + the session lifecycle live in `sim-host`, shared with `sim-ffi` (the C ABI for
// Unity/other hosts) so the two bridges run the SAME code and cannot drift.
use sim_host::item_view::item_to_view;
use sim_host::item_view::stat_mod_text;
use sim_host::session;

struct SimExtension;

#[gdextension]
unsafe impl ExtensionLibrary for SimExtension {}

/// `SimBridge` is the single Godot-visible class. Each Godot scene
/// holds one instance — there is no global; saves/loads create new
/// instances rather than mutate a singleton.
#[derive(GodotClass)]
#[class(base = RefCounted, init)]
struct SimBridge {
    /// `None` until `init_game` succeeds. All gameplay methods are
    /// no-ops while state is `None` — GDScript should check
    /// `view_state()` for emptiness before tick.
    state: Option<GameState>,
    /// Are DIRECT-mutation debug cheats permitted? False while ONLINE.
    ///
    /// Five bridge methods write `GameState` without going through `apply_input`, so nothing
    /// records them: `debug_set_hp_pct`, `debug_kill`, `debug_set_level`,
    /// `debug_give_currency`, `debug_grant_apex`. Online that is not a cheat, it is a corrupted
    /// session — the state stops being reproducible from (start + log) and EVERY later push is
    /// rejected as "replay hash mismatch", with nothing anywhere naming the cause. One stray
    /// keypress on 2026-08-03 cost an afternoon of chasing exactly that.
    ///
    /// Refusing is the point: a cheat that does nothing and says so is debuggable, a save that
    /// quietly stops verifying is not.
    debug_allowed: bool,
    /// Parsed + validated catalog. Stays around for the life of the
    /// SimBridge so future input events can reference it without
    /// re-parsing the JSON every tick.
    data: Option<data::GameData>,
    /// S1b: the session's START CHECKPOINT — a clone of `state` at new-game /
    /// create-character / load. `export_log()` pairs it with `state.input_log` to
    /// build a replayable, self-verifying `SaveBundle` (GDD §4.4).
    session_start: Option<GameState>,
    /// Set by `export_log()` at each push = `(the state snapshotted, input_log length
    /// then)`. `commit_anchor()` applies it AFTER the server confirms the save, moving
    /// the start checkpoint forward so the next bundle's log + snapshot chain stay SHORT
    /// (a long idle session never grows past MAX_REPLAY_SPAN). Confirm-gated: a rejected
    /// save leaves it and the log keeps growing until a save succeeds.
    pending_anchor: Option<(GameState, usize)>,
}

#[godot_api]
impl SimBridge {
    /// Initialize a new game from a seed + the contents of
    /// `game_data.json` + the id of the starting zone.
    ///
    /// Returns `true` on success. `false` means the JSON failed to
    /// parse, failed validation, or named an unknown zone — in any
    /// failure case the previous state (if any) is left untouched.
    #[func]
    fn init_game(&mut self, seed: i64, game_data_json: GString, zone_id: GString) -> bool {
        let Ok((st, gd)) = session::start_new(seed as u64, &game_data_json.to_string(), &zone_id.to_string()) else {
            return false;
        };
        // 0.0.12+: starting class lives on GameState (set by new_game_in_zone).
        // 0.0.162+: current_zone_id likewise lives on GameState (set by
        // populate_zone). The bridge no longer mirrors either.
        self.state = Some(st);
        self.data = Some(gd);
        self.session_start = self.state.clone(); // S1b: start checkpoint (pristine).
        self.pending_anchor = None; // a new session invalidates any stale re-anchor.
        true
    }

    /// Advance the sim by `n` ticks. No-op while uninitialized or n ≤ 0.
    ///
    /// After each tick we check `state.pending_zone_change`. If set, the sim has
    /// requested a zone swap (boss died) — we honour it here (change_zone stamps
    /// `state.current_zone_id`) so the next view_state() reflects the new zone.
    /// This is DETERMINISTIC (derived from tick state, not free input) so it stays
    /// in the driver rather than the event log — the replay verifier applies the
    /// same "pending → change_zone" rule.
    #[func]
    fn tick(&mut self, n: i64) {
        if n <= 0 {
            return;
        }
        let Some(d) = self.data.as_ref() else { return; };
        let Some(s) = self.state.as_mut() else { return; };
        // ONE driver step, shared with `replay::replay` (S1), so a re-run of this
        // session's (seed + initial + input log) reproduces it bit-for-bit.
        session::tick(s, d, n as u64);
    }

    /// Read-only snapshot of the full GameState as compact JSON.
    ///
    /// This is THE read interface — GDScript parses the JSON into a
    /// Dictionary, then draws from it. We deliberately do not expose
    /// per-entity getters — adding one would tempt GDScript to derive
    /// gameplay state ad-hoc, breaking §4.1.
    ///
    /// Empty string while uninitialized.
    #[func]
    fn view_state(&self) -> GString {
        // ALWAYS parseable JSON, never "". An empty string made `JSON.parse_string` log a parse
        // error on every frame before a game is loaded — harmless in itself (the caller falls back
        // to an empty dict) but it meant a REAL malformed payload would print the identical line
        // and be read as the usual noise. "{}" is the empty state; a parse error now means a bug.
        let Some(s) = self.state.as_ref() else {
            return "{}".into();
        };
        session::view_json(s, self.data.as_ref()).into()
    }

    /// Canonical state hash (hex). Useful for replay verification +
    /// debugging — paste into the golden test to confirm a scenario.
    #[func]
    fn state_hash(&self) -> GString {
        match self.state.as_ref() {
            Some(s) => session::state_hash(s).into(),
            None => "no_state".into(),
        }
    }

    /// `sim_core::VERSION`. Used to detect mismatch between bundled
    /// `.dll` and `game_data.json`'s `version` field.
    #[func]
    fn sim_version(&self) -> GString {
        sim_core::VERSION.into()
    }

    /// Git identity of the sim source this dylib was built from ("SIMGIT:<sha>",
    /// "-dirty" when built over uncommitted sim changes). Referencing the const
    /// here also guarantees the stamp lands in the compiled dylib, where
    /// tools/check_online_sync.sh extracts it with `strings` and compares it
    /// against the deployed server's /version git_sha.
    #[func]
    fn sim_git_stamp(&self) -> GString {
        sim_core::GIT_STAMP.into()
    }

    /// Serialize the current GameState as JSON for save-to-disk.
    /// Returns the empty string if no game is loaded.
    ///
    /// Phase 0 save format: just the GameState JSON. ALPHA/Phase 2
    /// will add a save header (game_data version, schema version,
    /// hash chain) — but the GameState payload itself stays the
    /// same so older saves can be migrated without re-running ticks.
    #[func]
    fn save_json(&self) -> GString {
        match self.state.as_ref() {
            Some(s) => session::save_json(s).into(),
            None => GString::default(),
        }
    }

    /// S1b: export a replayable, self-verifying `SaveBundle` as JSON — the session's
    /// start checkpoint + the recorded input log + the claimed end tick/hash (GDD
    /// §4.4). A server re-runs `SaveBundle::verify` (same sim-core, headless) to
    /// confirm the state was reached legitimately (server-plan Model C anti-cheat).
    /// Empty string if no game / start checkpoint is loaded.
    #[func]
    fn export_log(&mut self) -> GString {
        let (Some(start), Some(live), Some(gd)) = (
            self.session_start.as_ref(),
            self.state.as_ref(),
            self.data.as_ref(),
        ) else {
            return GString::default();
        };
        // Stash the re-anchor point for this push: the state as of now + its log length.
        // `commit_anchor()` applies it only if the server confirms the save.
        let Some((json, pending)) = session::export_log(start, live, gd) else {
            return GString::default();
        };
        self.pending_anchor = Some(pending);
        json.into()
    }

    /// Call AFTER the server VERIFIED the last `export_log()` bundle: move the start
    /// checkpoint to that confirmed state and rebase the log + snapshot chain, so the next
    /// bundle stays short (bounded replay span; a >14h idle session can still save). Events
    /// + ticks recorded during the async confirm are preserved. No-op if no pending anchor
    /// (nothing pushed, or a prior commit already consumed it).
    #[func]
    fn commit_anchor(&mut self) {
        let Some(pending) = self.pending_anchor.take() else {
            return;
        };
        let Some(live) = self.state.as_mut() else {
            return;
        };
        // The new start checkpoint = the confirmed state, cleaned to a pristine base.
        self.session_start = Some(session::commit_anchor(live, pending));
    }

    /// How many ticks the NEXT bundle would replay: live tick − the confirmed anchor's tick.
    /// ~0 in a healthy session (every 10s push re-anchors) and grows only while pushes fail.
    /// The client watches this against the server's `MAX_REPLAY_SPAN` — past that bound a bundle
    /// is refused outright and the span can never shrink again, so the session must stop before
    /// it gets there. -1 if there is no game loaded.
    #[func]
    fn replay_span(&self) -> i64 {
        let (Some(start), Some(live)) = (self.session_start.as_ref(), self.state.as_ref()) else {
            return -1;
        };
        session::replay_span(start, live)
    }

    /// Load a previously-saved GameState. Requires the same
    /// game_data_json that was active when the save was written —
    /// the catalog lives outside GameState so it must be re-supplied.
    /// Returns false on parse error or validation failure; in that
    /// case the existing state (if any) is left untouched.
    #[func]
    fn load_json(&mut self, save_json: GString, game_data_json: GString) -> bool {
        // Item migration to the CURRENT catalog, re-armed recording, and the pre-0.0.162
        // `current_zone_id` recovery all live in the shared lifecycle (sim-host::session::load).
        let Ok((state, gd)) = session::load(&save_json.to_string(), &game_data_json.to_string()) else {
            return false;
        };
        self.state = Some(state);
        self.data = Some(gd);
        // S1b: the loaded snapshot IS this session's start checkpoint; the log
        // restarts from here (GDD §4.4 "restore snapshot, replay events after it").
        self.session_start = self.state.clone();
        self.pending_anchor = None; // resume re-anchors here; drop any stale pending.
        true
    }
}
