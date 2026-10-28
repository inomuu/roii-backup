//! Private `InputEvent` handlers — the per-variant mutation logic that
//! `apply_input` dispatches to. Each fn is a silent no-op on bad input
//! (replay safety); see `super::InputEvent` for the per-variant contract.

use super::*;

/// Move the rare-bag item at `rare_idx` into the inventory. Respects the
/// inventory SLOT cap (0.0.209 — it used to bypass it like unequip, letting a
/// full bag overflow): a stackable that merges into an existing stack always
/// fits (no new slot); otherwise a full inventory makes this a no-op and the
/// item STAYS in the rare bag. No-op on a bad index.
pub(super) fn move_rare_to_inventory(state: &mut GameState, rare_idx: usize) {
    let Some(rare) = state.rare_bag.get(rare_idx) else {
        return;
    };
    // Stackable rares merge into an existing inventory stack of the same item.
    if rare.is_stackable() {
        let def_id = rare.def_id.clone();
        if let Some(existing) = state
            .inventory
            .iter_mut()
            .find(|i| i.def_id == def_id && i.is_stackable() && !i.bound)
        {
            let item = state.rare_bag.remove(rare_idx);
            existing.quantity = existing.quantity.saturating_add(item.quantity);
            return;
        }
    }
    if state.inventory.len() >= state.inventory_cap() {
        return; // full — the rare stays in the bag
    }
    let item = state.rare_bag.remove(rare_idx);
    state.inventory.push(item);
}

/// Merge `item` into a matching stack in `dest`, else push if under `cap`. Cards
/// stack by `same_card_stack` (def+foil+star) so a Foil never merges onto a plain
/// copy; other stackables by def_id. Returns the item back (`Err`) when `dest` is
/// full and nothing matched — the caller restores it, so a deposit/withdraw never
/// destroys an item.
fn bank_into(
    dest: &mut Vec<crate::items::Item>,
    cap: usize,
    item: crate::items::Item,
) -> Result<(), crate::items::Item> {
    if item.is_stackable() {
        let is_card = matches!(item.kind, crate::items::ItemKind::Card);
        if let Some(existing) = dest.iter_mut().find(|i| {
            i.is_stackable()
                && i.bound == item.bound // bound stacks never merge with unbound (0.0.227)
                && if is_card {
                    crate::drops::same_card_stack(i, &item)
                } else {
                    i.def_id == item.def_id
                }
        }) {
            existing.quantity = existing.quantity.saturating_add(item.quantity);
            return Ok(());
        }
    }
    if dest.len() < cap {
        dest.push(item);
        Ok(())
    } else {
        Err(item)
    }
}

/// Deposit the inventory item at `inv_idx` into the stash warehouse. No-op on a
/// bad index; if the warehouse is full the item is restored to its slot.
pub(super) fn deposit_to_stash(state: &mut GameState, inv_idx: usize) {
    if inv_idx >= state.inventory.len() {
        return;
    }
    let item = state.inventory.remove(inv_idx);
    let cap = state.stash_cap();
    if let Err(back) = bank_into(&mut state.stash, cap, item) {
        state.inventory.insert(inv_idx, back);
    }
}

/// Withdraw the warehouse item at `stash_idx` back to the inventory. No-op on a
/// bad index; if the inventory is full the item is restored to the warehouse.
pub(super) fn withdraw_from_stash(state: &mut GameState, stash_idx: usize) {
    if stash_idx >= state.stash.len() {
        return;
    }
    let item = state.stash.remove(stash_idx);
    let cap = state.inventory_cap();
    if let Err(back) = bank_into(&mut state.inventory, cap, item) {
        state.stash.insert(stash_idx, back);
    }
}

/// Deposit the CARD at inventory `inv_idx` into the card album. No-op if the slot
/// isn't a Card, the index is bad, or the album is full (item restored).
pub(super) fn deposit_card_to_album(state: &mut GameState, inv_idx: usize) {
    let Some(item) = state.inventory.get(inv_idx) else { return };
    if !matches!(item.kind, crate::items::ItemKind::Card) {
        return;
    }
    let item = state.inventory.remove(inv_idx);
    let cap = state.card_album_cap();
    if let Err(back) = bank_into(&mut state.card_album, cap, item) {
        state.inventory.insert(inv_idx, back);
    }
}

/// Withdraw the album card at `album_idx` back to the inventory. No-op on a bad
/// index; if the inventory is full the card is restored to the album.
pub(super) fn withdraw_card_from_album(state: &mut GameState, album_idx: usize) {
    if album_idx >= state.card_album.len() {
        return;
    }
    let item = state.card_album.remove(album_idx);
    let cap = state.inventory_cap();
    if let Err(back) = bank_into(&mut state.inventory, cap, item) {
        state.card_album.insert(album_idx, back);
    }
}

/// CARD STAR upgrade (gacha-style, 0.0.215): feed BASE (0★) duplicates of the
/// same def (+ same foil) into the card at `album_idx` — +1 star per copy, cap
/// 5★. Feeding from the target's own 0★ stack keeps one survivor; a starred
/// target draws from the def's separate 0★ stack (the fix for "การ์ด >1 ดาว
/// อัพเกรดไม่ได้"). Superseded by `feed_card` for the UI; retained for replay of
/// old logs. The album is unbounded (0.0.217), so the promoted copy always banks.
pub(super) fn upgrade_card(state: &mut GameState, album_idx: usize, copies: u32) {
    use crate::items::{ItemKind, CARD_STAR_MAX};
    let Some(target) = state.card_album.get(album_idx) else { return };
    if !matches!(target.kind, ItemKind::Card) || target.star >= CARD_STAR_MAX || copies == 0 {
        return;
    }
    let def_id = target.def_id.clone();
    let foil = target.foil;
    let t_star = target.star;
    // Feeder = the def's 0★ stack (same foil). May BE the target stack itself.
    let Some(feeder_idx) = state
        .card_album
        .iter()
        .position(|c| c.def_id == def_id && c.foil == foil && c.star == 0)
    else {
        return; // no base copies to feed
    };
    let self_feed = feeder_idx == album_idx;
    // BOTH slots are resolved BEFORE anything is consumed. A `get` that returns partway through
    // would leave the feeder decremented with nothing promoted — cards destroyed for free, which
    // is worse for the player than the panic this replaces.
    let Some(spare) = state
        .card_album
        .get(feeder_idx)
        .map(|f| if self_feed { f.quantity.saturating_sub(1) } else { f.quantity })
    else {
        return;
    };
    let Some(mut promoted) = state.card_album.get(album_idx).cloned() else {
        return;
    };
    // `spare`/`gain` bound the FEEDER's stock; the target is debited one copy separately and
    // nothing above checks it holds one. An empty row should not exist, but `quantity` is a u32
    // and `overflow-checks` is on in release — a 0 here is a panic, not a wrap.
    if promoted.quantity == 0 {
        return;
    }
    let gain = copies.min((CARD_STAR_MAX - t_star) as u32).min(spare);
    if gain == 0 {
        return;
    }
    // Both slots resolved and every amount settled — nothing below returns, so the feeder can
    // never be decremented without the promoted copy being produced. Cards destroyed for free
    // would be worse for the player than the panic this replaces.
    promoted.quantity = 1;
    promoted.star = t_star + gain as u8;
    let mut feeder_emptied = false;
    let mut target_emptied = false;
    if let Some(f) = state.card_album.get_mut(feeder_idx) {
        f.quantity -= gain;
        feeder_emptied = f.quantity == 0;
    }
    if let Some(t) = state.card_album.get_mut(album_idx) {
        t.quantity -= 1;
        target_emptied = t.quantity == 0;
    }
    // Remove emptied stacks (higher index first so the other index stays valid).
    let mut removed: Vec<usize> = Vec::new();
    if feeder_emptied {
        removed.push(feeder_idx);
    }
    if target_emptied && !(self_feed && feeder_emptied) {
        removed.push(album_idx);
    }
    removed.sort_unstable();
    removed.dedup();
    for idx in removed.iter().rev() {
        state.card_album.remove(*idx);
    }
    let cap = state.card_album_cap();
    let _ = bank_into(&mut state.card_album, cap, promoted);
}

