//! LANE MODE (user 2026-10-04, `docs/lane-combat-design.md`): a Trickcal-style 3-lane battle on the
//! LIVE combat AND the live stage flow. Every skill, buff, heal, break, Limit Break, Regalia and
//! Resonance effect is the live code's, and so is the stage frame: 3 rooms, each with its live kill
//! quota (delivered as groups, below) and room boss with minions, the 20% room-hop heal and the
//! Move-Speed march between rooms. Only WHERE things stand and move follows the lane rules.
//!
//! OFF unless `GameState::lane` is set. Nothing sets it in the game, the client or the server; a
//! headless runner (`sim-cli --lane`) opts in. With it `None` every hook in `tick` / `change_zone`
//! / the casts takes the old path untouched, the field is never serialized, and every golden keeps
//! its hash.
//!
//! The rules:
//! - Field 20 × 3 lanes. The party stands on a 3×3 formation (back / mid / front at x = 4 / 6 / 8,
//!   gap columns between) and HOLDS it: it never chases and never advances, so the monsters walk
//!   in and the clash lands mid-field. Nobody attacks before the crash (hold-fire), so no monster
//!   can out-range a holding party.
//! - A room's kill quota comes as `groups_per_room` (4) groups released at once from the lane
//!   edges (the live trickle is off), with a 2–3 s walk between groups; then the live room boss and
//!   its minions. `marching` tells the client when the party walks (scroll the scene).
//! - A group spawns as a 3×3 block (melee in front, shooters at the back) and walks in as ONE
//!   block, keeping its shape, until its front column stands next to the party's: the crash. After
//!   it, any unit with no target in range walks out to the nearest enemy, one cell per step, straight
//!   first; a monster shooter stops once a party unit is in its range. Monsters may share a cell
//!   after the crash; a monster may not step behind the frontmost party unit of a lane that holds
//!   one (the gap rule) unless it is taunted. Members off their slot teleport home when the march
//!   resumes.
//! - Iron Chains (a `pull` self-AoE) lands its targets beside the caster wherever she stands.
//!   Abyssal Embrace (a `field_pull` field) is centred on a fixed point (middle lane, the first
//!   cell in front of the formation) and hauls onto it, past the stacking cap.

use std::collections::BTreeMap;

use crate::data::GameData;
use crate::grid::{Cell, Team};
use crate::state::GameState;

pub const WIDTH: i32 = 20;
pub const LANES: i32 = 3;
pub const COL_BACK: i32 = 4;
pub const COL_MID: i32 = 6;
pub const COL_FRONT: i32 = 8;
/// Cells left of its slot a member enters a room from (the back row starts on the left edge).
pub const WALK_IN: i32 = COL_BACK;

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct LaneState {
    /// Monsters per cell where a cap still applies: the lane-edge placement of a boss's minions.
    /// Walking: before the crash no sharing; after it, unlimited (see `monster_dest`).
    pub stack_cap: u8,
    /// Formation slot per party entity id, as placed on entering a room.
    pub slots: BTreeMap<u32, (i32, i32)>,
    /// Where each slot stands (the slot itself: the formation does not advance; user 2026-10-05).
    pub homes: BTreeMap<u32, (i32, i32)>,
    /// Walking speed in lane mode, bp of the live speed (10000 = live). Default 50% (user
    /// 2026-10-05, after a 7-day read: same pacing, fewer wipes, Trickcal's slower walk).
    #[serde(default = "lane_speed")]
    pub speed_bp: u32,
    /// Where the live trickle's monsters enter after a room opens: `false` = the right edge of the
    /// lane (they walk in), `true` = the first free cell just right of the clash line. The room's
    /// opening monsters always walk in from the edge, with the party walking in from the left
    /// (user 2026-10-04: "wave แรกให้เดินเข้ามาหากัน"; which trickle reads better is under trial).
    #[serde(default)]
    pub spawn_near: bool,
    /// GROUPS (user 2026-10-05, "spawn เป็นกลุ่มใหญ่ แล้วปะทะทีเดียว -> เดินต่อนิดๆ"; 4 per room,
    /// user): the room's kill quota (`boss_threshold`) split into this many groups (~7–8 each, so a
    /// group fits the 3×3 release block one per cell), each released at once, with a short walk
    /// between them; then the live room boss. Replaces the live trickle.
    #[serde(default = "default_groups")]
    pub groups_per_room: u8,
    /// Groups still to come in this room.
    #[serde(default)]
    pub groups_left: u8,
    /// Tick the next group is released (0 = none scheduled: a group is on the field, or the boss).
    #[serde(default)]
    pub next_group_at: u64,
    /// The party is walking (nobody in it has a monster in range): the client scrolls the scene and
    /// plays the walk, so the party always reads as moving between clashes.
    #[serde(default)]
    pub marching: bool,
    /// RETIRED (always 0): the party's march now adds straight into each walker's own `move_acc`,
    /// so a walker has ONE progress counter and never leads its cell by more than one. Kept so a
    /// client reading it sees 0, not a missing key.
    #[serde(default)]
    pub march_acc: u32,
    /// The current group has CRASHED into the party (a monster within one cell of a member). Until
    /// then nobody attacks or casts, shooters included (user 2026-10-05: "ต้องให้เดินตรงกลางชนกัน
    /// ก่อนด้วยถึงตัว range จะโจมตีได้"). Reset when the field clears.
    #[serde(default)]
    pub clashed: bool,
    /// Party ids sent back to their slots THIS tick because the march resumed (a member that had
    /// left its slot — an ambusher behind the enemy line — teleports home). The client plays the
    /// teleport-back on these. Cleared every tick.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub returned: Vec<u32>,
    /// The player's own formation (the formation screen): slot per party entity id. It wins over the
    /// auto-formation on every room entry; a member not in it gets an auto slot.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub chosen: BTreeMap<u32, (i32, i32)>,
    /// The room's `lane_group_size` (`[min, max]` per group), when the zone sets one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub group_size: Option<[u8; 2]>,
    /// With `group_size`: monsters of the room's quota not yet released.
    #[serde(default)]
    pub to_release: u32,
}

/// The enemy block's columns at release: front, mid, back — one gap column between each, mirroring
/// the party's 4 / 6 / 8 (user 2026-10-06: "ให้ฝั่งศัตรูมีช่องว่างระหว่างกันด้วย").
pub const ENEMY_COLS: [i32; 3] = [WIDTH - 5, WIDTH - 3, WIDTH - 1];

fn default_groups() -> u8 {
    4
}

/// The walk between two groups: a distance worth 2–3 s at base walking speed.
pub const GROUP_WALK_SECS: (u32, u32) = (2, 3);
/// Base walking speed the walk between groups is measured at.
pub const BASE_SPEED_MILLI: i64 = 333;

pub const LANE_SPEED_BP: u32 = 5_000;

fn lane_speed() -> u32 {
    LANE_SPEED_BP
}

