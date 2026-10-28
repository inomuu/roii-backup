//! Character progression state — level, XP, job level, skill points, Apex.
//!
//! The six RO base stats (STR/AGI/VIT/INT/DEX/LUK) and their allocation are GONE
//! (0.0.317). A character's power is AUTHORED — the per-character ATK curve, the
//! Doctrine profile, gear and the Potential board — not poured into attributes at
//! level-up. What is left here is the progression state itself plus the derived
//! numbers that are a function of LEVEL alone (soft DEF, HIT, FLEE, crit base).

// DEBT (6 sites): this file predates `deny(clippy::indexing_slicing)`. An index that
// cannot be proven in range is a panic, and in verify-server (which links this crate and
// replays saves under `panic = "abort"`) a panic is the whole process. Shrink this, never
// add to it — a NEW file has no allow and is rejected on its first bare index.
#![allow(clippy::indexing_slicing)]

use serde::{Deserialize, Serialize};



/// The six RO base stats + unallocated point pool + level state.
///
/// All fields are part of the canonical serialization → reshuffling
/// or renaming requires a `VERSION` bump (GDD §4.4). Field order
/// chosen to mirror RO's UI top-to-bottom so the JSON is human-
/// readable when debugging a save.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Stats {
    /// Base level (1..). XP fills the bar toward `level + 1`.
    pub level: u32,
    /// XP toward the NEXT level. Resets to 0 on level up; overflow
    /// carries into the next bar (so a giant single kill doesn't
    /// waste excess XP).
    pub xp: u64,

    // --- APEX (0.0.227): post-cap progression. At LEVEL_CAP, XP no longer
    // parks at 0 — it drains into apex levels via `xp` (the same bar). Each
    // apex level = 1 Apex Point, spent into the 4 flat buckets below. All
    // skip-if-zero → pre-cap saves + every golden are byte-identical.
    /// Total Apex Points EARNED (= post-cap levels). Points not in `apex`
    /// are unspent. Persists through a cap raise (leveling resumes; this is kept).
    /// Tier-1 stat points earned from LEVEL-UPS (the level-1 Apex allocation), spent into the
    /// core buckets. Skip-if-zero → pre-grant saves byte-identical.
    #[serde(default, skip_serializing_if = "is_zero_u32")]
    pub level_points: u32,
    #[serde(default, skip_serializing_if = "is_zero_u32")]
    pub apex_level: u32,
    /// Points SPENT per bucket, indices = `APEX_*` below. `sum(apex) <=
    /// apex_level` is enforced at allocation.
    #[serde(default, skip_serializing_if = "is_zero_alloc", deserialize_with = "de_apex")]
    pub apex: [u32; APEX_BUCKETS],
}

fn is_zero_u32(n: &u32) -> bool {
    *n == 0
}

fn is_zero_alloc(a: &[u32; APEX_BUCKETS]) -> bool {
    *a == [0; APEX_BUCKETS]
}

/// Read `apex` from ANY array width. The bucket list was 9 wide before 0.0.347 and may widen again;
/// a fixed-width array makes serde reject the WHOLE save on a length mismatch, which turns a
/// widened bucket list into an unreadable save file rather than a migration.
///
/// A legacy width is REFUNDED, not remapped: the old indices meant different stats (1 was MATK,
/// 4/5 were HIT/FLEE, 8 was Soul — all retired), the survivors changed tier, and the old per-bucket
/// ceiling was 50 against today's 20. Any remap would be guessing how to spend someone's points for
/// them. Zeroing the allocation leaves `apex_level` untouched, so every point returns to the unspent
/// pool and the player re-spends it — which costs them nothing now that a reset is free and always
/// available.
fn de_apex<'de, D>(d: D) -> Result<[u32; APEX_BUCKETS], D::Error>
where
    D: serde::Deserializer<'de>,
{
    let v: Vec<u32> = serde::Deserialize::deserialize(d)?;
    if v.len() != APEX_BUCKETS {
        return Ok([0; APEX_BUCKETS]);
    }
    let mut out = [0u32; APEX_BUCKETS];
    out.copy_from_slice(&v);
    Ok(out)
}

