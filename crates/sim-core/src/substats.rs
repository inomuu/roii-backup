//! SUBSTATS — the authored lines a piece of gear carries at its grade (0.0.384).
//!
//! The rule is Task Bar Hero's, read off its datamine (`docs/tbh-substat-datamine.md`): a line
//! belongs to the PRODUCT (def × grade), never to the copy — every copy of "Royal Adamant Saber"
//! carries the same lines, which is what lets a market price it by name. How many lines, which
//! stats a slot may carry, how big each is at each tier, and where each stat caps all live in
//! the catalog (`loot_rules.substats`), so the whole system is tuned by editing one JSON block.
//! This module is only the resolver.
//!
//! The pool is DELIBERATELY small: ten stats the post-pivot combat actually reads (design
//! `tbh-substat-datamine.md` §7). The RO-era families (damage vs race / size, status resist,
//! ignore-DEF…) that the old roller carried are gone — they belonged to a stat model that no
//! longer exists.
//!
//! Determinism: the picks are a pure function of (def id × grade) — an FNV-1a seed into a local
//! PRNG — so no game draw is taken and the table is the same on every machine. The catalog is
//! digest-pinned, so a replay on the server resolves the same lines the client did.

use serde::{Deserialize, Serialize};

use crate::data::SubstatRules;
use crate::items::{EquipSlot, Grade, Item, StatMod};
use crate::rng::Pcg32;

/// The stat keys the catalog may name, and the `StatMod` each becomes. Adding one here is the
/// ONLY code a new substat needs; the values, the slots and the caps are the catalog's.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum SubstatKey {
    #[default]
    Atk,
    AtkPct,
    Crit,
    CritDmg,
    Cdr,
    Hp,
    HpPct,
    Def,
    MoveSpeed,
    Dodge,
    // Our own, not TBH's — reachable from the catalog whenever the design wants them.
    BreakBuildup,
    BreakDmg,
    ChargeRate,
    /// +% healing DONE (`StatMod::HealBp`) — gem-only today (the Fortitude set), so an armour
    /// gem can be the healer's piece without a third HP line.
    Heal,
    /// Ignore DEF (`StatMod::IgnoreDefBp`, consumed in `combat::damage`) — gem-only, added so
    /// eight sets can carry sixteen DIFFERENT stats (user 2026-09-16: "ดูให้ไม่มีอันไหนซ้ำเลย").
    IgnoreDef,
    /// RETIRED before it shipped (user 2026-09-16: the live kits have no cast time; skill tempo
    /// is Haste = `Cdr`). Deserializes; no set names it (`catalog_hygiene` guards).
    CastSpeed,
    /// +% damage with ONE element (`StatMod::ElementDmgBp`) — the only key that takes a
    /// parameter (`GemStat::element`). The three Arcana gems each carry two of these, one per
    /// element of their pair (user 2026-09-16: Twilight = Holy + Shadow, Fervor = Fire +
    /// Physical, Storm = Water + Thunder).
    ElementDmg,
    /// +% damage vs bosses (`StatMod::DmgVsBossBp`) and flat HP per regen window
    /// (`StatMod::HpRegenFlat`) — 0.0.391.
    BossDmg,
    HpRegen,
}

impl SubstatKey {
    /// Every key, in declaration order — the index space `gems::for_each_gem_mod` sums over.
    pub const ALL: [SubstatKey; 19] = [
        SubstatKey::Atk, SubstatKey::AtkPct, SubstatKey::Crit, SubstatKey::CritDmg, SubstatKey::Cdr,
        SubstatKey::Hp, SubstatKey::HpPct, SubstatKey::Def, SubstatKey::MoveSpeed, SubstatKey::Dodge,
        SubstatKey::BreakBuildup, SubstatKey::BreakDmg, SubstatKey::ChargeRate, SubstatKey::Heal,
        SubstatKey::IgnoreDef, SubstatKey::CastSpeed, SubstatKey::ElementDmg, SubstatKey::BossDmg, SubstatKey::HpRegen,
    ];

