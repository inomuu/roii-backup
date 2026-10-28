//! Every save this machine has ever written must still load.
//!
//! WHY THIS EXISTS, and why nothing else covers it. `GameState` and `Item` are serde structs whose
//! fields are not all `#[serde(default)]` — `def_id`, `kind`, `atk_bonus`, `def_bonus` and
//! `refine_level` have none, and the enums (`ItemKind`, `Rarity`, `WeaponType`) reject a variant
//! name they do not know. So renaming a field, retyping one, or renaming an enum variant makes
//! every previously-written save fail to deserialize.
//!
//! Every existing guard round-trips WITHIN one version — `a_round_trip_loses_only_allowlisted_fields`
//! and `contract_keys_are_stable` both write and read with the same code, so they pass by
//! construction. The failure mode is OLD JSON against a NEW struct, and nothing watched it. This is
//! CLAUDE.md's own gap 6: "migrate_save_items is tested against synthetic states, never against an
//! actual save file from an older VERSION."
//!
//! Not a market problem, though that is where it surfaced (an `item_listings` row that no longer
//! deserializes). `Item` lives in `GameState.inventory`, which lives in every row of
//! `saves.state_json`. If this breaks, every player's save breaks with it.
//!
//! REAL SAVES, NOT FIXTURES. It reads the live `user://` directory instead of committed files, so
//! the corpus is saves written by code from a month and ~130 VERSION bumps ago — and it grows for
//! free every time the game is played. A synthetic fixture only ever covers the shapes whoever
//! wrote it remembered to build, which is the weakness CLAUDE.md is complaining about above.
//! READ ONLY: this opens files and never writes, renames or deletes one.
//!
//! IT FAILS, RATHER THAN SKIPS, WHEN THERE IS NOTHING TO READ. The `pg_` suite skips when its
//! database is absent and reports ok, which is how "green because UNTESTED" happens — a machine
//! with no old saves has not proven anything about old saves, and should say so.
//!
//! WITH ONE BOUNDARY, added 2026-08-18 after this test blocked a deploy. The Dockerfile runs
//! `cargo test --release -p sim-core --tests` inside the build image, where there is no Godot,
//! no `user://`, and never will be. That is not "unproven", it is INAPPLICABLE — the distinction
//! being: does this machine have a Godot installation at all?
//!
//!   - Godot user-data root exists, our game's directory does not → **FAIL**. Someone plays here
//!     and the corpus is missing, unreadable, or the directory was RENAMED (see the
//!     `godot-user-dir-rename-hazard` note — a rename moves `user://` and looks exactly like
//!     "no saves"). That is the case worth shouting about and it keeps failing.
//!   - No Godot user-data root anywhere → **skip, loudly**. A build container or a CI runner.
//!     Failing there does not protect a save; it just stops deploys.
//!   - `ROI_SAVE_DIR` set → **FAIL if it yields nothing**. Setting it is a claim that saves are
//!     there; an empty answer means the claim is wrong.
//!
//! This is not "point it somewhere emptier": the strict behaviour is unchanged on every machine
//! that could hold a save. What changed is that a machine which cannot hold one says so instead
//! of failing.
//!
//! WHEN THIS GOES RED you changed the serialized shape. Two honest answers:
//!   1. give the renamed field `#[serde(alias = "old_name")]`, or the new one `#[serde(default)]`,
//!      so old saves keep loading — almost always the right one; or
//!   2. decide the break is intended and write the migration that reads the old shape.
//! Pointing the test somewhere emptier is not one of them.

use sim_core::data::GameData;
use sim_core::state::GameState;
use std::path::PathBuf;

fn game_data() -> GameData {
    GameData::from_json_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../data/game_data.json"
    )))
    .expect("game_data.json must parse")
}

