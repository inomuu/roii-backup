use super::*;
use crate::events::{apply_input, InputEvent};
use crate::grid::Cell;

fn specimen() -> GameState {
    let mut s = GameState::new(42, 12, 12);
    let mut e = Entity::idle(1, Cell::new(0, 0)).with_team(Team::Player);
    e.unit_def = "seraphine".into();
    // Level 1 on purpose: rank 5 is open from level 1, which covers every rank these fixtures
    // reach for; the gate test raises the level itself.
    s.grid.entities.push(e);
    rich(&mut s);
    s
}
/// Stock the ledger with every family the ladder can ask for, in plenty, plus Gil — so a fixture
/// tests the RULE and not a shortage. Sigil has no element (2026-09-30).
fn stock(s: &mut GameState, sigil: [u32; 3], core: u32, track: u32, gil: u64) {
    use crate::state::MaterialSource;
    // Start from an empty ledger so a fixture states its whole stock.
    let held: Vec<(String, u32)> = s.materials().map(|(k, v)| (k.to_string(), v)).collect();
    for (id, n) in held {
        s.debit_material(&id, n);
    }
    let mut push = |id: String, n: u32| s.credit_material(&id, n, MaterialSource::Drop);
    for g in 0..3 {
        push(sigil_def(g), sigil[g]);
    }
    push(BOSS_CORE_DEF.to_string(), core);
    push(TRACK_DEF.to_string(), track);
    s.set_gil_for_test(gil);
}
fn rich(s: &mut GameState) {
    stock(s, [999, 999, 999], 999, 999, 10_000_000_000);
}
fn sig(s: &GameState) -> [u32; 3] {
    purse(s).sigil
}
fn entity(s: &GameState) -> &Entity {
    s.grid.entity_of_id(1).expect("fixture member")
}
fn buy(s: &mut GameState, id: &str) {
    apply_input(
        s,
        InputEvent::UpgradePotential {
            entity_id: 1,
            node_id: id.into(),
        },
    );
}

#[test]
fn invalid_and_locked_inputs_preserve_points_and_state() {
    let mut s = specimen();
    let before = s.clone();
    for id in ["unknown", "root", "A", "C", "k4", "k1"] {
        buy(&mut s, id);
        assert_eq!(s, before, "invalid {id}");
    }
    stock(&mut s, [0, 0, 0], 0, 0, 0);
    let before = s.clone();
    buy(&mut s, "s1");
    assert_eq!(s, before, "no materials → nothing bought, nothing charged");
}

#[test]
fn purchase_changes_real_skill_and_charges_shards_and_never_resets() {
    let mut s = specimen();
    let e = s.grid.entities.first_mut().expect("member");
    e.skills
        .push(crate::skills::SkillState::new("iron_chains", 3));
    e.skills.push(crate::skills::SkillState::new("bulwark", 4));
    assert_eq!(effective_board(entity(&s)).rank("k1"), 3);
    buy(&mut s, "k1");
    buy(&mut s, "s4");
    buy(&mut s, "A");
    assert_eq!(entity(&s).skill_level("iron_chains"), 4);
    // k1 3→4 = Sigil I ×8 + 60k; s4 is T2 (II ×6 + 500k); A is M1 (III ×8 + Core 2 + 4M).
    assert_eq!(sig(&s), [991, 993, 991]);
    assert_eq!(purse(&s).core, 997);
    assert_eq!(s.gil(), 10_000_000_000 - 4_560_000);
    assert_eq!(entity(&s).potential.spent_points, 3, "one ledger entry per purchase");
    // NO RESET: the tombstone parses and changes nothing.
    let before = s.clone();
    apply_input(&mut s, InputEvent::ResetPotential { entity_id: 1 });
    assert_eq!(s, before);
    assert_eq!(entity(&s).skill_level("iron_chains"), 4);
}

#[test]
fn new_skill_is_slotted_and_a_tombstone_reset_keeps_it() {
    let mut s = specimen();
    for id in ["s1", "s2", "k1"] {
        buy(&mut s, id);
    }
    assert_eq!(entity(&s).skill_level("iron_chains"), 1);
    assert!(entity(&s)
        .active_skills
        .iter()
        .any(|id| id == "iron_chains"));
    apply_input(&mut s, InputEvent::ResetPotential { entity_id: 1 });
    assert_eq!(entity(&s).skill_level("iron_chains"), 1, "a tombstone reset takes nothing away");
    assert!(entity(&s)
        .active_skills
        .iter()
        .any(|id| id == "iron_chains"));
    // s1, s2 = Sigil I ×6 each; k1 0→1 rides the rank-2 row = Sigil I ×4.
    assert_eq!(sig(&s), [983, 999, 999]);
}