impl LaneState {
    pub fn new(stack_cap: u8) -> Self {
        LaneState { stack_cap: stack_cap.max(1), slots: BTreeMap::new(), homes: BTreeMap::new(), speed_bp: lane_speed(), spawn_near: false, groups_per_room: default_groups(), next_group_at: 0, marching: true, groups_left: 0, march_acc: 0, clashed: false, returned: Vec::new(), chosen: BTreeMap::new(), group_size: None, to_release: 0 }
    }
}

fn on(state: &GameState) -> bool {
    state.lane.is_some()
}

fn ent(state: &GameState, i: usize) -> Option<&crate::grid::Entity> {
    state.grid.entities.get(i)
}

/// Where a member stands: Sentinels and other melee in front (middle lane first), Supports in the
/// mid row, everything ranged at the back; overflow spills one row back.
fn auto_formation(state: &GameState) -> BTreeMap<u32, (i32, i32)> {
    use crate::potential::{doctrine_for_unit, Doctrine};
    let mut members: Vec<(u8, u32)> = state.grid.entities.iter()
        .filter(|e| matches!(e.team, Team::Player) && !e.is_pet)
        .map(|e| {
            let rank = match (doctrine_for_unit(&e.unit_def), e.atk_range <= 1) {
                (Some(Doctrine::Sentinel), _) => 0,
                (_, true) => 1,
                (Some(Doctrine::Support), _) => 2,
                _ => 3,
            };
            (rank, e.id)
        })
        .collect();
    members.sort_unstable();
    let mut used: Vec<(i32, i32)> = Vec::new();
    let mut out = BTreeMap::new();
    for (rank, id) in members {
        let cols: [i32; 3] = match rank {
            0 | 1 => [COL_FRONT, COL_MID, COL_BACK],
            2 => [COL_MID, COL_BACK, COL_FRONT],
            _ => [COL_BACK, COL_MID, COL_FRONT],
        };
        let slot = cols.iter().flat_map(|&c| [1, 0, 2].map(move |l| (c, l))).find(|s| !used.contains(s));
        if let Some(s) = slot {
            used.push(s);
            out.insert(id, s);
        }
    }
    out
}

/// The player's chosen slots laid over `auto`: a chosen member takes its slot, and an auto-placed
/// member whose slot was chosen by someone else moves to the first free one (back, mid, front;
/// middle lane first).
fn with_chosen(state: &GameState, auto: BTreeMap<u32, (i32, i32)>) -> BTreeMap<u32, (i32, i32)> {
    let Some(chosen) = state.lane.as_ref().map(|l| l.chosen.clone()) else { return auto };
    let mut out: BTreeMap<u32, (i32, i32)> = BTreeMap::new();
    for (id, s) in &chosen {
        if auto.contains_key(id) {
            out.insert(*id, *s);
        }
    }
    let all = [COL_FRONT, COL_MID, COL_BACK].iter().flat_map(|&c| [1, 0, 2].map(move |l| (c, l)));
    let mut free: Vec<(i32, i32)> = all.filter(|s| !out.values().any(|t| t == s)).collect();
    for (id, s) in auto {
        if out.contains_key(&id) {
            continue;
        }
        let pick = if free.contains(&s) { s } else { free.first().copied().unwrap_or(s) };
        free.retain(|t| *t != pick);
        out.insert(id, pick);
    }
    out
}

/// FORMATION SCREEN (dev slots for now): put party member `entity_id` on slot (`x`, `lane`) — x one
/// of the formation columns. A member already there swaps into the mover's old slot. The choice
/// persists across rooms; the members walk to their new slots. False on a bad id or slot.
pub fn set_slot(state: &mut GameState, entity_id: u32, x: i32, lane: i32) -> bool {
    if ![COL_BACK, COL_MID, COL_FRONT].contains(&x) || !(0..LANES).contains(&lane) {
        return false;
    }
    let is_member = state.grid.entities.iter().any(|e| e.id == entity_id && matches!(e.team, Team::Player) && !e.is_pet);
    let Some(l) = state.lane.as_mut() else { return false };
    if !is_member {
        return false;
    }
    let Some(&old) = l.slots.get(&entity_id) else { return false };
    let target = (x, lane);
    let other = l.slots.iter().find(|(id, s)| **id != entity_id && **s == target).map(|(id, _)| *id);
    if let Some(o) = other {
        l.slots.insert(o, old);
        l.homes.insert(o, old);
        l.chosen.insert(o, old);
    }
    l.slots.insert(entity_id, target);
    l.homes.insert(entity_id, target);
    l.chosen.insert(entity_id, target);
    true
}

/// Opt a state into lane mode (before the first tick, or on any zone).
pub fn enable(state: &mut GameState, data: &GameData, stack_cap: u8) -> Result<(), String> {
    state.lane = Some(LaneState::new(stack_cap));
    enter_zone(state, data)
}

/// Called at the end of every `change_zone` while lane mode is on: resize the field to 3 lanes,
/// stand the party on its formation and move the room's opening spawns to their lane's edge.
pub fn enter_zone(state: &mut GameState, data: &GameData) -> Result<(), String> {
    if !on(state) {
        return Ok(());
    }
    // A zone with a `lane_pool` (mixed melee / ranged groups) draws its groups from that instead.
    let group_size = state.current_zone_id.as_deref().and_then(|id| data.zone(id)).and_then(|z| z.lane_group_size);
    if let Some(zone) = state.current_zone_id.as_deref().and_then(|id| data.zone(id)) {
        if !zone.lane_pool.is_empty() {
            let pool = crate::data::build::pool_entries(&zone.lane_pool, zone, data)?;
            if let Some(zs) = state.zone_spawn.as_mut() {
                zs.pool = pool;
            }
        }
    }
    state.grid.width = WIDTH;
    state.grid.height = LANES;
    state.projectiles.clear();
    // The room's opening population is the live spawner's; lane mode releases its own groups.
    state.grid.entities.retain(|e| !matches!(e.team, Team::Monster));
    let slots = with_chosen(state, auto_formation(state));
    for e in state.grid.entities.iter_mut().filter(|e| matches!(e.team, Team::Player) && !e.is_pet) {
        if let Some(&(x, l)) = slots.get(&e.id) {
            // Every room opens with both sides walking in: the party from the left edge onto its
            // slots, the room's opening monsters from the right edge.
            let c = Cell::new(x - WALK_IN, l);
            e.pos = c;
            e.target = Cell::new(x, l);
            e.target_id = None;
            e.move_acc = 0;
        }
    }
    if let Some(l) = state.lane.as_mut() {
        l.homes = slots.clone();
        l.slots = slots;
        // The room's first group is released at once, while the party walks in from the left.
        l.next_group_at = state.tick;
        l.marching = true;
        l.groups_left = l.groups_per_room;
        l.group_size = group_size;
        l.to_release = 0;
    }
    if group_size.is_some() {
        let quota = state.zone_spawn.as_ref().map_or(0, |z| z.boss_threshold);
        if let Some(l) = state.lane.as_mut() {
            l.to_release = quota;
            l.groups_left = u8::from(quota > 0);
        }
    }
    Ok(())
}

