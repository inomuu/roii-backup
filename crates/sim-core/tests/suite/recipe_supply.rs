//! Can a player actually MAKE the things the recipe book lists?
//!
//! Three times in one session a recipe shipped that no player could ever craft — not because an
//! ingredient was missing from the catalog, but because its ingredients never share a zone. Every
//! one was found by hand; the 467-test suite reported green through all three, because nothing in
//! it ever asked "at what RATE does this accrue, and from where".
//!
//! So this file measures. It walks the real `game_data.json`, computes each material's yield per
//! hour of farming IN EACH ZONE from that zone's spawn pool and the monsters' own drop tables,
//! and reports what a bench can produce standing in the best zone for it. A recipe whose two
//! materials live in different bands scores ZERO here — that is the shape of the bug.
//!
//! It is also the pair to the recipe QUANTITIES. Consumption and supply were tuned against each
//! other (see the 0.0.268 changelog); they live in different parts of the same file with nothing
//! binding them, so they are asserted together here and fail together, which is the only time
//! either failing is useful.

use sim_core::data::{self, CraftStation};
use std::collections::HashMap;

fn game_data() -> data::GameData {
    data::GameData::from_json_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../data/game_data.json"
    )))
    .expect("game_data.json must parse")
}

/// Kills per hour for a five-member party. MEASURED END TO END: 181,175 kills over a 168h
/// sim-cli run (`--seed 42 --ticks 6048000`), steady across the whole week.
///
/// It was 2,450 for a day, derived by taking a 3-member run and scaling by 5/3. That scaling is
/// wrong and the error was 2.27x: kill rate is bounded by the zone's SPAWN rate, not by the
/// party's damage, so adding members does not make monsters appear faster. Anything derived from
/// this number was correspondingly optimistic. Re-measure with sim-cli, never by scaling.
const KILLS_PER_HOUR: i64 = 1_078;

/// Yield of `mat` per hour while farming `zone`, in HUNDREDTHS of an item (integer math — this
/// crate forbids floats, and the rates involved are fractions of a percent).
fn per_hour_x100(d: &data::GameData, zone: &data::ZoneDef, mat: &str) -> i64 {
    let pool_weight: i64 = zone.spawn_pool.iter().map(|s| s.weight as i64).sum();
    if pool_weight == 0 {
        return 0;
    }
    let mut acc = 0i64;
    for spawn in &zone.spawn_pool {
        let Some(m) = d.monsters.iter().find(|m| m.id == spawn.monster_id) else { continue };
        let table: i64 = m.drops.iter().map(|r| r.weight as i64).sum();
        if table == 0 {
            continue;
        }
        let hit: i64 = m.drops.iter().filter(|r| r.item_id == mat).map(|r| r.weight as i64).sum();
        // share of spawns that are this monster  ×  its chance to yield the material
        acc += spawn.weight as i64 * hit * 10_000 / (pool_weight * table);
    }
    acc * KILLS_PER_HOUR / 100
}

/// Best crafts per hour for `recipe` across every zone, in hundredths, with the zone and the
/// bottleneck material that produced it.
fn best_crafts_x100<'a>(
    d: &'a data::GameData,
    recipe: &'a data::CraftRecipeDef,
) -> (i64, &'a str, &'a str) {
    let mut best = (0i64, "", "");
    for zone in &d.zones {
        let mut worst = (i64::MAX, "");
        for ing in &recipe.ingredients {
            let n = per_hour_x100(d, zone, &ing.def_id) / ing.qty.max(1) as i64;
            if n < worst.0 {
                worst = (n, ing.def_id.as_str());
            }
        }
        if worst.0 != i64::MAX && worst.0 > best.0 {
            best = (worst.0, zone.id.as_str(), worst.1);
        }
    }
    best
}

/// Recipes knowingly shipped uncraftable-from-one-zone, and why.
///
/// This list is the point of the exemption, not a way around the test: an entry says a human
/// looked and chose to wait. Deleting one when the fix lands is how the fix gets proved. Anything
/// NOT on it that scores zero is a new bug and fails.
///
/// The four apothecary rows all fail on the same material: `glass_flask` drops in b1-6 only, while
/// `featherfern` is b7-12 and `yellow_herb` / `grave_wax` are b22-30. Widening the flask would fix
/// it in one line; the user is instead adding HARD MODE versions of the early bands, which carry
/// late materials into the early zones and fix the whole class of it (2026-08-05).
const AWAITING_HARD_MODE: &[&str] = &[
    "brew_speed_potion_1",
    "brew_speed_potion_2",
    "brew_speed_potion_3",
    "brew_yellow_phial",
];

/// A five-member party on a 2h Meal eats 2.5 meals/h. One farming spot is meant to cover about
/// two of five — short of full uptime on purpose, so the rest is bought (0.0.268).
const MEAL_FLOOR_X100: i64 = 90; // 0.90/h — below this a dish is not worth its recipe
/// 1.50/h = three of five. NOT the 2.50/h full-uptime point: set there, this guard survived its
/// own sabotage — halving a recipe's quantities lands on 2.46, which is 4.9 members fed and
/// exactly the outcome the number exists to prevent, yet it slipped under by four hundredths.
/// A guard placed on the cliff edge does not catch anyone; this one sits back from it.
const MEAL_CEILING_X100: i64 = 150;

