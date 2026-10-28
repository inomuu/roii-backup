//! Astier — SSR · Support · Water · staff and tome (0.0.468, user 2026-09-23). The tide that
//! keeps a party standing: she heals, and everything she heals hits harder. Lapis' partner —
//! her rolled element is as often Fire as anything else the team brings.
//!
//! Buff design follows what the Hoyo supports do (user: "ลองดูตัวบัฟพวกเกม Hoyo"): a FLAT
//! team-wide DMG% (Robin / Ruan Mei's shape — it multiplies with everything and helps a unit
//! that does not scale on ATK), plus ONE element picked at cast (Kazuha's shape, ours rolled
//! rather than chosen), and an ultimate on a different layer entirely (a heal-over-time and a
//! charge-rate window) so the two never collapse into one number.
//!
//! The kit's ids are the ones written for her in the class era (`unda_vitae`, `rivus`,
//! `vortex_undae` → `tidebless`, `fons_vitae`); only Tidebless is new.

use super::super::{BuffKind, BuffSpec, DEFAULT_SKILL, SkillDef, SkillKind};
use crate::grid::{DamageType, Element};

/// Unda Vitae — her PASSIVE basic override: a 50% ATK Water bolt at range 3 whose damage heals
/// the most wounded ally for a quarter of it, and every ally she heals carries +4% damage for
/// 8 s, three stacks deep (0.0.468).
pub const UNDA_VITAE: SkillDef = SkillDef {
    id: "unda_vitae",
    kind: SkillKind::Passive,
    max_level: 1,
    basic_element: Some(Element::Ice),
    basic_damage_type: Some(DamageType::Magic),
    basic_power_bp: 5_000,
    basic_range: 3,
    basic_ranged: true,
    heal_on_hit_bp: 2_500,
    healed_ally_dmg_bp: 400,
    healed_ally_stacks: 3,
    ..DEFAULT_SKILL
};

/// Tidebless — Skill 1 (0.0.468, replacing the old Vortex Undae): no damage. For 12 s the whole
/// party deals +25% +2.5%/lv damage, and ONE element drawn from what the party actually carries
/// deals +15% more on top. Every 18 s — long enough that cooldown reduction is worth building.
pub const TIDEBLESS: SkillDef = SkillDef {
    id: "tidebless",
    kind: SkillKind::Active,
    max_level: 10,
    loadout_slots: 2,
    cooldown_ticks: 180,
    motion_delay_ticks: 5,
    anim_slot: "cast_finish",
    range: 0,
    cast_buff: TIDEBLESS_BUFFS,
    cast_buff_party: true,
    element_roll_bp: 1_500,
    ..DEFAULT_SKILL
};

const TIDEBLESS_BUFFS: &[BuffSpec] = &[
    BuffSpec { kind: BuffKind::DmgDealtPct, magnitude: 2_500, per_level: 250, duration: 120 },
];

/// Fons Vitae — her ULTIMATE: a tide rather than a splash. For 8 s every ally is healed 5%
/// +0.5%/lv of its OWN max HP a second and fills its Limit Break gauge twice as fast.
pub const FONS_VITAE: SkillDef = SkillDef {
    id: "fons_vitae",
    kind: SkillKind::Active,
    max_level: 10,
    loadout_slots: 2,
    // 0.0.469 (user: "Astier อาจจะให้ ulti ถี่อีกหน่อย"): 7 casts an hour was the quietest
    // ultimate in the roster; a 900 gauge and a faster prayer put her near 14.
    lb_max: 900,
    lb_on_skill: 12,
    lb_on_basic: 4,
    lb_on_hit_taken: 0,
    lb_per_second: 4,
    motion_delay_ticks: 5,
    anim_slot: "special_2",
    range: 0,
    cast_buff: FONS_VITAE_BUFFS,
    cast_buff_party: true,
    hot_maxhp_bp: 500,
    hot_maxhp_per_level_bp: 50,
    ..DEFAULT_SKILL
};

const FONS_VITAE_BUFFS: &[BuffSpec] = &[
    BuffSpec { kind: BuffKind::GaugeRatePct, magnitude: 10_000, per_level: 0, duration: 80 },
];