    /// The key a line belongs to, for the lines this vocabulary can produce; `None` for any
    /// other `StatMod` (a card's race line, a retired stat).
    pub fn of(m: &StatMod) -> Option<SubstatKey> {
        Some(match *m {
            StatMod::AtkFlat(_) => SubstatKey::Atk,
            StatMod::AtkBp(_) => SubstatKey::AtkPct,
            StatMod::CritBp(_) => SubstatKey::Crit,
            StatMod::CritDmgBp(_) => SubstatKey::CritDmg,
            StatMod::CooldownBp(_) => SubstatKey::Cdr,
            StatMod::MaxHp(_) => SubstatKey::Hp,
            StatMod::MaxHpBp(_) => SubstatKey::HpPct,
            StatMod::DefFlat(_) => SubstatKey::Def,
            StatMod::MoveSpeedBp(_) => SubstatKey::MoveSpeed,
            StatMod::DodgeBp(_) => SubstatKey::Dodge,
            StatMod::BreakBuildupBp(_) => SubstatKey::BreakBuildup,
            StatMod::BreakDmgBp(_) => SubstatKey::BreakDmg,
            StatMod::ChargeRateBp(_) => SubstatKey::ChargeRate,
            StatMod::HealBp(_) => SubstatKey::Heal,
            StatMod::IgnoreDefBp(_) => SubstatKey::IgnoreDef,
            StatMod::CastBp(_) => SubstatKey::CastSpeed,
            StatMod::ElementDmgBp(_, _) => SubstatKey::ElementDmg,
            StatMod::DmgVsBossBp(_) => SubstatKey::BossDmg,
            StatMod::HpRegenFlat(_) => SubstatKey::HpRegen,
            _ => return None,
        })
    }

    /// `make`, with the element a parameterised key needs.
    pub fn make_for(self, element: Option<crate::grid::Element>, v: i32) -> StatMod {
        match (self, element) {
            (SubstatKey::ElementDmg, Some(el)) => StatMod::ElementDmgBp(el, v),
            _ => self.make(v),
        }
    }

    pub fn make(self, v: i32) -> StatMod {
        match self {
            SubstatKey::Atk => StatMod::AtkFlat(v),
            SubstatKey::AtkPct => StatMod::AtkBp(v),
            SubstatKey::Crit => StatMod::CritBp(v),
            SubstatKey::CritDmg => StatMod::CritDmgBp(v),
            SubstatKey::Cdr => StatMod::CooldownBp(v),
            SubstatKey::Hp => StatMod::MaxHp(v),
            SubstatKey::HpPct => StatMod::MaxHpBp(v),
            SubstatKey::Def => StatMod::DefFlat(v),
            SubstatKey::MoveSpeed => StatMod::MoveSpeedBp(v),
            SubstatKey::Dodge => StatMod::DodgeBp(v),
            SubstatKey::BreakBuildup => StatMod::BreakBuildupBp(v),
            SubstatKey::BreakDmg => StatMod::BreakDmgBp(v),
            SubstatKey::ChargeRate => StatMod::ChargeRateBp(v),
            SubstatKey::Heal => StatMod::HealBp(v),
            SubstatKey::IgnoreDef => StatMod::IgnoreDefBp(v),
            SubstatKey::CastSpeed => StatMod::CastBp(v),
            // Element-less here: Neutral. `make_for` is the gem path that supplies the element.
            SubstatKey::ElementDmg => StatMod::ElementDmgBp(crate::grid::Element::Physical, v),
            SubstatKey::BossDmg => StatMod::DmgVsBossBp(v),
            SubstatKey::HpRegen => StatMod::HpRegenFlat(v),
        }
    }
}

/// The lines `item` carries at ITS grade, from `rules`. Empty when the rules name no pool for
/// the slot (a slot the catalog has not authored yet carries nothing rather than something
/// invented), and for anything that is not gear.
pub fn lines_for(rules: &SubstatRules, item: &Item) -> Vec<StatMod> {
    if !matches!(item.kind, crate::items::ItemKind::Weapon | crate::items::ItemKind::Armor) {
        return Vec::new();
    }
    let slot = item.slot();
    lines_for_product(rules, &item.def_id, slot, item.grade)
}

/// The same, by product identity — what the drop-table bake and the generated table call, since
/// neither holds a graded instance.
pub fn lines_for_product(rules: &SubstatRules, def_id: &str, slot: EquipSlot, grade: Grade) -> Vec<StatMod> {
    let n = line_count(rules, slot, grade);
    if n == 0 {
        return Vec::new();
    }
    let Some(pool) = rules.slots.iter().find(|p| p.slot == slot) else { return Vec::new() };
    let tier = crate::gear_roll::def_tier(def_id).clamp(1, 7) as usize - 1;
    // FNV-1a over the def id + the grade: a stable identity for "this product", not a draw.
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in def_id.as_bytes() {
        h = (h ^ u64::from(*b)).wrapping_mul(0x1000_0000_01b3);
    }
    h = (h ^ (grade.step() as u64 + 1)).wrapping_mul(0x1000_0000_01b3);
    let mut rng = Pcg32::new(h);
    let mut left: Vec<SubstatKey> = pool
        .pool
        .iter()
        .copied()
        .filter(|k| rules.stats.iter().any(|s| s.key == *k))
        .collect();
    let mut out = Vec::with_capacity(n);
    for _ in 0..n {
        if left.is_empty() {
            break;
        }
        let key = left.remove(rng.gen_range_u32(0, left.len() as u32) as usize);
        let Some(def) = rules.stats.iter().find(|s| s.key == key) else { continue };
        let (lo, hi) = def.tiers.get(tier).copied().unwrap_or((0, 0));
        let base = if hi > lo { lo + rng.gen_range_u32(0, (hi - lo + 1) as u32) as i32 } else { lo };
        // The VALUE rides the percent ladder, not the raw one: these are mostly %-shaped lines
        // and the raw curve would run them off their own ceilings by mid-ladder (design §2.6).
        let mut v = ((base as i64 * grade.pct_bp() as i64 + 5_000) / 10_000) as i32;
        if def.cap > 0 {
            v = v.min(def.cap);
        }
        if v != 0 {
            out.push(key.make(v));
        }
    }
    out
}

