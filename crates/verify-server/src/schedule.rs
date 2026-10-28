//! SCHEDULE + CLAIM — "a reward, with a window, claimable once".
//!
//! One machine for the 7-day newbie ladder, login events, and later the availability of packs.
//! They were separate features in the plan and are one shape in code: a number that only rises, a
//! list of thresholds, and a record of what has already been paid.
//!
//! `daily::claimable` ALREADY IS that function — points against ladder rungs against a paid mask.
//! Everything here is the same idea with the source of the number and the payout pulled out into
//! data, which is what §10.2 of the gacha review asks for: server-side DATA, not a hardcoded
//! handler per feature. Nothing below is a second mechanism; if the two ever disagree about what
//! "claimed once" means, one of them is wrong.
//!
//! THE DEPENDENCY RUNS ONE WAY, as it does for `daily`: nothing here knows what a season is, what
//! a save is, or what a database looks like. Pure functions over (config, position, mask), so the
//! whole machine is testable without Postgres and without a clock.
//!
//! **What advances a track is a VERIFIED SAVE, not a request.** A commission day opens exactly
//! once per account inside the post-verify transaction, so "logged in today" means the account
//! played and produced a save the server accepted. An endpoint anyone can call would have meant a
//! login ladder farmable by a curl loop.
//!
//! LOGIN IS THE ONLY REQUIREMENT (user 2026-08-17). No counters, no quests — a track advances by
//! one per day played and by nothing else, which is why there is one clock here and not two.

use serde::{Deserialize, Serialize};

use crate::daily::{DAILY_GEM_SHARE, FREE_GEM_BUDGET_PER_DAY};

/// Free crystal per day that the Daily Commission ladder deliberately does not take.
///
/// Derived rather than typed, so the reserve cannot silently drift when either half moves.
/// `daily`'s own test explains what it is for: "a login calendar, mail gifts and event shops are
/// still to come, and if the first source built takes the whole faucet, every later one is either
/// free inflation or an argument about clawing budget back."
pub const RESERVE_GEM_BUDGET_PER_DAY: u32 = FREE_GEM_BUDGET_PER_DAY - DAILY_GEM_SHARE;

/// `claimed_mask` is one bit per entry in a Postgres BIGINT, so the ceiling is a storage fact
/// rather than a taste one — the same reasoning as `daily::MAX_DAILIES`. 63, not 64: the column is
/// SIGNED, and bit 63 is its sign bit.
pub const MAX_ENTRY_ID: u8 = 62;

/// ONBOARDING is budgeted separately from the daily faucet (user 2026-08-20).
///
/// `RESERVE_GEM_BUDGET_PER_DAY` governs what runs FOREVER: a faucet that pays every player every
/// day, where an extra 50 a day is an extra 18,000 a year. A once-per-account welcome is a
/// different quantity that happens to share a unit — it is paid once, to one cohort, and never
/// again, so charging it against a recurring rate would price a single 3,000-crystal gift as if it
/// arrived every day for the life of the game.
///
/// Separate does NOT mean unbounded, which is the whole reason this is a number and not an
/// exemption: `the_onboarding_grant_stays_inside_its_own_budget` holds the total, and a second
/// once-per-account ladder has to fit beside the first.
pub const ONBOARDING_GEM_BUDGET: u32 = 3_500;

/// Does this track pay a ONCE-PER-ACCOUNT welcome rather than a recurring faucet?
///
/// `SinceStart` is the marker: its window restarts for every new account and never repeats for the
/// same one. `login7` is windowless and also once-per-account in practice, but it is a daily drip
/// by shape — it pays on days played — so it stays on the recurring budget where it has always
/// been scored.
pub fn is_onboarding(t: &TrackDef) -> bool {
    matches!(t.window, Some(Window::SinceStart { .. }))
}

/// Days a closed track's rows are kept before the retirement sweep deletes them.
///
/// Not zero, because a claim can be in flight across the boundary and because a player who
/// disputes a missing reward needs the row to still exist while the answer is looked up.
pub const RETIRE_GRACE_DAYS: i64 = 14;

/// When a track is open. Two clocks, because two kinds of event need different ones.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Window {
    /// The SAME dates for everyone — a seasonal event. Inclusive at both ends.
    ///
    /// `opens_day` may be in the FUTURE: config is deployed ahead of time and the window opens
    /// itself, so starting an event needs no deploy at the minute it begins.
    Absolute { opens_day: i64, closes_day: i64 },
    /// `days` from THIS ACCOUNT's first day on the track — a newcomer offer, which has to be the
    /// same length for someone starting today as for someone who started a year ago. An absolute
    /// window cannot express that: it would be a race everyone joins at a different point.
    ///
    /// The clock starts on the account's first accepted save, which is when its row is created —
    /// so "90 days from creating the id" needs no separate account-creation lookup, and a player
    /// who makes an account and never plays has not started their week.
    SinceStart { days: i64 },
}