/// Where Godot puts `user://` for this game. `ROI_SAVE_DIR` overrides (only READ here — a test in
/// this suite may not SET an env var, the whole binary shares one process).
fn save_dir() -> Option<PathBuf> {
    if let Ok(p) = std::env::var("ROI_SAVE_DIR") {
        return Some(PathBuf::from(p));
    }
    let home = std::env::var("HOME").ok()?;
    ["Library/Application Support/Godot/app_userdata/reign_of_idlers",
     ".local/share/godot/app_userdata/reign_of_idlers"]
        .iter()
        .map(|rel| PathBuf::from(&home).join(rel))
        .find(|p| p.is_dir())
}

/// Could this machine hold a save at all? True when Godot's user-data root exists — the PARENT of
/// every game's directory, not ours. It is what separates "played here, corpus missing" (a real
/// problem) from "no Godot" (a build image).
fn machine_can_hold_saves() -> bool {
    let Ok(home) = std::env::var("HOME") else { return false };
    ["Library/Application Support/Godot", ".local/share/godot"]
        .iter()
        .any(|rel| PathBuf::from(&home).join(rel).is_dir())
}

/// Deserializing must not merely succeed — it must produce the state that was saved. A struct that
/// gained `#[serde(default)]` everywhere would pass a bare `from_str` while silently zeroing the
/// file, which is the failure this is meant to catch, not permit.
///
/// Then it runs the REAL load path. `migrate_save_items` is what `godot-ext/src/lib.rs:279` calls
/// on every `load_json` and what verify-server calls on resume; deserializing correctly and then
/// being flattened by the migration is indistinguishable, to a player, from not loading at all.
///
/// WHAT THIS ADDS, precisely — because it is less than it looks. Per-instance preservation is
/// already covered, and better, by `migrate_save_items_preserves_every_per_instance_field` in
/// data/mod.rs, which builds an item carrying all ten earned fields. What is NOT covered anywhere
/// else is running the migration over a REAL old save at all: real def_ids, real stacks, real
/// renamed-ore remaps, shapes nobody constructed on purpose.
///
/// MEASURED against the current corpus (2026-08-15), not assumed: across all five saves there are
/// 15 items with `quantity > 1` and ZERO with refine_level, refine_attempts or bound set. So the
/// only per-instance field this actually exercises today is `quantity` — the rest ride along and
/// become live the moment a refined or bound item is saved. Sabotage confirmed it: zeroing
/// refine_level in the migration still passes here, and is caught by the synthetic test instead.
fn assert_loads(name: &str, json: &str, data: &GameData) {
    let s: GameState = serde_json::from_str(json).unwrap_or_else(|e| {
        panic!(
            "{name}: a save that was valid when written no longer deserializes ({e}).\n\
             The serialized shape of GameState/Item changed. Add #[serde(alias)] or \
             #[serde(default)] so old saves keep loading, or write the migration."
        )
    });
    let raw: serde_json::Value = serde_json::from_str(json).expect("valid JSON");
    assert_eq!(
        s.tick,
        raw.get("tick").and_then(|v| v.as_u64()).unwrap_or(0),
        "{name}: tick did not survive the load"
    );
    if let Some(n) = raw.get("inventory").and_then(|v| v.as_array()).map(|a| a.len()) {
        assert_eq!(s.inventory.len(), n,
            "{name}: {n} inventory items went in, {} came out — a field lost its meaning rather \
             than erroring", s.inventory.len());
        for it in &s.inventory {
            assert!(!it.def_id.is_empty(),
                "{name}: an item loaded with an EMPTY def_id — the field was renamed and serde \
                 defaulted it instead of failing");
        }
    }

    // The real load path, against a real old save — gap 6's actual subject. A def the CURRENT
    // catalog keeps in the materials ledger (Sigil, Star Stones… `ItemDef::ledger`) is the one
    // thing migration may take OUT of the bag — and every unit of it must land in the ledger.
    let is_ledger = |id: &str| data.item(id).map(|d| d.ledger).unwrap_or(false);
    // A RETIRED def (taken out of the game, `data::RETIRED_ITEM_IDS`) leaves on purpose.
    let is_retired = |id: &str| sim_core::data::RETIRED_ITEM_IDS.contains(&id);
    // PER-INSTANCE state only — deliberately NOT the def_id. Migration is allowed to REMAP an id
    // (0.0.375 renamed four gear families when the slots were renamed, and `migrate_item` carries
    // the old→new table), and a test that pins the id would fail on a rename that worked. What
    // must never change is the EARNED state riding on the item, which is what this compares,
    // positionally — the inventory order is stable across migration.
    // GEAR's star is NOT earned state any more (0.0.382: fixed-stat gear has no ★, and the
    // migration clears a legacy one on purpose) — a card's is (its level), so only that is pinned.
    let star_of = |i: &sim_core::items::Item| if matches!(i.kind, sim_core::items::ItemKind::Card) { i.star } else { 0 };
    let before: Vec<(u8, u8, u32, bool)> = s.inventory.iter()
        .filter(|i| !is_ledger(&i.def_id) && !is_retired(&i.def_id))
        .map(|i| (star_of(i), i.resonance_stars, i.quantity, i.bound))
        .collect();
    let before_ids: Vec<String> = s.inventory.iter()
        .filter(|i| !is_ledger(&i.def_id) && !is_retired(&i.def_id))
        .map(|i| i.def_id.clone())
        .collect();
    let mut migrated = s.clone();
    sim_core::data::migrate_save_items(&mut migrated, data);
    assert_eq!(migrated.inventory.len(), before.len(),
        "{name}: migrate_save_items changed the inventory SIZE — it must refresh defs, not add or \
         drop items (only a ledger def may leave the bag, into the ledger)");
    for it in s.inventory.iter().filter(|i| is_ledger(&i.def_id)) {
        assert!(migrated.material_count(&it.def_id) >= it.quantity,
            "{name}: {} ×{} left the bag but did not reach the ledger", it.def_id, it.quantity);
    }
    let after: Vec<(u8, u8, u32, bool)> = migrated.inventory.iter()
        .map(|i| (star_of(i), i.resonance_stars, i.quantity, i.bound))
        .collect();
    let after_ids: Vec<String> = migrated.inventory.iter().map(|i| i.def_id.clone()).collect();
    // Items whose def was RETIRED (ore, junk, rune stones) stay in the bag as inert entries —
    // that is fine and is not what this checks. What must hold is that every id the REMAP table
    // produces exists; see `the_remap_table_points_at_real_defs` for that, pinned once rather
    // than per save.
    assert_eq!(after, before,
        "{name}: migrate_save_items did not preserve per-instance state (ids went \
         {before_ids:?} -> {after_ids:?}). These fields are EARNED — \
         stars, Resonance, stack size, bound — and rebuilding an item from its def \
         wipes them. That regression shipped once (0.0.222) and cost real player progress.");
}