/// How many lines a piece in `slot` carries at `grade`: the grade's count, plus the utility
/// extra for the slots that have no main stat (design §2.5: lines are all they have).
pub fn line_count(rules: &SubstatRules, slot: EquipSlot, grade: Grade) -> usize {
    let base = rules.lines_by_grade.get(grade.step()).copied().unwrap_or(0);
    let utility = matches!(slot, EquipSlot::Necklace | EquipSlot::Ring | EquipSlot::Earring | EquipSlot::Bracer);
    (base + if utility { rules.utility_extra } else { 0 }) as usize
}

/// Every grade's lines for one product, index = `Grade::step()` — what a drop-table row bakes so
/// the bank path (which holds no catalog) can stamp the rolled grade's lines.
pub fn lines_by_grade(rules: &SubstatRules, def_id: &str, slot: EquipSlot) -> Vec<Vec<StatMod>> {
    crate::items::GRADES.iter().map(|g| lines_for_product(rules, def_id, slot, *g)).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::items::{ItemKind, Rarity};

    fn rules() -> SubstatRules {
        crate::data::GameData::from_json_str(include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"), "/../../data/game_data.json"
        )))
        .expect("catalog")
        .loot_rules
        .substats
    }

    fn gear(def: &str, slot: EquipSlot, grade: Grade) -> Item {
        let mut it = Item::new(def, if slot == EquipSlot::Weapon { ItemKind::Weapon } else { ItemKind::Armor }, 10, 0, Rarity::Common);
        it.equip_slot = Some(slot);
        it.grade = grade;
        it
    }

    /// THE PROPERTY THE MARKET RESTS ON: two copies of one product carry identical lines.
    #[test]
    fn two_copies_of_one_product_carry_identical_lines() {
        let r = rules();
        let a = lines_for(&r, &gear("sword_s3", EquipSlot::Weapon, Grade::Beyond));
        let b = lines_for(&r, &gear("sword_s3", EquipSlot::Weapon, Grade::Beyond));
        assert_eq!(a, b);
        assert!(!a.is_empty());
        // And a different grade of the same def is a different product with different lines.
        let c = lines_for(&r, &gear("sword_s3", EquipSlot::Weapon, Grade::Arcana));
        assert_ne!(a, c);
    }

    /// The count follows the catalog's grade table, utility slots carry one more, and nothing
    /// has more lines than its slot's pool can supply without repeating a stat.
    #[test]
    fn line_count_follows_the_table_and_never_repeats_a_stat() {
        let r = rules();
        for (slot, def, extra) in [(EquipSlot::Weapon, "sword_s7", 0), (EquipSlot::Necklace, "necklace_s7", 1), (EquipSlot::Bracer, "bracer_s7", 1)] {
            for g in crate::items::GRADES {
                let lines = lines_for(&r, &gear(def, slot, g));
                let want = (r.lines_by_grade[g.step()] + extra) as usize;
                assert_eq!(lines.len(), want, "{def} at {g:?}");
                let mut kinds: Vec<String> = lines.iter().map(|m| format!("{m:?}").split('(').next().unwrap_or("").to_string()).collect();
                let n = kinds.len();
                kinds.sort();
                kinds.dedup();
                assert_eq!(kinds.len(), n, "{def} at {g:?}: a stat appeared twice");
            }
        }
        assert!(lines_for(&r, &gear("sword_s1", EquipSlot::Weapon, Grade::Common)).is_empty(), "Common carries none");
    }

    /// Values ride the PERCENT ladder and stop at the stat's cap.
    #[test]
    fn values_ride_the_percent_ladder_and_respect_the_cap() {
        let r = rules();
        let cap = r.stats.iter().find(|s| s.key == SubstatKey::AtkPct).expect("atk_pct authored").cap;
        for g in crate::items::GRADES {
            for m in lines_for(&r, &gear("sword_s7", EquipSlot::Weapon, g)) {
                if let StatMod::AtkBp(v) = m {
                    assert!(v <= cap, "{g:?}: {v} over the cap {cap}");
                }
            }
        }
        // Not gear → nothing.
        let card = Item::new("c", ItemKind::Card, 0, 0, Rarity::Common);
        assert!(lines_for(&r, &card).is_empty());
    }
}
