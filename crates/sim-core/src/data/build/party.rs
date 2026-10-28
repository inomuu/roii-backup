//! build::party — player-entity construction, recruitment, class advancement.

// DEBT (37 sites): this file predates `deny(clippy::indexing_slicing)`. An index that
// cannot be proven in range is a panic, and in verify-server (which links this crate and
// replays saves under `panic = "abort"`) a panic is the whole process. Shrink this, never
// add to it — a NEW file has no allow and is rejected on its first bare index.
#![allow(clippy::indexing_slicing)]

use super::*;
use crate::state::{GilSpend, ENGRAM_TIERS};

/// Build a player Entity from a class definition at `pos` with `id`.
/// `data` is needed to resolve the class's `starting_weapon` item.
pub(crate) fn build_player_entity(data: &GameData, class: &UnitDef, id: u32, pos: Cell) -> Entity {
    let mut e = Entity::idle(id, pos)
        .with_team(Team::Player)
        .with_walk(pos, class.speed_milli)
        .with_combat(
            class.hp, class.atk, class.def,
            class.atk_speed_milli, class.element,
        );
    e.atk_range = class.atk_range;
    e.proj_travel_ticks = class.proj_travel_ticks;
    e.atk_type = class.atk_type;
    e.is_ranged = class.is_ranged;
    e.atk_base_milli = class.atk_base_milli;
    e.atk_per_level_milli = class.atk_per_level_milli;
    e.hp_per_level = class.hp_per_level;
    e.hp_factor = class.hp_factor;
    e.hp_increase = class.hp_increase;
    // A unit's kit arrives after unit_def is assigned below (`base_kit::ensure_entity`); ranks
    // above one are bought on the Potential board.
    e.skills = Vec::new();
    // RO: damage comes from the equipped weapon, not a flat class atk.
    if let Some(def) = data.item(&class.starting_weapon) {
        e.equipped.push(instantiate_item(def));
    }
    // `max_hp` (= class.hp via with_combat) is the level-1 base;
    // effective_max_hp layers per-level growth + VIT%. Start full.
    e.hp = crate::combat::effective_max_hp(&e);
    e.unit_def = class.id.clone();
    crate::potential::base_kit::ensure_entity(&mut e);
    e
}

/// Spread party members around the spawn cell so they don't stack.
/// Falls back to the base cell when an offset is out of the `w`×`h` grid.
pub(crate) fn party_cell(base: Cell, i: usize, w: i32, h: i32) -> Cell {
    const OFFSETS: [(i32, i32); 5] = [(0, 0), (1, 0), (0, 1), (-1, 0), (0, -1)];
    let (dx, dy) = OFFSETS[i % OFFSETS.len()];
    let c = Cell::new(base.x + dx, base.y + dy);
    if c.x >= 0 && c.x < w && c.y >= 0 && c.y < h { c } else { base }
}

/// Hard cap on party size. Five party_cell offsets exist, so this is
/// also the spread limit before members would start sharing a cell.
pub const MAX_PARTY: usize = 5;

/// Gil price of the NEXT party slot — the one that lifts `party_slots()` by one — from the
/// catalog's `party.slot_gil` (index = the slot being opened − 2). `None` at `MAX_PARTY` or
/// when the catalog prices no such slot.
pub fn party_slot_cost(state: &GameState, data: &GameData) -> Option<u64> {
    let have = state.party_slots();
    if have >= MAX_PARTY {
        return None;
    }
    data.party.slot_gil.get(have - 1).copied()
}

/// PARTY SLOTS OPEN BY PROGRESS (user 2026-09-30): slots 2/3/4/5 on reaching stage 2 / 6 / 18 /
/// 24 (`band_reached`) — slot 2 therefore after stage 1's boss (room 1-3), not room 1-1's. No Gil.
/// The server pays each opening off `measures.party_slots_bought`: ten Starter pulls per slot,
/// slot 2 included (user 2026-10-12; slot 2 mailed a fixed Lumi before).
pub const SLOT_STAGES: [u32; 4] = [2, 6, 18, 24];

/// How many party slots this account's progress has earned (1..=MAX_PARTY).
pub fn slots_earned(state: &GameState) -> usize {
    (1 + SLOT_STAGES.iter().filter(|s| state.band_reached >= **s).count()).min(MAX_PARTY)
}

/// The stage whose arrival opens the next party slot, `None` once every slot is open.
pub fn next_slot_requirement(state: &GameState) -> Option<u32> {
    SLOT_STAGES.get(state.party_slots().saturating_sub(1)).copied()
}

/// Open every slot progress has earned. Runs each tick; a save that bought slots with Gil before
/// 0.0.490 keeps them (only ever raises). Each slot opened counts in `party_slots_bought`.
pub fn open_earned_slots(state: &mut GameState) {
    let earned = slots_earned(state);
    let have = state.party_slots();
    if earned > have {
        state.party_slots = earned as u8;
        state.measures.party_slots_bought.add((earned - have) as u64);
    }
}

/// Buy the next party slot with Gil (0.0.413). RETIRED as a player input (0.0.490 — slots open by
/// progress, `open_earned_slots`); kept for test fixtures and sim-cli benches that field a full
/// party at tick 0. Recruiting for Gil is retired — the party's
/// CHARACTERS come from gacha alone — but its SIZE is still bought, cheaply and only four times
/// (user: "ใช้ Gil ซื้อ party slot ได้แบบไม่ต้องแพงมาก max 5"). Errors leave state untouched.
pub fn buy_party_slot(state: &mut GameState, data: &GameData) -> Result<usize, String> {
    let cost = party_slot_cost(state, data).ok_or_else(|| "no slot to buy".to_string())?;
    if state.gil() < cost {
        return Err(format!("not enough Gil: need {cost}, have {}", state.gil()));
    }
    let next = state.party_slots() + 1;
    state.spend_gil(cost, GilSpend::PartySlot);
    state.party_slots = next as u8;
    state.measures.party_slots_bought.add(1);
    Ok(next)
}

/// The anchor deployed member = the LOWEST-id player on the grid (excl. pets).
/// There is no longer a special permanent "lead": the party is symmetric and any
/// member can be benched, so wherever a lead is still needed (recruit gate, the
/// deploy spawn anchor) we just take whoever is first. `FIRST_PLAYER_ID` is only the id
/// the FIRST character happens to spawn with at new-game, not a fixed role.
/// `None` if no member is deployed.
pub fn first_deployed(state: &GameState) -> Option<u32> {
    state
        .grid
        .entities
        .iter()
        .filter(|e| matches!(e.team, Team::Player) && !e.is_pet)
        .map(|e| e.id)
        .min()
}