#[test]
fn max_quote_matches_affordable_execution_without_overdraft() {
    let mut s = specimen();
    // Exactly the route's price and not a Sigil or a Gil more: s1 + s2 (6 + 6, 10k each) + k1
    // rank-2 row (4, 10k).
    stock(&mut s, [16, 0, 0], 0, 0, 30_000);
    let plan = max_plan(entity(&s), &purse(&s), "A").expect("reachable target");
    assert_eq!(plan, vec!["s1", "s2", "k1"]);
    apply_input(
        &mut s,
        InputEvent::MaxPotential {
            entity_id: 1,
            target_id: "A".into(),
        },
    );
    assert_eq!((sig(&s), s.gil()), ([0, 0, 0], 0), "spent exactly to the quoted plan");
    assert_eq!(entity(&s).potential.spent_points, 3);
    assert_eq!(entity(&s).potential.rank("k1"), 1);
    assert_eq!(entity(&s).potential.rank("A"), 0);
}

#[test]
fn point_budget_covers_real_reachable_major_route() {
    let mut s = specimen();
    let plan = max_plan(entity(&s), &purse(&s), "B").expect("route");
    assert_eq!(plan.len(), 3); // root -> ATK -> Steel Break -> Iron Wall
    apply_input(
        &mut s,
        InputEvent::MaxPotential {
            entity_id: 1,
            target_id: "B".into(),
        },
    );
    // s1 is T1 (I ×6), k2 0→1 rides the rank-2 row (I ×4), B is M1 (III ×8 + Core 2).
    assert_eq!(sig(&s), [989, 999, 991]);
    assert_eq!(purse(&s).core, 997);
    assert_eq!(entity(&s).potential.rank("B"), 1);
    assert_eq!(entity(&s).skill_level("steel_break"), 1);
}

#[test]
fn investments_survive_serialization_and_character_parking() {
    let mut s = specimen();
    let e = s.grid.entities.first_mut().expect("member");
    e.atk = 1_000;
    e.def = 4_000;
    e.skills
        .push(crate::skills::SkillState::new("steel_break", 2)); // deliberately unslotted
    for id in ["s1", "s2", "k1", "s4", "A"] {
        buy(&mut s, id);
    }
    assert_eq!(crate::combat::effective_atk(entity(&s)), 2_070);
    let saved = serde_json::to_string(&s).expect("save");
    let loaded: GameState = serde_json::from_str(&saved).expect("load");
    assert_eq!(s, loaded);
    let e = s.grid.entities.first_mut().expect("member");
    let parked = crate::state::CharacterData::capture(e);
    assert_eq!(parked.potential.spent_points, 5);
    assert_eq!(parked.active_skills, vec!["iron_chains"]);
    let parked_json = serde_json::to_string(&parked).expect("parked save");
    let parked: crate::state::CharacterData =
        serde_json::from_str(&parked_json).expect("parked load");
    let mut restored = Entity::idle(99, Cell::new(0, 0)).with_team(Team::Player);
    restored.atk = 1_000;
    restored.def = 4_000;
    restored.unit_def = "seraphine".into();
    parked.restore_onto(&mut restored);
    assert_eq!(restored.potential.rank("k1"), 1);
    assert_eq!(restored.potential.spent_points, 5);
    assert_eq!(restored.skill_level("iron_chains"), 1);
    assert!(
        restored.skill_slotted("iron_chains"),
        "paid active remains enabled after redeploy"
    );
    assert_eq!(restored.skill_level("steel_break"), 2);
    assert!(
        !restored.skill_slotted("steel_break"),
        "do not enable an intentionally unslotted skill"
    );
    assert_eq!(
        restored.potential_atk_from_def, 1_000,
        "Iron Edge is ready before the next periodic tick"
    );
    assert_eq!(crate::combat::effective_atk(&restored), 2_070);
}

#[test]
fn the_ladder_runs_to_twenty_with_no_level_gate_and_an_overlay_still_refuses() {
    // v2: a Lv1 unit may buy any rank the materials cover. iron_chains (max_level 10) at 5 → 6
    // costs the rank-6 row; past 10 every rank is an overrank, charged from the same ladder,
    // and stops at SKILL_RANK_CAP.
    let mut s = specimen();
    let e = s.grid.entities.first_mut().expect("member");
    e.stats.level = 1;
    e.skills.push(crate::skills::SkillState::new("iron_chains", 5));
    buy(&mut s, "k1");
    assert_eq!(entity(&s).skill_level("iron_chains"), 6);
    assert_eq!(sig(&s), [999, 991, 999], "rank 6 = Sigil II ×8");
    assert_eq!(s.gil(), 10_000_000_000 - 300_000);
    for _ in 0..30 {
        buy(&mut s, "k1");
    }
    assert_eq!(entity(&s).skill_level("iron_chains"), SKILL_RANK_CAP, "capped at 20, not at max_level");
    let def = crate::skills::skill_def("iron_chains").unwrap();
    assert_eq!(def.power_at(SKILL_RANK_CAP), def.power_at(def.max_level) * 12_000 / 10_000,
               "ten overranks = +20% on the level-10 number");
    // A skill supplied by an overlay is never bought.
    let mut s = specimen();
    let e = s.grid.entities.first_mut().expect("member");
    let mut k = crate::skills::SkillState::new("iron_chains", 5);
    k.from_exemplar = true;
    e.skills.push(k);
    let before = s.clone();
    buy(&mut s, "k1");
    assert_eq!(s, before);
}

