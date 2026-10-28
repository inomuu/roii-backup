//! GEAR FUSION — six pieces of one grade become one of the next
//! (`docs/gear-fusion-and-base-fix-design.md` §3, decisions D2/D5/D6/D7/D13).
//!
//! The rule is one sentence on purpose: **six items of the same GRADE, any type, any tier**.
//! Nothing to memorise about what combines with what, which is what makes it teachable in a
//! tooltip — and the reason the OUTPUT is random instead: if a player could choose both sides
//! they would never need anything from anyone, and a market with no demand is not a market.
//!
//! The output's TIER is the inputs' average, rounded by a coin whose bias is the fraction
//! (`P(up) = frac`). Two properties make that fair rather than merely clever, and both were
//! checked across all 117,649 combinations before the rule was written down:
//!   * `ceil(avg) <= max(input)` — a fusion can never mint a tier that was not fed in, so tier
//!     cannot be laundered upward by feeding junk;
//!   * the expectation equals the true average exactly — floor-only rounding would quietly tax
//!     every fusion by 11.5%, and players do notice a system that always rounds against them.
//!
//! PRNG: THREE draws, in a fixed order, taken on EVERY client-rolled fusion even when the outcome
//! is already determined (a whole-number average still draws its rounding coin). The stream must
//! not depend on the result — the same contract `roll_drop` keeps when it draws for a `nothing`.
//!
//! SERVER ROLL (design §7.8): the client holds the seed, so a modified client can read the next
//! three draws and press the button only when they say what it wants — the peek-and-select that
//! moved refine to a server roll. A fusion whose OUTPUT is `FUSE_SERVER_ROLL_FROM` or better
//! therefore carries a server-issued `server_roll`, and all three outcomes are read out of that
//! nonce with ZERO `state.rng` draws. The nonce is bound (blake3, server secret) to the account,
//! the six products going in (`binding`) and the attempt number, so it cannot be predicted,
//! re-rolled, or spent on a different six. Below that grade the client keeps rolling: the prize
//! is not worth the cheat and the server should not be asked about every Common combine.

use crate::data::GameData;
use crate::items::{Grade, Item, ItemKind};
use crate::state::{GameState, GilEarn};

/// Pieces one fusion eats. Six, not TBH's nine: a bag holds what it holds, and nine-at-a-time
/// means a player carrying fodder for two fusions is already out of space (user 2026-09-14).
pub const FUSE_INPUTS: usize = 6;

/// The highest grade a fusion may PRODUCE in this patch. The ladder exists to ten, but a grade
/// nobody can reach yet is a grade that cannot be priced — opening them one patch at a time is
/// the release valve the economy is steered with (design §3.6).
pub const FUSE_GRADE_CEILING: Grade = Grade::Beyond;

/// The lowest OUTPUT grade whose fusion must carry a server roll. Rare→Legendary is the first
/// step a cheat is worth steering (a Legendary is the 2-day prize the drop odds are tuned to);
/// everything below stays client-rolled. The sim accepts either path for any grade — the
/// verifier is what refuses a zero roll at or above this line, the same split refine used.
pub const FUSE_SERVER_ROLL_FROM: Grade = Grade::Legendary;