/// The anchor member's base level, or 0 if the party is empty. (Recruit gate.)
pub fn lead_level(state: &GameState) -> u32 {
    first_deployed(state)
        .and_then(|id| state.grid.idx_of_id(id))
        .map(|i| state.grid.entities[i].stats.level)
        .unwrap_or(0)
}

/// Count of Player-team members currently on the grid (alive OR dead —
/// a downed member is still a roster slot until the party wipes).
pub fn party_size(state: &GameState) -> usize {
    state.grid.entities.iter()
        .filter(|e| matches!(e.team, Team::Player))
        .count()
}

/// Field a member of class `unit_def` for NOTHING — the MODEL path (sim-cli, dev tools). No
/// input event reaches it since 0.0.413: a character enters the account through gacha
/// (`grant_exemplar_unit`) and the field through `deploy`; this exists so a headless run can
/// stand in for the pull. Capped by `party_slots()` like a deploy. Errors leave state untouched.
pub fn model_add_member(state: &mut GameState, data: &GameData, unit_def: &str) -> Result<(), String> {
    let size = party_size(state);
    if size >= state.party_slots() {
        return Err("party is full".to_string());
    }
    let class = data
        .unit(unit_def)
        .ok_or_else(|| format!("unknown unit: {unit_def}"))?;
    // Next id: max existing player id + 1 (robust against any gaps).
    let next_id = state.grid.entities.iter()
        .filter(|e| matches!(e.team, Team::Player))
        .map(|e| e.id)
        .max()
        .map(|m| m + 1)
        .unwrap_or(FIRST_PLAYER_ID);
    // Anchor the recruit next to the lead so it joins the cluster.
    let base = state.grid.idx_of_id(FIRST_PLAYER_ID)
        .map(|i| state.grid.entities[i].pos)
        .unwrap_or(Cell::new(0, 0));
    let pos = party_cell(base, size, state.grid.width, state.grid.height);
    state.grid.spawn(build_player_entity(data, class, next_id, pos));
    Ok(())
}

// --- Roster ⇄ Party (buy / deploy / bench) --------------------------------
// Deployed party members live as grid entities (`Team::Player`); benched ones
// park in `GameState.roster` (unique-id keyed → duplicate classes allowed). The
// The party is SYMMETRIC — there is no permanent lead; any member can be benched
// as long as one remains deployed (`first_deployed` picks the anchor wherever a
// "first member" is still needed). These mirror the old bridge-level
// `set_player_class` park/restore, moved into sim-core proper.

/// Characters the player OWNS = deployed party members (excl. pets) + benched
/// roster. Indexes the buy-cost curve.
pub fn owned_count(state: &GameState) -> usize {
    let deployed = state
        .grid
        .entities
        .iter()
        .filter(|e| matches!(e.team, Team::Player) && !e.is_pet)
        .count();
    deployed + state.roster.len()
}

/// Next free unique char id: max over deployed players + benched roster, +1.
fn next_char_id(state: &GameState) -> u32 {
    let grid_max = state
        .grid
        .entities
        .iter()
        .filter(|e| matches!(e.team, Team::Player) && !e.is_pet)
        .map(|e| e.id)
        .max();
    let roster_max = state.roster.keys().copied().max();
    grid_max
        .into_iter()
        .chain(roster_max)
        .max()
        .map(|m| m + 1)
        .unwrap_or(FIRST_PLAYER_ID)
}

/// Bench a fresh member of class `unit_def` for NOTHING — the MODEL path, the roster twin of
/// `model_add_member`. No input event reaches it since 0.0.413 (`BuyCharacter` is a tombstone).
pub fn model_bench_member(
    state: &mut GameState,
    data: &GameData,
    unit_def: &str,
) -> Result<u32, String> {
    let class = data
        .unit(unit_def)
        .ok_or_else(|| format!("unknown unit: {unit_def}"))?;
    let id = next_char_id(state);
    // Build a fresh entity off-grid just to capture a proper starting loadout
    // (class base stats + starting weapon), then store it benched.
    let mut e = build_player_entity(data, class, id, Cell::new(0, 0));
    let rc = crate::state::RosterChar {
        unit_def: unit_def.to_string(),
        data: crate::state::CharacterData::capture(&mut e),
    };
    state.roster.insert(id, rc);
    Ok(id)
}

/// Deploy a benched character into the party (rebuilds its grid entity from the
/// class def + overlays its saved loadout). Errors (party full, unknown id)
/// leave state untouched.
pub fn deploy(state: &mut GameState, data: &GameData, char_id: u32) -> Result<(), String> {
    let deployed = state
        .grid
        .entities
        .iter()
        .filter(|e| matches!(e.team, Team::Player) && !e.is_pet)
        .count();
    if deployed >= state.party_slots() {
        return Err("party is full".to_string());
    }
    if !state.roster.contains_key(&char_id) {
        return Err(format!("not in roster: {char_id}"));
    }
    let unit_def = state.roster[&char_id].unit_def.clone();
    let class = data
        .unit(&unit_def)
        .ok_or_else(|| format!("unknown unit: {unit_def}"))?;
    let rc = state.roster.remove(&char_id).unwrap();
    let base = first_deployed(state)
        .and_then(|id| state.grid.idx_of_id(id))
        .map(|i| state.grid.entities[i].pos)
        .unwrap_or(Cell::new(0, 0));
    let pos = party_cell(base, deployed, state.grid.width, state.grid.height);
    let mut e = build_player_entity(data, class, char_id, pos);
    rc.data.restore_onto(&mut e);
    e.hp = crate::combat::effective_max_hp(&e);
    state.grid.spawn(e);
    Ok(())
}

// ---------------------------------------------------------------------------------------------
// EXEMPLAR == UNIT (0.0.350, docs/exemplar-unit-and-xp-refund-design.md §2-§3). An Exemplar is a
// ROSTER UNIT, not a carrier worn by a class character. The first copy the account receives
// becomes a fresh Lv1 unit; every later copy banks a Resonance credit the PLAYER spends by hand.
// ---------------------------------------------------------------------------------------------

/// Does the account already hold `bare_id` as a unit, benched or deployed?
pub fn account_owns_exemplar(state: &GameState, bare_id: &str) -> bool {
    state
        .roster
        .values()
        .any(|rc| rc.data.exemplar.as_ref().is_some_and(|e| e.id == bare_id))
        || state
            .grid
            .entities
            .iter()
            .any(|e| e.exemplar.as_ref().is_some_and(|x| x.id == bare_id))
}

