//! Ranger TRAP system (Land Mine / Claymore / Freezing). Traps are dormant ground
//! objects (`GameState.traps`, NOT entities): an archer auto-places one on the 5×5
//! perimeter around itself when a slotted trap skill is off cooldown and a threat
//! exists; it arms until an enemy steps into its 3×3, then fires ONCE and is
//! consumed. Persist until a zone change (cleared in `change_zone`).
//!
//! Determinism: placement takes a SINGLE PRNG draw, and only when the archer owns
//! + slots a trap skill (no golden party does) → the stream is byte-identical.
//! Triggering and damage take no draws (freeze is guaranteed CC, no resist roll).

// DEBT (33 sites): this file predates `deny(clippy::indexing_slicing)`. An index that
// cannot be proven in range is a panic, and in verify-server (which links this crate and
// replays saves under `panic = "abort"`) a panic is the whole process. Shrink this, never
// add to it — a NEW file has no allow and is rejected on its first bare index.
#![allow(clippy::indexing_slicing)]

use super::xp::kill_monster;
use crate::grid::{Cell, DamageType, Element, Team};
use crate::skills::{self, Buff, BuffKind};
use crate::combat;
use crate::state::{GameState, HitEvent, Trap, TrapFire, TrapKind};

/// Max simultaneous traps PER skill, per owner (learning two trap kinds = two
/// independent perimeters). Per-kind: Land Mine is UNCAPPED (it's still naturally
/// bounded by the 16-cell perimeter ring — one trap per cell); Claymore/Freezing
/// keep a small cap so their AoE/CC doesn't blanket the ring.
const TRAP_CAP: usize = 3;

fn trap_cap(kind: TrapKind) -> usize {
    match kind {
        TrapKind::LandMine => usize::MAX, // uncapped (bounded by the perimeter ring)
        _ => TRAP_CAP,
    }
}
/// Claymore fuse after it triggers: 5 ticks (0.5 s) for more foes to wander in.
const CLAYMORE_FUSE: u32 = 5;

/// The 16 perimeter cells of the 5×5 around the archer (Chebyshev distance 2),
/// as `(dx, dy)` offsets. A defensive ring that intercepts approaching enemies.
const PERIMETER: [(i32, i32); 16] = [
    (-2, -2), (-1, -2), (0, -2), (1, -2), (2, -2), // top edge
    (2, -1), (2, 0), (2, 1), // right edge
    (2, 2), (1, 2), (0, 2), (-1, 2), (-2, 2), // bottom edge
    (-2, 1), (-2, 0), (-2, -1), // left edge
];

/// Map a skill id to its trap kind. The single source of truth for "is this a
/// trap skill"; `None` for everything else.
fn trap_kind_of(id: &str) -> Option<TrapKind> {
    match id {
        "land_mine" => Some(TrapKind::LandMine),
        "claymore_trap" => Some(TrapKind::Claymore),
        "freezing_trap" => Some(TrapKind::Freezing),
        _ => None,
    }
}

/// Auto-place a trap if entity `i` has a slotted trap skill off cooldown, is below
/// the per-kind cap, and a living enemy exists (don't lay traps in an empty room).
/// Returns true when a trap was placed (= this entity's action this tick), matching
/// the other proactive casts. Deterministic; one PRNG draw on success.
pub(crate) fn try_place_trap(state: &mut GameState, i: usize) -> bool {
    let owner_id = state.grid.entities[i].id;
    let me_team = state.grid.entities[i].team;

    // Pick the first ready trap skill: slotted (reserve budget), off cooldown, and
    // under its cap. Kinds are independent, so a full Land Mine ring doesn't block
    // a Freezing Trap.
    let mut chosen: Option<(usize, TrapKind, u8)> = None;
    for (slot, s) in state.grid.entities[i].skills.iter().enumerate() {
        let Some(kind) = trap_kind_of(&s.id) else { continue; };
        if s.cooldown > 0 {
            continue;
        }
        if !skills::loadout_fits(
            &state.grid.entities[i].active_skills,
            combat::loadout_capacity(&state.grid.entities[i]),
            &s.id,
        ) {
            continue;
        }
        let owned = state
            .traps
            .iter()
            .filter(|t| t.owner_id == owner_id && t.kind == kind)
            .count();
        if owned >= trap_cap(kind) {
            continue;
        }
        chosen = Some((slot, kind, s.level));
        break;
    }
    let Some((slot, kind, level)) = chosen else { return false; };

    // Only bother when a threat is actually present.
    let enemy_exists = state
        .grid
        .entities
        .iter()
        .any(|e| e.alive && e.team != me_team);
    if !enemy_exists {
        return false;
    }

    // Bias placement toward the ENEMY side — a uniform pick over the full ring
    // wasted ~half the traps behind the archer where nothing walks. Find the
    // nearest living enemy, keep only perimeter cells in its half-plane (offset ·
    // threat-dir > 0), and pick uniformly among THOSE (one draw). Fall back to the
    // full free ring if the front is blocked / there's no clear direction. The
    // threat lookup + filtering are deterministic (no draws); the single
    // `gen_range` draw is preserved + still skill-gated (goldens unaffected).
    let origin = state.grid.entities[i].pos;
    let threat = state
        .grid
        .entities
        .iter()
        .filter(|e| e.alive && e.team != me_team)
        .min_by_key(|e| e.pos.chebyshev(origin))
        .map(|e| e.pos);
    let free: Vec<Cell> = PERIMETER
        .iter()
        .map(|(dx, dy)| Cell::new(origin.x + dx, origin.y + dy))
        .filter(|c| trap_cell_free(state, *c))
        .collect();
    if free.is_empty() {
        return false;
    }
    let candidates: Vec<Cell> = match threat {
        Some(t) => {
            let (tx, ty) = (t.x - origin.x, t.y - origin.y);
            let front: Vec<Cell> = free
                .iter()
                .copied()
                .filter(|c| (c.x - origin.x) * tx + (c.y - origin.y) * ty > 0)
                .collect();
            if front.is_empty() {
                free
            } else {
                front
            }
        }
        None => free,
    };
    let pick = state.rng.gen_range_u32(0, candidates.len() as u32) as usize;
    let cell = candidates[pick];

    // Seed seen_ids with whoever's ALREADY inside the 3×3 so a trap dropped under a
    // sitting enemy doesn't go off — it waits for a NEW enemy to step in.
    let seen_ids = monster_ids_in(state, cell);
    state.traps.push(Trap { cell, kind, level, owner_id, fuse: 0, triggered: false, seen_ids });
    if let Some(def) = skills::skill_def(&state.grid.entities[i].skills[slot].id) {
        state.grid.entities[i].skills[slot].cooldown = def.cooldown_ticks;
    }
    true
}

