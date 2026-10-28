//! C-ABI bridge to sim-core, for a non-Godot host (Unity via P/Invoke, or anything with a C FFI).
//!
//! A thin wrapper over `sim_host::session::Session` — the SAME lifecycle code `godot-ext`'s
//! `SimBridge` runs (start / load / tick / apply input / view / export bundle / re-anchor), so the
//! two bridges cannot drift. JSON is the ONLY type that crosses, exactly as on the gdext side.
//!
//! Every function is TOTAL: a null or malformed argument returns null / 0 / does nothing rather
//! than unwinding across the FFI boundary, which is undefined behaviour. `sim-core` and `sim-host`
//! stay `forbid(unsafe_code)`; the unsafe lives here, where the pointers do.
//!
//! HOST RULES (see docs/sim-ffi-unity-gap.md):
//! - Every `char*` this library returns is owned by the CALLER and must be released with
//!   `roi_string_free` — never with the host's allocator. In C# declare those returns as
//!   `IntPtr`, copy with `Marshal.PtrToStringUTF8`, then free. Declaring them `string` lets the
//!   .NET marshaller free a Rust allocation: silent heap corruption.
//! - Strings passed IN are UTF-8, NUL-terminated, valid only for the duration of the call.
//! - One thread at a time per `Sim*`. Ship the RELEASE build only: it sets `panic = "abort"`,
//!   so a sim panic ends the process instead of unwinding through C.

use std::ffi::{c_char, CStr, CString};

use sim_host::session::Session;

/// Opaque handle: one game session.
pub struct Sim(Session);

/// Read a C string as `&str`, or None if null / not UTF-8.
///
/// # Safety
/// `p` must be null or a valid NUL-terminated C string that outlives the call.
unsafe fn as_str<'a>(p: *const c_char) -> Option<&'a str> {
    if p.is_null() {
        return None;
    }
    CStr::from_ptr(p).to_str().ok()
}

fn to_c(s: String) -> *mut c_char {
    // A NUL inside would truncate the payload silently; JSON never contains one, and if it
    // somehow did, a null return is a visibly wrong answer rather than a quietly short one.
    CString::new(s).map(|c| c.into_raw()).unwrap_or(std::ptr::null_mut())
}

// ------------------------------------------------------------------------------------ lifecycle

/// Start a game. `game_data_json` is the catalog (`data/game_data.json`), `zone_id` the start
/// zone. Returns null if either is missing or the catalog does not parse.
///
/// # Safety
/// Both pointers must be null or valid NUL-terminated C strings.
#[no_mangle]
pub unsafe extern "C" fn roi_new(seed: u64, game_data_json: *const c_char, zone_id: *const c_char) -> *mut Sim {
    let (Some(gd), Some(zone)) = (as_str(game_data_json), as_str(zone_id)) else {
        return std::ptr::null_mut();
    };
    match Session::new(seed, gd, zone) {
        Ok(s) => Box::into_raw(Box::new(Sim(s))),
        Err(_) => std::ptr::null_mut(),
    }
}

/// Character creation: start as `unit_def` in `zone_id` with a recorded party name (the Godot
/// client's `create_character`). Null on any bad argument.
///
/// # Safety
/// All pointers must be null or valid NUL-terminated C strings.
#[no_mangle]
pub unsafe extern "C" fn roi_new_as(
    seed: u64,
    game_data_json: *const c_char,
    zone_id: *const c_char,
    unit_def: *const c_char,
    party_name: *const c_char,
) -> *mut Sim {
    let (Some(gd), Some(zone), Some(class), Some(name)) =
        (as_str(game_data_json), as_str(zone_id), as_str(unit_def), as_str(party_name))
    else {
        return std::ptr::null_mut();
    };
    match Session::new_as(seed, gd, zone, class, name) {
        Ok(s) => Box::into_raw(Box::new(Sim(s))),
        Err(_) => std::ptr::null_mut(),
    }
}

