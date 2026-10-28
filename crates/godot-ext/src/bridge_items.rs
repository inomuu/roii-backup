// SimBridge — shop / stash / consumable methods.
//
// A SECONDARY `#[godot_api]` impl block (see bridge_world.rs for why the
// methods are split across modules this way).

// `use super::*` glob-imports the crate-root items the gdext `(secondary)`
// macro needs in scope: the `SimBridge` struct, its funcs-collection struct,
// and the module-local `__registration_*` statics the primary impl declares.
// It also brings in `item_to_view` (re-imported in lib.rs).
#[allow(unused_imports)]
use super::*;
use sim_core::{data, events};

#[godot_api(secondary)]
impl SimBridge {
    /// Sell every banked junk item for Gil (the Gil source). Returns the Gil
    /// gained so the UI can flash a "+N Gil" toast.
    #[func]
    fn sell_all_junk(&mut self) -> i64 {
        let Some(s) = self.state.as_mut() else { return 0; };
        let before = s.gil();
        events::apply_input(s, events::InputEvent::SellAllJunk);
        (s.gil() - before) as i64
    }

    /// Gil the banked junk would fetch if sold now (for the Sell button
    /// label). 0 when none is held.
    /// The most a listing may ask, per unit. The UI clamps its price field to this so the sim's
    /// silent no-op on an over-cap listing is unreachable from an honest client — the two numbers
    /// used to live in different languages (99999 in a SpinBox, the real bound in Rust) with
    /// nothing binding them.
    #[func]
    fn max_listing_price(&self) -> i64 {
        sim_core::mailbox_grant::MAX_LISTING_PRICE as i64
    }

    #[func]
    fn pending_junk_gil(&self) -> i64 {
        self.state.as_ref().map_or(0, |s| events::pending_junk_gil(s) as i64)
    }

    /// Slot a Consumable stash item into a character's consumable bar (cap 3).
    #[func]
    fn add_consumable(&mut self, entity_id: i64, stash_idx: i64) {
        if let Some(s) = self.state.as_mut() {
            events::apply_input(
                s,
                events::InputEvent::AddConsumable {
                    entity_id: entity_id as u32,
                    inv_idx: stash_idx as u32,
                },
            );
        }
    }

    /// Remove a consumable from a character's bar `slot`, back to the stash.
    #[func]
    fn remove_consumable(&mut self, entity_id: i64, slot: i64) {
        if let Some(s) = self.state.as_mut() {
            events::apply_input(
                s,
                events::InputEvent::RemoveConsumable {
                    entity_id: entity_id as u32,
                    slot: slot as u32,
                },
            );
        }
    }

    /// Shop catalog as JSON: every buyable item (`buy_price > 0`) as
    /// `{def_id, name, kind, buy_price}`. The Shop view builds its buy list
    /// from this.
    #[func]
    fn shop_items_json(&self) -> GString {
        let Some(d) = self.data.as_ref() else { return "[]".into(); };
        let arr: Vec<serde_json::Value> = d
            .items
            .iter()
            .filter(|it| it.buy_price > 0)
            .map(|def| {
                // FULL item view (same fields the inventory tooltip reads:
                // rarity, atk/def/matk, the consumable spec, card slots, name) so
                // the shop tooltip describes items correctly — plus the buy price,
                // which lives on the def, not the runtime Item.
                let item = data::instantiate_item(def);
                let mut v = item_to_view(&item);
                if let Some(o) = v.as_object_mut() {
                    o.insert("buy_price".into(), serde_json::json!(def.buy_price));
                }
                v
            })
            .collect();
        serde_json::to_string(&arr).unwrap_or_default().into()
    }

    /// ONE def as a full item view, by id. The craft bench shows a recipe's OUTPUT before the
    /// player owns a copy, and a tooltip needs the same fields the inventory one reads — the
    /// consumable spec above all, which the recipe JSON does not carry.
    ///
    /// Built through `instantiate_item` like the shop and album catalogs rather than
    /// hand-assembled, so a preview cannot describe an item differently from the real thing.
    /// Total: an unknown id returns `{}` rather than failing the call.
    #[func]
    fn item_preview_json(&self, def_id: GString) -> GString {
        let Some(d) = self.data.as_ref() else { return "{}".into(); };
        let id = def_id.to_string();
        let Some(def) = d.items.iter().find(|it| it.id == id) else { return "{}".into(); };
        serde_json::to_string(&item_to_view(&data::instantiate_item(def)))
            .unwrap_or_else(|_| "{}".into())
            .into()
    }

