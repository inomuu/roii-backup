//! DAILY COMMISSION — the eight-item board every account works through each day.
//!
//! Its own module, not a corner of `bp`, because the two run on different clocks and answer to
//! different rules: a commission day opens at 04:00 and its rewards are forfeited at the next
//! boundary, while a Season Pass slot stays claimable for the whole season. They were entangled
//! only because dailies were first built as a battle-pass feature.
//!
//! THE DEPENDENCY RUNS ONE WAY. Nothing here knows what a battle pass is, what XP is worth, or
//! that a season exists. `accrue` reports how many commissions were completed on this pass; the
//! pass side reads that number and decides what to pay for it. Reverse the arrow and the daily
//! could not ship without a season, which is exactly the coupling being removed.
//!
//! Everything is a pure function over (config, counters, masks) so the whole board is testable
//! without a database.

use serde::{Deserialize, Serialize};
use sim_core::state::{Counter, Measures};

#[allow(dead_code)] // the SPEC, enforced by `the_daily_ladder_stays_inside_its_share`
pub const FREE_GEM_BUDGET_PER_DAY: u32 = 350;

/// The DAILY LADDER's slice of that budget (user 2026-08-12). Deliberately not the whole thing:
/// the other touchpoints do not exist yet, and handing the full faucet to the first one built
/// means every later one is either free inflation or a fight to claw budget back.
///
/// The leftover is reserved, not spare. When a second source lands it takes its share from here,
/// and `the_daily_ladder_stays_inside_its_share` is what makes that a visible decision instead of
/// a quiet overrun.
#[allow(dead_code)] // ditto
pub const DAILY_GEM_SHARE: u32 = 200;

/// One rung of the day's point ladder.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub struct Milestone {
    /// Points needed, counting from the start of the day.
    pub points: u32,
    /// Crystal paid the first time the day's total reaches `points`.
    pub crystal: u32,
}

/// One daily objective template: complete when the day's counter delta ≥ target.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DailyDef {
    pub id: String,
    /// The bit this commission owns in `done_mask`, and its slot in `start_values`. STABLE and
    /// author-assigned — NOT the position in `board()`.
    ///
    /// A position-keyed mask re-points every in-flight row the moment the board is reordered or a
    /// commission is removed: yesterday's `kills` bit becomes today's `rooms` bit, and a row's
    /// opening counter is compared against a different counter entirely. The same hazard bit the
    /// consumable slots twice (see sim-core `ConsumableSlot::key`) and `schedule::Entry` was built
    /// with `id` for exactly this reason.
    ///
    /// Assigned equal to the current positions, so this change moves no live row.
    pub bit: u8,
    /// Which `Measures` field this reads.
    pub counter: CounterId,
    pub target: u64,
    /// "auto" (completes from idle play) or "action" (one intentional click).
    pub kind: DailyKind,
    /// Points this objective contributes toward the day's milestones.
    #[serde(default = "default_points")]
    pub points: u32,
}

