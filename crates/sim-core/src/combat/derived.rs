//! Derived stats — `effective_*` read an entity + its gear / skills /
//! stats. Split out of the old single-file combat.rs (0.0.34 refactor).

use crate::grid::Entity;
use super::stat_mods;
use crate::skills::{buff_sum, BuffKind};

/// Walk every ACTIVE socketed card — the ONE activation rule (per card class,
/// capped by the equipped gear's socket count feeding that class). Shared by
/// `card_bonuses` and the card SPECIAL-effect layer so activation can't drift.
pub fn for_each_active_card(e: &Entity, f: &mut dyn FnMut(&crate::items::Item)) {
    use crate::items::{card_class_of_slot, CARD_CLASSES};
    for class in CARD_CLASSES {
        let sockets: u8 = e
            .equipped
            .iter()
            .filter(|it| card_class_of_slot(it.equip_slot) == Some(class))
            .map(|it| it.socket_count())
            .sum();
        let mut n: u8 = 0;
        for card in e.card_panel.iter().filter(|c| c.card_slot == Some(class)) {
            if n >= sockets {
                break;
            }
            f(card);
            n += 1;
        }
    }
}

/// Apply a basis-point percentage buff sum to `base` (multiply-before-divide,
/// pure integer). `pct` is the signed sum of the relevant `*Pct` buffs; 0
/// leaves `base` untouched so a buff-less entity is byte-identical.
fn apply_pct(base: i32, pct: i32) -> i32 {
    if pct == 0 {
        base
    } else {
        ((base as i64) * (10_000 + pct as i64) / 10_000) as i32
    }
}

/// ATTACK-SPEED CEILING in accumulator milli (0.0.367). The swing accumulator
/// spends 1000 per attack and gains this value per 100 ms tick, so the unit is
/// milli-swings per tick — and `milli / 100` is attacks per second, which is what
/// the UI shows (LoL-style). 1000 = one swing per tick = **10.00 atk/s**, the
/// PHYSICAL ceiling: a faster rate cannot be represented at a 100 ms tick, so
/// this clamp is the arithmetic guard, not the balance dial. The balance bound
/// is `ATK_SPEED_BONUS_CAP_BP` below (+200%, which puts a 1.06 atk/s unit at 3.18) —
/// a bound on the stackable term rather than RO's invisible asymptote at 200.
const ATK_SPEED_CAP_MILLI: i64 = 1_000;
/// Attack-speed FLOOR (0.20 atk/s). A stack of slows must never stop an entity
/// attacking outright — MOBAs keep a floor for the same reason.
const ATK_SPEED_FLOOR_MILLI: i64 = 20;
/// ATTACK-SPEED BONUS ceiling in basis points (+200%). The bonus is additive across sources
/// (Dota's IAS points, written as a percentage), so THIS is the balance bound on
/// speed — the rate clamp above is only the tick's physical ceiling.
const ATK_SPEED_BONUS_CAP_BP: i64 = 20_000;
/// SHIELD weight: a shield costs −10% attack speed (was −5 ASPD, which is what
/// −10% works out to at every unit's base rate). Sword & Shield Mastery shaves
/// 1 point/level off it, erasing it at Lv10.
const SHIELD_ATK_SPEED_PENALTY_BP: i64 = 1_000;

/// Bloodthirst damage bonus (basis points) for `e`: `stacks × 50bp × level`
/// (+0.5% per stack per skill level). 0 with no stacks / no skill — so it never
/// changes a non-Assassin's damage (goldens safe). Folded into every hand's
/// `%ATK`, so it applies to basic AND skill damage.
pub fn bloodthirst_bonus_bp(e: &Entity) -> i32 {
    if e.bloodthirst_stacks == 0 {
        return 0;
    }
    let lvl = e.skill_level("bloodthirst");
    (e.bloodthirst_stacks as i32) * 50 * (lvl as i32)
}