// STAT-POINT buckets — TWO TIERS (docs/apex-from-level-1-design.md). Serialized by array
// index (APPEND-ONLY). Indices 0..APEX_CORE are Tier-1 CORE, bought with level-up points
// (`level_points`); APEX_CORE.. are Tier-2 SUBSTATS, bought with post-cap Apex points
// (`apex_level`). Each bucket maps to a StatMod so it folds through the one aggregation seam
// (`combat::stat_mods::for_each_mod`) — see `apex_mods`.
pub const APEX_CORE: usize = 6;
pub const APEX_BUCKETS: usize = 13; // 6 core + 7 substat
// Tier 1 — core (level-up points)
pub const AP_ATK: usize = 0;
pub const AP_HP: usize = 1;
pub const AP_DEF: usize = 2;
pub const AP_CRIT: usize = 3;
pub const AP_CRITDMG: usize = 4;
pub const AP_DODGE: usize = 5;
// Tier 2 — substats (post-cap Apex points)
pub const AP_DEFPEN: usize = 6;
pub const AP_BREAKDMG: usize = 7;
pub const AP_BREAKBUILD: usize = 8;
pub const AP_COOLDOWN: usize = 9;
pub const AP_CAST: usize = 10;
pub const AP_CHARGE: usize = 11;
pub const AP_HEAL: usize = 12;

/// Every bucket caps at this many points (docs D4, user: max 20/stat).
pub const APEX_BUCKET_CAP: u32 = 20;

/// Per-point magnitudes, tuned against measured base stats and the gear roll table (0.0.348).
///
/// THE RULE: a bucket taken all the way to the cap is worth ABOUT ONE TOP-TIER GEAR LINE — a
/// little more where the bucket is the whole build's commitment. Gear carries many lines and a
/// player carries one allocation, so a bucket that beat a fistful of lines would make gear
/// pointless, and one worth a fraction of a line would not be worth the screen.
///
/// THE MAGNITUDES ARE FLAT, NOT PERCENTAGES (user 2026-09-11: a percentage is harder to tune).
/// A point says what it gives — "+10 ATK" — and the number on the screen is the number in the
/// fight. Measured with `--dump-stats 70`, the cost of that choice is a known skew: base ATK runs
/// 260 -> 1141 from level 1 to the cap (4.4x), so a flat ATK point is worth about the same all the
/// way (24% of a level-10 character, 17% of a capped one), but base Max HP runs 440 -> 17851 (40x),
/// so a flat HP point is worth far more early than late. HP is sized for the middle rather than
/// either end, and it is the bucket to watch in play-testing.
/// DEF is flat for a second reason: it is SUBTRACTED from a hit and its base never grows at all
/// (40-80 at every level), so a percentage of it would be nothing. It is sized against what a hit
/// actually is — ~4500 at the cap — and is deliberately the early-strong pick.
pub const APEX_PER_PT: [u32; APEX_BUCKETS] = [
    10,   // ATK flat     +10/pt, +200 at cap    ~ 17% of a capped ATK, 24% of a level-10 one
    100,  // HP flat      +100/pt, +2000 at cap  ~ 11% of capped HP; worth more the earlier it is
    10,   // DEF flat     +10/pt, +200 at cap    ~ 4% off a capped hit; no gear family
    15,   // Crit bp      +0.15%/pt, +3% at cap  = one T3 CritBp line (300)
    100,  // Crit DMG bp  +1%/pt, +20% at cap    = one T3 line (1500-2000)
    60,   // Dodge bp     +0.6%/pt, +12% at cap  = one T3 line (1000-1200)
    50,   // DEF pen bp   +0.5%/pt, +10% at cap  ~ one T3 line (1200-1500); cap 5000, gear 3000
    100,  // Break DMG bp +1%/pt, +20% at cap    = one T3 line (1500-2000)
    50,   // Break Buildup bp +0.5%/pt, +10% at cap — HALF a line on purpose: the roll window is
          //                  narrow by design so the +50% ceiling costs most of a build, and a
          //                  bucket worth two lines would hand that over for nothing.
    20,   // Cooldown bp  +0.2%/pt, +4% at cap   — halved 0.0.490 with every other CDR source (they
          //                  summed to ~80% against the 3000 clamp); was 40.
    50,   // Cast bp      +0.5%/pt, +10% at cap  ~ one T3 line (1100-1500); cap 5000
    50,   // Charge bp    +0.5%/pt, +10% at cap  — no gear family; sized with its neighbours
    50,   // Heal bp      +0.5%/pt, +10% at cap  = one T3 line (1000-1300)
];

