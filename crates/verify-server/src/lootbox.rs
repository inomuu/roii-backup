//! SEALED CACHE — the server side of `InputEvent::OpenLootbox` (sim 0.0.449).
//!
//! The box is a rare, BOUND field/boss drop (`loot_rules.orbs`, server-rolled like every rare
//! drop). Opening one is the only thing a player can do with it, and the sim records nothing but
//! a count (`Measures.lootboxes_opened`). What was inside is decided HERE, per open beyond the
//! account's watermark, with a keyed nonce — deterministic, so a save seen twice pays once and
//! an audit can recompute every verdict.
//!
//! Design (user 2026-09-21): a farming "bonus", not a faucet — most opens are a few crystal, and
//! the Summon coupons inside are what a player screenshots. The coupon tiers carry a SERVER-WIDE
//! monthly quota that is decremented when someone actually wins one; a win past the quota pays
//! the consolation instead (`Outcome::Crystal(50)`), silently. The box cannot be bought or
//! traded, so nothing here is a disclosed odds table — the numbers below are tuning, not a
//! promise (user: "ไม่จำเป็นต้องแจกแจง เพราะไม่ใช่ของที่ซื้อได้").
//!
//! Every payout is BOUND: crystal into `crystal_wallet.bound`, coupons into `bound_items` under
//! `gacha::STANDARD_COUPON_KIND` (the Standard banner — user 2026-10-03), both with a ledger row — the same two sinks `slot_tickets` uses.

use serde::Serialize;

/// What one open produced.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum Outcome {
    /// Bound crystal.
    Crystal(u32),
    /// Standard-banner summon coupons (`gacha::STANDARD_COUPON_KIND`), a quota tier.
    Coupons(u32),
}

/// The tiers that count against a monthly quota, by coupon count.
pub const QUOTA_TIERS: [(u32, u32); 3] = [(100, 5), (10, 50), (1, 500)];

/// Per-open odds in parts per million, tuned for the launch scale the user expects — 10,000
/// players ≈ 100,000 opens a month — so every coupon tier reaches its monthly quota about once
/// (×100 5, ×10 50, ×1 500): the quota is a working ceiling from day one, not a far backstop. A
/// smaller server simply sees fewer (1,000 players: ~6 ×100 a year, ~5 ×10 a month). The rest is
/// 10 crystal, with a 50-crystal band that is also what a quota-exhausted win pays.
const PPM_COUPON_100: u64 = 50; // 0.005%
const PPM_COUPON_10: u64 = 500; // 0.05%
const PPM_COUPON_1: u64 = 5_000; // 0.5%
const PPM_CRYSTAL_50: u64 = 100_000; // 10%
pub const CONSOLATION: Outcome = Outcome::Crystal(50);
const FLOOR: Outcome = Outcome::Crystal(10);

/// Roll one open from its nonce. Pure and total.
pub fn roll(nonce: u64) -> Outcome {
    let r = nonce % 1_000_000;
    if r < PPM_COUPON_100 {
        Outcome::Coupons(100)
    } else if r < PPM_COUPON_100 + PPM_COUPON_10 {
        Outcome::Coupons(10)
    } else if r < PPM_COUPON_100 + PPM_COUPON_10 + PPM_COUPON_1 {
        Outcome::Coupons(1)
    } else if r < PPM_COUPON_100 + PPM_COUPON_10 + PPM_COUPON_1 + PPM_CRYSTAL_50 {
        Outcome::Crystal(50)
    } else {
        FLOOR
    }
}

/// The monthly quota for a coupon tier, `None` for a tier with no quota (there is none).
pub fn quota_of(coupons: u32) -> Option<u32> {
    QUOTA_TIERS.iter().find(|(c, _)| *c == coupons).map(|(_, q)| *q)
}

/// Month key for the quota row: `"2026-09"` from a millisecond timestamp (UTC).
pub fn month_key(now_ms: u64) -> String {
    let days = now_ms / 86_400_000;
    // Civil-from-days (Howard Hinnant), integer only.
    let z = days as i64 + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    format!("{y:04}-{m:02}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_bands_partition_the_nonce_space_in_the_tuned_shares() {
        let n = 1_000_000u64;
        let mut c100 = 0;
        let mut c10 = 0;
        let mut c1 = 0;
        let mut k50 = 0;
        let mut k10 = 0;
        for r in 0..n {
            match roll(r) {
                Outcome::Coupons(100) => c100 += 1,
                Outcome::Coupons(10) => c10 += 1,
                Outcome::Coupons(1) => c1 += 1,
                Outcome::Crystal(50) => k50 += 1,
                Outcome::Crystal(10) => k10 += 1,
                other => panic!("{other:?}"),
            }
        }
        assert_eq!((c100, c10, c1, k50), (50, 500, 5_000, 100_000));
        assert_eq!(k10, 894_450, "everything else is the floor");
        // Nonces above the modulus wrap, never fall through.
        assert_eq!(roll(u64::MAX - 3), roll((u64::MAX - 3) % 1_000_000));
    }

    #[test]
    fn quotas_name_the_three_coupon_tiers_and_nothing_else() {
        assert_eq!(quota_of(100), Some(5));
        assert_eq!(quota_of(10), Some(50));
        assert_eq!(quota_of(1), Some(500));
        assert_eq!(quota_of(7), None);
    }

    #[test]
    fn month_key_is_utc_civil() {
        assert_eq!(month_key(0), "1970-01");
        // 2026-09-21 ~ 1_789_948_800_000 ms.
        assert_eq!(month_key(1_789_948_800_000), "2026-09");
        assert_eq!(month_key(1_790_000_000_000 + 86_400_000 * 20), "2026-10");
    }
}
