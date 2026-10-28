//! build::items — item instantiation, save-migration, shop, loot, drop tables.

use super::*;
use crate::state::GilSpend;
/// Build an `Item` instance (Common grade, no earned state) from a definition.
pub fn instantiate_item(def: &ItemDef) -> Item {
    Item {
        def_id: def.id.clone(),
        kind: def.kind,
        name: def.name.clone(),
        atk_bonus: def.atk_bonus,
        atk_speed_bp: def.atk_speed_bp,
        weapon_type: def.weapon_type,
        offhand_type: def.offhand_type,
        weapon_level: def.weapon_level,
        def_bonus: def.def_bonus,
        resonance_stars: 0,
        enhance_level: 0,
        enhance_xp: 0,
        equipped_by: 0,
        rarity: def.rarity,
        // A gem def carries its grade (display colour, fusion checks); everything else starts at
        // the floor and EARNS a grade.
        grade: def.gem.as_ref().map(|g| g.grade).unwrap_or_default(),
        stat_override: None,
        ledger: def.ledger,
        quantity: 1,
        sell_value: def.sell_value,
        consumable: def.consumable,
        marketable: def.marketable,
        emission_group: def.emission_group.clone(),
        quota_class: def.quota_class.clone(),
        instance_id: None,
        lines: Vec::new(),
        equip_slot: def.equip_slot,
        card_slots: 0,
        card_slot: def.card_slot,
        card_mods: def.card_mods.clone(),
        card_star_step: def.card_star_step.clone(),
        card_star_ranks: def.card_star_ranks.clone(),
        foil_mods: def.foil_mods.clone(),
        foil_star_step: def.foil_star_step.clone(),
        element: def.element,
        bind_mods: def.bind_mods.clone(),
        star: 0,
        foil: def.foil,
        bound: false,
        bonuses: Vec::new(),
        unique: def.unique,
        steam: def.steam_itemdef.is_some(),
        orb: def.orb.clone(),
        locked_slot: None,
        locked: false,
        gems: Vec::new(),
        gem_set: def.gem.as_ref().map(|g| g.set.clone()).unwrap_or_default(),
    }
}

/// Migrate a CARD sitting in a character's panel.
///
/// `migrate_item` assigns `card_slot` from the def, and for a card that is EQUIPPED that field is
/// per-instance ROUTING — `equip_card` stamps it with the destination so an accessory card knows
/// whether it is in Necklace or Ring. Refreshing it from the def would silently move the
/// card. Exactly the hazard `equip_slot` is excluded for, one field along.
fn migrate_panel_card(item: &mut crate::items::Item, data: &GameData) {
    let routed = item.card_slot;
    migrate_item(item, data);
    item.card_slot = routed;
}

/// Refresh one item's DEF-DERIVED fields from its current `ItemDef`, preserving
/// the PER-INSTANCE state (refine, quantity, rolls, marks). This is how a balance
/// change (nerf/buff to atk/def/etc. in game_data) propagates to items already
/// sitting in a save — without resetting how upgraded or how many the player has.
/// No-op if the item's `def_id` is no longer in the catalog.
///
/// This ASSIGNS the def fields onto the live item. It must NOT rebuild the item from
/// the def and copy earned state back: that shape silently DESTROYS every per-instance
/// field someone forgets to add to the copy-back list, and it did — `resonance_stars` +
/// `equipped_by` (0.0.221 Exemplar marks, so every load unequipped your Exemplar and
/// reset its Resonance), `refine_attempts` (the server-roll nonce's monotonic component,
/// so reloading rewound the refine nonce) and `instance_id` (the market identity stamped
/// at first refine) were all being wiped on EVERY load. Adding a per-instance field to
/// `Item` must not require editing this function — only a new DEF-derived field does.
/// RENAMED def id PREFIXES (old save spelling → current), applied BEFORE the def lookup so an old
/// item still resolves instead of dangling as an unknown def.
///
/// 0.0.375 renamed four gear families along with the slots they live in (design §2.5) — a "mask"
/// worn on the hands and a "charm" worn on the legs were names describing what the slot used to
/// be. `ore_primocon → ore_valkirion` USED to sit here and was removed in the same pass: refine
/// took both ores with it (0.0.370), so the entry pointed at a def nobody authored any more —
/// a remap whose target does not exist is worse than no remap, because it turns a recognisable
/// stale item into an unrecognisable one. `the_remap_table_points_at_real_defs` pins that.
pub const DEF_ID_REMAP: [(&str, &str); 10] = [
    // Nemu's SSR Regalia renamed for its art (0.0.487).
    ("regalia_crimson_mirror", "regalia_lycoris_lullaby"),
    ("mask_", "gloves_"),
    ("garment_", "earring_"),
    ("pendant_", "necklace_"),
    // The two card-system test defs (0.0.379): a save that held one keeps the s1 piece.
    ("demo_socket_sword", "sword_s1"),
    ("demo_socket_armor", "armor_s1"),
    // Phase-0 leftovers (0.0.379): four unbadged pieces from before the tier ladder, referenced by
    // nothing, that the tier-1 fallback had been dealing at 250 ATK / 700 DEF against s1's
    // 140 / 156. A save holding one keeps the s1 piece of that slot.
    ("sword_short", "sword_s1"),
    ("sword_long", "sword_s1"),
    ("leather_armor", "armor_s1"),
    ("chain_armor", "armor_s1"),
];

