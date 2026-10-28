//! GEAR-MOD effect layer (0.0.134) — aggregates the rolled ENCHANT lines
//! (`Item.bonuses`) across everything an entity wears, one additive bucket
//! per mod family (never compounding — §4.2-style: sum, then apply once).
//!
//! The "can zero a mechanic" families are CAPPED HERE at the published
//! ceilings (gear-systems-design §2): whatever future sources stack, the
//! consumption sites can trust the cap. Damage-increasing families are
//! deliberately uncapped (additive buckets self-dilute).
//!
//! The ROLLING systems (orbs) are not built yet — until then bonuses only
//! enter via tests/authored data, but every hook below is live.

// DEBT (2 sites): this file predates `deny(clippy::indexing_slicing)`. An index that
// cannot be proven in range is a panic, and in verify-server (which links this crate and
// replays saves under `panic = "abort"`) a panic is the whole process. Shrink this, never
// add to it — a NEW file has no allow and is rejected on its first bare index.
#![allow(clippy::indexing_slicing)]

use crate::grid::{Element, Entity, Race, Size};
use crate::items::StatMod;
use crate::potential::{effects as potential, PotentialStat};

/// Status codes for `StatMod::ResistStatusBp` (u8 param keeps the enum Copy +
/// compact). APPEND-ONLY.
pub const STATUS_FREEZE: u8 = 0;
pub const STATUS_POISON: u8 = 1;
pub const STATUS_BLIND: u8 = 2;
pub const STATUS_STUN: u8 = 3;

/// Published caps (basis points) — the "ลดจนเป็น 0 ได้" family. Applied SYMMETRICALLY
/// (`-CAP..=CAP`) since 0.0.246: a NEGATIVE line is a real drawback (heavy armor that slows
/// you, a cursed card that deepens an elemental weakness), not a typo to be silently zeroed —
/// and it is bounded by the same published magnitude as the bonus side.
pub const CAP_IGNORE_DEF_BP: i32 = 5_000;
pub const CAP_COOLDOWN_BP: i32 = 3_000;
pub const CAP_CAST_BP: i32 = 5_000;
pub const CAP_MOVE_SPEED_BP: i32 = 3_000;
pub const CAP_RESIST_ELEMENT_BP: i32 = 7_500;
pub const CAP_RESIST_STATUS_BP: i32 = 7_500;
pub const CAP_DR_BP: i32 = 5_000;

/// The EFFECTIVE race of an entity: players are RO Demi-Human — resolved here
/// (not stamped on the entity) so player serialization never changes and every
/// pre-0.0.134 golden stays byte-identical.
pub fn race_of(e: &Entity) -> Race {
    if matches!(e.team, crate::grid::Team::Player) && matches!(e.race, Race::Unknown) {
        Race::DemiHuman
    } else {
        e.race
    }
}

/// Effective size (players default Medium — already the enum default).
pub fn size_of(e: &Entity) -> Size {
    e.size
}

/// Iterate EVERY StatMod affecting `e`: equipped-gear `.bonuses`, the Exemplar / Regalia overlay
/// mods (recomputed on demand from their defs), every ACTIVE slotted CARD's mods (resolved at its
/// star + foil), and the permanent BOUND-card grant. ONE modifier pipeline for all five sources —
/// the unify that named this module `stat_mods`, not `gear_mods`, and as of 0.0.245 there is no
/// second stat vocabulary left in the sim. Order is gear → exemplar → regalia → cards → bound;
/// every consumer here SUMS, so order does not affect results.
fn for_each_mod(e: &Entity, mut f: impl FnMut(&StatMod)) {
    for it in &e.equipped {
        // The product's SUBSTAT lines first (stamped from def × grade — every copy identical,
        // which is what keeps gear tradeable), then any legacy per-instance line.
        for m in &it.lines {
            f(m);
        }
        for m in &it.bonuses {
            f(m);
        }
    }
    // GEMS (0.0.386): the owner's socketed lines, summed per stat across every worn piece and
    // clamped by `gems::cap_of` — one line per stat, so the caps hold whatever the piece count.
    crate::gems::for_each_gem_mod(e, &mut f);
    if let Some(ex) = &e.exemplar {
        for m in &ex.mods() {
            f(m);
        }
    }
    if let Some(r) = &e.regalia {
        for m in &r.mods() {
            f(m);
        }
    }
    // Slotted cards, gated by the ONE activation rule (panel order × feeding sockets).
    super::derived::for_each_active_card(e, &mut |card| {
        for m in &card.active_card_mods() {
            f(m);
        }
    });
    // BOUND cards: a permanent grant with no gear or socket behind it (0.0.245).
    for m in &e.bound_card_mods {
        f(m);
    }
    // TRAINING (0.0.406): the party's Gil ladders, stamped per tick by `training::apply`.
    for m in &e.training_mods {
        f(m);
    }
    // CRESTS (0.0.416): the disenchant ladders, stamped per tick by `crest::apply`.
    for m in &e.crest_mods {
        f(m);
    }
    // APEX ALLOCATION — the two-tier stat points, as StatMods, so every reader above folds them
    // for free (docs/apex-from-level-1-design.md). Empty for an unallocated unit → byte-identical.
    e.stats.for_each_apex_mod(&mut f);
}

