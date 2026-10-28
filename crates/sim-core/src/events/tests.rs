use super::*;
use crate::grid::{Cell, Entity};
use crate::items::{Item, ItemKind, Rarity};

fn fixture() -> GameState {
    let mut s = GameState::new(1, 8, 8);
    let e = Entity::idle(1, Cell::new(0, 0));
    s.grid.spawn(e);
    s.inventory.push(Item::new("sword_short", ItemKind::Weapon, 10, 0, Rarity::Common));
    s
}

#[test]
fn input_authority_classifies_privileged_events() {
    // Reward-amplifier = server-authored; free grant = debug; the rest client.
    assert_eq!(
        InputEvent::SetCardRateBonus { bp: 1 }.authority(),
        InputAuthority::Server
    );
    assert_eq!(
        InputEvent::GrantLoot { def_id: "x".into(), foil: false }.authority(),
        InputAuthority::Debug
    );
    // Server-roll drop grant: server-authored, never client-authorable.
    assert_eq!(
        InputEvent::GrantDrop { candidate_seq: 1, server_roll: 42 }.authority(),
        InputAuthority::Server
    );
    for ev in [
        InputEvent::EquipItem { entity_id: 1, inventory_idx: 0 },
        InputEvent::BuyItem { def_id: "x".into(), qty: 1 },
        InputEvent::SelectBand { band: 2 },
        InputEvent::RespawnInZone,
    ] {
        assert_eq!(ev.authority(), InputAuthority::Client, "{ev:?} is client-legal");
    }
}

#[test]
fn discard_item_removes_bound_stack_for_zero_gil() {
    let mut s = fixture(); // inventory[0] = a sword
    let mut potion = Item::new("hp_pot_1", ItemKind::Consumable, 0, 0, Rarity::Common);
    potion.quantity = 50;
    potion.sell_value = 3; // even a sellable item yields NO Gil via discard
    potion.bound = true; // a bound stack refuses every sell path — discard is the only exit
    s.inventory.push(potion);
    let gil_before = s.gil();
    assert_eq!(s.inventory.len(), 2);

    apply_input(&mut s, InputEvent::DiscardItem { inv_idx: 1 });

    assert_eq!(s.inventory.len(), 1, "the whole bound stack is gone");
    assert_eq!(s.inventory[0].def_id, "sword_short", "the untouched item stays");
    assert_eq!(s.gil(), gil_before, "discard mints ZERO Gil — no laundering");

    // Out-of-range index is a silent no-op (never panics — replay must survive).
    apply_input(&mut s, InputEvent::DiscardItem { inv_idx: 99 });
    assert_eq!(s.inventory.len(), 1);
}

#[test]
fn expand_inventory_adds_a_permanent_slot_and_consumes_one() {
    use crate::state::MAX_INVENTORY_CAP_BONUS;
    let mut s = fixture(); // inventory[0] = a sword
    let base_cap = s.inventory_cap();
    let mut exp = Item::new("inventory_expander", ItemKind::Material, 0, 0, Rarity::Common);
    exp.quantity = 2;
    exp.bound = true;
    s.inventory.push(exp); // index 1

    apply_input(&mut s, InputEvent::ExpandInventory { inv_idx: 1 });
    assert_eq!(s.inventory_cap(), base_cap + 1, "one permanent bag slot added");
    assert_eq!(s.inventory[1].quantity, 1, "one expander consumed from the stack");

    apply_input(&mut s, InputEvent::ExpandInventory { inv_idx: 1 });
    assert_eq!(s.inventory_cap(), base_cap + 2, "second use adds another slot");
    assert_eq!(s.inventory.len(), 1, "the stack emptied → removed");

    // Wrong item is a no-op (the sword at index 0 isn't an expander).
    apply_input(&mut s, InputEvent::ExpandInventory { inv_idx: 0 });
    assert_eq!(s.inventory_cap(), base_cap + 2, "non-expander does nothing");
    assert_eq!(s.inventory.len(), 1, "sword untouched");

    // Cap ceiling: nothing consumed once maxed.
    s.inventory_cap_bonus = MAX_INVENTORY_CAP_BONUS;
    let mut exp2 = Item::new("inventory_expander", ItemKind::Material, 0, 0, Rarity::Common);
    exp2.quantity = 5;
    s.inventory.push(exp2);
    apply_input(&mut s, InputEvent::ExpandInventory { inv_idx: 1 });
    assert_eq!(s.inventory_cap_bonus, MAX_INVENTORY_CAP_BONUS, "cap not exceeded");
    assert_eq!(s.inventory[1].quantity, 5, "maxed → expander NOT wasted");
}

#[test]
fn set_card_rate_bonus_is_capped() {
    let mut s = fixture();
    // A forged u32-max multiplier is clamped to the sim's defense-in-depth cap.
    apply_input(&mut s, InputEvent::SetCardRateBonus { bp: u32::MAX });
    assert_eq!(s.card_rate_bonus_bp, crate::drops::MAX_CARD_RATE_BONUS_BP);
    // A value under the cap passes through unchanged.
    apply_input(&mut s, InputEvent::SetCardRateBonus { bp: 20_000 });
    assert_eq!(s.card_rate_bonus_bp, 20_000);
}

#[test]
fn bind_card_escalates_cost_and_caps_at_three() {
    use crate::combat::effective_atk;
    use crate::items::StatMod;
    let mut s = fixture();
    // A Redling-like card: bind grants +1 ATK. Stock plenty (1+2+3 = 6 to max).
    let mut card = Item::new("redling", ItemKind::Card, 0, 0, Rarity::Rare);
    card.quantity = 10;
    card.bind_mods = vec![StatMod::AtkFlat(1)];
    s.card_album.push(card);
    let cidx = (s.card_album.len() - 1) as u32;
    let atk0 = effective_atk(&s.grid.entities[0]);

    // 1st bind costs 1 card → +1 ATK. 2nd costs 2. 3rd costs 3. Total 6 cards.
    for n in 1..=3u32 {
        apply_input(&mut s, InputEvent::BindCard { entity_id: 1, album_idx: cidx });
        assert_eq!(effective_atk(&s.grid.entities[0]), atk0 + n as i32,
            "consume #{n} grants +1 ATK permanently");
    }
    assert_eq!(s.card_album[cidx as usize].quantity, 10 - 6, "spent 1+2+3 = 6 cards");

    // 4th consume is capped (already 3) → no change, no spend.
    apply_input(&mut s, InputEvent::BindCard { entity_id: 1, album_idx: cidx });
    assert_eq!(effective_atk(&s.grid.entities[0]), atk0 + 3, "capped at 3 consumes");
    assert_eq!(s.card_album[cidx as usize].quantity, 4, "no further cards spent at cap");
}

/// 0.0.245: the bound-card grant is DERIVED. `bind_card` accumulates `bind_mods` per bind,
/// and `migrate_save_items` rebuilds the whole list from `bound_card_counts` × the def's
/// CURRENT `bind_mods` — so a re-tuned bind reaches existing saves, a pre-0.0.245 save whose
/// `bound_card_bonus` blob no longer deserializes is restored from its counts, and the rebuild
/// is idempotent (the verify server runs it before replay).
#[test]
fn bound_card_mods_rebuild_from_counts_on_load() {
    use crate::items::StatMod;
    let data = crate::data::GameData::from_json_str(include_str!(
        "../../../../data/game_data.json"
    ))
    .expect("catalog");
    let mut s = fixture();
    // Pinkling binds +1 ATK. Bind it twice (cost 1 then 2).
    let mut card = Item::new("card_pinkling", ItemKind::Card, 0, 0, Rarity::Rare);
    card.quantity = 3;
    card.bind_mods = vec![StatMod::AtkFlat(1)];
    s.card_album.push(card);
    let cidx = (s.card_album.len() - 1) as u32;
    apply_input(&mut s, InputEvent::BindCard { entity_id: 1, album_idx: cidx });
    apply_input(&mut s, InputEvent::BindCard { entity_id: 1, album_idx: cidx });
    let e = &s.grid.entities[0];
    assert_eq!(e.bound_card_counts.get("card_pinkling"), Some(&2));
    assert_eq!(e.bound_card_mods, vec![StatMod::AtkFlat(1), StatMod::AtkFlat(1)]);
    assert_eq!(crate::combat::stat_mods::atk_flat(e), 2, "reaches the ONE pipeline");

    // A save that lost the list entirely (old blob dropped) heals from the counts alone.
    s.grid.entities[0].bound_card_mods.clear();
    crate::data::migrate_save_items(&mut s, &data);
    let healed = s.grid.entities[0].bound_card_mods.clone();
    assert_eq!(healed.len(), 2, "one copy per bind, rebuilt from the count");
    assert!(healed.iter().all(|m| matches!(m, StatMod::AtkFlat(1))));
    // Idempotent — running it again must not stack a second time.
    crate::data::migrate_save_items(&mut s, &data);
    assert_eq!(s.grid.entities[0].bound_card_mods, healed);
}

#[test]
fn bind_card_needs_enough_cards_for_the_tier() {
    use crate::items::StatMod;
    let mut s = fixture();
    let mut card = Item::new("redling", ItemKind::Card, 0, 0, Rarity::Rare);
    card.quantity = 1; // only 1 card
    card.bind_mods = vec![StatMod::AtkFlat(1)];
    s.card_album.push(card);
    let cidx = (s.card_album.len() - 1) as u32;
    // 1st consume (cost 1) works.
    apply_input(&mut s, InputEvent::BindCard { entity_id: 1, album_idx: cidx });
    assert!(s.card_album.iter().all(|i| i.def_id != "redling"), "the 1 card was bound");
    // No cards left → can't do the 2nd (cost 2). bound_card_counts has redling=1.
    assert_eq!(s.grid.entities[0].bound_card_counts.get("redling"), Some(&1));
}

/// The retired `ResetBandClears` still DESERIALIZES, and still does nothing.
///
/// Both halves matter and neither is about the event itself. A save is
/// `(seed + state + Vec<InputEvent>)`, so this variant is part of the replay wire format: delete
/// it and a log that still holds one fails to parse, losing the entire save rather than skipping
/// one dead instruction. The parse half of this test is what stops a future cleanup from doing
/// that. The no-op half stops it from being quietly re-pointed at `bands_cleared`, which is the
/// obvious thing to do to a reset event whose old target is gone — and which would let a claim
/// wipe the player's lifetime history.
#[test]
fn the_retired_reset_event_still_parses_and_changes_nothing() {
    let mut s = fixture();
    s.record_band_clear(5);
    s.record_band_clear(5);
    s.record_band_clear(4);
    assert_eq!(s.measures.bands_cleared.get(), 3);
    // The bare-string form the bridge sends must still deserialize to the unit variant.
    let ev: InputEvent = serde_json::from_str("\"ResetBandClears\"").unwrap();
    apply_input(&mut s, ev);
    assert_eq!(s.measures.bands_cleared.get(), 3, "a retired event moved the lifetime tally");
}

#[test]
fn equip_moves_item_from_stash_to_equipped() {
    let mut s = fixture();
    apply_input(&mut s, InputEvent::EquipItem { entity_id: 1, inventory_idx: 0 });
    assert_eq!(s.inventory.len(), 0);
    assert_eq!(s.grid.entities[0].equipped.len(), 1);
    assert_eq!(s.grid.entities[0].equipped[0].def_id, "sword_short");
}

/// 0.0.136: gear LEVEL REQUIREMENT — (item_level−1)×10 from the `_sN` badge.
/// An under-leveled equip is a silent no-op (stash untouched); reaching the
/// level makes the same input work.
#[test]
fn equip_gates_on_item_level_requirement() {
    let mut s = fixture();
    let s3 = Item::new("sword_s3", ItemKind::Weapon, 24, 0, Rarity::Common);
    s.inventory.push(s3);
    let idx = (s.inventory.len() - 1) as u32;
    s.grid.entities[0].stats.level = 19; // req is Lv20
    apply_input(&mut s, InputEvent::EquipItem { entity_id: 1, inventory_idx: idx });
    assert!(s.grid.entities[0].equipped.is_empty(), "Lv19 < req 20 → no-op");
    assert_eq!(s.inventory.len(), idx as usize + 1, "stash untouched on reject");
    s.grid.entities[0].stats.level = 20;
    apply_input(&mut s, InputEvent::EquipItem { entity_id: 1, inventory_idx: idx });
    assert_eq!(s.grid.entities[0].equipped.len(), 1, "at req level it equips");
}

