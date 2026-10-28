//! Damage model — element triangle, damage math, attack resolution.

// DEBT (47 sites): this file predates `deny(clippy::indexing_slicing)`. An index that
// cannot be proven in range is a panic, and in verify-server (which links this crate and
// replays saves under `panic = "abort"`) a panic is the whole process. Shrink this, never
// add to it — a NEW file has no allow and is rejected on its first bare index.
#![allow(clippy::indexing_slicing)]

use crate::grid::{DamageType, Element, Entity, Team};
use super::stat_mods;
use crate::rng::Pcg32;
use crate::state::HitEvent;
use super::*;

/// 3×3 element multiplier table in basis points (10000 = 100%).
///
/// Indexed by `[attacker_element][defender_element]`. Triangle:
///   Fire    -> Earth   advantage   (15000)
///   Earth   -> Water   advantage   (15000)
///   Water   -> Fire    advantage   (15000)
///   reverse                        ( 5000)
///   same element                   (10000)
///
/// Loose matchup numbers: advantage 150%, disadvantage 50%, else 100%.
/// No per-element levels — a familiar, RO-/Pokemon-referenceable web.
const ADVANTAGE_BP: u32 = 15_000;
const DISADVANTAGE_BP: u32 = 5_000;
const NEUTRAL_BP: u32 = 10_000;

/// Attacker-IS-STRONG (150%) ordered pairs. The REVERSE direction is the 50%
/// disadvantage; any pair not listed either way is 100%. Holy/Shadow appear
/// BOTH ways (mutual 150% — they shred each other). Built to be guessable:
/// the RO nature cycle (Fire>Earth>Wind>Water>Fire) + the universally-known
/// Thunder>Water / Earth>Thunder, + Fire clearing Poison. Poison has NO
/// offensive advantage and is weak only to Fire (its strength is the DoT).
const ADVANTAGE: &[(Element, Element)] = &[
    (Element::Fire, Element::Earth),
    (Element::Fire, Element::Poison),
    (Element::Ice, Element::Fire),
    (Element::Wind, Element::Ice),
    (Element::Earth, Element::Wind),
    (Element::Earth, Element::Thunder),
    (Element::Thunder, Element::Ice),
    (Element::Light, Element::Dark),
    (Element::Dark, Element::Light),
    // Spirit (ghost-kind): Holy busts ghosts; ghost attacks shred the unblessed
    // (reverse = Neutral hits pass through spirits at 50%). Shadow⇄Spirit = 100.
    (Element::Light, Element::Spirit),
    (Element::Spirit, Element::Physical),
];

/// WEAKNESS / RESIST multiplier for an attack of `el` landing on `target`
/// (`docs/element-weakness-system.md` §2): 120% if the element is on the target's
/// weakness list, 80% if it is on its resist list, 100% otherwise.
///
/// The 1.5x/0.5x MATRIX below is the fallback for a target carrying no list —
/// old saves, summons, players. Two reasons the list replaces it: a matrix is a
/// rule the player has to memorise in both directions, and its 3x spread between
/// right and wrong element made the wrong element a punishment rather than a
/// missed bonus. The gap here is 1.5x, deliberately small — the reason to collect
/// elements is the BREAK gate and Disorder, not the multiplier.
pub const WEAKNESS_BP: u32 = 12_000;
pub const RESIST_BP: u32 = 8_000;

pub fn element_mult_vs(target: &Entity, el: Element) -> u32 {
    // THE PLAYER SIDE HAS NO WEAKNESS (docs/element-weakness-system.md §5, decided 2026-09-07,
    // built 0.0.359): a unit takes 100% from every element; its defence against an element is
    // `resist_element_bp` (gear / card / meal RES%) and nothing else. Until now a unit with no
    // list fell through to the matrix below like any listless target, so Rapis (Fire) took 150%
    // from Water and 50% from Earth — the matrix the pivot retired, still running on players.
    if target.team == Team::Player {
        return NEUTRAL_BP;
    }
    // A monster has NO element (0.0.444): its lists are the whole answer, and a monster with
    // no lists takes 100% from everything — the matrix fallback is gone with the element.
    if target.weaknesses.contains(&el) {
        WEAKNESS_BP
    } else if target.resists.contains(&el) {
        RESIST_BP
    } else {
        NEUTRAL_BP
    }
}

/// Multiplier (basis points) `attacker` deals into `defender`: 150% if the
/// attacker is listed strong vs the defender, 50% if the defender is strong vs
/// the attacker (the reverse pair), else 100%. Dual-element defenders (future)
/// compose multiplicatively: multiply two results and divide by 10000.
pub fn element_mult_bp(attacker: Element, defender: Element) -> u32 {
    if ADVANTAGE.contains(&(attacker, defender)) {
        ADVANTAGE_BP // attacker is strong
    } else if ADVANTAGE.contains(&(defender, attacker)) {
        DISADVANTAGE_BP // defender is strong → attacker is weak
    } else {
        NEUTRAL_BP
    }
}

/// Pick the loaded ammo stack whose element best beats `target_el`, returning
/// its index in `ammo`. Considers ONLY stacks that have an element ADVANTAGE
/// (multiplier > neutral) and still have rounds — so a neutral/disadvantaged
/// shot spends no arrow (the archer fires bare Neutral instead). Ties break to
/// the highest multiplier, then the earliest stack (deterministic, no PRNG).
/// `None` ⇒ nothing in the pool beats this target → shoot Neutral, free.
pub fn pick_ammo_idx(ammo: &[crate::items::AmmoStack], target_el: Element) -> Option<usize> {
    let mut best: Option<(usize, u32)> = None;
    for (idx, st) in ammo.iter().enumerate() {
        if st.shots == 0 {
            continue;
        }
        let mult = element_mult_bp(st.element, target_el);
        if mult <= 10_000 {
            continue; // no advantage → don't waste an arrow
        }
        match best {
            Some((_, bm)) if mult <= bm => {}
            _ => best = Some((idx, mult)),
        }
    }
    best.map(|(idx, _)| idx)
}

/// Pure damage formula (Phase-0 legacy — production goes through
/// `apply_damage`; kept for the element-table unit tests). No state, no PRNG.
///
/// `is_crit` applies the renewal-crit rule (0.0.224): ×150% base crit damage
/// with DEF mitigating normally — the old pre-renewal def-ignore is gone.
/// The live crit multiplier (`effective_crit_dmg_bp`) also folds card/gear
/// bonuses; this pure helper applies just the base.
///
/// Returns at least 1 (no zero-damage hits even with terrible matchup,
/// per GDD §3.2). This prevents a tank wearing all-resist gear from
/// being literally invincible to the wrong element.
pub fn calculate_damage(
    atk: i32,
    def: i32,
    attacker_el: Element,
    defender_el: Element,
    is_crit: bool,
) -> i32 {
    let amt = if is_crit { (atk as i64 * 15_000 / 10_000) as i32 } else { atk };
    let raw = (amt - def).max(1);
    // a * b / 10000 — multiply before divide (§4.2.1).
    let scaled = (raw as i64) * (element_mult_bp(attacker_el, defender_el) as i64) / 10_000;
    let damaged = scaled.max(1);
    // Phase 0 damage fits comfortably in i32. The i64 cast above is
    // just to keep the multiply from overflowing on outsized values.
    damaged as i32
}

/// One resolved strike's OUTPUT — the packet that flies at a target.
///
/// This is deliberately decoupled from its source (which attack / skill
/// / weapon produced it): a target mitigates a `Damage` knowing only its
/// own properties, not how it was generated. `amount` is the attacker's
/// raw offense (ATK or MATK) BEFORE the target's DEF/MDEF + element
/// resistance, which `apply_damage` subtracts.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Damage {
    pub amount: i32,
    pub element: Element,
    pub kind: DamageType,
    /// Ranged vs melee — intrinsic to the attack, not the distance.
    /// Reserved for future "blocks ranged damage" style defenses.
    pub is_ranged: bool,
    /// Physical crit (DEF-ignoring). Always false for magic.
    pub is_crit: bool,
    /// This strike is part of an AoE blast (e.g. Fireball's 3×3). Set by
    /// the caster path, not `build_damage`; reserved for AoE-specific
    /// rules (damage falloff, "splash resist") + renderer/feedback hints.
    pub is_aoe: bool,
    /// Elemental Mastery bonus (basis points) the attacker carries — applied by
    /// `apply_damage` ONLY when this strike has an element ADVANTAGE over the
    /// target. 0 (no passive) ⇒ no change.
    pub mastery_bp: u32,
    /// Holy Bane bonus (basis points) — applied by `apply_damage` ONLY when the
    /// TARGET's element is Shadow (dark / undead / demon). 0 (no passive) ⇒ no change.
    pub shadow_bane_bp: u32,
    /// Umbral Exploit (0.0.426): +bp per distinct debuff kind on the target. 0 = none.
    pub debuff_amp_bp: u32,
    /// Attacker's LEVEL. The DEF curve is `DEF / (DEF + 5·level + 500)`, so the
    /// same armour is worth less against a stronger attacker — without it armour
    /// would harden forever as the numbers grow.
    pub atk_level: i32,
    /// Attacker's RACE/SIZE — carried so the DEFENDER's gear-mod damage
    /// reduction (DR vs race/size/ranged) can match inside `apply_damage`.
    pub atk_race: crate::grid::Race,
    pub atk_size: crate::grid::Size,
    /// Attacker's gear-mod DEF penetration (basis points, pre-capped at
    /// build time). 0 for mod-less attackers ⇒ streams unchanged.
    pub ignore_def_bp: i32,
}