fn sum_mods(e: &Entity, f: impl Fn(&StatMod) -> i32) -> i32 {
    let mut t: i32 = 0;
    for_each_mod(e, |m| t = t.saturating_add(f(m)));
    t
}


pub fn max_hp_flat(e: &Entity) -> i32 {
    sum_mods(e, |m| if let StatMod::MaxHp(v) = *m { v } else { 0 })
}
pub fn max_hp_bp(e: &Entity) -> i32 {
    sum_mods(e, |m| if let StatMod::MaxHpBp(v) = *m { v } else { 0 })
        .saturating_add(potential::stat_bp(e, PotentialStat::HpBp))
}
pub fn atk_bp(e: &Entity) -> i32 {
    sum_mods(e, |m| if let StatMod::AtkBp(v) = *m { v } else { 0 })
        .saturating_add(potential::stat_bp(e, PotentialStat::AtkBp))
}
pub fn crit_bp(e: &Entity) -> i32 {
    sum_mods(e, |m| if let StatMod::CritBp(v) = *m { v } else { 0 })
        .saturating_add(potential::stat_bp(e, PotentialStat::CritBp))
}

/// CRIT DAMAGE bonus lines (0.0.224) — additive into the crit multiplier's
/// 150% base alongside the card channel. Uncapped (offense-% family).
pub fn crit_dmg_bp(e: &Entity) -> i32 {
    sum_mods(e, |m| if let StatMod::CritDmgBp(v) = *m { v } else { 0 })
}
/// Flat hard DEF — joins the gear DEF sum BEFORE the DefPct multiplier (so buffs scale it),
/// the same stage the old card `def` field folded at.
pub fn def_flat(e: &Entity) -> i32 {
    sum_mods(e, |m| if let StatMod::DefFlat(v) = *m { v } else { 0 })
}
/// +% hard DEF (0.0.406, the Training Fortitude slot) — joins the `DefPct` buff bucket in
/// `effective_hard_def`, so it scales the flat adds exactly as Energy Coat does.
pub fn def_bp(e: &Entity) -> i32 {
    sum_mods(e, |m| if let StatMod::DefBp(v) = *m { v } else { 0 })
}
/// Flat ATK from every StatMod source. The `MatkFlat` twin folded in here from 0.0.330
/// until 0.0.336 deleted it — MATK has been ATK since 0.0.330.
pub fn atk_flat(e: &Entity) -> i32 {
    sum_mods(e, |m| if let StatMod::AtkFlat(v) = *m { v } else { 0 })
}
/// Flat damage taken −N (0.0.447) — read at the very end of `apply_damage`.
pub fn dmg_reduce_flat(e: &Entity) -> i32 {
    sum_mods(e, |m| if let StatMod::DmgReduceFlat(v) = *m { v } else { 0 })
}
pub fn heal_bp(e: &Entity) -> i32 {
    sum_mods(e, |m| if let StatMod::HealBp(v) = *m { v } else { 0 })
}

/// Attacker-side conditional damage vs the DEFENDER's attributes. WITHIN each
/// RO axis, lines add (two Fire pieces = one summed Fire bonus); ACROSS the
/// three axes the buckets MULTIPLY — +20% element +20% race +20% size = ×1.728,
/// not ×1.6 (user 2026-07-05: "เราจะ * เพราะเป็นกึ่ง incremental เกม" — stacking
/// all three on one target is the intended build payoff). Returns the COMBINED
/// bonus in bp so resolve sites keep their single ×(10000+vs)/10000 step.
/// Uncapped by design.
/// `d_weak` is the DEFENDER's weakness list (0.0.444 — a monster has no element): a
/// `DmgVsElement(Fire, v)` line pays against a monster WEAK to Fire.
pub fn dmg_vs_bp(attacker: &Entity, d_weak: &[Element], d_race: Race, d_size: Size) -> i32 {
    let el = sum_mods(attacker, |m| match *m {
        StatMod::DmgVsElement(x, v) if d_weak.contains(&x) => v,
        _ => 0,
    });
    let race = sum_mods(attacker, |m| match *m {
        StatMod::DmgVsRace(r, v) if r == d_race && d_race != Race::Unknown => v,
        _ => 0,
    });
    let size = sum_mods(attacker, |m| match *m {
        StatMod::DmgVsSize(z, v) if z == d_size => v,
        _ => 0,
    });
    let mut f: i64 = 10_000;
    for bp in [el, race, size] {
        f = f * (10_000 + bp as i64) / 10_000;
    }
    (f - 10_000) as i32
}