fn default_points() -> u32 {
    100
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum DailyKind {
    Auto,
    Action,
}

/// Stable names for the sim's `Measures` fields (serde names = wire/config names).
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum CounterId {
    Kills,
    RoomsCleared,
    OrbsUsed,
    CardsFed,
    ExemplarsFed,
    ItemsCrafted,
    ItemsListed,
    ItemsSold,
    GilSpent,
    SoulNodesAllocated,
    // 0.0.286-0.0.291. Only counters that RISE with play belong here: a daily reads a day's
    // DELTA, so anything that can fall would read as negative progress. `Measures` guarantees
    // monotonic, which is exactly why maxima live in `Records` and are not addressable here.
    BossKills,
    BandsCleared,
    DamageDealt,
    DamageTaken,
    HealingDone,
    SkillsCast,
    LevelsGained,
    Deaths,
    // 0.0.411-0.0.417: the sinks that replaced vendoring, crafting and recruiting.
    Fusions,
    TrainingLevels,
    ItemsDisenchanted,
    CrestLevels,
    GemFusions,
}

impl CounterId {
    pub fn read(self, m: &Measures) -> u64 {
        match self {
            CounterId::Kills => m.kills.get(),
            CounterId::RoomsCleared => m.rooms_cleared.get(),
            CounterId::OrbsUsed => m.orbs_used.get(),
            CounterId::CardsFed => m.cards_fed.get(),
            CounterId::ExemplarsFed => m.exemplars_fed.get(),
            CounterId::ItemsCrafted => m.items_crafted.get(),
            CounterId::ItemsListed => m.items_listed.get(),
            CounterId::ItemsSold => m.items_sold.get(),
            CounterId::GilSpent => m.gil_spent.get(),
            CounterId::SoulNodesAllocated => m.soul_nodes_allocated.get(),
            CounterId::BossKills => m.boss_kills.get(),
            CounterId::BandsCleared => m.bands_cleared.get(),
            CounterId::DamageDealt => m.damage_dealt.get(),
            CounterId::DamageTaken => m.damage_taken.get(),
            CounterId::HealingDone => m.healing_done.get(),
            CounterId::SkillsCast => m.skills_cast.get(),
            CounterId::LevelsGained => m.levels_gained.get(),
            CounterId::Deaths => m.deaths.get(),
            CounterId::Fusions => m.fusions.get(),
            CounterId::TrainingLevels => m.training_levels.get(),
            CounterId::ItemsDisenchanted => m.items_disenchanted.get(),
            CounterId::CrestLevels => m.crest_levels.get(),
            CounterId::GemFusions => m.gem_fusions.get(),
        }
    }
}

/// Points every commission is worth. Uniform, so "five of eight" is a count and not a puzzle.
pub const POINTS_PER_COMMISSION: u32 = 100;

/// The board: the SAME EIGHT, every day, for everyone (user 2026-08-12).
///
/// CODE, not a database row. It was season config while the deal was a per-account draw; a fixed
/// board is a constant, and storing a constant per player per day was 56% of every row.
///
/// Deliberately not rotating. A changing set has to be re-learned each morning and makes planning
/// impossible — "do I have the ore for today's refine?" has no answer the night before. A fixed
/// board is a routine, and a routine is what a daily is for.
pub fn board() -> Vec<DailyDef> {
    use CounterId::*;
    use DailyKind::*;
    // ⚠️ THE BIT IS WRITTEN OUT PER COMMISSION, never derived from where it sits.
    //
    // The first version of this "stable bit" fix took the bit from a counter incremented once per
    // constructor call — which IS the position, spelled differently. Reordering the list below
    // would have reordered the bits with it, so the whole fix was a fiction and its test could not
    // fail. Typing the number is what makes it stable.
    //
    // The numbers match the ORIGINAL positions, so no stored row changes meaning. Removing a
    // commission leaves its number retired; the next new one takes an unused number, never a
    // recycled one, or a row from before the removal would light the wrong box.
    let d = |id: &str, bit: u8, counter, target, kind| DailyDef {
        id: id.into(),
        bit,
        counter,
        target,
        kind,
        points: POINTS_PER_COMMISSION,
    };
    vec![

        // THE EIGHT. Fixed, same every day, in display order (user 2026-08-12).
        //
        // AUTO (3) — complete by leaving the game running. Worth 300 of the 500 needed, so
        // idling is most of a day but never all of it: at least one deliberate act is
        // required, which is the difference between a daily and a payout for being logged in.
        // Targets are calibrated against the MEASURED 1,078 kills/hour.
        d("kills_2500", 0, Kills, 2_500, Auto),          // ~2.3 h
        d("rooms_10", 1, RoomsCleared, 10, Auto),
        d("damage_1m", 2, DamageDealt, 1_000_000, Auto),
        // ACTION (5) — one deliberate press each. Any two finish the day.
        //
        // `fuse_3` is the SAFETY VALVE and must stay in the set: gear accrues constantly, so it
        // is the one action that never depends on holding a consumable. A Soul node needs Soul —
        // a board where every action can be blocked by an empty bag is a board that can be
        // impossible on a bad day.
        //
        // RETIRED BITS, never recycled (see the comment above `d`): 3 `sell_10` (vendoring is
        // gone, sim 0.0.411), 4 `refine_1` (refine, 0.0.370), 5 `craft_1` (craft materials left
        // the drop tables, catalog 0.0.163), 8 `card_feed_1` (cards held back from the first patch,
        // 2026-09-30). The replacements took fresh numbers.
        d("fuse_3", 9, Fusions, 3, Action),
        // Every accepted FuseGems counts, a failed one too, so luck cannot block it.
        d("gem_fuse_1", 11, GemFusions, 1, Action),
        d("train_1", 10, TrainingLevels, 1, Action),
        // ×100 with the Gil rescale (sim 0.0.412) and then some: a Training level is the
        // cheapest thing Gil buys and starts at 5,000.
        d("spend_gil", 6, GilSpent, 10_000, Action),
        d("soul_node_1", 7, SoulNodesAllocated, 1, Action),
    ]
}

/// The point ladder. Reaching a rung pays its crystal, once per day.
pub fn ladder() -> Vec<Milestone> {
    vec![

        Milestone { points: 100, crystal: 25 },
        Milestone { points: 200, crystal: 30 },
        Milestone { points: 300, crystal: 35 },
        Milestone { points: 400, crystal: 45 },
        Milestone { points: 500, crystal: 65 },
    ]
}

/// At most this many objectives a day — `done_mask` is one bit each and lives in a Postgres
/// SMALLINT, so the ceiling is a storage fact, not a taste one.
pub const MAX_DAILIES: usize = 12;


/// Offset from UTC to the moment a new commission day begins: 04:00 Thailand (ICT, UTC+7), which
/// is 21:00 UTC the previous evening.
///
/// 04:00 local rather than local midnight, and the reason is the audience: a reset at midnight
/// lands in the middle of the evening play session for anyone still up, and the day flips under
/// them mid-run. Four in the morning is after almost everyone has stopped and before anyone has
/// started. Every HoYo title resets at 04:00 for exactly this, and the player base already has
/// the habit.
///
/// One fixed offset, NOT the player's own timezone: a per-account boundary means storing it,
/// migrating it when they travel, and defending it from being edited to farm two days in one.
const DAY_OFFSET_MS: i64 = 21 * 3_600_000;

/// Index of the commission day a server timestamp falls in. Server-side only — a client's clock
/// never decides when its day rolls over.
pub fn day_of_ms(now_ms: u64) -> i64 {
    // Floor-divide so timestamps before the first boundary land on the day BEFORE, not day 0.
    // Rust's `/` truncates toward zero, which would fold the negatives onto the wrong side.
    (now_ms as i64 - DAY_OFFSET_MS).div_euclid(86_400_000)
}

/// The instant the day AFTER `day` begins, in epoch ms. The client renders a countdown from
/// this rather than computing the boundary itself — one clock decides when a day rolls over.
pub fn day_end_ms(day: i64) -> i64 {
    (day + 1) * 86_400_000 + DAY_OFFSET_MS
}

// ---------------------------------------------------------------- accrual

/// One account's commission day, as stored.
pub struct DayState<'a> {
    /// One bit per commission, in board order.
    pub done_mask: u16,
    /// One bit per LADDER RUNG already paid. Its own field, not spare bits of `done_mask`: two
    /// pay-once ledgers in one integer is how one of them eventually pays twice.
    pub milestone_mask: u16,
    /// Each commission's counter AS IT STOOD when the day opened, in board order. Progress is
    /// today's delta against this, which is why a lifetime total cannot satisfy a commission.
    ///
    /// One number per commission rather than a whole `Measures` snapshot. Smaller (46 bytes
    /// against 185, and fixed however many counters the sim grows), and it is the same shape
    /// quests and weekly objectives need — they are accepted at arbitrary moments and so have no
    /// shared instant to snapshot. Two mechanisms for one job is one too many.
    ///
    /// It also cuts the last tie to `bp_state`: the day no longer needs the account's last-save
    /// snapshot as a seed, so `daily` depends on nothing outside itself.
    pub start_values: &'a [u64],
}

