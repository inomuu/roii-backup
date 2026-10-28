//! BETA-4 — tick integration tests.
//!
//! Split into two cohesive groups:
//!   - `projectiles` — projectile travel / AoE resolution and the
//!     zone-clear / farm-loop progression paths.
//!   - `skills` — skill casts, buffs/DoTs, taunt/endure, passives.
//!
//! Shared `world()` / `push_projectile()` fixtures live here so both
//! groups build the same minimal world.

pub(crate) use super::*;
pub(crate) use crate::grid::{Cell, Element, Entity, Team};
pub(crate) use crate::items::{DropEntry, Item, ItemKind, Rarity};
pub(crate) use crate::state::{Projectile, MonsterSpawnEntry, MonsterTemplate, ZoneSpawnState};

mod projectiles;
mod skills;

/// Minimal world: 1 Player + 1 Monster + a no-op ZoneSpawnState
/// (kill counter exists but boss never fires). Player has empty
/// inventory so drop-on-death lands somewhere observable.
pub(crate) fn world() -> GameState {
    let mut s = GameState::new(42, 16, 8);
    s.grid.spawn({
        let p = Entity::idle(1, Cell::new(0, 0))
            .with_team(Team::Player)
            .with_combat(100, 10, 0, 0, Element::Fire);
        // Accuracy is HIT - FLEE and both are level functions now (0.0.317), so this
        // fixture cannot buy its way out of a miss with DEX any more.
        p
    });
    let scrap = Item::new("scrap", ItemKind::Material, 0, 0, Rarity::Common);
    let monster = Entity::idle(1000, Cell::new(5, 0))
        .with_team(Team::Monster)
        .with_combat(20, 0, 0, 0, Element::Earth)
        .with_drops(vec![DropEntry::new(scrap, 100)]);
    s.grid.spawn(monster);
    // Provide an empty-pool zone_spawn so handle_monster_death has
    // somewhere to bump kill_counter without producing real spawns.
    s.zone_spawn = Some(ZoneSpawnState {
        opened_at_tick: 0,
        deploy_grace_ticks: crate::tick::DEPLOY_GRACE_TICKS_OPEN,
        pool: vec![MonsterSpawnEntry {
            template: MonsterTemplate {
                gear_grade_odds: Vec::new(),
                resists: Vec::new(),
                weaknesses: Vec::new(),
                def_id: "ignored".to_string(),
                bestiary_group: String::new(),
                hp: 1, atk: 0, def: 0,
                atk_min: 0, atk_max: 0,
                skills: Vec::new(),
                element: Element::Earth,
                toughness_max: 100,
                speed_milli: 0,
                atk_speed_milli: 0,
                atk_range: 1,
                drops: vec![],
                proj_travel_ticks: 0,
                xp_reward: 0,
                gil_reward: 0,
                stage: 0,
                boss_minions: 0,
            race: crate::grid::Race::Unknown,
            size: crate::grid::Size::Medium,
            },
            weight: 1,
        }],
        target_population: 0,        // already at target → no spawns
        spawn_interval_ticks: 9_999,
        next_spawn_at_tick: u64::MAX,
        kill_counter: 0,
        boss_threshold: 9_999,       // never trips
        boss_spawned: false,
        boss_template: None,
        boss_from_pool: false,
        max_spawns: 0,
                player_skills: Vec::new(),
        zone_cleared: false,
        cleared_at_tick: None,
        band_clear_counted: false,
        next_zone_id: None,
        is_instance_end: false,
        instance_start_zone_id: String::new(),
    });
    s
}

pub(crate) fn push_projectile(s: &mut GameState, target_id: u32, arrive_at_tick: u64, damage: i32) {
    s.projectiles.push(Projectile {
        is_basic: false,
        target_id,
        spawn_pos: Cell::new(0, 0),
        target_pos: Cell::new(5, 0),
        spawn_tick: s.tick,
        arrive_at_tick,
        damage,
        is_magic: false,
        aoe_radius: 0,
        victim_team: Team::Monster,
        sky_fall: false,
        skill_id: String::new(),
        attacker_id: 0,
        display_hits: 1,
        burn_tier: 0,
        focus_mult_bp: 0,
        pin_slow_pct: 0,
        element: crate::grid::Element::Physical,
        crit: false,
    });
}