/// Create the fixed Seraphine + Rapis starter party.
/// # Safety
/// All string pointers must be null or valid NUL-terminated C strings.
#[no_mangle]
pub unsafe extern "C" fn roi_new_starter(seed: u64, game_data_json: *const c_char, zone_id: *const c_char, party_name: *const c_char) -> *mut Sim {
    let (Some(gd), Some(zone), Some(name)) = (as_str(game_data_json), as_str(zone_id), as_str(party_name)) else { return std::ptr::null_mut(); };
    match Session::new_starter(seed, gd, zone, name) {
        Ok(s) => Box::into_raw(Box::new(Sim(s))),
        Err(_) => std::ptr::null_mut(),
    }
}

/// Resume from a save (`roi_save_json` output) against the CURRENT catalog: items are migrated,
/// recording is re-armed, the loaded snapshot becomes the start checkpoint. Returns a NEW handle
/// (free the old one); null on parse/validation failure.
///
/// # Safety
/// Both pointers must be null or valid NUL-terminated C strings.
#[no_mangle]
pub unsafe extern "C" fn roi_load_json(save_json: *const c_char, game_data_json: *const c_char) -> *mut Sim {
    let (Some(save), Some(gd)) = (as_str(save_json), as_str(game_data_json)) else {
        return std::ptr::null_mut();
    };
    match Session::load(save, gd) {
        Ok(s) => Box::into_raw(Box::new(Sim(s))),
        Err(_) => std::ptr::null_mut(),
    }
}

/// Exact independent copy for input previews. Unlike loading a save, this never migrates
/// inventory, reparses the catalog, or resets recording/debug permissions.
/// # Safety
/// `sim` must be null or a valid live handle, with no concurrent mutation.
#[no_mangle]
pub unsafe extern "C" fn roi_clone(sim: *const Sim) -> *mut Sim {
    match sim.as_ref() {
        Some(s) => Box::into_raw(Box::new(Sim(s.0.clone()))),
        None => std::ptr::null_mut(),
    }
}

/// Destroy a sim. Safe on null; never call twice on the same pointer.
///
/// # Safety
/// `sim` must be null or a pointer from `roi_new*` / `roi_load_json` that has not been freed.
#[no_mangle]
pub unsafe extern "C" fn roi_free(sim: *mut Sim) {
    if !sim.is_null() {
        drop(Box::from_raw(sim));
    }
}

// ------------------------------------------------------------------------------------ driving

/// Advance `n` ticks (1 tick = 100 ms) through the replay-shared driver step. No-op on null.
///
/// # Safety
/// `sim` must be null or a live handle.
#[no_mangle]
pub unsafe extern "C" fn roi_tick(sim: *mut Sim, n: u64) {
    if let Some(s) = sim.as_mut() {
        s.0.tick(n);
    }
}

/// Apply one `InputEvent` as serde JSON (e.g. `{"SetPartyName":{"name":"x"}}`), recorded into the
/// session log. 1 = parsed and applied, 0 = null handle or JSON that is not an InputEvent. An
/// event the sim rejects is a silent no-op (still 1), by design.
///
/// # Safety
/// `sim` must be null or a live handle; `event_json` null or a valid C string.
#[no_mangle]
pub unsafe extern "C" fn roi_apply_input(sim: *mut Sim, event_json: *const c_char) -> i32 {
    let (Some(s), Some(j)) = (sim.as_mut(), as_str(event_json)) else {
        return 0;
    };
    i32::from(s.0.apply_input_json(j))
}

// ------------------------------------------------------------------------------------ reading

/// The client-facing VIEW JSON (derived fields injected, big lists trimmed) — what a renderer
/// draws from. Always parseable; `{}` if the view cannot be built. Caller frees.
///
/// # Safety
/// `sim` must be null or a live handle.
#[no_mangle]
pub unsafe extern "C" fn roi_view_json(sim: *mut Sim) -> *mut c_char {
    match sim.as_ref() {
        Some(s) => to_c(s.0.view_json()),
        None => to_c("{}".to_string()),
    }
}

