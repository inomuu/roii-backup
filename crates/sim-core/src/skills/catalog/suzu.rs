//! Suzu — SR · Destroyer · Fire · sword and shield (0.0.465, user 2026-09-22). The small loud
//! one: she cracks guards for a living and is delighted about it. Where Charon seals ONE enemy
//! and grinds it down whatever its element, Suzu sweeps a crowd's bars at once — and for the
//! fifteen seconds of her ultimate she can break anything at all.
//!
//! SR rule (memory `sr-vs-ssr-rule`): S0 ≈ 65% of Charon's S0 on the party bench, S6 ≈ 95%.

use super::super::{BuffKind, BuffSpec, DEFAULT_SKILL, DebuffSpec, SkillDef, SkillKind};
use crate::grid::{DamageType, Element};

/// Bright Edge — her PASSIVE basic override: a plain 110% ATK Fire swing. Everything special
/// about her is a skill (user: "passive โจมตีธรรมดา").
pub const BRIGHT_EDGE: SkillDef = SkillDef {
    id: "bright_edge",
    kind: SkillKind::Passive,
    max_level: 1,
    basic_element: Some(Element::Fire),
    basic_damage_type: Some(DamageType::Physical),
    basic_power_bp: 11_000,
    basic_range: 1,
    ..DEFAULT_SKILL
};

/// Sunder — Skill 1: a 3×3 sweep for 150% ATK +15%/lv of Fire at double Toughness damage that
/// leaves SUNDER on everything it touches: −20% DEF for 8 s. Every 8 s.
pub const SUNDER: SkillDef = SkillDef {
    id: "sunder",
    kind: SkillKind::Active,
    max_level: 10,
    loadout_slots: 2,
    cooldown_ticks: 80,
    motion_delay_ticks: 5,
    anim_slot: "special_1",
    range: 1,
    self_aoe_radius: 1,
    power_bp: 15_000,
    power_per_level_bp: 1_500,
    cast_element: Some(Element::Fire),
    damage_type: Some(DamageType::Physical),
    toughness_dmg_mult: 200,
    on_hit_debuff: Some(DebuffSpec {
        kind: BuffKind::Sunder,
        magnitude: 20,
        per_level: 0,
        duration: 80,
        chance_bp: 0,
        chance_per_level: 0,
    }),
    ..DEFAULT_SKILL
};

/// Bell Strike — Skill 2: one heavy blow for 190% ATK +19%/lv of Fire at double Toughness
/// damage. Every 12 s — but its cooldown CLEARS the moment any enemy's Toughness breaks, whoever
/// broke it (user: "รีเซ็ต cd เมื่อมีตัว break"), so a party that breaks often keeps her swinging.
pub const BELL_STRIKE: SkillDef = SkillDef {
    id: "bell_strike",
    kind: SkillKind::Active,
    max_level: 10,
    loadout_slots: 1,
    cooldown_ticks: 120,
    motion_delay_ticks: 5,
    anim_slot: "special_1",
    range: 1,
    power_bp: 19_000,
    power_per_level_bp: 1_900,
    cast_element: Some(Element::Fire),
    damage_type: Some(DamageType::Physical),
    toughness_dmg_mult: 200,
    reset_on_break: true,
    ..DEFAULT_SKILL
};

/// Break Fever — her ULTIMATE: no damage of its own. For 100 s she swings 40% faster, her BASIC
/// chips Toughness at double, and the weakness gate is off — she can break ANY enemy, element
/// or not. Gated by the Limit Break gauge, which fills mostly from breaks — 5 a break rather
/// than 80 (0.0.469: at 80 she cast it 91 times an hour, which is a cut-in every 40 seconds;
/// the same uptime now costs ~15 casts).
pub const BREAK_FEVER: SkillDef = SkillDef {
    id: "break_fever",
    kind: SkillKind::Active,
    max_level: 10,
    loadout_slots: 2,
    lb_max: 2_000,
    lb_on_skill: 10,
    lb_on_basic: 3,
    lb_on_break: 5,
    lb_on_hit_taken: 0,
    lb_per_second: 2,
    motion_delay_ticks: 4,
    anim_slot: "special_2",
    range: 0,
    cast_buff: BREAK_FEVER_BUFFS,
    ..DEFAULT_SKILL
};

const BREAK_FEVER_BUFFS: &[BuffSpec] = &[
    BuffSpec { kind: BuffKind::AspdPct, magnitude: 4_000, per_level: 0, duration: 1_000 },
    // The magnitude IS the basic's Toughness multiplier while it lasts (200 = double).
    BuffSpec { kind: BuffKind::BreakFever, magnitude: 200, per_level: 0, duration: 1_000 },
];