/// COMMANDER SMITE — the placed 2×2 blast (`docs/commander-skills-design.md`).
mod commander {
    use super::*;
    use crate::events::{apply_input, InputEvent};
    use crate::state::{COMMANDER_CHARGE_MAX, COMMANDER_CHARGE_TICKS};

    fn mob_at(s: &mut GameState, id: u32, x: i32, y: i32) {
        s.grid.spawn(
            Entity::idle(id, Cell::new(x, y))
                .with_team(Team::Monster)
                .with_combat(10_000, 0, 0, 0, Element::Earth),
        );
    }

    fn hp(s: &GameState, id: u32) -> i32 {
        s.grid.entities.iter().find(|e| e.id == id).map(|e| e.hp).unwrap_or(-1)
    }

    /// Cast smite as a player who OWNS it — smite is a first unlock, not a starter, so a cast now
    /// gates on ownership. Granting here (idempotent) keeps every existing test meaning "an owner
    /// casts"; the gate itself is pinned separately by `an_unowned_smite_is_a_free_no_op`.
    fn smite(s: &mut GameState, x: i32, y: i32) {
        s.commander_skills_owned
            .insert(crate::events::handlers::COMMANDER_SMITE.to_string());
        apply_input(s, InputEvent::CommanderSmite { x, y });
    }

    /// THE FOOTPRINT: the cell aimed at is the TOP-LEFT of the 2×2, so the block covers
    /// `(x, y) … (x+1, y+1)` and nothing up or left of it (user, 2026-08-21).
    #[test]
    fn the_aimed_cell_is_the_top_left_of_the_block() {
        let mut s = world();
        for (id, x, y) in [
            (2001, 5, 3), (2002, 6, 3), (2003, 5, 4), (2004, 6, 4),   // inside
            (2005, 4, 3), (2006, 5, 2), (2007, 7, 3), (2008, 5, 5),   // outside: L, U, R, D
        ] {
            mob_at(&mut s, id, x, y);
        }
        smite(&mut s, 5, 3);
        for inside in [2001, 2002, 2003, 2004] {
            assert!(hp(&s, inside) < 10_000, "{inside} is inside the 2×2 and took nothing");
        }
        for outside in [2005, 2006, 2007, 2008] {
            assert_eq!(hp(&s, outside), 10_000, "{outside} is outside the 2×2 and was hit");
        }
    }

    /// Damage is `5 × party average level`, and the average counts the DEAD (user) — a member
    /// dying must not weaken the answer to the fight that killed them.
    #[test]
    fn damage_is_five_times_the_party_average_and_the_dead_still_count() {
        let mut s = world();
        s.grid.entities[0].stats.level = 40;
        // A second member at level 20: average 30 → 150 damage.
        s.grid.spawn(
            Entity::idle(2, Cell::new(0, 1)).with_team(Team::Player)
                .with_combat(100, 10, 0, 0, Element::Fire),
        );
        s.grid.entities[2].stats.level = 20;
        mob_at(&mut s, 3001, 5, 3);
        smite(&mut s, 5, 3);
        assert_eq!(hp(&s, 3001), 10_000 - 150, "expected 5 × avg(40, 20) = 150");

        // Kill the level-20 member. The average must NOT rise to 40.
        s.grid.entities[2].alive = false;
        s.grid.entities[2].hp = 0;
        s.commander_cd.clear();
        smite(&mut s, 5, 3);
        assert_eq!(hp(&s, 3001), 10_000 - 300, "a dead member must still count toward the average");
    }

