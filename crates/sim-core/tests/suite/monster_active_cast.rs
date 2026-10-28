//! Integration test: a MONSTER active-cast skill fires through the SHARED cast
//! pipeline — the same `pick_active_skill` → windup → resolve path players use.
//!
//! This pins the enable in `build_and_spawn_monster`: a monster's `Active` skills are
//! copied into `active_skills` (players get theirs from the class tree), so the cast
//! machinery — which is NOT team-gated — drives them too. Uses `poison_attack`, the
//! reusable monster bite (Neutral, 150% + a Poison DoT, 1.5 s wind-up).
//!
//! Lives in an INTEGRATION test (not a `#[cfg(test)]` lib module) on purpose: it links
//! the plain lib, so it runs even while a sibling lib unit-test module is mid-edit.

use sim_core::grid::{Cell, Element, Entity, Team};
use sim_core::skills::{Buff, BuffKind, SkillState};
use sim_core::state::GameState;
use sim_core::tick::tick;

#[test]
fn monster_active_skill_casts_and_poisons() {
    let mut state = GameState::new(42, 16, 8);

    // Player dummy: big HP, 0 ATK — a stationary target the zombie can bite without
    // dying or being killed, so the cast completes cleanly.
    let mut player = Entity::idle(1, Cell::new(0, 0))
        .with_team(Team::Player)
        .with_walk(Cell::new(0, 0), 0)
        .with_combat(/*hp*/ 100_000, /*atk*/ 0, /*def*/ 0,
                     /*atk_speed_milli*/ 0, Element::Fire);
    player.atk_range = 1;
    state.grid.spawn(player);

    // Zombie carrying the ACTIVE bite, adjacent to the player. `active_skills` mirrors
    // what `build_and_spawn_monster` sets at spawn (this test hand-builds entities).
    let mut zombie = Entity::idle(2, Cell::new(1, 0))
        .with_team(Team::Monster)
        .with_walk(Cell::new(1, 0), 0)
        .with_combat(/*hp*/ 5_000, /*atk*/ 100, /*def*/ 0,
                     /*atk_speed_milli*/ 500, Element::Dark);
    zombie.atk_range = 1;
    zombie.skills = vec![SkillState::new("poison_attack", 1)];
    zombie.active_skills = vec!["poison_attack".to_string()];
    state.grid.spawn(zombie);

    // Cooldown starts ready (0) → 1.5 s wind-up (15 ticks) → fire. 40 ticks is ample.
    tick(&mut state, 40);

    let player = &state.grid.entities[0];
    let zombie = &state.grid.entities[1];

    // The monster CAST its active (records a fire tick) — the pipeline reached it.
    assert!(
        zombie.skills[0].last_fired > 0,
        "monster active skill never fired — the cast pipeline didn't reach the monster"
    );
    // The bite's on_hit_debuff landed a Poison DoT on the target.
    assert!(
        player.buffs.iter().any(|b| b.kind == BuffKind::Poison),
        "poison_attack should have applied a Poison DoT to the target"
    );
}

/// A monster GROUND-FIELD active (Umbral Roots) placed via `try_cast_ground_field` —
/// a different cast path than `pick_active_skill`, also not team-gated. Gnarloak drops
/// a 3×3 Shadow field at the target's feet that both ticks damage and slows.
#[test]
fn monster_ground_field_casts_slows_and_ticks() {
    let mut state = GameState::new(42, 16, 8);

    let mut player = Entity::idle(1, Cell::new(0, 0))
        .with_team(Team::Player)
        .with_walk(Cell::new(0, 0), 0)
        .with_combat(/*hp*/ 100_000, /*atk*/ 0, /*def*/ 0,
                     /*atk_speed_milli*/ 0, Element::Fire);
    player.atk_range = 1;
    state.grid.spawn(player);

    // Gnarloak carrying the ground-field active, within its 8-cell cast range.
    let mut gnarloak = Entity::idle(2, Cell::new(2, 0))
        .with_team(Team::Monster)
        .with_walk(Cell::new(2, 0), 0)
        .with_combat(/*hp*/ 9_000, /*atk*/ 100, /*def*/ 0,
                     /*atk_speed_milli*/ 500, Element::Earth);
    gnarloak.atk_range = 3;
    gnarloak.skills = vec![SkillState::new("umbral_roots", 1)];
    gnarloak.active_skills = vec!["umbral_roots".to_string()];
    state.grid.spawn(gnarloak);

    // Wind-up (2 s = 20 ticks) → field placed on the player → field ticks the slow + dmg.
    tick(&mut state, 60);

    let player = &state.grid.entities[0];
    assert!(
        player.buffs.iter().any(|b| b.kind == BuffKind::AgiDexSlowPct),
        "umbral_roots field should have slowed a target standing in it"
    );
    assert!(
        player.hp < 100_000,
        "umbral_roots field should have ticked damage on the target"
    );
}