/// Grant Exemplar `bare_id` to the account. First copy → a fresh Lv1 BENCHED unit on the
/// Exemplar's own unit def (`seraphine`, `lapis`, …), carrying `exemplar: Some` at 0★ so the star
/// ranks' mods and skills layer on through the existing overlay path. A duplicate →
/// `pending_resonance[bare_id]` += 1 and no unit; so does a PLANNED line whose unit def has not
/// shipped. Returns the new unit's id, or None for a dupe / a planned line / an unknown Exemplar.
/// The ONE acquisition primitive: a pull, the Mileage exchange and the dev grant all land here.
pub fn grant_exemplar_unit(state: &mut GameState, data: &GameData, bare_id: &str) -> Option<u32> {
    let def = crate::items::exemplar_def_by_id(bare_id)?;
    // A dupe, or a planned line with no unit def yet, banks a credit instead: the pull is not
    // lost, and the first grant after the unit ships stands it up with that credit waiting.
    let Some(class) = data.unit(bare_id).filter(|_| !account_owns_exemplar(state, bare_id)) else {
        *state.pending_resonance.entry(bare_id.to_string()).or_insert(0) += 1;
        return None;
    };
    let id = next_char_id(state);
    let mut e = build_player_entity(data, class, id, Cell::new(0, 0));
    e.exemplar = Some(crate::items::Exemplar {
        id: def.id.clone(),
        name: def.name.clone(),
        skin_id: def.skin_id.clone(),
        stars: 0,
    });
    let rc = crate::state::RosterChar {
        unit_def: class.id.clone(),
        data: crate::state::CharacterData::capture(&mut e),
    };
    state.roster.insert(id, rc);
    Some(id)
}

/// One row per Exemplar UNIT the account holds, benched or deployed, for the collection screen:
/// who it is, its stars and level, whether it is on the field, and how many Resonance credits
/// wait for it. Sorted by unit id so the list is stable across ticks.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct ExemplarUnitRow {
    pub unit_id: u32,
    pub exemplar_id: String,
    pub name: String,
    pub unit_def: String,
    pub stars: u8,
    pub level: u32,
    pub deployed: bool,
    pub credits: u32,
}

pub fn exemplar_units(state: &GameState) -> Vec<ExemplarUnitRow> {
    let mut out = Vec::new();
    let credits = |id: &str| state.pending_resonance.get(id).copied().unwrap_or(0);
    for (id, rc) in &state.roster {
        if let Some(x) = rc.data.exemplar.as_ref() {
            out.push(ExemplarUnitRow {
                unit_id: *id,
                exemplar_id: x.id.clone(),
                name: x.name.clone(),
                unit_def: rc.unit_def.clone(),
                stars: x.stars,
                level: rc.data.stats.level,
                deployed: false,
                credits: credits(&x.id),
            });
        }
    }
    for e in state.grid.entities.iter() {
        if !matches!(e.team, Team::Player) || e.is_pet {
            continue;
        }
        if let Some(x) = e.exemplar.as_ref() {
            out.push(ExemplarUnitRow {
                unit_id: e.id,
                exemplar_id: x.id.clone(),
                name: x.name.clone(),
                unit_def: e.unit_def.clone(),
                stars: x.stars,
                level: e.stats.level,
                deployed: true,
                credits: credits(&x.id),
            });
        }
    }
    out.sort_by_key(|r| r.unit_id);
    out
}

/// The PLAYER spends one Resonance credit on unit `unit_id` (benched or deployed), raising its
/// stars by one up to `EXEMPLAR_MAX_STARS`. No credit / not an Exemplar unit / at cap = silent
/// no-op. A deployed unit re-applies its overlay at once so the new rank's skills are live.
/// Units `FeedExemplarUnit` would raise right now — a banked Resonance credit for their Exemplar
/// and stars below the cap. Roster (benched) first in id order, then the field in spawn order.
pub fn feedable_units(state: &GameState) -> Vec<u32> {
    let max = crate::items::EXEMPLAR_MAX_STARS;
    let ready = |x: Option<&crate::items::Exemplar>| {
        x.is_some_and(|x| x.stars < max && state.pending_resonance.get(&x.id).is_some_and(|c| *c > 0))
    };
    let benched = state.roster.iter().filter(|(_, rc)| ready(rc.data.exemplar.as_ref())).map(|(id, _)| *id);
    let fielded = state.grid.entities.iter()
        .filter(|e| matches!(e.team, crate::grid::Team::Player) && !e.is_pet && ready(e.exemplar.as_ref()))
        .map(|e| e.id);
    benched.chain(fielded).collect()
}

pub fn feed_exemplar_unit(state: &mut GameState, unit_id: u32) {
    let max = crate::items::EXEMPLAR_MAX_STARS;
    // Which Exemplar is this unit, and can it still rise?
    let bare = if let Some(rc) = state.roster.get(&unit_id) {
        rc.data.exemplar.as_ref().filter(|x| x.stars < max).map(|x| x.id.clone())
    } else if let Some(e) = state.grid.entity_of_id(unit_id) {
        e.exemplar.as_ref().filter(|x| x.stars < max).map(|x| x.id.clone())
    } else {
        None
    };
    let Some(bare) = bare else { return; };
    let Some(credit) = state.pending_resonance.get_mut(&bare) else { return; };
    if *credit == 0 {
        return;
    }
    *credit -= 1;
    if *credit == 0 {
        state.pending_resonance.remove(&bare);
    }
    if let Some(rc) = state.roster.get_mut(&unit_id) {
        if let Some(x) = rc.data.exemplar.as_mut() {
            x.stars += 1;
        }
    } else if let Some(e) = state.grid.entity_of_id_mut(unit_id) {
        if let Some(x) = e.exemplar.as_mut() {
            x.stars += 1;
        }
        e.apply_exemplar_skill();
    }
    state.measures.exemplars_fed.add(1);
}

// ---------------------------------------------------------------------------------------------
// XP REFUND (0.0.349, docs/exemplar-unit-and-xp-refund-design.md §5-§7, re-integrated onto the
// 0.0.347 stat model). Roster-only: every primitive works on a BENCHED unit's `CharacterData`, so a
// live entity is never reset mid-tick (HP above a new max, buffs keyed to old stats). Base stream
// only — job level is the Potential Board's cap and decision (b) keeps the board out of a refund.
// ---------------------------------------------------------------------------------------------