pub fn migrate_item(item: &mut Item, data: &GameData) {
    // RENAMED def ids (old save id → current id) — remap BEFORE the def lookup so
    // an old stack still resolves + keeps its behaviour (else it'd dangle as an
    // unknown def and, for ore, stop being recognised as refine material).
    for (old, new) in DEF_ID_REMAP {
        if let Some(rest) = item.def_id.strip_prefix(old) {
            item.def_id = format!("{new}{rest}");
            break;
        }
    }
    // The SLOTTED variants (`<base>_1s` / `_2s`, 0.0.130-0.0.387) are gone: sockets are the
    // grade's (0.0.388). A save holding one keeps the base piece at its grade — same stats, and
    // the sockets it has now are what every copy of that grade has.
    if data.item(&item.def_id).is_none() {
        if let Some(base) = item.def_id.strip_suffix("_1s").or_else(|| item.def_id.strip_suffix("_2s")) {
            if data.item(base).is_some() {
                item.def_id = base.to_string();
            }
        }
    }
    let Some(def) = data.item(&item.def_id) else { return; };
    // DEF-derived only. NOT assigned here (per-instance, earned): refine_level,
    // refine_attempts, quantity, resonance_stars, enhance_level, equipped_by, instance_id, star, foil,
    // enhance_xp, broken, bonuses, locked_slot, locked — and `equip_slot`, which is per-instance
    // ROUTING state (main `Weapon` vs off-hand `WeaponLeft`) stamped at equip time: reset
    // it and a dual-wield off-hand reverts to the def's slot, vanishing from the paper-doll.
    item.kind = def.kind;
    item.name = def.name.clone();
    item.atk_bonus = def.atk_bonus;
    item.atk_speed_bp = def.atk_speed_bp;
    item.weapon_type = def.weapon_type;
    item.offhand_type = def.offhand_type;
    item.weapon_level = def.weapon_level;
    item.def_bonus = def.def_bonus;
    item.rarity = def.rarity;
    item.sell_value = def.sell_value;
    item.consumable = def.consumable;
    item.marketable = def.marketable;
    item.emission_group = def.emission_group.clone();
    item.quota_class = def.quota_class.clone();
    item.card_slots = 0; // retired 0.0.388 — sockets are the grade's
    item.card_slot = def.card_slot;
    item.card_mods = def.card_mods.clone();
    item.card_star_step = def.card_star_step.clone();
    item.card_star_ranks = def.card_star_ranks.clone();
    item.foil_mods = def.foil_mods.clone();
    item.foil_star_step = def.foil_star_step.clone();
    item.element = def.element;
    item.bind_mods = def.bind_mods.clone();
    item.unique = def.unique;
    item.steam = def.steam_itemdef.is_some();
    item.orb = def.orb.clone();
    // Gear carries NO rarity since 0.0.380 (grade is its one axis) — the def's `rarity` is the
    // serde default now, and the line above already wrote it. And since 0.0.382 no ★, no rolled
    // line and no lock either: a piece an old save enchanted comes back as the authored piece
    // at its grade. Cards keep their star — that is the card's level, a different thing.
    if matches!(item.kind, crate::items::ItemKind::Weapon | crate::items::ItemKind::Armor) {
        item.star = 0;
        item.bonuses.clear();
        item.locked_slot = None;
    }
    // The override and the SUBSTAT lines are DEF-derived (catalog rows × this item's grade), so
    // they refresh on load like every other def field — re-tuning either never needs a
    // migration. After the def fields above: the lines read the slot and the badge.
    let grade = item.grade;
    let lines = crate::substats::lines_by_grade(&data.loot_rules.substats, &item.def_id, item.slot());
    item.assign_grade(grade, &data.gear_overrides, &lines);
    // A gem def's grade and set are def fields too; a gemmed piece keeps its gems (they are the
    // owner's, rolled once) but never more than the sockets it has NOW.
    if let Some(spec) = &def.gem {
        item.grade = spec.grade;
        item.gem_set = spec.set.clone();
    }
    crate::gems::trim_to_sockets(item);
    // Dual wield is gone (0.0.383): a weapon that carried the off-hand stamp (`WeaponLeft`, or a
    // dagger stamped `Shield`) reverts to its natural slot; the per-entity de-dup below then
    // evicts it to the bag if the main hand is taken.
    if item.weapon_type != crate::items::WeaponType::None
        && matches!(item.equip_slot, Some(crate::items::EquipSlot::WeaponLeft) | Some(crate::items::EquipSlot::Shield))
    {
        item.equip_slot = None;
    }
}

/// Migrate EVERY item held in `state` to the current catalog (see `migrate_item`).
/// Walks the stash, rare bag, escrow, every entity's equipped + consumable items,
/// and parked roster loadouts. Called on load so a save always
/// reflects the latest item balance.
pub fn migrate_save_items(state: &mut GameState, data: &GameData) {
    // GIL RESCALE (0.0.412): a pre-rescale balance is lifted ×100 exactly once, before anything
    // below prices an item in the new unit.
    state.migrate_gil_scale();
    // PARTY SLOTS (0.0.418): what stands in the field is owned — a save from before slots keeps
    // its whole party, bench or not.
    state.claim_fielded_slots();
    migrate_unit_ids(state);
    migrate_exemplar_carriers(state, data);
    migrate_materials_to_ledger(state, data);
    migrate_element_sigils(state);
    migrate_retired_regalia(state);
    remove_retired_items(state);
    migrate_gems_to_bag(state, data);
    // TRAINING (0.0.406): re-resolve the bought levels against THIS catalog, so a re-tuned
    // `per_level` reaches an old save. Empty levels resolve to nothing → byte-identical.
    crate::training::resolve(state, data);
    // 0.0.431: crest levels saved under a slot's PRE-RENAME id (`relic_w3` for `crest_w3`) move
    // to the slot's current id — an exact lookup per catalog slot, never a shape match.
    for slot in &data.crests.slots {
        let Some(tail) = slot.id.strip_prefix("crest_") else { continue };
        let old_id = format!("relic_{tail}");
        if let Some(v) = state.crests.remove(&old_id) {
            let e = state.crests.entry(slot.id.clone()).or_insert(0);
            *e = (*e).max(v);
        }
    }
    crate::crest::resolve(state, data);
    // The Pants slot is retired (0.0.389): a piece stamped for it has no def and no slot to go
    // to, so it leaves the save here — bag, stash and worn alike — rather than dangling as an
    // unknown def the client cannot draw.
    let legs = |it: &Item| matches!(it.equip_slot, Some(crate::items::EquipSlot::Pants));
    state.inventory.retain(|it| !legs(it));
    state.stash.retain(|it| !legs(it));
    for e in state.grid.entities.iter_mut() {
        e.equipped.retain(|it| !legs(it));
    }
    for it in state.inventory.iter_mut() {
        migrate_item(it, data);
    }
    // The rare bag is retired (0.0.380): whatever a save still holds there comes home to the
    // inventory, past the cap if it must — losing a player's rare piece to a cap is not an
    // option, and the bag only ever held three.
    for mut it in std::mem::take(&mut state.rare_bag) {
        migrate_item(&mut it, data);
        state.inventory.push(it);
    }
    for it in state.escrow.iter_mut() {
        migrate_item(it, data);
    }
    for it in state.card_album.iter_mut() {
        migrate_item(it, data);
    }
    for it in state.stash.iter_mut() {
        migrate_item(it, data);
    }
    split_steam_stacks(&mut state.inventory);
    split_steam_stacks(&mut state.stash);
    for it in state.owned_exemplars.iter_mut() {
        migrate_item(it, data);
    }
    // NOTE (0.0.169): the old "sweep cards out of the inventory into the album" migration
    // was removed — cards LIVE in the inventory now (dropped there; deposited to the album
    // by hand), so pulling them out on load would fight the model. Pre-album saves that
    // still had cards in the bag are long gone.
    for e in state.grid.entities.iter_mut() {
        crate::potential::base_kit::ensure_entity(e);
        // Clamp any entity carried over a data change to its current max — a
        // MONSTER serialized before a stat re-tune (e.g. the band cascade) keeps
        // its old hp, which can now exceed its new max and read as "hp > max" on
        // the HP bar. Monsters are transient, but clamp on load so a stale save
        // never shows it. (Players are re-clamped in the class block below.)
        if matches!(e.team, crate::grid::Team::Monster) {
            e.hp = e.hp.min(crate::combat::effective_max_hp(e));
        }
        for it in e.equipped.iter_mut() {
            migrate_item(it, data);
        }
        // One item per paper-doll slot. Keep the FIRST item per slot; evict the rest to the
        // bag with their slot stamp cleared. This is also how a pre-0.0.383 dual-wielder's
        // second dagger comes off (`migrate_item` just cleared its off-hand stamp, so it now
        // collides with the main hand). Harmless no-op on healthy saves.
        let mut seen: Vec<crate::items::EquipSlot> = Vec::new();
        let mut kept: Vec<crate::items::Item> = Vec::new();
        for it in std::mem::take(&mut e.equipped) {
            let slot = it.slot();
            if seen.contains(&slot) {
                let mut evicted = it;
                evicted.equip_slot = None;
                state.inventory.push(evicted);
            } else {
                seen.push(slot);
                kept.push(it);
            }
        }
        e.equipped = kept;
        for slot in e.consumables.iter_mut() {
            migrate_item(&mut slot.item, data);
        }
        // The RESERVED slots too. They were missed, so a provision sitting in one froze at
        // whatever spec was serialized: retune a meal's magnitude and every copy already slotted
        // keeps the old number forever, while the same stack in the bag is corrected on load.
        for slot in e.provisions.iter_mut() {
            migrate_item(&mut slot.item, data);
        }
        // EQUIPPED CARDS. Combat reads the panel instance's own `card_mods` (via
        // `active_card_mods`) and never consults the catalog, so an unmigrated panel keeps
        // applying pre-patch numbers forever — while the same card in the album is corrected.
        for it in e.card_panel.iter_mut() {
            migrate_panel_card(it, data);
        }
        // Refresh the class-derived HP/SP CURVE from the current class def so a
        // balance change (e.g. the RO HP-curve port) propagates to existing saves —
        // an old entity serialized with hp_factor=0 would otherwise stay on the
        // legacy linear HP. Keeps stats/level/skills/gear; just re-reads the curve.
        if let Some(class) = data.unit(&e.unit_def) {
            e.max_hp = class.hp; // level-1 HP base
            e.hp_per_level = class.hp_per_level;
            e.hp_factor = class.hp_factor;
            e.hp_increase = class.hp_increase;
            // Base attack RATE is re-read too (persisted entities must re-take
            // authoritative template stats every load, 0.0.367).
            e.atk_speed_milli = class.atk_speed_milli;
            // Clamp current HP to the (possibly new) maximum.
            e.hp = e.hp.min(crate::combat::effective_max_hp(e));
        }
        // BOUND-CARD mods are DERIVED (0.0.245): rebuild them from `bound_card_counts` ×
        // the def's current `bind_mods`, exactly as `bind_card` accumulated them (the Nth
        // bind adds another copy). So a re-tuned bind reaches every existing save, and a
        // pre-0.0.245 save — whose `bound_card_bonus` blob no longer deserializes — is
        // restored in full from the counts it still carries. Deterministic: `BTreeMap`
        // iterates in key order, and the verify server runs this same pass before replay.
        e.bound_card_mods.clear();
        for (def_id, count) in e.bound_card_counts.iter() {
            let Some(def) = data.item(def_id) else { continue };
            for _ in 0..*count {
                e.bound_card_mods.extend_from_slice(&def.bind_mods);
            }
        }
    }
    // THE PARKED CHARACTERS. `roster` was never walked at all once, so a benched character's
    // gear kept its pre-patch numbers and `deploy` put them straight back on the board. Measured,
    // not inferred: a probe read `atk=12345` on a roster item after a migrate that corrected the
    // identical item in the bag.
    for rc in state.roster.values_mut() {
        crate::potential::base_kit::ensure_parked(&rc.unit_def, &mut rc.data);
        for it in rc.data.equipped.iter_mut() {
            migrate_item(it, data);
        }
        for it in rc.data.card_panel.iter_mut() {
            migrate_panel_card(it, data);
        }
        for slot in rc.data.consumables.iter_mut().chain(rc.data.provisions.iter_mut()) {
            migrate_item(&mut slot.item, data);
        }
    }
    // The party rack, and the REGALIA collection — the declared twin of `owned_exemplars`, which
    // was migrated while it was not.
    for slot in state.party_provisions.iter_mut() {
        migrate_item(&mut slot.item, data);
    }
    for it in state.owned_regalia.iter_mut() {
        migrate_item(it, data);
    }
}