/// ATTACKER-side +% damage for attacks of THIS element — the attacker's own
/// element, the mirror of `DmgVsElement` which keys on the defender's. Lines for
/// the same element add. Uncapped, like the dmg-vs buckets.
pub fn element_dmg_bp(attacker: &Entity, el: Element) -> i32 {
    sum_mods(attacker, |m| match *m {
        StatMod::ElementDmgBp(x, v) if x == el => v,
        _ => 0,
    })
        .saturating_add(potential::element_dmg_bp(attacker, el))
}

/// +% damage against a boss. Uncapped by sum (the gem wall is `gems::cap_of`).
pub fn dmg_vs_boss_bp(attacker: &Entity) -> i32 {
    sum_mods(attacker, |m| if let StatMod::DmgVsBossBp(v) = *m { v } else { 0 })
}

/// Flat HP restored per regen window from lines (gems today).
pub fn hp_regen_flat(e: &Entity) -> i32 {
    sum_mods(e, |m| if let StatMod::HpRegenFlat(v) = *m { v } else { 0 })
}

/// +% Limit Break gauge earned from every source. Scales the GAIN; the ceiling is
/// the Ultimate's own `lb_max` and is not touched.
pub fn charge_rate_bp(e: &Entity) -> i32 {
    sum_mods(e, |m| if let StatMod::ChargeRateBp(v) = *m { v } else { 0 })
        .saturating_add(potential::stat_bp(e, PotentialStat::ChargeRateBp))
}

/// +% BREAK damage. Scales the break's own number (see `combat::damage`), which
/// is built from the attacker's level and the gauge that broke — never from ATK,
/// so a break build and a damage build are two different builds.
pub fn break_dmg_bp(e: &Entity) -> i32 {
    sum_mods(e, |m| if let StatMod::BreakDmgBp(v) = *m { v } else { 0 })
}

/// DODGE CHANCE in basis points — the chance this entity takes nothing at all from
/// a direct hit.
///
/// Sources stack INVERSE MULTIPLICATIVELY (`1 - (1-p1)(1-p2)...`), Diablo 4's rule.
/// That is where the diminishing return comes from, and it is why this stat needs no
/// entry on the cap list: the fifth line is worth less than the first by
/// construction, and affix lines alone can never reach 100%.
pub fn dodge_bp(e: &Entity) -> i32 {
    let mut remaining: i64 = 10_000;
    let mut fold = |v: i32| {
        let p = (v as i64).clamp(0, 10_000);
        remaining = remaining * (10_000 - p) / 10_000;
    };
    for_each_mod(e, |m| {
        if let StatMod::DodgeBp(v) = *m {
            fold(v);
        }
    });
    // A passive (Nimble Mastery) folds in as one more source, not as a separate
    // additive bucket — one rule for everything that grants dodge.
    let passive = crate::skills::passive_dodge_bp(&e.skills);
    if passive > 0 {
        fold(passive);
    }
    // A dodge BUFF is one more source too, never an additive bucket of its own — folded
    // PER BUFF, so two 50% marks are 75% and no stack of them ever reaches 100%.
    for b in &e.buffs {
        if matches!(b.kind, crate::skills::BuffKind::DodgeBpAdd) && b.magnitude > 0 {
            fold(b.magnitude);
        }
    }
    (10_000 - remaining) as i32
}

/// +% BREAK BUILD-UP. Scales how much of the Toughness gauge one landed hit
/// chips — the pair to `break_dmg_bp`, which scales what the break is worth when
/// it lands. ZZZ splits its anomaly stat the same way for the same reason: one
/// stat that did both would make every break build identical.
pub fn break_buildup_bp(e: &Entity) -> i32 {
    sum_mods(e, |m| if let StatMod::BreakBuildupBp(v) = *m { v } else { 0 })
}

// NO CAP on Break Build-up, deliberately (user 2026-09-08). The seven caps above are
// the whole list of stats that stop paying, and it is worth keeping that list short
// and memorable: a stat capped in a comment nobody re-reads is a stat that behaves
// differently from how it looks. The ARGUMENT for capping it stands — break is a rate,
// and the measured perfect-coverage case already sat at 2.36 breaks per kill with no
// build-up at all, so a big enough stack buys its way back to stun-lock. If that shows
// up in play, the cap goes HERE, beside the others, rather than being invented at the
// call site.

