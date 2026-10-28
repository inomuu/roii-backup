//! Drop rolls — money-zone (GDD §7.1).
//!
//! RNG draw order (CONTRACT — read before changing):
//!   `roll_drop()` takes exactly ONE PRNG draw per call
//!   (a single `gen_range_u32(0, total_weight)`).
//!   `drop_on_death` takes ONE MORE draw IFF the rolled item is a CARD (the Foil
//!   roll) — a second draw, but only on the rare card-drop path.
//!
//! If the table is empty or all weights are zero, `roll_drop` returns
//! `None` and takes ZERO draws — this preserves PRNG stream when a
//! "dead with no drops" entity dies (e.g. a player character).
//!
//! Changing this contract reorders every drop event in every save
//! file ever made. Treat with §7.2 ceremony.

// DEBT (3 sites): this file predates `deny(clippy::indexing_slicing)`. An index that
// cannot be proven in range is a panic, and in verify-server (which links this crate and
// replays saves under `panic = "abort"`) a panic is the whole process. Shrink this, never
// add to it — a NEW file has no allow and is rejected on its first bare index.
#![allow(clippy::indexing_slicing)]

use crate::items::{DropEntry, Item};
use crate::rng::Pcg32;
use crate::state::{GameState, GilEarn};

/// Party-shared stash capacity in SLOTS. When full, dropped loot that would
/// need a new slot is lost — the player must clear space. (Stacking into an
/// existing slot is always allowed; it doesn't consume a slot.) Starting value;
/// upgrades raise it. Loot is the only capped path — unequip still returns gear to the stash.
///
/// 50 since 0.0.374 (was 20). FUSION is what changed: six of a grade have to be held AT ONCE to
/// combine, and a party feeding several grades needs room for several piles. At 20 the bag was
/// full of fodder before a single fusion could be assembled — measured, not guessed: the pacing
/// run reported "0 fusions run" and a party wearing 23 of its 50 slots, because incoming drops
/// were being lost to a bag full of parts.
pub const INVENTORY_CAPACITY: usize = 50;

/// Stash WAREHOUSE base capacity in SLOTS — the player-managed deposit store
/// (`GameState.stash`), separate from the 20-slot `inventory`. Large so it's a
/// real long-term bank; upgradeable via `stash_cap`. Deposits are player-driven
/// (DepositToStash), never automatic.
pub const STASH_CAPACITY: usize = 100;

/// Chance (basis points) that a dropped CARD is FOIL (shiny, grants the card's
/// authored `foil_effect`, star-scaled like the base lines — see `card_specials`).
/// 500 = 5% (the design's ~1/20-of-Epic Foil rate). See `drop_on_death`.
pub const FOIL_DROP_CHANCE_BP: u32 = 500;

/// The HIGHEST base level on the player team (dead or alive). The loot gap penalty that read it
/// is gone (0.0.398: rewards grow with the stage instead of shrinking behind the party); the
/// number still serves the sigil tier and the reports.
pub fn party_max_level(state: &GameState) -> u32 {
    state
        .grid
        .entities
        .iter()
        .filter(|e| matches!(e.team, crate::grid::Team::Player))
        .map(|e| e.stats.level)
        .max()
        .unwrap_or(1)
}

/// Roll one item from a weighted drop table.
///
/// Returns a borrowed reference to the chosen template; the caller
/// clones into the world (ground / inventory).
pub fn roll_drop<'a>(rng: &mut Pcg32, table: &'a [DropEntry]) -> Option<&'a Item> {
    roll_drop_scaled(rng, table, 10_000)
}

/// `roll_drop` under a loot multiplier (basis points): the draw range is
/// inflated to `total × 10000 / mult_bp`, so every real entry keeps its
/// relative weight while `mult_bp/10000` of rolls land in the table at all —
/// the rest miss (an implicit extra `nothing`). Exactly ONE draw either way,
/// and at 10000 bp the draw value is bit-identical to the unscaled roll, so
/// full-loot kill streams (all pre-existing goldens with gap ≤ grace) are
/// untouched.
pub fn roll_drop_scaled<'a>(rng: &mut Pcg32, table: &'a [DropEntry], mult_bp: u32) -> Option<&'a Item> {
    if table.is_empty() {
        return None;
    }
    let total: u32 = table.iter().map(|e| e.weight).sum();
    if total == 0 {
        return None;
    }
    let mult = mult_bp.clamp(1, 10_000);
    let stretched = ((total as u64) * 10_000 / mult as u64).min(u32::MAX as u64) as u32;
    let r = rng.gen_range_u32(0, stretched);
    let mut acc: u32 = 0;
    for entry in table {
        acc = acc.saturating_add(entry.weight);
        if r < acc {
            return Some(&entry.template);
        }
    }
    // r >= total: the penalty ate this roll (or total > 0 float-free rounding).
    None
}

/// Called by `tick.rs` immediately after an entity transitions from
/// alive → dead. Rolls the entity's drop table once and (on hit)
/// auto-loots the item straight into the first Player's `inventory`.
///
/// ALPHA-8c switched away from the previous "land on ground, click to
/// pick up" model — idle-ARPG players never want a manual loot step.
/// If there is no Player on the grid (shouldn't happen in normal play
/// since the zone always has one) the item is silently lost; this is
/// strictly defensive and never hits in any tested scenario.
///
/// Phase 0 cheats slightly: we clone the drop table to sidestep the
/// borrow checker. Drop tables are short and deaths are uncommon vs.
/// ticks, so the cost is negligible.
/// Pending-candidate bounds: hard cap on the list (bounded state) and a TTL after
/// which an unresolved (dud) candidate silently expires — the server resolves within
/// one save cycle (~10-20 s) online, so 8 h covers any offline stretch generously.
/// A BACKSTOP, not an operating limit — the distinction 0.0.255 was written in blood over.
///
/// 500 was sized against the load the INVERTED stage 1 produced, and was reachable in ~15
/// minutes; hitting it stopped every server-roll drop, silently. A bound that can be reached
/// in normal play and says nothing when it is, is the worst shape a bound can have.
///
/// So this is now set where ordinary play cannot get near it. Two drains keep it there:
/// `PruneCandidates` clears every adjudicated dud on each save (steady state ~1 online), and
/// the TTL clears them by age if prune never arrives (~118 at the measured kill rate). 1,000
/// needs 100,000 kills inside one TTL window — roughly 68 hours of play at the rate measured,
/// and still 2.8x clear of a hypothetical endgame party killing three times that fast.
///
/// That last margin is why it is 1,000 rather than 320: the measured 11,759 kills/8 h came
/// from a THREE-member party, which is a floor, not a ceiling. At 3x, steady state is ~353 and
/// a 320 cap would start biting again.
pub const RARE_CANDIDATE_CAP: usize = 1_000;
pub const RARE_CANDIDATE_TTL_TICKS: u64 = 288_000; // 8 h at 10 ticks/s

/// CRAFT candidates get their OWN budget rather than sharing the drop lottery's 500.
///
/// The two fill up nothing alike. Drop candidates are duds by design — a busy farmer
/// accrues hundreds and they sit until the 8 h TTL clears them, so the list is normally
/// somewhere near full. Craft candidates are a handful, and online the server resolves them
/// within one save cycle. Sharing one cap meant the fuller the player's drop list, the less
/// able they were to craft — and since `craft_item` refuses rather than eating the inputs,
/// the symptom was a Craft button that did nothing at all on exactly the established
/// accounts most likely to use the recipes.
///
/// NO LOAD-vs-CAP TEST, deliberately — and this is the exception that proves the rule in
/// CLAUDE.md, not an oversight. The other caps are consumed at a rate derivable from the
/// catalog (kills x drop weights), so a test can compute the load and check they still fit.
/// This one is consumed by BUTTON PRESSES, and there is no honest way to derive that number.
///
/// What bounds it in practice instead: since 0.0.257 a gear craft PUSHES IMMEDIATELY rather
/// than waiting for the autosave, so each ticket clears in about a second and the real queue
/// depth is ~1. Reaching 64 would mean 64 crafts landing inside one unsynced window — roughly
/// 1,200 materials spent in a few seconds — and gear crafting is refused outright while
/// offline. 64 is headroom, not a budget.
///
/// It still counts toward `every_candidate_cap_together_stays_inside_the_save_budget`, which
/// is where raising it would show up.
pub const CRAFT_CANDIDATE_CAP: usize = 64;

/// Pending counts as `(drop, craft)`. Each kind is counted against its OWN cap — sharing one
/// budget is what silently disabled crafting in 0.0.254, and adding GEAR to the same pile
/// would repeat it a third time.
pub fn candidate_counts(state: &GameState) -> (usize, usize) {
    let mut drops = 0;
    let mut crafts = 0;
    for c in &state.rare_candidates {
        match c.kind {
            crate::state::CandidateKind::Drop => drops += 1,
            crate::state::CandidateKind::Craft => crafts += 1,
            crate::state::CandidateKind::Gear { .. } => {}
        }
    }
    (drops, crafts)
}

