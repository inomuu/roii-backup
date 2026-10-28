//! Skill casts, buffs/DoTs, taunt/endure, and passive behaviors.

use super::*;

#[test]
fn buffs_age_and_poison_dot_damages_on_interval() {
    use crate::skills::{Buff, BuffKind};
    let mut s = world(); // player id1 @ (0,0) atk10 idle; monster id1000 hp20 @ (5,0)
    let mi = s.grid.idx_of_id(1000).unwrap();
    // Neither side moves/swings (speed 0, atk_speed 0), so only the DoT acts.
    s.grid.entities[mi].buffs.push(Buff { kind: BuffKind::AtkPct, magnitude: 1000, ticks: 25, source_id: 1 });
    s.grid.entities[mi].buffs.push(Buff { kind: BuffKind::PoisonDot, magnitude: 3, ticks: 100, source_id: 1 });
    // REGEN_INTERVAL_TICKS = 10 → exactly one poison interval in 10 ticks.
    tick(&mut s, 10);
    let mi = s.grid.idx_of_id(1000).unwrap();
    assert_eq!(s.grid.entities[mi].hp, 17, "one poison interval = 3 damage");
    let atk = s.grid.entities[mi].buffs.iter()
        .find(|b| matches!(b.kind, BuffKind::AtkPct)).expect("buff still live");
    assert_eq!(atk.ticks, 15, "buff aged 10 ticks (25 → 15)");
}

#[test]
fn move_speed_buff_speeds_movement() {
    use crate::skills::{Buff, BuffKind};
    fn run(buffed: bool) -> i32 {
        let mut s = world(); // player id1 @ (0,0), monster id1000
        s.grid.entities[1].alive = false; // no enemy → the player just walks
        s.grid.entities[0].speed_milli = 300;
        s.grid.entities[0].target = Cell::new(15, 0);
        if buffed {
            s.grid.entities[0].buffs.push(Buff {
                kind: BuffKind::MoveSpeedPct, magnitude: 25, ticks: 999, source_id: 1,
            });
        }
        tick(&mut s, 20);
        s.grid.entities[0].pos.x
    }
    assert!(run(true) > run(false), "Agility Up's +25% move speed should cover more ground");
}

#[test]
fn dead_monsters_are_capped_to_prevent_bloat() {
    let mut s = world(); // player id1, alive monster id1000
    for k in 0..100u32 {
        let mut m = Entity::idle(2000 + k, Cell::new(0, 0)).with_team(Team::Monster);
        m.alive = false;
        s.grid.spawn(m);
    }
    let before = s.grid.entities.len();
    tick(&mut s, 1);
    let dead = s.grid.entities.iter()
        .filter(|e| !e.alive && matches!(e.team, Team::Monster)).count();
    assert!(dead <= 64, "dead monsters should be capped, got {dead}");
    assert!(s.grid.entities.len() < before, "Vec should shrink");
    assert!(s.grid.idx_of_id(1).is_some(), "the player is never pruned");
}

#[test]
fn buff_expires_and_is_dropped() {
    use crate::skills::{Buff, BuffKind};
    let mut s = world();
    let mi = s.grid.idx_of_id(1000).unwrap();
    s.grid.entities[mi].buffs.push(Buff { kind: BuffKind::DefPct, magnitude: 5000, ticks: 3, source_id: 1 });
    tick(&mut s, 3);
    let mi = s.grid.idx_of_id(1000).unwrap();
    assert!(s.grid.entities[mi].buffs.is_empty(), "buff should expire after its duration");
}

#[test]
fn a_cast_takes_as_long_as_the_skill_says() {
    // The RO variable-cast curve is gone with the attributes it was made of
    // (0.0.87): a skill's cast time is a fixed number it names, and no amount of
    // DEX or INT shortens it.
    let e = crate::grid::Entity::idle(1, Cell::new(0, 0));
    let plain = crate::combat::effective_cast_time(&e, 30);
    assert_eq!(crate::combat::effective_cast_time(&e, 30), plain,
        "attributes must not shorten a cast");
    assert_eq!(plain, 30, "a 3-second cast takes 3 seconds");
}

#[test]
fn cast_retargets_when_its_target_dies_keeping_windup_progress() {
    use crate::skills::SkillState;
    // A slow caster (Fireball, 2.5 s) with two enemies in range: A (near, id1000) and B (far, id1001).
    let mut s = world(); // player id1 @ (0,0); monster id1000 @ (5,0)
    s.grid.entities[1].pos = Cell::new(2, 0); // A near
    s.grid.entities[1].hp = 1000;
    s.grid.entities[1].max_hp = 1000;
    let e = &mut s.grid.entities[0];
    e.skills = vec![SkillState::new("fireball", 1)];
    e.active_skills = vec!["fireball".to_string()];
    e.atk = 30;
    e.atk_type = crate::grid::DamageType::Magic;
    e.proj_travel_ticks = 2;
    e.atk_range = 6;
    e.atk_speed_milli = 1000;
    // B: farther, so A is the initial (nearest) target; B is the retarget after A dies.
    let mut b = Entity::idle(1001, Cell::new(4, 0))
        .with_team(Team::Monster)
        .with_combat(1000, 0, 0, 0, Element::Earth);
    b.max_hp = 1000;
    s.grid.spawn(b);

    // Begin the channel (locks onto A), then advance until only 1 windup tick remains.
    tick(&mut s, 1);
    let bi = s.grid.idx_of_id(1001).unwrap();
    assert_eq!(s.grid.entities[0].casting.as_ref().map(|c| c.target_id), Some(1000),
        "channel locks onto the near enemy A");
    let mut guard = 0;
    while s.grid.entities[0].casting.as_ref().map_or(0, |c| c.ticks_left) > 1 {
        tick(&mut s, 1);
        guard += 1;
        assert!(guard < 60, "windup should count down");
    }
    // A dies mid-cast with exactly ONE windup tick left.
    s.grid.entities[1].hp = 0;
    s.grid.entities[1].alive = false;
    let b_hp_before = s.grid.entities[bi].hp;

    // One more tick: the cast RETARGETS to B (A gone) and — because windup progress is
    // KEPT (not restarted) — the last tick fires THIS tick, launching the bolt at B.
    // (Reset progress would re-arm the full windup and fire NOTHING this tick.)
    tick(&mut s, 1);
    assert!(!s.projectiles.is_empty(),
        "the salvaged cast fires on the kept last windup tick (progress not restarted)");
    // Let the bolt fly its 2-tick travel and land on B.
    tick(&mut s, 3);
    assert!(s.grid.entities[bi].hp < b_hp_before,
        "the retargeted bolt lands on B, hp {} -> {}", b_hp_before, s.grid.entities[bi].hp);
}

#[test]
fn cast_abandons_when_no_enemy_remains() {
    use crate::skills::SkillState;
    // A lone enemy: when it dies mid-cast there is nothing to retarget → abandon cleanly.
    let mut s = world();
    s.grid.entities[1].pos = Cell::new(2, 0);
    s.grid.entities[1].hp = 1000;
    s.grid.entities[1].max_hp = 1000;
    let e = &mut s.grid.entities[0];
    e.skills = vec![SkillState::new("fireball", 1)];
    e.active_skills = vec!["fireball".to_string()];
    e.atk = 30;
    e.atk_type = crate::grid::DamageType::Magic;
    e.proj_travel_ticks = 2;
    e.atk_range = 6;
    e.atk_speed_milli = 1000;
    tick(&mut s, 1);
    assert!(s.grid.entities[0].casting.is_some(), "channelling");
    s.grid.entities[1].hp = 0;
    s.grid.entities[1].alive = false;
    tick(&mut s, 1);
    assert!(s.grid.entities[0].casting.is_none(), "no target anywhere → channel abandoned");
    assert!(s.projectiles.is_empty(), "nothing fired into the void");
}

#[test]
fn endure_grants_unconditional_vit_into_soft_def_and_max_hp() {
    let mut s = world(); // player id1 @ (0,0) — NOT mobbed; the VIT is always on.
    let base_soft = crate::combat::effective_soft_def(&s.grid.entities[0]);
    let base_hp = crate::combat::effective_max_hp(&s.grid.entities[0]);
    s.grid.entities[0].skills =
        vec![crate::skills::SkillState::new("endure", 10)]; // +10 VIT
    // Endure's toughness survived the attributes (0.0.86): it grants soft DEF and
    // MaxHP% directly instead of routing through a VIT nobody allocates.
    // 5 soft DEF per level at the x10 stat scale → Lv10 = +50.
    assert_eq!(
        crate::combat::effective_soft_def(&s.grid.entities[0]),
        base_soft + 50,
    );
    // ...and +1% MaxHP per level, so Lv10 Endure is visibly more HP.
    assert!(
        crate::combat::effective_max_hp(&s.grid.entities[0]) > base_hp,
        "Endure VIT must raise MaxHP ({} -> {})",
        base_hp,
        crate::combat::effective_max_hp(&s.grid.entities[0]),
    );
}

#[test]
fn endure_has_no_mobbed_penalty_left_to_resist() {
    use crate::combat::{mobbed_def, mobbed_def_resisted};
    use crate::skills::endure_mob_resist_pct;
    // 0.0.407 retired the crowd's DEF cut; Endure's resist reads the same DEF everyone keeps.
    assert_eq!(endure_mob_resist_pct(10), 50);
    assert_eq!(mobbed_def(100, 8), 100);
    assert_eq!(mobbed_def_resisted(100, 8, 50), 100);
    assert_eq!(mobbed_def(100, 22), 100);
    assert_eq!(mobbed_def_resisted(100, 22, 50), mobbed_def(100, 22));
}

#[test]
fn taunt_overrides_a_monsters_nearest_target() {
    let mut s = world(); // player id1 @ (0,0), monster id1000 @ (5,0)
    s.grid.entities[0].skills =
        vec![crate::skills::SkillState::new("taunt", 1)];
    // Monster within taunt reach (2) of the taunter, but a second player
    // sits CLOSER to it — without taunt the monster would pick that ally.
    s.grid.entities[1].pos = Cell::new(2, 0);
    s.grid.spawn(
        Entity::idle(2, Cell::new(3, 0))
            .with_team(Team::Player)
            .with_combat(100, 0, 0, 0, Element::Fire),
    );
    apply_taunt(&mut s);
    let mi = s.grid.idx_of_id(1000).unwrap();
    assert_eq!(s.grid.entities[mi].taunted_by, Some(1));
    // Aggro-lock duration is a FIXED 4s (40t) at every level...
    assert_eq!(s.grid.entities[mi].taunt_ticks, crate::skills::TAUNT_LOCK_TICKS);
    // ...while the cooldown scales DOWN with level (Lv1 = 100t / 10s).
    assert_eq!(s.grid.entities[0].skills[0].cooldown,
        crate::skills::taunt_cooldown_ticks(1));
    assert_eq!(crate::skills::taunt_cooldown_ticks(1), 100);
    assert_eq!(crate::skills::taunt_cooldown_ticks(7), 40);
    // The monster now targets the taunter (id1), not the nearer ally (id2).
    let t = crate::combat::find_enemy_idx(&s.grid.entities, mi).unwrap();
    assert_eq!(s.grid.entities[t].id, 1, "taunt beats nearest-target rule");
}

// --- Stun status (Sonic Blow) ---

/// A stunned entity takes NO action for the duration (immobilized).
#[test]
fn stun_skips_the_entity_turn() {
    use crate::skills::{Buff, BuffKind};
    // Build a monster that WOULD hit the player: adjacent, real ATK, swing clock.
    let setup = |stunned: bool| {
        let mut s = world();
        s.grid.entities[1].pos = Cell::new(1, 0); // adjacent to player @ (0,0)
        s.grid.entities[1].atk = 50;
        s.grid.entities[1].atk_speed_milli = 1000; // one swing/tick
        s.grid.entities[1].atk_range = 1;
        s.grid.entities[1].hp = 10_000; // survive
        s.grid.entities[1].max_hp = 10_000;
        if stunned {
            s.grid.entities[1].buffs.push(Buff {
                kind: BuffKind::Stun, magnitude: 1, ticks: 30, source_id: 1,
            });
        }
        let hp0 = s.grid.entities[0].hp;
        tick(&mut s, 1);
        hp0 - s.grid.entities[0].hp // damage dealt to the player
    };
    assert!(setup(false) > 0, "control: an un-stunned adjacent monster hits the player");
    assert_eq!(setup(true), 0, "a stunned monster deals no damage (skips its turn)");
}

/// Stun does NOT make the victim untargetable (unlike Hiding).
#[test]
fn stun_victim_is_still_targetable() {
    use crate::skills::{Buff, BuffKind};
    let mut s = world();
    s.grid.entities[1].pos = Cell::new(1, 0);
    s.grid.entities[1].buffs.push(Buff {
        kind: BuffKind::Stun, magnitude: 1, ticks: 30, source_id: 1,
    });
    assert_eq!(crate::combat::find_enemy_idx(&s.grid.entities, 0), Some(1),
        "a stunned enemy is still a valid target (unlike Hiding)");
}

/// Bosses are immune to Sonic Blow's stun: a Lv10 assassin pounding a boss
/// never lands a Stun (immune ⇒ guaranteed regardless of the RNG stream).
#[test]
fn boss_is_immune_to_sonic_blow_stun() {
    use crate::skills::{BuffKind, SkillState};
    let mut s = world();
    // Player → Lv10 Sonic Blow assassin, adjacent, swinging every tick.
    s.grid.entities[0].unit_def = "assassin_t2".to_string();
    s.grid.entities[0].skills = vec![SkillState::new("sonic_blow", 10)];
    s.grid.entities[0].active_skills = vec!["sonic_blow".to_string()];
    s.grid.entities[0].atk = 10;
    s.grid.entities[0].atk_speed_milli = 1000;
    s.grid.entities[0].atk_range = 1;
    // The target is a BOSS with huge HP, standing adjacent.
    s.grid.entities[1].pos = Cell::new(1, 0);
    s.grid.entities[1].is_boss = true;
    s.grid.entities[1].hp = 1_000_000;
    s.grid.entities[1].max_hp = 1_000_000;
    for _ in 0..300 {
        tick(&mut s, 1);
        assert!(s.grid.entities[1].alive, "boss should survive the test");
        assert_eq!(
            crate::skills::buff_sum(&s.grid.entities[1], BuffKind::Stun), 0,
            "a boss must never be stunned"
        );
    }
}

// --- After-cast delay (skill use-time lock) ---

/// A skill with `after_cast_delay_ticks` locks the caster — no basic attack
/// until the animation window elapses, THEN basics resume.
#[test]
fn after_cast_delay_locks_then_resumes() {
    use crate::skills::SkillState;
    let mut s = world();
    s.grid.entities[0].unit_def = "raplus".to_string();
    s.grid.entities[0].skills = vec![SkillState::new("veil_slash", 1)]; // motion 5t
    s.grid.entities[0].active_skills = vec!["veil_slash".to_string()];
    s.grid.entities[0].atk = 20;
    s.grid.entities[0].atk_speed_milli = 2000; // 2 swings/tick — would spam w/o lock
    s.grid.entities[0].atk_range = 1;
    s.grid.entities[1].pos = Cell::new(1, 0);
    s.grid.entities[1].hp = 1_000_000;
    s.grid.entities[1].max_hp = 1_000_000;

    // Tick 1: Veil Slash fires once and sets the after-cast lock.
    tick(&mut s, 1);
    let locked_until = s.grid.entities[0].act_locked_until;
    assert!(locked_until > s.tick, "after-cast delay should lock the caster");
    let hp_locked = s.grid.entities[1].hp;

    // While locked: the caster takes NO action → target HP frozen.
    while s.tick + 1 < locked_until {
        tick(&mut s, 1);
        assert_eq!(s.grid.entities[1].hp, hp_locked, "no action during the lock");
    }
    // The tick the lock expires: basics resume → target takes damage again.
    tick(&mut s, 1);
    assert!(s.grid.entities[1].hp < hp_locked, "basics resume once the lock ends");
}

/// Motion delay does NOT overlap the attack-recharge (0.0.481, user: a basic wind-up drawn over a
/// skill looked like a cancel): the accumulator is frozen through the lock, so at a slow ASPD the
/// first basic after it still needs its full wind-up.
#[test]
fn motion_delay_freezes_attack_recharge() {
    use crate::skills::SkillState;
    let mut s = world();
    s.grid.entities[0].unit_def = "raplus".to_string();
    s.grid.entities[0].skills = vec![SkillState::new("veil_slash", 1)]; // motion 5t
    s.grid.entities[0].active_skills = vec!["veil_slash".to_string()];
    s.grid.entities[0].atk = 20;
    s.grid.entities[0].atk_speed_milli = 250; // SLOW: 4 ticks per swing
    s.grid.entities[0].atk_acc = 1000;        // primed to cast on tick 1
    s.grid.entities[0].windup_cast = true;    // …a wind-up that opened with the skill ready
    s.grid.entities[0].atk_range = 1;
    s.grid.entities[1].pos = Cell::new(1, 0);
    s.grid.entities[1].hp = 1_000_000;
    s.grid.entities[1].max_hp = 1_000_000;

    tick(&mut s, 1); // Veil Slash fires; motion lock begins
    let locked_until = s.grid.entities[0].act_locked_until;
    assert!(locked_until > s.tick, "Veil Slash cast and locked");
    let acc_at_lock = s.grid.entities[0].atk_acc;
    let hp_locked = s.grid.entities[1].hp;
    while s.tick + 1 < locked_until {
        tick(&mut s, 1);
        assert_eq!(s.grid.entities[1].hp, hp_locked, "no damage during the lock");
        assert_eq!(s.grid.entities[0].atk_acc, acc_at_lock, "the accumulator is frozen");
    }
    // Unlocked: the wind-up resumes from where the cast left it (250) — two ticks land nothing,
    // the third swings. Charged through the lock, the first would have.
    assert_eq!(acc_at_lock, 250);
    for _ in 0..2 {
        tick(&mut s, 1);
        assert_eq!(s.grid.entities[1].hp, hp_locked, "still winding up");
    }
    tick(&mut s, 1);
    assert!(s.grid.entities[1].hp < hp_locked, "the basic lands after a full wind-up");
}

/// SWING-SLOT GATE (0.0.481): a cast outside `attack_step` (a party buff here) waits while a
/// swing is winding up, fires on the tick that swing would land, and takes its place.
#[test]
fn an_out_of_cycle_cast_waits_for_the_swing_and_replaces_it() {
    use crate::skills::SkillState;
    let mut s = world();
    s.grid.entities[0].unit_def = "sylvie".to_string();
    s.grid.entities[0].skills = vec![SkillState::new("arc_focus", 1)];
    s.grid.entities[0].active_skills = vec!["arc_focus".to_string()];
    s.grid.entities[0].atk = 20;
    s.grid.entities[0].atk_speed_milli = 250; // 4 ticks per swing
    s.grid.entities[0].atk_acc = 250;         // mid-swing
    s.grid.entities[0].windup_cast = true;    // a wind-up that opened with the buff ready
    s.grid.entities[0].atk_range = 1;
    s.grid.entities[1].pos = Cell::new(1, 0);
    s.grid.entities[1].hp = 1_000_000;
    s.grid.entities[1].max_hp = 1_000_000;
    let hp0 = s.grid.entities[1].hp;
    assert_eq!(crate::tick::preview_pending_skill(&s, s.grid.entities[0].id).as_deref(), Some("arc_focus"),
        "the wind-up names the cast it leads into");
    tick(&mut s, 1); // 500
    tick(&mut s, 1); // 750
    assert_eq!(s.grid.entities[0].skills[0].last_fired, 0, "not cast mid-swing");
    tick(&mut s, 1); // the swing's tick: the buff instead
    assert!(s.grid.entities[0].skills[0].last_fired > 0, "cast on the swing's tick");
    assert_eq!(s.grid.entities[1].hp, hp0, "and it took the swing's place");
}

