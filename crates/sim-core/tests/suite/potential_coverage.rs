//! Can every unit actually FARM its Potential material?
//!
//! A monster drops the Sigil of its FIRST weakness (`roll_potential_material`), graded by its
//! level. That makes two things about `game_data.json` load-bearing that nothing else asks
//! about: the ORDER of each weakness list, and whether every element in the player chart has a
//! field monster wearing it first in every level tier. A chart element with no such monster is a
//! unit that can never rank up past what the market sells it — and it is exactly the kind of gap
//! that shipped unnoticed for Earth / Thunder / Poison / Spirit before the chart was trimmed.
//!
//! Two rules, both user decisions of 2026-09-13:
//! - every chart element is the first weakness of ≥1 FIELD monster (not only a boss) in each
//!   Sigil grade tier (≤25 / 26-50 / 51+) — the 18 cells;
//! - all six appear on field monsters in the zones up to level 20 ("ควรจะครบ 6 ธาตุภายใน area 20"),
//!   because the element areas that take over as the main Sigil source unlock at level 20.

use sim_core::data;
use sim_core::grid::Element;
use sim_core::potential::{element_id, loot_tier, ELEMENTS};
use std::collections::{BTreeMap, BTreeSet};

fn game_data() -> data::GameData {
    data::GameData::from_json_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../data/game_data.json"
    )))
    .expect("game_data.json must parse")
}

/// Every monster that stands in a spawn pool (a zone's boss is not a field monster).
fn field_monsters(d: &data::GameData) -> BTreeSet<String> {
    let bosses: BTreeSet<&str> = d.zones.iter().map(|z| z.boss_monster_id.as_str()).collect();
    d.zones
        .iter()
        .flat_map(|z| z.spawn_pool.iter().map(|s| s.monster_id.clone()))
        .filter(|id| !bosses.contains(id.as_str()))
        .collect()
}

fn first_weakness(m: &data::MonsterDef) -> Option<Element> {
    let name = m.weaknesses.first()?;
    ELEMENTS.iter().copied().find(|e| format!("{e:?}") == *name)
}

#[test]
fn every_chart_element_has_a_field_monster_in_every_sigil_grade() {
    let d = game_data();
    let field = field_monsters(&d);
    let mut cells: BTreeMap<(String, usize), Vec<String>> = BTreeMap::new();
    for m in d.monsters.iter().filter(|m| field.contains(&m.id)) {
        if let Some(e) = first_weakness(m) {
            cells
                .entry((element_id(e).to_string(), loot_tier(m.stage).grade))
                .or_default()
                .push(m.id.clone());
        }
    }
    let mut missing = Vec::new();
    for e in ELEMENTS {
        for grade in 0..3 {
            if !cells.contains_key(&(element_id(e).to_string(), grade)) {
                missing.push(format!("{} grade {}", element_id(e), ["I", "II", "III"][grade]));
            }
        }
    }
    assert!(
        missing.is_empty(),
        "a chart element has no FIELD monster whose FIRST weakness it is, so a unit of that \
         element cannot farm its Sigil in that tier — reorder a weakness list (the first entry \
         is the Sigil): {missing:?}"
    );
}

#[test]
fn all_six_elements_are_farmable_on_field_monsters_by_level_20() {
    let d = game_data();
    let field = field_monsters(&d);
    // "By level 20" is about the ZONES a Lv1-20 party stands in, so the pool of every zone up to
    // level 20 — a Lv21 monster that spawns in a Lv16 zone is part of that stretch of the game.
    let in_early_zone: BTreeSet<&str> = d
        .zones
        .iter()
        .filter(|z| z.stage <= 48)
        .flat_map(|z| z.spawn_pool.iter().map(|s| s.monster_id.as_str()))
        .collect();
    let early: BTreeSet<String> = d
        .monsters
        .iter()
        .filter(|m| field.contains(&m.id) && in_early_zone.contains(m.id.as_str()))
        .filter_map(first_weakness)
        .map(|e| element_id(e).to_string())
        .collect();
    let missing: Vec<&str> = ELEMENTS
        .iter()
        .map(|e| element_id(*e))
        .filter(|id| !early.contains(*id))
        .collect();
    assert!(
        missing.is_empty(),
        "the first twenty levels must teach every chart element on a field monster (the element \
         areas unlock at 20): missing {missing:?}"
    );
}