/// One item COLLECTION as a JSON array: `inventory`, `stash`, `escrow`, `rare_bag`,
/// `card_album`, `owned_exemplars`, `owned_regalia`. Caller frees.
///
/// These are the lists `roi_view_json` deliberately empties — at ten ticks a second the bag alone
/// stutters — so a screen asks for the one it needs when it opens. ONE entry point with a key,
/// rather than the eighteen separate getters the Godot bridge grew: a C ABI pays for each symbol
/// twice, once here and once in every host's wrapper. An unknown key answers `[]`.
///
/// # Safety
/// `sim` must be null or a live handle; `key` null or a valid C string.
#[no_mangle]
pub unsafe extern "C" fn roi_query_json(sim: *mut Sim, key: *const c_char) -> *mut c_char {
    match (sim.as_ref(), as_str(key)) {
        (Some(s), Some(k)) => to_c(s.0.query_json(k)),
        _ => to_c("[]".to_string()),
    }
}

/// DEV CHEATS. One entry point taking a command, for the same reason `roi_query_json` takes a key:
/// a C ABI pays for every symbol twice, and the Godot bridge grew a dozen separate `debug_*`
/// functions this deliberately does not repeat.
///
/// These are DIRECT state edits and NOT inputs — see `session::debug_json` for why that is the
/// safe shape and not the lazy one. Refused once the host calls `roi_set_debug_allowed(sim, 0)`,
/// which it must do at login.
///
/// Returns 1 if the command applied, 0 if it was refused, unknown, or aimed at nothing.
///
/// # Safety
/// `sim` must be null or a live handle; `cmd` null or a valid C string.
#[no_mangle]
pub unsafe extern "C" fn roi_debug_json(sim: *mut Sim, cmd: *const c_char) -> i32 {
    match (sim.as_mut(), as_str(cmd)) {
        (Some(s), Some(c)) => i32::from(s.0.debug_json(c)),
        _ => 0,
    }
}

/// Permit or refuse the direct-edit cheats. A host MUST refuse them while online: a direct edit is
/// absent from the input log, so the save stops verifying and every push after it is rejected.
///
/// # Safety
/// `sim` must be null or a live handle.
#[no_mangle]
pub unsafe extern "C" fn roi_set_debug_allowed(sim: *mut Sim, allowed: i32) {
    if let Some(s) = sim.as_mut() {
        s.0.set_allow_debug(allowed != 0);
    }
}

/// The full `GameState` as JSON — the save-to-disk payload (NOT the view). Caller frees.
///
/// # Safety
/// `sim` must be null or a live handle.
#[no_mangle]
pub unsafe extern "C" fn roi_save_json(sim: *mut Sim) -> *mut c_char {
    match sim.as_ref() {
        Some(s) => to_c(s.0.save_json()),
        None => std::ptr::null_mut(),
    }
}

/// Deprecated alias of `roi_save_json` (the original name; it never was the view).
///
/// # Safety
/// As `roi_save_json`.
#[no_mangle]
pub unsafe extern "C" fn roi_state_json(sim: *mut Sim) -> *mut c_char {
    roi_save_json(sim)
}

/// Canonical state hash (hex). Caller frees.
///
/// # Safety
/// `sim` must be null or a live handle.
#[no_mangle]
pub unsafe extern "C" fn roi_state_hash(sim: *mut Sim) -> *mut c_char {
    match sim.as_ref() {
        Some(s) => to_c(s.0.state_hash()),
        None => to_c("no_state".to_string()),
    }
}

// ------------------------------------------------------------------------------------ online