/// A perimeter cell can hold a trap if it's in bounds and no other trap already
/// sits there. Traps lie ON THE GROUND, so an entity standing on the cell does NOT
/// block placement — that's the whole point: the ring must reach the side enemies
/// approach from (a monster-occupied cell is exactly where we WANT a trap). Filtering
/// out occupied cells used to shove every trap to the empty far side (the "always
/// bottom-left" bug).
fn trap_cell_free(state: &GameState, c: Cell) -> bool {
    if !state.grid.in_bounds(c) {
        return false;
    }
    if state.traps.iter().any(|t| t.cell == c) {
        return false;
    }
    true
}

/// Advance every trap once: fuse Claymores down, fire any trap an enemy entered.
/// Called once per tick AFTER all entities + pets have moved, so triggers see
/// final positions. Iterates in trap index order (deterministic). No PRNG draws.
pub(crate) fn tick_traps(state: &mut GameState) {
    if state.traps.is_empty() {
        return;
    }
    let mut remove = vec![false; state.traps.len()];
    for ti in 0..state.traps.len() {
        let trap = state.traps[ti].clone();

        // Already-triggered Claymore: count the fuse down, explode at 0.
        if trap.triggered {
            let nf = trap.fuse.saturating_sub(1);
            if nf == 0 {
                claymore_explode(state, &trap);
                state.trap_fires.push(TrapFire { x: trap.cell.x, y: trap.cell.y, kind: trap.kind });
                remove[ti] = true;
            } else {
                state.traps[ti].fuse = nf;
            }
            continue;
        }

        // Dormant: EDGE-trigger. Fire only when a NEW enemy id enters the 3×3 (one
        // not seen last check) — an enemy already sitting on the trap when it was
        // placed (or still loitering) doesn't set it off.
        let current = monster_ids_in(state, trap.cell);
        let new_entry = current.iter().any(|id| !trap.seen_ids.contains(id));
        if !new_entry {
            // No fresh intruder: remember who's currently inside (so a leaver must
            // step back OUT and IN to count as new next time) and wait.
            state.traps[ti].seen_ids = current;
            continue;
        }
        match trap.kind {
            TrapKind::LandMine => {
                land_mine_hit(state, &trap);
                state.trap_fires.push(TrapFire { x: trap.cell.x, y: trap.cell.y, kind: trap.kind });
                remove[ti] = true;
            }
            TrapKind::Freezing => {
                freezing_trigger(state, &trap);
                state.trap_fires.push(TrapFire { x: trap.cell.x, y: trap.cell.y, kind: trap.kind });
                remove[ti] = true;
            }
            TrapKind::Claymore => {
                // Arm the fuse; the blast lands when it reaches 0 (above).
                state.traps[ti].triggered = true;
                state.traps[ti].fuse = CLAYMORE_FUSE;
            }
        }
    }
    let mut k = 0;
    state.traps.retain(|_| {
        let keep = !remove[k];
        k += 1;
        keep
    });
}