// --- Poison Cloud (ground field) + classic Poison ---

#[test]
fn classic_poison_floors_at_25pct_and_cannot_kill() {
    use crate::skills::{Buff, BuffKind};
    let mut s = GameState::new(1, 16, 8);
    let mut mob = Entity::idle(1000, Cell::new(0, 0)).with_team(Team::Monster);
    mob.hp = 1000;
    mob.max_hp = 1000;
    mob.buffs.push(Buff { kind: BuffKind::Poison, magnitude: 1, ticks: 100_000, source_id: 0 });
    s.grid.spawn(mob);

    let floor = crate::combat::effective_max_hp(&s.grid.entities[0]) / 4;
    tick(&mut s, 500); // far more than enough to "kill" if poison could
    let e = &s.grid.entities[0];
    assert!(e.alive, "classic Poison can never kill");
    assert_eq!(e.hp, floor, "poison floors HP at 25% of effective MaxHP");
}

// --- DEBUG god mode ---

#[test]
fn god_mode_makes_players_invincible_and_topped_up() {
    use crate::skills::SkillState;
    let mut s = world();
    s.debug_god = true;
    // A monster that would normally chunk the player to death, adjacent.
    s.grid.entities[1].pos = Cell::new(1, 0);
    s.grid.entities[1].atk = 9_999;
    s.grid.entities[1].atk_speed_milli = 1000;
    s.grid.entities[1].atk_range = 1;
    s.grid.entities[1].hp = 10_000_000;
    s.grid.entities[1].max_hp = 10_000_000;
    // A skill stuck on cooldown — god should clear it.
    s.grid.entities[0].skills = vec![SkillState::new("bash", 1)];
    s.grid.entities[0].skills[0].cooldown = 50;

    let pmax = crate::combat::effective_max_hp(&s.grid.entities[0]);
    tick(&mut s, 30);
    let p = &s.grid.entities[0];
    assert!(p.alive, "god player never dies");
    assert_eq!(p.hp, pmax, "god player ends every tick at full HP");
    assert_eq!(p.skills[0].cooldown, 0, "god clears cooldowns");
    // The huge ATK buff makes the player one-shot-strong.
    assert!(crate::combat::effective_atk(p) > 1_000, "god grants massive ATK");
}

#[test]
fn god_mode_off_is_byte_identical_default() {
    // Sanity: the flag defaults off and a fresh state never sets it.
    let s = world();
    assert!(!s.debug_god, "god mode is off by default");
}

#[test]
fn subvenio_blinks_to_and_heals_a_critical_ally() {
    use crate::skills::SkillState;
    let mut s = world();
    s.grid.entities[0].skills =
        vec![SkillState::new("subvenio", 1), SkillState::new("heal", 5)];
    s.grid.entities[0].active_skills = vec!["subvenio".to_string()];
    let mut a = Entity::idle(2, Cell::new(10, 4)).with_team(Team::Player);
    a.max_hp = 1000;
    a.hp = 200; // 20% — below the 40% trigger
    s.grid.spawn(a);
    let ai = s.grid.idx_of_id(2).unwrap();
    assert!(try_cast_subvenio(&mut s, 0), "fires for an ally under 40% HP");
    assert!(s.grid.entities[ai].hp > 200, "the ally is healed");
    assert!(
        s.grid.entities[0].pos.chebyshev(Cell::new(10, 4)) <= 2,
        "the priest blinked into the ally's 5x5"
    );
}

#[test]
fn resurrection_revives_a_fallen_ally_once_per_charge() {
    use crate::skills::SkillState;
    let mut s = world();
    s.grid.entities[1].alive = false; // quiet the world monster
    let mut rez = SkillState::new("resurrection", 10); // Lv10 → 100% HP
    rez.charges_left = 1;
    s.grid.entities[0].skills = vec![rez];
    // Resurrection is an ACTIVE now (1 reserve slot) → must be slotted to fire.
    s.grid.entities[0].active_skills = vec!["resurrection".to_string()];
    let mut a = Entity::idle(2, Cell::new(1, 0)).with_team(Team::Player);
    a.max_hp = 1000;
    a.hp = 0;
    a.alive = false;
    s.grid.spawn(a);
    let ai = s.grid.idx_of_id(2).unwrap();
    // Grace delay (5 s = 50 ticks): NOT revived in the first few ticks.
    tick(&mut s, 10);
    assert!(!s.grid.entities[ai].alive, "still down during the rez grace delay");
    // Past the delay it pops back up, spending the charge.
    tick(&mut s, 45);
    assert!(s.grid.entities[ai].alive, "revived after the grace delay");
    assert!(s.grid.entities[ai].hp > 0);
    assert_eq!(s.grid.entities[0].skills[0].charges_left, 0, "charge spent");
    // No charge → a second death stays down (even past the delay).
    s.grid.entities[ai].alive = false;
    s.grid.entities[ai].hp = 0;
    tick(&mut s, 60);
    assert!(!s.grid.entities[ai].alive, "no charge left → no revive");
}

#[test]
fn a_downed_member_loses_debuffs_but_keeps_beneficial_buffs() {
    use crate::skills::{Buff, BuffKind};
    let mut s = world();
    s.grid.entities[1].alive = false; // quiet the world monster
    // A second LIVING player so downing the first isn't a full-party wipe.
    let mut p2 = Entity::idle(2, Cell::new(4, 4)).with_team(Team::Player);
    p2.max_hp = 100;
    p2.hp = 100;
    s.grid.spawn(p2);
    // Member 0: a debuff (Stun) + a beneficial buff (AtkPct), then it goes down.
    s.grid.entities[0].buffs.push(Buff { kind: BuffKind::Stun, magnitude: 1, ticks: 500, source_id: 9 });
    s.grid.entities[0].buffs.push(Buff { kind: BuffKind::AtkPct, magnitude: 10, ticks: 500, source_id: 9 });
    s.grid.entities[0].alive = false;
    s.grid.entities[0].hp = 0;
    tick(&mut s, 1);
    let has = |k: BuffKind| s.grid.entities[0].buffs.iter().any(|b| b.kind == k);
    assert!(!has(BuffKind::Stun), "debuff stripped when down");
    assert!(has(BuffKind::AtkPct), "beneficial buff kept");
}

#[test]
fn knock_back_shot_shoves_once_dominant_axis_then_diagonal() {
    use crate::skills::SkillState;
    let mut s = world();
    s.grid.entities[1].alive = false; // quiet the world monster
    // Archer at (0,0) with Knock Back Shot Lv5.
    s.grid.entities[0].pos = Cell::new(0, 0);
    s.grid.entities[0].skills = vec![SkillState::new("knock_back_shot", 5)];
    // Shallow angle (5,1): dominant axis → pushed HORIZONTALLY (y stays 1).
    let mut a = Entity::idle(2, Cell::new(5, 1)).with_team(Team::Monster);
    a.max_hp = 100_000;
    a.hp = 100_000;
    s.grid.spawn(a);
    // Near-45° (3,3): pushed DIAGONALLY.
    let mut b = Entity::idle(3, Cell::new(3, 3)).with_team(Team::Monster);
    b.max_hp = 100_000;
    b.hp = 100_000;
    s.grid.spawn(b);
    let (ai, bi) = (s.grid.idx_of_id(2).unwrap(), s.grid.idx_of_id(3).unwrap());
    // Roll the shove many times; it procs once per target then the marker blocks it.
    for _ in 0..400 {
        maybe_knock_back_shot(&mut s, Some(0), ai);
        maybe_knock_back_shot(&mut s, Some(0), bi);
    }
    // Shallow → dominant horizontal: x increased, y UNCHANGED (not diagonal).
    assert!(s.grid.entities[ai].kb_shot_hit, "shallow target shoved once");
    assert!(s.grid.entities[ai].pos.x > 5, "pushed horizontally away");
    assert_eq!(s.grid.entities[ai].pos.y, 1, "dominant-axis: y unchanged (not diagonal)");
    // 45° → diagonal: both axes increased.
    assert!(s.grid.entities[bi].kb_shot_hit, "diagonal target shoved once");
    assert!(s.grid.entities[bi].pos.x > 3 && s.grid.entities[bi].pos.y > 3, "pushed diagonally");
}

#[test]
fn pet_master_spawns_a_pet_that_attacks() {
    use crate::skills::SkillState;
    let mut s = world();
    s.grid.entities[1].alive = false; // quiet world monster
    s.grid.entities[0].pos = Cell::new(0, 0);
    s.grid.entities[0].skills = vec![SkillState::new("pet_master", 10)]; // 40 ATK
    // A monster right next to where the pet will spawn.
    let mut m = Entity::idle(2, Cell::new(2, 0)).with_team(Team::Monster);
    m.max_hp = 100_000;
    m.hp = 100_000;
    s.grid.spawn(m);
    let mi = s.grid.idx_of_id(2).unwrap();
    let hp0 = s.grid.entities[mi].hp;
    // Run long enough for the pet to spawn, close in, and bite a few times.
    tick(&mut s, 60);
    // A pet entity exists, owned by the player, untargetable.
    let pet = s.grid.entities.iter().find(|e| e.is_pet);
    assert!(pet.is_some(), "Pet Master spawned a pet");
    let pet = pet.unwrap();
    assert_eq!(pet.owner_id, 1, "pet owned by the player");
    assert!(pet.untargetable, "damage-pet is untargetable");
    // The monster took pet damage.
    assert!(s.grid.entities[mi].hp < hp0, "pet dealt damage to the monster");
}

#[test]
fn pet_attacks_in_a_swarm_not_just_the_nearest() {
    use crate::skills::SkillState;
    let mut s = world();
    s.grid.entities[1].alive = false;
    // Ranged owner that stays put; pet must do the work.
    s.grid.entities[0].pos = Cell::new(8, 8);
    s.grid.entities[0].skills = vec![SkillState::new("pet_master", 10)];
    s.grid.entities[0].atk_range = 0; // owner doesn't kill anything
    // Surround the owner with a ring of beefy monsters (a real swarm). The nearest
    // to the pet may be boxed in by the others — the pet must still bite SOMETHING.
    let ring = [
        (7, 7), (8, 7), (9, 7), (7, 8), (9, 8), (7, 9), (8, 9), (9, 9),
    ];
    let mut id = 2u32;
    for (x, y) in ring {
        let mut m = Entity::idle(id, Cell::new(x, y)).with_team(Team::Monster);
        m.max_hp = 10_000_000;
        m.hp = 10_000_000;
        s.grid.spawn(m);
        id += 1;
    }
    let total0: i64 = (2..id).map(|k| s.grid.entities[s.grid.idx_of_id(k).unwrap()].hp as i64).sum();
    tick(&mut s, 120);
    let total1: i64 = (2..id)
        .filter_map(|k| s.grid.idx_of_id(k))
        .map(|j| s.grid.entities[j].hp as i64)
        .sum();
    assert!(total1 < total0, "pet bit at least one monster in the swarm");
}

#[test]
fn pet_catches_a_moving_monster() {
    use crate::skills::SkillState;
    let mut s = world();
    s.grid.entities[1].alive = false;
    // Owner far from the monster; the monster will path toward the owner each tick,
    // so the pet must chase a MOVING target (the real-game case).
    s.grid.entities[0].pos = Cell::new(8, 8);
    s.grid.entities[0].skills = vec![SkillState::new("pet_master", 10)];
    s.grid.entities[0].atk_range = 0; // owner never kills it
    let mut m = Entity::idle(2, Cell::new(2, 2)).with_team(Team::Monster);
    m.max_hp = 10_000_000;
    m.hp = 10_000_000;
    m.atk_speed_milli = 100;
    s.grid.spawn(m);
    let hp0 = s.grid.entities[s.grid.idx_of_id(2).unwrap()].hp;
    tick(&mut s, 200);
    let mi = s.grid.idx_of_id(2).unwrap();
    assert!(s.grid.entities[mi].hp < hp0, "pet caught and bit the moving monster");
}

#[test]
fn snare_strike_roots_a_surviving_target_once() {
    use crate::skills::{BuffKind, SkillState, buff_sum};
    let mut s = world();
    s.grid.entities[1].alive = false;
    s.grid.entities[0].pos = Cell::new(0, 0);
    // Pet Master (to spawn the pet) + Snare Strike Lv10 (= 25%/bite).
    s.grid.entities[0].skills =
        vec![SkillState::new("pet_master", 10), SkillState::new("snare_strike", 10)];
    // Slot Snare Strike so it's active (reserve-gated like the design demands).
    s.grid.entities[0].active_skills = vec!["snare_strike".to_string()];
    // A beefy monster the pet can't kill, so it survives to be rooted.
    let mut m = Entity::idle(2, Cell::new(2, 0)).with_team(Team::Monster);
    m.max_hp = 10_000_000;
    m.hp = 10_000_000;
    s.grid.spawn(m);
    // Run long enough for many bites → the 25%/bite root is all but certain to land.
    tick(&mut s, 400);
    let mi = s.grid.idx_of_id(2).unwrap();
    assert!(s.grid.entities[mi].snare_used, "the target was snared (marked once)");
    let _ = buff_sum(&s.grid.entities[mi], BuffKind::Root); // root may be active or expired
    // The mark sticks for life — it never re-rolls, even after the root expires.
    tick(&mut s, 60);
    let mi = s.grid.idx_of_id(2).unwrap();
    assert!(s.grid.entities[mi].snare_used, "snare_used stays set (no re-snare)");
}

#[test]
fn traps_auto_place_on_the_perimeter_ring() {
    use crate::skills::SkillState;
    let mut s = world();
    s.grid.entities[1].alive = false;
    s.grid.entities[0].pos = Cell::new(8, 8);
    s.grid.entities[0].skills = vec![SkillState::new("land_mine", 5)];
    s.grid.entities[0].active_skills = vec!["land_mine".to_string()];
    // A distant threat (so placement fires) that won't reach a trap this soon.
    let mut m = Entity::idle(2, Cell::new(8, 0)).with_team(Team::Monster);
    m.max_hp = 10_000_000;
    m.hp = 10_000_000;
    s.grid.spawn(m);
    tick(&mut s, 1);
    assert!(!s.traps.is_empty(), "a trap was auto-placed");
    for t in &s.traps {
        assert_eq!(t.cell.chebyshev(Cell::new(8, 8)), 2, "trap sits on the 5×5 ring");
    }
}

#[test]
fn land_mine_fires_when_a_new_enemy_enters() {
    use crate::skills::SkillState;
    use crate::state::{Trap, TrapKind};
    let mut s = world();
    s.grid.entities[1].alive = false;
    s.grid.entities[0].pos = Cell::new(8, 8);
    s.grid.entities[0].skills = vec![SkillState::new("land_mine", 5)];
    let owner_id = s.grid.entities[0].id;
    // Manually arm a mine at a known cell (placement is random; this tests TRIGGER).
    s.traps.push(Trap {
        cell: Cell::new(8, 5),
        kind: TrapKind::LandMine,
        level: 5,
        owner_id,
        fuse: 0,
        triggered: false,
        seen_ids: Vec::new(),
    });
    // A monster starts OUTSIDE the 3×3, then steps in.
    let mut m = Entity::idle(2, Cell::new(8, 1)).with_team(Team::Monster);
    m.max_hp = 10_000_000;
    m.hp = 10_000_000;
    s.grid.spawn(m);
    tick(&mut s, 1); // baseline: nobody in the 3×3, mine stays armed
    assert_eq!(s.traps.len(), 1, "still armed before anyone enters");
    let mi = s.grid.idx_of_id(2).unwrap();
    s.grid.entities[mi].pos = Cell::new(8, 5); // STEP into the 3×3
    tick(&mut s, 1);
    assert!(s.traps.is_empty(), "Land Mine fired + was consumed when the mob stepped in");
}

#[test]
fn trap_under_a_sitting_enemy_waits_for_a_newcomer() {
    use crate::state::{Trap, TrapKind};
    let mut s = world();
    s.grid.entities[1].alive = false;
    s.grid.entities[0].pos = Cell::new(8, 8);
    let owner_id = s.grid.entities[0].id;
    // A monster already standing where the mine gets armed (it'll wander off toward
    // the owner). Armed UNDER it: seen_ids seeded with the sitter ⇒ no self-trigger.
    let mut sitter = Entity::idle(2, Cell::new(2, 2)).with_team(Team::Monster);
    sitter.max_hp = 10_000_000;
    sitter.hp = 10_000_000;
    s.grid.spawn(sitter);
    let sitter_cell = s.grid.entities[s.grid.idx_of_id(2).unwrap()].pos;
    s.traps.push(Trap {
        cell: sitter_cell,
        kind: TrapKind::LandMine,
        level: 5,
        owner_id,
        fuse: 0,
        triggered: false,
        seen_ids: vec![2],
    });
    tick(&mut s, 1);
    assert_eq!(s.traps.len(), 1, "a trap armed under a sitting enemy does NOT self-trigger");
}

#[test]
fn freezing_trap_freezes_a_newcomer_without_damage() {
    use crate::skills::{BuffKind, SkillState, buff_sum};
    use crate::state::{Trap, TrapKind};
    let mut s = world();
    s.grid.entities[1].alive = false;
    s.grid.entities[0].pos = Cell::new(8, 8);
    s.grid.entities[0].skills = vec![SkillState::new("freezing_trap", 5)];
    s.grid.entities[0].atk_range = 0; // owner can't break the freeze
    let owner_id = s.grid.entities[0].id;
    s.traps.push(Trap {
        cell: Cell::new(8, 5),
        kind: TrapKind::Freezing,
        level: 5,
        owner_id,
        fuse: 0,
        triggered: false,
        seen_ids: Vec::new(),
    });
    let mut m = Entity::idle(2, Cell::new(8, 1)).with_team(Team::Monster);
    m.max_hp = 10_000_000;
    m.hp = 10_000_000;
    s.grid.spawn(m);
    tick(&mut s, 1); // baseline outside the 3×3
    let mi = s.grid.idx_of_id(2).unwrap();
    s.grid.entities[mi].pos = Cell::new(8, 5); // step into the 3×3
    tick(&mut s, 1);
    let mi = s.grid.idx_of_id(2).unwrap();
    assert!(buff_sum(&s.grid.entities[mi], BuffKind::Freeze) > 0, "Freezing Trap froze the newcomer");
    assert_eq!(s.grid.entities[mi].hp, 10_000_000, "Freezing Trap deals no damage");
}
/// Combustion burn STACKING (mage fireball line B): re-applying adds the incoming
/// peak to the CURRENT (already-decayed) value and resets the 10 s duration —
/// user's example: peak 100 → after 5 s it reads 50 → a fresh 100 makes it 150.
#[test]
fn combustion_burn_stacks_onto_current_decayed_value() {
    let mut s = world();
    let idx = 1usize; // the world()'s monster
    s.grid.entities[idx].max_hp = 1_000_000;
    s.grid.entities[idx].hp = 1_000_000;
    // Peak = 20% × tier1 of a 500 hit = 100.
    crate::tick::apply_combustion_burn(&mut s, idx, 1, 500, 1);
    let burn = |s: &GameState| {
        s.grid.entities[idx].buffs.iter()
            .find(|b| matches!(b.kind, crate::skills::BuffKind::Burn)).cloned()
    };
    assert_eq!(burn(&s).unwrap().magnitude, 100, "peak = 20% × 500");
    // Simulate 5 s elapsed (ticks 100 → 50): current value = 100 × 50/100 = 50.
    s.grid.entities[idx].buffs.iter_mut()
        .find(|b| matches!(b.kind, crate::skills::BuffKind::Burn)).unwrap().ticks = 50;
    // Re-burn a fresh peak 100 → new peak = current(50) + 100 = 150, duration reset.
    crate::tick::apply_combustion_burn(&mut s, idx, 1, 500, 1);
    let b1 = burn(&s).unwrap();
    assert_eq!(b1.magnitude, 150, "stacks onto the current value: 50 + 100");
    assert_eq!(b1.ticks, crate::tick::BURN_DURATION_TICKS, "duration reset to a full 10 s");
}