impl Window {
    /// How many days one ACCOUNT gets. For `Absolute` that is also the calendar length; for
    /// `SinceStart` the two differ, which is the point.
    pub fn days(&self) -> i64 {
        match self {
            Window::Absolute { opens_day, closes_day } => closes_day - opens_day + 1,
            Window::SinceStart { days } => *days,
        }
    }
}

/// One thing a cell hands over. A cell may carry several.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Grant {
    /// Paid to the BOUND bucket. Only crystal bought with real money are paid crystal.
    Crystal(u32),
    /// A MINT into the money supply. See `no_windowless_track_hands_out_gil`.
    Gil(u64),
    Item { def_id: String, qty: u32 },
    /// Credits a `bound_items` COUNTER — a gacha coupon today, an EXP ticket later.
    ///
    /// NOT `Item`. A coupon delivered as an inventory item lands in the bag, where the pull path
    /// cannot see it: `gacha_roll` reads the counter, so a ladder paying items minted a coupon
    /// nobody could spend while the counter it actually needs stayed at zero. Two assets with one
    /// name. This is the one that pays for pulls.
    Bound { kind: String, qty: u32 },
}

/// One cell: claimable once the track's position reaches `at`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Entry {
    /// The bit this cell owns in `claimed_mask`. STABLE and author-assigned — NOT the position in
    /// `entries`.
    ///
    /// An index-keyed mask shifts every bit when a cell is inserted or removed, so editing a live
    /// event would silently re-pay or swallow cells for everyone mid-run. `daily` hit the same
    /// hazard when its board grew mid-day. With a stable id, reordering is free and deleting a
    /// cell just leaves a dead bit.
    pub id: u8,
    /// Days played, inside this track, at which the cell unlocks.
    pub at: u32,
    pub reward: Vec<Grant>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TrackDef {
    /// STABLE and never reused. A retired track's rows are deleted, so an id that comes back is
    /// an id every account can claim from scratch — put the date in it (`login_2026_09`) and
    /// reuse becomes a mistake you can see.
    pub id: String,
    /// `None` = never expires; it runs until the player has claimed every cell. `Some` = the
    /// unclaimed cells are forfeited when the window closes.
    pub window: Option<Window>,
    pub entries: Vec<Entry>,
}

impl TrackDef {
    /// Open on `day` for an account whose row opened on `opened_day`, and so eligible to advance
    /// and to be claimed against.
    ///
    /// `opened_day` is `None` when the account has no row yet — it has never played inside this
    /// track. A `SinceStart` track is OPEN for such an account: its clock has not started, so
    /// there is nothing to have expired. Getting that backwards would mean a newcomer offer that
    /// every newcomer is already too late for.
    pub fn is_active(&self, day: i64, opened_day: Option<i64>) -> bool {
        match self.window {
            None => true,
            Some(Window::Absolute { opens_day, closes_day }) => day >= opens_day && day <= closes_day,
            Some(Window::SinceStart { days }) => match opened_day {
                None => true,
                Some(start) => day < start + days,
            },
        }
    }

    /// Past its window by the full grace period, for EVERY account at once — so its rows and its
    /// config may both go.
    ///
    /// Only an `Absolute` track can answer this from config: a `SinceStart` track expires at a
    /// different moment for every account, so it is never globally retired and its rows are swept
    /// per-row by `opened_day` instead. A track with NO window is never retired either — deleting
    /// one of its rows is a double-payout rather than a cleanup, because a deleted row and a row
    /// that never existed are the same thing to the advance.
    pub fn is_retired(&self, day: i64) -> bool {
        match self.window {
            Some(Window::Absolute { closes_day, .. }) => day > closes_day + RETIRE_GRACE_DAYS,
            None | Some(Window::SinceStart { .. }) => false,
        }
    }

    /// The day an account's row for this track stops being sweepable, given when it opened.
    /// `None` when the track is not per-account expiring.
    ///
    /// ⚠️ SWEEPING A `SinceStart` ROW IS NOT SAFE ON ITS OWN, and this returning a day is not
    /// permission to do it. `is_active(day, None)` is TRUE for a `SinceStart` track — it has to
    /// be, or a newcomer would be too late for their own newcomer offer — so `advance_tracks`
    /// re-INSERTs any missing active track at `position 1, claimed_mask 0, opened_day = today`.
    /// Delete a finished row and the next accepted save hands the account a fresh ninety days and
    /// an empty mask, and the whole ladder pays again. Forever, every sweep interval.
    ///
    /// That is the same hazard `a_windowless_track_is_never_retired` exists for: a deleted row and
    /// a row that never existed are indistinguishable to the advance. A sweep therefore needs a
    /// TOMBSTONE — something that survives the delete and says "this account already had its
    /// run" — and there is none today, so nothing calls this. Kept, with the reason, because the
    /// obvious next step is the dangerous one.
    pub fn row_expires_on(&self, opened_day: i64) -> Option<i64> {
        match self.window {
            Some(Window::SinceStart { days }) => Some(opened_day + days + RETIRE_GRACE_DAYS),
            _ => None,
        }
    }