/// Return a unit's BASE progression to fresh Lv1: level, bar, both Apex tiers' points and their
/// allocation. Job level, job XP, skills, the Potential Board, gear and cards are untouched.
pub fn base_progression_reset(data: &mut crate::state::CharacterData) {
    let s = &mut data.stats;
    s.level = 1;
    s.xp = 0;
    s.level_points = 0;
    s.apex_level = 0;
    s.apex = [0; crate::stats::APEX_BUCKETS];
}

/// The account's proven progression CEILING: the lifetime marks (`level_reached` / `apex_reached`)
/// LIFTED by the current max across every unit, roster and deployed. Never below where a unit
/// already stands, so a stale mark (an old save defaults to 0) or a debug-levelled unit can't cap
/// a unit below itself. What a refund may pour a unit up to.
pub fn refund_ceiling(state: &GameState) -> (u32, u32) {
    let (mut lvl, mut apx) = (state.level_reached, state.apex_reached);
    let mut fold = |s: &crate::stats::Stats| {
        lvl = lvl.max(s.level);
        apx = apx.max(s.apex_level);
    };
    for rc in state.roster.values() {
        fold(&rc.data.stats);
    }
    for e in state.grid.entities.iter() {
        if matches!(e.team, Team::Player) && !e.is_pet {
            fold(&e.stats);
        }
    }
    (lvl, apx)
}

/// Drain a BENCHED unit's whole base progression into bound Engram items: its cumulative XP is
/// broken greedily into `ENGRAM_TIERS` (largest first; the sub-1,000 remainder is dropped — a tiny,
/// acknowledged sink), `owned_engrams` is credited, and the unit returns to Lv1. No-op on a
/// deployed / unknown / fresh unit. Engrams are minted ONLY here, from the unit's own provable XP;
/// nothing touches `Measures` or the marks, so draining A→B→A repeats for nothing and farms nothing.
pub fn refund_unit_xp(state: &mut GameState, unit_id: u32) {
    let Some(rc) = state.roster.get(&unit_id) else { return; };
    let mut remaining = crate::tick::cum_base(&rc.data.stats);
    if remaining == 0 {
        return;
    }
    for tier in (0..ENGRAM_TIERS.len()).rev() {
        let v = ENGRAM_TIERS[tier];
        let n = (remaining / v) as u32;
        if n > 0 {
            state.owned_engrams[tier] = state.owned_engrams[tier].saturating_add(n);
            remaining -= n as u64 * v;
        }
    }
    let Some(rc) = state.roster.get_mut(&unit_id) else { return; };
    base_progression_reset(&mut rc.data);
}

/// Spend `count` Engrams of `tier` on the BENCHED unit `unit_id`, capped at the account's ceiling
/// (overflow discarded). Clamps to owned stock; no-op on a bad tier / no stock / deployed-or-unknown
/// unit. Touches no `Measures`, no mark.
pub fn apply_engram(state: &mut GameState, unit_id: u32, tier: usize, count: u32) {
    if tier >= ENGRAM_TIERS.len() || !state.roster.contains_key(&unit_id) {
        return;
    }
    let use_n = count.min(state.owned_engrams[tier]);
    if use_n == 0 {
        return;
    }
    let (level_cap, apex_cap) = refund_ceiling(state);
    let v = ENGRAM_TIERS[tier].saturating_mul(use_n as u64);
    let Some(rc) = state.roster.get_mut(&unit_id) else { return; };
    crate::tick::apply_base_xp_capped(&mut rc.data.stats, v, level_cap, apex_cap);
    state.owned_engrams[tier] -= use_n;
}

/// What `apply_engrams_to_max` WOULD do — computed without mutating, for a screen to show (which
/// Engrams, the resulting level) and to WARN on before the player confirms.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
pub struct EngramFillPreview {
    /// Engrams that would be consumed, per tier (index = `ENGRAM_TIERS`).
    pub plan: [u32; ENGRAM_TIERS.len()],
    /// The unit's base level after applying `plan`.
    pub resulting_level: u32,
    /// Its Apex level after (only moves once the base ceiling is `LEVEL_CAP`).
    pub resulting_apex: u32,
    /// XP discarded at the ceiling — the waste a screen warns on when it is large.
    pub xp_wasted: u64,
}

/// Waste-minimising Engram selection toward the ceiling: the largest tiers that do not overshoot
/// first; then, only if still short, ONE smallest available Engram to top OVER the ceiling — the
/// player may reach max at a cost, and the preview surfaces that cost.
fn plan_engram_fill(
    stats: &crate::stats::Stats,
    owned: &[u32; ENGRAM_TIERS.len()],
    level_cap: u32,
    apex_cap: u32,
) -> [u32; ENGRAM_TIERS.len()] {
    let target = crate::tick::cum_at(level_cap, apex_cap);
    let current = crate::tick::cum_base(stats);
    let mut plan = [0u32; ENGRAM_TIERS.len()];
    if current >= target {
        return plan;
    }
    let mut need = target - current;
    let mut avail = *owned;
    for t in (0..ENGRAM_TIERS.len()).rev() {
        let n = ((need / ENGRAM_TIERS[t]) as u32).min(avail[t]);
        plan[t] += n;
        avail[t] -= n;
        need -= n as u64 * ENGRAM_TIERS[t];
    }
    if need > 0 {
        for t in 0..ENGRAM_TIERS.len() {
            if avail[t] > 0 {
                plan[t] += 1;
                break;
            }
        }
    }
    plan
}

/// Preview `apply_engrams_to_max` for a benched unit WITHOUT mutating. None on a deployed/unknown
/// unit. Waste reads 0 when nothing would be spent — a unit already at or over the ceiling (the
/// Apex holder with a partial bar) must not show a phantom figure.
pub fn engram_fill_preview(state: &GameState, unit_id: u32) -> Option<EngramFillPreview> {
    let rc = state.roster.get(&unit_id)?;
    let (level_cap, apex_cap) = refund_ceiling(state);
    let plan = plan_engram_fill(&rc.data.stats, &state.owned_engrams, level_cap, apex_cap);
    let granted: u64 = (0..ENGRAM_TIERS.len()).map(|t| plan[t] as u64 * ENGRAM_TIERS[t]).sum();
    let mut sim = rc.data.stats.clone();
    crate::tick::apply_base_xp_capped(&mut sim, granted, level_cap, apex_cap);
    let current = crate::tick::cum_base(&rc.data.stats);
    let target = crate::tick::cum_at(level_cap, apex_cap);
    let xp_wasted = if granted == 0 { 0 } else { (current + granted).saturating_sub(target) };
    Some(EngramFillPreview { plan, resulting_level: sim.level, resulting_apex: sim.apex_level, xp_wasted })
}