/// Every non-boss monster with an id at or above `first_id` (what the live spawner just produced)
/// moves to the rightmost cell of its own lane that has room (the edge if every cell is full).
pub(crate) fn to_lane_edges(state: &mut GameState, first_id: u32) {
    let near = first_id > 0 && state.lane.as_ref().is_some_and(|l| l.spawn_near);
    let fresh: Vec<usize> = (0..state.grid.entities.len())
        .filter(|&i| ent(state, i).is_some_and(|e| e.alive && matches!(e.team, Team::Monster) && !e.is_boss && e.id >= first_id))
        .collect();
    for i in fresh {
        let Some(lane) = ent(state, i).map(|e| e.pos.y.rem_euclid(LANES)) else { continue };
        if let Some(e) = state.grid.entities.get_mut(i) {
            e.pos = Cell::new(-1, lane);
        }
        let front = state.lane.as_ref().and_then(|l| l.homes.values().map(|h| h.0).max()).unwrap_or(COL_FRONT);
        let free = |x: &i32| monsters_on(state, Cell::new(*x, lane)) < cap(state) && !party_on(state, Cell::new(*x, lane));
        let x = if near {
            (front + 2..WIDTH).find(free)
        } else {
            (COL_FRONT + 2..WIDTH).rev().find(free)
        }.unwrap_or(WIDTH - 1);
        if let Some(e) = state.grid.entities.get_mut(i) {
            e.pos = Cell::new(x, lane);
            e.target = e.pos;
            e.move_acc = 0;
        }
    }
}

/// `move_step`'s speed: base `speed_milli` scaled by the move-speed buffs, slows and gear mods.
fn speed_of(e: &crate::grid::Entity) -> u32 {
    let pct = crate::skills::buff_sum(e, crate::skills::BuffKind::MoveSpeedPct)
        - crate::skills::effective_bp(e, crate::skills::BuffKind::AgiDexSlowPct)
        - crate::skills::buff_sum(e, crate::skills::BuffKind::MarkSlow).clamp(0, 70)
        + crate::combat::stat_mods::move_speed_bp(e) / 100;
    if pct == 0 { e.speed_milli } else { u32::try_from(i64::from(e.speed_milli) * i64::from(100 + pct).max(0) / 100).unwrap_or(0) }
}

/// Once per tick, before anyone acts: a member who joined mid-room gets a slot, groups are paced,
/// the march closes the distance, and walk progress is settled for drawing.
pub fn upkeep(state: &mut GameState) {
    let Some(lane) = state.lane.clone() else { return };
    let missing: Vec<u32> = state.grid.entities.iter()
        .filter(|e| matches!(e.team, Team::Player) && !e.is_pet && !lane.slots.contains_key(&e.id))
        .map(|e| e.id).collect();
    if !missing.is_empty() {
        let fresh = auto_formation(state);
        if let Some(l) = state.lane.as_mut() {
            for id in missing {
                let taken: Vec<(i32, i32)> = l.slots.values().copied().collect();
                let slot = fresh.get(&id).copied().filter(|s| !taken.contains(s)).or_else(|| {
                    [COL_BACK, COL_MID, COL_FRONT].iter().flat_map(|&c| [1, 0, 2].map(move |r| (c, r))).find(|s| !taken.contains(s))
                });
                if let Some(s) = slot {
                    l.slots.insert(id, s);
                    l.homes.insert(id, s);
                }
            }
        }
    }
    groups(state);
    // A unit that starts fighting mid-step KEEPS its `move_acc` (the client's sub-cell offset):
    // zeroing it drew a shooter that stopped in range snapping back toward the edge. Only a walker
    // that reaches the party is set to 0 (`monster_move`), where it lands on its cell's centre.
}

/// The group schedule: release a group when its time comes, schedule the next one (after a short
/// walk) when the field is clear, and leave the boss to the live `try_spawn_boss` once the room's
/// quota is killed. Also stamps `marching`.
fn groups(state: &mut GameState) {
    let party: Vec<(Cell, i32)> = state.grid.entities.iter()
        .filter(|e| e.alive && matches!(e.team, Team::Player) && !e.is_pet)
        .map(|e| (e.pos, crate::combat::effective_atk_range(e))).collect();
    let mons: Vec<Cell> = state.grid.entities.iter()
        .filter(|e| e.alive && e.on_board() && matches!(e.team, Team::Monster)).map(|e| e.pos).collect();
    let was_clashed = state.lane.as_ref().is_some_and(|l| l.clashed);
    // THE CRASH: a monster reaches the party, or the walking block has arrived — its front column
    // stands next to the party's front column (user 2026-10-12: every monster at the cell set by
    // where it spawned).
    let touching = party.iter().any(|(p, _)| mons.iter().any(|m| p.chebyshev(*m) <= 1));
    let arrived = mons.iter().map(|m| m.x).min().is_some_and(|x| x <= block_stop(state));
    let clashed = !mons.is_empty() && (was_clashed || touching || arrived);
    let Some((quota, boss_up)) = state.zone_spawn.as_ref().map(|z| (z.boss_threshold, z.boss_spawned)) else { return };
    let (next_at, left) = match state.lane.as_ref() {
        Some(l) => (l.next_group_at, l.groups_left),
        None => return,
    };
    let mut next = next_at;
    let mut left_now = left;
    if !boss_up && left > 0 {
        if next == 0 && mons.is_empty() {
            let secs = GROUP_WALK_SECS.0 + state.rng.gen_range_u32(0, GROUP_WALK_SECS.1 - GROUP_WALK_SECS.0 + 1);
            next = state.tick + walk_ticks(state, secs).max(1);
        }
        if next > 0 && state.tick >= next {
            next = 0;
            match state.lane.as_ref().and_then(|l| l.group_size.map(|g| (g, l.to_release))) {
                // A drawn size per group until the quota is out (`lane_group_size`).
                Some(([lo, hi], rest)) => {
                    let size = (u32::from(lo) + state.rng.gen_range_u32(0, u32::from(hi - lo) + 1)).min(rest);
                    release_group(state, size);
                    let rest = rest - size;
                    if let Some(l) = state.lane.as_mut() {
                        l.to_release = rest;
                    }
                    left_now = u8::from(rest > 0);
                }
                None => {
                    let per_room = state.lane.as_ref().map_or(4, |l| u32::from(l.groups_per_room.max(1)));
                    release_group(state, quota.div_ceil(per_room).max(1));
                    left_now = left - 1;
                }
            }
        }
    } else if !boss_up && mons.is_empty() {
        // Every group is down: meet the quota so the live `try_spawn_boss` raises the room boss.
        if let Some(z) = state.zone_spawn.as_mut() {
            z.kill_counter = z.kill_counter.max(z.boss_threshold);
        }
    }
    if let Some(l) = state.lane.as_mut() {
        l.next_group_at = next;
        l.groups_left = left_now;
        l.clashed = clashed;
        // Walk until the crash, fight until the group is down, then walk on.
        l.marching = !clashed;
        l.returned.clear();
    }
    if !clashed {
        block_walk(state);
    }
    // The march resumes: anyone off its slot (an ambusher behind the enemy line) teleports home.
    if was_clashed && !clashed {
        let slots = state.lane.as_ref().map(|l| l.homes.clone()).unwrap_or_default();
        let mut back = Vec::new();
        for e in state.grid.entities.iter_mut().filter(|e| e.alive && matches!(e.team, Team::Player) && !e.is_pet) {
            if let Some(&(x, l)) = slots.get(&e.id) {
                let home = Cell::new(x, l);
                if e.pos != home {
                    e.pos = home;
                    e.target = home;
                    e.move_acc = 0;
                    back.push(e.id);
                }
            }
        }
        if let Some(l) = state.lane.as_mut() {
            l.returned = back;
        }
    }
}

