//! CRESTS (0.0.416) — the disenchant sink. `docs/crest-system-design.md`; the model is WoG's
//! Blacksmith dismantle → crest materials (user 2026-09-17: "แยกของที่ disenchant อาวุธ / ชุด /
//! เครื่องประดับ / เพชร → unique material ตาม tier ไม่เก็บในช่อง + แลกเปลี่ยนไม่ได้ → อัพ stat ตามชนิด").
//!
//! Disenchanting a piece destroys it and credits `GameState.crest_mats[<category>_t<tier>]` —
//! a counter of SHARDS (`shards_per_material` make one, ten by default — a Common piece is a
//! tenth of a material, no roll), never a bag item, never tradeable. Twenty materials (four
//! categories × T3..T7), twenty crests, twenty stats: each crest eats exactly one material,
//! `mats_per_level × L` of it for level L. Fusion and disenchant both eat spare gear; which gets fed is the player's call
//! (user: "แย่งกันไม่เป็นไร").
//!
//! The levels reach combat exactly as Training's do: resolved to `GameState.crest_mods` where a
//! catalog is in hand, copied onto every player each tick (`apply`), folded by `for_each_mod`.

use crate::data::{GameData, CrestRules, CrestSlot};
use crate::items::{EquipSlot, Item, ItemKind, StatMod};
use crate::state::GameState;
use serde::{Deserialize, Serialize};

/// What a piece disenchants INTO — and which crests its materials lift.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CrestKind {
    /// Weapon + shield (the gem `Weapon` group).
    Weapon,
    /// Helmet / armour / gloves / shoes.
    Armor,
    /// Necklace / ring / earring / bracer.
    Accessory,
    /// A socketable gem (`gem_<set>_g<n>`); its grade step + 1 is its tier.
    Gem,
}

impl CrestKind {
    pub fn key(self) -> &'static str {
        match self {
            CrestKind::Weapon => "weapon",
            CrestKind::Armor => "armor",
            CrestKind::Accessory => "accessory",
            CrestKind::Gem => "gem",
        }
    }
}

/// The material key a disenchant of `it` credits — `(kind, tier)` — or `None` when the item is
/// not gear or a gem, is unique, or sits below `min_tier`. A tier above `max_tier` yields the
/// top material: five materials per category, T3..T7, twenty in all.
pub fn material_of(data: &GameData, it: &Item) -> Option<(CrestKind, u8)> {
    if it.unique {
        return None;
    }
    let rules = &data.crests;
    let clamp = |t: u8| -> Option<u8> { (t >= rules.min_tier && rules.min_tier > 0).then(|| t.min(rules.max_tier.max(rules.min_tier))) };
    match it.kind {
        ItemKind::Weapon | ItemKind::Armor => {
            let kind = match it.slot() {
                EquipSlot::Weapon | EquipSlot::Shield => CrestKind::Weapon,
                EquipSlot::Helmet | EquipSlot::Armor | EquipSlot::Gloves | EquipSlot::Shoes | EquipSlot::Pants => CrestKind::Armor,
                EquipSlot::Necklace | EquipSlot::Ring | EquipSlot::Earring | EquipSlot::Bracer => CrestKind::Accessory,
                _ => return None,
            };
            Some((kind, clamp(it.item_level().clamp(1, u32::from(u8::MAX)) as u8)?))
        }
        ItemKind::Material => {
            // A gem's grade step + 1 is its tier: Rare (step 2) is the first that yields.
            let (_, grade) = crate::gems::set_of(data, &it.def_id)?;
            Some((CrestKind::Gem, clamp((grade.step() + 1).min(usize::from(u8::MAX)) as u8)?))
        }
        _ => None,
    }
}

pub fn mat_key(kind: CrestKind, tier: u8) -> String {
    format!("{}_t{}", kind.key(), tier)
}

/// SHARDS `it` would yield: by grade for gear, flat for a gem, times the stack for a stack.
pub fn yield_of(rules: &CrestRules, it: &Item, kind: CrestKind) -> u32 {
    let per = match kind {
        CrestKind::Gem => rules.gem_yield,
        _ => rules.yield_by_grade.get(it.grade.step()).copied().unwrap_or(0),
    };
    per.saturating_mul(it.quantity.max(1))
}

