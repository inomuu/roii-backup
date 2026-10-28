//! Golden test for ALPHA-3 — combat vertical slice.
//!
//! Scenario: 1 Player vs 1 Monster, melee, Fire vs Earth (advantage).
//! Player should walk into range and kill the monster within a bounded
//! number of ticks. State hash is pinned.
//!
//! Why a single 1v1 first?
//!   - Easier to reason about than 2v1 — every event has one cause.
//!   - 1v1 covers all four code paths added in ALPHA-3: targeting,
//!     moving to engage, attacking, death. 2v1 only adds the targeting
//!     tie-break path, which `find_enemy_picks_nearest_then_lowest_id`
//!     already exercises as a unit test.

use sim_core::combat;
use sim_core::grid::{Cell, Element, Entity, Team};
use sim_core::hash::state_hash;
use sim_core::state::GameState;
use sim_core::tick::tick;

// Re-bumped in BETA-4 (sim-core 0.0.10-alpha): Entity gained
// `proj_travel_ticks: u32`. Both entities here are melee
// (proj_travel_ticks = 0 by default) — combat path unchanged, JSON
// shape only.
//
// Re-bumped in BETA-5 (sim-core 0.0.11-alpha): Entity gained
// `stats: Stats` + `xp_reward: u32`. Behavioral change in this
// scenario: player has STAT_MIN STR (= 1) which adds +1 to melee
// effective_atk → marginally higher damage / earlier kill / different
// number of ticks. Hash bumped to reflect the actual new outcome.
// Re-bumped in 0.0.12 (BETA-5 Phase 1.5): GameState gained
// `current_unit_def` + `class_stats`. This scenario constructs state
// directly (no zone load), so both stay at defaults. JSON shape only.
// Re-bumped in 0.0.14 (surround targeting): Monster's `target` field
// now points to a neighbor cell of the player rather than the player's
// own tile. Behavior at the kill site is identical (in_combat still
// computed against the enemy's actual position), but the state hash
// reads the per-entity target each tick.
// 0.0.14: +Entity.atk_type + is_ranged (Damage/magic refactor).
// Shape-only field add (defaults Physical/false) — behavior identical.
// 0.0.15: +Stats job_level/job_xp/skill_points + Entity/MonsterTemplate
// job_xp_reward (RO Job Level). Shape add; gear-less/job_xp=0 scenarios
// behave identically.
// 0.0.16: +Entity sp + skills (Bash active, HP Recovery passive).
// Shape add; skill-less/SP-less scenarios behave identically (alpha6
// now auto-casts Bash + HP-regens, so its run legitimately changes).
// 0.0.33: +Entity.base_aspd (iRO Renewal ASPD model). Both entities
// are hand-built via with_combat → base_aspd = 0, so they take the
// flat-atk_speed_milli fallback (500/300) exactly as before. Kill timing
// is unchanged (monster still dead / player alive by tick 200); the hash
// only moves because the new field (default 0) is serialized. Shape only.
// 0.0.34: +Entity {endure_soft_def, taunted_by, taunt_ticks} + Item.weapon_type
// (Endure/Taunt + weapon-mastery scaffolding). Every scenario here is
// skill-less / weapon_type None, so behavior is identical — pure JSON shape.
const ALPHA3_EXPECTED_HASH: &str =
    "48fe8d1d5e62c2289dcdad60d21f00f76f42fa81fc9d4a0e67831d8ecb14849a";

#[test]
fn alpha3_player_kills_monster() {
    let mut state = GameState::new(42, 16, 8);

    // Player at (0, 0): Fire, decent atk, slow swing.
    state.grid.spawn(
        Entity::idle(1, Cell::new(0, 0))
            .with_team(Team::Player)
            .with_walk(Cell::new(0, 0), 500)
            .with_combat(/*hp*/ 100, /*atk*/ 30, /*def*/ 5,
                         /*atk_speed_milli*/ 500, Element::Fire),
    );

    // Monster at (5, 2): Earth (weak to Fire), thin hp.
    state.grid.spawn(
        Entity::idle(2, Cell::new(5, 2))
            .with_team(Team::Monster)
            .with_walk(Cell::new(5, 2), 0)  // stationary monster
            .with_combat(/*hp*/ 40, /*atk*/ 15, /*def*/ 3,
                         /*atk_speed_milli*/ 300, Element::Earth),
    );

    // Sanity: pre-tick, both alive, monster targets player and vice versa.
    assert!(state.grid.entities[0].alive);
    assert!(state.grid.entities[1].alive);
    assert_eq!(combat::find_enemy_idx(&state.grid.entities, 0), Some(1));
    assert_eq!(combat::find_enemy_idx(&state.grid.entities, 1), Some(0));

    // Run for 200 ticks (= 20 sim-seconds @ 100ms/tick). Player should
    // close ~5 cells then chip down the monster. Each Fire-vs-Earth
    // swing deals (30-3)*1.5 = 40 damage — one swing kills.
    tick(&mut state, 200);

    // Monster must be dead; player still alive.
    assert!(!state.grid.entities[1].alive, "monster should be dead by tick 200");
    assert!(state.grid.entities[0].alive,  "player should not have died");

    let h = state_hash(&state);
    eprintln!("ALPHA-3 state_hash = {h}");
    assert_eq!(
        h, ALPHA3_EXPECTED_HASH,
        "ALPHA-3 golden hash drifted — see GDD §7.2"
    );
}