/// 0.0.133: accessories are GENERIC — two rings (both def-home Necklace)
/// land in BOTH accessory slots; a third swaps its home occupant; a pendant
/// (home Ring) routes to a free Necklace when its home is taken.
#[test]
fn accessories_are_generic_two_rings_or_two_pendants() {
    use crate::items::EquipSlot;
    let mut s = fixture();
    let ring = |id: &str| {
        let mut it = Item::new(id, ItemKind::Armor, 0, 2, Rarity::Common);
        it.equip_slot = Some(EquipSlot::Necklace);
        it
    };
    let mut pendant = Item::new("pend", ItemKind::Armor, 0, 3, Rarity::Common);
    pendant.equip_slot = Some(EquipSlot::Ring);
    s.inventory.clear();
    s.inventory.push(ring("ring_a"));
    s.inventory.push(ring("ring_b"));
    s.inventory.push(ring("ring_c"));
    s.inventory.push(pendant);

    // Two rings → Necklace then Ring (stash order shifts via swap_remove:
    // equip index 0 each time).
    apply_input(&mut s, InputEvent::EquipItem { entity_id: 1, inventory_idx: 0 });
    apply_input(&mut s, InputEvent::EquipItem { entity_id: 1, inventory_idx: 0 });
    let eq = &s.grid.entities[0].equipped;
    assert!(eq.iter().any(|i| i.slot() == EquipSlot::Necklace));
    assert!(eq.iter().any(|i| i.slot() == EquipSlot::Ring));
    assert_eq!(eq.len(), 2, "two rings worn at once");

    // Third ring → both taken → swaps the occupant of its HOME slot (Necklace).
    let third = s.inventory.iter().position(|i| i.def_id.starts_with("ring")).unwrap() as u32;
    apply_input(&mut s, InputEvent::EquipItem { entity_id: 1, inventory_idx: third });
    assert_eq!(s.grid.entities[0].equipped.len(), 2, "still two accessories");
    assert!(s.inventory.iter().any(|i| i.def_id.starts_with("ring")), "swapped ring returned");

    // Free Ring (unequip whatever sits there), then equip the PENDANT while
    // its home... make its home the TAKEN one: pendant home = Ring, so first
    // re-occupy: actually unequip Ring occupant → pendant goes to its free home.
    let acc2_idx = s.grid.entities[0]
        .equipped
        .iter()
        .position(|i| i.slot() == EquipSlot::Ring)
        .unwrap() as u32;
    apply_input(&mut s, InputEvent::UnequipItem { entity_id: 1, equipped_slot: acc2_idx });
    let pend_idx = s.inventory.iter().position(|i| i.def_id == "pend").unwrap() as u32;
    apply_input(&mut s, InputEvent::EquipItem { entity_id: 1, inventory_idx: pend_idx });
    // Pendant's home (Ring) was free → sits there, ring keeps Necklace.
    let eq = &s.grid.entities[0].equipped;
    assert!(eq.iter().any(|i| i.def_id == "pend" && i.slot() == EquipSlot::Ring));
    assert!(eq.iter().any(|i| i.def_id.starts_with("ring") && i.slot() == EquipSlot::Necklace));

    // Second pendant → home taken → routes to... Necklace is ALSO taken (ring),
    // so it swaps its home occupant: unequip ring first to expose the free-route.
    let ring_idx = s.grid.entities[0]
        .equipped
        .iter()
        .position(|i| i.slot() == EquipSlot::Necklace)
        .unwrap() as u32;
    apply_input(&mut s, InputEvent::UnequipItem { entity_id: 1, equipped_slot: ring_idx });
    let mut pend2 = Item::new("pend2", ItemKind::Armor, 0, 3, Rarity::Common);
    pend2.equip_slot = Some(EquipSlot::Ring);
    s.inventory.push(pend2);
    let p2 = s.inventory.iter().position(|i| i.def_id == "pend2").unwrap() as u32;
    apply_input(&mut s, InputEvent::EquipItem { entity_id: 1, inventory_idx: p2 });
    let eq = &s.grid.entities[0].equipped;
    assert!(
        eq.iter().any(|i| i.def_id == "pend2" && i.slot() == EquipSlot::Necklace),
        "two pendants: second routed to the free Necklace"
    );
    assert!(eq.iter().any(|i| i.def_id == "pend" && i.slot() == EquipSlot::Ring));
}

#[test]
fn equip_bad_entity_id_is_noop() {
    let mut s = fixture();
    apply_input(&mut s, InputEvent::EquipItem { entity_id: 999, inventory_idx: 0 });
    assert_eq!(s.inventory.len(), 1); // item still in bag
}

#[test]
fn equip_bad_inventory_idx_is_noop() {
    let mut s = fixture();
    apply_input(&mut s, InputEvent::EquipItem { entity_id: 1, inventory_idx: 99 });
    assert_eq!(s.grid.entities[0].equipped.len(), 0);
    assert_eq!(s.inventory.len(), 1);
}

#[test]
fn two_hand_and_shield_forbidden_without_mastery() {
    use crate::items::{EquipSlot, WeaponType};
    let mut s = GameState::new(1, 8, 8);
    let mut e = Entity::idle(1, Cell::new(0, 0));
    e.unit_def = "seraphine".into();
    // a 2H sword already equipped
    let mut gs = Item::new("greatsword", ItemKind::Weapon, 50, 0, Rarity::Common);
    gs.weapon_type = WeaponType::Sword2H;
    e.equipped.push(gs);
    s.grid.spawn(e);
    // a shield waiting in the stash
    let mut sh = Item::new("buckler", ItemKind::Armor, 0, 10, Rarity::Common);
    sh.equip_slot = Some(EquipSlot::Shield);
    s.inventory.push(sh);

    // No mastery → the shield can't go on over a 2H weapon (stays in the bag).
    apply_input(&mut s, InputEvent::EquipItem { entity_id: 1, inventory_idx: 0 });
    assert_eq!(s.inventory.len(), 1, "2H + shield is forbidden without the mastery");
    assert!(!s.grid.entities[0].equipped.iter().any(|i| i.slot() == EquipSlot::Shield));

    // Learn Sword & Shield Mastery → the combo unlocks.
    s.grid.entities[0]
        .skills
        .push(crate::skills::SkillState::new("sword_shield_mastery", 1));
    apply_input(&mut s, InputEvent::EquipItem { entity_id: 1, inventory_idx: 0 });
    assert_eq!(s.inventory.len(), 0, "mastery unlocks 2H + shield");
    assert!(s.grid.entities[0].equipped.iter().any(|i| i.slot() == EquipSlot::Shield));
}

#[test]
fn level_skill_without_a_board_node_is_a_no_op() {
    // 0.0.352: the Job skill tree and its points are gone; a skill rank rises only through
    // Potential. `LevelSkill` on a legacy tree-only skill parses and changes nothing.
    let mut s = fixture();
    s.grid.entities[0].unit_def = "seraphine".to_string();
    let before = s.clone();
    apply_input(&mut s, InputEvent::LevelSkill { entity_id: 1, skill_id: "iron_chains".into() });
    assert_eq!(s, before);
}

fn quiver(element: crate::grid::Element, shots: u32, qty: u32) -> Item {
    use crate::items::{ConsumableSpec, ConsumeEffect, ConsumeTrigger};
    let mut it = Item::new("quiver", ItemKind::Consumable, 0, 0, Rarity::Common);
    it.quantity = qty;
    it.consumable = Some(ConsumableSpec { provision: None,
        trigger: ConsumeTrigger::Manual,
        effect: ConsumeEffect::LoadAmmo { element, shots },
        cooldown: 0,
    });
    it
}

#[test]
fn equip_card_to_panel_activates_up_to_gear_sockets() {
    use crate::combat::effective_atk;
    use crate::items::{CardClass, EquipSlot, StatMod};
    let mut s = fixture();
    // A 1-socket weapon equipped (equip_slot set so it gates the Weapon card class).
    let mut weapon = Item::new("slotted_sword", ItemKind::Weapon, 0, 0, Rarity::Common);
    weapon.grade = crate::items::Grade::Rare; // sockets are the grade's (0.0.388)
    // ATK 0 so the grade changes below move only the sockets, not the weapon's own ATK.
    weapon.equip_slot = Some(EquipSlot::Weapon);
    s.grid.entities[0].equipped.push(weapon);
    let atk0 = effective_atk(&s.grid.entities[0]);
    // Two weapon cards (+5 ATK each) as one stash stack.
    let mut card = Item::new("boar_card", ItemKind::Card, 0, 0, Rarity::Rare);
    card.quantity = 2;
    card.card_slot = Some(CardClass::Weapon);
    card.card_mods = vec![StatMod::AtkFlat(5)];
    s.card_album.push(card);
    let cidx = (s.card_album.len() - 1) as u32;

    // Equip BOTH into the weapon panel (cap 4 → both fit).
    apply_input(&mut s, InputEvent::EquipCard { entity_id: 1, album_idx: cidx, target: None });
    apply_input(&mut s, InputEvent::EquipCard { entity_id: 1, album_idx: cidx, target: None });
    assert_eq!(s.grid.entities[0].card_panel.len(), 2, "both cards equipped to the panel");
    // Only 1 gear socket → only 1 active.
    assert_eq!(effective_atk(&s.grid.entities[0]), atk0 + 5, "1 active (gear has 1 socket)");
    // Upgrade the weapon to 2 sockets → both activate (no re-equip needed).
    s.grid.entities[0].equipped[0].grade = crate::items::Grade::Arcana; // sockets are the grade's (0.0.388)
    assert_eq!(effective_atk(&s.grid.entities[0]), atk0 + 10, "both active with 2 sockets");

    // Unequip one → back to the stash, one active again.
    apply_input(&mut s, InputEvent::UnequipCard { entity_id: 1, panel_idx: 0 });
    assert_eq!(s.grid.entities[0].card_panel.len(), 1, "card returned to stash");
    assert_eq!(effective_atk(&s.grid.entities[0]), atk0 + 5, "one card left");
}

#[test]
fn card_persists_across_gear_swap() {
    use crate::combat::effective_atk;
    use crate::items::{CardClass, EquipSlot, StatMod};
    let mut s = fixture();
    let mut weapon = Item::new("w1", ItemKind::Weapon, 0, 0, Rarity::Common);
    weapon.grade = crate::items::Grade::Rare; // sockets are the grade's (0.0.388)
    // ATK 0 so the grade changes below move only the sockets, not the weapon's own ATK.
    weapon.equip_slot = Some(EquipSlot::Weapon);
    s.grid.entities[0].equipped.push(weapon);
    let atk0 = effective_atk(&s.grid.entities[0]);
    let mut card = Item::new("boar_card", ItemKind::Card, 0, 0, Rarity::Rare);
    card.card_slot = Some(CardClass::Weapon);
    card.card_mods = vec![StatMod::AtkFlat(5)];
    s.card_album.push(card);
    let cidx = (s.card_album.len() - 1) as u32;
    apply_input(&mut s, InputEvent::EquipCard { entity_id: 1, album_idx: cidx, target: None });
    assert_eq!(effective_atk(&s.grid.entities[0]), atk0 + 5, "active with a socketed weapon");
    // Swap to a 0-socket weapon → the card STAYS in the panel, just inactive.
    s.grid.entities[0].equipped[0].grade = crate::items::Grade::Common; // sockets are the grade's (0.0.388)
    assert_eq!(s.grid.entities[0].card_panel.len(), 1, "card stays on the character");
    assert_eq!(effective_atk(&s.grid.entities[0]), atk0, "inactive: no socket to gate it");
    // Back to a socketed weapon → it reactivates.
    s.grid.entities[0].equipped[0].grade = crate::items::Grade::Rare; // sockets are the grade's (0.0.388)
    assert_eq!(effective_atk(&s.grid.entities[0]), atk0 + 5, "reactivates on socketed gear");
}

/// 0.0.128 regression guard: a foil PANEL card is a different album stack than the
/// plain one. Unequipping it into a full album that only holds the PLAIN stack must
/// no-op (stay equipped) — merging would silently strip the foil bit; and when a slot
/// frees up it must come back as its own foil row.
#[test]
fn unequip_foil_card_never_merges_onto_the_plain_stack() {
    use crate::items::CardClass;
    let mut s = fixture();
    let mut foil = Item::new("boar_card", ItemKind::Card, 0, 0, Rarity::Rare);
    foil.card_slot = Some(CardClass::Weapon);
    foil.foil = true;
    s.grid.entities[0].card_panel.push(foil);
    // A PLAIN stack of the same type sits in the (now unbounded) album.
    let mut plain = Item::new("boar_card", ItemKind::Card, 0, 0, Rarity::Rare);
    plain.card_slot = Some(CardClass::Weapon);
    s.card_album.push(plain);
    // Unequip the FOIL: it must return on its OWN row, NEVER merge onto the plain
    // stack (that would silently erase the foil bit).
    apply_input(&mut s, InputEvent::UnequipCard { entity_id: 1, panel_idx: 0 });
    assert!(s.grid.entities[0].card_panel.is_empty(), "foil unequipped (album unbounded)");
    let rows: Vec<_> = s.card_album.iter().filter(|c| c.def_id == "boar_card").collect();
    assert_eq!(rows.len(), 2, "foil landed on its own row, not the plain stack");
    assert!(rows.iter().any(|c| c.foil && c.quantity == 1));
    assert!(rows.iter().any(|c| !c.foil && c.quantity == 1));
}

/// 0.0.129 chum set: a DRAWBACK card (Burst Chom: atk +12 / def −3) sums its
/// negative line through the stat-mod pipeline — the penalty is real, not clamped away.
#[test]
fn drawback_card_applies_its_negative_effect() {
    use crate::items::{CardClass, EquipSlot, StatMod};
    let mut s = fixture();
    let mut acc = Item::new("ring", ItemKind::Armor, 0, 0, Rarity::Common);
    acc.grade = crate::items::Grade::Rare; // sockets are the grade's (0.0.388)
    acc.equip_slot = Some(EquipSlot::Necklace);
    s.grid.entities[0].equipped.push(acc);
    let mut card = Item::new("burst_chom_like", ItemKind::Card, 0, 0, Rarity::Rare);
    card.card_slot = Some(CardClass::Necklace);
    card.card_mods = vec![StatMod::AtkFlat(12), StatMod::DefFlat(-3)];
    let e = &mut s.grid.entities[0];
    let (atk0, def0) = (crate::combat::effective_atk(e), crate::combat::effective_def(e));
    e.card_panel.push(card);
    let e = &s.grid.entities[0];
    assert_eq!(crate::combat::effective_atk(e) - atk0, 12, "upside applies");
    assert_eq!(crate::combat::effective_def(e) - def0, -3, "drawback applies (negative survives)");
}