/// Crimsonne's Blood Slash (Duskmourn cast): the poison_attack shape reused with a
/// different on-hit debuff — an 80% Shadow hit that opens a BLEED instead of Poison.
/// Confirms a second monster active (different debuff kind, `chance_bp: 0`) fires and
/// applies through the same pipeline.
#[test]
fn monster_blood_slash_casts_and_bleeds() {
    let mut state = GameState::new(42, 16, 8);

    let mut player = Entity::idle(1, Cell::new(0, 0))
        .with_team(Team::Player)
        .with_walk(Cell::new(0, 0), 0)
        .with_combat(/*hp*/ 100_000, /*atk*/ 0, /*def*/ 0,
                     /*atk_speed_milli*/ 0, Element::Fire);
    player.atk_range = 1;
    state.grid.spawn(player);

    let mut crimsonne = Entity::idle(2, Cell::new(1, 0))
        .with_team(Team::Monster)
        .with_walk(Cell::new(1, 0), 0)
        .with_combat(/*hp*/ 5_000, /*atk*/ 100, /*def*/ 0,
                     /*atk_speed_milli*/ 500, Element::Dark);
    crimsonne.atk_range = 1;
    crimsonne.skills = vec![SkillState::new("blood_slash", 1)];
    crimsonne.active_skills = vec!["blood_slash".to_string()];
    state.grid.spawn(crimsonne);

    tick(&mut state, 40);

    let player = &state.grid.entities[0];
    let crimsonne = &state.grid.entities[1];
    assert!(
        crimsonne.skills[0].last_fired > 0,
        "blood_slash never fired"
    );
    assert!(
        player.buffs.iter().any(|b| b.kind == BuffKind::Bleed),
        "blood_slash should have opened a Bleed on the target"
    );
}

/// Bleed now mitigates like a PHYSICAL hit — armor (hard DEF) cuts it, applied ONCE to the
/// whole-lifetime total (so the stored per-tick magnitude is smaller for an armored target,
/// NOT the soft-DEF over-subtracted every interval).
#[test]
fn bleed_per_tick_is_reduced_by_armor() {
    fn bleed_per_tick(def: i32) -> i32 {
        let mut state = GameState::new(42, 16, 8);
        let mut target = Entity::idle(1, Cell::new(0, 0))
            .with_team(Team::Player)
            .with_walk(Cell::new(0, 0), 0)
            .with_combat(/*hp*/ 100_000, /*atk*/ 0, /*def*/ def, /*aspd*/ 0, Element::Fire);
        target.atk_range = 1;
        state.grid.spawn(target);

        let mut bleeder = Entity::idle(2, Cell::new(1, 0))
            .with_team(Team::Monster)
            .with_walk(Cell::new(1, 0), 0)
            .with_combat(/*hp*/ 5_000, /*atk*/ 800, /*def*/ 0, /*aspd*/ 500, Element::Dark);
        bleeder.atk_range = 1;
        bleeder.skills = vec![SkillState::new("blood_slash", 1)];
        bleeder.active_skills = vec!["blood_slash".to_string()];
        state.grid.spawn(bleeder);

        tick(&mut state, 20); // cast (0.8 s) fires, the bleed is applied and still active
        state.grid.entities[0]
            .buffs
            .iter()
            .find(|b| b.kind == BuffKind::Bleed)
            .map(|b| b.magnitude)
            .unwrap_or(0)
    }
    let no_armor = bleed_per_tick(0);
    let armored = bleed_per_tick(200);
    assert!(no_armor > 0, "bleed should be applied to a defenseless target");
    assert!(
        armored < no_armor,
        "armor should cut the per-tick bleed (no_armor={no_armor}, armored={armored})"
    );
}

/// Gloomtooth's Bloodfrenzy PASSIVE: every landed basic hit ramps the biter's OWN ASPD
/// +5%, capped at +100%. Lives in `skills` (a passive), NOT `active_skills` — it fires from
/// the on-landed-hit hook, growing a single self-sourced AspdPct buff toward the cap.
#[test]
fn monster_bloodfrenzy_passive_ramps_aspd() {
    fn aspd_stack(state: &GameState) -> i32 {
        let g = &state.grid.entities[1];
        g.buffs
            .iter()
            .find(|b| b.kind == BuffKind::AspdPct && b.source_id == g.id)
            .map(|b| b.magnitude)
            .unwrap_or(0)
    }

    let mut state = GameState::new(42, 16, 8);

    // A huge-HP dummy so the frenzy has something to keep biting.
    let mut target = Entity::idle(1, Cell::new(0, 0))
        .with_team(Team::Player)
        .with_walk(Cell::new(0, 0), 0)
        .with_combat(/*hp*/ 10_000_000, /*atk*/ 0, /*def*/ 0,
                     /*atk_speed_milli*/ 0, Element::Fire);
    target.atk_range = 1;
    state.grid.spawn(target);

    let mut gloomtooth = Entity::idle(2, Cell::new(1, 0))
        .with_team(Team::Monster)
        .with_walk(Cell::new(1, 0), 0)
        .with_combat(/*hp*/ 5_000, /*atk*/ 50, /*def*/ 0,
                     /*atk_speed_milli*/ 500, Element::Dark);
    gloomtooth.atk_range = 1;
    // Passive → lives in `skills`, NOT `active_skills`; it triggers on each landed hit.
    gloomtooth.skills = vec![SkillState::new("bloodfrenzy", 1)];
    state.grid.spawn(gloomtooth);

    tick(&mut state, 15);
    let early = aspd_stack(&state);
    tick(&mut state, 500);
    let late = aspd_stack(&state);

    assert!(early > 0, "bloodfrenzy should stack ASPD after a few landed hits (got {early})");
    assert!(late > early, "bloodfrenzy should keep ramping (early={early}, late={late})");
    assert_eq!(late, 10_000, "with enough hits it should reach and cap at +100% (got {late})");
}