/// Build the outgoing `Damage` for one strike by `attacker`, using the
/// attacker's own `atk_type` — the basic-attack path.
pub fn build_damage(attacker: &Entity, is_crit: bool) -> Damage {
    let mut d = build_damage_basic(attacker, is_crit);
    // Resonance ranks on the basic swing (0.0.441). 10000 for everyone without one.
    let rank_bp = crate::unit_rank::basic_power_mult_bp(attacker);
    if rank_bp != 10_000 {
        d.amount = ((d.amount as i64) * i64::from(rank_bp) / 10_000).min(i32::MAX as i64) as i32;
    }
    d
}

fn build_damage_basic(attacker: &Entity, is_crit: bool) -> Damage {
    // Reusable basic-attack override (Sagitta Lucis: ranged Holy magic @ 50% ATK).
    // Only the BASIC path goes through here — skills build their own packets.
    if let Some(o) = crate::skills::basic_override(&attacker.skills) {
        let kind = o.damage_type.unwrap_or(attacker.atk_type);
        let mut d = build_damage_as(attacker, is_crit, kind);
        if let Some(el) = o.element {
            d.element = el;
        }
        if o.ranged {
            d.is_ranged = true;
        }
        if o.power_bp > 0 {
            d.amount = ((d.amount as i64) * o.power_bp as i64 / 10_000) as i32;
        }
        // Re-apply the Holy-dmg-dealt buff now that the override may have set Holy
        // (build_damage_as saw the wearer's OWN element — e.g. the acolyte's Neutral —
        // so it skipped the bump). A basic-override wearer's own element is non-Holy,
        // so this never double-applies.
        if d.element == Element::Light {
            let bump =
                crate::skills::buff_sum(attacker, crate::skills::BuffKind::HolyDmgDealtPct).max(0);
            if bump > 0 {
                d.amount = ((d.amount as i64) * (10_000 + bump as i64) / 10_000) as i32;
            }
        }
        let dealt = crate::skills::dmg_dealt_bp(attacker, d.element);
        if dealt > 0 {
            d.amount = ((d.amount as i64) * (10_000 + dealt) / 10_000) as i32;
        }
        return d;
    }
    build_damage_as(attacker, is_crit, attacker.atk_type)
}

/// Build the outgoing `Damage` for one strike, forcing the damage `kind`
/// (Physical/Magic/True). This is the AXIS-SEPARATION seam (0.0.189): a SKILL
/// declares its own damage type independent of how the caster is otherwise
/// classified — so "magic" is a property of the SKILL, not of the caster's
/// weapon/delivery. Physical scales `effective_atk`, Magic scales
/// `effective_atk`; magic never crits (pre-renewal RO). `build_damage` passes
/// `attacker.atk_type`, so the basic path (and every current caller) is
/// byte-identical until a skill opts into an override.
pub fn build_damage_as(attacker: &Entity, is_crit: bool, kind: DamageType) -> Damage {
    // A self weapon-element buff (Magnum Break's Fire) overrides the packet
    // element WITHOUT mutating `Entity.element` (no display/hash churn). `None`
    // for a buff-less attacker ⇒ its own element, so streams stay identical.
    let element = crate::skills::buff_atk_element(attacker).unwrap_or(attacker.element);
    let mastery_bp = crate::skills::passive_elemental_mastery_bp(&attacker.skills);
    let shadow_bane_bp = crate::skills::passive_holy_bane_bp(&attacker.skills);
    let debuff_amp_bp = {
        let base = crate::skills::passive_debuff_amp_bp(&attacker.skills);
        if base > 0 { base + crate::unit_rank::exploit_bonus_bp(attacker) } else { 0 }
    };
    // One DEF, one penetration stat (0.0.330) — the retired mdef lines sum into it.
    let ignore_def_bp = stat_mods::ignore_def_bp(attacker);
    let mut d = match kind {
        // No entity has atk_type True; treat like physical for exhaustiveness.
        DamageType::True | DamageType::Physical => Damage {
            atk_level: attacker.stats.level as i32,
            amount: effective_atk(attacker),
            element,
            kind,
            is_ranged: attacker.is_ranged,
            is_crit,
            is_aoe: false,
            mastery_bp,
            shadow_bane_bp,
            debuff_amp_bp,
            atk_race: stat_mods::race_of(attacker),
            atk_size: stat_mods::size_of(attacker),
            ignore_def_bp,
        },
        DamageType::Magic => Damage {
            atk_level: attacker.stats.level as i32,
            amount: effective_atk(attacker),
            element,
            kind: DamageType::Magic,
            is_ranged: attacker.is_ranged,
            is_crit: false,
            is_aoe: false,
            mastery_bp,
            shadow_bane_bp,
            debuff_amp_bp,
            atk_race: stat_mods::race_of(attacker),
            atk_size: stat_mods::size_of(attacker),
            ignore_def_bp,
        },
    };
    // Holy-damage-dealt buff (Benedictio Lucis): amplify the attacker's OWN Holy
    // damage. 0 for an unbuffed / non-Holy attack ⇒ byte-identical.
    if element == Element::Light {
        let bump = crate::skills::buff_sum(attacker, crate::skills::BuffKind::HolyDmgDealtPct).max(0);
        if bump > 0 {
            d.amount = ((d.amount as i64) * (10_000 + bump as i64) / 10_000) as i32;
        }
    }
    // Tidebless (0.0.468): the flat team bump and, if the rolled element matches, its own.
    let dealt = crate::skills::dmg_dealt_bp(attacker, element);
    if dealt > 0 {
        d.amount = ((d.amount as i64) * (10_000 + dealt) / 10_000) as i32;
    }
    d
}

/// Like `build_damage_as` but scales the offensive `amount` by `power_bp`
/// (10000 = 100%) before the target's mitigation. `kind` forces the damage
/// type (axis-separation seam, 0.0.189) — pass `attacker.atk_type` for the
/// caster-derived default, or a skill's own type to make the SKILL decide.
pub fn build_damage_scaled(
    attacker: &Entity, is_crit: bool, power_bp: u32, kind: DamageType, power_from_atk: bool,
) -> Damage {
    let mut d = build_damage_as(attacker, is_crit, kind);
    // MAGIC damage that scales off ATK (a monster has no MATK) — Broomhilda's fireball, same
    // as Sanctus Terra's ATK-scaled field. Override the MATK base with ATK before the power %.
    if power_from_atk && matches!(kind, DamageType::Magic) {
        d.amount = effective_atk(attacker);
    }
    d.amount = ((d.amount as i64) * (power_bp as i64) / 10_000) as i32;
    d
}

/// Renewal hard-DEF reduction: `dmg × (K + DEF) / (K + FACTOR·DEF)`.
/// Diminishing returns, capped at `1/FACTOR` (90% with FACTOR 10) — never
/// 100%, so it can't cliff damage to 1. `K` is RO's 4000; at our small
/// DEF scale (tens) that means little reduction yet — lower K (or scale
/// DEF up) when tuning. Both are the only knobs.
/// The gacha DEF curve, `DEF / (DEF + 5·level + 500)` at the x10 stat scale
/// (0.0.85). At level 70 a defender with 530 DEF keeps about 38% of a blow off,
/// the same neighbourhood RO's two layers landed in — reached with one division.
/// BREAK DAMAGE, HSR's shape: a level constant × the gauge term `(toughness_max + 20)/40`,
/// with DEF, RES and `BreakDmgBp` on top. The level is the PARTY's — the sum of every member's
/// (user 2026-09-17: "level mult ของ total party lv") — so a break is the same event whoever
/// lands it, and levels and recruits grow it where they grow ATK by almost nothing. `A` is the
/// floor a lone Lv1 stands on; `B` per summed level. Against the ladder before DEF (K ×2 and
/// both constants ×2 in 0.0.408): ~50% of a stage-1 monster (two swings kill it anyway), ~23%
/// at 30, ~30% at 100, ~20% at the cap, ~10% at 240 — the tail is what `BreakDmgBp` (cap
/// +100%) is for, as Break Effect is in HSR. A boss
/// carries ×3 gauge, so its term is 5 against the field's 2, close to its ×2-3 HP.
///
/// The constant this replaces (`40 × target level × gauge`) was authored before the x10 stat
/// scale (0.0.85) and never moved with it — 40 against a ~300 swing at stage 1, 1,680 against
/// ~2,000 at stage 100 (user 2026-09-17: "break damage น้อยเกินไป น้อยกว่า basic attack").
// ×2 with the ladder's K (0.0.408) so a break stays the same share of a monster's HP.
const BREAK_DMG_FLOOR: i64 = 200;
const BREAK_DMG_PER_PARTY_LEVEL: i64 = 14;