/// Fill a benched unit toward the ceiling with the waste-minimising plan: consume the planned
/// Engrams and grant their total (capped, overflow discarded). The screen previews and confirms
/// first; this is the committing event, and it re-derives the same plan so the two cannot disagree.
pub fn apply_engrams_to_max(state: &mut GameState, unit_id: u32) {
    let Some(rc) = state.roster.get(&unit_id) else { return; };
    let (level_cap, apex_cap) = refund_ceiling(state);
    let plan = plan_engram_fill(&rc.data.stats, &state.owned_engrams, level_cap, apex_cap);
    let granted: u64 = (0..ENGRAM_TIERS.len()).map(|t| plan[t] as u64 * ENGRAM_TIERS[t]).sum();
    if granted == 0 {
        return;
    }
    for t in 0..ENGRAM_TIERS.len() {
        state.owned_engrams[t] -= plan[t];
    }
    let Some(rc) = state.roster.get_mut(&unit_id) else { return; };
    crate::tick::apply_base_xp_capped(&mut rc.data.stats, granted, level_cap, apex_cap);
}

/// Bench a deployed party member: capture its LIVE loadout into the roster, then
/// despawn its grid entity (and any pet it owns). The party is symmetric — ANY
/// member can be benched (no protected lead) as long as at least one remains
/// deployed. Errors leave state untouched.
pub fn bench(state: &mut GameState, char_id: u32) -> Result<(), String> {
    let idx = state
        .grid
        .entities
        .iter()
        .position(|e| e.id == char_id && matches!(e.team, Team::Player) && !e.is_pet)
        .ok_or_else(|| format!("not a deployed party member: {char_id}"))?;
    let deployed = state
        .grid
        .entities
        .iter()
        .filter(|e| matches!(e.team, Team::Player) && !e.is_pet)
        .count();
    if deployed <= 1 {
        return Err("cannot bench the last member".to_string());
    }
    let e = &mut state.grid.entities[idx];
    let rc = crate::state::RosterChar {
        unit_def: e.unit_def.clone(),
        data: crate::state::CharacterData::capture(e),
    };
    state.roster.insert(char_id, rc);
    // Drop the member AND any pet it owned (the pet has no meaning while benched).
    state
        .grid
        .entities
        .retain(|e| e.id != char_id && !(e.is_pet && e.owner_id == char_id));
    Ok(())
}

// --- DEBUG helpers (dev tools — NOT recorded events; called only from the
// bridge's debug menu, never during normal play) -------------------------

/// DEBUG: raise every skill in a member's kit to its max and slot all of its actives so they
/// auto-cast. Lets you test a full kit without grinding shards.
pub fn debug_max_skills(state: &mut GameState, entity_id: u32) -> bool {
    let Some(i) = state.grid.idx_of_id(entity_id) else { return false; };
    let unit_def = state.grid.entities[i].unit_def.clone();
    for node in crate::skills::unit_kit_tree(&unit_def) {
        let Some(def) = crate::skills::skill_def(node.skill_id) else { continue; };
        let e = &mut state.grid.entities[i];
        match e.skills.iter_mut().find(|s| s.id == node.skill_id) {
            Some(s) => s.level = def.max_level,
            None => e.skills.push(crate::skills::SkillState::new(node.skill_id, def.max_level)),
        }
        if matches!(def.kind, crate::skills::SkillKind::Active)
            && !e.active_skills.iter().any(|id| id == node.skill_id)
        {
            e.active_skills.push(node.skill_id.to_string());
        }
    }
    true
}

/// DEBUG: add `n` BASE levels, granting the proper RO STATUS points per level
/// gained (mirrors a real level-up via `stat_points_for_level`) + refill HP/SP to
/// the new max. Lets you test allocation without grinding.
pub fn debug_add_levels(state: &mut GameState, entity_id: u32, n: u32) -> bool {
    let Some(i) = state.grid.idx_of_id(entity_id) else { return false; };
    for _ in 0..n {
        let leaving = state.grid.entities[i].stats.level;
        if leaving >= 999 {
            break;
        }
        let st = &mut state.grid.entities[i].stats;
        st.level += 1; // attributes do not grow any more (0.0.86)
        // A debug level grants what a real one does, or the screen this cheat exists to fill
        // stays empty and the cheat models something the game never does.
        st.level_points = st
            .level_points
            .saturating_add(crate::tick::POINTS_PER_LEVEL);
    }
    state.grid.entities[i].stats.xp = 0;
    state.grid.entities[i].hp = crate::combat::effective_max_hp(&state.grid.entities[i]);
    true
}

/// DEBUG: equip the best catalog gear the member's class can wield, one per slot.
pub fn debug_give_gear(state: &mut GameState, data: &GameData, entity_id: u32) -> bool {
    use crate::items::{EquipSlot, ItemKind};
    let Some(i) = state.grid.idx_of_id(entity_id) else { return false; };
    const SLOTS: [EquipSlot; 10] = [
        EquipSlot::Weapon, EquipSlot::Armor, EquipSlot::Shield, EquipSlot::Earring,
        EquipSlot::Shoes, EquipSlot::Helmet, EquipSlot::Gloves, EquipSlot::Bracer,
        EquipSlot::Necklace, EquipSlot::Ring,
    ];
    let unit_def = state.grid.entities[i].unit_def.clone();
    let exemplar_id = state.grid.entities[i].exemplar.as_ref().map(|x| x.id.clone());
    for slot in SLOTS {
        let best = data
            .items
            .iter()
            .filter(|d| matches!(d.kind, ItemKind::Weapon | ItemKind::Armor))
            .map(instantiate_item)
            .filter(|it| it.slot() == slot && it.is_preferred_by(exemplar_id.as_deref(), &unit_def))
            .max_by_key(|it| (it.atk_bonus + it.def_bonus) as i64);
        if let Some(b) = best {
            let e = &mut state.grid.entities[i];
            e.equipped.retain(|x| x.slot() != slot);
            e.equipped.push(b);
        }
    }
    true
}