/// Buy `qty` of shop item `def_id` for Gil (the shop is the Gil sink + the
/// consumable supply). Bridge-level (needs `GameData` to resolve the def + its
/// `buy_price`). Deducts Gil and banks the stack into the stash. Errors (state
/// untouched): unknown id, item not for sale (`buy_price == 0`), or not enough Gil.
/// Does the Gil shop sell `def`? The ONE rule `buy_item` enforces and the host's shop list shows.
/// A price is required, and some kinds are refused whatever the price says: units, Memory
/// Fragments AND Memory Dust come from the gacha, mileage, drops, salvage and server grants
/// (0.0.489-490 — every one still carried the 6c8ab845 "temporary test price", so 20 units, 24
/// cards and the Dust were a BuyItem away; user: "memory dust ไม่ได้ซื้อได้นะ"), and cards while
/// `Features::cards` holds them back (0.0.490).
pub fn sells_for_gil(data: &GameData, def: &crate::data::ItemDef) -> bool {
    use crate::items::ItemKind;
    def.buy_price > 0
        && !matches!(def.kind, ItemKind::Exemplar | ItemKind::Regalia)
        && (data.features.cards || !matches!(def.kind, ItemKind::Card))
        && (data.features.craft || !is_craft_only_material(data, def))
}

/// A material whose only use is a recipe ingredient — worthless while craft is held back.
fn is_craft_only_material(data: &GameData, def: &crate::data::ItemDef) -> bool {
    matches!(def.kind, crate::items::ItemKind::Material)
        && data.recipes.iter().any(|r| r.ingredients.iter().any(|i| i.def_id == def.id))
}

pub fn buy_item(state: &mut GameState, data: &GameData, def_id: &str, qty: u32) -> Result<(), String> {
    if qty == 0 {
        return Ok(());
    }
    let def = data.item(def_id).ok_or_else(|| format!("unknown item: {def_id}"))?;
    if !sells_for_gil(data, def) {
        return Err(format!("{def_id} is not sold for Gil"));
    }
    let cost = def.buy_price as u64 * qty as u64;
    if state.gil() < cost {
        return Err(format!("not enough Gil: need {cost}, have {}", state.gil()));
    }
    // ROOM CHECK before charging (bug fix): a full inventory used to take the Gil
    // and lose the item to overflow. A stackable merges into its existing stack
    // (0 new slots) or opens ONE new stack; non-stackable gear needs `qty` slots;
    // a card lands in the album (merge-by-type or a free album slot).
    let sample = instantiate_item(def);
    // EXEMPLARS bank into the dedicated `owned_exemplars` collection (uncapped, never
    // lost) — no room check, and each copy is its own carrier (Resonance feeds dupes).
    if matches!(sample.kind, crate::items::ItemKind::Exemplar | crate::items::ItemKind::Regalia) {
        state.spend_gil(cost, GilSpend::Shop); // gil >= cost already checked above
        for _ in 0..qty {
            bank_collectible(state, data, instantiate_item(def));
        }
        return Ok(());
    }
    let has_room = if matches!(sample.kind, crate::items::ItemKind::Card) {
        state.card_album.iter().any(|i| i.def_id == def_id)
            || state.card_album.len() < state.card_album_cap()
    } else if sample.is_stackable() {
        // `can_bank`, not a def-id match (audit 2026-10-02): a BOUND stack of the same def does
        // not take an unbound purchase, so a full bag charged the Gil and lost the item.
        can_bank(state, &sample)
    } else {
        state.inventory.len() + qty as usize <= state.inventory_cap()
    };
    if !has_room {
        return Err("inventory full".to_string());
    }
    state.spend_gil(cost, GilSpend::Shop);
    // Bank one unit at a time: stackable consumables merge into a single stack,
    // non-stackable gear each take their own slot.
    for _ in 0..qty {
        // Purchases go STRAIGHT to the stash (never the rare-drop bag).
        crate::drops::bank_purchase_to_inventory(state, instantiate_item(def));
    }
    Ok(())
}