impl Stats {
    /// Brand-new character: level 1, 0 xp.
    pub fn new() -> Self {
        Self {
            level: 1,
            xp: 0,
            level_points: 0,
            apex_level: 0,
            apex: [0; APEX_BUCKETS],
        }
    }

    /// Tier-1 CORE points earned but not yet allocated (from `level_points`, the level-up grant).
    pub fn core_unspent(&self) -> u32 {
        self.level_points
            .saturating_sub(self.apex[..APEX_CORE].iter().sum())
    }

    /// Tier-2 SUBSTAT points earned but not yet allocated (from `apex_level`, the post-cap grant).
    pub fn apex_unspent(&self) -> u32 {
        self.apex_level
            .saturating_sub(self.apex[APEX_CORE..].iter().sum())
    }

    /// Spend up to `count` points into `bucket`, from that bucket's TIER pool (core buckets draw
    /// `level_points`, substat buckets draw `apex_level`). Clamps to the pool AND the per-bucket
    /// ceiling; a bad index / empty pool / capped bucket is a silent no-op (replay-safe).
    pub fn allocate_apex(&mut self, bucket: usize, count: u32) {
        if bucket >= APEX_BUCKETS {
            return;
        }
        let pool = if bucket < APEX_CORE { self.core_unspent() } else { self.apex_unspent() };
        let room = APEX_BUCKET_CAP.saturating_sub(self.apex[bucket]);
        let spend = count.min(pool).min(room);
        self.apex[bucket] = self.apex[bucket].saturating_add(spend);
    }

    /// Return every allocated point to its tier's pool — a free respec, both tiers (docs D6).
    pub fn reset_apex(&mut self) {
        self.apex = [0; APEX_BUCKETS];
    }

    /// One constructor per bucket, index-aligned with `APEX_PER_PT` and the `AP_*` indices, so a
    /// bucket's StatMod is a table lookup rather than a match that can fall out of step with it.
    const MOD_CTORS: [fn(i32) -> crate::items::StatMod; APEX_BUCKETS] = {
        use crate::items::StatMod as M;
        [
            M::AtkFlat,        // AP_ATK
            M::MaxHp,          // AP_HP
            M::DefFlat,        // AP_DEF
            M::CritBp,         // AP_CRIT
            M::CritDmgBp,      // AP_CRITDMG
            M::DodgeBp,        // AP_DODGE
            M::IgnoreDefBp,    // AP_DEFPEN
            M::BreakDmgBp,     // AP_BREAKDMG
            M::BreakBuildupBp, // AP_BREAKBUILD
            M::CooldownBp,     // AP_COOLDOWN
            M::CastBp,         // AP_CAST
            M::ChargeRateBp,   // AP_CHARGE
            M::HealBp,         // AP_HEAL
        ]
    };

    /// Visit the allocated buckets AS StatMods, so they fold through `for_each_mod` alongside gear.
    /// A CALLBACK, not a Vec: `for_each_mod` runs inside fifteen stat readers that combat calls
    /// many times per entity per tick, and handing each of those a heap allocation is a cost the
    /// old flat `apex()` struct never charged. Visits nothing for an unallocated unit.
    pub fn for_each_apex_mod(&self, mut f: impl FnMut(&crate::items::StatMod)) {
        for i in 0..APEX_BUCKETS {
            let pts = self.apex[i];
            if pts == 0 {
                continue;
            }
            f(&Self::MOD_CTORS[i]((pts.min(APEX_BUCKET_CAP) * APEX_PER_PT[i]) as i32));
        }
    }

    /// The same set, collected — for tests and for the client views that want a list.
    pub fn apex_mods(&self) -> Vec<crate::items::StatMod> {
        let mut out = Vec::new();
        self.for_each_apex_mod(|m| out.push(*m));
        out
    }

}

impl Default for Stats {
    fn default() -> Self {
        Self::new()
    }
}

