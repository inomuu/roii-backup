use super::*;
use crate::grid::{Cell, DamageType, Entity, Team};
use crate::skills::SkillState;
use crate::state::GameState;

#[test]
fn toxic_chom_death_blast_damages_poisons_and_shoves() {
    use crate::skills::{buff_sum, BuffKind, SkillState};
    let mut s = GameState::new(1, 16, 8);
    // A player standing right next to the bomb.
    let mut player = Entity::idle(1, Cell::new(5, 4)).with_team(Team::Player);
    player.hp = 500;
    player.max_hp = 500;
    s.grid.spawn(player);
    // A Toxic Chom carrying the on-death toxic blast passive.
    let mut chom = Entity::idle(1000, Cell::new(4, 4)).with_team(Team::Monster);
    chom.hp = 1;
    chom.max_hp = 200;
    chom.skills = vec![SkillState::new("toxic_burst", 1)];
    s.grid.spawn(chom);

    // Death SCHEDULES the blast (via the on-death blast passive) — doesn't fire yet.
    s.grid.entities[1].alive = false;
    handle_monster_death(&mut s, 1, None);
    assert_eq!(s.pending_explosions.len(), 1, "toxic_burst schedules one blast");
    let pe = s.pending_explosions[0];
    assert!(pe.poison_ticks > 0, "the blast carries poison");
    assert_eq!(pe.push, 1, "shoves 1 cell");
    assert_eq!(pe.dmg, 5, "flat 5 HP from power_bp");

    // A plain ChomBomb (self_destruct) schedules the same blast WITHOUT poison.
    let mut bomb = Entity::idle(1001, Cell::new(10, 4)).with_team(Team::Monster);
    bomb.skills = vec![SkillState::new("self_destruct", 1)];
    s.grid.spawn(bomb);
    s.grid.entities[2].alive = false;
    handle_monster_death(&mut s, 2, None);
    assert_eq!(s.pending_explosions[1].poison_ticks, 0, "self_destruct never poisons");

    // Fire the toxic blast after its delay.
    s.tick = pe.fire_tick;
    fire_pending_explosions(&mut s);
    let player = &s.grid.entities[0];
    assert!(player.hp < 500, "blast damaged the player");
    assert!(buff_sum(player, BuffKind::Poison) > 0, "player was poisoned by the cloud");
    assert_ne!(player.pos, Cell::new(5, 4), "player was knocked back");
}

#[test]
fn burst_chom_5x5_reaches_two_cells_and_hits_for_20() {
    use crate::skills::SkillState;
    let mut s = GameState::new(1, 16, 8);
    // A victim TWO cells from the bomb — inside a 5×5 (radius 2), OUTSIDE a 3×3.
    let mut victim = Entity::idle(1, Cell::new(6, 4)).with_team(Team::Player);
    victim.hp = 500;
    victim.max_hp = 500;
    s.grid.spawn(victim);
    let mut burst = Entity::idle(1000, Cell::new(4, 4)).with_team(Team::Monster);
    burst.skills = vec![SkillState::new("burst_blast", 1)];
    s.grid.spawn(burst);

    s.grid.entities[1].alive = false;
    handle_monster_death(&mut s, 1, None);
    let pe = s.pending_explosions[0];
    assert_eq!(pe.radius, 2, "burst_blast is 5×5");
    assert_eq!(pe.dmg, 20, "burst_blast hits for a flat 20");
    assert_eq!(pe.poison_ticks, 0, "burst_blast is not toxic");

    s.tick = pe.fire_tick;
    fire_pending_explosions(&mut s);
    let victim = &s.grid.entities[0];
    assert_eq!(victim.hp, 480, "the 2-cell-out victim took the flat 20");
    assert_ne!(victim.pos, Cell::new(6, 4), "and was knocked back");
}

#[test]
fn rechum_death_heals_ally_monsters_not_enemies() {
    use crate::skills::SkillState;
    let mut s = GameState::new(1, 16, 8);
    // A hurt player next to the bomb — must NOT be healed (heal is ally-only).
    let mut player = Entity::idle(1, Cell::new(5, 4)).with_team(Team::Player);
    player.hp = 200;
    player.max_hp = 500;
    s.grid.spawn(player);
    // A hurt ally monster 2 cells out — inside the 5×5 heal.
    let mut ally = Entity::idle(1000, Cell::new(6, 4)).with_team(Team::Monster);
    ally.hp = 100;
    ally.max_hp = 560;
    s.grid.spawn(ally);
    // Rechum at the centre.
    let mut rechum = Entity::idle(1001, Cell::new(4, 4)).with_team(Team::Monster);
    rechum.skills = vec![SkillState::new("mending_burst", 1)];
    s.grid.spawn(rechum);

    s.grid.entities[2].alive = false;
    handle_monster_death(&mut s, 2, None);
    let pe = s.pending_explosions[0];
    assert_eq!(pe.heal, 50, "mending_burst heals 50");
    assert_eq!(pe.dmg, 0, "a heal blast deals no damage");

    s.tick = pe.fire_tick;
    fire_pending_explosions(&mut s);
    assert_eq!(s.grid.entities[1].hp, 150, "ally monster healed 100 -> 150");
    assert_eq!(s.grid.entities[0].hp, 200, "the enemy player was NOT healed");
}