    /// Crystal this track pays per day of its run. The budget is a RATE, and a track advances by at
    /// most one cell per day, so total-over-span is what an account can actually draw from it.
    ///
    /// Counts GEMS and anything with a posted crystal PRICE. A coupon is the second kind: it is the
    /// pull currency and `COUPON_PRICE_GEMS` says exactly what one is worth, so a ladder paying
    /// coupons is handing out free crystal in all but name.
    ///
    /// It used to score every non-`Crystal` grant at 0, which was defensible while the alternative
    /// was items with no crystal price — and stopped being defensible the moment `Grant::Bound` paid
    /// coupons. `newcomer_gacha` pays ten of them, 3,000 crystal of pull value, and the budget check
    /// scored the whole track at zero.
    ///
    /// ⚠️ STILL BLIND to `Grant::Item`, which genuinely has no crystal price. That is a stated limit,
    /// not a hidden one: `every_track_declares_what_it_pays` is where it is recorded.
    pub fn crystal_rate(&self) -> u32 {
        let total: u32 = self
            .entries
            .iter()
            .flat_map(|e| e.reward.iter())
            .map(|g| match g {
                Grant::Crystal(n) => *n,
                Grant::Bound { kind, qty } if crate::gacha::is_coupon_kind(kind) => {
                    qty.saturating_mul(crate::gacha::COUPON_PRICE_GEMS as u32)
                }
                _ => 0,
            })
            .sum();
        let span = self.entries.iter().map(|e| e.at).max().unwrap_or(1).max(1);
        total / span
    }
}

// ---------------------------------------------------------------- the machine

/// Which cells `position` has reached. Pure and DERIVED — a cell is reached or it is not, and
/// that answer never depends on what was reached before.
///
/// Derived rather than accumulated for the same reason `daily::reached_mask` is: track advance
/// runs inside save accrual, which sits outside the save transaction and self-heals by re-running.
/// A sticky bit would outlive a correction; a recomputed one cannot.
pub fn reached_mask(entries: &[Entry], position: u32) -> u64 {
    let mut m = 0u64;
    for e in entries {
        // Out-of-range ids are skipped rather than shifted: `1u64 << 64` is a panic, and a panic
        // in this crate is a dead process (`panic = "abort"`), not a failed request.
        if e.id <= MAX_ENTRY_ID && position >= e.at {
            m |= 1u64 << e.id;
        }
    }
    m
}

/// What one press of Claim owes: every cell reached but not yet paid, and the mask to record.
///
/// ONE PRESS PAYS EVERYTHING OWED, not just the newest cell — a player who comes back after four
/// days collects all four. Paying only the latest would swallow the rest with nothing in the UI
/// to say so.
pub fn owed(entries: &[Entry], position: u32, claimed_mask: u64) -> (u64, Vec<&Entry>) {
    let pay = reached_mask(entries, position) & !claimed_mask;
    let list = entries
        .iter()
        .filter(|e| e.id <= MAX_ENTRY_ID && pay & (1u64 << e.id) != 0)
        .collect();
    (pay, list)
}

/// The worst single day's crystal rate across every track that can be open at once.
///
/// Checked at the BOUNDARIES of the windows because that is the only place the active set changes.
/// Windowless tracks are open on every day, so their rate is the floor under all of it.
///
/// This is the check a per-track budget test cannot make: five events can each be modest and still
/// blow the day's reserve together, because ONE login advances every one of them.
pub fn peak_crystal_rate(tracks: &[TrackDef]) -> u32 {
    // A windowless track is open on every day, so its rate is the floor under everything.
    //
    // ONBOARDING tracks are excluded and budgeted separately — see `ONBOARDING_GEM_BUDGET`. They
    // are not a rate: a once-per-account welcome divided by the days it spans is an arithmetic
    // result, not a faucet, and treating it as one prices a single gift as if it recurred forever.
    let floor: u32 = tracks
        .iter()
        .filter(|t| !matches!(t.window, Some(Window::Absolute { .. })) && !is_onboarding(t))
        .map(|t| t.crystal_rate())
        .sum();
    let mut peak = floor;
    for probe in tracks.iter().filter_map(|t| match t.window {
        Some(Window::Absolute { opens_day, .. }) => Some(opens_day),
        _ => None,
    }) {
        let sum: u32 = floor
            + tracks
                .iter()
                .filter(|t| {
                    matches!(t.window, Some(Window::Absolute { .. })) && t.is_active(probe, None)
                })
                .map(|t| t.crystal_rate())
                .sum::<u32>();
        peak = peak.max(sum);
    }
    peak
}

/// Tracks whose rows the retirement sweep should delete.
///
/// The caller asks this BEFORE touching the database: if it comes back empty — which it does on
/// almost every run — no query is issued at all. That is what keeps an hourly job from scanning a
/// million-row table to discover there is nothing to do, and is why the table needs no index on
/// `track` to support it.
pub fn retired_tracks(tracks: &[TrackDef], day: i64) -> Vec<&str> {
    tracks.iter().filter(|t| t.is_retired(day)).map(|t| t.id.as_str()).collect()
}

