//! CATALOG HYGIENE — guards on the DATA layer that no golden or unit test covers, because each
//! one is about a property of the FILES rather than of the sim:
//!
//! * every item's permanent `num` is unique and the well-known ones never move (append-only);
//! * the shipped COPIES of the catalog are byte-identical to the source — the client and the
//!   server each carry their own copy, and a stale one fails the handshake for every player;
//! * every `gear_overrides` row is a real, sane exception — not a leftover equal to the formula,
//!   and not a mistyped zero.

use sim_core::data::GameData;
use sim_core::items::{Grade, Item, ItemKind, Rarity};

const SRC: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../data/game_data.json");

fn source() -> String {
    std::fs::read_to_string(SRC).expect("data/game_data.json")
}

/// `num` is the one field that must NEVER change once assigned: the Steam item-definition id is
/// derived from it (`num × 16 + grade`), and an itemdefid that moves re-labels every item ever
/// minted. Uniqueness is the cheap half; the pinned sample is the half that catches a re-numbering
/// pass that "tidied up" the sequence.
#[test]
fn item_nums_are_unique_and_pinned() {
    let data = GameData::from_json_str(&source()).expect("catalog parses");
    let mut nums: Vec<u32> = data.items.iter().map(|d| d.num).collect();
    assert!(nums.iter().all(|n| *n > 0), "every item needs a num — 0 is 'unassigned'");
    let n = nums.len();
    nums.sort_unstable();
    nums.dedup();
    assert_eq!(nums.len(), n, "two items share a num — a Steam itemdefid would collide");
    // PINNED. These numbers were assigned 2026-09-15 in catalog order and are now permanent.
    // If this fails you have RE-NUMBERED, which is the one thing the field forbids: give a new
    // item the next unused number instead, and never reuse a deleted one.
    for (id, num) in [("nothing", 1), ("sword_s1", 12), ("armor_s7", 90), ("card_pinkling", 126)] {
        let d = data.item(id).unwrap_or_else(|| panic!("{id} left the catalog — its num stays retired"));
        assert_eq!(d.num, num, "{id}: num moved — nums are append-only");
    }
}

/// The catalog ships as COPIES (Unity `StreamingAssets`, the Godot data dir) and the server bakes
/// the source at image build. A copy that lags the source means the client and the server hold
/// different content digests and the version gate refuses every save. This was a manual `cp`
/// after every edit; a test is the only thing that notices the one time it is forgotten.
#[test]
fn shipped_catalog_copies_match_the_source() {
    let src = source();
    for copy in [
        concat!(env!("CARGO_MANIFEST_DIR"), "/../../unity/ROI/Assets/StreamingAssets/game_data.json"),
        concat!(env!("CARGO_MANIFEST_DIR"), "/../../godot/data/game_data.json"),
    ] {
        let Ok(got) = std::fs::read_to_string(copy) else { continue }; // a checkout without that client
        assert!(got == src,
            "{copy} differs from data/game_data.json — run `cp data/game_data.json {copy}` \
             (the client and the server would carry different catalogs)");
    }
}