/// CARD FEED (0.0.216): consume ONE copy from the album stack at `feeder_idx`
/// into the card at `target_idx`. Star gain = 1 + the feeder's star (copies are
/// conserved: a N★ feeder is N+1 absorbed duplicates), capped at 5★. A foil
/// feeder converts the target to foil — legal even on a 5★ plain target (pure
/// conversion, no star gain). All-or-nothing with an album-cap refund.
pub(super) fn feed_card(state: &mut GameState, target_idx: usize, feeder_idx: usize) {
    use crate::items::{ItemKind, CARD_STAR_MAX};
    let Some(target) = state.card_album.get(target_idx) else { return };
    let Some(feeder) = state.card_album.get(feeder_idx) else { return };
    if !matches!(target.kind, ItemKind::Card) || !matches!(feeder.kind, ItemKind::Card) {
        return;
    }
    if target.def_id != feeder.def_id {
        return;
    }
    let self_feed = target_idx == feeder_idx;
    if self_feed && target.quantity < 2 {
        return; // the survivor can't feed itself
    }
    // Saturating throughout: a version-skewed / tampered save may carry an
    // out-of-range star, and apply_input must NEVER panic (replay-survival
    // invariant). saturating_sub gives 0 headroom at/above the cap (so a 5★
    // foil conversion still runs), and saturating_add can't overflow u8.
    let headroom = CARD_STAR_MAX.saturating_sub(target.star);
    let star_gain = feeder.star.saturating_add(1).min(headroom);
    let foil_gain = feeder.foil && !target.foil;
    if star_gain == 0 && !foil_gain {
        return; // 5★ and no foil to transfer — nothing would change
    }
    let (t_star, t_foil) = (target.star, target.foil);
    let f_foil = feeder.foil;
    // Split the promoted copy out BEFORE consuming anything: past this point nothing returns,
    // so a feeder can never be spent without a promoted card to show for it.
    let Some(mut promoted) = state.card_album.get(target_idx).cloned() else { return };
    // Both rows lose a copy below. The only stock guard in this function is the self-feed one
    // (`target.quantity < 2`), which is skipped entirely when the feeder is a DIFFERENT row —
    // so an empty stack on either side underflows a u32 and, with `overflow-checks` on in
    // release, panics rather than wrapping.
    let feeder_stock = state.card_album.get(feeder_idx).map_or(0, |f| f.quantity);
    if promoted.quantity == 0 || feeder_stock == 0 {
        return;
    }
    state.measures.cards_fed.add(1);
    promoted.quantity = 1;
    promoted.star = t_star.saturating_add(star_gain);
    promoted.foil = t_foil || f_foil;
    // Consume ONE copy from the feeder stack, then one from the target.
    let mut feeder_emptied = false;
    let mut target_emptied = false;
    if let Some(f) = state.card_album.get_mut(feeder_idx) {
        f.quantity -= 1;
        feeder_emptied = f.quantity == 0;
    }
    if let Some(t) = state.card_album.get_mut(target_idx) {
        t.quantity -= 1;
        target_emptied = t.quantity == 0;
    }
    // Remove emptied stacks (higher index first so the other index stays valid).
    let mut removed: Vec<usize> = Vec::new();
    if feeder_emptied {
        removed.push(feeder_idx);
    }
    if target_emptied && !(self_feed && feeder_emptied) {
        removed.push(target_idx);
    }
    removed.sort_unstable();
    removed.dedup();
    for idx in removed.iter().rev() {
        state.card_album.remove(*idx);
    }
    // The album is unbounded (0.0.217), so the promoted copy always banks —
    // merging onto an existing same-identity stack or pushing a new row.
    let cap = state.card_album_cap();
    let _ = bank_into(&mut state.card_album, cap, promoted);
}

/// Spend one skill point to LEARN (level 0 → 1) or RAISE `skill_id` on
/// `entity_id`, enforcing the class skill-tree gate (job level +
/// prerequisite). Silent no-op on any failure (replay-safe).
pub(super) fn level_skill(state: &mut GameState, entity_id: u32, skill_id: &str) {
    // Once a character invests in the board, its legacy entry point must not
    // spend outside the board ledger (reset would otherwise erase those ranks).
    let board_node = state.grid.entity_of_id(entity_id).and_then(|e| {
        if e.potential.is_empty() { return None; }
        crate::potential::NODE_DEFS.iter().find_map(|n| {
            if let crate::potential::NodeKind::Skill { slot } = n.kind {
                if crate::potential::skill_id(&e.unit_def, slot) == Some(skill_id) { return Some(n.id); }
            }
            None
        })
    });
    if let Some(id) = board_node {
        crate::potential::actions::upgrade(state, entity_id, id);
        return;
    }
    // No board node for this skill → nothing to level (0.0.352): the Job skill tree and its
    // skill points are gone; Potential is the only way a skill rank rises.
}

/// Allocate a PATHS OF SOUL node (allocate-once), spending its Soul cost. The
/// node must be unowned and reachable — the root, or adjacent to an owned node
/// (you unlock outward through the web). Silent no-op on any failure (replay-
/// safe).
pub(super) fn level_soul_path(state: &mut GameState, skill_id: &str) {
    use crate::soul_paths;
    let Some(node) = soul_paths::path_node(skill_id) else { return; };
    if !soul_paths::is_allocatable(&state.soul_paths, node) {
        return;
    }
    if !state.spend_soul(node.cost, crate::state::SoulSpend::SoulPath) {
        return;
    }
    state.soul_paths.insert(skill_id.to_string(), 1);
    state.measures.soul_nodes_allocated.add(1);
}

/// VENDORING IS RETIRED (0.0.411, user 2026-09-17: "เอาระบบขายออกได้" — Gil comes from fusion
/// now, with no Gil market and no junk to sell). Both events stay in the enum so an old replay
/// log still parses, and both are silent no-ops here: the state a sale used to change is left
/// exactly as it stands.
pub(super) fn sell_all_junk(_state: &mut GameState) {}

/// See `sell_all_junk`.
pub(super) fn sell_inventory_item(_state: &mut GameState, _idx: usize) {}

/// DISCARD one inventory item (the whole stack) by index for ZERO Gil — a pure
/// destroy, no economy touch. Unlike `sell_inventory_item` this ACCEPTS bound
/// items (it's the one liquidation path a bound item allows: destroying it mints
/// no value and reaches no market, so it can't launder). No-op on a bad index.
pub(super) fn discard_inventory_item(state: &mut GameState, idx: usize) {
    if idx >= state.inventory.len() {
        return;
    }
    state.inventory.remove(idx);
}

/// The item that a `ExpandInventory` use consumes for a permanent +1 bag slot.
pub const INVENTORY_EXPANDER_ID: &str = "inventory_expander";

/// USE an Inventory Expander at `idx`: +1 permanent bag slot (capped at
/// `MAX_INVENTORY_CAP_BONUS`), consuming one expander unit. No-op — and nothing
/// consumed — if the item isn't an expander or the cap is already maxed.
pub(super) fn expand_inventory(state: &mut GameState, idx: usize) {
    // Every reason to refuse is checked BEFORE the bonus is granted, so the cap can never rise
    // without the expander being consumed.
    let Some(it) = state.inventory.get_mut(idx) else { return };
    if it.def_id != INVENTORY_EXPANDER_ID {
        return;
    }
    if state.inventory_cap_bonus >= crate::state::MAX_INVENTORY_CAP_BONUS {
        return; // already maxed — don't waste the item
    }
    state.inventory_cap_bonus += 1;
    if let Some(it) = state.inventory.get_mut(idx) {
        if it.quantity > 1 {
            it.quantity -= 1;
            return;
        }
    }
    state.inventory.remove(idx);
}

/// Max consumable slots per character.
pub const CONSUMABLE_SLOTS: usize = 3;

/// Slot/unslot a learned ACTIVE skill in `entity_id`'s loadout. The loadout is
/// the explicit set of skills that auto-cast; only slotted actives fire (empty =
/// none). See `InputEvent::ToggleActiveSkill`.
pub(super) fn toggle_active_skill(state: &mut GameState, entity_id: u32, skill_id: &str) {
    use crate::grid::ACTIVE_SLOT_CAP;
    use crate::skills::{skill_def, SkillKind};
    let Some(e) = state.grid.entity_of_id_mut(entity_id) else { return; };
    // The PARTY's loadout only (audit 2026-10-02): a monster's actives fire only from
    // `active_skills`, so this client input could switch a boss's skills off.
    if !matches!(e.team, crate::grid::Team::Player) || e.is_pet {
        return;
    }
    // Must KNOW the skill and it must be Active.
    if !e.skills.iter().any(|s| s.id == skill_id && s.level > 0) {
        return;
    }
    if !matches!(skill_def(skill_id).map(|d| d.kind), Some(SkillKind::Active)) {
        return;
    }
    if let Some(pos) = e.active_skills.iter().position(|s| s == skill_id) {
        e.active_skills.remove(pos); // slotted → remove
    } else if e.active_skills.len() < ACTIVE_SLOT_CAP {
        e.active_skills.push(skill_id.to_string()); // free slot → add
    }
    // else: bar full → no-op (player removes one first).
}

/// Move a `Consumable` stash item into `entity_id`'s consumable bar (cap 3).
/// A slot-holder must be a living party MEMBER — not a monster, not a pet.
///
/// `idx_of_id` resolves any entity on the grid, so without this an `InputEvent` naming a monster
/// debits the bag and pushes the item onto something that `consumable_upkeep` skips and
/// `prune_dead_monsters` deletes: the item is gone. Unreachable through the shipped UI, and that
/// is not the bar — a replay-verify server accepts whatever the log contains.
fn is_slot_holder(state: &GameState, ei: usize) -> bool {
    state
        .grid
        .entities
        .get(ei)
        .is_some_and(|e| matches!(e.team, crate::grid::Team::Player) && !e.is_pet)
}

pub(super) fn add_consumable(state: &mut GameState, entity_id: u32, stash_idx: usize) {
    use crate::items::{ConsumableSlot, ItemKind};
    let Some(ei) = state.grid.idx_of_id(entity_id) else { return; };
    if !is_slot_holder(state, ei) {
        return;
    }
    let Some(bar) = state.grid.entities.get(ei).map(|e| e.consumables.len()) else { return };
    if bar >= CONSUMABLE_SLOTS {
        return;
    }
    let Some(it) = state.inventory.get(stash_idx) else { return; };
    if !matches!(it.kind, ItemKind::Consumable) {
        return;
    }
    // A PROVISION MAY NOT SIT IN THE EMERGENCY BAR. The reserved slots exist because "food
    // competing with a healing potion for the same three slots is not an interesting choice" —
    // one Meal, one Tonic, deliberately. Parking meals in the bar makes that four meals at once,
    // and `buff_sum` adds them all: +26 to every stat from what is supposed to be one meal.
    //
    // `provisions_view.gd` filters them out of ITS picker and says so in a comment, but
    // `character_view_bag.gd` routes any Consumable straight here — so the rule was enforced by
    // one of the two screens that can reach it, which is the same as not enforced.
    if it.consumable.as_ref().and_then(|c| c.provision).is_some() {
        return;
    }
    let item = state.inventory.remove(stash_idx);
    // The item is already out of the bag — put it back rather than destroy it if the entity
    // somehow vanished between the two borrows.
    match state.grid.entities.get_mut(ei) {
        Some(e) => e.consumables.push(ConsumableSlot { key: 0, item, ready_tick: 0 }),
        None => state.inventory.insert(stash_idx, item),
    }
}