/// The equipped weapon item occupying `hand` (`Weapon` = main, `Shield` = the off-hand,
/// 0.0.383), or `None` if that hand holds no weapon — a shield is not a weapon, so the
/// off-hand only answers when the Assassin's dual-wield routing put a dagger there.
fn weapon_in_hand(e: &Entity, hand: crate::items::EquipSlot) -> Option<&crate::items::Item> {
    e.equipped
        .iter()
        .find(|it| it.weapon_type != crate::items::WeaponType::None && it.slot() == hand)
}

/// The MAIN-HAND weapon type (`WeaponType::None` if unarmed). Public so the bridge
/// can surface it for the renderer's per-weapon basic-attack VFX (e.g. a dagger vs a
/// 1H-sword slash) without leaking the item list.
pub fn main_hand_weapon_type(e: &Entity) -> crate::items::WeaponType {
    weapon_in_hand(e, crate::items::EquipSlot::Weapon)
        .map(|it| it.weapon_type)
        .unwrap_or(crate::items::WeaponType::None)
}

/// Effective attack counting EXACTLY ONE weapon hand (`hand`) plus all
/// non-weapon gear. The OTHER hand's weapon (atk_bonus + weapon_type)
/// is skipped so each hand draws only from its own blade — every other term
/// (StatusATK, masteries, aura, cards, %ATK) is the full per-swing value, since
/// in RO each hand is a complete attack. For a single-weapon entity computing
/// the main hand this is byte-identical to the legacy formula (the off-hand set
/// is empty, so nothing is skipped).
/// The worn Exemplar id — with `unit_def`, what decides a piece's weapon family.
fn wearer_exemplar(e: &Entity) -> Option<&str> {
    e.exemplar.as_ref().map(|x| x.id.as_str())
}

fn effective_atk_for_hand(e: &Entity, hand: crate::items::EquipSlot) -> i32 {
    let mut t = e.atk;
    let mut weapon = crate::items::WeaponType::None;
    for it in &e.equipped {
        let is_weapon = it.weapon_type != crate::items::WeaponType::None;
        // A weapon in the OTHER hand contributes nothing to this hand.
        if is_weapon && it.slot() != hand {
            continue;
        }
        // An item has ONE attack number (0.0.437 — MATK is gone; staves carry theirs in `atk_bonus`).
        t = t.saturating_add(it.worn_atk(wearer_exemplar(e), &e.unit_def));
        if is_weapon {
            weapon = it.weapon_type;
        }
    }
    // A character adds its AUTHORED curve; anything without one (a monster, a
    // pet, a test fixture) attacks with the flat `atk` its catalog entry states
    // and nothing else (user 2026-09-07). No attribute reaches attack power.
    if let Some(a) = e.authored_atk() {
        t = t.saturating_add(a);
    }
    // Passive flat-ATK skills (Keen Eye unconditional; Sword/2H/Knife Mastery
    // gated to the wielded weapon class).
    t = t.saturating_add(crate::skills::passive_atk_bonus(&e.skills, weapon));
    // Party aura ATK (the legacy always-on Blessing-style aura, stamped each
    // tick onto `aura_atk`). 0 for entities with no ATK aura.
    t = t.saturating_add(e.aura_atk);
    // Flat ATK from the Soul-path "Edge" aura (BuffKind::AtkAdd). 0 unless owned.
    t = t.saturating_add(buff_sum(e, BuffKind::AtkAdd));
    // Flat ATK from every StatMod source — gear, overlays, slotted + bound cards
    // (0.0.245: one pipeline). Joins before the % bucket.
    t = t.saturating_add(stat_mods::atk_flat(e));
    // Percentage ATK: buffs (Improve Concentration) + card atk_pct + Bloodthirst
    // kill-streak stacks in ONE additive bucket — then the GEAR-ENCHANT ATK%
    // bucket MULTIPLIES on top as its own layer (user 2026-07-05, same semi-
    // incremental rule as dmg-vs: sources ADD within a layer, layers MULTIPLY).
    // Both 0 → both apply_pct no-op for a fresh entity.
    let atk = apply_pct(
        apply_pct(
            t,
            buff_sum(e, BuffKind::AtkPct)
                + crate::skills::effective_bp(e, BuffKind::BellAtkPct)
                + bloodthirst_bonus_bp(e),
        ),
        stat_mods::atk_bp(e),
    ).saturating_add(crate::potential::effects::atk_from_def(e));
    // 2H + SHIELD penalty. The combo is normally FORBIDDEN (blocked at equip in
    // `events::equip_item`); Sword & Shield Mastery UNLOCKS it but at a damage
    // penalty of −50% at Lv1 easing to 0% at Lv10 (`5000 × (10 − level) / 9`).
    // No-op unless BOTH a 2H weapon and a shield are equipped (only reachable with
    // the mastery), so every other build is unchanged.
    if weapon == crate::items::WeaponType::Sword2H && has_shield(e) {
        let ssm = crate::skills::sword_shield_mastery_level(&e.skills) as i64;
        let penalty_bp = (5000 * (10 - ssm) / 9).clamp(0, 5000); // Lv1 −50% → Lv10 0%
        return ((atk as i64) * (10_000 - penalty_bp) / 10_000) as i32;
    }
    atk
}