/// Every override must be a DELIBERATE exception. Two ways one stops being that:
/// it equals the formula (a leftover from before a re-tune, now saying nothing), or it strays so
/// far that it is more likely a typo than intent — unless the row says `deliberate: true`.
#[test]
fn gear_overrides_are_real_and_sane_exceptions() {
    let data = GameData::from_json_str(&source()).expect("catalog parses");
    const MAX_RATIO: i64 = 2; // an undeclared override may not exceed 2× (or fall below ½ of) the formula
    for o in &data.gear_overrides {
        let def = data.item(&o.def_id)
            .unwrap_or_else(|| panic!("override for {} — no such def", o.def_id));
        let mut it = Item::new(&def.id, def.kind, def.atk_bonus, def.def_bonus, Rarity::Common);
        it.grade = o.grade;
        let formula = [it.graded_atk(), it.graded_def()];
        let mut any = false;
        for (name, want, have) in [("atk", o.atk, formula[0]), ("def", o.def, formula[1])] {
            let Some(v) = want else { continue };
            any = true;
            assert_ne!(v, have, "{} @ {:?}: {name} override equals the formula — a leftover, delete it",
                o.def_id, o.grade);
            if !o.deliberate && have != 0 {
                let (lo, hi) = (i64::from(have) / MAX_RATIO, i64::from(have) * MAX_RATIO);
                assert!((lo..=hi).contains(&i64::from(v)),
                    "{} @ {:?}: {name} {v} is more than {MAX_RATIO}× away from the formula's {have} — \
                     mark `deliberate: true` and say why, or fix the number", o.def_id, o.grade);
            }
        }
        assert!(any, "{} @ {:?}: an override that overrides nothing", o.def_id, o.grade);
        assert!(!o.why.is_empty(), "{} @ {:?}: say WHY — an exception with no reason is a number nobody will dare touch",
            o.def_id, o.grade);
    }
    // And the mechanism itself, on a synthetic row: an override must reach the item.
    let mut tuned = GameData::from_json_str(&source()).expect("catalog");
    tuned.gear_overrides.push(sim_core::data::GearOverride {
        def_id: "sword_s7".into(), grade: Grade::Immortal, atk: Some(9_000), def: None,
        deliberate: true, why: "test".into(),
    });
    let mut it = sim_core::data::instantiate_item(tuned.item("sword_s7").unwrap());
    it.grade = Grade::Immortal;
    assert_ne!(it.graded_atk(), 9_000, "before migration the formula applies");
    sim_core::data::migrate_item(&mut it, &tuned);
    assert_eq!(it.graded_atk(), 9_000, "after migration the override is stamped and read");
    assert_eq!(it.kind, ItemKind::Weapon);
}

/// THE GEM TABLE IS COMPLETE AND SANE (0.0.386). Every set has exactly two stats, each a key the
/// substat vocabulary can fold; every (set × grade) has a ledger def whose id follows
/// `gems::def_id` and whose spec points back at the set; every gem def belongs to a set; and no
/// group is empty. A missing def is silent in play — the drop row is skipped, the fusion output
/// vanishes — so it is caught here.
#[test]
fn the_gem_table_covers_every_set_at_every_grade() {
    let d = GameData::from_json_str(&source()).expect("catalog");
    let r = &d.loot_rules.gems;
    assert!(!r.sets.is_empty(), "gem sets are authored");
    assert_eq!(r.merge, 6, "six-into-one, the gear rule");
    assert!(r.roll_window_bp > 0 && r.roll_window_bp <= 2_000, "a narrow roll: ±{}bp", r.roll_window_bp);
    let mut ids: Vec<&str> = r.sets.iter().map(|s| s.id.as_str()).collect();
    ids.sort();
    ids.dedup();
    assert_eq!(ids.len(), r.sets.len(), "set ids are unique");
    for set in &r.sets {
        assert_eq!(set.stats.len(), 2, "{}: a gem is a pair of stats", set.id);
        assert!(set.stats.iter().all(|st| st.base > 0), "{}: every base is positive", set.id);
        assert!(set.stats[0].key != set.stats[1].key || set.stats[0].element != set.stats[1].element, "{}: two different stats", set.id);
        assert!(set.weight > 0, "{}: a zero weight never drops and never fuses", set.id);
        for g in sim_core::items::GRADES {
            let id = sim_core::gems::def_id(&set.id, g);
            let def = d.item(&id).unwrap_or_else(|| panic!("{id} is not in the catalog"));
            let spec = def.gem.as_ref().unwrap_or_else(|| panic!("{id} carries no gem spec"));
            assert_eq!(spec.set, set.id, "{id}");
            assert_eq!(spec.grade, g, "{id}");
            assert!(!def.ledger && matches!(def.kind, sim_core::items::ItemKind::Material), "{id} is a bag stack (0.0.448), not a ledger count");
            // A gem is a plain product on the Steam market (user 2026-09-16: "gem trade ใน steam
            // market ได้ด้วย" / "gil trade ไม่มีแล้ว") — it is never modified per instance, so the
            // clean-item rule cannot demote it.
            assert!(def.steam_tradeable && !def.gil_tradeable, "{id} trades on Steam; there is no Gil market");
            assert!(matches!(def.kind, ItemKind::Material), "{id} is a material");
            let it = sim_core::data::instantiate_item(def);
            assert_eq!(it.gem_set, set.id);
            assert_eq!(it.grade, g, "{id}: the instance carries its grade");
            assert_eq!(it.product_key(), id, "{id}: the def id IS the product");
        }
    }
    for def in d.items.iter().filter(|i| i.id.starts_with("gem_")) {
        let spec = def.gem.as_ref().unwrap_or_else(|| panic!("{} is named like a gem but is not one", def.id));
        assert!(r.sets.iter().any(|s| s.id == spec.set), "{}: set {} is not authored", def.id, spec.set);
    }
    for group in [sim_core::gems::GemGroup::Weapon, sim_core::gems::GemGroup::Armor, sim_core::gems::GemGroup::Accessory] {
        assert!(r.sets.iter().any(|s| s.group == group), "{group:?} has no set");
    }
    // NO STAT TWICE (user 2026-09-16: "ดูให้ไม่มีอันไหนซ้ำเลย"): every set is its own pair.
    let mut keys: Vec<String> = r.sets.iter().flat_map(|s| s.stats.iter().map(|st| format!("{:?}{:?}", st.key, st.element))).collect();
    let n = keys.len();
    keys.sort();
    keys.dedup();
    assert_eq!(keys.len(), n, "a stat appears on two sets");
    for st in r.sets.iter().flat_map(|s| s.stats.iter()) {
        assert_eq!(st.element.is_some(), st.key == sim_core::substats::SubstatKey::ElementDmg, "{:?}: only element_dmg names an element", st.key);
    }
    assert!(!r.sets.iter().flat_map(|s| s.stats.iter()).any(|st| st.key == sim_core::substats::SubstatKey::CastSpeed), "cast speed is retired: skill tempo is Haste (CDR)");
    assert!(r.field_bp > 0 && r.boss_bp >= r.field_bp, "gems drop, and a boss pays at least the field");
}