/// The column the walking block's front stops on: next to the party's front column.
fn block_stop(state: &GameState) -> i32 {
    state.lane.as_ref().and_then(|l| l.homes.values().map(|h| h.0).max()).unwrap_or(COL_FRONT) + 1
}

/// BEFORE THE CRASH the group walks in as ONE BLOCK (user 2026-10-12): every monster keeps the cell
/// it spawned on relative to the others, and the block steps together at its slowest member's pace
/// (plus the party's march, as before) until its front column reaches `block_stop`. Nothing is
/// checked on the way: nothing should stand there, and if something does, it is stood on.
fn block_walk(state: &mut GameState) {
    let idx: Vec<usize> = (0..state.grid.entities.len())
        .filter(|&i| ent(state, i).is_some_and(|e| e.alive && e.on_board() && matches!(e.team, Team::Monster)))
        .collect();
    let Some(min_x) = idx.iter().filter_map(|&i| ent(state, i)).map(|e| e.pos.x).min() else { return };
    let stop = block_stop(state);
    let bp = state.lane.as_ref().map_or(10_000, |l| l.speed_bp);
    let slowest = idx.iter().filter_map(|&i| ent(state, i)).map(speed_of).min().unwrap_or(0);
    let pace = u32::try_from(u64::from(slowest) * u64::from(bp) / 10_000).unwrap_or(u32::MAX).saturating_add(party_pace(state));
    let mut acc = idx.iter().filter_map(|&i| ent(state, i)).map(|e| e.move_acc).max().unwrap_or(0);
    let mut shift = 0;
    if min_x > stop {
        acc = acc.saturating_add(pace);
        if acc >= 1000 {
            acc -= 1000;
            shift = 1;
        }
    }
    if min_x - shift <= stop {
        acc = 0; // arrived: every monster on its cell's centre
    }
    for i in idx {
        if let Some(e) = state.grid.entities.get_mut(i) {
            e.pos.x -= shift;
            e.target = e.pos;
            e.move_acc = acc.min(999);
            e.atk_acc = 0;
        }
    }
}

/// Where an ambusher lands to strike `target` from behind (for a future ambush/teleport skill,
/// user 2026-10-06): the gap cell right behind it in its lane, else the cell behind it in an adjacent
/// lane. It must be in bounds, empty of monsters and party. `None` = nowhere to land.
pub fn ambush_cell(state: &GameState, target: usize) -> Option<Cell> {
    let t = ent(state, target)?.pos;
    [0, -1, 1].iter().map(|dy| Cell::new(t.x + 1, t.y + dy))
        .find(|&c| in_bounds(c) && monsters_on(state, c) == 0 && !party_on(state, c))
}

/// `size` pool monsters (the live weighted pick) at once, as a block on the right edge.
fn release_group(state: &mut GameState, size: u32) {
    // ONE MONSTER PER CELL at release (user 2026-10-05: "spawn ช่องละ 1 ตัว ยกเว้น wave มีมากกว่า 9
    // ตัว"): the group lands as a 3×3 block on `ENEMY_COLS` (gap columns between, user 2026-10-06),
    // front column first, lanes from a random one. Only a group of more than 9 stacks: monster k
    // lands on cell k % 9.
    let first = state.rng.gen_range_u32(0, LANES as u32);
    let mut cells = Vec::with_capacity(9);
    for x in ENEMY_COLS {
        for l in 0..LANES as u32 {
            cells.push(Cell::new(x, i32::try_from((first + l) % LANES as u32).unwrap_or(0)));
        }
    }
    let first_new = state.grid.entities.len();
    for k in 0..size {
        let Some(&cell) = cells.get(usize::try_from(k).unwrap_or(0) % cells.len().max(1)) else { return };
        if !spawn_pool_monster(state, cell) {
            break;
        }
    }
    // MELEE IN FRONT, SHOOTERS AT THE BACK (user 2026-10-12): the draws above are unchanged; the
    // cells are then dealt melee from the front column, shooters from the back one.
    let fresh: Vec<usize> = (first_new..state.grid.entities.len()).collect();
    if fresh.len() > cells.len() {
        return;
    }
    let shooter = |s: &GameState, i: usize| ent(s, i).is_some_and(|e| crate::combat::effective_atk_range(e) >= 2);
    let melee: Vec<usize> = fresh.iter().copied().filter(|&i| !shooter(state, i)).collect();
    let shooters: Vec<usize> = fresh.iter().copied().filter(|&i| shooter(state, i)).collect();
    let deal: Vec<(usize, Cell)> = melee.iter().zip(cells.iter())
        .chain(shooters.iter().zip(cells.iter().rev()))
        .map(|(&i, &c)| (i, c)).collect();
    for (i, c) in deal {
        if let Some(e) = state.grid.entities.get_mut(i) {
            e.pos = c;
            e.target = c;
        }
    }
}

/// One weighted pool pick (the live `spawn_one_monster` rule: one draw) placed at `cell`.
fn spawn_pool_monster(state: &mut GameState, cell: Cell) -> bool {
    let total: u32 = match state.zone_spawn.as_ref() {
        Some(zs) if !zs.pool.is_empty() => zs.pool.iter().map(|e| e.weight).sum(),
        _ => return false,
    };
    if total == 0 {
        return false;
    }
    let r = state.rng.gen_range_u32(0, total);
    let template = {
        let Some(zs) = state.zone_spawn.as_ref() else { return false };
        let mut acc = 0u32;
        let mut pick = None;
        for e in zs.pool.iter() {
            acc = acc.saturating_add(e.weight);
            if r < acc {
                pick = Some(e.template.clone());
                break;
            }
        }
        match pick.or_else(|| zs.pool.last().map(|e| e.template.clone())) {
            Some(t) => t,
            None => return false,
        }
    };
    crate::spawn::build_and_spawn_monster(state, template, cell);
    true
}