#[test]
fn legacy_skill_entry_uses_the_board_ledger_after_opt_in() {
    let mut s = specimen();
    let e = s.grid.entities.first_mut().expect("member");
    e.skills
        .push(crate::skills::SkillState::new("iron_chains", 3));
    buy(&mut s, "s4");
    apply_input(
        &mut s,
        InputEvent::LevelSkill {
            entity_id: 1,
            skill_id: "iron_chains".into(),
        },
    );
    assert_eq!(entity(&s).skill_level("iron_chains"), 4);
    // s4 is T2 (II ×6); the legacy LevelSkill rode the ledger for rank 4 = Sigil I ×8.
    assert_eq!(entity(&s).potential.spent_points, 2);
    assert_eq!(sig(&s), [991, 993, 999]);
    apply_input(&mut s, InputEvent::ResetPotential { entity_id: 1 });
    assert_eq!(entity(&s).skill_level("iron_chains"), 4, "no reset: the rank stays");
}

#[test]
fn recorded_upgrade_and_reset_replay_identically() {
    let start = specimen();
    let mut actual = start.clone();
    actual.record_inputs = true;
    for id in ["s1", "s2", "k1", "s4", "A"] {
        buy(&mut actual, id);
    }
    apply_input(&mut actual, InputEvent::ResetPotential { entity_id: 1 });
    actual.record_inputs = false;
    let log = std::mem::take(&mut actual.input_log);
    assert_eq!(log.len(), 6);
    let mut replayed = start;
    for input in log {
        apply_input(&mut replayed, input.event);
    }
    assert_eq!(actual, replayed);
}

#[test]
fn reset_and_rebuy_do_not_reset_a_skills_combat_clock() {
    let mut s = specimen();
    for id in ["s1", "s2", "k1"] {
        buy(&mut s, id);
    }
    let e = s.grid.entities.first_mut().expect("member");
    let skill = e
        .skills
        .iter_mut()
        .find(|x| x.id == "iron_chains")
        .expect("bought skill");
    skill.cooldown = 73;
    skill.last_fired = 10;
    apply_input(&mut s, InputEvent::ResetPotential { entity_id: 1 });
    let before = s.clone();
    apply_input(
        &mut s,
        InputEvent::ToggleActiveSkill {
            entity_id: 1,
            skill_id: "iron_chains".into(),
        },
    );
    assert_ne!(s, before, "the free rank-one skill remains available to toggle");
    for id in ["s1", "s2", "k1"] {
        buy(&mut s, id);
    }
    let e = entity(&s);
    let skill = e
        .skills
        .iter()
        .find(|x| x.id == "iron_chains")
        .expect("rebought skill");
    assert_eq!(skill.level, 2);
    assert_eq!(skill.cooldown, 73);
    assert_eq!(skill.last_fired, 10);
    assert!(!e.active_skills.iter().any(|s| s == "iron_chains"), "upgrade respects the skill disabled above");
}

/// Sigil conversion: 3 → 1 up, 1 → 3 down, lossless; short / bad input is a no-op. The logged
/// `element` field is ignored (Sigils have none since 2026-09-30).
#[test]
fn sigil_merge_and_split_are_lossless_and_bounded() {
    let mut s = specimen();
    stock(&mut s, [7, 2, 0], 0, 0, 0);
    let el = element_id(entity(&s).element).to_string();
    apply_input(&mut s, InputEvent::MergeSigil { element: el.clone(), grade: 0, count: 2 });
    assert_eq!(sig(&s), [1, 4, 0], "6 I → 2 II");
    apply_input(&mut s, InputEvent::MergeSigil { element: el.clone(), grade: 0, count: 1 });
    assert_eq!(sig(&s), [1, 4, 0], "one I cannot make a II");
    apply_input(&mut s, InputEvent::SplitSigil { element: el.clone(), grade: 1, count: 4 });
    assert_eq!(sig(&s), [13, 0, 0], "4 II → 12 I");
    apply_input(&mut s, InputEvent::SplitSigil { element: el.clone(), grade: 0, count: 1 });
    apply_input(&mut s, InputEvent::MergeSigil { element: el.clone(), grade: 2, count: 1 });
    assert_eq!(sig(&s), [13, 0, 0], "no grade below I, none above III");
    apply_input(&mut s, InputEvent::MergeSigil { element: el.clone(), grade: 0, count: 4 });
    apply_input(&mut s, InputEvent::MergeSigil { element: el, grade: 1, count: 1 });
    assert_eq!(sig(&s), [1, 1, 1], "12 I → 4 II → one III and one II");
    apply_input(&mut s, InputEvent::SplitSigil { element: "wind".into(), grade: 2, count: 1 });
    assert_eq!(sig(&s), [1, 4, 0], "an old log's element (even one off the chart) no longer matters");
}