/// Export the replayable `SaveBundle` for a push and arm the re-anchor. Null if it cannot be
/// built. Caller frees.
///
/// # Safety
/// `sim` must be null or a live handle.
#[no_mangle]
pub unsafe extern "C" fn roi_export_log(sim: *mut Sim) -> *mut c_char {
    let Some(s) = sim.as_mut() else { return std::ptr::null_mut() };
    let json = s.0.export_log();
    if json.is_empty() {
        std::ptr::null_mut()
    } else {
        to_c(json)
    }
}

/// Call AFTER the server confirmed the last exported bundle. No-op otherwise.
///
/// # Safety
/// `sim` must be null or a live handle.
#[no_mangle]
pub unsafe extern "C" fn roi_commit_anchor(sim: *mut Sim) {
    if let Some(s) = sim.as_mut() {
        s.0.commit_anchor();
    }
}

/// Ticks the next bundle would replay; -1 on a null handle.
///
/// # Safety
/// `sim` must be null or a live handle.
#[no_mangle]
pub unsafe extern "C" fn roi_replay_span(sim: *mut Sim) -> i64 {
    match sim.as_ref() {
        Some(s) => s.0.replay_span(),
        None => -1,
    }
}

// ------------------------------------------------------------------------------------ identity

/// sim-core's version, so the host can refuse a mismatched dylib the way the Godot client does.
#[no_mangle]
pub extern "C" fn roi_sim_version() -> *mut c_char {
    to_c(sim_core::VERSION.to_string())
}

/// Git identity of the sim source this dylib was built from (`SIMGIT:<sha>`, `-dirty` when built
/// over uncommitted sim changes) — compared against the deployed server's /version.
#[no_mangle]
pub extern "C" fn roi_sim_git_stamp() -> *mut c_char {
    to_c(sim_core::GIT_STAMP.to_string())
}

/// `sim_core::SIM_BUILD` — the digest the server's save gate compares. The client checks it
/// against `/auth`'s `sim_build` at login so a skewed build is caught before any save is refused.
#[no_mangle]
pub extern "C" fn roi_sim_build() -> *mut c_char {
    to_c(sim_core::SIM_BUILD.to_string())
}