/// Combustion burn DEALS damage each interval and DECAYS (front-loaded): the drop
/// on the first interval exceeds the drop on the next.
#[test]
fn combustion_burn_damages_and_decays_over_time() {
    let mut s = GameState::new(1, 16, 8);
    let mut mob = Entity::idle(1000, Cell::new(8, 4)).with_team(Team::Monster);
    mob.max_hp = 1_000_000; mob.hp = 1_000_000;
    s.grid.spawn(mob);
    // Peak = 20% × tier3 of a 5000 hit = 3000/s at the start.
    crate::tick::apply_combustion_burn(&mut s, 0, 1, 5000, 3);
    let hp = |s: &GameState| s.grid.entities[0].hp;
    let h0 = hp(&s);
    tick(&mut s, 10);
    let d1 = h0 - hp(&s);
    let h1 = hp(&s);
    tick(&mut s, 10);
    let d2 = h1 - hp(&s);
    assert!(d1 > 0, "burn dealt damage on the first interval");
    assert!(d2 > 0 && d2 < d1, "burn decays: interval 2 ({d2}) < interval 1 ({d1})");
}
#[test]
fn bestiary_tallies_by_group_and_claims_pay_once() {
    let mut s = world();
    s.grid.entities[1].def_id = "baby_pinkling".to_string();
    if let Some(zs) = s.zone_spawn.as_mut() {
        zs.pool[0].template.def_id = "baby_pinkling".to_string();
        zs.pool[0].template.bestiary_group = "pinkling".to_string();
    }
    s.monster_kills.insert("pinkling".to_string(), 999);
    let gil0 = s.gil();
    s.grid.entities[1].alive = false;
    crate::tick::combat_step::handle_monster_death(&mut s, 1, None);
    assert_eq!(s.monster_kills["pinkling"], 1_000, "kill pooled under the GROUP");
    assert_eq!(s.gil(), gil0, "no AUTO payout — rewards are claimed");
    // Premature claim (milestone 1 needs 10k) = silent no-op.
    crate::events::apply_input(&mut s, crate::events::InputEvent::ClaimBestiaryReward {
        group: "pinkling".to_string(), idx: 1 });
    assert_eq!(s.gil(), gil0);
    // Valid claim pays milestone 0 exactly once.
    let (_, gil_r, _) = crate::state::BESTIARY_MILESTONES[0];
    crate::events::apply_input(&mut s, crate::events::InputEvent::ClaimBestiaryReward {
        group: "pinkling".to_string(), idx: 0 });
    assert_eq!(s.gil(), gil0 + gil_r, "claim paid");
    assert_eq!(s.bestiary_claimed["pinkling"] & 1, 1);
    crate::events::apply_input(&mut s, crate::events::InputEvent::ClaimBestiaryReward {
        group: "pinkling".to_string(), idx: 0 });
    assert_eq!(s.gil(), gil0 + gil_r, "second claim is a no-op");
}

/// A Meal is only worth eating if the ward actually lands, and "the buff is present" proves
/// nothing about that — the resolver reads gear/card StatMods, and buffs were NOT a source
/// before 0.0.265. So MEASURE the thing the player feels: how long the poison sticks.
#[test]
fn a_meal_halves_how_long_poison_lasts() {
    use crate::skills::{apply_buff, Buff, BuffKind};
    let poison = || Buff {
        kind: BuffKind::Poison, magnitude: 1, ticks: 100, source_id: 9,
    };

    let mut bare = Entity::idle(1, Cell::new(0, 0)).with_team(Team::Player);
    apply_buff(&mut bare, poison());
    let unwarded = bare.buffs.iter().find(|b| b.kind == BuffKind::Poison).expect("poisoned").ticks;

    let mut fed = Entity::idle(1, Cell::new(0, 0)).with_team(Team::Player);
    apply_buff(&mut fed, Buff {
        kind: BuffKind::ResistPoisonBp, magnitude: 5_000, ticks: 72_000, source_id: 8,
    });
    apply_buff(&mut fed, poison());
    let warded = fed.buffs.iter().find(|b| b.kind == BuffKind::Poison).expect("poisoned").ticks;

    assert_eq!(unwarded, 100, "the fixture must poison for the full duration");
    assert_eq!(warded, 50, "50% ward should halve it, not shorten it by some amount");
}

/// XP is split across the party, and a DOWNED member used to be skipped — which punished the
/// member already behind and, worse, shrank the divisor so the survivors quietly earned MORE for
/// the same kill. Both halves are checked here because fixing one without the other is a buff to
/// dying, not a fix.
#[test]
fn a_downed_member_earns_the_same_share_as_the_living() {
    let mut s = world();
    // Two more players, so the party is 3 and the share is the 3-member rate either way.
    for id in [70u32, 71u32] {
        let e = Entity::idle(id, Cell::new(1, 1))
            .with_team(Team::Player)
            .with_combat(100, 10, 0, 100, Element::Physical);
        s.grid.spawn(e);
    }
    // The shared fixture's monster awards nothing; give it XP or the assertion below is vacuous.
    let mon0 = s.grid.entities.iter().position(|e| matches!(e.team, Team::Monster)).unwrap();
    s.grid.entities[mon0].xp_reward = 100;
    s.grid.entities[mon0].pos = Cell::new(1, 0);   // in reach; neither side has move speed
    let down_idx = s.grid.entities.iter().position(|e| e.id == 71).unwrap();
    s.grid.entities[down_idx].alive = false;

    let before: Vec<u64> = s.grid.entities.iter().map(|e| e.stats.xp).collect();
    // Kill the monster outright and let the tick award its XP.
    let mon = s.grid.entities.iter().position(|e| matches!(e.team, Team::Monster)).unwrap();
    s.grid.entities[mon].hp = 1;
    for _ in 0..40 {
        crate::tick::tick(&mut s, 1);
    }
    let gained: Vec<u64> = s
        .grid
        .entities
        .iter()
        .zip(before.iter())
        .map(|(e, b)| e.stats.xp.saturating_sub(*b))
        .collect();

    let living = gained[s.grid.entities.iter().position(|e| e.id == 70).unwrap()];
    let downed = gained[down_idx];
    assert!(living > 0, "the fixture must actually award XP or this proves nothing");
    assert_eq!(downed, living, "a downed member earns the same share");
}

/// The point of a RESERVED slot is that food never costs a healing slot, and that one Meal is
/// one Meal. The catalog Meals left the game (user 2026-10-02), so the Meal is a fixture now; the
/// phial below is still the real catalog item.
#[test]
fn a_provision_takes_its_own_slot_and_only_one_per_kind() {
    use crate::events::{apply_input, InputEvent};
    use crate::items::ProvisionKind;

    let d = crate::data::GameData::from_json_str(include_str!(
        concat!(env!("CARGO_MANIFEST_DIR"), "/../../data/game_data.json")
    ))
    .expect("catalog parses");
    let mut meal = crate::items::Item::new("meal_fixture", crate::items::ItemKind::Consumable, 0, 0, crate::items::Rarity::Common);
    meal.consumable = Some(crate::items::ConsumableSpec {
        provision: Some(ProvisionKind::Meal),
        trigger: crate::items::ConsumeTrigger::BuffMissing(crate::skills::BuffKind::AtkPct),
        effect: crate::items::ConsumeEffect::Buff { kind: crate::skills::BuffKind::AtkPct, magnitude: 100, ticks: 600 },
        cooldown: 50,
    });
    assert_eq!(
        meal.consumable.as_ref().and_then(|c| c.provision),
        Some(ProvisionKind::Meal),
        "the catalog must tag it, or it can never enter a reserved slot"
    );

    // ONE merged stack of 100, which is how the inventory actually holds a stackable — and the
    // reason a whole-stack move would have starved every member after the first.
    let mut s = world();
    let a = s.grid.entities[0].id;
    let mut stack = meal.clone();
    stack.quantity = 100;
    s.inventory.push(stack);
    let bar_before = s.grid.entities[0].consumables.len();

    apply_input(&mut s, InputEvent::AddProvision { entity_id: a, inv_idx: 0, qty: 10 });
    assert_eq!(s.grid.entities[0].provisions.len(), 1, "took the reserved slot");
    assert_eq!(s.grid.entities[0].provisions[0].item.quantity, 10, "moved exactly what was asked");
    assert_eq!(s.inventory[0].quantity, 90, "the REST stays in the bag");
    assert_eq!(
        s.grid.entities[0].consumables.len(), bar_before,
        "and did NOT consume an emergency slot — that is the whole reason it is reserved"
    );

    // A SECOND member can still be fed from the same stack. This is the case the whole split
    // exists for, so it is checked with a real second entity rather than inferred.
    let b = Entity::idle(77, Cell::new(2, 2)).with_team(Team::Player);
    s.grid.spawn(b);
    apply_input(&mut s, InputEvent::AddProvision { entity_id: 77, inv_idx: 0, qty: 10 });
    let bi = s.grid.entities.iter().position(|e| e.id == 77).unwrap();
    assert_eq!(s.grid.entities[bi].provisions[0].item.quantity, 10, "second member fed");
    assert_eq!(s.inventory[0].quantity, 80, "from the same shared stack");

    // Topping the SAME food up merges instead of swapping, or a slot could never be refilled.
    apply_input(&mut s, InputEvent::AddProvision { entity_id: a, inv_idx: 0, qty: 5 });
    assert_eq!(s.grid.entities[0].provisions.len(), 1, "still one slot");
    assert_eq!(s.grid.entities[0].provisions[0].item.quantity, 15, "topped up, not replaced");

    // Asking for more than the bag holds takes what is there rather than failing or over-drawing.
    apply_input(&mut s, InputEvent::AddProvision { entity_id: a, inv_idx: 0, qty: 9_999 });
    assert!(s.inventory.is_empty() || s.inventory[0].def_id != meal.def_id, "stack drained");
    assert_eq!(s.grid.entities[0].provisions[0].item.quantity, 90, "15 + the remaining 75");

    // A plain healing phial declares no kind, so it cannot squat in a reserved slot.
    let phial = crate::data::instantiate_item(d.item("hp_pot_1").expect("phial exists"));
    s.inventory.push(phial);
    let untagged_idx = s.inventory.len() - 1;
    apply_input(&mut s, InputEvent::AddProvision {
        entity_id: a, inv_idx: untagged_idx as u32, qty: 1,
    });
    assert_eq!(s.grid.entities[0].provisions.len(), 1, "an untagged consumable is refused");
}

#[test]
fn iron_chains_pulls_and_taunts_the_ring() {
    let mut s = world(); // player id1 @ (0,0); monster id1000 @ (5,0)
    s.grid.entities[1].pos = Cell::new(3, 0);
    s.grid.entities[1].hp = 5_000;
    s.grid.entities[1].max_hp = 5_000;
    // A second body in the ring, on the other axis, to prove the pull isn't
    // only correct along a straight line.
    let second = Entity::idle(1001, Cell::new(0, 3))
        .with_team(Team::Monster)
        .with_combat(5_000, 0, 0, 0, Element::Earth);
    s.grid.spawn(second);
    s.grid.entities[0].skills = vec![crate::skills::SkillState::new("iron_chains", 1)];
    s.grid.entities[0].active_skills = vec!["iron_chains".to_string()];
    s.grid.entities[0].atk = 50;
    s.grid.entities[0].atk_speed_milli = 1000;
    tick(&mut s, 1);

    let me = s.grid.entities[0].id;
    for id in [1000, 1001] {
        let j = s.grid.idx_of_id(id).unwrap();
        let e = &s.grid.entities[j];
        assert!(e.pos.chebyshev(Cell::new(0, 0)) < 3,
            "{id} should have been hauled closer, sits at {:?}", e.pos);
        assert_eq!(e.taunted_by, Some(me), "{id} should be locked onto the caster");
        assert!(e.taunt_ticks > 0, "{id} taunt should be running");
        assert!(e.hp < 5_000, "{id} should have taken the blow");
    }
    // Her own guard went up with the cast.
    assert!(s.grid.entities[0].buffs.iter().any(|b| b.kind == crate::skills::BuffKind::DefPct),
        "Iron Chains should raise her DEF");
}

#[test]
fn bulwark_scales_def_with_the_crowd_she_holds() {
    let mut s = world();
    s.grid.entities[0].skills = vec![crate::skills::SkillState::new("bulwark", 5)];
    let bare = crate::combat::effective_hard_def(&s.grid.entities[0]);
    // Holding three enemies: +2 DEF per level per body.
    s.grid.entities[0].taunt_hold = 3;
    let held = crate::combat::effective_hard_def(&s.grid.entities[0]);
    assert_eq!(held - bare, 20 * 5 * 3, "Bulwark should add 20 x level per body held");
    // The cap is what stops a room-wide pull becoming unbounded DEF.
    s.grid.entities[0].taunt_hold = 500;
    let capped = crate::combat::effective_hard_def(&s.grid.entities[0]);
    assert_eq!(capped - bare, crate::skills::BULWARK_DEF_CAP);
}

#[test]
fn aegis_shields_every_member_at_once() {
    let mut s = world(); // player id1 @ (0,0); monster id1000 alive
    s.grid.spawn(
        Entity::idle(2, Cell::new(1, 0))
            .with_team(Team::Player)
            .with_combat(200, 10, 0, 0, Element::Physical),
    );
    s.grid.entities[0].skills = vec![crate::skills::SkillState::new("aegis", 1)];
    s.grid.entities[0].active_skills = vec!["aegis".to_string()];
    s.grid.entities[0].max_hp = 1_000;
    // An Ultimate waits for its gauge, so an empty one fires nothing.
    tick(&mut s, 1);
    assert!(s.grid.entities[0].shields.is_empty(),
        "Aegis must not fire on an empty Limit Break gauge");
    s.grid.entities[0].limit_break = 1_000;
    tick(&mut s, 1);
    for id in [1, 2] {
        let j = s.grid.idx_of_id(id).unwrap();
        let e = &s.grid.entities[j];
        assert_eq!(e.shields.len(), 1, "member {id} should carry one Aegis shield");
        assert!(e.shields[0].pool > 0, "member {id} shield should hold something");
        assert!(e.shields[0].burst_dmg > 0, "an Aegis shield bursts when it breaks");
    }
}

#[test]
fn a_pull_is_not_recorded_as_a_knockback() {
    let mut s = world();
    s.grid.entities[1].pos = Cell::new(3, 0);
    s.grid.entities[1].hp = 5_000;
    s.grid.entities[1].max_hp = 5_000;
    s.grid.entities[0].skills = vec![crate::skills::SkillState::new("iron_chains", 1)];
    s.grid.entities[0].active_skills = vec!["iron_chains".to_string()];
    s.grid.entities[0].atk = 50;
    s.grid.entities[0].atk_speed_milli = 1000;
    tick(&mut s, 1);
    // The haul-in reports as a pull and NOTHING reports as a shove — the
    // distinction anything watching for a knockback depends on.
    assert!(s.pulls.contains(&1000), "the hauled enemy should be listed as pulled");
    assert!(s.knockbacks.is_empty(), "a pull must never register as a knockback");
}

/// A WIND-UP IS DECIDED WHEN IT BEGINS (0.0.483, user: the basic attack was cut): a skill that
/// comes ready halfway through a basic's wind-up does not take that swing — the basic lands and
/// the skill takes the next one. No `pending_skill` appears mid-wind-up.
#[test]
fn a_skill_ready_mid_windup_waits_for_the_next_swing() {
    use crate::skills::SkillState;
    let mut s = world();
    s.grid.entities[0].unit_def = "sylvie".to_string();
    let mut sk = SkillState::new("arc_focus", 1);
    sk.cooldown = 2; // comes ready two ticks into the wind-up
    s.grid.entities[0].skills = vec![sk];
    s.grid.entities[0].active_skills = vec!["arc_focus".to_string()];
    s.grid.entities[0].atk = 20;
    s.grid.entities[0].atk_speed_milli = 250; // 4 ticks per swing
    s.grid.entities[0].atk_acc = 0;
    s.grid.entities[0].atk_range = 1;
    s.grid.entities[1].pos = Cell::new(1, 0);
    s.grid.entities[1].hp = 1_000_000;
    s.grid.entities[1].max_hp = 1_000_000;
    let id = s.grid.entities[0].id;
    let hp0 = s.grid.entities[1].hp;
    for _ in 0..3 {
        tick(&mut s, 1);
        assert_eq!(crate::tick::preview_pending_skill(&s, id), None, "this wind-up stays a basic");
    }
    tick(&mut s, 1);
    assert!(s.grid.entities[1].hp < hp0, "the basic lands");
    assert_eq!(s.grid.entities[0].skills[0].last_fired, 0, "the skill did not take it");
    let hp1 = s.grid.entities[1].hp;
    tick(&mut s, 1); // the next wind-up opens
    assert_eq!(crate::tick::preview_pending_skill(&s, id).as_deref(), Some("arc_focus"),
        "the next wind-up opens with it ready");
    tick(&mut s, 3);
    assert!(s.grid.entities[0].skills[0].last_fired > 0, "and the skill takes the next swing");
    assert_eq!(s.grid.entities[1].hp, hp1, "in the basic's place");
}

/// SWING-SLOT GATE, the zero case (0.0.482): an ASPD that divides 1000 leaves `atk_acc` at
/// exactly 0 after a swing. With an enemy in reach that is still mid-fight — the cast waits a
/// full wind-up instead of cutting the strike's follow-through.
#[test]
fn a_cast_waits_even_when_the_accumulator_sits_at_zero() {
    use crate::skills::SkillState;
    let mut s = world();
    s.grid.entities[0].unit_def = "sylvie".to_string();
    s.grid.entities[0].skills = vec![SkillState::new("arc_focus", 1)];
    s.grid.entities[0].active_skills = vec!["arc_focus".to_string()];
    s.grid.entities[0].atk_speed_milli = 250;
    s.grid.entities[0].atk_acc = 0;
    s.grid.entities[0].atk_range = 1;
    s.grid.entities[1].pos = Cell::new(1, 0);
    s.grid.entities[1].hp = 1_000_000;
    s.grid.entities[1].max_hp = 1_000_000;
    let hp0 = s.grid.entities[1].hp;
    for _ in 0..3 {
        tick(&mut s, 1);
        assert_eq!(s.grid.entities[0].skills[0].cooldown, 0, "not cast during the wind-up");
    }
    tick(&mut s, 1);
    assert!(s.grid.entities[0].skills[0].cooldown > 0, "cast on the swing's tick");
    assert_eq!(s.grid.entities[1].hp, hp0, "in the swing's place");
}

/// IRON CHAINS TAKES THE FARTHEST (0.0.482): with three in the ring and a cap of two, the one
/// already at her feet is left and the two farther ones are hauled.
#[test]
fn iron_chains_hauls_the_farthest_two() {
    let mut s = world();
    s.grid.entities[1].pos = Cell::new(1, 0);
    for (id, x, y) in [(1001u32, 2, 2), (1002, 3, 3)] {
        let mut m = Entity::idle(id, Cell::new(x, y)).with_team(Team::Monster);
        m.hp = 1_000_000;
        m.max_hp = 1_000_000;
        s.grid.spawn(m);
    }
    s.grid.entities[1].hp = 1_000_000;
    s.grid.entities[1].max_hp = 1_000_000;
    s.grid.entities[0].skills = vec![crate::skills::SkillState::new("iron_chains", 1)];
    s.grid.entities[0].active_skills = vec!["iron_chains".to_string()];
    s.grid.entities[0].atk = 50;
    s.grid.entities[0].atk_speed_milli = 1000;
    tick(&mut s, 1);
    let hp = |id: u32| s.grid.entities[s.grid.idx_of_id(id).unwrap()].hp;
    assert_eq!(hp(1000), 1_000_000, "the adjacent one is left");
    assert!(hp(1001) < 1_000_000 && hp(1002) < 1_000_000, "the two farther ones are chained");
    let mut pulled = s.pulls.clone();
    pulled.sort_unstable();
    assert_eq!(pulled, vec![1001, 1002], "and hauled");
}

