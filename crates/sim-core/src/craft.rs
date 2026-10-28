//! Crafting — turn stackable materials + Gil into an output item via a catalog recipe.
//!
//! Deterministic (no PRNG draw → goldens unaffected) and self-cost-gated: the player
//! spends their OWN materials + Gil, so `CraftItem` is `InputAuthority::Client` (safe to
//! accept; the supply of materials/Gil bounds it). All-or-nothing: the full cost is verified
//! BEFORE any deduction, so a partial craft can never strand ingredients. Invalid input
//! (unknown recipe / insufficient inputs) is a silent no-op (replay-safe, never panics).
//!
//! Still no PRNG draw here even now that GEAR output is rolled: the craft only accrues a
//! pending candidate, and the star + bonus lines come out of the SERVER's nonce
//! (`grant_craft`), never the state PRNG.

use crate::data::GameData;
use crate::materials::count_stackable;
use crate::state::{GameState, GilSpend};

/// Apply a `LearnRecipe { recipe_id }` event. One-time knowledge purchase: spend the
/// recipe's `learn_soul` + `learn_gil`, add it to `recipes_learned`. No-op if the recipe
/// is unknown, already learned, or either currency is short (atomic — both checked before
/// either is deducted). Learned recipes are party-shared and survive a wipe (like `soul`).
pub fn learn_recipe(state: &mut GameState, data: &GameData, recipe_id: &str) {
    if !data.features.craft {
        return;
    }
    let Some(recipe) = data.recipe(recipe_id) else {
        return; // unknown recipe
    };
    if state.recipes_learned.contains(recipe_id) {
        return; // already learned
    }
    if state.soul() < recipe.learn_soul || state.gil() < recipe.learn_gil {
        return;
    }
    if !state.spend_soul(recipe.learn_soul, crate::state::SoulSpend::CraftLearn) {
        return;
    }
    state.spend_gil(recipe.learn_gil, GilSpend::CraftLearn);
    state.recipes_learned.insert(recipe_id.to_string());
}

/// RETIRED (0.0.382): no recipe output is server-rolled any more — gear is fixed-stat (D1) and
/// crafts grant on the spot. Always `false`; kept so the craft screens' `server_rolled` key keeps
/// its shape (a client that still reads it simply never sees the "needs a server" band).
pub fn output_is_rolled_gear(_data: &GameData, _output_def_id: &str) -> bool {
    false
}

/// Apply a `CraftItem { recipe_id }` event. No-op unless the recipe exists, has been
/// LEARNED (`LearnRecipe`), AND the player holds every ingredient + the Gil; then consume
/// the inputs and grant `output_qty` of the output (stackables merge into the inventory).
///
/// GEAR output takes the long way round: it accrues a `CandidateKind::Craft` candidate the
/// SERVER resolves into a star + bonus lines (`grant_craft`), because a 4★ output is
/// Legendary and a client must never roll its own jackpot. Everything else is granted here
/// and now, exactly as before.
pub fn craft_item(state: &mut GameState, data: &GameData, recipe_id: &str) {
    if !data.features.craft {
        return;
    }
    let Some(recipe) = data.recipe(recipe_id) else {
        return; // unknown recipe
    };
    if !state.recipes_learned.contains(recipe_id) {
        return; // not learned
    }
    // Cost-gate FIRST (atomic): every input must be affordable before anything is deducted.
    if state.gil() < recipe.gil_cost {
        return;
    }
    for ing in &recipe.ingredients {
        if count_stackable(state, &ing.def_id) < ing.qty {
            return;
        }
    }
    // Candidate-list ROOM is part of that same gate. `drop_on_death` may silently skip a
    // candidate at the cap because a dud lottery ticket costs nothing; a craft costs 30
    // materials, so running out of room has to refuse the craft outright rather than eat
    // the inputs and grant nothing.
    //
    // Counted against the CRAFT budget, not the drop lottery's. Sharing one cap made this
    // refusal fire constantly: a farmed account sits near the drop cap by design, so the
    // Craft button silently did nothing on exactly the accounts the recipes are for.
    // Deduct, then grant. (Clone the small recipe parts we need so the `data` borrow doesn't
    // conflict with the `&mut state` grants below.)
    state.measures.items_crafted.add(1);
    let gil_cost = recipe.gil_cost;
    let ingredients: Vec<(String, u32)> =
        recipe.ingredients.iter().map(|i| (i.def_id.clone(), i.qty)).collect();
    let output = recipe.output_def_id.clone();
    let output_qty = recipe.output_qty;

    state.spend_gil(gil_cost, GilSpend::Craft);
    // Consume BOUND ingredients first (clears a player's pass clutter before their
    // sellable stock), but the output stays TRADEABLE — the player can't pick which stack
    // feeds a craft, so surprise-binding the result is worse than the negligible bound-mat
    // laundering (in-game market; gate by price if abused) — user decision 2026-07-24.
    for (def_id, qty) in &ingredients {
        crate::materials::consume_n_stackable_bound_first(state, def_id, *qty);
    }
    // Gear and everything else alike, here and now (0.0.382). Gear used to accrue a
    // `CandidateKind::Craft` ticket the server resolved into a star + lines; with fixed-stat gear
    // (D1) the server had nothing left to decide, and the round trip was pure latency.
    for _ in 0..output_qty {
        crate::data::grant_loot(state, data, &output);
    }
}

