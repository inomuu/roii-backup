//! Battle Pass — server season core (docs/battle-pass-architecture.md, P1).
//!
//! The sim never knows the pass exists: it ships `GameState.measures` (monotonic
//! success-path counters) and the server turns VERIFIED deltas into XP here.
//!
//! Invariants (docs/battle-pass-implementation-plan.md):
//!   BP1 — XP derives ONLY from accepted saves: accrual diffs the verified final
//!         state's tick/counters against the snapshot stored in `bp_state`.
//!   BP2 — time-XP is bounded twice: Δticks comes from a save that already passed
//!         the additive-slack fast-forward cap, and per-day time-XP is capped by
//!   BP8 — the day's deal is a PURE function `fnv1a(seed ‖ account ‖ day ‖ salt)`
//!         over the season catalog — auditable, replayable; reroll = salt bump,
//!         one credit/day.
//!
//! Accrual is deliberately NOT in the /save transaction: `bp_state` itself is the
//! anchor (cumulative counters diffed against the stored snapshot), so a crash
//! between the save commit and the BP commit self-heals — the next accepted save
//! re-accrues the missed delta. BP writes are transactional among THEMSELVES.

use serde::{Deserialize, Serialize};
use sim_core::state::Measures;

// ---------------------------------------------------------------- config

/// One season row (`seasons` table), config parsed.
#[derive(Debug, Clone)]
pub struct Season {
    pub season_id: i64,
    pub starts_at_ms: i64,
    pub ends_at_ms: i64,
    pub seed: u64,
    pub config: SeasonConfig,
}

/// `seasons.config_json` — everything a season is. Content changes = new row.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SeasonConfig {
    /// Pass XP for one DAILY LADDER RUNG claimed (user 2026-08-12).
    ///
    /// The pass earns NOTHING on its own — no XP for time online, no XP for merely playing. Every
    /// point comes from a mission, and today the only missions are the daily commissions; weekly
    /// and season objectives are planned and will price themselves the same way.
    ///
    /// This is the seam, and it points one way: `daily` reports which rung was claimed, and the
    /// pass decides here what that is worth. Nothing in `daily.rs` reads this.
    pub xp_per_daily_rung: u64,
    /// Pass XP for one WEEKLY LADDER RUNG claimed (user 2026-10-01). Absent in a season row written
    /// before it existed → `DEFAULT_XP_PER_WEEKLY_RUNG`.
    #[serde(default = "default_xp_per_weekly_rung")]
    pub xp_per_weekly_rung: u64,
    // NO DAILY HOOK. The pass earns from TIME, and will earn from its own weekly / season
    // objectives (planned 2026-08-12). Daily Commission pays crystal and stops there — the two
    // systems now share nothing at all, which is why `daily.rs` compiles without this module.
    /// XP per slot (linear curve v1): level = xp / slot_cost, capped at `slots`.
    pub slot_cost: u64,
    /// Track length (50).
    pub slots: u32,
    /// FREE-track rewards, 1-indexed by slot (`free_track[slot-1]`). All free rewards
    /// mint BOUND (design rule: the free track is unsellable everywhere). v1 restriction:
    /// stackable defs only (one mail carries def qty as a single bound instance).
    #[serde(default)]
    pub free_track: Vec<SlotReward>,
    /// PREMIUM-track rewards, same indexing. Claimable only with an active `bp_entitlements`
    /// row for the season (the `battle_pass` SKU). Mints BOUND like the free track.
    #[serde(default)]
    pub premium_track: Vec<SlotReward>,
}

/// What a slot pays. `Item` (the default, so older season rows still parse) mails `def_id`
/// bound; the rest credit the account directly.
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum RewardKind {
    #[default]
    Item,
    StandardTicket,
    LimitedTicket,
    /// BOUND crystal.
    Crystal,
    /// Gil by mail.
    Gil,
}

/// One slot's reward: a single item stack, or `qty` of a currency.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SlotReward {
    #[serde(default)]
    pub kind: RewardKind,
    #[serde(default)]
    pub def_id: String,
    pub qty: u32,
}