/// Move up to `qty` of a provision into `entity_id`'s RESERVED slot for its kind.
///
/// SPLITS the stack. Slots are per-character but stackables merge into a single inventory stack,
/// so moving the whole stack would let the first member take all 100 Meals and leave the other
/// four with nothing to slot — the per-character design would break the moment a party had a
/// second member.
/// Move a PARTY provision from the inventory into the party rack.
///
/// Every rejection is a SILENT no-op, never a panic — `apply_input` must survive a log replayed
/// against a different `game_data` version, where an item's spec may no longer declare Party.
pub(super) fn add_party_provision(state: &mut GameState, inv_idx: usize, qty: u32) {
    use crate::items::{
        ItemKind, PartyProvision, ProvisionKind, PARTY_PROVISION_SLOTS, PARTY_PROVISION_SOURCE,
    };
    if qty == 0 {
        return;
    }
    let Some(it) = state.inventory.get(inv_idx) else { return };
    if !matches!(it.kind, ItemKind::Consumable) {
        return;
    }
    // Only an item that DECLARES itself a party provision may enter the rack.
    if it.consumable.as_ref().and_then(|c| c.provision) != Some(ProvisionKind::Party) {
        return;
    }
    // THE ONE RULE (user): no duplicate ITEM across the rack. Not "no duplicate buff kind" —
    // three different items each granting +ATK is a legal loadout, which is exactly why each slot
    // carries its own buff source.
    let def_id = it.def_id.clone();
    // REFILL IN PLACE (2026-10-03, Codex's "Add more"): the same item, with the same binding,
    // tops up the slot already holding it — its key, `ready_tick` and live buff are untouched, so
    // a running provision keeps running. A different binding is a different item for the rack
    // (bound and tradeable units must not merge), and is refused like any duplicate.
    if let Some(slot_idx) = state.party_provisions.iter().position(|s| s.item.def_id == def_id) {
        if state.party_provisions.get(slot_idx).is_some_and(|s| s.item.bound != it.bound) {
            return;
        }
        let take = qty.min(it.quantity.max(1));
        if let Some(inv) = state.inventory.get_mut(inv_idx) {
            if inv.quantity <= take {
                state.inventory.remove(inv_idx);
            } else {
                inv.quantity -= take;
            }
        }
        if let Some(slot) = state.party_provisions.get_mut(slot_idx) {
            slot.item.quantity = slot.item.quantity.saturating_add(take);
        }
        return;
    }
    if state.party_provisions.len() >= PARTY_PROVISION_SLOTS {
        return;
    }
    // PICK THE KEY BEFORE DEBITING. The search below can decline, and it used to decline AFTER
    // the bag was charged — so `moved` was dropped on the floor and the units were destroyed. Same
    // ordering bug this file fixed in `add_provision`; this sibling kept it.
    //
    // Rack membership alone is not a free key: removing an item deliberately leaves its buff
    // running (see `remove_party_provision`) and a depleted stack is swept by the upkeep, so a key
    // can be absent from the rack while its buff still has its whole duration left. Hand that key
    // to a new item and the newcomer's `BuffMissing` finds the ORPHAN on its own source: it never
    // fires and never drains until the orphan lapses.
    //
    // So: PREFER a key that is free and quiet, but FALL BACK to merely free. The rack's key space
    // is exactly as wide as the rack (3), so with a long buff a single removal could otherwise
    // make every subsequent add fail for the buff's full duration — a screen that silently refuses
    // to accept anything. Inheriting an orphan costs one slot one refresh cycle; refusing costs
    // the player the feature. Same preference-then-fallback shape as the per-character allocator,
    // which was bounded for the same reason.
    let live = |k: u8| {
        let src = PARTY_PROVISION_SOURCE + k as u32;
        state
            .grid
            .entities
            .iter()
            .any(|e| e.buffs.iter().any(|b| b.source_id == src))
    };
    let held = |k: u8| state.party_provisions.iter().any(|s| s.key == k);
    let Some(key) = (0..PARTY_PROVISION_SLOTS as u8)
        .find(|k| !held(*k) && !live(*k))
        .or_else(|| (0..PARTY_PROVISION_SLOTS as u8).find(|k| !held(*k)))
    else {
        // Every key is held by an item actually in the rack — the rack is full, which the length
        // check at the top already covers. Unreachable, and a no-op if it ever is.
        return;
    };
    let take = qty.min(it.quantity.max(1));
    let mut moved = it.clone();
    moved.quantity = take;
    // Debit only now, when nothing below can decline.
    if let Some(inv) = state.inventory.get_mut(inv_idx) {
        if inv.quantity <= take {
            state.inventory.remove(inv_idx);
        } else {
            inv.quantity -= take;
        }
    }
    state.party_provisions.push(PartyProvision { key, item: moved, ready_tick: 0 });
    state.measures.party_provisions_loaded.add(1);
}

/// Return a party rack slot to the inventory.
///
/// RUNNING BUFFS ARE LEFT ALONE (user 2026-08-18). A provision applies for a duration, and that
/// duration is already paid for: take the item out fifteen minutes into a thirty-minute buff and
/// the buff still ends at thirty. Stripping it would make the rack behave like a worn accessory,
/// which is not what a consumable is.
///
/// That is exactly why an entry carries a stable `key` rather than being identified by its
/// position: this removal shifts every later entry down, and a position-derived source would hand
/// the neighbour the departed item's still-running buff — the neighbour would stop refreshing
/// (its buff "already there"), then double up when the orphan lapsed.
pub(super) fn remove_party_provision(state: &mut GameState, slot: usize) {
    if slot >= state.party_provisions.len() {
        return;
    }
    // Look before leaping: `try_grant_item` returns false on a full bag and banks NOTHING, so
    // removing first and granting after would delete the item on the one press a player makes
    // when their bag is full. Clone out, try, and only then unrack.
    let Some(entry) = state.party_provisions.get(slot) else { return };
    let item = entry.item.clone();
    if !crate::data::try_grant_bag_item(state, item) {
        return; // bag full — the rack keeps it, the press is a no-op
    }
    state.party_provisions.remove(slot);
}

pub(super) fn add_provision(state: &mut GameState, entity_id: u32, inv_idx: usize, qty: u32) {
    use crate::items::{ConsumableSlot, ItemKind};
    if qty == 0 {
        return;
    }
    let Some(ei) = state.grid.idx_of_id(entity_id) else { return; };
    if !is_slot_holder(state, ei) {
        return;
    }
    let Some(it) = state.inventory.get(inv_idx) else { return; };
    if !matches!(it.kind, ItemKind::Consumable) {
        return;
    }
    // Only an item that DECLARES a kind may take a reserved slot — otherwise these are just two
    // more bar slots wearing a different label.
    let Some(kind) = it.consumable.as_ref().and_then(|c| c.provision) else { return; };
    // Party provisions belong to the RACK, not to a member. Letting one take a per-character
    // reserved slot would also make it invisible: `provisions_view.gd` draws the Meal and Tonic
    // columns only, so it could never be taken out again.
    if matches!(kind, crate::items::ProvisionKind::Party) {
        return;
    }
    let take = qty.min(it.quantity.max(1));
    let def_id = it.def_id.clone();
    let bound = it.bound;

    // Detach the moved portion first, so the inventory is consistent whichever branch runs below.
    let mut moved = it.clone();
    moved.quantity = take;
    // Resolve the destination BEFORE the stack is split — otherwise a miss below would leave the
    // inventory debited and the provision nowhere.
    let Some(e) = state.grid.entities.get_mut(ei) else { return };
    let existing = e
        .provisions
        .iter()
        .position(|s| s.item.consumable.as_ref().and_then(|c| c.provision) == Some(kind));
    // TOP UP only when the two stacks would really merge. `def_id` alone is not that test: the
    // bag's own merge rule (`drops::bank_to_inventory_inner`, mirrored by `can_bank`) also
    // requires `bound` to agree, and this path had no such check — so adding an UNBOUND stack onto
    // a BOUND slot silently relabelled it, and unslotting returned the whole merged quantity at
    // the slot's flag. `aspd_pot_1` is reachable both ways on shipped data: the battle-pass free
    // track mints it bound, and `brew_speed_potion_1` crafts it unbound.
    let top_up = matches!(
        existing.and_then(|pos| e.provisions.get(pos)),
        Some(slot) if slot.item.def_id == def_id && slot.item.bound == bound
    );

    // ASK BEFORE DEBITING. If this is a swap, the displaced provision has to go somewhere; check
    // that it can BEFORE the bag is touched, so there is nothing to put back on failure. The
    // earlier version debited first and restored on failure, and the restore was not guaranteed —
    // a bag already over cap (`migrate_save_items` can push past it) failed both, destroying the
    // portion that had just been taken out.
    if let Some(pos) = existing.filter(|_| !top_up) {
        let displaced = match state.grid.entities.get(ei).and_then(|e| e.provisions.get(pos)) {
            Some(slot) => slot.item.clone(),
            None => return,
        };
        // Measured against the bag as it stands, BEFORE the take — which is the pessimistic
        // reading, and the safe one: the take can only free space, never consume it.
        if !crate::data::can_bank(state, &displaced) {
            return;
        }
    }

    if let Some(inv) = state.inventory.get_mut(inv_idx) {
        if inv.quantity <= take {
            state.inventory.remove(inv_idx);
        } else {
            inv.quantity -= take;
        }
    }

    let Some(e) = state.grid.entities.get_mut(ei) else { return };
    match existing {
        // Same food — TOP UP rather than swap, or a player could never add to a slot.
        Some(pos) if top_up => {
            if let Some(slot) = e.provisions.get_mut(pos) {
                slot.item.quantity = slot.item.quantity.saturating_add(take);
            }
            return;
        }
        Some(pos) => {
            // The displaced provision is BANKED, not pushed — same merge + cap reasoning as
            // `remove_provision`. `can_bank` was asked above, before the debit, so this cannot
            // strand the portion that was taken out.
            let old = match state.grid.entities.get(ei).and_then(|e| e.provisions.get(pos)) {
                Some(slot) => slot.item.clone(),
                None => return,
            };
            crate::data::try_grant_bag_item(state, old);
            if let Some(e) = state.grid.entities.get_mut(ei) {
                e.provisions.remove(pos);
            }
        }
        None => {}
    }
    if let Some(e) = state.grid.entities.get_mut(ei) {
        e.provisions.push(ConsumableSlot { key: 0, item: moved, ready_tick: 0 });
    }
}