#[cfg(test)]
mod tests {
    use crate::data;
    use crate::events::{apply_input_with, InputEvent};

    const SHIPPED: &str = include_str!("../../../data/game_data.json");

    /// The real catalog with the craft switch ON — these tests are about how craft works when it
    /// ships; the shipped catalog holds it back (`craft_is_held_back_by_the_catalog_switch`).
    fn game_data() -> data::GameData {
        data::GameData::from_json_str(&SHIPPED.replacen("\"craft\": false", "\"craft\": true", 1))
            .expect("game_data.json must parse")
    }

    /// Craft held back (user 2026-10-02): learning and crafting are no-ops, the craft-only
    /// materials are not sold, and a catalog without the switch keeps craft on.
    #[test]
    fn craft_is_held_back_by_the_catalog_switch() {
        let off = data::GameData::from_json_str(SHIPPED).expect("catalog");
        assert!(!off.features.craft, "the shipped catalog holds craft back");
        let on = game_data();
        assert!(on.features.craft);
        let mut s = data::new_game_in_zone(1, &on, "zone_b1_1").unwrap();
        s.set_gil_for_test(1_000_000);
        s.set_soul_for_test(1_000_000);
        apply_input_with(&mut s, Some(&off), InputEvent::LearnRecipe { recipe_id: "brew_red_phial".into() });
        assert!(s.recipes_learned.is_empty(), "no recipe learned while craft is off");
        apply_input_with(&mut s, Some(&on), InputEvent::LearnRecipe { recipe_id: "brew_red_phial".into() });
        assert!(s.recipes_learned.contains("brew_red_phial"));
        let gil = s.gil();
        apply_input_with(&mut s, Some(&off), InputEvent::CraftItem { recipe_id: "brew_red_phial".into() });
        assert_eq!(s.gil(), gil, "no craft while craft is off");
        let herb = off.item("red_herb").expect("red_herb");
        assert!(!data::sells_for_gil(&off, herb), "a craft-only material is not sold");
        assert!(data::sells_for_gil(&on, on.item("red_herb").expect("def")), "switch on: sold again");
        let phial = off.item("hp_pot_1").expect("phial");
        assert!(data::sells_for_gil(&off, phial), "the phial itself stays in the shop");
        assert!(data::Features::default().craft);
    }

    /// Holding the inputs → the LEARNED recipe consumes them + Gil and grants the batch output.
    #[test]
    fn craft_consumes_inputs_and_grants_batch() {
        let d = game_data();
        let recipe = d.recipes.first().expect("at least one recipe seeded").clone();
        let mut s = data::new_game_in_zone(1, &d, "zone_b1_1").unwrap();
        s.recipes_learned.insert(recipe.id.clone());
        // Grant exactly the inputs + enough Gil.
        for ing in &recipe.ingredients {
            for _ in 0..ing.qty {
                data::grant_loot(&mut s, &d, &ing.def_id);
            }
        }
        s.set_gil_for_test(recipe.gil_cost + 100);
        let out_before = crate::materials::count_stackable(&s, &recipe.output_def_id);

        apply_input_with(&mut s, Some(&d), InputEvent::CraftItem { recipe_id: recipe.id.clone() });

        assert_eq!(s.gil(), 100, "Gil cost deducted");
        for ing in &recipe.ingredients {
            assert_eq!(crate::materials::count_stackable(&s, &ing.def_id), 0, "ingredient consumed");
        }
        assert_eq!(
            crate::materials::count_stackable(&s, &recipe.output_def_id),
            out_before + recipe.output_qty,
            "batch output granted",
        );
    }

