//! GEAR TIERS — the `_sN` badge that says which band a def belongs to.
//!
//! This module used to be the enchant roller (families, ranges, draw contracts) and then the
//! authored-line resolver; both are gone (0.0.382 / 0.0.384 — the lines live in `substats`,
//! from the catalog). What is left is the one thing every pool and every window keys off: the
//! tier a def declares in its id.

/// The def's TIER BADGE (its `_s<N>` segment: "sword_s3" / "sword_s3_1s" → 3).
/// This is our visible ilvl — tier windows key off the DEF, never a hidden
/// per-instance value (design rule 2: 0★ copies stay 100% fungible).
/// Defs without a badge (starter/legacy gear) count as tier 1.
pub fn def_tier(def_id: &str) -> u32 {
    def_tier_badge(def_id).unwrap_or(1)
}

/// The tier a def's id DECLARES (`_sN`), or `None` for an id that carries no badge. `def_tier`
/// reads an unbadged id as tier 1 so a starting weapon still enchants and scores; the drop and
/// fusion POOLS use this instead, so an unbadged def is never dealt — `starting_*` used to land
/// in the tier-1 pool through the fallback, and two `demo_socket_*` test defs with it.
pub fn def_tier_badge(def_id: &str) -> Option<u32> {
    def_id
        .split('_')
        .filter_map(|seg| seg.strip_prefix('s'))
        .filter(|digits| !digits.is_empty())
        .find_map(|digits| digits.parse::<u32>().ok())
        .map(|n| n.max(1))
}

/// Whether `def` is dealt by the tier-`tier` GEAR POOL — the one set both the drop rule and
/// fusion draw from, so fusion can never produce what farming cannot. Non-unique Weapon / Armor
/// whose id declares exactly this tier.
pub fn in_gear_pool(def: &crate::data::ItemDef, tier: u32) -> bool {
    matches!(def.kind, crate::items::ItemKind::Weapon | crate::items::ItemKind::Armor)
        && !def.unique
        && def_tier_badge(&def.id) == Some(tier)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn def_tier_parses_badges_and_falls_back() {
        assert_eq!(def_tier("sword_s3"), 3);
        assert_eq!(def_tier("sword_s3_1s"), 3);
        assert_eq!(def_tier("greatsword_s7_2s"), 7);
        assert_eq!(def_tier("sword_short"), 1, "no badge = tier 1");
        assert_eq!(def_tier("starting_sword"), 1);
        assert_eq!(def_tier_badge("starting_sword"), None, "the pools read the badge, not the fallback");
        assert_eq!(def_tier_badge("armor_s5_2s"), Some(5));
    }
}