/// Effective attack = base ATK + equipped atk_bonus
/// + stat bonus (STR for melee, DEX for ranged). This is the MAIN-HAND value —
/// the one skills scale and the one the basic right-hand swing uses.
///
/// Melee vs ranged is decided by `proj_travel_ticks`: 0 = a STR
/// fighter, > 0 = a DEX fighter. Mirrors RO's distinction where STR
/// gear (swords / maces) and DEX gear (bows / guns) draw from
/// different stats even though they share the ATK pool.
pub fn effective_atk(e: &Entity) -> i32 {
    effective_atk_for_hand(e, crate::items::EquipSlot::Weapon)
}

/// True if the entity has a SHIELD equipped (`EquipSlot::Shield`). Gates the
/// Swordmaster's shield-as-weapon skills + Sword & Shield Mastery's bonuses.
pub fn has_shield(e: &Entity) -> bool {
    // A tome / quiver / hidden knife sits in the same slot (0.0.446) and is not a shield.
    e.equipped
        .iter()
        .any(|it| it.offhand_type == crate::items::OffhandType::Shield)
}

/// HARD DEF = base DEF + equipped def_bonus. Renewal: this is
/// the part that gives a DIMINISHING %-reduction (see `apply_damage`),
/// so stacking it never hard-walls damage to 1 (no cliff).
pub fn effective_hard_def(e: &Entity) -> i32 {
    let mut t = e.def;
    for it in &e.equipped {
        t = t.saturating_add(it.worn_def(wearer_exemplar(e), &e.unit_def));
    }
    // Flat hard-DEF adds (Soul-path DEF spine) join the gear DEF BEFORE the %
    // multiplier, so DefPct scales them too. 0 for a buff-less entity.
    t = t.saturating_add(buff_sum(e, BuffKind::HardDefAdd));
    // Flat DEF from every StatMod source (card VIT cascades separately via soft-DEF).
    t = t.saturating_add(stat_mods::def_flat(e));
    // Sword & Shield Mastery (Swordmaster): +2 hard DEF/level while a shield is
    // equipped. Gated by the passive + a shield → no one else is affected.
    let ssm = crate::skills::sword_shield_mastery_level(&e.skills);
    if ssm > 0 && has_shield(e) {
        t = t.saturating_add(2 * ssm as i32);
    }
    // Bulwark (Seraphine): flat hard DEF per enemy she holds under her own Taunt.
    // 0 for anyone without the passive or holding nothing.
    t = t.saturating_add(crate::skills::bulwark_def_bonus(&e.skills, e.taunt_hold));
    // DefPct buffs raise hard DEF (Energy Coat / Angelus); an enemy-applied
    // DefDownPct reduces it. Net 0 for a buff-less entity ⇒ unchanged.
    // Sunder (0.0.465) cuts DEF the same way DefDownPct does, from its own kind.
    let down = crate::skills::effective_bp(e, BuffKind::DefDownPct) + crate::skills::effective_bp(e, BuffKind::Sunder) * 100;
    let def = apply_pct(t, buff_sum(e, BuffKind::DefPct) + stat_mods::def_bp(e) - down);
    // FROZEN (RO): a frozen target's DEF is halved. Gated on the buff → 0 for
    // everyone not frozen, so the common path is unchanged.
    if buff_sum(e, BuffKind::Freeze) > 0 { def / 2 } else { def }
}