/// Gil PAID OUT by a fusion, as a share of what the six inputs would have vendored for
/// (basis points). Fusing COSTS NOTHING (user 2026-09-15: "ผสมแล้วได้ gil ดังนั้นผสม cost 0 gil
/// ก็ได้") — it pays, like WoG's synthesis, which hands back gold and materials.
///
/// Reversing the fee matters more than it looks: a fee makes the player weigh every combine
/// against their purse, which is a decision about MONEY in a system that is supposed to be about
/// GEAR — and an early player is broke exactly when the fodder piles up. Paying instead makes
/// fusing strictly better than hoarding, which is the behaviour the pacing assumes.
///
/// 100% from 0.0.411 (vendoring retired, salvage was the whole Gil income) until 0.0.455, when
/// kills started paying Gil (`StageBase.gil`): T3+ pieces now return 20% so the late game is not
/// double-paid, while T1-T2 keep 50% — at stage 1-30 a kill pays 3-12 Gil, so salvage is still
/// the early purse (a flat 20% took day one from Lv25 / stage 46 to Lv18 / stage 21).
pub const FUSE_GIL_RETURN_BP: u64 = 1_500;
pub const FUSE_GIL_RETURN_EARLY_BP: u64 = 5_000;
/// The payout also scales with the inputs' GRADE (user 2026-09-21: "การให้ gil ควรคำนึงถึงทั้ง
/// grade + tier ของ input"), in bp by grade step, ×1.5 a step: a Legendary piece embodies 216
/// Commons of forge work, and its salvage should read as more than a Common's, without paying
/// the geometric truth (that would be a faucet that grows with investment).
pub const FUSE_GIL_GRADE_BP: [u64; 10] =
    [10_000, 15_000, 22_000, 33_000, 50_000, 75_000, 110_000, 160_000, 240_000, 350_000];

/// The CRAFTER level `runs` fusions have earned under `rules` (0 when the catalog authors no
/// ladder). Gear passes `Measures.fusions` (the Blacksmith), gems `Measures.gem_fusions` (the
/// Gem Crafter) — one rule, two counters.
pub fn crafter_level(rules: &crate::data::FusionRules, runs: u64) -> u32 {
    if rules.crafter_fusions_per_level == 0 {
        return 0;
    }
    ((runs / u64::from(rules.crafter_fusions_per_level)) as u32).min(rules.crafter_level_cap)
}

/// The fail odds a fusion FROM `grade` faces after `runs` fusions: the table's row (a row past
/// the end repeats the last) minus the crafter's bonus, floored at zero.
pub fn fail_bp_for(rules: &crate::data::FusionRules, runs: u64, grade: Grade) -> u32 {
    let row = rules.fail_bp_by_grade.get(grade.step()).or(rules.fail_bp_by_grade.last()).copied().unwrap_or(0);
    row.saturating_sub(crafter_level(rules, runs).saturating_mul(rules.crafter_bp_per_level))
}

/// The party's BLACKSMITH level — gear fusions run.
pub fn blacksmith_level(state: &GameState, data: &GameData) -> u32 {
    crafter_level(&data.loot_rules.fusion, state.measures.fusions.get())
}

/// Gear fusion fail odds from `grade` for THIS party, after the Blacksmith bonus.
pub fn fail_bp(state: &GameState, data: &GameData, grade: Grade) -> u32 {
    fail_bp_for(&data.loot_rules.fusion, state.measures.fusions.get(), grade)
}


/// What a fusion did, for the UI and for tests. `NoOp` means nothing was consumed — every
/// rejection path leaves the state untouched, so a mis-click costs nothing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FuseOutcome {
    /// The six went in and `def_id` at `grade` came out.
    Success { def_id: String, grade: Grade, tier: u32 },
    /// The six were consumed and `def_id` came back at the INPUT grade (five lost), with the full
    /// Gil payout — the same shape a gem fusion fails in.
    Failed { paid: u64, def_id: String },
    /// Rejected before anything changed (bad picks, mixed grades, at the ceiling, no Gil).
    NoOp,
}

/// Preview a fusion WITHOUT rolling it: the tier the average lands on and the odds of the step
/// up, the output grade, and the Gil it pays. The design makes this mandatory, not a nicety — an average
/// is only a fair rule if the player can see it before committing (§3, the UI line).
pub fn preview(state: &GameState, picks: &[u32; FUSE_INPUTS]) -> Option<(u32, u32, Grade, u64)> {
    let (grade, items) = resolve(state, picks)?;
    let next = grade.next().filter(|g| *g <= FUSE_GRADE_CEILING)?;
    let (tier_lo, up_bp) = tier_split(&items);
    Some((tier_lo, up_bp, next, payout_for(&items)))
}