#[test]
#[ignore = "craft materials left every drop table in catalog 0.0.163 (user 2026-09-17: \"เอา item material ออกไปก่อน จนกว่าจะคิดระบบ craft ใหม่\"); re-enable with the new craft design"]
fn every_consumable_recipe_is_craftable_somewhere() {
    let d = game_data();
    let mut broken = Vec::new();
    for r in &d.recipes {
        if r.station == CraftStation::Blacksmith {
            continue; // gear output is server-appraised and priced in Gil, not farm rate
        }
        let (rate, zone, bottleneck) = best_crafts_x100(&d, r);
        if rate == 0 && !AWAITING_HARD_MODE.contains(&r.id.as_str()) {
            broken.push(format!(
                "{}: no zone yields all of [{}] — nothing to farm it from",
                r.id,
                r.ingredients.iter().map(|i| i.def_id.as_str()).collect::<Vec<_>>().join(", ")
            ));
        }
        if rate > 0 && AWAITING_HARD_MODE.contains(&r.id.as_str()) {
            broken.push(format!(
                "{}: now craftable ({}.{:02}/h in {}) — remove it from AWAITING_HARD_MODE",
                r.id,
                rate / 100,
                rate % 100,
                zone
            ));
        }
        let _ = bottleneck;
    }
    assert!(broken.is_empty(), "uncraftable recipes:\n  {}", broken.join("\n  "));
}

#[test]
#[ignore = "craft materials left every drop table in catalog 0.0.163 (user 2026-09-17: \"เอา item material ออกไปก่อน จนกว่าจะคิดระบบ craft ใหม่\"); re-enable with the new craft design"]
fn a_meal_feeds_part_of_the_party_not_all_of_it() {
    let d = game_data();
    let mut wrong = Vec::new();
    for r in &d.recipes {
        if r.station != CraftStation::Kitchen {
            continue;
        }
        let (rate, zone, bottleneck) = best_crafts_x100(&d, r);
        if rate < MEAL_FLOOR_X100 || rate >= MEAL_CEILING_X100 {
            wrong.push(format!(
                "{}: {}.{:02} meals/h in {} (limited by {}) — outside [{}.{:02}, {}.{:02})",
                r.id,
                rate / 100,
                rate % 100,
                zone,
                bottleneck,
                MEAL_FLOOR_X100 / 100,
                MEAL_FLOOR_X100 % 100,
                MEAL_CEILING_X100 / 100,
                MEAL_CEILING_X100 % 100,
            ));
        }
    }
    assert!(
        wrong.is_empty(),
        "meal supply drifted from the 2-of-5 target; re-derive the recipe quantities \
         against the 2.5/h a party eats:\n  {}",
        wrong.join("\n  ")
    );
}

/// The other half of the same balance: no single zone may supply a MIXED party's dishes at once.
/// Halving the recipe quantities once pushed b25 to exactly 100% coverage — a zone where a party
/// never needs the market — and that is invisible in the per-dish numbers, which all looked fine.
#[test]
fn no_single_zone_feeds_a_mixed_party() {
    let d = game_data();
    // 2 melee / 2 casters / 1 front-liner, each on a 2h buff: meals per hour, x100.
    let demand: [(&str, i64); 3] = [("AtkPct", 100), ("MatkPct", 100), ("other", 50)];
    let role_of = |output: &str| -> &'static str {
        let Some(def) = d.items.iter().find(|i| i.id == output) else { return "other" };
        let Some(c) = def.consumable.as_ref() else { return "other" };
        let text = format!("{:?}", c.effect);
        if text.contains("AtkPct") {
            "AtkPct"
        } else if text.contains("MatkPct") {
            "MatkPct"
        } else {
            "other"
        }
    };

    let mut worst_zone = ("", 0i64);
    for zone in &d.zones {
        let mut supply: HashMap<&str, i64> = HashMap::new();
        for r in &d.recipes {
            if r.station != CraftStation::Kitchen {
                continue;
            }
            let mut worst = i64::MAX;
            for ing in &r.ingredients {
                worst = worst.min(per_hour_x100(d_ref(&d), zone, &ing.def_id) / ing.qty.max(1) as i64);
            }
            let role = role_of(&r.output_def_id);
            let slot = supply.entry(role).or_insert(0);
            if worst != i64::MAX && worst > *slot {
                *slot = worst;
            }
        }
        // Coverage = the worst-covered role, as a percentage.
        let mut cover = 100i64;
        for (role, need) in demand {
            let got = *supply.get(role).unwrap_or(&0);
            cover = cover.min((got * 100 / need).min(100));
        }
        if cover > worst_zone.1 {
            worst_zone = (zone.id.as_str(), cover);
        }
    }
    assert!(
        worst_zone.1 < 100,
        "{} covers {}% of a mixed party's meals — a party parked there never needs the market, \
         which is the thing the recipe quantities exist to prevent",
        worst_zone.0,
        worst_zone.1
    );
}

/// `HashMap` iteration order is banned in sim LOGIC; this is a test-local aggregation that never
/// feeds a hash or a draw, and the borrow it needs is spelled out here rather than inline so the
/// intent is obvious to the next reader.
fn d_ref(d: &data::GameData) -> &data::GameData {
    d
}