/// DEBUG / BENCH: equip the best catalog gear of tier ≤ `tier` the member can wield, one per
/// slot, every piece at `grade` (with its overrides and substat lines, the way a drop would carry
/// them). The wardrobe of `sim-cli --bench` — a standard party to read DPS off.
pub fn debug_give_gear_at(state: &mut GameState, data: &GameData, entity_id: u32, tier: u32, grade: crate::items::Grade) -> bool {
    use crate::items::{EquipSlot, ItemKind};
    let Some(i) = state.grid.idx_of_id(entity_id) else { return false; };
    const SLOTS: [EquipSlot; 10] = [
        EquipSlot::Weapon, EquipSlot::Armor, EquipSlot::Shield, EquipSlot::Earring,
        EquipSlot::Shoes, EquipSlot::Helmet, EquipSlot::Gloves, EquipSlot::Bracer,
        EquipSlot::Necklace, EquipSlot::Ring,
    ];
    let unit_def = state.grid.entities[i].unit_def.clone();
    let exemplar_id = state.grid.entities[i].exemplar.as_ref().map(|x| x.id.clone());
    for slot in SLOTS {
        let best = data
            .items
            .iter()
            .filter(|d| matches!(d.kind, ItemKind::Weapon | ItemKind::Armor))
            .map(instantiate_item)
            .filter(|it| !it.unique && it.slot() == slot && it.item_level() <= tier && it.is_preferred_by(exemplar_id.as_deref(), &unit_def))
            .max_by_key(|it| (it.item_level(), (it.atk_bonus + it.def_bonus) as i64));
        if let Some(mut b) = best {
            let overrides: Vec<crate::data::GearOverride> = data.gear_overrides.iter().filter(|o| o.def_id == b.def_id).cloned().collect();
            let lines = crate::substats::lines_by_grade(&data.loot_rules.substats, &b.def_id, b.slot());
            b.assign_grade(grade, &overrides, &lines);
            let e = &mut state.grid.entities[i];
            e.equipped.retain(|x| x.slot() != slot);
            e.equipped.push(b);
        }
    }
    true
}

#[cfg(test)]
mod unit_grant_tests {
    use super::*;
    use crate::grid::Cell;
    use crate::items::EXEMPLAR_MAX_STARS;

    fn game_data() -> GameData {
        GameData::from_json_str(include_str!("../../../../../data/game_data.json"))
            .expect("game_data.json must parse")
    }

    fn solo(d: &GameData) -> GameState {
        let mut s = GameState::new(1, 16, 8);
        s.current_unit_def = Some("seraphine".to_string());
        let thief = d.unit("seraphine").unwrap();
        s.grid.spawn(build_player_entity(d, thief, FIRST_PLAYER_ID, Cell::new(0, 0)));
        s.party_slots = MAX_PARTY as u8; // these tests are about the unit, not the slots
        s
    }

    /// 0.0.492 renamed `rapis` → `lapis` and `nyx` → `raplus`: a save written on the old ids
    /// loads onto the new ones everywhere it names a unit.
    #[test]
    fn a_save_on_the_old_unit_ids_loads_onto_lapis_and_raplus() {
        let d = game_data();
        let mut s = solo(&d);
        let id = grant_exemplar_unit(&mut s, &d, "lapis").expect("unit");
        // Write it back the way a 0.0.491 save stored it.
        let rc = s.roster.get_mut(&id).unwrap();
        rc.unit_def = "rapis".into();
        rc.data.exemplar.as_mut().unwrap().id = "rapis".into();
        s.pending_resonance.insert("nyx".into(), 2);
        s.pending_resonance.insert("raplus".into(), 1);
        s.current_unit_def = Some("nyx".into());
        s.inventory.push(crate::items::Item::new("exemplar_nyx", crate::items::ItemKind::Material, 0, 0, crate::items::Rarity::Common));
        crate::data::migrate_save_items(&mut s, &d);
        let rc = &s.roster[&id];
        assert_eq!((rc.unit_def.as_str(), rc.data.exemplar.as_ref().unwrap().id.as_str()), ("lapis", "lapis"));
        assert_eq!(s.pending_resonance.get("raplus"), Some(&3), "credits merge onto the new id");
        assert!(!s.pending_resonance.contains_key("nyx"));
        assert_eq!(s.current_unit_def.as_deref(), Some("raplus"));
        assert!(s.inventory.iter().all(|it| it.def_id != "exemplar_nyx"));
        assert_eq!(crate::items::current_unit_id("seraphine"), "seraphine", "other ids pass through");
    }

    #[test]
    fn first_copy_is_a_unit_and_every_later_copy_is_a_credit() {
        let d = game_data();
        let mut s = solo(&d);
        let id = grant_exemplar_unit(&mut s, &d, "raplus").expect("first copy makes a unit");
        let rc = &s.roster[&id];
        assert_eq!(rc.unit_def, "raplus", "the unit stands on its own def");
        let x = rc.data.exemplar.as_ref().expect("the unit IS the Exemplar");
        assert_eq!((x.id.as_str(), x.stars), ("raplus", 0));
        assert_eq!(rc.data.stats.level, 1);
        assert!(s.pending_resonance.is_empty());

        assert_eq!(grant_exemplar_unit(&mut s, &d, "raplus"), None, "a dupe makes no unit");
        assert_eq!(grant_exemplar_unit(&mut s, &d, "raplus"), None);
        assert_eq!(s.roster.len(), 1, "still one unit");
        assert_eq!(s.pending_resonance["raplus"], 2, "two credits banked, spent by nobody yet");
    }

    #[test]
    fn a_fixed_kit_exemplar_becomes_its_own_player_class() {
        let d = game_data();
        let mut s = solo(&d);
        let id = grant_exemplar_unit(&mut s, &d, "seraphine").expect("unit");
        assert_eq!(s.roster[&id].unit_def, "seraphine", "seraphine is her own class, not a swordsman");
        // Her fixed kit and her Exemplar overlay describe the same character: deploying must not
        // leave a skill twice in the list, and must not lose the kit.
        deploy(&mut s, &d, id).expect("deploys");
        let e = s.grid.entity_of_id(id).unwrap();
        let mut ids: Vec<&str> = e.skills.iter().map(|k| k.id.as_str()).collect();
        let n = ids.len();
        ids.sort();
        ids.dedup();
        assert_eq!(ids.len(), n, "no skill id appears twice: {:?}", e.skills.iter().map(|k| &k.id).collect::<Vec<_>>());
        assert!(n > 0, "the kit is present");
    }

    #[test]
    fn unknown_exemplar_grants_nothing() {
        let d = game_data();
        let mut s = solo(&d);
        assert_eq!(grant_exemplar_unit(&mut s, &d, "nobody"), None);
        assert!(s.roster.is_empty() && s.pending_resonance.is_empty());
    }