/// SOFT DEF = the VIT/AGI/Level flat mitigation (`Stats::def_bonus`),
/// subtracted AFTER the hard-DEF %-reduction in `apply_damage`. Endure's
/// VIT (0.0.178) arrives via `buffed_stats` like any other VIT.
pub fn effective_soft_def(e: &Entity) -> i32 {
    // Monsters have their own renewal formula — `(VIT + BaseLv)/2` — off their STAGE (0.0.399):
    // `(level/2) × 10` with level = stage × 100/240 is `stage × 25/12`, kept exact so the soft
    // layer is what it was on every stage. Before 0.0.2xx they had NO soft DEF at all.
    let base = if matches!(e.team, crate::grid::Team::Monster) {
        (i32::from(e.mob_stage) * 25) / 12
    } else {
        // Level, plus whatever a passive grants directly (Endure) — the half of
        // the old VIT term that survived the attributes (0.0.86). 5 per point is
        // the old `VIT/2` at the x10 scale.
        e.stats.def_bonus()
            + crate::skills::passive_vit_bonus(&e.skills) as i32 * 5
    };
    // Classic Poison cuts soft DEF by 25% (marker buff; fixed, not magnitude-scaled).
    if buff_sum(e, BuffKind::Poison) > 0 {
        base * 3 / 4
    } else {
        base
    }
}

/// Total DEF for display only (hard + soft); combat uses the two
/// separately because they mitigate differently (% vs flat).
// MDEF IS GONE (0.0.332). `effective_hard_mdef`/`effective_soft_mdef`/`effective_mdef`
// stood after 0.0.85 merged the two armours, still computing a number no formula read
// — and the client still SHOWED it, which is worse than a missing stat: a player
// builds toward what the sheet reports. One DEF, and `effective_def` is all of it.
pub fn effective_def(e: &Entity) -> i32 {
    // Potential's DEF% scales current total DEF in the same place read by
    // mitigation, Flame Shield, and Iron Edge; no second StatMod vocabulary.
    apply_pct(effective_hard_def(e) + effective_soft_def(e),
        crate::potential::effects::stat_bp(e, crate::potential::PotentialStat::DefBp))
}

/// Effective max HP — RO Renewal job growth: `(base + per-class growth ×
/// BaseLevel) × (1 + VIT/100)`. `max_hp` is the class's level-1 base;
/// `hp_per_level` the per-class growth; VIT adds +1% each (a multiplier,
/// not flat). Monsters keep their flat `max_hp` (hp_per_level 0, VIT 1 ≈
/// +1%).
/// RO Renewal BASE HP at a base level via the rAthena "factor mode" formula:
/// each level above 1 adds `(hp_increase + hp_factor·level)/100`, so the per-level
/// gain RISES with level (a convex curve — RO's HP table is ~quadratic). `base` is
/// the level-1 HP (RO uses 40 for every class). Closed-form integer sum of
/// `Σ_{i=2..L} (hp_increase + hp_factor·i)/100`.
///
/// Reference: rAthena `db/re/job_basepoints.yml` (HpFactor = "exponential HP
/// increase, per level [HpFactor·BaseLv/100]"; HpIncrease = "linear, [HpIncrease/100]")
/// — https://github.com/rathena/rathena/blob/master/db/re/job_basepoints.yml ; and
/// the Max HP formula MaxHP = floor(BaseHP·(1+VIT/100)) — https://irowiki.org/wiki/Max_HP .
/// Per-class HpFactor/HpIncrease were fitted from the real RO base-HP tables.
fn ro_base_hp(base: i64, level: i64, hp_increase: i64, hp_factor: i64) -> i64 {
    if level <= 1 {
        return base;
    }
    // Σ_{i=2..level} (hp_increase + hp_factor·i) = hp_increase·(level-1)
    //   + hp_factor·(level·(level+1)/2 - 1). Divide by 100 at the end (one floor).
    let tri = level * (level + 1) / 2 - 1; // Σ i for i=2..level
    let acc = hp_increase * (level - 1) + hp_factor * tri;
    base + acc / 100
}