/// Marionette's Puppet Lunge: a monster reusing the thief's `teleport` + `target_lowest_hp`
/// (Shadow Strike). It starts OUT of melee range with zero move speed, so the ONLY way it can
/// land a hit is by teleporting in — a clean isolation of the lunge.
#[test]
fn monster_puppet_lunge_teleports_and_backstabs() {
    let mut state = GameState::new(42, 16, 8);

    let mut target = Entity::idle(1, Cell::new(0, 0))
        .with_team(Team::Player)
        .with_walk(Cell::new(0, 0), 0)
        .with_combat(/*hp*/ 100_000, /*atk*/ 0, /*def*/ 0,
                     /*atk_speed_milli*/ 0, Element::Fire);
    target.atk_range = 1;
    state.grid.spawn(target);

    // 4 cells away, melee range, and CANNOT walk (speed 0) — only the lunge can reach it.
    let mut marionette = Entity::idle(2, Cell::new(4, 0))
        .with_team(Team::Monster)
        .with_walk(Cell::new(4, 0), 0)
        .with_combat(/*hp*/ 5_000, /*atk*/ 200, /*def*/ 0,
                     /*atk_speed_milli*/ 500, Element::Dark);
    marionette.atk_range = 1;
    marionette.skills = vec![SkillState::new("puppet_lunge", 1)];
    marionette.active_skills = vec!["puppet_lunge".to_string()];
    state.grid.spawn(marionette);

    tick(&mut state, 30);

    let target = &state.grid.entities[0];
    let marionette = &state.grid.entities[1];
    assert!(marionette.skills[0].last_fired > 0, "puppet_lunge never fired");
    assert!(
        target.hp < 100_000,
        "puppet_lunge should have teleported in and landed its backstab (target unharmed)"
    );
}

/// Brutus's Feral Transformation is an ENRAGE: it only fires below 50% HP and only a 50% chance
/// per attempt. Wounded to 40%, over many retry cycles the -50%-incoming buff should land at
/// least once. Also confirms the SEPARATE reused `blood_slash` bite bleeds the target regardless.
#[test]
fn monster_brutus_enrage_transforms_when_wounded() {
    let mut state = GameState::new(42, 16, 8);

    let mut target = Entity::idle(1, Cell::new(0, 0))
        .with_team(Team::Player)
        .with_walk(Cell::new(0, 0), 0)
        .with_combat(/*hp*/ 100_000, /*atk*/ 0, /*def*/ 0,
                     /*atk_speed_milli*/ 0, Element::Fire);
    target.atk_range = 1;
    state.grid.spawn(target);

    let mut brutus = Entity::idle(2, Cell::new(1, 0))
        .with_team(Team::Monster)
        .with_walk(Cell::new(1, 0), 0)
        .with_combat(/*hp*/ 5_000, /*atk*/ 100, /*def*/ 0,
                     /*atk_speed_milli*/ 500, Element::Dark);
    brutus.atk_range = 1;
    brutus.hp = 1_250; // 25% of 5000 max → below the 30% enrage gate (target's atk is 0, stays wounded)
    brutus.skills = vec![
        SkillState::new("feral_transformation", 1),
        SkillState::new("blood_slash", 1),
    ];
    brutus.active_skills = vec![
        "feral_transformation".to_string(),
        "blood_slash".to_string(),
    ];
    state.grid.spawn(brutus);

    // The bite has no HP gate — a few ticks is enough to open a Bleed.
    tick(&mut state, 20);
    assert!(
        state.grid.entities[0].buffs.iter().any(|b| b.kind == BuffKind::Bleed),
        "the SEPARATE bleeding bite (blood_slash) should open a Bleed on the target"
    );

    // The enrage is 50%/cycle (cd 100). Over ~30 cycles it is all-but-certain to land at least once.
    let mut transformed = state.grid.entities[1].buffs.iter().any(|b| b.kind == BuffKind::DmgReducePct);
    for _ in 0..30 {
        if transformed {
            break;
        }
        tick(&mut state, 100);
        transformed = state.grid.entities[1].buffs.iter().any(|b| b.kind == BuffKind::DmgReducePct);
    }
    assert!(transformed, "Feral Transformation should enrage (-50% incoming) within ~30 cycles while wounded");

    // Sanity: at FULL HP it must NOT enrage. Fresh Brutus, above the gate, ticked a while.
    let mut healthy = GameState::new(7, 16, 8);
    let mut t2 = Entity::idle(1, Cell::new(0, 0))
        .with_team(Team::Player).with_walk(Cell::new(0, 0), 0)
        .with_combat(100_000, 0, 0, 0, Element::Fire);
    t2.atk_range = 1;
    healthy.grid.spawn(t2);
    let mut b2 = Entity::idle(2, Cell::new(1, 0))
        .with_team(Team::Monster).with_walk(Cell::new(1, 0), 0)
        .with_combat(5_000, 100, 0, 500, Element::Dark);
    b2.atk_range = 1;
    b2.skills = vec![SkillState::new("feral_transformation", 1)];
    b2.active_skills = vec!["feral_transformation".to_string()];
    healthy.grid.spawn(b2);
    tick(&mut healthy, 500);
    assert!(
        !healthy.grid.entities[1].buffs.iter().any(|b| b.kind == BuffKind::DmgReducePct),
        "Feral Transformation must NOT fire while the wolf is above 50% HP"
    );
}

