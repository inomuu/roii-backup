//! STACKABLE MATERIALS — counting and consuming the fungible stock a player holds.
//!
//! "Held" is two places at once: the materials LEDGER (`GameState.materials`, a count) and
//! stacks sitting in the bag. Every caller that spends a material has to look at both, in the
//! same order, or the same unit gets spent twice. These helpers are that single rule.
//!
//! They lived in `refine.rs` until 0.0.370 — refine was simply their first caller. Crafting,
//! the market listing path and the Sigil merge all use them, so they outlived it.

use crate::state::GameState;

/// Total units of stackable `def_id` held — the materials ledger (ore, since 0.0.365) plus the
/// bag's stacks.
pub(crate) fn count_stackable(state: &GameState, def_id: &str) -> u32 {
    let bag: u32 = state
        .inventory
        .iter()
        .filter(|it| it.def_id == def_id && it.is_stackable())
        .map(|it| it.quantity)
        .sum();
    state.material_count(def_id).saturating_add(bag)
}

/// Consume `n` units of stackable `def_id`, spending BOUND stacks FIRST and the
/// unbound remainder second. Returns how many BOUND units were taken; a non-zero
/// result means a bound material fed the craft/refine, so the caller MUST bind the
/// output — bound-first consumption + bound output closes the laundering channel
/// where a free pass material's value leaks into a tradeable item (audit H1/H2,
/// docs/economy-invariants.md). It also clears a player's bound clutter before
/// their sellable stock. Caller MUST verify `count_stackable >= n` first.
pub(crate) fn consume_n_stackable_bound_first(state: &mut GameState, def_id: &str, n: u32) -> u32 {
    let bound_taken = consume_stackable_pass(state, def_id, n, true);
    // Then the ledger (never bound), then the bag's unbound stacks.
    let mut left = n - bound_taken;
    let from_ledger = left.min(state.material_count(def_id));
    if from_ledger > 0 && state.debit_material(def_id, from_ledger) {
        left -= from_ledger;
    }
    consume_stackable_pass(state, def_id, left, false);
    bound_taken
}

/// One consumption pass over stacks whose `bound` flag equals `want_bound`.
/// Returns the units taken. Shared by `consume_n_stackable_bound_first`.
fn consume_stackable_pass(state: &mut GameState, def_id: &str, mut n: u32, want_bound: bool) -> u32 {
    let mut taken = 0u32;
    let mut i = 0;
    while i < state.inventory.len() && n > 0 {
        let Some(it) = state.inventory.get_mut(i) else { break };
        if it.def_id == def_id && it.is_stackable() && it.bound == want_bound {
            let take = n.min(it.quantity);
            it.quantity -= take;
            n -= take;
            taken += take;
            if it.quantity == 0 {
                state.inventory.remove(i);
                continue;
            }
        }
        i += 1;
    }
    taken
}

