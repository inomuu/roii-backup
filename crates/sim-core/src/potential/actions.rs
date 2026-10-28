//! Shared quotes and point mutations. Renderers never reproduce these rules.
use super::*;
use crate::grid::{Entity, Team};
use crate::state::GameState;

/// Existing skill ranks are credited without spending them a second time. A
/// zero-investment legacy save is read unchanged; its skills also provide graph
/// entry points. Stat/major ranks only come from paid board state.
pub fn effective_board(e: &Entity) -> PotentialState {
    let mut board = e.potential.clone();
    if doctrine_for_unit(&e.unit_def).is_none() {
        return board;
    }
    for def in &NODE_DEFS {
        if let NodeKind::Skill { slot } = def.kind {
            if let Some(id) = skill_id(&e.unit_def, slot) {
                let rank = e.skill_level(id).min(def.max_rank());
                if rank > board.rank(def.id) {
                    board.ranks.insert(def.id.into(), rank);
                }
            }
        }
    }
    board
}

pub fn caps(_e: &Entity) -> TierCaps {
    // Commander is deliberately ungated until a trusted entitlement is wired.
    TierCaps { commander: 10 }
}

/// Everything a purchase can spend, read from the bag and the purse.
pub fn purse(state: &GameState) -> Purse {
    // The ledger, not the bag (0.0.354): every family is a `GameState.materials` count.
    let mut p = Purse { gil: state.gil(), ..Purse::default() };
    for (g, held) in p.sigil.iter_mut().enumerate() {
        *held = state.material_count(&sigil_def(g));
    }
    p.core = state.material_count(BOSS_CORE_DEF);
    p.track = state.material_count(TRACK_DEF);
    p
}

fn take(state: &mut GameState, def_id: &str, owed: u32) {
    if owed > 0 {
        // The purse was read from this ledger a moment ago, so the debit cannot come up short.
        state.debit_material(def_id, owed);
    }
}

/// Debit the ledger and the purse by what a purchase spent: `before - after` per family, so the
/// batch preview and the commit cannot disagree.
fn debit(state: &mut GameState, before: Purse, after: Purse) {
    for (g, (b, a)) in before.sigil.iter().zip(after.sigil.iter()).enumerate() {
        take(state, &sigil_def(g), b.saturating_sub(*a));
    }
    take(state, BOSS_CORE_DEF, before.core.saturating_sub(after.core));
    take(state, TRACK_DEF, before.track.saturating_sub(after.track));
    let gil = before.gil.saturating_sub(after.gil);
    if gil > 0 {
        state.spend_gil(gil, crate::state::GilSpend::Potential);
    }
}

pub fn lock_text(reason: LockReason) -> String {
    match reason {
        LockReason::UnsupportedCharacter => {
            "This character's fixed kit is not authored for Potential yet.".into()
        }
        LockReason::UnknownNode => "Unknown node.".into(),
        LockReason::RootAlreadyUnlocked => "Origin is already unlocked.".into(),
        LockReason::AlreadyMax => "Maximum rank reached.".into(),
        LockReason::NotAuthored => {
            "This node's effect is still being designed; no points can be spent.".into()
        }
        LockReason::CommanderTier { required, .. } => {
            format!("Requires Commander tier {required}.")
        }
        LockReason::NeedsNeighbor => "Unlock a connected node first.".into(),
        LockReason::NeedsTwoMajors => "Unlock any two major traces first.".into(),
        LockReason::NotMajor => "Select a major trace as the upgrade destination.".into(),
        LockReason::NoPath => "No available path to this major trace.".into(),
    }
}

fn requirements(e: &Entity, id: &str) -> Result<Upgrade, String> {
    if e.team != Team::Player || e.is_pet {
        return Err("Only party characters can use Potential.".into());
    }
    let board = effective_board(e);
    let up = eligibility(&e.unit_def, &board, id, caps(e)).map_err(lock_text)?;
    if let Some(NodeDef {
        kind: NodeKind::Skill { slot },
        ..
    }) = node(id)
    {
        let skill = skill_id(&e.unit_def, *slot)
            .and_then(crate::skills::skill_def)
            .ok_or_else(|| lock_text(LockReason::NotAuthored))?;
        // A rank past the skill's own `max_level` is an OVERRANK (+2% each, `power_at`); the
        // ladder's cap is `SKILL_RANK_CAP`, enforced by `max_rank`.
        let _ = skill;
        // Overlay grants are refreshed every tick; a paid rank must not be lost
        // to that refresh or later returned as a grant the player did not buy.
        if e.skills
            .iter()
            .any(|s| s.id == skill.id && (s.from_exemplar || s.from_regalia))
        {
            return Err("This skill is supplied by an equipped overlay.".into());
        }
    }
    Ok(up)
}

/// What raising `id` costs, whether or not it can be afforded.
pub fn cost_of(e: &Entity, id: &str) -> Result<Cost, String> {
    let up = requirements(e, id)?;
    let def = node(id).ok_or_else(|| lock_text(LockReason::UnknownNode))?;
    Ok(node_cost(def.kind, up.next_rank))
}