const DEF_CURVE_BASE: i64 = 5_000;
const DEF_CURVE_PER_LEVEL: i64 = 50;

/// Magic's constant is FOUR TIMES steeper than physical's — RO's own numbers, verified against the
/// iRO Wiki's worked examples (MDEF 60 cuts about a third, 125 cuts half, where DEF needs 275 and
/// 500 for the same). So the same point of MDEF is worth far more than a point of DEF, and an
/// mdef range must be set against that, not against how it reads next to a def range.


/// DISPLAY-ONLY: the %-reduction (bp) this entity's DEF gives an incoming hit of
/// either kind, from the same curve `apply_damage` runs — computed by the SIM so
/// the stat panel can't drift from the combat math. Read against an attacker of
/// the entity's own level, un-mobbed and un-penetrated.
pub fn def_mitigation_bp(e: &Entity) -> i32 {
    let def = effective_def(e).max(0) as i64;
    let k = DEF_CURVE_BASE + DEF_CURVE_PER_LEVEL * (e.stats.level.max(1) as i64);
    (10_000 - 10_000 * k / (def + k)) as i32
}

/// Apply a `Damage` packet to `target`: the target mitigates with its OWN DEF
/// and element resistance — one DEF for both kinds since 0.0.85. Magic
/// ignores physical DEF entirely (uses MDEF). Element triangle is keyed
/// on the packet's element vs the target's. Returns final HP loss (≥ 1).
pub fn apply_damage(target: &Entity, dmg: &Damage, mob_attackers: u32) -> i32 {
    let amount = dmg.amount as i64;
    let raw: i64 = match dmg.kind {
        // True ignores DEF, element and ward; Iron Wall's party reduction
        // applies to every damage channel, as its authored description states.
        DamageType::True => return crate::potential::effects::reduce_incoming(target, amount).max(1) as i32,
        // ONE curve for physical and magic alike (0.0.85). MDEF is gone from the
        // damage path: a character has a single DEF, and what a blow is made of is
        // decided by its element, not by which armour value it happens to meet.
        //
        //     damage x K / (DEF + K)   where K = 5·attacker level + 500, x10 scale
        //
        // This is the shape the current gacha games use, chosen over RO's two
        // layers (a diminishing % and then a flat subtraction) for two reasons: a
        // flat subtraction floors small hits to nothing, and tying K to the
        // attacker's level keeps armour worth a stable share of a blow for the
        // whole game instead of hardening as the numbers grow.
        DamageType::Physical | DamageType::Magic => {
            let pen_bp = dmg.ignore_def_bp;
            let pen = (10_000 - pen_bp as i64).max(0);
            // Endure resists a share of the mobbed penalty (5%·level, Lv10 halves it).
            let resist = crate::skills::endure_mob_resist_pct(target.skill_level("endure"));
            let def = mobbed_def_resisted(effective_def(target).max(0), mob_attackers, resist)
                .max(0) as i64
                * pen
                / 10_000;
            let k = DEF_CURVE_BASE + DEF_CURVE_PER_LEVEL * dmg.atk_level.max(1) as i64;
            (amount * k / (def + k)).max(1)
        }
    };
    // a * b / 10000 — multiply before divide (§4.2.1).
    // FROZEN (RO): a frozen target's armor turns Water element (so Wind nukes
    // shred it, Fire is resisted). Gated on the buff → normal element otherwise.
    // FROZEN keeps the RO rule and stays on the MATRIX: the ice, not the creature,
    // is what the blow meets, so the target's own lists do not apply.
    // A frozen PLAYER stays at 100% too: the player side has no weakness, ice or not.
    // FROZEN (0.0.444): the ice adds Ice (Water) to what the target is weak to — a Water blow on
    // a frozen monster is a weakness hit — and nothing else about its lists changes.
    let frozen = crate::skills::buff_sum(target, crate::skills::BuffKind::Freeze) > 0
        && target.team != Team::Player;
    let emult = if frozen && dmg.element == Element::Ice && !target.resists.contains(&Element::Ice) {
        WEAKNESS_BP
    } else {
        element_mult_vs(target, dmg.element)
    };
    let mut scaled = raw * (emult as i64) / 10_000;
    // DEFENDER gear mods (0.0.134): elemental resist (cap 75%) + conditional
    // damage reduction vs the attacker's race/size or ranged strikes (cap 50%).
    // Both are additive buckets applied ONCE each — 0 for mod-less targets.
    let res_bp = stat_mods::resist_element_bp(target, dmg.element);
    if res_bp != 0 {
        // NEGATIVE = an elemental WEAKNESS: the same multiply, above 1.0.
        scaled = scaled * (10_000 - res_bp as i64) / 10_000;
    }
    let dr = stat_mods::dr_bp(target, dmg.atk_race, dmg.atk_size, dmg.is_ranged);
    if dr != 0 {
        scaled = scaled * (10_000 - dr as i64) / 10_000;
    }
    // Elemental Mastery: extra damage ONLY when this element beats the target
    // (mult > 100%). 0 mastery_bp ⇒ no change (goldens safe).
    if emult > 10_000 && dmg.mastery_bp > 0 {
        scaled = scaled * (10_000 + dmg.mastery_bp as i64) / 10_000;
    }
    // Holy Bane (acolyte): extra damage ONLY vs a target WEAK to Holy (the undead / demon
    // signature, now a list entry rather than an element). 0 shadow_bane_bp ⇒ no change.
    if dmg.shadow_bane_bp > 0 && target.weaknesses.contains(&Element::Light) {
        scaled = scaled * (10_000 + dmg.shadow_bane_bp as i64) / 10_000;
    }
    // Umbral Exploit (0.0.426): +`debuff_amp_bp` per DISTINCT debuff kind the target carries.
    if dmg.debuff_amp_bp > 0 {
        let n = crate::skills::distinct_debuff_kinds(target) as i64;
        if n > 0 {
            scaled = scaled * (10_000 + dmg.debuff_amp_bp as i64 * n) / 10_000;
        }
    }
    // Arcane Ward: flat %-reduction of the final number (after DEF + element),
    // capped at 90%. 0 for an un-warded target ⇒ no change (goldens safe).
    let reduce = (crate::skills::buff_sum(target, crate::skills::BuffKind::DmgReducePct))
        .clamp(0, 9_000) as i64;
    let warded = scaled * (10_000 - reduce) / 10_000;
    // Vulnerable (thief Open Wound rune): +% damage taken from ALL sources — the
    // mirror of the ward above. 0 for an un-marked target ⇒ no change (goldens safe).
    let vuln = crate::skills::effective_bp(target, crate::skills::BuffKind::DmgTakenPct).max(0) as i64;
    let exposed = if vuln > 0 { warded * (10_000 + vuln) / 10_000 } else { warded };
    // Holy Vulnerability (Sanctus Terra) — the first ELEMENT-specific vuln: amplifies ONLY
    // Holy-element damage by the sum of holy-vuln debuffs. Non-Holy / unmarked ⇒ no change
    // (goldens safe — no golden deals Holy damage to a holy-vuln'd target).
    let exposed = if dmg.element == Element::Light {
        let hv = crate::skills::holy_vuln_bp(target).max(0) as i64;
        if hv > 0 { exposed * (10_000 + hv) / 10_000 } else { exposed }
    } else if dmg.element == Element::Dark {
        // Umbral Mark (0.0.424): the Dark twin — amplifies only Dark damage.
        let sv = crate::skills::shadow_vuln_bp(target).max(0) as i64;
        if sv > 0 { exposed * (10_000 + sv) / 10_000 } else { exposed }
    } else if dmg.element == Element::Ice {
        // Frost Mark (0.0.459): amplifies only Ice damage. Shattered (0.0.476) adds to it.
        let wv = (crate::skills::water_vuln_bp(target).max(0) + crate::skills::shattered_bp(target)) as i64;
        if wv > 0 { exposed * (10_000 + wv) / 10_000 } else { exposed }
    } else if dmg.element == Element::Fire {
        // Shattered (0.0.476, Zolfia): a broken, chilled target takes more Fire as well as Ice.
        let fv = crate::skills::shattered_bp(target) as i64;
        if fv > 0 { exposed * (10_000 + fv) / 10_000 } else { exposed }
    } else if dmg.element == Element::Thunder {
        // Static Mark (0.0.462): amplifies Thunder damage — and so does a FROST Mark (user:
        // "Lumi debuff + dmg สายฟ้าด้วย": ice conducts; Lumi's Chill opens a target to Airi).
        let tv = (crate::skills::thunder_vuln_bp(target).max(0) + crate::skills::water_vuln_bp(target).max(0)) as i64;
        if tv > 0 { exposed * (10_000 + tv) / 10_000 } else { exposed }
    } else {
        exposed
    };
    // Flat reduction LAST (0.0.447, Training Guard): a fixed −N off the final number, so it
    // is a third of a stage-1 swing and a rounding error at stage 100. 0 for everyone without
    // the line ⇒ goldens safe.
    let flat = stat_mods::dmg_reduce_flat(target).max(0) as i64;
    let exposed = if flat > 0 { (exposed - flat).max(1) } else { exposed };
    crate::potential::effects::reduce_incoming(target, exposed).max(1) as i32
}