/// STAGE-1 candidate gate — ONE roll per kill, the SAME rate for every item and every
/// monster (`docs/server-roll-drops-design.md` §Design: "the sim rolls a low-rate
/// `rare_candidate` flag per kill (e.g. 1%)"). `candidate_rate` sets server-roll LOAD;
/// per-item rarity belongs to STAGE 2, on the server.
///
/// This is the invariant 0.0.207 broke. That commit needed clean final rates (1/2,500 ..
/// 1/200,000) and got them by scaling STAGE 1 per item at `bp = 10^7/den` — which inverts the
/// two stages, because a rate of that form grows as the item gets MORE common: 1/2,500 needs a
/// 40% candidate rate. Rolled per drop-table ENTRY on top of that, a 12-entry monster accrued
/// 1.3 candidates per kill. The final drop rates were right; the load was 62x what the design
/// budgets for, so the list pegged at its cap after ~800 kills and every server-roll item —
/// cards included, though cards alone would never have filled it — stopped for good.
pub const CANDIDATE_RATE_BP: u32 = 100; // 1% — the doc's balanced default

/// Kills in 8 h of play, MEASURED (`sim-cli --ticks 288000 --recruit ...`, 2026-08-02: 11,759
/// kills to band 10).
///
/// This is the window the cap has to cover, and the reason is DUDS, not disconnection. The
/// server resolves every candidate on every save but only tells the client about the ~0.1%
/// that hit; the rest sit in the list until `RARE_CANDIDATE_TTL_TICKS` ages them out 8 h
/// later. So a continuously-online session accumulates a full 8 h of unresolved tickets just
/// the same. (Being disconnected adds nothing: the client freezes its own sim after
/// `SPAN_STALL_TICKS` ~5 min unsaved, and there is no offline build.)
///
/// Used by `candidate_load_fits_the_offline_window` to keep the rate and the cap in step —
/// the pairing nothing checked when 0.0.207 moved 380 rows onto the pipeline.
pub const KILLS_PER_8H: u64 = 11_759;

/// Accrue at most ONE pending candidate for this kill.
///
/// Both draws are taken whether or not the candidate can be stored, so the PRNG stream does
/// NOT depend on how full the list is. The old per-entry loop `break`-ed at the cap and
/// skipped its draws, which meant crossing the cap silently re-phased every downstream roll.
fn accrue_candidate(state: &mut GameState, table: &[DropEntry]) {
    // The event multiplier rides on the ENTRY WEIGHT, not on the gate: the gate is shared by
    // every server-roll item, so scaling it would boost orbs and ore along with the cards.
    let boost = combined_card_rate_bp(state);
    let total_bp: u32 = table.iter().map(|e| card_boosted_bp(e, boost)).sum();
    if total_bp == 0 {
        return; // no server-roll entries — no draw, so every such kill stream is untouched
    }
    if state.rng.gen_range_u32(0, 10_000) >= CANDIDATE_RATE_BP {
        return;
    }
    // WHICH item this ticket is for, weighted by the entry's own `server_bp` — those values
    // stay exactly as authored and keep their meaning as RELATIVE frequency within a monster.
    let r = state.rng.gen_range_u32(0, total_bp);
    let mut acc = 0u32;
    let Some(entry) = table.iter().find(|e| {
        let bp = card_boosted_bp(e, boost);
        acc = acc.saturating_add(bp);
        bp > 0 && r < acc
    }) else {
        return;
    };
    if candidate_counts(state).0 >= RARE_CANDIDATE_CAP {
        return; // bounded state — the draws above already happened
    }
    state.rare_candidate_seq += 1;
    state.rare_candidates.push(crate::state::RareCandidate {
        seq: state.rare_candidate_seq,
        def_id: entry.template.def_id.clone(),
        tick: state.tick,
        kind: crate::state::CandidateKind::Drop,
        hit_ppm: stage2_hit_ppm(total_bp),
    });
}

/// STAGE-2 hit rate for a ticket drawn off a table whose `server_bp` sums to `total_bp`,
/// in parts per million. Carried ON the candidate because the server resolves it knowing only
/// the candidate — it has no idea which monster died.
///
/// Derived to preserve every authored final rate EXACTLY. For item i on a monster whose
/// entries sum to W, the old pipeline gave `bp_i/10^4 x 1/1000 = bp_i/10^7`. The new one gives
/// `1% x (bp_i/W) x h`, so `h = W/10^5` — the same for every item on that monster, and an
/// exact integer in ppm (`10 x W`). No rounding, no rebalance: the numbers in game_data.json
/// keep the meaning they were confirmed with.
pub fn stage2_hit_ppm(total_bp: u32) -> u32 {
    total_bp.saturating_mul(10)
}

/// Pending GEAR tickets allowed at once. Unlike the drop lottery these ALWAYS pay, so they
/// only ever queue between one save and the next.
///
/// Sized against the real window, which is not 8 h: `SPAN_STALL_TICKS` (~5 min unsaved)
/// freezes the client's own sim, so a session cannot bank more than about 5 minutes of kills
/// without syncing. At the measured rate that is ~1 piece. 128 is two orders of magnitude of
/// headroom and `gear_tickets_fit_the_unsynced_window` asserts it stays that way.
///
/// Hitting it drops the piece, which is deliberate: banking it un-starred instead would hand
/// a client a way to mint gear the server never saw. It is unreachable while the stall guard
/// works, and a lost common drop is a far smaller failure than a minting hole.
pub const GEAR_CANDIDATE_CAP: usize = 128;

/// Kills the client can accrue before `SPAN_STALL_TICKS` (3,000 ticks = 5 min) freezes the
/// sim, at the measured rate. The bound that actually matters for anything the server clears
/// on every save.
pub const KILLS_PER_STALL_WINDOW: u64 = KILLS_PER_8H * 3_000 / 288_000;

/// How many GEAR tickets are pending.
pub fn gear_candidate_count(state: &GameState) -> usize {
    state
        .rare_candidates
        .iter()
        .filter(|c| matches!(c.kind, crate::state::CandidateKind::Gear { .. }))
        .count()
}

/// Hand a dropped piece of gear to the server to be valued. Takes NO draw — which item
/// dropped was already decided by the weighted table above.

/// The POTENTIAL MATERIALS draw (0.0.355): ONE extra PRNG draw per kill against the loot row for
/// the monster's level (`potential::loot_tier`) — absolute odds, so it never dilutes the weighted
/// table (the per-monster rows it replaces shifted every other drop's share). The Sigil has no
/// element (2026-09-30); a monster with no weakness list, or whose first weakness is outside the
/// player chart, still yields nothing from this draw — the gate kept the total unchanged (the coverage ratchet in
/// `tests/suite/potential_coverage.rs` keeps that from being any field monster a unit needs). NOT scaled by the level gap: grade already follows the monster's level,
/// and a capped unit needs grade-I Sigil for every stat node it ever buys.
fn roll_potential_material(state: &mut GameState, dead_idx: usize) {
    use crate::items::{ItemKind, Rarity};
    use crate::potential::{in_chart, loot_tier, sigil_def, BOSS_GRADE_BP, GRADE_BP};
    let e = &state.grid.entities[dead_idx];
    let (level, boss, weak) = (e.mob_stage, e.is_boss, e.weaknesses.first().copied());
    let tier = loot_tier(level);
    let odds = if boss { &BOSS_GRADE_BP } else { &GRADE_BP };
    // The draw is taken BEFORE the yield is known — every kill, one draw, whatever it pays.
    let r = state.rng.gen_range_u32(0, 10_000);
    // The gate is unchanged (a first weakness in the chart), so the total Sigil dropped is too; the
    // Sigil itself has no element since 2026-09-30.
    if !weak.is_some_and(in_chart) {
        return;
    }
    // Every grade up to the zone's own, highest first, each at its written odds: the ranges sit
    // end to end, so grade III lands in [0, 3), II in [3, 11), I in [11, 31) of a grade-III kill.
    let mut acc = 0u32;
    let mut grade = None;
    for g in (0..=tier.grade).rev() {
        acc = acc.saturating_add(odds[g]);
        if r < acc {
            grade = Some(g);
            break;
        }
    }
    let Some(grade) = grade else { return };
    let id = sigil_def(grade);
    let rarity = match grade { 0 => Rarity::Uncommon, 1 => Rarity::Rare, _ => Rarity::Mythical };
    let mut sigil = Item::new(&id, ItemKind::Material, 0, 0, rarity);
    sigil.ledger = true;
    bank_loot_drop(state, sigil);
}