/// 0.0.129 chum set: hp_pct is BASIS POINTS (300 = +3%) folded into max HP —
/// the Rechum Card's axis. Pins the unit so nobody authors hp_pct: 3 for 3%.
#[test]
fn hp_pct_card_scales_max_hp_in_basis_points() {
    use crate::items::{CardClass, EquipSlot, StatMod};
    let mut s = fixture();
    let mut shoes = Item::new("boots", ItemKind::Armor, 0, 0, Rarity::Common);
    shoes.grade = crate::items::Grade::Rare; // sockets are the grade's (0.0.388)
    shoes.equip_slot = Some(EquipSlot::Shoes);
    s.grid.entities[0].equipped.push(shoes);
    let hp0 = crate::combat::effective_max_hp(&s.grid.entities[0]);
    let mut card = Item::new("rechum_like", ItemKind::Card, 0, 0, Rarity::Rare);
    card.card_slot = Some(CardClass::Shoes);
    card.card_mods = vec![StatMod::MaxHpBp(300)];
    s.grid.entities[0].card_panel.push(card);
    let hp1 = crate::combat::effective_max_hp(&s.grid.entities[0]);
    assert_eq!(hp1, hp0 * 10_300 / 10_000, "+3% from 300 bp");
}

#[test]
fn foil_card_adds_its_extra_on_top_of_epic() {
    use crate::items::{CardClass, EquipSlot, StatMod};
    let mut s = fixture();
    let mut armor = Item::new("plate", ItemKind::Armor, 0, 0, Rarity::Common);
    armor.grade = crate::items::Grade::Rare; // 1 socket → the panel card is active // sockets are the grade's (0.0.388)
    armor.equip_slot = Some(EquipSlot::Armor);
    s.grid.entities[0].equipped.push(armor);
    // A FOIL Pinkling: Epic +5 ATK/+1 STR, foil ADDS +5 ATK → total +10 ATK/+1 STR.
    let mut card = Item::new("pinkling", ItemKind::Card, 0, 0, Rarity::Rare);
    card.card_slot = Some(CardClass::Armor);
    card.card_mods = vec![StatMod::AtkFlat(5)];
    card.foil_mods = vec![StatMod::AtkFlat(5)];
    card.foil = true;
    s.grid.entities[0].card_panel.push(card);
    let e = &s.grid.entities[0];
    assert_eq!(crate::combat::stat_mods::atk_flat(e), 10, "foil = Epic +5 plus the foil extra +5");

    // Without the foil flag, only the Epic effect applies.
    s.grid.entities[0].card_panel[0].foil = false;
    assert_eq!(
        crate::combat::stat_mods::atk_flat(&s.grid.entities[0]), 5, "non-foil = Epic only"
    );
}

/// 0.0.243: a card's star ladder is AUTHORED, not a ×(1+0.2·star) fold — `card_star_step`
/// climbs every star while `card_star_ranks` bumps only at the ranks it names (so a small
/// flat stat can step at 1★/3★/5★ instead of being floored into nothing). The FOIL extra
/// carries its own step.
#[test]
fn card_star_ladder_is_authored_per_star() {
    use crate::items::{CardClass, CardStarRank, EquipSlot, StatMod};
    let mut s = fixture();
    let mut armor = Item::new("plate", ItemKind::Armor, 0, 0, Rarity::Common);
    armor.grade = crate::items::Grade::Rare; // sockets are the grade's (0.0.388)
    armor.equip_slot = Some(EquipSlot::Armor);
    s.grid.entities[0].equipped.push(armor);
    let mut card = Item::new("pinkling", ItemKind::Card, 0, 0, Rarity::Rare);
    card.card_slot = Some(CardClass::Armor);
    // Pinkling's real ladder: ATK +1/star, CRIT +1% at 1★/3★/5★, foil ATK +1/star.
    card.card_mods = vec![StatMod::AtkFlat(5), StatMod::CritBp(100)];
    card.card_star_step = vec![StatMod::AtkFlat(1)];
    card.card_star_ranks = (1..=3)
        .map(|i| CardStarRank { star: i * 2 - 1, mods: vec![StatMod::CritBp(100)] })
        .collect();
    card.foil_mods = vec![StatMod::AtkFlat(5)];
    card.foil_star_step = vec![StatMod::AtkFlat(1)];
    card.foil = true;
    card.star = 3;
    s.grid.entities[0].card_panel.push(card);
    let e = &s.grid.entities[0];
    // ATK: (5 + 3) base + (5 + 3) foil = 16.
    assert_eq!(crate::combat::stat_mods::atk_flat(e), 16, "3★: base 8 + foil 8");
    let card = &mut s.grid.entities[0].card_panel[0];
    card.star = 5;
    assert_eq!(crate::combat::stat_mods::atk_flat(&s.grid.entities[0]), 20, "5★: 10 + 10");
}

#[test]
fn accessory_card_equips_into_either_panel_via_target() {
    use crate::items::{CardClass, StatMod};
    let mut s = fixture();
    // A generic accessory card (canonical class Necklace).
    let mut card = Item::new("gemstone", ItemKind::Card, 0, 0, Rarity::Rare);
    card.quantity = 2;
    card.card_slot = Some(CardClass::Necklace);
    card.card_mods = vec![StatMod::CritBp(300)];
    s.card_album.push(card);
    let cidx = (s.card_album.len() - 1) as u32;

    // Target Ring → it lands in the Ring panel (stamped).
    apply_input(&mut s, InputEvent::EquipCard {
        entity_id: 1, album_idx: cidx, target: Some(CardClass::Ring),
    });
    // The other copy targets Necklace → the SAME card sits in both accessory panels.
    apply_input(&mut s, InputEvent::EquipCard {
        entity_id: 1, album_idx: cidx, target: Some(CardClass::Necklace),
    });
    let panel = &s.grid.entities[0].card_panel;
    assert_eq!(panel.len(), 2, "both copies equipped");
    assert!(panel.iter().any(|c| c.card_slot == Some(CardClass::Necklace)), "one in Accessory1");
    assert!(panel.iter().any(|c| c.card_slot == Some(CardClass::Ring)), "one in Accessory2");

    // A weapon card can NOT be routed to an accessory panel (target mismatch → no-op).
    let mut wc = Item::new("boar", ItemKind::Card, 0, 0, Rarity::Rare);
    wc.card_slot = Some(CardClass::Weapon);
    s.card_album.push(wc);
    let wi = (s.card_album.len() - 1) as u32;
    apply_input(&mut s, InputEvent::EquipCard {
        entity_id: 1, album_idx: wi, target: Some(CardClass::Necklace),
    });
    assert_eq!(s.grid.entities[0].card_panel.len(), 2, "weapon card rejected from an accessory panel");
}

#[test]
fn equip_card_rejects_non_card_and_respects_panel_cap() {
    use crate::items::CardClass;
    let mut s = fixture();
    // A non-card sitting in the album slot 0 must be rejected by the kind guard.
    s.card_album.push(Item::new("sword_short", ItemKind::Weapon, 10, 0, Rarity::Common));
    apply_input(&mut s, InputEvent::EquipCard { entity_id: 1, album_idx: 0, target: None });
    assert!(s.grid.entities[0].card_panel.is_empty(), "a non-card is not equipped");
    // Shield panel cap = 1. Two shield cards in one album stack.
    let mut card = Item::new("thorn_card", ItemKind::Card, 0, 0, Rarity::Rare);
    card.quantity = 2;
    card.card_slot = Some(CardClass::Shield);
    card.card_mods = vec![crate::items::StatMod::DefFlat(2)];
    s.card_album.push(card);
    let cidx = (s.card_album.len() - 1) as u32;
    apply_input(&mut s, InputEvent::EquipCard { entity_id: 1, album_idx: cidx, target: None });
    apply_input(&mut s, InputEvent::EquipCard { entity_id: 1, album_idx: cidx, target: None });
    let shields = s.grid.entities[0]
        .card_panel
        .iter()
        .filter(|c| c.card_slot == Some(CardClass::Shield))
        .count();
    assert_eq!(shields, 1, "shield panel caps at 1");
    assert_eq!(s.card_album[cidx as usize].quantity, 1, "the second equip was a no-op");
}

#[test]
fn load_ammo_fills_pool_and_spends_one_quiver() {
    use crate::grid::Element;
    let mut s = fixture();
    s.inventory.push(quiver(Element::Fire, 10_000, 2));
    let qidx = (s.inventory.len() - 1) as u32;
    apply_input(&mut s, InputEvent::LoadAmmo { entity_id: 1, inv_idx: qidx });
    // One quiver's worth of rounds loaded; the stack drops to 1.
    assert_eq!(s.grid.entities[0].ammo.len(), 1);
    assert_eq!(s.grid.entities[0].ammo[0].element, Element::Fire);
    assert_eq!(s.grid.entities[0].ammo[0].shots, 10_000);
    assert_eq!(s.inventory[qidx as usize].quantity, 1, "one quiver consumed");

    // Loading the last quiver removes the depleted stash entry and stacks
    // the rounds onto the existing Fire pool.
    apply_input(&mut s, InputEvent::LoadAmmo { entity_id: 1, inv_idx: qidx });
    assert_eq!(s.grid.entities[0].ammo[0].shots, 20_000);
    assert!(s.inventory.iter().all(|it| it.def_id != "quiver"), "empty quiver gone from stash");
}

#[test]
fn load_ammo_ignores_non_quiver_and_bad_inputs() {
    use crate::grid::Element;
    let mut s = fixture(); // stash[0] is a sword, not a quiver
    apply_input(&mut s, InputEvent::LoadAmmo { entity_id: 1, inv_idx: 0 });
    assert!(s.grid.entities[0].ammo.is_empty(), "a weapon never loads as ammo");
    assert_eq!(s.inventory.len(), 1, "non-quiver stays in the bag");
    // Bad entity / index are silent no-ops (replay safety).
    s.inventory.push(quiver(Element::Ice, 5, 1));
    apply_input(&mut s, InputEvent::LoadAmmo { entity_id: 999, inv_idx: 1 });
    apply_input(&mut s, InputEvent::LoadAmmo { entity_id: 1, inv_idx: 99 });
    assert!(s.grid.entities[0].ammo.is_empty());
}

#[cfg(test)]
mod loadout_tests {
    use super::*;
    use crate::grid::{Cell, Entity};
    use crate::skills::SkillState;
    use crate::state::GameState;

    // bash + fireball are Active; keen_eye is Passive (not slottable).
    fn player_with_skills() -> GameState {
        let mut s = GameState::new(1, 8, 8);
        let mut e = Entity::idle(1, Cell::new(0, 0));
        e.skills = vec![
            SkillState::new("bash", 1),
            SkillState::new("fireball", 1),
            SkillState::new("keen_eye", 1),
        ];
        s.grid.spawn(e);
        s
    }

    #[test]
    fn empty_loadout_casts_nothing() {
        let s = player_with_skills();
        let e = &s.grid.entities[0];
        assert!(e.active_skills.is_empty());
        assert!(!e.skill_slotted("bash") && !e.skill_slotted("fireball"),
            "nothing fires until slotted");
    }

    #[test]
    fn toggle_slots_then_unslots() {
        let mut s = player_with_skills();
        // Slot fireball → only fireball fires.
        apply_input(&mut s, InputEvent::ToggleActiveSkill { entity_id: 1, skill_id: "fireball".into() });
        let e = &s.grid.entities[0];
        assert_eq!(e.active_skills, vec!["fireball".to_string()]);
        assert!(e.skill_slotted("fireball") && !e.skill_slotted("bash"));
        // Toggle again → unslotted.
        apply_input(&mut s, InputEvent::ToggleActiveSkill { entity_id: 1, skill_id: "fireball".into() });
        assert!(s.grid.entities[0].active_skills.is_empty());
    }

    #[test]
    fn auto_slot_fills_then_stops_at_cap() {
        use crate::grid::ACTIVE_SLOT_CAP;
        let mut e = Entity::idle(1, Cell::new(0, 0));
        e.auto_slot_skill("fireball");
        e.auto_slot_skill("fireball"); // dup → no double-slot
        assert_eq!(e.active_skills, vec!["fireball".to_string()]);
        for n in 0..(ACTIVE_SLOT_CAP + 3) {
            e.auto_slot_skill(&format!("s{n}"));
        }
        assert_eq!(e.active_skills.len(), ACTIVE_SLOT_CAP, "capped at the slot limit");
    }

    #[test]
    fn passive_and_unknown_are_noops() {
        let mut s = player_with_skills();
        apply_input(&mut s, InputEvent::ToggleActiveSkill { entity_id: 1, skill_id: "keen_eye".into() });
        apply_input(&mut s, InputEvent::ToggleActiveSkill { entity_id: 1, skill_id: "ghost".into() });
        assert!(s.grid.entities[0].active_skills.is_empty(), "passive/unknown ignored");
    }
}