/// UMBRAL MARK / SWEET POISON: an attacker carrying a `mark_step_bp` passive stacks its mark on
/// whatever it lands damage on — one instance per source, `mark_step_bp` per hit up to
/// `mark_max_stacks`, timer refreshed — and (0.0.451) the mark's move / ASPD slows with it.
/// Called from every landing path (a swing, a projectile, a field tick). No draw.
/// STATIC SPARK (0.0.465, Mirei): a hit landed on `target_idx` fires ONE spark per layer —
/// flat damage booked to the layer — and spends a stack unless the attacker IS the layer.
/// No PRNG, no recursion (the spark never re-enters this path).
pub fn fire_static_spark(entities: &mut [Entity], attacker_idx: usize, target_idx: usize, hits: &mut Vec<HitEvent>) {
    let me = entities[attacker_idx].id;
    let layers: Vec<(u32, i32)> = {
        let t = &entities[target_idx];
        let mut seen: Vec<(u32, i32)> = Vec::new();
        for b in t.buffs.iter().filter(|b| matches!(b.kind, crate::skills::BuffKind::StaticSpark)) {
            if !seen.iter().any(|(src, _)| *src == b.source_id) {
                seen.push((b.source_id, b.magnitude));
            }
        }
        seen
    };
    for (src, per) in layers {
        if per <= 0 {
            continue;
        }
        // The layer's S4 (0.0.466): a spark on a target under its threshold survives whoever
        // fires it.
        let keep_pct = entities
            .iter()
            .find(|e| e.id == src)
            .map_or(0, crate::unit_rank::spark_keep_below_pct);
        let t = &mut entities[target_idx];
        let low = keep_pct > 0
            && (t.hp as i64) * 100 < (crate::combat::effective_max_hp(t) as i64) * i64::from(keep_pct);
        // The layer's own blows fire the spark for free; everyone else spends a stack.
        if src != me && !low {
            if let Some(pos) = t.buffs.iter().position(|b| matches!(b.kind, crate::skills::BuffKind::StaticSpark) && b.source_id == src) {
                t.buffs.remove(pos);
            }
        }
        let dealt = per.min(t.hp.max(0));
        t.take_hit(per);
        t.spark_fx.push((src, dealt));
        hits.push(HitEvent { target_id: t.id, attacker_id: src, amount: dealt, hits: 1, kind: crate::state::HitKind::Normal });
    }
}

/// Detonate EVERY Static Spark on `target_idx` at once (0.0.465, Mirei's ultimate).
pub fn detonate_static_spark(entities: &mut [Entity], target_idx: usize, hits: &mut Vec<HitEvent>) {
    let sparks: Vec<(u32, i32)> = entities[target_idx]
        .buffs
        .iter()
        .filter(|b| matches!(b.kind, crate::skills::BuffKind::StaticSpark))
        .map(|b| (b.source_id, b.magnitude))
        .collect();
    if sparks.is_empty() {
        return;
    }
    entities[target_idx].buffs.retain(|b| !matches!(b.kind, crate::skills::BuffKind::StaticSpark));
    for (src, per) in sparks {
        if per <= 0 {
            continue;
        }
        let t = &mut entities[target_idx];
        let dealt = per.min(t.hp.max(0));
        t.take_hit(per);
        t.spark_fx.push((src, dealt));
        hits.push(HitEvent { target_id: t.id, attacker_id: src, amount: dealt, hits: 1, kind: crate::state::HitKind::Normal });
    }
}

pub fn stamp_mark(entities: &mut [Entity], attacker_idx: usize, target_idx: usize) {
    // UMBRAL MARK (0.0.424): an attacker carrying a `mark_step_bp` passive stacks its mark
    // on whatever it lands a hit on — one instance per source, climbing `mark_step_bp` per
    // hit to `mark_max_stacks × step`, its timer refreshed. No draw.
    let (step, max_stacks, ticks, slow_pct, aspd_slow_bp, elem) = entities[attacker_idx]
        .skills
        .iter()
        .filter(|s| s.level > 0)
        .filter_map(|s| crate::skills::skill_def(&s.id))
        .fold((0i32, 0i32, 0u32, 0i32, 0i32, None), |(a, b, c, d1, d2, el), d| (
            a.max(d.mark_step_bp as i32), b.max(d.mark_max_stacks as i32), c.max(d.mark_ticks),
            d1.max(d.mark_slow_pct), d2.max(d.mark_aspd_slow_bp),
            el.or(if d.mark_step_bp > 0 { d.basic_element } else { None }),
        ));
    // The mark's vuln follows the passive's ATTACK element (0.0.459): a Dark marker lays Umbral
    // Mark (+Dark taken), a Water marker Frost Mark (+Water taken), a Thunder marker Static
    // Mark (+Thunder taken, 0.0.462). Lumi's Chill was Umbral Mark from 0.0.456 to 0.0.458 —
    // it fed Nyx, not her.
    let mark_kind = match elem {
        Some(Element::Ice) => crate::skills::BuffKind::FrostMark,
        Some(Element::Thunder) => crate::skills::BuffKind::StaticMark,
        _ => crate::skills::BuffKind::UmbralMark,
    };
    let max_stacks = if max_stacks > 0 { max_stacks + crate::unit_rank::mark_extra_stacks(&entities[attacker_idx]) } else { 0 };
    let step = if step > 0 { step + crate::unit_rank::mark_extra_step_bp(&entities[attacker_idx]) } else { 0 };
    let ticks = ((ticks as u64) * u64::from(crate::unit_rank::mark_duration_mult_bp(&entities[attacker_idx])) / 10_000) as u32;
    if step > 0 && max_stacks > 0 {
        let src = entities[attacker_idx].id;
        let t = &mut entities[target_idx];
        let stacks = match t.buffs.iter_mut().find(|b| b.kind == mark_kind && b.source_id == src) {
            Some(b) => {
                b.magnitude = (b.magnitude + step).min(step * max_stacks);
                b.ticks = ticks;
                b.magnitude / step
            }
            None => {
                t.buffs.push(crate::skills::Buff { kind: mark_kind, magnitude: step, ticks, source_id: src });
                1
            }
        };
        // SWEET POISON (0.0.451): the mark's move / ASPD slows ride the same stack count,
        // per source, refreshed with it.
        for (kind, per) in [(crate::skills::BuffKind::MarkSlow, slow_pct), (crate::skills::BuffKind::MarkAspdSlow, aspd_slow_bp)] {
            if per <= 0 {
                continue;
            }
            let mag = per * stacks;
            match t.buffs.iter_mut().find(|b| b.kind == kind && b.source_id == src) {
                Some(b) => {
                    b.magnitude = mag;
                    b.ticks = ticks;
                }
                None => t.buffs.push(crate::skills::Buff { kind, magnitude: mag, ticks, source_id: src }),
            }
        }
    }
}

/// Resolve one melee strike from `attacker_idx` to `target_idx`. Mutates
/// the target's hp/alive and returns the damage dealt (0 on a miss).
/// PRNG contract: magic takes zero draws; physical takes up to two in a
/// fixed order — crit (only when crit chance > 0), then hit (only on a
/// non-crit swing below full accuracy). A STAT_MIN physical attacker vs a
/// flee-less target consumes zero draws — preserving every golden stream.
pub fn resolve_attack(
    entities: &mut [Entity],
    rng: &mut Pcg32,
    attacker_idx: usize,
    target_idx: usize,
    hits: &mut Vec<HitEvent>,
) -> i32 {
    // A MELEE basic is a 100% packet through the scaled path, which never saw the basic-attack
    // override (only the ranged path builds through `build_damage`): Seraphine's 70% swing and
    // the Resonance basic ranks apply here as the power (0.0.441).
    let power = basic_power_bp(&entities[attacker_idx]);
    // A passive can make the BASIC chip harder (0.0.463, Charon's oar); 0 = the baseline.
    // BREAK FEVER (0.0.465, Suzu's ultimate) raises it for its duration and wins when bigger.
    let basic_mult = crate::skills::basic_toughness_mult(&entities[attacker_idx])
        .max(crate::skills::effective_bp(&entities[attacker_idx], crate::skills::BuffKind::BreakFever).max(0) as u32);
    resolve_attack_scaled(entities, rng, attacker_idx, target_idx, power, 1, hits,
        entities[attacker_idx].atk_type, basic_mult)
}

