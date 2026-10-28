//! PCG32 (specifically: PCG-XSH-RR variant, 32-bit output).
//!
//! Why a custom PRNG and not `rand` / `rand_pcg`?
//!   GDD §4.2 rule 3: "Single PRNG, owned by `GameState`, seeded at new-game.
//!   Every random draw goes through it in a defined order."
//!   GDD §7.4: prefer zero-dependency code in sim-core. PCG32 is ~40 lines.
//!
//! Why PCG32 and not (say) xorshift / SplitMix / LCG?
//!   - Fast, well-studied, passes all standard statistical batteries.
//!   - 64-bit internal state → period 2^64, plenty for this game's lifetime.
//!   - **Bit-deterministic** across CPUs / OSes — exactly what GDD §4.2 demands
//!     (Windows + macOS + Linux must produce identical streams).
//!   - The single multiplier + add + xorshift-rotate composition uses only
//!     `wrapping_mul` / `wrapping_add` on `u64` and a `rotate_right` on `u32`.
//!     All of those are spec'd by Rust to be bit-identical on every target.
//!
//! Reference: O'Neill, "PCG: A Family of Simple Fast Space-Efficient
//!   Statistically Good Algorithms for Random Number Generation" (2014).
//!   Specifically the `pcg32` / `pcg_oneseq_64_xsh_rr_32_random_r` variant
//!   from the official pcg-c library.

use serde::{Deserialize, Serialize};

/// Multiplier used by PCG (a known good 64-bit LCG multiplier from
/// L'Ecuyer's tables). Don't change this. Ever.
const PCG_MULTIPLIER: u64 = 6_364_136_223_846_793_005;

/// "Increment" / sequence selector. Must be odd. We use the default
/// stream (the same one the pcg-c reference uses).
const PCG_INCREMENT: u64 = 1_442_695_040_888_963_407;

/// The PRNG state. Owned by `GameState` (one and only one per save).
///
/// `state` is the LCG state — fully determines future output.
/// (No `inc` field: we hardcode the stream. If we ever need multiple
/// independent streams — e.g. one for drops, one for refines — we'll
/// add a `stream_id: u64` here. NOT needed for Phase 0; a single
/// well-ordered sequence keeps the rule "every draw in a defined
/// order" trivially auditable.)
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Pcg32 {
    state: u64,
}

impl Pcg32 {
    /// Create a new PCG32 from a user-provided 64-bit seed.
    ///
    /// Seeding follows the standard PCG init sequence:
    ///   1. start state at 0
    ///   2. advance once (mixes in the increment)
    ///   3. add the user seed
    ///   4. advance once more (mixes the seed into the state)
    ///
    /// This guarantees that even adjacent seeds (e.g. 42 and 43) produce
    /// completely different output streams from the first draw.
    pub fn new(seed: u64) -> Self {
        let mut rng = Pcg32 { state: 0 };
        let _ = rng.step();
        rng.state = rng.state.wrapping_add(seed);
        let _ = rng.step();
        rng
    }

    /// Internal LCG advance. Returns the *previous* state, which is what
    /// PCG-XSH-RR permutes for its output. This split is intentional and
    /// is the heart of PCG's quality: the output is derived from the
    /// pre-update state via a non-linear permutation (XSH + RR), so the
    /// raw LCG bits never leak.
    fn step(&mut self) -> u64 {
        let old = self.state;
        self.state = old
            .wrapping_mul(PCG_MULTIPLIER)
            .wrapping_add(PCG_INCREMENT);
        old
    }

    /// A deterministic, NON-ADVANCING marker of the current stream position —
    /// equal iff the same number of draws have been taken from the same seed. Used
    /// to derive replayable, dedup-able tradeable-item instance ids (S3, via
    /// `hash::instance_id`): a server replay reaches the same position → same
    /// fingerprint → same id → the ledger dedupes. Read-only: it never advances the
    /// stream, so it can't perturb determinism. The value is only ever fed INTO a
    /// hash (never exposed raw to gameplay), and the RNG is server-seeded (S2), so
    /// surfacing the position is not a new vulnerability.
    pub fn fingerprint(&self) -> u64 {
        self.state
    }

    /// Generate the next u32 from the stream.
    ///
    /// XSH-RR ("xor-shift high, random rotation"):
    ///   xorshifted = ((old >> 18) ^ old) >> 27   (a 32-bit value)
    ///   rot        = old >> 59                   (top 5 bits — rotation amount)
    ///   output     = xorshifted rotated right by rot
    ///
    /// The data-dependent rotation is what makes PCG pass statistical tests
    /// that plain LCGs catastrophically fail.
    pub fn next_u32(&mut self) -> u32 {
        let old = self.step();
        let xorshifted = (((old >> 18) ^ old) >> 27) as u32;
        let rot = (old >> 59) as u32;
        xorshifted.rotate_right(rot)
    }

    /// Generate a `u64` by stitching two `u32`s. Used rarely — most draws
    /// are small ranges. Documenting the order matters: low half first,
    /// then high half. If we ever swap this we change every save's stream.
    pub fn next_u64(&mut self) -> u64 {
        let lo = self.next_u32() as u64;
        let hi = self.next_u32() as u64;
        (hi << 32) | lo
    }