/// Defender-side elemental resist vs the incoming damage element (cap 75%).
pub fn resist_element_bp(defender: &Entity, el: Element) -> i32 {
    sum_mods(defender, |m| match *m {
        StatMod::ResistElement(x, v) if x == el => v,
        _ => 0,
    })
    .clamp(-CAP_RESIST_ELEMENT_BP, CAP_RESIST_ELEMENT_BP)
}

/// DEF penetration. There has been one DEF to penetrate since 0.0.85; the `IgnoreMdefBp`
/// twin folded in here from 0.0.330 until 0.0.335 deleted the variant.
pub fn ignore_def_bp(attacker: &Entity) -> i32 {
    sum_mods(attacker, |m| match *m {
        StatMod::IgnoreDefBp(v) => v,
        _ => 0,
    })
    .clamp(-CAP_IGNORE_DEF_BP, CAP_IGNORE_DEF_BP)
}
pub fn cooldown_bp(e: &Entity) -> i32 {
    sum_mods(e, |m| if let StatMod::CooldownBp(v) = *m { v } else { 0 }).clamp(-CAP_COOLDOWN_BP, CAP_COOLDOWN_BP)
}
pub fn cast_bp(e: &Entity) -> i32 {
    sum_mods(e, |m| if let StatMod::CastBp(v) = *m { v } else { 0 }).clamp(-CAP_CAST_BP, CAP_CAST_BP)
}
pub fn move_speed_bp(e: &Entity) -> i32 {
    sum_mods(e, |m| if let StatMod::MoveSpeedBp(v) = *m { v } else { 0 })
        .clamp(-CAP_MOVE_SPEED_BP, CAP_MOVE_SPEED_BP)
}

/// Defender-side damage reduction vs the ATTACKER's race/size (+ ranged) —
/// one additive bucket, capped at 50% combined so full dedication never
/// reaches immunity (that's unique/card territory).
pub fn dr_bp(defender: &Entity, a_race: Race, a_size: Size, ranged: bool) -> i32 {
    sum_mods(defender, |m| match *m {
        StatMod::DrVsRace(r, v) if r == a_race && a_race != Race::Unknown => v,
        StatMod::DrVsSize(z, v) if z == a_size => v,
        StatMod::DrVsRanged(v) if ranged => v,
        _ => 0,
    })
    .clamp(-CAP_DR_BP, CAP_DR_BP)
}

/// Defender-side status resist (cap 75%) — applied as a DURATION cut on the
/// inflicted status, not a block chance: deterministic, zero extra PRNG draws
/// (deliberate deviation from RO's chance-roll — noted in gear-systems-design).
pub fn resist_status_bp(defender: &Entity, code: u8) -> i32 {
    let from_gear = sum_mods(defender, |m| match *m {
        StatMod::ResistStatusBp(c, v) if c == code => v,
        _ => 0,
    });
    // BUFFS are a source too, or a Meal could not ward anything: `sum_mods` walks gear, cards and
    // overlays only. Timed wards live as their own BuffKind because a buff carries one magnitude
    // and cannot also carry a status code.
    let from_buffs: i32 = if code == STATUS_POISON {
        crate::skills::buff_sum(defender, crate::skills::BuffKind::ResistPoisonBp)
    } else {
        0
    };
    (from_gear + from_buffs).clamp(-CAP_RESIST_STATUS_BP, CAP_RESIST_STATUS_BP)
}

