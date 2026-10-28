//! COMBAT SCORE — the one number a gacha player reads (user 2026-09-21: "ระบบ combat score
//! แบบเกม gacha"). DISPLAY ONLY: nothing in `tick()` reads it.
//!
//! It measures INVESTMENT, not damage (user: "คำนวณแต่ละตัวด้วย rarity เดียวกันด้วยค่าคงที่ + gear …
//! ไม่ว่าจะเป็น sup หรือ tank ควร score ใกล้ๆ dps"). A support and a Striker of the same rarity,
//! level, stars and gear read the SAME number — the score answers "how far have I built this
//! character", and the DPS meter answers "how hard does it hit". Role never enters.
//!
//! Shape, integer only:
//!   member = [unit(rarity, level) + Σ gear(tier, grade, enhance, lines, gems)
//!            + regalia(rarity, level, stars) + share of (training + crests)] × stars(rarity)
//!   party  = Σ fielded members
//! Stars MULTIPLY the whole member (user: "S1-S6 เป็น multiplier"): a star is worth more on a
//! built character, the way it is in a fight.
//!
//! The rarity constants follow the roster rule (memory `sr-vs-ssr-rule`): an SR's S0 is 65% of
//! an SSR's S0 and its S6 is ~95% of the SSR's S0; an SSR's S6 is +100% over its S0.
//! Calibration (sim-cli `--bench` prints `CP`, 0.0.450): reference party (five SSR, Lv70, T6
//! Legendary, SSR Regalia Lv60, S0) ≈ 10K, every member within 2% of the others; S6 ×2;
//! no Regalia 8.4K; R Regalia 8.8K; T4/Rare 7.3K; Lv50/T4 6.4K; Lv30/T2 2.2K; Lv10/T1 1.3K.
//! The bench's dmg/h moves further than the score on the same steps (gear compounds in a
//! fight) — the score is a ladder to read progress on, not a damage predictor.

use crate::grid::{Entity, Team};
use crate::items::{Item, ItemKind};
use crate::state::GameState;

/// UNIT base at Lv1 by rarity rung (0 = SR, 1 = SSR), and per level. An SR S0 is 65% of an
/// SSR S0 at every level.
const UNIT_BASE: [i64; 2] = [650, 1_000];
const UNIT_PER_LEVEL: [i64; 2] = [65, 100];
/// Stars, in bp of the WHOLE member score per star: an SSR's S6 doubles it (+100%); an SR's
/// S6 +45%.
const STAR_BP: [i64; 2] = [750, 1_667];

/// GEAR: a piece is worth its tier and grade; enhance, rolled lines and socketed gems add.
const GEAR_PER_TIER: i64 = 350;
const GEAR_GRADE_BP: [i64; 10] = [10_000, 12_500, 15_000, 18_000, 21_000, 24_000, 27_000, 30_000, 33_000, 36_000];
const GEAR_PER_ENHANCE: i64 = 40;
const GEAR_PER_LINE: i64 = 120;
const GEAR_PER_GEM: i64 = 200;

/// REGALIA: base by rarity rung (0 = R, 1 = SR, 2 = SSR), per level, per star (bp of base).
const REGALIA_BASE: [i64; 3] = [1_000, 2_000, 4_000];
const REGALIA_PER_LEVEL: [i64; 3] = [30, 60, 120];
const REGALIA_PER_STAR_BP: i64 = 1_000;

/// Party-wide: Training and Crest levels are shared, so each member carries a fifth.
const TRAIN_PER_LEVEL: i64 = 40;
const CREST_PER_LEVEL: i64 = 60;

/// Divisor that puts the reference party near 10,000 (user: "แสดงเลขหาร 10").
const SCALE: i64 = 30;

fn gear_score(it: &Item) -> i64 {
    if !matches!(it.kind, ItemKind::Weapon | ItemKind::Armor) {
        return 0;
    }
    let tier = i64::from(it.item_level().clamp(1, 9));
    let grade_bp = GEAR_GRADE_BP.get(it.grade.step()).copied().unwrap_or(10_000);
    let base = tier * GEAR_PER_TIER * grade_bp / 10_000;
    base + i64::from(it.enhance_level) * GEAR_PER_ENHANCE
        + it.lines.len() as i64 * GEAR_PER_LINE
        + it.gems.len() as i64 * GEAR_PER_GEM
}

/// (rarity rung, stars) of the worn Exemplar; a bare unit reads as an SSR at S0.
fn rarity_and_stars(e: &Entity) -> (usize, i64) {
    e.exemplar
        .as_ref()
        .map(|x| (crate::items::exemplar_rarity_index(&x.id).unwrap_or(1).min(1), i64::from(x.stars)))
        .unwrap_or((1, 0))
}

fn unit_score(e: &Entity) -> i64 {
    let (r, _) = rarity_and_stars(e);
    let level = i64::from(e.stats.level.max(1));
    let base = UNIT_BASE.get(r).copied().unwrap_or(0);
    let per = UNIT_PER_LEVEL.get(r).copied().unwrap_or(0);
    base + per * (level - 1)
}

