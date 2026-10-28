//! Combat resolution — money-zone (GDD §7.1, §3.2).
//!
//! Rules (re-read these line by line before touching anything here):
//!   1. Damage is integer-only: `damage = max(1, (atk - def) * element_bp / 10000)`.
//!      Multiply BEFORE divide (GDD §4.2.1 — `a * b / 10000`, never
//!      `a / 10000 * b`).
//!   2. Element triangle is published and fixed at compile time (will
//!      move to data in ALPHA-6).
//!   3. Targeting is deterministic: nearest enemy by Chebyshev,
//!      tie-break by spawn id (GDD §3.2).
//!   4. Symmetric entity rule (§4.2.2): there is no "player attacks
//!      monster" — there is `resolve_attack(attacker_idx, target_idx)`,
//!      and it works in either direction.
//!   5. No PRNG draws here yet — ALPHA-3 has no crit / dodge.
//!      Those come with cards/refine, not the base attack.

pub mod card_specials;
mod damage;
mod derived;
pub mod stat_mods;
mod targeting;

pub use damage::*;
pub use derived::*;
pub use targeting::*;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::grid::Element;
    use crate::rng::Pcg32;

    #[test]
    fn element_matchups_follow_the_loose_web() {
        // RO nature cycle: Fire>Earth>Wind>Water>Fire.
        assert_eq!(element_mult_bp(Element::Fire, Element::Earth), 15_000);
        assert_eq!(element_mult_bp(Element::Earth, Element::Wind), 15_000);
        assert_eq!(element_mult_bp(Element::Wind, Element::Ice), 15_000);
        assert_eq!(element_mult_bp(Element::Ice, Element::Fire), 15_000);
        // ...and their reverses are 50%.
        assert_eq!(element_mult_bp(Element::Earth, Element::Fire), 5_000);
        assert_eq!(element_mult_bp(Element::Fire, Element::Ice), 5_000);

        // Thunder (Pokemon-familiar): Thunder>Water, Earth>Thunder.
        assert_eq!(element_mult_bp(Element::Thunder, Element::Ice), 15_000);
        assert_eq!(element_mult_bp(Element::Earth, Element::Thunder), 15_000);
        assert_eq!(element_mult_bp(Element::Thunder, Element::Earth), 5_000);
        // Thunder is distinct from Wind — no matchup between them.
        assert_eq!(element_mult_bp(Element::Thunder, Element::Wind), 10_000);

        // Light vs Dark — mutual advantage (both 150%).
        assert_eq!(element_mult_bp(Element::Light, Element::Dark), 15_000);
        assert_eq!(element_mult_bp(Element::Dark, Element::Light), 15_000);

        // Poison: NO offensive advantage; weak only to Fire (not Wind).
        assert_eq!(element_mult_bp(Element::Fire, Element::Poison), 15_000);
        assert_eq!(element_mult_bp(Element::Poison, Element::Fire), 5_000);
        assert_eq!(element_mult_bp(Element::Wind, Element::Poison), 10_000);
        assert_eq!(element_mult_bp(Element::Poison, Element::Ice), 10_000);

        // Same element / Neutral = 100%.
        assert_eq!(element_mult_bp(Element::Fire, Element::Fire), 10_000);
        assert_eq!(element_mult_bp(Element::Physical, Element::Ice), 10_000);
    }

    #[test]
    fn damage_uses_atk_minus_def_then_element() {
        // 100 atk vs 30 def = 70 raw. Neutral element = 70.
        assert_eq!(calculate_damage(100, 30, Element::Fire, Element::Fire, false), 70);
        // 100 atk vs 30 def = 70 raw. Advantage = 70 * 1.5 = 105.
        assert_eq!(calculate_damage(100, 30, Element::Fire, Element::Earth, false), 105);
        // Disadvantage = 70 * 0.5 = 35.
        assert_eq!(calculate_damage(100, 30, Element::Fire, Element::Ice, false), 35);
    }

    #[test]
    fn damage_minimum_is_1_even_when_outclassed() {
        // Tank's def is way over the attacker's atk — should still take 1.
        assert_eq!(calculate_damage(10, 100, Element::Fire, Element::Fire, false), 1);
        // Even with element disadvantage stacked on top.
        assert_eq!(calculate_damage(10, 100, Element::Fire, Element::Ice, false), 1);
    }

    #[test]
    fn attack_speed_is_the_units_own_rate_and_nothing_else_moves_it() {
        use crate::grid::{Cell, Entity};
        // 0.0.367: attack speed IS the template's rate (milli/100 = atk/s).
        // Level and attributes never touched it (AGI/DEX went in 0.0.87) and
        // weapon type no longer does either (familiarity went in 0.0.366).
        use crate::items::{Item, ItemKind, Rarity, WeaponType};
        let mut e = Entity::idle(1, Cell::new(0, 0));
        e.atk_speed_milli = 106; // 1.06 atk/s — seraphine's rate
        let bare = effective_atk_speed_milli(&e);
        assert_eq!(bare, 106, "the rate is read, not derived");
        e.stats.level = 70;
        assert_eq!(effective_atk_speed_milli(&e), bare, "levelling must not change it");
        for wt in [WeaponType::Dagger, WeaponType::Sword2H, WeaponType::Staff, WeaponType::Mace] {
            let mut holder = e.clone();
            let mut w = Item::new("w", ItemKind::Weapon, 10, 0, Rarity::Common);
            w.weapon_type = wt;
            holder.equipped = vec![w];
            assert_eq!(effective_atk_speed_milli(&holder), bare, "{wt:?} must not change it");
        }
        // A unit written faster IS faster, proportionally.
        e.atk_speed_milli = 212;
        assert_eq!(effective_atk_speed_milli(&e), 2 * bare, "the base rate is the dial");
    }

    #[test]
    fn haste_is_additive_and_means_exactly_what_it_says() {
        use crate::grid::{Cell, Entity};
        use crate::items::{Item, ItemKind, Rarity};
        use crate::skills::{apply_buff, Buff, BuffKind};
        // The whole point of the rework: "+30%" is +30% of the attack RATE, on
        // every unit. The old ASPD fold turned +30% into +43% and made the same
        // affix worth different amounts on different units.
        let mk = |quicken: i32, potion: i32, item_bp: i32| {
            let mut e = Entity::idle(1, Cell::new(0, 0));
            e.atk_speed_milli = 100; // 1.00 atk/s, so milli reads as a percentage
            for (kind, mag) in [(BuffKind::AspdPct, quicken), (BuffKind::AspdPotionPct, potion)] {
                if mag > 0 {
                    apply_buff(&mut e, Buff { kind, magnitude: mag, ticks: 10, source_id: 1 });
                }
            }
            if item_bp != 0 {
                let mut w = Item::new("w", ItemKind::Weapon, 10, 0, Rarity::Common);
                w.atk_speed_bp = item_bp;
                e.equipped = vec![w];
            }
            effective_atk_speed_milli(&e)
        };
        assert_eq!(mk(0, 0, 0), 100, "no haste = the base rate");
        assert_eq!(mk(3_000, 0, 0), 130, "+30% is +30%");
        assert_eq!(mk(3_000, 1_000, 0), 140, "sources are additive");
        assert_eq!(mk(0, 0, 500), 105, "an item's atk_speed_bp joins the same sum");
        assert_eq!(mk(3_000, 1_000, 500), 145, "…all of it one additive term");
    }

    #[test]
    fn the_rate_cap_and_floor_are_written_numbers() {
        use crate::grid::{Cell, Entity};
        use crate::skills::{apply_buff, Buff, BuffKind};
        // RO's ceiling was an asymptote nobody could see. Ours are written: haste
        // stops at +200%, the rate at one swing per tick (10.00 atk/s), and the
        // floor at 0.20 atk/s.
        let mut fast = Entity::idle(1, Cell::new(0, 0));
        fast.atk_speed_milli = 100;
        apply_buff(&mut fast, Buff { kind: BuffKind::AspdPct, magnitude: 9_000, ticks: 10, source_id: 1 });
        apply_buff(&mut fast, Buff { kind: BuffKind::AspdPotionPct, magnitude: 9_000, ticks: 10, source_id: 2 });
        assert_eq!(effective_atk_speed_milli(&fast), 280, "+180% so far, still additive");
        apply_buff(&mut fast, Buff { kind: BuffKind::CardAspdStack, magnitude: 9_000, ticks: 10, source_id: 3 });
        assert_eq!(effective_atk_speed_milli(&fast), 300, "haste stops at +200%");
        let mut capped = Entity::idle(3, Cell::new(0, 0));
        capped.atk_speed_milli = 900;
        apply_buff(&mut capped, Buff { kind: BuffKind::AspdPct, magnitude: 5_000, ticks: 10, source_id: 1 });
        assert_eq!(effective_atk_speed_milli(&capped), 1_000, "one swing per tick is the ceiling");
        let mut slow = Entity::idle(2, Cell::new(0, 0));
        slow.atk_speed_milli = 20;
        apply_buff(&mut slow, Buff { kind: BuffKind::AspdSlowPct, magnitude: 9_000, ticks: 10, source_id: 3 });
        assert_eq!(effective_atk_speed_milli(&slow), 20, "floor = 0.20 atk/s, never zero");
    }

    #[test]
    fn crit_multiplies_and_def_applies_renewal_style() {
        // Non-crit: (100 - 80) * 1.0 = 20 (matched elements = 100%).
        assert_eq!(calculate_damage(100, 80, Element::Fire, Element::Fire, false), 20);
        // Crit (0.0.224): ×1.5 base then DEF applies — (150 - 80) = 70,
        // NOT the old def-ignore 100.
        assert_eq!(calculate_damage(100, 80, Element::Fire, Element::Fire, true), 70);
        // Element matchup applies on top: 70 × 150% = 105.
        assert_eq!(calculate_damage(100, 80, Element::Fire, Element::Earth, true), 105);
    }

    #[test]
    fn crit_dmg_stat_folds_base_cards_and_gear() {
        use crate::grid::{Cell, Entity};
        use crate::items::{CardClass, EquipSlot, StatMod, Item, ItemKind, Rarity};
        // Bare entity: the 150% base alone.
        let e = Entity::idle(1, Cell::new(0, 0));
        assert_eq!(effective_crit_dmg_bp(&e), 15_000);

        // Gear line: +10% crit dmg → 160%.
        let mut g = Entity::idle(2, Cell::new(0, 0));
        let mut ring = Item::new("ring", ItemKind::Armor, 0, 0, Rarity::Common);
        ring.equip_slot = Some(EquipSlot::Necklace);
        ring.bonuses.push(StatMod::CritDmgBp(1_000));
        g.equipped.push(ring);
        assert_eq!(effective_crit_dmg_bp(&g), 16_000);

        // A CARD's crit-dmg line stacks additively in the same stat (0.0.243: authored
        // `CritDmgBp` mods, no separate card channel) — Chomb 5★ = 1000 + 5 × 200 = +20%
        // on a socketed weapon → 180% together with the gear line.
        let mut sword = Item::new("sword", ItemKind::Armor, 0, 0, Rarity::Common);
        sword.equip_slot = Some(EquipSlot::Weapon);
        sword.grade = crate::items::Grade::Rare; // sockets are the grade's (0.0.388)
        g.equipped.push(sword);
        let mut chomb = Item::new("card_chomb", ItemKind::Card, 0, 0, Rarity::Mythical);
        chomb.card_slot = Some(CardClass::Weapon);
        chomb.star = 5;
        chomb.card_mods = vec![StatMod::CritDmgBp(1_000)];
        chomb.card_star_step = vec![StatMod::CritDmgBp(200)];
        g.card_panel.push(chomb);
        assert_eq!(effective_crit_dmg_bp(&g), 18_000);

        // apply_damage no longer special-cases crits: same packet, same
        // mitigation whether is_crit is set or not (multiplier lives at resolve).
        let mut tank = Entity::idle(3, Cell::new(0, 0));
        tank.def = 0;
        let mk = |crit: bool| Damage {
            atk_level: 1, amount: 100,
            element: Element::Physical,
            kind: crate::grid::DamageType::Physical,
            is_ranged: false,
            is_crit: crit,
            is_aoe: false,
            mastery_bp: 0, shadow_bane_bp: 0, debuff_amp_bp: 0,
            atk_race: crate::grid::Race::Unknown,
            atk_size: crate::grid::Size::Medium,
            ignore_def_bp: 0,
        };
        assert_eq!(
            apply_damage(&tank, &mk(true), 1),
            apply_damage(&tank, &mk(false), 1),
            "crit packets mitigate exactly like normal hits now"
        );
        assert!(apply_damage(&tank, &mk(true), 1) <= 100, "the DEF curve applies to a crit too");
    }

    #[test]
    fn roll_crit_skips_draw_at_zero_chance() {
        // Bit-precise check: if crit_chance is 0, the PRNG must NOT
        // advance. We compare a stream that calls roll_crit against
        // an untouched stream and expect them to match.
        let mut a = Pcg32::new(123);
        let mut b = Pcg32::new(123);
        assert!(!roll_crit(&mut a, 0));
        assert_eq!(a.gen_range_u32(0, 1000), b.gen_range_u32(0, 1000));
    }

    #[test]
    fn find_enemy_picks_nearest_then_lowest_id() {
        use crate::grid::{Cell, Entity, Team};

        let mut entities = vec![
            Entity::idle(1, Cell::new(0, 0)).with_team(Team::Player),
            // Two monsters equidistant at Chebyshev 3.
            Entity::idle(7, Cell::new(3, 0)).with_team(Team::Monster),
            Entity::idle(2, Cell::new(0, 3)).with_team(Team::Monster),
        ];
        // Tie-break: lower id (= 2) wins.
        let idx = find_enemy_idx(&entities, 0).unwrap();
        assert_eq!(entities[idx].id, 2);

        // Kill the lower-id one; should fall back to id 7.
        entities[2].alive = false;
        let idx = find_enemy_idx(&entities, 0).unwrap();
        assert_eq!(entities[idx].id, 7);

        // Kill all monsters.
        entities[1].alive = false;
        assert_eq!(find_enemy_idx(&entities, 0), None);
    }





    #[test]
    fn the_mobbed_penalty_is_retired_def_is_def() {
        for n in [1, 2, 3, 8, 22, 50] {
            assert_eq!(mobbed_def(100, n), 100);
        }
        assert_eq!(mobbed_def(0, 8), 0);
    }

    #[test]
    fn a_mobbed_target_takes_the_same_physical_damage() {
        use crate::grid::{Cell, DamageType, Element, Entity, Team};
        // A target with soft DEF; a 1000-damage physical packet. The attacker count used to cut
        // DEF 5% a head beyond two; it is inert now.
        let mut t = Entity::idle(1, Cell::new(0, 0)).with_team(Team::Player);
        // Soft DEF is a function of LEVEL since 0.0.86, not of VIT.
        t.stats.level = 60;
        let dmg = Damage {
            atk_level: 1, amount: 1000, element: Element::Physical, kind: DamageType::Physical,
            is_ranged: false, is_crit: false, is_aoe: false, mastery_bp: 0, shadow_bane_bp: 0, debuff_amp_bp: 0,
            atk_race: crate::grid::Race::Unknown,
            atk_size: crate::grid::Size::Medium,
            ignore_def_bp: 0,
        };
        let calm = apply_damage(&t, &dmg, 1);
        let swarmed = apply_damage(&t, &dmg, 8);
        assert_eq!(swarmed, calm, "0.0.407: the crowd no longer strips DEF");
    }



    #[test]
    fn only_a_dodger_avoids_anything_and_it_costs_no_draw_otherwise() {
        use crate::grid::{Cell, Entity, Team};
        use crate::items::{Item, ItemKind, Rarity, StatMod};
        // A target with NO dodge is hit every time — the HIT/FLEE miss is gone
        // (0.0.333) — and, just as importantly, takes no PRNG draw for the
        // question, which is what lets a plain fight keep its stream.
        let mut plain = vec![
            Entity::idle(1, Cell::new(0, 0)).with_team(Team::Player)
                .with_combat(100, 50, 0, 0, Element::Fire),
            Entity::idle(2, Cell::new(1, 0)).with_team(Team::Monster)
                .with_combat(100_000, 0, 0, 0, Element::Fire),
        ];
        let mut rng = Pcg32::new(1);
        for _ in 0..50 {
            plain[1].alive = true;
            assert!(resolve_attack(&mut plain, &mut rng, 0, 1, &mut Vec::new()) > 0,
                "nothing dodges without the stat");
        }

        // 40% dodge: some blows land on nothing.
        let mut dodgy = plain.clone();
        let mut cloak = Item::new("cloak", ItemKind::Armor, 0, 0, Rarity::Common);
        cloak.bonuses = vec![StatMod::DodgeBp(4_000)];
        dodgy[1].equipped.push(cloak);
        assert_eq!(crate::combat::stat_mods::dodge_bp(&dodgy[1]), 4_000);
        let mut rng2 = Pcg32::new(1);
        let mut dodged = 0;
        for _ in 0..100 {
            dodgy[1].alive = true;
            if resolve_attack(&mut dodgy, &mut rng2, 0, 1, &mut Vec::new()) == 0 {
                dodged += 1;
            }
        }
        assert!((20..=60).contains(&dodged), "40% dodge landed {dodged}/100 dodges");

        // Two lines stack inverse-multiplicatively: 1 - 0.6 x 0.7 = 58%, not 70%.
        let mut both = plain.clone();
        let mut a = Item::new("cloak", ItemKind::Armor, 0, 0, Rarity::Common);
        a.bonuses = vec![StatMod::DodgeBp(4_000)];
        let mut b = Item::new("boots", ItemKind::Armor, 0, 0, Rarity::Common);
        b.bonuses = vec![StatMod::DodgeBp(3_000)];
        both[1].equipped.push(a);
        both[1].equipped.push(b);
        assert_eq!(crate::combat::stat_mods::dodge_bp(&both[1]), 5_800);
    }

    #[test]
    fn neutral_element_is_always_100pct() {
        for e in [Element::Physical, Element::Fire, Element::Earth, Element::Ice] {
            assert_eq!(element_mult_bp(Element::Physical, e), 10_000,
                "Neutral attacker is neutral vs everything");
            assert_eq!(element_mult_bp(e, Element::Physical), 10_000,
                "Neutral defender takes neutral from everything");
        }
        // The Fire/Earth/Water triangle is untouched by adding Neutral.
        assert_eq!(element_mult_bp(Element::Fire, Element::Earth), 15_000);
        assert_eq!(element_mult_bp(Element::Ice, Element::Fire), 15_000);
    }

    #[test]
    fn magic_and_physical_meet_the_same_def() {
        use crate::grid::{Cell, DamageType, Entity, Team};
        // Same offense, same target, only the damage type differs — and since
        // 0.0.85 that no longer changes what the blow has to get through. MDEF is
        // gone; one DEF answers both, and what a hit is made of shows up in its
        // ELEMENT instead.
        let mage = Entity::idle(1, Cell::new(0, 0))
            .with_combat(100, 1000, 0, 0, Element::Physical)
            .with_attack(DamageType::Magic, false);
        let fighter = Entity::idle(1, Cell::new(0, 0))
            .with_combat(100, 1000, 0, 0, Element::Physical); // physical default
        let tank = Entity::idle(2, Cell::new(1, 0))
            .with_team(Team::Monster)
            .with_combat(100, 0, 800, 0, Element::Physical); // DEF 800

        let mdmg = apply_damage(&tank, &build_damage(&mage, false), 0);
        let pdmg = apply_damage(&tank, &build_damage(&fighter, false), 0);
        assert_eq!(mdmg, pdmg, "one DEF answers both kinds of damage");
        // And the DEF actually bites: K = 500 + 5 x level 1 = 5050 at the x10
        // scale, so 800 DEF keeps roughly an eighth of the blow off.
        assert!(pdmg < 1000 && pdmg > 700, "DEF 800 should shave a real slice: {pdmg}");
    }

    #[test]
    fn a_defenders_own_def_scales_how_much_it_keeps_off() {
        use crate::grid::{Cell, DamageType, Entity, Team};
        // Soft MDEF is gone with MDEF itself (0.0.85). What replaced the question
        // it asked — "does the defender's own armour matter against magic?" — is
        // simply DEF, and more of it keeps more off.
        let mage = Entity::idle(1, Cell::new(0, 0))
            .with_combat(100, 1000, 0, 0, Element::Physical)
            .with_attack(DamageType::Magic, false);
        let soft = Entity::idle(2, Cell::new(1, 0))
            .with_team(Team::Monster)
            .with_combat(100, 0, 0, 0, Element::Physical);
        let armoured = Entity::idle(3, Cell::new(1, 0))
            .with_team(Team::Monster)
            .with_combat(100, 0, 2000, 0, Element::Physical);
        let a = apply_damage(&soft, &build_damage(&mage, false), 0);
        let b = apply_damage(&armoured, &build_damage(&mage, false), 0);
        assert!(b < a, "armour must reduce magic too: {b} < {a}");
    }

    #[test]
    fn magic_crits_but_is_never_dodged() {
        use crate::grid::{Cell, DamageType, Entity, Team};
        // 0.0.472 (one ATK, no MATK): a magic blow rolls its crit like any other — ONE draw —
        // and still takes no dodge roll, so it always lands.
        let mut entities = vec![
            Entity::idle(1, Cell::new(0, 0)).with_team(Team::Player)
                .with_combat(100, 50, 0, 0, Element::Physical)
                .with_attack(DamageType::Magic, true),
            Entity::idle(2, Cell::new(1, 0)).with_team(Team::Monster)
                .with_combat(100_000, 0, 0, 0, Element::Physical),
        ];
        assert!(effective_crit_chance_bp(&entities[0]) > 0, "the case needs a crit chance");
        let mut a = Pcg32::new(99);
        let mut b = Pcg32::new(99);
        let dmg = resolve_attack(&mut entities, &mut a, 0, 1, &mut Vec::new());
        assert!(dmg > 0, "magic always lands");
        b.gen_range_u32(0, 10_000); // the crit roll
        assert_eq!(a.gen_range_u32(0, 1000), b.gen_range_u32(0, 1000),
            "magic takes exactly the crit draw — no dodge draw");
    }

    #[test]
    fn resolve_attack_can_kill() {
        use crate::grid::{Cell, Entity, Team};

        let mut entities = vec![
            Entity::idle(1, Cell::new(0, 0))
                .with_team(Team::Player)
                .with_combat(100, 50, 5, 0, Element::Fire),
            Entity::idle(2, Cell::new(1, 0))
                .with_team(Team::Monster)
                .with_combat(20, 10, 0, 0, Element::Earth),
        ];

        // DEX 30 → HIT 206 vs the mob's FLEE 102 → accuracy caps at 100% (renewal
        // rate 0 would otherwise leave a STAT_MIN attacker at 75% → a flaky miss).
        // Fire vs Earth = 1.5x; the hit lands and kills the 20-HP mob.
        let mut rng = Pcg32::new(0);
        let dmg = resolve_attack(&mut entities, &mut rng, 0, 1, &mut Vec::new());
        assert!(dmg >= 20, "the hit lands and kills the 20-HP mob, got {dmg}");
        assert!(!entities[1].alive);
        assert!(entities[1].hp <= 0);
    }
}

