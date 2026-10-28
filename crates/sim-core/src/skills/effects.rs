//! Passive-skill folding helpers + Taunt/Endure tuning.
//!
//! These read an entity's `SkillState` list and sum the standing bonuses
//! the catalog's passives grant. The combat/tick layers call them when
//! computing effective stats or running the per-tick upkeep systems.

use super::{skill_def, SkillKind, SkillState};
use crate::items::WeaponType;

/// Taunt aggro reach (Chebyshev) — a 5×5 box around the caster.
pub const TAUNT_RADIUS: i32 = 2;

/// Aggro-lock DURATION in ticks — a fixed **40 (4 s)** at every level. How long
/// a taunted enemy stays locked onto the caster. Leveling Taunt shrinks the
/// COOLDOWN (`taunt_cooldown_ticks`), NOT this, so a higher level re-casts
/// before the lock lapses → less chance of losing aggro between casts.
pub const TAUNT_LOCK_TICKS: u32 = 40;

/// Taunt COOLDOWN in ticks by level: Lv1 = 100 (10 s) shrinking to Lv7 = 40
/// (4 s), 10 ticks/level. The lock lasts a fixed 40 ticks (4 s), so a LOW level
/// leaves an aggro gap (Lv1: 4 s locked, then ~6 s free before the next cast);
/// at Lv7 the cooldown equals the lock, so aggro is held continuously. This is
/// what leveling Taunt buys: reliability (smaller gap), not a longer lock.
pub fn taunt_cooldown_ticks(level: u8) -> u32 {
    (11 - level.clamp(1, 7) as u32) * 10
}

/// Endure (0.0.178 rework): flat VIT granted unconditionally, +1 per level.
/// Folded into `buffed_stats` (→ soft DEF, status math) AND `effective_max_hp`,
/// so the skill pays off even when NOT mobbed (design: 2026-07-08).
pub fn passive_vit_bonus(states: &[SkillState]) -> i32 {
    states.iter().filter(|s| s.id == "endure").map(|s| s.level as i32).sum()
}

/// Endure (0.0.178 rework): % of the mobbed-DEF penalty RESISTED = 5·level
/// (Lv10 halves the 5%/attacker penalty). This is the anti-mob identity: the
/// penalty punishes everyone standing in a swarm EXCEPT the swordsman who
/// trained for it. Consumed by `apply_damage` via `mobbed_def_resisted`.
pub fn endure_mob_resist_pct(level: u8) -> u32 {
    (level.min(10) as u32) * 5
}

/// Total party-aura ATK (Blessing) from a unit's PASSIVE skills, summed
/// as `party_atk_per_level · level`. Stamped onto every ally's
/// `Entity.aura_atk` by `tick::apply_party_auras`.
pub fn party_atk_bonus(states: &[SkillState]) -> i32 {
    party_atk_bonus_impl(states)
}

/// Slot cost of ONE loadout entry: every ACTIVE skill costs at least 1 loadout
/// slot (a big aura overrides higher via `loadout_slots`, e.g. Blessing = 2).
/// Passives never enter the loadout, so loadout entries always cost ≥ 1.
pub fn loadout_slot_cost(skill_id: &str) -> i32 {
    skill_def(skill_id).map_or(1, |d| d.loadout_slots.max(1))
}

/// True if the slotted active `skill_id` FITS within the entity's loadout
/// `capacity` (`combat::loadout_capacity`). Walks the loadout in order summing
/// `loadout_slot_cost`; an active that overflows the budget is INERT (won't
/// fire). Returns false if `skill_id` isn't in the loadout at all (so it
/// subsumes the old `skill_slotted` gate).
pub fn loadout_fits(active_skills: &[String], capacity: i32, skill_id: &str) -> bool {
    let mut used = 0;
    for id in active_skills {
        let cost = loadout_slot_cost(id);
        let fits = used + cost <= capacity;
        if fits {
            used += cost;
        }
        if id == skill_id {
            return fits;
        }
    }
    false
}