/// Free a string returned by this library. Never pass it anything else.
///
/// # Safety
/// `p` must be null or a pointer returned by one of this library's string functions, freed once.
#[no_mangle]
pub unsafe extern "C" fn roi_string_free(p: *mut c_char) {
    if !p.is_null() {
        drop(CString::from_raw(p));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cstr(s: &str) -> CString {
        CString::new(s).unwrap()
    }

    unsafe fn take(p: *mut c_char) -> String {
        assert!(!p.is_null(), "null string from the bridge");
        let s = CStr::from_ptr(p).to_str().unwrap().to_string();
        roi_string_free(p);
        s
    }

    fn catalog() -> CString {
        cstr(&std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/../../data/game_data.json")).expect("catalog"))
    }

    #[test]
    fn exact_preview_clone_preserves_inventory_and_is_independent() {
        unsafe {
            assert!(roi_clone(std::ptr::null()).is_null());
            let gd = catalog();
            let source = roi_new_starter(3914, gd.as_ptr(), cstr("zone_b1_1").as_ptr(), cstr("Preview").as_ptr());
            assert!(!source.is_null());
            assert_eq!(roi_debug_json(source, cstr(r#"{"grant_item":{"def_id":"ring_s1"}}"#).as_ptr()), 1);
            roi_set_debug_allowed(source, 0);
            let hash = take(roi_state_hash(source));
            let inventory = take(roi_query_json(source, cstr("inventory").as_ptr()));
            let log = take(roi_export_log(source));
            let copy = roi_clone(source);
            assert!(!copy.is_null());
            assert_eq!(take(roi_state_hash(copy)), hash);
            assert_eq!(take(roi_query_json(copy, cstr("inventory").as_ptr())), inventory);
            assert_eq!(take(roi_export_log(copy)), log);
            assert_eq!(roi_debug_json(copy, cstr(r#"{"grant_item":{"def_id":"armor_s1"}}"#).as_ptr()), 0);
            assert_eq!(roi_apply_input(copy, cstr(r#"{"EquipItem":{"entity_id":1,"inventory_idx":0}}"#).as_ptr()), 1);
            assert_ne!(take(roi_state_hash(copy)), hash);
            assert_eq!(take(roi_state_hash(source)), hash);
            assert_eq!(take(roi_query_json(source, cstr("inventory").as_ptr())), inventory);
            assert_eq!(take(roi_export_log(source)), log);
            roi_free(source);
            roi_tick(copy, 1);
            roi_free(copy);
        }
    }

    /// The whole round trip through the C surface, because a bridge that compiles proves nothing
    /// about whether a host can actually drive it: create, tick, input, read view, export a bundle
    /// that VERIFIES by replay (what the server does), re-anchor, free.
    #[test]
    fn the_c_surface_drives_a_verifiable_session() {
        let gd = catalog();
        let zone = cstr("zone_b1_1");
        unsafe {
            let sim = roi_new(42, gd.as_ptr(), zone.as_ptr());
            assert!(!sim.is_null(), "roi_new returned null on a real catalog");
            let h0 = take(roi_state_hash(sim));
            roi_tick(sim, 300);
            let ev = cstr(r#"{"ResetDps":null}"#);
            assert_eq!(roi_apply_input(sim, ev.as_ptr()), 1);
            roi_tick(sim, 300);
            let h1 = take(roi_state_hash(sim));
            assert_ne!(h0, h1, "600 ticks changed nothing — the bridge is inert");
            // View is the trimmed/derived contract, not the raw state.
            let view = take(roi_view_json(sim));
            let v: serde_json::Value = serde_json::from_str(&view).expect("view is JSON");
            assert!(v.get("tick").is_some(), "view carries the tick");
            let save = take(roi_save_json(sim));
            assert!(save.len() > view.len(), "raw save must be the bigger payload (view is trimmed)");
            // The bundle produced through C must pass the server's replay verification.
            let bundle_json = take(roi_export_log(sim));
            let gd_s = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/../../data/game_data.json")).unwrap();
            let data = sim_core::data::GameData::from_json_str(&gd_s).unwrap();
            let bundle: sim_core::replay::SaveBundle = serde_json::from_str(&bundle_json).unwrap();
            assert!(bundle.verify(&data), "bundle exported over the C ABI failed replay verify");
            assert_eq!(roi_replay_span(sim), 600);
            roi_commit_anchor(sim);
            assert_eq!(roi_replay_span(sim), 0);
            // Load round-trips to the same hash through the C surface.
            let save_c = cstr(&save);
            let loaded = roi_load_json(save_c.as_ptr(), gd.as_ptr());
            assert!(!loaded.is_null());
            assert_eq!(take(roi_state_hash(loaded)), h1);
            roi_free(loaded);
            roi_free(sim);
        }
    }

    /// Character creation records the name as an event, so the bundle's start stays name-free.
    #[test]
    fn create_character_records_the_name() {
        let gd = catalog();
        let (zone, class, name) = (cstr("zone_b1_1"), cstr("seraphine"), cstr("Idlers"));
        unsafe {
            let sim = roi_new_as(5, gd.as_ptr(), zone.as_ptr(), class.as_ptr(), name.as_ptr());
            assert!(!sim.is_null(), "roi_new_as refused a starter class");
            let save = take(roi_save_json(sim));
            assert!(save.contains("Idlers"), "party name missing from the state");
            let bundle = take(roi_export_log(sim));
            assert!(bundle.contains("SetPartyName"), "name must be a recorded event, not baked into the start");
            roi_free(sim);
        }
    }

    /// The same JSON keys and events the Unity window uses, exercised against
    /// a real starter kit, save/load, and the server's bundle replay path. The currency is the
    /// v2 purse — Sigil of the unit's own element (a LEDGER count, 0.0.354) plus Gil for a stat
    /// node; the test stocks them by editing the save it loads from.
    #[test]
    fn potential_purchase_and_load_cross_the_c_abi() {
        let gd = catalog();
        let (zone, class, name) = (cstr("zone_b1_1"), cstr("seraphine"), cstr("Board probe"));
        let query = cstr("potential:1");
        unsafe {
            let fresh = roi_new_as(42, gd.as_ptr(), zone.as_ptr(), class.as_ptr(), name.as_ptr());
            assert!(!fresh.is_null());
            // Stock twelve grade-I Sigil (two T1 purchases' worth) and the Gil for one, by way of
            // the save, then load it.
            let mut state: sim_core::state::GameState = serde_json::from_str(&take(roi_save_json(fresh))).unwrap();
            roi_free(fresh);
            state.credit_material(
                &sim_core::potential::sigil_def(0), 12, sim_core::state::MaterialSource::Drop);
            state.set_gil_for_test(10_000);
            let stocked = cstr(&serde_json::to_string(&state).unwrap());
            let sim = roi_load_json(stocked.as_ptr(), gd.as_ptr());
            assert!(!sim.is_null());
            let before: serde_json::Value = serde_json::from_str(&take(roi_query_json(sim, query.as_ptr()))).unwrap();
            assert_eq!(before["supported"], true);
            assert_eq!(before["purse"]["sigil"][0], 12, "the Sigil count crosses the ABI");
            assert_eq!(before["purse"]["gil"], 10_000);
            let buy = cstr(r#"{"UpgradePotential":{"entity_id":1,"node_id":"s1"}}"#);
            assert_eq!(roi_apply_input(sim, buy.as_ptr()), 1);
            roi_tick(sim, 20);
            let bought: serde_json::Value = serde_json::from_str(&take(roi_query_json(sim, query.as_ptr()))).unwrap();
            assert_eq!(bought["purse"]["sigil"][0], 6, "six grade-I Sigil spent (T1)");
            assert_eq!(bought["purse"]["gil"], 0, "the stat node's 10k Gil is spent");
            assert_eq!(bought["can_reset"], false, "no reset, ever");
            let save = cstr(&take(roi_save_json(sim)));
            let loaded = roi_load_json(save.as_ptr(), gd.as_ptr());
            assert!(!loaded.is_null());
            assert_eq!(take(roi_state_hash(loaded)), take(roi_state_hash(sim)));
            roi_free(sim);
            // A reset is a tombstone: parses, changes nothing.
            let reset = cstr(r#"{"ResetPotential":{"entity_id":1}}"#);
            assert_eq!(roi_apply_input(loaded, reset.as_ptr()), 1);
            let after: serde_json::Value = serde_json::from_str(&take(roi_query_json(loaded, query.as_ptr()))).unwrap();
            assert_eq!(after["purse"]["sigil"][0], 6);
            roi_tick(loaded, 20);
            let bundle: sim_core::replay::SaveBundle = serde_json::from_str(&take(roi_export_log(loaded))).unwrap();
            let data = sim_core::data::GameData::from_json_str(gd.to_str().unwrap()).unwrap();
            assert!(bundle.verify(&data), "a board purchase after load must replay through the server verifier");
            roi_free(loaded);
        }
    }

    #[test]
    fn the_fixed_starter_survives_save_and_verifier_replay() {
        let gd = catalog();
        let zone = cstr("zone_b1_1");
        let name = cstr("New party");
        unsafe {
            let sim = roi_new_starter(42, gd.as_ptr(), zone.as_ptr(), name.as_ptr());
            assert!(!sim.is_null());
            let state: sim_core::state::GameState = serde_json::from_str(&take(roi_save_json(sim))).unwrap();
            let members: Vec<_> = state.grid.entities.iter().filter(|e| e.team == sim_core::grid::Team::Player).collect();
            // One unit alone since 0.0.415 (Suzu since 0.0.493) — every other character is a pull.
            assert_eq!(members.iter().map(|e| (e.id, e.unit_def.as_str())).collect::<Vec<_>>(), vec![(1, sim_core::data::STARTER_UNIT)]);
            for id in [1] {
                let event = cstr(&format!(r#"{{"UpgradePotential":{{"entity_id":{id},"node_id":"s1"}}}}"#));
                assert_eq!(roi_apply_input(sim, event.as_ptr()), 1);
            }
            roi_tick(sim, 20);
            let saved = cstr(&take(roi_save_json(sim)));
            let loaded = roi_load_json(saved.as_ptr(), gd.as_ptr());
            assert!(!loaded.is_null());
            assert_eq!(take(roi_state_hash(sim)), take(roi_state_hash(loaded)));
            let bundle: sim_core::replay::SaveBundle = serde_json::from_str(&take(roi_export_log(sim))).unwrap();
            let data = sim_core::data::GameData::from_json_str(gd.to_str().unwrap()).unwrap();
            assert!(bundle.verify(&data));
            roi_free(loaded); roi_free(sim);
        }
    }

    #[test]
    fn old_unskilled_pair_loads_with_free_kit_and_matches_server_migration() {
        let gd = catalog();
        let data = sim_core::data::GameData::from_json_str(gd.to_str().unwrap()).unwrap();
        let mut old = sim_core::data::new_starter_party(42, &data, "zone_b1_1").unwrap();
        for e in old.grid.entities.iter_mut().filter(|e| e.team == sim_core::grid::Team::Player) {
            e.skills.clear(); e.active_skills.clear(); e.stats.level = 12;
        }
        let raw = cstr(&serde_json::to_string(&old).unwrap());
        sim_core::data::migrate_save_items(&mut old, &data);
        unsafe {
            let loaded = roi_load_json(raw.as_ptr(), gd.as_ptr());
            assert!(!loaded.is_null());
            assert_eq!(take(roi_state_hash(loaded)), sim_core::hash::state_hash(&old));
            roi_tick(loaded, 200);
            let bundle: sim_core::replay::SaveBundle = serde_json::from_str(&take(roi_export_log(loaded))).unwrap();
            assert!(bundle.verify(&data), "migrated skills must replay from the server-trusted checkpoint");
            roi_free(loaded);
        }
        for e in old.grid.entities.iter().filter(|e| e.team == sim_core::grid::Team::Player) {
            assert_eq!(e.stats.level, 12);
            assert_eq!(e.skills.len(), 4);
        }
    }

    /// Bad input must return null / 0, never unwind — a panic across `extern "C"` is UB.
    #[test]
    fn null_and_garbage_are_refused_not_panics() {
        unsafe {
            assert!(roi_new(1, std::ptr::null(), std::ptr::null()).is_null());
            let junk = cstr("{ not json");
            let zone = cstr("zone_b1_1");
            assert!(roi_new(1, junk.as_ptr(), zone.as_ptr()).is_null());
            assert!(roi_load_json(junk.as_ptr(), junk.as_ptr()).is_null());
            roi_tick(std::ptr::null_mut(), 10);
            assert_eq!(roi_apply_input(std::ptr::null_mut(), junk.as_ptr()), 0);
            assert!(roi_save_json(std::ptr::null_mut()).is_null());
            assert!(roi_export_log(std::ptr::null_mut()).is_null());
            assert_eq!(roi_replay_span(std::ptr::null_mut()), -1);
            let v = take(roi_view_json(std::ptr::null_mut()));
            assert_eq!(v, "{}");
            roi_commit_anchor(std::ptr::null_mut());
            roi_string_free(std::ptr::null_mut());
            roi_free(std::ptr::null_mut());
        }
    }
}
