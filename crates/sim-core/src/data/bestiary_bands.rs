//! How strong a monster is FOR WHAT IT IS — the bestiary's answer now that absolute numbers
//! depend on where the creature is standing.
//!
//! Since 0.0.280 a monster's HP is `level_base[the zone's level] x its own multiplier`, so the
//! same banshee is 1535 HP in one band and 2043 in another and neither number is "banshee". The
//! multiplier is what does not move, and it is the thing worth showing.
//!
//! COMPARED WITHIN ITS OWN KIND, which was measured rather than assumed. On one shared scale
//! 29 of the 45 ordinary monsters read "low" and all 22 bosses read "very high" — because the
//! multipliers are each monster's share of its band's GEOMETRIC MEAN, and that mean includes the
//! band's boss, which drags every ordinary mob below 1.0 (their median is 0.78x, the bosses' is
//! 1.98x). A scale where the common case is "low" and the top band means "is a boss" tells the
//! player nothing they cannot already see. Dividing by the group's own median fixes both ends:
//! `mob_b11_a` at 0.65x becomes 0.83x of its peers and reads normal, and dullahan at 8.58x
//! becomes 4.3x of the bosses and stays exceptional where an ordinary boss is merely typical.
//!
//! INTEGER ONLY, like everything else here — every ratio is basis points, no floats anywhere.

// DEBT (3 sites): this file predates `deny(clippy::indexing_slicing)`. An index that
// cannot be proven in range is a panic, and in verify-server (which links this crate and
// replays saves under `panic = "abort"`) a panic is the whole process. Shrink this, never
// add to it — a NEW file has no allow and is rejected on its first bare index.
#![allow(clippy::indexing_slicing)]

use super::defs::{GameData, MonsterDef};

/// Five bands, thresholds as the user set them (0.5 / 0.8 / 1.2 / 1.5), applied to the RATIO
/// against the group's median rather than to the raw multiplier.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StatBand {
    VeryLow,
    Low,
    Normal,
    High,
    VeryHigh,
}

impl StatBand {
    /// A stable key for the UI to localise. Never a display string — sim-core is
    /// locale-agnostic and the client owns every word the player reads.
    pub fn key(self) -> &'static str {
        match self {
            StatBand::VeryLow => "very_low",
            StatBand::Low => "low",
            StatBand::Normal => "normal",
            StatBand::High => "high",
            StatBand::VeryHigh => "very_high",
        }
    }

    fn of_ratio_bp(ratio_bp: u32) -> Self {
        match ratio_bp {
            0..=4_999 => StatBand::VeryLow,
            5_000..=7_999 => StatBand::Low,
            8_000..=12_000 => StatBand::Normal,
            12_001..=15_000 => StatBand::High,
            _ => StatBand::VeryHigh,
        }
    }
}

/// Which stat a band is describing. Only the two the bestiary shows — DEF is `x1` on 65 of the
/// 67 monsters (identity lives in `def_mult_bp` and almost nobody names one), so a band for it
/// would read "normal" for everything and earn no space.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BandStat {
    Hp,
    Atk,
}

/// The medians a monster is judged against. Built once per catalog — walking every zone to find
/// out who is a boss is not something to redo per bestiary row.
#[derive(Debug, Clone, Default)]
pub struct BandScale {
    boss_hp_bp: u32,
    boss_atk_bp: u32,
    mob_hp_bp: u32,
    mob_atk_bp: u32,
}

/// A multiplier of 0 means "no multiplier named", which `base_scaled_stats` reads as x1 — the
/// same reading has to happen here or an unauthored monster would be judged as 0x and read
/// "very low" while standing at exactly the level's base.
fn mult(bp: u32) -> u32 {
    if bp == 0 { 10_000 } else { bp }
}

fn median(mut v: Vec<u32>) -> u32 {
    if v.is_empty() {
        return 10_000;
    }
    v.sort_unstable();
    let n = v.len();
    if n % 2 == 1 { v[n / 2] } else { (v[n / 2 - 1] + v[n / 2]) / 2 }
}

impl BandScale {
    pub fn build(data: &GameData) -> Self {
        // A monster is judged as a BOSS if any zone fields it as one, even if it also walks in a
        // pool somewhere — being a band's boss is the stronger claim about what it is.
        let mut boss_ids: Vec<&str> = Vec::new();
        let mut pool_ids: Vec<&str> = Vec::new();
        for z in &data.zones {
            if !z.boss_monster_id.is_empty() && !boss_ids.contains(&z.boss_monster_id.as_str()) {
                boss_ids.push(&z.boss_monster_id);
            }
            for e in &z.spawn_pool {
                if !pool_ids.contains(&e.monster_id.as_str()) {
                    pool_ids.push(&e.monster_id);
                }
            }
        }
        let pick = |ids: &[&str], f: fn(&MonsterDef) -> u32| -> Vec<u32> {
            ids.iter()
                .filter_map(|id| data.monster(id))
                .map(|m| mult(f(m)))
                .collect()
        };
        let pool_only: Vec<&str> =
            pool_ids.iter().copied().filter(|id| !boss_ids.contains(id)).collect();
        Self {
            boss_hp_bp: median(pick(&boss_ids, |m| m.hp_mult_bp)),
            boss_atk_bp: median(pick(&boss_ids, |m| m.atk_mult_bp)),
            mob_hp_bp: median(pick(&pool_only, |m| m.hp_mult_bp)),
            mob_atk_bp: median(pick(&pool_only, |m| m.atk_mult_bp)),
        }
    }