pub struct Accrued {
    pub done_mask: u16,
    /// Rungs REACHED. Not rungs paid — `claimed_mask` is a separate column that only
    /// `/daily/claim` writes. They were one field once, and the result was that accrual marked
    /// every rung "paid" on the way past, so a claim would have handed over nothing. Two
    /// pay-once ledgers in one integer is the hazard this field's own comment warned about.
    pub milestone_mask: u16,
    /// The day's points, clamped to the top rung, for display.
    pub points: u32,
}

/// The start values a NEW day opens on: every commission's counter, right now.
///
/// Captured at the moment the day's row is created, so the day genuinely starts at zero — nothing
/// earned before it counts toward it (user 2026-08-12). The alternative, seeding from the last
/// accepted save, quietly carried up to five minutes of pre-boundary play into the new day.
/// Sentinel for "this bit had no opening reading" — a commission the board did not contain when
/// the day opened. Not zero: zero is a legitimate counter value, and reading a hole as zero makes
/// `progress` the player's LIFETIME total, which completes the new commission instantly and pays a
/// rung for nothing. No play counter can reach `u64::MAX`.
pub const NO_START: u64 = u64::MAX;

/// Indexed BY BIT, not by board position, so a row keeps meaning the same thing when the board is
/// reordered. Sized to the highest bit in the board, with `NO_START` in any gap.
///
/// For the current board — contiguous bits 0..7 — this is byte-identical to what the old
/// position-indexed version wrote, so no stored row changes meaning.
pub fn open_day(board: &[DailyDef], now: &Measures) -> Vec<u64> {
    let len = board.iter().map(|d| d.bit as usize + 1).max().unwrap_or(0).min(MAX_DAILIES);
    let mut sv = vec![NO_START; len];
    for d in board.iter() {
        if let Some(cell) = sv.get_mut(d.bit as usize) {
            *cell = d.counter.read(now);
        }
    }
    sv
}