/// Banshee's Wail: a monster reusing the GENERALIZED Aspd Dampening path (now driven by
/// `aoe_aspd_slow_bp`, not the hardcoded acolyte skill id). One scream slows the ASPD of EVERY
/// party member in the 5×5 — confirms the AoE debuff lands on multiple targets from a mob cast.
#[test]
fn monster_wail_aoe_slows_party_aspd() {
    let mut state = GameState::new(42, 16, 8);

    // Two party members standing together — both should be caught by the 5×5 scream.
    for (id, cell) in [(1u32, Cell::new(0, 0)), (2, Cell::new(1, 0))] {
        let mut p = Entity::idle(id, cell)
            .with_team(Team::Player)
            .with_walk(cell, 0)
            .with_combat(/*hp*/ 100_000, /*atk*/ 0, /*def*/ 0,
                         /*atk_speed_milli*/ 1_000, Element::Fire);
        p.atk_range = 1;
        state.grid.spawn(p);
    }

    let mut banshee = Entity::idle(3, Cell::new(4, 0))
        .with_team(Team::Monster)
        .with_walk(Cell::new(4, 0), 0)
        .with_combat(/*hp*/ 5_000, /*atk*/ 100, /*def*/ 0,
                     /*atk_speed_milli*/ 500, Element::Spirit);
    // Basic range 1 + zero move speed: it is 3-4 cells from the party so it CANNOT basic-attack —
    // only the Wail (range 6) reaches. Isolates the scream so the 0-damage check is about the wail.
    banshee.atk_range = 1;
    banshee.skills = vec![SkillState::new("wail", 1)];
    banshee.active_skills = vec!["wail".to_string()];
    state.grid.spawn(banshee);

    tick(&mut state, 40); // cast_time 15 + resolve

    let slowed = |idx: usize| {
        state.grid.entities[idx]
            .buffs
            .iter()
            .any(|b| b.kind == BuffKind::AspdSlowPct)
    };
    assert!(slowed(0), "Wail should slow party member 0's ASPD");
    assert!(slowed(1), "Wail should slow party member 1's ASPD too (5×5 AoE)");
    // Pure debuff — the scream deals NO damage at all (no min-1 floor).
    assert_eq!(state.grid.entities[0].hp, 100_000, "Wail must deal zero damage");
    assert_eq!(state.grid.entities[1].hp, 100_000, "Wail must deal zero damage");
}

/// Wail fires ONCE per spawn (user 2026-10-19, `max_charges` 1): the -15% slow lands, wears off,
/// and the banshee never screams again.
#[test]
fn monster_wail_casts_once_per_spawn() {
    let mut state = GameState::new(42, 16, 8);
    let mut p = Entity::idle(1, Cell::new(0, 0))
        .with_team(Team::Player)
        .with_walk(Cell::new(0, 0), 0)
        .with_combat(/*hp*/ 100_000, /*atk*/ 0, /*def*/ 0,
                     /*atk_speed_milli*/ 1_000, Element::Fire);
    p.atk_range = 1;
    state.grid.spawn(p);
    let mut banshee = Entity::idle(2, Cell::new(4, 0))
        .with_team(Team::Monster)
        .with_walk(Cell::new(4, 0), 0)
        .with_combat(/*hp*/ 5_000, /*atk*/ 100, /*def*/ 0,
                     /*atk_speed_milli*/ 500, Element::Spirit);
    banshee.atk_range = 1;
    banshee.skills = vec![SkillState::new("wail", 1)];
    banshee.active_skills = vec!["wail".to_string()];
    state.grid.spawn(banshee);

    let slowed = |s: &GameState| s.grid.entities[0].buffs.iter().any(|b| b.kind == BuffKind::AspdSlowPct);
    tick(&mut state, 40);
    assert!(slowed(&state), "the first wail lands");
    assert_eq!(state.grid.entities[1].skills[0].charges_left, 0, "its one charge is spent");
    tick(&mut state, 1_200);
    assert!(!slowed(&state), "the slow wore off and was never cast again");
}