    /// A BOUND ingredient is consumed first, but the output stays TRADEABLE — the player
    /// can't pick which stack feeds the craft, so no surprise-bind (user decision 2026-07-24).
    #[test]
    fn craft_from_bound_ingredient_spends_bound_first_but_output_is_tradeable() {
        use crate::items::{Item, ItemKind, Rarity};
        let d = game_data();
        let recipe = d.recipes.first().unwrap().clone();
        let mut s = data::new_game_in_zone(1, &d, "zone_b1_1").unwrap();
        s.recipes_learned.insert(recipe.id.clone());
        s.set_gil_for_test(recipe.gil_cost + 100);
        // Each ingredient: a BOUND stack of exactly the needed qty PLUS a spare unbound
        // stack that must survive (proving the bound one is spent first).
        for ing in &recipe.ingredients {
            let mut b = Item::new(&ing.def_id, ItemKind::Material, 0, 0, Rarity::Common);
            b.quantity = ing.qty;
            b.bound = true;
            s.inventory.push(b);
            let mut u = Item::new(&ing.def_id, ItemKind::Material, 0, 0, Rarity::Common);
            u.quantity = ing.qty;
            s.inventory.push(u);
        }

        apply_input_with(&mut s, Some(&d), InputEvent::CraftItem { recipe_id: recipe.id.clone() });

        for ing in &recipe.ingredients {
            assert_eq!(
                crate::materials::count_stackable(&s, &ing.def_id), ing.qty,
                "the unbound spare survives — bound stack consumed first",
            );
            assert!(
                s.inventory.iter().filter(|it| it.def_id == ing.def_id).all(|it| !it.bound),
                "no bound ingredient remains",
            );
        }
        let out: Vec<&Item> = s.inventory.iter()
            .filter(|it| it.def_id == recipe.output_def_id).collect();
        assert!(!out.is_empty(), "output granted");
        assert!(out.iter().all(|it| !it.bound), "output stays tradeable even when a bound input fed it");
    }

    /// Missing an ingredient (or Gil) → total no-op: nothing consumed, nothing granted.
    #[test]
    fn craft_without_inputs_is_a_noop() {
        let d = game_data();
        let recipe = d.recipes.first().unwrap().clone();
        let mut s = data::new_game_in_zone(1, &d, "zone_b1_1").unwrap();
        s.recipes_learned.insert(recipe.id.clone());
        s.set_gil_for_test(recipe.gil_cost + 100);
        // No ingredients granted.
        let out_before = crate::materials::count_stackable(&s, &recipe.output_def_id);

        apply_input_with(&mut s, Some(&d), InputEvent::CraftItem { recipe_id: recipe.id.clone() });

        assert_eq!(s.gil(), recipe.gil_cost + 100, "no Gil spent when inputs are missing");
        assert_eq!(
            crate::materials::count_stackable(&s, &recipe.output_def_id),
            out_before,
            "no output granted",
        );
    }

    /// UNLEARNED recipe → craft is a total no-op even with all inputs in hand.
    #[test]
    fn craft_unlearned_is_a_noop() {
        let d = game_data();
        let recipe = d.recipes.first().unwrap().clone();
        let mut s = data::new_game_in_zone(1, &d, "zone_b1_1").unwrap();
        for ing in &recipe.ingredients {
            for _ in 0..ing.qty {
                data::grant_loot(&mut s, &d, &ing.def_id);
            }
        }
        s.set_gil_for_test(recipe.gil_cost + 100);
        let out_before = crate::materials::count_stackable(&s, &recipe.output_def_id);

        apply_input_with(&mut s, Some(&d), InputEvent::CraftItem { recipe_id: recipe.id.clone() });

        assert_eq!(s.gil(), recipe.gil_cost + 100, "no Gil spent");
        for ing in &recipe.ingredients {
            assert_eq!(
                crate::materials::count_stackable(&s, &ing.def_id),
                ing.qty,
                "ingredients untouched",
            );
        }
        assert_eq!(
            crate::materials::count_stackable(&s, &recipe.output_def_id),
            out_before,
            "no output granted",
        );
    }

    /// Craft a full recipe's worth of inputs into whatever it makes, and hand back the state.
    fn craft(recipe_id: &str) -> (data::GameData, crate::state::GameState) {
        let d = game_data();
        let recipe = d.recipe(recipe_id).expect("recipe exists").clone();
        let mut s = data::new_game_in_zone(1, &d, "zone_b1_1").unwrap();
        s.recipes_learned.insert(recipe.id.clone());
        for ing in &recipe.ingredients {
            for _ in 0..ing.qty {
                data::grant_loot(&mut s, &d, &ing.def_id);
            }
        }
        s.set_gil_for_test(recipe.gil_cost);
        apply_input_with(&mut s, Some(&d), InputEvent::CraftItem { recipe_id: recipe.id.clone() });
        (d, s)
    }