pub fn effective_max_hp(e: &Entity) -> i32 {
    // MONSTERS carry their AUTHORED max_hp verbatim — the VIT/level/curve scaling
    // below is a PLAYER mechanic. Applying it (via the STAT_MIN=1 default VIT)
    // inflated a monster's effective max ~1%, so its HP bar never read full at
    // spawn and the boss bar's "max" didn't match the DB number. Pets are
    // Team::Player, so they still use the full curve.
    if matches!(e.team, crate::grid::Team::Monster) {
        return e.max_hp;
    }
    let level = e.stats.level as i64;
    // RO HP curve when the class carries a factor; else the legacy linear growth.
    let base = if e.hp_factor != 0 || e.hp_increase != 0 {
        ro_base_hp(e.max_hp as i64, level, e.hp_increase, e.hp_factor)
    } else {
        (e.max_hp as i64) + (e.hp_per_level as i64) * level
    };
    // Endure's passive VIT (0.0.178) counts toward MaxHP like trained VIT.
    // (Timed VitAdd BUFFS deliberately still don't — HP wobbling with a buff's
    // uptime reads as a bug; a permanent passive has no uptime to wobble.)
    // VIT is gone (0.0.86). What is left of the old `+1% MaxHP per VIT` is the
    // PASSIVE half — Endure and its kin — which now reads as a straight percentage
    // instead of routing through an attribute nobody allocates.
    let hp_pct = crate::skills::passive_vit_bonus(&e.skills) as i64;
    let from_vit = base * (100 + hp_pct) / 100;
    // Flat max-HP added after the VIT multiplier (a flat HP card grants exactly its
    // number; card VIT already scaled `base` above): every StatMod source + the
    // Soul-path "Lifeblood" aura (BuffKind::MaxHpAdd).
    let with_cards = from_vit
        + buff_sum(e, BuffKind::MaxHpAdd) as i64
        + stat_mods::max_hp_flat(e) as i64;
    // Global "Fortitude" aura % + card hp_pct + gear-mod HP% fold on top.
    apply_pct(
        with_cards as i32,
        buff_sum(e, BuffKind::HpPct) + stat_mods::max_hp_bp(e),
    )
}





/// LOADOUT capacity: how many `loadout_slots` worth of active skills an entity
/// can keep slotted at once. A FIXED budget — it used to read INT, which no
/// longer feeds anything (0.0.314); an over-budget tail goes inert via
/// `loadout_fits`.
pub const LOADOUT_CAPACITY: i32 = 10;

pub fn loadout_capacity(_e: &Entity) -> i32 {
    LOADOUT_CAPACITY
}

