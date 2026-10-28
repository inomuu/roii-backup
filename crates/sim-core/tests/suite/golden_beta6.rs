//! Golden test for BETA-5 Phase 3 — hit / flee (accuracy & evasion).
//!
//! Scenario:
//!   1. Player melee at (0, 0): Fire, atk 10, swings once per tick,
//!      STAT_MIN otherwise (HIT = 175 + 1 + 1 + 0 = 177).
//!   2. A single dodgy Earth slime at (1, 0): AGI 100 (FLEE = 100 + 1 +
//!      100 + 0 = 201) and a huge HP pool so it survives the whole run.
//!      Accuracy = 80 + 177 − 201 = 56% → roughly half the swings whiff.
//!   3. The slime never retaliates (atk 0), so the only randomness in
//!      the run is the player's hit/miss roll — exactly what we want to
//!      pin. One attacker on the slime → no mobbed-flee penalty.
//!   4. Run 30 ticks, then pin the slime's HP and the state hash.
//!
//! What this pins end-to-end:
//!   - `combat::accuracy_bp` (RO `80 + HIT − FLEE`) below 100% actually
//!     drives a PRNG draw via `roll_hit`.
//!   - Misses deal zero damage (slime HP loss < swings × per-hit dmg).
//!   - The hit roll sits AFTER the (skipped, LUK 1) crit roll, so the
//!     draw order is fixed and reproducible.
//!
//! Pre-0.0.14 goldens are unaffected: every entity in them is STAT_MIN /
//! level 1, where accuracy caps at 100% and `roll_hit` takes no draw.

use sim_core::grid::{Cell, Element, Entity, Team};
use sim_core::hash::state_hash;
use sim_core::state::GameState;
use sim_core::tick::tick;

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
// fallback, so swing/miss timing is unchanged. Pure JSON shape change
// (new field, default 0).
// 0.0.34: +Entity {endure_soft_def, taunted_by, taunt_ticks} + Item.weapon_type
// (Endure/Taunt + weapon-mastery scaffolding). Every scenario here is
// skill-less / weapon_type None, so behavior is identical — pure JSON shape.
const BETA6_EXPECTED_HASH: &str =
    "6512a7bb2b65222c9bf776cc14ec3e63527d83c5bc99de16a2ba1dbe684e00d4";

// The per-hit number this test used to pin drifted every time damage was retuned
// (it was written when a clean hit was 16), and pinning it again would only mean
// pinning it wrong later. The subject is AVOIDANCE, so the test now compares two
// identical runs — one target with dodge, one without — and asserts the dodgy one
// takes strictly less. That holds whatever a hit is worth.
const SWINGS: i32 = 30;
const SLIME_HP: i32 = 100_000;

#[test]
fn beta6_a_dodgy_slime_takes_less_than_a_plain_one() {
    let mut state = GameState::new(42, 16, 8);

    let mut player = Entity::idle(1, Cell::new(0, 0))
        .with_team(Team::Player)
        .with_walk(Cell::new(0, 0), 0)
        .with_combat(/*hp*/ 100, /*atk*/ 30, /*def*/ 0,
                     /*atk_speed_milli*/ 1000, Element::Fire);
    player.atk_range = 1;
    state.grid.spawn(player);

    let mut slime = Entity::idle(2, Cell::new(1, 0))
        .with_team(Team::Monster)
        .with_walk(Cell::new(1, 0), 0)
        .with_combat(/*hp*/ SLIME_HP, /*atk*/ 0, /*def*/ 0,
                     /*atk_speed_milli*/ 0, Element::Earth);
    slime.atk_range = 1;
    // AGI is gone (0.0.86): a slippery monster is slippery because its DATA says
    // so. mob_flee 201 is the same 56% chance to be hit this test was built on.
    slime.equipped.push({
        let mut cloak = sim_core::items::Item::new(
            "cloak", sim_core::items::ItemKind::Armor, 0, 0, sim_core::items::Rarity::Common);
        cloak.bonuses = vec![sim_core::items::StatMod::DodgeBp(4_400)];
        cloak
    });
    state.grid.spawn(slime);

    tick(&mut state, SWINGS as u64);

    let slime_hp = state.grid.entities[1].hp;
    let lost = SLIME_HP - slime_hp;
    eprintln!("BETA-6 slime_hp = {slime_hp}  (lost {lost})");
    assert!(lost > 0, "no hits landed at all");
    assert!(state.grid.entities[1].alive, "slime should survive its huge HP pool");

    // THE CONTROL: the same fight against a slime with no dodge. It must take more.
    let mut plain = {
        let mut st = GameState::new(42, 16, 8);
        let mut p = Entity::idle(1, Cell::new(0, 0))
            .with_team(Team::Player)
            .with_walk(Cell::new(0, 0), 0)
            .with_combat(100, 30, 0, 1000, Element::Fire);
        p.atk_range = 1;
        st.grid.spawn(p);
        let mut m = Entity::idle(2, Cell::new(1, 0))
            .with_team(Team::Monster)
            .with_walk(Cell::new(1, 0), 0)
            .with_combat(SLIME_HP, 0, 0, 0, Element::Earth);
        m.atk_range = 1;
        st.grid.spawn(m);
        st
    };
    tick(&mut plain, SWINGS as u64);
    let plain_lost = SLIME_HP - plain.grid.entities[1].hp;
    assert!(
        lost < plain_lost,
        "a 44% dodger must take less than the same slime without it: {lost} vs {plain_lost}"
    );

    let h = state_hash(&state);
    eprintln!("BETA-6 state_hash = {h}");
    assert_eq!(
        h, BETA6_EXPECTED_HASH,
        "BETA-6 golden hash drifted — see GDD §7.2"
    );
}
