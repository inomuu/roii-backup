//! WEEKLY COMMISSION — the daily board, five times over, on a seven-day clock.
//!
//! Deliberately NOT a second copy of the accrual code. A commission board is a `[DailyDef]`, a
//! reward ladder is a `[Milestone]`, and `daily::{open_day, accrue, points_of, reached_mask,
//! claimable}` are pure over (board, ladder, counters, masks). So a "weekly" is those same
//! functions fed three different things and nothing else:
//!   * targets ×5     — "the same quest, five times" (user 2026-08-30),
//!   * a flat ladder  — 100 crystal per completed commission, at most five paid = 500/week,
//!   * a seven-day window instead of a day.
//!
//! Reusing the daily's machinery means the pay-once ledger, the stable-bit mask, the run-twice
//! self-healing accrual, and the `NO_START` hole are all inherited already-correct rather than
//! re-derived — the daily learned each of those the hard way and its tests still guard them.

use crate::daily::{self, DailyDef, Milestone};

/// Weekly's slice of the free-crystal faucet: **500/week** — 2.5× a day's `DAILY_GEM_SHARE` (200),
/// written as the finished value, not a runtime multiply (user 2026-08-30).
///
/// Carved from the reserve the daily deliberately left behind: `daily::FREE_GEM_BUDGET_PER_DAY` is
/// 350/day and the daily ladder takes 200/day, leaving 150/day = 1050/week unspent-but-reserved, of
/// which this takes 500. `weekly_and_daily_stay_inside_the_free_budget` turns "there is room" into a
/// checked fact — but only for the TWO sources it names; a later third faucet must extend that test
/// itself, it is not caught automatically. The bound is also a SUSTAINED-RATE one, not a per-day
/// cap: `/weekly/claim` pays the whole 500 in one burst, so on a single day a player can mint
/// 200 (daily) + 500 (weekly) = 700 bound crystal. That is by design (a week's share, paid once); the
/// budget guarantees the AVERAGE stays under 350/day, not any single day.
#[allow(dead_code)] // the SPEC, enforced by `weekly_and_daily_stay_inside_the_free_budget`
pub const WEEKLY_GEM_SHARE: u32 = 500;

/// How many completed commissions are paid — the "max 5" (user 2026-08-30). The board offers more
/// than five, so one objective having a bad week is not a lost payout; the cap falls out of the
/// ladder topping out at five rungs, the same way the daily's does.
pub const PAID_COMMISSIONS: u32 = 5;

/// Crystal each paid commission pays — 100, so the five rungs sum to the 500 share.
pub const GEMS_PER_COMMISSION: u32 = 100;

/// The weekly board: the daily's eight commissions with targets at the week's scale — the daily's
/// ×5, "the same quest, five times" (user 2026-08-30), written out as finished CONSTANTS rather than
/// multiplied off `daily::board()` at runtime. The ids/bits/counters/kinds match the daily so a
/// weekly row's mask means what a daily row's does; the targets are the weekly's own to tune.
pub fn board() -> Vec<DailyDef> {
    use daily::CounterId::*;
    use daily::DailyKind::*;
    let d = |id: &str, bit: u8, counter, target, kind| DailyDef {
        id: id.into(),
        bit,
        counter,
        target,
        kind,
        points: daily::POINTS_PER_COMMISSION,
    };
    vec![
        // AUTO (3) — finished by idle play (the daily's targets ×5).
        d("kills_2500", 0, Kills, 12_500, Auto),
        d("rooms_10", 1, RoomsCleared, 50, Auto),
        d("damage_1m", 2, DamageDealt, 5_000_000, Auto),
        // ACTION (5) — one deliberate act each, over the week.
        d("fuse_3", 9, Fusions, 15, Action),
        // Bits 4 (refine, 0.0.370) and 8 (card feed, cards held back 2026-09-30) are retired.
        // One is enough on the week too (user 2026-09-30).
        d("gem_fuse_1", 11, GemFusions, 1, Action),
        d("train_1", 10, TrainingLevels, 5, Action),
        d("spend_gil", 6, GilSpent, 50_000, Action),
        d("soul_node_1", 7, SoulNodesAllocated, 5, Action),
    ]
}