/// Track ids that may advance today — the argument to the one UPDATE that advances them all.
/// `opened` maps a track id to the day this account's row opened, for the tracks it has a row
/// for. A `SinceStart` track needs it; the others ignore it.
pub fn active_ids<'a>(
    tracks: &'a [TrackDef],
    day: i64,
    opened: &std::collections::BTreeMap<String, i64>,
) -> Vec<&'a str> {
    tracks
        .iter()
        .filter(|t| t.is_active(day, opened.get(&t.id).copied()))
        .map(|t| t.id.as_str())
        .collect()
}

/// Whether a track OPENS today, which is the second reason to run the advance.
///
/// The first is a new commission day. This one exists because an event can open while a player is
/// already mid-session: their day was opened hours ago, and hanging the advance on that alone
/// would hand them nothing until tomorrow. Answered from config, so it costs no query.
pub fn any_opens_on(tracks: &[TrackDef], day: i64) -> bool {
    tracks
        .iter()
        .any(|t| matches!(t.window, Some(Window::Absolute { opens_day, .. }) if opens_day == day))
}

/// Tracks that expire PER ACCOUNT, with the number of days each allows — what the row sweep needs
/// in order to delete by `opened_day` rather than by track.
pub fn per_account_expiring(tracks: &[TrackDef]) -> Vec<(&str, i64)> {
    tracks
        .iter()
        .filter_map(|t| match t.window {
            Some(Window::SinceStart { days }) => Some((t.id.as_str(), days)),
            _ => None,
        })
        .collect()
}

// ---------------------------------------------------------------- the tracks

pub const LOGIN7: &str = "login7";

/// The NEWCOMER GACHA track — the first login event.
pub const NEWCOMER_GACHA: &str = "newcomer_gacha";

/// Days a newcomer has to finish it, counted from their OWN first day (user 2026-08-18).
///
/// Far longer than the seven cells need, deliberately: the ladder is the reward for coming back,
/// not a race. Ninety days is long enough that missing a fortnight costs nothing, and short enough
/// that the row can eventually be swept instead of being kept for every account that ever existed.
pub const NEWCOMER_GACHA_DAYS: i64 = 90;

/// Every track the server runs.
///
/// CODE for now, and `Deserialize` from the first line so moving it into a table later needs no
/// change to the machine. Code buys the tests: `the_tracks_stay_inside_the_daily_reserve` and
/// friends run on every deploy. A table buys running an event without one. The trade only starts
/// to bite when events ship weekly, and the types are already shaped for it.
pub fn tracks() -> Vec<TrackDef> {
    vec![login7(), newcomer_gacha()]
}

pub fn track(id: &str) -> Option<TrackDef> {
    tracks().into_iter().find(|t| t.id == id)
}

/// The 7-day newbie ladder — ONCE PER ACCOUNT, counted in days actually PLAYED (user 2026-08-17).
///
/// Not consecutive days, and not calendar days. Missing a day costs that day and nothing else: the
/// next time the player comes back they take the next cell, never a skipped one. A streak ladder
/// punishes exactly the player it exists to keep — someone who misses day 3 of their first week is
/// the one still deciding whether to stay, and resetting them answers that question for them.
/// Counting play-days also needs no streak state and no reset rule, because the position only
/// rises, which is the one property the whole machine is built on.
///
/// NO WINDOW: it runs until all seven cells are claimed. Its rows are therefore permanent and
/// bounded by account count rather than by time — one per account, forever.
///
/// ARMOUR AND MATERIALS, not a weapon. The known early wall is b1→b2 (ATK 8 → 24) and it is
/// weapon-shaped, but weapons are class-specific while this table is data with no reader of the
/// save. TODO.md already assigns that wall to tutorial + starter gear at character creation, which
/// is where a class-aware grant belongs.
/// NEWCOMER GACHA — seven played days, ten gacha coupons, ninety days to do it in.
///
/// `1 1 2 1 1 1 3` (user 2026-08-18) adds to exactly TEN, which is one ten-pull. A new account's
/// first week therefore ends on the biggest single action in the game rather than on a handful of
/// coupons that cannot be spent yet — and the two bumps on days 3 and 7 give the week a shape
/// instead of a flat drip.
///
/// `SinceStart`, not `Absolute`: every account gets its own ninety days from ITS first day. An
/// absolute window would make this a race that everyone joins at a different point, and a player
/// who installs the game a month after launch would find their newcomer offer already half over.
///
/// The clock starts on the first ACCEPTED SAVE, which is when the row is created — so someone who
/// makes an account and does not play has not started their week, and "90 days from creating the
/// id" needs no separate account-creation lookup.
fn newcomer_gacha() -> TrackDef {
    let cell = |id: u8, at: u32, qty: u32| Entry {
        id,
        at,
        // `Bound`, not `Item` — the pull path spends a COUNTER, and a coupon in the bag is a
        // different asset that happens to share a name.
        // STANDARD tickets (user 2026-09-30): this is a daily-claim gift, not the Starter's 50.
        reward: vec![Grant::Bound { kind: crate::gacha::STANDARD_COUPON_KIND.into(), qty }],
    };
    TrackDef {
        id: NEWCOMER_GACHA.into(),
        window: Some(Window::SinceStart { days: NEWCOMER_GACHA_DAYS }),
        entries: vec![
            cell(1, 1, 1),
            cell(2, 2, 1),
            cell(3, 3, 2),
            cell(4, 4, 1),
            cell(5, 5, 1),
            cell(6, 6, 1),
            cell(7, 7, 3),
            // 1+1+2+1+1+1+3 = 10, one ten-pull.
        ],
    }
}