/// Effective attack speed as a per-tick accumulator increment (milli):
/// `tick::attack_step` adds this each tick toward the 1000-milli swing
/// threshold, so a larger value = faster swings.
///
/// ATTACK SPEED (0.0.367) — one formula for every entity, player and monster:
///
/// ```text
///   rate = base_rate × (1 + atk_speed_bonus) × (1 − slow)     clamped [0.20, 10.00] atk/s
/// ```
///
/// `base_rate` is the template's own `atk_speed_milli` (milli-swings per tick;
/// `/100` = attacks per second), the BONUS is every speed source summed
/// ADDITIVELY in basis points, and `slow` stays a FINAL multiplier so it bites a
/// sped-up target too. (The word "Haste" is deliberately NOT used here: the
/// design reserves it for COOLDOWN reduction, `skill-system-redesign.md` §4.5.1.) This is the LoL/Dota model — an additive % on a per-unit base
/// rate, with a written cap — and it replaces RO's ASPD-point curve (a hyperbola
/// closing on an asymptote at 200, plus a knee at 180, plus per-weapon
/// familiarity). That model existed to serve AGI/DEX and weapon familiarity,
/// both retired; what it left behind was a scale no player could read and a "+%"
/// that lied (its `200 − (200 − ASPD)·(1 − pct)` fold turned +30% into +43%),
/// which also made one "+ASPD" affix worth different amounts on different units —
/// poison for a market where every buyer should value a line the same.
pub fn effective_atk_speed_milli(e: &Entity) -> u32 {
    // A rate of 0 means "this entity has no attack rate authored" (hand-built
    // test entities, non-combatants) — it must stay 0, never rise to the floor.
    if e.atk_speed_milli == 0 {
        return 0;
    }
    // Every attack-speed source, additive, in bp (1000 = +10%): Sword Quicken and the
    // monster pack-buffs (`AspdPct`), speed potions (`AspdPotionPct`), card
    // stacks (`CardAspdStack`), and each equipped item's own `atk_speed_bp` (unique
    // speed lines; ordinary gear stores 0). The OFF-HAND weapon is excluded —
    // dual-wield's whole cost is the flat penalty below.
    let item_bonus: i64 = e
        .equipped
        .iter()
        .filter(|i| !(i.slot() == crate::items::EquipSlot::Shield && i.weapon_type != crate::items::WeaponType::None))
        .map(|i| i.graded_atk_speed_bp() as i64)
        .sum();
    let mut bonus = buff_sum(e, BuffKind::AspdPct) as i64
        + buff_sum(e, BuffKind::AspdPotionPct) as i64
        + buff_sum(e, BuffKind::CardAspdStack) as i64
        + item_bonus;
    if has_shield(e) {
        let ssm = crate::skills::sword_shield_mastery_level(&e.skills) as i64;
        bonus -= (SHIELD_ATK_SPEED_PENALTY_BP - 100 * ssm).max(0);
    }
    let bonus = bonus.clamp(-9_000, ATK_SPEED_BONUS_CAP_BP);
    // Aspd Dampening (acolyte): a direct attack-speed slow (bp, capped 90%) kept
    // as a FINAL multiplier rather than a negative bonus, so it bites hardest
    // exactly where it matters — a target already stacked with speed. Works on
    // monsters and in PvP because nothing about it reads an attribute.
    let slow = (crate::skills::effective_bp(e, BuffKind::AspdSlowPct)
        + crate::skills::buff_sum(e, BuffKind::MarkAspdSlow))
        .clamp(0, 9_000) as i64;
    let base = e.atk_speed_milli as i64;
    let rate = base * (10_000 + bonus) / 10_000 * (10_000 - slow) / 10_000;
    rate.clamp(ATK_SPEED_FLOOR_MILLI, ATK_SPEED_CAP_MILLI) as u32
}