fn party_atk_bonus_impl(states: &[SkillState]) -> i32 {
    let mut bonus = 0i32;
    for s in states {
        if let Some(def) = skill_def(&s.id) {
            if matches!(def.kind, SkillKind::Passive) {
                bonus = bonus.saturating_add(def.party_atk_per_level * s.level as i32);
            }
        }
    }
    bonus
}

/// Total double-strike chance (basis points, capped at 100%) from all
/// PASSIVE skills an entity carries, summed as `double_atk_chance_bp ·
/// level`. 0 when the entity has no such passive — callers MUST skip the
/// PRNG roll entirely in that case to preserve the draw-order contract.
pub fn passive_double_atk_chance_bp(states: &[SkillState]) -> u32 {
    let mut bp = 0u32;
    for s in states {
        if let Some(def) = skill_def(&s.id) {
            if matches!(def.kind, SkillKind::Passive) {
                bp = bp.saturating_add(def.double_atk_chance_bp * s.level as u32);
            }
        }
    }
    bp.min(10_000)
}

/// Cleave (Swordmaster) splash factor (basis points): the fraction of a basic
/// hit's damage that splashes to enemies ORTHOGONALLY adjacent to the target —
/// `25% + 5%/level` (Lv1 = 3000, Lv10 = 7500). 0 when the entity lacks Cleave,
/// which gates the splash entirely (no extra work / PRNG → goldens safe). A
/// MARKER passive: the magnitude lives here, not on the SkillDef.
pub fn cleave_splash_bp(states: &[SkillState]) -> u32 {
    for s in states {
        if s.id == "cleave" && s.level > 0 {
            return 2500 + 500 * s.level as u32;
        }
    }
    0
}

/// Autocounter (Swordmaster) riposte chance (basis points): `5% + 1%/level`
/// (Lv1 = 600, Lv10 = 1500) when the defender knows Autocounter. 0 otherwise,
/// which gates the riposte entirely (no PRNG draw → goldens safe). The caller
/// also requires a shield equipped. A MARKER passive.
/// True when the entity knows the Bloodfrenzy passive (Gloomtooth): each landed basic
/// hit ramps its own ASPD (grown in `combat_step::try_bloodfrenzy`).
pub fn has_bloodfrenzy(states: &[SkillState]) -> bool {
    states.iter().any(|s| s.id == "bloodfrenzy" && s.level > 0)
}

/// True when the entity carries a vanish skill whose `move_while_hidden` flag is set (Twinling):
/// it may reposition while under a `Hiding` buff, instead of the thief's stand-still panic hide.
pub fn allows_move_while_hidden(states: &[SkillState]) -> bool {
    states
        .iter()
        .any(|s| s.level > 0 && super::skill_def(&s.id).map_or(false, |d| d.move_while_hidden))
}

pub fn autocounter_chance_bp(states: &[SkillState]) -> u32 {
    for s in states {
        if s.id == "autocounter" && s.level > 0 {
            return 500 + 100 * s.level as u32;
        }
    }
    0
}

/// Incantation Mastery cast-time reduction (basis points): `2%/level` (Lv10 =
/// 2000 = −20%) when known, applied on TOP of the DEX/INT variable-cast cut. 0
/// otherwise → goldens unaffected. A MARKER passive.
pub fn incantation_mastery_bp(states: &[SkillState]) -> u32 {
    for s in states {
        if s.id == "incantation_mastery" && s.level > 0 {
            return (200 * s.level as u32).min(10_000);
        }
    }
    0
}

/// Sword & Shield Mastery level (0 if unknown). Drives the shield-wielding bonus
/// (flat hard DEF + a touch of ASPD) applied in `combat::derived` while a shield
/// is equipped. A MARKER passive.
pub fn sword_shield_mastery_level(states: &[SkillState]) -> u32 {
    for s in states {
        if s.id == "sword_shield_mastery" && s.level > 0 {
            return s.level as u32;
        }
    }
    0
}