fn login7() -> TrackDef {
    let cell = |id: u8, at: u32, reward: Vec<Grant>| Entry { id, at, reward };
    let item = |def_id: &str, qty: u32| Grant::Item { def_id: def_id.into(), qty };
    TrackDef {
        id: LOGIN7.into(),
        window: None,
        entries: vec![
            cell(1, 1, vec![Grant::Crystal(50)]),
            // def 8, the strongest headgear of the opening tier.
            cell(2, 2, vec![item("helm_s2", 1)]),
            cell(3, 3, vec![Grant::Crystal(50)]),
            // Refine stock. Doubly useful: it is also what `refine_1` on the daily board needs, so
            // it introduces the bench rather than just topping up a bag.
            cell(4, 4, vec![item("red_herb", 10)]),
            cell(5, 5, vec![Grant::Crystal(50)]),
            // def 10.
            cell(6, 6, vec![item("shield_s1", 1)]),
            cell(7, 7, vec![Grant::Crystal(50), item("hp_pot_1", 5)]),
        ],
    }
}

// ---------------------------------------------------------------- tests

#[cfg(test)]
mod tests {
    use super::*;

    fn cell(id: u8, at: u32, crystal: u32) -> Entry {
        Entry { id, at, reward: vec![Grant::Crystal(crystal)] }
    }
    fn ladder(n: u8) -> Vec<Entry> {
        (1..=n).map(|i| cell(i, u32::from(i), 10)).collect()
    }
    fn windowed(id: &str, opens: i64, closes: i64, entries: Vec<Entry>) -> TrackDef {
        TrackDef { id: id.into(), window: Some(Window::Absolute { opens_day: opens, closes_day: closes }), entries }
    }

    /// The property the whole machine rests on: a cell pays once, however many times Claim is
    /// pressed. Same guarantee `daily::claimable` gives, restated for the generalised form — if
    /// these two ever disagree, one of them is paying twice.
    #[test]
    fn a_cell_pays_once_however_many_times_claim_is_pressed() {
        let e = ladder(3);
        let (pay, list) = owed(&e, 3, 0);
        assert_eq!(list.len(), 3);
        assert_eq!(owed(&e, 3, pay).0, 0, "PAID TWICE on re-claim");
        assert_eq!(owed(&e, 3, pay).1.len(), 0, "a third press still owed cells");
    }

    /// One press collects EVERY cell reached, not just the newest — the player who comes back on
    /// day 4 having claimed nothing.
    #[test]
    fn one_press_collects_every_cell_reached_so_far() {
        let e = ladder(5);
        let (pay, list) = owed(&e, 4, 0);
        assert_eq!(list.len(), 4, "a four-day absence should pay four cells");
        let (_, later) = owed(&e, 5, pay);
        assert_eq!(later.len(), 1, "the later press should owe exactly the new cell");
    }

    /// NEVER SKIP (user 2026-08-17). Position is days PLAYED, so a calendar gap moves nothing: two
    /// accounts on their 5th played day are in the same place whether or not one took a week off.
    #[test]
    fn missing_days_costs_those_days_and_never_skips_a_cell() {
        let e = ladder(7);
        // Played 5 days across three weeks, or 5 days straight — the input is the same number.
        let (mask, list) = owed(&e, 5, 0);
        assert_eq!(list.len(), 5);
        assert_eq!(mask.count_ones(), 5);
        // The 6th cell is the next one reached, not the one the calendar would have been on.
        let (_, next) = owed(&e, 6, mask);
        assert_eq!(next.len(), 1);
        assert_eq!(next.first().map(|c| c.at), Some(6), "a cell was skipped");
    }

    /// Re-running the same position must not move the mask. Track advance runs inside save
    /// accrual, which sits outside the save transaction and self-heals by RE-RUNNING, so anything
    /// that drifts on a repeat drifts in production.
    #[test]
    fn re_running_the_same_position_changes_nothing() {
        let e = ladder(2);
        let first = reached_mask(&e, 2);
        assert_eq!(first, reached_mask(&e, 2), "reached-cells drifted on a repeat");
        assert_eq!(owed(&e, 2, first).0, 0);
    }

    /// Position 0 owes nothing. The floor case: an off-by-one on `>=` would hand every account the
    /// first cell for having an account.
    #[test]
    fn a_track_not_yet_started_owes_nothing() {
        let e = ladder(1);
        assert_eq!(owed(&e, 0, 0).0, 0, "a cell paid before the track started");
        assert_eq!(owed(&e, 1, 0).1.len(), 1, "the first cell must pay AT its threshold");
    }