/// The star multiplier on the whole member, in bp.
fn star_mult_bp(e: &Entity) -> i64 {
    let (r, stars) = rarity_and_stars(e);
    10_000 + stars * STAR_BP.get(r).copied().unwrap_or(0)
}

fn regalia_score(e: &Entity) -> i64 {
    let Some(r) = e.regalia.as_ref() else { return 0 };
    let rung = crate::items::regalia_rarity_index(&r.id).unwrap_or(0).min(2);
    let base = REGALIA_BASE.get(rung).copied().unwrap_or(0)
        + REGALIA_PER_LEVEL.get(rung).copied().unwrap_or(0) * i64::from(r.level.max(1) - 1);
    base * (10_000 + i64::from(r.stars) * REGALIA_PER_STAR_BP) / 10_000
}

/// The party-shared share every fielded member carries (Training + Crests).
fn shared_score(state: &GameState) -> i64 {
    let train: i64 = state.training.values().map(|v| i64::from(*v)).sum();
    let crest: i64 = state.crests.values().map(|v| i64::from(*v)).sum();
    (train * TRAIN_PER_LEVEL + crest * CREST_PER_LEVEL) / i64::from(crate::data::MAX_PARTY as u32).max(1)
}

/// A member's own score — (unit + gear + regalia) × stars — without the party-shared share.
pub fn member_own_score(e: &Entity) -> i64 {
    if !matches!(e.team, Team::Player) || e.is_pet {
        return 0;
    }
    let built = unit_score(e) + e.equipped.iter().map(gear_score).sum::<i64>() + regalia_score(e);
    built * star_mult_bp(e) / 10_000 / SCALE
}

/// A member's Combat Score as shown: (unit + gear + regalia + its share of Training and
/// Crests) × stars.
pub fn member_score(state: &GameState, e: &Entity) -> i64 {
    if !matches!(e.team, Team::Player) || e.is_pet {
        return 0;
    }
    let built = unit_score(e) + e.equipped.iter().map(gear_score).sum::<i64>() + regalia_score(e) + shared_score(state);
    built * star_mult_bp(e) / 10_000 / SCALE
}

/// The party's Combat Score: the fielded members summed.
pub fn party_score(state: &GameState) -> i64 {
    state.grid.entities.iter().map(|e| member_score(state, e)).sum()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::grid::Cell;
    use crate::items::{Exemplar, Grade, Rarity};

    fn unit(id: &str, stars: u8, level: u32) -> Entity {
        let mut e = Entity::idle(1, Cell::new(0, 0)).with_team(Team::Player);
        e.exemplar = Some(Exemplar { id: id.into(), name: String::new(), skin_id: String::new(), stars });
        e.stats.level = level;
        e
    }

    /// Role never enters: two SSRs at the same investment read the same number; an SR S0 is
    /// 65% of an SSR S0, an SR S6 ≈ 95% of it, an SSR S6 = 2× its S0.
    #[test]
    fn same_investment_same_score_and_the_rarity_rule_holds() {
        let sera = member_own_score(&unit("seraphine", 0, 70));
        let nemu = member_own_score(&unit("nemu", 0, 70));
        let lapis = member_own_score(&unit("lapis", 0, 70));
        assert_eq!(sera, nemu, "two SSRs at the same investment read the same");
        assert_eq!(sera, lapis);
        // Lucielle is an SR since 0.0.469 — she reads the SR rung, like every other SR.
        assert_eq!(member_own_score(&unit("lucielle", 0, 70)), member_own_score(&unit("rowena", 0, 70)));
        let sr0 = member_own_score(&unit("rowena", 0, 70));
        let sr6 = member_own_score(&unit("rowena", 6, 70));
        let ssr6 = member_own_score(&unit("seraphine", 6, 70));
        assert!((64..=66).contains(&(sr0 * 100 / sera)), "SR S0 = 65% of SSR S0: {}", sr0 * 100 / sera);
        assert!((93..=97).contains(&(sr6 * 100 / sera)), "SR S6 ≈ 95% of SSR S0: {}", sr6 * 100 / sera);
        assert!((199..=201).contains(&(ssr6 * 100 / sera)), "SSR S6 = 2× S0: {}", ssr6 * 100 / sera);
    }

    #[test]
    fn gear_regalia_and_levels_add_and_a_monster_scores_nothing() {
        let mob = Entity::idle(9, Cell::new(0, 0)).with_team(Team::Monster);
        let s = GameState::new(1, 8, 8);
        assert_eq!(member_score(&s, &mob), 0);
        let a = unit("raplus", 0, 30);
        let base = member_own_score(&a);
        let mut b = a.clone();
        b.stats.level = 60;
        assert!(member_own_score(&b) > base);
        let mut c = a.clone();
        let mut sword = Item::new("sword_s3", ItemKind::Weapon, 100, 0, Rarity::Common);
        sword.grade = Grade::Legendary;
        c.equipped.push(sword);
        let geared = member_own_score(&c);
        assert!(geared > base);
        let mut d = c.clone();
        d.equipped[0].grade = Grade::Common;
        assert!(member_own_score(&d) < geared, "grade counts");
        let mut e = c.clone();
        e.equipped[0].enhance_level = 10;
        assert!(member_own_score(&e) > geared, "enhance counts");
    }
}
