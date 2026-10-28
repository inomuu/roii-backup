// sim-cli build/auto-play helpers.
//
// These functions model a player who actually plays the character — allocating
// stat points, learning skills, equipping the best gear, vendoring surplus, and
// recruiting party members — so a headless 24h run measures a real progression
// curve rather than the never-touches-anything worst case. sim-cli ONLY; the
// game keeps all of this manual.

use sim_core::data::{self, GameData};
use sim_core::events::{apply_input, InputEvent};
use sim_core::grid::{Element, Team};
use sim_core::state::GameState;


/// Spend the party's SOUL currency on the Soul-Paths web: allocate every node that's
/// currently reachable (origin, or adjacent to an owned node) and affordable, cheapest
/// first, until nothing more can be bought. Models a player dumping souls into the
/// passive web as they accrue — a MAJOR, party-wide part of the real power curve that
/// the headless run otherwise ignores entirely. (sim-cli only; the game keeps manual
/// allocation.) Uses the same `LevelSoulPath` input the UI does.
/// Model the player buying TRAINING levels (0.0.406): among the slots a plain player would
/// touch — the six power ladders, the Attunement of every element the party attacks with and
/// the Ward of the element the current zone's monsters hit with — buy the CHEAPEST affordable
/// level, repeatedly, until nothing is. Cheapest-first is what the WoG window's price sort
/// invites; it is not the optimum, which is the point of a pacing measurement. Returns the
/// levels bought.
pub(crate) fn auto_train(state: &mut GameState, data: &GameData) -> u32 {
    use sim_core::training::{self, TrainStat};
    let attack_elements: Vec<Element> = state
        .grid
        .entities
        .iter()
        .filter(|e| matches!(e.team, Team::Player) && !e.is_pet && e.owner_id == 0)
        .map(|e| e.element)
        .collect();
    let incoming: Vec<Element> = state
        .grid
        .entities
        .iter()
        .filter(|e| matches!(e.team, Team::Monster))
        .map(|e| e.element)
        .collect();
    let wanted = |s: &sim_core::data::TrainingSlot| match s.stat {
        TrainStat::AtkPct | TrainStat::HpPct | TrainStat::DefPct | TrainStat::CritDmg
        | TrainStat::BossDmg | TrainStat::BreakDmg
        | TrainStat::AtkFlat | TrainStat::HpFlat | TrainStat::DmgReduceFlat => true,
        TrainStat::ElementDmg => s.element.is_some_and(|el| attack_elements.contains(&el)),
        TrainStat::ResistElement => s.element.is_some_and(|el| incoming.contains(&el)),
        _ => false,
    };
    let mut bought = 0;
    loop {
        // Cheapest affordable level among the WANTED slots that the breadth gate lets through;
        // when every wanted slot is held at a gate step, the cheapest level of ANY slot — that
        // is what the gate asks a player to do (0.0.417: sim-cli sat on 89 levels and 21M Gil
        // for four days because it would not).
        let pick = |any: bool| -> Option<(String, u64)> {
            let mut best: Option<(String, u64)> = None;
            for sl in data.training.slots.iter().filter(|s| any || wanted(s)) {
                if !training::gate_open(state, data, &sl.id) {
                    continue;
                }
                let Some(c) = training::next_cost(state, data, &sl.id) else { continue };
                if c <= state.gil() && best.as_ref().map_or(true, |(_, b)| c < *b) {
                    best = Some((sl.id.clone(), c));
                }
            }
            best
        };
        let Some((id, _)) = pick(false).or_else(|| pick(true)) else { break };
        if !training::train(state, data, &id) {
            break;
        }
        bought += 1;
    }
    bought
}

