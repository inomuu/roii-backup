//! Charon — SSR · Destroyer · Dark · mace and trinket (0.0.463, user 2026-09-22; design:
//! `docs/character-design-charon.md`). The ferryman of Tartarus: the first Destroyer — she is
//! not here to out-damage a Striker, she is here to BREAK. Every swing of the oar chips the
//! gauge harder than anyone's, the seal she lays on one enemy lets her crack it whatever its
//! element, and a broken target is where her damage lives.
//!
//! Numbers sized against Rapis / Nyx S0 on the party bench (`docs/exemplar-roster.md` §0):
//! ≈ equal on a boss she can keep broken, below them on a crowd.

use super::super::{BuffKind, DEFAULT_SKILL, DebuffSpec, SkillDef, SkillKind};
use crate::grid::{DamageType, Element};

/// Ferryman's Toll — her PASSIVE basic override: an 85% ATK Dark swing of the oar that chips
/// Toughness at TWICE the baseline; every hit she lands on a BROKEN target deals +30%.
pub const FERRYMANS_TOLL: SkillDef = SkillDef {
    id: "ferrymans_toll",
    kind: SkillKind::Passive,
    max_level: 1,
    basic_element: Some(Element::Dark),
    basic_damage_type: Some(DamageType::Physical),
    basic_power_bp: 8_500,
    basic_range: 1,
    basic_toughness_mult: 200,
    broken_target_bp: 3_000,
    ..DEFAULT_SKILL
};

/// Tolling Blow — Skill 1: the oar hits the ground — 100% ATK +10%/lv of Dark on the 3×3
/// around her, triple Toughness damage. Every 8 s.
pub const TOLLING_BLOW: SkillDef = SkillDef {
    id: "tolling_blow",
    kind: SkillKind::Active,
    max_level: 10,
    loadout_slots: 2,
    cooldown_ticks: 80,
    motion_delay_ticks: 6,
    anim_slot: "special_1",
    range: 1,
    self_aoe_radius: 1,
    power_bp: 10_000,
    power_per_level_bp: 1_000,
    cast_element: Some(Element::Dark),
    damage_type: Some(DamageType::Physical),
    toughness_dmg_mult: 300,
    ..DEFAULT_SKILL
};

/// Blackwater Mark — Skill 2: a seal on ONE enemy — the boss in range first, else the biggest —
/// for 120% ATK +12%/lv of Dark and 10 s of the mark: HER hits chip its Toughness for +50%
/// build-up when it is weak to Dark, and at half when it is not — a bar nobody else could touch.
/// Every 12 s. (User: "basic 40 → ไม่แพ้+mark 20, แพ้+mark 60".)
pub const BLACKWATER_MARK: SkillDef = SkillDef {
    id: "blackwater_mark",
    kind: SkillKind::Active,
    max_level: 10,
    loadout_slots: 1,
    cooldown_ticks: 120,
    motion_delay_ticks: 5,
    anim_slot: "special_1",
    range: 2,
    target_boss_first: true,
    power_bp: 12_000,
    power_per_level_bp: 1_200,
    cast_element: Some(Element::Dark),
    damage_type: Some(DamageType::Physical),
    on_hit_debuff: Some(DebuffSpec {
        kind: BuffKind::BlackwaterMark,
        magnitude: 5_000,
        per_level: 0,
        duration: 100,
        chance_bp: 0,
        chance_per_level: 0,
    }),
    ..DEFAULT_SKILL
};

/// Passage of No Return — her ULTIMATE: the shadow gate opens behind her and the oar sweeps
/// the 5×5 for 400% ATK +40%/lv of Dark at five times the Toughness damage. The gauge fills
/// from BREAKS (`lb_on_break`) more than from anything else.
pub const PASSAGE_OF_NO_RETURN: SkillDef = SkillDef {
    id: "passage_of_no_return",
    kind: SkillKind::Active,
    max_level: 10,
    loadout_slots: 2,
    lb_max: 1_000,
    lb_on_skill: 10,
    lb_on_basic: 3,
    lb_on_break: 60,
    lb_on_hit_taken: 0,
    lb_per_second: 2,
    motion_delay_ticks: 8,
    anim_slot: "special_2",
    range: 2,
    self_aoe_radius: 2,
    power_bp: 40_000,
    power_per_level_bp: 4_000,
    cast_element: Some(Element::Dark),
    damage_type: Some(DamageType::Physical),
    toughness_dmg_mult: 500,
    ..DEFAULT_SKILL
};