    /// `is_boss` is the caller's, not re-derived here: the bestiary groups variants under one key
    /// and the representative it picked is the one whose band is being shown.
    pub fn band(&self, m: &MonsterDef, stat: BandStat, is_boss: bool) -> StatBand {
        let (value, med) = match (stat, is_boss) {
            (BandStat::Hp, true) => (mult(m.hp_mult_bp), self.boss_hp_bp),
            (BandStat::Hp, false) => (mult(m.hp_mult_bp), self.mob_hp_bp),
            (BandStat::Atk, true) => (mult(m.atk_mult_bp), self.boss_atk_bp),
            (BandStat::Atk, false) => (mult(m.atk_mult_bp), self.mob_atk_bp),
        };
        let med = med.max(1);
        StatBand::of_ratio_bp(((value as u64) * 10_000 / (med as u64)) as u32)
    }

    /// Whether any zone fields this monster as its boss.
    pub fn is_boss(data: &GameData, id: &str) -> bool {
        data.zones.iter().any(|z| z.boss_monster_id == id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn catalog() -> GameData {
        GameData::from_json_str(include_str!("../../../../data/game_data.json"))
            .expect("game_data.json must parse")
    }

    /// MEASURES the spread the player will actually see.
    ///
    /// The first version of this test asserted no band may hold more than half the roster, and it
    /// failed at 51 of 67 in "normal". The assertion was wrong, not the scale: measured against
    /// their own median, ordinary monsters run p25 0.83 / p75 1.08 on HP and p25 1.00 / p75 1.05
    /// on ATK. Most creatures ARE the same strength for their level — only 6 of 45 fall outside
    /// +-20% on ATK — because that is how the roster was built. A bestiary that says so is telling
    /// the truth, and it makes the handful of outliers findable at a glance, which is the point.
    ///
    /// So what is checked is that the scale is not DEGENERATE: the extremes are reachable and more
    /// than one label is in use. A scale that answers "normal" to everything would be decoration.
    #[test]
    fn the_bands_are_used_and_the_common_case_is_the_common_answer() {
        let d = catalog();
        let scale = BandScale::build(&d);

        let mut placed: Vec<&MonsterDef> = Vec::new();
        for z in &d.zones {
            for e in &z.spawn_pool {
                if let Some(m) = d.monster(&e.monster_id) {
                    if !placed.iter().any(|x| x.id == m.id) { placed.push(m); }
                }
            }
            if let Some(m) = d.monster(&z.boss_monster_id) {
                if !placed.iter().any(|x| x.id == m.id) { placed.push(m); }
            }
        }
        assert!(placed.len() > 50, "only {} monsters placed — nothing measured", placed.len());

        for stat in [BandStat::Hp, BandStat::Atk] {
            let mut counts = [0usize; 5];
            for m in &placed {
                let b = scale.band(m, stat, BandScale::is_boss(&d, &m.id));
                counts[match b {
                    StatBand::VeryLow => 0,
                    StatBand::Low => 1,
                    StatBand::Normal => 2,
                    StatBand::High => 3,
                    StatBand::VeryHigh => 4,
                }] += 1;
            }
            let total: usize = counts.iter().sum();
            println!("{stat:?} bands over {total} placed monsters: {counts:?}");
            let used = counts.iter().filter(|c| **c > 0).count();
            assert!(
                used >= 4,
                "{stat:?}: only {used} of the 5 bands are ever used {counts:?} — labels nothing \
                 can earn are decoration"
            );
            assert!(
                counts[2] > 0,
                "{stat:?}: NOTHING reads as normal {counts:?} — 'normal' has to describe the \
                 common case or the other four labels have no anchor"
            );
            assert!(
                counts[0] + counts[1] > 0 && counts[3] + counts[4] > 0,
                "{stat:?}: nothing is notable in either direction {counts:?} — a scale that only \
                 ever answers 'normal' is worse than showing nothing"
            );
        }
    }

    /// The two ends have to keep meaning something, or the scale is decorative. These are the
    /// creatures the labels were sanity-checked against by hand.
    #[test]
    fn the_extremes_are_still_extreme_after_dividing_by_the_median() {
        let d = catalog();
        let scale = BandScale::build(&d);
        let band = |id: &str, stat| {
            scale.band(d.monster(id).expect(id), stat, BandScale::is_boss(&d, id))
        };
        // The frailest thing in the world, at 0.34x where its peers sit at 0.78x.
        assert_eq!(band("bat", BandStat::Hp), StatBand::VeryLow);
        // 8.58x raw is 4.3x of the bosses — it must NOT read the same as an ordinary boss.
        assert_eq!(band("dullahan", BandStat::Hp), StatBand::VeryHigh);
        assert_ne!(
            band("boss_b5", BandStat::Hp),
            StatBand::VeryHigh,
            "an ordinary band boss reading 'very high' is the flaw this scale exists to fix"
        );
        // A plain mid-band mob is the anchor for 'normal'.
        assert_eq!(band("mob_b11_a", BandStat::Hp), StatBand::Normal);
    }

    /// An unnamed multiplier is x1 everywhere else in the build path; it must be x1 here too.
    #[test]
    fn a_monster_that_names_no_multiplier_is_judged_at_one_times() {
        assert_eq!(mult(0), 10_000);
        assert_eq!(mult(7_500), 7_500);
        assert_eq!(median(vec![]), 10_000);
        assert_eq!(median(vec![300, 100, 200]), 200);
        assert_eq!(median(vec![100, 200, 300, 400]), 250);
    }
}
