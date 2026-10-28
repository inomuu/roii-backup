//! MINT COUPONS (name not final; user 2026-10-03): the cost of sending one item to Steam.
//!
//! Free only and untradeable. One coupon accrues every `PERIOD_MS` of REAL time from the account's
//! first login — no need to be online — into an uncollected pool of at most `PENDING_CAP`; once
//! full, the clock stops. A claim moves coupons into the held balance (a `bound_items` row of kind
//! `KIND`), which holds at most `HELD_CAP`; what does not fit STAYS uncollected. Sending one item
//! to Steam costs one held coupon, spent when the mint worker sends it (refunded if Steam refuses).
//!
//! Pure arithmetic over the clock anchor; `main.rs` owns the table and the ledger.

/// The `bound_items` / `bound_ledger` kind holding claimed coupons.
pub const KIND: &str = "mint_coupon";
/// One coupon per this much real time.
pub const PERIOD_MS: i64 = 4 * 3_600_000;
/// Uncollected coupons at most; the clock stops while the pool is full.
pub const PENDING_CAP: u32 = 5;
/// Claimed coupons an account may hold.
pub const HELD_CAP: u32 = 10;

/// `(pending, anchor)`: coupons waiting at `now`, and the anchor normalised so time spent with a
/// FULL pool is not banked (the clock stops at the cap).
pub fn pending(anchor_ms: i64, now_ms: i64) -> (u32, i64) {
    let periods = (now_ms - anchor_ms).max(0) / PERIOD_MS;
    if periods >= i64::from(PENDING_CAP) {
        (PENDING_CAP, now_ms - i64::from(PENDING_CAP) * PERIOD_MS)
    } else {
        (u32::try_from(periods).unwrap_or(0), anchor_ms)
    }
}

/// A claim at `now` with `held` coupons already held: `(taken, new anchor)`. Takes as many as fit
/// under `HELD_CAP`; the rest stay pending, and progress toward the next coupon is kept.
pub fn claim(anchor_ms: i64, held: u32, now_ms: i64) -> (u32, i64) {
    let (pending, anchor) = pending(anchor_ms, now_ms);
    let take = pending.min(HELD_CAP.saturating_sub(held));
    (take, anchor + i64::from(take) * PERIOD_MS)
}

/// When the next coupon accrues, or `None` while the pool is full (the clock is stopped).
pub fn next_at(anchor_ms: i64, now_ms: i64) -> Option<i64> {
    let (pending, anchor) = pending(anchor_ms, now_ms);
    (pending < PENDING_CAP).then(|| anchor + (i64::from(pending) + 1) * PERIOD_MS)
}

#[cfg(test)]
mod tests {
    use super::*;
    const H: i64 = 3_600_000;

    #[test]
    fn one_coupon_every_four_hours_up_to_five_then_the_clock_stops() {
        assert_eq!(pending(0, 0).0, 0);
        assert_eq!(pending(0, 4 * H - 1).0, 0);
        assert_eq!(pending(0, 4 * H).0, 1);
        assert_eq!(pending(0, 19 * H).0, 4);
        assert_eq!(pending(0, 20 * H).0, 5);
        assert_eq!(pending(0, 100 * H).0, 5, "capped");
        assert_eq!(next_at(0, 100 * H), None, "the clock is stopped while full");
        assert_eq!(next_at(0, 5 * H), Some(8 * H));
    }

    #[test]
    fn a_claim_fills_to_ten_and_leaves_the_rest_pending() {
        // 5 pending, 7 held: 3 fit, 2 stay.
        let (take, anchor) = claim(0, 7, 20 * H);
        assert_eq!(take, 3);
        assert_eq!(pending(anchor, 20 * H).0, 2);
        // Progress toward the next one survives a claim.
        let (take, anchor) = claim(0, 0, 6 * H);
        assert_eq!((take, pending(anchor, 6 * H).0), (1, 0));
        assert_eq!(next_at(anchor, 6 * H), Some(8 * H), "2 h already earned toward the next");
        // A full pool banks nothing beyond the cap: claiming all five restarts the clock at now.
        let (take, anchor) = claim(0, 0, 100 * H);
        assert_eq!(take, 5);
        assert_eq!(next_at(anchor, 100 * H), Some(104 * H));
        // Already at ten: nothing moves.
        assert_eq!(claim(0, HELD_CAP, 20 * H).0, 0);
    }
}
