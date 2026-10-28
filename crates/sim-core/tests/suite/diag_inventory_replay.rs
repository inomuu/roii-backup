//! DIAGNOSTIC: reproduce the online char's resume `hash-mismatch` that the user observed
//! WHILE looking at the bag + changing equipment + selling items. Two independent probes:
//!
//! 1. `serde_stash_inventory_roundtrip` — the compiler flags `#[serde(alias="stash")]` on
//!    `inventory` as colliding with the real `stash` field (unreachable_patterns). If serde
//!    routes a save's "stash" key to `inventory`, a warehouse's contents are LOST / misrouted
//!    on load (save roundtrip is not identity). This asserts the roundtrip preserves `stash`.
//!
//! 2. `replay_matches_live_for_inventory_actions` — mirror the BRIDGE exactly: the client
//!    issues Sell / Deposit / Withdraw via `apply_input` (data = None) but the server replay
//!    runs every event via `apply_input_with(Some(data))`. If any of those handlers behaves
//!    differently with None vs Some, `live != replay(session_start, log)` → the exact
//!    `hash-mismatch` a save would get. Equip goes through `apply_input_json` (Some) so it's
//!    exercised that way.

use sim_core::data::{self, GameData};
use sim_core::events::{apply_input, apply_input_with, InputEvent};
use sim_core::hash::state_hash;
use sim_core::items::{Item, ItemKind, Rarity};
use sim_core::replay::{advance, replay};
use sim_core::state::GameState;

fn game_data() -> GameData {
    GameData::from_json_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../data/game_data.json"
    )))
    .expect("game_data.json must parse")
}

/// PROBE 1: a warehouse (`stash`) item must survive a serialize→deserialize roundtrip.
/// If `#[serde(alias="stash")]` on `inventory` steals the "stash" key, this fails.
#[test]
fn serde_stash_inventory_roundtrip() {
    let data = game_data();
    let mut state = data::new_game_in_zone(42, &data, "zone_b1_1").unwrap();

    // Distinct sentinels so we can tell which field each item landed in after a roundtrip.
    let mut bag_item = Item::new("scrap", ItemKind::Material, 0, 0, Rarity::Common);
    bag_item.quantity = 7;
    state.inventory.push(bag_item);

    let mut warehouse_item = Item::new("scrap", ItemKind::Material, 0, 0, Rarity::Common);
    warehouse_item.quantity = 99; // distinct qty = the warehouse sentinel
    state.stash.push(warehouse_item);

    let inv_len = state.inventory.len();
    let json = serde_json::to_string(&state).unwrap();
    let back: GameState = serde_json::from_str(&json).unwrap();

    assert_eq!(
        back.stash.len(),
        1,
        "the warehouse (stash) item was LOST on roundtrip (serde alias collision routed \"stash\" to inventory)"
    );
    assert_eq!(back.stash[0].quantity, 99, "warehouse item identity preserved");
    assert_eq!(
        back.inventory.len(),
        inv_len,
        "inventory must not absorb the warehouse's \"stash\" key"
    );
    assert_eq!(
        state_hash(&state),
        state_hash(&back),
        "save roundtrip must be identity (load(save(s)) == s)"
    );
}

/// PROBE 2: replay (server-side, Some(data)) must reproduce a live session that issued
/// inventory actions the way the BRIDGE does (Sell/Deposit/Withdraw via apply_input = None).
#[test]
fn replay_matches_live_for_inventory_actions() {
    let data = game_data();
    let mut live = data::new_game_in_zone(42, &data, "zone_b1_1").unwrap();

    // Give the bag some sellable gear + a stackable + a card-able nothing, like a resumed save.
    for atk in [4, 7, 11] {
        let mut g = Item::new("sword_short", ItemKind::Weapon, atk, 0, Rarity::Common);
        g.quantity = 1;
        live.inventory.push(g);
    }
    let mut mat = Item::new("scrap", ItemKind::Material, 0, 0, Rarity::Common);
    mat.quantity = 20;
    live.inventory.push(mat);

    // Resume: this loaded state IS the session start (mirror load_json), then record.
    live.record_inputs = true;
    let session_start = live.clone();
    let lead = live.grid.entities[0].id;

    // Mirror the BRIDGE call sites exactly (None where the bridge uses apply_input):
    advance(&mut live, &data, 15);
    // Equip goes through apply_input_json → apply_input_with(Some).
    apply_input_with(&mut live, Some(&data), InputEvent::UnequipItem { entity_id: lead, equipped_slot: 0 });
    advance(&mut live, &data, 8);
    apply_input_with(&mut live, Some(&data), InputEvent::EquipItem { entity_id: lead, inventory_idx: 0 });
    advance(&mut live, &data, 8);
    // Sell / Deposit / Withdraw go through apply_input (None) in bridge_items.rs.
    apply_input(&mut live, InputEvent::SellInventoryItem { inv_idx: 0 });
    advance(&mut live, &data, 8);
    apply_input(&mut live, InputEvent::DepositToStash { inv_idx: 0 });
    advance(&mut live, &data, 8);
    apply_input(&mut live, InputEvent::WithdrawFromStash { stash_idx: 0 });
    advance(&mut live, &data, 8);

    assert!(!live.input_log.is_empty(), "the session recorded its inputs");

    let replayed = replay(&session_start, &data, &live.input_log, live.tick);
    assert_eq!(
        state_hash(&replayed),
        state_hash(&live),
        "server replay (Some(data)) diverged from the live bridge session (None) — this IS the resume hash-mismatch"
    );
}