/// MEMORY DUST DROP TABLE (0.0.490, user 2026-09-29: "ดรอปเยอะไปก็ดูเฟ้อ … อยากให้เยอะแต่ไม่เฟ้อ").
/// Odds are in 1/100,000 of a kill and grow with the monster's STAGE, and deep stages pay a larger
/// tier, so farming the first zone is always the worst XP/hour and late drops are few and large.
/// `(dust, radiant, crystal)` odds for one kill at `stage`:
/// - ordinary kill: Memory Dust `(30 + s)·10` at every stage (0.31% → 1.98% at 168), Radiant
///   `(s − 80)·100/88` past stage 80 (0.1% at 168);
/// - boss: Radiant `(50 + s)·10` (0.51% → 2.18% at 168), Crystal `(s − 120)·300/48` past 120
///   (0.3% at 168).
/// At the measured 168 pace (704 kills + 17 bosses an hour, 8 h a day) that is ~110 Dust,
/// ~9 Radiant and ~0.4 Crystal a day ≈ 240k XP. PROVISIONAL.
pub fn memory_dust_odds(stage: u16, boss: bool) -> (u32, u32, u32) {
    let s = u32::from(stage);
    if boss {
        (0, (50 + s) * 10, s.saturating_sub(120) * 300 / 48)
    } else {
        ((30 + s) * 10, s.saturating_sub(80) * 100 / 88, 0)
    }
}

/// The Memory Dust draw (0.0.490): ONE draw on every kill, taken whatever it pays (the stream never
/// depends on the outcome), read against `memory_dust_odds` laid end to end — Crystal, Radiant,
/// Dust. All three are ledger counts, so a full bag never loses one.
fn roll_memory_dust(state: &mut GameState, dead_idx: usize) {
    use crate::items::{ItemKind, Rarity, REGALIA_FODDER_CRYSTAL_DEF_ID, REGALIA_FODDER_DEF_ID, REGALIA_FODDER_SR_DEF_ID};
    let e = &state.grid.entities[dead_idx];
    let (dust, radiant, crystal) = memory_dust_odds(e.mob_stage, e.is_boss);
    let r = state.rng.gen_range_u32(0, 100_000);
    let (id, rarity) = if r < crystal {
        (REGALIA_FODDER_CRYSTAL_DEF_ID, Rarity::Legendary)
    } else if r < crystal + radiant {
        (REGALIA_FODDER_SR_DEF_ID, Rarity::Rare)
    } else if r < crystal + radiant + dust {
        (REGALIA_FODDER_DEF_ID, Rarity::Common)
    } else {
        return;
    };
    let mut item = Item::new(id, ItemKind::Regalia, 0, 0, rarity);
    item.ledger = true;
    bank_loot_drop(state, item);
}

/// GEAR GRADE odds per BAND (design §3.5, D9) in basis points of one gear drop, indexed by
/// `Grade::step()`. Read as: at the launch band a piece is Common 76% of the time and Beyond
/// 0.01%, and the tail fattens as the party climbs.
///
/// THE RATIO IS THE WHOLE DESIGN. Fusion is 6-into-1, so a grade is worth 6× the one below it:
/// odds that fall FASTER than 1/6 per step make fusion the main road (you farm and combine),
/// SLOWER than 1/6 makes farming a lottery that floods the market with top grades. These rows
/// run 1/10 early → 1/4.3 at the cap on purpose: predictable while the player is learning, with
/// a real chance of a moment once they are deep.
///
/// Common ABSORBS the rounding so every row sums to exactly 10,000 (`grade_odds_sum_to_one`):
/// the tail's numbers are the designed ones, and the floor takes whatever is left.
const GRADE_ODDS_BP: [(u16, [u32; 10]); 5] = [
    //            Common  Unc   Rare   Leg  Immo  Arc  Bey  Cel Div Cos
    (34,  [9_000, 900, 90, 10, 0, 0, 0, 0, 0, 0]),
    (68,  [8_740, 1_100, 140, 18, 2, 0, 0, 0, 0, 0]),
    (101, [8_324, 1_400, 230, 39, 7, 0, 0, 0, 0, 0]),
    (135, [7_988, 1_600, 330, 66, 13, 3, 0, 0, 0, 0]),
    (168, [7_638, 1_800, 430, 100, 25, 6, 1, 0, 0, 0]),
];

/// The odds row for a monster's STAGE (0.0.399): five rows across the launch ladder (stages
/// 1-168), each covering a fifth of it — the design's twenty bands, written by stage.
///
/// Past the last row the TOP row holds: a cap raise is meant to extend the ladder with new
/// GRADES (§3.6), not to make the existing curve steeper on its own.
fn grade_odds_for_stage(stage: u16) -> &'static [u32; 10] {
    let row = GRADE_ODDS_BP.iter().find(|(max, _)| stage.max(1) <= *max).unwrap_or(&GRADE_ODDS_BP[4]);
    &row.1
}

/// Roll the GRADE of one dropped piece of gear (0.0.372). ONE PRNG draw, taken ONLY when a real
/// piece of gear banks — the same conditional-draw shape the foil roll uses, so a stream without
/// gear drops is untouched.
///
/// This is the ONLY roll left on a dropped item, and it does not break D1 (fixed stats): grade
/// picks WHICH catalog entry you got, exactly like the weighted table picking a type. Two Rare
/// swords are identical and therefore fungible on a market that matches by name; a rolled
/// substat would not be.
fn roll_gear_grade(state: &mut GameState, odds: &[u32; 10]) -> crate::items::Grade {
    let r = state.rng.gen_range_u32(0, 10_000);
    // Highest grade first, ranges end to end — a rounding hole can then only ever land in
    // Common, which is the floor anyway.
    let mut acc = 0u32;
    for (i, g) in crate::items::GRADES.iter().enumerate().rev() {
        acc = acc.saturating_add(odds[i]);
        if r < acc {
            return *g;
        }
    }
    crate::items::Grade::Common
}

pub fn drop_on_death(state: &mut GameState, dead_idx: usize) {
    let table = state.grid.entities[dead_idx].drops.clone();
    // No level-gap penalty (0.0.398): a kill pays its table in full wherever the party stands.
    accrue_candidate(state, &table);
    // Drop-rate crest (0.0.433): every REAL entry's weight grows by the bonus while `nothing`
    // keeps its weight, so the same single draw lands in the table more often. 0 = the table
    // as authored (byte-identical stream).
    let drop_bp = crate::crest::drop_rate_bp(state).max(0) as u64;
    let rolled = if drop_bp > 0 {
        let boosted: Vec<DropEntry> = table
            .iter()
            .map(|e| {
                let mut e = e.clone();
                if e.template.def_id != crate::items::NOTHING_DROP_ID {
                    e.weight = ((e.weight as u64).saturating_mul(10_000 + drop_bp) / 10_000).min(u32::MAX as u64) as u32;
                }
                e
            })
            .collect();
        roll_drop(&mut state.rng, &boosted).cloned()
    } else {
        roll_drop(&mut state.rng, &table).cloned()
    };
    // The Potential materials draw sits HERE in the stream — after the weighted roll, before the
    // conditional foil draw — on every kill, whatever the weighted roll said.
    roll_potential_material(state, dead_idx);
    roll_memory_dust(state, dead_idx);
    let Some(mut item) = rolled else { return; };
    // No-drop band: the weighted table includes a `nothing` sentinel so most
    // kills bank no loot (idle pacing — ~20 keepable gear/hour, stash fills in
    // ~1h). The draw already happened above (single-PRNG-draw contract intact);
    // we simply don't bank the sentinel.
    if item.def_id == crate::items::NOTHING_DROP_ID {
        return;
    }
    // GEAR IS FIXED-STAT NOW (D1, 0.0.369) — nothing about a dropped piece is rolled, by the
    // client OR the server: its power is (type × tier) authored in the catalog, and depth comes
    // from what the PLAYER does to it afterwards (gem / card / fusion —
    // docs/gear-fusion-and-base-fix-design.md). The stage-3 server-roll detour existed only to
    // stop a modified client minting value out of ★/bonus lines/runes; with none of those
    // rolled, there is nothing to protect and the piece banks straight away.
    //
    // THREE PRNG DRAWS LEFT THE STREAM HERE (star, bonuses, rune). That is why this change had
    // to land in one patch with the rune removal — every golden re-pins once.
    // FOIL roll (0.0.126): a dropped CARD has a 5% chance to be Foil (shiny; grants the
    // card's authored foil_effect, star-scaled since 0.0.218).
    // ONE extra PRNG draw, taken ONLY when a real card drops (cards are ~1/20k, so this
    // fires rarely) → the stream stays stable given the same drops. Non-cards never draw.
    if matches!(item.kind, crate::items::ItemKind::Card)
        && state.rng.gen_range_u32(0, 10_000) < FOIL_DROP_CHANCE_BP
    {
        item.foil = true;
    }
    // GRADE roll (0.0.372): the piece's power axis. Same conditional shape as the foil draw
    // above — one draw, only when the thing that banks is actually gear. Odds come from the
    // dead monster's level, so the tail fattens with the band the party is farming.
    if matches!(item.kind, crate::items::ItemKind::Weapon | crate::items::ItemKind::Armor)
        && !item.unique
    {
        // The odds ride on the MONSTER (baked from `loot_rules.gear.grade_odds` at build time),
        // because `tick()` holds no catalog. A monster whose template carries none — a hand-built
        // test entity, or a save from before the rows were authored — falls back to the code
        // table, so the two sources can never disagree about what "no odds" means.
        let e = &state.grid.entities[dead_idx];
        let odds: [u32; 10] = match <[u32; 10]>::try_from(e.gear_grade_odds.as_slice()) {
            Ok(rows) => rows,
            Err(_) => *grade_odds_for_stage(e.mob_stage),
        };
        let grade = roll_gear_grade(state, &odds);
        // Overrides come from the BAKED template row — there is no catalog here. `assign_grade`
        // is the same path fusion and migration take, so every copy of a product agrees.
        let row = table.iter().find(|d| d.template.def_id == item.def_id);
        let overrides = row.map(|d| d.overrides.as_slice()).unwrap_or(&[]);
        let lines = row.map(|d| d.lines_by_grade.as_slice()).unwrap_or(&[]);
        item.assign_grade(grade, overrides, lines);
    }
    // A GEM drops the same way (0.0.386): one draw on the monster's grade odds, only when the
    // thing that banks is a gem, and the rolled grade re-keys the def — the table's row is the
    // set's Common def, the ledger credit is `gem_<set>_g<step>`.
    if !item.gem_set.is_empty() {
        let e = &state.grid.entities[dead_idx];
        let odds: [u32; 10] = match <[u32; 10]>::try_from(e.gear_grade_odds.as_slice()) {
            Ok(rows) => rows,
            Err(_) => *grade_odds_for_stage(e.mob_stage),
        };
        let grade = roll_gear_grade(state, &odds);
        item.def_id = crate::gems::def_id(&item.gem_set, grade);
        item.grade = grade;
    }
    // DIRECT-GRANT (0.0.47): drops bank STRAIGHT to the party stash — no escrow.
    // Losing hard-won gear on a party wipe was too punishing for an idle game:
    // keeping every drop means a failed instance still yields an upgrade, so the
    // player gets stronger and eventually clears it. The `INVENTORY_CAPACITY` cap is
    // the only loot sink now (full stash → overflow lost; clear space to keep
    // more), which drives the ~hourly stash-clear loop.
    // Remember it for the dungeon run summary BEFORE banking — `bank_to_inventory` consumes the
    // item, and it is shared with shop purchases, which are not run loot.
    if state.run_tally.loot.len() < crate::state::RUN_LOOT_SHOWN {
        state.run_tally.loot.push(item.clone());
    }
    bank_to_inventory(state, item);
}

