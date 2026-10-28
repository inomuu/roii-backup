//! A STAGE (user 2026-10-04): 7–10 waves, then one big boss with its minions. Between waves the
//! party walks on through a looping map. The walk is a DISTANCE: 5–7 s at base walking speed,
//! shorter for a party with Move Speed. HP does not recover between waves, and cooldowns keep
//! counting through the walk. A wipe fails the stage (it restarts from wave 1).
//!
//! Wave size grows with the stage: early waves are small, and late waves are 4–9 monsters. Every
//! monster is a live one from the stage's zone pool, entering a random lane. The boss is one cell,
//! only drawn big.

use sim_core::combat;
use sim_core::data::GameData;

use crate::bench::{self, BenchSpec};
use crate::{Battle, Outcome, Team, Traits, LANES};

/// Ticks a wave may last before it is called a stall.
pub const WAVE_LIMIT_TICKS: u64 = 3_000;
/// Base walking speed (`speed_milli` of every unit today): the speed the 5–7 s walk is measured at.
pub const BASE_SPEED_MILLI: i64 = 333;

/// SplitMix64: a small deterministic integer stream for the stage's own rolls.
pub struct Rolls(u64);

impl Rolls {
    pub fn new(seed: u64) -> Self {
        Rolls(seed)
    }
    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }
    /// Uniform in `lo..=hi`.
    pub fn range(&mut self, lo: u64, hi: u64) -> u64 {
        lo + self.next() % (hi - lo + 1)
    }
}

/// Monsters per wave at `stage` (of 240): 2–4 at the start, 4–9 at the top.
pub fn wave_size_range(stage: u32) -> (u64, u64) {
    let s = u64::from(stage.min(240));
    (2 + 2 * s / 240, 4 + 5 * s / 240)
}

#[derive(Clone, Debug)]
pub struct WaveReport {
    pub monsters: usize,
    pub fight_ticks: u64,
    pub walk_ticks: u64,
    pub party_hp_bp: i64,
}

#[derive(Clone, Debug)]
pub struct StageReport {
    pub cleared: bool,
    pub waves: Vec<WaveReport>,
    pub boss_ticks: u64,
    pub boss_minions: usize,
    pub total_ticks: u64,
    pub dead: usize,
}

/// The party's walk, in ticks, over a distance that takes `secs` at `BASE_SPEED_MILLI`: the
/// SLOWEST standing member sets the pace, its live Move Speed bonus included.
fn walk_ticks(b: &Battle, secs: u64) -> u64 {
    let slowest = b.units.iter().filter(|u| u.team == Team::Party && u.alive)
        .filter_map(|u| u.ent.as_deref())
        .map(|e| i64::from(e.speed_milli) * (10_000 + i64::from(combat::stat_mods::move_speed_bp(e))) / 10_000)
        .min().unwrap_or(BASE_SPEED_MILLI).max(1);
    u64::try_from(i64::try_from(secs * 10).unwrap_or(0) * BASE_SPEED_MILLI / slowest).unwrap_or(0)
}

fn party_hp_bp(b: &Battle) -> i64 {
    let (hp, max) = b.units.iter().filter(|u| u.team == Team::Party)
        .fold((0, 0), |(h, m), u| (h + u.hp.max(0), m + u.stats.hp));
    if max == 0 { 0 } else { hp * 10_000 / max }
}

fn fight(b: &mut Battle) -> (Outcome, u64) {
    let start = b.tick;
    while b.tick - start < WAVE_LIMIT_TICKS {
        b.step();
        if let Some(o) = b.outcome() {
            return (o, b.tick - start);
        }
    }
    (Outcome::Timeout, b.tick - start)
}