/// A FLAT ladder: one rung per completed commission (every `POINTS_PER_COMMISSION` points), each
/// paying `GEMS_PER_COMMISSION`, for `PAID_COMMISSIONS` rungs. Reaching all five pays the whole
/// `WEEKLY_GEM_SHARE`; completing a sixth pays nothing because the ladder simply stops — the "max
/// 5" cap is that absence, not a special case, and `points_of` already clamps to the top rung.
pub fn ladder() -> Vec<Milestone> {
    (1..=PAID_COMMISSIONS)
        .map(|n| Milestone { points: n * daily::POINTS_PER_COMMISSION, crystal: GEMS_PER_COMMISSION })
        .collect()
}

// ---------------------------------------------------------------- window

/// Days per commission week.
const DAYS_PER_WEEK: i64 = 7;

/// Index of the commission WEEK a server timestamp falls in: the daily boundary (04:00 Thailand),
/// bucketed into sevens. Anchored to the same instant as `daily::day_of_ms`, so a week always
/// begins ON a day boundary and never flips mid-day. Server-side only — a client's clock never
/// decides when its week rolls over, exactly as for the day.
pub fn week_of_ms(now_ms: u64) -> i64 {
    (daily::day_of_ms(now_ms) + WEEK_SHIFT).div_euclid(DAYS_PER_WEEK)
}

/// The game day a week starts on: day 0 begins Friday 1970-01-02 04:00 Thailand time, so day 3 is
/// Monday. Weeks turn over Monday 04:00 — Sunday night into Monday morning (user 2026-10-03; the
/// bare `day / 7` turned them over on Friday).
const WEEK_START_DAY: i64 = 3;
/// Shifts days so a week begins on `WEEK_START_DAY` with ids that never fall BELOW the old
/// `day / 7` ids: subtracting instead re-opened last week's (already played) row at the switch.
const WEEK_SHIFT: i64 = DAYS_PER_WEEK - WEEK_START_DAY;