pub fn quote(e: &Entity, purse: &Purse, id: &str) -> Result<Upgrade, String> {
    let upgrade = requirements(e, id)?;
    let def = node(id).ok_or_else(|| lock_text(LockReason::UnknownNode))?;
    // The Sigil is the UNIT'S ELEMENT — not its doctrine.
    purse.can_pay(&node_cost(def.kind, upgrade.next_rank))?;
    Ok(upgrade)
}

fn apply_to(e: &mut Entity, purse: &mut Purse, id: &str) -> bool {
    let Ok(up) = quote(e, purse, id) else {
        return false;
    };
    let Some(def) = node(id) else {
        return false;
    };
    if let NodeKind::Skill { slot } = def.kind {
        let Some(sid) = skill_id(&e.unit_def, slot) else {
            return false;
        };
        let old = e.skill_level(sid);
        e.potential
            .base_skill_levels
            .entry(sid.to_string())
            .or_insert(old);
        if let Some(s) = e.skills.iter_mut().find(|s| s.id == sid) {
            s.level = up.next_rank;
        } else {
            e.skills
                .push(crate::skills::SkillState::new(sid, up.next_rank));
        }
        if old == 0 {
            e.auto_slot_skill(sid);
        }
    }
    let cost = node_cost(def.kind, up.next_rank);
    purse.pay(&cost);
    e.potential.spent_points = e.potential.spent_points.saturating_add(1);
    e.potential.ranks.insert(id.into(), up.next_rank);
    true
}

pub fn upgrade(state: &mut GameState, entity_id: u32, id: &str) {
    let before = purse(state);
    let mut p = before;
    let Some(e) = state.grid.entity_of_id_mut(entity_id) else {
        return;
    };
    if apply_to(e, &mut p, id) {
        debit(state, before, p);
        state.measures.potential_ranks.add(1);
        effects::refresh(&mut state.grid.entities, state.tick, true);
    }
}

/// A bounded, affordable prefix of the shortest route to a selected major.
/// The quote and action run the same simulator over the same stable node order.
pub fn max_plan(e: &Entity, purse: &Purse, target: &str) -> Result<Vec<String>, String> {
    if e.team != Team::Player || e.is_pet {
        return Err("Only party characters can use Potential.".into());
    }
    let path =
        path_toward_major(&e.unit_def, &effective_board(e), target, caps(e)).map_err(lock_text)?;
    if path.is_empty() {
        return Err("This major trace is already unlocked.".into());
    }
    let mut preview = e.clone();
    let mut p = *purse;
    let mut plan = Vec::new();
    for id in path {
        match quote(&preview, &p, id) {
            Ok(_) => {
                if apply_to(&mut preview, &mut p, id) {
                    plan.push(id.to_string());
                }
            }
            Err(reason) => {
                if plan.is_empty() {
                    return Err(reason);
                }
                break;
            }
        }
    }
    Ok(plan)
}

pub fn upgrade_max(state: &mut GameState, entity_id: u32, target: &str) {
    let before = purse(state);
    let mut p = before;
    let Some(e) = state.grid.entity_of_id_mut(entity_id) else {
        return;
    };
    let Ok(plan) = max_plan(e, &p, target) else {
        return;
    };
    let mut bought = 0u64;
    for id in plan {
        if !apply_to(e, &mut p, &id) {
            break;
        }
        bought += 1;
    }
    debit(state, before, p);
    state.measures.potential_ranks.add(bought);
    effects::refresh(&mut state.grid.entities, state.tick, true);
}

/// NO RESET (user 2026-09-11): a rank bought stays bought. The reason is the same for every
/// character, so a screen can say it rather than show a dead button.
/// `MergeSigil`: `count × SIGIL_MERGE` of `grade` → `count` of `grade + 1`. `_element` is read and
/// ignored: Sigils have no element since 2026-09-30, the field stays so old logs parse.
pub fn merge_sigil(state: &mut GameState, _element: &str, grade: u8, count: u32) {
    let g = grade as usize;
    if count == 0 || g + 1 >= SIGIL_GRADES {
        return;
    }
    let Some(need) = count.checked_mul(SIGIL_MERGE) else { return };
    if !state.debit_material(&sigil_def(g), need) {
        return;
    }
    state.credit_material(&sigil_def(g + 1), count, crate::state::MaterialSource::Conversion);
}

/// `SplitSigil`: `count` of `grade` → `count × SIGIL_MERGE` of `grade - 1`. `_element` is ignored
/// like `merge_sigil`'s.
pub fn split_sigil(state: &mut GameState, _element: &str, grade: u8, count: u32) {
    let g = grade as usize;
    if count == 0 || g == 0 || g >= SIGIL_GRADES {
        return;
    }
    let Some(out) = count.checked_mul(SIGIL_MERGE) else { return };
    if !state.debit_material(&sigil_def(g), count) {
        return;
    }
    state.credit_material(&sigil_def(g - 1), out, crate::state::MaterialSource::Conversion);
}

pub fn reset_reason(_e: &Entity) -> Option<String> {
    Some("Potential cannot be reset.".into())
}

/// Tombstone — `ResetPotential` still parses for old event logs and does nothing.
pub fn reset(_state: &mut GameState, _entity_id: u32) {}

#[cfg(test)]
mod tests;
