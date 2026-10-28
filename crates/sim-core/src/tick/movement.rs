//! Movement & positioning — surround-cell pick, step toward target,
//! teleport-behind. Split out of tick.rs (0.0.34 refactor).

// DEBT (18 sites): this file predates `deny(clippy::indexing_slicing)`. An index that
// cannot be proven in range is a panic, and in verify-server (which links this crate and
// replays saves under `panic = "abort"`) a panic is the whole process. Shrink this, never
// add to it — a NEW file has no allow and is rejected on its first bare index.
#![allow(clippy::indexing_slicing)]

use crate::grid::{Cell, Grid};
use crate::state::GameState;

/// Closest empty neighbour of `center` to the mover, using strict
/// occupancy (any alive entity blocks — including other monsters
/// who are already at the surround). Returns `None` if all 8
/// neighbours are taken or out of bounds; the caller stacks on
/// `center` in that case.
///
/// Tie-break: smallest Chebyshev distance to mover first, then
/// lower y, then lower x. Same ordering rule everywhere in sim
/// (see `BFS_NEIGHBOR_ORDER` and `combat::find_enemy_idx`).
pub(crate) fn pick_surround_cell(grid: &Grid, mover_idx: usize, center: Cell) -> Option<Cell> {
    let mover_pos = grid.entities[mover_idx].pos;
    let mut best: Option<(i32, i32, i32, Cell)> = None;
    for dy in -1..=1 {
        for dx in -1..=1 {
            if dx == 0 && dy == 0 {
                continue;
            }
            let c = Cell::new(center.x + dx, center.y + dy);
            if !grid.in_bounds(c) {
                continue;
            }
            // Strict: any alive entity (Player or Monster) at this
            // cell blocks. Stricter than `is_occupied` because we
            // don't want two monsters to both pick the same surround.
            let taken = grid.entities.iter().enumerate().any(|(j, e)| {
                j != mover_idx && e.alive && e.pos == c
            });
            if taken {
                continue;
            }
            let dist = c.chebyshev(mover_pos);
            let key = (dist, c.y, c.x);
            let better = best.map_or(true, |(d, y, x, _)| {
                key < (d, y, x)
            });
            if better {
                best = Some((dist, c.y, c.x, c));
            }
        }
    }
    best.map(|(_, _, _, c)| c)
}


/// Unit step (each axis clamped to -1/0/1) from `a` toward `b`.
pub(crate) fn unit_step(a: Cell, b: Cell) -> (i32, i32) {
    ((b.x - a.x).signum(), (b.y - a.y).signum())
}

/// True if no alive entity occupies `c` and it's in bounds.
pub(crate) fn cell_is_free(state: &GameState, c: Cell) -> bool {
    state.grid.in_bounds(c)
        && !state.grid.entities.iter().any(|e| e.alive && e.pos == c)
}

/// Blink `attacker_i` to the cell BEHIND `target_i` (opposite the side it
/// faces — a guaranteed backstab). A target "faces" whatever it's
/// attacking (`target_id`); with no such target it faces the attacker, so
/// behind = the far side. Falls back to a free neighbour, then the
/// target's own cell, if the ideal cell is blocked/out of bounds.
/// Teleport `mover_i` to a RANDOM free cell in the 5×5 (radius 2) around
/// `target_i` — a friendly blink (Subvenio drops the priest somewhere near the
/// hurt ally). One PRNG draw; falls back to the target's cell if all 24 are taken.
pub(crate) fn teleport_adjacent(state: &mut GameState, mover_i: usize, target_i: usize) {
    let tpos = state.grid.entities[target_i].pos;
    let mut free: Vec<Cell> = Vec::new();
    for dy in -2..=2 {
        for dx in -2..=2 {
            if dx == 0 && dy == 0 {
                continue;
            }
            let c = Cell::new(tpos.x + dx, tpos.y + dy);
            if !state.grid.in_bounds(c) {
                continue;
            }
            let taken = state
                .grid
                .entities
                .iter()
                .enumerate()
                .any(|(j, e)| j != mover_i && e.alive && e.pos == c);
            if !taken {
                free.push(c);
            }
        }
    }
    let dest = if free.is_empty() {
        tpos
    } else {
        free[state.rng.gen_range_u32(0, free.len() as u32) as usize]
    };
    let e = &mut state.grid.entities[mover_i];
    e.pos = dest;
    e.target = dest;
    e.move_acc = 0;
}

pub(crate) fn teleport_behind(state: &mut GameState, attacker_i: usize, target_i: usize) {
    let tpos = state.grid.entities[target_i].pos;
    let face_pos = state.grid.entities[target_i]
        .target_id
        .and_then(|id| state.grid.idx_of_id(id))
        .map(|fi| state.grid.entities[fi].pos)
        .unwrap_or(state.grid.entities[attacker_i].pos);
    let dir = unit_step(tpos, face_pos); // toward what the target faces
    let behind = Cell::new(tpos.x - dir.0, tpos.y - dir.1);
    let dest = if (dir != (0, 0)) && cell_is_free(state, behind) {
        behind
    } else {
        pick_surround_cell(&state.grid, attacker_i, tpos).unwrap_or(tpos)
    };
    let e = &mut state.grid.entities[attacker_i];
    e.pos = dest;
    e.target = dest;
    e.move_acc = 0;
}


pub(crate) fn move_step(state: &mut GameState, i: usize) {
    // Drain the attack accumulator while out of range. Without this,
    // a stale atk_acc would carry over to the next engagement and let
    // the player land an "instant" first swing on a new target — fine
    // for sim balance, but visually the swing animation skips its
    // windup. Clearing the accumulator each move tick keeps "new
    // target = full charge time" both mechanically and visually.
    state.grid.entities[i].atk_acc = 0;
    // MoveSpeedPct buffs (Agility Up) scale the accumulator increment; 0 for an
    // un-buffed entity ⇒ raw speed (goldens unchanged).
    let base = state.grid.entities[i].speed_milli;
    // Net move %: Agility-Up etc. (MoveSpeedPct) MINUS Quicksand's slow. 0 for an
    // un-buffed entity ⇒ raw speed (goldens unchanged).
    let pct = crate::skills::buff_sum(&state.grid.entities[i], crate::skills::BuffKind::MoveSpeedPct)
        - crate::skills::effective_bp(&state.grid.entities[i], crate::skills::BuffKind::AgiDexSlowPct)
        // Sweet Poison's mark slow (0.0.451), per source, whole percent; capped so a boss under
        // three Nemus still walks.
        - crate::skills::buff_sum(&state.grid.entities[i], crate::skills::BuffKind::MarkSlow).clamp(0, 70)
        // Gear-mod MOVE SPEED (shoes pool, cap 30%) — bp → whole-% units.
        + crate::combat::stat_mods::move_speed_bp(&state.grid.entities[i]) / 100;
    let speed = if pct == 0 {
        base
    } else {
        ((base as i64) * (100 + pct as i64).max(0) / 100) as u32
    };
    state.grid.entities[i].move_acc =
        state.grid.entities[i].move_acc.saturating_add(speed);
    while state.grid.entities[i].move_acc >= 1000 {
        state.grid.entities[i].move_acc -= 1000;
        state.grid.step_toward_target(i);
    }
}