    /// PETS ARE EXCLUDED. They are `Team::Player` too, so a low-level pet would drag the average
    /// down and a Ranger build would quietly do less Commander damage than the same party without
    /// one. `party_max_level` does not filter them, which is exactly the trap this pins.
    #[test]
    fn a_pet_does_not_drag_the_average_down() {
        let mut s = world();
        s.grid.entities[0].stats.level = 40;
        let mut pet = Entity::idle(9, Cell::new(0, 2)).with_team(Team::Player)
            .with_combat(50, 5, 0, 0, Element::Fire);
        pet.is_pet = true;
        pet.stats.level = 1;
        s.grid.spawn(pet);
        mob_at(&mut s, 4001, 5, 3);
        smite(&mut s, 5, 3);
        assert_eq!(hp(&s, 4001), 10_000 - 200, "the pet must not be in the average");
    }

    /// The shared pool and the per-skill cooldown are SEPARATE limiters, and both bind.
    #[test]
    fn the_charge_and_the_cooldown_both_gate_it() {
        let mut s = world();
        s.grid.entities[0].stats.level = 10;
        mob_at(&mut s, 5001, 5, 3);
        smite(&mut s, 5, 3);
        assert_eq!(s.commander_charges, COMMANDER_CHARGE_MAX - 1, "one charge spent");
        let after_one = hp(&s, 5001);

        // Charges remain, but the cooldown is up: nothing happens and NOTHING is spent.
        smite(&mut s, 5, 3);
        assert_eq!(hp(&s, 5001), after_one, "fired again while on cooldown");
        assert_eq!(s.commander_charges, COMMANDER_CHARGE_MAX - 1, "a blocked cast still charged");

        // Cooldown clear but pool empty: also blocked.
        s.commander_cd.clear();
        s.commander_charges = 0;
        smite(&mut s, 5, 3);
        assert_eq!(hp(&s, 5001), after_one, "fired with an empty pool");
    }

    /// A cast that finds nothing still costs. Otherwise the cheapest play is to fire into empty
    /// space until something walks in.
    #[test]
    fn a_cast_into_empty_space_still_spends() {
        let mut s = world();
        smite(&mut s, 12, 6);
        assert_eq!(s.commander_charges, COMMANDER_CHARGE_MAX - 1);
        assert!(s.commander_cd.contains_key(crate::events::handlers::COMMANDER_SMITE));
    }

    /// Off-board is a silent no-op that costs nothing — `apply_input` may never panic, because a
    /// replay has to survive game_data version skew.
    #[test]
    fn an_off_board_cell_is_a_free_no_op() {
        let mut s = world();
        for (x, y) in [(-1, 0), (0, -1), (999, 0), (0, 999)] {
            smite(&mut s, x, y);
        }
        assert_eq!(s.commander_charges, COMMANDER_CHARGE_MAX, "an illegal cell must not cost");
        assert!(s.commander_cd.is_empty());
    }

    /// A pool that sat full for ten intervals must not bank them and hand the time back the
    /// instant it is spent.
    ///
    /// Spent through the REAL path rather than by assigning the field: the regen clock is stamped
    /// on the spend, so a test that zeroes `commander_charges` by hand is testing a state the game
    /// cannot reach and would report a bug that is not there. (It did, once.)
    #[test]
    fn a_long_full_pool_does_not_bank_progress() {
        let mut s = world();
        crate::tick::tick(&mut s, COMMANDER_CHARGE_TICKS * 10);
        assert_eq!(s.commander_charges, COMMANDER_CHARGE_MAX, "a full pool must not overflow");

        smite(&mut s, 5, 3);
        assert_eq!(s.commander_charges, COMMANDER_CHARGE_MAX - 1);
        crate::tick::tick(&mut s, COMMANDER_CHARGE_TICKS - 1);
        assert_eq!(
            s.commander_charges, COMMANDER_CHARGE_MAX - 1,
            "ten intervals of standing still were banked and refunded the spend",
        );
        crate::tick::tick(&mut s, 1);
        assert_eq!(s.commander_charges, COMMANDER_CHARGE_MAX, "one charge, one interval later");
    }