#[test]
fn deposit_and_withdraw_item_round_trips_between_inventory_and_stash() {
    let mut s = fixture(); // inventory: 1 sword
    assert_eq!(s.inventory.len(), 1);
    assert!(s.stash.is_empty());
    // Deposit the sword into the warehouse.
    apply_input(&mut s, InputEvent::DepositToStash { inv_idx: 0 });
    assert!(s.inventory.is_empty(), "item left the inventory");
    assert_eq!(s.stash.len(), 1, "item is in the warehouse");
    // Withdraw it back.
    apply_input(&mut s, InputEvent::WithdrawFromStash { stash_idx: 0 });
    assert_eq!(s.inventory.len(), 1, "item is back in the inventory");
    assert!(s.stash.is_empty());
    // Bad indices are silent no-ops.
    apply_input(&mut s, InputEvent::DepositToStash { inv_idx: 99 });
    apply_input(&mut s, InputEvent::WithdrawFromStash { stash_idx: 99 });
    assert_eq!(s.inventory.len(), 1);
}

#[test]
fn deposit_to_full_stash_restores_the_item() {
    let mut s = fixture();
    // Fill the warehouse to its base cap with distinct non-stackable gear.
    for k in 0..crate::drops::STASH_CAPACITY {
        s.stash.push(Item::new(&format!("filler_{k}"), ItemKind::Weapon, 1, 0, Rarity::Common));
    }
    assert_eq!(s.stash.len(), s.stash_cap());
    apply_input(&mut s, InputEvent::DepositToStash { inv_idx: 0 });
    // Full + non-stackable → the sword stays in the inventory, never lost.
    assert_eq!(s.inventory.len(), 1, "deposit into a full warehouse is a no-op");
    assert_eq!(s.stash.len(), s.stash_cap());
}

#[test]
fn deposit_and_withdraw_card_between_inventory_and_album() {
    let mut s = fixture();
    let card = Item::new("boar_card", ItemKind::Card, 0, 0, Rarity::Rare);
    s.inventory.push(card); // idx 1 (sword at 0)
    apply_input(&mut s, InputEvent::DepositCardToAlbum { inv_idx: 1 });
    assert!(s.card_album.iter().any(|i| i.def_id == "boar_card"), "card -> album");
    assert!(!s.inventory.iter().any(|i| i.def_id == "boar_card"), "card left inventory");
    // A non-card slot is rejected (the sword at idx 0 stays put).
    apply_input(&mut s, InputEvent::DepositCardToAlbum { inv_idx: 0 });
    assert_eq!(s.inventory.len(), 1, "non-card deposit is a no-op");
    // Withdraw the card back to the inventory.
    let ai = s.card_album.iter().position(|i| i.def_id == "boar_card").unwrap();
    apply_input(&mut s, InputEvent::WithdrawCardFromAlbum { album_idx: ai as u32 });
    assert!(s.inventory.iter().any(|i| i.def_id == "boar_card"), "card back in inventory");
}

#[test]
fn foil_card_deposits_as_its_own_album_stack() {
    let mut s = fixture();
    // Album already holds a PLAIN boar card.
    let plain = Item::new("boar_card", ItemKind::Card, 0, 0, Rarity::Rare);
    s.card_album.push(plain);
    // A FOIL of the same type sits in the inventory.
    let mut foil = Item::new("boar_card", ItemKind::Card, 0, 0, Rarity::Rare);
    foil.foil = true;
    s.inventory.push(foil); // idx 1
    apply_input(&mut s, InputEvent::DepositCardToAlbum { inv_idx: 1 });
    // Foil must NOT merge onto the plain stack — two separate album rows.
    let boar_rows = s.card_album.iter().filter(|i| i.def_id == "boar_card").count();
    assert_eq!(boar_rows, 2, "foil is its own stack, never folded onto the plain copy");
}

#[test]
fn the_retired_carrier_events_are_tombstones() {
    // 0.0.350: an Exemplar is a UNIT. The three carrier inputs still parse (old event logs) and
    // change nothing — no panic, no state.
    let mut s = fixture();
    let before = crate::hash::state_hash(&s);
    apply_input(&mut s, InputEvent::EquipExemplar { entity_id: 1, owned_idx: 0 });
    apply_input(&mut s, InputEvent::FeedExemplar { target_idx: 0, feeder_idx: 1 });
    apply_input(&mut s, InputEvent::UnequipExemplar { entity_id: 1 });
    assert_eq!(crate::hash::state_hash(&s), before, "tombstones touch nothing");
}

#[test]
fn legacy_carriers_migrate_into_units_and_credits_once() {
    // A pre-0.0.350 save: a swordsman WEARING Rowena (fed to 2★), one unworn Seraphine carrier
    // fed to 1★, and an unworn second Seraphine. After `migrate_save_items`: the wearer keeps
    // its overlay and takes the 2★; Seraphine's first carrier is a benched unit and its star is a
    // credit; the second carrier is one more credit; the carrier list is empty; and a second
    // run changes nothing.
    use crate::items::{Exemplar, Item, ItemKind, Rarity};
    let d = crate::data::GameData::from_json_str(include_str!("../../../../data/game_data.json"))
        .expect("game_data.json must parse");
    let mut s = GameState::new(1, 16, 8);
    let sw = d.unit("seraphine").unwrap();
    let mut wearer = crate::data::build_player_entity(&d, sw, 1, Cell::new(0, 0));
    wearer.exemplar = Some(Exemplar { id: "rowena".into(), name: String::new(), skin_id: String::new(), stars: 0 });
    s.grid.spawn(wearer);
    let mut worn = Item::new("exemplar_rowena", ItemKind::Exemplar, 0, 0, Rarity::Legendary);
    worn.equipped_by = 1;
    worn.resonance_stars = 2;
    let mut sera = Item::new("exemplar_seraphine", ItemKind::Exemplar, 0, 0, Rarity::Legendary);
    sera.resonance_stars = 1;
    let sera2 = Item::new("exemplar_seraphine", ItemKind::Exemplar, 0, 0, Rarity::Legendary);
    s.owned_exemplars = vec![worn, sera, sera2];

    crate::data::migrate_save_items(&mut s, &d);

    assert!(s.owned_exemplars.is_empty(), "no carriers survive");
    let w = s.grid.entity_of_id(1).unwrap().exemplar.as_ref().unwrap();
    assert_eq!((w.id.as_str(), w.stars), ("rowena", 2), "the wearer IS the unit and keeps its stars");
    assert!(!s.pending_resonance.contains_key("rowena"), "a worn carrier banks no credit");
    let sera_units: Vec<_> = s.roster.values().filter(|rc| rc.data.exemplar.as_ref().is_some_and(|x| x.id == "seraphine")).collect();
    assert_eq!(sera_units.len(), 1, "one Seraphine unit, benched");
    assert_eq!(sera_units[0].data.exemplar.as_ref().unwrap().stars, 0, "stars are not auto-applied");
    assert_eq!(s.pending_resonance["seraphine"], 2, "1 fed star + 1 dupe = 2 credits for the player");

    let h = crate::hash::state_hash(&s);
    crate::data::migrate_save_items(&mut s, &d);
    assert_eq!(crate::hash::state_hash(&s), h, "idempotent");
}

#[test]
fn a_carrier_worn_by_a_benched_unit_migrates_onto_that_unit() {
    // The wearer was benched before the upgrade: `capture()` carried its overlay into the roster
    // and the carrier still says `equipped_by = 2`. The bench unit must take the stars — no credit,
    // no second unit.
    use crate::items::{Exemplar, Item, ItemKind, Rarity};
    let d = crate::data::GameData::from_json_str(include_str!("../../../../data/game_data.json")).unwrap();
    let mut s = GameState::new(1, 16, 8);
    let sw = d.unit("seraphine").unwrap();
    s.grid.spawn(crate::data::build_player_entity(&d, sw, 1, Cell::new(0, 0)));
    let mut benched = crate::data::build_player_entity(&d, sw, 2, Cell::new(1, 0));
    benched.exemplar = Some(Exemplar { id: "rowena".into(), name: String::new(), skin_id: String::new(), stars: 0 });
    s.roster.insert(2, crate::state::RosterChar { unit_def: "seraphine".into(),
        data: crate::state::CharacterData::capture(&mut benched) });
    let mut worn = Item::new("exemplar_rowena", ItemKind::Exemplar, 0, 0, Rarity::Legendary);
    worn.equipped_by = 2;
    worn.resonance_stars = 3;
    s.owned_exemplars = vec![worn];

    crate::data::migrate_save_items(&mut s, &d);

    assert_eq!(s.roster.len(), 1, "no second unit");
    assert_eq!(s.roster[&2].data.exemplar.as_ref().unwrap().stars, 3, "the bench unit took the stars");
    assert!(s.pending_resonance.is_empty(), "a worn carrier banks no credit");
}

/// AUDIT 2026-10-02: LeaveInstance with no return point goes to the party's own reached band,
/// not band 10.
#[test]
fn leaving_with_no_return_point_does_not_skip_bands() {
    let d = crate::data::GameData::from_json_str(include_str!("../../../../data/game_data.json")).unwrap();
    let mut s = crate::data::new_game_in_zone(1, &d, "zone_b1_1").unwrap();
    assert!(s.return_zone_id.is_empty());
    apply_input_with(&mut s, Some(&d), InputEvent::LeaveInstance);
    assert_eq!(s.current_zone_id.as_deref(), Some("zone_b1_1"));
    assert!(s.band_reached <= 1, "band_reached stays put");
}

/// AUDIT 2026-10-02: the active-skill toggle is the party's — a monster's loadout cannot be edited
/// by a client input.
#[test]
fn a_monster_s_active_skills_cannot_be_toggled_off() {
    assert!(matches!(crate::skills::skill_def("fireball").map(|d| d.kind), Some(crate::skills::SkillKind::Active)),
        "the fixture needs a real Active skill or the toggle refuses for another reason");
    let mut s = fixture();
    let mut boss = Entity::idle(1000, Cell::new(3, 0)).with_team(crate::grid::Team::Monster);
    boss.skills = vec![crate::skills::SkillState::new("fireball", 1)];
    boss.active_skills = vec!["fireball".to_string()];
    s.grid.spawn(boss);
    apply_input(&mut s, InputEvent::ToggleActiveSkill { entity_id: 1000, skill_id: "fireball".into() });
    assert_eq!(s.grid.entities[1].active_skills, vec!["fireball".to_string()], "the boss keeps its skill");
}

/// AUDIT 2026-10-02: equipping a card another member wears MOVES it — the old wearer loses the
/// overlay too, so one card is never in effect on two members (nor in effect while salvaged).
#[test]
fn equipping_a_worn_regalia_moves_it_off_the_old_wearer() {
    let mut s = fixture();
    s.grid.spawn(Entity::idle(2, Cell::new(1, 0)));
    s.owned_regalia.push(Item::new("regalia_litany_of_dawn", ItemKind::Regalia, 0, 0, Rarity::Legendary));
    apply_input(&mut s, InputEvent::EquipRegalia { entity_id: 1, owned_idx: 0 });
    assert!(s.grid.entities[0].regalia.is_some(), "member 1 wears it");
    apply_input(&mut s, InputEvent::EquipRegalia { entity_id: 2, owned_idx: 0 });
    assert_eq!(s.owned_regalia[0].equipped_by, 2);
    assert!(s.grid.entities[1].regalia.is_some(), "member 2 wears it now");
    assert!(s.grid.entities[0].regalia.is_none(), "and member 1 no longer does");
}

/// ENHANCE IS XP (0.0.490): Dust adds XP, the level follows the rung's curve, the card never
/// passes the cap, and Dust the cap does not need is NOT spent.
#[test]
fn enhance_adds_xp_along_the_curve_and_stops_at_the_cap() {
    use crate::items::{regalia_xp_to_level, REGALIA_BASE_CAP, REGALIA_MAX_LEVEL, REGALIA_OVERCAP_XP, REGALIA_TOTAL_XP};
    let mut s = fixture();
    s.owned_regalia.push(Item::new("regalia_litany_of_dawn", ItemKind::Regalia, 0, 0, Rarity::Legendary));
    apply_input(&mut s, InputEvent::EquipRegalia { entity_id: 1, owned_idx: 0 });
    let feed = |s: &mut crate::state::GameState, def: &str, n: u32| {
        s.credit_material(def, n, crate::state::MaterialSource::Drop);
        apply_input(s, InputEvent::EnhanceRegaliaDust { target_idx: 0, dust_id: def.to_string(), count: n });
    };
    let at = |s: &crate::state::GameState| (s.owned_regalia[0].enhance_level, s.owned_regalia[0].enhance_xp);

    // The curve: 1M to Lv60 for an SSR, the last step ~4x the first, and it sums exactly.
    assert_eq!(regalia_xp_to_level(2, REGALIA_BASE_CAP), REGALIA_TOTAL_XP[2], "2M to Lv60");
    assert_eq!(regalia_xp_to_level(2, REGALIA_MAX_LEVEL), REGALIA_TOTAL_XP[2] + REGALIA_OVERCAP_XP[2], "+20M to Lv70");
    let late = |l: u8| regalia_xp_to_level(2, l + 1) - regalia_xp_to_level(2, l);
    assert!(late(60) > late(59), "Lv60→61 costs more than the last base step");
    assert!(late(69) > late(60) * 15, "the late sink is steep: {} vs {}", late(69), late(60));
    let step = |l: u8| regalia_xp_to_level(2, l + 1) - regalia_xp_to_level(2, l);
    assert!(step(59) > step(0) * 3, "late levels cost more: {} vs {}", step(59), step(0));

    feed(&mut s, "regalia_sr_fodder", 4); // 40,000 XP
    let want = crate::items::regalia_level_of_xp(2, 40_000);
    assert_eq!(at(&s), want, "40k XP lands where the curve says");
    assert!(want.0 >= 10, "the early levels are cheap: 40k XP is Lv{}", want.0);
    assert_eq!(regalia_xp_to_level(2, want.0) + want.1, 40_000, "no XP lost");
    assert_eq!(s.grid.entities[0].regalia.as_ref().map(|r| r.level), Some(want.0), "the overlay follows");

    // 300 Crystals is past the cap: only what the cap needs is spent (22M − 40k = 219.6 → 220).
    feed(&mut s, "regalia_crystal_fodder", 300);
    assert_eq!(at(&s), (REGALIA_MAX_LEVEL, 0), "capped, no remainder");
    assert_eq!(s.material_count("regalia_crystal_fodder"), 300 - 220, "only 220 Crystals spent");
    feed(&mut s, "regalia_fodder", 1);
    assert_eq!(s.material_count("regalia_fodder"), 1, "a feed at the cap spends nothing");
}

