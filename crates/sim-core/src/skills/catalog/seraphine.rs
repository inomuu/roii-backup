//! Seraphine — Swordsman · Physical · Sentinel. The first kit written against the
//! new design (`docs/exemplar-roster.md` §8), prototyped on the CURRENT engine so
//! the pull can be measured before the stat rewrite lands. Everything here is
//! deliberately built out of machinery that already exists: taunt, self-AoE, a
//! DefPct buff, and the absorb-shield engine.
//!
//! `toughness_dmg_mult` on Steel Break is inert until the Toughness gauge exists —
//! it is written down now so the skill that is meant to break the gauge says so.

use super::super::{BuffKind, BuffSpec, DEFAULT_SKILL, SkillDef, SkillKind};
use crate::grid::DamageType;

/// Iron Chains — she throws the chains out across a 7×7 and hauls the TWO
/// nearest enemies in the ring to her feet (0.0.427, user: "target up to 2"; it
/// used to take the room and was 5-16% of the party's damage on its own):
/// damage to each, both locked onto her for 6 s, and her own guard raised while
/// she holds them. The pull walks the
/// same path a shove does (`displace_from`), but is its own field and its own
/// event — nothing that watches for a knockback should see a haul-in.
pub const IRON_CHAINS: SkillDef = SkillDef {
    id: "iron_chains",
    kind: SkillKind::Active,
    max_level: 10,
    loadout_slots: 2,
    cooldown_ticks: 120,        // 12 s — a gather, not a rotation filler
    motion_delay_ticks: 5,
    anim_slot: "special_1",
    range: 3,                   // cast when something is inside the ring
    self_aoe_radius: 3,         // 7×7 around her
    self_aoe_max_targets: 2,    // the two FARTHEST in the ring (a pull; 0.0.482), not the room
    pull: 3,                    // hauls every victim 3 cells toward her
    taunt_on_hit_ticks: 60,     // 6 s locked onto her
    power_bp: 10_000,           // 100% ATK at Lv1 → Lv10 190% (0.0.439: Seraphine nerf)
    power_per_level_bp: 1_000,
    damage_type: Some(DamageType::Physical),
    cast_buff: IRON_CHAINS_BUFFS,
    ..DEFAULT_SKILL
};

/// +20% DEF at Lv1, +3%/level, for 10 s — live while the chains hold. Basis points: `DefPct`
/// folds beside gear DEF% in `effective_hard_def` (0.0.457 — it was authored as 20 and landed
/// as 0.2% from the day the chains were written).
const IRON_CHAINS_BUFFS: &[BuffSpec] = &[
    BuffSpec { kind: BuffKind::DefPct, magnitude: 2_000, per_level: 300, duration: 100 },
];

/// Steel Break — the sweep she pays the gather off with: a 3×3 around her, and
/// the skill carrying the Toughness-damage bonus once that gauge exists. Short
/// cooldown, because breaking is meant to be her rhythm rather than her finisher.
pub const STEEL_BREAK: SkillDef = SkillDef {
    id: "steel_break",
    kind: SkillKind::Active,
    max_level: 10,
    loadout_slots: 1,
    cooldown_ticks: 80,         // 8 s (5 s until 0.0.425)
    motion_delay_ticks: 5,
    anim_slot: "special_1",
    range: 1,
    self_aoe_radius: 1,         // 3×3 around her
    // 0.0.425 (user: "nerf steel break"): 80% ATK +8%/lv (was 160% +16%): with Iron Chains
    // piling the room onto her, this 3×3 every 5 s was 16% of the whole party's damage and
    // she dealt 46-52% of it as the tank. The break bonus below is untouched — that is the job.
    power_bp: 9_000,            // 90% +9%/lv (0.0.439)
    power_per_level_bp: 900,
    damage_type: Some(DamageType::Physical),
    toughness_dmg_mult: 300,    // triple Toughness damage — INERT until the gauge lands
    ..DEFAULT_SKILL
};

/// Aegis — her ultimate: an absorb shield on EVERY member at once, sized from
/// her own MaxHP, and when a member's shield breaks it bursts. Cast through
/// `try_cast_aegis`, which extends the Praesidium path from one ally to the
/// whole party; the shield fields themselves are the existing `AbsorbShield`.
///
/// Gated by her Limit Break gauge, which fills mostly from damage she TAKES —
/// the loop closes on itself: she hauls the room onto herself, the beating fills
/// the gauge, and the gauge pays the party back with a shield.
pub const AEGIS: SkillDef = SkillDef {
    id: "aegis",
    kind: SkillKind::Active,
    max_level: 10,
    loadout_slots: 2,
    // No cooldown: an Ultimate is gated by its gauge, not by a clock.
    lb_max: 1_000,
    lb_on_hit_taken: 80, // 0.0.460: the gauge fills from what she TAKES (a full-size blow = 8%)
    lb_on_basic: 2,
    lb_on_skill: 10,
    lb_per_second: 2,
    motion_delay_ticks: 5,
    anim_slot: "special_1",
    range: 9,
    ..DEFAULT_SKILL
};

/// Bulwark — flat hard DEF for every enemy she currently holds under her own
/// Taunt, `2 × level` each, capped in `skills::BULWARK_DEF_CAP`. The gather and
/// the guard are the same act: the more she has pulled in, the harder she is to
/// move. Folded in `combat::effective_hard_def`.
pub const BULWARK: SkillDef = SkillDef {
    id: "bulwark",
    kind: SkillKind::Passive,
    max_level: 10,
    // Her BASIC swings at 70% ATK (0.0.439, user: "ปรับ basic attack เป็น 70% atk แทน" — a flat
    // ATK cut would have shrunk Aegis and her skills too). Skills read full ATK.
    basic_power_bp: 7_000,
    ..DEFAULT_SKILL
};
