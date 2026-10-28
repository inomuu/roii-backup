//! Mirei — SR · Catalyst · Thunder · staff and tome (0.0.465, user 2026-09-22). The elder
//! sister of the crew: she does not chase kills, she CHARGES the room and lets the others
//! collect. Every enemy she seeds carries her sparks; anyone's blow sets one off, and only
//! theirs spends it.
//!
//! SR rule: S0 ≈ 65% of Laplus' S0 on the party bench, S6 ≈ 95%.

use super::super::{BuffKind, BuffSpec, DEFAULT_SKILL, SkillDef, SkillKind};
use crate::grid::{DamageType, Element};

/// Conductor — her PASSIVE basic override: a 90% ATK Thunder bolt at range 6 that aims at
/// whoever still carries her spark, sets one off, and does NOT spend it (user: "การโจมตีของ
/// ตัวเองจะไม่กิน static spark แต่ยัง trigger dmg").
pub const CONDUCTOR: SkillDef = SkillDef {
    id: "conductor",
    kind: SkillKind::Passive,
    max_level: 1,
    basic_element: Some(Element::Thunder),
    basic_damage_type: Some(DamageType::Magic),
    basic_power_bp: 9_000,
    basic_range: 6,
    basic_ranged: true,
    spark_keeper: true,
    ..DEFAULT_SKILL
};

/// Storm Field — Skill 1: a 5×5 sheet of lightning for 140% ATK +14%/lv of Thunder that leaves
/// FIVE Static Sparks on everything inside, each worth 150% of her ATK when it goes off. Every
/// 10 s.
pub const STORM_FIELD: SkillDef = SkillDef {
    id: "storm_field",
    kind: SkillKind::Active,
    max_level: 10,
    loadout_slots: 2,
    cooldown_ticks: 100,
    motion_delay_ticks: 6,
    cast_time_ticks: 10,
    anim_slot: "cast_finish",
    range: 6,
    aoe_radius: 2,
    power_bp: 14_000,
    power_per_level_bp: 1_400,
    cast_element: Some(Element::Thunder),
    damage_type: Some(DamageType::Magic),
    spark_stacks: 5,
    spark_atk_bp: 15_000,
    ..DEFAULT_SKILL
};

/// Quicken — Skill 2: she hurries her own hands, ATK speed +30% +2%/lv for 6 s. Every 14 s.
pub const QUICKEN: SkillDef = SkillDef {
    id: "quicken",
    kind: SkillKind::Active,
    max_level: 10,
    loadout_slots: 1,
    cooldown_ticks: 140,
    motion_delay_ticks: 3,
    anim_slot: "cast_finish",
    range: 0,
    cast_buff: QUICKEN_BUFFS,
    ..DEFAULT_SKILL
};

const QUICKEN_BUFFS: &[BuffSpec] = &[
    BuffSpec { kind: BuffKind::AspdPct, magnitude: 3_000, per_level: 200, duration: 60 },
];

/// Thunderfall — her ULTIMATE: 400% ATK +30%/lv of Thunder on every enemy on the board, and
/// every Static Spark on every one of them goes off at once. Gated by the Limit Break gauge.
pub const THUNDERFALL: SkillDef = SkillDef {
    id: "thunderfall",
    kind: SkillKind::Active,
    max_level: 10,
    loadout_slots: 2,
    lb_max: 1_000,
    lb_on_skill: 12,
    lb_on_basic: 4,
    lb_on_hit_taken: 0,
    lb_per_second: 2,
    motion_delay_ticks: 8,
    anim_slot: "special_2",
    range: 1,
    zone_wide: true,
    power_bp: 50_000,
    power_per_level_bp: 3_750,
    cast_element: Some(Element::Thunder),
    damage_type: Some(DamageType::Magic),
    spark_detonate: true,
    ..DEFAULT_SKILL
};
