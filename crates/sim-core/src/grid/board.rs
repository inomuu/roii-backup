//! `Grid` — the world board (Vec storage) + BFS pathing.

// DEBT (7 sites): this file predates `deny(clippy::indexing_slicing)`. An index that
// cannot be proven in range is a panic, and in verify-server (which links this crate and
// replays saves under `panic = "abort"`) a panic is the whole process. Shrink this, never
// add to it — a NEW file has no allow and is rejected on its first bare index.
#![allow(clippy::indexing_slicing)]

use std::collections::VecDeque;

use serde::{Deserialize, Serialize};

use super::{Cell, Entity, Team, BFS_NEIGHBOR_ORDER};

/// The world's grid (Vec storage, no HashMap — GDD §4.2 rule 4).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Grid {
    pub width: i32,
    pub height: i32,
    pub entities: Vec<Entity>,
}

impl Grid {
    pub fn new(width: i32, height: i32) -> Self {
        Self { width, height, entities: Vec::new() }
    }

    pub fn spawn(&mut self, e: Entity) {
        self.entities.push(e);
    }

    pub fn in_bounds(&self, c: Cell) -> bool {
        c.x >= 0 && c.x < self.width && c.y >= 0 && c.y < self.height
    }

    /// BETA-5 Phase 1.6: team-aware stacking, RO-style.
    ///
    /// Rules:
    ///   * Players are SOLID — a cell with an alive Player blocks
    ///     movement for everyone (no walking past your own party,
    ///     no monster Phasing through you).
    ///   * Monsters STACK — Monster-occupied cells are walkable for
    ///     other Monsters (mobs clump on the player), but they still
    ///     block Player movement (player has to attack the mob
    ///     instead of stepping into it).
    ///
    /// `mover_team` is the team of whoever is asking; spawn pickers
    /// pass `Team::Monster` (we're placing a new mob). Pre-0.0.14
    /// the function blocked any non-excluded entity, which made
    /// monsters queue up in a polite line behind the leader.
    pub fn is_occupied(&self, c: Cell, exclude_idx: usize, mover_team: Team) -> bool {
        for (i, e) in self.entities.iter().enumerate() {
            if i == exclude_idx || !e.alive || e.pos != c {
                continue;
            }
            // Players always block — same-team-or-not.
            if matches!(e.team, Team::Player) {
                return true;
            }
            // Monster cells block Players but not other Monsters.
            if matches!(mover_team, Team::Player) {
                return true;
            }
        }
        false
    }

    /// Find entity index by id. Linear scan — fine for Phase 0 sizes.
    /// Returns `None` if not found.
    pub fn idx_of_id(&self, id: u32) -> Option<usize> {
        self.entities.iter().position(|e| e.id == id)
    }

    /// The entity itself, by id. PREFER THIS over `idx_of_id` + indexing wherever the caller
    /// only needs the entity: it hands back a borrow, so there is no index to carry into the
    /// next statement, hold across a mutation, or store in state. A stored index is what took
    /// the server down in 0.0.294. Reach for `idx_of_id` only when the index is genuinely
    /// needed — to pass to another `&mut GameState` call, or to name a second entity.
    pub fn entity_of_id(&self, id: u32) -> Option<&Entity> {
        self.entities.iter().find(|e| e.id == id)
    }

    /// Mutable twin of `entity_of_id`.
    pub fn entity_of_id_mut(&mut self, id: u32) -> Option<&mut Entity> {
        self.entities.iter_mut().find(|e| e.id == id)
    }