/// THE SUBSTAT TABLE IS COMPLETE AND SANE. Every wearable slot has a pool, every pool names only
/// stats the table defines, every stat has seven tier windows that never run backwards, and the
/// count table covers all ten grades. A hole here is silent in play — a slot with no pool simply
/// carries nothing, and a typo'd key is dropped — so it is caught at the catalog instead.
#[test]
fn the_substat_table_covers_every_slot_and_every_key_is_real() {
    use sim_core::items::EquipSlot;
    let d = GameData::from_json_str(&source()).expect("catalog");
    let r = &d.loot_rules.substats;
    assert_eq!(r.lines_by_grade.len(), 10, "one line count per grade");
    assert!(r.lines_by_grade.windows(2).all(|w| w[0] <= w[1]), "lines never fall as the grade rises");
    for s in &r.stats {
        assert_eq!(s.tiers.len(), 7, "{:?}: seven tier windows", s.key);
        for (i, (lo, hi)) in s.tiers.iter().enumerate() {
            assert!(lo <= hi, "{:?} tier {}: window runs backwards", s.key, i + 1);
        }
        assert!(s.tiers.windows(2).all(|w| w[0].0 <= w[1].0), "{:?}: a higher tier must not pay less", s.key);
        assert!(s.cap >= 0);
    }
    let wearable = [
        EquipSlot::Weapon, EquipSlot::Shield, EquipSlot::Helmet, EquipSlot::Gloves,
        EquipSlot::Armor, EquipSlot::Earring, EquipSlot::Shoes, EquipSlot::Necklace, EquipSlot::Ring,
        EquipSlot::Bracer,
    ];
    for slot in wearable {
        let pool = r.slots.iter().find(|p| p.slot == slot).unwrap_or_else(|| panic!("{slot:?} has no substat pool"));
        for k in &pool.pool {
            assert!(r.stats.iter().any(|s| s.key == *k), "{slot:?} names {k:?}, which the table does not define");
        }
        // The pool must be able to fill the top grade's count without repeating a stat.
        let need = sim_core::substats::line_count(r, slot, Grade::Cosmic);
        assert!(pool.pool.len() >= need, "{slot:?}: {} stats in the pool, {need} lines at Cosmic", pool.pool.len());
    }
    // And every gear def in the catalog resolves to the count its grade promises.
    for def in d.items.iter().filter(|i| matches!(i.kind, ItemKind::Weapon | ItemKind::Armor) && !i.unique) {
        let it = sim_core::data::instantiate_item(def);
        for g in sim_core::items::GRADES {
            let lines = sim_core::substats::lines_for_product(r, &def.id, it.slot(), g);
            let want = sim_core::substats::line_count(r, it.slot(), g);
            assert_eq!(lines.len(), want, "{} at {g:?}", def.id);
        }
    }
}