/// Grant a 3-hour-loop LOOT item by def id straight into the player inventory —
/// RARE/Legendary route to the rare bag, the rest to the stash. Returns false on
/// an unknown id. (The WHICH-item roll is the client's for now; this just makes
/// the granted item REAL in the deterministic state.)
pub fn grant_loot(state: &mut GameState, data: &GameData, def_id: &str) -> bool {
    match data.item(def_id) {
        Some(def) => {
            crate::drops::bank_loot_drop(state, instantiate_item(def));
            true
        }
        None => false,
    }
}

/// Like `grant_loot` but stamps `bound` on the granted item. The craft path binds
/// its output when a bound ingredient was consumed (bound-first + bound output
/// closes the laundering channel where a free pass material's value would leak into
/// a tradeable craft; audit H1, docs/economy-invariants.md).
pub fn grant_loot_bound(state: &mut GameState, data: &GameData, def_id: &str, bound: bool) -> bool {
    match data.item(def_id) {
        Some(def) => {
            let mut item = instantiate_item(def);
            item.bound = bound;
            crate::drops::bank_loot_drop(state, item);
            true
        }
        None => false,
    }
}

/// Grant `qty` of `def_id` to the inventory ONLY IF it fits — a stackable merges into an
/// existing stack (no new slot), else it needs a free slot. Returns whether it was granted;
/// if a NEW slot is needed and the inventory is full it does NOTHING and returns false (NEVER
/// the overflow-lost path). The mailbox claim uses this so a full inventory BLOCKS the claim
/// instead of losing the item (docs/l1-commodity-market-scope.md Slice 2b full-inventory guard).
pub fn try_grant(state: &mut GameState, data: &GameData, def_id: &str, qty: u32) -> bool {
    let Some(def) = data.item(def_id) else {
        return false; // unknown def (data skew) → don't consume the mail
    };
    let mut item = instantiate_item(def);
    item.quantity = qty.max(1);
    // COLLECTIBLES first, exactly as `try_grant_item` does. An Exemplar or Regalia def arriving as
    // a `def_id`×`qty` payload would otherwise be pushed into `inventory`, breaking the "never in
    // inventory/stash/market" rule its own home enforces.
    if bank_collectible(state, data, item.clone()) {
        return true;
    }
    // THROUGH `can_bank`, not a hand-rolled predicate. This function carried its own copy that
    // matched on `def_id` ALONE while the real merge in `bank_to_inventory_inner` also requires
    // `bound` to agree (and `same_card_stack` for cards) — so on a mismatch it predicted a merge,
    // skipped the cap check, the merge was refused, and the item fell through to
    // `overflow_recover` and was auto-sold or lost, while this returned `true` and the caller
    // consumed the mail. `try_grant_item` was fixed in 0.0.299; this sibling, 38 lines away and
    // serving the SAME caller (`claim_mail` splits full items from def_id payloads between them),
    // was not.
    if !can_bank(state, &item) {
        return false; // needs a new slot but the inventory is full → mail stays claimable
    }
    // Verified it fits → bank it (merge or new slot; can't reach the overflow-lost branch).
    crate::drops::bank_purchase_to_inventory(state, item);
    true
}

/// Grant a specific full ITEM INSTANCE (an equipment piece delivered by the item market) ONLY IF
/// it fits — preserves the exact instance (stats / refine / stars), unlike `try_grant` (which
/// rebuilds a fresh item from a def_id). A non-stackable piece always needs a free slot; returns
/// false (banks NOTHING → the mail stays claimable) when the inventory is full.
pub fn try_grant_item(state: &mut GameState, data: &GameData, mut item: crate::items::Item) -> bool {
    // Collectibles bypass the bag entirely — uncapped, so this never fails.
    if bank_collectible(state, data, item.clone()) {
        return true;
    }
    // A mailed item may predate the `steam` stamp (0.0.480). One with a Steam identity lands as
    // single copies, all or nothing.
    if let Some(def) = data.item(&item.def_id) {
        item.steam = def.steam_itemdef.is_some();
    }
    if item.steam && item.quantity > 1 {
        let n = item.quantity as usize;
        if state.inventory.len() + n > state.inventory_cap() {
            return false;
        }
        item.quantity = 1;
        for _ in 0..n {
            crate::drops::bank_purchase_to_inventory(state, item.clone());
        }
        return true;
    }
    try_grant_bag_item(state, item)
}

/// STEAM ITEMS ARE SINGLES (0.0.480): a stack saved before `Item.steam` existed splits into one
/// copy per slot, past the cap if it must — a load migration cannot lose a player's items.
fn split_steam_stacks(items: &mut Vec<crate::items::Item>) {
    let mut out = Vec::with_capacity(items.len());
    for mut it in std::mem::take(items) {
        if it.steam && it.quantity > 1 {
            let n = it.quantity;
            it.quantity = 1;
            for _ in 1..n {
                out.push(it.clone());
            }
        }
        out.push(it);
    }
    *items = out;
}

/// The bag half of `try_grant_item`, for a caller that KNOWS the item is not a collectible and
/// has no catalog in hand (the provisions swap). A collectible handed here is refused, not
/// banked — an Exemplar envelope must never reach the bag by a side door.
pub fn try_grant_bag_item(state: &mut GameState, item: crate::items::Item) -> bool {
    if matches!(item.kind, crate::items::ItemKind::Exemplar | crate::items::ItemKind::Regalia) {
        return false;
    }
    if !can_bank(state, &item) {
        return false;
    }
    crate::drops::bank_purchase_to_inventory(state, item);
    true
}

/// Would `bank_purchase_to_inventory` actually take this item? Pure — asks, does not act.
///
/// ⚠️ THIS MUST MIRROR `drops::bank_purchase_to_inventory`'s merge rule EXACTLY. It used to
/// predict a merge from `def_id` alone while the real rule also requires `bound` to match (and
/// `same_card_stack` for cards). When they disagreed the cap check was SKIPPED, the inner merge
/// then failed, the push was refused, and the item fell through to `overflow_recover` — auto-sold
/// or gone — while the caller was told `true`.
///
/// That is not theoretical: the battle pass free track mints BOUND stackables and delivers them
/// as full items in mail, so claiming one at cap while an UNBOUND stack of the same def sat in the
/// bag destroyed the reward and let the server consume the mail — the exact opposite of what
/// `claim_mail` documents.
pub fn can_bank(state: &GameState, item: &crate::items::Item) -> bool {
    // A ledger material is a count — no slot, so it always fits.
    if item.ledger {
        return true;
    }
    let merges = item.is_stackable()
        && state.inventory.iter().any(|i| {
            i.is_stackable()
                && i.bound == item.bound
                && if matches!(item.kind, crate::items::ItemKind::Card) {
                    crate::drops::same_card_stack(i, item)
                } else {
                    i.def_id == item.def_id
                }
        });
    merges || state.inventory.len() < state.inventory_cap()
}

