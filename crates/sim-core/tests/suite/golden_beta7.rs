//! Golden test for BETA-5 Phase 3 — magic damage (the mage's fireball).
//!
//! Scenario:
//!   1. A mage at (0, 0): `DamageType::Magic`, ranged (range 6, travel 4
//!      ticks), Fire element, base magic power 20, INT 50. MATK =
//!      20 + matk_from_int(50) = 20 + (50 + ⌊50/5⌋²) = 20 + 150 = 170.
//!   2. A heavily-armored Earth slime at (3, 0): DEF 100, HP 200. If the
//!      fireball respected physical DEF it would deal (170−100)×1.5 = 105
//!      and need two casts; because magic IGNORES DEF it deals
//!      (170−0)×1.5 = 255 (Fire vs Earth) and one-shots.
//!   3. The mage fires on tick 1; the bolt travels 4 ticks and lands on
//!      tick 5. Run 6 ticks, then pin the kill + state hash.
//!
//! What this pins end-to-end:
//!   - `build_damage` picks MATK for a Magic attack and `apply_damage`
//!     bypasses physical DEF (uses MDEF, which is 0 at INT 1).
//!   - Magic rides the ranged/projectile path with NO crit/hit draws.
//!   - Fire→Earth element multiplier applies to magic too.

use sim_core::grid::{Cell, DamageType, Element, Entity, Team};
use sim_core::hash::state_hash;
use sim_core::state::GameState;
use sim_core::tick::tick;

// 0.0.15: +Stats job_level/job_xp/skill_points + Entity/MonsterTemplate
// job_xp_reward (RO Job Level). Shape add; gear-less/job_xp=0 scenarios
// behave identically.
// 0.0.16: +Entity sp + skills (Bash active, HP Recovery passive).
// Shape add; skill-less/SP-less scenarios behave identically (alpha6
// now auto-casts Bash + HP-regens, so its run legitimately changes).
// 0.0.33: +Entity.base_aspd (iRO Renewal ASPD model). The mage is
// hand-built via with_combat → base_aspd = 0 → flat-atk_speed
// fallback; fireball cast/armor-ignore behavior is unchanged. Pure JSON
// shape change (new field, default 0).
// 0.0.34: +Entity {endure_soft_def, taunted_by, taunt_ticks} + Item.weapon_type
// (Endure/Taunt + weapon-mastery scaffolding). Every scenario here is
// skill-less / weapon_type None, so behavior is identical — pure JSON shape.
const BETA7_EXPECTED_HASH: &str =
    "2816ae40eac5e56e8e35ce42c173bdb9f58532f46c6e9e277a563726bdf43d89";

const SLIME_HP: i32 = 100;
const SLIME_DEF: i32 = 100;

#[test]
fn beta7_mage_bolt_is_mitigated_like_any_hit() {
    let mut state = GameState::new(42, 16, 8);

    let mut mage = Entity::idle(1, Cell::new(0, 0))
        .with_team(Team::Player)
        .with_walk(Cell::new(0, 0), 0)
        .with_combat(/*hp*/ 100, /*magic power*/ 20, /*def*/ 0,
                     /*atk_speed_milli*/ 1000, Element::Fire)
        .with_attack(DamageType::Magic, true);
    mage.atk_range = 6;
    mage.proj_travel_ticks = 4;
    // INT no longer feeds MATK, and magic no longer ignores DEF (0.0.85-87): one
    // attack number, one mitigation curve, and what makes a bolt magic is its
    // ELEMENT. So the subject of this golden changed — it is now "a bolt flies,
    // lands on schedule, and is mitigated like anything else" — and the mage's
    // power is stated outright instead of being grown out of an attribute.
    state.grid.spawn(mage);

    let mut slime = Entity::idle(2, Cell::new(3, 0))
        .with_team(Team::Monster)
        .with_walk(Cell::new(3, 0), 0)
        .with_combat(/*hp*/ SLIME_HP, /*atk*/ 0, /*def*/ SLIME_DEF,
                     /*atk_speed_milli*/ 0, Element::Earth);
    slime.atk_range = 1;
    state.grid.spawn(slime);

    // Flight time now scales with distance (0.0.20): at dist 3 / range 6
    // the travel is round(4·3/6)=2 ticks, so the bolt fired on tick 1
    // arrives on tick 3. Before that the slime is untouched.
    tick(&mut state, 2);
    assert!(state.grid.entities[1].alive, "slime should still be airborne-safe pre-arrival");
    assert_eq!(state.grid.entities[1].hp, SLIME_HP, "no damage before the bolt lands");

    // Tick 3: the bolt lands. It hurts, and the slime's DEF took a share of it —
    // the share is what used to be zero.
    tick(&mut state, 1);
    let dealt = SLIME_HP - state.grid.entities[1].hp.max(0);
    assert!(dealt > 0, "the bolt must land");
    assert!(dealt < 20, "DEF {SLIME_DEF} must keep a real share of a 20-power bolt off: {dealt}");

    let h = state_hash(&state);
    eprintln!("BETA-7 state_hash = {h}");
    assert_eq!(
        h, BETA7_EXPECTED_HASH,
        "BETA-7 golden hash drifted — see GDD §7.2"
    );
}