/// The walk, in ticks, over a distance worth `secs` at `BASE_SPEED_MILLI`: the SLOWEST standing
/// member sets the pace, with the same Move Speed sources `move_step` reads.
fn walk_ticks(state: &GameState, secs: u32) -> u64 {
    let slowest = state.grid.entities.iter()
        .filter(|e| matches!(e.team, Team::Player) && e.alive && !e.is_pet)
        .map(|e| i64::from(speed_of(e)))
        .min().unwrap_or(BASE_SPEED_MILLI).max(1);
    u64::try_from(i64::from(secs) * 10 * BASE_SPEED_MILLI / slowest).unwrap_or(0)
}

fn cap(state: &GameState) -> usize {
    state.lane.as_ref().map_or(1, |l| usize::from(l.stack_cap))
}

fn monsters_on(state: &GameState, c: Cell) -> usize {
    state.grid.entities.iter().filter(|e| e.alive && e.on_board() && matches!(e.team, Team::Monster) && e.pos == c).count()
}

fn party_on(state: &GameState, c: Cell) -> bool {
    state.grid.entities.iter().any(|e| e.alive && e.on_board() && matches!(e.team, Team::Player) && e.pos == c)
}

fn home_of(state: &GameState, id: u32) -> Option<Cell> {
    state.lane.as_ref().and_then(|l| l.homes.get(&id)).map(|&(x, l)| Cell::new(x, l))
}

/// The gap rule: a cell behind the frontmost party unit of a lane that holds one.
fn gap_blocked(state: &GameState, c: Cell) -> bool {
    state.grid.entities.iter()
        .filter(|e| e.alive && e.on_board() && matches!(e.team, Team::Player) && !e.is_pet && e.pos.y == c.y)
        .map(|e| e.pos.x).max().is_some_and(|front| c.x < front)
}

fn in_bounds(c: Cell) -> bool {
    (0..WIDTH).contains(&c.x) && (0..LANES).contains(&c.y)
}

/// Where a monster's step of (dx, dy) lands, passing through up to `skip` monster-filled cells
/// (straight steps only).
fn monster_dest(state: &GameState, i: usize, dx: i32, dy: i32, ignore_gap: bool) -> Option<Cell> {
    let pos = ent(state, i)?.pos;
    if dx == 0 && dy == 0 {
        return None;
    }
    let c = Cell::new(pos.x + dx, pos.y + dy);
    if !in_bounds(c) || party_on(state, c) || (!ignore_gap && gap_blocked(state, c)) {
        return None;
    }
    // BEFORE THE CRASH (user 2026-10-06) a walker never shares a cell, and the enemy block keeps its
    // gaps: it enters only an empty cell whose next cell along its way is empty too. A release of
    // more than 9 is the one stack, made at spawn.
    if !clashed(state) {
        let ahead = Cell::new(c.x + if dx != 0 { dx } else { -1 }, c.y);
        let free = monsters_on(state, c) == 0 && (!in_bounds(ahead) || monsters_on(state, ahead) == 0);
        return free.then_some(c);
    }
    // AFTER THE CRASH any cell takes any number, so the ones behind walk through a stopped shooter
    // and close in together.
    Some(c)
}

/// The live `move_step` accumulator: true when a one-cell step is due this tick.
fn step_due(state: &mut GameState, i: usize, extra: u32) -> bool {
    let Some(e) = state.grid.entities.get(i) else { return false };
    let bp = state.lane.as_ref().map_or(10_000, |l| l.speed_bp);
    let speed = u32::try_from(u64::from(speed_of(e)) * u64::from(bp) / 10_000).unwrap_or(u32::MAX).saturating_add(extra);
    let Some(e) = state.grid.entities.get_mut(i) else { return false };
    e.atk_acc = 0;
    e.move_acc = e.move_acc.saturating_add(speed);
    e.move_acc >= 1000
}

/// After a due step: a step taken spends one cell of progress; a BLOCKED step holds the walker at
/// the edge of its cell (999) instead of wrapping to 0, so the client's sub-cell drawing
/// (`pos ∓ move_acc/1000`) waits at the edge rather than snapping back a cell.
fn settle_step(state: &mut GameState, i: usize, moved: bool) {
    if let Some(e) = state.grid.entities.get_mut(i) {
        e.move_acc = if moved { e.move_acc.saturating_sub(1000).min(999) } else { 999 };
    }
}

/// A monster out of range: the lane walk (replaces `pick_surround_cell` + `move_step`).
pub(crate) fn monster_move(state: &mut GameState, i: usize, enemy: Option<usize>) {
    let Some(me) = ent(state, i) else { return };
    let (pos, range) = (me.pos, crate::combat::effective_atk_range(me));
    let taunted = me.taunt_ticks > 0 && me.taunted_by.is_some();
    let Some(t) = enemy.and_then(|t| ent(state, t)).map(|t| t.pos) else { return };
    // Before the crash the block walks as one (`block_walk`).
    if !clashed(state) {
        return;
    }
    // Next to a party unit: it fights, it does not walk. Its progress stays as it is: a walker that
    // STEPPED here was set to 0 on that step (below); one the party walked up to keeps its offset
    // (zeroing it drew a rewind).
    if touching_party(state, pos) {
        return;
    }
    // A SHOOTER stops where it can shoot (user 2026-10-12: "ตัวยิงไกลให้ยืนแค่ระยะที่ยิง ไม่เดินเข้าไป
    // ถ้ายิงได้"), before the crash too; it keeps its sub-cell offset, so it draws where it stopped.
    if range >= 2 && party_in_reach(state, pos, range) {
        return;
    }
    // Both sides walk while the party marches: the party's pace adds to this walker's own, in ONE
    // counter, so the crash lands mid-field and the walker never leads its cell by more than one.
    let extra = if state.lane.as_ref().is_some_and(|l| l.marching) { party_pace(state) } else { 0 };
    if !step_due(state, i, extra) {
        return;
    }
    let sx = (t.x - pos.x).signum();
    let sy = (t.y - pos.y).signum();
    let band = if range >= 2 { LANES } else { 1 };
    let mut tries: Vec<(i32, i32, i32)> = Vec::new();
    // ONE CELL PER STEP, NO OVERTAKING (user 2026-10-05: "ทำไมมีการเดินแซง ไม่ควรมีนะ"): a walker
    // moves straight along its own lane; a full cell ahead is QUEUED behind (it waits at its cell's
    // edge), never skipped or gone around. A lane change happens only toward a target no lane in
    // reach holds (or the taunter), one diagonal cell at a time.
    if (t.y - pos.y).abs() <= band && sx != 0 {
        tries.push((sx, 0, 1));
    } else {
        tries.push((sx, sy, 1));
        tries.push((0, sy, 1));
    }
    if taunted && sy != 0 {
        tries.extend([(sx, sy, 1), (0, sy, 1)]);
    }
    for (dx, dy, _) in tries {
        if let Some(c) = monster_dest(state, i, dx, dy, taunted) {
            if let Some(e) = state.grid.entities.get_mut(i) {
                e.pos = c;
                e.target = c;
            }
            settle_step(state, i, true);
            // Arrived where it fights (the party's side, or a shooter's range): it stands on the
            // cell's centre. Never a rewind — it has just stepped onto this cell.
            if touching_party(state, c) || (range >= 2 && party_in_reach(state, c, range)) {
                if let Some(e) = state.grid.entities.get_mut(i) {
                    e.move_acc = 0;
                }
            }
            return;
        }
    }
    settle_step(state, i, false);
}