/// Return the provision in `entity_id`'s reserved `slot` to the inventory.
///
/// Through `try_grant_item`, and only removing the slot once it SUCCEEDS. A raw push did neither
/// of the two things banking does: it did not MERGE with an existing stack of the same def — so
/// slotting a stack, looting more, then unslotting left two stacks of one consumable, wasting a
/// bag slot the cap counts — and it did not respect the cap at all, so the bag could sit at 61/60
/// and the next drop would fall through to `overflow_recover` and be auto-sold or lost.
pub(super) fn remove_provision(state: &mut GameState, entity_id: u32, slot: usize) {
    let Some(ei) = state.grid.idx_of_id(entity_id) else { return; };
    let Some(item) = state
        .grid
        .entities
        .get(ei)
        .and_then(|e| e.provisions.get(slot))
        .map(|s| s.item.clone())
    else {
        return;
    };
    if !crate::data::try_grant_bag_item(state, item) {
        return; // bag full — the slot keeps it rather than the item vanishing
    }
    if let Some(e) = state.grid.entities.get_mut(ei) {
        e.provisions.remove(slot);
    }
}

/// Load one elemental quiver from `stash_idx` into `entity_id`'s ammo pool.
/// Reads the quiver's `LoadAmmo` effect (element + shots), grants those rounds,
/// and consumes one quiver from the stash stack.
pub(super) fn load_ammo(state: &mut GameState, entity_id: u32, stash_idx: usize) {
    use crate::items::{ConsumeEffect, ItemKind};
    let Some(ei) = state.grid.idx_of_id(entity_id) else { return; };
    // Holder-guarded like `add_consumable`: `idx_of_id` resolves ANY entity, so without this a log
    // naming a monster spends the quiver and loads the rounds onto something the pruner deletes.
    if !is_slot_holder(state, ei) {
        return;
    }
    let Some(it) = state.inventory.get(stash_idx) else { return; };
    if !matches!(it.kind, ItemKind::Consumable) {
        return;
    }
    let Some(ConsumeEffect::LoadAmmo { element, shots }) = it.consumable.map(|c| c.effect) else {
        return;
    };
    let Some(e) = state.grid.entities.get_mut(ei) else { return };
    e.load_ammo(element, shots);
    // Spend one quiver; drop the stash entry when the stack empties.
    let mut spent_out = false;
    if let Some(entry) = state.inventory.get_mut(stash_idx) {
        entry.quantity = entry.quantity.saturating_sub(1);
        spent_out = entry.quantity == 0;
    }
    if spent_out {
        state.inventory.remove(stash_idx);
    }
}

/// Equip one card from `stash_idx` into `entity_id`'s CARD PANEL (by the card's
/// `card_slot` class). No-op on any mismatch: bad ids, not a card, no class, or the
/// class's panel is already full (`card_panel_capacity`). Cards equip to the
/// CHARACTER — activation is gated later by the matching gear's sockets, so a full
/// panel with low-socket gear is fine (extras just sit inactive).
pub(super) fn equip_card(
    state: &mut GameState,
    entity_id: u32,
    album_idx: usize,
    target: Option<crate::items::CardClass>,
) {
    use crate::items::{card_panel_capacity, is_accessory_class, ItemKind};
    let Some(ei) = state.grid.idx_of_id(entity_id) else { return; };
    // Party members only (audit 2026-10-02): a card equipped onto a monster died with it.
    if !is_slot_holder(state, ei) {
        return;
    }
    // Cards live in the dex-style ALBUM (separate from the gear stash) — equip pulls
    // from there.
    let Some(card) = state.card_album.get(album_idx) else { return; };
    if !matches!(card.kind, ItemKind::Card) {
        return;
    }
    let Some(cs) = card.card_slot else { return; };
    // Resolve the destination panel: an accessory card may target EITHER accessory
    // panel; any other card must match its own class (or no target = its own class).
    let dest = match target {
        None => cs,
        Some(t) if t == cs => t,
        Some(t) if is_accessory_class(cs) && is_accessory_class(t) => t,
        Some(_) => return, // target mismatch
    };
    let Some(held) = state.grid.entities.get(ei).map(|e| {
        e.card_panel.iter().filter(|c| c.card_slot == Some(dest)).count()
    }) else {
        return;
    };
    if held >= card_panel_capacity(dest) as usize {
        return; // that panel is full
    }
    // Move ONE card off the album stack into the panel, STAMPING the destination panel.
    // Cloned and placed BEFORE the album is debited, so the copy can never vanish in between.
    let Some(mut one) = state.card_album.get(album_idx).cloned() else { return };
    one.quantity = 1;
    one.card_slot = Some(dest);
    let Some(e) = state.grid.entities.get_mut(ei) else { return };
    e.card_panel.push(one);
    let mut emptied = false;
    if let Some(entry) = state.card_album.get_mut(album_idx) {
        entry.quantity = entry.quantity.saturating_sub(1);
        emptied = entry.quantity == 0;
    }
    if emptied {
        state.card_album.remove(album_idx);
    }
}

/// Unequip the card at `panel_idx` from `entity_id`'s card panel back to the ALBUM
/// (the album is the equip SOURCE, so an unequipped card returns there ready to
/// re-slot). Stacks onto its matching album row via `bank_card_to_album`. No-op on
/// bad id/index. A panel card is already the player's property, so returning it must
/// NEVER lose it: if the album can't take it (full + a brand-new type with no stack
/// to merge onto), leave the card equipped rather than delete it.
pub(super) fn unequip_card(state: &mut GameState, entity_id: u32, panel_idx: usize) {
    let Some(ei) = state.grid.idx_of_id(entity_id) else { return; };
    let Some(card) = state.grid.entities.get(ei).and_then(|e| e.card_panel.get(panel_idx)) else {
        return;
    };
    // Variant-aware: a foil/starred card only merges onto ITS OWN stack (else it
    // would need a fresh slot — never fold a foil into a plain row and lose the bit).
    let can_merge = state
        .card_album
        .iter()
        .any(|i| crate::drops::same_card_stack(i, card));
    if !can_merge && state.card_album.len() >= state.card_album_cap() {
        return; // album full, nowhere to put a new type — keep it equipped
    }
    let Some(e) = state.grid.entities.get_mut(ei) else { return };
    let card = e.card_panel.remove(panel_idx);
    crate::drops::bank_card_to_album(state, card);
}

/// Max times a single card type may be bound for permanent stats.
pub const BIND_CARD_CAP: u8 = 3;

/// Bind cards from the ALBUM at `album_idx` into `entity_id` for a permanent stat
/// bonus. The Nth bind of a type costs N cards (1/2/3) and caps at `BIND_CARD_CAP`.
pub(super) fn bind_card(state: &mut GameState, _entity_id: u32, album_idx: usize) {
    use crate::items::ItemKind;
    let Some(card) = state.card_album.get(album_idx) else { return; };
    if !matches!(card.kind, ItemKind::Card) {
        return;
    }
    if card.foil {
        return; // FOIL cards are too special to bind — equip / trade only.
    }
    if card.bind_mods.is_empty() {
        return; // nothing to grant — not a bindable card
    }
    let bind_mods = card.bind_mods.clone();
    let def_id = card.def_id.clone();
    // PARTY-WIDE (2026-07-08): binding a card grants its PERMANENT bonus to EVERY
    // party member. Escalating cost + cap by the party's current bind level (max
    // over members — they stay in sync since a bind always applies to all).
    let done = state
        .grid
        .entities
        .iter()
        .filter(|e| matches!(e.team, crate::grid::Team::Player) && !e.is_pet)
        .map(|e| *e.bound_card_counts.get(&def_id).unwrap_or(&0))
        .max()
        .unwrap_or(0);
    if done >= BIND_CARD_CAP {
        return; // this card type is already maxed for the party
    }
    let cost = (done + 1) as u32;
    let Some(stock) = state.card_album.get(album_idx).map(|c| c.quantity) else { return };
    if stock < cost {
        return; // not enough cards held for this bind tier
    }
    // Grant the permanent bonus to every member + bump each one's count in sync.
    for e in state
        .grid
        .entities
        .iter_mut()
        .filter(|e| matches!(e.team, crate::grid::Team::Player) && !e.is_pet)
    {
        e.bound_card_mods.extend_from_slice(&bind_mods);
        e.bound_card_counts.insert(def_id.clone(), done + 1);
    }
    let mut emptied = false;
    if let Some(entry) = state.card_album.get_mut(album_idx) {
        entry.quantity -= cost;
        emptied = entry.quantity == 0;
    }
    if emptied {
        state.card_album.remove(album_idx);
    }
}