#[test]
fn toughness_breaks_after_enough_hits_and_faster_on_element() {
    // A target that survives long enough to be broken rather than killed.
    let mut s = world();
    s.grid.entities[1].hp = 1_000_000;
    s.grid.entities[1].max_hp = 1_000_000;
    s.grid.entities[1].toughness_max = 100;
    s.grid.entities[1].toughness = 100;
    s.grid.entities[1].element = crate::grid::Element::Earth;
    s.grid.entities[1].pos = Cell::new(1, 0);
    s.grid.entities[0].atk = 10;
    s.grid.entities[0].atk_range = 1;
    s.grid.entities[0].atk_speed_milli = 1000; // one swing per tick
    s.grid.entities[0].element = crate::grid::Element::Ice; // NOT a weakness of Earth
    tick(&mut s, 6);
    // WEAKNESS ONLY (0.0.326): an off-element attacker never touches the bar, however
    // long it swings. It is still doing damage — it just is not doing THIS.
    assert_eq!(s.grid.entities[1].toughness, 100, "an off-element hit must not chip");
    assert_eq!(s.grid.entities[1].break_ticks, 0, "and must not break");
    // Fire beats Earth: only that attacker moves the bar at all.
    let mut s2 = world();
    s2.grid.entities[1].hp = 1_000_000;
    s2.grid.entities[1].max_hp = 1_000_000;
    s2.grid.entities[1].toughness_max = 100;
    s2.grid.entities[1].toughness = 100;
    s2.grid.entities[1].element = crate::grid::Element::Earth;
    s2.grid.entities[1].pos = Cell::new(1, 0);
    s2.grid.entities[0].atk = 10;
    s2.grid.entities[0].atk_range = 1;
    s2.grid.entities[0].atk_speed_milli = 1000;
    s2.grid.entities[0].element = crate::grid::Element::Fire; // beats Earth
    tick(&mut s2, 6);
    // The weakness element has already BROKEN it: the gauge sits empty-and-rising
    // (0.0.324) and the target is stunned for the window it refills across.
    assert!(s2.grid.entities[1].break_ticks > 0, "a weakness element should have broken it");
    assert!(
        crate::skills::buff_sum(&s2.grid.entities[1], crate::skills::BuffKind::Stun) > 0,
        "a break stuns"
    );
    assert!(
        s2.grid.entities[1].toughness < 100,
        "the gauge refills ACROSS the stun, it does not snap back full: {}",
        s2.grid.entities[1].toughness
    );
    assert_eq!(s.grid.entities[1].break_ticks, 0, "the off-element attacker has not broken it");
    assert_eq!(
        crate::skills::buff_sum(&s.grid.entities[1], crate::skills::BuffKind::Stun), 0,
        "and so has not stunned it"
    );
}

#[test]
fn steel_break_chips_the_gauge_harder_than_a_plain_hit() {
    fn run(skill: Option<&str>) -> u16 {
        let mut s = world();
        s.grid.entities[1].hp = 1_000_000;
        s.grid.entities[1].max_hp = 1_000_000;
        s.grid.entities[1].toughness_max = 1_000;
        s.grid.entities[1].toughness = 1_000;
        s.grid.entities[1].element = crate::grid::Element::Earth;
        s.grid.entities[1].pos = Cell::new(1, 0);
        s.grid.entities[0].atk = 10;
        s.grid.entities[0].atk_range = 1;
        s.grid.entities[0].atk_speed_milli = 1000;
        s.grid.entities[0].element = crate::grid::Element::Fire; // Earth's weakness
        if let Some(id) = skill {
            s.grid.entities[0].skills = vec![crate::skills::SkillState::new(id, 1)];
            s.grid.entities[0].active_skills = vec![id.to_string()];
        }
        tick(&mut s, 1);
        s.grid.entities[1].toughness
    }
    let plain = run(None);
    let steel = run(Some("steel_break"));
    assert!(plain > steel,
        "Steel Break should take more of the gauge than a basic swing: {plain} vs {steel}");
    assert_eq!(1_000 - plain, crate::grid::TOUGHNESS_CHIP_ADVANTAGE,
        "a basic swing on the weakness takes exactly the baseline chip");
    assert_eq!(1_000 - steel, crate::grid::TOUGHNESS_CHIP_ADVANTAGE * 3,
        "Steel Break's x3 should be exactly three times that");
}

#[test]
fn ember_cascade_answers_a_break_and_paces_itself() {
    let mut s = world();
    // A target tough enough to break but not to die.
    s.grid.entities[1].hp = 1_000_000;
    s.grid.entities[1].max_hp = 1_000_000;
    s.grid.entities[1].toughness_max = 20;
    s.grid.entities[1].toughness = 20;
    s.grid.entities[1].pos = Cell::new(1, 0);
    s.grid.entities[0].skills = vec![
        crate::skills::SkillState::new("ember_cascade", 1),
    ];
    s.grid.entities[0].atk = 10;
    s.grid.entities[0].atk_range = 1;
    s.grid.entities[0].atk_speed_milli = 1000;
    tick(&mut s, 1);
    // The basic swing broke a 20 gauge, and the break queued her answer.
    assert!(
        !s.grid.entities[0].break_queue.is_empty() || s.grid.entities[0].break_cd > 0,
        "a break should have queued or already fired an ember"
    );
    // A dead target is dropped rather than fired at.
    s.grid.entities[0].break_queue = vec![9999];
    s.grid.entities[0].break_cd = 0;
    let hp_before = s.grid.entities[1].hp;
    tick(&mut s, 1);
    assert!(!s.grid.entities[0].break_queue.contains(&9999),
        "a target that no longer exists should be dropped from the queue");
    let _ = hp_before;
}

#[test]
fn fire_cat_meteor_hits_every_enemy_in_the_room() {
    let mut s = world();
    for (n, id) in [(1001, 1001), (1002, 1002)] {
        let _ = n;
        s.grid.spawn(
            Entity::idle(id, Cell::new(6, id as i32 % 5))
                .with_team(Team::Monster)
                .with_combat(9_999, 0, 0, 0, Element::Earth),
        );
    }
    s.grid.entities[1].hp = 9_999;
    s.grid.entities[1].max_hp = 9_999;
    s.grid.entities[0].skills = vec![crate::skills::SkillState::new("fire_cat_meteor", 1)];
    s.grid.entities[0].active_skills = vec!["fire_cat_meteor".to_string()];
    s.grid.entities[0].atk = 200;
    s.grid.entities[0].atk_speed_milli = 1000;
    s.grid.entities[0].limit_break = 1_300; // the gauge is the gate
    tick(&mut s, 1);
    for id in [1000, 1001, 1002] {
        let j = s.grid.idx_of_id(id).unwrap();
        assert!(s.grid.entities[j].hp < 9_999,
            "{id} should have been struck — the meteor reaches the whole room");
    }
}

#[test]
fn the_limit_break_gauge_fills_from_the_kit_that_owns_it_and_empties_on_use() {
    let mut s = world();
    s.grid.entities[1].pos = Cell::new(1, 0);
    s.grid.entities[1].hp = 1_000_000;
    s.grid.entities[1].max_hp = 1_000_000;
    s.grid.entities[1].atk = 5;
    s.grid.entities[1].atk_speed_milli = 1000;
    s.grid.entities[0].skills = vec![crate::skills::SkillState::new("aegis", 1)];
    s.grid.entities[0].active_skills = vec!["aegis".to_string()];
    s.grid.entities[0].hp = 1_000_000;
    s.grid.entities[0].max_hp = 1_000_000;
    s.grid.entities[0].atk = 1;
    s.grid.entities[0].atk_range = 1;
    s.grid.entities[0].atk_speed_milli = 1000;
    tick(&mut s, 5);
    let charged = s.grid.entities[0].limit_break;
    assert!(charged > 0, "trading blows should have charged the gauge");
    // Fill it the rest of the way and the Ultimate spends it.
    s.grid.entities[0].limit_break = 1_000;
    tick(&mut s, 1);
    assert!(!s.grid.entities[0].shields.is_empty(), "a full gauge should fire Aegis");
    assert!(s.grid.entities[0].limit_break < 100, "firing must empty the gauge");
    let id = s.grid.entities[0].id;
    assert!(s.ult_fx.iter().any(|f| f.caster_id == id && f.skill_id == "aegis"),
        "the cast must be announced in ult_fx for the renderer's cut-in");
    tick(&mut s, 1);
    assert!(s.ult_fx.is_empty(), "ult_fx is per-tick: cleared on the next tick");
    // A monster has no Ultimate, so nothing charges on it.
    assert_eq!(s.grid.entities[1].limit_break, 0, "monsters carry no gauge");
}

#[test]
fn charging_off_a_break_has_to_be_granted_by_a_skill() {
    // The ultimate alone gives no break-charge: that source belongs to the
    // Destroyer doctrine, and anyone else pays a skill slot for it.
    let ult_only = vec![crate::skills::SkillState::new("fire_cat_meteor", 1)];
    assert_eq!(crate::skills::lb_gain(&ult_only, |d| d.lb_on_break), 0,
        "an ultimate must not hand out break-charge by itself");
    // Ember Cascade is what grants it to Rapis, who is a Striker.
    let with_passive = vec![
        crate::skills::SkillState::new("fire_cat_meteor", 1),
        crate::skills::SkillState::new("ember_cascade", 1),
    ];
    assert!(crate::skills::lb_gain(&with_passive, |d| d.lb_on_break) > 0,
        "the passive is what earns the break-charge");
}

/// Rivus is a stream, not a beam: three touches down the party's worst-off, decaying. Two
/// things are easy to get wrong and both are pinned here — the order is HP PERCENT (missing
/// HP would always point at the biggest health bar in the party), and the bounces decay.
#[test]
fn rivus_bounces_down_the_party_by_percent_not_by_missing_hp() {
    use crate::tick::combat_step::cast::try_cast_rivus;
    let mut s = world();
    s.grid.entities[0].skills = vec![crate::skills::SkillState::new("rivus", 5)];
    s.grid.entities[0].active_skills = vec!["rivus".to_string()];
    s.grid.entities[0].max_hp = 100;
    s.grid.entities[0].hp = 90; // 90% — healthiest, gets the third bounce
    // A tank MISSING far more HP than anyone, but in the best shape but for the caster.
    let mut tank = Entity::idle(2, Cell::new(1, 0))
        .with_team(Team::Player)
        .with_combat(1000, 10, 0, 0, Element::Fire);
    tank.hp = 700; // 70%, missing 300
    s.grid.spawn(tank);
    let mut frail = Entity::idle(3, Cell::new(2, 0))
        .with_team(Team::Player)
        .with_combat(100, 10, 0, 0, Element::Fire);
    frail.hp = 20; // 20%, missing only 80 — but the one actually in trouble
    s.grid.spawn(frail);

    let hp = |s: &GameState, id: u32| s.grid.entities[s.grid.idx_of_id(id).unwrap()].hp;
    let before = [hp(&s, 3), hp(&s, 2), hp(&s, 1)];
    assert!(try_cast_rivus(&mut s, 0), "someone is missing health — it fires");
    let got = [
        hp(&s, 3) - before[0], // worst by percent → full heal
        hp(&s, 2) - before[1], // second → 70%
        hp(&s, 1) - before[2], // third → 50%
    ];
    assert!(got[0] > 0, "the frail ally is healed first, not the tank");
    assert_eq!(got[1], got[0] * 70 / 100, "second bounce is 70%");
    assert_eq!(got[2], got[0] * 50 / 100, "third bounce is 50%");
}

/// FONS VITAE (0.0.468): the ultimate is a party HEAL-OVER-TIME and a charge-rate window, cast
/// through the generic buff path on a full gauge — not an instant heal any more.
#[test]
fn fons_vitae_lays_a_party_hot_and_doubles_the_gauge_rate() {
    use crate::grid::{Cell, Element, Entity, Team};
    use crate::skills::{BuffKind, SkillState};
    let mut s = crate::tick::tests::world();
    s.grid.entities.retain(|e| matches!(e.team, Team::Player));
    let mut astier = Entity::idle(90, Cell::new(2, 4)).with_team(Team::Player).with_combat(1_000, 100, 0, 100, Element::Ice);
    astier.unit_def = "astier".into();
    astier.skills = vec![SkillState::new("unda_vitae", 1), SkillState::new("rivus", 1), SkillState::new("tidebless", 1), SkillState::new("fons_vitae", 1)];
    astier.active_skills = vec!["rivus".into(), "tidebless".into(), "fons_vitae".into()];
    astier.max_hp = 1_000;
    s.grid.spawn(astier);
    let mut ally = Entity::idle(91, Cell::new(3, 4)).with_team(Team::Player).with_combat(2_000, 100, 0, 100, Element::Fire);
    ally.max_hp = 2_000;
    ally.hp = 1_000;
    s.grid.spawn(ally);
    let mut m = Entity::idle(301, Cell::new(5, 4)).with_team(Team::Monster).with_combat(100_000, 10, 0, 50, Element::Earth);
    m.max_hp = 100_000;
    s.grid.spawn(m);
    let ai = s.grid.idx_of_id(90).unwrap();
    let li = s.grid.idx_of_id(91).unwrap();
    // The gauge is the gate (Tidebless is parked so only the ultimate is a candidate).
    s.grid.entities[ai].skills[2].cooldown = 100;
    assert!(!crate::tick::combat_step::cast::try_cast_buffs(&mut s, ai), "an empty gauge holds the tide");
    s.grid.entities[ai].limit_break = 1_000;
    assert!(crate::tick::combat_step::cast::try_cast_buffs(&mut s, ai));
    assert_eq!(s.grid.entities[ai].limit_break, 0, "the gauge empties on use");
    // Lv1: 5% of each ally's OWN max HP a second → 2,000 × 5% / 10 ticks = 10 per tick.
    let hot = |s: &crate::state::GameState, i: usize| s.grid.entities[i].buffs.iter().filter(|b| matches!(b.kind, BuffKind::HealHot)).map(|b| b.magnitude).sum::<i32>();
    assert_eq!(hot(&s, li), 10, "the ally's own max HP sets its tick");
    assert_eq!(hot(&s, ai), 5, "and hers sets hers");
    // And everyone charges twice as fast while it lasts.
    assert_eq!(crate::skills::buff_sum(&s.grid.entities[li], BuffKind::GaugeRatePct), 10_000);
}

/// LUCIELLE'S KIT (0.0.421): Lux Medica heals for a share of her ATK; Communio then heals the
/// allies that share her element or stand as Sentinels — and nobody else; Benedictio waits on
/// the Limit Break gauge and spends it.
#[test]
fn lucielle_heals_by_atk_communio_reaches_holy_and_sentinel_allies_and_the_ultimate_waits_on_its_gauge() {
    use crate::grid::{Cell, Element, Entity, Team};
    use crate::skills::SkillState;
    let mut s = crate::tick::tests::world();
    // Lucielle: ATK 1000 (no weapon), the kit at Lv5 / Lv1.
    let mut luc = Entity::idle(10, Cell::new(2, 2)).with_team(Team::Player).with_combat(5_000, 1_000, 0, 0, Element::Light);
    luc.unit_def = "lucielle".into();
    luc.skills = vec![SkillState::new("lux_medica", 5), SkillState::new("communio", 1), SkillState::new("benedictio_lucis", 1)];
    luc.active_skills = vec!["lux_medica".into(), "benedictio_lucis".into()];
    s.grid.spawn(luc);
    // A wounded Holy ally (Communio target by element), a wounded Sentinel (by doctrine), a
    // wounded Fire archer (neither).
    let mut holy = Entity::idle(11, Cell::new(3, 2)).with_team(Team::Player).with_combat(10_000, 10, 0, 0, Element::Light);
    holy.hp = 1_000;
    let mut sent = Entity::idle(12, Cell::new(4, 2)).with_team(Team::Player).with_combat(10_000, 10, 0, 0, Element::Physical);
    sent.unit_def = "seraphine".into();
    sent.hp = 2_000;
    let mut fire = Entity::idle(13, Cell::new(5, 2)).with_team(Team::Player).with_combat(10_000, 10, 0, 0, Element::Fire);
    fire.unit_def = "archer_t1".into();
    fire.hp = 3_000;
    s.grid.spawn(holy); s.grid.spawn(sent); s.grid.spawn(fire);
    let li = s.grid.idx_of_id(10).unwrap();
    let atk = crate::combat::effective_atk(&s.grid.entities[li]) as i64;
    assert!(crate::tick::combat_step::try_cast_support(&mut s, li), "Lux Medica casts on the most wounded");
    let hp = |s: &crate::state::GameState, id: u32| s.grid.entities[s.grid.idx_of_id(id).unwrap()].hp as i64;
    // The primary heal is Lv5 Lux Medica = 50% + 4 × 5% = 70% ATK, plus Communio's 10%.
    let medica = atk * 7_000 / 10_000;
    let communio = atk * 1_000 / 10_000;
    assert_eq!(hp(&s, 11), 1_000 + medica + communio, "the Holy ally took the heal and Communio");
    assert_eq!(hp(&s, 12), 2_000 + communio, "the Sentinel took Communio only");
    assert_eq!(hp(&s, 13), 3_000, "the Fire archer is outside Communio");
    // The ultimate: gauge empty → no cast; gauge full → cast, gauge spent, party buffed.
    assert!(!crate::tick::combat_step::try_cast_buffs(&mut s, li), "Benedictio waits on its gauge");
    s.grid.entities[li].limit_break = 1_000;
    assert!(crate::tick::combat_step::try_cast_buffs(&mut s, li));
    assert_eq!(s.grid.entities[li].limit_break, 0, "the gauge is spent");
    let fi = s.grid.idx_of_id(13).unwrap();
    assert!(s.grid.entities[fi].buffs.iter().any(|b| matches!(b.kind, crate::skills::BuffKind::AtkPct) && b.magnitude == 3_000),
            "+30% ATK at ultimate Lv1 reaches every member");
}

