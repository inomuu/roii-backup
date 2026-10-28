//! Sylvie — SR · Support · Thunder · staff (0.0.473, user 2026-09-24). Easygoing to a fault,
//! forgets everything, smiles through all of it as if nothing could ever be too hot or too cold.
//! A pure buffer (user: no heal): crit and attack speed for the party, crit damage and ATK for
//! its carry. Every hit crits since 0.0.472, so her layer reaches magic and skills alike.
//!
//! SR rule (memory `sr-vs-ssr-rule`): S0 ≈ 65% of the SSR support's S0 on the party bench,
//! S6 ≈ 95%.

use super::super::{BuffKind, BuffSpec, DEFAULT_SKILL, SkillDef, SkillKind};
use crate::grid::{DamageType, Element};

/// Spark Tap — her PASSIVE basic override: a 60% ATK Thunder bolt at range 4. Nothing else.
pub const SPARK_TAP: SkillDef = SkillDef {
    id: "spark_tap",
    kind: SkillKind::Passive,
    max_level: 1,
    basic_element: Some(Element::Thunder),
    basic_damage_type: Some(DamageType::Magic),
    basic_power_bp: 6_000,
    basic_range: 4,
    basic_ranged: true,
    ..DEFAULT_SKILL
};

/// Surge — Skill 1: for 10 s the whole party gains CRIT +15% +1%/lv and attack speed +10%
/// +1%/lv — Thunder allies half as much attack speed again (0.0.474, user: the Thunder crew's
/// synergy). Every 16 s. Her ladder's crit riders ride this buff's duration.
pub const SURGE: SkillDef = SkillDef {
    id: "surge",
    kind: SkillKind::Active,
    max_level: 10,
    loadout_slots: 2,
    cooldown_ticks: 160,
    motion_delay_ticks: 5,
    anim_slot: "cast_finish",
    range: 0,
    cast_buff: SURGE_BUFFS,
    cast_buff_party: true,
    aspd_kin_element: Some(Element::Thunder),
    aspd_kin_mult_bp: 15_000,
    ..DEFAULT_SKILL
};

const SURGE_BUFFS: &[BuffSpec] = &[
    BuffSpec { kind: BuffKind::CritBpAdd, magnitude: 1_500, per_level: 100, duration: 100 },
    BuffSpec { kind: BuffKind::AspdPct, magnitude: 1_000, per_level: 100, duration: 100 },
];

/// Arc Focus — Skill 2: the ally with the highest ATK gains CRIT DAMAGE +50% +5%/lv and ATK
/// +15% +1.5%/lv for 8 s. Every 14 s.
pub const ARC_FOCUS: SkillDef = SkillDef {
    id: "arc_focus",
    kind: SkillKind::Active,
    max_level: 10,
    loadout_slots: 1,
    cooldown_ticks: 140,
    motion_delay_ticks: 3,
    anim_slot: "cast_finish",
    range: 0,
    cast_buff: ARC_FOCUS_BUFFS,
    buff_top_atk_allies: 1,
    ..DEFAULT_SKILL
};

const ARC_FOCUS_BUFFS: &[BuffSpec] = &[
    BuffSpec { kind: BuffKind::CritDmgBpAdd, magnitude: 5_000, per_level: 500, duration: 80 },
    BuffSpec { kind: BuffKind::AtkPct, magnitude: 1_500, per_level: 150, duration: 80 },
];

/// Bolt from the Blue — her ULTIMATE: the whole party gains CRIT +25% for 15 s, and every
/// ally's skill cooldowns drop by 3 s at once. Gated by the Limit Break gauge.
pub const BOLT_FROM_THE_BLUE: SkillDef = SkillDef {
    id: "bolt_from_the_blue",
    kind: SkillKind::Active,
    max_level: 10,
    loadout_slots: 2,
    lb_max: 1_000,
    lb_on_basic: 2,
    lb_on_skill: 10,
    lb_per_second: 2,
    motion_delay_ticks: 5,
    anim_slot: "special_2",
    range: 0,
    cast_buff: BOLT_BUFFS,
    cast_buff_party: true,
    party_cooldown_cut_ticks: 30,
    ..DEFAULT_SKILL
};

const BOLT_BUFFS: &[BuffSpec] = &[
    BuffSpec { kind: BuffKind::CritBpAdd, magnitude: 2_500, per_level: 100, duration: 150 },
];
