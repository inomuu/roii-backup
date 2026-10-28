//! REGRESSION GUARD for the save-verify replay hole: transient COMBAT fields on `Entity`
//! (`shields`, `act_locked_until`, `bloodthirst_stacks`, `down_ticks`, `kb_shot_hit`, …) and
//! `Projectile.attacker_id` used to be `#[serde(skip)]`. The client kept them live, but an autosave
//! ANCHOR captured mid-combat serialized WITHOUT them → the server's replay from that anchor diverged
//! → intermittent `hash-mismatch` rejects (whenever such a field was non-default at the save tick).
//! Now they're serialized. This asserts a serialize round-trip at a mid-combat tick reproduces the
//! SAME forward outcome — i.e. the anchor carries everything the next ticks depend on.

use sim_core::data;
use sim_core::grid::{Cell, Element, Entity, Team};
use sim_core::hash::state_hash;
use sim_core::replay::advance;
use sim_core::state::GameState;

fn game_data() -> data::GameData {
    data::GameData::from_json_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../data/game_data.json"
    )))
    .expect("game_data.json must parse")
}

#[test]
fn anchor_roundtrip_preserves_combat_transients() {
    let d = game_data();
    // Band-4 combat (bomb monsters + knockback) sets the transient fields naturally.
    let mut state = data::new_game_in_zone(42, &d, "zone_b4_1").unwrap();
    let mut killer = Entity::idle(90_002, Cell::new(2, 7))
        .with_team(Team::Player)
        .with_walk(Cell::new(2, 7), 200)
        .with_combat(5_000_000, 5_000, 500, 300, Element::Light);
    killer.atk_range = 3;
    state.grid.spawn(killer);
    advance(&mut state, &d, 400);

    // Explicitly set combat-transient fields on the (attacking) killer so they are guaranteed
    // non-default AND consequential: act_locked_until pauses its attacks for 60 ticks, which
    // changes the next 150 ticks — so if the anchor drops it, the outcomes diverge.
    let ki = state
        .grid
        .entities
        .iter()
        .position(|e| e.id == 90_002)
        .expect("killer present");
    state.grid.entities[ki].act_locked_until = state.tick + 60;
    state.grid.entities[ki].bloodthirst_stacks = 7;

    // LIVE (in-memory) vs ANCHOR round-trip (serialize → deserialize → advance): must match.
    let mut live = state.clone();
    advance(&mut live, &d, 150);

    let json = serde_json::to_string(&state).expect("serialize");
    let mut replayed: GameState = serde_json::from_str(&json).expect("deserialize");
    advance(&mut replayed, &d, 150);

    assert_eq!(
        state_hash(&live),
        state_hash(&replayed),
        "a serialize round-trip at a mid-combat tick must reproduce the same 150-tick outcome — \
         transient combat fields (act_locked_until / bloodthirst / shields / …) must survive the \
         anchor (this was the save-verify replay hole where they were #[serde(skip)])"
    );
}