#[test]
fn regalia_equip_resonance_and_unequip_overlay() {
    // P0 overlay: equip a Regalia → entity.regalia set + its stat folds into card_bonuses; feed a
    // dupe → Resonance +1 (stat grows); unequip → cleared. Its own slot, independent of Exemplar.
    use crate::items::{Item, ItemKind, Rarity};
    // The overlay grant flows through the `stat_mods` pipeline (0.0.24x unify), NOT
    // `card_bonuses` — and since 0.0.330 a MATK line IS an ATK line, so the Regalia's
    // grant is read through `atk_flat` like every other flat-attack source.
    let matk_flat = |s: &crate::state::GameState| crate::combat::stat_mods::atk_flat(&s.grid.entities[0]);
    let mut s = fixture();
    let base_matk = matk_flat(&s);
    s.owned_regalia.push(Item::new("regalia_litany_of_dawn", ItemKind::Regalia, 0, 0, Rarity::Legendary));

    apply_input(&mut s, InputEvent::EquipRegalia { entity_id: 1, owned_idx: 0 });
    assert_eq!(s.grid.entities[0].regalia.as_ref().map(|r| r.id.as_str()), Some("litany_of_dawn"));
    assert_eq!(s.owned_regalia[0].equipped_by, 1, "carrier marked");
    assert_eq!(matk_flat(&s), base_matk + 200, "Lv0/S0 = the SSR lump at the floor (acolyte line)");

    // LEVEL axis: feed an R shard → +5 levels → the lump climbs its own curve. Exemplar has NO level.
    s.credit_material("regalia_sr_fodder", 4, crate::state::MaterialSource::Drop);
    apply_input(&mut s, InputEvent::EnhanceRegaliaDust { target_idx: 0, dust_id: "regalia_sr_fodder".into(), count: 4 });
    let lv = crate::items::regalia_level_of_xp(2, 40_000).0;
    assert_eq!(s.grid.entities[0].regalia.as_ref().map(|r| r.level), Some(lv), "40k XP: Lv0 → Lv{lv}");
    assert_eq!(s.material_count("regalia_sr_fodder"), 0, "Dust spent");
    let lump5 = crate::items::regalia_lump_at(200, lv);
    assert!(lump5 > 200, "the lump grew: {lump5}");
    assert_eq!(matk_flat(&s), base_matk + lump5, "the lump at that level");

    // RESONANCE (Star axis): feed a duplicate → +1 star. The passive web is empty today, so the
    // numbers do not move — only the rank does.
    s.owned_regalia.push(Item::new("regalia_litany_of_dawn", ItemKind::Regalia, 0, 0, Rarity::Legendary));
    apply_input(&mut s, InputEvent::FeedRegalia { target_idx: 0, feeder_idx: 1 });
    assert_eq!(s.grid.entities[0].regalia.as_ref().map(|r| (r.level, r.stars)), Some((lv, 1)), "Lv + R1");
    assert_eq!(matk_flat(&s), base_matk + lump5, "a rank adds nothing while the passive is empty");

    // Independent of the Exemplar slot: equipping a Regalia never touches exemplar.
    assert!(s.grid.entities[0].exemplar.is_none(), "regalia doesn't set exemplar");

    apply_input(&mut s, InputEvent::UnequipRegalia { entity_id: 1 });
    assert!(s.grid.entities[0].regalia.is_none(), "overlay cleared");
    assert_eq!(matk_flat(&s), base_matk, "stat reverted");
}

#[test]
fn rare_candidate_accrues_grants_and_expires() {
    // Stage-1 pipeline (server-roll drops): a `server_bp` entry accrues a PENDING
    // candidate on kill (never client-rolls the item), a server GrantDrop banks it
    // exactly once, and an unresolved dud expires by TTL.
    use crate::data::GameData;
    use crate::items::{DropEntry, Item, ItemKind, Rarity};
    let data = GameData::from_json_str(include_str!("../../../../data/game_data.json"))
        .expect("game_data.json must parse");
    let mut s = fixture();
    // A dying monster with ONE server-roll entry for the chomb card. STAGE 1 is a flat 1%
    // gate now (drops::CANDIDATE_RATE_BP), so the ticket does not come on the first kill —
    // keep killing until it does, then assert what it carries.
    let card = Item::new("card_chomb", ItemKind::Card, 0, 0, Rarity::Rare);
    let mut e = crate::grid::Entity::idle(900, crate::grid::Cell::new(1, 1))
        .with_team(crate::grid::Team::Monster);
    e.drops = vec![DropEntry { template: card, weight: 0, server_bp: 4_000, overrides: Vec::new(), lines_by_grade: Vec::new() }];
    s.grid.spawn(e);
    let idx = s.grid.idx_of_id(900).unwrap();
    let mut kills = 0;
    while s.rare_candidates.is_empty() && kills < 5_000 {
        crate::drops::drop_on_death(&mut s, idx);
        kills += 1;
    }
    assert_eq!(s.rare_candidates.len(), 1, "a candidate accrued within {kills} kills");
    assert!(kills > 1, "stage 1 is a 1% gate, not per-item — it must not fire every kill");
    assert_eq!(s.rare_candidates[0].seq, 1);
    assert_eq!(s.rare_candidates[0].def_id, "card_chomb");
    assert_eq!(
        s.rare_candidates[0].hit_ppm, crate::drops::stage2_hit_ppm(4_000),
        "the ticket carries its own stage-2 odds — the server has no other way to know them",
    );
    let seq1 = s.rare_candidates[0].seq;
    assert_eq!(seq1, 1);
    assert!(
        !s.card_album.iter().any(|i| i.def_id == "card_chomb")
            && !s.inventory.iter().any(|i| i.def_id == "card_chomb"),
        "candidate is PENDING — nothing banked client-side"
    );
    // Server grant (HIT): banks the card + consumes the candidate.
    apply_input_with(&mut s, Some(&data),
        InputEvent::GrantDrop { candidate_seq: 1, server_roll: 7 });
    assert!(s.rare_candidates.is_empty(), "candidate consumed");
    let banked = s
        .card_album
        .iter()
        .chain(s.inventory.iter())
        .chain(s.rare_bag.iter())
        .any(|i| i.def_id == "card_chomb");
    assert!(banked, "card banked somewhere (album/inventory/rare bag)");
    // Duplicate grant (same seq) = no-op — seq never reuses.
    let before = s.card_album.len() + s.inventory.len();
    apply_input_with(&mut s, Some(&data),
        InputEvent::GrantDrop { candidate_seq: 1, server_roll: 7 });
    assert_eq!(s.card_album.len() + s.inventory.len(), before, "no double-grant");
    // TTL expiry: an old unresolved candidate is pruned by the tick upkeep path.
    s.rare_candidates.push(crate::state::RareCandidate {
        seq: 2, def_id: "card_chomb".into(), tick: 0,
        kind: crate::state::CandidateKind::Drop, hit_ppm: 1_000,
    });
    // A same-age CRAFT candidate sits beside it and must SURVIVE — it is a paid-for item,
    // not a dud, so the TTL that prunes lottery tickets must not delete it.
    s.rare_candidates.push(crate::state::RareCandidate {
        seq: 3, def_id: "sword_s3".into(), tick: 0,
        kind: crate::state::CandidateKind::Craft, hit_ppm: 0,
    });
    s.tick = crate::drops::RARE_CANDIDATE_TTL_TICKS + 1;
    crate::tick::upkeep::expire_rare_candidates(&mut s);
    assert_eq!(s.rare_candidates.len(), 1, "dud expired, craft survived");
    assert_eq!(s.rare_candidates[0].seq, 3);
}

/// PRUNE drops the duds and nothing else.
#[test]
fn pruning_clears_judged_drop_tickets_but_never_a_paid_one() {
    use crate::data::GameData;
    use crate::state::{CandidateKind, RareCandidate};
    let data = GameData::from_json_str(include_str!("../../../../data/game_data.json"))
        .expect("game_data.json must parse");
    let mut s = fixture();
    let mut add = |seq: u64, kind: CandidateKind| {
        s.rare_candidates.push(RareCandidate {
            seq, def_id: "card_chomb".into(), tick: 0, kind, hit_ppm: 1_000,
        });
    };
    add(1, CandidateKind::Drop);
    add(2, CandidateKind::Craft);
    add(3, CandidateKind::Gear { boss: false });
    add(4, CandidateKind::Drop);
    add(9, CandidateKind::Drop); // accrued AFTER the server replayed — not judged yet

    apply_input_with(&mut s, Some(&data), InputEvent::PruneCandidates { up_to_seq: 4 });

    let left: Vec<(u64, bool)> =
        s.rare_candidates.iter().map(|c| (c.seq, c.kind.is_drop())).collect();
    assert_eq!(
        left, vec![(2, false), (3, false), (9, true)],
        "judged DUDS go; a craft and a gear ticket are PAID FOR and must never be pruned; \
         a ticket the server has not seen yet stays",
    );
}

/// ORDER IS LOAD-BEARING. The save response carries the winners AND how far the server judged,
/// and the client must apply the grants FIRST. This pins both halves: grant-then-prune keeps
/// the item, prune-then-grant destroys it. If the client ever reorders those two blocks, this
/// is the test that explains why the loot vanished.
#[test]
fn a_prune_before_its_grant_would_destroy_the_win() {
    use crate::data::GameData;
    use crate::state::{CandidateKind, RareCandidate};
    let data = GameData::from_json_str(include_str!("../../../../data/game_data.json"))
        .expect("game_data.json must parse");
    let winner = |s: &mut crate::state::GameState| {
        s.rare_candidates.push(RareCandidate {
            seq: 1, def_id: "card_chomb".into(), tick: 0,
            kind: CandidateKind::Drop, hit_ppm: 1_000,
        });
    };
    let banked = |s: &crate::state::GameState| {
        s.card_album.iter().chain(s.inventory.iter()).chain(s.rare_bag.iter())
            .filter(|i| i.def_id == "card_chomb").count()
    };

    // The order the client uses.
    let mut good = fixture();
    winner(&mut good);
    apply_input_with(&mut good, Some(&data),
        InputEvent::GrantDrop { candidate_seq: 1, server_roll: 7 });
    apply_input_with(&mut good, Some(&data), InputEvent::PruneCandidates { up_to_seq: 1 });
    assert_eq!(banked(&good), 1, "grant then prune — the win is banked");

    // The order a careless refactor would produce.
    let mut bad = fixture();
    winner(&mut bad);
    apply_input_with(&mut bad, Some(&data), InputEvent::PruneCandidates { up_to_seq: 1 });
    apply_input_with(&mut bad, Some(&data),
        InputEvent::GrantDrop { candidate_seq: 1, server_roll: 7 });
    assert_eq!(banked(&bad), 0, "prune then grant — the ticket was gone before the grant ran");
}

/// D1 end to end: a gear drop is the AUTHORED piece and banks at the moment of death. The
/// stage-3 server-valuation detour is gone because ★/lines/runes are no longer rolled at all —
/// the only draw left on this path is the weighted table pick.
#[test]
fn a_gear_drop_banks_immediately_and_is_never_rolled() {
    use crate::items::{DropEntry, Item, ItemKind, Rarity};
    let mut s = fixture();
    let gear = Item::new("sword_s3", ItemKind::Weapon, 5, 0, Rarity::Common);
    let mut e = crate::grid::Entity::idle(901, crate::grid::Cell::new(2, 2))
        .with_team(crate::grid::Team::Monster);
    e.drops = vec![DropEntry { template: gear, weight: 100, server_bp: 0, overrides: Vec::new(), lines_by_grade: Vec::new() }];
    s.grid.spawn(e);
    let idx = s.grid.idx_of_id(901).unwrap();

    let rng_before = s.rng.clone();
    crate::drops::drop_on_death(&mut s, idx);
    // D1 (0.0.369): gear is FIXED-STAT, so no ticket is raised and the piece banks immediately —
    // the server has nothing left to value. The weighted table pick is still the client's, and
    // it is still the ONLY draw this path takes.
    assert_eq!(crate::drops::gear_candidate_count(&s), 0, "no ticket: nothing to roll");
    let w = s.inventory.iter().chain(s.rare_bag.iter())
        .find(|i| i.def_id == "sword_s3").expect("banked at the moment of death");
    assert_eq!(w.star, 0, "★ is not rolled any more");
    assert!(w.bonuses.is_empty(), "no rolled lines either");
    assert_eq!(w.rarity, Rarity::Common, "rarity is the def's");
    assert_ne!(s.rng, rng_before, "the table roll still happens client-side");
}