    /// The mask follows the cell's ID, not its position in the list. Editing a LIVE event —
    /// inserting a cell, dropping one — must not move what an account has already been paid.
    #[test]
    fn editing_a_live_track_does_not_move_what_was_already_paid() {
        let before = vec![cell(1, 1, 10), cell(2, 2, 10), cell(3, 3, 10)];
        let (paid, _) = owed(&before, 2, 0); // cells 1 and 2 collected
        // A cell is inserted at the FRONT and another is dropped from the middle.
        let after = vec![cell(9, 1, 10), cell(1, 1, 10), cell(3, 3, 10)];
        let (pay, list) = owed(&after, 2, paid);
        assert_eq!(pay & (1 << 1), 0, "an already-paid cell came back owing");
        assert_eq!(list.len(), 1, "only the newly added cell should be owed");
        assert_eq!(list.first().map(|c| c.id), Some(9));
    }

    /// Every cell must hand over something. `mailbox::send` rejects an empty payload, so an empty
    /// cell is not a smaller reward — it is a claim that fails at the counter.
    #[test]
    fn every_cell_hands_over_something() {
        for t in tracks() {
            for e in &t.entries {
                assert!(!e.reward.is_empty(), "{} cell {} pays nothing", t.id, e.id);
            }
        }
    }

    /// Ids are unique and fit the stored mask; cells strictly ascend.
    ///
    /// Two cells sharing an id share a bit and pay together forever. A cell above `MAX_ENTRY_ID`
    /// is silently skipped by `reached_mask` — which is the safe failure, but it is still a cell
    /// that can never be claimed.
    #[test]
    fn every_track_is_authored_legally() {
        for t in tracks() {
            let mut ids = Vec::new();
            let mut last = 0u32;
            for e in &t.entries {
                assert!(!ids.contains(&e.id), "{} reuses cell id {}", t.id, e.id);
                assert!(e.id <= MAX_ENTRY_ID, "{} cell {} overflows the stored mask", t.id, e.id);
                assert!(e.at > last, "{} has a cell at {} after {last}", t.id, e.at);
                ids.push(e.id);
                last = e.at;
            }
        }
    }

    /// A windowed track must be finishable by someone present for the whole window.
    ///
    /// A 14-day event with 14 cells cannot be completed by anyone who joins on day 2, because one
    /// login moves one cell. Real login events run fewer cells than days for exactly this; the
    /// test pins the minimum, and leaving slack beyond it is what lets latecomers finish too.
    #[test]
    fn a_windowed_track_can_be_finished_inside_its_window() {
        for t in tracks() {
            let Some(w) = t.window else { continue };
            let cells = t.entries.iter().map(|e| e.at).max().unwrap_or(0) as i64;
            assert!(cells <= w.days(), "{} needs {cells} days inside a {}-day window", t.id, w.days());
        }
    }

    /// GIL IS A FAUCET. `monetary-policy`'s test is "does this raise Gil/hour?" — and Gil handed
    /// out for logging in raises it without playing. Only an ABSOLUTE window bounds that: it mints
    /// on dated days and then stops for everyone at once.
    ///
    /// A `SinceStart` track does NOT bound it. Its ninety days restart for every new account, so
    /// it mints for as long as the game acquires players — the same open-ended faucet a windowless
    /// track is, wearing a per-account clock. The filter used to read `window.is_none()`, which
    /// silently stopped covering half the shipped tracks the moment the second window kind existed.
    #[test]
    fn only_a_dated_window_may_hand_out_gil() {
        for t in tracks()
            .iter()
            .filter(|t| !matches!(t.window, Some(Window::Absolute { .. })))
        {
            for e in &t.entries {
                assert!(
                    !e.reward.iter().any(|g| matches!(g, Grant::Gil(_))),
                    "{} has no DATED end and mints Gil at cell {}",
                    t.id,
                    e.id
                );
            }
        }
    }

    /// The tracks together stay inside the budget the daily ladder LEFT — the reserve the daily's
    /// own test set aside by name for "a login calendar".
    ///
    /// ACROSS OVERLAPPING TRACKS, not per track: one login advances every open track at once, so
    /// five modest events can blow a day's reserve while each passes on its own.
    /// The ONCE-PER-ACCOUNT grants have their own ceiling, and it is a ceiling — not an exemption.
    ///
    /// `newcomer_gacha` pays 10 coupons, and a coupon has an exact crystal price, so the welcome is
    /// worth 3,000 crystal. That is real money-equivalent value and it belongs in a budget; what it
    /// does not belong in is a DAILY rate, which would price a one-off gift as if it arrived every
    /// day for the life of the game.
    #[test]
    fn the_onboarding_grant_stays_inside_its_own_budget() {
        let total: u32 = tracks().iter().filter(|t| is_onboarding(t)).map(|t| {
            t.entries.iter().flat_map(|e| e.reward.iter()).map(|g| match g {
                Grant::Crystal(n) => *n,
                Grant::Bound { kind, qty } if crate::gacha::is_coupon_kind(kind) => {
                    qty.saturating_mul(crate::gacha::COUPON_PRICE_GEMS as u32)
                }
                _ => 0,
            }).sum::<u32>()
        }).sum();
        assert!(
            total <= ONBOARDING_GEM_BUDGET,
            "the once-per-account welcome is worth {total} crystal against a {ONBOARDING_GEM_BUDGET} \
             budget — a second onboarding ladder has to fit beside the first, which is why this is \
             a number and not an exemption"
        );
        assert!(total > 0, "no onboarding track was measured — this would pass on an empty set");
    }