impl SlotReward {
    pub fn item(def_id: &str, qty: u32) -> Self {
        SlotReward { kind: RewardKind::Item, def_id: def_id.into(), qty }
    }
    pub fn currency(kind: RewardKind, qty: u32) -> Self {
        SlotReward { kind, def_id: String::new(), qty }
    }
}

/// Season length (user 2026-10-01: six weeks, the genre's version cadence).
pub const SEASON_DAYS: i64 = 42;

/// FINISH AT HALF SEASON (user 2026-10-01, HSR's pace): daily 5 × 35 = 175 a day, weekly 5 × 90 =
/// 450 a week — 5,025 by day 21 against the 5,000 the 50 slots cost. A player who starts on day 21
/// and does both still finishes by the last day; dailies alone finish around day 29.
pub const DEFAULT_XP_PER_DAILY_RUNG: u64 = 35;
pub const DEFAULT_XP_PER_WEEKLY_RUNG: u64 = 90;

fn default_xp_per_weekly_rung() -> u64 {
    DEFAULT_XP_PER_WEEKLY_RUNG
}

/// The season config (user 2026-10-01): 50 slots, every reward something the game still spends.
pub fn default_config() -> SeasonConfig {
    SeasonConfig {
        xp_per_daily_rung: DEFAULT_XP_PER_DAILY_RUNG,
        xp_per_weekly_rung: DEFAULT_XP_PER_WEEKLY_RUNG,
        slot_cost: 100,
        slots: 50,
        free_track: free_track(),
        premium_track: premium_track(),
    }
}

/// The gem sets the free track rotates through (the eight non-Arcana sets, grade 0).
const FREE_GEMS: [&str; 8] = [
    "gem_strife_g0", "gem_destruction_g0", "gem_ruin_g0", "gem_life_g0",
    "gem_fortitude_g0", "gem_wind_g0", "gem_valor_g0", "gem_charge_g0",
];

/// FREE: a Standard ticket every 10th slot, Sigil I ×6 every other 5th, and the slots between
/// cycle Memory Dust / Gil / EXP Elixir / a gem.
fn free_track() -> Vec<SlotReward> {
    let mut between = 0usize;
    (1..=50u32)
        .map(|slot| {
            if slot % 10 == 0 {
                return SlotReward::currency(RewardKind::StandardTicket, 1);
            }
            if slot % 5 == 0 {
                return SlotReward::item("sigil_1", 6);
            }
            let n = between;
            between += 1;
            match n % 4 {
                0 => SlotReward::item("regalia_fodder", 5),
                1 => SlotReward::currency(RewardKind::Gil, 20_000),
                2 => SlotReward::item("exp_elixir", 1),
                _ => SlotReward::item(FREE_GEMS.get((n / 4) % FREE_GEMS.len()).copied().unwrap_or("gem_strife_g0"), 1),
            }
        })
        .collect()
}

/// PREMIUM: Limited tickets ×2 every 10th slot, Track at 25 / 45, Boss Core at 5 / 15 / 35, and
/// the slots between cycle Radiant Memory Dust / Sigil II / EXP Elixir / bound crystal.
fn premium_track() -> Vec<SlotReward> {
    let mut between = 0usize;
    (1..=50u32)
        .map(|slot| match slot {
            s if s % 10 == 0 => SlotReward::currency(RewardKind::LimitedTicket, 2),
            25 | 45 => SlotReward::item("track", 1),
            5 | 15 | 35 => SlotReward::item("boss_core", 1),
            _ => {
                let n = between;
                between += 1;
                match n % 4 {
                    0 => SlotReward::item("regalia_sr_fodder", 2),
                    1 => SlotReward::item("sigil_2", 3),
                    2 => SlotReward::item("exp_elixir", 2),
                    _ => SlotReward::currency(RewardKind::Crystal, 100),
                }
            }
        })
        .collect()
}

/// The reward table for `track` (0 = free, 1 = premium).
pub fn track_rewards(cfg: &SeasonConfig, track: u8) -> Option<&[SlotReward]> {
    match track {
        0 => Some(&cfg.free_track),
        1 => Some(&cfg.premium_track),
        _ => None,
    }
}

// ---------------------------------------------------------------- deal (BP8)