    /// The FULL card catalog as JSON — every `Card`-kind def as an item view. The
    /// Album screen uses it as a DEX: cards you own render in colour, the rest
    /// grayscale. Ordered as they appear in the data file (stable dex order).
    #[func]
    fn all_cards_json(&self) -> GString {
        let Some(d) = self.data.as_ref() else { return "[]".into(); };
        let arr: Vec<serde_json::Value> = d
            .items
            .iter()
            .filter(|it| matches!(it.kind, sim_core::items::ItemKind::Card))
            .map(|def| item_to_view(&data::instantiate_item(def)))
            .collect();
        serde_json::to_string(&arr).unwrap_or_default().into()
    }

    /// Bind (bind) info for the album card at `album_idx`, for `entity_id`:
    /// `{effect, done, cost, cap, maxed}` — drives the Album Bind confirm popup.
    /// `effect` is the human string (e.g. "+5 STR, +10 ATK"). "null" on bad index /
    /// a card with no bind effect.
    #[func]
    fn bind_info_json(&self, entity_id: i64, album_idx: i64) -> GString {
        let _ = entity_id; // bind is party-wide now — the count is the party's, not one member's
        let Some(s) = self.state.as_ref() else { return "null".into(); };
        let Some(card) = s.card_album.get(album_idx as usize) else { return "null".into(); };
        if card.bind_mods.is_empty() {
            return "null".into();
        }
        let done: u8 = s
            .grid
            .entities
            .iter()
            .filter(|e| matches!(e.team, sim_core::grid::Team::Player) && !e.is_pet)
            .map(|e| e.bound_card_counts.get(&card.def_id).copied().unwrap_or(0))
            .max()
            .unwrap_or(0);
        let cap: u8 = 3; // mirrors events::handlers::BIND_CARD_CAP
        let v = serde_json::json!({
            "effect": card.bind_mods.iter().map(stat_mod_text).collect::<Vec<_>>().join(", "),
            "done": done,
            "cost": (done + 1) as u32,
            "cap": cap,
            "maxed": done >= cap,
        });
        serde_json::to_string(&v).unwrap_or_default().into()
    }

    /// Buy `qty` of shop item `def_id` for Gil (banks to the stash). Returns
    /// true on success (false: unknown/not-for-sale item or not enough Gil).
    /// EVENTED (S0): success = Gil was spent (a purchase always costs Gil).
    #[func]
    fn buy_item(&mut self, def_id: GString, qty: i64) -> bool {
        if qty <= 0 || self.data.is_none() {
            return false;
        }
        let Some(s) = self.state.as_mut() else { return false; };
        let before_gil = s.gil();
        events::apply_input_with(
            s,
            self.data.as_ref(),
            events::InputEvent::BuyItem { def_id: def_id.to_string(), qty: qty as u32 },
        );
        s.gil() < before_gil
    }

    /// Grant a loot item by def id into the stash (rare → rare bag). EVENTED (S0)
    /// via `InputEvent::GrantLoot`. TRANSITIONAL — a remnant of the removed 3-hour
    /// claim flow, slated to become a flagged debug tool once the claim UI is gone.
    /// Optimistic return (the sole UI caller discards it).
    #[func]
    fn grant_loot(&mut self, def_id: GString) -> bool {
        if self.data.is_none() {
            return false;
        }
        let Some(s) = self.state.as_mut() else { return false; };
        events::apply_input_with(
            s,
            self.data.as_ref(),
            events::InputEvent::GrantLoot { def_id: def_id.to_string(), foil: false },
        );
        true
    }

    /// DEBUG: grant the FOIL variant of `def_id` (card foil testing).
    #[func]
    fn grant_loot_foil(&mut self, def_id: GString) -> bool {
        if self.data.is_none() {
            return false;
        }
        let Some(s) = self.state.as_mut() else { return false; };
        events::apply_input_with(
            s,
            self.data.as_ref(),
            events::InputEvent::GrantLoot { def_id: def_id.to_string(), foil: true },
        );
        true
    }

    /// Sell ONE stash item (gear or junk) by index for its Gil. Click-to-sell so
    /// the player vendors surplus gear deliberately. Returns the Gil gained
    /// (0 on a bad index / unsellable item).
    #[func]
    fn sell_stash_item(&mut self, stash_idx: i64) -> i64 {
        let Some(s) = self.state.as_mut() else { return 0; };
        let before = s.gil();
        events::apply_input(s, events::InputEvent::SellInventoryItem { inv_idx: stash_idx as u32 });
        (s.gil() - before) as i64
    }

