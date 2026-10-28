//! The SIM side of the mailbox: `ClaimMail` (claim = a server-authored payload enters the
//! inventory/Gil). It was built as half of the L1 Gil-market bridge
//! (docs/l1-commodity-market-scope.md); the market's trade side (list / buy, commodity and
//! per-item) was removed in 0.0.490 — its four InputEvents are tombstones.
//!
//! Deterministic (no PRNG → goldens unaffected) and total (invalid input = silent no-op). The
//! full-inventory guard on claim is the important safety: a claim that can't fit is a no-op, so
//! nothing is lost and the server keeps the mail claimable (consume-on-verified-save).

use crate::data::GameData;
use crate::state::{GameState, GilPeer, MarketOp};

/// The listing price cap of the removed Gil market. Nothing in the sim reads it any more; kept
/// only because the historical `godot-ext` bridge still exposes it to its (dead) market UI.
pub const MAX_LISTING_PRICE: u64 = 1_000_000_000;

/// Allocate the next monotonic market-op id (never reused → a stable server dedup key).
fn next_op_id(state: &mut GameState) -> u64 {
    let id = state.next_market_op_id;
    state.next_market_op_id = id.wrapping_add(1);
    id
}

/// `ClaimMail`: apply a mail payload (`gil` and/or a `def_id`×`qty` item) to GameState,
/// ALL-OR-NOTHING with a FULL-INVENTORY GUARD. If the item can't fit (needs a new slot and the
/// inventory is full) the WHOLE claim is a no-op — Gil included — so nothing is lost and the
/// mail stays claimable. A stackable merging into an existing stack always fits. A Gil-only mail
/// always applies (Gil has no slot cap). Returns whether the payload was applied, so the SERVER
/// consumes the mail only on apply (exactly-once = at-most-once via the consumed set + at-least-
/// once because a non-apply leaves the mail claimable).
pub fn claim_mail(
    state: &mut GameState,
    data: &GameData,
    mail_id: u64,
    gil: crate::state::Gil,
    def_id: &str,
    qty: u32,
    item: Option<crate::items::Item>,
) -> bool {
    // One claim per mail (audit 2026-10-02): a second ClaimMail for a mail this state already
    // holds a claim op for is a no-op — the client applied it twice (two windows), and the server
    // would reject the whole save as a duplicate claim.
    if state.market_ops.iter().any(|op| matches!(op, MarketOp::Claim { mail_id: m, .. } if *m == mail_id)) {
        return false;
    }
    // Grant the payload FIRST, all-or-nothing: a FULL item instance (item market) OR a stackable
    // (`def_id`×`qty`, commodity). If it can't be banked, apply NOTHING (Gil included).
    if let Some(it) = &item {
        if !crate::data::try_grant_item(state, data, it.clone()) {
            return false; // full → no-op, mail stays claimable
        }
    } else if !def_id.is_empty() && qty > 0 {
        if !crate::data::try_grant(state, data, def_id, qty) {
            return false; // full / unknown def → no-op, mail stays claimable
        }
    }
    state.receive_gil(gil.get(), GilPeer::MailPayload);
    // Record the APPLIED claim + the payload the sim applied, so the server can verify it against
    // the real mail (a forged payload passes save-verify, so the reconcile must compare). A no-op
    // above records nothing → the mail stays claimable, no loss.
    let id = next_op_id(state);
    state.market_ops.push(MarketOp::Claim { id, mail_id, gil, def_id: def_id.to_string(), qty, item });
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::materials::count_stackable;
    use crate::data::GameData;
    use crate::items::{Item, ItemKind, Rarity};

    fn game_data() -> GameData {
        GameData::from_json_str(include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/../../data/game_data.json")))
            .expect("game_data.json must parse")
    }

    fn herb_stack(qty: u32) -> Item {
        let mut it = Item::new("red_herb", ItemKind::Material, 0, 0, Rarity::Common);
        it.quantity = qty;
        it
    }

    #[test]
    fn claim_full_item_banks_the_exact_instance() {
        let d = game_data();
        let mut s = GameState::new(1, 8, 8);
        let mut sword = Item::new("sword_short", ItemKind::Weapon, 12, 0, Rarity::Common);
        sword.star = 3;
        // A full-item claim banks the EXACT instance (stats + per-instance state preserved),
        // and records the op.
        assert!(claim_mail(&mut s, &d, 42, crate::state::Gil::new(0), "", 0, Some(sword.clone())));
        let got = s.inventory.iter().find(|i| i.def_id == "sword_short").expect("banked");
        assert_eq!(got.atk_bonus, 12);
        assert_eq!(got.star, 3, "the exact instance's earned state is preserved");
        assert!(matches!(&s.market_ops[0], MarketOp::Claim { mail_id: 42, item: Some(_), .. }));
    }

    #[test]
    fn claim_gil_only_always_applies() {
        let d = game_data();
        let mut s = GameState::new(1, 8, 8);
        let g0 = s.gil();
        assert!(claim_mail(&mut s, &d, 5, crate::state::Gil::new(76), "", 0, None));
        assert_eq!(s.gil(), g0 + 76);
        assert!(matches!(&s.market_ops[0], MarketOp::Claim { mail_id: 5, .. }), "applied claim records the mail id");
    }

    #[test]
    fn claim_item_merges_into_existing_stack_even_when_slots_full() {
        let d = game_data();
        let mut s = GameState::new(1, 8, 8);
        // Fill EVERY slot with gear, but keep one held herb stack among them.
        s.inventory.push(herb_stack(5));
        for k in 1..s.inventory_cap() {
            s.inventory.push(Item::new(&format!("g{k}"), ItemKind::Weapon, 1, 0, Rarity::Common));
        }
        assert_eq!(s.inventory.len(), s.inventory_cap(), "inventory full");
        // A herb return MERGES into the existing stack → fits even at cap.
        assert!(claim_mail(&mut s, &d, 7, crate::state::Gil::new(10), "red_herb", 6, None));
        assert_eq!(count_stackable(&s, "red_herb"), 11, "merged 6 herb");
        assert_eq!(s.gil(), 10, "gil applied too");
        assert_eq!(s.market_ops.len(), 1, "applied claim records an op");
    }

    #[test]
    fn claim_new_item_blocked_when_inventory_full_loses_nothing() {
        let d = game_data();
        let mut s = GameState::new(1, 8, 8);
        // Fill every slot with gear (NO herb stack to merge into).
        for k in 0..s.inventory_cap() {
            s.inventory.push(Item::new(&format!("g{k}"), ItemKind::Weapon, 1, 0, Rarity::Common));
        }
        let g0 = s.gil();
        // A herb return needs a NEW slot → full → the WHOLE claim no-ops (gil included).
        assert!(!claim_mail(&mut s, &d, 9, crate::state::Gil::new(10), "red_herb", 6, None), "full → claim blocked");
        assert_eq!(count_stackable(&s, "red_herb"), 0, "no herb granted");
        assert_eq!(s.gil(), g0, "gil NOT applied (all-or-nothing) → mail stays fully claimable");
        assert!(s.market_ops.is_empty(), "a blocked claim records NOTHING → server won't consume the mail");
    }

    /// A Potential material is a LEDGER count, never a bag stack (0.0.354): it drops into the
    /// ledger, lists on the commodity book from the ledger, arrives by mail into the ledger even
    /// at a full bag, and a pre-ledger save's bag stacks move over on load. The market seams see
    /// one kind of stock either way.
    #[test]
    fn potential_materials_live_in_the_ledger_across_drop_mail_and_load() {
        let d = game_data();
        let mut s = GameState::new(1, 8, 8);
        for k in 0..s.inventory_cap() {
            s.inventory.push(Item::new(&format!("g{k}"), ItemKind::Weapon, 1, 0, Rarity::Common));
        }
        // DROP: banks into the ledger and takes no slot even though the bag is full.
        let mut drop = crate::data::instantiate_item(d.item("sigil_1").expect("catalog"));
        drop.quantity = 5;
        crate::drops::bank_loot_drop(&mut s, drop);
        assert_eq!(s.material_count("sigil_1"), 5);
        assert!(!s.inventory.iter().any(|it| it.def_id == "sigil_1"), "never a bag stack");
        assert_eq!(count_stackable(&s, "sigil_1"), 5, "held stock counts the ledger");
        // MAIL: a delivery lands in the ledger at a full bag — the claim is never blocked.
        assert!(claim_mail(&mut s, &d, 7, crate::state::Gil::new(0), "sigil_2", 4, None));
        assert_eq!(s.material_count("sigil_2"), 4);
        assert_eq!(s.inventory.len(), s.inventory_cap(), "no slot was taken");
        // LOAD: a pre-ledger save held these as bag stacks under the old kind.
        let mut old = GameState::new(2, 8, 8);
        let mut legacy = Item::new("track", ItemKind::Material, 0, 0, Rarity::Legendary);
        legacy.quantity = 2;
        old.inventory.push(legacy);
        crate::data::migrate_save_items(&mut old, &d);
        assert_eq!(old.material_count("track"), 2, "moved into the ledger");
        assert!(old.inventory.is_empty(), "and the slot was freed");
    }

    /// A gacha result arrives as MAIL, and a paid pull must never be lost to a full bag.
    /// Collectibles bypass the inventory entirely (`bank_collectible`). An EXEMPLAR is a UNIT
    /// (0.0.350): the mailed item is only the envelope — the first copy becomes a benched Lv1
    /// unit, a later copy a Resonance credit, and the envelope is never kept. A Regalia is still
    /// a carrier and banks into its collection.
    #[test]
    fn mailed_collectibles_never_need_the_bag_and_an_exemplar_arrives_as_a_unit() {
        let d = game_data();
        let mut s = GameState::new(1, 8, 8);
        for k in 0..s.inventory_cap() {
            s.inventory.push(Item::new(&format!("g{k}"), ItemKind::Weapon, 1, 0, Rarity::Common));
        }
        let full = s.inventory.len();

        let ex = Item::new("exemplar_seraphine", ItemKind::Exemplar, 0, 0, Rarity::Legendary);
        assert!(claim_mail(&mut s, &d, 1, crate::state::Gil::new(0), "", 0, Some(ex)), "claim succeeds on a FULL bag");
        assert_eq!(s.roster.len(), 1, "the first copy is a benched UNIT");
        let unit = s.roster.values().next().unwrap();
        assert_eq!(unit.data.exemplar.as_ref().map(|x| x.id.as_str()), Some("seraphine"));
        assert!(s.owned_exemplars.is_empty(), "the envelope is not kept as a carrier");

        let rg = Item::new("regalia_aurora_edge", ItemKind::Regalia, 0, 0, Rarity::Legendary);
        assert!(claim_mail(&mut s, &d, 2, crate::state::Gil::new(0), "", 0, Some(rg)), "same for a Regalia");
        assert_eq!(s.owned_regalia.len(), 1);

        assert_eq!(s.inventory.len(), full, "neither touched the bag");

        // A dupe becomes a Resonance credit the PLAYER spends later — never a second unit.
        let dupe = Item::new("exemplar_seraphine", ItemKind::Exemplar, 0, 0, Rarity::Legendary);
        assert!(claim_mail(&mut s, &d, 3, crate::state::Gil::new(0), "", 0, Some(dupe)));
        assert_eq!(s.roster.len(), 1, "still one unit");
        assert_eq!(s.pending_resonance["seraphine"], 1, "one credit banked");
    }

    #[test]
    fn bound_stacks_never_merge_with_unbound() {
        let mut s = GameState::new(1, 8, 8);
        let mut b = herb_stack(5);
        b.bound = true;
        crate::data::try_grant_bag_item(&mut s, b);
        crate::data::try_grant_bag_item(&mut s, herb_stack(7));
        assert_eq!(s.inventory.len(), 2, "bound and unbound herbs occupy separate stacks");
        let unbound: u32 = s.inventory.iter().filter(|it| !it.bound).map(|it| it.quantity).sum();
        assert_eq!(unbound, 7);
        assert_eq!(count_stackable(&s, "red_herb"), 12);
    }
}
