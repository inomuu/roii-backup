//! Targeting (nearest-enemy + taunt lock) and the hit / crit PRNG rolls.

// DEBT (4 sites): this file predates `deny(clippy::indexing_slicing)`. An index that
// cannot be proven in range is a panic, and in verify-server (which links this crate and
// replays saves under `panic = "abort"`) a panic is the whole process. Shrink this, never
// add to it — a NEW file has no allow and is rejected on its first bare index.
#![allow(clippy::indexing_slicing)]

use crate::grid::Entity;
use crate::rng::Pcg32;
use super::*;



/// Count of living enemies that are actually ATTACKING `defender_idx`: their
/// current target IS this defender AND it's in their range to hit (Chebyshev ≤
/// their atk_range). This is the "how many things are swinging at me" number the
/// mobbed flee/DEF penalties key off. Target-gated (not range-only) so a ranged
/// enemy whose range happens to cover several party members — but who is hitting
/// only ONE of them — penalizes just that one, not the whole party.
pub fn attackers_on(entities: &[Entity], defender_idx: usize) -> u32 {
    let d = &entities[defender_idx];
    let mut n = 0u32;
    for (i, e) in entities.iter().enumerate() {
        if i == defender_idx || !e.alive || e.team == d.team {
            continue;
        }
        if e.target_id == Some(d.id) && e.pos.chebyshev(d.pos) <= effective_atk_range(e) {
            n += 1;
        }
    }
    n
}


// The renewal "mobbed" DEF penalty (−5% DEF per attacker beyond two, −100% at 22) is RETIRED
// (0.0.407, user 2026-09-17: "เอา penalty ตอนโดนรุมออก"): a fight is meant to be a long stand
// inside the crowd where the crowd chips, WoG-style, and the penalty made standing in one a
// death sentence. `attackers_on` stays — the mob count still drives flee and targeting.

/// The mobbed penalty, now the identity: DEF is DEF whoever is swinging. Kept as the seam the
/// damage path calls, so restoring a crowd rule is one function.
pub fn mobbed_def(def: i32, _attacker_count: u32) -> i32 {
    def
}

/// See `mobbed_def`. `resist_pct` (Endure) no longer has a penalty to resist.
pub fn mobbed_def_resisted(def: i32, _attacker_count: u32, _resist_pct: u32) -> i32 {
    def
}


/// Find the index of the nearest living enemy of `attacker_idx`.
/// Tie-break by spawn id ASC (GDD §3.2).
///
/// Returns `None` if no living enemy exists.
pub fn find_enemy_idx(entities: &[Entity], attacker_idx: usize) -> Option<usize> {
    let attacker = &entities[attacker_idx];
    if !attacker.alive {
        return None;
    }
    let my_team = attacker.team;
    let my_pos = attacker.pos;

    // Taunt lock: while taunted, target the taunter (if still a living
    // enemy) regardless of distance — that's the whole point of a taunt.
    if attacker.taunt_ticks > 0 {
        if let Some(tid) = attacker.taunted_by {
            if let Some((j, e)) = entities.iter().enumerate()
                .find(|(_, e)| e.id == tid)
            {
                if e.alive && e.team != my_team {
                    return Some(j);
                }
            }
        }
    }

    let mut best_idx: Option<usize> = None;
    let mut best_dist = i32::MAX;
    let mut best_id = u32::MAX;
    for (i, e) in entities.iter().enumerate() {
        if i == attacker_idx { continue; }
        if !e.alive { continue; }
        if e.team == my_team { continue; }
        // HIDING (thief panic) makes the entity untargetable — enemies pick the
        // next-nearest instead. If everyone is hidden, no target → enemies idle.
        if crate::skills::buff_sum(e, crate::skills::BuffKind::Hiding) > 0 { continue; }
        // An UNTARGETABLE pet (Ranger damage-pet) can't be attacked — enemies pick
        // the next target. (A future tank pet sets untargetable=false to soak hits.)
        if e.untargetable { continue; }
        // HELD by the player's hand (`regent_hand`) — lifted off the board, so
        // there is nothing here to attack. The hold is capped and costs the
        // character's damage uptime, which is what stops it being a free shield.
        if e.is_held() { continue; }   // (the `alive` check above + this = `on_board`)
        let d = my_pos.chebyshev(e.pos);
        let better = d < best_dist || (d == best_dist && e.id < best_id);
        if better {
            best_dist = d;
            best_id = e.id;
            best_idx = Some(i);
        }
    }
    best_idx
}