/// AVELINE'S KIT (0.0.424): the volley lands on the three lowest-HP enemies and no one else and
/// every hit stacks an Umbral Mark (5% Shadow-taken per hit, three deep); Eclipse Rain waits
/// on its gauge, prefers a boss in range, and otherwise lands on the densest knot.
#[test]
fn aveline_volleys_the_three_weakest_marks_what_she_hits_and_rains_on_the_boss_or_the_densest_knot() {
    use crate::grid::{Cell, Element, Entity, Team};
    use crate::skills::SkillState;
    let mut s = crate::tick::tests::world();
    s.grid.entities.retain(|e| matches!(e.team, Team::Player));
    let mut av = Entity::idle(20, Cell::new(2, 4)).with_team(Team::Player).with_combat(5_000, 1_000, 0, 0, Element::Dark);
    av.unit_def = "aveline".into();
    av.skills = vec![SkillState::new("umbra_fletch", 1), SkillState::new("nightfall_volley", 5), SkillState::new("eclipse_rain", 1)];
    av.active_skills = vec!["nightfall_volley".into(), "eclipse_rain".into()];
    av.atk_type = crate::grid::DamageType::Physical;
    s.grid.spawn(av);
    // Seven monsters in range: HP 9000 / 8000 / 7000 near the archer, and a denser four further away.
    for (id, x, y, hp) in [(101u32, 4i32, 4i32, 9_000i32), (102, 5, 4, 8_000), (103, 4, 5, 7_000), (104, 8, 8, 50_000), (105, 8, 9, 50_000), (106, 9, 8, 50_000), (107, 9, 9, 50_000)] {
        let m = Entity::idle(id, Cell::new(x, y)).with_team(Team::Monster).with_combat(hp, 0, 0, 0, Element::Earth);
        s.grid.spawn(m);
    }
    let ai = s.grid.idx_of_id(20).unwrap();
    assert!(crate::tick::combat_step::try_cast_volley(&mut s, ai), "the volley casts");
    let hp = |s: &crate::state::GameState, id: u32| s.grid.entities[s.grid.idx_of_id(id).unwrap()].hp;
    assert!(hp(&s, 103) < 7_000 && hp(&s, 102) < 8_000 && hp(&s, 101) < 9_000, "the three weakest are hit");
    assert_eq!(hp(&s, 104), 50_000, "the fourth enemy is untouched");
    let mark = |s: &crate::state::GameState, id: u32| crate::skills::shadow_vuln_bp(&s.grid.entities[s.grid.idx_of_id(id).unwrap()]);
    assert_eq!(mark(&s, 103), 500, "one hit = one 5% mark");
    assert_eq!(mark(&s, 104), 0);
    // Four more volleys: the mark climbs to 15% and stops.
    for _ in 0..4 {
        s.grid.entities[ai].skills[1].cooldown = 0;
        crate::tick::combat_step::try_cast_volley(&mut s, ai);
    }
    assert_eq!(mark(&s, 103), 1_500, "three stacks is the cap");
    // Eclipse Rain: no gauge → no field; full gauge → the densest pair, not the nearest survivor.
    assert!(!crate::tick::combat_step::try_cast_ground_field(&mut s, ai), "waits on the gauge");
    s.grid.entities[ai].limit_break = 1_000;
    assert!(crate::tick::combat_step::try_cast_ground_field(&mut s, ai));
    assert_eq!(s.grid.entities[ai].limit_break, 0, "the gauge is spent");
    let f = s.ground_fields.last().expect("a field was placed");
    assert!(f.center.chebyshev(Cell::new(8, 8)) <= 1, "centred on the densest knot, got {:?}", f.center);
    // A boss in range wins over the crowd.
    let mut boss = Entity::idle(200, Cell::new(6, 1)).with_team(Team::Monster).with_combat(90_000, 0, 0, 0, Element::Earth);
    boss.is_boss = true;
    s.grid.spawn(boss);
    s.grid.entities[ai].limit_break = 1_000;
    s.grid.entities[ai].skills[2].cooldown = 0;
    assert!(crate::tick::combat_step::try_cast_ground_field(&mut s, ai));
    assert_eq!(s.ground_fields.last().unwrap().center, Cell::new(6, 1), "the boss takes priority");
}

/// RAPLUS'S KIT (0.0.426): Veil Slash lays Darkness (20%, halved on a boss) that feeds `avoid_bp`;
/// Umbral Exploit amplifies a packet by +10% per DISTINCT debuff kind; Umbral Requiem waits on its
/// gauge, then hits every enemy on the board and leaves each in Darkness and Bleed.
#[test]
fn raplus_veils_rends_and_her_requiem_reaches_every_enemy() {
    use crate::grid::{Cell, Element, Entity, Team};
    use crate::skills::{Buff, BuffKind, SkillState};
    let mut s = crate::tick::tests::world();
    s.grid.entities.retain(|e| matches!(e.team, Team::Player));
    let mut raplus = Entity::idle(30, Cell::new(2, 4)).with_team(Team::Player).with_combat(5_000, 1_000, 0, 0, Element::Dark);
    raplus.unit_def = "raplus".into();
    raplus.skills = vec![SkillState::new("umbral_exploit", 1), SkillState::new("veil_slash", 1), SkillState::new("rend", 1), SkillState::new("umbral_requiem", 1)];
    raplus.active_skills = vec!["veil_slash".into(), "rend".into(), "umbral_requiem".into()];
    raplus.atk_type = crate::grid::DamageType::Physical;
    s.grid.spawn(raplus);
    for (id, x, y, boss) in [(201u32, 3i32, 4i32, false), (202, 15, 8, false), (203, 18, 1, true)] {
        let mut m = Entity::idle(id, Cell::new(x, y)).with_team(Team::Monster).with_combat(50_000, 0, 0, 0, Element::Earth);
        m.is_boss = boss;
        s.grid.spawn(m);
    }
    let ni = s.grid.idx_of_id(30).unwrap();
    let idx = |s: &crate::state::GameState, id: u32| s.grid.idx_of_id(id).unwrap();
    // Darkness magnitude → avoid_bp, halved on a boss.
    let mi = idx(&s, 201);
    crate::skills::apply_buff(&mut s.grid.entities[mi], Buff { kind: BuffKind::Darkness, magnitude: 2_000, ticks: 80, source_id: 30 });
    assert_eq!(crate::combat::avoid_bp(&s.grid.entities[mi], &s.grid.entities[ni]), 2_000, "Darkness whiffs 20%");
    s.grid.entities[mi].buffs.clear();
    // Exploit: same packet, 0 / 1 / 2 distinct debuffs → ×1.0 / ×1.1 / ×1.2 (two Bleeds count once).
    let packet = |s: &crate::state::GameState| {
        let dmg = crate::combat::build_damage_scaled(&s.grid.entities[ni], false, 10_000, crate::grid::DamageType::Physical, false);
        crate::combat::apply_damage(&s.grid.entities[mi], &dmg, 0) as i64
    };
    let base = packet(&s);
    s.grid.entities[mi].buffs.push(Buff { kind: BuffKind::Bleed, magnitude: 1, ticks: 50, source_id: 30 });
    s.grid.entities[mi].buffs.push(Buff { kind: BuffKind::Bleed, magnitude: 1, ticks: 50, source_id: 31 });
    assert_eq!(packet(&s), base * 11_000 / 10_000, "+10% for one distinct debuff, two instances");
    s.grid.entities[mi].buffs.push(Buff { kind: BuffKind::Darkness, magnitude: 2_000, ticks: 80, source_id: 30 });
    assert_eq!(packet(&s), base * 12_000 / 10_000, "+20% for two distinct debuffs");
    s.grid.entities[mi].buffs.clear();
    // Requiem: no gauge → nothing; full gauge → every enemy on the board, each veiled and bleeding.
    assert!(!crate::tick::combat_step::try_cast_zone_wide(&mut s, ni), "waits on the gauge");
    s.grid.entities[ni].limit_break = 1_000;
    assert!(crate::tick::combat_step::try_cast_zone_wide(&mut s, ni));
    assert_eq!(s.grid.entities[ni].limit_break, 0, "the gauge is spent");
    for id in [201u32, 202, 203] {
        let e = &s.grid.entities[idx(&s, id)];
        assert!(e.hp < 50_000, "{id} took the requiem");
        // 0.0.464 (Laplus): the heist DAZZLES instead of veiling — same whiff, its own kind.
        let dazzle = crate::skills::effective_bp(e, BuffKind::Dazzle);
        assert_eq!(dazzle, if e.is_boss { 1_000 } else { 2_000 }, "{id} is dazzled (boss half)");
        assert!(crate::skills::buff_sum(e, BuffKind::Bleed) > 0, "{id} is rent");
    }
    // (the fixtures here carry no `gil_reward`, so the heist's purse is exercised in the Laplus test)
}

/// LAPLUS (0.0.464): Pickpocket deals nothing, dazzles (a hit-rate cut like Darkness, its own
/// kind) and pays 10% of the target's price at once; a monster she last-hits pays +20%.
#[test]
fn laplus_pickpockets_and_her_last_hits_pay_half_again() {
    use crate::grid::{Cell, Element, Entity, Team};
    use crate::skills::{Buff, BuffKind, SkillState};
    let mut s = crate::tick::tests::world();
    s.grid.entities.retain(|e| matches!(e.team, Team::Player));
    let mut raplus = Entity::idle(30, Cell::new(2, 4)).with_team(Team::Player).with_combat(5_000, 1_000, 0, 100, Element::Dark);
    raplus.unit_def = "raplus".into();
    raplus.skills = vec![SkillState::new("umbral_exploit", 1), SkillState::new("veil_slash", 1), SkillState::new("rend", 1), SkillState::new("umbral_requiem", 1)];
    raplus.active_skills = vec!["veil_slash".into(), "rend".into(), "umbral_requiem".into()];
    s.grid.spawn(raplus);
    let mut m = Entity::idle(201, Cell::new(3, 4)).with_team(Team::Monster).with_combat(50_000, 100, 0, 100, Element::Earth);
    m.max_hp = 50_000;
    m.gil_reward = 1_000;
    s.grid.spawn(m);
    let ni = s.grid.idx_of_id(30).unwrap();
    let mi = s.grid.idx_of_id(201).unwrap();
    // Dazzle whiffs like Darkness.
    crate::skills::apply_buff(&mut s.grid.entities[mi], Buff { kind: BuffKind::Dazzle, magnitude: 2_000, ticks: 80, source_id: 30 });
    assert_eq!(crate::combat::avoid_bp(&s.grid.entities[mi], &s.grid.entities[ni]), 2_000, "Dazzle whiffs 20%");
    s.grid.entities[mi].buffs.clear();
    // Pickpocket: 10% of the price, now.
    let g0 = s.gil();
    crate::tick::combat_step::pickpocket(&mut s, mi, 1_000);
    assert_eq!(s.gil() - g0, 100);
    // Her last hit pays 1.5× the price; someone else's pays 1×.
    let g0 = s.gil();
    s.grid.entities[mi].hp = 0;
    s.grid.entities[mi].alive = false;
    crate::tick::combat_step::handle_monster_death(&mut s, mi, Some(ni));
    assert_eq!(s.gil() - g0, 1_200, "Sticky Fingers: +20% on her own kill");
    let mut m2 = Entity::idle(202, Cell::new(4, 4)).with_team(Team::Monster).with_combat(50_000, 100, 0, 100, Element::Earth);
    m2.gil_reward = 1_000;
    m2.alive = false;
    s.grid.spawn(m2);
    let m2i = s.grid.idx_of_id(202).unwrap();
    let g0 = s.gil();
    crate::tick::combat_step::handle_monster_death(&mut s, m2i, None);
    assert_eq!(s.gil() - g0, 1_000, "a kill that is not hers pays the plain price");
}

/// ENRAGE (0.0.430): a monster gains +20% ATK at 30 s alive and +20% more every 15 s after,
/// uncapped; the party never does.
#[test]
fn monsters_enrage_after_thirty_seconds_and_keep_stacking() {
    use crate::grid::{Cell, Element, Entity, Team};
    use crate::skills::{buff_sum, BuffKind};
    let mut s = crate::tick::tests::world();
    let m = Entity::idle(300, Cell::new(9, 4)).with_team(Team::Monster).with_combat(1_000_000, 100, 0, 0, Element::Earth);
    s.grid.spawn(m);
    let mi = s.grid.idx_of_id(300).unwrap();
    let pi = s.grid.entities.iter().position(|e| matches!(e.team, Team::Player)).unwrap();
    for _ in 0..299 { crate::tick::upkeep::tick_enrage(&mut s); }
    assert_eq!(buff_sum(&s.grid.entities[mi], BuffKind::AtkPct), 0, "no enrage inside the grace window");
    crate::tick::upkeep::tick_enrage(&mut s);
    assert_eq!(buff_sum(&s.grid.entities[mi], BuffKind::AtkPct), 2_000, "+20% at 30 s");
    for _ in 0..150 { crate::tick::upkeep::tick_enrage(&mut s); }
    assert_eq!(buff_sum(&s.grid.entities[mi], BuffKind::AtkPct), 4_000, "+20% more at 45 s");
    for _ in 0..1500 { crate::tick::upkeep::tick_enrage(&mut s); }
    assert_eq!(buff_sum(&s.grid.entities[mi], BuffKind::AtkPct), 24_000, "no cap");
    assert_eq!(buff_sum(&s.grid.entities[pi], BuffKind::AtkPct), 0, "the party never enrages");
}

/// REGALIA PASSIVES (0.0.435): each signature's triggers fire on its role condition and are a
/// no-op for a wearer without the piece.
#[test]
fn signature_regalia_passives_fire_on_their_role_conditions() {
    use crate::grid::{Cell, Element, Entity, Team};
    use crate::items::Regalia;
    use crate::skills::{buff_sum, effective_bp, Buff, BuffKind};
    let mut s = crate::tick::tests::world();
    s.grid.entities.retain(|e| matches!(e.team, Team::Player));
    let wear = |id: &str, stars: u8| Some(Regalia { id: id.into(), stars, level: 1, ..Default::default() });
    let mut sera = Entity::idle(40, Cell::new(2, 4)).with_team(Team::Player).with_combat(5_000, 500, 0, 0, Element::Physical);
    sera.unit_def = "seraphine".into(); sera.regalia = wear("aurora_edge", 0);
    let mut lapis = Entity::idle(41, Cell::new(1, 4)).with_team(Team::Player).with_combat(5_000, 500, 0, 0, Element::Fire);
    lapis.unit_def = "lapis".into(); lapis.regalia = wear("empyrean", 4);
    let mut av = Entity::idle(42, Cell::new(0, 4)).with_team(Team::Player).with_combat(5_000, 1_000, 0, 0, Element::Dark);
    av.unit_def = "aveline".into(); av.regalia = wear("meridian", 0); av.atk_type = crate::grid::DamageType::Physical;
    av.is_ranged = true;
    let mut raplus = Entity::idle(43, Cell::new(3, 4)).with_team(Team::Player).with_combat(5_000, 1_000, 0, 0, Element::Dark);
    raplus.unit_def = "raplus".into(); raplus.regalia = wear("silent_verdict", 0);
    s.grid.spawn(sera); s.grid.spawn(lapis); s.grid.spawn(av); s.grid.spawn(raplus);
    for (id, x) in [(301u32, 6i32), (302, 7), (303, 8)] {
        s.grid.spawn(Entity::idle(id, Cell::new(x, 4)).with_team(Team::Monster).with_combat(10_000, 10, 0, 0, Element::Earth));
    }
    let idx = |s: &crate::state::GameState, id: u32| s.grid.idx_of_id(id).unwrap();
    // Rapis: +8% ATK per enemy at S1 (S5: +16%) → three enemies at S5 = +48% (0.0.488);
    // Seraphine: nothing until she holds a taunt.
    crate::regalia_passive::tick(&mut s);
    assert_eq!(buff_sum(&s.grid.entities[idx(&s, 41)], BuffKind::AtkPct), 4_800, "S5 Empyrean, three enemies");
    assert_eq!(buff_sum(&s.grid.entities[idx(&s, 42)], BuffKind::DmgReducePct), 0, "no taunt held yet");
    let si = idx(&s, 40);
    s.grid.entities[si].taunt_hold = 2;
    crate::regalia_passive::tick(&mut s);
    assert_eq!(buff_sum(&s.grid.entities[idx(&s, 42)], BuffKind::DmgReducePct), 1_000, "Aurora Edge guards the party while she holds the room");
    // Shield break on Seraphine → +60% ATK at S1.
    crate::regalia_passive::on_shield_break(&mut s, si);
    assert_eq!(buff_sum(&s.grid.entities[si], BuffKind::AtkPct), 6_000);
    // Break by Rapis → +30% ATK on top of the per-enemy line.
    let ri = idx(&s, 41);
    crate::regalia_passive::on_break(&mut s.grid.entities[ri]);
    // 0.0.488: the break pays DMG% (its own zone), not more ATK% on top of the per-enemy line.
    assert_eq!(buff_sum(&s.grid.entities[ri], BuffKind::AtkPct), 4_800, "ATK stays the per-enemy line");
    assert_eq!(buff_sum(&s.grid.entities[ri], BuffKind::DmgDealtPct), 3_000 + 750 * 4, "S5: +60% DMG after the break");
    // Aveline: a ranged attacker → +15% DMG; a debuffed target adds CRIT RATE +13% and CRIT DMG +30%
    // (0.0.488), not DMG.
    let ai = idx(&s, 42);
    let mi = idx(&s, 301);
    assert_eq!(crate::regalia_passive::hit_bonus_bp(&s.grid.entities[ai], &s.grid.entities[mi]), 1_500, "ranged, no debuff");
    s.grid.entities[mi].buffs.push(Buff { kind: BuffKind::Bleed, magnitude: 1, ticks: 50, source_id: 42 });
    assert_eq!(crate::regalia_passive::hit_bonus_bp(&s.grid.entities[ai], &s.grid.entities[mi]), 1_500, "the debuff moves to crit, not DMG");
    assert_eq!(crate::regalia_passive::crit_dmg_bonus_bp(&s.grid.entities[ai], &s.grid.entities[mi]), 3_000, "CRIT DMG +30% vs a debuffed target");
    assert_eq!(crate::regalia_passive::crit_rate_bonus_bp(&s.grid.entities[ai], &s.grid.entities[mi]), 1_300, "and CRIT RATE +13%");
    // Nyx: applying statuses stacks +12% taken to 36%; a target under half HP takes +50%.
    let ni = idx(&s, 43);
    for _ in 0..5 { crate::regalia_passive::on_status_applied(&mut s, ni, mi); }
    assert_eq!(effective_bp(&s.grid.entities[mi], BuffKind::DmgTakenPct), 3_600, "three stacks is the cap");
    assert_eq!(crate::regalia_passive::hit_bonus_bp(&s.grid.entities[ni], &s.grid.entities[mi]), 0, "target above half");
    s.grid.entities[mi].hp = 4_000;
    assert_eq!(crate::regalia_passive::hit_bonus_bp(&s.grid.entities[ni], &s.grid.entities[mi]), 5_000, "target below half");
    // Off-doctrine: Nyx (Catalyst) wearing Aveline's Meridian (Striker) takes its stats but no passive.
    s.grid.entities[ni].regalia = wear("meridian", 0);
    assert_eq!(crate::regalia_passive::hit_bonus_bp(&s.grid.entities[ni], &s.grid.entities[mi]), 0, "the passive asks for the doctrine");
    assert!(!s.grid.entities[ni].regalia.as_ref().unwrap().mods().is_empty(), "the stats never ask");
    // No piece → nothing.
    s.grid.entities[ni].regalia = None;
    assert_eq!(crate::regalia_passive::hit_bonus_bp(&s.grid.entities[ni], &s.grid.entities[mi]), 0);
}