/// Dustelle's Mending Dirge: a MONSTER AoE healer via the shared `try_cast_support` path (fires
/// even with no enemy). It centres on the most-injured ally and mends EVERY ally in the 5×5 for
/// 15% of THAT ally's max HP (`heal_maxhp_bp`) — scaling with a monster's big pool, not the
/// near-zero INT/MATK Renewal amount. Two hurt pack-mates → BOTH healed by one cast.
#[test]
fn monster_mending_dirge_aoe_heals_injured_allies() {
    let mut state = GameState::new(42, 16, 8);

    // Two injured pack-mates, both in the 5×5 around Dustelle.
    for (id, cell) in [(1u32, Cell::new(1, 0)), (2, Cell::new(0, 1))] {
        let mut ally = Entity::idle(id, cell)
            .with_team(Team::Monster)
            .with_walk(cell, 0)
            .with_combat(/*hp*/ 10_000, /*atk*/ 100, /*def*/ 0,
                         /*atk_speed_milli*/ 500, Element::Dark);
        ally.atk_range = 1;
        ally.hp = 3_000; // 30% of 10_000
        state.grid.spawn(ally);
    }

    let mut dustelle = Entity::idle(3, Cell::new(0, 0))
        .with_team(Team::Monster)
        .with_walk(Cell::new(0, 0), 0)
        .with_combat(/*hp*/ 5_000, /*atk*/ 50, /*def*/ 0,
                     /*atk_speed_milli*/ 500, Element::Physical);
    dustelle.atk_range = 1;
    dustelle.skills = vec![SkillState::new("mending_dirge", 1)];
    dustelle.active_skills = vec!["mending_dirge".to_string()];
    state.grid.spawn(dustelle);

    let before = [state.grid.entities[0].hp, state.grid.entities[1].hp];
    tick(&mut state, 20);
    for idx in 0..2 {
        let healed = state.grid.entities[idx].hp - before[idx];
        assert!(
            (900..=1_100).contains(&healed),
            "ally {idx} should be AoE-healed ~10% of its 10k max HP (0.0.419; got {healed})"
        );
    }
}

/// Ashelle's Frenzied Rite: a MONSTER party-buff via `cast_buff_party` (the pure-buff cast path,
/// fires even with no enemy). It goads the whole pack — every ally, herself included, gains +30%
/// ASPD.
#[test]
fn monster_frenzied_rite_buffs_pack_aspd() {
    let mut state = GameState::new(42, 16, 8);

    let mut ally = Entity::idle(1, Cell::new(1, 0))
        .with_team(Team::Monster)
        .with_walk(Cell::new(1, 0), 0)
        .with_combat(/*hp*/ 5_000, /*atk*/ 100, /*def*/ 0,
                     /*atk_speed_milli*/ 500, Element::Dark);
    ally.atk_range = 1;
    state.grid.spawn(ally);

    let mut ashelle = Entity::idle(2, Cell::new(0, 0))
        .with_team(Team::Monster)
        .with_walk(Cell::new(0, 0), 0)
        .with_combat(/*hp*/ 5_000, /*atk*/ 50, /*def*/ 0,
                     /*atk_speed_milli*/ 500, Element::Dark);
    ashelle.atk_range = 1;
    ashelle.skills = vec![SkillState::new("frenzied_rite", 1)];
    ashelle.active_skills = vec!["frenzied_rite".to_string()];
    state.grid.spawn(ashelle);

    tick(&mut state, 30); // cast_time 15 + resolve

    let has_aspd =
        |idx: usize| state.grid.entities[idx].buffs.iter().any(|b| b.kind == BuffKind::AspdPct);
    assert!(has_aspd(0), "Frenzied Rite should buff the pack-mate's ASPD");
    assert!(has_aspd(1), "Frenzied Rite should buff Ashelle herself too (party buff)");
}

/// Mending Dirge is limited to TWO casts per maid per spawn (user 2026-10-19, `max_charges` 2):
/// over a minute of injured allies (six cooldowns) she heals exactly twice, at full strength.
#[test]
fn monster_mending_dirge_casts_twice_per_spawn() {
    let mut state = GameState::new(42, 16, 8);
    let mut ally = Entity::idle(1, Cell::new(1, 0))
        .with_team(Team::Monster)
        .with_walk(Cell::new(1, 0), 0)
        .with_combat(/*hp*/ 10_000, /*atk*/ 100, /*def*/ 0,
                     /*atk_speed_milli*/ 500, Element::Dark);
    ally.atk_range = 1;
    ally.hp = 3_000;
    state.grid.spawn(ally);
    let mut dustelle = Entity::idle(2, Cell::new(0, 0))
        .with_team(Team::Monster)
        .with_walk(Cell::new(0, 0), 0)
        .with_combat(/*hp*/ 5_000, /*atk*/ 50, /*def*/ 0,
                     /*atk_speed_milli*/ 500, Element::Physical);
    dustelle.atk_range = 1;
    dustelle.skills = vec![SkillState::new("mending_dirge", 1)];
    dustelle.active_skills = vec!["mending_dirge".to_string()];
    state.grid.spawn(dustelle);

    tick(&mut state, 600);
    let healed = state.grid.entities[0].hp - 3_000;
    assert!((1_900..=2_200).contains(&healed), "two full-strength casts (~1000 each), got {healed}");
    assert_eq!(state.grid.entities[1].skills[0].charges_left, 0, "both charges spent");
}