/// Public so the ranged path in `tick::attack_step` can use the same
/// rule (one draw, only if crit_chance > 0) when committing arrow
/// damage at fire time.
/// Chance this blow avoids the target entirely: the TARGET's dodge and the
/// ATTACKER's blindness, stacked the same inverse-multiplicative way dodge lines
/// stack — `1 - (1-dodge)(1-blind)`.
///
/// BLIND used to cut HIT by 25% and is the only thing the retired accuracy pair did
/// that was worth keeping: a status that makes you whiff. It lands here rather than
/// as its own roll so a blinded attacker facing a dodgy target still costs ONE draw.
pub fn avoid_bp(attacker: &Entity, target: &Entity) -> i32 {
    let dodge = crate::combat::stat_mods::dodge_bp(target).clamp(0, 10_000) as i64;
    let blind = if crate::skills::buff_sum(attacker, crate::skills::BuffKind::Blind) > 0 {
        BLIND_MISS_BP as i64
    } else {
        0
    };
    // Darkness (0.0.426): a magnitude-driven whiff, stacked the same inverse way.
    let dark = crate::skills::effective_bp(attacker, crate::skills::BuffKind::Darkness).clamp(0, 10_000) as i64;
    // Dazzle (0.0.464): the same whiff, its own kind, stacked the same inverse way.
    let dazzle = crate::skills::effective_bp(attacker, crate::skills::BuffKind::Dazzle).clamp(0, 10_000) as i64;
    let land = (10_000 - dodge) * (10_000 - blind) / 10_000 * (10_000 - dark) / 10_000 * (10_000 - dazzle) / 10_000;
    (10_000 - land) as i32
}

/// A blinded attacker whiffs a quarter of its blows — what `BuffKind::Blind` was
/// worth when it cut HIT by 25%, kept at the same value through the change.
pub const BLIND_MISS_BP: i32 = 2_500;

/// Does this blow avoid the target? ONE draw, and only when something can actually
/// avoid it — a plain attacker against a plain target takes no draw at all, which is
/// what keeps an ordinary fight's PRNG stream free of this question entirely.
pub fn roll_dodge(rng: &mut Pcg32, attacker: &Entity, target: &Entity) -> bool {
    let bp = avoid_bp(attacker, target);
    if bp <= 0 {
        return false;
    }
    rng.gen_range_u32(0, 10_000) < bp.min(10_000) as u32
}

pub fn roll_crit(rng: &mut Pcg32, crit_chance_bp: u32) -> bool {
    if crit_chance_bp == 0 {
        return false;
    }
    rng.gen_range_u32(0, 10_000) < crit_chance_bp
}

/// A crit for a skill hit that does not go through `resolve_attack_scaled` (a damage-field
/// tick, a shield burst, a plague blast), rolled off its SOURCE: on a crit the source's crit
/// damage is folded into `amount`. No draw when the source has no crit chance (0.0.472).
pub fn roll_source_crit(rng: &mut Pcg32, src: &crate::grid::Entity, amount: i32) -> (i32, bool) {
    if !roll_crit(rng, super::effective_crit_chance_bp(src)) {
        return (amount, false);
    }
    let amt = (amount as i64) * i64::from(super::effective_crit_dmg_bp(src)) / 10_000;
    (amt.min(i32::MAX as i64) as i32, true)
}



#[cfg(test)]
mod hiding_tests {
    use super::*;
    use crate::grid::{Cell, Entity, Team};
    use crate::skills::{Buff, BuffKind};

    #[test]
    fn hiding_entity_is_untargetable() {
        let mut es = vec![
            Entity::idle(1000, Cell::new(0, 0)).with_team(Team::Monster),
            Entity::idle(1, Cell::new(1, 0)).with_team(Team::Player),
        ];
        // Visible → the monster targets the player.
        assert_eq!(find_enemy_idx(&es, 0), Some(1));
        // Hidden → untargetable; with no other enemy the monster finds nothing.
        es[1].buffs.push(Buff { kind: BuffKind::Hiding, magnitude: 1, ticks: 100, source_id: 1 });
        assert_eq!(find_enemy_idx(&es, 0), None, "a hiding entity can't be targeted");
    }
}