/// DISENCHANT the bag item at `idx`: destroy it, credit its material. Refuses (no-op) anything
/// `material_of` rejects and a zero yield. Bound is accepted — this is a use, not a sale.
pub fn disenchant(state: &mut GameState, data: &GameData, idx: usize) -> bool {
    let Some(it) = state.inventory.get(idx) else { return false };
    let Some((kind, tier)) = material_of(data, it) else { return false };
    let n = yield_of(&data.crests, it, kind);
    if n == 0 {
        return false;
    }
    state.inventory.remove(idx);
    let e = state.crest_mats.entry(mat_key(kind, tier)).or_insert(0);
    *e = e.saturating_add(n);
    state.measures.items_disenchanted.add(1);
    true
}

pub fn slot<'a>(data: &'a GameData, id: &str) -> Option<&'a CrestSlot> {
    data.crests.slots.iter().find(|s| s.id == id)
}

pub fn level(state: &GameState, id: &str) -> u8 {
    state.crests.get(id).copied().unwrap_or(0)
}

/// What level `next` of `s` costs: `(material key, SHARDS)` — its own category × tier material,
/// `mats_per_level × next` materials, in shards — `None` past the cap.
pub fn cost(rules: &CrestRules, s: &CrestSlot, next: u8) -> Option<(String, u32)> {
    if next == 0 || next > s.max_level {
        return None;
    }
    let shards = s.mats_per_level.saturating_mul(u32::from(next)).saturating_mul(rules.shards_per_material.max(1));
    Some((mat_key(s.category, s.tier), shards))
}

pub fn next_cost(state: &GameState, data: &GameData, id: &str) -> Option<(String, u32)> {
    let s = slot(data, id)?;
    cost(&data.crests, s, level(state, id).checked_add(1)?)
}

/// Lift crest `id` one level, spending its tier's materials. Silent no-op when short or capped.
pub fn upgrade(state: &mut GameState, data: &GameData, id: &str) -> bool {
    let Some((key, n)) = next_cost(state, data, id) else { return false };
    let have = state.crest_mats.get(&key).copied().unwrap_or(0);
    if have < n {
        return false;
    }
    state.crest_mats.insert(key, have - n);
    let lv = level(state, id) + 1;
    state.crests.insert(id.to_string(), lv);
    state.measures.crest_levels.add(1);
    resolve(state, data);
    true
}

pub fn slot_mod(s: &CrestSlot, level: u8) -> Option<StatMod> {
    if level == 0 {
        return None;
    }
    s.stat.stat_mod(s.element, s.per_level.saturating_mul(i32::from(level)))
}

pub fn mods(state: &GameState, data: &GameData) -> Vec<StatMod> {
    if state.crests.is_empty() {
        return Vec::new();
    }
    data.crests.slots.iter().filter_map(|s| slot_mod(s, level(state, &s.id))).collect()
}

/// Resolve the levels against the catalog (every `upgrade`, and the load path).
/// Party-wide ECONOMY bonus (basis points) of one kind, summed over the resolved crests. 0 for a
/// party without that crest — every caller's arithmetic is then the identity.
pub fn economy_bp(state: &GameState, pick: fn(&StatMod) -> Option<i32>) -> i64 {
    state.crest_mods.iter().filter_map(pick).map(i64::from).sum()
}
pub fn xp_gain_bp(state: &GameState) -> i64 { economy_bp(state, |m| if let StatMod::XpGainBp(v) = *m { Some(v) } else { None }) }
pub fn gil_gain_bp(state: &GameState) -> i64 { economy_bp(state, |m| if let StatMod::GilGainBp(v) = *m { Some(v) } else { None }) }
pub fn soul_gain_bp(state: &GameState) -> i64 { economy_bp(state, |m| if let StatMod::SoulGainBp(v) = *m { Some(v) } else { None }) }
pub fn drop_rate_bp(state: &GameState) -> i64 { economy_bp(state, |m| if let StatMod::DropRateBp(v) = *m { Some(v) } else { None }) }

pub fn resolve(state: &mut GameState, data: &GameData) {
    state.crest_mods = mods(state, data);
}