/// Frenzied Rite fires ONCE per spawn (user 2026-10-19, `max_charges` 1): the +30% pack buff lands,
/// expires, and is not cast again.
#[test]
fn monster_frenzied_rite_casts_once_per_spawn() {
    let mut state = GameState::new(42, 16, 8);
    let mut ashelle = Entity::idle(1, Cell::new(0, 0))
        .with_team(Team::Monster)
        .with_walk(Cell::new(0, 0), 0)
        .with_combat(/*hp*/ 5_000, /*atk*/ 50, /*def*/ 0,
                     /*atk_speed_milli*/ 500, Element::Dark);
    ashelle.atk_range = 1;
    ashelle.skills = vec![SkillState::new("frenzied_rite", 1)];
    ashelle.active_skills = vec!["frenzied_rite".to_string()];
    state.grid.spawn(ashelle);

    let aspd = |s: &GameState| s.grid.entities[0].buffs.iter().any(|b| b.kind == BuffKind::AspdPct);
    tick(&mut state, 30);
    assert!(aspd(&state), "the first rite lands");
    tick(&mut state, 600);
    assert!(!aspd(&state), "the buff expired and was never cast again");
    assert_eq!(state.grid.entities[0].skills[0].charges_left, 0);
}

/// Twinling's Vanish is `move_while_hidden`: while under the `Hiding` buff it keeps STALKING —
/// stepping toward its target — instead of standing still. A control hider WITHOUT the flag holds
/// position (the thief's panic hide), proving the behavior rides the SKILL, not the Hiding buff.
#[test]
fn monster_vanish_stalks_while_hidden() {
    let mut state = GameState::new(42, 16, 8);

    // The prey both hiders target.
    let mut player = Entity::idle(1, Cell::new(0, 0))
        .with_team(Team::Player)
        .with_walk(Cell::new(0, 0), 0)
        .with_combat(/*hp*/ 1_000_000, /*atk*/ 0, /*def*/ 0,
                     /*atk_speed_milli*/ 0, Element::Fire);
    player.atk_range = 1;
    state.grid.spawn(player);

    // Twinling — hidden AND carries Vanish (move_while_hidden) → stalks toward its target.
    let mut tw = Entity::idle(2, Cell::new(6, 0))
        .with_team(Team::Monster)
        .with_walk(Cell::new(6, 0), 100)
        .with_combat(/*hp*/ 5_000, /*atk*/ 100, /*def*/ 0,
                     /*atk_speed_milli*/ 500, Element::Dark);
    tw.atk_range = 1;
    tw.target = Cell::new(0, 0);
    tw.skills = vec![SkillState::new("vanish", 1)];
    tw.active_skills = vec!["vanish".to_string()];
    tw.buffs.push(Buff { kind: BuffKind::Hiding, magnitude: 1, ticks: 200, source_id: 2 });
    state.grid.spawn(tw);

    // Control — hidden, same target + speed, but NO vanish skill → holds position.
    let mut ctrl = Entity::idle(3, Cell::new(6, 3))
        .with_team(Team::Monster)
        .with_walk(Cell::new(6, 3), 100)
        .with_combat(/*hp*/ 5_000, /*atk*/ 100, /*def*/ 0,
                     /*atk_speed_milli*/ 500, Element::Dark);
    ctrl.atk_range = 1;
    ctrl.target = Cell::new(0, 3);
    ctrl.buffs.push(Buff { kind: BuffKind::Hiding, magnitude: 1, ticks: 200, source_id: 3 });
    state.grid.spawn(ctrl);

    let tw_x0 = state.grid.entities[1].pos.x;
    let ctrl_x0 = state.grid.entities[2].pos.x;
    tick(&mut state, 20);

    assert!(
        state.grid.entities[1].pos.x < tw_x0,
        "Twinling (move_while_hidden) should stalk toward the prey while hidden (x {tw_x0} -> {})",
        state.grid.entities[1].pos.x
    );
    assert_eq!(
        state.grid.entities[2].pos.x, ctrl_x0,
        "a hider WITHOUT the flag must hold position (thief panic hide)"
    );
    // Both stayed hidden (200-tick buff) the whole window.
    assert!(state.grid.entities[1].buffs.iter().any(|b| b.kind == BuffKind::Hiding));
}