/// True if a flat-ATK passive applies given the wielded `weapon` type.
/// Weapon-mastery passives only fire for their weapon class; every other
/// passive (Keen Eye, …) is unconditional.
fn passive_atk_applies(id: &str, weapon: WeaponType) -> bool {
    match id {
        // Sword Mastery: 1H swords + daggers.
        "sword_mastery" => matches!(weapon, WeaponType::Sword1H | WeaponType::Dagger),
        // Two-Handed Sword Mastery: 2H swords only.
        "twohand_sword_mastery" => matches!(weapon, WeaponType::Sword2H),
        // Knife Mastery (Assassin 2-1): daggers only.
        "knife_mastery" => matches!(weapon, WeaponType::Dagger),
        // Mace Mastery (Bishop 2-1): maces only.
        "mace_mastery" => matches!(weapon, WeaponType::Mace),
        _ => true,
    }
}

/// Sum of flat ATK granted by all PASSIVE skills an entity carries
/// (each `atk_flat_per_level · level`), gated by the wielded `weapon`
/// type for weapon-mastery passives. Folded into `effective_atk`. Active
/// skills never contribute here — their power is the cast multiplier.
pub fn passive_atk_bonus(states: &[SkillState], weapon: WeaponType) -> i32 {
    let mut bonus = 0i32;
    for s in states {
        if let Some(def) = skill_def(&s.id) {
            if matches!(def.kind, SkillKind::Passive)
                && passive_atk_applies(&s.id, weapon)
            {
                // x10 with the stat scale (0.0.83) — a mastery's flat ATK has to
                // grow with the number it is added to.
                bonus = bonus.saturating_add(def.atk_flat_per_level * s.level as i32 * 10);
            }
        }
    }
    bonus
}

/// Elemental Mastery (mage passive): extra damage in basis points dealt ONLY
/// when the attacker's element BEATS the target's. +2.5% (250 bp) per level
/// (max +25% at level 10). Carried on the `Damage` packet by `build_damage` and
/// applied in `apply_damage` when the matchup is advantageous.
pub fn passive_elemental_mastery_bp(states: &[SkillState]) -> u32 {
    let mut bp = 0u32;
    for s in states {
        if s.id == "elemental_mastery" {
            bp = bp.saturating_add(250 * s.level as u32);
        }
    }
    bp
}

/// Aspd Dampening (acolyte): attack-speed SLOW in basis-points-of-percent by
/// level = `(10 + 1.5·level)%` → Lv1 1150 (11.5%), Lv10 2500 (25%). Carried to
/// the AoE victims on the projectile and applied as an `AspdSlowPct` buff.
pub fn aspd_dampening_slow_bp(level: u8) -> i32 {
    1000 + 150 * level as i32
}

/// Holy Bane (acolyte passive): extra damage in basis points dealt ONLY when
/// the TARGET's element is Shadow (dark / undead / demon). +3% (300 bp) per
/// level → +30% at Lv10. Carried on the `Damage` packet by `build_damage`,
/// applied in `apply_damage` when the defender is Shadow-element. Id-keyed
/// (like Elemental Mastery); 0 for anyone without it ⇒ goldens safe.
/// Umbral Exploit (0.0.426): +damage per distinct debuff on the target, from any passive
/// carrying `debuff_amp_bp`. 0 for everyone else.
pub fn passive_debuff_amp_bp(states: &[SkillState]) -> u32 {
    states
        .iter()
        .filter(|s| s.level > 0)
        .filter_map(|s| crate::skills::skill_def(&s.id))
        .map(|d| d.debuff_amp_bp)
        .max()
        .unwrap_or(0)
}

pub fn passive_holy_bane_bp(states: &[SkillState]) -> u32 {
    states
        .iter()
        .filter(|s| s.id == "holy_bane")
        .map(|s| 300 * s.level as u32)
        .sum()
}


/// Passive DEX granted by Keen Eye (RO Owl's Eye): +1 DEX per skill level.
/// Folded into `buffed_stats`, so it flows into ranged ATK + HIT + ASPD at once.
/// Id-keyed (like the aura/taunt specials) rather than a `SkillDef` field.
pub fn passive_dex_bonus(states: &[SkillState]) -> i32 {
    states.iter().filter(|s| s.id == "keen_eye").map(|s| s.level as i32).sum()
}