/// NEMU (0.0.451): Sweet Poison stacks on every hit she lands — a field tick included — and
/// carries a move slow and an ASPD slow per stack; Heartstring's DoT heals her a tenth of each
/// tick; Abyssal Embrace hauls the board toward the hole a cell at a time, roots what it holds,
/// and detonates only when it closes.
#[test]
fn nemu_poisons_leeches_and_her_hole_draws_the_board_in_before_it_closes() {
    use crate::grid::{Cell, Element, Entity, Team};
    use crate::skills::{Buff, BuffKind, SkillState};
    let mut s = crate::tick::tests::world();
    s.grid.entities.retain(|e| matches!(e.team, Team::Player));
    let mut nemu = Entity::idle(40, Cell::new(2, 4)).with_team(Team::Player).with_combat(5_000, 1_000, 0, 100, Element::Dark);
    nemu.unit_def = "nemu".into();
    nemu.skills = vec![SkillState::new("sweet_poison", 1), SkillState::new("velvet_bog", 1), SkillState::new("heartstring", 1), SkillState::new("abyssal_embrace", 1)];
    nemu.active_skills = vec!["velvet_bog".into(), "heartstring".into(), "abyssal_embrace".into()];
    nemu.atk_type = crate::grid::DamageType::Magic;
    nemu.is_ranged = true;
    nemu.atk_range = 6;
    nemu.hp = 2_000;
    s.grid.spawn(nemu);
    for (id, x, y) in [(301u32, 6i32, 4i32), (302, 9, 4), (303, 12, 6)] {
        let mut m = Entity::idle(id, Cell::new(x, y)).with_team(Team::Monster).with_combat(1_000_000, 0, 0, 0, Element::Earth);
        m.max_hp = 1_000_000;
        m.speed_milli = 0;
        s.grid.spawn(m);
    }
    let ni = s.grid.idx_of_id(40).unwrap();
    let idx = |s: &crate::state::GameState, id: u32| s.grid.idx_of_id(id).unwrap();
    // 1. The mark: a landed hit stacks poison + both slows; the 11th hit caps at ten.
    let mi = idx(&s, 301);
    for _ in 0..11 {
        crate::combat::stamp_mark(&mut s.grid.entities, ni, mi);
    }
    let m = &s.grid.entities[mi];
    assert_eq!(crate::skills::effective_bp(m, BuffKind::UmbralMark), 3_000, "ten stacks × 3% Dark vuln");
    assert_eq!(crate::skills::buff_sum(m, BuffKind::MarkSlow), 30, "ten × 3% move slow");
    assert_eq!(crate::skills::buff_sum(m, BuffKind::MarkAspdSlow), 5_000, "ten × 5% ASPD slow");
    let plain_aspd = crate::combat::effective_atk_speed_milli(&Entity::idle(9, Cell::new(0, 0)).with_combat(1, 1, 0, 100, Element::Earth));
    assert!(crate::combat::effective_atk_speed_milli(m) < plain_aspd, "the slow reaches attack speed");
    s.grid.entities[mi].buffs.clear();
    // 2. Heartstring: the tether's tick heals her a tenth of what it dealt.
    let before_hp = s.grid.entities[ni].hp;
    s.grid.entities[mi].buffs.push(Buff { kind: BuffKind::LeechBleed, magnitude: 1_000, ticks: 60, source_id: 40 });
    s.tick = crate::tick::REGEN_INTERVAL_TICKS as u64 - 1;
    crate::tick::tick(&mut s, 1);
    assert_eq!(s.grid.entities[idx(&s, 301)].hp, 1_000_000 - 1_000, "the tick landed");
    assert_eq!(s.grid.entities[ni].hp, before_hp + 100, "and a tenth came back to her");
    let mi = idx(&s, 301);
    s.grid.entities[mi].buffs.clear();
    // 3. The hole: cast at the densest knot; over its life every enemy in reach is drawn one
    //    cell per interval and rooted, nothing detonates until it closes, then everything held
    //    takes the burst and is poisoned by it (a field tick is a landed hit).
    s.grid.entities[ni].limit_break = 1_000;
    let slot = crate::skills::SkillSlot::new(3);
    let center = s.grid.entities[idx(&s, 302)].pos;
    crate::tick::combat_step::cast::place_ground_field(&mut s, ni, slot, center);
    assert_eq!(s.ground_fields.len(), 1, "the hole is open");
    // Watch the FAR one (303, out of her basic's reach) so her swings do not muddy the read.
    let far_before = s.grid.entities[idx(&s, 303)].pos.chebyshev(center);
    let hp_before = s.grid.entities[idx(&s, 303)].hp;
    crate::tick::tick(&mut s, 25);
    let far_after = s.grid.entities[idx(&s, 303)].pos.chebyshev(center);
    assert!(far_after < far_before, "the far one was drawn in ({far_before} → {far_after})");
    assert!(s.grid.entities[idx(&s, 303)].buffs.iter().any(|b| matches!(b.kind, BuffKind::Root)), "and is held");
    assert_eq!(s.grid.entities[idx(&s, 303)].hp, hp_before, "nothing detonates while it lives");
    crate::tick::tick(&mut s, 10);
    assert!(!s.ground_fields.iter().any(|f| f.skill_id == "abyssal_embrace"), "closed (her Bog may still lie there)");
    assert!(s.grid.entities[idx(&s, 303)].hp < hp_before, "the burst landed on close");
    assert!(crate::skills::effective_bp(&s.grid.entities[idx(&s, 303)], BuffKind::UmbralMark) > 0, "the burst poisoned");
}

/// LUMI (0.0.456): Frost Aura on an ally stamps HER Chill on whatever hits that ally; Ice Mend
/// heals only herself, +1%/lv; Hoarfrost Crown heals the party a tenth of its max HP on her
/// ultimate — and only on a Sentinel.
#[test]
fn lumi_aura_chills_attackers_mends_herself_and_her_crown_heals_the_party_on_the_ult() {
    use crate::grid::{Cell, Element, Entity, Team};
    use crate::items::Regalia;
    use crate::skills::{Buff, BuffKind, SkillState};
    let mut s = crate::tick::tests::world();
    s.grid.entities.retain(|e| matches!(e.team, Team::Player));
    let mut lumi = Entity::idle(50, Cell::new(2, 4)).with_team(Team::Player).with_combat(5_000, 1_000, 0, 100, Element::Ice);
    lumi.unit_def = "lumi".into();
    lumi.skills = vec![SkillState::new("chill_touch", 1), SkillState::new("frost_taunt", 1), SkillState::new("ice_mend", 5), SkillState::new("frost_aura", 1)];
    lumi.active_skills = vec!["frost_taunt".into(), "ice_mend".into(), "frost_aura".into()];
    lumi.max_hp = 5_000;
    lumi.hp = 1_000;
    s.grid.spawn(lumi);
    let mut ally = Entity::idle(51, Cell::new(3, 4)).with_team(Team::Player).with_combat(4_000, 100, 0, 100, Element::Fire);
    ally.unit_def = "lapis".into();
    ally.max_hp = 4_000;
    ally.hp = 1_000;
    s.grid.spawn(ally);
    let mut m = Entity::idle(301, Cell::new(4, 4)).with_team(Team::Monster).with_combat(1_000_000, 500, 0, 100, Element::Earth);
    m.max_hp = 1_000_000;
    s.grid.spawn(m);
    let idx = |s: &crate::state::GameState, id: u32| s.grid.idx_of_id(id).unwrap();
    let (li, ai, mi) = (idx(&s, 50), idx(&s, 51), idx(&s, 301));
    // 1. Ice Mend heals HER, never the more-wounded ally: 10% + 1%/lv → Lv5 = 14% of 5,000.
    assert!(crate::tick::combat_step::cast::try_cast_support(&mut s, li));
    assert_eq!(s.grid.entities[li].hp, 1_000 + 700, "14% of her own max HP");
    assert_eq!(s.grid.entities[ai].hp, 1_000, "the ally is untouched");
    // 2. Frost Aura on the ally: a monster hit on the ally leaves Lumi's Chill on the monster.
    s.grid.entities[ai].buffs.push(Buff { kind: BuffKind::FrostAura, magnitude: 1, ticks: 300, source_id: 50 });
    let mut hits = Vec::new();
    crate::combat::resolve_attack(&mut s.grid.entities, &mut s.rng, mi, ai, &mut hits);
    let m = &s.grid.entities[mi];
    assert_eq!(crate::skills::effective_bp(m, BuffKind::FrostMark), 200, "one Chill stack: +2% Water taken");
    assert_eq!(crate::skills::effective_bp(m, BuffKind::UmbralMark), 0, "and no Dark vuln");
    assert_eq!(crate::skills::buff_sum(m, BuffKind::MarkSlow), 4);
    assert_eq!(crate::skills::buff_sum(m, BuffKind::MarkAspdSlow), 400);
    //    Friendly splash on the carrier chills nobody.
    let before = s.grid.entities[li].buffs.len();
    let ally_hp = s.grid.entities[ai].hp;
    crate::combat::resolve_attack(&mut s.grid.entities, &mut s.rng, li, ai, &mut hits);
    assert_eq!(s.grid.entities[li].buffs.len(), before, "an ally's hit on the carrier leaves no Chill on the ally");
    s.grid.entities[ai].hp = ally_hp;
    s.grid.entities[ai].alive = true;
    // 3. Hoarfrost Crown: her ult heals everyone 20% of THEIR max HP at S1; the aura + DEF land too.
    s.grid.entities[li].regalia = Some(Regalia { id: "hoarfrost_crown".into(), stars: 0, level: 1, ..Default::default() });
    s.grid.entities[li].limit_break = 1_000;
    s.grid.entities[li].skills[3].cooldown = 0;
    let (lh, ah) = (s.grid.entities[li].hp, s.grid.entities[ai].hp);
    assert!(crate::tick::combat_step::cast::try_cast_buffs(&mut s, li), "the ult fires on a full gauge");
    assert_eq!(s.grid.entities[li].hp, lh + 1_000, "20% of 5,000");
    assert_eq!(s.grid.entities[ai].hp, ah + 800, "20% of 4,000");
    assert_eq!(crate::skills::buff_sum(&s.grid.entities[ai], BuffKind::DefPct), 2_000, "DEF +20% at Lv1");
    assert!(s.grid.entities[ai].buffs.iter().any(|b| matches!(b.kind, BuffKind::FrostAura) && b.source_id == 50));
    // 4. The same crown on a Striker heals nothing (doctrine-gated).
    s.grid.entities[ai].regalia = Some(Regalia { id: "hoarfrost_crown".into(), stars: 0, level: 1, ..Default::default() });
    let ah = s.grid.entities[ai].hp;
    crate::regalia_passive::on_ult_cast(&mut s, ai);
    assert_eq!(s.grid.entities[ai].hp, ah);
}

/// AIRI (0.0.462): Static Shot lays a STATIC mark (+Thunder taken, never Dark or Water); Storm
/// Volley is a volley that waits on the gauge, spends it, and does not fire on cooldown alone.
#[test]
fn airi_marks_thunder_and_her_storm_waits_on_the_gauge() {
    use crate::grid::{Cell, Element, Entity, Team};
    use crate::skills::{BuffKind, SkillState};
    let mut s = crate::tick::tests::world();
    s.grid.entities.retain(|e| matches!(e.team, Team::Player));
    let mut airi = Entity::idle(60, Cell::new(2, 4)).with_team(Team::Player).with_combat(5_000, 1_000, 0, 100, Element::Thunder);
    airi.unit_def = "airi".into();
    airi.skills = vec![SkillState::new("static_shot", 1), SkillState::new("chain_bolt", 1), SkillState::new("overload", 1), SkillState::new("storm_volley", 1)];
    airi.active_skills = vec!["chain_bolt".into(), "overload".into(), "storm_volley".into()];
    airi.is_ranged = true;
    airi.atk_range = 6;
    s.grid.spawn(airi);
    let mut m = Entity::idle(301, Cell::new(5, 4)).with_team(Team::Monster).with_combat(1_000_000, 0, 0, 0, Element::Earth);
    m.max_hp = 1_000_000;
    s.grid.spawn(m);
    let ai = s.grid.idx_of_id(60).unwrap();
    let mi = s.grid.idx_of_id(301).unwrap();
    crate::combat::stamp_mark(&mut s.grid.entities, ai, mi);
    let m = &s.grid.entities[mi];
    assert_eq!(crate::skills::effective_bp(m, BuffKind::StaticMark), 200, "one Static stack: +2% Thunder taken");
    assert_eq!(crate::skills::effective_bp(m, BuffKind::UmbralMark), 0);
    assert_eq!(crate::skills::effective_bp(m, BuffKind::FrostMark), 0);
    // The chain (no gauge) fires on cooldown; the storm (gauge 1,000) does not until it is full.
    s.grid.entities[ai].skills[3].cooldown = 0;
    s.grid.entities[ai].skills[1].cooldown = 50;
    s.grid.entities[ai].limit_break = 0;
    assert!(!crate::tick::combat_step::cast::try_cast_volley(&mut s, ai), "storm waits on the gauge");
    s.grid.entities[ai].limit_break = 1_000;
    let hp = s.grid.entities[mi].hp;
    assert!(crate::tick::combat_step::cast::try_cast_volley(&mut s, ai));
    assert!(s.grid.entities[mi].hp < hp, "the storm landed");
    assert_eq!(s.grid.entities[ai].limit_break, 0, "and spent the gauge");
    // A Frost Mark on the target opens it to Thunder as much as to Water.
    s.grid.entities[mi].buffs.push(crate::skills::Buff { kind: BuffKind::FrostMark, magnitude: 600, ticks: 60, source_id: 50 });
    let static_bp = crate::skills::thunder_vuln_bp(&s.grid.entities[mi]);
    assert!(static_bp >= 200, "the storm stamped Static too");
    assert_eq!(crate::skills::water_vuln_bp(&s.grid.entities[mi]), 600, "Chill counts on a Thunder hit (apply_damage adds both)");
    // A taunted target takes +20% from EVERY hit of hers (the passive): the chain, taunt on vs
    // off, same seed.
    s.grid.entities[mi].buffs.clear();
    assert_eq!(crate::skills::taunted_target_bp(&s.grid.entities[ai], &s.grid.entities[mi]), 0);
    let mut plain = s.clone();
    plain.grid.entities[ai].skills[1].cooldown = 0;
    plain.grid.entities[ai].limit_break = 0;
    let hp0 = plain.grid.entities[mi].hp;
    assert!(crate::tick::combat_step::cast::try_cast_volley(&mut plain, ai));
    let plain_dealt = hp0 - plain.grid.entities[mi].hp;
    let mut held = s.clone();
    held.grid.entities[ai].skills[1].cooldown = 0;
    held.grid.entities[ai].limit_break = 0;
    held.grid.entities[mi].taunted_by = Some(50);
    held.grid.entities[mi].taunt_ticks = 30;
    let hp0 = held.grid.entities[mi].hp;
    assert!(crate::tick::combat_step::cast::try_cast_volley(&mut held, ai));
    let held_dealt = hp0 - held.grid.entities[mi].hp;
    assert_eq!(held_dealt, plain_dealt * 12 / 10, "taunted: +20% ({plain_dealt} → {held_dealt})");
    assert_eq!(crate::skills::taunted_target_bp(&held.grid.entities[ai], &held.grid.entities[mi]), 2_000);
}

/// CHARON (0.0.463): the oar chips at twice the baseline on a weak target; Blackwater Mark makes
/// a NON-weak target chip at half and a weak one at 1.5× (user's numbers: 40 → 20 / 60); the mark
/// picks the boss in range over a bigger non-boss; a broken target takes +30% from her.
#[test]
fn charon_marks_the_boss_and_her_seal_opens_a_bar_nobody_else_could_touch() {
    use crate::grid::{Cell, Element, Entity, Team};
    use crate::skills::{Buff, BuffKind, SkillState};
    let mut s = crate::tick::tests::world();
    s.grid.entities.retain(|e| matches!(e.team, Team::Player));
    let mut charon = Entity::idle(70, Cell::new(2, 4)).with_team(Team::Player).with_combat(5_000, 1_000, 0, 100, Element::Dark);
    charon.unit_def = "charon".into();
    charon.skills = vec![SkillState::new("ferrymans_toll", 1), SkillState::new("tolling_blow", 1), SkillState::new("blackwater_mark", 1), SkillState::new("passage_of_no_return", 1)];
    charon.active_skills = vec!["tolling_blow".into(), "blackwater_mark".into(), "passage_of_no_return".into()];
    s.grid.spawn(charon);
    let mut weak = Entity::idle(301, Cell::new(3, 4)).with_team(Team::Monster).with_combat(1_000_000, 0, 0, 0, Element::Earth);
    weak.max_hp = 1_000_000;
    weak.weaknesses = vec![Element::Dark];
    weak.toughness_max = 10_000;
    weak.toughness = 10_000;
    s.grid.spawn(weak);
    let mut hard = Entity::idle(302, Cell::new(3, 5)).with_team(Team::Monster).with_combat(1_000_000, 0, 0, 0, Element::Earth);
    hard.max_hp = 1_000_000;
    hard.toughness_max = 10_000;
    hard.toughness = 10_000;
    s.grid.spawn(hard);
    let ci = s.grid.idx_of_id(70).unwrap();
    let wi = s.grid.idx_of_id(301).unwrap();
    let hi = s.grid.idx_of_id(302).unwrap();
    let base = u16::from(crate::grid::TOUGHNESS_CHIP_ADVANTAGE);
    let mut hits = Vec::new();
    // Weak, no mark: the oar chips at ×2 (the passive's basic multiplier).
    crate::combat::resolve_attack(&mut s.grid.entities, &mut s.rng, ci, wi, &mut hits);
    assert_eq!(10_000 - s.grid.entities[wi].toughness, base * 2, "basic chips at twice the baseline on a weak target");
    // Not weak, no mark: nothing.
    crate::combat::resolve_attack(&mut s.grid.entities, &mut s.rng, ci, hi, &mut hits);
    assert_eq!(s.grid.entities[hi].toughness, 10_000, "a target not weak to Dark is untouched");
    // Marked: the non-weak target chips at HALF, the weak one at 1.5×.
    for j in [wi, hi] {
        s.grid.entities[j].buffs.push(Buff { kind: BuffKind::BlackwaterMark, magnitude: 5_000, ticks: 100, source_id: 70 });
    }
    s.grid.entities[wi].toughness = 10_000;
    crate::combat::resolve_attack(&mut s.grid.entities, &mut s.rng, ci, hi, &mut hits);
    assert_eq!(10_000 - s.grid.entities[hi].toughness, base * 2 / 2, "marked + not weak: half");
    crate::combat::resolve_attack(&mut s.grid.entities, &mut s.rng, ci, wi, &mut hits);
    assert_eq!(10_000 - s.grid.entities[wi].toughness, base * 2 * 3 / 2, "marked + weak: ×1.5");
    // Someone else's mark is not hers.
    s.grid.entities[hi].buffs.clear();
    s.grid.entities[hi].buffs.push(Buff { kind: BuffKind::BlackwaterMark, magnitude: 5_000, ticks: 100, source_id: 999 });
    let t0 = s.grid.entities[hi].toughness;
    crate::combat::resolve_attack(&mut s.grid.entities, &mut s.rng, ci, hi, &mut hits);
    assert_eq!(s.grid.entities[hi].toughness, t0, "a mark from another source does not open the bar to her");
    // Boss first: a small boss beats a big non-boss.
    s.grid.entities[hi].is_boss = true;
    s.grid.entities[hi].hp = 10;
    let def = crate::skills::skill_def("blackwater_mark").unwrap();
    let picked = crate::tick::combat_step::find_boss_or_highest_hp_enemy_in_range(&s, ci, def.range).unwrap();
    assert_eq!(s.grid.entities[picked].id, 302, "the boss in range is the mark's target");
    s.grid.entities[hi].is_boss = false;
    let picked = crate::tick::combat_step::find_boss_or_highest_hp_enemy_in_range(&s, ci, def.range).unwrap();
    assert_eq!(s.grid.entities[picked].id, 301, "no boss: the biggest");
    // Broken target: +30% on her hits.
    assert_eq!(crate::skills::broken_target_bp(&s.grid.entities[ci], &s.grid.entities[wi]), 0);
    s.grid.entities[wi].break_ticks = 30;
    assert_eq!(crate::skills::broken_target_bp(&s.grid.entities[ci], &s.grid.entities[wi]), 3_000);
}