/// A party unit within `range` of `c`.
fn party_in_reach(state: &GameState, c: Cell, range: i32) -> bool {
    state.grid.entities.iter().any(|e| e.alive && e.on_board() && matches!(e.team, Team::Player) && !e.is_pet && e.pos.chebyshev(c) <= range)
}

/// A party unit within one cell of `c` (diagonals included): the crash distance.
fn touching_party(state: &GameState, c: Cell) -> bool {
    state.grid.entities.iter().any(|e| e.alive && e.on_board() && matches!(e.team, Team::Player) && !e.is_pet && e.pos.chebyshev(c) <= 1)
}

/// The party's walking pace in lane mode: the slowest standing member, lane speed applied.
fn party_pace(state: &GameState) -> u32 {
    let bp = state.lane.as_ref().map_or(10_000, |l| l.speed_bp);
    let pace = state.grid.entities.iter()
        .filter(|e| matches!(e.team, Team::Player) && e.alive && !e.is_pet)
        .map(speed_of).min().unwrap_or(0);
    u32::try_from(u64::from(pace) * u64::from(bp) / 10_000).unwrap_or(0)
}

/// A party member with nothing in range: before the crash it holds its slot (or walks back to it);
/// after the crash it walks out to the nearest monster (user 2026-10-12: "ทุกตัวสามารถเดินออกไปตีได้
/// ถ้าไม่มีเป้าหมายในระยะ แต่มีศัตรูเหลืออยู่"). It goes home when the march resumes (`groups`).
pub(crate) fn party_move(state: &mut GameState, i: usize) {
    let Some(me) = ent(state, i) else { return };
    let Some(home) = home_of(state, me.id) else { return };
    let pos = me.pos;
    if clashed(state) {
        let nearest = state.grid.entities.iter()
            .filter(|e| e.alive && e.on_board() && matches!(e.team, Team::Monster))
            .min_by_key(|e| e.pos.chebyshev(pos)).map(|e| e.pos);
        if let Some(t) = nearest {
            party_walk_out(state, i, t);
            return;
        }
    }
    if pos == home {
        if let Some(e) = state.grid.entities.get_mut(i) {
            e.target = home;
            e.atk_acc = 0;
            e.move_acc = 0;
        }
        return;
    }
    if !step_due(state, i, 0) {
        return;
    }
    let (dx, dy) = ((home.x - pos.x).signum(), (home.y - pos.y).signum());
    for (sx, sy) in [(dx, dy), (dx, 0), (0, dy)] {
        let c = Cell::new(pos.x + sx, pos.y + sy);
        if (sx, sy) != (0, 0) && in_bounds(c) && !party_on(state, c) && monsters_on(state, c) == 0 {
            settle_step(state, i, true);
            if let Some(e) = state.grid.entities.get_mut(i) {
                if c == home {
                    e.move_acc = 0; // arrived: stand on the slot's centre, no overshoot
                }
                e.pos = c;
                e.target = home;
            }
            return;
        }
    }
    settle_step(state, i, false);
}

/// One step toward `t` after the crash, with the monsters' walking rules: one cell, straight along
/// the lane while the target is within the lane band (diagonal otherwise), never onto a monster or
/// another party unit, never overtaking (a blocked step waits at the cell's edge).
fn party_walk_out(state: &mut GameState, i: usize, t: Cell) {
    let Some(me) = ent(state, i) else { return };
    let (pos, range) = (me.pos, crate::combat::effective_atk_range(me));
    if !step_due(state, i, 0) {
        return;
    }
    let (sx, sy) = ((t.x - pos.x).signum(), (t.y - pos.y).signum());
    let band = if range >= 2 { LANES } else { 1 };
    let tries: &[(i32, i32)] = if (t.y - pos.y).abs() <= band && sx != 0 { &[(sx, 0)] } else { &[(sx, sy), (0, sy)] };
    for &(dx, dy) in tries {
        let c = Cell::new(pos.x + dx, pos.y + dy);
        if (dx, dy) != (0, 0) && in_bounds(c) && !party_on(state, c) && monsters_on(state, c) == 0 {
            settle_step(state, i, true);
            if let Some(e) = state.grid.entities.get_mut(i) {
                e.pos = c;
                e.target = c;
            }
            return;
        }
    }
    settle_step(state, i, false);
}

/// Iron Chains in lanes: the victim lands beside the caster wherever she stands, past the gap rule
/// and the stacking cap (never onto a party unit).
pub(crate) fn pull_beside(state: &mut GameState, victim: usize, caster: usize) {
    let (Some(c), Some(v)) = (ent(state, caster).map(|e| e.pos), ent(state, victim).map(|e| e.pos)) else { return };
    let side = if v.x >= c.x { 1 } else { -1 };
    let spots = [(side, 0), (side, -1), (side, 1), (0, -1), (0, 1), (-side, 0), (-side, -1), (-side, 1)];
    for (dx, dy) in spots {
        let s = Cell::new(c.x + dx, c.y + dy);
        if in_bounds(s) && !party_on(state, s) {
            if let Some(e) = state.grid.entities.get_mut(victim) {
                if e.pos != s {
                    e.move_acc = 0; // a victim already there keeps its offset (no rewind)
                }
                e.pos = s;
                e.target = s;
            }
            return;
        }
    }
}

/// Abyssal Embrace's fixed point: the middle lane, the first cell in front of the formation.
pub(crate) fn abyss_point(state: &GameState) -> Cell {
    let front = state.lane.as_ref().and_then(|l| l.homes.values().map(|h| h.0).max()).unwrap_or(COL_FRONT);
    Cell::new((front + 1).min(WIDTH - 1), 1)
}

/// A `field_pull` in lanes: straight onto the centre, past the stacking cap.
pub(crate) fn pull_onto(state: &mut GameState, victim: usize, center: Cell) {
    if party_on(state, center) {
        return;
    }
    if let Some(e) = state.grid.entities.get_mut(victim) {
        if e.pos != center {
            e.move_acc = 0;
        }
        e.pos = center;
        e.target = center;
    }
}

/// Has the current group crashed into the party yet (lane hold-fire)?
pub(crate) fn clashed(state: &GameState) -> bool {
    state.lane.as_ref().map_or(true, |l| l.clashed)
}