    #[test]
    fn the_tracks_stay_inside_the_daily_reserve() {
        let peak = peak_crystal_rate(&tracks());
        assert!(
            peak <= RESERVE_GEM_BUDGET_PER_DAY,
            "the tracks peak at {peak} crystal/day against a {RESERVE_GEM_BUDGET_PER_DAY} reserve"
        );
        assert!(RESERVE_GEM_BUDGET_PER_DAY > 0, "the daily ladder swallowed the whole free budget");
    }

    /// And the check actually catches an overlap — a budget test that only ever passes proves
    /// nothing about the day someone adds the fifth event.
    #[test]
    fn the_budget_check_catches_events_that_only_overlap() {
        let fat = |id: &str, opens| windowed(id, opens, opens + 6, vec![cell(1, 1, 350)]);
        // Apart: each runs alone, so the peak is one track's rate.
        let apart = vec![fat("a", 0), fat("b", 100)];
        assert_eq!(peak_crystal_rate(&apart), 350);
        // Overlapping: the same two tracks, unchanged, now cost double on the shared days.
        let over = vec![fat("a", 0), fat("b", 3)];
        assert_eq!(peak_crystal_rate(&over), 700, "the overlap was not counted");
        assert!(peak_crystal_rate(&over) > RESERVE_GEM_BUDGET_PER_DAY, "this pair must be rejected");
    }

    /// A track opens and closes on the days it says, inclusive at both ends.
    /// EVERY TRACK MUST DECLARE WHAT IT PAYS, and the budget check must be told when it cannot
    /// see a track at all.
    ///
    /// `peak_crystal_rate` polices GEMS. `newcomer_gacha` pays only items, so its rate is 0 and it is
    /// invisible to that check — fine for crystal, and NOT fine as an unstated hole. This pins the
    /// two facts together rather than leaving the blind spot implicit.
    #[test]
    fn every_track_declares_what_it_pays() {
        for t in tracks() {
            let crystal: u32 = t.entries.iter().flat_map(|e| e.reward.iter())
                .map(|g| match g { Grant::Crystal(n) => *n, _ => 0 }).sum();
            let others = t.entries.iter().flat_map(|e| e.reward.iter())
                .any(|g| !matches!(g, Grant::Crystal(_)));
            assert!(crystal > 0 || others, "{} pays nothing at all", t.id);
            if crystal == 0 {
                assert!(others, "{} reports a 0 crystal rate and pays nothing else", t.id);
            }
        }
    }

    /// A COUPON CELL MUST PAY THE COUNTER THE PULL PATH SPENDS.
    ///
    /// `gacha_roll` reads `bound_items`; an inventory item of the same name is a different asset
    /// that cannot be spent on a pull. Paying `Grant::Item { def_id: "gacha_coupon" }` therefore
    /// minted a coupon the player could see and never use, while the balance the gate checks
    /// stayed at zero and refused every pull the client could make.
    #[test]
    fn a_coupon_reward_credits_the_counter_and_not_the_bag() {
        for t in tracks() {
            for e in &t.entries {
                for g in &e.reward {
                    if let Grant::Item { def_id, .. } = g {
                        assert!(
                            !crate::gacha::is_coupon_kind(def_id),
                            "{} cell {} pays a coupon as a BAG ITEM — the pull path spends the \
                             bound_items counter and will never see it",
                            t.id, e.id
                        );
                    }
                }
            }
        }
    }

    /// A `SinceStart` row is NOT swept, and that is deliberate. Deleting one is indistinguishable
    /// from an account that never had the track, so the next accepted save re-creates it with a
    /// fresh window and an empty mask and the whole ladder pays again — the same reasoning as
    /// `a_windowless_track_is_never_retired`, and the reason `row_expires_on` has no caller.
    #[test]
    fn a_since_start_row_is_never_swept_because_re_creation_would_re_pay() {
        let t = newcomer_gacha();
        assert!(!t.is_retired(10_000_000), "a SinceStart track was globally retired");
        assert!(retired_tracks(&tracks(), 10_000_000).is_empty());
        // The property that makes a sweep unsafe: with no row, the track is OPEN again.
        assert!(t.is_active(10_000_000, None), "a newcomer would be too late for their own offer");
        assert!(t.row_expires_on(0).is_some(), "the helper still describes when a row goes cold");
        assert_eq!(per_account_expiring(&tracks()), vec![(NEWCOMER_GACHA, NEWCOMER_GACHA_DAYS)]);
    }