/// Whether a fusion of `input_grade` pieces needs a server roll — its output would be
/// `FUSE_SERVER_ROLL_FROM` or better. Shared by the client (ask before pressing) and the verifier
/// (refuse a zero roll), so the line is drawn once.
pub fn needs_server_roll(input_grade: Grade) -> bool {
    input_grade.next().is_some_and(|g| g >= FUSE_SERVER_ROLL_FROM)
}

/// What a server roll is BOUND to: the six products going in (sorted `product_key`s, so pick
/// order does not matter) and the attempt number. Products, not instances: the output depends on
/// nothing else about the inputs, so two sets of the same six products are the same fusion, and a
/// nonce issued for one set is correctly valid for the other. `None` for anything `fuse` would
/// reject, so a client never asks the server about a combine that cannot happen.
pub fn binding(state: &GameState, picks: &[u32; FUSE_INPUTS]) -> Option<(String, u64)> {
    let (grade, items) = resolve(state, picks)?;
    grade.next().filter(|g| *g <= FUSE_GRADE_CEILING)?;
    let mut keys: Vec<String> = items.iter().map(Item::product_key).collect();
    keys.sort_unstable();
    Some((keys.join(","), state.measures.fusions.get()))
}

/// The three outcomes a server nonce encodes, in the order the client path draws them:
/// (tier coin, type index, success), each in `[0, 10_000)` except the type, which is reduced by
/// the caller against the pool. Mixed radix rather than bit slices: a 63-bit nonce divided by
/// 10_000 twice still leaves ~9×10¹⁰ for the type, so none of the three is measurably biased.
fn rolls_from_nonce(nonce: u64) -> (u32, u64, u32) {
    let tier = (nonce % 10_000) as u32;
    let rest = nonce / 10_000;
    let ok = (rest % 10_000) as u32;
    (tier, rest / 10_000, ok)
}

/// Gil a fusion hands back: a share of the six inputs' vendor value. Reads the pieces
/// themselves, so feeding better gear pays better — the payout tracks what was actually spent.
fn payout_for(items: &[Item]) -> u64 {
    items
        .iter()
        .map(|i| {
            let grade_bp = FUSE_GIL_GRADE_BP.get(i.grade.step()).copied().unwrap_or(10_000);
            u64::from(i.sell_value) * return_bp_for(i) / 10_000 * grade_bp / 10_000
        })
        .sum()
}

/// The share of a piece's vendor value a fusion hands back: T1-T2 pay
/// `FUSE_GIL_RETURN_EARLY_BP`, everything above `FUSE_GIL_RETURN_BP`.
fn return_bp_for(item: &Item) -> u64 {
    if item.item_level() <= 2 { FUSE_GIL_RETURN_EARLY_BP } else { FUSE_GIL_RETURN_BP }
}

/// Resolve the six picks into (their shared grade, the pieces), rejecting anything that is not
/// six DISTINCT pieces of gear sharing one grade. Duplicated indices are the interesting
/// rejection: without the check, six copies of one index would fuse one item into a better one.
fn resolve(state: &GameState, picks: &[u32; FUSE_INPUTS]) -> Option<(Grade, Vec<Item>)> {
    let mut seen = picks.to_vec();
    seen.sort_unstable();
    seen.dedup();
    if seen.len() != FUSE_INPUTS {
        return None;
    }
    let mut out = Vec::with_capacity(FUSE_INPUTS);
    for p in picks {
        let it = state.inventory.get(*p as usize)?;
        if !matches!(it.kind, ItemKind::Weapon | ItemKind::Armor) || it.unique || it.bound {
            return None;
        }
        out.push(it.clone());
    }
    let grade = out.first()?.grade;
    if out.iter().any(|i| i.grade != grade) {
        return None;
    }
    Some((grade, out))
}