/// PRE-LEDGER MATERIALS → LEDGER (0.0.354; Star Stones joined 0.0.364). A save from before a def
/// moved to the ledger holds it as bag stacks; the CURRENT catalog says which defs are `ledger`,
/// so every such stack (bag and rare bag) is moved into the ledger and its slot freed. Bound never
/// applied to these, so nothing is lost in the move.
/// LEDGER GEMS → BAG (0.0.448). A save from 0.0.386-0.0.447 holds gems as ledger counts; they
/// are bag stacks now, so each count becomes one stack, pushed past the cap if it must — a load
/// migration cannot lose a player's gems to a full bag. Idempotent: the ledger holds no gem
/// after the first run.
fn migrate_gems_to_bag(state: &mut GameState, data: &GameData) {
    let moved: Vec<(String, u32)> = state
        .materials()
        .filter(|(id, n)| *n > 0 && crate::gems::set_of(data, id).is_some())
        .map(|(id, n)| (id.to_string(), n))
        .collect();
    for (id, n) in moved {
        if !state.debit_material(&id, n) {
            continue;
        }
        let Some(def) = data.item(&id) else { continue };
        match state.inventory.iter_mut().find(|it| it.def_id == id && it.is_stackable() && !it.bound) {
            Some(st) => st.quantity = st.quantity.saturating_add(n),
            None => {
                let mut it = crate::data::instantiate_item(def);
                it.quantity = n;
                state.inventory.push(it);
            }
        }
    }
}

/// RETIRED SR REGALIA → RADIANT DUST (0.0.489). Each copy of a retired card pays what salvaging
/// it would have — 1 Radiant per copy (1 + Resonance stars) and every fed level back — and a
/// member (fielded or benched) wearing one loses the overlay. Idempotent: nothing retired is left
/// after the first run.
fn migrate_retired_regalia(state: &mut GameState) {
    use crate::items::{REGALIA_MAX_STARS, RETIRED_SR_REGALIA};
    let retired = |def_id: &str| def_id.strip_prefix("regalia_").is_some_and(|b| RETIRED_SR_REGALIA.contains(&b));
    let mut xp_total = 0u32;
    state.owned_regalia.retain(|it| {
        if !retired(&it.def_id) {
            return true;
        }
        // Valued as an SR card of today (rung 1): its copies plus its invested XP.
        let copies = u32::from(it.resonance_stars.min(REGALIA_MAX_STARS)) + 1;
        let xp = crate::items::REGALIA_SALVAGE_XP[1].saturating_mul(copies)
            .saturating_add(crate::items::regalia_xp_to_level(1, it.enhance_level))
            .saturating_add(it.enhance_xp);
        xp_total = xp_total.saturating_add(xp);
        false
    });
    for (id, n) in crate::items::regalia_xp_as_dust(xp_total).tiers() {
        state.credit_material(id, n, crate::state::MaterialSource::Migration);
    }
    let worn = |r: &Option<crate::items::Regalia>| r.as_ref().is_some_and(|r| RETIRED_SR_REGALIA.contains(&r.id.as_str()));
    for e in state.grid.entities.iter_mut() {
        if worn(&e.regalia) {
            e.clear_regalia_skill();
            e.regalia = None;
        }
    }
    for rc in state.roster.values_mut() {
        if worn(&rc.data.regalia) {
            rc.data.regalia = None;
        }
    }
}

/// Items taken out of the game (0.0.490, user 2026-09-29: "พวกนี้ไม่ใช้แล้วเอาออกจากเกม") — the ASPD
/// potions, the field ration, the inventory expander and the seven unique pieces. Their defs are
/// gone from the catalog, so a copy a save still holds is removed on load wherever it sits — bag,
/// stash, rare bag, a member's gear, consumable / provision slots, the party rack, a benched
/// member — rather than lingering as an item nothing can describe or use.
pub const RETIRED_ITEM_IDS: [&str; 36] = [
    "aspd_pot_1", "aspd_pot_2", "aspd_pot_3", "loot_field_ration", "inventory_expander",
    "unique_blade", "unique_greatblade", "unique_dagger", "unique_longbow", "unique_rod", "unique_mace",
    "unique_plate",
    "loot_max_potion", "loot_rejuvenating", "loot_phoenix_tail", "loot_monster_lure", "loot_monster_accelerator",
    "gacha_coupon",
    // Meals and their ingredients (user 2026-10-02: Meal cut; the EXP Elixir is the provision).
    "meal_hearty", "meal_feast", "meal_root_skewer", "meal_herb_broth", "meal_roast_haunch",
    "meal_spiced_broth", "meal_grand_roast", "meal_ember_stew",
    "daikon", "featherfern", "beast_meat", "wild_honey", "cave_garlic", "ember_root", "dew_cap",
    "night_truffle", "slime_jelly", "bitter_root",
];

/// ELEMENT-LESS SIGIL (2026-09-30): every retired `sigil_<element>_<n>` count — in the ledger, or
/// a stack still in the bag or stash — folds into the generic Sigil of its grade, so a save loses
/// none. Runs after `migrate_materials_to_ledger`, which already moved ledger-flagged bag stacks.
fn migrate_element_sigils(state: &mut GameState) {
    use crate::state::MaterialSource;
    for (old, g) in crate::potential::legacy_element_sigils() {
        let mut n = state.material_count(&old);
        if n > 0 {
            state.debit_material(&old, n);
        }
        for bag in [&mut state.inventory, &mut state.stash, &mut state.rare_bag] {
            bag.retain(|it| {
                if it.def_id == old {
                    n = n.saturating_add(it.quantity.max(1));
                    false
                } else {
                    true
                }
            });
        }
        state.credit_material(&crate::potential::sigil_def(g), n, MaterialSource::Migration);
    }
}

fn remove_retired_items(state: &mut GameState) {
    let retired = |it: &Item| RETIRED_ITEM_IDS.contains(&it.def_id.as_str());
    state.inventory.retain(|it| !retired(it));
    state.stash.retain(|it| !retired(it));
    state.rare_bag.retain(|it| !retired(it));
    // Escrow too (audit 2026-10-02): a save taken mid-run kept a retired item there and banked it
    // into the bag on the clear, still usable.
    state.escrow.retain(|it| !retired(it));
    // Slot lists shrink by design (a depleted stack is swept every tick), and a slot's buff
    // source is its stable `key`, not its position — so removing one is safe.
    for e in state.grid.entities.iter_mut() {
        e.equipped.retain(|it| !retired(it));
        e.consumables.retain(|c| !retired(&c.item));
        e.provisions.retain(|c| !retired(&c.item));
    }
    for rc in state.roster.values_mut() {
        rc.data.equipped.retain(|it| !retired(it));
        rc.data.consumables.retain(|c| !retired(&c.item));
        rc.data.provisions.retain(|c| !retired(&c.item));
    }
    state.party_provisions.retain(|p| !retired(&p.item));
}

