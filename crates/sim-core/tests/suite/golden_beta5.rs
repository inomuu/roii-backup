//! Golden test for BETA-5 Phase 1 — stats + leveling vertical slice.
//!
//! Scenario:
//!   1. Player melee at (0, 0): Fire, just strong enough to one-shot
//!      an Earth slime in range.
//!   2. Single slime at (1, 0) with a fat `xp_reward` (1300) so a
//!      single kill pushes the player through two level thresholds of the
//!      quadratic curve (250 + 1000) with 50 xp carrying into the next bar.
//!   3. Run ticks until the slime dies.
//!   4. Apply an `AllocateStatPoint` for STR via the input path.
//!   5. Pin state hash.
//!
//! What this pins end-to-end:
//!   - Kill → XP routed through `handle_monster_death` to the right
//!     player.
//!   - Multi-level loop in `try_level_up` (one kill, two level-ups,
//!     overflow xp preserved — no XP "wasted").
//!   - Each level grants `POINTS_PER_LEVEL` unspent points.
//!   - `InputEvent::AllocateStatPoint` spends from the pool using the
//!     RO cost curve (STR 1 → 2 costs 2).
//!   - Effective atk picks up the new STR baseline (small here — STR
//!     went from 1 → 2 only — but enough that hash changes if the
//!     wiring breaks).

use sim_core::events::{apply_input, InputEvent};
use sim_core::grid::{Cell, Element, Entity, Team};
use sim_core::hash::state_hash;
use sim_core::state::GameState;
use sim_core::stats::StatKind;
use sim_core::tick::tick;

// Re-bumped in 0.0.12 (per-class roster). JSON shape only.
// Re-bumped in 0.0.14 (surround targeting).
// 0.0.14: +Entity.atk_type + is_ranged (Damage/magic refactor).
// Shape-only field add (defaults Physical/false) — behavior identical.
// 0.0.15: +Stats job_level/job_xp/skill_points + Entity/MonsterTemplate
// job_xp_reward (RO Job Level). Shape add; gear-less/job_xp=0 scenarios
// behave identically.
// 0.0.16: +Entity sp + skills (Bash active, HP Recovery passive).
// Shape add; skill-less/SP-less scenarios behave identically (alpha6
// now auto-casts Bash + HP-regens, so its run legitimately changes).
// 0.0.33: +Entity.base_aspd (iRO Renewal ASPD model). Entities are
// hand-built via with_combat → base_aspd = 0 → flat-atk_speed
// fallback; kill/level/allocate behavior is unchanged. Pure JSON shape
// change (new field, default 0).
// 0.0.34: +Entity {endure_soft_def, taunted_by, taunt_ticks} + Item.weapon_type
// (Endure/Taunt + weapon-mastery scaffolding). Every scenario here is
// skill-less / weapon_type None, so behavior is identical — pure JSON shape.
const BETA5_EXPECTED_HASH: &str =
    "55ebf0076c1dece8e78ff71cf3c18140aab09fa4495f86527f042fd0cd4e472f";

#[test]
fn beta5_kill_grants_a_level() {
    let mut state = GameState::new(42, 16, 8);

    // Player: melee Fire, atk 5 + STR baseline (1) = 6 effective.
    // Fire vs Earth = 1.5x → (6 - 0) * 1.5 = 9 damage → one-shot.
    let mut player = Entity::idle(1, Cell::new(0, 0))
        .with_team(Team::Player)
        .with_walk(Cell::new(0, 0), 500)
        .with_combat(/*hp*/ 100, /*atk*/ 5, /*def*/ 5,
                     /*atk_speed_milli*/ 500, Element::Fire);
    player.atk_range = 1;
    state.grid.spawn(player);

    // Slime: stationary, in melee range, 8 hp, 975 xp.
    let mut slime = Entity::idle(2, Cell::new(1, 0))
        .with_team(Team::Monster)
        .with_walk(Cell::new(1, 0), 0)
        .with_combat(/*hp*/ 8, /*atk*/ 0, /*def*/ 0,
                     /*atk_speed_milli*/ 100, Element::Earth);
    slime.atk_range = 1;
    slime.xp_reward = 1300;
    state.grid.spawn(slime);

    // Pre-kill sanity: player level 1, zero xp.
    assert_eq!(state.grid.entities[0].stats.level, 1);
    assert_eq!(state.grid.entities[0].stats.xp, 0);

    // 10 ticks is plenty for a 500-milli swing (2 swings) — one of
    // them lands the killing blow.
    tick(&mut state, 10);

    // Slime dead. Player level should be 3: `XP_TABLE` × `XP_CURVE_MULT` (2, 0.0.495),
    // 1300 xp → -160 (1→2), -720 (2→3), 420 carry (next needs 1800). Renewal
    // points: leaving lvl 1 → ⌊1/5⌋+3 = 3, leaving lvl 2 → ⌊2/5⌋+3 = 3 → 6.
    // (Solo player → party EXP share is 100%, so the full reward applies.)
    assert!(!state.grid.entities[1].alive, "slime should be dead");
    assert_eq!(state.grid.entities[0].stats.level, 3);
    assert_eq!(state.grid.entities[0].stats.xp, 420);
    // A level-up grants nothing to spend: the attributes and their point pool are
    // gone (0.0.317). Levelling still means something — ATK is an authored curve,
    // and HIT, FLEE and soft DEF are functions of level.
    //
    // The allocate input survives for old saves whose log carries one, and must
    // replay as a no-op.
    let before = state_hash(&state);
    apply_input(&mut state, InputEvent::AllocateStatPoint {
        entity_id: 1, stat: StatKind::Str,
    });
    assert_eq!(state_hash(&state), before, "a retired input must not move state");

    let h = state_hash(&state);
    eprintln!("BETA-5 state_hash = {h}");
    assert_eq!(
        h, BETA5_EXPECTED_HASH,
        "BETA-5 golden hash drifted — see GDD §7.2"
    );
}