/// Bank one item into the party stash under a UNIFIED slot cap (`inventory_cap()`):
/// gear, materials, and junk ALL count toward the same capacity, where one stash
/// entry = one slot. Merging more of an item you already hold consumes NO new
/// slot, so it's always allowed (even at cap) — a held junk stack keeps growing so
/// Gil income never stalls. A NEW slot (a fresh stack or a piece of gear) needs a
/// free slot; when the stash is full the overflow is auto-sold (Reclaimer) or
/// lost. Shared by the live drop path and escrow draining.
pub(crate) fn bank_to_inventory(state: &mut GameState, item: Item) {
    bank_to_inventory_inner(state, item, true, false);
}

/// Like `bank_to_inventory` but NEVER routes to the rare bag — for SHOP PURCHASES.
/// A bought item is an intentional acquisition, so it belongs in the normal
/// stash; the rare bag exists to protect RANDOM rare DROPS from the common-gear
/// flood, not to swallow things the player deliberately paid for.
pub fn bank_purchase_to_inventory(state: &mut GameState, item: Item) {
    bank_to_inventory_inner(state, item, false, false);
}

/// Bank a 3-hour-loop LOOT drop: RARE/Legendary non-stackables route to the
/// rare bag (per the player's threshold), everything else to the stash.
pub fn bank_loot_drop(state: &mut GameState, item: Item) {
    bank_to_inventory_inner(state, item, true, false);
}

/// The Sealed Cache (0.0.449): a server-rolled field/boss drop (`loot_rules.orbs`) that binds
/// on arrival — it cannot be listed, mailed or sold, only opened (`OpenLootbox`).
pub const LOOTBOX_ID: &str = "lootbox";
/// The Farm Banner box (0.0.477): granted by the SERVER for online time and delivered by mail,
/// TRADEABLE (unlike the Sealed Cache). What an open pays is the server's roll against the event
/// that is running — see `verify-server/src/farm.rs`, `docs/farm-banner-design.md`.
pub const FARM_BOX_ID: &str = "farm_box";

/// Bank a SERVER grant (a rolled drop, craft or gear piece): like a loot drop / purchase, except a
/// full bag takes it PAST the cap instead of overflowing (audit 2026-10-03). Its candidate is
/// already spent and the server never re-issues it — overflow sold or lost an item the player had
/// earned, and waiting for room let a drop candidate expire. Over the cap, ordinary drops are
/// refused until the player makes room.
pub fn bank_server_grant(state: &mut GameState, item: Item, loot: bool) {
    bank_to_inventory_inner(state, item, loot, true);
}

fn bank_to_inventory_inner(state: &mut GameState, mut item: Item, route_rare: bool, past_cap: bool) {
    if item.def_id == LOOTBOX_ID {
        item.bound = true;
    }
    // LOOT METER (diagnostic, serde-skip): a piece of gear that ARRIVED as loot, by tier.
    if route_rare && matches!(item.kind, crate::items::ItemKind::Weapon | crate::items::ItemKind::Armor) {
        let tier = crate::gear_roll::def_tier_badge(&item.def_id).unwrap_or(0) as usize;
        if let Some(n) = state.gear_dropped_by_tier.get_mut(tier.min(7)) {
            *n += 1;
        }
    }
    // Announce it — but only a RANDOM acquisition. `route_rare` is already the flag that
    // separates a drop from a shop purchase (see bank_purchase_to_inventory), so reusing it
    // keeps one definition of "this was loot" instead of inventing a second.
    if route_rare {
        state.loot_fx.push(item.clone());
    }
    // 0. A ledger material never touches the bag: it is a count in `GameState.materials`.
    if item.ledger {
        state.credit_material(&item.def_id, item.quantity, crate::state::MaterialSource::Drop);
        return;
    }
    // 1. Stackable + a matching inventory stack → merge (no new slot, always allowed).
    //    A duplicate the player already holds keeps accumulating where it lives —
    //    it is NOT pulled back into the rare bag. CARDS land in the inventory too now
    //    (0.0.169: dropped → inventory → player deposits to the album manually), so they
    //    match by `same_card_stack` (def+foil+star) — a Foil must not merge onto a plain
    //    copy and lose its bit; other stackables match by def_id.
    if item.is_stackable() {
        let is_card = matches!(item.kind, crate::items::ItemKind::Card);
        if let Some(existing) = state.inventory.iter_mut().find(|i| {
            i.is_stackable()
                && i.bound == item.bound // bound stacks never merge with unbound (0.0.227)
                && if is_card { same_card_stack(i, &item) } else { i.def_id == item.def_id }
        }) {
            existing.quantity = existing.quantity.saturating_add(item.quantity);
            return;
        }
    }
    // 2. (RETIRED 0.0.380) The rare bag: a non-stackable drop at or above `rare_threshold` used to
    //    land in a separate 3-slot bag. Nothing routes there now — the bag is being rethought
    //    (user 2026-09-15) — and `route_rare` only still decides whether the drop is ANNOUNCED.
    // 3. UNIFIED CAP: anything that needs a NEW slot (fresh stack or gear) takes
    //    one while the stash is below `inventory_cap()`. Merges already returned above.
    if past_cap || state.inventory.len() < state.inventory_cap() {
        state.inventory.push(item);
        return;
    }
    // 4. Stash full → overflow policy (auto-sell keynode, else lost).
    overflow_recover(state, item);
}

/// A container is full and can't take a NEW slot. The PATHS OF SOUL "Reclaimer" keynode
/// (AutoSellOverflow) auto-vendors the overflow for a % of its value instead of losing
/// it; GilPct ("Greed") folds in since it's sale income. No node => pct 0 => the item is
/// simply lost. Shared by the stash and the card album.
fn overflow_recover(state: &mut GameState, item: Item) {
    let pct = crate::soul_paths::path_bonus(
        &state.soul_paths,
        crate::soul_paths::PathEffect::AutoSellOverflow,
    );
    if pct > 0 && item.sell_value > 0 {
        // Recover pct% of the lost item's value, then award_gil folds GilPct on
        // top (carrying the sub-Gil remainder so cheap overflow still pays out).
        let recovered = item.sell_value as u64 * item.quantity as u64 * pct as u64 / 100;
        state.earn_gil(recovered, GilEarn::Salvage);
    }
}

/// Star-roll weights at drop, in BASIS POINTS of 10000 (index = star count
/// 0-4). Trash keeps 3★ at "daily jackpot" volume (~1% of gear); bosses
/// concentrate star luck. 4★ (0.0.135, user: "ยากมากๆ") is the chase drop —
/// 1/10,000 gear from trash (~one per weeks at 100k kills/day), 1/1,000 from
/// bosses; 5★ stays upgrade-only, never dropped. Draft numbers — the
/// loot-per-day audit (sim-cli) recalibrates per band before launch
/// (volume rule, gear-systems-design §1).
pub const STAR_WEIGHTS_TRASH: [u32; 5] = [7500, 1800, 600, 99, 1];
pub const STAR_WEIGHTS_BOSS: [u32; 5] = [4000, 3000, 2000, 990, 10];