/// Dark Ambush targets the NEWEST non-boss enemy in reach (highest spawn id),
/// skipping bosses and out-of-range mobs.
fn assassin_with_dark_ambush() -> GameState {
    let mut s = GameState::new(1, 32, 8);
    let mut sin = Entity::idle(1, Cell::new(0, 0)).with_team(Team::Player);
    sin.atk_type = DamageType::Physical;
    sin.skills = vec![SkillState::new("dark_ambush", 1)];
    sin.active_skills = vec!["dark_ambush".to_string()];
    s.grid.spawn(sin);
    s
}

#[test]
fn cleave_splashes_orthogonal_neighbors_only() {
    // A Swordmaster with Cleave Lv10 hits a target; the splash lands on the
    // target's 4-neighbours (orthogonal), NOT on a diagonal neighbour.
    let mut s = GameState::new(1, 16, 8);
    let mut sm = Entity::idle(1, Cell::new(1, 2)).with_team(Team::Player);
    sm.atk_type = DamageType::Physical;
    sm.atk = 100;
    sm.atk_range = 1;
    sm.skills = vec![SkillState::new("cleave", 10)];
    s.grid.spawn(sm);
    let mut tgt = Entity::idle(1000, Cell::new(2, 2)).with_team(Team::Monster);
    tgt.hp = 100_000; tgt.max_hp = 100_000;
    s.grid.spawn(tgt);
    for (id, x, y) in [(1001u32, 2, 1), (1002, 2, 3)] {   // orthogonal to (2,2)
        let mut m = Entity::idle(id, Cell::new(x, y)).with_team(Team::Monster);
        m.hp = 100_000; m.max_hp = 100_000;
        s.grid.spawn(m);
    }
    let mut diag = Entity::idle(1003, Cell::new(3, 3)).with_team(Team::Monster); // diagonal
    diag.hp = 100_000; diag.max_hp = 100_000;
    s.grid.spawn(diag);

    // Force a basic swing on the target (idx 1), bypassing auto-target ties.
    resolve_instant(&mut s, 0, 1, None);
    let dealt = 100_000 - s.grid.entities[1].hp;
    assert!(dealt > 0, "main hit landed");
    let expect = dealt * 7500 / 10_000;   // Cleave Lv10 = 75%
    assert_eq!(100_000 - s.grid.entities[2].hp, expect, "orthogonal neighbour splashed");
    assert_eq!(100_000 - s.grid.entities[3].hp, expect, "other orthogonal neighbour splashed");
    assert_eq!(s.grid.entities[4].hp, 100_000, "diagonal neighbour NOT splashed");
}

#[test]
fn dark_ambush_ignores_out_of_range_and_bosses() {
    let mut s = assassin_with_dark_ambush();
    // Only a far non-boss (out of range 9) and a near boss.
    let mut far = Entity::idle(1001, Cell::new(20, 0)).with_team(Team::Monster);
    far.hp = 500; far.max_hp = 500;
    s.grid.spawn(far);
    let mut boss = Entity::idle(1002, Cell::new(2, 0)).with_team(Team::Monster);
    boss.hp = 500; boss.max_hp = 500; boss.is_boss = true;
    s.grid.spawn(boss);
    // No valid (non-boss, in-range) add → no Dark Ambush target.
    assert!(find_newest_nonboss_enemy_in_range(&s, 0, 9).is_none());
}

/// A channelled cast stores a SLOT INDEX into `skills`, and `clear_exemplar_skill` shrinks that
/// list. Two ways that goes wrong: the index lands past the end (a panic) or, quieter, on a
/// DIFFERENT skill that then fires. Every other path that invalidates a cast drops it — death
/// (`tick/mod.rs`), warp/revive (`build/zone.rs`), being lifted (`handlers.rs`); this one did not.
#[test]
fn unequipping_an_exemplar_drops_the_windup_that_indexed_its_skills() {
    let mut s = crate::tick::tests::world();
    let e = &mut s.grid.entities[0];
    for id in ["heal", "blessing", "kyrie"] {
        let mut sk = SkillState::new(id, 1);
        sk.from_exemplar = true;
        e.skills.push(sk);
    }
    e.skills.push(SkillState::new("fire_bolt", 1)); // learned AFTER the grants -> slot 3
    e.casting = Some(crate::grid::Casting {
        ticks_left: 4,
        ticks_total: 10,
        slot: crate::skills::SkillSlot::new(3),
        target_id: 1000,
        ground_cell: None,
    });

    e.clear_exemplar_skill();

    assert_eq!(e.skills.len(), 1, "the three granted skills are gone");
    assert!(e.casting.is_none(), "the windup indexing them must go with them");
}

/// The other half, and the one that matters for the SERVER: `Casting` is serialized, so a save
/// written before the fix above — or any state whose slot drifted — arrives with the stale index
/// already in it, and `clear_exemplar_skill` never runs on that path. verify-server links
/// sim-core and REPLAYS uploaded saves under `panic = "abort"`, so an index here is not a failed
/// request, it is the process.
#[test]
fn a_windup_slot_past_the_end_fizzles_instead_of_panicking() {
    let mut s = crate::tick::tests::world();
    s.grid.entities[0].skills.push(SkillState::new("fire_bolt", 1));
    s.grid.entities[0].casting = Some(crate::grid::Casting {
        ticks_left: 0,
        ticks_total: 10,
        slot: crate::skills::SkillSlot::new(99), // as if loaded from a save
        target_id: 1000,
        ground_cell: None,
    });

    fire_pending_cast(&mut s, 0);

    assert!(s.grid.entities[0].casting.is_none(), "the cast is consumed, not left to retry");
}