/// Every non-unique piece of gear the catalog defines drops SOMEWHERE — the gear rows are a rule
/// (`loot_rules.gear`, 0.0.365), so a def whose tier badge nobody's level reaches, or a band no
/// monster stands in, is a piece that exists only in the shop. Read off the spawned tables.
#[test]
fn every_gear_def_is_reachable_from_some_spawned_table() {
    let d = game_data();
    // SPAWNED tables, as the name says: a table is built for a monster AT the stage it is
    // placed on (0.0.399), and the top tier only previews in the last quarter of the last band
    // (0.0.453), so a monster's own first stage would miss it.
    let mut droppable = BTreeSet::new();
    for z in &d.zones {
        for id in z.spawn_pool.iter().map(|s| s.monster_id.as_str()).chain(std::iter::once(z.boss_monster_id.as_str())) {
            let Some(m) = d.monster(id) else { continue };
            for e in data::build_drop_table(m, &d, z.stage) {
                droppable.insert(e.template.def_id);
            }
        }
    }
    let missing: Vec<&str> = d
        .items
        .iter()
        .filter(|i| matches!(i.kind, sim_core::items::ItemKind::Weapon | sim_core::items::ItemKind::Armor) && !i.unique)
        .filter(|i| !droppable.contains(&i.id))
        .map(|i| i.id.as_str())
        .collect();
    // The ONE legitimate way out of the pool is to carry no tier badge, and the only gear allowed
    // to do that is a class's starting weapon (0.0.379) — it is handed out at creation, not dealt.
    // Anything else here is a def that exists only in the shop, or a test leftover.
    let starting: BTreeSet<&str> = d.units.iter().map(|u| u.starting_weapon.as_str()).collect();
    let stray: Vec<&&str> = missing.iter().filter(|id| !starting.contains(*id)).collect();
    assert!(stray.is_empty(), "gear no monster can drop: {stray:?}");
    for id in &missing {
        assert_eq!(sim_core::gear_roll::def_tier_badge(id), None, "{id}: a badged def must be dealt somewhere");
    }
}

/// EVERY SLOT PAYS THE SAME (0.0.378). The rule's odds are per slot, so a party member's eleven
/// slots fill at one rate — not at the rate of how many defs each slot happens to have. Read off
/// a real field table and a real boss table: the rule rows grouped by paper-doll slot must carry
/// equal weight (to rounding), and every slot of the tier must be present.
#[test]
fn the_gear_rule_pays_every_slot_equally() {
    let d = game_data();
    let rule = d.loot_rules.gear.as_ref().expect("the gear rule exists");
    for (monster, boss) in [("pinkling", false), ("teenager_pinkling", true)] {
        let m = d.monsters.iter().find(|m| m.id == monster).unwrap_or_else(|| panic!("{monster} exists"));
        // Placed on stage 1: the slot odds row is the first (they grow with the stage, 0.0.402).
        let table = data::build_drop_table(m, &d, 1);
        let mut by_slot: BTreeMap<String, u64> = BTreeMap::new();
        for e in table.iter().filter(|e| e.server_bp == 0 && e.weight > 0
            && matches!(e.template.kind, sim_core::items::ItemKind::Weapon | sim_core::items::ItemKind::Armor))
        {
            *by_slot.entry(format!("{:?}", e.template.slot())).or_default() += u64::from(e.weight);
        }
        assert_eq!(by_slot.len(), 10, "{monster}: every wearable slot of the tier drops (Bracer since 0.0.383, Pants gone 0.0.389): {by_slot:?}");
        let (lo, hi) = (by_slot.values().min().copied().unwrap_or(0), by_slot.values().max().copied().unwrap_or(0));
        assert!(hi * 100 <= lo * 102, "{monster}: slot shares must be equal to rounding, got {by_slot:?}");
        // And the slot's share is the authored number: `slot_bp` of the table, out of `nothing`.
        let total: u64 = table.iter().map(|e| u64::from(e.weight)).sum();
        let (field_bp, boss_bp) = rule.slot_bp_at(1);
        let slot_bp = u64::from(if boss { boss_bp } else { field_bp });
        let got_bp = lo * 10_000 / total;
        assert!(got_bp.abs_diff(slot_bp) <= 1, "{monster}: a slot pays {got_bp} bp of a kill, authored {slot_bp}");
    }
}

/// The gear rule's share must come OUT OF `nothing`, never on top of the table. `build_drop_table`
/// subtracts the rule's weight from the `nothing` row with a `saturating_sub`, so a table whose
/// `nothing` row is too thin (or missing) silently DILUTES every authored row instead — the drop
/// odds a monster was tuned with quietly change, with nothing on screen to show it. Today every
/// table has room; this pins that, because the failure is invisible without it.
#[test]
fn the_gear_rule_never_eats_into_the_authored_rows() {
    let d = game_data();
    let thin: Vec<String> = d
        .monsters
        .iter()
        .filter_map(|m| {
            let table = data::build_drop_table(m, &d, 0);
            // Rule rows carry `weight > 0` and `server_bp == 0`; a table that got none (an
            // excluded monster, or one with no authored rows at all) has nothing to check.
            let got_rule = table.iter().any(|e| e.server_bp == 0 && e.weight > 0
                && matches!(e.template.kind, sim_core::items::ItemKind::Weapon | sim_core::items::ItemKind::Armor));
            if !got_rule {
                return None;
            }
            let nothing = table
                .iter()
                .find(|e| e.template.def_id == sim_core::items::NOTHING_DROP_ID)
                .map(|e| e.weight);
            match nothing {
                Some(w) if w > 0 => None,
                Some(_) => Some(format!("{} (nothing exhausted)", m.id)),
                None => Some(format!("{} (no `nothing` row)", m.id)),
            }
        })
        .collect();
    assert!(
        thin.is_empty(),
        "the gear rule's weight could not come out of `nothing` — these tables had their authored \
         odds diluted: {thin:?}"
    );
}