/// RO Renewal cast time = FIXED + VARIABLE. A spell's base windup splits into a
/// FIXED CAST (20%, UNREDUCIBLE — the floor) and a VARIABLE CAST (80%, reduced by
/// the caster's DEX + INT):
///   `VCT = baseVCT × (1 − √((DEX·2 + INT) / 530))`,  baseVCT = 80% of base,
/// and DEX·2 + INT ≥ 530 ⇒ VCT 0, but the 20% FCT still applies — there is NO
/// truly-instant spell. DEX is twice as effective as INT, so DEX is the
/// cast-reduction stat for mages (its whole point); INT helps too (and already
/// drives MATK). Stats are the BUFFED values, so Blessing's INT/DEX and INT/DEX
/// cards shorten casts the RO way.
///
/// Integer math (no float — clippy::float_arithmetic is denied): we form the
/// reduction in basis points. `ratio_bp ≈ (stat/530)·10000`; scaling its sqrt
/// radicand by 1e4 makes `isqrt ≈ √ratio · 100 = √(stat/530)·10000`.
pub fn effective_cast_time(e: &Entity, base_ticks: u32) -> u32 {
    if base_ticks == 0 {
        return 0;
    }
    // A cast takes as long as the SKILL says (user 2026-09-07). The RO variable-cast
    // curve — a fixed fifth plus a `√((DEX·2 + INT)/530)` reduction on the rest —
    // is gone with the attributes it was made of. A skill that should be quick is
    // written quick; how fast a character casts belongs to Haste, not to a stat
    // nobody allocates.
    //
    // What survives is the two things that were never attributes: Incantation
    // Mastery's flat cut and the gear-mod cast reduction, both of which are
    // decisions a player made.
    let im_bp = crate::skills::incantation_mastery_bp(&e.skills) as u64;
    let after_passive = (base_ticks as u64) * (10_000 - im_bp) / 10_000;
    // Clamped both ways: a NEGATIVE cast line is a drawback that lengthens the
    // cast, not a no-op — clamping at zero would have quietly deleted every
    // downside roll in the game.
    let gear_bp = stat_mods::cast_bp(e).clamp(-5_000, 5_000) as i64;
    (((after_passive as i64) * (10_000 - gear_bp) / 10_000).max(1)) as u32
}


/// Effective attack range in cells — the entity's intrinsic `atk_range` plus
/// any passive range bonus (Hawk Sight: +1 per 2 levels). 0 bonus for anything
/// without that passive (every monster), so it equals `atk_range` there.
pub fn effective_atk_range(e: &Entity) -> i32 {
    // Reusable basic-attack override (Sagitta Lucis) can force a new reach.
    if let Some(o) = crate::skills::basic_override(&e.skills) {
        if o.range > 0 {
            return o.range;
        }
    }
    e.atk_range + crate::skills::passive_range_bonus(&e.skills)
}

/// LUK-derived crit chance (basis points) for `e`. Wraps
/// `Stats::crit_chance_bp` so equipment crit can fold in later
/// without touching call sites.
pub fn effective_crit_chance_bp(e: &Entity) -> u32 {
    (e.stats.crit_chance_bp() as i32
        + stat_mods::crit_bp(e)
        + buff_sum(e, BuffKind::CritBpAdd)) // Gloria + the Soul-path Precision arm
    .max(0) as u32
}

/// CRIT DAMAGE multiplier in bp (15_000 = the ×1.5 base everyone has).
/// Renewal-style crit (0.0.224, user: "เพิ่มเดเมจเฉยๆ ไม่ ignore def"): a crit
/// MULTIPLIES the packet by this and takes normal DEF mitigation — the old
/// def-ignore rule is gone. Sources ADD onto the base: card specials
/// (Gloomtooth/Scarlette/Chomb) + gear CritDmgBp lines. Its own multiply
/// layer at every crit site (both hands + ranged).
pub fn effective_crit_dmg_bp(e: &Entity) -> i32 {
    15_000 + stat_mods::crit_dmg_bp(e) + buff_sum(e, BuffKind::CritDmgBpAdd).max(0)
}



#[cfg(test)]
mod blind_tests {
    use super::*;
    use crate::grid::{Cell, Entity};
    use crate::skills::Buff;