    /// A line the banner sells before its unit def ships (rowena) cannot be stood up, so the
    /// pull banks a credit: the first grant after the unit ships makes the unit, and the credit
    /// is a star waiting for it. A pull is never lost.
    #[test]
    fn a_planned_line_banks_a_credit_instead_of_a_unit() {
        let d = game_data();
        let mut s = solo(&d);
        assert!(d.unit("rowena").is_none(), "rowena is still a planned line");
        assert_eq!(grant_exemplar_unit(&mut s, &d, "rowena"), None);
        assert!(s.roster.is_empty());
        assert_eq!(s.pending_resonance["rowena"], 1);
    }

    #[test]
    fn stars_rise_only_when_the_player_feeds_and_only_while_a_credit_exists() {
        let d = game_data();
        let mut s = solo(&d);
        let id = grant_exemplar_unit(&mut s, &d, "raplus").unwrap();
        feed_exemplar_unit(&mut s, id);
        assert_eq!(s.roster[&id].data.exemplar.as_ref().unwrap().stars, 0, "no credit → no star");
        grant_exemplar_unit(&mut s, &d, "raplus"); // +1 credit
        assert_eq!(s.roster[&id].data.exemplar.as_ref().unwrap().stars, 0, "a dupe does NOT auto-feed");
        let fed = s.measures.exemplars_fed.get();
        feed_exemplar_unit(&mut s, id);
        assert_eq!(s.roster[&id].data.exemplar.as_ref().unwrap().stars, 1);
        assert!(!s.pending_resonance.contains_key("raplus"), "the spent credit is gone");
        assert_eq!(s.measures.exemplars_fed.get(), fed + 1);
    }

    #[test]
    fn stars_cap_and_a_credit_is_not_burned_at_the_cap() {
        let d = game_data();
        let mut s = solo(&d);
        let id = grant_exemplar_unit(&mut s, &d, "raplus").unwrap();
        for _ in 0..(EXEMPLAR_MAX_STARS as usize + 3) {
            grant_exemplar_unit(&mut s, &d, "raplus");
        }
        for _ in 0..(EXEMPLAR_MAX_STARS as usize + 3) {
            feed_exemplar_unit(&mut s, id);
        }
        assert_eq!(s.roster[&id].data.exemplar.as_ref().unwrap().stars, EXEMPLAR_MAX_STARS);
        assert_eq!(s.pending_resonance["raplus"], 3, "credits past the cap stay banked");
    }

    #[test]
    fn a_deployed_unit_can_be_fed_and_its_new_rank_is_live() {
        let d = game_data();
        let mut s = solo(&d);
        let id = grant_exemplar_unit(&mut s, &d, "raplus").unwrap();
        grant_exemplar_unit(&mut s, &d, "raplus");
        deploy(&mut s, &d, id).expect("deploys");
        feed_exemplar_unit(&mut s, id);
        let e = s.grid.entity_of_id(id).unwrap();
        assert_eq!(e.exemplar.as_ref().unwrap().stars, 1);
    }

    #[test]
    fn feeding_a_non_exemplar_unit_is_a_no_op() {
        let d = game_data();
        let mut s = solo(&d);
        s.pending_resonance.insert("raplus".into(), 1);
        feed_exemplar_unit(&mut s, FIRST_PLAYER_ID); // the plain thief
        assert_eq!(s.pending_resonance["raplus"], 1);
    }
}

#[cfg(test)]
mod refund_tests {
    use super::*;
    use crate::grid::Cell;
    use crate::tick::{cum_at, cum_base, xp_to_next, APEX_LEVEL_CAP, LEVEL_CAP};

    fn game_data() -> GameData {
        GameData::from_json_str(include_str!("../../../../../data/game_data.json"))
            .expect("game_data.json must parse")
    }

    /// A deployed lead (id 1) plus a BENCHED unit (id 2) at `level` with a reconciled bar.
    fn state_with_benched(level: u32, apex: u32, bar: u64) -> GameState {
        let d = game_data();
        let mut s = GameState::new(1, 16, 8);
        s.current_unit_def = Some("seraphine".to_string());
        let thief = d.unit("seraphine").unwrap();
        s.grid.spawn(build_player_entity(&d, thief, FIRST_PLAYER_ID, Cell::new(0, 0)));
        let mut e = build_player_entity(&d, thief, 2, Cell::new(1, 0));
        e.stats.level = level;
        e.stats.apex_level = apex;
        e.stats.xp = bar;
        e.stats.level_points = level.saturating_sub(1); // as if levelled the honest way
        let rc = crate::state::RosterChar {
            unit_def: "seraphine".into(),
            data: crate::state::CharacterData::capture(&mut e),
        };
        s.roster.insert(2, rc);
        s
    }

    fn total_engram_xp(s: &GameState) -> u64 {
        (0..ENGRAM_TIERS.len()).map(|t| s.owned_engrams[t] as u64 * ENGRAM_TIERS[t]).sum()
    }

    #[test]
    fn refund_mints_nothing_and_resets_the_unit() {
        // Lv30 with a partial bar: the Engrams must equal the unit's cumulative XP minus only the
        // sub-1,000 remainder, and the unit must come back as a fresh Lv1 with nothing owed.
        let mut s = state_with_benched(30, 0, 777);
        let before = cum_base(&s.roster[&2].data.stats);
        let measures_before = s.measures.levels_gained.get();
        refund_unit_xp(&mut s, 2);
        let got = total_engram_xp(&s);
        assert!(got <= before && before - got < 1_000, "before={before} engrams={got}");
        let st = &s.roster[&2].data.stats;
        assert_eq!((st.level, st.xp, st.apex_level, st.level_points), (1, 0, 0, 0));
        assert_eq!(st.apex, [0; crate::stats::APEX_BUCKETS], "allocation returns with the points");
        assert_eq!(s.measures.levels_gained.get(), measures_before, "a refund is invisible to Measures");
        // A second refund of the now-fresh unit mints nothing.
        refund_unit_xp(&mut s, 2);
        assert_eq!(total_engram_xp(&s), got);
    }

    #[test]
    fn refund_is_roster_only() {
        let mut s = state_with_benched(30, 0, 0);
        s.grid.entities[0].stats.level = 40;
        refund_unit_xp(&mut s, FIRST_PLAYER_ID); // deployed → no-op
        assert_eq!(s.grid.entities[0].stats.level, 40);
        assert_eq!(total_engram_xp(&s), 0);
        refund_unit_xp(&mut s, 99); // unknown → no-op
        assert_eq!(total_engram_xp(&s), 0);
    }