/// The room boss waits for the last group (a kill count past the quota mid-room must not raise it).
pub(crate) fn boss_allowed(state: &GameState) -> bool {
    match state.lane.as_ref() {
        None => true,
        Some(l) => l.groups_left == 0 && !state.grid.entities.iter().any(|e| e.alive && matches!(e.team, Team::Monster)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn data() -> GameData {
        GameData::from_json_str(include_str!("../../../data/game_data.json")).expect("catalog")
    }

    #[test]
    fn lane_mode_runs_the_live_room_flow_on_three_lanes() {
        let d = data();
        let mut s = crate::data::new_game_as(42, &d, "zone_b5_1", "seraphine").unwrap();
        let id = s.grid.entities.iter().find(|e| matches!(e.team, Team::Player)).unwrap().id;
        crate::data::debug_add_levels(&mut s, id, 60);
        enable(&mut s, &d, 2).unwrap();
        assert_eq!((s.grid.width, s.grid.height), (WIDTH, LANES));
        let mut edge_spawns = 0;
        let mut seen = std::collections::BTreeSet::new();
        let mut biggest_release = 0;
        let mut drawn_x: std::collections::BTreeMap<u32, i64> = std::collections::BTreeMap::new();
        for _ in 0..30_000 {
            let before = seen.len();
            crate::tick::tick(&mut s, 1);
            for e in s.grid.entities.iter().filter(|e| e.alive && e.on_board()) {
                assert!((0..LANES).contains(&e.pos.y) && (0..WIDTH).contains(&e.pos.x), "{:?}", e.pos);
                if matches!(e.team, Team::Monster) && !e.is_boss && seen.insert(e.id) && e.pos.x > COL_FRONT + 1 {
                    edge_spawns += 1;
                }
            }
            biggest_release = biggest_release.max(seen.len() - before);
            // Before the crash walkers never share a cell (groups here are ≤ 9, so no release stack).
            if s.lane.as_ref().is_some_and(|l| !l.clashed) && !s.zone_spawn.as_ref().is_some_and(|z| z.boss_spawned) {
                let mut seen_cells = std::collections::BTreeMap::new();
                for e in s.grid.entities.iter().filter(|e| e.alive && e.on_board() && matches!(e.team, Team::Monster)) {
                    *seen_cells.entry((e.pos.x, e.pos.y)).or_insert(0) += 1;
                }
                for (&(x, y), &n) in &seen_cells {
                    assert!(n == 1, "{n} walkers share ({x},{y}) before the crash");
                }
            }
            // Before the crash the enemy block keeps a free column between monsters in a lane.
            let pre = s.lane.as_ref().is_some_and(|l| !l.clashed) && !s.zone_spawn.as_ref().is_some_and(|z| z.boss_spawned);
            if pre {
                let mons: Vec<Cell> = s.grid.entities.iter().filter(|e| e.alive && e.on_board() && matches!(e.team, Team::Monster)).map(|e| e.pos).collect();
                for (a, m) in mons.iter().enumerate() {
                    for n in mons.iter().skip(a + 1) {
                        assert!(m.y != n.y || (m.x - n.x).abs() >= 2, "walkers {m:?} {n:?} closed the gap before the crash");
                    }
                }
            }
            // The drawn x (`pos − move_acc/1000`, milli-cells) of a walker never moves back toward
            // the spawn edge, and before the crash never enters the party's front column (after it,
            // party members walk out to their targets).
            let front = s.grid.entities.iter().filter(|e| e.alive && matches!(e.team, Team::Player)).map(|e| e.pos.x).max().unwrap_or(0);
            for e in s.grid.entities.iter().filter(|e| e.alive && e.on_board() && matches!(e.team, Team::Monster)) {
                let drawn = i64::from(e.pos.x) * 1000 - i64::from(e.move_acc);
                if let Some(&prev) = drawn_x.get(&e.id) {
                    let back = drawn - prev;
                    assert!(back <= 0 || back >= 1000, "monster {} drew {} → {} (a rewind)", e.id, prev, drawn);
                }
                assert!(!pre || drawn >= i64::from(front + 1) * 1000 || e.pos.x <= front, "monster {} drawn at {} inside the party column {}", e.id, drawn, front);
                drawn_x.insert(e.id, drawn);
            }
            if s.pending_zone_change.is_some() {
                break;
            }
        }
        let quota = s.zone_spawn.as_ref().map_or(0, |z| z.boss_threshold) as usize;
        assert!(biggest_release >= quota / 4, "a room's quota comes in groups of a quarter: {biggest_release} at once (quota {quota})");
        assert!(edge_spawns > 10, "groups enter from the right of the lanes: {edge_spawns}");
        assert_eq!(s.pending_zone_change.as_deref(), Some("zone_b5_2"), "room boss down: on to room 2");
    }

    #[test]
    fn a_member_off_its_slot_teleports_home_when_the_march_resumes() {
        let d = data();
        let mut s = crate::data::new_game_as(42, &d, "zone_b5_1", "seraphine").unwrap();
        enable(&mut s, &d, 2).unwrap();
        let id = s.grid.entities.iter().find(|e| matches!(e.team, Team::Player)).unwrap().id;
        let home = s.lane.as_ref().unwrap().homes[&id];
        // Standing behind the enemy line when the last monster of a group falls.
        s.grid.entities.retain(|e| !matches!(e.team, Team::Monster));
        s.grid.entities.iter_mut().find(|e| e.id == id).unwrap().pos = Cell::new(16, 0);
        s.lane.as_mut().unwrap().clashed = true;
        upkeep(&mut s);
        let e = s.grid.entities.iter().find(|e| e.id == id).unwrap();
        assert_eq!((e.pos.x, e.pos.y), home, "back on its slot");
        assert_eq!(s.lane.as_ref().unwrap().returned, vec![id], "the client is told to play the teleport");
        upkeep(&mut s);
        assert!(s.lane.as_ref().unwrap().returned.is_empty(), "one tick only");
    }

    #[test]
    fn a_chosen_slot_swaps_and_survives_a_room_change() {
        let d = data();
        let mut s = crate::data::new_starter_party(42, &d, "zone_b5_1").unwrap();
        s.party_slots = crate::data::MAX_PARTY as u8;
        crate::data::model_add_member(&mut s, &d, "lapis").unwrap();
        enable(&mut s, &d, 2).unwrap();
        let slots = s.lane.as_ref().unwrap().slots.clone();
        let mut ids = slots.keys().copied();
        let (a, b) = (ids.next().unwrap(), ids.next().unwrap());
        let (sa, sb) = (slots[&a], slots[&b]);
        assert!(set_slot(&mut s, a, sb.0, sb.1), "move a onto b's slot");
        assert_eq!(s.lane.as_ref().unwrap().slots[&a], sb);
        assert_eq!(s.lane.as_ref().unwrap().slots[&b], sa, "b swapped into a's old slot");
        assert!(!set_slot(&mut s, a, 5, 1), "a gap column is not a slot");
        crate::data::change_zone(&mut s, &d, "zone_b5_2").unwrap();
        assert_eq!(s.lane.as_ref().unwrap().slots[&a], sb, "the choice persists across rooms");
        assert_eq!(s.lane.as_ref().unwrap().slots[&b], sa);
    }

    #[test]
    fn early_stages_release_melee_groups_of_three_to_five_until_the_quota_is_out() {
        let d = data();
        let mut s = crate::data::new_game_as(42, &d, "zone_b1_1", "suzu").unwrap();
        let id = s.grid.entities.iter().find(|e| matches!(e.team, Team::Player)).unwrap().id;
        crate::data::debug_add_levels(&mut s, id, 60);
        enable(&mut s, &d, 2).unwrap();
        assert_eq!(s.lane.as_ref().unwrap().group_size, Some([3, 5]));
        let quota = s.zone_spawn.as_ref().unwrap().boss_threshold;
        let mut seen = std::collections::BTreeSet::new();
        let mut sizes = Vec::new();
        while !s.zone_spawn.as_ref().unwrap().boss_spawned {
            crate::tick::tick(&mut s, 1);
            assert!(s.tick < 20_000, "the room reaches its boss");
            let fresh: Vec<_> = s.grid.entities.iter()
                .filter(|e| e.alive && matches!(e.team, Team::Monster) && !e.is_boss && seen.insert(e.id))
                .map(|e| crate::combat::effective_atk_range(e)).collect();
            if !fresh.is_empty() {
                assert!(fresh.iter().all(|&r| r <= 1), "stages 1-5 field melee only");
                sizes.push(fresh.len() as u32);
            }
        }
        assert_eq!(sizes.iter().sum::<u32>(), quota, "the groups add up to the quota: {sizes:?}");
        let last = sizes.len() - 1;
        assert!(sizes.iter().enumerate().all(|(k, &n)| (3..=5).contains(&n) || (k == last && n < 3)), "3-5 per group, the last one may be the rest: {sizes:?}");
    }

    #[test]
    fn lane_mode_off_serializes_nothing() {
        let d = data();
        let s = crate::data::new_game_as(42, &d, "zone_b1_1", "seraphine").unwrap();
        assert!(s.lane.is_none());
        let json = serde_json::to_string(&s).unwrap();
        assert!(!json.contains("\"lane\""), "an old save stays byte-identical");
    }

    #[test]
    fn a_group_walks_in_as_its_spawn_block_with_shooters_at_the_back() {
        let d = data();
        let mut s = crate::data::new_game_as(42, &d, "zone_b1_1", "seraphine").unwrap();
        let id = s.grid.entities.iter().find(|e| matches!(e.team, Team::Player)).unwrap().id;
        crate::data::debug_add_levels(&mut s, id, 60);
        enable(&mut s, &d, 2).unwrap();
        let mons = |s: &GameState| s.grid.entities.iter().filter(|e| e.alive && e.on_board() && matches!(e.team, Team::Monster)).map(|e| (e.id, e.pos, crate::combat::effective_atk_range(e) >= 2)).collect::<Vec<_>>();
        let mut spawned: BTreeMap<u32, Cell> = BTreeMap::new();
        let mut checked = 0;
        for _ in 0..20_000 {
            let was = clashed(&s);
            crate::tick::tick(&mut s, 1);
            for e in s.grid.entities.iter().filter(|e| e.alive && e.on_board() && matches!(e.team, Team::Monster)) {
                spawned.entry(e.id).or_insert(e.pos);
            }
            if !was && clashed(&s) && !s.zone_spawn.as_ref().is_some_and(|z| z.boss_spawned) {
                let now = mons(&s);
                let shift: Vec<i32> = now.iter().map(|(id, p, _)| spawned[id].x - p.x).collect();
                assert!(shift.windows(2).all(|w| w[0] == w[1]), "the block kept its shape: {shift:?}");
                assert!(now.iter().all(|(id, p, _)| spawned[id].y == p.y), "nobody changed lane on the walk");
                assert_eq!(now.iter().map(|(_, p, _)| p.x).min(), Some(block_stop(&s)), "the front column stops next to the party");
                let back_melee = now.iter().filter(|m| !m.2).map(|m| m.1.x).max();
                let front_shooter = now.iter().filter(|m| m.2).map(|m| m.1.x).min();
                if let (Some(m), Some(f)) = (back_melee, front_shooter) {
                    assert!(m <= f, "melee in front of the shooters: {now:?}");
                }
                checked += 1;
            }
            if s.pending_zone_change.is_some() {
                break;
            }
        }
        assert!(checked >= 2, "several groups crashed ({checked})");
    }

    #[test]
    fn after_the_crash_a_member_with_nothing_in_reach_walks_out_to_the_nearest_enemy() {
        let d = data();
        let mut s = crate::data::new_game_as(42, &d, "zone_b6_1", "suzu").unwrap();
        let id = s.grid.entities.iter().find(|e| matches!(e.team, Team::Player)).unwrap().id;
        enable(&mut s, &d, 2).unwrap();
        while !clashed(&s) {
            crate::tick::tick(&mut s, 1);
            assert!(s.tick < 3_000, "the first group crashes");
        }
        // Keep only the farthest monster: nothing is in her reach any more.
        let far = s.grid.entities.iter().filter(|e| e.alive && matches!(e.team, Team::Monster)).max_by_key(|e| e.pos.x).unwrap().id;
        for e in s.grid.entities.iter_mut().filter(|e| matches!(e.team, Team::Monster) && e.id != far) {
            e.alive = false;
        }
        let gap = |s: &GameState| {
            let me = s.grid.entities.iter().find(|e| e.id == id).unwrap().pos;
            s.grid.entities.iter().find(|e| e.id == far).unwrap().pos.chebyshev(me)
        };
        let start = gap(&s);
        assert!(start > 1, "the survivor stands out of her reach ({start})");
        let home = home_of(&s, id).unwrap();
        let (mut left_slot, mut closest) = (false, start);
        for _ in 0..200 {
            crate::tick::tick(&mut s, 1);
            if !s.grid.entities.iter().any(|e| e.id == far && e.alive) {
                break;
            }
            left_slot |= s.grid.entities.iter().any(|e| e.id == id && e.pos != home);
            closest = closest.min(gap(&s));
        }
        assert!(left_slot, "she left her slot");
        assert!(closest < start, "and walked toward the enemy ({start} -> {closest})");
    }

    #[test]
    fn a_lane_pool_feeds_lane_groups_and_never_the_live_field() {
        let d = data();
        let ids = |s: &GameState| s.zone_spawn.as_ref().map(|z| z.pool.iter().map(|e| e.template.def_id.clone()).collect::<Vec<_>>()).unwrap_or_default();
        let mut s = crate::data::new_game_as(42, &d, "zone_b6_1", "seraphine").unwrap();
        assert!(!ids(&s).iter().any(|id| id.starts_with("ph_ranged_")), "the live field draws the spawn_pool");
        enable(&mut s, &d, 2).unwrap();
        let pool = ids(&s);
        assert!(pool.iter().any(|id| id == "ph_ranged_baby_pinkling"), "lane groups draw the lane_pool: {pool:?}");
        let ranged = s.zone_spawn.as_ref().and_then(|z| z.pool.iter().find(|e| e.template.def_id == "ph_ranged_baby_pinkling")).map(|e| e.template.atk_range);
        assert_eq!(ranged, Some(5));
    }
}
