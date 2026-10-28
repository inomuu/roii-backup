//! Projectile travel / AoE resolution and the zone-clear / farm-loop
//! progression paths.
//!
//! These tests cover `resolve_arrived_projectiles` in isolation:
//! projectiles are constructed directly (no fire path) so a single
//! test pins exactly one behavior of the resolver. The full
//! fire→fly→arrive path is exercised by the integration goldens.

use super::*;

#[test]
fn projectile_in_flight_does_not_apply_damage_early() {
    let mut s = world();
    // Fire at tick 0, arrive at tick 3.
    push_projectile(&mut s, 1000, 3, 5);
    // After 2 ticks the projectile should still be airborne; HP intact.
    tick(&mut s, 2);
    assert_eq!(s.grid.entities[1].hp, 20, "monster took damage too early");
    assert_eq!(s.projectiles.len(), 1, "projectile consumed too early");
}

#[test]
fn projectile_applies_damage_on_arrival_tick() {
    let mut s = world();
    push_projectile(&mut s, 1000, 3, 5);
    tick(&mut s, 3);
    assert_eq!(s.grid.entities[1].hp, 15, "damage didn't land on arrival");
    assert!(s.projectiles.is_empty(), "projectile should be consumed");
    assert!(s.grid.entities[1].alive, "should NOT have died from 5 dmg");
}