    /// Attempt one step of entity `idx` toward its `target` cell using
    /// BFS shortest path (8-neighbor / Chebyshev).
    ///
    /// Why BFS (not greedy)?
    ///   Pre-ALPHA-8a we used greedy step on the strip arena (16×3)
    ///   where there's no geometry that traps greedy. ALPHA-8a moved
    ///   to 32×16 with several monsters at once — greedy started
    ///   producing visibly suboptimal zigzag paths around clusters.
    ///   BFS is the algorithm GDD §3.2 specified all along; we
    ///   delayed it because it wasn't needed for fun yet, and now
    ///   it is.
    ///
    /// Algorithm (deterministic, published):
    ///   1. BFS frontier expansion in 8 directions, expanded in the
    ///      fixed order `BFS_NEIGHBOR_ORDER` below (lower y first,
    ///      then lower x — matches the tie-break in `combat`).
    ///   2. Cells occupied by other alive entities are blocked. The
    ///      target cell itself is treated as walkable (so we can
    ///      ROUTE toward an enemy even though they stand on it).
    ///   3. Return the first cell on any shortest path. If we'd
    ///      step ONTO the target (only possible when target was
    ///      adjacent), refuse — the caller checks attack-range
    ///      before calling and shouldn't have invoked us.
    ///
    /// Returns true if the entity moved.
    pub fn step_toward_target(&mut self, idx: usize) -> bool {
        let (pos, target) = {
            let e = &self.entities[idx];
            (e.pos, e.target)
        };
        if pos == target {
            return false;
        }
        // Always step onto the BFS first cell. In combat the tick loop
        // checks `atk_range` BEFORE calling us, so we're never asked to
        // step ONTO an alive enemy — first step toward an adjacent enemy
        // means dist <= 1 = in-range = attack instead.
        match self.bfs_first_step(pos, target, idx) {
            Some(next) => {
                self.entities[idx].pos = next;
                true
            }
            None => false,
        }
    }

    /// Compute the first step of a shortest BFS path from `start` to
    /// `target`. Returns `None` if no path exists.
    ///
    /// Neighbor expansion order (FIXED — replay-critical):
    ///   delta (dx, dy) in this exact order:
    ///     (-1,-1), (0,-1), (1,-1),     // top row, left to right
    ///     (-1, 0),         (1, 0),     // middle row
    ///     (-1, 1), (0, 1), (1, 1),     // bottom row
    ///   This expands lower y first, then lower x within a row —
    ///   matching the tie-break order used everywhere else in sim.
    pub fn bfs_first_step(&self, start: Cell, target: Cell, ignore_idx: usize) -> Option<Cell> {
        if start == target {
            return None;
        }
        if !self.in_bounds(start) || !self.in_bounds(target) {
            return None;
        }
        let w = self.width;
        let h = self.height;
        let n = (w as usize).saturating_mul(h as usize);
        // (parent_x, parent_y), or sentinel (-1, -1) for "unvisited".
        let mut parent: Vec<(i32, i32)> = vec![(-1, -1); n];
        let cell_idx = |c: Cell| -> usize { (c.y * w + c.x) as usize };
        // Mark start as visited via a self-parent (any non-sentinel works).
        parent[cell_idx(start)] = (start.x, start.y);

        let neighbors: [(i32, i32); 8] = BFS_NEIGHBOR_ORDER;

        let mut queue: VecDeque<Cell> = VecDeque::new();
        queue.push_back(start);

        while let Some(curr) = queue.pop_front() {
            for (dx, dy) in neighbors.iter() {
                let nx = curr.x + dx;
                let ny = curr.y + dy;
                if nx < 0 || nx >= w || ny < 0 || ny >= h {
                    continue;
                }
                let ncell = Cell::new(nx, ny);
                let ni = cell_idx(ncell);
                if parent[ni] != (-1, -1) {
                    continue;
                }
                // Target cell is walkable for routing (we want to reach
                // it even if an enemy stands there). Everything else
                // checks occupancy of alive entities.
                let mover_team = self.entities[ignore_idx].team;
                if ncell != target && self.is_occupied(ncell, ignore_idx, mover_team) {
                    continue;
                }
                parent[ni] = (curr.x, curr.y);
                if ncell == target {
                    // Reconstruct: walk parent chain back to find the
                    // FIRST step taken from `start`.
                    let mut node = ncell;
                    loop {
                        let pp = parent[cell_idx(node)];
                        let prev = Cell::new(pp.0, pp.1);
                        if prev == start {
                            return Some(node);
                        }
                        node = prev;
                    }
                }
                queue.push_back(ncell);
            }
        }
        None
    }
}