/// The instant the week AFTER `week` begins, in epoch ms — the `day_end` of the week's last day, so
/// the client renders one countdown the server owns rather than computing the boundary itself.
pub fn week_end_ms(week: i64) -> i64 {
    daily::day_end_ms((week + 1) * DAYS_PER_WEEK - WEEK_SHIFT - 1)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The week turns over Monday 04:00 Thailand time (= Sunday 21:00 UTC).
    #[test]
    fn the_week_turns_over_on_monday_morning_thai_time() {
        // 2026-10-04 is a Sunday; 21:00 UTC that evening is Monday 04:00 in Thailand.
        let monday_0400_th: u64 = 1_791_147_600_000; // 2026-10-04T21:00:00Z
        let w = week_of_ms(monday_0400_th);
        assert_eq!(week_of_ms(monday_0400_th - 1), w - 1, "one ms earlier is the previous week");
        assert_eq!(week_end_ms(w - 1), monday_0400_th as i64, "the previous week ends exactly then");
        assert_eq!(week_of_ms(monday_0400_th + 6 * 86_400_000 + 86_399_999), w, "Sunday night is still the same week");
    }

    /// The board is the daily's, with every target multiplied by five and nothing else touched.
    #[test]
    fn the_board_is_the_daily_board_times_five() {
        let day = daily::board();
        let week = board();
        assert_eq!(week.len(), day.len(), "weekly must offer the same commissions as the daily");
        for (d, w) in day.iter().zip(week.iter()) {
            assert_eq!(w.id, d.id, "same commission");
            assert_eq!(w.bit, d.bit, "same bit — a weekly row must mean what a daily row means");
            assert_eq!(w.counter, d.counter, "same counter");
            assert_eq!(w.kind, d.kind, "same kind");
            assert_eq!(w.points, d.points, "same points");
            // The hand-written weekly targets are the daily's ×5 — pinned so a typo in a constant
            // above is caught, without deriving them (the board keeps its own values to tune).
            // One exception, by decision: a single gem fusion finishes the week too (user
            // 2026-09-30, "ผสม 1 ครั้งก็พอ").
            let want = if w.id == "gem_fuse_1" { d.target } else { d.target.saturating_mul(5) };
            assert_eq!(w.target, want, "weekly target must be the daily's ×5 ({})", w.id);
        }
    }

    /// Five rungs, 100 crystal each, summing to the whole weekly share — the "100/quest, max 5".
    #[test]
    fn the_ladder_pays_a_hundred_a_rung_for_five_rungs() {
        let l = ladder();
        assert_eq!(l.len(), PAID_COMMISSIONS as usize, "exactly five paid rungs");
        for (i, rung) in l.iter().enumerate() {
            assert_eq!(rung.crystal, GEMS_PER_COMMISSION, "each rung pays a flat 100");
            let n = (i + 1) as u32;
            assert_eq!(rung.points, n * daily::POINTS_PER_COMMISSION, "a rung per completed commission");
        }
        let total: u32 = l.iter().map(|m| m.crystal).sum();
        assert_eq!(total, WEEKLY_GEM_SHARE, "the five rungs must sum to the weekly share");
    }

    /// Completing more than five commissions still pays only five — the cap is the ladder stopping.
    #[test]
    fn a_sixth_commission_pays_nothing() {
        let (board, ladder) = (board(), ladder());
        // Every commission on the board done.
        let all_done: u16 = board.iter().map(|d| 1u16 << d.bit).fold(0, |a, b| a | b);
        let points = daily::points_of(&board, &ladder, all_done);
        assert_eq!(points, WEEKLY_GEM_SHARE, "points clamp to the top rung (500), not 8×100");
        let (_, crystal) = daily::claimable(&ladder, points, 0);
        assert_eq!(crystal, WEEKLY_GEM_SHARE as u64, "a full board pays the cap, not more");
    }

    /// A three-commission week pays exactly three rungs — the ladder is linear below the cap.
    #[test]
    fn three_done_pays_three_hundred() {
        let (board, ladder) = (board(), ladder());
        // Set the first three bits the board defines.
        let three: u16 = board.iter().take(3).map(|d| 1u16 << d.bit).fold(0, |a, b| a | b);
        let points = daily::points_of(&board, &ladder, three);
        let (_, crystal) = daily::claimable(&ladder, points, 0);
        assert_eq!(crystal, 3 * GEMS_PER_COMMISSION as u64, "three commissions pay 300");
    }

    /// The weekly faucet fits inside the free-crystal budget the daily reserved — the whole reason
    /// `WEEKLY_GEM_SHARE` is a checked constant and not just a number in the ladder.
    #[test]
    fn weekly_and_daily_stay_inside_the_free_budget() {
        // A SUSTAINED-RATE bound, not a per-day cap (a weekly claim bursts its whole share at once —
        // see `WEEKLY_GEM_SHARE`). Amortise the week's share across seven days, ceil so the bound is
        // the conservative one, and check the AVERAGE daily mint stays under the free budget.
        let weekly_per_day = WEEKLY_GEM_SHARE.div_ceil(DAYS_PER_WEEK as u32);
        assert!(
            daily::DAILY_GEM_SHARE + weekly_per_day <= daily::FREE_GEM_BUDGET_PER_DAY,
            "daily {} + weekly/day {} exceeds the free budget {}",
            daily::DAILY_GEM_SHARE, weekly_per_day, daily::FREE_GEM_BUDGET_PER_DAY,
        );
    }

    /// A week is seven commission days, aligned to the day boundary so it never flips mid-day.
    #[test]
    fn a_week_is_seven_days_aligned_to_the_day_boundary() {
        // Days 3..=9 (Monday..Sunday) all fall in one week; day 10 rolls it over.
        let first = week_of_ms(daily::day_end_ms(WEEK_START_DAY - 1) as u64 + 3_600_000);
        for day in WEEK_START_DAY..WEEK_START_DAY + DAYS_PER_WEEK {
            let mid = daily::day_end_ms(day - 1) as u64 + 3_600_000; // an hour into `day`
            assert_eq!(week_of_ms(mid), first, "day {day} must sit in the same week");
        }
        let into_next = daily::day_end_ms(WEEK_START_DAY + DAYS_PER_WEEK - 1) as u64 + 3_600_000;
        assert_eq!(week_of_ms(into_next), first + 1, "the eighth day must roll into the next week");
        // week_end_ms(w) is exactly the instant week w+1's first day begins.
        assert_eq!(week_of_ms(week_end_ms(first) as u64), first + 1, "the boundary belongs to the next week");
        // Ids never fall below the old `day / 7` ones (no reopened week at the switch).
        for day in 20_000..20_014i64 {
            let at = daily::day_end_ms(day - 1) as u64 + 3_600_000;
            assert!(week_of_ms(at) >= day.div_euclid(DAYS_PER_WEEK), "day {day}");
        }
    }
}
