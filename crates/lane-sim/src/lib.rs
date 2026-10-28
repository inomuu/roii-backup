//! LANE COMBAT TRIAL (2026-10-04): a Trickcal-style 3-lane battle, built BESIDE the live combat
//! in `sim-core` — nothing here is wired into `GameState`, saves or the client.
//!
//! The rules (prototype: `scratchpad/lane-formation/lane-formation.html`):
//! - The field is `WIDTH` × `LANES`. The party places up to `Rules::party_limit` units on a 3×3
//!   formation: back / mid / front columns at x = 4 / 6 / 8, gap columns between them.
//! - Range is Chebyshev, so attacks cross lanes and diagonals. Every unit, every tick: a target in
//!   range → attack; none, but one within reach of its lane band → step along x; none at all →
//!   step diagonally toward the nearest enemy.
//! - One party unit per cell; monsters stack up to `Rules::stack_cap` per cell and may pass
//!   through up to `PASS_THROUGH` cells full of monsters, never through a party unit.
//! - A normal monster may not enter a cell behind the frontmost party unit of a lane that has one
//!   (the gap rule); an empty lane can be walked around.
//! - Divers (mode C) ignore the gap rule and go for the rearmost party unit. Burrowers travel
//!   underground (untargetable) and surface next to the rearmost unit. Rear spawns enter from x = 0.
//! - Party units HOLD their slot: they attack what comes into range and never chase (a chasing
//!   melee unit dragged the whole formation forward and out of its lanes in the first runs). The
//!   formation advances as ONE block, a cell at a time, only while no party unit has a monster in
//!   range (`Battle::advance`).
//! - A teleporting party unit jumps behind the farthest monster; with no free cell the skill does
//!   not fire and costs no cooldown. When its target dies it walks back to its slot.
//!
//! Integer-only and deterministic: units act in id order, every tie breaks on
//! (distance, lane distance, x distance, id).

#![forbid(unsafe_code)]
#![deny(clippy::float_arithmetic)]

pub mod bench;
pub mod catalog;
pub mod skills;
pub mod stage;
pub mod ladder;

pub const WIDTH: i32 = 20;
pub const LANES: i32 = 3;
/// Monster-filled cells a monster may pass through in one step.
pub const PASS_THROUGH: i32 = 4;
/// Cells a diver may pass over (party units included) in one step.
pub const DIVER_PASS: i32 = 6;
pub const TELEPORT_INTERVAL: u32 = 80;
pub const TELEPORT_RETRY: u32 = 10;
/// Ticks a burrower waits underground at its destination before surfacing.
pub const BURROW_SURFACE_TICKS: u32 = 20;
/// Ticks between two steps of the whole formation.
pub const ADVANCE_INTERVAL: u32 = 3;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Team {
    Party,
    Monster,
}

/// How a normal monster picks what to stop for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TargetMode {
    /// The nearest party unit in range, diagonals included.
    A,
    /// Only party units in its own lane; an empty lane is walked to the far edge first.
    B,
    /// A for normal monsters; divers dive.
    C,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Col {
    Back,
    Mid,
    Front,
}