/// Is this drop the kind of item that rolls stars? Equipable gear only —
/// materials/cards/consumables never star, and UNIQUE gear has def-fixed stars
/// per the design ("unique ไม่ roll ดาว"). Keyed on the `unique` TAG, not
/// Legendary rarity (user: "Legendary is not unique") — a plain Legendary-
/// rarity drop is just a 4★ jackpot and rolls like any gear.



/// Roll a CRAFTED gear's star count (1-4) from the server nonce's own PRNG. Takes no
/// draw from the state PRNG — the whole outcome is determined by `server_roll`, so it
/// replays identically everywhere and no client can shift it.

/// Public wrapper — `grant_gear` rolls the drop star table from the SERVER's nonce.

/// Roll a dropped gear's star count (0-4) — ONE draw against the bp table.

/// The COMBINED card drop-rate multiplier: sources compose MULTIPLICATIVELY
/// (bp × bp / 10000 per layer). Today only the EVENT layer exists
/// Card drop-rate multiplier (basis points, 10000 = ×1) COMBINED from every
/// source. Each source has its own authority + gating so the total is always
/// legitimate (see `InputEvent::authority`):
///   - **`state.card_rate_bonus_bp`** — the GLOBAL/game-event multiplier (a
///     "double-card weekend"). `InputAuthority::Server`, which is a LABEL and not a
///     gate: the client writes its own input log and hashes it by replaying, so it
///     can author one. What actually rejects it is `server_event_is_checked` in the
///     verify server, plus the `MAX_CARD_RATE_BONUS_BP` clamp below.
///   - future **buff-derived** — per-player boost CONSUMABLES (RO Bubble Gum). A
///     Buff applied by eating an item → client-legal because it's cost-gated (the
///     item is spent), naturally bounded by item availability.
///   - future **path-derived** — soul-path keystones.
/// All live in recorded state, so every layer is replay-safe by construction.
pub(crate) fn combined_card_rate_bp(state: &GameState) -> u32 {
    state.card_rate_bonus_bp
    // future: .mul_bp(buff-derived: Bubble-Gum-style consumables) .mul_bp(path-derived)
}

/// Cap on the GLOBAL card-rate multiplier (×10), and for a while the ONLY thing bounding it.
///
/// This used to read "the real player-facing defense is that `SetCardRateBonus` is
/// `InputAuthority::Server` (a client can't author it)". That was false. Authority is a label:
/// the client writes its own input log and computes the save hash by replaying it, so a modified
/// client could author a ×10 card event for itself — and the boost is not cosmetic, it rides into
/// `stage2_hit_ppm` and the verify server issues grants against that number without re-deriving
/// the odds. So this clamp was the entire defence, and it capped the exploit rather than stopping
/// it. The real gate now lives in the verify server (`server_event_is_checked`, 2026-08-10), which
/// rejects any bundle whose log carries this event; the clamp stays as defence-in-depth against a
/// buggy server grant and the u32-max absurdity.
///
/// Legit game events use ≤×5; sim-cli balance runs set the field DIRECTLY (trusted tool, unbounded).
pub const MAX_CARD_RATE_BONUS_BP: u32 = 100_000;

/// The CARD-rate multiplier applied to ONE drop-table entry's stage-1 weight.
///
/// This replaced a function that scaled the client WEIGHT table, which by 0.0.259 was not
/// merely dead but dangerous: every card sits at `weight: 0` since they moved to server-roll,
/// and the old code did `(0 * bp / 10000).max(1)` — so the first card event ever run would
/// have set every card's client weight to 1 and started dropping cards straight from the
/// client table at ~1/117,000, bypassing the server roll completely and 1.7x more often than
/// intended. Never triggered only because no event has run yet.
///
/// Scaling `server_bp` instead is exact, and the reason is worth stating: the final rate for
/// item i works out to `1% x (bp_i/W) x (10W/1e6)` = `bp_i x 1e-7` — **W cancels**. So raising
/// one item's bp raises ONLY that item's rate and leaves every other item on the same monster
/// exactly where it was. A double-card weekend cannot quietly halve orb drops.
///
/// 0 or 10000 = x1. No `.max()` floor: an entry with no bp is not on the server-roll path at
/// all, and inventing a rate for it is precisely the bug above.
pub(crate) fn card_boosted_bp(entry: &DropEntry, bp: u32) -> u32 {
    if bp == 0 || bp == 10_000 || !matches!(entry.template.kind, crate::items::ItemKind::Card) {
        return entry.server_bp;
    }
    (((entry.server_bp as u64) * (bp as u64)) / 10_000) as u32
}

/// Do two cards belong on the SAME album stack? Type alone isn't enough: foil and
/// star are per-copy VARIANTS (a foil grants the card's authored foil_effect, stars
/// scale all lines +20%/★), so each variant keeps its own row — merging a foil onto a
/// plain stack would silently erase the foil bit (equip clones the ROW's flags).
pub(crate) fn same_card_stack(a: &Item, b: &Item) -> bool {
    a.def_id == b.def_id && a.foil == b.foil && a.star == b.star
}

/// Bank a CARD into the dex-style ALBUM (separate from the gear stash). Cards stack
/// by type + variant (`same_card_stack`), so a duplicate merges onto its existing
/// stack (never a new slot); a brand-new card TYPE/variant takes one album slot while
/// below `card_album_cap()`. A full album hitting a new stack falls back to the
/// shared overflow policy. Deterministic; no PRNG.
pub(crate) fn bank_card_to_album(state: &mut GameState, item: Item) {
    if let Some(existing) = state.card_album.iter_mut().find(|i| same_card_stack(i, &item)) {
        existing.quantity = existing.quantity.saturating_add(item.quantity);
        return;
    }
    if state.card_album.len() < state.card_album_cap() {
        state.card_album.push(item);
        return;
    }
    overflow_recover(state, item);
}

/// Drain any instance escrow into the stash. VESTIGIAL since direct-grant —
/// drops no longer fill the escrow, so this is a no-op in normal play; kept so
/// the instance-clear caller (and any legacy save carrying escrow) still works.
pub fn grant_escrow_to_stash(state: &mut GameState) {
    let escrow = std::mem::take(&mut state.escrow);
    for item in escrow {
        bank_to_inventory(state, item);
    }
}

/// Discard the instance escrow (party wipe). VESTIGIAL since direct-grant — the
/// escrow is normally empty, and stash loot now SURVIVES a wipe (the whole point
/// of direct-grant). Kept so the wipe-restart caller still has a hook.
pub fn clear_escrow(state: &mut GameState) {
    state.escrow.clear();
}

#[cfg(test)]
mod tests {
    /// Every band's odds must sum to EXACTLY one drop. A row that sums to 9,962 (which is what
    /// the design table's rounded percentages literally add up to) would leave a 0.38% hole that
    /// the roll silently pays as Common — the same outcome, arrived at by accident instead of by
    /// decision. Common absorbs the remainder ON PURPOSE; this test is what says so.
    #[test]
    fn grade_odds_sum_to_one_and_fatten_with_the_band() {
        use super::GRADE_ODDS_BP;
        for (band, row) in GRADE_ODDS_BP.iter() {
            assert_eq!(row.iter().sum::<u32>(), 10_000, "band {band}'s odds must sum to 1.0");
            assert!(row[0] >= 7_000, "band {band}: Common stays the majority of drops (D9)");
        }
        // The tail fattens monotonically: Common falls, and the odds of beating Common rise,
        // every band. A band that got SAFER as the player climbed would invert the whole pacing.
        for w in GRADE_ODDS_BP.windows(2) {
            let (lo, hi) = (&w[0].1, &w[1].1);
            assert!(hi[0] < lo[0], "Common must thin out as the band rises");
            let tail = |r: &[u32; 10]| -> u32 { r[1..].iter().sum() };
            assert!(tail(hi) > tail(lo), "the tail must fatten as the band rises");
        }
        // The launch band's ratio: Common:(everything else) ≈ 1:4.3, which is BELOW the 1/6
        // fusion ratio — farming stays a supplement to combining, never a replacement.
        let top = &GRADE_ODDS_BP[4].1;
        let tail: u32 = top[1..].iter().sum();
        assert!(top[0] / tail >= 3 && top[0] / tail <= 5, "launch band ≈ 1:4.3, got 1:{}", top[0] / tail);
    }