fn migrate_materials_to_ledger(state: &mut GameState, data: &GameData) {
    let is_ledger = |it: &crate::items::Item| {
        it.ledger || data.item(&it.def_id).map(|d| d.ledger).unwrap_or(false)
    };
    let mut moved: Vec<(String, u32)> = Vec::new();
    state.inventory.retain(|it| {
        if is_ledger(it) {
            moved.push((it.def_id.clone(), it.quantity));
            false
        } else {
            true
        }
    });
    state.rare_bag.retain(|it| {
        if is_ledger(it) {
            moved.push((it.def_id.clone(), it.quantity));
            false
        } else {
            true
        }
    });
    // Memory Dust carriers (0.0.489): one `owned_regalia` entry per copy before, a count now.
    state.owned_regalia.retain(|it| {
        if is_ledger(it) {
            moved.push((it.def_id.clone(), it.quantity.max(1)));
            false
        } else {
            true
        }
    });
    for (id, n) in moved {
        state.credit_material(&id, n, crate::state::MaterialSource::Migration);
    }
}

/// LEGACY CARRIERS → UNITS (0.0.350). Before this version an Exemplar was an item in
/// `owned_exemplars`, worn by a class character. Each carrier a save still holds becomes what the
/// unit model says it is: a WORN carrier's wearer already IS that unit (it carries `exemplar:
/// Some`), so the wearer keeps the overlay and takes the carrier's stars; an unworn carrier's
/// first copy becomes a benched Lv1 unit, and every further copy — and every star a carrier had
/// been fed — becomes a Resonance credit the player spends by hand. The list is then emptied, so
/// the pass is idempotent and an already-migrated save is untouched.
/// UNIT ID RENAME (0.0.492): `rapis` → `lapis`, `nyx` → `raplus` (`items::RENAMED_UNIT_IDS`)
/// everywhere a save names a unit — the field and the bench (unit def + Exemplar overlay), the
/// zone's unit, banked Resonance credits, and every `exemplar_<id>` carrier item. A save already
/// on the new ids is untouched.
fn migrate_unit_ids(state: &mut GameState) {
    use crate::items::current_unit_id;
    fn rename(id: &mut String) {
        let cur = current_unit_id(id);
        if cur != id.as_str() {
            *id = cur.to_string();
        }
    }
    fn rename_item(it: &mut Item) {
        if let Some(bare) = it.def_id.strip_prefix("exemplar_") {
            let cur = current_unit_id(bare);
            if cur != bare {
                it.def_id = format!("exemplar_{cur}");
            }
        }
    }
    if let Some(u) = state.current_unit_def.as_mut() {
        rename(u);
    }
    for e in state.grid.entities.iter_mut() {
        rename(&mut e.unit_def);
        if let Some(x) = e.exemplar.as_mut() {
            rename(&mut x.id);
        }
        e.equipped.iter_mut().for_each(rename_item);
    }
    for rc in state.roster.values_mut() {
        rename(&mut rc.unit_def);
        if let Some(x) = rc.data.exemplar.as_mut() {
            rename(&mut x.id);
        }
        rc.data.equipped.iter_mut().for_each(rename_item);
    }
    let old = std::mem::take(&mut state.pending_resonance);
    for (id, n) in old {
        *state.pending_resonance.entry(current_unit_id(&id).to_string()).or_insert(0) += n;
    }
    for list in [&mut state.inventory, &mut state.stash, &mut state.escrow, &mut state.card_album, &mut state.rare_bag, &mut state.owned_exemplars] {
        list.iter_mut().for_each(rename_item);
    }
}

fn migrate_exemplar_carriers(state: &mut GameState, data: &GameData) {
    if state.owned_exemplars.is_empty() {
        return;
    }
    let carriers = std::mem::take(&mut state.owned_exemplars);
    for c in carriers {
        let bare = c.def_id.strip_prefix("exemplar_").unwrap_or(&c.def_id).to_string();
        let stars = c.resonance_stars.min(crate::items::EXEMPLAR_MAX_STARS) as u32;
        if c.equipped_by != 0 {
            // The wearer may be on the field OR on the bench — `capture()` moved its overlay into
            // the roster with it, and the carrier's `equipped_by` still points at it.
            let worn = if let Some(e) = state.grid.entity_of_id_mut(c.equipped_by) {
                e.exemplar.as_mut()
            } else if let Some(rc) = state.roster.get_mut(&c.equipped_by) {
                rc.data.exemplar.as_mut()
            } else {
                None
            };
            if let Some(x) = worn {
                if x.id == bare {
                    x.stars = x.stars.max(stars as u8);
                    continue;
                }
            }
        }
        // Unworn (or the wearer is gone): the grant makes a unit if the account has none and
        // banks a credit itself if it does — and the stars the carrier had been fed come along
        // as further credits either way. An unknown id makes nothing and banks nothing.
        if crate::items::exemplar_def_by_id(&bare).is_none() {
            continue;
        }
        let _ = crate::data::grant_exemplar_unit(state, data, &bare);
        if stars > 0 {
            *state.pending_resonance.entry(bare).or_insert(0) += stars;
        }
    }
}

/// COLLECTIBLE routing: Exemplars and Regalia never live in the bag, so a gacha pull or a mailed
/// reward can never be lost to a full inventory. Callers that would otherwise do a room check
/// consult this FIRST. Returns true when the item was a collectible and has been handled.
///
/// An EXEMPLAR is a UNIT (0.0.350): the item is only the delivery envelope, and it lands through
/// `grant_exemplar_unit` — the first copy becomes a benched Lv1 unit, a later copy a Resonance
/// credit the player spends by hand. The envelope is not kept. A Regalia is still a carrier and
/// banks into `owned_regalia`, each copy its own so dupes remain what its Resonance feeds on.
pub fn bank_collectible(state: &mut GameState, data: &GameData, item: crate::items::Item) -> bool {
    if data.item(&item.def_id).is_some_and(|d| d.xp_envelope) {
        crate::tick::grant_party_xp(state, u64::from(item.quantity));
        return true;
    }
    if let Some(key) = data.item(&item.def_id).and_then(|d| d.crest_material.as_ref()) {
        let shards = item.quantity.max(1).saturating_mul(data.crests.shards_per_material.max(1));
        let held = state.crest_mats.entry(key.clone()).or_insert(0);
        *held = held.saturating_add(shards);
        return true;
    }
    match item.kind {
        crate::items::ItemKind::Exemplar => {
            let bare = item.def_id.strip_prefix("exemplar_").unwrap_or(&item.def_id);
            // Unknown id: nothing to make a unit of. Report handled — the envelope was a
            // collectible either way and must not fall through into the bag.
            let _ = crate::data::grant_exemplar_unit(state, data, bare);
        }
        // Memory Dust is a ledger count (0.0.489), not a carrier.
        crate::items::ItemKind::Regalia if item.ledger || data.item(&item.def_id).is_some_and(|d| d.ledger) => {
            state.credit_material(&item.def_id, item.quantity.max(1), crate::state::MaterialSource::Mail);
        }
        crate::items::ItemKind::Regalia => state.owned_regalia.push(item),
        _ => return false,
    }
    true
}

