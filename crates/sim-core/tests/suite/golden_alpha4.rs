//! Golden test for ALPHA-4 — drops + equip end to end.
//!
//! Scenario:
//!   1. Player at (0, 0) with no equipment, 20 atk, Fire.
//!   2. Three stationary Earth-element slimes at (3,0), (6,0), (9,0)
//!      with a weighted drop table favoring "scrap" (90) over
//!      "sword_short +10 atk" (10).
//!   3. Run 500 ticks. Player kills all three.
//!   4. Equip the first dropped item via apply_input.
//!   5. Pin the resulting state hash.
//!
//! What this proves end-to-end:
//!   - Combat + death triggers drops.
//!   - Drops use the sim PRNG and are reproducible from seed.
//!   - InputEvent equip path actually moves items + survives serialize.
//!   - effective_atk increase shows up in hash (item left on entity).

use sim_core::events::{apply_input, InputEvent};
use sim_core::grid::{Cell, Element, Entity, Team};
use sim_core::hash::state_hash;
use sim_core::items::{DropEntry, Item, ItemKind, Rarity};
use sim_core::state::GameState;
use sim_core::tick::tick;

// Re-bumped in BETA-4 (sim-core 0.0.10-alpha): Entity gained
// `proj_travel_ticks: u32`. Default 0 = melee — combat resolves
// instantly as before. Pure JSON shape change.
//
// Re-bumped in BETA-5 (sim-core 0.0.11-alpha): Stats baseline (+1
// STR) shifts every damage roll by ±1, which cascades through the
// 500-tick scenario (different last-hit, different drop RNG draw
// order on later kills). Hash bumped to capture the new sequence.
// Re-bumped in 0.0.12 (per-class roster fields). JSON shape only.
// Re-bumped in 0.0.14 (surround targeting). Per-monster `target`
// field shifts to a neighbor cell of the player.
// 0.0.14: +Entity.atk_type + is_ranged (Damage/magic refactor).
// Shape-only field add (defaults Physical/false) — behavior identical.
// 0.0.15: +Stats job_level/job_xp/skill_points + Entity/MonsterTemplate
// job_xp_reward (RO Job Level). Shape add; gear-less/job_xp=0 scenarios
// behave identically.
// 0.0.16: +Entity sp + skills (Bash active, HP Recovery passive).
// Shape add; skill-less/SP-less scenarios behave identically (alpha6
// now auto-casts Bash + HP-regens, so its run legitimately changes).
// 0.0.33: +Entity.base_aspd (iRO Renewal ASPD model). The player is
// hand-built via with_combat → base_aspd = 0 → flat-atk_speed_milli
// fallback (500), and the equipped sword_short is an in-test Item::new
// (not the game_data row whose aspd_base changed). Kill/equip behavior is
// identical; the hash only moves for the new field (default 0). Shape only.
// 0.0.34: +Entity {endure_soft_def, taunted_by, taunt_ticks} + Item.weapon_type
// (Endure/Taunt + weapon-mastery scaffolding). Every scenario here is
// skill-less / weapon_type None, so behavior is identical — pure JSON shape.
// 0.0.42: +Item.equip_slot (RO 10-slot paper-doll). Pure shape add — this
// scenario's gear has equip_slot=None → slot() falls back to kind, identical
// behavior; only the serialized hash shifts. Re-blessed.
// 0.0.45: +Item.equip_classes (weapon class-restriction override). Pure shape add
// for this scenario (empty vec) — only the serialized hash shifts. Re-blessed.
const ALPHA4_EXPECTED_HASH: &str =
    "1290274b036f964032b38aca78710fa7d548a1e3adc895cd7df99398ec7f9342";

fn slime_drops() -> Vec<DropEntry> {
    vec![
        DropEntry::new(
            Item::new("scrap", ItemKind::Material, 0, 0, Rarity::Common),
            90,
        ),
        DropEntry::new(
            Item::new("sword_short", ItemKind::Weapon, 10, 0, Rarity::Uncommon),
            10,
        ),
    ]
}

#[test]
fn alpha4_kill_three_then_equip() {
    let mut state = GameState::new(42, 16, 8);

    state.grid.spawn(
        Entity::idle(1, Cell::new(0, 0))
            .with_team(Team::Player)
            .with_walk(Cell::new(0, 0), 1000)
            .with_combat(/*hp*/ 200, /*atk*/ 20, /*def*/ 5,
                         /*atk_speed_milli*/ 500, Element::Fire),
    );

    for (i, x) in [3, 6, 9].iter().enumerate() {
        state.grid.spawn(
            Entity::idle(10 + i as u32, Cell::new(*x, 0))
                .with_team(Team::Monster)
                .with_walk(Cell::new(*x, 0), 0)
                .with_combat(/*hp*/ 30, /*atk*/ 8, /*def*/ 2,
                             /*atk_speed_milli*/ 200, Element::Earth)
                .with_drops(slime_drops()),
        );
    }

    tick(&mut state, 500);

    // All three slimes should be dead by tick 500.
    for i in 1..=3 {
        assert!(!state.grid.entities[i].alive, "slime {i} should be dead");
    }
    // Player should be alive.
    assert!(state.grid.entities[0].alive);
    // 0.0.38: drops now land in the instance ESCROW, not the stash. Bank it
    // (simulating an instance clear) so this equip-from-stash scenario holds.
    sim_core::drops::grant_escrow_to_stash(&mut state);
    // Three deaths, and what they drop is a PRNG outcome — `roll_drop` always takes
    // its draw, and the inert `nothing` item is one of the results. This used to pin
    // "3 items", which was really pinning one particular draw sequence; removing the
    // accuracy roll (0.0.333) moved every stream and with it this number. What the
    // scenario is about is the EQUIP-from-stash flow below, so it asserts the stash
    // has something to equip and leaves the count to the hash.
    let total_qty: u32 = state.inventory
        .iter()
        .map(|i| i.quantity)
        .sum();
    assert!(total_qty > 0, "three deaths must leave something in the stash");
    let rows_before = state.inventory.len();
    // Player has nothing equipped yet.
    assert_eq!(state.grid.entities[0].equipped.len(), 0);

    // Equip the item at inventory_idx=0 (indexes into the shared stash).
    let first_id = state.inventory[0].def_id.clone();
    apply_input(&mut state, InputEvent::EquipItem { entity_id: 1, inventory_idx: 0 });
    assert_eq!(state.grid.entities[0].equipped.len(), 1);
    assert_eq!(state.grid.entities[0].equipped[0].def_id, first_id);
    // Equipping removes ONE row (the whole stack if it was stacked).
    assert_eq!(state.inventory.len(), rows_before - 1);

    let h = state_hash(&state);
    eprintln!("ALPHA-4 state_hash = {h}");
    assert_eq!(
        h, ALPHA4_EXPECTED_HASH,
        "ALPHA-4 golden hash drifted — see GDD §7.2"
    );
}