/// The average tier as (floor, P(+1) in bp). Integer throughout: the sim has no floats, and the
/// fraction is exactly what the rounding coin needs anyway.
fn tier_split(items: &[Item]) -> (u32, u32) {
    let sum: u32 = items.iter().map(|i| i.item_level().max(1)).sum();
    let n = items.len() as u32;
    (sum / n, (sum % n) * 10_000 / n)
}

/// Run one fusion. Consumes the six picks, pays out Gil, and banks the result on success.
///
/// `data` is needed to know WHAT can come out — the output type is drawn from the catalog's gear
/// of the resulting tier. A caller without a catalog (`apply_input` with `None`) gets a no-op,
/// the same contract `CraftItem` keeps, because inventing a pool in code would make the sim and
/// the catalog disagree about what exists.
///
/// `server_roll != 0` reads all three outcomes out of the nonce and takes NO `state.rng` draw;
/// `0` is the client roll (three draws). The draw count depends on the EVENT, which is in the
/// log, never on the outcome — so replay stays exact either way.
pub fn fuse(
    state: &mut GameState,
    data: &GameData,
    picks: &[u32; FUSE_INPUTS],
    server_roll: u64,
) -> FuseOutcome {
    let Some((grade, items)) = resolve(state, picks) else { return FuseOutcome::NoOp };
    let Some(next) = grade.next().filter(|g| *g <= FUSE_GRADE_CEILING) else {
        return FuseOutcome::NoOp;
    };
    let (tier_lo, up_bp) = tier_split(&items);
    let served = (server_roll != 0).then(|| rolls_from_nonce(server_roll));

    // DRAW 1 — the tier rounding coin. Taken even when `up_bp` is 0, so the stream does not
    // depend on which six pieces went in.
    let roll_tier = match served {
        Some((t, _, _)) => t,
        None => state.rng.gen_range_u32(0, 10_000),
    };
    let tier = tier_lo + u32::from(roll_tier < up_bp);

    // The pool is every droppable piece of the OUTPUT tier — the same set the drop table uses,
    // so fusion cannot produce something farming never could.
    let pool: Vec<&crate::data::ItemDef> =
        data.items.iter().filter(|d| crate::gear_roll::in_gear_pool(d, tier)).collect();

    // DRAW 2 — the output type. Taken before the empty-pool check fails the fusion, so the
    // stream does not depend on what the catalog holds at this tier.
    let roll_type = match served {
        Some((_, t, _)) => (t % pool.len().max(1) as u64) as u32,
        None => state.rng.gen_range_u32(0, pool.len().max(1) as u32),
    };
    // DRAW 3 — the fail check, against the INPUT grade's odds (`loot_rules.fusion`) after the
    // Blacksmith bonus. Same reasoning: always taken.
    let roll_ok = match served {
        Some((_, _, ok)) => ok,
        None => state.rng.gen_range_u32(0, 10_000),
    };
    let fail_bp = fail_bp(state, data, grade);

    let Some(def) = pool.get(roll_type as usize % pool.len().max(1)) else {
        return FuseOutcome::NoOp;
    };
    let payout = payout_for(&items);
    // Consume: highest index first, so the earlier indices stay valid as the vec shrinks.
    let mut idx = picks.to_vec();
    idx.sort_unstable_by(|a, b| b.cmp(a));
    for i in idx {
        state.inventory.remove(i as usize);
    }
    // Counted once the six are gone: this is the attempt number the NEXT nonce binds to, so it
    // must move on failure too (a failed attempt was still an attempt).
    state.measures.fusions.add(1);
    let failed = roll_ok < fail_bp;
    let mut out = crate::data::instantiate_item(def);
    let lines = crate::substats::lines_by_grade(&data.loot_rules.substats, &out.def_id, out.slot());
    // A failure hands the drawn type back at the INPUT grade — five pieces lost, not six — the
    // same shape a gem fusion fails in (user 2026-09-16: "ควรทำเหมือนกันสองระบบ").
    out.assign_grade(if failed { grade } else { next }, &data.gear_overrides, &lines);
    // The Gil is the same either way (user 2026-09-16: "ให้ gil เต็มไปเลยถึงจะล้มเหลว"): the six were
    // salvaged whatever the forge made of them; the failure is the grade that did not come.
    state.earn_gil(payout, GilEarn::Salvage);
    let made = if failed {
        state.measures.fusions_failed.add(1);
        FuseOutcome::Failed { paid: payout, def_id: out.def_id.clone() }
    } else {
        FuseOutcome::Success { def_id: out.def_id.clone(), grade: next, tier }
    };
    // Never the rare bag: a fusion is something the player DID, like a purchase — the bag exists
    // to keep random rare drops out of the flood, and a result that vanished into it would read
    // as a fusion that ate six items and gave nothing.
    crate::drops::bank_purchase_to_inventory(state, out);
    made
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::items::{ItemKind, Rarity};

    fn data() -> GameData {
        GameData::from_json_str(include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"), "/../../data/game_data.json"
        ))).expect("catalog")
    }

    fn feed(state: &mut GameState, defs: &[&str], grade: Grade) {
        for d in defs {
            let mut it = Item::new(d, ItemKind::Weapon, 10, 0, Rarity::Common);
            it.grade = grade;
            state.inventory.push(it);
        }
    }

    /// T1-T2 inputs hand back 50% of their vendor value, T3+ 20% (0.0.455).
    #[test]
    fn the_gil_return_is_tiered_by_the_inputs() {
        let mut low = Item::new("sword_s1", ItemKind::Weapon, 10, 0, Rarity::Common);
        let mut high = Item::new("sword_s3", ItemKind::Weapon, 10, 0, Rarity::Common);
        low.sell_value = 100;
        high.sell_value = 300;
        assert_eq!(low.item_level(), 1);
        assert_eq!(high.item_level(), 3);
        let six_low: Vec<Item> = (0..6).map(|_| low.clone()).collect();
        let six_high: Vec<Item> = (0..6).map(|_| high.clone()).collect();
        let vendor_low = 6 * u64::from(low.sell_value);
        let vendor_high = 6 * u64::from(high.sell_value);
        assert!(vendor_low > 0 && vendor_high > 0);
        assert_eq!(payout_for(&six_low), vendor_low / 2);
        assert_eq!(payout_for(&six_high), vendor_high * 15 / 100);
        // Grade multiplies on top: six Legendary (step 3, ×3.3) T3 pay 3.3× the Common six.
        let mut leg = high.clone();
        leg.grade = Grade::Legendary;
        let six_leg: Vec<Item> = (0..6).map(|_| leg.clone()).collect();
        assert_eq!(payout_for(&six_leg), 6 * (300 * 15 / 100 * 33 / 10), "per piece: 300 × 15% × 3.3, rounded down each step");
    }

    /// Blacksmith: fusions run buy the fail odds down, one level per `per_level`, to the cap;
    /// a free grade stays free. One rule for gear and gems (`loot_rules.fusion`).
    #[test]
    fn the_blacksmith_level_buys_the_fail_odds_down_to_the_cap() {
        let d = data();
        let r = &d.loot_rules.fusion;
        assert_eq!(r.fail_bp_by_grade.len(), 10);
        assert_eq!(r.fail_bp_by_grade[0], 0, "Common is free");
        assert_eq!(*r.fail_bp_by_grade.last().unwrap(), 5_000, "the top reads ~50% success, as TBH");
        let mut s = GameState::new(1, 8, 8);
        assert_eq!(blacksmith_level(&s, &d), 0);
        let top = Grade::Cosmic;
        assert_eq!(fail_bp(&s, &d, top), 5_000);
        s.measures.fusions.add(25);
        assert_eq!(blacksmith_level(&s, &d), 2);
        assert_eq!(fail_bp(&s, &d, top), 5_000 - 2 * r.crafter_bp_per_level);
        assert_eq!(fail_bp(&s, &d, Grade::Common), 0, "a free grade stays free");
        s.measures.fusions.add(10_000);
        assert_eq!(blacksmith_level(&s, &d), r.crafter_level_cap);
        assert_eq!(fail_bp(&s, &d, top), 5_000 - r.crafter_level_cap * r.crafter_bp_per_level, "50% − cap × 1%");
    }

    /// A FAILED fusion is a gem failure's twin: one piece of the INPUT grade comes back (five
    /// lost), the FULL Gil, the attempt counted — and three draws all the same.
    #[test]
    fn a_failed_fusion_hands_one_piece_back_at_the_input_grade() {
        let mut d = data();
        d.loot_rules.fusion.fail_bp_by_grade = vec![10_000];
        let mut s = GameState::new(7, 8, 8);
        feed(&mut s, &["sword_s3"; 6], Grade::Rare);
        let before = s.rng.clone();
        let gil0 = s.gil();
        match fuse(&mut s, &d, &[0, 1, 2, 3, 4, 5], 0) {
            FuseOutcome::Failed { def_id, paid } => {
                // Hand-built fixtures vendor for 0, so the payout is 0 here; the equality is the
                // property — a failure pays exactly what it says it paid, same as a success.
                assert_eq!(s.gil(), gil0 + paid, "paid in full");
                assert_eq!(s.inventory.len(), 1, "six in, one back");
                assert_eq!(s.inventory[0].def_id, def_id);
                assert_eq!(s.inventory[0].grade, Grade::Rare, "the INPUT grade");
            }
            other => panic!("a 100% fail row must fail: {other:?}"),
        }
        assert_eq!(s.measures.fusions.get(), 1);
        assert_eq!(s.measures.fusions_failed.get(), 1);
        let mut probe = before;
        for _ in 0..3 {
            probe.gen_range_u32(0, 2);
        }
        assert_eq!(s.rng, probe, "three draws, fail or not");
    }

    /// THE PROPERTY THE WHOLE RULE RESTS ON: a fusion can never produce a tier higher than the
    /// best thing fed into it. Feed five s7 and one s1 and the average drags the result to s6 —
    /// no lottery, no laundering a stack of junk into the top tier through a lucky roll.
    #[test]
    fn a_fusion_can_never_mint_a_tier_above_its_inputs() {
        for combo in [
            (vec!["sword_s7", "sword_s7", "sword_s7", "sword_s7", "sword_s7", "sword_s1"], 6, 0),
            (vec!["sword_s7"; 6], 7, 0),
            (vec!["sword_s1"; 6], 1, 0),
        ] {
            let mut s = GameState::new(1, 8, 8);
            feed(&mut s, &combo.0, Grade::Common);
            let picks = [0, 1, 2, 3, 4, 5];
            let (tier_lo, up_bp, grade, _fee) = preview(&s, &picks).expect("previewable");
            assert_eq!(tier_lo, combo.1, "{:?}", combo.0);
            assert_eq!(up_bp, combo.2, "a whole average must have NO chance of a step up");
            assert_eq!(grade, Grade::Uncommon, "grade always steps exactly one");
        }
        // The fractional case: s1×3 + s5×2 + s7 averages 3.33 → s3 two times in three, s4 one.
        let mut s = GameState::new(1, 8, 8);
        feed(&mut s, &["sword_s1", "sword_s1", "sword_s1", "sword_s5", "sword_s5", "sword_s7"], Grade::Common);
        let (lo, up, _, _) = preview(&s, &[0, 1, 2, 3, 4, 5]).expect("previewable");
        assert_eq!((lo, up), (3, 3_333), "3.33 → s3 with a 33.33% step to s4");
    }

    /// Mixed grades, duplicate picks and a maxed grade must all bounce WITHOUT touching a thing.
    /// The duplicate case is the one worth writing down: six copies of index 0 would otherwise
    /// fuse a single item into a better one.
    #[test]
    fn every_rejection_leaves_the_bag_and_the_purse_untouched() {
        let d = data();
        let mut s = GameState::new(1, 8, 8);
        s.set_gil_for_test(1_000_000);
        feed(&mut s, &["sword_s1"; 5], Grade::Common);
        feed(&mut s, &["sword_s1"], Grade::Rare);
        let before = (s.inventory.len(), s.gil());

        assert_eq!(fuse(&mut s, &d, &[0, 1, 2, 3, 4, 5], 0), FuseOutcome::NoOp, "mixed grades");
        assert_eq!(fuse(&mut s, &d, &[0, 0, 0, 0, 0, 0], 0), FuseOutcome::NoOp, "one item six times");
        assert_eq!(fuse(&mut s, &d, &[0, 1, 2, 3, 4, 99], 0), FuseOutcome::NoOp, "index past the bag");
        assert_eq!((s.inventory.len(), s.gil()), before, "a rejected fusion costs nothing");
        assert_eq!(s.measures.fusions.get(), 0, "a rejection is not an attempt");

        // At the patch ceiling there is nowhere to go, and that must be a no-op, not a loss.
        let mut top = GameState::new(1, 8, 8);
        top.set_gil_for_test(1_000_000);
        feed(&mut top, &["sword_s1"; 6], FUSE_GRADE_CEILING);
        assert_eq!(fuse(&mut top, &d, &[0, 1, 2, 3, 4, 5], 0), FuseOutcome::NoOp, "at the ceiling");
        assert_eq!(top.inventory.len(), 6, "the six are still there");
    }

    /// Six in, one out, one grade up — and Gil comes BACK either way. Runs the real catalog so
    /// the output type comes from the same pool a drop would.
    #[test]
    fn a_fusion_consumes_six_and_banks_one_of_the_next_grade() {
        let mut d = data();
        d.loot_rules.fusion.fail_bp_by_grade.clear(); // the grade rule is tested on its own above
        let mut s = GameState::new(7, 8, 8);
        s.set_gil_for_test(100_000);
        feed(&mut s, &["sword_s3"; 6], Grade::Rare);
        let gil_before = s.gil();
        match fuse(&mut s, &d, &[0, 1, 2, 3, 4, 5], 0) {
            FuseOutcome::Success { grade, tier, def_id } => {
                assert_eq!(grade, Grade::Legendary);
                assert_eq!(tier, 3, "six s3 average to s3 exactly");
                assert_eq!(crate::gear_roll::def_tier(&def_id), 3, "the output is of the output tier");
                assert_eq!(s.inventory.len(), 1, "six consumed, one banked");
                assert_eq!(s.inventory[0].grade, Grade::Legendary);
            }
            other => panic!("with no fail row a fusion succeeds, got {other:?}"),
        }
        assert!(s.gil() >= gil_before, "a fusion PAYS (0.0.374) — it never charges");
        assert_eq!(s.measures.fusions.get(), 1, "one attempt on the record");
    }

    /// THREE DRAWS, ALWAYS, whatever the outcome. The sim's stream must not depend on results —
    /// a fusion whose tier is already decided still draws its rounding coin, or two saves that
    /// fused different things would desync every later roll.
    #[test]
    fn a_fusion_takes_exactly_three_draws_whatever_happens() {
        let d = data();
        let count = |defs: &[&str], gil: u64| -> u64 {
            let mut s = GameState::new(99, 8, 8);
            s.set_gil_for_test(gil);
            feed(&mut s, defs, Grade::Common);
            let before = s.rng.clone();
            let _ = fuse(&mut s, &d, &[0, 1, 2, 3, 4, 5], 0);
            // Count draws by replaying the pre-state until the streams line up again.
            let mut probe = before;
            for n in 0..8 {
                if probe == s.rng {
                    return n;
                }
                probe.gen_range_u32(0, 2);
            }
            99
        };
        assert_eq!(count(&["sword_s7"; 6], 100_000), 3, "a whole average still draws its coin");
        assert_eq!(count(&["sword_s1", "sword_s1", "sword_s1", "sword_s5", "sword_s5", "sword_s7"], 100_000), 3);
        assert_eq!(count(&["sword_s1"; 6], 0), 3, "even with an empty purse — fusing is free");
    }

    /// A SERVER-ROLLED fusion takes NO draw from the game's stream and is a pure function of the
    /// nonce: the same six + the same nonce give the same item on any client, which is what lets
    /// the verifier replay it. And the stream after it is exactly the stream before it, so the
    /// hybrid (client-rolled below Legendary, server-rolled from it) never desyncs a later roll.
    #[test]
    fn a_server_rolled_fusion_takes_no_draw_and_is_a_function_of_the_nonce() {
        let mut d = data();
        d.loot_rules.fusion.fail_bp_by_grade.clear();
        let run = |nonce: u64| {
            let mut s = GameState::new(5, 8, 8);
            feed(&mut s, &["sword_s3", "sword_s3", "sword_s3", "sword_s3", "sword_s3", "sword_s4"], Grade::Rare);
            let before = s.rng.clone();
            let out = fuse(&mut s, &d, &[0, 1, 2, 3, 4, 5], nonce);
            assert_eq!(s.rng, before, "zero draws under a server roll");
            out
        };
        let a = run(0x1234_5678_9abc);
        assert_eq!(a, run(0x1234_5678_9abc), "same nonce, same outcome");
        assert!(matches!(a, FuseOutcome::Success { grade: Grade::Legendary, .. }), "no fail row: {a:?}");
        // A different nonce is a different roll — over a few, the type must vary (the pool at
        // tier 3/4 holds many defs), which is the property that makes the nonce the entropy.
        let kinds: std::collections::BTreeSet<String> = (1..40u64)
            .filter_map(|n| match run(n.wrapping_mul(0x9E37_79B9_7F4A_7C15) & 0x7FFF_FFFF_FFFF_FFFF) {
                FuseOutcome::Success { def_id, .. } => Some(def_id),
                _ => None,
            })
            .collect();
        assert!(kinds.len() > 3, "the nonce decides the type: {kinds:?}");
    }

    /// The binding a server roll is issued against: pick ORDER must not matter (the same six in
    /// any order are one fusion), the attempt number moves with every run, and a combine `fuse`
    /// would reject has no binding at all — so the client never asks about it.
    #[test]
    fn the_binding_is_order_free_and_counts_attempts() {
        let d = data();
        let mut s = GameState::new(3, 8, 8);
        feed(&mut s, &["sword_s2", "sword_s1", "sword_s2", "sword_s1", "sword_s3", "sword_s1"], Grade::Rare);
        let (k1, n1) = binding(&s, &[0, 1, 2, 3, 4, 5]).expect("bindable");
        let (k2, n2) = binding(&s, &[5, 4, 3, 2, 1, 0]).expect("bindable");
        assert_eq!((k1.as_str(), n1), (k2.as_str(), n2));
        assert_eq!(k1, "sword_s1#Rare,sword_s1#Rare,sword_s1#Rare,sword_s2#Rare,sword_s2#Rare,sword_s3#Rare");
        assert_eq!(n1, 0);
        assert!(binding(&s, &[0, 0, 0, 0, 0, 0]).is_none(), "a rejected combine has no binding");
        fuse(&mut s, &d, &[0, 1, 2, 3, 4, 5], 7);
        feed(&mut s, &["sword_s1"; 6], Grade::Common);
        let n = s.inventory.len() as u32;
        let (_, next) = binding(&s, &[n - 6, n - 5, n - 4, n - 3, n - 2, n - 1]).expect("bindable");
        assert_eq!(next, 1, "the next attempt binds to the next number");
        // The line the hybrid is drawn at: Rare in (Legendary out) needs the server, Uncommon
        // in (Rare out) does not.
        assert!(needs_server_roll(Grade::Rare));
        assert!(!needs_server_roll(Grade::Uncommon));
        assert!(!needs_server_roll(Grade::Cosmic), "nothing comes out at the top, nothing to roll");
    }
}
