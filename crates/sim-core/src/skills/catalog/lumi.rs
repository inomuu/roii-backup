//! Lumi — SR · Sentinel · Ice (Water) · sword and shield (0.0.456, user 2026-09-21: "Sentinel
//! ธาตุน้ำ … SR ถือเกราะโล่ เป็นสาวน้อยขี้ประหม่า"). The timid one: the cold leaks out of her when
//! anyone gets close. A plain SR kit by design (user: "ธรรมดาหน่อย ค่อยไปเสริมให้ S1-S6") —
//! a chill basic, a wide taunt, a self-heal, and an ultimate that hardens the party and
//! chills whoever strikes it.
//!
//! SR rule (memory `sr-vs-ssr-rule`): S0 ≈ 65% of Seraphine's S0 on the bench, S6 ≈ 95%.

use super::super::{BuffKind, BuffSpec, DEFAULT_SKILL, SkillDef, SkillKind};
use crate::grid::{DamageType, Element};

/// Chill Touch — her PASSIVE basic override + CHILL. Her basic is a 55% ATK Ice poke, and every
/// hit she lands stacks Chill on the target: +2% Ice AND Thunder damage taken (ice conducts —
/// user: "Lumi debuff + dmg สายฟ้าด้วย"), −4% move speed, −4% attack speed per stack, three deep
/// (user: five was too much at S0), 6 s, refreshed by each hit. Frost Aura (her ultimate) stamps the
/// same Chill on anything that strikes the party.
pub const CHILL_TOUCH: SkillDef = SkillDef {
    id: "chill_touch",
    kind: SkillKind::Passive,
    max_level: 1,
    basic_element: Some(Element::Ice),
    basic_damage_type: Some(DamageType::Physical),
    basic_power_bp: 5_500,
    basic_range: 1,
    mark_step_bp: 200,
    mark_max_stacks: 3,
    mark_ticks: 60,
    mark_slow_pct: 4,
    mark_aspd_slow_bp: 400,
    ..DEFAULT_SKILL
};

/// Frost Taunt — Skill 1: a 5×5 around her for 80% ATK +8%/lv of Ice; everything hit is
/// taunted onto her for 5 s. Every 10 s. Damage and the taunt, nothing else (user).
pub const FROST_TAUNT: SkillDef = SkillDef {
    id: "frost_taunt",
    kind: SkillKind::Active,
    max_level: 10,
    loadout_slots: 2,
    cooldown_ticks: 100,
    motion_delay_ticks: 5,
    anim_slot: "special_1",
    range: 2,
    self_aoe_radius: 2,
    taunt_on_hit_ticks: 50,
    power_bp: 8_000,
    power_per_level_bp: 800,
    cast_element: Some(Element::Ice),
    damage_type: Some(DamageType::Physical),
    ..DEFAULT_SKILL
};

/// Ice Mend — Skill 2: she heals HERSELF for 10% +1%/lv of her max HP, when she is under the
/// support path's 70% line. Every 12 s.
pub const ICE_MEND: SkillDef = SkillDef {
    id: "ice_mend",
    kind: SkillKind::Active,
    max_level: 10,
    loadout_slots: 1,
    cooldown_ticks: 120,
    motion_delay_ticks: 5,
    anim_slot: "cast_finish",
    range: 0,
    heals_ally: true,
    heal_self_only: true,
    heal_maxhp_bp: 1_000,
    heal_maxhp_per_level_bp: 100,
    ..DEFAULT_SKILL
};

/// Frost Aura — her ULTIMATE: the whole party gains DEF +20% +2%/lv for 30 s, and for those
/// 30 s every enemy that lands a hit on a party member is stamped with her Chill
/// (`BuffKind::FrostAura`, resolved in `combat::apply_hit` → `stamp_mark` from her). Gated by
/// the Limit Break gauge; fixed magnitudes like Benedictio — no ATK, no cooldown.
pub const FROST_AURA: SkillDef = SkillDef {
    id: "frost_aura",
    kind: SkillKind::Active,
    max_level: 10,
    loadout_slots: 2,
    lb_max: 1_000,
    lb_on_hit_taken: 0,
    lb_on_basic: 2,
    lb_on_skill: 10,
    lb_per_second: 2,
    motion_delay_ticks: 5,
    anim_slot: "special_2",
    range: 0,
    cast_buff: FROST_AURA_BUFFS,
    cast_buff_party: true,
    ..DEFAULT_SKILL
};

const FROST_AURA_BUFFS: &[BuffSpec] = &[
    BuffSpec { kind: BuffKind::DefPct, magnitude: 2_000, per_level: 200, duration: 300 },
    BuffSpec { kind: BuffKind::FrostAura, magnitude: 1, per_level: 0, duration: 300 },
];
