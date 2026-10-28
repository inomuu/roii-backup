//! Lane stats from the live catalog. Rough on purpose: base + per-level growth only, no gear,
//! skills, Regalia or Resonance — enough to see the rules move, not a balance read.

use crate::{Stats, Traits};
use sim_core::data::GameData;

/// Ticks per action for a `*_milli` rate (actions per tick × 1000), rounded up.
pub fn interval(milli: u32) -> u32 {
    1000u32.div_ceil(milli.max(1))
}

/// A unit at `level` (raw catalog numbers: base + per-level growth).
pub fn unit(data: &GameData, id: &str, level: i64) -> Option<(Stats, Traits)> {
    let u = data.units.iter().find(|u| u.id == id)?;
    let lv = (level - 1).max(0);
    let stats = Stats {
        hp: i64::from(u.hp) + i64::from(u.hp_per_level) * lv,
        atk: (i64::from(u.atk_base_milli) + i64::from(u.atk_per_level_milli) * lv) / 1000,
        def: i64::from(u.def),
        range: u.atk_range.max(1),
        atk_interval: interval(u.atk_speed_milli),
        move_interval: interval(u.speed_milli),
    };
    Some((stats, Traits::default()))
}

pub fn monster(data: &GameData, id: &str) -> Option<Stats> {
    let m = data.monsters.iter().find(|m| m.id == id)?;
    Some(Stats {
        hp: i64::from(m.hp),
        atk: i64::from(m.atk),
        def: i64::from(m.def),
        range: m.atk_range.max(1),
        atk_interval: interval(m.atk_speed_milli),
        move_interval: interval(m.speed_milli),
    })
}
