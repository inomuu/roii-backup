//! Zolfia — SSR · Destroyer · Ice · sword and shield (0.0.476, user 2026-09-24). The headless
//! knight of Duskmourn — the Dulahaness boss, won to the party — who carries her own head and
//! burns with a cold blue flame. Her kit is built for the party, not the boss fight she was:
//! everything she strikes is chilled, and a chilled enemy that breaks — whoever breaks it —
//! shatters, taking more Ice and Fire from everyone.

use super::super::{BuffKind, BuffSpec, DEFAULT_SKILL, SkillDef, SkillKind};
use crate::grid::{DamageType, Element};

/// Coldflame Knight — her PASSIVE: her basic strikes for 100% ATK of Ice, and every hit she lands lays Winter
/// Chill for 8 s. A chilled enemy that BREAKS (from anyone) is Shattered: +20% Ice and Fire
/// damage taken for the break's stun plus 5 s. Resolved in `tick::winter_pass`.
pub const COLDFLAME_KNIGHT: SkillDef = SkillDef {
    id: "coldflame_knight",
    kind: SkillKind::Passive,
    max_level: 1,
    basic_element: Some(Element::Ice),
    basic_damage_type: Some(DamageType::Physical),
    winter_chill_ticks: 80,
    shatter_bp: 2_000,
    shatter_extra_ticks: 50,
    ..DEFAULT_SKILL
};

/// Coldflame Split — Skill 1: a heavy blow for 350% ATK +35%/lv of Ice at triple Toughness
/// damage — the one that breaks, and so the one that shatters. Every 10 s.
pub const COLDFLAME_SPLIT: SkillDef = SkillDef {
    id: "coldflame_split",
    kind: SkillKind::Active,
    max_level: 10,
    loadout_slots: 2,
    cooldown_ticks: 100,
    motion_delay_ticks: 5,
    anim_slot: "special_1",
    range: 1,
    power_bp: 35_000,
    power_per_level_bp: 3_500,
    cast_element: Some(Element::Ice),
    damage_type: Some(DamageType::Physical),
    toughness_dmg_mult: 300,
    ..DEFAULT_SKILL
};

/// Remorseless Winter — Skill 2: an aura around her for 8 s. Every second each enemy within 2
/// cells is slowed (ATK speed −20% −1%/lv) and takes 45% +4.5%/lv of her ATK as Ice. Every 14 s.
/// Resolved in `tick::winter_aura`.
pub const REMORSELESS_WINTER: SkillDef = SkillDef {
    id: "remorseless_winter",
    kind: SkillKind::Active,
    max_level: 10,
    loadout_slots: 1,
    cooldown_ticks: 140,
    motion_delay_ticks: 3,
    anim_slot: "cast_finish",
    range: 0,
    cast_buff: REMORSELESS_WINTER_BUFFS,
    aura_radius: 2,
    aura_dot_atk_bp: 4_500,
    aura_dot_per_level_bp: 450,
    aura_aspd_slow_bp: 2_000,
    aura_aspd_slow_per_level_bp: 100,
    ..DEFAULT_SKILL
};

const REMORSELESS_WINTER_BUFFS: &[BuffSpec] = &[
    BuffSpec { kind: BuffKind::WinterAura, magnitude: 1, per_level: 0, duration: 80 },
];

/// Winter's Verdict — her ULTIMATE: she drives the sword into the ground — 500% ATK +50%/lv of
/// Ice on every enemy on the board at double Toughness damage; each is chilled by the blow.
/// Gated by the Limit Break gauge.
pub const WINTERS_VERDICT: SkillDef = SkillDef {
    id: "winters_verdict",
    kind: SkillKind::Active,
    max_level: 10,
    loadout_slots: 2,
    lb_max: 1_000,
    lb_on_basic: 2,
    lb_on_skill: 10,
    lb_per_second: 2,
    room_nuke: true,
    motion_delay_ticks: 10,
    anim_slot: "special_2",
    range: 99,
    power_bp: 50_000,
    power_per_level_bp: 5_000,
    cast_element: Some(Element::Ice),
    damage_type: Some(DamageType::Physical),
    toughness_dmg_mult: 200,
    ..DEFAULT_SKILL
};