/// What a BASIC swing is worth in basis points of a 100% packet: the passive override's
/// `basic_power_bp` (0 = 100%) times the Resonance basic ranks. 10000 for everyone else.
pub fn basic_power_bp(attacker: &Entity) -> u32 {
    let over = crate::skills::basic_override(&attacker.skills).map_or(0, |o| o.power_bp);
    let base: u64 = if over > 0 { u64::from(over) } else { 10_000 };
    (base * u64::from(crate::unit_rank::basic_power_mult_bp(attacker)) / 10_000).min(u32::MAX as u64) as u32
}

/// Max downward damage swing (basis points) for `e`'s PHYSICAL attacks: 5% per
/// equipped weapon level (RO weapon-level variance), level 1 for no weapon /
/// monsters — so higher-tier weapons swing wider.
fn weapon_variance_bp(e: &Entity) -> u32 {
    let wlvl = e
        .equipped
        .iter()
        .filter(|i| matches!(i.kind, crate::items::ItemKind::Weapon))
        .map(|i| i.weapon_level)
        .max()
        .unwrap_or(1)
        .max(1);
    wlvl as u32 * 500
}

/// Roll `amount` by ONE PRNG draw (shared by melee + offhand + ranged, physical).
///
/// - MONSTERS roll ATK across their DB damage RANGE (`mob_atk_min..=mob_atk_max`,
///   RO atk1/atk2): a rolled ATK is picked uniformly and the packet is scaled by
///   `rolled / atk` (so it also works for power-scaled mob skills). No range set
///   (max ≤ min) ⇒ flat `atk`, NO draw.
/// - PLAYERS keep the RO weapon-level roll-DOWN (5% per weapon level, full
///   damage is the ceiling, never rolls above max).
///
/// A monster WITH a range and a player with a weapon each take exactly one draw;
/// only the resulting values shift vs the old flat mob damage (goldens re-blessed).
pub(crate) fn apply_weapon_variance(rng: &mut Pcg32, amount: &mut i32, attacker: &Entity) {
    if attacker.team == crate::grid::Team::Monster {
        let (lo, hi, base) = (
            attacker.mob_atk_min as i64,
            attacker.mob_atk_max as i64,
            attacker.atk as i64,
        );
        if hi > lo && base > 0 {
            let rolled = rng.gen_range_u32(lo as u32, (hi + 1) as u32) as i64;
            *amount = ((*amount as i64) * rolled / base).max(1) as i32;
        }
        return;
    }
    let spread = weapon_variance_bp(attacker);
    if spread == 0 {
        return;
    }
    let roll = rng.gen_range_u32(0, spread + 1) as i64;
    *amount = ((*amount as i64) * (10_000 - roll) / 10_000) as i32;
}

/// RO RENEWAL MATK range. In Renewal only the WEAPON MATK varies — StatusMATK
/// (our `status_matk`, an exact INT/DEX/LUK/Level match) is fixed, the
/// pre-renewal INT-multiples-of-5/7 spread is gone. Weapon variance is
/// `± 0.1 × WeaponLevel × WeaponMATK` per equipped weapon. We turn that flat
/// spread into a fraction of total MATK and apply it to the (power-scaled)
/// packet, rolling uniformly in `[−spread, +spread]`. No weapon MATK ⇒ no
/// variance (no draw), so monsters / unarmed casters stay deterministic.
pub(crate) fn apply_matk_variance(rng: &mut Pcg32, amount: &mut i32, attacker: &Entity) {
    if *amount <= 0 {
        return;
    }
    // Flat weapon-MATK spread = Σ 0.1 × weapon_level × matk_bonus (weapons only).
    let mut spread10 = 0i64; // ×10 (defer the 0.1) to stay integer
    for it in &attacker.equipped {
        // GRADED, like every other read of a gear stat (0.0.372): the spread has to widen with
        // the weapon it belongs to, or a Cosmic rod would roll the variance of a Common one.
        // WEAPONS only (a ring's ATK line is not a blade to swing wide).
        if it.weapon_type == crate::items::WeaponType::None {
            continue;
        }
        let atk = it.worn_atk(attacker.exemplar.as_ref().map(|x| x.id.as_str()), &attacker.unit_def);
        if atk > 0 {
            spread10 += (it.weapon_level as i64) * (atk as i64);
        }
    }
    let spread = spread10 / 10;
    if spread <= 0 {
        return;
    }
    // Scale the flat spread to the packet (which is total MATK × power_bp).
    let eff = crate::combat::effective_atk(attacker) as i64;
    if eff <= 0 {
        return;
    }
    let amt = *amount as i64;
    let max_delta = amt * spread / eff;
    if max_delta <= 0 {
        return;
    }
    let roll = rng.gen_range_u32(0, (2 * max_delta + 1) as u32) as i64 - max_delta;
    *amount = (amt + roll).max(1) as i32;
}

/// Soak `loss` through a Flame Shield absorb pool (the target's), returning the
/// HP loss that gets THROUGH. A blow exceeding the remaining pool passes its
/// overflow through and flags the shield to EXPLODE next (`absorb_burst_pending`,
/// resolved in `tick::resolve_absorb_bursts`). No pool ⇒ returns `loss` as-is, so
/// every un-shielded entity (all monsters, every golden) is byte-identical.
pub(crate) fn absorb_soak(t: &mut Entity, loss: i32) -> i32 {
    if loss <= 0 || t.shields.is_empty() {
        return loss;
    }
    let mut remaining = loss;
    // Drain the OLDEST shield first; a shield emptied by the blow moves to
    // `pending_bursts` (Flame Shield explodes there; Praesidium has burst_dmg 0 so
    // it's just cleaned up) and the overflow cascades to the next.
    while remaining > 0 && !t.shields.is_empty() {
        let s = &mut t.shields[0];
        let absorbed = remaining.min(s.pool);
        s.pool -= absorbed;
        remaining -= absorbed;
        let src = s.src_id;
        if absorbed > 0 {
            t.soaked_fx.push((src, absorbed));
        }
        // A hit-capped shield (Praesidium) spends ONE hit per blow it absorbs and
        // ENDS when its hit budget runs out, even if pool remains.
        let mut ended = s.pool <= 0;
        if s.hits_left > 0 {
            s.hits_left -= 1;
            if s.hits_left <= 0 {
                ended = true;
            }
        }
        if ended {
            let spent = t.shields.remove(0);
            t.pending_bursts.push(spent);
            // pool-empty → overflow (remaining>0) cascades to the next shield;
            // hit-cap end on a fully-absorbed blow leaves remaining 0 → loop exits.
        } else {
            break; // shield still holds → no cascade, no burst
        }
    }
    remaining
}

/// Same as `resolve_attack` but the damage is scaled by `power_bp` — the
/// shared path for both basic attacks (10000) and skills (their level's
/// power). Crit/hit rolls and the magic branch are identical, so a
/// skill cast consumes the same PRNG draws as the basic attack it
/// replaces.
/// A blow of this share of max HP (basis points) pays the full `lb_on_hit_taken`; lighter blows
/// pay proportionally, heavier ones proportionally more.
pub const LB_HIT_TAKEN_FULL_BP: i64 = 1_000;

/// Source of the Toughness-break vulnerability on the broken target.
pub const BREAK_VULN_SRC: u32 = u32::MAX - 36;