/// SUZU + MIREI (0.0.465): Sunder cuts DEF from its own kind; Break Fever lets her basic chip a
/// bar she is not weak to, at double; Bell Strike's cooldown clears on any break. Mirei's spark
/// fires on anyone's hit, spends a stack for everyone BUT her, and her ultimate detonates the lot.
#[test]
fn suzu_breaks_what_she_should_not_and_mireis_sparks_are_everyone_else_to_spend() {
    use crate::grid::{Cell, Element, Entity, Team};
    use crate::skills::{Buff, BuffKind, SkillState};
    let mut s = crate::tick::tests::world();
    s.grid.entities.retain(|e| matches!(e.team, Team::Player));
    let mut suzu = Entity::idle(80, Cell::new(2, 4)).with_team(Team::Player).with_combat(5_000, 1_000, 0, 100, Element::Fire);
    suzu.unit_def = "suzu".into();
    suzu.skills = vec![SkillState::new("bright_edge", 1), SkillState::new("sunder", 1), SkillState::new("bell_strike", 1), SkillState::new("break_fever", 1)];
    suzu.active_skills = vec!["sunder".into(), "bell_strike".into(), "break_fever".into()];
    s.grid.spawn(suzu);
    let mut mirei = Entity::idle(81, Cell::new(2, 5)).with_team(Team::Player).with_combat(5_000, 1_000, 0, 100, Element::Thunder);
    mirei.unit_def = "mirei".into();
    mirei.skills = vec![SkillState::new("conductor", 1), SkillState::new("storm_field", 1), SkillState::new("quicken", 1), SkillState::new("thunderfall", 1)];
    mirei.is_ranged = true;
    mirei.atk_range = 6;
    s.grid.spawn(mirei);
    let mut m = Entity::idle(301, Cell::new(3, 4)).with_team(Team::Monster).with_combat(1_000_000, 0, 0, 0, Element::Earth);
    m.max_hp = 1_000_000;
    m.toughness_max = 10_000;
    m.toughness = 10_000;
    m.weaknesses = vec![Element::Ice]; // not Fire — the fever is what opens this bar
    s.grid.spawn(m);
    let si = s.grid.idx_of_id(80).unwrap();
    let mi2 = s.grid.idx_of_id(81).unwrap();
    let ti = s.grid.idx_of_id(301).unwrap();
    // 1. Sunder is its own kind and cuts DEF.
    s.grid.entities[ti].def = 1_000;
    let before = crate::combat::effective_hard_def(&s.grid.entities[ti]);
    s.grid.entities[ti].buffs.push(Buff { kind: BuffKind::Sunder, magnitude: 20, ticks: 80, source_id: 80 });
    assert!(crate::combat::effective_hard_def(&s.grid.entities[ti]) < before, "Sunder cuts DEF");
    s.grid.entities[ti].buffs.clear();
    // 2. Not weak to Fire → no chip; under Break Fever → chips at double.
    let mut hits = Vec::new();
    crate::combat::resolve_attack(&mut s.grid.entities, &mut s.rng, si, ti, &mut hits);
    assert_eq!(s.grid.entities[ti].toughness, 10_000, "no weakness, no bar");
    s.grid.entities[si].buffs.push(Buff { kind: BuffKind::BreakFever, magnitude: 200, ticks: 150, source_id: 80 });
    crate::combat::resolve_attack(&mut s.grid.entities, &mut s.rng, si, ti, &mut hits);
    assert_eq!(10_000 - s.grid.entities[ti].toughness, u16::from(crate::grid::TOUGHNESS_CHIP_ADVANTAGE) * 2, "fever: any bar, at double");
    s.grid.entities[si].buffs.clear();
    // 3. Bell Strike's cooldown clears when anything breaks.
    s.grid.entities[si].skills[2].cooldown = 90;
    s.recent_hits.push(crate::state::HitEvent { target_id: 301, attacker_id: 80, amount: 1, hits: 1, kind: crate::state::HitKind::Break });
    crate::tick::collect_break_triggers(&mut s);
    assert_eq!(s.grid.entities[si].skills[2].cooldown, 0, "a break rings the bell again");
    s.recent_hits.clear();
    // 4. Mirei's sparks: her own hit fires one and keeps the stack; Suzu's spends one.
    let def = crate::skills::skill_def("storm_field").unwrap();
    crate::tick::combat_step::lay_static_spark(&mut s, mi2, ti, def);
    let stacks = |s: &crate::state::GameState, ti: usize| s.grid.entities[ti].buffs.iter().filter(|b| matches!(b.kind, BuffKind::StaticSpark)).count();
    assert_eq!(stacks(&s, ti), 5, "five sparks");
    let hp0 = s.grid.entities[ti].hp;
    crate::combat::fire_static_spark(&mut s.grid.entities, mi2, ti, &mut hits);
    assert!(s.grid.entities[ti].hp < hp0, "the spark fired");
    assert_eq!(stacks(&s, ti), 5, "and cost her nothing");
    crate::combat::fire_static_spark(&mut s.grid.entities, si, ti, &mut hits);
    assert_eq!(stacks(&s, ti), 4, "anyone else spends one");
    // 5. The ultimate detonates the rest at once.
    let hp0 = s.grid.entities[ti].hp;
    crate::combat::detonate_static_spark(&mut s.grid.entities, ti, &mut hits);
    assert_eq!(stacks(&s, ti), 0, "the lot goes off");
    assert!(s.grid.entities[ti].hp < hp0);
}

/// SYLVIE (0.0.473): Arc Focus lands on the top-ATK DAMAGE DEALER (a healer with more ATK is
/// passed over), her ultimate crits the party and cuts every ally's cooldowns, and a crit under
/// the S1 rider charges the critter's gauge.
#[test]
fn sylvie_focuses_the_dealer_cuts_cooldowns_and_crits_charge_the_gauge() {
    use crate::grid::{Cell, Element, Entity, Team};
    use crate::skills::{buff_sum, Buff, BuffKind, SkillState};
    let mut s = crate::tick::tests::world();
    s.grid.entities.retain(|e| matches!(e.team, Team::Player));
    let mut syl = Entity::idle(90, Cell::new(2, 4)).with_team(Team::Player).with_combat(1_000, 100, 0, 100, Element::Thunder);
    syl.unit_def = "sylvie".into();
    syl.skills = vec![SkillState::new("spark_tap", 1), SkillState::new("surge", 1), SkillState::new("arc_focus", 1), SkillState::new("bolt_from_the_blue", 1)];
    syl.active_skills = vec!["surge".into(), "arc_focus".into(), "bolt_from_the_blue".into()];
    s.grid.spawn(syl);
    let mut healer = Entity::idle(91, Cell::new(3, 4)).with_team(Team::Player).with_combat(1_000, 3_000, 0, 100, Element::Light);
    healer.unit_def = "lucielle".into();
    s.grid.spawn(healer);
    let mut dealer = Entity::idle(92, Cell::new(3, 5)).with_team(Team::Player).with_combat(1_000, 2_000, 0, 100, Element::Fire);
    dealer.unit_def = "lapis".into();
    dealer.skills = vec![SkillState::new("fireball", 1), SkillState::new("fire_cat_meteor", 1)];
    s.grid.spawn(dealer);
    let mut m = Entity::idle(301, Cell::new(6, 4)).with_team(Team::Monster).with_combat(100_000, 10, 0, 50, Element::Physical);
    m.max_hp = 100_000;
    s.grid.spawn(m);
    let (si, hi, di) = (s.grid.idx_of_id(90).unwrap(), s.grid.idx_of_id(91).unwrap(), s.grid.idx_of_id(92).unwrap());

    // Arc Focus (Surge parked): the dealer, not the higher-ATK healer, and never herself.
    s.grid.entities[si].skills[1].cooldown = 100;
    assert!(crate::tick::combat_step::cast::try_cast_buffs(&mut s, si));
    assert!(buff_sum(&s.grid.entities[di], BuffKind::CritDmgBpAdd) > 0, "the dealer is focused");
    assert_eq!(buff_sum(&s.grid.entities[hi], BuffKind::CritDmgBpAdd), 0, "the healer is passed over");
    assert_eq!(buff_sum(&s.grid.entities[si], BuffKind::CritDmgBpAdd), 0, "she never focuses herself");

    // The ultimate: party crit, and every ally's cooldowns drop by 3 s.
    s.grid.entities[di].skills[0].cooldown = 100;
    s.grid.entities[si].limit_break = 1_000;
    assert!(crate::tick::combat_step::cast::try_cast_buffs(&mut s, si));
    assert_eq!(s.grid.entities[di].skills[0].cooldown, 70, "3 s off the dealer's fireball");
    assert_eq!(buff_sum(&s.grid.entities[hi], BuffKind::CritBpAdd), 2_500, "the whole party crits");

    // A crit under the S1 rider charges the critter's own gauge; a plain hit does not.
    s.grid.entities[di].limit_break = 0;
    crate::skills::apply_buff(&mut s.grid.entities[di], Buff { kind: BuffKind::CritGauge, magnitude: 5, ticks: 100, source_id: 1 });
    let hit = |kind| crate::state::HitEvent { target_id: 301, attacker_id: 92, amount: 10, hits: 1, kind };
    s.recent_hits = vec![hit(crate::state::HitKind::Normal)];
    crate::tick::crit_riders(&mut s);
    assert_eq!(s.grid.entities[di].limit_break, 0, "a plain hit charges nothing");
    s.recent_hits = vec![hit(crate::state::HitKind::Crit)];
    crate::tick::crit_riders(&mut s);
    assert!(s.grid.entities[di].limit_break > 0, "a crit charges the gauge");
}

/// SURGE (0.0.474): Thunder allies get half as much attack speed again; the crit is the same.
#[test]
fn surge_gives_thunder_allies_half_again_the_attack_speed() {
    use crate::grid::{Cell, Element, Entity, Team};
    use crate::skills::{buff_sum, BuffKind, SkillState};
    let mut s = crate::tick::tests::world();
    s.grid.entities.retain(|e| matches!(e.team, Team::Player));
    let mut syl = Entity::idle(90, Cell::new(2, 4)).with_team(Team::Player).with_combat(1_000, 100, 0, 100, Element::Thunder);
    syl.unit_def = "sylvie".into();
    syl.skills = vec![SkillState::new("surge", 1)];
    syl.active_skills = vec!["surge".into()];
    s.grid.spawn(syl);
    s.grid.spawn(Entity::idle(91, Cell::new(3, 4)).with_team(Team::Player).with_combat(1_000, 100, 0, 100, Element::Thunder));
    s.grid.spawn(Entity::idle(92, Cell::new(3, 5)).with_team(Team::Player).with_combat(1_000, 100, 0, 100, Element::Fire));
    s.grid.spawn(Entity::idle(301, Cell::new(6, 4)).with_team(Team::Monster).with_combat(100_000, 10, 0, 50, Element::Physical));
    let si = s.grid.idx_of_id(90).unwrap();
    assert!(crate::tick::combat_step::cast::try_cast_buffs(&mut s, si));
    let aspd = |s: &crate::state::GameState, id| buff_sum(&s.grid.entities[s.grid.idx_of_id(id).unwrap()], BuffKind::AspdPct);
    assert_eq!(aspd(&s, 92), 1_000, "a Fire ally: +10%");
    assert_eq!(aspd(&s, 91), 1_500, "a Thunder ally: +15%");
    assert_eq!(aspd(&s, 90), 1_500, "and Sylvie herself is Thunder");
    let crit = |s: &crate::state::GameState, id| buff_sum(&s.grid.entities[s.grid.idx_of_id(id).unwrap()], BuffKind::CritBpAdd);
    assert_eq!(crit(&s, 91), crit(&s, 92), "the crit is the same for everyone");
}

/// ZOLFIA (0.0.476): her hit chills; a break on a chilled enemy — from anyone — shatters it
/// (+20% Ice and Fire taken, nothing else); her aura slows and burns every second.
#[test]
fn zolfia_chills_a_break_shatters_and_the_winter_burns() {
    use crate::grid::{Cell, DamageType, Element, Entity, Team};
    use crate::skills::{buff_sum, Buff, BuffKind, SkillState};
    let mut s = crate::tick::tests::world();
    s.grid.entities.retain(|e| matches!(e.team, Team::Player));
    let mut z = Entity::idle(90, Cell::new(2, 4)).with_team(Team::Player).with_combat(1_000, 1_000, 0, 100, Element::Ice);
    z.unit_def = "zolfia".into();
    z.skills = vec![SkillState::new("coldflame_knight", 1), SkillState::new("remorseless_winter", 1)];
    s.grid.spawn(z);
    s.grid.spawn(Entity::idle(91, Cell::new(2, 5)).with_team(Team::Player).with_combat(1_000, 1_000, 0, 100, Element::Fire));
    let mut m = Entity::idle(301, Cell::new(3, 4)).with_team(Team::Monster).with_combat(100_000, 10, 0, 50, Element::Physical);
    m.max_hp = 100_000;
    s.grid.spawn(m);
    let (zi, mi) = (s.grid.idx_of_id(90).unwrap(), s.grid.idx_of_id(301).unwrap());
    let hit = |att, kind| crate::state::HitEvent { target_id: 301, attacker_id: att, amount: 10, hits: 1, kind };

    // A break with no chill shatters nothing.
    s.recent_hits = vec![hit(91, crate::state::HitKind::Break)];
    crate::tick::winter_pass(&mut s);
    assert_eq!(buff_sum(&s.grid.entities[mi], BuffKind::Shattered), 0, "unchilled: no shatter");
    // Her hit chills; then the ALLY's break shatters it.
    s.recent_hits = vec![hit(90, crate::state::HitKind::Normal)];
    crate::tick::winter_pass(&mut s);
    assert!(buff_sum(&s.grid.entities[mi], BuffKind::WinterChill) > 0, "her hit chills");
    s.recent_hits = vec![hit(91, crate::state::HitKind::Break)];
    crate::tick::winter_pass(&mut s);
    assert_eq!(crate::skills::shattered_bp(&s.grid.entities[mi]), 2_000, "an ally's break shatters it");

    // Shattered: +20% Ice and Fire, Thunder untouched.
    let pkt = |el| crate::combat::Damage { element: el, kind: DamageType::Magic, ..crate::combat::build_damage(&s.grid.entities[zi], false) };
    let bare = {
        let mut b = s.grid.entities[mi].clone();
        b.buffs.retain(|x| x.kind != BuffKind::Shattered);
        b
    };
    for (el, more) in [(Element::Ice, true), (Element::Fire, true), (Element::Thunder, false)] {
        let with = crate::combat::apply_damage(&s.grid.entities[mi], &pkt(el), 0);
        let without = crate::combat::apply_damage(&bare, &pkt(el), 0);
        if more { assert!(with > without, "{el:?} is amplified"); } else { assert_eq!(with, without, "{el:?} is not"); }
    }

    // The aura: once a second, slow + Ice DoT on every enemy within 2 cells.
    crate::skills::apply_buff(&mut s.grid.entities[zi], Buff { kind: BuffKind::WinterAura, magnitude: 1, ticks: 80, source_id: 90 });
    s.tick = 20;
    let hp = s.grid.entities[mi].hp;
    crate::tick::winter_aura(&mut s);
    assert!(s.grid.entities[mi].hp < hp, "the winter burns");
    assert_eq!(crate::skills::effective_bp(&s.grid.entities[mi], BuffKind::AspdSlowPct), 2_000, "and slows by 20%");
}

/// SEND TO STEAM (0.0.478): an item whose catalog def carries a `steam_itemdef` leaves the bag
/// one at a time and is counted in `steam_sent`; a bound copy, an item without an itemdef, and a
/// call without the catalog do nothing.
#[test]
fn send_to_steam_takes_one_minted_item_and_counts_it() {
    use crate::events::{apply_input_with, InputEvent};
    let data = crate::data::GameData::from_json_str(include_str!("../../../../../data/game_data.json")).expect("catalog");
    let mut s = crate::tick::tests::world();
    let mut bx = crate::data::instantiate_item(data.item("farm_box").expect("farm_box"));
    bx.quantity = 2;
    s.inventory = vec![bx];
    apply_input_with(&mut s, Some(&data), InputEvent::SendToSteam { inv_idx: 0 });
    assert_eq!(s.inventory[0].quantity, 1);
    assert_eq!(s.steam_sent.get("farm_box"), Some(&1));
    apply_input_with(&mut s, None, InputEvent::SendToSteam { inv_idx: 0 });
    assert_eq!(s.steam_sent.get("farm_box"), Some(&1), "no catalog, no send");
    s.inventory[0].bound = true;
    apply_input_with(&mut s, Some(&data), InputEvent::SendToSteam { inv_idx: 0 });
    assert_eq!(s.steam_sent.get("farm_box"), Some(&1), "a bound copy stays");
    s.inventory[0].bound = false;
    apply_input_with(&mut s, Some(&data), InputEvent::SendToSteam { inv_idx: 0 });
    assert!(s.inventory.is_empty(), "the last one leaves the bag");
    assert_eq!(s.steam_sent.get("farm_box"), Some(&2));
    s.inventory = vec![crate::data::instantiate_item(data.item("lootbox").expect("lootbox"))];
    apply_input_with(&mut s, Some(&data), InputEvent::SendToSteam { inv_idx: 0 });
    assert_eq!(s.inventory.len(), 1, "no steam_itemdef: it stays in the game");
}

/// FARM BANNER TICKETS (0.0.479): any catalog `farm_box` def opens one at a time into
/// `farm_opened[def]`; another item, or a call without the catalog, does nothing.
#[test]
fn open_farm_box_counts_per_box_def() {
    use crate::events::{apply_input_with, InputEvent};
    let data = crate::data::GameData::from_json_str(include_str!("../../../../../data/game_data.json")).expect("catalog");
    let mut s = crate::tick::tests::world();
    let mut bx = crate::data::instantiate_item(data.item("farm_box").expect("farm_box"));
    bx.quantity = 2;
    s.inventory = vec![bx];
    apply_input_with(&mut s, None, InputEvent::OpenFarmBox { inv_idx: 0 });
    assert!(s.farm_opened.is_empty(), "no catalog, no open");
    apply_input_with(&mut s, Some(&data), InputEvent::OpenFarmBox { inv_idx: 0 });
    assert_eq!((s.inventory[0].quantity, s.farm_opened.get("farm_box")), (1, Some(&1)));
    apply_input_with(&mut s, Some(&data), InputEvent::OpenFarmBox { inv_idx: 0 });
    assert!(s.inventory.is_empty(), "the last one leaves the bag");
    assert_eq!(s.farm_opened.get("farm_box"), Some(&2));
    s.inventory = vec![crate::data::instantiate_item(data.item("lootbox").expect("lootbox"))];
    apply_input_with(&mut s, Some(&data), InputEvent::OpenFarmBox { inv_idx: 0 });
    assert_eq!(s.inventory.len(), 1, "not a farm box: it stays");
    assert_eq!(s.measures.farm_boxes_opened.get(), 0, "the 0.0.477 counter is retired");
}

/// STEAM ITEMS ARE SINGLES (0.0.480): a mailed box lands in its own slot (a pre-0.0.480 mail
/// without the stamp included), never onto another; a mailed ×2 takes two slots or none; a saved
/// stack splits on load.
#[test]
fn steam_items_never_stack() {
    let data = crate::data::GameData::from_json_str(include_str!("../../../../../data/game_data.json")).expect("catalog");
    let mut s = crate::tick::tests::world();
    s.inventory.clear();
    let mut old = crate::data::instantiate_item(data.item("farm_box").expect("farm_box"));
    old.steam = false; // as a 0.0.478 server mailed it
    for id in 1..=3 {
        assert!(crate::mailbox_grant::claim_mail(&mut s, &data, id, crate::state::Gil::ZERO, "", 0, Some(old.clone())));
    }
    assert_eq!(s.inventory.len(), 3, "three boxes, three slots");
    assert!(s.inventory.iter().all(|it| it.steam && it.quantity == 1 && !it.is_stackable()));
    let mut two = old.clone();
    two.quantity = 2;
    assert!(crate::mailbox_grant::claim_mail(&mut s, &data, 4, crate::state::Gil::ZERO, "", 0, Some(two)));
    assert_eq!(s.inventory.len(), 5, "a ×2 lands as two singles");
    s.inventory.truncate(1);
    s.inventory[0].quantity = 4; // a stack saved before 0.0.480
    crate::data::migrate_save_items(&mut s, &data);
    assert_eq!(s.inventory.len(), 4, "the stack splits on load");
    assert!(s.inventory.iter().all(|it| it.quantity == 1));
}