/// The kind guard, all three ways. Each grant resolves only its OWN kind, so a nonce
/// legitimately issued for one cannot be replayed against a forged history where that seq
/// holds another kind — the cross-kind swap the separate nonce domains also block.
#[test]
fn each_grant_resolves_only_its_own_kind() {
    use crate::data::GameData;
    use crate::state::{CandidateKind, RareCandidate};
    let data = GameData::from_json_str(include_str!("../../../../data/game_data.json"))
        .expect("game_data.json must parse");
    let kinds = [
        ("drop", CandidateKind::Drop),
        ("craft", CandidateKind::Craft),
        ("gear", CandidateKind::Gear { boss: false }),
    ];
    for (name, kind) in kinds {
        for wrong in [
            InputEvent::GrantDrop { candidate_seq: 1, server_roll: 42 },
            InputEvent::GrantCraft { candidate_seq: 1, server_roll: 42 },
            InputEvent::GrantGear { candidate_seq: 1, server_roll: 42 },
        ] {
            let matching = matches!(
                (&kind, &wrong),
                (CandidateKind::Drop, InputEvent::GrantDrop { .. })
                    | (CandidateKind::Craft, InputEvent::GrantCraft { .. })
                    | (CandidateKind::Gear { .. }, InputEvent::GrantGear { .. })
            );
            let mut s = fixture();
            s.rare_candidates.push(RareCandidate {
                seq: 1, def_id: "sword_s3".into(), tick: 0, kind, hit_ppm: 1_000,
            });
            apply_input_with(&mut s, Some(&data), wrong.clone());
            let consumed = s.rare_candidates.is_empty();
            assert_eq!(
                consumed, matching,
                "{name} candidate vs {wrong:?}: consumed={consumed}, should be {matching}",
            );
        }
    }
}

/// A FULL inventory rejects a rare-bag claim (the item stays in the bag) —
/// except a stackable that merges into an existing stack (no new slot).
#[test]
fn rare_claim_respects_inventory_cap() {
    let mut s = GameState::new(1, 8, 8);
    let cap = s.inventory_cap();
    for i in 0..cap {
        s.inventory.push(Item::new(&format!("junk_{i}"), ItemKind::Weapon, 1, 0, Rarity::Common));
    }
    s.rare_bag.push(Item::new("rare_sword", ItemKind::Weapon, 9, 0, Rarity::Mythical));

    apply_input(&mut s, InputEvent::MoveRareToInventory { rare_idx: 0 });
    assert_eq!(s.rare_bag.len(), 1, "full inventory — the rare stays in the bag");
    assert_eq!(s.inventory.len(), cap, "no overflow slot created");

    // A stackable that MERGES still fits: seed a matching stack, claim a stackable rare.
    let mut mat = Item::new("some_mat", ItemKind::Material, 0, 0, Rarity::Common);
    mat.quantity = 2;
    s.inventory[0] = mat.clone();
    let mut rare_mat = mat.clone();
    rare_mat.quantity = 3;
    s.rare_bag.push(rare_mat);
    apply_input(&mut s, InputEvent::MoveRareToInventory { rare_idx: 1 });
    assert_eq!(s.inventory[0].quantity, 5, "stackable merged into the existing stack");
    assert_eq!(s.rare_bag.len(), 1, "merged rare left the bag");

    // Free one slot — the non-stackable rare now claims normally.
    s.inventory.pop();
    apply_input(&mut s, InputEvent::MoveRareToInventory { rare_idx: 0 });
    assert!(s.rare_bag.is_empty(), "rare claimed once a slot is free");
    assert!(s.inventory.iter().any(|i| i.def_id == "rare_sword"));
}

/// Card star upgrades (gacha-style v3): +1 star per fed BASE (0★) copy —
/// a starred card feeds from the def's 0★ stack, so it can ALWAYS keep
/// upgrading while base copies exist. Clamps to the 5★ cap.
#[test]
fn card_star_upgrade_consumes_dupes_and_scales() {
    use crate::items::StatMod;
    let mut s = GameState::new(1, 8, 8);
    let mut card = Item::new("card_test", ItemKind::Card, 0, 0, Rarity::Mythical);
    card.quantity = 6;
    card.card_mods = vec![StatMod::AtkFlat(6)];
    card.card_star_step = vec![StatMod::AtkFlat(1)];
    s.card_album.push(card);

    // Feed 2 base copies → a 2★ card; base stack keeps the rest.
    apply_input(&mut s, InputEvent::UpgradeCard { album_idx: 0, copies: 2 });
    assert_eq!(s.card_album.len(), 2);
    assert_eq!(s.card_album[0].quantity, 3, "6 − 2 fed − 1 promoted out");
    assert_eq!(s.card_album[1].star, 2);

    // Star ladder: the AUTHORED +1 ATK per star (0.0.243), resolved off the card itself.
    let atk_of = |it: &Item| -> i32 {
        it.active_card_mods()
            .iter()
            .map(|m| if let StatMod::AtkFlat(v) = *m { v } else { 0 })
            .sum()
    };
    assert_eq!(atk_of(&s.card_album[1]), 8, "2★ = 6 + 2 steps");
    let mut at5 = s.card_album[1].clone();
    at5.star = 5;
    assert_eq!(atk_of(&at5), 11);
    at5.star = 0;
    assert_eq!(atk_of(&at5), 6);

    // THE FIX ("การ์ด >1 ดาว อัพเกรดไม่ได้"): the 2★ card feeds from the 0★
    // stack — ask 99, clamps to min(cap 3 left, 3 base copies) = 5★.
    apply_input(&mut s, InputEvent::UpgradeCard { album_idx: 1, copies: 99 });
    assert_eq!(s.card_album.len(), 1, "base emptied; only the 5★ remains");
    assert_eq!(s.card_album[0].star, 5);
    assert_eq!(s.card_album[0].quantity, 1);

    // Capped: no further upgrade.
    apply_input(&mut s, InputEvent::UpgradeCard { album_idx: 0, copies: 1 });
    assert_eq!(s.card_album[0].star, 5, "5★ cap holds");
}

/// Card FEED (0.0.216, picker re-spec): star gain = 1 + the feeder's star
/// (copies conserved), capped at 5★; a self-feed keeps one survivor.
#[test]
fn feed_card_transfers_stars_and_clamps_at_cap() {
    let mut s = GameState::new(1, 8, 8);
    let mut base = Item::new("card_test", ItemKind::Card, 0, 0, Rarity::Mythical);
    base.quantity = 3;
    s.card_album.push(base);
    let mut starred = Item::new("card_test", ItemKind::Card, 0, 0, Rarity::Mythical);
    starred.star = 2;
    s.card_album.push(starred);

    // Self-feed the 0★ stack: one copy consumed, one promoted to 1★ (merges
    // nowhere — new row).
    apply_input(&mut s, InputEvent::FeedCard { target_idx: 0, feeder_idx: 0 });
    let one_star = s.card_album.iter().find(|c| c.star == 1).expect("1★ row exists");
    assert_eq!(one_star.quantity, 1);
    assert_eq!(s.card_album.iter().find(|c| c.star == 0).unwrap().quantity, 1);

    // Feed the 2★ INTO the 1★: gain = 1 + 2 = 3 → 4★. Feeder row vanishes.
    let t = s.card_album.iter().position(|c| c.star == 1).unwrap() as u32;
    let f = s.card_album.iter().position(|c| c.star == 2).unwrap() as u32;
    apply_input(&mut s, InputEvent::FeedCard { target_idx: t, feeder_idx: f });
    assert!(s.card_album.iter().any(|c| c.star == 4), "1★ + (1+2★) = 4★");
    assert!(s.card_album.iter().all(|c| c.star != 2), "starred feeder consumed");

    // Cap: the last 0★ tops the 4★ up to 5★; once capped (and quantity 1), a
    // further self-feed is a no-op.
    let t4 = s.card_album.iter().position(|c| c.star == 4).unwrap() as u32;
    let f0 = s.card_album.iter().position(|c| c.star == 0).unwrap() as u32;
    apply_input(&mut s, InputEvent::FeedCard { target_idx: t4, feeder_idx: f0 });
    assert!(s.card_album.iter().any(|c| c.star == 5));
    let before = s.card_album.clone();
    let t5 = s.card_album.iter().position(|c| c.star == 5).unwrap() as u32;
    apply_input(&mut s, InputEvent::FeedCard { target_idx: t5, feeder_idx: t5 });
    assert_eq!(s.card_album, before, "5★ self-feed with no foil change is a no-op");
}

/// Card FEED never panics on out-of-range stars (version-skew / tampered save):
/// apply_input MUST be a silent no-op, not a u8 overflow panic (replay-survival).
#[test]
fn feed_card_survives_out_of_range_stars() {
    let mut s = GameState::new(1, 8, 8);
    // A future/tampered save: star past the cap on the target, u8-max on the feeder.
    let mut target = Item::new("card_test", ItemKind::Card, 0, 0, Rarity::Mythical);
    target.star = 9;
    s.card_album.push(target);
    let mut feeder = Item::new("card_test", ItemKind::Card, 0, 0, Rarity::Mythical);
    feeder.star = u8::MAX;
    feeder.quantity = 2;
    s.card_album.push(feeder);
    // No panic (overflow-checks are on in every profile); nothing is created.
    apply_input(&mut s, InputEvent::FeedCard { target_idx: 0, feeder_idx: 1 });
    apply_input(&mut s, InputEvent::FeedCard { target_idx: 1, feeder_idx: 1 });
    // A capped/over-capped target grants no stars — the promoted copy never
    // exceeds what was fed (no dupe out of the underflow).
    assert!(s.card_album.iter().all(|c| c.star >= crate::items::CARD_STAR_MAX),
        "no copy dropped below the cap; the over-range target stays over-range");
}

/// Card FEED foil rules: a foil feeder converts a plain target (star + foil
/// together); a plain feeder into a foil target keeps the foil; a foil into a
/// 5★ PLAIN target is a pure conversion (legal, no star gain); a foil into a
/// 5★ FOIL target is a no-op.
#[test]
fn feed_card_foil_transfer_and_conversion() {
    let mut s = GameState::new(1, 8, 8);
    let mut plain = Item::new("card_test", ItemKind::Card, 0, 0, Rarity::Mythical);
    plain.quantity = 2;
    s.card_album.push(plain);
    let mut foil = Item::new("card_test", ItemKind::Card, 0, 0, Rarity::Mythical);
    foil.foil = true;
    foil.quantity = 2;
    s.card_album.push(foil);

    // Foil feeder → plain target: +1★ AND the target turns foil.
    apply_input(&mut s, InputEvent::FeedCard { target_idx: 0, feeder_idx: 1 });
    let converted = s.card_album.iter().find(|c| c.star == 1).expect("promoted row");
    assert!(converted.foil, "foil bit transferred with the star");
    assert_eq!(s.card_album.iter().find(|c| c.star == 0 && !c.foil).unwrap().quantity, 1);

    // Plain feeder → foil target: foil kept, +1★.
    let t = s.card_album.iter().position(|c| c.star == 1 && c.foil).unwrap() as u32;
    let f = s.card_album.iter().position(|c| c.star == 0 && !c.foil).unwrap() as u32;
    apply_input(&mut s, InputEvent::FeedCard { target_idx: t, feeder_idx: f });
    let up = s.card_album.iter().find(|c| c.star == 2).expect("2★ row");
    assert!(up.foil, "plain food never strips foil");

    // 5★ plain + foil feeder = pure conversion (user spec: "ไม่ได้ดาวเพิ่ม
    // แค่เปลี่ยนสถานะเป็น foil").
    let mut maxed = Item::new("card_test", ItemKind::Card, 0, 0, Rarity::Mythical);
    maxed.star = 5;
    s.card_album.push(maxed);
    let t5 = s.card_album.iter().position(|c| c.star == 5 && !c.foil).unwrap() as u32;
    let ff = s.card_album.iter().position(|c| c.star == 0 && c.foil).unwrap() as u32;
    apply_input(&mut s, InputEvent::FeedCard { target_idx: t5, feeder_idx: ff });
    let m = s.card_album.iter().find(|c| c.star == 5).expect("still one 5★");
    assert!(m.foil, "5★ plain converted to foil");
    assert!(s.card_album.iter().all(|c| !(c.star == 0 && c.foil)), "foil feeder consumed");

    // 5★ foil + foil feeder would change nothing → no-op even with copies left.
    let mut spare_foil = Item::new("card_test", ItemKind::Card, 0, 0, Rarity::Mythical);
    spare_foil.foil = true;
    s.card_album.push(spare_foil);
    let before = s.card_album.clone();
    let t5f = s.card_album.iter().position(|c| c.star == 5 && c.foil).unwrap() as u32;
    let ff2 = s.card_album.iter().position(|c| c.star == 0 && c.foil).unwrap() as u32;
    apply_input(&mut s, InputEvent::FeedCard { target_idx: t5f, feeder_idx: ff2 });
    assert_eq!(s.card_album, before, "foil into 5★ foil is a no-op");
}