/// THE LANDING HALF OF A BLOW, shared by a melee swing and a projectile's impact (audit
/// 2026-10-02: projectiles skipped all of it — shields never caught a ranged hit, ranged attacks
/// never chipped Toughness, marks never stacked from a ranged basic). `total` is the already-
/// mitigated damage. Soaks it through shields, lands it, then the on-hit-taken / mark / spark /
/// Frost Aura riders and the Toughness chip + break. Returns what got through the shields.
#[allow(clippy::too_many_arguments)]
pub(crate) fn land_blow(
    entities: &mut [Entity],
    attacker_idx: usize,
    target_idx: usize,
    total: i32,
    dmg: &Damage,
    mob: u32,
    toughness_mult_pct: u32,
    hits: &mut Vec<HitEvent>,
) -> i32 {
    let dmg = *dmg;
    let through = {
        let t = &mut entities[target_idx];
        let thr = absorb_soak(t, total);
        // hp − damage, shatter Freeze, flag death — all in Entity::take_hit (the
        // ONE place a direct hit lands, so the freeze-break isn't duplicated).
        t.take_hit(thr);
        thr
    };
    // LIMIT BREAK: the blow feeds both clocks — the attacker's for landing it and
    // the target's for wearing it. A no-op for anyone without an Ultimate, which
    // is every monster.
    // Only the TAKEN side charges here: a blow is one event to whoever wears it,
    // however many bodies the attacker caught. The dealing side charges at the
    // call sites, which know whether this was a swing or a cast.
    if through > 0 {
        // The TAKEN charge is worth its `lb_on_hit_taken` for a blow of `LB_HIT_TAKEN_FULL_BP`
        // of max HP and scales down with lighter blows (0.0.434, user: Seraphine "โดนเดเมจเยอะจน
        // ชาจรัวๆ … แต่โดนเบามาก") — thirty scratches are not thirty wounds. Rounds down: a
        // blow under a tenth of the full-charge size charges nothing.
        let base = crate::skills::lb_gain(&entities[target_idx].skills, |d| d.lb_on_hit_taken);
        let max_hp = crate::combat::effective_max_hp(&entities[target_idx]).max(1) as i64;
        let share_bp = (through as i64) * 10_000 / max_hp;
        let gain = ((base as i64) * share_bp / LB_HIT_TAKEN_FULL_BP).min(u16::MAX as i64) as u16;
        crate::skills::charge_limit_break(&mut entities[target_idx], gain);
        crate::unit_rank::on_hit_taken(&mut entities[target_idx]);
        stamp_mark(entities, attacker_idx, target_idx);
        // UNDA VITAE (0.0.468, Astier): a share of what she deals is booked for the most wounded
        // ally; the tick layer spends it (a heal cannot run inside the damage path).
        let share = crate::skills::heal_on_hit_bp(&entities[attacker_idx]);
        if share > 0 {
            let amount = ((i64::from(through) * i64::from(share)) / 10_000).max(1).min(i32::MAX as i64) as i32;
            entities[attacker_idx].heal_debt += amount;
        }
        fire_static_spark(entities, attacker_idx, target_idx, hits);
        // FROST AURA (0.0.456): a hit on an aura carrier stamps the aura's source's mark on the
        // attacker — the caster's own Chill, as if she had landed the blow.
        // Only a hit from the OTHER side: friendly splash (a party AoE catching an ally) must not
        // chill the caster.
        let aura_src = if entities[attacker_idx].team != entities[target_idx].team {
            entities[target_idx].buffs.iter().find(|b| matches!(b.kind, crate::skills::BuffKind::FrostAura)).map(|b| b.source_id)
        } else {
            None
        };
        if let Some(src) = aura_src {
            if let Some(ci) = entities.iter().position(|e| e.id == src && e.alive) {
                if ci != attacker_idx {
                    stamp_mark(entities, ci, attacker_idx);
                }
            }
        }
    }
    // TOUGHNESS: every landed damage instance chips the gauge, twice as fast when
    // the attack's element is one the target is weak to. The weakness LIST does not
    // exist yet, so "weak to" is read from the element matrix that does — the same
    // question, answered by the table we have (`docs/element-weakness-system.md` §2).
    // Damage over time never chips: it ticks every second and would break the gauge
    // with nobody deciding to.
    // A KILLING blow does not also break (audit 2026-10-02): the break packet, its triggers and
    // the Regalia / reset / gauge riders ran on the corpse.
    if through > 0 && entities[target_idx].alive {
        // BLACKWATER MARK (0.0.463, user: "1x + 0.5x — ตราเพิ่ม 0.5x แบบไม่คิด element"): the seal
        // ADDS half a chip to the marker's hits whatever the element — a weak target chips
        // 1× + 0.5× (base 40 → 60), a target that is not weak chips 0 + 0.5× (→ 20). Written
        // as a build-up term on the one chip call: +magnitude on a weak target, −5000 (half of
        // the baseline) on one that is not.
        // BREAK FEVER (0.0.465): for its 15 s the owner's blows chip any bar, weakness or not.
        let fever = crate::skills::buff_sum(&entities[attacker_idx], crate::skills::BuffKind::BreakFever) > 0;
        let weak = entities[target_idx].is_weak_to(dmg.element) || fever;
        let mark = crate::skills::blackwater_mark_from(&entities[attacker_idx], &entities[target_idx]);
        let advantage = weak || mark.is_some();
        let mark_bp = match (weak, mark) {
            (true, Some(bp)) => bp,
            (false, Some(_)) => -5_000,
            _ => 0,
        };
        let buildup = stat_mods::break_buildup_bp(&entities[attacker_idx])
            .saturating_add(mark_bp)
            // Striking Stone (0.0.485): the bar only — this term never reaches HP damage.
            .saturating_add(crate::regalia_passive::toughness_bonus_bp(&entities[attacker_idx]));
        if entities[target_idx].chip_toughness(advantage, toughness_mult_pct, buildup) {
            // A BREAK IS THE SAME EVENT WHOEVER LANDS IT (user 2026-09-07): damage,
            // a stun, and a window where the target takes more from everyone. What
            // differs between characters is what they DO with the event — see the
            // break triggers — not what the event itself is worth.
            //
            // So the number is built from the PARTY's summed level and the gauge that broke.
            // Not from ATK (a break build and a damage build stay separate), and not from the
            // breaker's own level, or a support landing the last chip would be worth less
            // than a striker landing it. Dead members count: the party's break must not sag
            // while it is losing. `BreakDmgBp` is the one thing that scales it. It is still a
            // real packet: DEF, the element triangle and shields all apply.
            let party_level: i64 = entities
                .iter()
                .filter(|e| matches!(e.team, Team::Player) && !e.is_pet && e.owner_id == 0)
                .map(|e| i64::from(e.stats.level.max(1)))
                .sum();
            let gauge = ((entities[target_idx].toughness_max as i64 + 20) / 40).max(1);
            let base = (BREAK_DMG_FLOOR + BREAK_DMG_PER_PARTY_LEVEL * party_level.max(1)) * gauge;
            let bp = stat_mods::break_dmg_bp(&entities[attacker_idx]) as i64;
            let mut pkt = dmg;
            pkt.amount = (base * (10_000 + bp) / 10_000).clamp(1, i32::MAX as i64) as i32;
            let brk = apply_damage(&entities[target_idx], &pkt, mob);
            let dealt = {
                let t = &mut entities[target_idx];
                let thr = absorb_soak(t, brk);
                t.take_hit(thr);
                thr
            };
            // STUN + VULNERABILITY. Applied straight onto the target rather than
            // through the `cast_buff` path, because that path makes bosses immune to
            // Stun — a rule for skills, which a break is not. A boss that could not be
            // stunned would have no reason to carry the longer gauge it now carries.
            let ticks = if entities[target_idx].is_boss {
                crate::grid::BREAK_STUN_TICKS_BOSS
            } else {
                crate::grid::BREAK_STUN_TICKS
            };
            let src = entities[attacker_idx].id;
            let t = &mut entities[target_idx];
            // The STUN and the RECOVERY are two different clocks. `ticks` is the stun —
            // how long the target is out — and it drives the Stun and Vulnerability
            // buffs below. `break_ticks` is the recovery: how long the gauge takes to
            // come back, and how long it is immune to being chipped. Recovery is never
            // shorter than the stun, so a boss's longer stun is not undone by it.
            let recovery = crate::grid::BREAK_RECOVERY_TICKS.max(ticks);
            t.break_ticks = recovery.min(u8::MAX as u32) as u8;
            // ESCALATION: the next bar on this body is half again as big. A boss is
            // exempt — repeated breaks ARE the boss fight.
            if !t.is_boss {
                t.toughness_max = ((t.toughness_max as u32
                    * crate::grid::BREAK_GAUGE_ESCALATION_NUM
                    / crate::grid::BREAK_GAUGE_ESCALATION_DEN)
                    .min(u16::MAX as u32)) as u16;
            }
            crate::skills::apply_buff(t, crate::skills::Buff {
                kind: crate::skills::BuffKind::Stun,
                magnitude: 1,
                ticks,
                source_id: src,
            });
            // Its OWN source (audit 2026-10-02): on the breaker's id it shared a slot with that
            // unit's rank / skill vulnerabilities (Iron Chains, Rapis's meteor), so one replaced
            // the other.
            crate::skills::apply_buff(t, crate::skills::Buff {
                kind: crate::skills::BuffKind::DmgTakenPct,
                magnitude: crate::grid::BREAK_VULN_BP,
                ticks,
                source_id: BREAK_VULN_SRC,
            });
            hits.push(HitEvent {
                target_id: entities[target_idx].id,
                attacker_id: entities[attacker_idx].id,
                amount: dealt,
                hits: 1,
                kind: crate::state::HitKind::Break,
            });
            crate::regalia_passive::on_break(&mut entities[attacker_idx]);
        }
    }
    // A shield that ate the WHOLE blow → "Guarded" (no hp-delta means no number
    // would otherwise show the block landed).
    if total > 0 && through == 0 {
        hits.push(HitEvent {
            target_id: entities[target_idx].id,
            attacker_id: entities[attacker_idx].id,
            amount: 0,
            hits: 0,
            kind: crate::state::HitKind::Guard,
        });
    }
    through
}

