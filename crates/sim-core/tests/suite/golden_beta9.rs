//! Golden test for the skill spine — an active cast over the basic swing.
//!
//! Scenario: a Seraphine-kit attacker (Steel Break active + Bulwark passive) starts at
//! half HP and beats on an invincible 0-ATK dummy.
//!   - Steel Break auto-casts over the basic swing whenever its cooldown allows,
//!     so the dummy loses HP faster than a basic-only attacker would.
//!   - No regen line anywhere: the 5-second regen window passes and heals nothing,
//!     so a damaged unit stays damaged.
//!
//! Pins the integrated active-cast + regen-window behaviour. (Pure formula checks
//! for the damage scaling live in combat.rs unit tests.) 0.0.445: re-authored on
//! the unit kit when the class skills (Bash / HP Recovery) were deleted.

use sim_core::grid::{Cell, Element, Entity, Team};
use sim_core::hash::state_hash;
use sim_core::skills::SkillState;
use sim_core::state::GameState;
use sim_core::tick::tick;

// 0.0.33: +Entity.base_aspd (iRO Renewal ASPD model). Entities are
// hand-built via with_combat → base_aspd = 0 → flat-atk_speed
// fallback; Bash + HP-recovery behavior is unchanged. Pure JSON shape
// change (new field, default 0).
// 0.0.34: +Entity {endure_soft_def, taunted_by, taunt_ticks} + Item.weapon_type
// (Endure/Taunt + weapon-mastery scaffolding). Every scenario here is
// skill-less / weapon_type None, so behavior is identical — pure JSON shape.
// 0.0.44: passive HP regen moved to a 5-second interval (was 1s), amount
// scaled x5 so avg HP/s is unchanged. Ticks bumped 30->60 to cross one regen
// boundary; HP/state-hash shift. Re-blessed.
// 0.0.52b: SP regen moved to the 5-second interval (was 1s, amount x5) to match
// HP — bigger, less jittery bounces. Bash cast timing / SP shift the hash. Re-blessed.
// 0.0.53: spam skills equalised to SP 14 (Bash 12->14) so no cheaper skill
// dominates; small CDs rotate them. Bash SP/timing shift the hash. Re-blessed.
// 0.0.54: CLEAN skill model — active skills are cooldown-only (no SP cost);
// SP is reservation-only. Bash now free + cd 20; SP/timing shift the hash. Re-blessed.
// 0.0.55: +SkillState.last_fired (auto-cast rotation tie-break). Bash records its
// fire tick → serialized state shifts. Re-blessed.
const BETA9_EXPECTED_HASH: &str =
    "a1a3140809ebb7b16b15bf5e1a35cdef3bdd57edf4d490d336417feefe0028ce";

const DUMMY_HP: i32 = 100_000;
const START_HP: i32 = 100;

#[test]
fn beta9_active_cast_over_the_basic() {
    let mut state = GameState::new(42, 16, 8);

    let mut player = Entity::idle(1, Cell::new(0, 0))
        .with_team(Team::Player)
        .with_walk(Cell::new(0, 0), 0)
        .with_combat(/*hp*/ 200, /*atk*/ 50, /*def*/ 5,
                     /*atk_speed_milli*/ 500, Element::Fire);
    player.atk_range = 1;
    player.skills = vec![SkillState::new("steel_break", 1), SkillState::new("bulwark", 1)];
    // Loadout: skills only fire when slotted now (empty = none). Slot Steel Break so it
    // still auto-casts here; Bulwark is passive (always applies, no slot).
    player.active_skills = vec!["steel_break".to_string()];
    player.hp = START_HP; // damaged, so HP Recovery has room to heal
    state.grid.spawn(player);

    let mut dummy = Entity::idle(2, Cell::new(1, 0))
        .with_team(Team::Monster)
        .with_walk(Cell::new(1, 0), 0)
        .with_combat(/*hp*/ DUMMY_HP, /*atk*/ 0, /*def*/ 0,
                     /*atk_speed_milli*/ 0, Element::Earth);
    dummy.atk_range = 1;
    state.grid.spawn(dummy);

    // 60 ticks (6s) — past one 5-second regen boundary (HP_REGEN_INTERVAL_TICKS=50).
    tick(&mut state, 60);

    let p = &state.grid.entities[0];
    let d = &state.grid.entities[1];
    // Steel Break (+ basic) chewed the dummy.
    assert!(d.hp < DUMMY_HP, "dummy should have taken damage");
    assert!(d.alive, "dummy has 100k HP — still standing");
    // Nothing on this unit regenerates — the window passed and healed nothing.
    assert_eq!(p.hp, START_HP, "no regen line, no heal");

    let h = state_hash(&state);
    eprintln!("BETA-9 state_hash = {h}  (dummy_hp={}, player_hp={})", d.hp, p.hp);
    assert_eq!(h, BETA9_EXPECTED_HASH, "BETA-9 golden hash drifted — see GDD §7.2");
}
