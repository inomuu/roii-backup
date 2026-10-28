//! GOLDEN ROSTER — shared harness (0.0.459, user 2026-09-22: "เพิ่ม golden เลย") — the REAL five-unit party, the
//! way `sim-cli --bench` stands it up, pinned as two state hashes. Every golden before this one
//! ran a class-era or single-entity scenario, so a week of unit kits (Iron Chains' DEF buff at
//! 1/100 strength, Lumi's Chill landing as a Dark mark, Frost Aura chilling friendly splash)
//! passed the whole suite. These two walk the kits, the Resonance ranks, the signature Regalia
//! passives, the ultimates and the marks against a stage-168 room for five sim minutes.
//!
//! Built through the same public primitives the bench uses (`grant_exemplar_unit` → `deploy`,
//! `debug_add_levels`, `debug_give_gear_at`, `EquipRegalia`, the overlay's stars) — no
//! sim-cli code, so the scenario is exactly reproducible from sim-core alone.
//!
//! The two pins live in `golden_roster_a.rs` / `golden_roster_b.rs` (one hash per file, so
//! `tools/bless_goldens.sh` re-pins each on its own). Say why in `lib.rs`'s changelog.

use sim_core::data::{self, GameData};
use sim_core::events::{apply_input, InputEvent};
use sim_core::grid::Team;
use sim_core::hash::state_hash;
use sim_core::items::{Exemplar, Grade, Item, ItemKind, Rarity};
use sim_core::state::GameState;

pub fn game_data() -> GameData {
    GameData::from_json_str(include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/../../data/game_data.json")))
        .expect("game_data.json must parse")
}

/// Stand a party up the bench's way: the starter plus `pulls`, Lv70, T6 Legendary, each unit's
/// signature SSR Regalia at Lv60 S0, every overlay at `stars`.
pub fn stand_up(seed: u64, d: &GameData, pulls: &[&str], stars: u8) -> GameState {
    let mut s = data::new_game_in_zone(seed, d, "zone_b168_1").expect("zone_b168_1 exists");
    s.set_gil_for_test(1_000_000);
    for id in pulls {
        while s.grid.entities.iter().filter(|e| matches!(e.team, Team::Player) && !e.is_pet).count() >= s.party_slots() {
            data::buy_party_slot(&mut s, d).expect("a party slot is affordable");
        }
        let char_id = data::grant_exemplar_unit(&mut s, d, id).unwrap_or_else(|| panic!("{id} is a pull"));
        data::deploy(&mut s, d, char_id).expect("deploys");
    }
    let members: Vec<(u32, u32, String)> = s
        .grid
        .entities
        .iter()
        .filter(|e| matches!(e.team, Team::Player) && !e.is_pet)
        .map(|e| (e.id, e.stats.level, e.unit_def.clone()))
        .collect();
    assert_eq!(members.len(), pulls.len() + 1, "the whole party is fielded");
    for (id, level, unit) in &members {
        if 70 > *level {
            assert!(data::debug_add_levels(&mut s, *id, 70 - level));
        }
        data::debug_give_gear_at(&mut s, d, *id, 6, Grade::Legendary);
        let line = sim_core::items::regalia_line_for_unit(unit).unwrap_or_else(|| panic!("{unit} has a Regalia line"));
        // The unit's highest EXISTING rung (an SR built one piece at a time has only the SR).
        let (rid, rarity) = if !line[2].is_empty() { (line[2], Rarity::Legendary) } else { (line[1], Rarity::Mythical) };
        let mut item = Item::new(&sim_core::items::regalia_item_def_id(rid), ItemKind::Regalia, 0, 0, rarity);
        item.enhance_level = 60;
        s.owned_regalia.push(item);
        let owned_idx = (s.owned_regalia.len() - 1) as u32;
        apply_input(&mut s, InputEvent::EquipRegalia { entity_id: *id, owned_idx });
    }
    for e in s.grid.entities.iter_mut() {
        if !matches!(e.team, Team::Player) || e.is_pet {
            continue;
        }
        if e.exemplar.is_none() {
            let def = sim_core::items::exemplar_def_by_id(&e.unit_def).expect("every unit has an Exemplar def");
            e.exemplar = Some(Exemplar { id: def.id.clone(), name: def.name.clone(), skin_id: def.skin_id.clone(), stars: 0 });
        }
        if let Some(x) = e.exemplar.as_mut() {
            x.stars = stars;
        }
        e.apply_exemplar_skill();
    }
    s
}

pub fn report(name: &str, s: &GameState) -> String {
    let h = state_hash(s);
    let alive = s.grid.entities.iter().filter(|e| matches!(e.team, Team::Player) && e.alive).count();
    let dealt: u64 = s.dps.by_class.values().sum();
    eprintln!("{name} state_hash = {h}");
    eprintln!("  members_alive={alive} kills={} damage_dealt={dealt} by_class={:?}", s.measures.kills.get(), s.dps.by_class);
    assert!(alive >= 3, "{name}: the party must still be standing (a wipe is a tuning regression, not a golden)");
    assert!(s.measures.kills.get() > 20, "{name}: the room was actually fought");
    for unit in s.dps.by_class.keys() {
        assert!(s.dps.by_class[unit] > 0, "{name}: {unit} dealt nothing");
    }
    h
}