pub fn resolve_attack_scaled(
    entities: &mut [Entity],
    rng: &mut Pcg32,
    attacker_idx: usize,
    target_idx: usize,
    power_bp: u32,
    display_hits: u32,
    hits: &mut Vec<HitEvent>,
    kind: DamageType,
    // The firing skill's `toughness_dmg_mult` (0 = a plain hit at the baseline).
    toughness_mult_pct: u32,
) -> i32 {
    let is_crit = match kind {
        DamageType::True => false, // no crit/miss/draws
        // 0.0.472 (user: MATK is gone, one ATK — magic and skills crit too): a magic hit rolls
        // its crit like any other, but it still cannot be dodged.
        DamageType::Magic => roll_crit(rng, effective_crit_chance_bp(&entities[attacker_idx])
            + crate::regalia_passive::crit_rate_bonus_bp(&entities[attacker_idx], &entities[target_idx])),
        DamageType::Physical => {
            // + a Regalia passive's crit rate against THIS target (0.0.488, Meridian: debuffed).
            let crit_chance = effective_crit_chance_bp(&entities[attacker_idx])
                + crate::regalia_passive::crit_rate_bonus_bp(&entities[attacker_idx], &entities[target_idx]);
            let crit = roll_crit(rng, crit_chance);
            if !crit && roll_dodge(rng, &entities[attacker_idx], &entities[target_idx]) {
                // DODGED — the whole blow, no hp-delta and so no number.
                hits.push(HitEvent {
                    target_id: entities[target_idx].id,
                    attacker_id: entities[attacker_idx].id,
                    amount: 0,
                    hits: 0,
                    kind: crate::state::HitKind::Dodge,
                });
                return 0;
            }
            crit
        }
    };
    let mut dmg = build_damage_scaled(&entities[attacker_idx], is_crit, power_bp, kind, false);
    // Regalia passive conditionals (0.0.435): debuffed target / range / low HP. 0 for everyone
    // without such a piece.
    let rb = crate::regalia_passive::hit_bonus_bp(&entities[attacker_idx], &entities[target_idx])
        + crate::skills::taunted_target_bp(&entities[attacker_idx], &entities[target_idx])
        + crate::skills::broken_target_bp(&entities[attacker_idx], &entities[target_idx]);
    if rb > 0 {
        dmg.amount = ((dmg.amount as i64) * (10_000 + rb) / 10_000).min(i32::MAX as i64) as i32;
    }
    // Attacker gear-mod conditional damage (dmg vs element/race/size — Hydra
    // lineage): one additive bucket vs THIS defender, multiplied pre-mitigation.
    let vs = stat_mods::dmg_vs_bp(
        &entities[attacker_idx],
        &entities[target_idx].weaknesses,
        stat_mods::race_of(&entities[target_idx]),
        stat_mods::size_of(&entities[target_idx]),
    );
    if vs > 0 {
        dmg.amount = ((dmg.amount as i64) * (10_000 + vs as i64) / 10_000) as i32;
    }
    // CARD SPECIAL (0.0.212) — the one attacker-side fold no StatMod family expresses:
    // a bonus keyed on the ATTACK's own element (Broomhilda/Burst Chom). DMG-vs-the-
    // DEFENDER's-element folded into `dmg_vs_bp` above with gear (0.0.243); crit damage
    // into the unified crit stat below (0.0.224). 0 without the card.
    let card_bp =
        super::card_specials::dmg_with_atk_element_bp(&entities[attacker_idx], dmg.element)
            + stat_mods::element_dmg_bp(&entities[attacker_idx], dmg.element);
    if card_bp > 0 {
        dmg.amount = ((dmg.amount as i64) * (10_000 + card_bp as i64) / 10_000) as i32;
    }
    // BOSS DAMAGE (0.0.391): its own multiply against a boss, 0 for everyone without the line.
    if entities[target_idx].is_boss {
        let boss_bp = stat_mods::dmg_vs_boss_bp(&entities[attacker_idx]);
        if boss_bp > 0 {
            dmg.amount = ((dmg.amount as i64) * (10_000 + boss_bp as i64) / 10_000) as i32;
        }
    }
    // CRIT DAMAGE (0.0.224 renewal-crit): its own multiply layer — ×150% base
    // + card/gear crit-dmg lines. Replaces the old def-ignore crit rule.
    if dmg.is_crit {
        // + a Regalia passive's crit damage against THIS target (0.0.488, Meridian: debuffed).
        let cd = i64::from(super::effective_crit_dmg_bp(&entities[attacker_idx]))
            + crate::regalia_passive::crit_dmg_bonus_bp(&entities[attacker_idx], &entities[target_idx]);
        dmg.amount = ((dmg.amount as i64) * cd / 10_000) as i32;
    }
    // CARD SPECIAL defender-side (legacy set): Greenling shaves 2.5→5% off BASIC
    // hits. `power_bp == 10_000` is the established basic-swing test (Double
    // Attack below uses the same); 0 without the card (goldens safe). Folding
    // into the packet (not the total) lets a Double Attack second hit inherit it.
    if power_bp == 10_000 {
        let red = super::card_specials::basic_dmg_taken_reduction_bp(&entities[target_idx]);
        if red > 0 {
            dmg.amount = ((dmg.amount as i64) * (10_000 - red as i64) / 10_000).max(1) as i32;
        }
    }
    // RO-style weapon variance: a connecting PHYSICAL hit rolls its damage DOWN
    // from max by up to weapon_level × 5% (one draw — placed AFTER crit/hit so the
    // stream stays simple). Magic keeps its zero-draw determinism (no variance).
    if matches!(entities[attacker_idx].atk_type, DamageType::Physical) {
        apply_weapon_variance(rng, &mut dmg.amount, &entities[attacker_idx]);
    }
    let mob = attackers_on(entities, target_idx);
    let mut total = apply_damage(&entities[target_idx], &dmg, mob);
    if matches!(entities[target_idx].team, Team::Player) {
        entities[target_idx].mitigated_fx += i64::from(dmg.amount) - i64::from(total);
    }
    let mut instances = 1u32;
    let _ = land_blow(entities, attacker_idx, target_idx, total, &dmg, mob, toughness_mult_pct, hits);
    // Double Attack (thief passive): a BASIC swing (power 100%) can strike
    // twice. The PRNG roll is taken ONLY when the attacker actually has
    // the passive (chance > 0) and the target survived the first hit — so
    // every other entity's draw stream stays byte-identical (goldens safe).
    if power_bp == 10_000 && entities[target_idx].alive {
        let dbl_bp = crate::skills::passive_double_atk_chance_bp(&entities[attacker_idx].skills);
        if dbl_bp > 0 && rng.gen_range_u32(0, 10_000) < dbl_bp {
            let dealt2 = apply_damage(&entities[target_idx], &dmg, mob);
            let t = &mut entities[target_idx];
            let through2 = absorb_soak(t, dealt2);
            t.hp = t.hp.saturating_sub(through2);
            if t.hp <= 0 {
                t.alive = false;
            }
            total += dealt2;
            instances = 2;
        }
    }
    // Record the swing for the renderer's multi-hit popup: one event per call,
    // showing `max(instances, display_hits)` numbers that sum to `total`
    // (Double Attack → 2, Sonic Blow → 8, a plain hit → 1).
    hits.push(HitEvent {
        target_id: entities[target_idx].id,
        attacker_id: entities[attacker_idx].id,
        amount: total,
        hits: instances.max(display_hits.max(1)),
        kind: if dmg.is_crit { crate::state::HitKind::Crit } else { crate::state::HitKind::Normal },
    });
    total
}

#[cfg(test)]
mod ward_tests {
    use super::{apply_damage, Damage};
    use crate::grid::{Cell, DamageType, Element, Entity};
    use crate::skills::{Buff, BuffKind};

    fn warded(reduce_bp: i32) -> Entity {
        let mut e = Entity::idle(1, Cell::new(0, 0));
        e.buffs.push(Buff { kind: BuffKind::DmgReducePct, magnitude: reduce_bp, ticks: 100, source_id: 1 });
        e
    }