#[cfg(test)]
mod aspd_slow_tests {
    use super::*;
    use crate::grid::{Cell, Entity};
    use crate::skills::{apply_buff, Buff, BuffKind};

    #[test]
    fn aspd_slow_bites_monsters_flat_aspd() {
        // A monster reads the same `atk_speed_milli` a player does now, and
        // AspdSlowPct (Aspd Dampening) must bite it.
        let mut mob = Entity::idle(1000, Cell::new(0, 0));
        mob.atk_speed_milli = 200;
        let base = effective_atk_speed_milli(&mob);
        assert_eq!(base, 200, "monster keeps flat ASPD un-slowed");
        // Lv10 slow = 2500 bp = 25% → 200 * 0.75 = 150.
        apply_buff(&mut mob, Buff {
            kind: BuffKind::AspdSlowPct, magnitude: 2500, ticks: 50, source_id: 1,
        });
        assert_eq!(effective_atk_speed_milli(&mob), 150, "25% slower attack rate");
    }

    #[test]
    fn aspd_slow_bites_players_for_pvp() {
        // A player must be slowed by the same buff, by the same percentage.
        let mut p = Entity::idle(1, Cell::new(0, 0));
        p.atk_speed_milli = 108;
        let base = effective_atk_speed_milli(&p);
        apply_buff(&mut p, Buff {
            kind: BuffKind::AspdSlowPct, magnitude: 2500, ticks: 50, source_id: 2,
        });
        let slowed = effective_atk_speed_milli(&p);
        assert!(slowed < base, "player attack rate drops ({base} -> {slowed})");
        assert_eq!(slowed, (base as i64 * 7_500 / 10_000) as u32, "exactly 25% off");
    }
    #[test]
    fn a_break_deals_its_own_damage() {
        use crate::grid::{Cell, Element, Entity, Team};
        let mut rng = crate::rng::Pcg32::new(1);
        let mut es = vec![
            Entity::idle(1, Cell::new(0, 0)).with_team(Team::Player)
                .with_combat(1000, 100, 0, 0, Element::Fire),
            Entity::idle(2, Cell::new(1, 0)).with_team(Team::Monster)
                .with_combat(100_000, 0, 0, 0, Element::Earth),
        ];
        es[0].stats.level = 10;
        es[1].toughness_max = 60;
        es[1].toughness = 1; // the next landed hit empties it
        let before = es[1].hp;
        let mut hits = Vec::new();
        let normal = crate::combat::resolve_attack(&mut es, &mut rng, 0, 1, &mut hits);
        let dealt = before - es[1].hp;
        let brk: i32 = hits.iter()
            .filter(|h| matches!(h.kind, crate::state::HitKind::Break))
            .map(|h| h.amount)
            .sum();
        assert!(brk > 0, "a break must land its own number, got {brk} (hits={hits:?})");
        assert_eq!(dealt, normal + brk, "the break's damage is on top of the swing's");
    }

}