pub(crate) fn auto_allocate_soul_paths(state: &mut GameState) {
    use sim_core::soul_paths::{is_allocatable, SOUL_PATHS};
    loop {
        // Cheapest reachable+affordable node this pass (greedy frontier expansion).
        let mut best: Option<(&'static str, u64)> = None;
        for nd in SOUL_PATHS {
            if nd.cost <= state.soul() && is_allocatable(&state.soul_paths, nd) {
                if best.map_or(true, |(_, c)| nd.cost < c) {
                    best = Some((nd.id, nd.cost));
                }
            }
        }
        let Some((id, _)) = best else { break };
        apply_input(state, InputEvent::LevelSoulPath { skill_id: id.to_string() });
    }
}

/// Spend earned STAT POINTS (0.0.347's two tiers) on a committed build — core points from
/// level-ups, substat points from past the level cap. In-game the player does this by hand; a
/// headless run that never spends them measures a character strictly weaker than any real one,
/// and those points are now the main thing a level BUYS. (sim-cli only.)
///
/// PRIORITY-FILL, not a spread: a bucket is filled to its cap before the next is touched, which
/// models a player committing to a build. A round-robin would measure a character nobody plays.
/// The order is offence-first because the runs this tool exists for are paced by kill speed.
pub(crate) fn auto_allocate_stat_points(state: &mut GameState) {
    use sim_core::stats as st;
    const CORE_ORDER: [usize; st::APEX_CORE] =
        [st::AP_ATK, st::AP_CRITDMG, st::AP_CRIT, st::AP_HP, st::AP_DEF, st::AP_DODGE];
    const SUB_ORDER: [usize; st::APEX_BUCKETS - st::APEX_CORE] = [
        st::AP_DEFPEN,
        st::AP_BREAKDMG,
        st::AP_BREAKBUILD,
        st::AP_COOLDOWN,
        st::AP_CAST,
        st::AP_CHARGE,
        st::AP_HEAL,
    ];
    let ids: Vec<u32> = state
        .grid
        .entities
        .iter()
        .filter(|e| matches!(e.team, Team::Player) && !e.is_pet)
        .map(|e| e.id)
        .collect();
    for id in ids {
        for (order, core) in [(&CORE_ORDER[..], true), (&SUB_ORDER[..], false)] {
            for &bucket in order {
                let Some(e) = state.grid.entities.iter().find(|e| e.id == id) else { break };
                let pool = if core { e.stats.core_unspent() } else { e.stats.apex_unspent() };
                if pool == 0 {
                    break;
                }
                let room = st::APEX_BUCKET_CAP.saturating_sub(e.stats.apex[bucket]);
                if room == 0 {
                    continue;
                }
                apply_input(
                    state,
                    InputEvent::AllocateApex {
                        entity_id: id,
                        bucket: bucket as u8,
                        count: pool.min(room),
                    },
                );
            }
        }
    }
}

/// Spend what the run has looted (Sigil of the unit's element, Essence, Core, Track, Gil) on the
/// Potential board — a player buys every rank the bag allows, skill nodes first. Without this a
/// run that drops materials never spends them, and a kit stays at rank 1 forever. (sim-cli only.)
pub(crate) fn auto_potential(state: &mut GameState) {
    let ids: Vec<u32> = state
        .grid
        .entities
        .iter()
        .filter(|e| matches!(e.team, Team::Player) && !e.is_pet)
        .map(|e| e.id)
        .collect();
    for id in ids {
        // Skill nodes first (they carry the kit), then whatever else the bag reaches.
        let mut order: Vec<&'static str> = Vec::new();
        for def in sim_core::potential::NODE_DEFS.iter() {
            if matches!(def.kind, sim_core::potential::NodeKind::Skill { .. }) { order.push(def.id); }
        }
        for def in sim_core::potential::NODE_DEFS.iter() {
            if !matches!(def.kind, sim_core::potential::NodeKind::Skill { .. }) { order.push(def.id); }
        }
        loop {
            let purse = sim_core::potential::actions::purse(state);
            let Some(e) = state.grid.entity_of_id(id) else { break };
            if let Some(next) = order.iter().find(|n| sim_core::potential::actions::quote(e, &purse, n).is_ok()) {
                apply_input(state, InputEvent::UpgradePotential { entity_id: id, node_id: next.to_string() });
                continue;
            }
            // Nothing affordable as held: a player would convert — split a higher grade down when
            // a lower one is short, merge a lower grade up when a higher one is. One conversion,
            // then look again; stop when neither direction helps.
            if !convert_toward_affordable(state, &order, id) {
                break;
            }
        }
    }
}

/// One Sigil conversion that makes SOME node on `order` affordable, if any does. Tries every
/// (grade, direction) once against a copy of the purse; applies the first that works.
fn convert_toward_affordable(state: &mut GameState, order: &[&str], id: u32) -> bool {
    use sim_core::potential::{actions, SIGIL_GRADES, SIGIL_MERGE};
    let purse = actions::purse(state);
    let Some(e) = state.grid.entity_of_id(id) else { return false };
    for g in 0..SIGIL_GRADES {
        // Split one of grade g into SIGIL_MERGE of g-1.
        if g > 0 && purse.sigil[g] >= 1 {
            let mut p = purse;
            p.sigil[g] -= 1;
            p.sigil[g - 1] += SIGIL_MERGE;
            if order.iter().any(|n| actions::quote(e, &p, n).is_ok()) {
                apply_input(state, InputEvent::SplitSigil { element: String::new(), grade: g as u8, count: 1 });
                return true;
            }
        }
        // Merge SIGIL_MERGE of grade g into one of g+1.
        if g + 1 < SIGIL_GRADES && purse.sigil[g] >= SIGIL_MERGE {
            let mut p = purse;
            p.sigil[g] -= SIGIL_MERGE;
            p.sigil[g + 1] += 1;
            if order.iter().any(|n| actions::quote(e, &p, n).is_ok()) {
                apply_input(state, InputEvent::MergeSigil { element: String::new(), grade: g as u8, count: 1 });
                return true;
            }
        }
    }
    false
}


/// Fill each of the 10 RO paper-doll slots with the best piece in the stash
/// (score = atk_bonus + def_bonus, since each piece is mostly one stat) when it
/// beats whatever is worn in that slot. sim-cli ONLY — models the player keeping
/// every slot upgraded as gear drops. Equips one slot per call (stash indices
/// shift on equip); repeated ticks converge the whole paper-doll.
pub(crate) fn auto_equip_best(state: &mut GameState) {
    use sim_core::items::{EquipSlot, ItemKind};
    const SLOTS: [EquipSlot; 10] = [
        EquipSlot::Weapon, EquipSlot::Armor, EquipSlot::Shield,
        EquipSlot::Earring, EquipSlot::Shoes, EquipSlot::Helmet,
        EquipSlot::Gloves, EquipSlot::Bracer,
        EquipSlot::Necklace, EquipSlot::Ring,
    ];
    // GRADED score (0.0.372). Reading the authored `atk_bonus` would rank a Common s7 above a
    // Cosmic s3 — a party that wears the wrong half of its drops measures the wrong game, and
    // the whole point of the grade axis is that it decides which piece is better.
    // SUB-STATS COUNT. Accessories carry no main stat by design (§2.5) — their whole value is
    // their authored lines — so a scorer that reads only ATK/DEF rates them at zero and the party
    // wears nothing in three of its ten slots. Measured: 35 of 50 slots filled until this was
    // added. Basis-point lines are divided by 100 so a 600 bp line weighs like 6 flat points
    // rather than 600; it is a "worth wearing?" heuristic, not the sim's own maths.
    let score = |i: &sim_core::items::Item, ex: Option<&str>, unit: &str| {
        let lines: i32 = i.lines
            .iter()
            .map(|m| {
                // A basis-point line reads 600 for "+6%", so divide those down to the same order
                // as a flat line; `Debug` carries the Bp suffix the type itself encodes.
                let v = m.value();
                if format!("{m:?}").contains("Bp") { v / 100 } else { v }
            })
            .sum();
        // Worn value: off-family pieces count at their reduced ATK/DEF.
        i.worn_atk(ex, unit) + i.worn_def(ex, unit) + lines
    };
    // EVERY member, not just the lead. The pacing question is what a PARTY of five wears (55
    // slots, design D11); measuring one character and multiplying is exactly the mistake the
    // 55-slot rule exists to correct.
    let members: Vec<(usize, u32)> = state
        .grid
        .entities
        .iter()
        .enumerate()
        .filter(|(_, e)| matches!(e.team, Team::Player) && e.alive && !e.is_pet)
        .map(|(i, e)| (i, e.id))
        .collect();
    for (eidx, entity_id) in members {
        let unit_def = state.grid.entities[eidx].unit_def.clone();
        let exemplar_id = state.grid.entities[eidx].exemplar.as_ref().map(|x| x.id.clone());
        for slot in SLOTS {
            let worn = state.grid.entities[eidx]
                .equipped
                .iter()
                .filter(|i| i.slot() == slot)
                .map(|i| score(i, exemplar_id.as_deref(), &unit_def))
                .max()
                .unwrap_or(0);
            let best = state
                .inventory
                .iter()
                .enumerate()
                .filter(|(_, i)| !matches!(i.kind, ItemKind::Material) && i.slot() == slot)
                .map(|(idx, i)| (idx, score(i, exemplar_id.as_deref(), &unit_def)))
                .max_by_key(|(_, b)| *b);
            if let Some((idx, bonus)) = best {
                if bonus > worn {
                    apply_input(state, InputEvent::EquipItem { entity_id, inventory_idx: idx as u32 });
                }
            }
        }
    }
}

/// Model a player FUSING surplus gear (design §3): six pieces of one grade become one of the
/// next. A real player does this constantly — six-into-one is the only road up the ladder, and
/// the design's whole pacing (55 slots, CE per hour) assumes every spare piece is fed into it.
/// Without this the measured party wears only what dropped, which understates late-game power by
/// the entire height of the grade ladder.
///
/// Feeds the LOWEST grade first and stops at the patch ceiling — the same order a player uses,
/// because fusing the good stuff first destroys the fodder that would have lifted it.
pub(crate) fn auto_fuse_surplus(state: &mut GameState, data: &sim_core::data::GameData) -> u32 {
    use sim_core::fusion::{fuse, FuseOutcome, FUSE_GRADE_CEILING, FUSE_INPUTS};
    use sim_core::items::{ItemKind, GRADES};
    let mut made = 0;
    for grade in GRADES.iter().take_while(|g| **g < FUSE_GRADE_CEILING) {
        loop {
            let picks: Vec<u32> = state
                .inventory
                .iter()
                .enumerate()
                .filter(|(_, i)| {
                    matches!(i.kind, ItemKind::Weapon | ItemKind::Armor)
                        && !i.unique && !i.bound && i.grade == *grade
                })
                .map(|(idx, _)| idx as u32)
                .take(FUSE_INPUTS)
                .collect();
            let Ok(six): Result<[u32; FUSE_INPUTS], _> = picks.try_into() else { break };
            match fuse(state, data, &six, 0) {
                FuseOutcome::Success { .. } => made += 1,
                FuseOutcome::Failed { .. } => {}
                // No Gil (or nothing valid): stop, or this loops forever on the same six.
                FuseOutcome::NoOp => break,
            }
        }
    }
    made
}

/// Socket every held gem the party can use (sim-cli only): for each worn piece with a free
/// socket, the highest-grade held gem whose set fits it, the slot's own group before a
/// universal set (a universal gem is the one worth saving). Never overwrites — a report that
/// re-rolls a filled socket measures gem churn, not the system. Returns gems placed.
pub(crate) fn auto_socket_gems(state: &mut GameState, data: &GameData) -> u32 {
    use sim_core::gems::{refusal, set_of, socket, GemGroup, GemTarget};
    let mut placed = 0;
    let ids: Vec<u32> = state
        .grid
        .entities
        .iter()
        .filter(|e| matches!(e.team, sim_core::grid::Team::Player) && !e.is_pet)
        .map(|e| e.id)
        .collect();
    for id in ids {
        let Some(eidx) = state.grid.idx_of_id(id) else { continue };
        let worn = state.grid.entities[eidx].equipped.len();
        for slot in 0..worn {
            loop {
                let target = GemTarget::Equipped { entity_id: id, equipped_slot: slot as u32 };
                let socket_no = state.grid.entities[eidx].equipped[slot].gems.len() as u8;
                // Best candidate: (fits, not universal, grade) — highest wins.
                let mut best: Option<(bool, usize, String)> = None;
                let held: Vec<(String, u32)> = sim_core::gems::held(state, data).map(|(g, n)| (g.to_string(), n)).collect();
                for (gem, _) in &held {
                    let gem = gem.as_str();
                    let Some((set, grade)) = set_of(data, gem) else { continue };
                    if refusal(state, data, target, socket_no, gem).is_some() {
                        continue;
                    }
                    let key = (set.group != GemGroup::Universal, grade.step(), gem.to_string());
                    if best.as_ref().map_or(true, |b| (key.0, key.1) > (b.0, b.1)) {
                        best = Some(key);
                    }
                }
                let Some((_, _, gem)) = best else { break };
                if !socket(state, data, target, socket_no, &gem) {
                    break;
                }
                placed += 1;
            }
        }
    }
    placed
}

/// Fuse held gems six-into-one at every grade below the ceiling (sim-cli only), lowest grade
/// first so a fused gem can feed the next round. Returns fusions run.
pub(crate) fn auto_fuse_gems(state: &mut GameState, data: &GameData) -> u32 {
    use sim_core::fusion::FUSE_GRADE_CEILING;
    use sim_core::gems::{fuse, set_of};
    use sim_core::items::GRADES;
    let merge = data.loot_rules.gems.merge as usize;
    if merge == 0 {
        return 0;
    }
    let mut made = 0;
    for grade in GRADES.iter().take_while(|g| **g < FUSE_GRADE_CEILING) {
        loop {
            let mut picks: Vec<String> = Vec::new();
            let held: Vec<(String, u32)> = sim_core::gems::held(state, data).map(|(g, n)| (g.to_string(), n)).collect();
            for (gem, n) in &held {
                let (gem, n) = (gem.as_str(), *n);
                let Some((_, g)) = set_of(data, gem) else { continue };
                if g != *grade {
                    continue;
                }
                for _ in 0..n {
                    if picks.len() < merge {
                        picks.push(gem.to_string());
                    }
                }
            }
            if picks.len() < merge || !fuse(state, data, &picks) {
                break;
            }
            made += 1;
        }
    }
    made
}

/// Vendor surplus stash gear so the gear cap never clogs (sim-cli only). After
/// `auto_equip_best` has worn the best, any remaining stash piece that the LEAD
/// cannot wield OR that is worse-or-equal to its worn counterpart is dead weight —
/// sell it so incoming higher-tier drops always have room to bank. Wearable
/// strict-upgrades are KEPT (the next equip pass wears them).
/// Disenchant every bag piece that yields a crest material and is not a wearable upgrade for the
/// lead (sim-cli model of the player feeding T3+ surplus to the crests). Returns pieces fed.
pub(crate) fn auto_disenchant_surplus(state: &mut GameState, data: &GameData) -> u32 {
    auto_disenchant(state, data, false)
}

/// The model's "fusion or disenchant" split (0.0.422): with `leftovers_only`, feed the crests
/// only pieces fusion CANNOT use right now — a grade the bag holds fewer than six of — so the
/// sixes still fuse for Gil and the odd pieces become materials instead of waiting for a bag
/// sweep that a fusing party never reaches (measured: 0 disenchants in 7 days).
pub(crate) fn auto_disenchant(state: &mut GameState, data: &GameData, leftovers_only: bool) -> u32 {
    let score = |i: &sim_core::items::Item| i.atk_bonus + i.def_bonus;
    let Some(eidx) = state.grid.entities.iter().position(|e| matches!(e.team, Team::Player)) else { return 0 };
    let unit_def = state.grid.entities[eidx].unit_def.clone();
    let exemplar_id = state.grid.entities[eidx].exemplar.as_ref().map(|x| x.id.clone());
    let mut fed = 0;
    loop {
        let target = {
            let worn = |slot| {
                state.grid.entities[eidx].equipped.iter().filter(|i| i.slot() == slot).map(&score).max().unwrap_or(0)
            };
            let of_grade = |g: sim_core::items::Grade| state.inventory.iter().filter(|i| i.grade == g && matches!(i.kind, sim_core::items::ItemKind::Weapon | sim_core::items::ItemKind::Armor) && !i.unique && !i.bound).count();
            state.inventory.iter().position(|i| {
                sim_core::crest::material_of(data, i).is_some()
                    && !(i.is_preferred_by(exemplar_id.as_deref(), &unit_def) && score(i) > worn(i.slot()))
                    && (!leftovers_only || !matches!(i.kind, sim_core::items::ItemKind::Weapon | sim_core::items::ItemKind::Armor) || of_grade(i.grade) < sim_core::fusion::FUSE_INPUTS)
            })
        };
        let Some(idx) = target else { break };
        if !sim_core::crest::disenchant(state, data, idx) {
            break;
        }
        fed += 1;
    }
    fed
}

/// Lift whichever crest is cheapest among those the purse can pay, until none can (sim-cli).
pub(crate) fn auto_crests(state: &mut GameState, data: &GameData) -> u32 {
    let mut lifted = 0;
    loop {
        let mut best: Option<(String, u32)> = None;
        for sl in &data.crests.slots {
            let Some((key, n)) = sim_core::crest::next_cost(state, data, &sl.id) else { continue };
            if state.crest_mats.get(&key).copied().unwrap_or(0) >= n && best.as_ref().map_or(true, |(_, b)| n < *b) {
                best = Some((sl.id.clone(), n));
            }
        }
        let Some((id, _)) = best else { break };
        if !sim_core::crest::upgrade(state, data, &id) {
            break;
        }
        lifted += 1;
    }
    lifted
}

/// Vendoring is retired (0.0.411): what used to be sold is DISCARDED, but only once the bag is
/// nearly full — fodder has to ACCUMULATE to six of a grade for `auto_fuse_surplus` to take it,
/// and a per-tick sweep never let it (0 fusions in 12 h, measured). Same "not a wearable upgrade
/// for the lead" filter; drains to half the cap so the next sweep is a while off.
pub(crate) fn auto_discard_surplus_gear(state: &mut GameState, data: &GameData) {
    let cap = state.inventory_cap();
    if state.inventory.len() + 2 < cap {
        return;
    }
    // DISENCHANT FIRST (0.0.419 model): every surplus piece that yields a material goes to the
    // crests rather than the floor — the player's half of "fusion or disenchant" that a run
    // has to show, since fusion already took the sixes. Then the plain discard below.
    auto_disenchant_surplus(state, data);
    let score = |i: &sim_core::items::Item| i.atk_bonus + i.def_bonus;
    let Some(eidx) = state
        .grid
        .entities
        .iter()
        .position(|e| matches!(e.team, Team::Player))
    else {
        return;
    };
    let unit_def = state.grid.entities[eidx].unit_def.clone();
    let exemplar_id = state.grid.entities[eidx].exemplar.as_ref().map(|x| x.id.clone());
    loop {
        let target = {
            let worn = |slot| {
                state.grid.entities[eidx]
                    .equipped
                    .iter()
                    .filter(|i| i.slot() == slot)
                    .map(&score)
                    .max()
                    .unwrap_or(0)
            };
            state.inventory.iter().position(|i| {
                !i.is_stackable()
                    && matches!(i.kind, sim_core::items::ItemKind::Weapon | sim_core::items::ItemKind::Armor)
                    && !i.unique
                    && !(i.is_preferred_by(exemplar_id.as_deref(), &unit_def) && score(i) > worn(i.slot()))
            })
        };
        if state.inventory.len() <= cap / 2 {
            break;
        }
        match target {
            Some(idx) => apply_input(state, InputEvent::DiscardItem { inv_idx: idx as u32 }),
            None => break,
        }
    }
}


/// Model the party growing (0.0.413): buy the next PARTY SLOT whenever the Gil is there (the
/// prices are set so all four fall inside the first hour or two), and stand a member in it at
/// once — in the game the slot's 10-pull ticket is what fills it; here the pull is modelled as
/// "the next class in `order` arrives", so a headless run measures the pace and not the roll.
/// Gil is NOT topped up: the slots are the first thing income buys, which is the point.
pub(crate) fn auto_party(state: &mut GameState, data: &GameData, order: &[String], buy: bool) {
    loop {
        let size = state
            .grid
            .entities
            .iter()
            .filter(|e| matches!(e.team, Team::Player) && !e.is_pet)
            .count();
        // `order[0]` is the SECOND member: the run starts on Seraphine alone.
        if size == 0 || size - 1 >= order.len() {
            break;
        }
        if size < state.party_slots() {
            // An Exemplar id (lapis, lucielle, …) arrives the way a pull does — granted as a
            // benched unit through the ONE acquisition primitive, then deployed; any other unit
            // id (a test fixture) is fielded bare through the model path.
            let id = &order[size - 1];
            let ok = if sim_core::items::exemplar_def_by_id(id).is_some() {
                match data::grant_exemplar_unit(state, data, id) {
                    Some(char_id) => data::deploy(state, data, char_id).is_ok(),
                    // A REPEATED id (the account already owns it, so the pull banked a credit):
                    // field it bare as a stand-in for "another unit of this rarity and kit" —
                    // a modelling convenience for SR-heavy party reads, not a game path.
                    None => data::model_add_member(state, data, id).is_ok(),
                }
            } else {
                data::model_add_member(state, data, id).is_ok()
            };
            if !ok {
                break;
            }
            continue;
        }
        // Slots open by progress in the sim (0.0.490); only a bench's tick-0 setup still buys them
        // so the whole party stands up at once.
        if !buy {
            break;
        }
        match data::party_slot_cost(state, data) {
            Some(cost) if state.gil() >= cost => {
                if data::buy_party_slot(state, data).is_err() {
                    break;
                }
            }
            _ => break,
        }
    }
}

/// `--level` / `--kit`: hold every member at a target base level, with maxed
/// skills and a gear set. This is the ONLY way a headless run can measure LATE
/// content: the natural curve takes days of sim time to reach Lv46-50, so a
/// dungeon tuned for that band is otherwise unreachable and untunable outside the
/// live game. Re-applied each tick so members recruited later are topped up too
/// (a Lv1 recruit in a Lv48 party would just feed the boss and skew the numbers).
///
/// It pokes the same debug helpers the game's own debug view exposes. sim-cli is a
/// TRUSTED balance tool whose runs are never replay-verified, so this never enters
/// an event log — but it does mean a `--level` run's state hash is meaningless as a
/// determinism reference.
pub(crate) fn force_power(state: &mut GameState, data: &GameData, level: u32, kit: bool) {
    if level == 0 && !kit {
        return;
    }
    let members: Vec<(u32, u32)> = state
        .grid
        .entities
        .iter()
        .filter(|e| matches!(e.team, Team::Player) && !e.is_pet)
        .map(|e| (e.id, e.stats.level))
        .collect();
    for (id, cur) in members {
        if level > cur {
            data::debug_add_levels(state, id, level - cur);
        }
        if kit {
            // Both are idempotent no-ops once applied (skills already max / gear already worn).
            data::debug_max_skills(state, id);
            data::debug_give_gear(state, data, id);
        }
    }
    if level > 0 {
        // The recruit slots are LEVEL+Gil gated; a level-forced run should field a
        // full party from tick 0 rather than trickle members in as loot sells.
        state.set_gil_for_test(state.gil().max(1_000_000));
    }
}

/// `--regalia`: every fielded member without a Regalia gets one and wears it — the SIGNATURE
/// piece of its unit (`sig`) or the R rung of its class line (`r`), at `level` / `stars`. The
/// carrier is pushed straight into `owned_regalia` (a pull's outcome, not its roll) and equipped
/// through the real input, so the wearer's stats resolve the way a player's would.
pub(crate) fn auto_regalia(state: &mut GameState, mode: &str, level: u8, stars: u8) {
    use sim_core::items::{Item, ItemKind, Rarity};
    // The line is the unit's DOCTRINE's (0.0.436): `sig` = its SSR rung, `r` = its R rung.
    let pick = |unit_def: &str| -> Option<(&'static str, Rarity)> {
        let ids = sim_core::items::regalia_line_for_unit(unit_def)?;
        // A unit built one piece at a time (0.0.462) has an empty rung: `sig` is its highest
        // EXISTING rung, `r` its lowest.
        let rung = |i: usize| -> Option<(&'static str, Rarity)> {
            let id = ids.get(i).copied().filter(|s| !s.is_empty())?;
            Some((id, [Rarity::Common, Rarity::Mythical, Rarity::Legendary][i]))
        };
        Some(match mode {
            "sig" => rung(2).or_else(|| rung(1)).or_else(|| rung(0))?,
            "r" => rung(0).or_else(|| rung(1)).or_else(|| rung(2))?,
            // A bare Regalia id: EVERY member wears that one piece (0.0.457, "ลองใช้ regalia
            // อันเดียวกัน") — the passive still gates itself on the wearer's doctrine.
            id => {
                let rung = sim_core::items::regalia_rarity_index(id)?;
                let rarity = [Rarity::Common, Rarity::Mythical, Rarity::Legendary].get(rung).copied()?;
                (sim_core::items::regalia_static_id(id)?, rarity)
            }
        })
    };
    let bare: Vec<(u32, String)> = state
        .grid
        .entities
        .iter()
        .filter(|e| matches!(e.team, Team::Player) && !e.is_pet && e.alive && e.regalia.is_none())
        .map(|e| (e.id, e.unit_def.clone()))
        .collect();
    for (id, unit_def) in bare {
        let Some((rid, rarity)) = pick(&unit_def) else { continue };
        let mut item = Item::new(&sim_core::items::regalia_item_def_id(rid), ItemKind::Regalia, 0, 0, rarity);
        item.enhance_level = level.min(sim_core::items::REGALIA_MAX_LEVEL);
        item.resonance_stars = stars.min(sim_core::items::REGALIA_MAX_STARS);
        state.owned_regalia.push(item);
        let owned_idx = (state.owned_regalia.len() - 1) as u32;
        sim_core::events::apply_input(state, sim_core::events::InputEvent::EquipRegalia { entity_id: id, owned_idx });
    }
}

/// `--bench`: every member wears the best gear of tier ≤ `tier` at `grade` (see
/// `debug_give_gear_at`). Idempotent — the same catalog pick lands in the same slot.
pub(crate) fn bench_gear(state: &mut GameState, data: &GameData, tier: u32, grade: u8) {
    let grade = sim_core::items::GRADES.get(grade as usize).copied().unwrap_or(sim_core::items::Grade::Legendary);
    let ids: Vec<u32> = state
        .grid
        .entities
        .iter()
        .filter(|e| matches!(e.team, Team::Player) && !e.is_pet)
        .map(|e| e.id)
        .collect();
    for id in ids {
        data::debug_give_gear_at(state, data, id, tier, grade);
    }
}

/// `--stars`: every fielded Exemplar unit at Resonance `stars`, its overlay re-applied so the
/// level ranks land. Idempotent (a unit already there is untouched).
pub(crate) fn set_stars_for(state: &mut GameState, unit: &str, stars: u8) {
    let stars = stars.min(sim_core::items::EXEMPLAR_MAX_STARS);
    for e in state.grid.entities.iter_mut() {
        if !matches!(e.team, Team::Player) || e.is_pet || e.unit_def != unit {
            continue;
        }
        if e.exemplar.is_none() {
            if let Some(def) = sim_core::items::exemplar_def_by_id(&e.unit_def) {
                e.exemplar = Some(sim_core::items::Exemplar { id: def.id.clone(), name: def.name.clone(), skin_id: def.skin_id.clone(), stars: 0 });
            }
        }
        let Some(x) = e.exemplar.as_mut() else { continue };
        x.stars = stars;
        e.apply_exemplar_skill();
    }
}

pub(crate) fn set_stars(state: &mut GameState, stars: u8) {
    let stars = stars.min(sim_core::items::EXEMPLAR_MAX_STARS);
    for e in state.grid.entities.iter_mut() {
        if !matches!(e.team, Team::Player) || e.is_pet {
            continue;
        }
        // A `--lead` unit is fielded bare (no overlay); give it its own Exemplar so the web applies.
        if e.exemplar.is_none() {
            if let Some(def) = sim_core::items::exemplar_def_by_id(&e.unit_def) {
                e.exemplar = Some(sim_core::items::Exemplar { id: def.id.clone(), name: def.name.clone(), skin_id: def.skin_id.clone(), stars: 0 });
            }
        }
        let Some(x) = e.exemplar.as_mut() else { continue };
        if x.stars == stars {
            continue;
        }
        x.stars = stars;
        e.apply_exemplar_skill();
    }
}