    #[test]
    fn apply_cannot_exceed_the_ceiling_and_lands_with_an_empty_bar() {
        // The account has only ever reached Lv30 (the deployed lead). Pouring a fortune into a
        // Lv1 benched unit lands it at EXACTLY Lv30 with a zero bar — overflow discarded, never a
        // parked bar that turns into a free level when the cap rises.
        let mut s = state_with_benched(1, 0, 0);
        s.grid.entities[0].stats.level = 30;
        s.owned_engrams = [0, 0, 0, 50]; // 50M XP
        apply_engram(&mut s, 2, 3, 50);
        let st = &s.roster[&2].data.stats;
        assert_eq!(st.level, 30, "capped at the account's ceiling");
        assert_eq!(st.xp, 0, "the ceiling level's bar is EMPTY");
        assert_eq!(st.level_points, 29, "the levels it was poured through paid their points");
        assert_eq!(s.owned_engrams[3], 0, "stock is spent even when part of it was waste");
    }

    #[test]
    fn a_stale_mark_never_caps_a_unit_below_a_unit_that_already_stands_higher() {
        // Old saves default the marks to 0; the ceiling is lifted by the current max across units.
        let mut s = state_with_benched(1, 0, 0);
        s.level_reached = 0;
        s.grid.entities[0].stats.level = 12;
        s.owned_engrams = [0, 0, 0, 5];
        apply_engram(&mut s, 2, 3, 5);
        assert_eq!(s.roster[&2].data.stats.level, 12);
    }

    #[test]
    fn apply_across_the_level_cap_pours_into_apex_and_stops_at_the_apex_mark() {
        let mut s = state_with_benched(1, 0, 0);
        s.grid.entities[0].stats.level = LEVEL_CAP;
        s.grid.entities[0].stats.apex_level = 3;
        s.owned_engrams = [0, 0, 0, 2_000]; // 2,000M XP, far past 3 Apex points
        apply_engram(&mut s, 2, 3, 2_000);
        let st = &s.roster[&2].data.stats;
        assert_eq!(st.level, LEVEL_CAP);
        assert_eq!(st.apex_level, 3, "apex capped at the account's apex mark");
        assert_eq!(st.xp, 0);
        assert!(st.apex_level <= APEX_LEVEL_CAP);
    }

    #[test]
    fn apply_clamps_to_stock_and_ignores_a_bad_tier() {
        let mut s = state_with_benched(1, 0, 0);
        s.grid.entities[0].stats.level = 50;
        s.owned_engrams = [3, 0, 0, 0];
        apply_engram(&mut s, 2, 0, 100); // asks for 100, owns 3
        assert_eq!(s.owned_engrams[0], 0);
        assert_eq!(cum_base(&s.roster[&2].data.stats), 3_000);
        apply_engram(&mut s, 2, 9, 1); // bad tier → no-op
        assert_eq!(cum_base(&s.roster[&2].data.stats), 3_000);
    }

    #[test]
    fn fill_to_max_preview_and_commit_agree_and_minimise_waste() {
        let mut s = state_with_benched(1, 0, 0);
        s.grid.entities[0].stats.level = 20;
        // Enough to reach Lv20 with change: the plan must use the big tiers that fit, then one
        // small Engram to top over, and the commit must land where the preview said.
        s.owned_engrams = [50, 50, 50, 50];
        let target = cum_at(20, 0);
        let pv = engram_fill_preview(&s, 2).expect("benched unit previews");
        assert_eq!(pv.resulting_level, 20);
        let planned: u64 = (0..4).map(|t| pv.plan[t] as u64 * ENGRAM_TIERS[t]).sum();
        assert!(planned >= target, "the plan reaches the ceiling");
        assert!(planned - target < 1_000, "and overshoots by less than one smallest Engram: waste={}", pv.xp_wasted);
        assert_eq!(pv.xp_wasted, planned - target);
        apply_engrams_to_max(&mut s, 2);
        let st = &s.roster[&2].data.stats;
        assert_eq!((st.level, st.xp), (20, 0));
        for t in 0..4 {
            assert_eq!(s.owned_engrams[t], 50 - pv.plan[t], "commit spent exactly the previewed plan");
        }
    }

    #[test]
    fn fill_to_max_on_a_unit_already_at_the_ceiling_spends_nothing_and_warns_nothing() {
        let mut s = state_with_benched(20, 0, xp_to_next(20) / 2); // AT the ceiling with a partial bar
        s.grid.entities[0].stats.level = 20;
        s.owned_engrams = [9, 9, 9, 9];
        let pv = engram_fill_preview(&s, 2).unwrap();
        assert_eq!(pv.plan, [0; 4]);
        assert_eq!(pv.xp_wasted, 0, "no phantom waste when nothing is spent");
        apply_engrams_to_max(&mut s, 2);
        assert_eq!(s.owned_engrams, [9, 9, 9, 9]);
    }

    #[test]
    fn refund_then_apply_round_trips_a_unit_within_one_engram() {
        // Drain A, pour it back into A: A comes back to (nearly) where it was, and the account's
        // marks and Measures have not moved — the exploit this design exists to close.
        let mut s = state_with_benched(35, 0, 4_321);
        s.grid.entities[0].stats.level = 35;
        s.level_reached = 35;
        let before = cum_base(&s.roster[&2].data.stats);
        let lg = s.measures.levels_gained.get();
        refund_unit_xp(&mut s, 2);
        let pv = engram_fill_preview(&s, 2).unwrap();
        apply_engrams_to_max(&mut s, 2);
        let after = cum_base(&s.roster[&2].data.stats);
        let stock = total_engram_xp(&s);
        // EXACT accounting. The unit lands at the ceiling with an EMPTY bar (invariant), so its old
        // partial bar cannot come back onto it. Every XP is either on the unit, still in stock, or
        // in one of the two acknowledged sinks: the sub-1k remainder the refund drops, and the
        // overshoot of the one oversized Engram the fill spends to reach the ceiling — which is
        // precisely the figure the preview warned about.
        assert_eq!(
            before - (after + stock),
            (before % 1_000) + pv.xp_wasted,
            "before={before} unit={after} stock={stock} wasted={}",
            pv.xp_wasted
        );
        assert_eq!((s.roster[&2].data.stats.level, s.roster[&2].data.stats.xp), (35, 0));
        assert_eq!(s.level_reached, 35);
        assert_eq!(s.measures.levels_gained.get(), lg);
    }
}