/// Land Mine: hit the single triggering enemy (lowest entity index in the 3×3)
/// for `250% + 50%/lvl` of the owner's ATK. Neutral physical.
fn land_mine_hit(state: &mut GameState, trap: &Trap) {
    let Some(oidx) = state.grid.idx_of_id(trap.owner_id) else { return; };
    let atk = combat::effective_atk(&state.grid.entities[oidx]);
    let power = 250 + 50 * trap.level as i32; // Lv1 300% .. Lv5 500%
    let amount = atk * power / 100;
    let Some(tgt) = first_victim_in(state, trap.cell) else { return; };
    apply_trap_damage(state, oidx, tgt, amount, false, "land_mine");
}

/// Claymore: blast every enemy in the 3×3 for `150% + 30%/lvl` of the owner's ATK.
fn claymore_explode(state: &mut GameState, trap: &Trap) {
    let Some(oidx) = state.grid.idx_of_id(trap.owner_id) else { return; };
    let atk = combat::effective_atk(&state.grid.entities[oidx]);
    let power = 150 + 30 * trap.level as i32; // Lv1 180% .. Lv5 300%
    let amount = atk * power / 100;
    let victims: Vec<usize> = (0..state.grid.entities.len())
        .filter(|&j| {
            let e = &state.grid.entities[j];
            e.alive && matches!(e.team, Team::Monster) && e.pos.chebyshev(trap.cell) <= 1
        })
        .collect();
    for tgt in victims {
        if state.grid.entities[tgt].alive {
            apply_trap_damage(state, oidx, tgt, amount, true, "claymore_trap");
        }
    }
}

/// Freezing Trap: guaranteed FREEZE on every enemy in the 3×3 for `lvl+2` seconds
/// (no damage, no resist roll → no PRNG draw). Bosses are immune.
fn freezing_trigger(state: &mut GameState, trap: &Trap) {
    let ticks = (trap.level as u32 + 2) * 10; // Lv1 3 s .. Lv5 7 s
    let src = trap.owner_id;
    let victims: Vec<usize> = (0..state.grid.entities.len())
        .filter(|&j| {
            let e = &state.grid.entities[j];
            e.alive && matches!(e.team, Team::Monster) && !e.is_boss
                && e.pos.chebyshev(trap.cell) <= 1
        })
        .collect();
    for j in victims {
        skills::apply_buff(
            &mut state.grid.entities[j],
            Buff { kind: BuffKind::Freeze, magnitude: 1, ticks, source_id: src },
        );
    }
}

/// Ids of living monsters currently inside the 3×3 around `center`, in entity
/// (spawn) order — the edge-trigger occupancy set.
fn monster_ids_in(state: &GameState, center: Cell) -> Vec<u32> {
    state
        .grid
        .entities
        .iter()
        .filter(|e| e.alive && matches!(e.team, Team::Monster) && e.pos.chebyshev(center) <= 1)
        .map(|e| e.id)
        .collect()
}

/// First (lowest-index) living monster within the trap's 3×3.
fn first_victim_in(state: &GameState, center: Cell) -> Option<usize> {
    (0..state.grid.entities.len()).find(|&j| {
        let e = &state.grid.entities[j];
        e.alive && matches!(e.team, Team::Monster) && e.pos.chebyshev(center) <= 1
    })
}

/// Route a trap hit through the normal damage path (mitigation + recent_hits popup
/// + on-kill credit to the owner), like a non-attack AoE source (cf. Poison
/// Explosion). Neutral physical; `aoe` only changes the damage packet flag.
fn apply_trap_damage(state: &mut GameState, oidx: usize, tgt: usize, amount: i32, aoe: bool, skill_id: &str) {
    let dmg = combat::Damage {
        atk_level: 1, amount,
        element: Element::Physical,
        kind: DamageType::Physical,
        is_ranged: true,
        is_crit: false,
        is_aoe: aoe,
        mastery_bp: 0, shadow_bane_bp: 0, debuff_amp_bp: 0,
            atk_race: crate::grid::Race::Unknown,
            atk_size: crate::grid::Size::Medium,
            ignore_def_bp: 0,
    };
    let tgt_id = state.grid.entities[tgt].id;
    let owner_id = state.grid.entities[oidx].id;
    let mob = combat::attackers_on(&state.grid.entities, tgt);
    let pre_hp = state.grid.entities[tgt].hp.max(0);
    let dealt = combat::apply_damage(&state.grid.entities[tgt], &dmg, mob);
    state.grid.entities[tgt].take_hit(dealt); // hp − dmg, shatter Freeze, flag death
    state.recent_hits.push(HitEvent { target_id: tgt_id, attacker_id: owner_id, amount: dealt, hits: 1, kind: crate::state::HitKind::Normal });
    // DPS-meter attribution (effective damage = clamp overkill to the target's HP).
    state.record_damage(oidx, Some(skill_id), dealt.min(pre_hp));
    if !state.grid.entities[tgt].alive {
        kill_monster(state, tgt, Some(oidx));
    }
}