/// Return the consumable in `entity_id`'s bar `slot` to the stash.
pub(super) fn remove_consumable(state: &mut GameState, entity_id: u32, slot: usize) {
    // Same as `remove_provision`: bank it (merge + cap), and only free the slot once that worked.
    let Some(ei) = state.grid.idx_of_id(entity_id) else { return; };
    let Some(item) = state
        .grid
        .entities
        .get(ei)
        .and_then(|e| e.consumables.get(slot))
        .map(|s| s.item.clone())
    else {
        return;
    };
    if !crate::data::try_grant_bag_item(state, item) {
        return;
    }
    if let Some(e) = state.grid.entities.get_mut(ei) {
        e.consumables.remove(slot);
    }
}

pub(super) fn unequip_item(state: &mut GameState, entity_id: u32, equipped_slot: u32) {
    let Some(eidx) = state.grid.idx_of_id(entity_id) else { return; };
    let slot = equipped_slot as usize;
    let Some(worn) = state.grid.entities.get(eidx).map(|e| e.equipped.len()) else { return };
    if slot >= worn {
        return;
    }
    // `remove` (not swap_remove) — equipped slot order is part of the
    // public state because the UI's [Refine] / [Unequip] buttons map to
    // these indices. Reordering on unequip would be disorienting. Item
    // returns to the party-shared stash.
    let Some(e) = state.grid.entities.get_mut(eidx) else { return };
    let item = e.equipped.remove(slot);
    state.inventory.push(item);
}

pub(super) fn equip_item(state: &mut GameState, entity_id: u32, inventory_idx: u32) {
    let Some(eidx) = state.grid.idx_of_id(entity_id) else { return; };
    // Party members only (audit 2026-10-02): gear equipped onto a monster died with it.
    if !is_slot_holder(state, eidx) {
        return;
    }
    use crate::items::{EquipSlot, WeaponType};
    let iidx = inventory_idx as usize;
    // PEEK PHASE — every fact the checks below need is copied out here, BEFORE the stash is
    // touched. A rejected equip must leave both the stash and the paper doll untouched, and
    // gathering the reads in one place is what makes that true by construction rather than by
    // each check remembering to peek.
    let Some(e) = state.grid.entities.get(eidx) else { return };
    let level = e.stats.level as u32;
    let mastery_unlocked = crate::skills::sword_shield_mastery_level(&e.skills) > 0;
    let worn_slots: Vec<EquipSlot> = e.equipped.iter().map(|i| i.slot()).collect();
    let has_2h = e.equipped.iter().any(|i| matches!(i.weapon_type, WeaponType::Sword2H));
    let Some(it) = state.inventory.get(iidx) else { return };
    // GEAR ONLY (audit 2026-10-02): `Item::slot()` falls back to Armor, so a material, card or
    // consumable stack could be "worn" — freeing bag slots and splitting stacks on unequip.
    if !matches!(it.kind, crate::items::ItemKind::Weapon | crate::items::ItemKind::Armor) {
        return;
    }
    // WEAPON FAMILY: every piece fits (user 2026-09-29); off-family gear pays in `Item::worn_atk`.
    // LEVEL REQUIREMENT (0.0.136): gear gates at (item_level-1)*10 — s2=Lv10,
    // s3=Lv20 … s7=Lv60. Under-leveled equip is a silent no-op like every other
    // invalid input (replay-safe).
    if level < it.level_req() {
        return;
    }
    // 2H + SHIELD is FORBIDDEN unless the member knows Sword & Shield Mastery
    // (which unlocks the combo at a damage penalty, see `combat::derived`).
    if !mastery_unlocked {
        let adding_shield = it.slot() == EquipSlot::Shield;
        let adding_2h = matches!(it.weapon_type, WeaponType::Sword2H);
        let has_shield_eq = worn_slots.contains(&EquipSlot::Shield);
        if (adding_shield && has_2h) || (adding_2h && has_shield_eq) {
            return;
        }
    }
    // Pull from the shared stash. `swap_remove` is O(1) and reorders the
    // stash tail — that reorder IS public state; replays match because
    // events apply in the same order on the same initial stash. Don't
    // switch to `remove` (O(n) + DIFFERENT order) without a version bump.
    let mut item = state.inventory.swap_remove(iidx);
    // GENERIC ACCESSORIES (0.0.133, RO-style): the two accessory slots are
    // interchangeable — two rings, two pendants, or one of each. The def's
    // Necklace/Ring is only a "home"; on equip prefer the item's own
    // stamp if that slot is free, else the OTHER accessory slot, else fall
    // through to the auto-swap below (replaces the occupant of its own slot).
    // The placed slot is stamped on the instance and preserved by migration.
    {
        let own = item.slot();
        if matches!(own, EquipSlot::Necklace | EquipSlot::Ring) {
            let other = if own == EquipSlot::Necklace {
                EquipSlot::Ring
            } else {
                EquipSlot::Necklace
            };
            if worn_slots.contains(&own) && !worn_slots.contains(&other) {
                item.equip_slot = Some(other);
            }
        }
    }
    let new_slot = item.slot();
    // Auto-swap: if this member already wears a piece in the SAME paper-doll
    // slot, pop it back into the stash so each of the 10 RO slots holds at most
    // one item (a shield and a body armor are both `Armor` kind but different
    // slots, so they coexist; two body armors do not).
    let Some(e) = state.grid.entities.get_mut(eidx) else {
        state.inventory.push(item); // entity gone between peek and place — never eat the item
        return;
    };
    if let Some(existing) = e.equipped.iter().position(|i| i.slot() == new_slot) {
        let old_item = e.equipped.remove(existing);
        state.inventory.push(old_item);
    }
    if let Some(e) = state.grid.entities.get_mut(eidx) {
        e.equipped.push(item);
        state.measures.items_equipped.add(1);
    }
}

// ---- EXEMPLAR CARRIER FLOW — RETIRED (0.0.350) ----
// An Exemplar is a UNIT now (`data::grant_exemplar_unit`); nothing is worn, marked or merged.
// The four handlers that lived here (equip / unequip / feed / sync) are gone with the mechanic;
// their input events survive as tombstones in `apply_input` so an old event log still parses.

// ---- REGALIA (twins of the Exemplar equip/unequip/feed/sync above) ----

/// Equip the owned Regalia at `owned_idx` onto `entity_id` (a SEPARATE slot from the Exemplar —
/// a member may wear both). Twin of `equip_exemplar`.
pub(super) fn equip_regalia(state: &mut GameState, entity_id: u32, owned_idx: usize) {
    use crate::items::ItemKind;
    let Some(member) = state.grid.entity_of_id(entity_id) else { return; };
    if !matches!(member.team, crate::grid::Team::Player) || member.is_pet {
        return;
    }
    let unit_def = member.unit_def.clone();
    let Some(item) = state.owned_regalia.get(owned_idx) else { return; };
    if !matches!(item.kind, ItemKind::Regalia) || item.equipped_by == entity_id {
        return;
    }
    let Some(def) = crate::items::regalia_def(&item.def_id) else { return; };
    let allowed = crate::items::regalia_allowed_classes(&def.id);
    if !allowed.is_empty() && !allowed.contains(&unit_def.as_str()) {
        return;
    }
    // A card worn by ANOTHER unit MOVES here, and that unit loses the overlay with it (audit
    // 2026-10-02): it used to keep `regalia` with no card behind it, so the freed card could be
    // salvaged or fed while still in effect — one card, two wearers.
    let prev = item.equipped_by;
    for c in state.owned_regalia.iter_mut() {
        if c.equipped_by == entity_id {
            c.equipped_by = 0;
        }
    }
    if let Some(c) = state.owned_regalia.get_mut(owned_idx) {
        c.equipped_by = entity_id;
        state.measures.regalia_equipped.add(1);
    }
    if prev != 0 {
        if state.grid.idx_of_id(prev).is_some() {
            sync_member_regalia(state, prev);
        } else if let Some(rc) = state.roster.get_mut(&prev) {
            rc.data.regalia = None;
        }
    }
    sync_member_regalia(state, entity_id);
}

/// Unequip `entity_id`'s Regalia — clears the mark and drops the overlay. Twin of `unequip_exemplar`.
pub(super) fn unequip_regalia(state: &mut GameState, entity_id: u32) {
    if state.grid.idx_of_id(entity_id).is_none() {
        return;
    }
    let mut changed = false;
    for c in state.owned_regalia.iter_mut() {
        if c.equipped_by == entity_id {
            c.equipped_by = 0;
            changed = true;
        }
    }
    if changed {
        sync_member_regalia(state, entity_id);
    }
}

