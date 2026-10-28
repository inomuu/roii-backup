//! Rapis — Fire · Striker (AoE). `docs/exemplar-roster.md` §0 / §9. Fireball and
//! Flame Shield came over from the retired mage set when the class system was
//! removed; nothing else learns them.

use super::super::{DEFAULT_SKILL, SkillDef, SkillKind};
use crate::grid::{DamageType, Element};

/// Fireball — the mage's signature AoE nuke. A magic bolt (fires through
/// the projectile path) that detonates in a 3×3 blast on impact, hitting
/// every enemy in it. 200% ATK at level 1, +20%/level. Magic damage —
/// uses MDEF, never crits. SP-hungry with a real cooldown.
pub const FIREBALL: SkillDef = SkillDef {
    id: "fireball",
    kind: SkillKind::Active,
    max_level: 10,
    cooldown_ticks: 60,         // 6 s (0.0.428, was 10 s — Rapis was 12% of the party)
    power_bp: 60_000,           // 600% ATK +60%/lv (0.0.437: Rapis ×4)
    power_per_level_bp: 6_000,
    range: 6,
    aoe_radius: 1,
    cast_time_ticks: 25,
    motion_delay_ticks: 5, // 0.5 s act-lock -> "cast_finish" anim window (like acolyte)
    anim_slot: "cast_finish",
    ..DEFAULT_SKILL
};

/// Flame Shield — Elementalist self absorb-shield (active, max Lv10, cd 30s). Cast
/// on cooldown when the Elementalist has no shield up, it banks an absorb pool of
/// `INT × level × 2` damage (snapshot at cast) that soaks incoming phys + magic.
/// When the pool empties it EXPLODES: a 3×3 (radius 1) 100% ATK Fire blast on
/// surrounding enemies + a knockback of 1 (Lv1-3) / 2 (Lv4-6) / 3 (Lv7-10). A
/// MARKER active — the pool + explosion live on the Entity (`absorb_*`) + in
/// `tick::resolve_absorb_bursts`; shared absorb engine with Bishop Praesidium.
/// The 30 s cooldown starts when the shield BREAKS (not at cast), so there's real
/// downtime after the blast before it can be re-cast.
pub const FLAME_SHIELD: SkillDef = SkillDef {
    id: "flame_shield",
    kind: SkillKind::Active,
    max_level: 10,
    cooldown_ticks: 300, // 30 s
    cast_element: Some(Element::Fire),
    ..DEFAULT_SKILL
};

/// Ember Cascade — her passive and her whole identity. Whenever ANY enemy's
/// Toughness gauge breaks, from any source, she answers it with a Fireball at
/// that enemy. Her power therefore scales with the team's ability to break
/// gauges rather than with her own damage, which is what makes her and a
/// Sentinel who gathers a crowd worth fielding together.
///
/// The chain is deliberate: her Fireball chips Toughness like anything else and
/// can break the next enemy, which queues another. It paces itself — a break
/// refills the gauge, and the queue releases one shot every
/// `BREAK_TRIGGER_CD_TICKS`, so a room-wide break becomes a spread of fire rather
/// than one indivisible detonation.
pub const EMBER_CASCADE: SkillDef = SkillDef {
    id: "ember_cascade",
    kind: SkillKind::Passive,
    max_level: 10,
    // 40% ATK at Lv1 → Lv10 130% (0.0.428: back to the pre-halving line — this is her engine).
    // Below Fireball on purpose: this fires far more often than any cast.
    power_bp: 16_000,
    power_per_level_bp: 4_000,
    cast_element: Some(Element::Fire),
    damage_type: Some(DamageType::Magic),
    // Charging off a break is the Destroyer doctrine's own source. Rapis is a
    // Striker, so she may only have it because THIS passive grants it — the same
    // trigger it already answers, paid for with the skill slot it occupies.
    lb_on_break: 16,
    break_trigger: true,
    ..DEFAULT_SKILL
};

/// Fire Cat Meteor — her ultimate. Every living enemy in the room takes ONE
/// instance at the same moment, rather than a rain of separate meteors: a single
/// number on every enemy reads far better than a shower whose hits land out of
/// step, and it costs the sim one pass instead of a queue of projectiles.
///
/// Gated by her Limit Break gauge, which fills from Toughness breaks — the same
/// trigger Ember Cascade answers, so the character who makes gauges break also
/// reaches her ultimate faster. The cut-in and the cast bar are still client work.
pub const FIRE_CAT_METEOR: SkillDef = SkillDef {
    id: "fire_cat_meteor",
    kind: SkillKind::Active,
    max_level: 10,
    loadout_slots: 2,
    // No cooldown: an Ultimate is gated by its gauge, not by a clock.
    lb_max: 1_300,
    room_nuke: true,
    // 0.0.469: ~23 casts an hour → ~15, and the meteor pays 55% more to hold her damage.
    lb_on_basic: 2,
    lb_on_skill: 4,
    lb_per_second: 2,
    motion_delay_ticks: 10,
    anim_slot: "special_1",
    range: 99,                  // the whole room; the cast picks no single target
    power_bp: 182_000,           // 0.0.470: gauge 1,000 → 1,300 (19 → ~15 casts/h), power +30% back
    power_per_level_bp: 18_200,
    cast_element: Some(Element::Fire),
    damage_type: Some(DamageType::Magic),
    toughness_dmg_mult: 200,    // a room-wide blow leans on the gauge too
    ..DEFAULT_SKILL
};