/// Passive attack-range granted by Hawk Sight (RO Vulture's Eye): +1 range per
/// TWO skill levels (Lv2 → +1 … Lv10 → +5). Folded into `effective_atk_range`.
pub fn passive_range_bonus(states: &[SkillState]) -> i32 {
    states.iter().filter(|s| s.id == "hawk_sight").map(|s| s.level as i32 / 2).sum()
}

/// A basic-attack profile override (Sagitta Lucis-style), read from a wearer's PASSIVE
/// skills. REUSABLE — any skill that sets the `basic_*` SkillDef fields drives it, so a
/// future class/Exemplar re-profiles its basic without touching combat code. The first
/// qualifying passive wins (one such skill is expected per member). `None` = no override.
pub struct BasicOverride {
    pub range: i32, // 0 = keep base
    pub ranged: bool,
    pub damage_type: Option<crate::grid::DamageType>,
    pub element: Option<crate::grid::Element>,
    pub power_bp: u32, // 0 = 100%
}

pub fn basic_override(states: &[SkillState]) -> Option<BasicOverride> {
    for s in states {
        if let Some(def) = skill_def(&s.id) {
            if matches!(def.kind, SkillKind::Passive)
                && (def.basic_range > 0 || def.basic_damage_type.is_some() || def.basic_element.is_some() || def.basic_power_bp > 0)
            {
                return Some(BasicOverride {
                    range: def.basic_range,
                    ranged: def.basic_ranged,
                    damage_type: def.basic_damage_type,
                    element: def.basic_element,
                    power_bp: def.basic_power_bp,
                });
            }
        }
    }
    None
}



/// Bulwark (Seraphine): flat hard DEF per enemy she currently holds under her
/// own Taunt — `2 × level` each, capped at `BULWARK_DEF_CAP`. The cap is what
/// keeps a room-wide pull from turning into unbounded DEF; without it the skill
/// that gathers the most enemies would also make her unkillable.
pub const BULWARK_DEF_CAP: i32 = 1_000; // x10 stat scale (0.0.83)

pub fn bulwark_def_bonus(states: &[crate::skills::SkillState], taunt_hold: u32) -> i32 {
    let level: i32 = states
        .iter()
        .filter(|s| s.id == "bulwark")
        .map(|s| s.level as i32)
        .sum();
    if level == 0 || taunt_hold == 0 {
        return 0;
    }
    (20 * level * taunt_hold as i32).min(BULWARK_DEF_CAP)
}

/// The Ultimate a set of skills carries, if any — the one whose `lb_max` is set.
/// A character has exactly one; the first is taken if data ever says otherwise.
pub fn ultimate_of(states: &[crate::skills::SkillState]) -> Option<&'static crate::skills::SkillDef> {
    states
        .iter()
        .filter(|s| s.level > 0)
        .find_map(|s| crate::skills::skill_def(&s.id).filter(|d| d.lb_max > 0))
}

/// What a character gains from one charge EVENT, summed across every skill it
/// knows — not just its Ultimate.
///
/// The Ultimate owns the gauge (`lb_max`); the SOURCES are owned by whatever
/// skill earns them. That distinction is the rule that keeps a charge source
/// from being handed out for free: charging off a Toughness break belongs to the
/// Destroyer doctrine, so a character of any other doctrine can only have it by
/// carrying a skill that grants it — Rapis charges off breaks because Ember
/// Cascade says so, not because her ultimate quietly does.
/// The attacker's `broken_target_bp` passives, summed, when `target`'s Toughness is broken (in
/// its recovery window) — 0 otherwise (0.0.463, Charon).
pub fn broken_target_bp(attacker: &crate::grid::Entity, target: &crate::grid::Entity) -> i64 {
    if target.break_ticks == 0 {
        return 0;
    }
    attacker
        .skills
        .iter()
        .filter(|s| s.level > 0)
        .filter_map(|s| crate::skills::skill_def(&s.id))
        .map(|d| i64::from(d.broken_target_bp))
        .sum()
}