    /// THE CATALOG IS THE SOURCE, not the code table. `GRADE_ODDS_BP` is a fallback for a
    /// hand-built state, and a fallback that quietly takes over is the worst of both worlds: the
    /// numbers a designer edits in `game_data.json` would have no effect and nothing would say
    /// so. This asserts a real monster template carries the AUTHORED rows.
    #[test]
    fn a_real_monster_carries_the_catalogs_grade_odds_not_the_code_fallback() {
        let data = crate::data::GameData::from_json_str(include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"), "/../../data/game_data.json"
        ))).expect("catalog parses");
        let rule = data.loot_rules.gear.as_ref().expect("the gear rule exists");
        assert!(!rule.grade_odds.is_empty(), "grade odds must be AUTHORED in the catalog");
        for row in &rule.grade_odds {
            assert_eq!(row.bp.len(), crate::items::GRADES.len(),
                "stage {}: one weight per grade, or the roll reads past its own table", row.max_stage);
            assert_eq!(row.bp.iter().sum::<u32>(), 10_000, "stage {}: odds must sum to 1.0", row.max_stage);
        }
        // Built through the real path: a launch-cap monster's template must carry the LAST row.
        let odds = crate::data::build::items::grade_odds_for(&data, crate::state::LAUNCH_STAGE);
        assert_eq!(odds, rule.grade_odds.last().unwrap().bp, "stage 168 resolves to the top row");
        let early = crate::data::build::items::grade_odds_for(&data, 3);
        assert_eq!(early, rule.grade_odds[0].bp, "an early stage resolves to the first row");
        assert!(early[0] > odds[0], "Common must thin out as the band rises");
    }

    /// The roll must actually produce the distribution the table promises, and must never hand
    /// out a grade the band has not opened (the P2W lever in §2.7 IS "how many grades exist here").
    #[test]
    fn the_grade_roll_matches_its_table_and_never_exceeds_the_band() {
        use crate::items::Grade;
        let mut s = crate::state::GameState::new(42, 8, 8);
        let mut counts = [0u32; 10];
        for _ in 0..20_000 {
            counts[super::roll_gear_grade(&mut s, super::grade_odds_for_stage(168)).step()] += 1;
        }
        // 76% Common at the launch band, ±2 points of sampling noise on 20k draws.
        let common_pct = counts[0] * 100 / 20_000;
        assert!((74..=78).contains(&common_pct), "expected ~76% Common, got {common_pct}%");
        assert!(counts[1] > counts[2] && counts[2] > counts[3], "each step must be rarer than the last");
        assert_eq!(counts[Grade::Celestial.step()], 0, "Celestial is not open at launch");
        assert_eq!(counts[Grade::Cosmic.step()], 0, "Cosmic is not open at launch");
        // An EARLY band must not leak a late grade at all — this is the one that breaks silently
        // if a future row is edited without re-checking its length.
        let mut early = crate::state::GameState::new(7, 8, 8);
        for _ in 0..20_000 {
            let g = super::roll_gear_grade(&mut early, super::grade_odds_for_stage(20));
            assert!(g <= Grade::Legendary, "an early band handed out {g:?}");
        }
        // The BAND MAPPING itself, pinned: the design writes 20 bands across the launch ladder
        // while the catalog numbers zones 1-100 over the same content. Read the catalog number
        // as a design band and every high-level kill falls off the end of the table.
        assert_eq!(super::grade_odds_for_stage(168)[0], 7_638, "stage 168 is the LAST row");
        assert_eq!(super::grade_odds_for_stage(1)[0], 9_000, "stage 1 is the first");
        assert_eq!(super::grade_odds_for_stage(999), super::grade_odds_for_stage(168),
            "past the cap the top row holds — a cap raise adds grades, it does not bend the curve");
    }

    use super::*;
    use crate::items::{Grade, ItemKind, Rarity};

    /// The Potential draw: grade follows the MONSTER'S level, the Sigil follows its first
    /// weakness, a weakness-less monster pays nothing, a boss rolls the boss odds, and every kill
    /// takes exactly ONE draw whatever it yields — the stream must not depend on the outcome.
    #[test]
    fn potential_draw_follows_monster_level_and_first_weakness() {
        use crate::grid::{Cell, Element, Entity, Team};
        use crate::potential::{loot_tier, BOSS_GRADE_BP, GRADE_BP};
        fn mob(level: u16, weak: Vec<Element>, boss: bool) -> Entity {
            let mut e = Entity::idle(1000, Cell::new(1, 0)).with_team(Team::Monster);
            e.mob_stage = level;
            e.weaknesses = weak;
            e.is_boss = boss;
            e
        }
        assert_eq!(loot_tier(0).grade, 0);
        assert_eq!(loot_tier(60).grade, 0);
        assert_eq!(loot_tier(61).grade, 1);
        assert_eq!(loot_tier(121).grade, 2);
        assert_eq!(loot_tier(u16::MAX).grade, 2);
        const N: u32 = 100_000;
        // Field, stage 72 (the old Lv30), weak to Holy → grade II AND grade I, never III.
        let mut s = GameState::new(9, 16, 8);
        s.grid.spawn(mob(72, vec![Element::Light, Element::Fire], false));
        let before = s.rng.fingerprint();
        for _ in 0..N {
            roll_potential_material(&mut s, 0);
        }
        for (id, bp) in [(crate::potential::sigil_def(1), GRADE_BP[1]), (crate::potential::sigil_def(0), GRADE_BP[0])] {
            let id = id.as_str();
            let got = s.material_count(id);
            let expect = N * bp / 10_000;
            assert!((got as i64 - expect as i64).abs() < expect as i64 / 3, "{id} {got} vs ~{expect}");
        }
        assert_eq!(s.material_count(&crate::potential::sigil_def(2)), 0, "never above the zone's grade");
        assert!(s.inventory.is_empty(), "a material never lands in the bag");
        assert_ne!(s.rng.fingerprint(), before);
        // Exactly one draw per kill: the same seed and count land on the same fingerprint
        // whether the monster can yield anything or not.
        let mut a = GameState::new(11, 16, 8);
        a.grid.spawn(mob(72, vec![Element::Light], false));
        let mut b = GameState::new(11, 16, 8);
        b.grid.spawn(mob(72, vec![], false));
        for _ in 0..100 {
            roll_potential_material(&mut a, 0);
            roll_potential_material(&mut b, 0);
        }
        assert_eq!(a.rng.fingerprint(), b.rng.fingerprint(), "one draw per kill, outcome-independent");
        assert_eq!(b.materials().count(), 0, "no weakness → nothing from this draw");
        // Boss odds.
        let mut c = GameState::new(13, 16, 8);
        c.grid.spawn(mob(144, vec![Element::Ice], true));
        for _ in 0..N {
            roll_potential_material(&mut c, 0);
        }
        let got = c.material_count(&crate::potential::sigil_def(2));
        let expect = N * BOSS_GRADE_BP[2] / 10_000;
        assert!((got as i64 - expect as i64).abs() < expect as i64 / 4, "boss sigil {got} vs ~{expect}");
    }

    /// Grace window → full loot; 20% opening cut at gap 16, −5%/level after;
    /// hard floor 10%; level-less (0) mobs exempt.

    /// At 10000 bp the scaled roll consumes the SAME draw value as the plain
    /// roll (goldens with in-grace parties stay byte-identical); at the floor
    /// most rolls miss the table entirely.
    #[test]
    fn scaled_roll_identity_and_floor() {
        let t = table(&[10, 90]);
        let mut a = Pcg32::new(7);
        let mut b = Pcg32::new(7);
        for _ in 0..64 {
            let x = roll_drop(&mut a, &t).map(|i| i.def_id.clone());
            let y = roll_drop_scaled(&mut b, &t, 10_000).map(|i| i.def_id.clone());
            assert_eq!(x, y, "full-loot scaled roll must be bit-identical");
        }
        let mut rng = Pcg32::new(11);
        let hits = (0..10_000).filter(|_| roll_drop_scaled(&mut rng, &t, 1_000).is_some()).count();
        assert!((800..1200).contains(&hits), "floor keeps ~10% of rolls: {hits}");
    }

    fn table(weights: &[u32]) -> Vec<DropEntry> {
        weights
            .iter()
            .enumerate()
            .map(|(i, &w)| {
                DropEntry::new(
                    Item::new(
                        &format!("item_{i}"),
                        ItemKind::Material,
                        0,
                        0,
                        Rarity::Common,
                    ),
                    w,
                )
            })
            .collect()
    }

    #[test]
    fn empty_table_returns_none() {
        let mut rng = Pcg32::new(1);
        let t: Vec<DropEntry> = Vec::new();
        assert!(roll_drop(&mut rng, &t).is_none());
    }

    #[test]
    fn zero_weight_table_returns_none() {
        let mut rng = Pcg32::new(1);
        let t = table(&[0, 0, 0]);
        assert!(roll_drop(&mut rng, &t).is_none());
    }

    #[test]
    fn always_picks_only_nonzero_entry() {
        let mut rng = Pcg32::new(42);
        // Only entry 2 has nonzero weight — it should win every roll.
        let t = table(&[0, 0, 100, 0]);
        for _ in 0..1000 {
            let r = roll_drop(&mut rng, &t).unwrap();
            assert_eq!(r.def_id, "item_2");
        }
    }

    /// Heavy-weight entry should dominate proportionally.
    #[test]
    fn weight_distribution_roughly_matches() {
        let mut rng = Pcg32::new(123);
        // Weights 1:99 → expect ~1% / ~99%.
        let t = table(&[1, 99]);
        let n = 50_000;
        let mut hits_0 = 0;
        for _ in 0..n {
            if roll_drop(&mut rng, &t).unwrap().def_id == "item_0" {
                hits_0 += 1;
            }
        }
        let rate = (hits_0 * 10_000) / n;  // basis points
        assert!(
            (50..=200).contains(&rate),
            "expected ~100 bp (1%), got {} bp",
            rate
        );
    }

    #[test]
    fn drops_go_straight_to_stash_respecting_cap() {
        use crate::grid::{Cell, Entity, Team};
        let mut s = GameState::new(7, 16, 8);
        // Weapon = non-stackable, so each drop wants its OWN slot (tests the cap).
        let gear = Item::new("sword", ItemKind::Weapon, 5, 0, Rarity::Common);
        s.grid.spawn(
            Entity::idle(1000, Cell::new(1, 0))
                .with_team(Team::Monster)
                .with_drops(vec![DropEntry::new(gear, 100)]),
        );
        // Star-promoted rarity (0.0.135) would route 2-star+ rolls to the rare
        // bag — pin the threshold above anything reachable so THIS test stays
        // about the stash cap (rare-bag routing has its own tests).
        s.rare_threshold = Rarity::Immortal;
        // D1 (0.0.369): gear is FIXED-STAT, so there is nothing for the server to value and the
        // piece banks at the moment of death again — the stage-3 candidate detour is gone.
        drop_on_death(&mut s, 0);
        assert_eq!(gear_candidate_count(&s), 0, "no server ticket: nothing is rolled");
        assert_eq!(s.inventory.len(), 1, "the piece banks straight into the stash");
        s.inventory.clear();
        bank_loot_drop(&mut s, Item::new("sword", ItemKind::Weapon, 5, 0, Rarity::Common));
        assert_eq!(s.inventory.len(), 1, "loot lands directly in the stash");
        assert!(s.escrow.is_empty(), "escrow is unused now");
        // A wipe does NOT touch the stash — loot survives (the point of direct-grant).
        clear_escrow(&mut s);
        assert_eq!(s.inventory.len(), 1, "stash loot survives a wipe");

        // The GEAR cap is the loot sink: fill it with non-stackable gear, then a
        // further gear drop is lost. (Stackable junk/materials are EXEMPT from the
        // cap — they never block gear — so fillers here must be gear.)
        for k in 1..super::INVENTORY_CAPACITY {
            s.inventory.push(Item::new(&format!("filler_{k}"), ItemKind::Weapon, 1, 0, Rarity::Common));
        }
        assert_eq!(s.inventory.len(), super::INVENTORY_CAPACITY);
        bank_loot_drop(&mut s, Item::new("sword", ItemKind::Weapon, 5, 0, Rarity::Common));
        assert_eq!(s.inventory.len(), super::INVENTORY_CAPACITY, "full gear stash blocks new gear");
    }

    /// Gear at a GRADE — its rarity is always Common (0.0.380); the router reads `grade`.
    fn gear(id: &str, grade: Grade) -> Item {
        let mut it = Item::new(id, ItemKind::Weapon, 5, 0, Rarity::Common);
        it.grade = grade;
        it
    }
    fn ore(id: &str, rarity: Rarity, qty: u32) -> Item {
        let mut it = Item::new(id, ItemKind::Material, 0, 0, rarity);
        it.quantity = qty;
        it
    }

    /// RETIRED (0.0.380): nothing routes to the rare bag any more, at any rarity, any grade, any
    /// setting — and a save that still holds pieces there gets them back in the inventory.
    #[test]
    fn nothing_routes_to_the_retired_rare_bag_and_old_bags_come_home() {
        let mut s = GameState::new(1, 8, 8);
        s.rare_threshold = Rarity::Common;
        bank_to_inventory(&mut s, gear("top", Grade::Cosmic));
        bank_to_inventory(&mut s, Item::new("ex", ItemKind::Exemplar, 0, 0, Rarity::Immortal));
        assert!(s.rare_bag.is_empty(), "the bag takes nothing");
        assert_eq!(s.inventory.len(), 2, "everything banks to the inventory");

        // An old save's leftovers: migrate moves them home, past the cap if it must.
        let data = crate::data::GameData::from_json_str(include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"), "/../../data/game_data.json"
        ))).expect("catalog");
        let mut old = GameState::new(2, 8, 8);
        for k in 0..INVENTORY_CAPACITY { old.inventory.push(gear(&format!("f{k}"), Grade::Common)); }
        old.rare_bag.push(gear("sword_s3", Grade::Rare));
        crate::data::migrate_save_items(&mut old, &data);
        assert!(old.rare_bag.is_empty());
        assert_eq!(old.inventory.len(), INVENTORY_CAPACITY + 1, "the piece is not lost to the cap");
        assert!(old.inventory.iter().any(|i| i.def_id == "sword_s3" && i.grade == Grade::Rare));
    }

    #[test]
    fn unified_cap_counts_everything_but_lets_held_stacks_grow() {
        let mut s = GameState::new(3, 8, 8); // default cap = INVENTORY_CAPACITY
        // Fill EVERY slot — mix gear + a junk stack — to the unified cap.
        s.inventory.push(ore("held_junk", Rarity::Common, 5)); // 1 stackable slot
        for k in 1..super::INVENTORY_CAPACITY {
            s.inventory.push(gear(&format!("g{k}"), Grade::Common)); // gear slots
        }
        assert_eq!(s.inventory.len(), super::INVENTORY_CAPACITY, "stash filled to the unified cap");

        // A NEW item type (gear OR a new material stack) is blocked at cap.
        bank_to_inventory(&mut s, gear("overflow_gear", Grade::Common));
        bank_to_inventory(&mut s, ore("new_junk", Rarity::Common, 1));
        assert_eq!(s.inventory.len(), super::INVENTORY_CAPACITY, "no new slot past the cap");
        assert!(s.inventory.iter().all(|i| i.def_id != "overflow_gear"));
        assert!(s.inventory.iter().all(|i| i.def_id != "new_junk"));

        // But MORE of a junk stack the player already holds still merges (no new
        // slot) — Gil income never stalls just because the stash is full.
        bank_to_inventory(&mut s, ore("held_junk", Rarity::Common, 7));
        let held = s.inventory.iter().find(|i| i.def_id == "held_junk").unwrap();
        assert_eq!(held.quantity, 12, "held stack grows even at cap");
    }

    /// Kills per hour, derived from the same measured run as `KILLS_PER_8H`.
    const KILLS_PER_HOUR: u64 = KILLS_PER_8H / 8;

    /// Chance per kill (basis points) that a drop needs a NEW inventory SLOT — i.e. gear.
    /// Stackables merge into a stack the player already holds after the first hour, so gear is
    /// what actually consumes the cap in steady state. Read off the SPAWNED tables — since
    /// 0.0.365 the gear rows come from `loot_rules.gear`, not the monster's own list.
    fn slot_consuming_drops_per_kill_bp() -> u32 {
        let d = crate::data::GameData::from_json_str(include_str!("../../../data/game_data.json"))
            .expect("catalog parses");
        let mut total_bp = 0u64;
        let mut n = 0u64;
        for m in &d.monsters {
            // Every monster as if placed on STAGE 1 — the measured kill rate is a stage-1 one, and
            // since 0.0.402 the odds per kill grow with the stage exactly as the kill rate falls
            // (drops per hour are flat by design), so the load is the same on every stage.
            let table = crate::data::build_drop_table(m, &d, 1);
            let w: u64 = table.iter().map(|e| e.weight as u64).sum();
            if w == 0 {
                continue;
            }
            let gear: u64 = table.iter()
                .filter(|e| {
                    matches!(e.template.kind, crate::items::ItemKind::Weapon | crate::items::ItemKind::Armor)
                        && !e.template.unique
                })
                .map(|e| e.weight as u64)
                .sum();
            total_bp += gear * 10_000 / w;
            n += 1;
        }
        (total_bp / n.max(1)) as u32
    }

    /// LOAD vs CAP, pair 1: the bag.
    ///
    /// `INVENTORY_CAPACITY` is the loot sink the whole hourly play loop is built on — the
    /// design note says "~20 keepable gear/hour, stash fills in ~1h". Nothing checked that
    /// against the catalog, so a drop-table edit could quietly make the bag fill in minutes
    /// (loot lost to overflow) or never (no reason to log in) and no test would notice.
    #[test]
    fn the_bag_fills_at_the_pace_the_loop_is_designed_around() {
        let bp = slot_consuming_drops_per_kill_bp();
        assert!(bp > 0, "no gear drops anywhere in the catalog?");
        let kills_to_fill = INVENTORY_CAPACITY as u64 * 10_000 / bp as u64;
        let minutes = kills_to_fill * 60 / KILLS_PER_HOUR;
        assert!(
            (20..=360).contains(&minutes),
            "the bag fills in {minutes} min ({kills_to_fill} kills at {bp} bp/kill). \
             Design intent is ~1 h; under 20 min means loot is being lost to overflow faster \
             than anyone can clear it, over 6 h means the clear loop has stopped existing.",
        );
    }

    /// LOAD vs CAP, pair 3: what a save actually CARRIES, in bytes.
    ///
    /// The first version of this summed the three caps as if they could all be full at once,
    /// and it was wrong twice over. It ignored the DRAINS — the TTL alone holds the drop list
    /// near its steady state whatever the cap is — so it measured a state that requires an
    /// independent failure to reach, and then it blocked raising a cap for the right reasons.
    /// A test that fails a correct change is not a strict test, it is a mismeasuring one.
    ///
    /// Two bounds instead, because they fail for different reasons and deserve different room.
    #[test]
    fn a_save_does_not_fill_up_with_pending_tickets() {
        let sample = crate::state::RareCandidate {
            seq: 9_999_999,
            def_id: "star_stone_death".into(),
            tick: 9_999_999,
            kind: crate::state::CandidateKind::Gear { boss: true },
            hit_ppm: 132_500,
        };
        let bytes = serde_json::to_vec(&sample).expect("serializes").len() as u64;

        // NORMAL: prune is dead and only the TTL drains. This is what every save carries, so
        // it has to be small — live saves run ~310 KB and this is pure overhead.
        let steady = KILLS_PER_8H * CANDIDATE_RATE_BP as u64 / 10_000;
        let steady_kb = steady * bytes / 1024;
        assert!(
            steady_kb <= 24,
            "a TTL-only save carries {steady_kb} KB of tickets ({steady} x {bytes} B) — that \
             rides every save, hash and replay",
        );

        // CATASTROPHE: prune AND the TTL both fail and every cap fills. Loose on purpose — it
        // only bounds the blast radius, and reaching it needs two independent breakages.
        let worst = (RARE_CANDIDATE_CAP + CRAFT_CANDIDATE_CAP + GEAR_CANDIDATE_CAP) as u64;
        let worst_kb = worst * bytes / 1024;
        assert!(
            worst_kb <= 192,
            "even with both drains broken the ticket list would be {worst_kb} KB ({worst} \
             candidates) — raising a cap is not free",
        );
    }

    /// THE TEST 0.0.207 SHOULD HAVE HAD. Stage 1 is a per-KILL gate with ONE global rate, so
    /// no drop table — however many `server_bp` rows it grows — can raise the load. Reads the
    /// real catalog, so adding a monster or an entry cannot quietly re-break this.
    #[test]
    fn candidate_load_is_one_gated_roll_per_kill_whatever_the_table_holds() {
        let d = crate::data::GameData::from_json_str(include_str!("../../../data/game_data.json"))
            .expect("catalog parses");
        // The SPAWNED table — authored rows plus the rule rows `build_drop_table` appends
        // (Boss Core / Track; the Star Stone rules were parked and gear stopped being a ticket
        // in 0.0.369) — is what a kill actually rolls against.
        let tables: Vec<Vec<crate::items::DropEntry>> = d.monsters.iter()
            .map(|m| crate::data::build_drop_table(m, &d, 0))
            .collect();
        let sum = |t: &Vec<crate::items::DropEntry>| t.iter().map(|e| e.server_bp as u64).sum::<u64>();
        let worst = tables.iter().map(sum).max().unwrap_or(0);
        assert!(worst > 0, "sanity: some monster still has server_bp entries ({worst} bp)");
        // Whatever that sum is, ONE kill can accrue at most ONE candidate.
        let mut st = crate::data::new_game_in_zone(1, &d, "zone_b1_1").unwrap();
        let table: Vec<crate::items::DropEntry> = tables.into_iter().max_by_key(sum).unwrap();
        let mut accrued = 0usize;
        const KILLS: usize = 200_000;
        for _ in 0..KILLS {
            let before = st.rare_candidates.len();
            super::accrue_candidate(&mut st, &table);
            let got = st.rare_candidates.len() - before;
            assert!(got <= 1, "one kill accrued {got} candidates");
            accrued += got;
            st.rare_candidates.clear(); // measure the RATE, not the cap
        }
        let per_kill_bp = (accrued * 10_000 / KILLS) as u32;
        assert!(
            (CANDIDATE_RATE_BP.saturating_sub(15)..=CANDIDATE_RATE_BP + 15).contains(&per_kill_bp),
            "measured {per_kill_bp} bp/kill, expected ~{CANDIDATE_RATE_BP} — stage 1 must not \
             carry per-item rarity (that inversion is what 0.0.207 shipped)",
        );
    }

    /// GEAR tickets always pay, so they only queue between saves — and the client freezes
    /// its own sim after `SPAN_STALL_TICKS` unsynced. Size the cap against THAT window, not
    /// against hours: hitting it drops the piece, which is the one outcome worth being
    /// paranoid about.
    #[test]
    fn gear_tickets_fit_the_unsynced_window() {
        // Every star-eligible gear drop makes a ticket, so the worst case is "every kill in
        // the window drops gear" — far past the real ~1-in-130, and still comfortable.
        assert!(
            KILLS_PER_STALL_WINDOW < GEAR_CANDIDATE_CAP as u64,
            "{KILLS_PER_STALL_WINDOW} kills can pass before the sim stalls, against a \
             {GEAR_CANDIDATE_CAP} ticket cap — a piece of loot would be dropped on the floor",
        );
    }

    /// Does the cap actually cover 8 h of play? It has to: the server only reports the ~0.1%
    /// of candidates that HIT, so every dud sits in the list until the 8 h TTL clears it. That
    /// happens while perfectly online — the whole window has to fit.
    ///
    /// This is the pairing nothing checked: the 500 cap was sized against the documented ~1%
    /// rate, then 0.0.207 raised the real rate 62x and left the cap alone. Asserting 2x
    /// headroom means a faster party than the one measured still fits, and any future attempt
    /// to move rarity back into stage 1 fails here first.
    #[test]
    fn candidate_load_fits_the_offline_window() {
        let worst_case = KILLS_PER_8H * CANDIDATE_RATE_BP as u64 / 10_000;
        assert!(
            worst_case * 2 <= RARE_CANDIDATE_CAP as u64,
            "8 h offline accrues ~{worst_case} candidates against a {RARE_CANDIDATE_CAP} cap — \
             under 2x headroom. Raise the cap or lower CANDIDATE_RATE_BP.",
        );
        // For the record: the pre-0.0.256 inverted pipeline ran at 0.62 candidates/kill, 62x
        // what stage 1 budgets. Asserted against TODAY's cap rather than a multiple, because
        // the multiple drifts every time the cap moves — it briefly failed here for exactly
        // that reason, which is the coupling being removed.
        let old_load = KILLS_PER_8H * 6_200 / 10_000;
        assert!(
            old_load > RARE_CANDIDATE_CAP as u64,
            "the inverted load was {old_load} against a {RARE_CANDIDATE_CAP} cap — it did not \
             fit even at the generous cap we have now, which is what made it invisible",
        );
    }
    /// 0.0.131: the TEST/EVENT card-rate multiplier scales ONLY Card entry weights
    /// (floor 1), leaves everything else untouched, and no-ops at 0 / 10000.
    
    /// 0.0.128 regression guard: foil is a per-copy VARIANT — a foil copy must keep
    /// its own album row, never fold onto (or absorb) the plain stack, or the foil
    /// bit is silently gained/lost when equip clones the row.
    #[test]
    fn foil_cards_keep_their_own_album_stack() {
        let mut s = GameState::new(7, 16, 8);
        let card = |foil: bool| {
            let mut c = Item::new("boar", ItemKind::Card, 0, 0, Rarity::Rare);
            c.foil = foil;
            c
        };
        bank_card_to_album(&mut s, card(false));
        bank_card_to_album(&mut s, card(true));
        assert_eq!(s.card_album.len(), 2, "foil takes its own row, not the plain stack");
        // Duplicates still merge — but only onto their MATCHING variant row.
        bank_card_to_album(&mut s, card(true));
        bank_card_to_album(&mut s, card(false));
        assert_eq!(s.card_album.len(), 2);
        let plain = s.card_album.iter().find(|c| !c.foil).unwrap();
        let foil = s.card_album.iter().find(|c| c.foil).unwrap();
        assert_eq!((plain.quantity, foil.quantity), (2, 2));
    }

    #[test]
    fn drop_sequence_is_stable_for_seed_42() {
        let t = table(&[10, 20, 30, 40]);
        let mut rng_a = Pcg32::new(42);
        let mut rng_b = Pcg32::new(42);
        for _ in 0..50 {
            let a = roll_drop(&mut rng_a, &t).map(|x| x.def_id.clone());
            let b = roll_drop(&mut rng_b, &t).map(|x| x.def_id.clone());
            assert_eq!(a, b);
        }
    }
}