/// Card FEED guards: def mismatch, non-card, bad index, and a lone survivor
/// self-feed are all silent no-ops; promotion merges onto an existing stack of
/// the resulting identity.
#[test]
fn feed_card_guards_and_merges() {
    let mut s = GameState::new(1, 8, 8);
    let mut a = Item::new("card_a", ItemKind::Card, 0, 0, Rarity::Mythical);
    a.quantity = 2;
    s.card_album.push(a);
    s.card_album.push(Item::new("card_b", ItemKind::Card, 0, 0, Rarity::Mythical));
    let before = s.card_album.clone();
    // Cross-def, lone-survivor self-feed, and bad index: all no-ops.
    apply_input(&mut s, InputEvent::FeedCard { target_idx: 0, feeder_idx: 1 });
    apply_input(&mut s, InputEvent::FeedCard { target_idx: 1, feeder_idx: 1 });
    apply_input(&mut s, InputEvent::FeedCard { target_idx: 0, feeder_idx: 9 });
    assert_eq!(s.card_album, before);

    // Merge: promoting onto an existing 1★ row stacks quantities.
    let mut one = Item::new("card_a", ItemKind::Card, 0, 0, Rarity::Mythical);
    one.star = 1;
    s.card_album.push(one);
    apply_input(&mut s, InputEvent::FeedCard { target_idx: 0, feeder_idx: 0 });
    let row = s.card_album.iter().find(|c| c.def_id == "card_a" && c.star == 1).unwrap();
    assert_eq!(row.quantity, 2, "promoted copy merged onto the existing 1★ stack");
    assert!(s.card_album.iter().all(|c| !(c.def_id == "card_a" && c.star == 0)),
        "0★ stack fully consumed (1 fed + 1 promoted)");
}

/// Card SPECIALS: an active Ashelle card turns skill heals into stacking ASPD
/// buffs (cap 3, refresh at cap), and the special fns star-scale; inactive
/// (no socket) grants nothing.
#[test]
fn card_specials_scale_and_gate_on_active_sockets() {
    use crate::items::{CardClass, EquipSlot};
    use crate::combat::card_specials;
    let mut s = fixture();
    // Socketed accessory feeding the Necklace card class.
    let mut acc = Item::new("ring", ItemKind::Armor, 0, 0, Rarity::Common);
    acc.grade = crate::items::Grade::Rare; // sockets are the grade's (0.0.388)
    acc.equip_slot = Some(EquipSlot::Necklace);
    s.grid.entities[0].equipped.push(acc);
    let mut ash = Item::new("card_ashelle", ItemKind::Card, 0, 0, Rarity::Mythical);
    ash.card_slot = Some(CardClass::Necklace);
    ash.star = 5;
    s.grid.entities[0].card_panel.push(ash);
    assert_eq!(card_specials::active_card_star(&s.grid.entities[0], "card_ashelle"), Some(5));

    // Skill heals stack the ASPD buff up to 3 instances (4.0% each at 5★).
    s.grid.entities[0].hp = 1;
    for _ in 0..4 {
        crate::tick::combat_step::apply_skill_heal(&mut s, 0, 5);
    }
    let stacks: Vec<i32> = s.grid.entities[0]
        .buffs
        .iter()
        .filter(|b| matches!(b.kind, crate::skills::BuffKind::CardAspdStack))
        .map(|b| b.magnitude)
        .collect();
    assert_eq!(stacks.len(), 3, "capped at 3 stacks");
    assert!(stacks.iter().all(|&m| m == 400), "5★ = 4.0% (400 bp) per stack");

    // Unequip the gear → no active socket → the card grants nothing.
    s.grid.entities[0].equipped.clear();
    assert_eq!(card_specials::active_card_star(&s.grid.entities[0], "card_ashelle"), None);
    assert_eq!(crate::combat::stat_mods::crit_dmg_bp(&s.grid.entities[0]), 0);
}

/// LEGACY card specials (0.0.223): channel folds star-scale and stack with the
/// gothic cards sharing them; the Greenling/Rechum defender/healer channels are
/// 0 without the card; display values match the mechanics (incl. the decimal
/// Greenling format).
#[test]
fn legacy_card_specials_scale_and_stack() {
    use crate::items::{CardClass, EquipSlot, StatMod};
    use crate::combat::{card_specials, stat_mods};
    let mut s = fixture();
    let e = &mut s.grid.entities[0];
    // Card-less baselines for the two NEW channels.
    assert_eq!(card_specials::basic_dmg_taken_reduction_bp(e), 0);
    assert_eq!(card_specials::heal_skill_bonus_bp(e), 0);

    // One socketed host per card class used below.
    for slot in [EquipSlot::Armor, EquipSlot::Weapon, EquipSlot::Necklace, EquipSlot::Shoes] {
        let mut host = Item::new("host", ItemKind::Armor, 0, 0, Rarity::Common);
        host.grade = crate::items::Grade::Rare; // sockets are the grade's (0.0.388)
        host.equip_slot = Some(slot);
        e.equipped.push(host);
    }
    // Baselines WITH the hosts on and no cards yet — everything below is asserted as a delta.
    let e0 = &s.grid.entities[0];
    let (race0, size0) = (stat_mods::race_of(e0), stat_mods::size_of(e0));
    let base_fire = stat_mods::dmg_vs_bp(e0, &[crate::grid::Element::Fire], race0, size0);
    let base_water = stat_mods::dmg_vs_bp(e0, &[crate::grid::Element::Ice], race0, size0);
    let base_crit = stat_mods::crit_dmg_bp(e0);
    // 0.0.243: the DMG-vs-element and CRIT-DMG channels are AUTHORED mods now, so the test
    // cards carry the same lines game_data gives them (base + per-star step).
    let mut put = |id: &str, class: CardClass, star: u8, mods: Vec<StatMod>, step: Vec<StatMod>| {
        let mut c = Item::new(id, ItemKind::Card, 0, 0, Rarity::Mythical);
        c.card_slot = Some(class);
        c.star = star;
        c.card_mods = mods;
        c.card_star_step = step;
        s.grid.entities[0].card_panel.push(c);
    };
    put(
        "card_pinkling", CardClass::Armor, 0,
        vec![StatMod::DmgVsElement(crate::grid::Element::Fire, 1_000)],
        vec![StatMod::DmgVsElement(crate::grid::Element::Fire, 200)],
    );
    put(
        "card_chomb", CardClass::Weapon, 5,
        vec![StatMod::CritDmgBp(1_000)], vec![StatMod::CritDmgBp(200)],
    );
    put("card_burst_chom", CardClass::Necklace, 5, vec![], vec![]);
    put("card_rechum", CardClass::Shoes, 0, vec![], vec![]);

    // MEASURE THE CARD'S CONTRIBUTION, not the total. Since 0.0.376 worn gear carries AUTHORED
    // lines of its own (def × grade, and a utility piece carries one even at Common), so the
    // socketed HOSTS this fixture equips bring stats of their own. The cards cannot simply be
    // measured against zero, and the hosts cannot be removed either — an un-socketed character
    // has no ACTIVE cards at all.
    let e = &s.grid.entities[0];
    // Pinkling 0★: +10% vs Fire, nothing vs others — through the SAME dmg-vs fold as gear.
    let (race, size) = (stat_mods::race_of(e), stat_mods::size_of(e));
    assert_eq!(stat_mods::dmg_vs_bp(e, &[crate::grid::Element::Fire], race, size) - base_fire, 1_000);
    assert_eq!(stat_mods::dmg_vs_bp(e, &[crate::grid::Element::Ice], race, size) - base_water, 0);
    // Chomb 5★ joins the crit-damage stat at +20% (1000 base + 5 × 200).
    assert_eq!(stat_mods::crit_dmg_bp(e) - base_crit, 2_000);
    // Burst Chom 5★: +20% on Fire-element attacks.
    assert_eq!(card_specials::dmg_with_atk_element_bp(e, crate::grid::Element::Fire), 2_000);
    // Rechum 0★: +25% on the Heal skill's output.
    assert_eq!(card_specials::heal_skill_bonus_bp(e), 2_500);

    // Greenling (defender side): swap the panel over, 3★ = 250+150 = 400 bp.
    s.grid.entities[0].card_panel.clear();
    let mut green = Item::new("card_greenling", ItemKind::Card, 0, 0, Rarity::Mythical);
    green.card_slot = Some(CardClass::Armor);
    green.star = 3;
    s.grid.entities[0].card_panel.push(green);
    assert_eq!(card_specials::basic_dmg_taken_reduction_bp(&s.grid.entities[0]), 400);

    // Display values track the mechanics (Greenling shows decimals off-step).
    assert_eq!(card_specials::display_values("card_greenling", 0), vec!["2.5"]);
    assert_eq!(card_specials::display_values("card_greenling", 3), vec!["4"]);
    assert_eq!(card_specials::display_values("card_greenling", 5), vec!["5"]);
    assert_eq!(card_specials::display_values("card_rechum", 5), vec!["50"]);
    assert_eq!(card_specials::display_values("card_toxic_chom", 5), vec!["10"]);
    assert_eq!(card_specials::display_values("card_yellowling", 0), vec!["5"]);
    // 0.0.243: a card whose effect became an authored `StatMod` has NO special line any
    // more — its numbers ride `card_lines` (resolved by the bridge), so a leftover display
    // value here would double-print it in the tooltip.
    assert!(card_specials::display_values("card_chomb", 5).is_empty());
    assert!(card_specials::display_values("card_pinkling", 5).is_empty());
    assert!(card_specials::display_values("card_scarlette", 5).is_empty());
}

// --- bound vendor guards + measurement counters (0.0.227) ---

#[test]
fn vendoring_is_retired_nothing_sells_bound_or_not() {
    let mut s = GameState::new(1, 8, 8);
    let mut junk = Item::new("scrap", ItemKind::Junk, 0, 5, Rarity::Common);
    junk.quantity = 4;
    s.inventory.push(junk);
    let gear = Item::new("sw", ItemKind::Weapon, 3, 50, Rarity::Common);
    s.inventory.push(gear);
    assert_eq!(pending_junk_gil(&s), 0);
    apply_input(&mut s, InputEvent::SellAllJunk);
    apply_input(&mut s, InputEvent::SellInventoryItem { inv_idx: 1 });
    assert_eq!(s.gil(), 0, "0.0.411: a sale is a no-op");
    assert_eq!(s.inventory.len(), 2, "nothing removed");
    assert_eq!(s.measures.items_sold.get(), 0);
}

#[test]
fn measures_count_only_successful_actions() {
    let mut s = GameState::new(1, 8, 8);
    assert!(s.measures.is_empty());
    s.set_gil_for_test(100_000);
    // Listing counts (unbound stack).
    let mut herb = Item::new("red_herb", ItemKind::Material, 0, 0, Rarity::Common);
    herb.quantity = 5;
    s.inventory.push(herb);
    // The Gil market is closed (0.0.490): a listing input is a no-op and counts nothing.
    apply_input(&mut s, InputEvent::DepositCommodity { def_id: "red_herb".into(), qty: 5, unit_price: 3 });
    assert_eq!(s.measures.items_listed.get(), 0);
    // An invalid feed (bad indices) never counts.
    apply_input(&mut s, InputEvent::FeedCard { target_idx: 9, feeder_idx: 9 });
    assert_eq!(s.measures.cards_fed.get(), 0);
}

/// The prune is a SERVER event and it only removes what it names — the two properties that stop
/// it from becoming a way to erase a listing the server never took.
#[test]
fn prune_market_ops_is_server_only_and_removes_exactly_what_it_names() {
    use crate::state::MarketOp;
    assert_eq!(
        InputEvent::PruneMarketOps { ids: vec![1] }.authority(),
        InputAuthority::Server,
        "a client must never author its own prune",
    );

    let d = GameData::from_json_str(include_str!("../../../../data/game_data.json")).unwrap();
    let mut st = crate::data::new_game_in_zone(1, &d, "zone_b1_1").unwrap();
    for id in 0..4u64 {
        st.market_ops.push(MarketOp::Deposit { id, def_id: "herb".into(), qty: 1, unit_price: 10 });
    }

    apply_input_with(&mut st, Some(&d), InputEvent::PruneMarketOps { ids: vec![1, 2] });
    let left: Vec<u64> = st.market_ops.iter().map(|o| o.id()).collect();
    assert_eq!(left, vec![0, 3], "settled ops leave, unsettled ones stay");

    // Idempotent: replaying the same prune (a growing-log re-save) is a no-op, not an error.
    apply_input_with(&mut st, Some(&d), InputEvent::PruneMarketOps { ids: vec![1, 2] });
    assert_eq!(st.market_ops.len(), 2);

    // An empty prune touches nothing — the common case, and it must not be a clear-all.
    apply_input_with(&mut st, Some(&d), InputEvent::PruneMarketOps { ids: vec![] });
    assert_eq!(st.market_ops.len(), 2, "an empty id list is NOT 'prune everything'");
}