/// Copy the resolved lines onto every player each tick, as `training::apply` does.
pub fn apply(state: &mut GameState) {
    if state.crest_mods.is_empty() && state.grid.entities.iter().all(|e| e.crest_mods.is_empty()) {
        return;
    }
    let mods = state.crest_mods.clone();
    for e in state.grid.entities.iter_mut() {
        if !matches!(e.team, crate::grid::Team::Player) || e.is_pet || e.owner_id != 0 {
            continue;
        }
        if e.crest_mods != mods {
            e.crest_mods = mods.clone();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::items::{Grade, Rarity};
    use crate::training::TrainStat;

    fn data() -> GameData {
        let mut d = GameData::from_json_str(include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/../../data/game_data.json"))).unwrap();
        d.crests = CrestRules {
            min_tier: 3,
            max_tier: 7,
            yield_by_grade: vec![1, 2, 3],
            gem_yield: 1,
            shards_per_material: 1,
            slots: vec![
                CrestSlot { id: "edge".into(), category: CrestKind::Weapon, tier: 3, stat: TrainStat::AtkPct, element: None, per_level: 50, max_level: 4, mats_per_level: 1 },
                CrestSlot { id: "facet".into(), category: CrestKind::Gem, tier: 5, stat: TrainStat::BreakDmg, element: None, per_level: 100, max_level: 4, mats_per_level: 1 },
            ],
        };
        d
    }

    fn state(d: &GameData) -> GameState {
        crate::data::new_game_in_zone(7, d, "zone_b1_1").unwrap()
    }

    #[test]
    fn a_piece_becomes_its_category_and_tier_of_material_by_grade() {
        let d = data();
        let mut s = state(&d);
        let mut sword = Item::new("sword_s3", ItemKind::Weapon, 10, 0, Rarity::Common);
        sword.grade = Grade::Rare;
        s.inventory.push(sword);
        let boots = Item::new("shoes_s1", ItemKind::Armor, 0, 3, Rarity::Common);
        s.inventory.push(boots);
        let mut crown = Item::new("helmet_s9", ItemKind::Armor, 0, 3, Rarity::Common);
        crown.grade = Grade::Uncommon;
        s.inventory.push(crown);
        let uniq = { let mut u = Item::new("sword_s3", ItemKind::Weapon, 10, 0, Rarity::Common); u.unique = true; u };
        s.inventory.push(uniq);
        assert_eq!(material_of(&d, &s.inventory[0]), Some((CrestKind::Weapon, 3)));
        assert_eq!(material_of(&d, &s.inventory[1]), None, "tier 1 is fusion fodder, not material");
        assert_eq!(material_of(&d, &s.inventory[2]), Some((CrestKind::Armor, 7)), "past the top tier → the top material");
        assert_eq!(material_of(&d, &s.inventory[3]), None, "a unique never disenchants");
        assert!(disenchant(&mut s, &d, 0));
        assert_eq!(s.crest_mats.get("weapon_t3"), Some(&3), "Rare = grade step 2 → 3 materials");
        assert!(!disenchant(&mut s, &d, 0), "the tier-1 boots stay");
        assert!(disenchant(&mut s, &d, 1));
        assert_eq!(s.crest_mats.get("armor_t7"), Some(&2));
        assert!(!disenchant(&mut s, &d, 1), "the unique stays");
        assert_eq!(s.inventory.len(), 2);
        assert_eq!(s.measures.items_disenchanted.get(), 2);
    }

    #[test]
    fn a_gem_yields_its_grade_as_a_tier() {
        let d = data();
        let mut s = state(&d);
        s.inventory.push(Item::new("gem_strife_g4", ItemKind::Material, 0, 0, Rarity::Common));
        s.inventory.push(Item::new("gem_strife_g1", ItemKind::Material, 0, 0, Rarity::Common));
        assert_eq!(material_of(&d, &s.inventory[0]), Some((CrestKind::Gem, 5)));
        assert_eq!(material_of(&d, &s.inventory[1]), None, "a grade-1 gem is below the floor");
        assert!(disenchant(&mut s, &d, 0));
        assert_eq!(s.crest_mats.get("gem_t5"), Some(&1));
    }

    #[test]
    fn ten_shards_make_a_material_and_prices_are_in_shards() {
        let mut d = data();
        d.crests.shards_per_material = 10;
        assert_eq!(cost(&d.crests, &d.crests.slots[0], 1), Some(("weapon_t3".into(), 10)), "level 1 = one material = ten shards");
        assert_eq!(cost(&d.crests, &d.crests.slots[0], 3), Some(("weapon_t3".into(), 30)));
        let mut s = state(&d);
        for _ in 0..9 {
            let mut sword = Item::new("sword_s3", ItemKind::Weapon, 10, 0, Rarity::Common);
            sword.grade = Grade::Common;
            s.inventory.push(sword);
        }
        while disenchant(&mut s, &d, 0) {}
        assert_eq!(s.crest_mats.get("weapon_t3"), Some(&9), "nine Commons = nine shards");
        assert!(!upgrade(&mut s, &d, "edge"), "one shard short of a material");
        s.inventory.push(Item::new("sword_s3", ItemKind::Weapon, 10, 0, Rarity::Common));
        assert!(disenchant(&mut s, &d, 0));
        assert!(upgrade(&mut s, &d, "edge"), "the tenth Common completes the material");
        assert_eq!(s.crest_mats["weapon_t3"], 0);
    }

    #[test]
    fn a_crest_climbs_on_its_own_tier_of_material_and_reaches_the_party() {
        let d = data();
        let mut s = state(&d);
        // Every level of `edge` wants weapon_t3, `L` of it; level 5 is past the cap.
        assert_eq!(cost(&d.crests, &d.crests.slots[0], 1), Some(("weapon_t3".into(), 1)));
        assert_eq!(cost(&d.crests, &d.crests.slots[0], 3), Some(("weapon_t3".into(), 3)));
        assert_eq!(cost(&d.crests, &d.crests.slots[0], 5), None);
        assert!(!upgrade(&mut s, &d, "edge"), "no materials");
        s.crest_mats.insert("weapon_t3".into(), 6);
        assert!(upgrade(&mut s, &d, "edge"));
        assert!(upgrade(&mut s, &d, "edge"));
        assert!(upgrade(&mut s, &d, "edge"));
        assert_eq!(s.crest_mats["weapon_t3"], 0, "1 + 2 + 3");
        assert!(!upgrade(&mut s, &d, "edge"), "level 4 wants four more");
        assert_eq!(level(&s, "edge"), 3);
        assert_eq!(s.crest_mods, vec![StatMod::AtkBp(150)]);
        apply(&mut s);
        let lead = s.grid.entities.iter().find(|e| matches!(e.team, crate::grid::Team::Player)).unwrap();
        assert_eq!(crate::combat::stat_mods::atk_bp(lead), 150);
        assert_eq!(s.measures.crest_levels.get(), 3);
    }

    /// A Crest material envelope arriving by mail credits its counter — one material
    /// (`shards_per_material` shards) per unit — and never lands in the bag; every envelope in the
    /// shipped catalog names a key some Crest actually spends.
    #[test]
    fn a_crest_material_envelope_credits_the_counter_not_the_bag() {
        let d = crate::data::GameData::from_json_str(include_str!("../../../data/game_data.json")).expect("catalog");
        let spent: std::collections::BTreeSet<String> = d.crests.slots.iter()
            .filter_map(|c| cost(&d.crests, c, 1).map(|(k, _)| k))
            .collect();
        let envelopes: Vec<_> = d.items.iter().filter(|i| i.crest_material.is_some()).collect();
        assert_eq!(envelopes.len(), 4);
        for e in &envelopes {
            let key = e.crest_material.as_ref().unwrap();
            assert!(spent.contains(key), "{} credits {key}, which no Crest spends", e.id);
        }
        let mut s = crate::data::new_game_in_zone(1, &d, "zone_b1_1").unwrap();
        let bag = s.inventory.len();
        let mut it = crate::data::instantiate_item(d.item("crest_mat_weapon_t3").unwrap());
        it.quantity = 2;
        assert!(crate::data::try_grant_item(&mut s, &d, it));
        assert_eq!(s.crest_mats.get("weapon_t3"), Some(&(2 * d.crests.shards_per_material)));
        assert_eq!(s.inventory.len(), bag, "never in the bag");
    }
}