// --- BETA-5 Phase 1: stat → combat bonuses ---
//
// Tuning lives here (not data.json) for the same reason `combat.rs`
// element multipliers do: simple math, rarely touched, easier to audit
// when the formula is *next to* the values it operates on. If we want
// designer-tunable later, move into game_data.json as basis points.
//
// Formulas (Phase 1 melee/ranged/aspd/hp/def; Phase 2 adds SP + crit):
//
//   STR: melee StatusATK ⌊Lvl/4⌋+STR+⌊DEX/5⌋+⌊LUK/3⌋ (RO Renewal, linear)
//   DEX: ranged StatusATK ⌊Lvl/4⌋+⌊STR/5⌋+DEX+⌊LUK/3⌋
//   AGI: +5 milli atk_speed per point    (faster swing accumulator)
//   VIT: +5 max_hp + 1 def per 5 points  (50 hp + 2 def @ VIT 10)
//   INT: +6 max SP per point             (Phase 2 — UI only until skills land)
//   LUK: +30 bp crit chance per point ABOVE BASELINE (LUK 1 → 0%, LUK 2 → 0.3%, LUK 99 → ~29.4%)
//
// AGI doesn't yet feed Flee (no dodge system); SP regen + skills are
// Phase 3, so for now SP is a static display number that scales with INT.
//
// Why `(luk - 1) * 30` and not `luk * 30`?
//   Every character starts at STAT_MIN = 1, so the LUK baseline must
//   produce *zero* crit chance — otherwise even brand-new characters
//   roll crit dice every swing, which would shift PRNG draw order vs.
//   every pre-0.0.14 golden hash. The `-1` keeps STAT_MIN crit at 0,
//   so resolve_attack can skip the draw entirely and the goldens stay
//   pinned (see `combat::roll_crit`).

// Crit damage rule (0.0.224): RENEWAL style — a crit multiplies the packet
// by `combat::effective_crit_dmg_bp` (150% base + card/gear crit-dmg lines)
// and DEF mitigates normally. The pre-renewal def-ignore "boss killer" rule
// is retired (user 2026-07-21); crit builds now scale via the CRIT DMG stat.

impl Stats {
    // (AGI → ASPD moved to combat.rs `effective_atk_speed_milli`: it's
    //  now the RO weapon-ASPD model — weapon base + AGI×4 + DEX, 50/(200-
    //  ASPD) curve, cap 190 — not a flat per-AGI milli bonus.)
    // (VIT→MaxHP moved to combat.rs: it's a per-class multiplier,
    //  `(base + growth·Lvl) × (1 + VIT/100)`, not a flat stat bonus —
    //  see effective_max_hp. MaxSP is gone entirely, 0.0.315.)
    /// Soft DEF — RO **Renewal** flat mitigation (subtracted after the
    /// future hard-DEF %-reduction): `⌊VIT/2 + AGI/5 + BaseLevel/2⌋`.
    /// Flat-subtract fits our current model; the hard-DEF soft-cap %
    /// (gear) is a separate, later change.
    pub fn def_bonus(&self) -> i32 {
        // Level only since 0.0.86 — VIT and AGI are gone. x10 stat scale.
        ((self.level as i32) / 2) * 10
    }
    /// LUK-derived crit chance in basis points (10000 = 100%).
    /// Returns 0 at STAT_MIN so STAT_MIN attackers consume no PRNG
    /// draws — preserves the pre-Phase-2 PRNG stream (see module-
    /// level comment for the rationale).
    pub fn crit_chance_bp(&self) -> u32 {
        // LUK is gone (0.0.86). Crit is a FLAT base every character shares, and
        // everything above it comes from gear — the arrangement every current
        // gacha uses, and the one our own measurement already showed us running
        // (crit never moved with level even before this).
        CRIT_BASE_BP
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_old_nine_wide_apex_save_loads_and_refunds_its_points() {
        // The shape a pre-0.0.347 save carries for a unit that had spent Apex points. Against a
        // fixed-width array serde rejects this outright ("invalid length 9, expected an array of
        // length 13") and the whole save is unreadable — the widening has to be a migration.
        let old = r#"{"level":70,"xp":0,
                      "apex_level":10,"apex":[4,0,6,0,0,0,0,0,0]}"#;
        let s: Stats = serde_json::from_str(old).expect("an old save must still load");
        assert_eq!(s.level, 70);
        assert_eq!(s.apex_level, 10, "the EARNED points survive");
        assert_eq!(s.apex, [0; APEX_BUCKETS], "the allocation is refunded, never remapped");
        assert_eq!(s.apex_unspent(), 10, "every point is spendable again");
    }

