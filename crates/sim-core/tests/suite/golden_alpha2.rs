//! Golden test for ALPHA-2 scenario.
//!
//! Behavior pinned: seed=42, 16×8 grid, one Entity walking from (0,0)
//! to (12,6) at speed_milli=400, ticked 1000 times → must finish at
//! (12,6) with the same hash forever.
//!
//! Hash bumped in ALPHA-3 (sim-core 0.0.1-alpha) because the Entity
//! struct grew combat fields. ALPHA-2 walking *behavior* is unchanged:
//! with no living enemies in the arena the entity still walks toward
//! its `target` cell exactly as it did in 0.0.0.
//!
//! Re-bumped in BETA-4 (sim-core 0.0.10-alpha): Entity gained
//! `proj_travel_ticks: u32`. Default is 0 (melee) so this scenario's
//! behaviour does NOT change — the entity has no enemies anyway and
//! never enters the attack branch. Pure JSON shape change.
//!
//! Re-bumped in BETA-5 (sim-core 0.0.11-alpha): Entity gained
//! `stats: Stats` + `xp_reward: u32`. Defaults (all stats = 1, xp =
//! 0) make no behavioral difference here — the entity has nothing to
//! attack so STR's +1 baseline is never read. JSON shape only.

use sim_core::grid::{Cell, Entity};
use sim_core::hash::state_hash;
use sim_core::state::GameState;
use sim_core::tick::tick;

// Re-bumped in 0.0.12 (BETA-5 Phase 1.5 per-class roster): GameState
// gained `current_unit_def` + `class_stats`. Defaults are None + empty
// — this scenario doesn't run new_game_in_zone so neither is touched.
// Pure JSON shape change.
// 0.0.14: +Entity.atk_type + is_ranged (Damage/magic refactor).
// Shape-only field add (defaults Physical/false) — behavior identical.
// 0.0.15: +Stats job_level/job_xp/skill_points + Entity/MonsterTemplate
// job_xp_reward (RO Job Level). Shape add; gear-less/job_xp=0 scenarios
// behave identically.
// 0.0.16: +Entity sp + skills (Bash active, HP Recovery passive).
// Shape add; skill-less/SP-less scenarios behave identically (alpha6
// now auto-casts Bash + HP-regens, so its run legitimately changes).
// 0.0.33: +Entity.base_aspd (iRO Renewal ASPD model). Default 0 for
// this lone, enemy-less walker — it never enters the attack branch, so
// ASPD is never read. Pure JSON shape change; walk behavior identical.
// 0.0.34: +Entity {endure_soft_def, taunted_by, taunt_ticks} + Item.weapon_type
// (Endure/Taunt + weapon-mastery scaffolding). Every scenario here is
// skill-less / weapon_type None, so behavior is identical — pure JSON shape.
const ALPHA2_EXPECTED_HASH: &str =
    "c5d766939c0dde4db60114080efc310be6f1e1b6ddac8a08bef70c5c528abc74";

#[test]
fn alpha2_one_entity_1000_ticks() {
    let mut state = GameState::new(42, 16, 8);
    state.grid.spawn(
        Entity::idle(1, Cell::new(0, 0))
            .with_walk(Cell::new(12, 6), 400),
    );

    tick(&mut state, 1000);

    assert_eq!(state.tick, 1000);
    assert_eq!(state.grid.entities[0].pos, Cell::new(12, 6));

    let h = state_hash(&state);
    eprintln!("ALPHA-2 (re-pinned for 0.0.1) state_hash = {h}");
    assert_eq!(
        h, ALPHA2_EXPECTED_HASH,
        "ALPHA-2 golden hash drifted — see GDD §7.2"
    );
}