/// Time-based pass XP for one accrue pass.
///
/// Level from XP (linear v1), capped at the track length.
pub fn level_of(cfg: &SeasonConfig, xp: u64) -> u32 {
    ((xp / cfg.slot_cost.max(1)) as u32).min(cfg.slots)
}

// ---------------------------------------------------------------- tests

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn level_is_linear_and_capped() {
        let cfg = default_config();
        assert_eq!(level_of(&cfg, 0), 0);
        assert_eq!(level_of(&cfg, cfg.slot_cost), 1);
        assert_eq!(level_of(&cfg, cfg.slot_cost * 999_999), cfg.slots, "level exceeded the track");
    }

    fn totals(track: &[SlotReward]) -> std::collections::BTreeMap<String, u32> {
        let mut m = std::collections::BTreeMap::new();
        for r in track {
            let key = if r.kind == RewardKind::Item {
                if FREE_GEMS.contains(&r.def_id.as_str()) { "gem".to_string() } else { r.def_id.clone() }
            } else {
                format!("{:?}", r.kind)
            };
            *m.entry(key).or_insert(0) += r.qty;
        }
        m
    }

    /// The season the user approved (2026-10-01), as totals.
    #[test]
    fn the_tracks_pay_the_approved_totals() {
        let cfg = default_config();
        assert_eq!((cfg.free_track.len(), cfg.premium_track.len()), (50, 50));
        let f = totals(&cfg.free_track);
        let want_free = [("StandardTicket", 5), ("sigil_1", 30), ("regalia_fodder", 50), ("Gil", 200_000), ("exp_elixir", 10), ("gem", 10)];
        assert_eq!(f, want_free.iter().map(|(k, v)| (k.to_string(), *v)).collect());
        let p = totals(&cfg.premium_track);
        let want_premium = [("LimitedTicket", 10), ("boss_core", 3), ("track", 2), ("regalia_sr_fodder", 20), ("sigil_2", 30), ("exp_elixir", 20), ("Crystal", 1_000)];
        assert_eq!(p, want_premium.iter().map(|(k, v)| (k.to_string(), *v)).collect());
    }

    /// XP after `days` of every daily and weekly rung.
    fn full_xp(cfg: &SeasonConfig, days: u64) -> u64 {
        5 * cfg.xp_per_daily_rung * days + 5 * cfg.xp_per_weekly_rung * (days / 7)
    }

    /// Doing every rung finishes the track by HALF the season, so a player who arrives mid-season
    /// still finishes by its end (user 2026-10-01); dailies alone finish inside the season too.
    #[test]
    fn the_track_finishes_by_half_season() {
        let cfg = default_config();
        let need = cfg.slot_cost * u64::from(cfg.slots);
        let half = (SEASON_DAYS / 2) as u64;
        assert!(full_xp(&cfg, half) >= need, "{} < {need} by day {half}", full_xp(&cfg, half));
        assert!(full_xp(&cfg, half - 3) < need, "finishes well before half season");
        assert!(5 * cfg.xp_per_daily_rung * (SEASON_DAYS as u64) >= need, "dailies alone never finish");
    }

    /// Every item slot names a real catalog def that is not a Steam item.
    #[test]
    fn every_item_slot_is_in_the_catalog() {
        let data = sim_core::data::GameData::from_json_str(include_str!("../../../data/game_data.json")).unwrap();
        let cfg = default_config();
        for r in cfg.free_track.iter().chain(&cfg.premium_track).filter(|r| r.kind == RewardKind::Item) {
            let d = data.item(&r.def_id);
            assert!(d.is_some_and(|d| d.steam_itemdef.is_none()), "{} missing or a Steam item", r.def_id);
        }
    }

    /// A season row written before `kind` / `xp_per_weekly_rung` existed still parses.
    #[test]
    fn an_old_season_row_still_parses() {
        let old = r#"{"xp_per_daily_rung":20,"slot_cost":100,"slots":50,
            "free_track":[{"def_id":"hp_pot_1","qty":5}],"premium_track":[]}"#;
        let cfg: SeasonConfig = serde_json::from_str(old).unwrap();
        assert_eq!(cfg.xp_per_weekly_rung, DEFAULT_XP_PER_WEEKLY_RUNG);
        assert_eq!(cfg.free_track[0], SlotReward::item("hp_pot_1", 5));
    }
}