/// Build a runtime `DropEntry` vector from a monster's drop rows.
/// Unknown item ids are filtered out — but `validate()` already
/// caught those, so in practice all rows resolve.
/// `stage` is where the monster is PLACED (0 = unplaced: a test, or a monster nobody spawned) —
/// it picks the gear slot odds row (`GearRule::slot_bp_at`), so a kill pays more the deeper the
/// stage.
pub fn build_drop_table(monster: &MonsterDef, data: &GameData, stage: u16) -> Vec<DropEntry> {
    let mut table: Vec<DropEntry> = monster
        .drops
        .iter()
        .filter_map(|row| {
            // Cards held back from the release (`Features::cards`) never enter a drop table.
            data.item(&row.item_id).filter(|def| data.features.cards || !matches!(def.kind, crate::items::ItemKind::Card)).map(|def| DropEntry {
                lines_by_grade: Vec::new(),
                template: instantiate_item(def),
                weight: row.weight,
                server_bp: row.server_bp,
                overrides: overrides_of(data, &def.id),
            })
        })
        .collect();
    // RULE rows (`GameData::loot_rules`): appended AFTER the authored rows, in catalog order, so
    // the weighted draw and the candidate draw see one stable table. Weight 0 — these are
    // server-rolled and never compete in the weighted roll.
    let boss = data.zones.iter().any(|z| z.boss_monster_id == monster.id);
    // Orb / capstone rows still name a `max_level` / `min_level`: those are STAGES now (0.0.399).
    let level = monster.stage;
    let rule_row = |item_id: &str, server_bp: u32| {
        data.item(item_id).map(|def| DropEntry { template: instantiate_item(def), weight: 0, server_bp, overrides: Vec::new(), lines_by_grade: Vec::new() })
    };
    for r in &data.loot_rules.orbs {
        if r.boss == boss && (r.max_level == 0 || level <= r.max_level) {
            // The first matching tier row wins per item: a lower tier's row shadows a higher one.
            if table.iter().any(|e| e.template.def_id == r.item_id && e.weight == 0) {
                continue;
            }
            table.extend(rule_row(&r.item_id, r.server_bp));
        }
    }
    if boss {
        for r in &data.loot_rules.capstones {
            if level >= r.min_level {
                table.extend(rule_row(&r.item_id, r.server_bp));
            }
        }
    }
    if let Some(rule) = &data.loot_rules.gear {
        if !rule.exclude.iter().any(|id| *id == monster.id) {
            // The tier follows WHERE the monster stands (0.0.399): the placed stage; an unplaced
            // one (a test) takes its own, or the lowest zone it appears in.
            let stage = if stage > 0 {
                stage
            } else if monster.stage > 0 {
                monster.stage
            } else {
                data.zones
                    .iter()
                    .filter(|z| z.spawn_pool.iter().any(|s| s.monster_id == monster.id) || z.boss_monster_id == monster.id)
                    .map(|z| z.stage)
                    .min()
                    .unwrap_or(1)
            };
            let (tier, preview) = rule.tier_at(stage);
            // The pool is the WHOLE tier, weapon families included that no unit wields yet
            // (user 2026-09-14: "ทำดรอบรวมๆไปก่อนก็ได้ ไม่ต้องสกรีนดรอบออก"). A parked family
            // drops as sellable gear rather than as nothing; screening the pool by the equip
            // gate is a one-line change here if that ever starts to feel like junk.
            //
            // Grouped by SLOT: every slot pays `slot_bp` per kill, split across its defs by their
            // relative weights (`GearRule`). The pool order is the catalog's, so the table — which
            // is baked into the save — is the same on every build.
            let (field_bp, boss_bp) = rule.slot_bp_at(stage);
            let slot_bp = u64::from(if boss { boss_bp } else { field_bp });

            // Each group is (bp per kill, its defs with relative weights): the paper-doll slots
            // first, then the GEM group (0.0.386) — one row per set at the Common def; the drop
            // path rolls the grade and re-keys the def, exactly as it grades a piece of gear.
            let mut by_slot: Vec<(u64, Vec<(&ItemDef, u64)>)> = Vec::new();
            let mut slot_of: Vec<crate::items::EquipSlot> = Vec::new();
            // THE NEXT TIER PREVIEWS (0.0.404): in the last quarter of a band the tier above joins
            // the pool at the same weight, so the party gears up for the boundary instead of
            // walling on it — every measured run stalled 12-48 h at the end of a band (stage ~37,
            // ~110), the wall being "tier N gear against a tier N+1 ladder".
            // With a TIER MIX (0.0.455) every tier with a non-zero share joins the pool and a
            // def's relative weight carries its tier's share; without one, the band's tier and
            // its preview (the old shape).
            let mix = rule.mix_at(stage);
            let tier_share = |d: &ItemDef| -> u64 {
                match mix {
                    Some(m) => crate::gear_roll::def_tier_badge(&d.id).and_then(|t| m.get(t as usize - 1)).copied().map_or(0, u64::from),
                    None => u64::from(crate::gear_roll::in_gear_pool(d, tier) || preview.is_some_and(|t| crate::gear_roll::in_gear_pool(d, t))) * 10_000,
                }
            };
            for def in data.items.iter().filter(|d| {
                matches!(d.kind, crate::items::ItemKind::Weapon | crate::items::ItemKind::Armor) && !d.unique && tier_share(d) > 0
            }) {
                let rel = u64::from(if def.weapon_type == crate::items::WeaponType::Dagger { rule.dagger_mult } else { 1 }) * tier_share(def);
                if rel == 0 || slot_bp == 0 {
                    continue;
                }
                let slot = instantiate_item(def).slot();
                match slot_of.iter().position(|s| *s == slot) {
                    Some(i) => {
                        if let Some((_, defs)) = by_slot.get_mut(i) {
                            defs.push((def, rel));
                        }
                    }
                    None => {
                        slot_of.push(slot);
                        by_slot.push((slot_bp, vec![(def, rel)]));
                    }
                }
            }
            let (gem_field, gem_boss) = data.loot_rules.gems.bp_at(stage);
            let gem_bp = u64::from(if boss { gem_boss } else { gem_field });
            if gem_bp > 0 {
                let rows: Vec<(&ItemDef, u64)> = data
                    .loot_rules
                    .gems
                    .sets
                    .iter()
                    .filter_map(|set| {
                        let def = data.item(&crate::gems::def_id(&set.id, crate::items::Grade::Common))?;
                        (set.weight > 0).then_some((def, u64::from(set.weight)))
                    })
                    .collect();
                if !rows.is_empty() {
                    by_slot.push((gem_bp, rows));
                }
            }
            // Absolute bp → a weight on THIS table: the authored rows sum to `authored`, and the
            // rule's share comes out of `nothing`, so every authored row keeps its exact odds.
            // A small authored table (a hundred-weight one) cannot express a fraction of a bp in
            // whole weights, so it is scaled up first — every authored row ×k keeps its share, and
            // the rule rows then land on theirs instead of a rounded-up floor. The finest row is
            // the lightest def of the fullest slot, measured in milli-bp.
            let smallest_mbp = by_slot
                .iter()
                .flat_map(|(bp, defs)| {
                    let sum: u64 = defs.iter().map(|(_, rel)| *rel).sum();
                    // In MICRO-bp (0.0.455): a 0.01% tier share of a slot is the finest row the
                    // table has to express.
                    defs.iter().map(move |(_, rel)| bp * 1_000_000 * *rel / sum.max(1))
                })
                .min()
                .unwrap_or(1)
                .max(1);
            let mut authored: u64 = table.iter().map(|e| e.weight as u64).sum();
            // Cap the scale so `weight` stays a u32 with room for the sum: the finest row wants
            // ~1 whole weight, the largest authored row must not overflow.
            let k = (10_000_000_000 / (smallest_mbp * authored.max(1))).clamp(1, 1_000_000);
            if k > 1 {
                let k = (k + 1) as u32;
                for e in table.iter_mut() {
                    e.weight = e.weight.saturating_mul(k);
                }
                authored = table.iter().map(|e| e.weight as u64).sum();
            }
            let mut taken: u64 = 0;
            for (bp, defs) in &by_slot {
                let sum: u64 = defs.iter().map(|(_, rel)| *rel).sum();
                for (def, rel) in defs {
                    let weight = (bp * authored * *rel / (10_000 * sum.max(1))).max(1) as u32;
                    taken += u64::from(weight);
                    let template = instantiate_item(def);
                    // Baked per grade, like the overrides: the bank rolls the grade and stamps.
                    let lines_by_grade = crate::substats::lines_by_grade(&data.loot_rules.substats, &def.id, template.slot());
                    table.push(DropEntry { template, weight, server_bp: 0, overrides: overrides_of(data, &def.id), lines_by_grade });
                }
            }
            if let Some(nothing) = table.iter_mut().find(|e| e.template.def_id == crate::items::NOTHING_DROP_ID) {
                nothing.weight = (u64::from(nothing.weight).saturating_sub(taken)) as u32;
            }
        }
    }
    table
}