    /// A full pool must not WRITE to the regen stamp, or every save carries a non-default field —
    /// which serializes it, which moves every golden. Twelve of them, when this was first written
    /// the obvious way.
    #[test]
    fn a_pool_that_never_casts_leaves_no_trace_in_the_save() {
        let mut s = world();
        crate::tick::tick(&mut s, COMMANDER_CHARGE_TICKS * 3);
        assert_eq!(s.commander_charge_tick, 0, "the stamp was written without a cast");
        assert!(s.commander_cd.is_empty());
        let json = serde_json::to_string(&s).expect("state serializes");
        for field in ["commander_charges", "commander_charge_tick", "commander_cd"] {
            assert!(!json.contains(field), "{field} serialized on a save that never cast");
        }
    }

    /// The cooldown ages on the tick and clears itself.
    #[test]
    fn the_cooldown_ages_out() {
        let mut s = world();
        smite(&mut s, 5, 3);
        assert!(!s.commander_cd.is_empty());
        crate::tick::tick(&mut s, crate::events::handlers::COMMANDER_SMITE_CD as u64);
        assert!(s.commander_cd.is_empty(), "the cooldown never cleared");
    }

    /// THE UNLOCK GATE: smite is the first Commander UNLOCK, not a starter, so a cast before the
    /// player owns it is a FREE no-op — no damage, no charge spent, no cooldown started. This is
    /// what lets `commander_skills_owned` act as the unlock latch (`docs/commander-skills-design.md`).
    /// Cast directly (not via `smite()`, which grants) so the unowned path is what's under test.
    #[test]
    fn an_unowned_smite_is_a_free_no_op() {
        let mut s = world();
        mob_at(&mut s, 6001, 5, 3);
        // Unowned: the cast does nothing and costs nothing.
        apply_input(&mut s, InputEvent::CommanderSmite { x: 5, y: 3 });
        assert_eq!(hp(&s, 6001), 10_000, "an unowned cast dealt damage");
        assert_eq!(s.commander_charges, COMMANDER_CHARGE_MAX, "an unowned cast spent a charge");
        assert!(s.commander_cd.is_empty(), "an unowned cast started a cooldown");
        assert!(s.commander_skills_owned.is_empty(), "nothing granted the skill");

        // Grant it (the debug event the dev toggle sends), then the SAME cast lands.
        apply_input(
            &mut s,
            InputEvent::SetCommanderSkillOwned {
                skill_id: crate::events::handlers::COMMANDER_SMITE.to_string(),
                owned: true,
            },
        );
        apply_input(&mut s, InputEvent::CommanderSmite { x: 5, y: 3 });
        assert!(hp(&s, 6001) < 10_000, "an owned cast should hit");
        assert_eq!(s.commander_charges, COMMANDER_CHARGE_MAX - 1, "an owned cast spends a charge");
    }

    /// The PRODUCTION grant (`GrantCommanderSkill`, server-authored) unlocks the same way the debug
    /// twin does — the sim is total, it just inserts; the nonce is verify-server's to check, not the
    /// sim's. `docs/commander-level-and-outpost.md`.
    #[test]
    fn a_production_grant_unlocks_smite() {
        let mut s = world();
        mob_at(&mut s, 7001, 5, 3);
        apply_input(
            &mut s,
            InputEvent::GrantCommanderSkill {
                skill_id: crate::events::handlers::COMMANDER_SMITE.to_string(),
                nonce: 12345,
            },
        );
        assert!(s.commander_skills_owned.contains(crate::events::handlers::COMMANDER_SMITE));
        apply_input(&mut s, InputEvent::CommanderSmite { x: 5, y: 3 });
        assert!(hp(&s, 7001) < 10_000, "a production-granted smite should cast");
    }