    // A DEF-ignoring crit packet, so the ward's %-cut is the only modifier.
    fn crit(amount: i32) -> Damage {
        Damage {
            atk_level: 1,
            amount, element: Element::Physical, kind: DamageType::Physical,
            is_ranged: false, is_crit: true, is_aoe: false, mastery_bp: 0, shadow_bane_bp: 0, debuff_amp_bp: 0,
            atk_race: crate::grid::Race::Unknown, atk_size: crate::grid::Size::Medium,
            ignore_def_bp: 0,
        }
    }

    #[test]
    fn ward_cuts_incoming_damage() {
        let e = warded(2_500); // 25% reduction
        assert_eq!(apply_damage(&e, &crit(100), 0), 75);
        let mut bare = e.clone();
        bare.buffs.clear();
        assert_eq!(apply_damage(&bare, &crit(100), 0), 100, "no ward ⇒ full damage");
    }

    /// Guard (0.0.447): a flat −N comes off the FINAL number and floors at 1 — a stage-1 swing
    /// loses a third, a big one loses a rounding error, nothing goes below 1.
    #[test]
    fn flat_reduction_is_last_and_floors_at_one() {
        let mut e = warded(2_500);
        e.bound_card_mods.push(crate::items::StatMod::DmgReduceFlat(20));
        assert_eq!(apply_damage(&e, &crit(100), 0), 55, "75 after the ward, then −20");
        assert_eq!(apply_damage(&e, &crit(20), 0), 1, "floors at 1");
        assert_eq!(apply_damage(&e, &crit(20_000), 0), 14_980, "a rounding error late");
    }
}

#[cfg(test)]
mod elemental_mastery_tests {
    use super::*;
    use crate::grid::{Cell, Entity, Team};
    use crate::skills::SkillState;

    fn mage(mastery_lvl: u8) -> Entity {
        let mut e = Entity::idle(1, Cell::new(0, 0))
            .with_team(Team::Player)
            .with_combat(100, 0, 0, 500, Element::Fire);
        e.atk = 100; // MATK base
        e.atk_type = DamageType::Magic;
        if mastery_lvl > 0 {
            e.skills = vec![SkillState::new("elemental_mastery", mastery_lvl)];
        }
        e
    }
    fn target(el: Element) -> Entity {
        Entity::idle(2, Cell::new(1, 0)).with_team(Team::Monster).with_combat(9999, 0, 0, 0, el)
    }

    /// The player side has no weakness (0.0.359): a Fire unit takes 100% from Water — the element
    /// the retired matrix said beats it — and 100% from Earth, which it said was resisted; frozen
    /// or not. A MONSTER has no element either (0.0.444): no list = 100% from everything; a list
    /// is the whole rule.
    #[test]
    fn players_take_full_damage_from_every_element_and_monsters_keep_their_rules() {
        let player = Entity::idle(3, Cell::new(1, 0)).with_team(Team::Player).with_combat(9999, 0, 0, 0, Element::Fire);
        assert_eq!(element_mult_vs(&player, Element::Ice), NEUTRAL_BP, "was 150% on the matrix");
        assert_eq!(element_mult_vs(&player, Element::Earth), NEUTRAL_BP, "was 50% on the matrix");
        assert_eq!(element_mult_vs(&player, Element::Fire), NEUTRAL_BP);
        let mut frozen = player.clone();
        crate::skills::apply_buff(&mut frozen, crate::skills::Buff {
            kind: crate::skills::BuffKind::Freeze, magnitude: 1, ticks: 50, source_id: 9,
        });
        let water = Damage { element: Element::Fire, ..build_damage(&mage(0), false) };
        let thawed = apply_damage(&player, &water, 0);
        let iced = apply_damage(&frozen, &water, 0);
        assert_eq!(iced, thawed, "a frozen player is not a Water target");
        // Monsters: nothing without a list, the list with one.
        assert_eq!(element_mult_vs(&target(Element::Earth), Element::Fire), NEUTRAL_BP, "no element, no list, no matrix");
        let mut listed = target(Element::Earth);
        listed.weaknesses = vec![Element::Ice];
        assert_eq!(element_mult_vs(&listed, Element::Fire), NEUTRAL_BP);
        assert_eq!(element_mult_vs(&listed, Element::Ice), WEAKNESS_BP);
    }

    #[test]
    fn mastery_boosts_only_on_element_advantage() {
        // A target WEAK to Fire → 1.2x, +25% at level 10 on top.
        let mut weak = target(Element::Physical);
        weak.weaknesses = vec![Element::Fire];
        let base = apply_damage(&weak, &build_damage(&mage(0), false), 0);
        let mastered = apply_damage(&weak, &build_damage(&mage(10), false), 0);
        assert_eq!(mastered, base * 125 / 100, "level 10 = +25% vs a weak element");

        // No list (neutral) → NO mastery bonus.
        let n0 = apply_damage(&target(Element::Fire), &build_damage(&mage(0), false), 0);
        let n10 = apply_damage(&target(Element::Fire), &build_damage(&mage(10), false), 0);
        assert_eq!(n0, n10, "no bonus without an element advantage");

        // Resisted → NO mastery bonus either.
        let mut res = target(Element::Physical);
        res.resists = vec![Element::Fire];
        let d0 = apply_damage(&res, &build_damage(&mage(0), false), 0);
        let d10 = apply_damage(&res, &build_damage(&mage(10), false), 0);
        assert_eq!(d0, d10, "no bonus when at an element disadvantage");
    }
}

#[cfg(test)]
mod ammo_autoswap_tests {
    use super::*;
    use crate::items::AmmoStack;

    fn stack(element: Element, shots: u32) -> AmmoStack {
        AmmoStack { element, shots }
    }

    #[test]
    fn picks_the_advantaged_element_and_ignores_neutral_or_weak() {
        // Pool: Fire (beats Earth), Water (loses to Earth). Vs an Earth target
        // only Fire has the advantage → pick Fire, never spend the Water stack.
        let pool = [stack(Element::Ice, 10), stack(Element::Fire, 10)];
        assert_eq!(pick_ammo_idx(&pool, Element::Earth), Some(1));
        // Vs a Fire target, Water wins (Water > Fire); Fire-vs-Fire is neutral.
        assert_eq!(pick_ammo_idx(&pool, Element::Fire), Some(0));
        // Nothing in the pool beats a Water target (Fire loses, Water neutral)
        // → None ⇒ the archer shoots bare Neutral, spending no arrow.
        assert_eq!(pick_ammo_idx(&pool, Element::Ice), None);
    }

    #[test]
    fn skips_depleted_stacks_and_handles_two_winners() {
        // An empty advantaged stack is ignored (its shots ran out).
        let pool = [stack(Element::Fire, 0), stack(Element::Ice, 5)];
        assert_eq!(pick_ammo_idx(&pool, Element::Earth), None, "out of Fire → no spend");
        // If two loaded elements both beat the target, the higher multiplier
        // wins; here both are ×1.5 triangle wins so the earliest stack is taken
        // (deterministic tie-break, no PRNG).
        let two = [stack(Element::Fire, 3), stack(Element::Fire, 3)];
        assert_eq!(pick_ammo_idx(&two, Element::Earth), Some(0));
    }
}

#[cfg(test)]
mod one_armour_tests {
    use super::*;
    use crate::grid::{Cell, Team};

    /// THE TWO ARMOURS ARE ONE ARMOUR (0.0.85), AND MDEF IS GONE FROM THE CODE (0.0.332).
    ///
    /// This module used to sweep every built monster template and assert that MDEF's
    /// steeper curve did not mitigate magic harder than DEF mitigated physical — a
    /// real bug it caught twice while the two existed side by side. There is one DEF
    /// now, so the question cannot be asked; what is worth pinning instead is that a
    /// magic packet and a physical packet of the same size take the same mitigation.
    #[test]
    fn magic_and_physical_meet_the_same_armour() {
        let mut e = Entity::idle(1, Cell::new(0, 0)).with_team(Team::Monster);
        e.def = 400;
        e.mob_stage = 0;
        let packet = |kind: DamageType| Damage {
            amount: 1_000,
            element: Element::Physical,
            kind,
            is_ranged: false,
            is_crit: false,
            is_aoe: false,
            mastery_bp: 0,
            shadow_bane_bp: 0, debuff_amp_bp: 0,
            atk_level: 10,
            atk_race: crate::grid::Race::Unknown,
            atk_size: crate::grid::Size::Medium,
            ignore_def_bp: 0,
        };
        assert_eq!(
            apply_damage(&e, &packet(DamageType::Physical), 1),
            apply_damage(&e, &packet(DamageType::Magic), 1),
            "one DEF means one mitigation, whatever the blow is made of"
        );
    }
}