    /// The newcomer ladder: seven played days, ten coupons, inside a per-account window.
    #[test]
    fn the_newcomer_ladder_pays_exactly_one_ten_pull() {
        let t = newcomer_gacha();
        // COUNTS `Bound`, NOT `Item`. The ladder used to pay an inventory item of the same name,
        // which the pull path cannot see — it spends a `bound_items` counter. Two assets, one
        // name: the coupon in the bag was unspendable and the counter it needed stayed at zero.
        // Asserting on the wrong variant is how that shipped looking correct.
        let coupons: u32 = t.entries.iter().flat_map(|e| e.reward.iter())
            .map(|g| match g {
                Grant::Bound { kind, qty } if kind == crate::gacha::STANDARD_COUPON_KIND => *qty,
                _ => 0,
            }).sum();
        assert_eq!(coupons, 10, "the first week must end on exactly one ten-pull");
        assert_eq!(t.entries.len(), 7);
        assert!(matches!(t.window, Some(Window::SinceStart { days: 90 })));
        assert!(t.is_active(20_000, Some(20_000)));
        assert!(t.is_active(20_089, Some(20_000)), "day 90 of 90 must still be open");
        assert!(!t.is_active(20_090, Some(20_000)), "day 91 must be closed");
    }

    #[test]
    fn a_window_is_inclusive_at_both_ends() {
        let t = windowed("e", 10, 20, ladder(3));
        assert!(!t.is_active(9, None));
        assert!(t.is_active(10, None), "the opening day must be open");
        assert!(t.is_active(20, None), "the closing day must still be open");
        assert!(!t.is_active(21, None));
        assert_eq!(t.window.map(|w| w.days()), Some(11));
    }

    /// A WINDOWLESS track is never retired, and this is the one that matters most.
    ///
    /// Deleting the row of a finished 7-day ladder looks exactly like housekeeping: every cell is
    /// claimed, the row does nothing. But a deleted row and a row that never existed are the same
    /// thing to the advance, which re-INSERTs it at position 0 — and the account walks the whole
    /// ladder again. A row dies with its TRACK, never on its own.
    #[test]
    fn a_windowless_track_is_never_retired() {
        let t = TrackDef { id: "forever".into(), window: None, entries: ladder(7) };
        assert!(!t.is_retired(0));
        assert!(!t.is_retired(10_000_000), "a windowless track was swept — every row would re-pay");
        for t in tracks().iter().filter(|t| t.window.is_none()) {
            assert!(!t.is_retired(10_000_000), "{} would be swept and re-pay", t.id);
        }
        assert!(retired_tracks(&tracks(), 10_000_000).is_empty());
    }

    /// Retirement waits out the grace period, and the sweep returns the ids to delete.
    #[test]
    fn a_closed_track_retires_only_after_the_grace_period() {
        let all = vec![windowed("ev", 0, 30, ladder(3))];
        assert!(retired_tracks(&all, 30).is_empty(), "retired while still open");
        assert!(retired_tracks(&all, 30 + RETIRE_GRACE_DAYS).is_empty(), "retired ON the grace day");
        assert_eq!(retired_tracks(&all, 31 + RETIRE_GRACE_DAYS), vec!["ev"]);
    }

    /// The advance list holds only open tracks — a closed event must not keep counting play-days
    /// for accounts that will never be able to claim them.
    #[test]
    fn only_open_tracks_advance() {
        let all = vec![
            TrackDef { id: "forever".into(), window: None, entries: ladder(3) },
            windowed("past", 0, 5, ladder(3)),
            windowed("now", 5, 15, ladder(3)),
            windowed("future", 100, 110, ladder(3)),
        ];
        assert_eq!(active_ids(&all, 10, &Default::default()), vec!["forever", "now"]);
    }

    /// An event opening MID-DAY is caught, so a player already in session gets its first cell
    /// today rather than tomorrow. Answered from config alone — no query.
    #[test]
    fn a_track_opening_today_is_visible_without_touching_the_database() {
        let all = vec![windowed("ev", 42, 60, ladder(3))];
        assert!(any_opens_on(&all, 42));
        assert!(!any_opens_on(&all, 41));
        assert!(!any_opens_on(&all, 43), "only the opening day itself forces an extra advance");
        // A windowless track never opens on any particular day; it is already open.
        assert!(!any_opens_on(&tracks(), 42));
    }

    /// The shipped ladder is 7 cells over 7 played days, and nothing above cell 7 exists for a
    /// long-lived account to find.
    #[test]
    fn the_newbie_ladder_is_seven_days_and_stops() {
        let t = login7();
        assert_eq!(t.entries.len(), 7);
        assert!(t.window.is_none(), "the newbie ladder must not expire");
        assert_eq!(owed(&t.entries, 7, 0).1.len(), 7, "seven played days should collect it all");
        assert_eq!(owed(&t.entries, 5_000, 0).1.len(), 7, "a veteran found cells past the end");
    }
}