/// The owner's basic-swing Toughness multiplier (percent; 0 = baseline) — the largest passive
/// that names one (0.0.463, Charon's Ferryman's Toll).
pub fn basic_toughness_mult(e: &crate::grid::Entity) -> u32 {
    e.skills
        .iter()
        .filter(|s| s.level > 0)
        .filter_map(|s| crate::skills::skill_def(&s.id))
        .map(|d| d.basic_toughness_mult)
        .max()
        .unwrap_or(0)
}

/// The Blackwater Mark `attacker` holds on `target`: `Some(extra build-up bp)` when the mark's
/// source is this attacker, else `None` (0.0.463).
pub fn blackwater_mark_from(attacker: &crate::grid::Entity, target: &crate::grid::Entity) -> Option<i32> {
    target
        .buffs
        .iter()
        .find(|b| matches!(b.kind, crate::skills::BuffKind::BlackwaterMark) && b.source_id == attacker.id)
        .map(|b| b.magnitude)
}

/// The attacker's `taunted_target_bp` passives, summed, when `target` is under a taunt — 0
/// otherwise (0.0.462, Airi).
pub fn taunted_target_bp(attacker: &crate::grid::Entity, target: &crate::grid::Entity) -> i64 {
    if target.taunted_by.is_none() || target.taunt_ticks == 0 {
        return 0;
    }
    attacker
        .skills
        .iter()
        .filter(|s| s.level > 0)
        .filter_map(|s| crate::skills::skill_def(&s.id))
        .map(|d| i64::from(d.taunted_target_bp))
        .sum()
}

pub fn lb_gain(states: &[crate::skills::SkillState], pick: fn(&crate::skills::SkillDef) -> u16) -> u16 {
    states
        .iter()
        .filter(|s| s.level > 0)
        .filter_map(|s| crate::skills::skill_def(&s.id))
        .map(pick)
        .fold(0u16, |a, b| a.saturating_add(b))
}

/// Add `amount` to `e`'s Limit Break gauge, clamped at its Ultimate's `lb_max`.
/// A no-op for anyone without an Ultimate, which is every monster and every
/// character whose kit predates the gauge.
/// The share of the owner's landed damage that is booked as a heal (0.0.468, Unda Vitae).
pub fn heal_on_hit_bp(e: &crate::grid::Entity) -> u32 {
    e.skills
        .iter()
        .filter(|s| s.level > 0)
        .filter_map(|s| crate::skills::skill_def(&s.id))
        .map(|d| d.heal_on_hit_bp)
        .max()
        .unwrap_or(0)
}

pub fn charge_limit_break(e: &mut crate::grid::Entity, amount: u16) {
    if amount == 0 {
        return;
    }
    let Some(max) = ultimate_of(&e.skills).map(|d| d.lb_max) else { return; };
    // CHARGE RATE scales the gain at the ONE fold point every source passes
    // through, so a new charge source cannot forget to honour it. The ceiling is
    // the Ultimate's own `lb_max` and stays where it is. Rounds DOWN, but a
    // non-zero gain never rounds to nothing.
    // A `GaugeRatePct` BUFF (0.0.468, Fons Vitae) rides the same fold as the gear line.
    let rate = (crate::combat::stat_mods::charge_rate_bp(e)
        + crate::skills::buff_sum(e, crate::skills::BuffKind::GaugeRatePct).max(0))
        .max(-10_000) as i64;
    let scaled = ((amount as i64) * (10_000 + rate) / 10_000).max(1) as u16;
    e.limit_break = e.limit_break.saturating_add(scaled).min(max);
}

/// DODGE from PASSIVE skills, in basis points, summed as `dodge_bp_per_level x level`
/// (Nimble Mastery). Folded beside the gear lines in `stat_mods::dodge_bp`, so a
/// passive and an affix stack the same inverse-multiplicative way everything else does.
pub fn passive_dodge_bp(states: &[SkillState]) -> i32 {
    let mut bp = 0i32;
    for s in states {
        if let Some(def) = skill_def(&s.id) {
            if matches!(def.kind, SkillKind::Passive) {
                bp = bp.saturating_add(def.dodge_bp_per_level * s.level as i32);
            }
        }
    }
    bp
}