    #[test]
    fn a_save_with_no_apex_at_all_still_loads() {
        // No `apex` key at all — the ordinary case, since the field skips at zero.
        let plain = r#"{"level":12,"xp":5}"#;
        let s: Stats = serde_json::from_str(plain).expect("loads");
        assert_eq!(s.level, 12);
        assert_eq!(s.apex, [0; APEX_BUCKETS]);
        assert_eq!(s.level_points, 0);
    }

    #[test]
    fn apex_allocation_clamps_and_folds_as_mods() {
        let mut s = Stats::new();
        assert_eq!(s.core_unspent(), 0);
        assert_eq!(s.apex_unspent(), 0);
        assert!(s.apex_mods().is_empty(), "no allocation → no mods → byte-identical");

        // Earn 10 CORE points (Tier-1, from level-ups); over-spend one bucket clamps to the pool.
        s.level_points = 10;
        s.allocate_apex(AP_ATK, 4);
        s.allocate_apex(AP_HP, 100); // only 6 left → clamps to 6
        assert_eq!(s.apex[AP_ATK], 4);
        assert_eq!(s.apex[AP_HP], 6);
        assert_eq!(s.core_unspent(), 0);
        s.allocate_apex(99, 1); // bad index → no-op
        // A SUBSTAT bucket draws the (empty) apex_level pool → no-op.
        s.allocate_apex(AP_DEFPEN, 1);
        assert_eq!(s.apex[AP_DEFPEN], 0);

        let mods = s.apex_mods();
        assert!(mods.contains(&crate::items::StatMod::AtkFlat(4 * APEX_PER_PT[AP_ATK] as i32)));
        assert!(mods.contains(&crate::items::StatMod::MaxHp(6 * APEX_PER_PT[AP_HP] as i32)));

        // A free respec returns everything.
        s.reset_apex();
        assert_eq!(s.core_unspent(), 10);
        assert!(s.apex_mods().is_empty());
    }

    #[test]
    fn apex_bucket_caps_at_20_points() {
        let mut s = Stats::new();
        s.level_points = 1_000;
        s.allocate_apex(AP_CRIT, 1_000);
        s.allocate_apex(AP_ATK, 1_000);
        assert_eq!(s.apex[AP_CRIT], APEX_BUCKET_CAP, "clamped to the 20-pt ceiling");
        let mods = s.apex_mods();
        assert!(mods.contains(&crate::items::StatMod::CritBp(
            (APEX_BUCKET_CAP * APEX_PER_PT[AP_CRIT]) as i32
        )));
        assert!(mods.contains(&crate::items::StatMod::AtkFlat(
            (APEX_BUCKET_CAP * APEX_PER_PT[AP_ATK]) as i32
        )));
    }

    #[test]
    fn soft_def_is_level_only() {
        let mut s = Stats::new();
        // Soft DEF is LEVEL only since 0.0.86 — at the x10 stat scale. Lvl 40 → 20 x 10.
        s.level = 40;
        assert_eq!(s.def_bonus(), 20 * 10);
    }








}

/// The x10 stat scale (0.0.83-84). ATK, HP and DEF all carry it, so any formula
/// that reads one of them as a PERCENTAGE — a number out of 100 rather than an
/// amount of damage — has to divide it back out. RO's freeze resist is the one
/// such formula today, and the reason this is named rather than a bare 10.
pub const STAT_SCALE: i32 = 10;

/// Base critical chance every character carries, in basis points. Flat, like the
/// 5% every Genshin and HSR character starts with; the rest comes from gear.
pub const CRIT_BASE_BP: u32 = 500;

/// Which of the six retired RO stats an `InputEvent::AllocateStatPoint` names.
///
/// The stats are gone (0.0.317) and the event is a no-op, but `InputEvent` is
/// SERIALIZED and every save replays its whole input log, so the shape has to
/// keep deserializing. Nothing reads the value.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum StatKind {
    Str,
    Agi,
    Vit,
    Int,
    Dex,
    Luk,
}