/// Advance a commission day against fresh counters. Pure.
pub fn accrue(board: &[DailyDef], ladder: &[Milestone], now: &Measures, st: &DayState) -> Accrued {
    let mut mask = st.done_mask;
    for d in board.iter() {
        // Out-of-range bits are skipped rather than shifted: `1u16 << 16` is a panic, and the mask
        // is a Postgres SMALLINT whose top bit is the sign.
        if d.bit as usize >= MAX_DAILIES {
            continue;
        }
        let bit = 1u16 << d.bit;
        if mask & bit != 0 {
            continue;
        }
        // saturating: an older save accepted after a newer one must contribute zero, never
        // negative progress.
        // NO START VALUE, NO COMMISSION. This happens when the board GROWS mid-day: the row was
        // written with eight values and now the board asks for nine, so the ninth has no opening
        // reading. Skipping it means the new commission simply cannot be finished until tomorrow.
        //
        // Defaulting to 0 instead would make `progress` the player's LIFETIME total, which
        // completes the new commission instantly and pays a ladder rung for nothing. An earlier
        // version did exactly that, with a comment claiming the opposite.
        // NO OPENING READING, NO COMMISSION — whether the row is simply too short (written before
        // this bit existed) or carries the `NO_START` hole. Either way the commission cannot be
        // finished today. Treating it as zero would make `progress` the LIFETIME total, complete
        // it instantly and pay a rung for nothing; an earlier version did exactly that.
        let Some(start) = st.start_values.get(d.bit as usize).copied() else { continue };
        if start == NO_START {
            continue;
        }
        let progress = d.counter.read(now).saturating_sub(start);
        if progress >= d.target {
            mask |= bit;
        }
    }
    // Points are RECOMPUTED from the full mask, never accumulated. This pass can run twice on one
    // save — accrual sits outside the save transaction and self-heals by re-running — and a
    // running total would drift every retry. Derived from state, it cannot.
    let points = points_of(board, ladder, mask);
    // Reached is DERIVED from points every pass, never accumulated — same reason as the points
    // above: this can run twice on one save and a sticky bit would outlive a correction.
    let ms_mask = reached_mask(ladder, points);
    Accrued { done_mask: mask, milestone_mask: ms_mask, points }
}

/// The day's points for a `done_mask`, clamped to the top rung — the ONE conversion.
///
/// It existed three times: `accrue` clamped, `/daily/state` did not, and `/daily/claim` had its
/// own copy, so the two endpoints could report different numbers for the same day. All three were
/// also keyed by the commission's POSITION rather than its stable `bit`, which is the defect
/// `DailyDef::bit` was added to close — fixing it in `accrue` alone left the readers behind.
pub fn points_of(board: &[DailyDef], ladder: &[Milestone], done_mask: u16) -> u32 {
    let raw: u32 = board
        .iter()
        .filter(|d| (d.bit as usize) < MAX_DAILIES && done_mask & (1u16 << d.bit) != 0)
        .map(|d| d.points)
        .sum();
    let top = ladder.last().map(|m| m.points).unwrap_or(raw);
    raw.min(top)
}