#[test]
fn every_real_save_on_this_machine_still_loads() {
    let explicit = std::env::var("ROI_SAVE_DIR").is_ok();
    let Some(dir) = save_dir() else {
        // No Godot on this machine at all → the test is inapplicable, not unproven. Say it loudly
        // so a green run is never read as "old saves were checked". See the module header.
        if !explicit && !machine_can_hold_saves() {
            eprintln!(
                "[save_shape] SKIPPED — no Godot user-data root on this machine, so it holds no \
                 saves to check (a build image or CI runner). This is the ONLY case that skips; a \
                 machine with Godot and no corpus still fails."
            );
            return;
        }
        panic!(
            "no Godot user:// directory found, but this machine HAS a Godot user-data root — so \
             saves either were never written, are unreadable, or the game's directory was RENAMED \
             (which moves user:// and looks exactly like this). This test proves old saves still \
             load; with nothing to read it proves nothing, so it fails instead of reporting ok. \
             Point ROI_SAVE_DIR at a directory of saves."
        )
    };
    let data = game_data();
    let mut checked = Vec::new();
    for e in std::fs::read_dir(&dir).unwrap_or_else(|e| panic!("{}: {e}", dir.display())) {
        let path = e.expect("readable dir entry").path();
        let name = path.file_name().unwrap().to_string_lossy().to_string();
        // `save_N.json` only. Two exclusions, both earned:
        //
        // `save_N.UNREADABLE-<t>.json` are preserved copies of saves the sim ALREADY rejected, so
        // requiring them to load would fail on exactly the files that exist because they did not.
        //
        // `savegame.json` predates 0.0.168, when the party bag was renamed `stash` -> `inventory`
        // and the name `stash` was REUSED for the deposit warehouse. It cannot be loaded by adding
        // `alias = "stash"`, and this is not a guess — state.rs says the alias was tried and made
        // things worse: "serde saw both keys and errored `duplicate field inventory`, so any save
        // with a non-empty warehouse became UNLOADABLE (0.0.169-0.0.193)". Reading it needs a real
        // versioned migration, and the file is a crest rather than a slot anyone plays.
        //
        // (state.rs also says pre-0.0.168 saves are "long gone". One is still sitting in this
        // directory — which is the sort of thing this test exists to notice.)
        let is_save = name.starts_with("save_")
            && name.ends_with(".json")
            && !name.contains("UNREADABLE");
        if !is_save {
            continue;
        }
        let json = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{name}: {e}"));
        assert_loads(&name, &json, &data);
        checked.push(name);
    }
    assert!(
        !checked.is_empty(),
        "{} has no saves. Nothing was verified, and a test that cannot fail is not a test — play \
         the game once, or point ROI_SAVE_DIR somewhere with saves.",
        dir.display()
    );
    println!("  old-save compatibility: {} real saves still load — {checked:?}", checked.len());
}