/// RESONANCE feed for Regalia — twin of `feed_exemplar` (feeder must be unequipped, same def,
/// target may be equipped and re-syncs). No measure counter (P0).
pub(super) fn feed_regalia(state: &mut GameState, target_idx: usize, feeder_idx: usize) {
    use crate::items::{ItemKind, REGALIA_MAX_STARS};
    if target_idx == feeder_idx {
        return;
    }
    let (Some(t), Some(f)) =
        (state.owned_regalia.get(target_idx), state.owned_regalia.get(feeder_idx))
    else {
        return;
    };
    if !matches!(t.kind, ItemKind::Regalia) || !matches!(f.kind, ItemKind::Regalia) {
        return;
    }
    if t.def_id != f.def_id || f.equipped_by != 0 || f.locked {
        return;
    }
    let headroom = REGALIA_MAX_STARS.saturating_sub(t.resonance_stars);
    let gain = f.resonance_stars.saturating_add(1).min(headroom);
    if gain == 0 {
        return;
    }
    let target_member = t.equipped_by;
    if let Some(t) = state.owned_regalia.get_mut(target_idx) {
        t.resonance_stars = t.resonance_stars.saturating_add(gain);
    }
    state.owned_regalia.remove(feeder_idx);
    if target_member != 0 {
        sync_member_regalia(state, target_member);
    }
}

/// ENHANCE — the LEVEL axis (Regalia only; XP since 0.0.490). Spend up to `count` of the ledger
/// Dust `dust_id` on the real Regalia at `target_idx`: each Dust adds `regalia_dust_xp` to the
/// card's invested XP and the level follows its rung's curve. Takes no more Dust than reaching
/// the cap needs and no more than the ledger holds.
pub(super) fn enhance_regalia(state: &mut GameState, target_idx: usize, dust_id: &str, count: u32) {
    use crate::items::{regalia_level_of_xp, regalia_xp_to_level, ItemKind, REGALIA_MAX_LEVEL};
    let per = crate::items::regalia_dust_xp(dust_id);
    if per == 0 || count == 0 {
        return;
    }
    let Some(t) = state.owned_regalia.get(target_idx) else { return };
    if !matches!(t.kind, ItemKind::Regalia)
        || crate::items::regalia_fodder_levels(&t.def_id) > 0
        || crate::items::regalia_def(&t.def_id).is_none()
        || t.enhance_level >= REGALIA_MAX_LEVEL
    {
        return;
    }
    let Some(rung) = crate::items::regalia_rung_of(&t.def_id) else { return };
    let cap = regalia_xp_to_level(rung, REGALIA_MAX_LEVEL);
    let have = regalia_xp_to_level(rung, t.enhance_level).saturating_add(t.enhance_xp).min(cap);
    let target_member = t.equipped_by;
    let n = count.min((cap - have).div_ceil(per)).min(state.material_count(dust_id));
    if n == 0 || !state.debit_material(dust_id, n) {
        return;
    }
    let (level, rest) = regalia_level_of_xp(rung, have.saturating_add(n.saturating_mul(per)).min(cap));
    if let Some(t) = state.owned_regalia.get_mut(target_idx) {
        t.enhance_level = level;
        t.enhance_xp = rest;
    }
    state.measures.regalia_enhanced.add(1);
    if target_member != 0 {
        sync_member_regalia(state, target_member);
    }
}

/// SALVAGE the owned Regalia at `owned_idx` into Dust — only when `items::regalia_salvage_of`
/// allows it (not worn, not locked, an SSR only past S5). True when salvaged.
pub(super) fn salvage_regalia(state: &mut GameState, owned_idx: usize) -> bool {
    let Some(pay) = crate::items::regalia_salvage_of(&state.owned_regalia, owned_idx) else {
        return false;
    };
    state.owned_regalia.remove(owned_idx);
    state.measures.regalia_salvaged.add(1);
    for (id, n) in pay.tiers() {
        state.credit_material(id, n, crate::state::MaterialSource::Salvage);
    }
    true
}

/// Salvage every untouched 3★ card (`items::regalia_bulk_salvageable`; user 2026-09-28: bulk never
/// takes an SR, an SSR or a card the player invested in). Walks from the back so removal keeps the
/// indices still to visit valid.
pub(super) fn salvage_regalia_all(state: &mut GameState) {
    for i in (0..state.owned_regalia.len()).rev() {
        if crate::items::regalia_bulk_salvageable(&state.owned_regalia, i) {
            salvage_regalia(state, i);
        }
    }
}

/// Rebuild `entity_id`'s Regalia overlay from its equipped carrier. Twin of `sync_member_exemplar`.
fn sync_member_regalia(state: &mut GameState, entity_id: u32) {
    let Some(eidx) = state.grid.idx_of_id(entity_id) else {
        // A BENCHED wearer keeps its overlay on the roster copy (audit 2026-10-02): feed / enhance
        // re-synced only fielded members, so a benched unit came back with the old stars / level.
        let card = state.owned_regalia.iter().find(|c| c.equipped_by == entity_id)
            .map(|c| (c.resonance_stars.min(crate::items::REGALIA_MAX_STARS), c.enhance_level.min(crate::items::REGALIA_MAX_LEVEL)));
        if let Some(rc) = state.roster.get_mut(&entity_id) {
            match (card, rc.data.regalia.as_mut()) {
                (Some((stars, level)), Some(r)) => {
                    r.stars = stars;
                    r.level = level;
                }
                (None, Some(_)) => rc.data.regalia = None,
                _ => {}
            }
        }
        return;
    };
    let found = state
        .owned_regalia
        .iter()
        .find(|c| c.equipped_by == entity_id)
        .and_then(|c| crate::items::regalia_def(&c.def_id).map(|d| (d, c.resonance_stars, c.enhance_level)));
    let Some(e) = state.grid.entities.get_mut(eidx) else { return };
    match found {
        Some((def, stars, level)) => {
            let stars = stars.min(crate::items::REGALIA_MAX_STARS);
            let level = level.min(crate::items::REGALIA_MAX_LEVEL);
            if e.regalia.as_ref().is_some_and(|cur| cur.id == def.id) {
                if let Some(r) = &mut e.regalia {
                    r.stars = stars;
                    r.level = level;
                }
            } else {
                e.clear_regalia_skill();
                // Store the LEAN overlay (id/name/anim/stars/level); stats recompute from the def.
                e.regalia = Some(crate::items::Regalia {
                    id: def.id.clone(),
                    name: def.name.clone(),
                    signature_anim: def.signature_anim.clone(),
                    stars,
                    level,
                });
            }
            e.apply_regalia_skill();
        }
        None => {
            e.clear_regalia_skill();
            e.regalia = None;
        }
    }
    e.hp = e.hp.min(crate::combat::effective_max_hp(e));
}

/// Apply a SERVER-authored rare-drop grant (server-roll-drops-design.md): remove
/// pending candidate `seq` and bank its fixed item. A card's FOIL comes from the
/// nonce's high bits — server-authored, zero client PRNG draws. Silent no-op when
/// the candidate is gone (expired / already granted — `seq` is never reused, so a
/// duplicated event can't double-grant). The NONCE's legitimacy is the VERIFIER's
/// job (recomputed at save-verify); the sim only applies.
pub(super) fn grant_drop(state: &mut GameState, data: &GameData, seq: u64, server_roll: u64) {
    // Kind-guarded: a DROP grant resolves only a DROP candidate. See `GrantCraft`'s doc for
    // the cross-kind replay this closes.
    let Some(pos) = state
        .rare_candidates
        .iter()
        .position(|c| c.seq == seq && c.kind.is_drop())
    else {
        return;
    };
    let cand = state.rare_candidates.remove(pos);
    let Some(def) = data.item(&cand.def_id) else { return; };
    let mut item = crate::data::instantiate_item(def);
    if matches!(item.kind, crate::items::ItemKind::Card)
        && ((server_roll >> 32) % 10_000) < crate::drops::FOIL_DROP_CHANCE_BP as u64
    {
        item.foil = true;
    }
    crate::drops::bank_server_grant(state, item, true);
}

/// Apply a SERVER-authored CRAFT grant: remove the pending `CandidateKind::Craft`
/// candidate `seq` and bank its output, rolled to 1-4★ with matching bonus lines.
///
/// The whole roll runs on a PRNG seeded from `server_roll` alone, so it takes NO draw
/// from the state PRNG — the outcome is fixed by the server's nonce and replays
/// identically on every machine. Silent no-op when the candidate is gone or is a DROP
/// candidate (`seq` is never reused, so a duplicated event can't double-grant). The
/// nonce's legitimacy is the VERIFIER's job; the sim only applies.
///
/// A crafted weapon banks like a SHOP purchase, not like loot: the rare bag exists to keep
/// random rare drops out of the common-gear flood, and a piece the player deliberately paid
/// 30 materials for is not something they need protecting from.
pub(super) fn grant_craft(state: &mut GameState, data: &GameData, seq: u64, server_roll: u64) {
    // Matched EXACTLY, not as "anything that is not a Drop". It was the loose form once, and
    // adding the third kind silently made a craft nonce able to resolve a GEAR ticket — the
    // very cross-kind swap the separate nonce domains exist to stop. `each_grant_resolves_
    // only_its_own_kind` caught it the moment Gear landed; keep it exact.
    let Some(pos) = state
        .rare_candidates
        .iter()
        .position(|c| c.seq == seq && matches!(c.kind, crate::state::CandidateKind::Craft))
    else {
        return;
    };
    let cand = state.rare_candidates.remove(pos);
    let Some(def) = data.item(&cand.def_id) else { return; };
    // NOTHING IS ROLLED AT CRAFT EITHER (D1, 0.0.369): a crafted piece is the authored piece,
    // exactly like a dropped one. Rolling here while drops stay fixed would make the recipe the
    // only way to get a piece with lines — the opposite of "base gear is fungible".
    let item = crate::data::instantiate_item(def);
    let _ = server_roll;
    crate::drops::bank_server_grant(state, item, false);
}