/// Scale a skill cooldown by the wearer's CDR bucket (cap 30%). Floors at 1
/// tick — CDR can shorten, never erase, a cooldown. Applied at EVERY arm site
/// (0.0.387): the standard cast path and the bespoke timers (Dupla Medicina,
/// Rivus, Subvenio, Taunt, Flame Shield's break-cooldown) — one rule, so a CDR
/// line is worth the same on every skill.
pub fn scale_cooldown(e: &Entity, cd: u32) -> u32 {
    let bp = cooldown_bp(e);
    if bp == 0 || cd == 0 {
        return cd;
    }
    (((cd as i64) * (10_000 - bp as i64)) / 10_000).max(1) as u32
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::combat::{self, apply_damage, build_damage};
    use crate::grid::{Cell, Team};
    use crate::items::{Item, ItemKind, Rarity};

    /// An entity wearing one gear piece that carries the given mods.
    fn wearer(mods: Vec<StatMod>) -> Entity {
        let mut e = Entity::idle(1, Cell::new(0, 0)).with_team(Team::Player);
        e.stats.level = 10;
        let mut it = Item::new("t", ItemKind::Armor, 0, 5, Rarity::Common);
        it.bonuses = mods;
        e.equipped.push(it);
        e
    }

    #[test]
    fn flat_and_percent_buckets_apply_once() {
        // ATK%: two lines sum additively (6% + 4% = one 10% multiply).
        let mut e = wearer(vec![StatMod::AtkBp(600), StatMod::AtkBp(400)]);
        e.atk = 100;
        let mut plain = e.clone();
        plain.equipped[0].bonuses.clear();
        let gain = combat::effective_atk(&e) - combat::effective_atk(&plain);
        assert_eq!(gain, combat::effective_atk(&plain) / 10, "additive 10%, not compounded");
        // MaxHP flat + %
        let h = wearer(vec![StatMod::MaxHp(200), StatMod::MaxHpBp(1000)]);
        let mut hp = h.clone();
        hp.equipped[0].bonuses.clear();
        let (with, without) = (combat::effective_max_hp(&h), combat::effective_max_hp(&hp));
        assert_eq!(with, (without + 200) * 11_000 / 10_000);
        // CRIT and DODGE are separate families and neither leaks into the other.
        let f = wearer(vec![StatMod::CritBp(300)]);
        assert_eq!(crit_bp(&f), 300);
        assert_eq!(crate::combat::stat_mods::dodge_bp(&f), 0, "a crit line is not dodge");
        // Flat DEF joins BEFORE the DefPct multiplier (0.0.243 — the CardEffect gap).
        let d = wearer(vec![StatMod::DefFlat(7)]);
        let mut plain_d = d.clone();
        plain_d.equipped[0].bonuses.clear();
        assert_eq!(def_flat(&d), 7);
        assert_eq!(combat::effective_def(&d) - combat::effective_def(&plain_d), 7);
    }

    #[test]
    fn exemplar_and_regalia_feed_the_same_pipeline_as_gear() {
        // The 0.0.242 unify: an overlay's grant is recomputed from its def and walked by
        // `for_each_mod` exactly like a gear line — no separate `card_bonuses` fold.
        use crate::items::{Exemplar, Regalia};
        let mut e = Entity::idle(1, Cell::new(0, 0)).with_team(Team::Player);
        e.stats.level = 10;
        let bare_atk = combat::effective_atk(&e);

        // Seraphine (SSR of the swordsman line) 0★ = +8% crit DMG / +40 flat ATK / +8% ATK.
        e.exemplar = Some(Exemplar { id: "seraphine".to_string(), ..Default::default() });
        assert_eq!(crit_dmg_bp(&e), 800);
        assert_eq!(atk_flat(&e), 40);
        assert_eq!(atk_bp(&e), 800);
        // 2★ = base + two ranks (+1.6% crit DMG / +8 ATK each); the percentage does not move.
        e.exemplar.as_mut().unwrap().stars = 2;
        assert_eq!(crit_dmg_bp(&e), 800 + 320);
        assert_eq!(atk_flat(&e), 40 + 16);
        assert_eq!(atk_bp(&e), 800, "stars never move a rarity's percentage");

        // Aurora Edge (SSR of the swordsman line) Lv0/0★, on TOP of the Exemplar — both overlays
        // sum. A Regalia is a fixed LUMP of ATK plus one advanced stat, both on its OWN level.
        let ex_only = atk_flat(&e);
        let ex_cd = crit_dmg_bp(&e);
        e.regalia = Some(Regalia { id: "aurora_edge".to_string(), ..Default::default() });
        assert_eq!(atk_flat(&e), ex_only + 200, "SSR lump at Lv1");
        assert_eq!(atk_bp(&e), 800, "the hero's percent; the signature's advanced stat is DEF%");
        // 0.0.435: a SIGNATURE piece carries its unit's doctrine stat — Aurora Edge is DEF%
        // (Sentinel), not the sword line's crit dmg; the R/SR rungs keep the line's.
        assert_eq!(crit_dmg_bp(&e), ex_cd, "no crit dmg on the signature piece");
        assert_eq!(def_bp(&e), 880, "SSR advanced stat at Lv1");
        assert_eq!(max_hp_bp(&e), 0, "no HP on a sword-line piece");

        // THE LADDER RULE: the level scales both numbers on the piece's own curve; Resonance
        // ranks are empty today. The tier is the piece — an R at Lv60 is 1,500, an SSR 3,000.
        let r = e.regalia.as_mut().unwrap();
        r.level = crate::items::REGALIA_BASE_CAP;
        r.stars = 1;
        let lump60 = crate::items::regalia_lump_at(200, crate::items::REGALIA_BASE_CAP);
        assert!((2_900..=3_100).contains(&lump60), "SSR lump at Lv60 ≈ 3,000: {lump60}");
        assert_eq!(atk_flat(&e), ex_only + lump60);
        let adv60 = crate::items::regalia_advanced_at(880, crate::items::REGALIA_BASE_CAP);
        assert_eq!(def_bp(&e), adv60);
        assert!((2_100..=2_300).contains(&adv60), "≈ +22% at Lv60: {adv60}");
        // Lv70 (0.0.490) is the AUTHORED value, not a percentage of Lv60.
        assert_eq!(crate::items::regalia_lump_at(200, crate::items::REGALIA_MAX_LEVEL), 3_600);
        assert_eq!(crate::items::regalia_advanced_at(880, crate::items::REGALIA_MAX_LEVEL), 2_600);
        // …and the whole stack lands on the real ATK number.
        assert!(combat::effective_atk(&e) > bare_atk);

        // An UNKNOWN overlay id grants nothing (replay-safe on data skew).
        e.exemplar = Some(Exemplar { id: "no_such_hero".to_string(), stars: 5, ..Default::default() });
        e.regalia = None;
        assert_eq!(atk_flat(&e), 0);
        assert_eq!(atk_bp(&e), 0);
    }

    #[test]
    fn conditional_damage_axes_multiply() {
        // The user's canonical example: +20% on each axis → ×1.728, not ×1.6.
        let a = wearer(vec![
            StatMod::DmgVsElement(Element::Fire, 2000),
            StatMod::DmgVsRace(Race::Plant, 2000),
            StatMod::DmgVsSize(Size::Small, 2000),
        ]);
        assert_eq!(dmg_vs_bp(&a, &[Element::Fire], Race::Plant, Size::Small), 7280);
        // Mixed values: 1.10 x 1.15 x 1.05 = 1.32825 → 3282 bp (integer trunc).
        let b = wearer(vec![
            StatMod::DmgVsElement(Element::Fire, 1000),
            StatMod::DmgVsRace(Race::Plant, 1500),
            StatMod::DmgVsSize(Size::Small, 500),
        ]);
        assert_eq!(dmg_vs_bp(&b, &[Element::Fire], Race::Plant, Size::Small), 3282);
        // Partial match: a single axis is just that axis's bonus.
        assert_eq!(dmg_vs_bp(&b, &[Element::Ice], Race::Plant, Size::Large), 1500);
        // Unknown race can never match (no accidental bonus vs legacy data)
        let u = wearer(vec![StatMod::DmgVsRace(Race::Unknown, 9999)]);
        assert_eq!(dmg_vs_bp(&u, &[Element::Fire], Race::Unknown, Size::Small), 0);
    }

    #[test]
    fn zero_out_family_caps_hold() {
        let e = wearer(vec![
            StatMod::IgnoreDefBp(9_000),
            StatMod::CooldownBp(9_000),
            StatMod::CastBp(9_000),
            StatMod::MoveSpeedBp(9_000),
            StatMod::ResistElement(Element::Fire, 9_000),
            StatMod::DrVsRanged(9_000),
            StatMod::ResistStatusBp(STATUS_FREEZE, 9_000),
        ]);
        assert_eq!(ignore_def_bp(&e), CAP_IGNORE_DEF_BP);
        assert_eq!(cooldown_bp(&e), CAP_COOLDOWN_BP);
        assert_eq!(cast_bp(&e), CAP_CAST_BP);
        assert_eq!(move_speed_bp(&e), CAP_MOVE_SPEED_BP);
        assert_eq!(resist_element_bp(&e, Element::Fire), CAP_RESIST_ELEMENT_BP);
        assert_eq!(dr_bp(&e, Race::Unknown, Size::Small, true), CAP_DR_BP);
        assert_eq!(resist_status_bp(&e, STATUS_FREEZE), CAP_RESIST_STATUS_BP);
    }

    /// 0.0.246: a NEGATIVE line is a real drawback, not a typo. It survives the aggregator
    /// (symmetric `-CAP..=CAP`) AND bites at the consumer — every consumption site used to
    /// guard on `> 0`, which would have swallowed the whole feature silently.
    /// 0.0.247: the MATK party-aura channel is wired end-to-end but NO skill grants it yet.
    /// Pin both halves so the plumbing can't rot before its skill arrives: it must fold into
    /// `effective_atk` when set, and it must be 0 (and cost nothing) for everyone today.
    /// A tooltip must show ONE line per family. The star ladder emits one entry per source
    /// (base ++ step ++ each rank bump), which is right for the summing pipeline and wrong for
    /// display — Brutus at 5★ read "+5 STR +1 STR +1 STR +1 STR" in game (2026-07-30).
    #[test]
    fn merged_sums_a_family_but_keeps_parameters_apart() {
        use crate::grid::Element;
        // Brutus' real ladder: base 5 STR + a +1 bump at 1★/3★/5★.
        let brutus = [StatMod::AtkFlat(5), StatMod::AtkFlat(1), StatMod::AtkFlat(1), StatMod::AtkFlat(1)];
        assert_eq!(StatMod::merged(&brutus), vec![StatMod::AtkFlat(8)]);

        // Different PARAMETERS of the same variant must NOT merge…
        let elems = [
            StatMod::DmgVsElement(Element::Fire, 1_000),
            StatMod::DmgVsElement(Element::Ice, 500),
            StatMod::DmgVsElement(Element::Fire, 200),
        ];
        assert_eq!(
            StatMod::merged(&elems),
            vec![
                StatMod::DmgVsElement(Element::Fire, 1_200),
                StatMod::DmgVsElement(Element::Ice, 500),
            ],
            "same element sums, different elements stay apart, first-seen order kept"
        );

        // …and merging never changes what the PIPELINE computes.
        let e = wearer(elems.to_vec());
        let m = wearer(StatMod::merged(&elems));
        let (race, size) = (race_of(&e), size_of(&e));
        assert_eq!(
            dmg_vs_bp(&e, &[Element::Fire], race, size),
            dmg_vs_bp(&m, &[Element::Fire], race, size)
        );
    }

    #[test]
    fn atk_aura_folds_flat_and_pre_pct() {
        let mut e = Entity::idle(1, Cell::new(0, 0)).with_team(Team::Player);
        e.stats.level = 10;
        let before = combat::effective_atk(&e);
        e.aura_atk = 25;
        assert_eq!(combat::effective_atk(&e) - before, 25, "flat, before the % fold");
    }

    #[test]
    fn negative_lines_are_drawbacks_not_no_ops() {
        // Aggregator: negatives pass through, floored at −CAP (same magnitude as the bonus).
        let e = wearer(vec![
            StatMod::ResistElement(Element::Fire, -2_000),
            StatMod::MoveSpeedBp(-9_000), // beyond the floor
            StatMod::CastBp(-1_000),
            StatMod::CooldownBp(-1_000),
            StatMod::ResistStatusBp(STATUS_FREEZE, -2_000),
            StatMod::DrVsRanged(-1_500),
        ]);
        assert_eq!(resist_element_bp(&e, Element::Fire), -2_000);
        assert_eq!(move_speed_bp(&e), -CAP_MOVE_SPEED_BP, "floored at −CAP, not zeroed");
        assert_eq!(cast_bp(&e), -1_000);
        assert_eq!(dr_bp(&e, Race::Unknown, Size::Small, true), -1_500);

        // Consumer 1 — an elemental WEAKNESS raises incoming damage.
        let mut attacker = Entity::idle(4, Cell::new(0, 0)).with_team(Team::Player);
        attacker.atk = 100;
        attacker.is_ranged = true;
        let weak = wearer(vec![StatMod::ResistElement(attacker.element, -2_000)]);
        let mut bare = weak.clone();
        bare.equipped[0].bonuses.clear();
        let pkt = build_damage(&attacker, false);
        assert_eq!(
            apply_damage(&weak, &pkt, 0),
            apply_damage(&bare, &pkt, 0) * 12_000 / 10_000,
            "−20% resist = ×1.2 damage taken"
        );

        // Consumer 2 — a negative CDR LENGTHENS the cooldown (and never wraps).
        let slow = wearer(vec![StatMod::CooldownBp(-2_000)]);
        assert_eq!(scale_cooldown(&slow, 100), 120);

        // Consumer 3 — negative cast bp lengthens the cast. Guards the `as u64` trap: the
        // old unsigned cast turned −1000 into ~1.8e19 and blew the subtraction away.
        let caster = wearer(vec![StatMod::CastBp(-5_000)]);
        let mut plain = caster.clone();
        plain.equipped[0].bonuses.clear();
        let (slowed, normal) = (
            crate::combat::effective_cast_time(&caster, 100),
            crate::combat::effective_cast_time(&plain, 100),
        );
        assert!(slowed > normal, "negative cast bp must LENGTHEN: {slowed} vs {normal}");
    }

    #[test]
    fn ignore_def_raises_damage_through_the_pipeline() {
        let mut attacker = wearer(vec![StatMod::IgnoreDefBp(5_000)]);
        attacker.atk = 100;
        let mut naked = attacker.clone();
        naked.equipped[0].bonuses.clear();
        let mut tank = Entity::idle(3, Cell::new(1, 0));
        tank.def = 200; // hard def
        let with = apply_damage(&tank, &build_damage(&attacker, false), 0);
        let without = apply_damage(&tank, &build_damage(&naked, false), 0);
        assert!(with > without, "50% def pen must out-damage none ({with} vs {without})");
    }

    #[test]
    fn defender_resist_and_dr_cut_damage() {
        let mut attacker = Entity::idle(4, Cell::new(0, 0)).with_team(Team::Player);
        attacker.atk = 100;
        // ranged attacker for the DrVsRanged check
        attacker.is_ranged = true;
        let defender = wearer(vec![
            StatMod::ResistElement(attacker.element, 2_000),
            StatMod::DrVsRanged(3_000),
            StatMod::DrVsRace(Race::DemiHuman, 1_000),
        ]);
        let mut bare = defender.clone();
        bare.equipped[0].bonuses.clear();
        let pkt = build_damage(&attacker, false);
        let cut = apply_damage(&defender, &pkt, 0);
        let full = apply_damage(&bare, &pkt, 0);
        // resist 20% then DR (30% ranged + 10% vs DemiHuman = 40%, capped fine)
        assert_eq!(cut, full * 8_000 / 10_000 * 6_000 / 10_000);
    }

    #[test]
    fn cooldown_cast_and_status_hooks() {
        let e = wearer(vec![
            StatMod::CooldownBp(3_000),
            StatMod::CastBp(5_000),
            StatMod::ResistStatusBp(STATUS_POISON, 5_000),
        ]);
        assert_eq!(scale_cooldown(&e, 100), 70, "30% CDR");
        assert_eq!(scale_cooldown(&e, 0), 0);
        // cast: 50% cap halves whatever the DEX/INT-reduced windup would be.
        let mut plain = e.clone();
        plain.equipped[0].bonuses.clear();
        let base_ct = crate::combat::effective_cast_time(&plain, 20) as i64;
        assert_eq!(
            crate::combat::effective_cast_time(&e, 20) as i64,
            (base_ct * 5_000 / 10_000).max(1)
        );
        // status duration: poison 100 ticks → 50
        let mut victim = e.clone();
        crate::skills::apply_buff(
            &mut victim,
            crate::skills::Buff {
                kind: crate::skills::BuffKind::Poison,
                magnitude: 1,
                ticks: 100,
                source_id: 9,
            },
        );
        let b = victim.buffs.iter().find(|b| matches!(b.kind, crate::skills::BuffKind::Poison));
        assert_eq!(b.unwrap().ticks, 50);
    }

    #[test]
    fn flat_atk_and_matk_join_before_the_pct_fold() {
        let mut e = wearer(vec![StatMod::AtkFlat(25), StatMod::AtkBp(1_000)]);
        e.atk = 100;
        let mut plain = e.clone();
        plain.equipped[0].bonuses.clear();
        // (base+flat) × 110% vs base × 100% — flat folds BEFORE the pct bucket.
        let with = combat::effective_atk(&e);
        let without = combat::effective_atk(&plain);
        assert_eq!(with, (without + 25) * 11_000 / 10_000);
        // MATK reads the same pool: one flat line moves both numbers by the same amount.
        let m = wearer(vec![StatMod::AtkFlat(30)]);
        let mut mp = m.clone();
        mp.equipped[0].bonuses.clear();
        assert_eq!(combat::effective_atk(&m) - combat::effective_atk(&mp), 30);
    }

    #[test]
    fn heal_bp_scales_heal_output() {
        assert_eq!(heal_bp(&wearer(vec![StatMod::HealBp(1_500)])), 1_500);
    }
    #[test]
    fn element_dmg_keys_on_the_attackers_element_and_charge_rate_scales_the_gain() {
        use crate::grid::Element;
        // ElementDmgBp is the ATTACKER's element — the mirror of DmgVsElement, which
        // keys on the defender's. A Fire line must not pay out on a Water attack.
        let e = wearer(vec![StatMod::ElementDmgBp(Element::Fire, 1_500)]);
        assert_eq!(element_dmg_bp(&e, Element::Fire), 1_500);
        assert_eq!(element_dmg_bp(&e, Element::Ice), 0);
        assert_eq!(dmg_vs_bp(&e, &[Element::Fire], crate::grid::Race::Unknown,
                             crate::grid::Size::Medium), 0,
            "an own-element line must not leak into the defender-keyed bucket");

        // Charge rate scales a gain and never the ceiling.
        let mut c = wearer(vec![StatMod::ChargeRateBp(5_000)]);
        assert_eq!(charge_rate_bp(&c), 5_000);
        c.skills = vec![crate::skills::SkillState::new("aegis", 1)];
        crate::skills::charge_limit_break(&mut c, 10);
        assert_eq!(c.limit_break, 15, "+50% charge rate turns 10 into 15");
        let max = crate::skills::ultimate_of(&c.skills).map(|d| d.lb_max).unwrap_or(0);
        crate::skills::charge_limit_break(&mut c, max);
        assert_eq!(c.limit_break, max, "the ceiling is still lb_max");
    }

}