/// A RETIRED orb variant must not take the whole save down with it. The real corpus on this
/// machine holds a `rune_stone` whose `orb` serialised `InsertRune`, a variant deleted in
/// 0.0.369 — which made every one of those saves unloadable until `Item.orb` learned to
/// deserialize leniently. This test is the synthetic half: it keeps the tolerance pinned once
/// the real save is gone (deleted, or played past), and it names the mechanism so the next
/// deletion pass knows the shape of the fix.
///
/// The JSON is taken from a REAL catalog item and then edited, not hand-written — a hand-written
/// Item is missing required fields and would fail for the wrong reason.
///
/// Sabotage check: removing `deserialize_with = "orb_lenient"` fails this.
#[test]
fn a_save_holding_a_retired_orb_still_loads() {
    // The stones left the catalog with the system (0.0.381), so the carrier is any real item
    // with an orb written onto it the way an old save has one.
    let data = game_data();
    let def = data.item("sword_s1").expect("a real def");
    let mut live = sim_core::data::instantiate_item(def);
    live.orb = Some(sim_core::items::OrbEffect::RerollAll);

    let mut raw = serde_json::to_value(&live).expect("an item serializes");
    assert_eq!(
        serde_json::from_value::<sim_core::items::Item>(raw.clone()).map(|i| i.orb.is_some()).ok(),
        Some(true),
        "a KNOWN (if retired) orb variant still round-trips through the lenient path"
    );

    raw["orb"] = serde_json::Value::String("InsertRune".to_string());
    let loaded: sim_core::items::Item = serde_json::from_value(raw.clone()).unwrap_or_else(|e| {
        panic!(
            "an item carrying a RETIRED orb effect no longer loads ({e}). Deleting an OrbEffect \
             variant must not brick saves that hold the consumable — keep `Item.orb` on \
             `orb_lenient` (unknown variant -> None), which is sound because orbs are def-driven \
             and an unknown one is inert."
        )
    });
    assert!(loaded.orb.is_none(), "a retired orb must load as None, not as some other effect");

    // Leniency is scoped to the orb field: a genuinely broken shape must still error.
    raw["quantity"] = serde_json::Value::String("not a number".to_string());
    assert!(
        serde_json::from_value::<sim_core::items::Item>(raw).is_err(),
        "a malformed item must still fail to load — this test must not license silent repair"
    );
}