impl Col {
    pub const ALL: [Col; 3] = [Col::Back, Col::Mid, Col::Front];
    pub fn x(self) -> i32 {
        match self {
            Col::Back => 4,
            Col::Mid => 6,
            Col::Front => 8,
        }
    }
    pub fn tag(self) -> char {
        match self {
            Col::Back => 'B',
            Col::Mid => 'M',
            Col::Front => 'F',
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Stats {
    pub hp: i64,
    pub atk: i64,
    pub def: i64,
    pub range: i32,
    pub atk_interval: u32,
    pub move_interval: u32,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Traits {
    pub teleport: bool,
    pub diver: bool,
    pub burrow: bool,
    /// Enters from the left edge.
    pub rear: bool,
}

#[derive(Clone, Debug)]
pub struct Unit {
    pub id: u32,
    pub team: Team,
    pub name: String,
    pub stats: Stats,
    pub traits: Traits,
    pub x: i32,
    pub lane: i32,
    pub home: Option<(i32, i32)>,
    /// The formation slot this unit was placed on; `reset_formation` puts it back there.
    pub slot: Option<(i32, i32)>,
    pub hp: i64,
    pub alive: bool,
    atk_cd: u32,
    move_cd: u32,
    skill_cd: u32,
    /// Underground ticks (burrowers); 0 = on the surface.
    pub under: u32,
    dest: Option<(i32, i32)>,
    edge: bool,
    target: Option<u32>,
    tp_target: Option<u32>,
    returning: bool,
    /// The live `sim-core` entity behind this unit, when built from one: its blows go through
    /// the live damage formula (`bench::blow`).
    pub ent: Option<Box<sim_core::grid::Entity>>,
    /// The live skills whose behaviour depends on position (`skills::LANE_SKILLS`).
    pub kit: Vec<skills::LaneSkill>,
    /// Taunted onto this unit id until this tick (Frost Taunt, the Iron Chains lock): the monster
    /// goes for it and crosses the gap rule to reach it.
    pub taunted: Option<(u32, u64)>,
    /// Damage taken (party) / dealt.
    pub taken: i64,
    pub dealt: i64,
    /// Monsters attacking this unit this tick / the most at once so far.
    pub engaged: u32,
    pub max_engaged: u32,
}

impl Unit {
    fn new(id: u32, team: Team, name: &str, stats: Stats, traits: Traits, x: i32, lane: i32) -> Self {
        Unit {
            id, team, name: name.to_string(), hp: stats.hp, stats, traits, x, lane, home: None, slot: None, alive: true,
            atk_cd: 0, move_cd: 0, skill_cd: 0, under: 0, dest: None, edge: false, target: None, tp_target: None,
            returning: false, ent: None, kit: Vec::new(), taunted: None, taken: 0, dealt: 0, engaged: 0, max_engaged: 0,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Rules {
    pub mode: TargetMode,
    pub stack_cap: usize,
    pub party_limit: usize,
    /// Party units per cell. Above 1, a melee unit with nothing in range steps forward INSIDE the
    /// formation (never past its front slot), sharing a cell with an ally, until it reaches a
    /// monster; it walks back to its slot when the field clears or the formation moves on.
    pub party_stack: usize,
}

impl Default for Rules {
    fn default() -> Self {
        Rules { mode: TargetMode::C, stack_cap: 2, party_limit: 6, party_stack: 1 }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Event {
    Attack { from: u32, to: u32, dmg: i64 },
    Died { id: u32 },
    Teleport { id: u32, x: i32, lane: i32 },
    TeleportFizzled { id: u32 },
    Surfaced { id: u32, x: i32, lane: i32 },
    ReachedEdge { id: u32 },
    Cast { id: u32, skill: &'static str },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FormationError {
    PartyFull,
    SlotTaken,
    BadLane,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Outcome {
    Cleared,
    Wiped,
    Timeout,
}

pub struct Battle {
    pub rules: Rules,
    pub units: Vec<Unit>,
    queue: Vec<Unit>,
    pub tick: u64,
    next_id: u32,
    advance_cd: u32,
    pub fields: Vec<skills::Field>,
    /// Damage dealt per lane skill.
    pub skill_dmg: std::collections::BTreeMap<&'static str, i64>,
    pub events: Vec<(u64, Event)>,
}

fn cheb(ax: i32, al: i32, bx: i32, bl: i32) -> i32 {
    (ax - bx).abs().max((al - bl).abs())
}

impl Battle {
    pub fn new(rules: Rules) -> Self {
        Battle { rules, units: Vec::new(), queue: Vec::new(), tick: 0, next_id: 1, advance_cd: 0, fields: Vec::new(), skill_dmg: std::collections::BTreeMap::new(), events: Vec::new() }
    }

    /// Put a party unit on a formation slot.
    pub fn place(&mut self, name: &str, stats: Stats, traits: Traits, col: Col, lane: i32) -> Result<u32, FormationError> {
        if !(0..LANES).contains(&lane) {
            return Err(FormationError::BadLane);
        }
        let party = self.units.iter().filter(|u| u.team == Team::Party).count();
        if party >= self.rules.party_limit {
            return Err(FormationError::PartyFull);
        }
        if self.units.iter().any(|u| u.team == Team::Party && u.home == Some((col.x(), lane))) {
            return Err(FormationError::SlotTaken);
        }
        let id = self.next_id;
        self.next_id += 1;
        let mut u = Unit::new(id, Team::Party, name, stats, traits, col.x(), lane);
        u.home = Some((col.x(), lane));
        u.slot = u.home;
        u.skill_cd = if traits.teleport { 30 } else { 0 };
        self.units.push(u);
        Ok(id)
    }

    /// Place a live entity (its stats read through the combat module).
    pub fn place_entity(&mut self, name: &str, ent: sim_core::grid::Entity, traits: Traits, col: Col, lane: i32) -> Result<u32, FormationError> {
        let id = self.place(name, bench::stats_of(&ent), traits, col, lane)?;
        if let Some(u) = self.units.iter_mut().find(|u| u.id == id) {
            u.kit = skills::kit_of(&ent);
            u.ent = Some(Box::new(ent));
        }
        Ok(id)
    }

    pub fn spawn_entity(&mut self, name: &str, ent: sim_core::grid::Entity, traits: Traits, lane: i32) -> u32 {
        let id = self.spawn(name, bench::stats_of(&ent), traits, lane);
        if let Some(u) = self.queue.iter_mut().find(|u| u.id == id) {
            u.ent = Some(Box::new(ent));
        }
        id
    }

    /// Queue a monster; it enters at its edge of `lane` once the cell has room.
    pub fn spawn(&mut self, name: &str, stats: Stats, traits: Traits, lane: i32) -> u32 {
        let id = self.next_id;
        self.next_id += 1;
        let x = if traits.rear { 0 } else { WIDTH - 1 };
        let mut u = Unit::new(id, Team::Monster, name, stats, traits, x, lane.clamp(0, LANES - 1));
        if traits.burrow {
            u.under = 1;
        }
        self.queue.push(u);
        id
    }

    /// Every standing party unit back on its formation slot (the map scrolled on to the next fight).
    pub fn reset_formation(&mut self) {
        for u in self.units.iter_mut().filter(|u| u.team == Team::Party && u.alive) {
            if let Some((x, lane)) = u.slot {
                u.home = Some((x, lane));
                u.x = x;
                u.lane = lane;
            }
            u.returning = false;
            u.tp_target = None;
        }
        self.advance_cd = 0;
    }

    pub fn queued(&self) -> usize {
        self.queue.len()
    }

    fn live(&self, team: Team) -> impl Iterator<Item = usize> + '_ {
        self.units.iter().enumerate().filter(move |(_, u)| u.alive && u.team == team).map(|(i, _)| i)
    }

    /// Surface units standing on (x, lane), except `skip`.
    fn occupants(&self, x: i32, lane: i32, skip: Option<usize>) -> impl Iterator<Item = usize> + '_ {
        self.units.iter().enumerate()
            .filter(move |(i, u)| Some(*i) != skip && u.alive && u.under == 0 && u.x == x && u.lane == lane)
            .map(|(i, _)| i)
    }

    fn is_diver(&self, u: &Unit) -> bool {
        self.rules.mode == TargetMode::C && u.traits.diver
    }

    fn ignores_gap(&self, u: &Unit) -> bool {
        u.under > 0 || self.is_diver(u) || u.taunted.is_some_and(|(_, until)| self.tick < until)
    }

    /// The gap rule: behind the frontmost party unit of a lane that has one (as seen from the
    /// monster's side).
    fn gap_blocked(&self, rear: bool, x: i32, lane: i32) -> bool {
        let xs: Vec<i32> = self.live(Team::Party).map(|i| &self.units[i]).filter(|p| p.lane == lane).map(|p| p.x).collect();
        match (rear, xs.iter().min(), xs.iter().max()) {
            (true, Some(&min), _) => x > min,
            (false, _, Some(&max)) => x < max,
            _ => false,
        }
    }

    fn in_bounds(x: i32, lane: i32) -> bool {
        (0..WIDTH).contains(&x) && (0..LANES).contains(&lane)
    }

    fn enterable(&self, i: usize, x: i32, lane: i32) -> bool {
        if !Self::in_bounds(x, lane) {
            return false;
        }
        let u = &self.units[i];
        let here: Vec<usize> = self.occupants(x, lane, Some(i)).collect();
        if u.team == Team::Party {
            return !here.iter().any(|&o| self.units[o].team == Team::Monster) && here.len() < self.rules.party_stack.max(1);
        }
        if here.iter().any(|&o| self.units[o].team == Team::Party) || here.len() >= self.rules.stack_cap {
            return false;
        }
        self.ignores_gap(u) || !self.gap_blocked(u.traits.rear, x, lane)
    }

    fn passable(&self, i: usize, x: i32, lane: i32) -> bool {
        if !Self::in_bounds(x, lane) {
            return false;
        }
        let u = &self.units[i];
        if u.team == Team::Party {
            return false;
        }
        if self.is_diver(u) {
            return true;
        }
        if self.occupants(x, lane, Some(i)).any(|o| self.units[o].team == Team::Party) {
            return false;
        }
        self.ignores_gap(u) || !self.gap_blocked(u.traits.rear, x, lane)
    }

    /// Where a step of (dx, dy) lands, passing through up to `skip` cells (straight steps only).
    fn try_move(&self, i: usize, dx: i32, dy: i32, skip: i32) -> Option<(i32, i32)> {
        if dx == 0 && dy == 0 {
            return None;
        }
        let u = &self.units[i];
        for k in 1..=skip.max(1) {
            let (x, lane) = (u.x + dx * k, u.lane + dy * k);
            if self.enterable(i, x, lane) {
                return Some((x, lane));
            }
            if dy != 0 || !self.passable(i, x, lane) {
                return None;
            }
        }
        None
    }

    fn move_first(&mut self, i: usize, tries: &[(i32, i32, i32)]) -> bool {
        for &(dx, dy, skip) in tries {
            if let Some((x, lane)) = self.try_move(i, dx, dy, skip) {
                let u = &mut self.units[i];
                u.x = x;
                u.lane = lane;
                u.move_cd = u.stats.move_interval;
                return true;
            }
        }
        false
    }

    fn near_key(&self, from: usize, to: usize) -> (i32, i32, i32, u32) {
        let (a, b) = (&self.units[from], &self.units[to]);
        (cheb(a.x, a.lane, b.x, b.lane), (a.lane - b.lane).abs(), (a.x - b.x).abs(), b.id)
    }

    fn nearest(&self, from: usize, cands: impl Iterator<Item = usize>) -> Option<usize> {
        cands.min_by_key(|&j| self.near_key(from, j))
    }

    fn dist(&self, a: usize, b: usize) -> i32 {
        let (a, b) = (&self.units[a], &self.units[b]);
        cheb(a.x, a.lane, b.x, b.lane)
    }

    fn index_of(&self, id: u32) -> Option<usize> {
        self.units.iter().position(|u| u.id == id && u.alive)
    }

    /// The party unit deepest in the formation as seen from monster `m`'s side.
    fn rearmost(&self, m: usize) -> Option<usize> {
        let rear = self.units[m].traits.rear;
        self.live(Team::Party).min_by_key(|&i| {
            let p = &self.units[i];
            (if rear { -p.x } else { p.x }, p.lane, p.id)
        })
    }

    fn attack(&mut self, a: usize, t: usize) {
        if self.units[a].atk_cd > 0 {
            return;
        }
        self.units[a].atk_cd = self.units[a].stats.atk_interval;
        let victims = [t];
        let (atk, from) = (self.units[a].stats.atk, self.units[a].id);
        for v in victims {
            let dmg = match (&self.units[a].ent, &self.units[v].ent) {
                (Some(ae), Some(ve)) => bench::blow(ae, ve, self.units[v].engaged),
                _ => (atk - self.units[v].stats.def / 2).max(1),
            };
            self.units[a].dealt += dmg;
            let vu = &mut self.units[v];
            vu.hp -= dmg;
            if vu.team == Team::Party {
                vu.taken += dmg;
            }
            let to = vu.id;
            self.events.push((self.tick, Event::Attack { from, to, dmg }));
            if vu.hp <= 0 && vu.alive {
                vu.alive = false;
                self.events.push((self.tick, Event::Died { id: to }));
            }
        }
    }

    fn step_party(&mut self, i: usize) {
        let mons: Vec<usize> = self.live(Team::Monster).filter(|&m| self.units[m].under == 0).collect();
        if self.units[i].traits.teleport && self.units[i].skill_cd == 0 && !mons.is_empty() {
            let t = mons.iter().copied().min_by_key(|&m| {
                let u = &self.units[m];
                (u.traits.rear, -u.x, u.lane, u.id)
            });
            if let Some(t) = t {
                let (tx, tl, back) = (self.units[t].x, self.units[t].lane, if self.units[t].traits.rear { -1 } else { 1 });
                let spots = [(tx + back, tl), (tx + back, tl - 1), (tx + back, tl + 1), (tx, tl - 1), (tx, tl + 1)];
                let id = self.units[i].id;
                match spots.iter().copied().find(|&(x, l)| self.enterable(i, x, l)) {
                    Some((x, lane)) => {
                        let tid = self.units[t].id;
                        let u = &mut self.units[i];
                        u.x = x;
                        u.lane = lane;
                        u.tp_target = Some(tid);
                        u.returning = false;
                        u.skill_cd = TELEPORT_INTERVAL;
                        self.events.push((self.tick, Event::Teleport { id, x, lane }));
                    }
                    None => {
                        self.units[i].skill_cd = TELEPORT_RETRY;
                        self.events.push((self.tick, Event::TeleportFizzled { id }));
                    }
                }
            }
        }
        if !self.units[i].returning && self.try_cast(i) {
            return;
        }
        if let Some(tid) = self.units[i].tp_target {
            if self.index_of(tid).is_none() {
                self.units[i].tp_target = None;
                self.units[i].returning = true;
            }
        }
        let range = self.units[i].stats.range;
        let in_range: Vec<usize> = mons.iter().copied().filter(|&m| self.dist(i, m) <= range).collect();
        let preferred = self.units[i].tp_target.and_then(|tid| in_range.iter().copied().find(|&m| self.units[m].id == tid));
        let target = preferred.or_else(|| self.nearest(i, in_range.iter().copied()));
        let returning = self.units[i].returning;
        if let (Some(t), false) = (target, returning) {
            self.attack(i, t);
            return;
        }
        if self.units[i].move_cd > 0 {
            return;
        }
        let Some((hx, hl)) = self.units[i].home else { return };
        if returning || mons.is_empty() {
            let (x, lane) = (self.units[i].x, self.units[i].lane);
            if (x, lane) == (hx, hl) {
                self.units[i].returning = false;
                return;
            }
            let (dx, dy) = ((hx - x).signum(), (hl - lane).signum());
            self.move_first(i, &[(dx, dy, 1), (dx, 0, 1), (0, dy, 1)]);
            return;
        }
        if self.rules.party_stack > 1 && range <= 1 {
            let homes: Vec<i32> = self.live(Team::Party).filter_map(|p| self.units[p].home.map(|h| h.0)).collect();
            let (back, front) = (homes.iter().copied().min().unwrap_or(hx), homes.iter().copied().max().unwrap_or(hx));
            let lane = self.units[i].lane;
            if let Some(t) = self.nearest(i, mons.iter().copied().filter(|&m| (self.units[m].lane - lane).abs() <= 1)) {
                let dx = (self.units[t].x - self.units[i].x).signum();
                let nx = self.units[i].x + dx;
                if dx != 0 && (back..=front).contains(&nx) {
                    self.move_first(i, &[(dx, 0, 1)]);
                }
            }
        }
    }

    /// The formation advances as one block while NO party unit has a monster in range: a monster
    /// that out-ranges the whole party (a range-3 shooter standing just past the reach of the back
    /// row) would otherwise shoot a slot-holding party forever. Every slot moves one cell toward
    /// the monsters; a unit whose cell is blocked keeps its place this step but its slot moves.
    fn advance(&mut self) {
        if self.advance_cd > 0 {
            self.advance_cd -= 1;
            return;
        }
        let party: Vec<usize> = self.live(Team::Party).collect();
        let mons: Vec<usize> = self.live(Team::Monster).filter(|&m| self.units[m].under == 0).collect();
        if party.is_empty() || mons.is_empty() {
            return;
        }
        if party.iter().any(|&p| mons.iter().any(|&m| self.dist(p, m) <= self.units[p].stats.range)) {
            return;
        }
        let front = party.iter().map(|&p| self.units[p].x).max().unwrap_or(0);
        let back = party.iter().map(|&p| self.units[p].x).min().unwrap_or(0);
        let dir = if mons.iter().any(|&m| self.units[m].x > front) && front < WIDTH - 2 {
            1
        } else if mons.iter().any(|&m| self.units[m].x < back) && back > 1 {
            -1
        } else {
            return;
        };
        let mut order = party;
        order.sort_by_key(|&p| (-dir * self.units[p].x, self.units[p].id));
        for p in order {
            if let Some((hx, hl)) = self.units[p].home {
                self.units[p].home = Some((hx + dir, hl));
            }
            let (x, lane) = (self.units[p].x + dir, self.units[p].lane);
            if self.enterable(p, x, lane) {
                self.units[p].x = x;
            }
            if self.units[p].home != Some((self.units[p].x, self.units[p].lane)) {
                self.units[p].returning = true;
            }
        }
        self.advance_cd = ADVANCE_INTERVAL;
    }

    fn step_burrower(&mut self, m: usize) {
        if self.units[m].dest.is_none() {
            let Some(r) = self.rearmost(m) else { return };
            let (rx, rl) = (self.units[r].x, self.units[r].lane);
            let around = [(rx + 1, rl), (rx - 1, rl), (rx, rl - 1), (rx, rl + 1), (rx + 1, rl - 1), (rx + 1, rl + 1), (rx - 1, rl - 1), (rx - 1, rl + 1)];
            let dest = around.iter().copied().find(|&(x, l)| self.enterable(m, x, l)).unwrap_or((rx + 2, rl));
            self.units[m].dest = Some(dest);
        }
        if self.units[m].move_cd > 0 {
            return;
        }
        let Some((dx, dl)) = self.units[m].dest else { return };
        let u = &mut self.units[m];
        u.move_cd = 2;
        if (u.x, u.lane) != (dx, dl) {
            u.x += (dx - u.x).signum();
            u.lane += (dl - u.lane).signum();
            u.under += 1;
            return;
        }
        u.under += 1;
        if u.under > BURROW_SURFACE_TICKS {
            let id = u.id;
            let (x, lane) = (u.x, u.lane);
            if self.enterable(m, x, lane) {
                self.units[m].under = 0;
                self.events.push((self.tick, Event::Surfaced { id, x, lane }));
            } else {
                self.units[m].dest = None;
                self.units[m].under = 2;
            }
        }
    }

    fn step_monster(&mut self, m: usize) {
        if self.live(Team::Party).next().is_none() {
            return;
        }
        if self.units[m].under > 0 {
            self.step_burrower(m);
            return;
        }
        let range = self.units[m].stats.range;
        if let Some((tid, until)) = self.units[m].taunted {
            match self.index_of(tid) {
                Some(t) if self.tick < until => {
                    self.units[m].target = Some(tid);
                    if self.dist(m, t) <= range {
                        self.attack(m, t);
                    } else if self.units[m].move_cd == 0 {
                        let dx = (self.units[t].x - self.units[m].x).signum();
                        let dy = (self.units[t].lane - self.units[m].lane).signum();
                        // Around a party unit standing in the way: a side step, then on.
                        self.move_first(m, &[(dx, dy, PASS_THROUGH), (dx, 0, PASS_THROUGH), (0, dy, 1), (dx, 1, 1), (dx, -1, 1), (0, 1, 1), (0, -1, 1)]);
                    }
                    return;
                }
                _ => self.units[m].taunted = None,
            }
        }
        let party: Vec<usize> = self.live(Team::Party).collect();
        let mut in_range: Vec<usize> = party.iter().copied().filter(|&p| self.dist(m, p) <= range).collect();
        let rear = self.units[m].traits.rear;
        let fwd = if rear { 1 } else { -1 };
        if self.is_diver(&self.units[m]) {
            let Some(r) = self.rearmost(m) else { return };
            if self.dist(m, r) <= range {
                self.units[m].target = Some(self.units[r].id);
                self.attack(m, r);
                return;
            }
            if self.units[m].move_cd == 0 {
                let dx = (self.units[r].x - self.units[m].x).signum();
                let dy = (self.units[r].lane - self.units[m].lane).signum();
                if self.move_first(m, &[(dx, dy, DIVER_PASS), (dx, 0, DIVER_PASS), (0, dy, 1)]) {
                    self.units[m].target = None;
                    return;
                }
            }
            if let Some(t) = self.nearest(m, in_range.iter().copied()) {
                self.units[m].target = Some(self.units[t].id);
                self.attack(m, t);
            }
            return;
        }
        let lane = self.units[m].lane;
        let lane_first = self.rules.mode == TargetMode::B && !self.units[m].edge;
        if lane_first {
            in_range.retain(|&p| self.units[p].lane == lane);
        }
        if let Some(t) = self.nearest(m, in_range.iter().copied()) {
            self.units[m].target = Some(self.units[t].id);
            self.attack(m, t);
            return;
        }
        self.units[m].target = None;
        if self.units[m].move_cd > 0 {
            return;
        }
        if lane_first {
            let same = party.iter().any(|&p| self.units[p].lane == lane);
            let at_edge = if rear { self.units[m].x >= WIDTH - 1 } else { self.units[m].x <= 0 };
            if same || !at_edge {
                if !self.move_first(m, &[(fwd, 0, PASS_THROUGH)]) && !same {
                    self.units[m].edge = true;
                }
                return;
            }
            self.units[m].edge = true;
            self.events.push((self.tick, Event::ReachedEdge { id: self.units[m].id }));
        }
        let band = if range >= 2 { LANES } else { 1 };
        let reach = self.nearest(m, party.iter().copied().filter(|&p| (self.units[p].lane - lane).abs() <= band));
        if let Some(t) = reach {
            let sx = match (self.units[t].x - self.units[m].x).signum() { 0 => fwd, s => s };
            let ty = (self.units[t].lane - lane).signum();
            let tl = self.units[t].lane;
            let mut tries = vec![(sx, 0, PASS_THROUGH)];
            for dy in [ty, -ty, 1, -1] {
                if dy != 0 && (lane + dy - tl).abs() <= band {
                    tries.push((sx, dy, 1));
                }
            }
            self.move_first(m, &tries);
        } else if let Some(t) = self.nearest(m, party.iter().copied()) {
            let sx = (self.units[t].x - self.units[m].x).signum();
            let sy = (self.units[t].lane - lane).signum();
            self.move_first(m, &[(sx, sy, 1), (0, sy, 1), (sx, 0, PASS_THROUGH)]);
        }
    }

    /// One tick (100 ms).
    pub fn step(&mut self) {
        self.tick += 1;
        let mut k = 0;
        while k < self.queue.len() {
            let u = &self.queue[k];
            let room = u.under > 0 || {
                let here: Vec<usize> = self.occupants(u.x, u.lane, None).collect();
                !here.iter().any(|&o| self.units[o].team == Team::Party) && here.len() < self.rules.stack_cap
            };
            if room {
                let u = self.queue.remove(k);
                self.units.push(u);
            } else {
                k += 1;
            }
        }
        for u in self.units.iter_mut().filter(|u| u.alive) {
            u.atk_cd = u.atk_cd.saturating_sub(1);
            u.move_cd = u.move_cd.saturating_sub(1);
            u.skill_cd = u.skill_cd.saturating_sub(1);
            u.kit.iter_mut().for_each(|k| k.cd = k.cd.saturating_sub(1));
        }
        self.advance();
        let mut order: Vec<(u32, usize)> = self.units.iter().enumerate().filter(|(_, u)| u.alive).map(|(i, u)| (u.id, i)).collect();
        order.sort_unstable();
        for (_, i) in order {
            if !self.units[i].alive {
                continue;
            }
            match self.units[i].team {
                Team::Party => self.step_party(i),
                Team::Monster => self.step_monster(i),
            }
        }
        self.tick_fields();
        for u in self.units.iter_mut() {
            u.engaged = 0;
        }
        let hits: Vec<usize> = self.live(Team::Monster)
            .filter(|&m| self.units[m].under == 0)
            .filter_map(|m| {
                let t = self.index_of(self.units[m].target?)?;
                (self.dist(m, t) <= self.units[m].stats.range).then_some(t)
            })
            .collect();
        for t in hits {
            self.units[t].engaged += 1;
        }
        for u in self.units.iter_mut().filter(|u| u.team == Team::Party) {
            u.max_engaged = u.max_engaged.max(u.engaged);
        }
        self.units.retain(|u| u.alive || u.team == Team::Party);
    }

    pub fn outcome(&self) -> Option<Outcome> {
        if self.live(Team::Party).next().is_none() {
            Some(Outcome::Wiped)
        } else if self.queue.is_empty() && self.live(Team::Monster).next().is_none() {
            Some(Outcome::Cleared)
        } else {
            None
        }
    }

    pub fn run(&mut self, max_ticks: u64) -> Outcome {
        while self.tick < max_ticks {
            self.step();
            if let Some(o) = self.outcome() {
                return o;
            }
        }
        Outcome::Timeout
    }

    /// FNV-1a over everything that decides the next tick.
    pub fn state_hash(&self) -> u64 {
        let mut h: u64 = 0xcbf2_9ce4_8422_2325;
        let mut mix = |v: i64| {
            for b in v.to_le_bytes() {
                h ^= u64::from(b);
                h = h.wrapping_mul(0x0100_0000_01b3);
            }
        };
        mix(self.tick as i64);
        for u in self.units.iter().chain(self.queue.iter()) {
            for v in [u.id as i64, u.x as i64, u.lane as i64, u.hp, u.alive as i64, u.under as i64, u.atk_cd as i64, u.move_cd as i64, u.skill_cd as i64] {
                mix(v);
            }
        }
        h
    }

    /// Party units standing on one cell, and monsters per cell — the invariants the rules promise.
    pub fn cell_counts(&self) -> std::collections::BTreeMap<(i32, i32), (usize, usize)> {
        let mut m = std::collections::BTreeMap::new();
        for u in self.units.iter().filter(|u| u.alive && u.under == 0) {
            let e = m.entry((u.x, u.lane)).or_insert((0, 0));
            if u.team == Team::Party { e.0 += 1 } else { e.1 += 1 }
        }
        m
    }

    /// The grid as text: party units by initial, monster stacks by count, `~` underground.
    pub fn render(&self) -> String {
        let mut out = String::new();
        for lane in 0..LANES {
            for x in 0..WIDTH {
                let here: Vec<&Unit> = self.units.iter().filter(|u| u.alive && u.x == x && u.lane == lane).collect();
                let cell = if let Some(p) = here.iter().find(|u| u.team == Team::Party && u.under == 0) {
                    p.name.chars().next().map(|c| c.to_ascii_uppercase()).unwrap_or('P').to_string()
                } else {
                    let n = here.iter().filter(|u| u.team == Team::Monster && u.under == 0).count();
                    if n > 0 { n.to_string() } else if here.iter().any(|u| u.under > 0) { "~".into() } else { ".".into() }
                };
                out.push_str(&cell);
            }
            out.push('\n');
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn st(hp: i64, atk: i64, range: i32) -> Stats {
        Stats { hp, atk, def: 0, range, atk_interval: 10, move_interval: 4 }
    }
    fn tank() -> Stats { st(100_000, 5, 1) }
    fn mob() -> Stats { st(5_000, 5, 1) }

    fn front_wall(rules: Rules) -> Battle {
        let mut b = Battle::new(rules);
        for lane in 0..3 {
            b.place("tank", tank(), Traits::default(), Col::Front, lane).unwrap();
        }
        b.place("back", tank(), Traits::default(), Col::Back, 1).unwrap();
        b
    }

    fn assert_cells(b: &Battle) {
        for (cell, (p, m)) in b.cell_counts() {
            assert!(p <= b.rules.party_stack.max(1), "{p} party units on {cell:?}");
            assert!(p == 0 || m == 0, "party and monster share {cell:?}");
            assert!(m <= b.rules.stack_cap, "{m} monsters on {cell:?}");
        }
    }

    #[test]
    fn the_formation_refuses_a_taken_slot_and_a_full_party() {
        let mut b = Battle::new(Rules { party_limit: 2, ..Rules::default() });
        b.place("a", tank(), Traits::default(), Col::Front, 0).unwrap();
        assert_eq!(b.place("b", tank(), Traits::default(), Col::Front, 0), Err(FormationError::SlotTaken));
        b.place("b", tank(), Traits::default(), Col::Mid, 0).unwrap();
        assert_eq!(b.place("c", tank(), Traits::default(), Col::Back, 0), Err(FormationError::PartyFull));
        assert_eq!(b.place("c", tank(), Traits::default(), Col::Back, 3), Err(FormationError::BadLane));
    }

    #[test]
    fn the_same_battle_replays_bit_identically() {
        let run = || {
            let mut b = front_wall(Rules::default());
            for lane in 0..3 {
                for _ in 0..3 {
                    b.spawn("m", mob(), Traits::default(), lane);
                }
            }
            b.spawn("d", mob(), Traits { diver: true, ..Traits::default() }, 1);
            b.spawn("b", mob(), Traits { burrow: true, ..Traits::default() }, 2);
            (0..600).map(|_| { b.step(); b.state_hash() }).collect::<Vec<_>>()
        };
        assert_eq!(run(), run());
    }

    #[test]
    fn a_full_front_holds_nine_melee_out_of_the_gaps_and_caps_the_swarm() {
        let mut b = front_wall(Rules { stack_cap: 2, ..Rules::default() });
        for lane in 0..3 {
            for _ in 0..3 {
                b.spawn("m", mob(), Traits::default(), lane);
            }
        }
        for _ in 0..400 {
            b.step();
            assert_cells(&b);
            assert!(b.units.iter().filter(|u| u.team == Team::Monster && u.alive).all(|u| u.x > Col::Front.x()), "{}", b.render());
        }
        // Each monster prefers the tank in its own lane (lane distance breaks the tie), so a full
        // front splits the swarm: two per tank with two per cell.
        for u in b.units.iter().filter(|u| u.name == "tank") {
            assert_eq!(u.max_engaged, 2, "{}", u.name);
        }
    }

    #[test]
    fn an_empty_lane_lets_monsters_flank_the_formation() {
        let mut b = Battle::new(Rules::default());
        b.place("t0", tank(), Traits::default(), Col::Front, 0).unwrap();
        b.place("t1", tank(), Traits::default(), Col::Front, 1).unwrap();
        b.place("m1", tank(), Traits::default(), Col::Mid, 1).unwrap();
        b.place("b1", tank(), Traits::default(), Col::Back, 1).unwrap();
        for _ in 0..10 {
            b.spawn("m", mob(), Traits::default(), 2);
        }
        let mut flanked = false;
        for _ in 0..400 {
            b.step();
            assert_cells(&b);
            let front = b.units.iter().find(|u| u.name == "t1").map_or(0, |u| u.x);
            flanked |= b.units.iter().any(|u| u.team == Team::Monster && u.alive && u.lane == 2 && u.x < front);
        }
        assert!(flanked, "{}", b.render());
        // The tank beside the empty lane takes the side cells first (6 = three cells × 2); the
        // overflow walks on and reaches the mid row.
        let t1 = b.units.iter().find(|u| u.name == "t1").unwrap();
        assert!(t1.max_engaged >= 5, "the tank beside the empty lane: {}", t1.max_engaged);
        let mid = b.units.iter().find(|u| u.name == "m1").unwrap();
        assert!(mid.max_engaged > 0, "the mid row is reached from the side\n{}", b.render());
    }

    #[test]
    fn mode_b_walks_an_empty_lane_to_the_edge() {
        let mut b = Battle::new(Rules { mode: TargetMode::B, ..Rules::default() });
        b.place("t0", tank(), Traits::default(), Col::Front, 0).unwrap();
        b.spawn("m", mob(), Traits::default(), 2);
        b.run(300);
        assert!(b.events.iter().any(|(_, e)| matches!(e, Event::ReachedEdge { .. })));
    }

    #[test]
    fn a_diver_reaches_the_back_row_only_in_mode_c() {
        for (mode, reaches) in [(TargetMode::C, true), (TargetMode::A, false)] {
            let mut b = front_wall(Rules { mode, ..Rules::default() });
            b.spawn("d", mob(), Traits { diver: true, ..Traits::default() }, 1);
            let mut hit_back = false;
            for _ in 0..300 {
                b.step();
                assert_cells(&b);
                hit_back |= b.units.iter().any(|u| u.name == "back" && u.engaged > 0);
            }
            assert_eq!(hit_back, reaches, "{mode:?}\n{}", b.render());
        }
    }

    #[test]
    fn a_burrower_surfaces_beside_the_rearmost_unit_and_is_untouchable_below() {
        let mut b = front_wall(Rules::default());
        let id = b.spawn("b", mob(), Traits { burrow: true, ..Traits::default() }, 1);
        b.run(200);
        let surfaced = b.events.iter().find_map(|(_, e)| match e { Event::Surfaced { id: s, x, lane } if *s == id => Some((*x, *lane)), _ => None });
        let (x, lane) = surfaced.expect("surfaced");
        assert!(cheb(x, lane, Col::Back.x(), 1) <= 1, "beside the back row: ({x},{lane})");
        let surfaced_at = b.events.iter().find(|(_, e)| matches!(e, Event::Surfaced { id: s, .. } if *s == id)).map(|(t, _)| *t).unwrap();
        assert!(!b.events.iter().any(|(t, e)| *t < surfaced_at && matches!(e, Event::Attack { to, .. } if *to == id)), "untargetable underground");
    }

    #[test]
    fn a_teleport_lands_behind_the_farthest_monster_or_does_not_fire() {
        let mut b = Battle::new(Rules::default());
        b.place("assassin", st(10_000, 50, 1), Traits { teleport: true, ..Traits::default() }, Col::Mid, 1).unwrap();
        b.place("t", tank(), Traits::default(), Col::Front, 1).unwrap();
        let far = b.spawn("m", st(100_000, 1, 1), Traits::default(), 1);
        b.run(31);
        let (x, lane) = b.events.iter().find_map(|(_, e)| match e { Event::Teleport { x, lane, .. } => Some((*x, *lane)), _ => None }).expect("teleported");
        let m = b.units.iter().find(|u| u.id == far).unwrap();
        assert_eq!((x, lane), (m.x + 1, m.lane), "behind it");

        // Boxed in: every candidate cell is off the map or full, so the skill does not fire.
        let mut b = Battle::new(Rules { stack_cap: 1, ..Rules::default() });
        b.place("assassin", st(10_000, 50, 1), Traits { teleport: true, ..Traits::default() }, Col::Mid, 1).unwrap();
        // Range 20: they attack from the spawn edge and never move.
        for lane in 0..3 {
            b.spawn("wall", st(100_000, 1, 20), Traits::default(), lane);
        }
        b.run(31);
        assert!(b.events.iter().any(|(_, e)| matches!(e, Event::TeleportFizzled { .. })), "{}", b.render());
        let a = b.units.iter().find(|u| u.name == "assassin").unwrap();
        assert!(a.skill_cd <= TELEPORT_RETRY, "no full cooldown spent");
    }

    #[test]
    fn a_shooter_that_outranges_the_party_makes_the_formation_advance() {
        let mut b = Battle::new(Rules::default());
        b.place("tank", tank(), Traits::default(), Col::Front, 1).unwrap();
        b.spawn("shooter", st(50, 5, 3), Traits::default(), 1);
        assert_eq!(b.run(600), Outcome::Cleared, "{}", b.render());
        assert!(b.units[0].home.is_some_and(|(x, _)| x > Col::Front.x()), "the slot moved with the block");
    }

    #[test]
    fn with_party_stacking_a_back_melee_steps_up_to_fight_and_goes_home_after() {
        let run = |party_stack| {
            let mut b = Battle::new(Rules { party_stack, ..Rules::default() });
            b.place("tank", tank(), Traits::default(), Col::Front, 1).unwrap();
            b.place("back", st(100_000, 50, 1), Traits::default(), Col::Back, 1).unwrap();
            for _ in 0..3 {
                b.spawn("m", st(300, 5, 1), Traits::default(), 1);
            }
            let mut stacked = false;
            for _ in 0..5_000 {
                b.step();
                assert_cells(&b);
                let (t, bk) = (&b.units[0], &b.units[1]);
                stacked |= t.alive && bk.alive && (t.x, t.lane) == (bk.x, bk.lane);
                if b.outcome().is_some() && bk.home == Some((bk.x, bk.lane)) {
                    break;
                }
            }
            let bk = b.units.iter().find(|u| u.name == "back").unwrap();
            (bk.dealt, stacked, bk.home == Some((bk.x, bk.lane)))
        };
        assert_eq!(run(1).0, 0, "one per cell: the back melee never reaches");
        let (dealt, stacked, home) = run(2);
        assert!(dealt > 0 && stacked, "it shares the tank's cell and fights");
        assert!(home, "and walks back to its slot once the field is clear");
    }

    #[test]
    fn rear_spawns_hit_the_back_row_first() {
        let mut b = front_wall(Rules::default());
        b.spawn("r", mob(), Traits { rear: true, ..Traits::default() }, 1);
        let mut first = None;
        for _ in 0..200 {
            b.step();
            if first.is_none() {
                first = b.units.iter().find(|u| u.team == Team::Party && u.engaged > 0).map(|u| u.name.clone());
            }
        }
        assert_eq!(first.as_deref(), Some("back"));
    }
}