/// Apply a SERVER-authored GEAR grant: the star, bonus lines and skill rune for a piece that
/// already dropped. Everything runs on a PRNG seeded from `server_roll` alone, so it takes no
/// draw from the state PRNG and replays identically everywhere.
///
/// Kind-guarded like the others, and it reproduces the OLD client-side order exactly — star,
/// then bonuses, then rune — so the only thing that changed about a dropped item is who rolled
/// it. Banks as LOOT (not as a purchase, unlike a craft): this is a random drop, so rare-bag
/// routing is exactly what it wants.
pub(super) fn grant_gear(state: &mut GameState, data: &GameData, seq: u64, server_roll: u64) {
    let Some(pos) = state.rare_candidates.iter().position(|c| {
        c.seq == seq && matches!(c.kind, crate::state::CandidateKind::Gear { .. })
    }) else {
        return;
    };
    let cand = state.rare_candidates.remove(pos);
    let boss = matches!(cand.kind, crate::state::CandidateKind::Gear { boss: true });
    let Some(def) = data.item(&cand.def_id) else { return; };
    // NOTHING IS ROLLED HERE ANY MORE (D1, 0.0.369). Raw gear is FIXED-STAT — its power is
    // (type × tier), authored in the catalog — so the server has nothing left to decide about a
    // gear drop: the ★ count, the bonus lines and the skill rune it used to roll are gone
    // (docs/gear-fusion-and-base-fix-design.md §1). The handler stays because the CANDIDATE
    // path still exists for cards/materials, and because a save mid-flight may still carry a
    // pending gear candidate: it now simply banks the authored piece.
    let item = crate::data::instantiate_item(def);
    let _ = (boss, server_roll);
    // The run summary counts what the RUN produced, and the piece only becomes real here —
    // tallying at drop time would have listed an item the player had not been given yet.
    if state.run_tally.loot.len() < crate::state::RUN_LOOT_SHOWN {
        state.run_tally.loot.push(item.clone());
    }
    crate::drops::bank_server_grant(state, item, true);
}

/// The player's own skill ids (not a character's). One id per ability; a zone
/// lists the ones it grants in `ZoneDef.player_skills`.
pub(crate) const REGENT_HAND: &str = "regent_hand";

/// PLAYER SKILLS (`regent_hand`, …) are the PLAYER's abilities rather than a
/// character's, and they are gated by CONTENT: today the only source is the
/// current zone's `player_skills` list, so a boss room can grant an interaction
/// the idle overworld never gets. A future Regent skill tree grants skills the
/// player OWNS everywhere — that becomes a second source checked right here, so
/// no call site has to learn about it.
pub(super) fn player_skill_available(state: &GameState, skill_id: &str) -> bool {
    // `regent_hand` is available EVERYWHERE — decided 2026-08-18, no longer a stand-in.
    //
    // It landed on 2026-08-14 as a temporary bypass so the 3D prototype's pick-up flow was
    // testable outside `zone_dungeon_duskmourn_4`, with a note saying the real version should ADD
    // a source rather than skip the check. The user then decided the everywhere behaviour is the
    // one they want: it is the Commander's own ability, not the room's. So this IS the second
    // source the doc comment above reserves room for — the Commander grants it, unconditionally,
    // and the zone list stays for every OTHER player skill.
    //
    // Zone grants are untouched: the `zone_spawn` lookup below still runs for everything else, so
    // content can still hand out an interaction the overworld does not have.
    //
    // Verified when the decision landed: no golden hash moved (39/39). Availability is consulted
    // when the input is APPLIED, not baked into the state at zone entry, so a save that never
    // lifts anyone hashes identically either way.
    //
    // `godot/proto/arena3d/arena3d.gd::_regent_hand_granted` carries the matching client-side
    // rule; `regent_hand_lifts_a_member_off_the_board_in_any_zone` is the test.
    if skill_id == REGENT_HAND {
        return true;
    }
    state
        .zone_spawn
        .as_ref()
        .is_some_and(|zs| zs.player_skills.iter().any(|s| s == skill_id))
}

/// `regent_hand` PICK UP: lift a party member off the board. Silent no-op unless
/// the zone grants the skill and the member is a living, non-pet, not-already-held
/// player off cooldown.
pub(super) fn pick_up_member(state: &mut GameState, entity_id: u32) {
    if !player_skill_available(state, REGENT_HAND) {
        return;
    }
    let tick = state.tick;
    if state.pickup_spent >= crate::grid::PICKUP_MAX_CHARGES {
        return; // the pool is empty — wait for a charge to come back
    }
    let Some(e) = state.grid.entities.iter_mut().find(|e| e.id == entity_id) else {
        return;
    };
    if !matches!(e.team, crate::grid::Team::Player) || !e.alive || e.is_pet || e.is_held() {
        return;
    }
    e.casting = None; // being lifted breaks a windup (RO: any interrupt drops the cast)
    e.held_until = tick + crate::grid::PICKUP_HOLD_MAX_TICKS;
}

/// `regent_hand` DROP: put the held member down on `(x, y)`. Every constraint —
/// in bounds, cell free, within `PICKUP_RANGE` of the pick-up cell — is enforced
/// HERE, in the sim, so a forged input can only do what a real drag could. A
/// failed check leaves the character held (the player can drop elsewhere, or let
/// the hold time out and drop in place).
pub(super) fn place_member(state: &mut GameState, entity_id: u32, x: i32, y: i32) {
    let cell = crate::grid::Cell::new(x, y);
    if !state.grid.in_bounds(cell) {
        return;
    }
    let Some(idx) = state.grid.entities.iter().position(|e| e.id == entity_id) else {
        return;
    };
    let Some(held) = state.grid.entities.get(idx) else { return };
    if !held.is_held() {
        return;
    }
    if held.pos.chebyshev(cell) > crate::grid::PICKUP_RANGE {
        return;
    }
    // Don't drop anyone on top of a living entity (held members are off the board
    // and so don't block a landing).
    if state
        .grid
        .entities
        .iter()
        .any(|e| e.alive && !e.is_held() && e.id != entity_id && e.pos == cell)
    {
        return;
    }
    drop_held_and_arm_cooldown(state, idx, Some(cell));
}

/// Land a held member: at `cell` when the player placed them, in place when the
/// hold timed out. Shared so both paths pay the SAME landing cost.
pub(crate) fn drop_held_member(e: &mut crate::grid::Entity, tick: u64, cell: Option<crate::grid::Cell>) {
    if let Some(c) = cell {
        e.pos = c;
        e.target = c;
        e.move_acc = 0;
    }
    e.target_id = None;
    e.held_until = 0;
    e.act_locked_until = tick + crate::grid::PICKUP_LAND_LOCK_TICKS;
}

/// Land a held member AND start the player's shared cooldown. The entity-only
/// `drop_held_member` exists for the auto-release path, which must not be able to
/// forget the cooldown either — both go through here.
pub(crate) fn drop_held_and_arm_cooldown(state: &mut GameState, idx: usize, cell: Option<crate::grid::Cell>) {
    let tick = state.tick;
    let Some(e) = state.grid.entities.get_mut(idx) else { return };
    drop_held_member(e, tick, cell);
    // Spend the charge on the DROP, not the lift: a hold that is still in the air has
    // not resolved yet, and charging on drop means a lift the player immediately puts
    // back still costs — no free repositioning by tapping.
    state.pickup_spent = (state.pickup_spent + 1).min(crate::grid::PICKUP_MAX_CHARGES);
    if state.pickup_recharge_at == 0 {
        state.pickup_recharge_at = tick + crate::grid::PICKUP_COOLDOWN_TICKS;
    }
}

/// The id `commander_smite` books its cooldown under. A `&str` key rather than an enum because
/// `commander_cd` is serialized state and an id survives a reorder; see the stored-index rule.
pub const COMMANDER_SMITE: &str = "commander_smite";
/// Ticks between casts, on TOP of the shared charge pool. 600 = 60 s.
pub const COMMANDER_SMITE_CD: u32 = 600;
/// Damage per point of party average level. Deliberately flat rather than a % of ATK: a Mage
/// party and a Swordsman party have wildly different ATK, so one multiplier would be a different
/// skill depending on the build. Measured against the catalog, `5 ×` holds 4-5% of a boss's HP
/// from Lv30 to the cap — see `docs/commander-skills-design.md`.
pub const COMMANDER_SMITE_PER_LEVEL: i32 = 5;
/// Footprint SIDE, not radius. 2 is the smallest even box, and even boxes only became expressible
/// when `grid::Blast` moved the shape test into half-cell coordinates.
pub const COMMANDER_SMITE_SIDE: i32 = 2;

/// How a Commander skill asks the player for its target.
///
/// The renderer dispatches its aim mode on this rather than on the skill ID, which is the whole
/// point of the type: a second skill that places a cell — or one that picks an ally — costs a row
/// in `COMMANDER_SKILLS` and no new input code. The four kinds are the ones
/// `docs/commander-skills-design.md` needs across all five designed skills; only the kinds an
/// implemented skill uses are reachable today.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum CommanderTarget {
    /// Point at a cell. The footprint is `side × side`, anchored by `grid::Blast::sided`.
    PlaceCell,
    /// Click a living, non-pet party member.
    PickAlly,
    /// Click a living monster.
    PickEnemy,
    /// No target at all — fires on press.
    Instant,
}