    #[test]
    fn blind_makes_the_attacker_whiff_a_quarter_of_the_time() {
        // HIT/FLEE are gone (0.0.333) and Blind used to cut HIT by 25%. It is worth
        // exactly that still, applied at the avoidance roll instead.
        let plain = Entity::idle(1, Cell::new(0, 0));
        let target = Entity::idle(2, Cell::new(1, 0));
        assert_eq!(crate::combat::avoid_bp(&plain, &target), 0, "nothing avoids by default");
        let mut blinded = plain.clone();
        blinded.buffs.push(Buff { kind: BuffKind::Blind, magnitude: 1, ticks: 50, source_id: 9 });
        assert_eq!(
            crate::combat::avoid_bp(&blinded, &target),
            crate::combat::BLIND_MISS_BP,
            "a blinded attacker whiffs a quarter of its blows"
        );
    }
}

#[cfg(test)]
mod two_hand_shield_tests {
    use super::*;
    use crate::grid::{Cell, Entity};
    use crate::items::{EquipSlot, Item, ItemKind, Rarity, WeaponType};
    use crate::skills::SkillState;

    fn shield() -> Item {
        let mut s = Item::new("sh", ItemKind::Armor, 0, 20, Rarity::Common);
        s.equip_slot = Some(EquipSlot::Shield);
        s.offhand_type = crate::items::OffhandType::Shield;
        s
    }
    fn sword(wt: WeaponType) -> Item {
        let mut s = Item::new("sw", ItemKind::Weapon, 100, 0, Rarity::Common);
        s.weapon_type = wt;
        s
    }

    #[test]
    fn two_hand_plus_shield_penalty_eases_with_mastery() {
        let base = Entity::idle(1, Cell::new(0, 0));

        // 1H sword + shield = the un-penalised reference.
        let mut one = base.clone();
        one.equipped.push(sword(WeaponType::Sword1H));
        one.equipped.push(shield());
        let full = effective_atk(&one);

        // 2H + shield + Sword & Shield Mastery Lv1 → −50%.
        let mut lv1 = base.clone();
        lv1.equipped.push(sword(WeaponType::Sword2H));
        lv1.equipped.push(shield());
        lv1.skills.push(SkillState::new("sword_shield_mastery", 1));
        let at_lv1 = effective_atk(&lv1);
        assert!(at_lv1 < full, "Lv1 mastery still costs damage");
        assert!(full - 2 * at_lv1 <= 1, "Lv1 = -50%");

        // Lv10 → penalty gone, equals the un-penalised value.
        let mut lv10 = lv1.clone();
        lv10.skills[0] = SkillState::new("sword_shield_mastery", 10);
        assert_eq!(effective_atk(&lv10), full, "Lv10 removes the penalty");
    }
}


#[cfg(test)]
mod loadout_slot_tests {
    use super::*;
    use crate::grid::{Cell, Entity};
    use crate::skills::loadout_fits;

    #[test]
    fn capacity_is_fixed() {
        let e = Entity::idle(1, Cell::new(0, 0));
        assert_eq!(loadout_capacity(&e), LOADOUT_CAPACITY);
        assert_eq!(loadout_capacity(&e), LOADOUT_CAPACITY, "INT no longer buys slots");
    }

    #[test]
    fn loadout_overflows_past_capacity() {
        // iron_chains costs 2 slots; fireball + steel_break cost 1 each.
        let bar = vec![
            "iron_chains".to_string(),
            "fireball".to_string(),
            "steel_break".to_string(),
        ];
        assert!(loadout_fits(&bar, 3, "iron_chains"), "2 <= 3 fits");
        assert!(loadout_fits(&bar, 3, "fireball"), "2+1 = 3 <= 3 fits");
        assert!(!loadout_fits(&bar, 3, "steel_break"), "3+1 = 4 > 3 overflows");
        assert!(loadout_fits(&bar, 10, "steel_break"), "roomy budget fits all");
        assert!(!loadout_fits(&bar, 10, "lux_medica"), "a skill not in the bar never fits");
    }

}