/// Which rungs `points` has reached. Pure and derived — a rung is reached or it is not, and
/// that answer never depends on what was reached before.
pub fn reached_mask(ladder: &[Milestone], points: u32) -> u16 {
    let mut m = 0u16;
    for (i, rung) in ladder.iter().enumerate().take(16) {
        if points >= rung.points {
            m |= 1u16 << i;
        }
    }
    m
}

/// What one press of Claim hands over: the rungs reached but not yet paid, and their crystal.
///
/// Recomputed from `points` rather than read off a stored "unpaid" tally, so a row written by an
/// older build — or by an accrual that ran twice — still pays exactly the rungs it owes.
pub fn claimable(ladder: &[Milestone], points: u32, claimed_mask: u16) -> (u16, u64) {
    let pay = reached_mask(ladder, points) & !claimed_mask;
    let crystal = ladder
        .iter()
        .enumerate()
        .take(16)
        .filter(|(i, _)| pay & (1u16 << i) != 0)
        .map(|(_, m)| m.crystal as u64)
        .sum();
    (pay, crystal)
}

/// Pass XP paid per ladder rung claimed (user 2026-08-12). The pass earns nothing on its own;
/// every point it has comes from a claimed commission rung.
pub const XP_PER_RUNG: u64 = 20;

// ---------------------------------------------------------------- tests

#[cfg(test)]
mod tests {
    use super::*;

    /// Advance whichever counter a commission reads. Exhaustive on purpose: adding a `CounterId`
    /// without teaching the tests to move it leaves the new commission untested while every
    /// existing test still passes.
    fn bump(m: &mut Measures, c: CounterId, n: u64) {
        match c {
            CounterId::Kills => m.kills.add(n),
            CounterId::RoomsCleared => m.rooms_cleared.add(n),
            CounterId::OrbsUsed => m.orbs_used.add(n),
            CounterId::CardsFed => m.cards_fed.add(n),
            CounterId::ExemplarsFed => m.exemplars_fed.add(n),
            CounterId::ItemsCrafted => m.items_crafted.add(n),
            CounterId::ItemsListed => m.items_listed.add(n),
            CounterId::BossKills => m.boss_kills.add(n),
            CounterId::BandsCleared => m.bands_cleared.add(n),
            CounterId::DamageDealt => m.damage_dealt.add(n),
            CounterId::DamageTaken => m.damage_taken.add(n),
            CounterId::HealingDone => m.healing_done.add(n),
            CounterId::SkillsCast => m.skills_cast.add(n),
            CounterId::LevelsGained => m.levels_gained.add(n),
            CounterId::Deaths => m.deaths.add(n),
            CounterId::ItemsSold => m.items_sold.add(n),
            CounterId::GilSpent => m.gil_spent.add(n),
            CounterId::SoulNodesAllocated => m.soul_nodes_allocated.add(n),
            CounterId::Fusions => m.fusions.add(n),
            CounterId::TrainingLevels => m.training_levels.add(n),
            CounterId::ItemsDisenchanted => m.items_disenchanted.add(n),
            CounterId::CrestLevels => m.crest_levels.add(n),
            CounterId::GemFusions => m.gem_fusions.add(n),
        }
    }