/// Scarlette's Quick Step: a pure self-buff (fires proactively via try_cast_buffs) — she gains
/// +Flee (evasion) AND +ASPD, the nimble dancer's footwork.
#[test]
fn monster_quick_step_self_buffs_flee_and_aspd() {
    let mut state = GameState::new(42, 16, 8);

    let mut scarlette = Entity::idle(1, Cell::new(0, 0))
        .with_team(Team::Monster)
        .with_walk(Cell::new(0, 0), 0)
        .with_combat(/*hp*/ 5_000, /*atk*/ 100, /*def*/ 0,
                     /*atk_speed_milli*/ 500, Element::Physical);
    scarlette.atk_range = 1;
    scarlette.skills = vec![SkillState::new("quick_step", 1)];
    scarlette.active_skills = vec!["quick_step".to_string()];
    state.grid.spawn(scarlette);

    tick(&mut state, 20); // cast_time 8 + resolve

    let s = &state.grid.entities[0];
    assert!(
        s.buffs.iter().any(|b| b.kind == BuffKind::DodgeBpAdd),
        "Quick Step should self-buff dodge (evasion)"
    );
    assert!(
        s.buffs.iter().any(|b| b.kind == BuffKind::AspdPct),
        "Quick Step should self-buff ASPD"
    );
}

/// Broomhilda's Witch's Fireball: a monster Fire nuke that SPLASHES a 3×3. She can't basic-reach
/// (atk_range 1, prey 2-3 cells off, zero move) so any damage to BOTH clustered players is the AoE.
#[test]
fn monster_witch_fireball_aoe_splashes() {
    let mut state = GameState::new(42, 16, 8);
    for (id, cell) in [(1u32, Cell::new(0, 0)), (2, Cell::new(1, 0))] {
        let mut p = Entity::idle(id, cell)
            .with_team(Team::Player)
            .with_walk(cell, 0)
            .with_combat(/*hp*/ 100_000, /*atk*/ 0, /*def*/ 0,
                         /*atk_speed_milli*/ 0, Element::Physical);
        p.atk_range = 1;
        state.grid.spawn(p);
    }
    let mut witch = Entity::idle(3, Cell::new(3, 0))
        .with_team(Team::Monster)
        .with_walk(Cell::new(3, 0), 0)
        .with_combat(/*hp*/ 5_000, /*atk*/ 500, /*def*/ 0,
                     /*atk_speed_milli*/ 60, Element::Dark);
    witch.atk_range = 6; // engaged; basics target the NEAREST (player 1), so any hit on player 0 is the AoE
    witch.skills = vec![SkillState::new("witch_fireball", 1)];
    witch.active_skills = vec!["witch_fireball".to_string()];
    state.grid.spawn(witch);

    tick(&mut state, 60); // engage + cast_time 20 + projectile
    // Player 0 is NOT the nearest (that's player 1) — basics never touch it, so damage here is
    // purely the Fireball's 3×3 splash.
    assert!(state.grid.entities[0].hp < 100_000, "Fireball's 3x3 AoE should splash player 0");
    assert!(state.grid.entities[1].hp < 100_000, "Fireball should hit player 1 (primary)");
}

/// Broomhilda's Summon Bats: spawns 2 familiars scaled off HER (20% max HP), and — the anti-farm
/// invariant — they carry NO XP and NO drops (an infinite summon must never be a loot faucet).
#[test]
fn monster_summon_bats_spawns_scaled_farm_safe_familiars() {
    let mut state = GameState::new(42, 24, 12);
    // An enemy must exist for the summon to fire.
    let mut player = Entity::idle(1, Cell::new(0, 0))
        .with_team(Team::Player)
        .with_walk(Cell::new(0, 0), 0)
        .with_combat(/*hp*/ 1_000_000, /*atk*/ 0, /*def*/ 0,
                     /*atk_speed_milli*/ 0, Element::Fire);
    player.atk_range = 1;
    state.grid.spawn(player);

    let mut witch = Entity::idle(2, Cell::new(12, 6))
        .with_team(Team::Monster)
        .with_walk(Cell::new(12, 6), 0)
        .with_combat(/*hp*/ 10_000, /*atk*/ 500, /*def*/ 0,
                     /*atk_speed_milli*/ 60, Element::Dark);
    witch.atk_range = 6;
    witch.skills = vec![SkillState::new("summon_bats", 1)];
    witch.active_skills = vec!["summon_bats".to_string()];
    state.grid.spawn(witch);

    tick(&mut state, 15);

    let bats: Vec<&sim_core::grid::Entity> =
        state.grid.entities.iter().filter(|e| e.def_id == "bat").collect();
    assert_eq!(bats.len(), 2, "Summon Bats should spawn exactly 2 bats");
    // Scaled off the summoner: 20% of 10_000 max HP = 2_000.
    assert!(
        (1_800..=2_200).contains(&bats[0].hp),
        "a bat's HP should be ~20% of the summoner's (got {})",
        bats[0].hp
    );
    // Anti-farm invariant: summoned familiars mint NOTHING.
    assert_eq!(bats[0].xp_reward, 0, "summoned bats must grant no XP");
    assert!(bats[0].drops.is_empty(), "summoned bats must drop nothing");
}

