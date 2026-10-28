//! Combat effects of owned Potential nodes. No temporary modifier Vecs or RNG.
//! Iron Wall uses the same once-per-tick lifetime as existing party auras;
//! Iron Edge snapshots current DEF once per second, or on a board mutation.

use super::{
    doctrine_for_unit, major_spec, MajorEffect, MajorSlot, NodeKind, PotentialStat, NODE_DEFS,
};
use crate::grid::{Element, Entity, Team};

fn supported(e: &Entity) -> bool {
    matches!(e.team, Team::Player) && !e.is_pet && doctrine_for_unit(&e.unit_def).is_some()
}

/// Generic stat nodes join the existing additive stat buckets. Empty boards
/// return immediately; stale ranks on an unsupported class confer no effects.
pub fn stat_bp(e: &Entity, wanted: PotentialStat) -> i32 {
    if e.potential.ranks.is_empty() || !supported(e) {
        return 0;
    }
    NODE_DEFS
        .iter()
        .filter_map(|def| match def.kind {
            NodeKind::Stat { stat, magnitude_bp, .. }
                if stat == wanted && e.potential.rank(def.id) > 0 =>
            {
                Some(magnitude_bp)
            }
            _ => None,
        })
        .fold(0i32, i32::saturating_add)
}

fn major(e: &Entity, slot: MajorSlot) -> Option<MajorEffect> {
    if !supported(e) || e.potential.rank(slot.node_id()) == 0 {
        return None;
    }
    major_spec(&e.unit_def, slot)?.effect
}

/// Snapshot bonus stays separate from ATK%: "+25% of DEF" adds that exact
/// amount, not another percent-scaled copy. The ownership gate clears stale
/// caches immediately when a class or board is reset.
pub fn atk_from_def(e: &Entity) -> i32 {
    if matches!(major(e, MajorSlot::A), Some(MajorEffect::AtkFromDefBp(_))) {
        e.potential_atk_from_def.max(0)
    } else {
        0
    }
}

pub fn element_dmg_bp(e: &Entity, element: Element) -> i32 {
    // Generic elemental nodes strengthen the character's own element, not an
    // off-element skill or weapon endow. Cinderhide independently keys on Fire.
    let generic = if element == e.element {
        stat_bp(e, PotentialStat::ElementDmgBp)
    } else {
        0
    };
    if element != Element::Fire {
        return generic;
    }
    let Some(MajorEffect::FireDamageWhileShieldedBp(bp)) = major(e, MajorSlot::B) else {
        return generic;
    };
    // Aegis and Praesidium can coexist with Flame Shield. Neither a party
    // shield nor another caster's Flame Shield activates Rapis's own passive.
    if e.shields
        .iter()
        .any(|s| s.pool > 0 && s.src_id == e.id && s.source_skill_id == "flame_shield")
    {
        generic.saturating_add(i32::from(bp))
    } else {
        generic
    }
}

/// Refresh after aura/taunt upkeep; force the DEF snapshot after an explicit
/// board change/deployment. This never mutates ranks or parked skill baselines.
pub fn refresh(entities: &mut [Entity], tick: u64, force_atk: bool) {
    let party_dr = entities
        .iter()
        .filter(|e| e.on_board())
        .filter_map(|e| match major(e, MajorSlot::B) {
            Some(MajorEffect::PartyDamageReductionBp(bp)) => Some(i32::from(bp)),
            _ => None,
        })
        .max()
        .unwrap_or(0);
    for e in entities {
        e.potential_party_dr_bp = if matches!(e.team, Team::Player) && !e.is_pet && e.on_board() {
            party_dr
        } else {
            0
        };
        match major(e, MajorSlot::A) {
            Some(MajorEffect::AtkFromDefBp(bp)) if force_atk || tick % 10 == 0 => {
                e.potential_atk_from_def =
                    (i64::from(crate::combat::effective_def(e).max(0)) * i64::from(bp) / 10_000)
                        .min(i64::from(i32::MAX)) as i32;
            }
            Some(MajorEffect::AtkFromDefBp(_)) => {}
            _ => e.potential_atk_from_def = 0,
        }
    }
}

