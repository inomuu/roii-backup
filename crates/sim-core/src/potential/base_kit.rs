//! Fixed-kit characters start with every authored ability at rank one for free.
//! Shared by construction and the client/server trusted-save migration.
use super::{NodeKind, PotentialState, NODE_DEFS};
use crate::grid::{Entity, Team};
use crate::skills::{SkillKind, SkillState};
use crate::state::CharacterData;

pub fn is_base_skill(class: &str, id: &str) -> bool {
    super::doctrine_for_unit(class).is_some()
        && crate::skills::unit_kit_tree(class)
            .iter()
            .any(|n| n.skill_id == id)
}

pub fn ensure_entity(e: &mut Entity) {
    if e.team != Team::Player || e.is_pet {
        return;
    }
    ensure(
        &e.unit_def,
        &mut e.skills,
        &mut e.active_skills,
        &mut e.potential,
    );
}

pub fn ensure_parked(class: &str, c: &mut CharacterData) {
    ensure(
        class,
        &mut c.skills,
        &mut c.active_skills,
        &mut c.potential,
    );
}

fn ensure(
    class: &str,
    skills: &mut Vec<SkillState>,
    active: &mut Vec<String>,
    board: &mut PotentialState,
) {
    if super::doctrine_for_unit(class).is_none() {
        return;
    }
    for node in crate::skills::unit_kit_tree(class) {
        let Some(def) = crate::skills::skill_def(node.skill_id) else {
            continue;
        };
        let mut newly_learned = false;
        if let Some(skill) = skills.iter_mut().find(|s| s.id == node.skill_id) {
            if skill.level == 0 {
                skill.level = 1;
                newly_learned = true;
            }
        } else {
            skills.push(SkillState::new(node.skill_id, 1));
            newly_learned = true;
        }
        // Do not re-enable an existing skill the player intentionally disabled.
        if newly_learned
            && matches!(def.kind, SkillKind::Active)
            && active.len() < crate::grid::ACTIVE_SLOT_CAP
            && !active.iter().any(|s| s == node.skill_id)
        {
            active.push(node.skill_id.to_string());
        }
    }
    // The free first rank is the baseline the board measures paid ranks from.
    for node in &NODE_DEFS {
        let NodeKind::Skill { slot } = node.kind else {
            continue;
        };
        let Some(id) = super::skill_id(class, slot) else {
            continue;
        };
        if board.base_skill_levels.get(id) == Some(&0) {
            board.base_skill_levels.insert(id.into(), 1);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn data() -> crate::data::GameData {
        crate::data::GameData::from_json_str(include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../data/game_data.json"
        )))
        .unwrap()
    }
    #[test]
    fn fixed_kit_starts_at_one_and_casts_for_free() {
        let d = data();
        let mut s = crate::data::new_game_as(42, &d, "zone_b1_1", "seraphine").unwrap();
        // Rapis joins as a pull now (0.0.415): field her the model way so the test still covers a
        // fixed-kit caster beside the starter.
        s.party_slots = 2;
        crate::data::model_add_member(&mut s, &d, "lapis").unwrap();
        for e in s.grid.entities.iter().filter(|e| e.team == Team::Player) {
            assert_eq!(e.potential.spent_points, 0);
            for n in crate::skills::unit_kit_tree(&e.unit_def) {
                assert_eq!(e.skill_level(n.skill_id), 1, "{}", n.skill_id);
                if matches!(
                    crate::skills::skill_def(n.skill_id).unwrap().kind,
                    SkillKind::Active
                ) {
                    assert!(e.active_skills.iter().any(|id| id == n.skill_id));
                }
            }
            for id in ["k1", "k2", "k3"] {
                assert_eq!(super::super::actions::effective_board(e).rank(id), 1);
            }
        }
        crate::replay::advance(&mut s, &d, 600);
        for (id, skill) in [(1, "iron_chains"), (2, "fireball")] {
            let e = s.grid.entity_of_id(id).unwrap();
            assert!(
                e.skills.iter().any(|x| x.id == skill && x.last_fired > 0),
                "{skill} never cast"
            );
        }
    }
    /// Lucielle is a UNIT since 0.0.421: a pull fields `unit_def` "lucielle" with her whole kit
    /// active at rank one, and she casts — before, she deployed as an acolyte carrying three
    /// overlay skills that nothing had activated (measured: 120 s, not one cast).
    #[test]
    fn a_pulled_lucielle_fields_as_her_own_unit_and_casts() {
        let d = data();
        let mut s = crate::data::new_starter_party(42, &d, "zone_b1_1").unwrap();
        s.party_slots = 2;
        let id = crate::data::grant_exemplar_unit(&mut s, &d, "lucielle").expect("granted");
        crate::data::deploy(&mut s, &d, id).expect("deployed");
        let e = s.grid.entity_of_id(id).unwrap();
        assert_eq!(e.unit_def, "lucielle");
        for n in crate::skills::unit_kit_tree("lucielle") {
            assert!(e.skill_level(n.skill_id) >= 1, "{} missing", n.skill_id);
        }
        assert!(e.active_skills.iter().any(|k| k == "lux_medica"), "her actives are slotted");
        assert_eq!(e.skill_level("lux_medica"), 5, "the 0★ overlay lifts her actives to Lv5");
        // 0.0.441: the ultimate sits at Lv5 with the actives and S5 lifts it +3 like theirs.
        assert_eq!(e.skill_level("benedictio_lucis"), 5, "the 0★ overlay lifts the ultimate to Lv5 too");
        crate::replay::advance(&mut s, &d, 900);
        let e = s.grid.entity_of_id(id).unwrap();
        assert!(e.skills.iter().any(|x| x.id == "lumen_contagio" && x.last_fired > 0), "Contagio never cast");
    }

    #[test]
    fn the_free_first_rank_is_the_baseline_and_a_tombstone_reset_changes_nothing() {
        let d = data();
        let mut s = crate::data::new_starter_party(42, &d, "zone_b1_1").unwrap();
        s.party_slots = 2;
        crate::data::model_add_member(&mut s, &d, "lapis").unwrap();
        let e = s.grid.entity_of_id_mut(2).unwrap();
        e.skills.clear();
        e.active_skills.clear();
        let mut fire = SkillState::new("fireball", 3);
        fire.cooldown = 7;
        fire.last_fired = 9;
        e.skills.push(fire);
        e.potential.ranks.insert("k1".into(), 3);
        e.potential.base_skill_levels.insert("fireball".into(), 0);
        e.potential.spent_points = 3;
        crate::data::migrate_save_items(&mut s, &d);
        let hash = crate::hash::state_hash(&s);
        crate::data::migrate_save_items(&mut s, &d);
        assert_eq!(
            crate::hash::state_hash(&s),
            hash,
            "migration must be idempotent"
        );
        let e = s.grid.entity_of_id(2).unwrap();
        // The paid ledger is untouched now that there are no points to refund into (0.0.352);
        // the baseline simply records the free first rank.
        assert_eq!(e.potential.spent_points, 3);
        assert_eq!(e.potential.base_skill_levels.get("fireball"), Some(&1));
        assert!(
            !e.active_skills.iter().any(|x| x == "fireball"),
            "respect manual disabled skill"
        );
        let fire = e.skills.iter().find(|x| x.id == "fireball").unwrap();
        assert_eq!((fire.level, fire.cooldown, fire.last_fired), (3, 7, 9));
        // NO RESET (0.0.351): the tombstone changes nothing — the ledger and the ranks stay.
        super::super::actions::reset(&mut s, 2);
        let e = s.grid.entity_of_id(2).unwrap();
        assert_eq!(e.potential.spent_points, 3);
        assert_eq!(e.skill_level("fireball"), 3);
    }
}