/// UMBRAL ROOTS AGAINST A REAL TARGET — the test the one above should have been.
///
/// `monster_ground_field_casts_slows_and_ticks` fights a STAT_MIN dummy whose soft MDEF is about 1,
/// so its `player.hp < 100_000` was true no matter what the field did. `.max(1)` guarantees a
/// ground field always deals SOMETHING, which makes a boolean assertion permanently green — and it
/// stayed green through the entire window in which this field dealt 10 total damage.
///
/// The field is TEN packets and the magic path subtracts soft MDEF from EACH. A level-23 mage
/// carries 53 of it. At the authored 25% a tick was 18 and every one of them floored to 1.
///
/// So this MEASURES, against a caster and a victim built at the band `demon_tree` actually stands
/// in (levels 21-25). The bound is a floor, not a pin: the field must land a real fraction of what
/// it swings for, which is the property that broke.
#[test]
fn the_umbral_roots_field_is_not_swallowed_whole_by_a_real_targets_soft_mdef() {
    // demon_tree's ATK where it lives: `level_base[23].atk` x its multiplier, read from the
    // shipped catalog so this tracks the balance rather than a number typed here.
    let d = sim_core::data::GameData::from_json_str(include_str!(
        "../../../../data/game_data.json"
    ))
    .expect("game_data.json must parse");
    let caster_atk = {
        let m = d.monster("demon_tree").expect("demon_tree is in the catalog");
        let base = d.stage_stats(56).atk as i64; // stage 56 = the old Lv23
        let bp = if m.atk_mult_bp == 0 { 10_000 } else { m.atk_mult_bp } as i64;
        (base * bp / 10_000) as i32
    };
    assert!(caster_atk > 30, "demon_tree's ATK at level 23 came out {caster_atk} — re-derive this");

    let mut state = GameState::new(42, 16, 8);

    // A level-23 mage, not a dummy: INT is what soft MDEF is made of, and it is the whole point.
    let mut player = Entity::idle(1, Cell::new(0, 0))
        .with_team(Team::Player)
        .with_walk(Cell::new(0, 0), 0)
        .with_combat(/*hp*/ 100_000, /*atk*/ 0, /*def*/ 0,
                     /*atk_speed_milli*/ 0, Element::Physical);
    player.atk_range = 1;
    // A REAL level-23 mage. MDEF is gone entirely (0.0.332) so what makes this target
    // real is its LEVEL, which is what soft DEF and the DEF curve both read.
    player.stats.level = 23;
    let soft = sim_core::combat::effective_soft_def(&player);
    assert!(
        (100..=130).contains(&soft),
        "this target's soft DEF came out {soft}, expected 110 (level 23 / 2, x the stat scale) — \
         either the level curve moved or the number above is stale. It has to be a REAL level-23 \
         mage: too low and this is the dummy fight the other test already is, too high and it \
         fails a field that is fine in play."
    );
    state.grid.spawn(player);

    // SEVEN cells away: inside the skill's 8-cell cast range, outside its 3-cell attack range,
    // and neither side can walk (speed 0). The first version of this test stood them two apart and
    // measured total HP loss — which included the caster's ordinary PHYSICAL attacks, and those
    // land fine. They masked a field dealing 10, and the test passed at 25% as happily as at 90%.
    let mut caster = Entity::idle(2, Cell::new(7, 0))
        .with_team(Team::Monster)
        .with_walk(Cell::new(7, 0), 0)
        .with_combat(/*hp*/ 9_000, caster_atk, /*def*/ 0,
                     /*atk_speed_milli*/ 500, Element::Earth);
    caster.atk_range = 3;
    caster.skills = vec![SkillState::new("umbral_roots", 1)];
    caster.active_skills = vec!["umbral_roots".to_string()];
    state.grid.spawn(caster);

    let before = state.grid.entities[0].hp;
    tick(&mut state, 90); // 2 s wind-up + the 5 s field, with room to spare
    let dealt = before - state.grid.entities[0].hp;

    // What the field SWINGS for, before any mitigation: `field_hits` ticks of `power_bp` x ATK,
    // both read off the def so this tracks a retune instead of pinning today's numbers.
    let def = sim_core::skills::skill_def("umbral_roots").expect("umbral_roots is in the catalog");
    let swing = (caster_atk as i64) * def.power_bp as i64 / 10_000 * def.field_hits as i64;
    assert!(swing > 0, "the field swings for nothing — power_bp or field_hits is 0");
    // A THIRD of what it swings for. Measured: at the shipped 80% this lands 44% (260 of 580) and
    // at 25% it landed 6% (10 of 180). The bound sits between with room on both sides — it is a
    // floor on "did the flat term eat it", not a pin on today's tuning.
    assert!(
        dealt as i64 * 3 >= swing,
        "the field swung for {swing} and landed {dealt} — under a quarter. Its per-tick is at or \
         below the target's soft MDEF ({soft}), so `.max(1)` is eating it ten times over. Raising \
         `power_bp` moves the cliff; it does not remove it."
    );
    assert!(
        dealt > 10,
        "{dealt} total damage over five seconds is the floored case verbatim (ten ticks x 1)"
    );
}
