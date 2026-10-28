//! Golden test for BETA-4 — ranged attack (projectile travel).
//!
//! Scenario: 1 Archer-style Player vs 1 stationary Slime monster, both
//! within `atk_range` of each other on the same row. Archer is Fire
//! (advantage vs Earth) with `proj_travel_ticks = 3`, so every swing
//! pushes an `Projectile` whose damage lands 3 ticks later.
//!
//! What this pins end-to-end:
//!   - Projectile is queued (not resolved) on the fire tick.
//!   - Damage applies on `arrive_at_tick`, not on swing.
//!   - On-kill drops still fire correctly when the killing blow is an
//!     projectile (`resolve_arrived_projectiles` must route through `drop_on_death`
//!     just like melee).
//!   - `state.projectiles` is drained — no stale projectiles after combat.
//!   - PRNG advances by exactly the right number of draws (1 drop roll
//!     here) — the only way the hash matches across runs.
//!
//! Why a single 1v1 with stationary monster:
//!   - Mirrors `golden_alpha3` so the diff between melee and ranged is
//!     just the new fields + the resolve step.
//!   - A moving monster would make the projectile's pinned `target_pos`
//!     diverge from the kill position, masking whether the resolver
//!     uses id (correct) or position (would be a bug).

use sim_core::grid::{Cell, Element, Entity, Team};
use sim_core::hash::state_hash;
use sim_core::items::{DropEntry, Item, ItemKind, Rarity};
use sim_core::state::GameState;
use sim_core::tick::tick;

// Re-bumped in BETA-5 (sim-core 0.0.11-alpha): Stats baseline shifts
// archer's effective_atk by +1 (DEX, since ranged). Damage per projectile
// goes up slightly → kill is the same, drop fires the same, but the
// player now also carries XP from the kill, which changes the hash.
// Re-bumped in 0.0.12 (per-class roster). JSON shape only.
// Re-bumped in 0.0.14 (surround targeting). The slime target field
// drifts; archer is ranged so its own target stays on the slime.
// 0.0.14: +Entity.atk_type + is_ranged (Damage/magic refactor).
// Shape-only field add (defaults Physical/false) — behavior identical.
// 0.0.15: +Stats job_level/job_xp/skill_points + Entity/MonsterTemplate
// job_xp_reward (RO Job Level). Shape add; gear-less/job_xp=0 scenarios
// behave identically.
// 0.0.16: +Entity sp + skills (Bash active, HP Recovery passive).
// Shape add; skill-less/SP-less scenarios behave identically (alpha6
// now auto-casts Bash + HP-regens, so its run legitimately changes).
// 0.0.33: +Entity.base_aspd (iRO Renewal ASPD model). Both entities
// are hand-built via with_combat → base_aspd = 0 → flat-atk_speed
// fallback, so swing timing is unchanged (slime still dead / archer alive
// by tick 50). Pure JSON shape change (new field, default 0).
// 0.0.34: +Entity {endure_soft_def, taunted_by, taunt_ticks} + Item.weapon_type
// (Endure/Taunt + weapon-mastery scaffolding). Every scenario here is
// skill-less / weapon_type None, so behavior is identical — pure JSON shape.
// 0.0.42: +Item.equip_slot (RO 10-slot paper-doll). Pure shape add — this
// scenario's gear has equip_slot=None → slot() falls back to kind, identical
// behavior; only the serialized hash shifts. Re-blessed.
// 0.0.45: +Item.equip_classes (weapon class-restriction override). Pure shape add
// for this scenario (empty vec) — only the serialized hash shifts. Re-blessed.
// 0.0.47: direct-grant — drops bank to the stash, not the escrow. This run's
// loot now lands in state.inventory (was state.escrow) → state hash shifts. Re-blessed.
const BETA4_EXPECTED_HASH: &str =
    "3184eae65a337d0167eba397289443b9ffbcd1bb1351962b2698ebff57e128b3";

#[test]
fn beta4_archer_kills_monster_with_projectile() {
    let mut state = GameState::new(42, 16, 8);

    // Archer at (0, 0): Fire, range 5, projectile travels 3 ticks.
    let mut archer = Entity::idle(1, Cell::new(0, 0))
        .with_team(Team::Player)
        .with_walk(Cell::new(0, 0), 500)
        .with_combat(/*hp*/ 150, /*atk*/ 32, /*def*/ 5,
                     /*atk_speed_milli*/ 130, Element::Fire);
    archer.atk_range = 5;
    archer.proj_travel_ticks = 3;
    state.grid.spawn(archer);

    // Stationary Earth slime at (4, 0): well within the archer's range
    // so no walk-in phase confuses the trace. 100%-scrap drop table so
    // exactly ONE item lands in the archer's inventory on kill.
    let mut slime = Entity::idle(2, Cell::new(4, 0))
        .with_team(Team::Monster)
        .with_walk(Cell::new(4, 0), 0)
        .with_combat(/*hp*/ 35, /*atk*/ 5, /*def*/ 3,
                     /*atk_speed_milli*/ 100, Element::Earth)
        .with_drops(vec![DropEntry::new(
            Item::new("scrap", ItemKind::Material, 0, 0, Rarity::Common),
            100,
        )]);
    slime.atk_range = 1;
    state.grid.spawn(slime);

    // Sanity: no projectiles in flight at start.
    assert!(state.projectiles.is_empty());

    // 50 ticks = 5 sim-seconds. Plenty for one swing (≈ 8 ticks @ 130
    // milli/tick) + 3-tick flight + cleanup. Anything more is just
    // empty waiting and keeps RNG state stable.
    tick(&mut state, 50);

    // Monster dead by projectile; archer untouched (monster never in range).
    assert!(!state.grid.entities[1].alive, "slime should be dead by tick 50");
    assert!(state.grid.entities[0].alive, "archer should not have died");
    // Projectile drained, exactly one drop landed (direct-grant → the stash).
    assert!(state.projectiles.is_empty(), "no stale projectiles after combat");
    assert_eq!(state.inventory.len(), 1,
        "exactly one drop should fire from projectile kill");

    let h = state_hash(&state);
    eprintln!("BETA-4 state_hash = {h}");
    assert_eq!(
        h, BETA4_EXPECTED_HASH,
        "BETA-4 golden hash drifted — see GDD §7.2"
    );
}