/// Resolve the GRADE-odds row for a monster of `level` from the catalog (`loot_rules.gear
/// .grade_odds`), to be baked into its template. Empty when the catalog authors none — the drop
/// path then falls back to `drops::GRADE_ODDS_BP`, so a catalog that says nothing behaves exactly
/// as the code always did.
///
/// The level is mapped onto the DESIGN's 20-band scale (the launch ladder, Lv1-70) because that
/// is the scale the rows are written in; the catalog's own `zone_b<N>` numbering counts 100
/// sub-bands over the same content, and reading one as the other silently falls off the end of
/// the table at every high-level kill.
pub fn grade_odds_for(data: &GameData, stage: u16) -> Vec<u32> {
    let Some(rule) = &data.loot_rules.gear else { return Vec::new() };
    if rule.grade_odds.is_empty() {
        return Vec::new();
    }
    rule.grade_odds
        .iter()
        .find(|r| stage.max(1) <= r.max_stage)
        .or_else(|| rule.grade_odds.last())
        .map(|r| r.bp.clone())
        .unwrap_or_default()
}

/// Every `gear_overrides` row for one def — baked onto its drop template so the bank path can
/// apply the right one once the grade is rolled. Almost always empty.
fn overrides_of(data: &GameData, def_id: &str) -> Vec<crate::data::GearOverride> {
    data.gear_overrides.iter().filter(|o| o.def_id == def_id).cloned().collect()
}

#[cfg(test)]
mod tier_mix_tests {
    fn ppm(d: &crate::data::GameData, stage: u16, boss: bool, tier: u32) -> u64 {
        let z = d.zones.iter().find(|z| z.stage == stage).expect("zone");
        let id = if boss { z.boss_monster_id.as_str() } else { z.spawn_pool[0].monster_id.as_str() };
        let t = super::build_drop_table(d.monster(id).unwrap(), d, stage);
        let total: u64 = t.iter().map(|e| e.weight as u64).sum();
        let w: u64 = t.iter().filter(|e| crate::gear_roll::def_tier_badge(&e.template.def_id) == Some(tier) && matches!(e.template.kind, crate::items::ItemKind::Weapon | crate::items::ItemKind::Armor)).map(|e| e.weight as u64).sum();
        w * 1_000_000 / total.max(1)
    }
    fn gear_ppm(d: &crate::data::GameData, stage: u16) -> u64 { (1..=7).map(|t| ppm(d, stage, false, t)).sum() }

    /// 0.0.455 (user): gear per kill is the SAME at every stage; T1-T2 are always the bulk; the
    /// share of the top tiers climbs with the stage — T6 ≈ 1/week, T7 ≈ 1/month at ~15,000
    /// field kills a day.
    #[test]
    fn gear_odds_are_flat_and_only_the_tier_mix_climbs() {
        let d = crate::data::GameData::from_json_str(include_str!("../../../../../data/game_data.json")).unwrap();
        let (g60, g180, g232) = (gear_ppm(&d, 60), gear_ppm(&d, 180), gear_ppm(&d, 232));
        assert!(g180 * 10 / 8 >= g60 && g60 * 10 / 8 >= g180 * 10 / 13, "gear/kill about flat: 60 {g60} 180 {g180}");
        assert!(g232 >= g180, "and does not fall off at the end: {g232}");
        for stage in [60u16, 120, 180, 232] {
            let low = ppm(&d, stage, false, 1) + ppm(&d, stage, false, 2);
            assert!(low * 100 / gear_ppm(&d, stage) >= 75, "T1-T2 are the bulk at {stage}: {low} of {}", gear_ppm(&d, stage));
        }
        assert!(ppm(&d, 180, false, 5) > ppm(&d, 120, false, 5), "the T5 share climbs");
        let t6 = ppm(&d, 232, false, 6);
        let t7 = ppm(&d, 240, false, 7);
        assert!((4..25).contains(&t6), "T6 ~1/week: {t6} ppm");
        assert!((1..8).contains(&t7), "T7 ~1/month: {t7} ppm");
    }
}


#[cfg(test)]
mod tier_mix_print {
    #[test]
    #[ignore]
    fn print_ppm() {
        let d = crate::data::GameData::from_json_str(include_str!("../../../../../data/game_data.json")).unwrap();
        for stage in [30u16, 60, 100, 120, 150, 180, 210, 232, 240] {
            let z = d.zones.iter().find(|z| z.stage == stage).unwrap();
            for (label, id) in [("field", z.spawn_pool[0].monster_id.as_str()), ("boss", z.boss_monster_id.as_str())] {
                let t = super::build_drop_table(d.monster(id).unwrap(), &d, stage);
                let total: u64 = t.iter().map(|e| e.weight as u64).sum();
                let mut row = vec![];
                for tier in 1..=7u32 {
                    let w: u64 = t.iter().filter(|e| crate::gear_roll::def_tier_badge(&e.template.def_id) == Some(tier) && matches!(e.template.kind, crate::items::ItemKind::Weapon | crate::items::ItemKind::Armor)).map(|e| e.weight as u64).sum();
                    row.push(w * 1_000_000 / total.max(1));
                }
                eprintln!("PPM stage {stage} {label}: {row:?}");
            }
        }
    }
}