    /// DISCARD one inventory item (the whole stack) by index for NO Gil — the only
    /// exit a `bound` item accepts (a pure destroy, no value minted / no market). The
    /// UI gates this behind a confirm and offers it only on bound items. Returns
    /// `true` if a slot was removed (bad index → false, no-op).
    #[func]
    fn discard_item(&mut self, inv_idx: i64) -> bool {
        let Some(s) = self.state.as_mut() else { return false; };
        let before = s.inventory.len();
        events::apply_input(s, events::InputEvent::DiscardItem { inv_idx: inv_idx as u32 });
        s.inventory.len() < before
    }

    /// USE an Inventory Expander at `inv_idx` for a permanent +1 bag slot. Returns
    /// true if the cap actually grew (false = wrong item / already maxed).
    #[func]
    fn expand_inventory(&mut self, inv_idx: i64) -> bool {
        let Some(s) = self.state.as_mut() else { return false; };
        let before = s.inventory_cap();
        events::apply_input(s, events::InputEvent::ExpandInventory { inv_idx: inv_idx as u32 });
        s.inventory_cap() > before
    }

    /// The party-shared INVENTORY (main bag) as a JSON array — fetched on demand by
    /// the inventory view (omitted from `view_state()` to keep the per-tick payload
    /// small). Empty string while uninitialized.
    #[func]
    fn inventory_json(&self) -> GString {
        match self.state.as_ref() {
            Some(s) => {
                let items: Vec<serde_json::Value> = s.inventory.iter().map(item_to_view).collect();
                serde_json::to_string(&items).unwrap_or_default().into()
            }
            None => GString::default(),
        }
    }

    /// EXEMPLAR UNITS (0.0.350) — every Exemplar the account holds IS a unit, benched or on the
    /// field: `[{unit_id, exemplar_id, name, unit_def, stars, level, deployed, credits}]`, where
    /// `credits` is the Resonance the player may spend on it. The collection screen's list.
    #[func]
    fn exemplar_units_json(&self) -> GString {
        match self.state.as_ref() {
            Some(s) => serde_json::to_string(&sim_core::data::exemplar_units(s))
                .unwrap_or_default()
                .into(),
            None => GString::default(),
        }
    }

    /// The PLAYER spends one Resonance credit on `unit_id` (+1★). EVENTED via
    /// `FeedExemplarUnit` (replay-safe; silent no-op without a credit / at the cap).
    #[func]
    fn feed_exemplar_unit(&mut self, unit_id: i64) -> bool {
        let Some(s) = self.state.as_mut() else { return false; };
        events::apply_input(s, events::InputEvent::FeedExemplarUnit { unit_id: unit_id as u32 });
        true
    }

    /// The OWNED REGALIA collection as a JSON array (the Exemplar twin). Fetched on demand by
    /// the Regalia slot/collection UI. Equip/unequip/feed go through `apply_input_json`.
    #[func]
    fn owned_regalia_json(&self) -> GString {
        match self.state.as_ref() {
            Some(s) => {
                let items: Vec<serde_json::Value> =
                    s.owned_regalia.iter().map(item_to_view).collect();
                serde_json::to_string(&items).unwrap_or_default().into()
            }
            None => GString::default(),
        }
    }

    /// DEBUG: drop a Regalia carrier (`regalia_<id>`) into the owned collection so P0 can be
    /// tested before the gacha exists. Directly pushes (not a recorded event) — dev tool only;
    /// the REAL grant is the server gacha pull (deferred). Returns false on an unknown id.
    #[func]
    fn debug_grant_regalia(&mut self, id: GString) -> bool {
        use sim_core::items::{Item, ItemKind, Rarity};
        let Some(s) = self.state.as_mut() else { return false; };
        let def_id = sim_core::items::regalia_item_def_id(&id.to_string());
        if sim_core::items::regalia_def(&def_id).is_none() {
            return false;
        }
        s.owned_regalia
            .push(Item::new(&def_id, ItemKind::Regalia, 0, 0, Rarity::Legendary));
        true
    }

    /// The STASH WAREHOUSE (deposit store) as a JSON array — fetched on demand by the
    /// stash view. Separate from `inventory_json` (the bag); items land here only via
    /// DepositToStash. Empty string while uninitialized.
    #[func]
    fn stash_json(&self) -> GString {
        match self.state.as_ref() {
            Some(s) => {
                let items: Vec<serde_json::Value> = s.stash.iter().map(item_to_view).collect();
                serde_json::to_string(&items).unwrap_or_default().into()
            }
            None => GString::default(),
        }
    }