    /// GEAR is granted here and now, exactly like a potion (0.0.382): nothing is rolled on a
    /// crafted piece (D1), so the server round-trip that used to carry it had nothing to decide.
    #[test]
    fn crafting_gear_grants_the_authored_piece_instantly() {
        let (d, s) = craft("forge_sword_s3");
        assert_eq!(s.gil(), 0, "the craft ran — Gil spent");
        assert!(s.rare_candidates.is_empty(), "no server ticket for a craft any more");
        let w = s.inventory.iter().find(|i| i.def_id == "sword_s3").expect("weapon banked now");
        assert_eq!(w.star, 0, "nothing is rolled at craft (D1)");
        assert!(w.bonuses.is_empty(), "no rolled lines — the authored lines come from the grade");
        assert_eq!(w.rarity, d.item("sword_s3").expect("def").rarity, "rarity is the def's");
    }

    /// A POTION recipe lands immediately too.
    #[test]
    fn crafting_a_potion_still_grants_instantly() {
        let (_d, s) = craft("brew_red_phial");
        assert!(s.rare_candidates.is_empty(), "no candidate for a consumable");
        assert!(crate::materials::count_stackable(&s, "hp_pot_1") > 0, "potions banked now");
    }

    /// An IN-FLIGHT craft ticket from an older save (one accrued before 0.0.382) is still
    /// honoured by `GrantCraft`: the authored piece banks, the ticket is consumed, a replay of
    /// the same seq is a no-op, and a DROP grant cannot resolve it (the kind guard).
    #[test]
    fn an_old_craft_ticket_still_resolves_and_only_to_its_own_kind() {
        let d = game_data();
        let mut s = data::new_game_in_zone(1, &d, "zone_b1_1").unwrap();
        s.rare_candidate_seq = 41;
        s.rare_candidates.push(crate::state::RareCandidate {
            seq: 41, def_id: "sword_s3".into(), tick: 0,
            kind: crate::state::CandidateKind::Craft, hit_ppm: 0,
        });
        apply_input_with(&mut s, Some(&d), InputEvent::GrantDrop { candidate_seq: 41, server_roll: 12_345 });
        assert_eq!(s.rare_candidates.len(), 1, "a drop grant cannot resolve a craft ticket");
        let rng_before = s.rng.clone();
        apply_input_with(&mut s, Some(&d), InputEvent::GrantCraft { candidate_seq: 41, server_roll: 12_345 });
        assert_eq!(s.rng, rng_before, "the grant takes NO draw from the state PRNG");
        assert!(s.rare_candidates.is_empty(), "ticket consumed");
        let n = s.inventory.iter().filter(|i| i.def_id == "sword_s3").count();
        assert_eq!(n, 1, "the authored piece banked once");
        apply_input_with(&mut s, Some(&d), InputEvent::GrantCraft { candidate_seq: 41, server_roll: 12_345 });
        assert_eq!(s.inventory.iter().filter(|i| i.def_id == "sword_s3").count(), 1, "no double-grant");
    }

    /// LearnRecipe spends soul + Gil exactly once; a repeat is a no-op; short
    /// currency (either one) is an atomic no-op.
    #[test]
    fn learn_recipe_spends_once_and_gates() {
        let d = game_data();
        let recipe = d.recipes.first().unwrap().clone();
        let mut s = data::new_game_in_zone(1, &d, "zone_b1_1").unwrap();

        // Short on soul → no-op (Gil untouched).
        s.set_soul_for_test(recipe.learn_soul.saturating_sub(1));
        s.set_gil_for_test(recipe.learn_gil + 7);
        apply_input_with(&mut s, Some(&d), InputEvent::LearnRecipe { recipe_id: recipe.id.clone() });
        assert!(!s.recipes_learned.contains(&recipe.id), "not learned when soul is short");
        assert_eq!(s.gil(), recipe.learn_gil + 7, "no Gil spent on a failed learn");

        // Exact cost → learned, both currencies deducted.
        s.set_soul_for_test(recipe.learn_soul);
        s.set_gil_for_test(recipe.learn_gil + 7);
        apply_input_with(&mut s, Some(&d), InputEvent::LearnRecipe { recipe_id: recipe.id.clone() });
        assert!(s.recipes_learned.contains(&recipe.id), "learned");
        assert_eq!(s.soul(), 0, "soul spent");
        assert_eq!(s.gil(), 7, "Gil spent");

        // Repeat → no-op (nothing double-charged).
        s.set_soul_for_test(recipe.learn_soul);
        s.set_gil_for_test(recipe.learn_gil);
        apply_input_with(&mut s, Some(&d), InputEvent::LearnRecipe { recipe_id: recipe.id.clone() });
        assert_eq!(s.soul(), recipe.learn_soul, "repeat learn spends nothing");
        assert_eq!(s.gil(), recipe.learn_gil, "repeat learn spends nothing");
    }
}