/// ENHANCE TAKES DUST ONLY (0.0.489, user 2026-09-28): a card is salvaged into Memory Dust first.
/// A worn or locked card is never salvaged, an SSR only once another copy is S5, the invested XP
/// comes back, and the old card-feed input is inert. Values are XP since 0.0.490.
#[test]
fn enhance_takes_dust_and_a_card_is_salvaged_first() {
    use crate::events::{apply_input_with, InputEvent};
    use crate::items::{regalia_xp_to_level, Item, ItemKind, Rarity, REGALIA_MAX_STARS};
    let data = crate::data::GameData::from_json_str(include_str!("../../../../../data/game_data.json")).expect("catalog");
    let card = |id: &str| Item::new(id, ItemKind::Regalia, 0, 0, Rarity::Rare);
    let dust = |s: &crate::state::GameState| crate::items::DustPay {
        dust: s.material_count("regalia_fodder"),
        radiant: s.material_count("regalia_sr_fodder"),
        crystal: s.material_count("regalia_crystal_fodder"),
    };
    let mut s = crate::tick::tests::world();
    s.owned_regalia = vec![card("regalia_kestrel"), card("regalia_faultline"), card("regalia_vesper")];
    s.owned_regalia[2].equipped_by = 999; // worn

    apply_input_with(&mut s, Some(&data), InputEvent::EnhanceRegalia { target_idx: 0, feeder_idx: 1 });
    assert_eq!((s.owned_regalia.len(), s.owned_regalia[0].enhance_level), (3, 0), "a card is no longer feed");
    apply_input_with(&mut s, Some(&data), InputEvent::SalvageRegalia { owned_idx: 2 });
    assert_eq!(s.owned_regalia.len(), 3, "a worn card is never salvaged");
    apply_input_with(&mut s, Some(&data), InputEvent::LockRegalia { owned_idx: 1, locked: true });
    apply_input_with(&mut s, Some(&data), InputEvent::SalvageRegalia { owned_idx: 1 });
    assert_eq!(s.owned_regalia.len(), 3, "a locked card is never salvaged");
    apply_input_with(&mut s, Some(&data), InputEvent::LockRegalia { owned_idx: 1, locked: false });
    apply_input_with(&mut s, Some(&data), InputEvent::SalvageRegalia { owned_idx: 1 });
    assert_eq!((s.owned_regalia.len(), dust(&s)), (2, crate::items::regalia_xp_as_dust(50_000)), "a 3★ card is 50,000 XP = 5 Radiant");
    assert_eq!(dust(&s).radiant, 5);

    // An SR card at S2 (3 copies) with Lv5 + 700 XP invested: 3 x 150,000 + the invested XP.
    let mut sr = card("regalia_emberbell");
    sr.resonance_stars = 2;
    sr.enhance_level = 5;
    sr.enhance_xp = 700;
    s.owned_regalia.push(sr);
    let invested = regalia_xp_to_level(1, 5) + 700;
    let total = 450_000 + invested;
    let before = dust(&s);
    apply_input_with(&mut s, Some(&data), InputEvent::SalvageRegalia { owned_idx: 2 });
    let paid = crate::items::regalia_xp_as_dust(total);
    assert_eq!(dust(&s), crate::items::DustPay { dust: before.dust + paid.dust, radiant: before.radiant + paid.radiant, crystal: before.crystal + paid.crystal }, "every copy and every invested XP comes back");

    // SSR: refused until another copy is S5, then worth 600,000 XP a copy.
    let before = dust(&s);
    s.owned_regalia.push(card("regalia_meridian"));
    let spare = s.owned_regalia.len() - 1;
    apply_input_with(&mut s, Some(&data), InputEvent::SalvageRegalia { owned_idx: spare as u32 });
    assert_eq!(s.owned_regalia.len(), spare + 1, "an SSR dupe belongs in Resonance first");
    let mut maxed = card("regalia_meridian");
    maxed.resonance_stars = REGALIA_MAX_STARS;
    s.owned_regalia.push(maxed);
    apply_input_with(&mut s, Some(&data), InputEvent::SalvageRegalia { owned_idx: spare as u32 });
    assert_eq!(dust(&s), crate::items::DustPay { crystal: before.crystal + 6, ..before }, "past S5 it salvages for 6 Crystals");
    assert_eq!(s.owned_regalia[spare].resonance_stars, REGALIA_MAX_STARS, "the S5 copy stays");
}

/// Salvage all takes only UNTOUCHED 3★ cards: never an SR / SSR, a worn or locked card, or a 3★
/// with Enhance levels or Resonance.
#[test]
fn salvage_all_takes_only_untouched_three_star_cards() {
    use crate::events::{apply_input_with, InputEvent};
    use crate::items::{Item, ItemKind, Rarity};
    let data = crate::data::GameData::from_json_str(include_str!("../../../../../data/game_data.json")).expect("catalog");
    let card = |id: &str| Item::new(id, ItemKind::Regalia, 0, 0, Rarity::Rare);
    let mut s = crate::tick::tests::world();
    s.owned_regalia = vec![
        card("regalia_kestrel"), card("regalia_emberbell"), card("regalia_vesper"),
        card("regalia_faultline"), card("regalia_meridian"), card("regalia_cutpurse"),
        card("regalia_last_rivet"), card("regalia_emberwick"),
    ];
    s.owned_regalia[2].equipped_by = 999;
    s.owned_regalia[3].locked = true;
    s.owned_regalia[6].enhance_level = 5;
    s.owned_regalia[7].resonance_stars = 1;
    apply_input_with(&mut s, Some(&data), InputEvent::SalvageRegaliaAll {});
    let left: Vec<&str> = s.owned_regalia.iter().map(|c| c.def_id.as_str()).collect();
    assert_eq!(left, [
        "regalia_emberbell", "regalia_vesper", "regalia_faultline", "regalia_meridian",
        "regalia_last_rivet", "regalia_emberwick",
    ], "SR, SSR, worn, locked, enhanced and resonated cards all stay");
    assert_eq!(s.material_count("regalia_sr_fodder"), 10, "two 3★ cards = 100,000 XP");
    // Each of the kept 3★ cards can still go one at a time.
    apply_input_with(&mut s, Some(&data), InputEvent::SalvageRegalia { owned_idx: 4 });
    let back = crate::items::regalia_xp_as_dust(50_000 + crate::items::regalia_xp_to_level(0, 5));
    assert_eq!(
        (s.material_count("regalia_fodder"), s.material_count("regalia_sr_fodder")),
        (back.dust, 10 + back.radiant),
        "Lv5 comes back with the card"
    );
}

/// Dust is a ledger count: a mailed Dust never becomes a carrier, and a save that still holds
/// Dust carriers moves them into the ledger on load.
#[test]
fn memory_dust_lives_in_the_ledger() {
    use crate::items::{Item, ItemKind, Rarity};
    let data = crate::data::GameData::from_json_str(include_str!("../../../../../data/game_data.json")).expect("catalog");
    let mut s = crate::tick::tests::world();
    let def = data.item("regalia_sr_fodder").expect("def");
    assert!(crate::data::build::items::bank_collectible(&mut s, &data, crate::data::instantiate_item(def)));
    assert_eq!((s.owned_regalia.len(), s.material_count("regalia_sr_fodder")), (0, 1));
    s.owned_regalia = vec![
        Item::new("regalia_fodder", ItemKind::Regalia, 0, 0, Rarity::Common),
        Item::new("regalia_kestrel", ItemKind::Regalia, 0, 0, Rarity::Rare),
        Item::new("regalia_fodder", ItemKind::Regalia, 0, 0, Rarity::Common),
    ];
    crate::data::migrate_save_items(&mut s, &data);
    assert_eq!(s.owned_regalia.iter().map(|c| c.def_id.as_str()).collect::<Vec<_>>(), ["regalia_kestrel"]);
    assert_eq!(s.material_count("regalia_fodder"), 2);
}

/// The six stat-only SR cards are gone (0.0.489): a save holding one gets its salvage value in
/// Radiant (1 per copy + fed levels), and a member wearing one loses the overlay.
#[test]
fn a_retired_sr_card_becomes_radiant_dust_on_load() {
    use crate::items::{Item, ItemKind, Rarity};
    let data = crate::data::GameData::from_json_str(include_str!("../../../../../data/game_data.json")).expect("catalog");
    let mut s = crate::tick::tests::world();
    let mut worn = Item::new("regalia_sunbreaker", ItemKind::Regalia, 0, 0, Rarity::Rare);
    worn.resonance_stars = 1;
    worn.enhance_level = 20;
    worn.equipped_by = s.grid.entities[0].id;
    s.grid.entities[0].regalia = Some(crate::items::Regalia { id: "sunbreaker".into(), ..Default::default() });
    s.owned_regalia = vec![worn, Item::new("regalia_kestrel", ItemKind::Regalia, 0, 0, Rarity::Rare)];
    assert!(crate::items::regalia_def("regalia_sunbreaker").is_none(), "the card no longer exists");
    crate::data::migrate_save_items(&mut s, &data);
    assert_eq!(s.owned_regalia.iter().map(|c| c.def_id.as_str()).collect::<Vec<_>>(), ["regalia_kestrel"]);
    let pay = crate::items::regalia_xp_as_dust(2 * 150_000 + crate::items::regalia_xp_to_level(1, 20));
    assert_eq!(
        (s.material_count("regalia_fodder"), s.material_count("regalia_sr_fodder"), s.material_count("regalia_crystal_fodder")),
        (pay.dust, pay.radiant, pay.crystal),
        "2 SR copies + Lv20, valued in today's XP"
    );
    assert!(s.grid.entities[0].regalia.is_none(), "the overlay is taken off");
}

/// Units, Memory Fragments and Memory Dust are never sold for Gil — even when a catalog row
/// carries a price.
#[test]
fn the_gil_shop_sells_no_unit_or_card() {
    use crate::events::{apply_input_with, InputEvent};
    let src = include_str!("../../../../../data/game_data.json");
    let priced = |json: &str, id: &str| {
        let at = json.find(&format!("\"id\": \"{id}\",")).expect("row");
        let p = at + json[at..].find("\"buy_price\": 0").expect("unpriced");
        format!("{}\"buy_price\": 1000{}", &json[..p], &json[p + "\"buy_price\": 0".len()..])
    };
    let json = priced(&priced(&priced(src, "exemplar_seraphine"), "regalia_meridian"), "regalia_fodder");
    let data = crate::data::GameData::from_json_str(&json).expect("catalog");
    let mut s = crate::tick::tests::world();
    s.set_gil_for_test(1_000_000);
    let (units, cards) = (s.grid.entities.len(), s.owned_regalia.len());
    for id in ["exemplar_seraphine", "regalia_meridian"] {
        apply_input_with(&mut s, Some(&data), InputEvent::BuyItem { def_id: id.into(), qty: 1 });
    }
    assert_eq!((s.gil(), s.grid.entities.len(), s.owned_regalia.len()), (1_000_000, units, cards), "refused by kind");
    apply_input_with(&mut s, Some(&data), InputEvent::BuyItem { def_id: "regalia_fodder".into(), qty: 2 });
    assert_eq!((s.gil(), s.material_count("regalia_fodder")), (1_000_000, 0), "Dust is not sold either");
}

/// CARDS HELD BACK (0.0.490, `features.cards: false`): no card row enters a drop table and the shop
/// neither lists nor sells one; a catalog without the switch keeps cards on.
#[test]
fn cards_held_back_by_the_catalog_switch() {
    use crate::events::{apply_input_with, InputEvent};
    let src = include_str!("../../../../../data/game_data.json");
    let data = crate::data::GameData::from_json_str(src).expect("catalog");
    assert!(!data.features.cards, "the shipped catalog holds cards back");
    let is_card = |id: &str| data.item(id).is_some_and(|d| matches!(d.kind, crate::items::ItemKind::Card));
    for m in &data.monsters {
        let table = crate::data::build_drop_table(m, &data, m.stage);
        assert!(table.iter().all(|e| !is_card(&e.template.def_id)), "{}: a card row entered the table", m.id);
    }
    let card = data.items.iter().find(|d| matches!(d.kind, crate::items::ItemKind::Card) && d.buy_price > 0).expect("a priced card");
    assert!(!crate::data::sells_for_gil(&data, card));
    let mut s = crate::tick::tests::world();
    s.set_gil_for_test(1_000_000);
    apply_input_with(&mut s, Some(&data), InputEvent::BuyItem { def_id: card.id.clone(), qty: 1 });
    assert_eq!(s.gil(), 1_000_000, "a card is not sold while cards are held back");

    let on = crate::data::GameData::from_json_str(&src.replacen("\"cards\": false", "\"cards\": true", 1)).expect("catalog");
    assert!(on.features.cards && crate::data::sells_for_gil(&on, on.item(&card.id).expect("def")), "switch on: sold again");
    assert!(crate::data::Features::default().cards, "a catalog without the block keeps cards on");
}

/// Items taken out of the game (0.0.490) leave a save on load wherever they sit — bag, stash,
/// worn gear, consumable slots — and nothing else is touched.
/// ELEMENT-LESS SIGIL (2026-09-30): a save holding element Sigils — in the ledger or as a bag
/// stack — loads with the same totals folded into the generic Sigil of each grade.
#[test]
fn element_sigils_fold_into_the_generic_sigil_on_load() {
    use crate::items::{Item, ItemKind, Rarity};
    use crate::potential::{legacy_element_sigils, sigil_def};
    use crate::state::MaterialSource;
    let data = crate::data::GameData::from_json_str(include_str!("../../../../../data/game_data.json")).expect("catalog");
    let old = legacy_element_sigils();
    for (id, _) in &old {
        assert!(data.item(id).is_none(), "{id} is still in the catalog");
    }
    let mut s = crate::tick::tests::world();
    // Two elements of grade I and one of grade III in the ledger, one grade-II bag stack.
    let (a, b, c, d) = (&old[0], &old[3], &old[5], &old[4]);
    assert_eq!((a.1, b.1, c.1, d.1), (0, 0, 2, 1), "the table pairs each id with its grade");
    s.credit_material(&a.0, 5, MaterialSource::Drop);
    s.credit_material(&b.0, 7, MaterialSource::Drop);
    s.credit_material(&c.0, 2, MaterialSource::Drop);
    let mut stack = Item::new(&d.0, ItemKind::Material, 0, 0, Rarity::Rare);
    stack.quantity = 4;
    s.inventory.push(stack);
    crate::data::migrate_save_items(&mut s, &data);
    assert_eq!(
        (s.material_count(&sigil_def(0)), s.material_count(&sigil_def(1)), s.material_count(&sigil_def(2))),
        (12, 4, 2),
        "every element Sigil is kept, by grade",
    );
    assert!(old.iter().all(|(id, _)| s.material_count(id) == 0), "no element Sigil survives");
    assert!(s.inventory.iter().all(|i| old.iter().all(|(id, _)| &i.def_id != id)));
}

#[test]
fn retired_items_are_removed_on_load() {
    use crate::items::{ConsumableSlot, Item, ItemKind, Rarity};
    let data = crate::data::GameData::from_json_str(include_str!("../../../../../data/game_data.json")).expect("catalog");
    for id in crate::data::RETIRED_ITEM_IDS {
        assert!(data.item(id).is_none(), "{id} is still in the catalog");
    }
    let mut s = crate::tick::tests::world();
    s.inventory = vec![
        Item::new("aspd_pot_2", ItemKind::Consumable, 0, 0, Rarity::Common),
        Item::new("hp_pot_1", ItemKind::Consumable, 0, 0, Rarity::Common),
    ];
    s.stash = vec![Item::new("inventory_expander", ItemKind::Material, 0, 0, Rarity::Common)];
    let e = &mut s.grid.entities[0];
    e.equipped.push(Item::new("unique_blade", ItemKind::Weapon, 50, 0, Rarity::Legendary));
    e.consumables.push(ConsumableSlot { item: Item::new("loot_field_ration", ItemKind::Consumable, 0, 0, Rarity::Common), ready_tick: 0, key: 1 });
    crate::data::migrate_save_items(&mut s, &data);
    assert_eq!(s.inventory.iter().map(|i| i.def_id.as_str()).collect::<Vec<_>>(), ["hp_pot_1"], "only the live item stays");
    assert!(s.stash.is_empty());
    assert!(s.grid.entities[0].equipped.iter().all(|i| i.def_id != "unique_blade"), "a worn unique is taken off");
    assert!(s.grid.entities[0].consumables.is_empty(), "the ration's slot is gone");
}

/// The batch preview the view uses (one probe clone) names the same skill as the single form
/// (a clone per member) for every entity, tick after tick of a real fight (audit 2026-10-02).
#[test]
fn batch_skill_preview_matches_the_single_form() {
    let d = crate::data::GameData::from_json_str(include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/../../data/game_data.json"))).unwrap();
    let mut s = crate::data::new_game_as(3, &d, "zone_b1_1", "suzu").unwrap();
    let lumi = crate::data::grant_exemplar_unit(&mut s, &d, "lumi").unwrap();
    s.party_slots = 2;
    crate::data::deploy(&mut s, &d, lumi).unwrap();
    let mut seen = 0;
    for _ in 0..3_000 {
        crate::tick::tick(&mut s, 1);
        let batch = crate::tick::preview_pending_skills(&s);
        for e in &s.grid.entities {
            let single = crate::tick::preview_pending_skill(&s, e.id);
            assert_eq!(batch.get(&e.id).cloned(), single, "entity {} at tick {}", e.id, s.tick);
            seen += usize::from(single.is_some());
        }
    }
    assert!(seen > 0, "the fight never previewed a skill — the test covers nothing");
}

/// AUDIT 2026-10-02: a BOUND stack of a shop item does not take an unbound purchase, so a full
/// bag refuses the buy instead of charging the Gil and losing the item.
#[test]
fn a_bound_stack_does_not_make_room_for_a_purchase() {
    let d = crate::data::GameData::from_json_str(include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/../../data/game_data.json"))).unwrap();
    let mut s = crate::data::new_game_in_zone(1, &d, "zone_b1_1").unwrap();
    let mut potion = crate::data::instantiate_item(d.item("hp_pot_1").unwrap());
    potion.bound = true;
    s.inventory.push(potion);
    while s.inventory.len() < s.inventory_cap() {
        s.inventory.push(crate::data::instantiate_item(d.item("sword_s1").unwrap()));
    }
    s.set_gil_for_test(1_000_000);
    assert!(crate::data::buy_item(&mut s, &d, "hp_pot_1", 1).is_err(), "full bag, only a bound stack");
    assert_eq!(s.gil(), 1_000_000, "no Gil taken");
}

/// AUDIT 2026-10-02: a summoned familiar's death pays nothing — no soul, kill count or bestiary
/// credit — so an unlimited summoner is not a farm.
#[test]
fn a_summoned_familiar_pays_nothing_when_it_dies() {
    let d = crate::data::GameData::from_json_str(include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/../../data/game_data.json"))).unwrap();
    let mut s = crate::data::new_game_in_zone(1, &d, "zone_b1_1").unwrap();
    let mut witch = Entity::idle(5000, Cell::new(10, 5)).with_team(Team::Monster);
    witch.max_hp = 1000;
    witch.hp = 1000;
    s.grid.spawn(witch);
    let widx = s.grid.entities.len() - 1;
    crate::spawn::spawn_familiars(&mut s, widx, 1, "bat");
    let bat = s.grid.entities.iter().position(|e| e.is_summon).expect("a bat was summoned");
    let (soul, kills, bestiary) = (s.soul(), s.measures.kills.get(), s.monster_kills.clone());
    s.grid.entities[bat].alive = false;
    crate::tick::combat_step::kill_monster(&mut s, bat, None);
    assert_eq!(s.soul(), soul, "no soul");
    assert_eq!(s.measures.kills.get(), kills, "no kill counted");
    assert_eq!(s.monster_kills, bestiary, "no bestiary credit");
}