impl CommanderTarget {
    /// The wire name the view state carries. Written out rather than derived from the variant
    /// name, so renaming the variant in Rust cannot silently change the string GDScript matches.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::PlaceCell => "place_cell",
            Self::PickAlly => "pick_ally",
            Self::PickEnemy => "pick_enemy",
            Self::Instant => "instant",
        }
    }
}

/// One Commander skill, as the renderer needs to know it: what to call it, how long it is out for,
/// and how to ask for a target. Numbers only — the NAME and the icon are presentation and live in
/// the client's own table.
pub struct CommanderSkillDef {
    pub id: &'static str,
    pub cooldown: u32,
    pub target: CommanderTarget,
    /// Footprint side for `PlaceCell`; 0 for every other kind.
    pub side: i32,
}

/// Every Commander skill the player can actually cast, in BAR ORDER.
///
/// The client builds its skill bar straight from this list — one button per row, hotkey by
/// position — so the bar cannot offer a skill `apply_input` would refuse.
///
/// A skill belongs here when its HANDLER exists, not when its design does. The other four in
/// `docs/commander-skills-design.md` are designed and unbuilt, and adding them early would put
/// buttons on the bar that do nothing: the unreachable-feature shape inverted, and the player is
/// the one who finds it, by pressing it.
pub const COMMANDER_SKILLS: &[CommanderSkillDef] = &[CommanderSkillDef {
    id: COMMANDER_SMITE,
    cooldown: COMMANDER_SMITE_CD,
    target: CommanderTarget::PlaceCell,
    side: COMMANDER_SMITE_SIDE,
}];

/// `commander_smite`: a placed 2×2 blast for `5 × party average level` PURE damage.
///
/// Silent no-op on every failure — no charge, on cooldown, off the board. `apply_input` must never
/// panic, because a replay has to survive game_data version skew.
///
/// PURE (`DamageType::True`) on purpose. The whole reason the damage left `power_bp` was
/// predictability, and a normal physical hit hands that straight back: DEF would make the same
/// skill worth different amounts against a heavy-armour boss than a soft one, reintroducing the
/// build variance the flat value removes. `click_strike` is True for the same reason.
pub(crate) fn commander_smite(state: &mut GameState, x: i32, y: i32) {
    // UNLOCK GATE: smite is the first Commander unlock, not a starter — a cast before the player
    // owns it is a silent no-op (the bar does not draw it either; `view_state` filters the same
    // owned set). Kept FIRST so an unowned cast spends no charge (docs/commander-skills-design.md).
    if !state.commander_skills_owned.contains(COMMANDER_SMITE) {
        return;
    }
    if state.commander_charges == 0 || state.commander_cd.contains_key(COMMANDER_SMITE) {
        return;
    }
    let cell = crate::grid::Cell::new(x, y);
    if !state.grid.in_bounds(cell) {
        return;
    }
    // Charge and cooldown are spent BEFORE the scan, so a cast that finds nothing still costs.
    // Otherwise the cheapest way to play is to fire into empty space until something walks in.
    // Start the regen clock HERE when the pool leaves full, so a full pool needs no per-tick
    // write — the field stays at its default in every save that never casts, and the goldens with
    // it. Spending from an already-draining pool leaves the existing schedule alone.
    if state.commander_charges >= crate::state::COMMANDER_CHARGE_MAX {
        state.commander_charge_tick = state.tick;
    }
    state.commander_charges -= 1;
    state.commander_cd.insert(COMMANDER_SMITE.to_string(), COMMANDER_SMITE_CD);

    let dmg = COMMANDER_SMITE_PER_LEVEL * party_avg_level(state) as i32;
    let box_ = crate::grid::Blast::sided(cell, COMMANDER_SMITE_SIDE);
    // Collected first, then applied: `kill_monster` mutates `entities`, and a filter borrowing it
    // while the body writes to it would not compile — and indexing a shrinking list is the bug the
    // stored-index rule exists for.
    let victims: Vec<usize> = state
        .grid
        .entities
        .iter()
        .enumerate()
        .filter(|(_, e)| {
            e.alive && !matches!(e.team, crate::grid::Team::Player) && box_.contains(e.pos)
        })
        .map(|(j, _)| j)
        .collect();
    // Hoisted: the packet does not vary per victim, and building it in the loop invited the
    // reader to think it might.
    let pkt = crate::combat::Damage {
        atk_level: 1, amount: dmg,
        element: crate::grid::Element::Physical,
        kind: crate::grid::DamageType::True,
        is_ranged: false,
        is_crit: false,
        is_aoe: true,
        mastery_bp: 0,
        shadow_bane_bp: 0, debuff_amp_bp: 0,
        atk_race: crate::grid::Race::Unknown,
        atk_size: crate::grid::Size::Medium,
        ignore_def_bp: 0,
    };
    for idx in victims {
        // `.get()`, not `[idx]`, and NOT only to satisfy `deny(clippy::indexing_slicing)` — the
        // lint is right. `kill_monster` runs inside this loop for an earlier victim and nothing
        // here promises the list is the length it was when `victims` was collected. An index that
        // has gone stale must read as "skip", never as a panic: in verify-server, which links this
        // crate and replays uploaded saves under `panic = "abort"`, a panic is the process.
        //
        // Aliveness is re-checked for the same reason: an earlier death can change this one.
        let Some(target) = state.grid.entities.get(idx) else { continue };
        if !target.alive {
            continue;
        }
        let dealt = crate::combat::apply_damage(target, &pkt, 1);
        let Some(e) = state.grid.entities.get_mut(idx) else { continue };
        e.hp = (e.hp - dealt).max(0);
        let died = e.hp == 0;
        if died {
            e.alive = false;
            // Outside the `&mut e` borrow: `kill_monster` takes the whole state.
            crate::tick::kill_monster(state, idx, None);
        }
    }
}

/// Regenerate the shared Commander pool and age every Commander cooldown. Called once per tick,
/// beside `recharge_pickup`, because both are the PLAYER's resources rather than an entity's.
///
/// NOT merged with `regent_hand`'s own charges (`pickup_spent` / `pickup_recharge_at`), though the
/// design calls for one shared pool eventually. Folding a shipped ability into a new currency
/// would change how it behaves today and move goldens, for no gain until a second skill exists —
/// so the pool starts with the new skills and `regent_hand` migrates as its own decision.
///
/// Regen is a stamp comparison, not a countdown: `tick − commander_charge_tick >= INTERVAL`. The
/// stamp advances by exactly one interval per grant rather than being set to `tick`, so a pool
/// spent twice in quick succession still gets both charges on schedule.
///
/// A FULL POOL IS NOT TOUCHED HERE, and that is load-bearing rather than tidy. The obvious version
/// holds the clock at `now` every tick while full, so that a pool which sat full for an hour does
/// not bank an hour the instant it is spent — but writing the stamp every tick makes the field
/// non-default in every save, which serializes it, which moved TWELVE goldens. The clock is
/// stamped on the SPEND instead (`commander_smite`), which has the same effect and leaves a save
/// that never casts byte-identical to one from before this existed.
pub(crate) fn tick_commander(state: &mut GameState) {
    state.commander_cd.retain(|_, cd| {
        *cd = cd.saturating_sub(1);
        *cd > 0
    });
    if state.commander_charges >= crate::state::COMMANDER_CHARGE_MAX {
        return;
    }
    if state.tick.saturating_sub(state.commander_charge_tick) < crate::state::COMMANDER_CHARGE_TICKS
    {
        return;
    }
    state.commander_charges += 1;
    state.commander_charge_tick += crate::state::COMMANDER_CHARGE_TICKS;
}

/// Party average level: every non-pet Player entity, ALIVE OR DEAD (user, 2026-08-21).
///
/// Counting only the living would weaken the Commander exactly as a fight turns — and smite is
/// what a losing fight wants. A death already costs that member's full damage; taking a second
/// cut out of the Commander on top is a spiral rather than a difficulty curve. It also stops the
/// number flickering mid-fight, which would make it unexplainable to anyone watching it.
///
/// PETS ARE EXCLUDED and that is not cosmetic: `drops::party_max_level` filters `Team::Player` and
/// takes the MAX, where a pet is harmless — an AVERAGE is not, so a Ranger build would quietly do
/// less Commander damage than the same party without one. `Team::Player && !e.is_pet` is the
/// idiom the codebase already uses for "party member".
pub(crate) fn party_avg_level(state: &GameState) -> u32 {
    let mut sum: u64 = 0;
    let mut n: u64 = 0;
    for e in state.grid.entities.iter() {
        if matches!(e.team, crate::grid::Team::Player) && !e.is_pet {
            sum += e.stats.level as u64;
            n += 1;
        }
    }
    // `checked_div` rather than a zero guard: the guard and the division are then one expression
    // and cannot drift apart. An empty party is not reachable in a live game, but a replay of a
    // wiped save can hand this an entity list with no members.
    sum.checked_div(n).unwrap_or(1).max(1) as u32
}

/// Return spent `regent_hand` charges over time — one per `PICKUP_COOLDOWN_TICKS`,
/// started when the first was spent. Called once per tick.
pub(crate) fn recharge_pickup(state: &mut GameState) {
    if state.pickup_spent == 0 || state.pickup_recharge_at == 0 || state.tick < state.pickup_recharge_at {
        return;
    }
    state.pickup_spent -= 1;
    state.pickup_recharge_at = if state.pickup_spent > 0 {
        state.tick + crate::grid::PICKUP_COOLDOWN_TICKS
    } else {
        0
    };
}