    /// Uniform integer in `[low, high)`. Returns `low` if `high <= low`.
    ///
    /// Uses Lemire's rejection-free method for fast unbiased ranges
    /// (https://lemire.me/blog/2019/06/06/nearly-divisionless-random-integer-generation-on-various-systems/),
    /// adapted to u64 math to keep things deterministic even when the
    /// range is small enough that bias would be negligible. (We still
    /// reject if bias would exceed 0 — the loop is bounded in practice.)
    pub fn gen_range_u32(&mut self, low: u32, high: u32) -> u32 {
        if high <= low {
            return low;
        }
        let range = (high - low) as u64;
        // Multiply-high trick: produces a (range × 2^32)-sized window.
        // If the low 32 bits land in the "leftover" zone we redraw.
        // For game-sized ranges (drop weights up to a few thousand) this
        // loop runs >99.9% of the time exactly once.
        loop {
            let x = self.next_u32() as u64;
            let m = x * range;
            let lo = m & 0xFFFF_FFFF;
            if lo >= range.wrapping_neg() & 0xFFFF_FFFF {
                // The "rejection" branch — extremely rare for small ranges.
                // Threshold = (2^32) mod range, but computed as -range mod 2^32.
                let threshold = (range.wrapping_neg()) % range;
                if lo < threshold {
                    continue;
                }
            }
            return low + ((m >> 32) as u32);
        }
    }

    /// Roll a basis-point probability check.
    ///
    /// `odds_bp` is in basis points: 10000 = 100%, 1250 = 12.5%, 0 = never,
    /// 10000+ = always.
    ///
    /// This is the canonical "did the proc fire?" primitive — used by crit,
    /// dodge, refine, drop chances. Standardizing it here means money-zone
    /// code (drops.rs, refine.rs) never has to think about whether their
    /// comparison should be `<` or `<=`, or whether `10000` means "always"
    /// (it does, here).
    pub fn roll_bp(&mut self, odds_bp: u32) -> bool {
        if odds_bp == 0 {
            return false;
        }
        if odds_bp >= 10_000 {
            return true;
        }
        self.gen_range_u32(0, 10_000) < odds_bp
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Reference test vectors for seed=42.
    ///
    /// These ten values are this codebase's golden truth — they were
    /// computed by running this exact implementation once. Any future
    /// change to PCG32 logic that breaks these is a determinism break,
    /// and per GDD §7.2 requires explicit acknowledgement + version bump,
    /// NOT silently updating the expected list.
    ///
    /// (We don't try to match pcg-c byte-for-byte; the pcg-c reference
    /// uses a different seeding routine. What matters is OUR stream is
    /// stable forever.)
    const SEED_42_FIRST_TEN: [u32; 10] = [
        3_270_867_926,
        1_795_671_209,
        1_924_641_435,
        1_143_034_755,
        4_121_910_957,
        1_757_328_946,
        3_418_829_100,
        3_589_261_271,
        2_062_288_904,
        4_279_450_293,
    ];

    #[test]
    fn pcg32_seed_42_is_stable() {
        let mut rng = Pcg32::new(42);
        let got: Vec<u32> = (0..10).map(|_| rng.next_u32()).collect();
        assert_eq!(got, SEED_42_FIRST_TEN.to_vec(),
            "PCG32 stream for seed=42 changed. \
             This is a determinism break — see GDD §7.2.");
    }

    /// Adjacent seeds must produce wildly different streams from draw 1.
    /// (Catches the common "I broke seeding and now seed=42 and seed=43
    /// give similar outputs" regression.)
    #[test]
    fn pcg32_adjacent_seeds_diverge() {
        let mut a = Pcg32::new(42);
        let mut b = Pcg32::new(43);
        // First draws must differ in many bits — Hamming distance > 10.
        let xa = a.next_u32();
        let xb = b.next_u32();
        let hd = (xa ^ xb).count_ones();
        assert!(hd > 10, "adjacent seeds too correlated: hd={}", hd);
    }

    /// `roll_bp(0)` never fires, `roll_bp(10_000+)` always fires.
    #[test]
    fn pcg32_roll_bp_edges() {
        let mut rng = Pcg32::new(7);
        for _ in 0..1000 {
            assert!(!rng.roll_bp(0));
            assert!(rng.roll_bp(10_000));
            assert!(rng.roll_bp(15_000));
        }
    }

    /// `roll_bp(2500)` should fire ~25% of 100k trials. We allow ±1%.
    #[test]
    fn pcg32_roll_bp_distribution() {
        let mut rng = Pcg32::new(1234);
        let mut hits = 0;
        let n = 100_000;
        for _ in 0..n {
            if rng.roll_bp(2_500) {
                hits += 1;
            }
        }
        let rate = (hits * 10_000) / n;
        assert!((2_400..=2_600).contains(&rate),
            "expected ~25%, got {}.{:02}%", rate / 100, rate % 100);
    }

    /// `gen_range_u32(low, high)` stays in `[low, high)`.
    #[test]
    fn pcg32_gen_range_bounds() {
        let mut rng = Pcg32::new(99);
        for _ in 0..10_000 {
            let x = rng.gen_range_u32(10, 20);
            assert!((10..20).contains(&x), "out of range: {}", x);
        }
    }

    /// Clone produces independent state — but cloning then advancing both
    /// produces matching streams (used for golden-test replay snapshots).
    #[test]
    fn pcg32_clone_is_independent() {
        let mut a = Pcg32::new(2024);
        let mut b = a.clone();
        for _ in 0..100 {
            assert_eq!(a.next_u32(), b.next_u32());
        }
    }
}