    fn run(board: &[DailyDef], now: &Measures, st: &DayState) -> Accrued {
        accrue(board, &ladder(), now, st)
    }
    fn fresh(sv: &[u64]) -> DayState<'_> {
        DayState { done_mask: 0, milestone_mask: 0, start_values: sv }
    }

    #[test]
    fn the_board_offers_eight_and_asks_for_five() {
        let b = board();
        assert_eq!(b.len(), 8, "the board should hold eight commissions");
        let per = b[0].points;
        assert!(b.iter().all(|d| d.points == per), "mixed point values break the 5-of-8 shape");
        let top = ladder().last().unwrap().points;
        assert_eq!(top.div_ceil(per), 5, "the top rung should need five commissions");
        assert!(b.len() as u32 * per > top, "no surplus left to skip with");
    }

    /// Every commission reads a DIFFERENT counter, so no single activity ticks two boxes.
    ///
    /// The one intended overlap sits outside this rule: a Training level costs Gil, so `train_1`
    /// also moves `spend_gil`. That is an action paired with its COST, not two commissions
    /// counting the same event.
    #[test]
    fn every_commission_reads_its_own_counter() {
        let mut seen = Vec::new();
        for d in board() {
            assert!(!seen.contains(&d.counter), "{:?} appears twice ({})", d.counter, d.id);
            seen.push(d.counter);
        }
    }

    /// IDLING IS NOT ENOUGH — at least one deliberate act is required.
    ///
    /// The line between a daily and a payout for being logged in, and the arithmetic enforces it
    /// rather than a rule: three auto commissions at 100 against a 500 top rung. Cheap to break
    /// by accident — add a fourth auto, or drop the top rung to 400 — and nothing else notices.
    #[test]
    fn idling_alone_cannot_finish_the_day() {
        let b = board();
        let z = Measures::default();
        let mut now = z.clone();
        for d in b.iter().filter(|d| d.kind == DailyKind::Auto) {
            bump(&mut now, d.counter, d.target);
        }
        let out = run(&b, &now, &fresh(&open_day(&b, &z)));
        let top = ladder().last().unwrap().points;
        assert!(out.points < top, "idling alone reached the top rung");
        assert!(out.points * 2 >= top, "idling earned only {} of {top}", out.points);
    }

    /// A commission added to the board mid-day cannot be completed that day, and above all does
    /// not pay out instantly off the player's lifetime total.
    ///
    /// The row stores one opening value per commission. Grow the board and the newest one has no
    /// opening reading — the fallback for that decides whether the player gets a free rung.
    #[test]
    fn a_commission_with_no_opening_value_is_not_free() {
        let mut b = board();
        let z = Measures::default();
        let sv = open_day(&b, &z); // opened while the board held its current size
        // Now the board grows, and the player has a large lifetime total on the new counter.
        // The next FREE bit, not `len()`: bits are never recycled, so a retired commission (bit 4,
        // the old `refine_1`) leaves the two numbers apart — and reusing a live bit here would
        // make this test read another commission's progress instead of the new one's.
        let added = b.iter().map(|d| d.bit).max().unwrap_or(0) + 1;
        b.push(DailyDef {
            id: "added_midday".into(),
            bit: added,
            counter: CounterId::Kills,
            target: 10,
            kind: DailyKind::Auto,
            points: POINTS_PER_COMMISSION,
        });
        let now = Measures { kills: Counter::seeded_for_test(9_999_999), ..Default::default() };
        let out = accrue(&b, &ladder(), &now, &fresh(&sv));
        let added_bit = 1u16 << added;
        assert_eq!(out.done_mask & added_bit, 0, "a mid-day commission paid off a lifetime total");
    }

    /// Progress is TODAY's delta. A lifetime total cannot satisfy a commission.
    #[test]
    fn lifetime_totals_do_not_satisfy_commissions() {
        let b = board();
        let day_start = Measures { kills: Counter::seeded_for_test(1_000_000), ..Default::default() };
        let out = run(&b, &day_start, &fresh(&open_day(&b, &day_start)));
        assert_eq!(out.done_mask, 0, "a lifetime total completed a commission");
        assert_eq!(claimable(&ladder(), out.points, 0).1, 0, "and it owed crystal for it");
    }

    /// Completing the board pays every rung exactly once, and a second pass pays nothing.
    #[test]
    fn a_full_board_pays_each_rung_once_and_only_once() {
        let b = board();
        let z = Measures::default();
        let mut now = z.clone();
        for d in &b {
            bump(&mut now, d.counter, d.target);
        }
        let sv = open_day(&b, &z);
        let out = run(&b, &now, &fresh(&sv));
        assert_eq!(out.done_mask.count_ones(), 8);
        let all: u64 = ladder().iter().map(|m| m.crystal as u64).sum();
        // What ONE press of Claim owes on a fresh day — the path that actually pays, not the
        // accrual's opinion of it. Accrual deliberately pays nothing now.
        let (pay, crystal) = claimable(&ladder(), out.points, 0);
        assert_eq!(crystal, all, "a full sweep should collect the whole ladder");
        assert_eq!(pay.count_ones() as usize, ladder().len(), "not every rung was owed");
        assert_eq!(out.points, ladder().last().unwrap().points, "points overflowed the ladder");
        // Claim again with those rungs recorded: a second press owes nothing.
        assert_eq!(claimable(&ladder(), out.points, pay), (0, 0), "GEMS PAID TWICE on re-claim");
        // Re-accrue on the SAME save. Accrual runs outside the save transaction and self-heals by
        // re-running, so anything that pays twice here pays twice in production.
        let again = accrue(
            &b,
            &ladder(),
            &now,
            &DayState { done_mask: out.done_mask, milestone_mask: out.milestone_mask, start_values: &sv },
        );
        assert_eq!(again.done_mask, out.done_mask, "commissions re-completed");
        assert_eq!(again.milestone_mask, out.milestone_mask, "reached-rungs drifted on re-accrue");
    }

    /// ONE press pays every rung reached so far, not just the newest (user 2026-08-12).
    ///
    /// The case that matters is a player who finishes three commissions before opening the
    /// screen: paying only rung 3 would quietly swallow rungs 1 and 2, and nothing in the UI
    /// would say so.
    #[test]
    fn one_press_pays_every_rung_reached_so_far() {
        let l = ladder();
        let (pay, crystal) = claimable(&l, 300, 0);
        assert_eq!(pay.count_ones(), 3, "a 300-point day owes three rungs, not one");
        let want: u64 = l.iter().take(3).map(|m| m.crystal as u64).sum();
        assert_eq!(crystal, want);
        // Then nothing is owed until more points arrive.
        assert_eq!(claimable(&l, 300, pay), (0, 0), "a second press paid again");
        // More points later owe only the NEW rungs.
        let (pay2, crystal2) = claimable(&l, 500, pay);
        assert_eq!(pay2.count_ones(), 2, "the later press should owe exactly rungs 4 and 5");
        assert_eq!(crystal2, l[3].crystal as u64 + l[4].crystal as u64);
    }

    /// Points below the first rung owe nothing — the floor case, which an off-by-one on `>=`
    /// would turn into a free 25 crystal every day for logging in.
    #[test]
    fn a_day_under_the_first_rung_owes_nothing() {
        let l = ladder();
        assert_eq!(claimable(&l, 0, 0), (0, 0));
        assert_eq!(claimable(&l, l[0].points - 1, 0), (0, 0));
        assert_eq!(claimable(&l, l[0].points, 0).1, l[0].crystal as u64, "the first rung must pay AT its threshold");
    }

    /// The ladder pays its SHARE of the free-crystal budget, and the rest stays reserved.
    ///
    /// Two numbers set in different places that must agree, so they are asserted together. The
    /// reserve is the point: a login calendar, mail gifts and event shops are still to come, and
    /// if the first source built takes the whole faucet, every later one is either free inflation
    /// or an argument about clawing budget back.
    #[test]
    fn the_ladder_stays_inside_its_share() {
        let l = ladder();
        let total: u32 = l.iter().map(|m| m.crystal).sum();
        assert_eq!(total, DAILY_GEM_SHARE, "the ladder drifted from its allotted share");
        assert!(DAILY_GEM_SHARE < FREE_GEM_BUDGET_PER_DAY, "the daily swallowed the whole budget");
        // Bounded from BOTH sides: too flat and finishing earns nothing extra, too steep and the
        // early rungs are a toll gate on the last.
        let last = l.last().unwrap().crystal;
        let biggest_other = l[..l.len() - 1].iter().map(|m| m.crystal).max().unwrap_or(0);
        assert!(last > biggest_other, "not back-loaded; finishing earns nothing extra");
        assert!(last * 100 <= total * 40, "the last rung is {}% — a cliff", last * 100 / total);
    }

    /// A REORDERED BOARD MUST NOT RE-POINT AN IN-FLIGHT ROW. The bit belongs to the commission,
    /// not to where it sits, so swapping two entries — a cosmetic edit no test used to reject —
    /// leaves every stored `done_mask` and `start_values` meaning what it meant.
    #[test]
    fn reordering_the_board_does_not_move_a_stored_rows_meaning() {
        let b = board();
        let z = Measures::default();
        let sv = open_day(&b, &z);
        // Finish exactly the kills commission.
        let mut now = z.clone();
        let kills = b.iter().find(|d| d.counter == CounterId::Kills).expect("kills commission");
        bump(&mut now, CounterId::Kills, kills.target);
        let before = run(&b, &now, &fresh(&sv));
        let kills_bit = 1u16 << kills.bit;
        assert_eq!(before.done_mask, kills_bit, "the fixture did not finish exactly one commission");

        // Now REORDER the board — same eight, different order — and re-accrue the SAME row.
        let mut shuffled = b.clone();
        shuffled.reverse();
        let after = accrue(&shuffled, &ladder(), &now, &DayState {
            done_mask: before.done_mask, milestone_mask: 0, start_values: &sv,
        });
        assert_eq!(after.done_mask, before.done_mask,
            "reordering the board re-pointed a stored row at a different counter");
        assert_eq!(after.points, before.points);
    }

    /// The bit must be WRITTEN OUT, not derived from position — otherwise the "stable bit" is the
    /// position wearing a different name and reordering still re-points every stored row.
    ///
    /// The first version of that fix assigned the bit from a counter incremented per constructor
    /// call, so `reordering_the_board_does_not_move_a_stored_rows_meaning` could not fail: the
    /// bits reordered with the list. This checks the property that test assumed.
    #[test]
    fn a_commissions_bit_does_not_follow_its_position() {
        // PINNED TO THE ID. Reversing an already-built Vec proves nothing — the structs carry
        // their bits — so the expectation has to live outside `board()` entirely. Reorder the
        // constructor calls under a positional scheme and these pairs stop matching.
        const PINNED: &[(&str, u8)] = &[
            ("kills_2500", 0), ("rooms_10", 1), ("damage_1m", 2), ("fuse_3", 9),
            ("train_1", 10), ("spend_gil", 6), ("soul_node_1", 7), ("gem_fuse_1", 11),
        ];
        // Retired bits stay retired: 3 sell_10, 4 refine_1, 5 craft_1, 8 card_feed_1.
        for retired in [3u8, 4, 5, 8] {
            assert!(board().iter().all(|d| d.bit != retired), "bit {retired} was recycled");
        }
        let b = board();
        for (id, bit) in PINNED {
            let d = b.iter().find(|d| d.id == *id)
                .unwrap_or_else(|| panic!("{id} left the board — retire its bit, do not reuse it"));
            assert_eq!(d.bit, *bit, "{id}'s bit moved; every stored row now points elsewhere");
        }
        assert_eq!(b.len(), PINNED.len(), "the board changed size without updating the pins");
        // And every bit is distinct and inside the stored mask.
        let mut bits: Vec<u8> = b.iter().map(|d| d.bit).collect();
        bits.sort_unstable();
        bits.dedup();
        assert_eq!(bits.len(), b.len(), "two commissions share a bit");
        assert!(b.iter().all(|d| (d.bit as usize) < MAX_DAILIES), "a bit overflows the mask");
    }

    /// The points conversion is ONE function, and it clamps. It used to exist three times —
    /// `accrue` clamped, `/daily/state` did not — so two server paths could report different
    /// numbers for the same day.
    #[test]
    fn points_are_one_clamped_conversion() {
        let b = board();
        let l = ladder();
        let all: u16 = b.iter().map(|d| 1u16 << d.bit).fold(0, |a, x| a | x);
        let top = l.last().map(|m| m.points).unwrap_or(0);
        assert_eq!(points_of(&b, &l, all), top, "a full board must clamp to the top rung");
        assert_eq!(points_of(&b, &l, 0), 0);
        // And it agrees with what `accrue` reports for the same mask.
        let z = Measures::default();
        let mut now = z.clone();
        for d in &b { bump(&mut now, d.counter, d.target); }
        let out = run(&b, &now, &fresh(&open_day(&b, &z)));
        assert_eq!(out.points, points_of(&b, &l, out.done_mask));
    }

    /// A new commission day starts at 04:00 Thailand, not at UTC midnight.
    #[test]
    fn the_day_turns_over_at_0400_thailand() {
        let t: u64 = 1_786_000_000_000;
        let boundary = t - (t - 21 * 3_600_000) % 86_400_000 + 86_400_000;
        assert_eq!(day_of_ms(boundary - 1) + 1, day_of_ms(boundary), "index must advance there");
        assert_eq!(day_of_ms(boundary), day_of_ms(boundary + 86_399_999), "a day is 24 h");
        assert_eq!((boundary / 3_600_000) % 24, 21, "the day does not turn at 21:00 UTC");
    }
}


