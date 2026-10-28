//! The overlay defs live in Rust (`items.rs`) and their ItemDefs live in `game_data.json`, so a
//! name exists in TWO places and nothing made them agree. That is the shape of bug this codebase
//! keeps finding late: the tooltip renders whatever the catalog says while the sim uses the def,
//! and they drift apart silently because both sides are individually valid.
//!
//! Binds them: every overlay in the table must have an ItemDef, and the two names must match.

use sim_core::data::GameData;

fn game_data() -> GameData {
    GameData::from_json_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../data/game_data.json"
    )))
    .expect("game_data.json must parse")
}

#[test]
fn every_overlay_has_an_item_def_with_a_matching_name() {
    let d = game_data();
    let mut checked = 0;

    for def_id in sim_core::items::all_regalia_def_ids() {
        let item = d
            .item(def_id)
            .unwrap_or_else(|| panic!("{def_id}: no ItemDef — it cannot be bought, granted or mailed"));
        let overlay = sim_core::items::regalia_def(def_id).expect("resolves");
        assert_eq!(
            item.name, overlay.name,
            "{def_id}: the shop would show {:?} while the sim uses {:?}",
            item.name, overlay.name
        );
        checked += 1;
    }
    for def_id in sim_core::items::all_exemplar_def_ids() {
        let item = d
            .item(def_id)
            .unwrap_or_else(|| panic!("{def_id}: no ItemDef — it cannot be bought, granted or mailed"));
        let overlay = sim_core::items::exemplar_def(def_id).expect("resolves");
        assert_eq!(item.name, overlay.name, "{def_id}: catalog name and def name disagree");
        checked += 1;
    }

    // 24 Regalia (10 3★ + 5 SR + 9 SSR; six stat-only SR retired 0.0.489) + 2 Dust + 20
    // Exemplars. A count, so DELETING a line fails here too rather than quietly shrinking what
    // the loop walks.
    assert_eq!(checked, 46, "expected every overlay in both tables to be covered");
}