/// Run one stage for a placed party (`b`) at `spec` (its `stage` picks the zone and the boss).
pub fn run(data: &GameData, spec: &BenchSpec, b: &mut Battle, rolls: &mut Rolls) -> Result<StageReport, String> {
    let waves = rolls.range(7, 10) as usize;
    let (lo, hi) = wave_size_range(spec.stage);
    let sizes: Vec<usize> = (0..waves).map(|_| rolls.range(lo, hi) as usize).collect();
    let mobs = bench::monsters(data, spec, sizes.iter().sum())?;
    let mut mobs = mobs.into_iter();
    let start = b.tick;
    let mut report = StageReport { cleared: false, waves: Vec::new(), boss_ticks: 0, boss_minions: 0, total_ticks: 0, dead: 0 };
    for n in sizes.iter().copied() {
        // The walk to this wave (the first one included: the stage opens on a walk-in).
        let walk = walk_ticks(b, rolls.range(5, 7));
        for _ in 0..walk {
            b.step();
        }
        b.reset_formation();
        for _ in 0..n {
            let lane = rolls.range(0, (LANES - 1) as u64) as i32;
            if let Some(e) = mobs.next() {
                b.spawn_entity("mob", e, Traits::default(), lane);
            }
        }
        let (out, ticks) = fight(b);
        report.waves.push(WaveReport { monsters: n, fight_ticks: ticks, walk_ticks: walk, party_hp_bp: party_hp_bp(b) });
        if out != Outcome::Cleared {
            report.total_ticks = b.tick - start;
            report.dead = b.units.iter().filter(|u| u.team == Team::Party && !u.alive).count();
            return Ok(report);
        }
    }
    let walk = walk_ticks(b, rolls.range(5, 7));
    for _ in 0..walk {
        b.step();
    }
    b.reset_formation();
    let (boss, minions) = bench::boss(data, spec)?;
    report.boss_minions = minions.len();
    b.spawn_entity("BOSS", boss, Traits::default(), 1);
    for m in minions {
        let lane = rolls.range(0, (LANES - 1) as u64) as i32;
        b.spawn_entity("minion", m, Traits::default(), lane);
    }
    let (out, ticks) = fight(b);
    report.boss_ticks = ticks;
    report.cleared = out == Outcome::Cleared;
    report.total_ticks = b.tick - start;
    report.dead = b.units.iter().filter(|u| u.team == Team::Party && !u.alive).count();
    Ok(report)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Col, Rules};

    #[test]
    fn waves_grow_with_the_stage_and_the_rolls_replay() {
        assert_eq!(wave_size_range(1), (2, 4));
        assert_eq!(wave_size_range(240), (4, 9), "late game: 4-9");
        let a: Vec<u64> = { let mut r = Rolls::new(7); (0..20).map(|_| r.range(7, 10)).collect() };
        let b: Vec<u64> = { let mut r = Rolls::new(7); (0..20).map(|_| r.range(7, 10)).collect() };
        assert_eq!(a, b);
        assert!(a.iter().all(|w| (7..=10).contains(w)));
    }

    #[test]
    fn move_speed_shortens_the_walk_and_a_stage_runs_waves_then_the_boss() {
        let data = GameData::from_json_str(include_str!("../../../data/game_data.json")).unwrap();
        let spec = BenchSpec::default();
        let mut b = Battle::new(Rules::default());
        for (id, col, lane) in [("seraphine", Col::Front, 1), ("lumi", Col::Front, 0), ("lapis", Col::Back, 1)] {
            let (e, t) = bench::unit(&data, id, &spec).unwrap();
            b.place_entity(id, e, t, col, lane).unwrap();
        }
        let base = walk_ticks(&b, 6);
        for u in b.units.iter_mut() {
            if let Some(e) = u.ent.as_deref_mut() {
                e.speed_milli *= 2;
            }
        }
        assert!(walk_ticks(&b, 6) < base, "a faster party covers the same distance sooner");
        let r = run(&data, &spec, &mut b, &mut Rolls::new(1)).unwrap();
        assert!((7..=10).contains(&r.waves.len()) || !r.cleared);
        assert!(!r.cleared || r.boss_ticks > 0, "a cleared stage ends on its boss");
    }
}