    /// Deposit inventory item `inv_idx` into the stash warehouse. No-op on a bad
    /// index or full warehouse (the item stays put).
    #[func]
    fn deposit_to_stash(&mut self, inv_idx: i64) {
        if let Some(s) = self.state.as_mut() {
            events::apply_input(s, events::InputEvent::DepositToStash { inv_idx: inv_idx as u32 });
        }
    }

    /// Withdraw warehouse item `stash_idx` back to the inventory. No-op on a bad
    /// index or full inventory.
    #[func]
    fn withdraw_from_stash(&mut self, stash_idx: i64) {
        if let Some(s) = self.state.as_mut() {
            events::apply_input(s, events::InputEvent::WithdrawFromStash { stash_idx: stash_idx as u32 });
        }
    }

    /// Deposit the CARD at inventory `inv_idx` into the album. No-op if it isn't a
    /// card, the index is bad, or the album is full.
    #[func]
    fn deposit_card_to_album(&mut self, inv_idx: i64) {
        if let Some(s) = self.state.as_mut() {
            events::apply_input(s, events::InputEvent::DepositCardToAlbum { inv_idx: inv_idx as u32 });
        }
    }

    /// Withdraw the album card at `album_idx` back to the inventory. No-op on a bad
    /// index or full inventory.
    #[func]
    fn withdraw_card_from_album(&mut self, album_idx: i64) {
        if let Some(s) = self.state.as_mut() {
            events::apply_input(s, events::InputEvent::WithdrawCardFromAlbum { album_idx: album_idx as u32 });
        }
    }

    /// The RARE LOOT bag as a JSON array — fetched on demand (like the stash).
    /// High-rarity drops land here, separate from the common stash.
    #[func]
    fn rare_bag_json(&self) -> GString {
        match self.state.as_ref() {
            Some(s) => {
                let items: Vec<serde_json::Value> = s.rare_bag.iter().map(item_to_view).collect();
                serde_json::to_string(&items).unwrap_or_default().into()
            }
            None => GString::default(),
        }
    }

    /// The CARD ALBUM as a JSON array — fetched on demand (like the stash). Cards live
    /// here (a dex-style store separate from the gear stash); the card panel equips from
    /// it by index. Empty string while uninitialized.
    #[func]
    fn card_album_json(&self) -> GString {
        match self.state.as_ref() {
            Some(s) => {
                let items: Vec<serde_json::Value> = s
                    .card_album
                    .iter()
                    .map(|it| {
                        let mut v = item_to_view(it);
                        if let Some(obj) = v.as_object_mut() {
                            // STAR-UPGRADE surface (0.0.213): the album UI shows the
                            // star, the dupe cost of the next star, and the special's
                            // resolved display values — all sim-derived.
                            obj.insert("star".into(), serde_json::json!(it.star));
                            // Gacha-style (0.0.214): flat 1 copy = 1 star.
                            if (it.star as usize) < sim_core::items::CARD_STAR_MAX as usize {
                                obj.insert("upgrade_dupes".into(), serde_json::json!(1));
                            }
                            // special_vals is added by item_to_view (every card view).
                        }
                        v
                    })
                    .collect();
                serde_json::to_string(&items).unwrap_or_default().into()
            }
            None => GString::default(),
        }
    }

    /// Special-effect display values for card `def_id` at `star` — the SAME
    /// catalog that drives the mechanics (combat::card_specials), so panel text
    /// can't drift. JSON array of strings; empty = no special.
    #[func]
    fn card_special_vals_json(&self, def_id: GString, star: i64) -> GString {
        let vals = sim_core::combat::card_specials::display_values(
            &def_id.to_string(),
            star.clamp(0, 5) as u8,
        );
        serde_json::to_string(&vals).unwrap_or_default().into()
    }

    /// Upgrade the album card stack at `album_idx` one star (consumes duplicate
    /// copies — see `InputEvent::UpgradeCard`). Evented, replay-safe.
    #[func]
    fn upgrade_card(&mut self, album_idx: i64, copies: i64) {
        if let Some(s) = self.state.as_mut() {
            events::apply_input(s, events::InputEvent::UpgradeCard {
                album_idx: album_idx as u32,
                copies: copies.max(1) as u32,
            });
        }
    }

    /// Feed ONE copy from album stack `feeder_idx` into the card at
    /// `target_idx` — star transfer + foil conversion (see
    /// `InputEvent::FeedCard`). Evented, replay-safe.
    #[func]
    fn feed_card(&mut self, target_idx: i64, feeder_idx: i64) {
        if let Some(s) = self.state.as_mut() {
            events::apply_input(s, events::InputEvent::FeedCard {
                target_idx: target_idx.max(0) as u32,
                feeder_idx: feeder_idx.max(0) as u32,
            });
        }
    }