/// `quantity` is a u32 and `overflow-checks` is ON in release, so an empty card stack is not a
/// harmless `0 - 1` wrap — it is a panic, and under `panic = "abort"` in verify-server that is
/// the process. Both card handlers debit a copy from TWO rows; the only stock guard either had
/// was `feed_card`'s self-feed case, which is skipped whenever the feeder is a different row.
/// Found by an audit sweep, reproduced before the guard existed.
#[test]
fn feeding_from_an_empty_card_stack_is_a_no_op_not_an_underflow() {
    use crate::items::{Item, ItemKind, Rarity};
    let mut st = GameState::new(1, 8, 8);
    let mut full = Item::new("card_pinkling", ItemKind::Card, 0, 0, Rarity::Common);
    full.quantity = 1;
    let mut empty = Item::new("card_pinkling", ItemKind::Card, 0, 0, Rarity::Common);
    empty.star = 1;
    empty.quantity = 0;
    st.card_album.push(full);
    st.card_album.push(empty);

    // Empty row as the FEEDER, then as the TARGET.
    crate::events::apply_input(&mut st, InputEvent::FeedCard { target_idx: 0, feeder_idx: 1 });
    crate::events::apply_input(&mut st, InputEvent::FeedCard { target_idx: 1, feeder_idx: 0 });

    assert_eq!(st.card_album.len(), 2, "neither row was consumed");
    assert_eq!(st.card_album[0].quantity, 1, "the real stack is untouched");
    assert_eq!(st.card_album[1].quantity, 0);
}

/// Twin of the above for the legacy `UpgradeCard` path: `spare`/`gain` bound the FEEDER's stock,
/// so only the target's separate `-= 1` was exposed.
#[test]
fn upgrading_an_empty_card_stack_is_a_no_op_not_an_underflow() {
    use crate::items::{Item, ItemKind, Rarity};
    let mut st = GameState::new(1, 8, 8);
    let mut base = Item::new("card_pinkling", ItemKind::Card, 0, 0, Rarity::Common);
    base.quantity = 4;
    let mut empty = Item::new("card_pinkling", ItemKind::Card, 0, 0, Rarity::Common);
    empty.star = 1;
    empty.quantity = 0;
    st.card_album.push(base);
    st.card_album.push(empty);

    crate::events::apply_input(&mut st, InputEvent::UpgradeCard { album_idx: 1, copies: 1 });

    assert_eq!(st.card_album[0].quantity, 4, "no copies were spent");
    assert_eq!(st.card_album[1].quantity, 0);
}

/// THE BAND GATE (0.0.295) HAS A TEST NOW. It is the check that stops a level-1 party typing 20
/// and standing in the endgame, and until this it was covered only by a test that asserts
/// `SelectBand` is client-authored — which is the reason the gate has to exist, not proof that it
/// works. `InputAuthority::Client` means replay-verify will not catch an over-reach: the server
/// runs this same code and agrees with whatever it accepts, so the refusal has to happen HERE.
#[test]
fn select_band_refuses_anything_past_the_frontier() {
    use crate::data::GameData;
    let d = GameData::from_json_str(include_str!("../../../../data/game_data.json"))
        .expect("game_data.json must parse");
    let mut s = crate::data::new_game_in_zone(1, &d, "zone_b1_1").unwrap();
    s.band_reached = 0; // a brand-new party: nothing reached, standing in band 1

    // The frontier is the band after the highest CLEARED one (audit 2026-10-02). Nothing cleared:
    // band 2 is refused, and a chain of single steps can no longer walk anywhere.
    apply_input_with(&mut s, Some(&d), InputEvent::SelectBand { band: 2 });
    assert_eq!(s.current_zone_id.as_deref(), Some("zone_b1_1"), "band 1 not cleared — band 2 refused");
    for b in 2..=24 {
        apply_input_with(&mut s, Some(&d), InputEvent::SelectBand { band: b });
        apply_input_with(&mut s, Some(&d), InputEvent::EnterInstance { start_zone_id: format!("zone_b{b}_1") });
    }
    assert_eq!(s.current_zone_id.as_deref(), Some("zone_b1_1"), "neither input walks past the frontier");
    assert!(s.band_reached <= 1, "and band_reached does not move");

    // Band 1's boss killed → band 2 opens, band 3 does not.
    s.record_band_clear(1);
    apply_input_with(&mut s, Some(&d), InputEvent::SelectBand { band: 3 });
    assert_eq!(s.current_zone_id.as_deref(), Some("zone_b1_1"), "two past the clear — refused");
    apply_input_with(&mut s, Some(&d), InputEvent::SelectBand { band: 2 });
    assert_eq!(s.current_zone_id.as_deref(), Some("zone_b2_1"), "the frontier");
    apply_input_with(&mut s, Some(&d), InputEvent::SelectBand { band: 20 });
    assert_eq!(s.current_zone_id.as_deref(), Some("zone_b2_1"), "the endgame is refused");

    // A refusal is a SILENT no-op, never a panic: a save containing one must still replay.
    apply_input_with(&mut s, Some(&d), InputEvent::SelectBand { band: 0 });
    assert_eq!(s.current_zone_id.as_deref(), Some("zone_b2_1"), "band 0 is refused");

    // Going BACK is always allowed — a reached band stays selectable.
    apply_input_with(&mut s, Some(&d), InputEvent::SelectBand { band: 1 });
    assert_eq!(s.current_zone_id.as_deref(), Some("zone_b1_1"), "a reached band is still open");
    apply_input_with(&mut s, Some(&d), InputEvent::SelectBand { band: 2 });
    assert_eq!(s.current_zone_id.as_deref(), Some("zone_b2_1"), "and so is the one reached after it");
}

#[test]
fn retiring_weapon_familiarity_did_not_speed_anyone_up() {
    // 0.0.366 folded each unit's STARTING-weapon `aspd_mods` entry into its base
    // before deleting the familiarity table, and 0.0.367 converted that base
    // through the RO curve into a plain attacks/sec rate. Both conversions were
    // meant to be behaviour-neutral, and these are the numbers the ORIGINAL
    // (familiarity + curve) model produced: seraphine 1.06 atk/s, lapis 0.92.
    // If a future edit makes a unit faster than this, it is a buff — decide it,
    // don't inherit it.
    let d = crate::data::GameData::from_json_str(include_str!("../../../../data/game_data.json"))
        .expect("game_data.json must parse");
    for (id, want_milli) in [("seraphine", 106), ("lapis", 92)] {
        let u = d.unit(id).unwrap();
        assert_eq!(u.atk_speed_milli, want_milli, "{id} base attack rate");
        let e = crate::data::build_player_entity(&d, u, 1, Cell::new(0, 0));
        let bare = crate::combat::effective_atk_speed_milli(&e);
        // …and holding ANY weapon type gives exactly that same rate now.
        for wt in [
            crate::items::WeaponType::Dagger,
            crate::items::WeaponType::Sword1H,
            crate::items::WeaponType::Sword2H,
            crate::items::WeaponType::Bow,
            crate::items::WeaponType::Staff,
            crate::items::WeaponType::Mace,
        ] {
            let mut holder = e.clone();
            let mut w = crate::items::Item::new("w", crate::items::ItemKind::Weapon, 10, 0, crate::items::Rarity::Common);
            w.weapon_type = wt;
            holder.equipped = vec![w];
            assert_eq!(
                crate::combat::effective_atk_speed_milli(&holder), bare,
                "{id} must swing the same with {wt:?}",
            );
        }
        assert_eq!(bare, want_milli, "{id} attack rate (milli/100 = atk/s)");
    }
}

/// Sealed Cache (0.0.449): opening takes one box, counts one open, and nothing else moves —
/// the contents are the server's. The box binds when it lands; a non-box index is a no-op.
#[test]
fn opening_a_lootbox_counts_and_consumes_and_never_rolls() {
    let d = crate::data::GameData::from_json_str(include_str!("../../../../data/game_data.json")).unwrap();
    let mut s = crate::data::new_game_in_zone(1, &d, "zone_b1_1").unwrap();
    let mut boxes = crate::data::instantiate_item(d.item(crate::drops::LOOTBOX_ID).expect("lootbox"));
    boxes.quantity = 2;
    crate::drops::bank_loot_drop(&mut s, boxes);
    let idx = s.inventory.iter().position(|i| i.def_id == crate::drops::LOOTBOX_ID).expect("banked");
    assert!(s.inventory[idx].bound, "a cache binds on arrival");
    let rng = s.rng.clone();
    let gil = s.gil();
    apply_input(&mut s, InputEvent::OpenLootbox { inv_idx: idx as u32 });
    assert_eq!(s.measures.lootboxes_opened.get(), 1);
    assert_eq!(s.inventory[idx].quantity, 1);
    apply_input(&mut s, InputEvent::OpenLootbox { inv_idx: idx as u32 });
    assert_eq!(s.measures.lootboxes_opened.get(), 2);
    assert!(!s.inventory.iter().any(|i| i.def_id == crate::drops::LOOTBOX_ID), "the stack is gone");
    assert_eq!(s.rng, rng, "no draw — the contents are not the sim's to roll");
    assert_eq!(s.gil(), gil);
    apply_input(&mut s, InputEvent::OpenLootbox { inv_idx: 0 });
    assert_eq!(s.measures.lootboxes_opened.get(), 2, "not a cache: no-op");
}

/// MEMORY DUST DROPS (0.0.490): odds grow with the monster's stage and deep stages pay larger
/// tiers — ordinary kills Dust (+ Radiant past 80), bosses Radiant (+ Crystal past 120) — so the
/// first zone is always the worst place to farm.
#[test]
fn memory_dust_drops_scale_with_stage() {
    let run = |stage: u16, boss: bool, kills: u32| {
        let mut s = fixture();
        let mut e = crate::grid::Entity::idle(900, crate::grid::Cell::new(1, 1)).with_team(crate::grid::Team::Monster);
        e.is_boss = boss;
        e.mob_stage = stage;
        s.grid.spawn(e);
        let idx = s.grid.idx_of_id(900).unwrap();
        for _ in 0..kills {
            crate::drops::drop_on_death(&mut s, idx);
        }
        (s.material_count("regalia_fodder"), s.material_count("regalia_sr_fodder"), s.material_count("regalia_crystal_fodder"))
    };
    let early = run(1, false, 100_000);
    assert!((250..=370).contains(&early.0) && early.1 == 0 && early.2 == 0, "stage 1: ~0.31% Dust only: {early:?}");
    let deep = run(168, false, 100_000);
    assert!((1_850..=2_110).contains(&deep.0), "stage 168: ~1.98% Dust: {deep:?}");
    assert!((70..=130).contains(&deep.1) && deep.2 == 0, "stage 168: ~0.1% Radiant, no Crystal: {deep:?}");
    let boss = run(168, true, 20_000);
    assert!(boss.0 == 0 && (380..=500).contains(&boss.1), "boss 168: ~2.18% Radiant: {boss:?}");
    assert!((40..=80).contains(&boss.2), "boss 168: ~0.3% Crystal: {boss:?}");
    let xp = |t: (u32, u32, u32)| t.0 * 1_000 + t.1 * 10_000 + t.2 * 100_000;
    assert!(xp(deep) > xp(early) * 5, "a deep kill is worth far more than a first-zone kill");
}

/// THE GIL MARKET IS CLOSED AT THE SOURCE (0.0.490): none of the four trade inputs touches the
/// state — no op in `market_ops`, no Gil held, no item or stack removed.
#[test]
fn gil_market_trade_inputs_are_tombstones() {
    let mut s = GameState::new(1, 8, 8);
    s.set_gil_for_test(100_000);
    let mut herb = Item::new("red_herb", ItemKind::Material, 0, 0, Rarity::Common);
    herb.quantity = 5;
    s.inventory.push(herb);
    s.inventory.push(Item::new("sword_s1", ItemKind::Weapon, 10, 0, Rarity::Common));
    let before = (s.gil(), s.inventory.clone());
    for ev in [
        InputEvent::DepositCommodity { def_id: "red_herb".into(), qty: 5, unit_price: 3 },
        InputEvent::BuyCommodity { def_id: "red_herb".into(), qty: 1, max_price: 10 },
        InputEvent::DepositItem { inv_idx: 1, price: 500 },
        InputEvent::BuyItemListing { listing_id: 1, max_price: crate::state::Gil::new(100) },
    ] {
        apply_input(&mut s, ev);
    }
    assert!(s.market_ops.is_empty(), "no trade op can reach the server");
    assert_eq!((s.gil(), s.inventory.clone()), before, "nothing held, nothing removed");
}

/// AUDIT 2026-10-03: a long party name is cut BEFORE it is recorded, so it never rides the save log.
#[test]
fn a_long_party_name_is_cut_before_it_is_recorded() {
    let mut s = crate::state::GameState::new(1, 20, 10);
    s.record_inputs = true;
    apply_input(&mut s, InputEvent::SetPartyName { name: "x".repeat(10_000) });
    match s.input_log.last().map(|r| &r.event) {
        Some(InputEvent::SetPartyName { name }) => assert_eq!(name.chars().count(), PARTY_NAME_MAX_CHARS),
        other => panic!("not recorded: {other:?}"),
    }
}

/// A dungeon opens with progression: its first room's stage must be within the party's reach.
#[test]
fn a_dungeon_opens_only_once_its_stage_is_within_reach() {
    let d = crate::data::GameData::from_json_str(include_str!("../../../../data/game_data.json")).unwrap();
    let mut s = crate::data::new_game_in_zone(1, &d, "zone_b1_1").unwrap();
    apply_input_with(&mut s, Some(&d), InputEvent::EnterInstance { start_zone_id: "zone_dungeon_duskmourn_1".into() });
    assert_eq!(s.current_zone_id.as_deref(), Some("zone_b1_1"), "a new party cannot walk into the stage-111 dungeon");
    s.band_reached = 111;
    apply_input_with(&mut s, Some(&d), InputEvent::EnterInstance { start_zone_id: "zone_dungeon_duskmourn_1".into() });
    assert_eq!(s.current_zone_id.as_deref(), Some("zone_dungeon_duskmourn_1"), "reached: it opens");
}