    /// A 0 nonce is a benign NO-OP, not an unlock — the sim must not insert it, so it stays in step
    /// with the verifier (which skips a 0 nonce rather than flagging a forgery). A dropped/missing
    /// nonce field must never brand a legitimate account a cheat.
    #[test]
    fn a_zero_nonce_grant_is_a_no_op() {
        let mut s = world();
        apply_input(
            &mut s,
            InputEvent::GrantCommanderSkill {
                skill_id: crate::events::handlers::COMMANDER_SMITE.to_string(),
                nonce: 0,
            },
        );
        assert!(s.commander_skills_owned.is_empty(), "a 0-nonce grant must not unlock");
    }
}

/// STALEMATE (0.0.418): a boss up, a member down, and the boss's HP floor not moving for two
/// minutes → the survivors fall, so the host's wipe path runs. A floor that keeps dropping (a
/// fight being won, however slowly) never trips it; nobody down never trips it.
#[test]
fn a_boss_fight_nobody_can_end_fells_the_survivors_but_a_winning_one_never_does() {
    use crate::grid::{Cell, Element, Entity, Team};
    let build = || {
        let mut s = world();
        let mut down = Entity::idle(2, Cell::new(1, 0)).with_team(Team::Player).with_combat(100, 10, 0, 0, Element::Fire);
        down.alive = false;
        down.hp = 0;
        s.grid.spawn(down);
        let mut boss = Entity::idle(3, Cell::new(6, 0)).with_team(Team::Monster).with_combat(10_000, 0, 0, 0, Element::Earth);
        boss.is_boss = true;
        s.grid.spawn(boss);
        let bi = s.grid.entities.len() - 1;
        s.zone_spawn.as_mut().unwrap().boss_spawned = true;
        (s, bi)
    };
    // The floor holds: felled at the clock.
    let (mut s, _) = build();
    for _ in 0..=crate::state::STALEMATE_TICKS - 1 {
        super::break_stalemate(&mut s);
    }
    assert!(s.grid.entities[0].alive, "one tick short, still standing");
    super::break_stalemate(&mut s);
    assert!(!s.grid.entities[0].alive, "two minutes on a flat floor: the survivor falls");
    assert_eq!((s.stalemate_ticks, s.stalemate_floor), (0, 0));

    // The floor keeps dropping (a slow win): never felled, however long.
    let (mut s, bi) = build();
    for i in 0..3 * crate::state::STALEMATE_TICKS {
        if i % 500 == 0 {
            s.grid.entities[bi].hp -= 100;
        }
        super::break_stalemate(&mut s);
    }
    assert!(s.grid.entities[0].alive, "a boss losing HP is a fight being won");

    // Healed back above the floor does not reset it: the maid case.
    let (mut s, bi) = build();
    super::break_stalemate(&mut s);
    s.grid.entities[bi].hp -= 3_000;
    super::break_stalemate(&mut s);
    s.grid.entities[bi].hp += 3_000; // healed back
    for _ in 0..crate::state::STALEMATE_TICKS {
        super::break_stalemate(&mut s);
    }
    assert!(!s.grid.entities[0].alive, "healing back above the floor is the deadlock itself");

    // Nobody down: the LONG window (0.0.443) — two minutes on a flat floor is fine, five is not.
    let mut s = world();
    let mut boss = Entity::idle(3, Cell::new(6, 0)).with_team(Team::Monster).with_combat(10_000, 0, 0, 0, Element::Earth);
    boss.is_boss = true;
    s.grid.spawn(boss);
    s.zone_spawn.as_mut().unwrap().boss_spawned = true;
    for _ in 0..2 * crate::state::STALEMATE_TICKS {
        super::break_stalemate(&mut s);
    }
    assert!(s.grid.entities[0].alive, "two minutes at full strength is still a fight");
    for _ in 0..crate::state::STALEMATE_TICKS_HEALTHY {
        super::break_stalemate(&mut s);
    }
    assert!(!s.grid.entities[0].alive, "five minutes on a flat floor with nobody down: still a deadlock");
}
