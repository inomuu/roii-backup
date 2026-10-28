//! Nemu — SSR · Striker · Dark · staff, range 6 (0.0.451, user 2026-09-21). The succubus of
//! the roster: everything she touches weakens, slows and feeds her. Void/gravity in the ult —
//! Enigma's Black Hole: the board is drawn in over three seconds, then the hole closes on it.
//!
//! Numbers were set against Rapis on the bench (`docs/exemplar-roster.md` §0): worse than her
//! on one target, better on a crowd, ≈ equal in the standard party.

use super::super::{BuffKind, DEFAULT_SKILL, DebuffSpec, SkillDef, SkillKind};
use crate::grid::{DamageType, Element};

/// Kiss of Ruin — her PASSIVE basic override + SWEET POISON. Her basic is a 90% ATK Dark bolt
/// at range 6, and every hit she LANDS (basic, Bog tick, Heartstring, the Embrace) stacks Sweet
/// Poison on the target: +3% Dark damage taken, −3% move speed, −5% attack speed per stack,
/// ten deep, 6 s, refreshed by each hit. Rides the Umbral Mark machinery (`mark_*`).
pub const SWEET_POISON: SkillDef = SkillDef {
    id: "sweet_poison",
    kind: SkillKind::Passive,
    max_level: 1,
    basic_element: Some(Element::Dark),
    basic_damage_type: Some(DamageType::Magic),
    basic_power_bp: 9_000,
    basic_range: 6,
    basic_ranged: true,
    mark_step_bp: 300,
    mark_max_stacks: 10,
    mark_ticks: 60,
    mark_slow_pct: 3,
    mark_aspd_slow_bp: 500,
    ..DEFAULT_SKILL
};

/// Velvet Bog — Skill 1: a 3×3 pool on the densest knot for 6 s that SLOWS 50% and burns
/// 25% ATK +2.5%/lv every half-second (12 waves = 300% +30%/lv over its life). Every 10 s.
pub const VELVET_BOG: SkillDef = SkillDef {
    id: "velvet_bog",
    kind: SkillKind::Active,
    max_level: 10,
    loadout_slots: 2,
    cooldown_ticks: 100,
    motion_delay_ticks: 5,
    cast_time_ticks: 10,
    range: 6,
    ground_field_radius: 1,
    field_place_densest: true,
    field_duration_ticks: 60,
    field_hits: 12,
    field_power_from_atk: true,
    power_bp: 2_500,
    power_per_level_bp: 250,
    cast_element: Some(Element::Dark),
    damage_type: Some(DamageType::Magic),
    on_hit_debuff: Some(DebuffSpec {
        kind: BuffKind::AgiDexSlowPct,
        magnitude: 50,
        per_level: 0,
        duration: 10,
        chance_bp: 10_000,
        chance_per_level: 0,
    }),
    anim_slot: "cast_finish",
    ..DEFAULT_SKILL
};

/// Heartstring — Skill 2: a tether to one target within 6 for 60% ATK +6%/lv of Dark per second
/// for 6 s (360% +36%/lv); every tick heals Nemu for `tick::LEECH_BP` (10%) of it. The
/// initial touch is 50% ATK. Only cast on a target not already tethered. Every 12 s.
pub const HEARTSTRING: SkillDef = SkillDef {
    id: "heartstring",
    kind: SkillKind::Active,
    max_level: 10,
    loadout_slots: 2,
    cooldown_ticks: 120,
    motion_delay_ticks: 5,
    power_bp: 5_000,
    power_per_level_bp: 500,
    range: 6,
    damage_type: Some(DamageType::Magic),
    cast_element: Some(Element::Dark),
    on_hit_debuff: Some(DebuffSpec {
        kind: BuffKind::LeechBleed,
        magnitude: 36_000,
        per_level: 3_600,
        duration: 60,
        chance_bp: 0,
        chance_per_level: 0,
    }),
    leech_bp: 1_000,
    anim_slot: "special_1",
    ..DEFAULT_SKILL
};

/// Abyssal Embrace — her ULTIMATE, Enigma's Black Hole: a 3×3 hole on the densest knot that
/// for 3 s hauls every enemy within reach one cell toward its centre every half-second and
/// holds them there (Root); when it CLOSES it detonates for 1,800% ATK +180%/lv of Dark on the
/// 5×5 around it. Gated by the Limit Break gauge.
pub const ABYSSAL_EMBRACE: SkillDef = SkillDef {
    id: "abyssal_embrace",
    kind: SkillKind::Active,
    max_level: 10,
    loadout_slots: 2,
    lb_max: 1_000,
    lb_on_skill: 12,
    lb_on_basic: 4,
    lb_on_hit_taken: 0,
    lb_per_second: 2,
    motion_delay_ticks: 8,
    cast_time_ticks: 10,
    range: 8,
    ground_field_radius: 2,
    field_place_densest: true,
    field_duration_ticks: 30,
    field_hits: 6,
    field_fuse: true,
    field_pull: 1,
    field_power_from_atk: true,
    power_bp: 180_000,
    power_per_level_bp: 18_000,
    cast_element: Some(Element::Dark),
    damage_type: Some(DamageType::Magic),
    anim_slot: "special_2",
    ..DEFAULT_SKILL
};
