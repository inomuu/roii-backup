//! Airi — SR · Striker · Thunder (Lightning) · bow and quiver (0.0.462, user 2026-09-22: "นักธนู
//! ธาตุสายฟ้า เป็นคู่หูกับ Lumi SR เหมือนกัน … cocky overconfident"). Lumi's partner and her
//! opposite: loud where Lumi is quiet, and always sure the next arrow lands. A plain SR kit by
//! design — a static-charging basic, a bolt that chains, an overload of her own hands, and a
//! storm on five heads.
//!
//! SR rule (memory `sr-vs-ssr-rule`): S0 ≈ 65% of an SSR Striker's S0 on the party bench,
//! S6 ≈ 95%. Built from what exists: the mark engine (now per element — `StaticMark`), the
//! volley path, a self `AspdPct` cast buff, the gauge.

use super::super::{BuffKind, BuffSpec, DEFAULT_SKILL, SkillDef, SkillKind};
use crate::grid::{DamageType, Element};

/// Static Shot — her PASSIVE basic override + STATIC. Her basic is a 70% ATK Thunder arrow at
/// range 6, and every hit she lands stacks Static on the target: +2% Thunder damage taken per
/// stack, three deep, 8 s, refreshed by each hit. Every hit she lands — basic and every skill —
/// on a TAUNTED target deals +20% (user: the pair with Lumi — she holds them, Airi hits what is
/// held).
pub const STATIC_SHOT: SkillDef = SkillDef {
    id: "static_shot",
    kind: SkillKind::Passive,
    max_level: 1,
    basic_element: Some(Element::Thunder),
    basic_power_bp: 7_000, // 0.0.462 (user: "เดเมจยังน้อยไปหน่อย"): 60 → 70%
    basic_range: 6,
    basic_ranged: true,
    mark_step_bp: 200,
    mark_max_stacks: 3,
    mark_ticks: 80,
    taunted_target_bp: 2_000,
    ..DEFAULT_SKILL
};

/// Chain Bolt — Skill 1: one arrow that jumps — 90% ATK +9%/lv of Thunder on each of the three
/// lowest-HP enemies in range 6. Every 10 s. No Toughness bonus: Striker, not Destroyer (user).
pub const CHAIN_BOLT: SkillDef = SkillDef {
    id: "chain_bolt",
    kind: SkillKind::Active,
    max_level: 10,
    loadout_slots: 2,
    cooldown_ticks: 100,
    motion_delay_ticks: 5,
    anim_slot: "special_1",
    power_bp: 9_000,
    power_per_level_bp: 900,
    range: 6,
    volley_targets: 3,
    cast_element: Some(Element::Thunder),
    damage_type: Some(DamageType::Physical),
    ..DEFAULT_SKILL
};

/// Overload — Skill 2: she overcharges her own hands — ATK speed +30% +2%/lv for 6 s. Every
/// 14 s. A buff and nothing else (user: keep the SR plain).
pub const OVERLOAD: SkillDef = SkillDef {
    id: "overload",
    kind: SkillKind::Active,
    max_level: 10,
    loadout_slots: 1,
    cooldown_ticks: 140,
    motion_delay_ticks: 3,
    anim_slot: "cast_finish",
    range: 0,
    cast_buff: OVERLOAD_BUFFS,
    ..DEFAULT_SKILL
};

const OVERLOAD_BUFFS: &[BuffSpec] = &[
    BuffSpec { kind: BuffKind::AspdPct, magnitude: 3_000, per_level: 200, duration: 60 },
];

/// Storm Volley — her ULTIMATE: 350% ATK +35%/lv of Thunder on each of the five lowest-HP
/// enemies in range 8. Gated by the Limit Break gauge.
pub const STORM_VOLLEY: SkillDef = SkillDef {
    id: "storm_volley",
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
    power_bp: 35_000,
    power_per_level_bp: 3_500,
    range: 8,
    volley_targets: 5,
    cast_element: Some(Element::Thunder),
    damage_type: Some(DamageType::Physical),
    ..DEFAULT_SKILL
};