#[test]
fn aoe_projectile_hits_every_enemy_in_the_blast() {
    let mut s = world();
    // world() has Monster id 1000 at (5,0). Cluster two more within a
    // 3×3 of it, and put one far away that must survive.
    for (id, cell) in [(1001, Cell::new(6, 0)), (1002, Cell::new(6, 1))] {
        s.grid.spawn(
            Entity::idle(id, cell)
                .with_team(Team::Monster)
                .with_combat(20, 0, 0, 0, Element::Earth),
        );
    }
    s.grid.spawn(
        Entity::idle(1003, Cell::new(0, 7))
            .with_team(Team::Monster)
            .with_combat(20, 0, 0, 0, Element::Earth),
    );
    // AoE bolt centred on id 1000, radius 1, lethal damage.
    s.projectiles.push(Projectile {
        is_basic: false,
        target_id: 1000,
        spawn_pos: Cell::new(0, 0),
        target_pos: Cell::new(5, 0),
        spawn_tick: s.tick,
        arrive_at_tick: 1,
        damage: 999,
        is_magic: true,
        aoe_radius: 1,
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
    tick(&mut s, 1);
    let alive = |id: u32| s.grid.idx_of_id(id)
        .map(|i| s.grid.entities[i].alive).unwrap_or(false);
    assert!(!alive(1000), "blast centre should die");
    assert!(!alive(1001), "neighbour in 3×3 should die");
    assert!(!alive(1002), "diagonal neighbour in 3×3 should die");
    assert!(alive(1003), "monster outside the blast must survive");
    // Three kills counted; three drops rolled into the stash.
    assert_eq!(s.zone_spawn.as_ref().unwrap().kill_counter, 3);
}

/// 0.0.225: the ranged FIRE path folds gear dmg-vs (and the card channels)
/// like resolve_attack_scaled — a Hydra-lineage line on a bow must raise the
/// landed damage. Same seed both runs (the fold takes no draws), so the only
/// difference is the ×(1 + vs) fold.
#[test]
fn ranged_shot_folds_gear_dmg_vs() {
    use crate::items::{EquipSlot, StatMod};
    fn run(with_gear: bool) -> i32 {
        let mut s = world();
        let e = &mut s.grid.entities[0];
        e.proj_travel_ticks = 2; // ranged archer
        e.atk_range = 6; // reaches the monster at distance 5
        e.atk_speed_milli = 1000;
        if with_gear {
            let mut bow = Item::new("bow", ItemKind::Armor, 0, 0, Rarity::Common);
            bow.equip_slot = Some(EquipSlot::Weapon);
            // The world() monster is WEAK to Earth below — +20% vs what it is weak to.
            bow.bonuses.push(StatMod::DmgVsElement(Element::Earth, 2_000));
            e.equipped.push(bow);
        }
        s.grid.entities[1].hp = 10_000;
        s.grid.entities[1].max_hp = 10_000;
        s.grid.entities[1].weaknesses = vec![Element::Earth];
        tick(&mut s, 10); // fire + fly + land
        10_000 - s.grid.entities[1].hp
    }
    let base = run(false);
    let folded = run(true);
    assert!(base > 0, "control shot never landed");
    assert!(
        folded > base,
        "dmg-vs gear must raise ranged damage (base {base}, folded {folded})"
    );
}

#[test]
fn projectile_whiffs_if_target_already_dead() {
    let mut s = world();
    // Pre-kill the monster (e.g. melee from another source).
    s.grid.entities[1].hp = 0;
    s.grid.entities[1].alive = false;
    push_projectile(&mut s, 1000, 1, 999);
    tick(&mut s, 1);
    // HP stays at 0 (no further damage); projectile removed; no double
    // death triggers (kill_counter must not bump on a whiff).
    assert_eq!(s.grid.entities[1].hp, 0);
    assert!(s.projectiles.is_empty(), "whiffed projectile should still be removed");
    let zs = s.zone_spawn.as_ref().unwrap();
    assert_eq!(zs.kill_counter, 0, "whiff should NOT count as a kill");
}

#[test]
fn projectile_that_kills_triggers_drop_and_kill_counter() {
    let mut s = world();
    // Lethal damage so the resolver enters the death branch.
    push_projectile(&mut s, 1000, 1, 999);
    let inv_before = s.inventory.len();
    tick(&mut s, 1);
    assert!(!s.grid.entities[1].alive, "monster should be dead");
    assert_eq!(s.projectiles.len(), 0);
    // Direct-grant: the dropped item lands straight in the stash.
    let inv_after = s.inventory.len();
    assert_eq!(inv_after, inv_before + 1, "drop didn't fire on projectile kill");
    let zs = s.zone_spawn.as_ref().unwrap();
    assert_eq!(zs.kill_counter, 1, "kill_counter didn't bump on projectile kill");
}

#[test]
fn projectiles_resolve_in_fifo_order() {
    // Two projectiles arrive on the same tick. Order matters: the first
    // to land is whichever was pushed first. With damage 10 each
    // against a 15 hp monster, projectile A reduces hp to 5; projectile B
    // then kills. Only ONE drop should fire (one death event).
    let mut s = world();
    s.grid.entities[1].hp = 15;
    s.grid.entities[1].max_hp = 15;
    push_projectile(&mut s, 1000, 1, 10);   // A — first
    push_projectile(&mut s, 1000, 1, 10);   // B — second, kills
    tick(&mut s, 1);
    assert!(!s.grid.entities[1].alive);
    assert_eq!(s.inventory.len(), 1,
        "exactly one drop should fire even with overkill projectiles");
    let zs = s.zone_spawn.as_ref().unwrap();
    assert_eq!(zs.kill_counter, 1, "kill_counter must not double-count");
}

#[test]
fn kills_award_soul() {
    let mut s = world(); // monster id1000 @ (5,0)
    s.grid.entities[0].atk = 100;
    s.grid.entities[0].atk_speed_milli = 1000;
    s.grid.entities[0].atk_range = 10;
    s.grid.entities[0].pos = Cell::new(4, 0);
    s.grid.entities[1].hp = 1;
    assert_eq!(s.soul(), 0);
    tick(&mut s, 1);
    assert!(!s.grid.entities[1].alive);
    assert_eq!(s.soul(), 1, "+1 soul per normal kill");
}

#[test]
fn farm_mode_loops_instance_else_advances() {
    fn run(farm: bool) -> Option<String> {
        let mut s = world();
        {
            let zs = s.zone_spawn.as_mut().unwrap();
            zs.is_instance_end = true;
            zs.next_zone_id = Some("zone_advance".to_string());
            zs.instance_start_zone_id = "zone_farm".to_string();
        }
        s.farm_current_instance = farm;
        s.grid.entities[1].is_boss = true;
        s.grid.entities[1].hp = 1;
        s.grid.entities[1].drops = vec![];
        // Boss appeared → spawner halted; sweep-clear fires when this lone monster dies.
        s.zone_spawn.as_mut().unwrap().boss_spawned = true;
        s.grid.entities[0].atk = 100;
        s.grid.entities[0].atk_speed_milli = 1000;
        s.grid.entities[0].atk_range = 10;
        s.grid.entities[0].pos = Cell::new(4, 0);
        // Boss dies tick 1; the zone change is HELD ZONE_CLEAR_DELAY_TICKS
        // for the death animation, so tick past the hold before reading it.
        tick(&mut s, ZONE_CLEAR_DELAY_TICKS + 2);
        s.pending_zone_change.clone()
    }
    assert_eq!(run(false), Some("zone_advance".to_string()), "advance when not farming");
    assert_eq!(run(true), Some("zone_farm".to_string()), "loop room 1 when farming");
}

#[test]
fn party_wipe_during_clear_hold_blocks_the_advance() {
    let mut s = world();
    {
        let zs = s.zone_spawn.as_mut().unwrap();
        zs.is_instance_end = true;
        zs.next_zone_id = Some("zone_advance".to_string());
        zs.instance_start_zone_id = "zone_farm".to_string();
    }
    s.grid.entities[1].is_boss = true;
    s.grid.entities[1].hp = 1;
    // Boss has appeared → spawner halted; the sweep-clear rule (all monsters dead) can now fire
    // once this lone monster dies. Without it `zone_cleared` never flips under the new rule.
    s.zone_spawn.as_mut().unwrap().boss_spawned = true;
    s.grid.entities[1].drops = vec![];
    s.grid.entities[0].atk = 100;
    s.grid.entities[0].atk_speed_milli = 1000;
    s.grid.entities[0].atk_range = 10;
    s.grid.entities[0].pos = Cell::new(4, 0);
    // Boss dies on tick 1 → the clear-hold starts.
    tick(&mut s, 1);
    assert!(!s.grid.entities[1].alive, "boss should die");
    assert!(s.zone_spawn.as_ref().unwrap().zone_cleared, "zone marked cleared");
    // The party gets finished off DURING the hold (leftover minions).
    s.grid.entities[0].alive = false;
    s.grid.entities[0].hp = 0;
    // Tick past the hold: the advance must NOT fire — a full wipe forfeits the
    // clear, leaving the harness to restart the instance.
    tick(&mut s, ZONE_CLEAR_DELAY_TICKS + 4);
    assert_eq!(s.pending_zone_change, None, "wiped party does not advance");
}

#[test]
fn instance_end_clear_counts_one_band_run() {
    let mut s = world();
    {
        let zs = s.zone_spawn.as_mut().unwrap();
        zs.is_instance_end = true;
        zs.next_zone_id = Some("zone_advance".to_string());
        zs.instance_start_zone_id = "zone_b5_1".to_string(); // band 5
    }
    s.grid.entities[1].is_boss = true;
    s.grid.entities[1].hp = 1;
    // Boss has appeared → spawner halted; the sweep-clear rule (all monsters dead) can now fire
    // once this lone monster dies. Without it `zone_cleared` never flips under the new rule.
    s.zone_spawn.as_mut().unwrap().boss_spawned = true;
    s.grid.entities[1].drops = vec![];
    s.grid.entities[0].atk = 100;
    s.grid.entities[0].atk_speed_milli = 1000;
    s.grid.entities[0].atk_range = 10;
    s.grid.entities[0].pos = Cell::new(4, 0);
    // Boss of room 3 dies → exactly ONE band-5 run is logged, even though the
    // is_end check runs every tick across the clear-hold (the latch guards it).
    tick(&mut s, ZONE_CLEAR_DELAY_TICKS + 4);
    assert_eq!(s.measures.bands_cleared.get(), 1, "one band run recorded");
}

#[test]
fn instance_end_clear_banks_escrow_to_stash() {
    use crate::items::{Item, ItemKind, Rarity};
    let mut s = world(); // player id1 @ (0,0); monster id1000 @ (5,0)
    // Make this an instance-FINAL room and put loot in escrow.
    {
        let zs = s.zone_spawn.as_mut().unwrap();
        zs.is_instance_end = true;
        zs.next_zone_id = None; // terminal for this sim-only test
    }
    s.escrow.push(Item::new("trophy", ItemKind::Material, 0, 0, Rarity::Common));
    // The monster is the boss; the player one-shots it.
    s.grid.entities[1].is_boss = true;
    s.grid.entities[1].hp = 1;
    // Boss has appeared → spawner halted; the sweep-clear rule (all monsters dead) can now fire
    // once this lone monster dies. Without it `zone_cleared` never flips under the new rule.
    s.zone_spawn.as_mut().unwrap().boss_spawned = true;
    s.grid.entities[1].drops = vec![]; // keep the boss's own drop out of it
    s.grid.entities[0].atk = 100;
    s.grid.entities[0].atk_speed_milli = 1000;
    s.grid.entities[0].atk_range = 10;
    s.grid.entities[0].pos = Cell::new(4, 0);
    tick(&mut s, 1);
    assert!(!s.grid.entities[1].alive, "boss should die");
    // Escrow banks with the (now delayed) zone-change request — tick past
    // the death-animation hold before asserting it moved.
    tick(&mut s, ZONE_CLEAR_DELAY_TICKS + 2);
    assert!(s.escrow.is_empty(), "escrow banked on instance clear");
    assert_eq!(s.inventory.len(), 1, "trophy moved escrow → stash");
}

/// 0.0.128 regression guard: the boss must spawn from its FULL template — the old
/// hand-rolled copy in `try_spawn_boss` silently dropped mdef/hit/flee, the
/// atk min-max variance band and the catalog skills, so every boss fought far
/// weaker than its game_data entry said.
#[test]
fn boss_spawns_with_full_template_stats_and_skills() {
    let mut s = world();
    {
        let zs = s.zone_spawn.as_mut().unwrap();
        zs.boss_threshold = 0; // kill_counter 0 >= 0 → the boss fires on the next tick
        zs.boss_template = Some(MonsterTemplate {
                resists: Vec::new(),
            weaknesses: Vec::new(),
            def_id: "boss_test".to_string(),
            bestiary_group: String::new(),
            hp: 500, atk: 40, def: 12,
            atk_min: 35, atk_max: 55,
            skills: vec!["self_destruct".to_string()],
            element: Element::Fire,
            toughness_max: 100,
            speed_milli: 0,
            atk_speed_milli: 0,
            atk_range: 1,
            drops: vec![],
            gear_grade_odds: Vec::new(),
            proj_travel_ticks: 0,
            xp_reward: 7, gil_reward: 5,
            stage: 0,
            boss_minions: 0,
            race: crate::grid::Race::Unknown,
            size: crate::grid::Size::Medium,
        });
    }
    tick(&mut s, 1);
    let boss = s
        .grid
        .entities
        .iter()
        .find(|e| e.is_boss)
        .expect("boss should have spawned");
    assert_eq!(boss.def_id, "boss_test");
    assert_eq!(boss.hp, 500);
    assert_eq!(boss.mob_atk_min, 35, "template atk_min must reach the entity");
    assert_eq!(boss.mob_atk_max, 55, "template atk_max must reach the entity");
    assert!(
        boss.skills.iter().any(|sk| sk.id == "self_destruct"),
        "template skills must be instantiated on the boss"
    );
}


/// AUDIT 2026-10-03: a shot whose firer died in flight still lands THROUGH the victim's shield.
#[test]
fn a_shot_from_a_dead_firer_is_caught_by_a_shield() {
    let mut s = world();
    s.grid.entities[1].shields.push(crate::grid::AbsorbShield { pool: 100, ..Default::default() });
    push_projectile(&mut s, 1000, 1, 5);
    s.projectiles[0].attacker_id = 777; // nobody: the firer is gone
    tick(&mut s, 1);
    assert_eq!(s.grid.entities[1].hp, 20, "the shield took it");
    assert_eq!(s.grid.entities[1].shields[0].pool, 95);
    assert!(s.recent_hits.iter().any(|h| h.target_id == 1000 && h.kind == crate::state::HitKind::Guard), "shown as Guarded");
}