    /// The instance loot ESCROW as a JSON array — fetched on demand by the loot
    /// panel (omitted from `view_state()`). This is the held loot the party
    /// banks on instance clear / loses on a wipe. Empty string if uninitialized.
    #[func]
    fn escrow_json(&self) -> GString {
        match self.state.as_ref() {
            Some(s) => {
                let items: Vec<serde_json::Value> = s.escrow.iter().map(item_to_view).collect();
                serde_json::to_string(&items).unwrap_or_default().into()
            }
            None => GString::default(),
        }
    }

    /// The crafting recipe catalog as JSON, for the Craft screen — each recipe with its
    /// output (id + resolved name + qty), Gil cost, learn price (`learn_soul`/`learn_gil`)
    /// and `learned` flag, and ingredients (def_id + resolved name + qty). GDScript reads
    /// this + cross-references `inventory()` for have/need counts; it never computes the
    /// recipe. `[]` when no game is loaded.
    #[func]
    fn recipes_json(&self) -> GString {
        let Some(d) = self.data.as_ref() else {
            return "[]".into();
        };
        let learned = |id: &str| -> bool {
            self.state.as_ref().is_some_and(|s| s.recipes_learned.contains(id))
        };
        let name_of = |id: &str| -> String {
            d.item(id)
                .map(|it| it.name.as_str())
                .filter(|n| !n.is_empty())
                .unwrap_or(id)
                .to_string()
        };
        let arr: Vec<serde_json::Value> = d
            .recipes
            .iter()
            .map(|r| {
                let ings: Vec<serde_json::Value> = r
                    .ingredients
                    .iter()
                    .map(|ing| {
                        serde_json::json!({
                            "def_id": ing.def_id,
                            "qty": ing.qty,
                            "name": name_of(&ing.def_id),
                        })
                    })
                    .collect();
                serde_json::json!({
                    "id": r.id,
                    "name": if r.name.is_empty() { r.id.clone() } else { r.name.clone() },
                    "output_def_id": r.output_def_id,
                    "output_name": name_of(&r.output_def_id),
                    "output_qty": r.output_qty,
                    "gil_cost": r.gil_cost,
                    "learn_soul": r.learn_soul,
                    "learn_gil": r.learn_gil,
                    "learned": learned(&r.id),
                    // Which bench tab the recipe belongs under ("blacksmith" / "apothecary").
                    // Grouping only; the sim does not gate crafting on it.
                    "station": r.station,
                    "ingredients": ings,
                    // Does this recipe's output go to the SERVER to be rolled (star + bonus
                    // lines) rather than landing in the bag immediately? The craft screen
                    // needs it for two things: warning that the item arrives on the next
                    // sync, and refusing the craft OFFLINE, where no server will ever
                    // resolve the candidate and the materials would be spent for nothing.
                    // The SIM's own predicate, not a copy of it — a second copy is how a
                    // recipe ends up craftable in the UI and a silent no-op in the sim.
                    "server_rolled":
                        sim_core::craft::output_is_rolled_gear(d, &r.output_def_id),
                })
            })
            .collect();
        serde_json::to_string(&arr).map(GString::from).unwrap_or_else(|_| "[]".into())
    }

    /// RETIRED (0.0.381) with the Star Stone system — answers `{}` so the frozen Godot enchant
    /// view, if opened, draws nothing instead of erroring.
    #[func]
    fn orb_check_json(&self, _orb: GString, _target_json: GString) -> GString {
        "{}".into()
    }

    /// The materials LEDGER (`GameState.materials`, 0.0.354/0.0.364): one item view per held def,
    /// `quantity` = the count — Sigil, Boss Core, Track, Star Stones. Not in `inventory_json`
    /// because they are not bag items; a screen that lists stones or sells materials reads this.
    #[func]
    fn materials_json(&self) -> GString {
        let (Some(s), Some(d)) = (self.state.as_ref(), self.data.as_ref()) else { return "[]".into(); };
        let rows: Vec<serde_json::Value> = s
            .materials()
            .filter_map(|(id, n)| {
                let mut it = sim_core::data::instantiate_item(d.item(id)?);
                it.quantity = n;
                Some(item_to_view(&it))
            })
            .collect();
        serde_json::to_string(&rows).unwrap_or_else(|_| "[]".to_string()).into()
    }
}