/// A final independent reduction layer, including True packets. A dead/held
/// recipient cannot keep a cached aura. Multiple Iron Walls never stack.
pub fn reduce_incoming(e: &Entity, amount: i64) -> i64 {
    if !matches!(e.team, Team::Player) || e.is_pet || !e.on_board() {
        return amount;
    }
    let bp = e.potential_party_dr_bp.clamp(0, 9_000);
    if bp == 0 {
        return amount;
    }
    amount * (10_000 - i64::from(bp)) / 10_000
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        combat,
        grid::{AbsorbShield, Cell, DamageType},
        items::StatMod,
        rng::Pcg32,
    };

    fn character(id: u32, class: &str) -> Entity {
        let mut e = Entity::idle(id, Cell::new(0, 0)).with_team(Team::Player);
        e.unit_def = class.into();
        e.atk = 1_000;
        e.def = 1_000;
        e.max_hp = 1_000;
        e.hp = 1_000;
        e.element = Element::Fire;
        e.atk_type = DamageType::Magic;
        e
    }

    fn own(e: &mut Entity, ids: &[&str]) {
        for id in ids {
            e.potential.ranks.insert((*id).into(), 1);
        }
    }

    fn strike(attacker: &Entity, element: Element) -> i32 {
        let mut a = attacker.clone();
        a.buffs.push(crate::skills::Buff {
            kind: crate::skills::BuffKind::AtkElement,
            magnitude: crate::skills::element_to_mag(element),
            ticks: 10,
            source_id: 1,
        });
        let mut target = Entity::idle(1000, Cell::new(1, 0)).with_team(Team::Monster);
        target.hp = 1_000_000;
        target.max_hp = 1_000_000;
        target.element = Element::Physical;
        let mut entities = [a, target];
        combat::resolve_attack(&mut entities, &mut Pcg32::new(42), 0, 1, &mut Vec::new())
    }

    #[test]
    fn stat_nodes_change_real_damage_def_hp_crit_and_limit_gain() {
        let mut e = character(1, "lapis");
        e.bound_card_mods = vec![
            StatMod::AtkBp(1_000),
            StatMod::MaxHpBp(2_000),
            StatMod::CritBp(300),
            StatMod::ElementDmgBp(Element::Fire, 1_500),
            StatMod::ChargeRateBp(400),
        ];
        let crit_before = combat::effective_crit_chance_bp(&e);
        own(
            &mut e,
            &["s1", "s2", "s3", "s4", "s5", "s6", "s7", "s8", "s9", "s10"],
        );
        assert_eq!(
            combat::effective_atk(&e),
            1_200,
            "10% board + 10% gear share one bucket"
        );
        assert_eq!(combat::effective_max_hp(&e), 1_280);
        assert_eq!(combat::effective_def(&e), 1_040);
        assert_eq!(combat::effective_crit_chance_bp(&e) - crit_before, 200);
        assert_eq!(
            strike(&e, Element::Fire),
            1_500,
            "10% board + 15% Fire gear reaches a real hit"
        );
        assert_eq!(
            strike(&e, Element::Ice),
            1_200,
            "off-element attack gets neither native Fire board bonus nor Fire gear"
        );
        e.skills
            .push(crate::skills::SkillState::new("fire_cat_meteor", 1));
        crate::skills::charge_limit_break(&mut e, 50);
        assert_eq!(
            e.limit_break, 55,
            "6% board + 4% gear scales the real gauge gain"
        );
    }

    #[test]
    fn iron_edge_snapshots_current_def_every_second_and_resets_immediately() {
        let mut e = character(1, "seraphine");
        e.def = 4_000;
        own(&mut e, &["A"]);
        refresh(std::slice::from_mut(&mut e), 1, true);
        assert_eq!(combat::effective_atk(&e), 2_000);
        e.buffs.push(crate::skills::Buff {
            kind: crate::skills::BuffKind::DefPct,
            magnitude: 10_000,
            ticks: 50,
            source_id: 1,
        });
        refresh(std::slice::from_mut(&mut e), 9, false);
        assert_eq!(
            combat::effective_atk(&e),
            2_000,
            "no per-hit re-evaluation between seconds"
        );
        refresh(std::slice::from_mut(&mut e), 10, false);
        assert_eq!(combat::effective_atk(&e), 3_000);
        own(&mut e, &["s6"]);
        refresh(std::slice::from_mut(&mut e), 20, false);
        assert_eq!(
            combat::effective_atk(&e),
            3_080,
            "snapshot includes the board's current DEF%"
        );
        e.bound_card_mods.push(StatMod::AtkBp(1_000));
        assert_eq!(
            combat::effective_atk(&e),
            3_180,
            "the DEF contribution is not scaled again by ATK%"
        );
        e.potential.ranks.clear();
        refresh(std::slice::from_mut(&mut e), 21, false);
        assert_eq!(combat::effective_atk(&e), 1_100);
    }

    #[test]
    fn iron_wall_reduces_all_channels_once_and_requires_living_deployed_source() {
        let mut source = character(1, "seraphine");
        source.def = 0;
        own(&mut source, &["B"]);
        let mut duplicate = source.clone();
        duplicate.id = 2;
        let mut ally = character(3, "lapis");
        ally.def = 0;
        ally.element = Element::Physical;
        let enemy = Entity::idle(1000, Cell::new(1, 0)).with_team(Team::Monster);
        let mut party = [source, duplicate, ally, enemy];
        refresh(&mut party, 1, true);
        let attacker = character(99, "");
        for kind in [DamageType::Physical, DamageType::Magic, DamageType::True] {
            let mut pkt = combat::build_damage_as(&attacker, false, kind);
            pkt.element = Element::Physical;
            let ally = party.get(2).expect("ally");
            assert_eq!(
                combat::apply_damage(ally, &pkt, 1),
                900,
                "strongest aura only, {kind:?}"
            );
            assert_eq!(
                combat::apply_damage(party.first().expect("source"), &pkt, 1),
                900
            );
            assert_eq!(
                combat::apply_damage(party.last().expect("enemy"), &pkt, 1),
                1_000
            );
        }
        party.first_mut().expect("source").alive = false;
        party.get_mut(1).expect("duplicate").held_until = 100;
        refresh(&mut party, 2, false);
        let mut pkt = combat::build_damage_as(&attacker, false, DamageType::True);
        pkt.element = Element::Physical;
        assert_eq!(
            combat::apply_damage(party.get(2).expect("ally"), &pkt, 1),
            1_000
        );
        assert_eq!(party.first().expect("dead source").potential_party_dr_bp, 0);
    }

    #[test]
    fn cinderhide_needs_own_intact_flame_shield_and_only_increases_fire() {
        let mut e = character(1, "lapis");
        own(&mut e, &["B"]);
        assert_eq!(strike(&e, Element::Fire), 1_000);
        e.shields.push(AbsorbShield {
            pool: 100,
            src_id: 1,
            source_skill_id: "aegis".into(),
            ..Default::default()
        });
        assert_eq!(
            strike(&e, Element::Fire),
            1_000,
            "Aegis is not Flame Shield"
        );
        let shield = e.shields.first_mut().expect("shield");
        shield.source_skill_id = "flame_shield".into();
        shield.src_id = 2;
        assert_eq!(
            strike(&e, Element::Fire),
            1_000,
            "another caster cannot activate Cinderhide"
        );
        e.shields.first_mut().expect("shield").src_id = 1;
        assert_eq!(strike(&e, Element::Fire), 1_100);
        assert_eq!(strike(&e, Element::Ice), 1_000);
        e.shields.first_mut().expect("shield").pool = 0;
        assert_eq!(
            strike(&e, Element::Fire),
            1_000,
            "an empty pool cannot activate the passive"
        );
        e.shields.first_mut().expect("shield").pool = 100;
        assert_eq!(combat::absorb_soak(&mut e, 100), 0);
        assert_eq!(
            strike(&e, Element::Fire),
            1_000,
            "a broken shield loses the bonus immediately"
        );
    }

    #[test]
    fn actual_flame_shield_cast_activates_cinderhide() {
        let mut state = crate::state::GameState::new(42, 8, 8);
        let mut e = character(1, "lapis");
        own(&mut e, &["B"]);
        e.skills
            .push(crate::skills::SkillState::new("flame_shield", 2));
        e.active_skills.push("flame_shield".into());
        state.grid.spawn(e);
        crate::tick::tick(&mut state, 1);
        let caster = state.grid.entities.first().expect("caster");
        assert_eq!(caster.shields.iter().map(|s| s.pool).sum::<i32>(), 1_000);
        assert_eq!(
            strike(caster, Element::Fire),
            1_100,
            "the production cast stamps shield provenance"
        );
    }

    #[test]
    fn empty_boards_are_inert_and_unknown_classes_cannot_use_stale_ranks() {
        let mut e = character(1, "seraphine");
        let before = serde_json::to_string(&e).expect("entity");
        refresh(std::slice::from_mut(&mut e), 10, true);
        assert_eq!(serde_json::to_string(&e).expect("entity"), before);
        assert_eq!(strike(&e, Element::Fire), 1_000);
        e.unit_def = "mage_t1".into();
        own(&mut e, &["s1", "s5", "s6", "s8", "A", "B"]);
        refresh(std::slice::from_mut(&mut e), 10, true);
        assert_eq!(combat::effective_atk(&e), 1_000);
        assert_eq!(combat::effective_max_hp(&e), 1_000);
        assert_eq!(combat::effective_def(&e), 1_000);
        assert_eq!(strike(&e, Element::Fire), 1_000);
        e.unit_def = "lapis".into();
        e.is_pet = true;
        assert_eq!(
            strike(&e, Element::Fire),
            1_000,
            "a pet cannot inherit a character board"
        );
        e.is_pet = false;
        e.team = Team::Monster;
        assert_eq!(
            strike(&e, Element::Fire),
            1_000,
            "stale class IDs never grant monsters a board"
        );
    }
}
