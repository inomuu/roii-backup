//! Monster spawning — used both for initial zone fill (called from
//! `data::populate_zone`) and for continuous random spawn during the
//! tick loop (called from `tick.rs`).
//!
//! PRNG draw contract:
//!   `spawn_one_monster()` takes EXACTLY 2 PRNG draws per successful
//!   spawn — one for the weighted pool pick, one for the random cell.
//!   (Plus extra cell draws on retry, but each is a single u32.) When
//!   no spawn can happen (no zone, pool empty, no valid cell), zero
//!   draws are consumed. Reordering or adding draws here reorders
//!   every drop/refine event in every save — treat like §7.2.

// DEBT (6 sites): this file predates `deny(clippy::indexing_slicing)`. An index that
// cannot be proven in range is a panic, and in verify-server (which links this crate and
// replays saves under `panic = "abort"`) a panic is the whole process. Shrink this, never
// add to it — a NEW file has no allow and is rejected on its first bare index.
#![allow(clippy::indexing_slicing)]

use crate::grid::{Cell, Entity, Team};
use crate::state::GameState;

/// Minimum Chebyshev distance from the player at which monsters are
/// allowed to spawn. Phase 0 keeps this a constant; in BETA it could
/// move to ZoneDef so a "swarm" zone can let monsters spawn on top.
const MIN_PLAYER_DISTANCE: i32 = 4;

/// Max attempts for the random-cell rejection loop. With grids in
/// the 32x16+ range and small populations this almost always
/// succeeds on the first try; we cap it so the sim never spins on a
/// pathological full-board state.
const MAX_CELL_RETRIES: u32 = 24;

/// Hard ceiling on ALIVE monsters in a room. The crowd builds toward this as the
/// player kills; `ZoneDef.target_population` is now the FLOOR (minimum), not the
/// steady target. Global for now; could move to ZoneDef for per-band swarms.
pub const MONSTER_CAP: u32 = 20;
/// Trickle cadence between the floor and the cap: one new monster roughly this
/// often (5 s at 10 ticks/s) so the room fills SLOWLY — the field thins out under
/// the party's DPS rather than being a constant XP fountain. Below the floor the
/// cadence is bypassed (refill is immediate).
pub const MONSTER_TRICKLE_TICKS: u64 = 50;

/// Attempt to spawn one monster from the active zone's pool at a
/// random valid cell. Returns true on success.
///
/// Failure modes (silent — caller should not panic):
///   - No zone loaded (`zone_spawn = None`)
///   - Pool empty or total weight is 0
///   - No valid cell within `MAX_CELL_RETRIES`
pub fn spawn_one_monster(state: &mut GameState, allow_lure: bool) -> bool {
    // Snapshot pool weights without borrowing across rng draws.
    let total_weight: u32 = match state.zone_spawn.as_ref() {
        Some(zs) if !zs.pool.is_empty() => zs.pool.iter().map(|e| e.weight).sum(),
        _ => return false,
    };
    if total_weight == 0 {
        return false;
    }

    // Draw 1: pick pool entry by weight (Lemire-style via `gen_range`).
    let r = state.rng.gen_range_u32(0, total_weight);
    let entry_idx = pick_weighted_index(state.zone_spawn.as_ref().unwrap(), r);

    // First living player drives the safe-distance check.
    let first_player_pos = state
        .grid
        .entities
        .iter()
        .find(|e| matches!(e.team, Team::Player) && e.alive)
        .map(|e| e.pos);
    // Monster Lure (TRICKLE spawns only — the initial room wave fills normally so
    // it isn't crammed onto the player): if a living player carries the lure buff,
    // monsters spawn within `radius` cells of THAT player, measured from them.
    let (player_pos, max_radius) = if allow_lure {
        let lure = state
            .grid
            .entities
            .iter()
            .filter(|e| matches!(e.team, Team::Player) && e.alive)
            .find_map(|e| {
                // TIGHTEST, not the sum. `SpawnLure`'s magnitude is a spawn RADIUS in cells, and
                // `buff_sum` ADDS across sources — so two lures widened the ring instead of
                // keeping the closer one, and stacking them made spawns land FURTHER away, the
                // opposite of what the item does. A radius is not a bonus; it does not add.
                let r = e
                    .buffs
                    .iter()
                    .filter(|b| b.kind == crate::skills::BuffKind::SpawnLure)
                    .map(|b| b.magnitude)
                    .min()
                    .unwrap_or(0);
                if r > 0 { Some((e.pos, r)) } else { None }
            });
        match lure {
            Some((pos, r)) => (Some(pos), Some(r)),
            None => (first_player_pos, None),
        }
    } else {
        (first_player_pos, None)
    };

    // Draw 2+: random cell with rejection (each retry = 2 u32 draws).
    let Some(cell) = pick_random_cell(state, player_pos, max_radius) else {
        return false;
    };

    // Clone the template now (small — a handful of DropEntry rows).
    let template = state.zone_spawn.as_ref().unwrap().pool[entry_idx].template.clone();
    build_and_spawn_monster(state, template, cell);
    true
}

/// Build + spawn one monster from `template` at `cell`. PRNG-FREE (the caller
/// already drew pool + cell) — shared by `spawn_one_monster`, the boss's minion
/// ring AND the boss itself (`try_spawn_boss`) so they produce identical
/// entities. Returns the spawned entity's grid index (bosses flag it after).
pub(crate) fn build_and_spawn_monster(
    state: &mut GameState,
    template: crate::state::MonsterTemplate,
    cell: Cell,
) -> usize {
    let id = state.next_monster_id;
    state.next_monster_id = state.next_monster_id.wrapping_add(1);
    let mut e = Entity::idle(id, cell)
        .with_team(Team::Monster)
        .with_walk(cell, template.speed_milli)
        .with_combat(
            template.hp, template.atk, template.def,
            template.atk_speed_milli, template.element,
        )
        .with_drops(template.drops);
    e.atk_range = template.atk_range;
    e.proj_travel_ticks = template.proj_travel_ticks;
    e.xp_reward = template.xp_reward;
    e.gil_reward = template.gil_reward;
    e.toughness_max = template.toughness_max;
    e.toughness = template.toughness_max;
    e.gear_grade_odds = template.gear_grade_odds.clone();
    e.weaknesses = template.weaknesses.clone();
    e.resists = template.resists.clone();
    e.mob_atk_min = template.atk_min;
    e.mob_atk_max = template.atk_max;
    e.mob_stage = template.stage;
    // Instantiate the monster's catalog skills at level 1 (on-death blasts,
    // future abilities). Unknown ids are dropped (replay-safe, like a stale save).
    e.skills = template
        .skills
        .iter()
        .filter_map(|id| {
            crate::skills::skill_def(id).map(|_| crate::skills::SkillState::new(id, 1))
        })
        .collect();
    // Active-kind monster skills go into the loadout, so the SAME active-cast pipeline
    // that fires player skills (`pick_active_skill` → `attack_step`) fires them for
    // monsters too. On-death / passive blasts stay OUT — they trigger via the death
    // path (`schedule_explosion`), not the loadout.
    //
    // DESYNC: stagger each active's FIRST cast by a per-monster offset from the entity
    // id (a multiplicative hash — NO PRNG draw, so the loot/refine RNG stream stays
    // byte-identical). A pack spawned on the same tick then fires out of sync, not in
    // lockstep. (Whether a mob uses a skill 0/1/2 times is a later per-skill charge cap;
    // most mobs realistically cast once anyway on a long CD.)
    let h = id.wrapping_mul(2_654_435_761);
    for st in e.skills.iter_mut() {
        let Some(def) = crate::skills::skill_def(&st.id) else {
            continue;
        };
        if !matches!(def.kind, crate::skills::SkillKind::Active) {
            continue;
        }
        // Per-skill spawn gate: a common mob only SOMETIMES carries the active (organic
        // variety — some cast it, some never); a signature/elite (chance 0) always does.
        if def.spawn_use_chance_bp > 0 && h % 10_000 >= def.spawn_use_chance_bp {
            continue;
        }
        e.active_skills.push(st.id.clone());
        if def.once_per_spawn {
            // Fire ONCE, 1-3 s after spawn (10-30 ticks) — a hash-derived delay so it
            // varies per mob WITHOUT a PRNG draw (loot/refine stream stays byte-identical).
            // The huge cooldown_ticks then blocks any recast. Twinling's vanish-and-stalk.
            st.cooldown = 10 + h % 21;
        } else if def.cooldown_ticks > 0 {
            st.cooldown = h % def.cooldown_ticks; // stagger the first cast
        }
    }
    e.def_id = template.def_id.clone();
    e.race = template.race;
    e.size = template.size;
    state.grid.spawn(e);
    state.grid.entities.len() - 1
}

/// Spawn up to `count` minions from the zone pool on FREE cells ringing `center`
/// (the boss), nearest ring first. Each placed minion takes ONE PRNG draw (the
/// weighted pool pick); the cell is chosen deterministically, so the stream is
/// stable given the board. Used at boss spawn (then continuous spawn halts).
pub fn spawn_minions_around(state: &mut GameState, center: Cell, count: u32) {
    let total_weight: u32 = match state.zone_spawn.as_ref() {
        Some(zs) if !zs.pool.is_empty() => zs.pool.iter().map(|e| e.weight).sum(),
        _ => return,
    };
    if total_weight == 0 {
        return;
    }
    let mut placed = 0u32;
    'rings: for radius in 1..=4i32 {
        for dy in -radius..=radius {
            for dx in -radius..=radius {
                if dx.abs().max(dy.abs()) != radius {
                    continue; // ring perimeter only
                }
                if placed >= count {
                    break 'rings;
                }
                let c = Cell::new(center.x + dx, center.y + dy);
                if !state.grid.in_bounds(c)
                    || state.grid.is_occupied(c, usize::MAX, Team::Monster)
                {
                    continue;
                }
                let r = state.rng.gen_range_u32(0, total_weight);
                let entry_idx = pick_weighted_index(state.zone_spawn.as_ref().unwrap(), r);
                let template = state.zone_spawn.as_ref().unwrap().pool[entry_idx].template.clone();
                build_and_spawn_monster(state, template, c);
                placed += 1;
            }
        }
    }
}

/// Summon `count` FAMILIARS (Broomhilda's bats) ringing the summoner. Their combat stats scale
/// off the SUMMONER — 20% of its (buffed) max HP, 40% of its ATK — so they track the band through
/// her, and they carry NO drops / XP / Gil: a summoned mob must NEVER mint loot, else infinite
/// summons = an infinite farm. `def_id` sets the sprite/identity; element is inherited, race/size
/// fixed to a small demon. Reuses `build_and_spawn_monster` (id-desync, active-cast wiring, etc.).
/// Deterministic — no PRNG (cells are chosen in a fixed ring order; the familiars carry no skills).
pub fn spawn_familiars(state: &mut GameState, summoner_idx: usize, count: u32, def_id: &str) {
    let (center, hp, atk, level, element) = {
        let s = &state.grid.entities[summoner_idx];
        let hp = ((crate::combat::effective_max_hp(s) as i64) * 20 / 100).max(1) as i32;
        let atk = ((crate::combat::effective_atk(s) as i64) * 40 / 100).max(1) as i32;
        (s.pos, hp, atk, s.mob_stage, s.element)
    };
    let template = crate::state::MonsterTemplate {
        def_id: def_id.to_string(),
        bestiary_group: String::new(),
        hp,
        atk,
        def: 0,
        atk_min: 0,
        atk_max: 0,
        skills: Vec::new(),
        element, // inherit the summoner's element (Broomhilda = Shadow)
        weaknesses: Vec::new(), // a summon carries no gauge, so nothing chips it
        resists: Vec::new(),
        toughness_max: 0, // summons carry no gauge — they are not a break target
        speed_milli: 200, // a bat flits fast
        atk_speed_milli: 150,
        atk_range: 1,
        drops: Vec::new(), // NO loot — anti-farm
        gear_grade_odds: Vec::new(),
        proj_travel_ticks: 0,
        xp_reward: 0, // NO xp/gil — anti-farm
        gil_reward: 0,
        boss_minions: 0,
        race: crate::grid::Race::Demon,
        size: crate::grid::Size::Small,
        stage: level,
    };
    let mut placed = 0u32;
    'rings: for radius in 1..=4i32 {
        for dy in -radius..=radius {
            for dx in -radius..=radius {
                if dx.abs().max(dy.abs()) != radius {
                    continue; // ring perimeter only
                }
                if placed >= count {
                    break 'rings;
                }
                let c = Cell::new(center.x + dx, center.y + dy);
                if !state.grid.in_bounds(c) || state.grid.is_occupied(c, usize::MAX, Team::Monster) {
                    continue;
                }
                let i = build_and_spawn_monster(state, template.clone(), c);
                if let Some(e) = state.grid.entities.get_mut(i) {
                    e.is_summon = true;
                }
                placed += 1;
            }
        }
    }
}

/// Continuous-spawn check called once per tick from `tick.rs`. Spawns
/// at most ONE monster per tick (keeps PRNG order predictable).
pub fn try_continuous_spawn(state: &mut GameState) {
    let (next_at, floor, boss_spawned, max_spawns, killed) = match state.zone_spawn.as_ref() {
        Some(zs) => (
            zs.next_spawn_at_tick,
            zs.target_population,
            zs.boss_spawned,
            zs.max_spawns,
            zs.kill_counter,
        ),
        None => return,
    };
    // Once the boss has arrived (with its minion ring) the room is sealed — no
    // more trickle. The fight is the boss + whoever is already on the field.
    if boss_spawned {
        return;
    }
    let alive_monsters: u32 = state
        .grid
        .entities
        .iter()
        .filter(|e| matches!(e.team, Team::Monster) && e.alive)
        .count() as u32;
    // At the hard cap → never overspawn.
    if alive_monsters >= MONSTER_CAP {
        return;
    }
    // A CAPPED zone stops once it has produced its quota. Derived from `kill_counter + alive`
    // rather than a separate counter, so no new field has to be kept in step with reality: what
    // this zone has ever trickled in is exactly what is dead plus what is standing.
    //
    // A band zone has `max_spawns == 0` and is unaffected — the endless trickle is the farm loop.
    if max_spawns > 0 && killed.saturating_add(alive_monsters) >= max_spawns {
        return;
    }
    // BELOW the floor → top up IMMEDIATELY (ignore the trickle timer) so the room
    // never feels empty. AT/above the floor → only trickle on the cadence.
    if alive_monsters >= floor && state.tick < next_at {
        return;
    }
    if spawn_one_monster(state, true) {
        // Monster Accelerator: any living player carrying the haste buff halves
        // the trickle interval (2x spawn rate), party-wide. No buff → unchanged.
        let haste = state.grid.entities.iter().any(|e| {
            matches!(e.team, Team::Player)
                && e.alive
                && crate::skills::buff_sum(e, crate::skills::BuffKind::SpawnHaste) > 0
        });
        let trickle = if haste { MONSTER_TRICKLE_TICKS / 2 } else { MONSTER_TRICKLE_TICKS };
        state.zone_spawn.as_mut().unwrap().next_spawn_at_tick = state.tick + trickle;
    }
}

fn pick_weighted_index(zs: &crate::state::ZoneSpawnState, r: u32) -> usize {
    let mut acc: u32 = 0;
    for (i, entry) in zs.pool.iter().enumerate() {
        acc = acc.saturating_add(entry.weight);
        if r < acc {
            return i;
        }
    }
    // Unreachable when total_weight > 0; defensive fallback.
    zs.pool.len() - 1
}

/// Where the boss enters: the RIGHT-CENTRE of the field. The party spawns on the left and advances
/// rightward to meet it — the same direction it walks on to the next area once the room is swept —
/// so the whole zone reads as a left-to-right march. DETERMINISTIC (no PRNG draw): the target is
/// `(width-2, mid-row)` and, if taken, the nearest free cell by expanding Chebyshev rings in a
/// fixed order, so replay stays byte-stable. `player_pos` is unused now (kept for the caller).
pub fn pick_random_cell_for_boss(state: &mut GameState, _player_pos: Option<Cell>) -> Option<Cell> {
    let w = state.grid.width;
    let h = state.grid.height;
    if w <= 0 || h <= 0 {
        return None;
    }
    let target = Cell::new((w - 2).max(0), (h - 1) / 2);
    if !state.grid.is_occupied(target, usize::MAX, Team::Monster) {
        return Some(target);
    }
    for r in 1..=w.max(h) {
        for dy in -r..=r {
            for dx in -r..=r {
                if dx.abs() != r && dy.abs() != r {
                    continue; // ring perimeter only
                }
                let (x, y) = (target.x + dx, target.y + dy);
                if x < 0 || y < 0 || x >= w || y >= h {
                    continue;
                }
                let c = Cell::new(x, y);
                if !state.grid.is_occupied(c, usize::MAX, Team::Monster) {
                    return Some(c);
                }
            }
        }
    }
    None
}

/// RANDOM boss from the pool (gothic bands 5-10): weighted-pick a pool monster — any but
/// `mini_ghost` / `bat` — and make it WEAR `boss_stats`'s stats + rewards, keeping its own
/// look / skills / element / speed / range. So each clear's boss is a different band mob at
/// the SAME tuned boss difficulty. Exactly ONE weighted PRNG draw; falls back to `boss_stats`
/// if nothing is eligible.
pub(crate) fn pick_pool_boss_template(
    state: &mut GameState,
    boss_stats: &crate::state::MonsterTemplate,
) -> crate::state::MonsterTemplate {
    let elig: Vec<(usize, u32)> = {
        let Some(zs) = state.zone_spawn.as_ref() else {
            return boss_stats.clone();
        };
        zs.pool
            .iter()
            .enumerate()
            .filter(|(_, e)| e.template.def_id != "mini_ghost" && e.template.def_id != "bat")
            .map(|(i, e)| (i, e.weight))
            .collect()
    };
    if elig.is_empty() {
        return boss_stats.clone();
    }
    let total: u32 = elig.iter().map(|&(_, w)| w).sum::<u32>().max(1);
    let r = state.rng.gen_range_u32(0, total);
    let mut acc: u32 = 0;
    let mut chosen = elig[0].0;
    for &(i, w) in &elig {
        acc = acc.saturating_add(w);
        if r < acc {
            chosen = i;
            break;
        }
    }
    let mut t = state.zone_spawn.as_ref().unwrap().pool[chosen].template.clone();
    // WEAR the boss stats + rewards; KEEP the mob's look/skills/element/speed/range.
    t.hp = boss_stats.hp;
    t.atk = boss_stats.atk;
    t.def = boss_stats.def;
    t.atk_min = boss_stats.atk_min;
    t.atk_max = boss_stats.atk_max;
    t.xp_reward = boss_stats.xp_reward;
    t.gil_reward = boss_stats.gil_reward;
    t.drops = boss_stats.drops.clone();
    t.boss_minions = boss_stats.boss_minions;
    t
}

/// How long a zone's opening lasts — 10 s at 10 ticks/s. Monsters entering during it come from the
/// right half only.
pub const OPENING_WINDOW_TICKS: u64 = 100;

/// Is this zone still inside its opening window? False with no zone state, so nothing outside a
/// normal zone (tests, sandboxes) changes behaviour.
fn in_opening_window(state: &GameState) -> bool {
    state
        .zone_spawn
        .as_ref()
        .is_some_and(|zs| state.tick.saturating_sub(zs.opened_at_tick) < OPENING_WINDOW_TICKS)
}

fn pick_random_cell(
    state: &mut GameState,
    player_pos: Option<Cell>,
    max_radius: Option<i32>,
) -> Option<Cell> {
    let w = state.grid.width as u32;
    let h = state.grid.height as u32;
    if w == 0 || h == 0 {
        return None;
    }
    // Monster Lure (Some only while a lure is active): sample DIRECTLY inside the
    // radius box around the holder, so trickle monsters ALWAYS land within `r`
    // cells. (Sampling the whole grid then rejecting `> r` wastes most picks on a
    // big board and can give up entirely, leaving the lure sparse.) The square
    // box `[cx-r, cx+r] × [cy-r, cy+r]` IS exactly Chebyshev ≤ r; the
    // safe-distance is intentionally NOT applied (pull them in close — occupancy
    // still keeps them off the holder's own tile). No lure → unchanged below.
    if let (Some(center), Some(r)) = (player_pos, max_radius) {
        // Enumerate the FREE cells in the radius box (Chebyshev ≤ r) around the
        // holder, then pick one at random (ONE draw). This always finds a free
        // near-cell if one exists — no random-then-reject that can miss free
        // cells by bad luck. Safe-distance is intentionally not applied (pull
        // them in close; occupancy keeps them off the holder's own tile).
        let mut free: Vec<Cell> = Vec::new();
        let xlo = (center.x - r).max(0);
        let xhi = (center.x + r).min(state.grid.width - 1);
        let ylo = (center.y - r).max(0);
        let yhi = (center.y + r).min(state.grid.height - 1);
        for y in ylo..=yhi {
            for x in xlo..=xhi {
                let c = Cell::new(x, y);
                if !state.grid.is_occupied(c, usize::MAX, Team::Monster) {
                    free.push(c);
                }
            }
        }
        if !free.is_empty() {
            let idx = state.rng.gen_range_u32(0, free.len() as u32) as usize;
            return Some(free[idx]);
        }
        // No free cell near the holder → fall through to a normal spawn (whole
        // grid + safe-distance) so the trickle never starves.
    }
    // Default (no lure, or the lure fallback): uniform over the whole grid,
    // keeping monsters at least MIN_PLAYER_DISTANCE from the player. With no lure
    // this is IDENTICAL draws to pre-lure code, so every golden stays byte-identical.
    // OPENING WINDOW: for the first `OPENING_WINDOW_TICKS` of a zone, monsters may only enter from
    // the RIGHT half. The party spawns on the left, so the fight opens as an approach across the
    // board and they meet in the middle, instead of the party starting surrounded — uniform
    // placement put a third of the first wave BEHIND them.
    //
    // The window covers the initial wave too: `spawn_initial_population` goes through this same
    // function, so there is one rule rather than two that have to agree.
    let xlo = if in_opening_window(state) { w / 2 } else { 0 };
    for _ in 0..MAX_CELL_RETRIES {
        // Two draws per retry — x then y. Same order forever; only x's RANGE narrows in the
        // opening window, so the draw COUNT per retry is unchanged.
        let x = state.rng.gen_range_u32(xlo, w) as i32;
        let y = state.rng.gen_range_u32(0, h) as i32;
        let c = Cell::new(x, y);
        // Spawn picker is always placing a Monster (boss or mob);
        // Monster-on-Monster stacking is allowed but Player-on-cell
        // is not. exclude_idx = usize::MAX = no entity excluded.
        if state.grid.is_occupied(c, usize::MAX, Team::Monster) {
            continue;
        }
        if let Some(p) = player_pos {
            if c.chebyshev(p) < MIN_PLAYER_DISTANCE {
                continue;
            }
        }
        return Some(c);
    }
    None
}

// `is_occupied` is &Grid, but we used &mut state above for rng. The
// borrow checker is happy because `&state.grid` and `&mut state.rng`
// are disjoint fields. Same trick used in `combat::resolve_attack`.

/// Used by `data::populate_zone` to spawn the initial population
/// before the tick loop runs. Spawns until we hit `target_population`
/// or `spawn_one_monster` fails (e.g. tiny grid).
pub fn spawn_initial_population(state: &mut GameState) {
    let target = match state.zone_spawn.as_ref() {
        Some(zs) => zs.target_population,
        None => return,
    };
    for _ in 0..target {
        // Initial room wave ignores Monster Lure — it fills the room normally
        // (only the continuous trickle gets pulled in near the lure-holder).
        if !spawn_one_monster(state, false) {
            break;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::grid::{Element, Entity, Team};
    use crate::items::{DropEntry, Item, ItemKind, Rarity};
    use crate::state::{GameState, MonsterSpawnEntry, MonsterTemplate, ZoneSpawnState};

    fn make_state_with_pool(seed: u64) -> GameState {
        let mut state = GameState::new(seed, 16, 8);
        // Drop a Player so the safe-distance check has something to compare against.
        state.grid.spawn(
            Entity::idle(1, Cell::new(0, 0))
                .with_team(Team::Player)
                .with_combat(100, 10, 5, 500, Element::Fire),
        );
        // One-entry pool, weight 1 — every draw picks it.
        let template = MonsterTemplate {
                resists: Vec::new(),
            weaknesses: Vec::new(),
            def_id: "slime_test".to_string(),
            bestiary_group: String::new(),
            hp: 20, atk: 5, def: 1,
            atk_min: 0, atk_max: 0,
            skills: Vec::new(),
            element: Element::Earth,
            toughness_max: 100,
            speed_milli: 100,
            atk_speed_milli: 200,
            atk_range: 1,
            drops: vec![DropEntry::new(
                Item::new("scrap", ItemKind::Material, 0, 0, Rarity::Common),
                100,
            )],
            gear_grade_odds: Vec::new(),
            proj_travel_ticks: 0,
            xp_reward: 10,
            gil_reward: 5,
            stage: 0,
            boss_minions: 0,
            race: crate::grid::Race::Unknown,
            size: crate::grid::Size::Medium,
        };
        state.zone_spawn = Some(ZoneSpawnState {
            pool: vec![MonsterSpawnEntry { template, weight: 1 }],
            target_population: 3,
            spawn_interval_ticks: 10,
            opened_at_tick: 0,
            deploy_grace_ticks: crate::tick::DEPLOY_GRACE_TICKS_OPEN,
            next_spawn_at_tick: 0,
            kill_counter: 0,
            boss_threshold: 999,  // high so boss never fires in these tests
            boss_spawned: false,
            boss_template: None,
            boss_from_pool: false,
            max_spawns: 0,
            player_skills: Vec::new(),
            zone_cleared: false,
            cleared_at_tick: None,
            band_clear_counted: false,
            next_zone_id: None,
            is_instance_end: false,
            instance_start_zone_id: String::new(),
        });
        state
    }

    /// A CAPPED zone produces exactly its quota and then stops — the property a story encounter
    /// needs ("ten pinklings and a boss") and the one a band zone must NOT have.
    ///
    /// It COUNTS what the zone actually produced rather than asserting a flag, because the failure
    /// this guards against is off-by-a-few: the cap is derived from `kill_counter + alive`, so a
    /// wrong derivation still stops eventually, just at the wrong number. Killing everything each
    /// tick keeps the floor hungry, which is the case that would overspawn if the gate were absent.
    #[test]
    fn a_capped_zone_spawns_its_quota_and_no_more() {
        const QUOTA: u32 = 10;
        let mut s = make_state_with_pool(7);
        {
            let zs = s.zone_spawn.as_mut().unwrap();
            zs.max_spawns = QUOTA;
            zs.target_population = 3;
        }
        spawn_initial_population(&mut s);
        let mut produced = alive_monsters(&s) as u32;
        for _ in 0..4000 {
            s.tick += 1;
            let before = alive_monsters(&s) as u32;
            try_continuous_spawn(&mut s);
            produced += alive_monsters(&s) as u32 - before;
            // Kill one per tick so the floor keeps asking for more; an uncapped zone answers
            // forever, which is exactly the behaviour being fenced off here.
            if let Some(e) = s.grid.entities.iter_mut().find(|e| {
                matches!(e.team, Team::Monster) && e.alive
            }) {
                e.alive = false;
                s.zone_spawn.as_mut().unwrap().kill_counter += 1;
            }
        }
        assert_eq!(
            produced, QUOTA,
            "a zone capped at {QUOTA} produced {produced} monsters"
        );

        // And the cap is opt-in: the same run with max_spawns = 0 must keep producing, or the gate
        // has quietly changed the farm loop for every band zone in the game.
        let mut u = make_state_with_pool(7);
        u.zone_spawn.as_mut().unwrap().target_population = 3;
        spawn_initial_population(&mut u);
        let mut uncapped = alive_monsters(&u) as u32;
        for _ in 0..4000 {
            u.tick += 1;
            let before = alive_monsters(&u) as u32;
            try_continuous_spawn(&mut u);
            uncapped += alive_monsters(&u) as u32 - before;
            if let Some(e) = u.grid.entities.iter_mut().find(|e| {
                matches!(e.team, Team::Monster) && e.alive
            }) {
                e.alive = false;
                u.zone_spawn.as_mut().unwrap().kill_counter += 1;
            }
        }
        assert!(
            uncapped > QUOTA * 10,
            "an UNcapped zone produced only {uncapped} — the cap leaked into the farm loop"
        );
    }

    #[test]
    fn initial_population_fills_to_target() {
        let mut s = make_state_with_pool(42);
        spawn_initial_population(&mut s);
        let mons: usize = s.grid.entities.iter()
            .filter(|e| matches!(e.team, Team::Monster) && e.alive)
            .count();
        assert_eq!(mons, 3);
    }

    #[test]
    fn spawned_monsters_keep_safe_distance_from_player() {
        let mut s = make_state_with_pool(42);
        spawn_initial_population(&mut s);
        let player_pos = s.grid.entities[0].pos;
        for e in s.grid.entities.iter().filter(|e| matches!(e.team, Team::Monster)) {
            assert!(e.pos.chebyshev(player_pos) >= MIN_PLAYER_DISTANCE,
                "monster spawned too close: {:?} vs player {:?}", e.pos, player_pos);
        }
    }

    fn alive_monsters(s: &GameState) -> usize {
        s.grid.entities.iter()
            .filter(|e| matches!(e.team, Team::Monster) && e.alive).count()
    }

    #[test]
    fn continuous_spawn_trickles_from_floor_toward_cap() {
        let mut s = make_state_with_pool(42);
        spawn_initial_population(&mut s); // fills to the floor (target_population = 3)
        assert_eq!(alive_monsters(&s), 3);
        // At the floor on a ready tick → trickle ONE, then arm the timer.
        try_continuous_spawn(&mut s);
        assert_eq!(alive_monsters(&s), 4, "trickle adds one above the floor");
        // Same tick again → above the floor + timer not elapsed → no spam.
        try_continuous_spawn(&mut s);
        try_continuous_spawn(&mut s);
        assert_eq!(alive_monsters(&s), 4, "no spam within the trickle interval");
        // Past the interval → one more.
        s.tick += MONSTER_TRICKLE_TICKS;
        try_continuous_spawn(&mut s);
        assert_eq!(alive_monsters(&s), 5, "trickles one per interval");
    }

    #[test]
    fn below_floor_refills_immediately() {
        let mut s = make_state_with_pool(42);
        spawn_initial_population(&mut s);
        // Kill two → below the floor (3). Arm the timer far in the future.
        let mut killed = 0;
        for e in s.grid.entities.iter_mut() {
            if matches!(e.team, Team::Monster) && e.alive && killed < 2 {
                e.alive = false;
                killed += 1;
            }
        }
        s.zone_spawn.as_mut().unwrap().next_spawn_at_tick = 9_999;
        // Below the floor ⇒ refills NOW despite the timer, one per call, up to the floor.
        try_continuous_spawn(&mut s);
        assert_eq!(alive_monsters(&s), 2, "first refill toward the floor");
        try_continuous_spawn(&mut s);
        assert_eq!(alive_monsters(&s), 3, "back to the floor");
        try_continuous_spawn(&mut s);
        assert_eq!(alive_monsters(&s), 3, "at the floor → timer gates further spawns");
    }

    #[test]
    fn monster_lure_confines_trickle_spawns_near_the_holder() {
        let mut s = make_state_with_pool(42);
        // Lure radius 8 on the player at (0,0): trickle monsters spawn within 8
        // cells, ignoring the usual safe-distance (grid is 16 wide → real confinement).
        crate::skills::apply_buff(&mut s.grid.entities[0], crate::skills::Buff {
            kind: crate::skills::BuffKind::SpawnLure, magnitude: 8, ticks: 36000, source_id: 1,
        });
        // TRICKLE spawns only (below the floor → fire immediately). The lure does
        // NOT apply to the initial wave, so this exercises the continuous path.
        for _ in 0..3 {
            try_continuous_spawn(&mut s);
        }
        let p = s.grid.entities[0].pos;
        let mons: Vec<_> = s.grid.entities.iter()
            .filter(|e| matches!(e.team, Team::Monster)).collect();
        assert!(!mons.is_empty(), "lure still spawns trickle monsters");
        for e in mons {
            let d = e.pos.chebyshev(p);
            // Within 8 cells AND not on the holder's tile (occupancy keeps d >= 1).
            assert!(d >= 1 && d <= 8,
                "lured trickle monster at dist {} (want 1..=8 of holder)", d);
        }
    }

    #[test]
    fn initial_wave_ignores_the_lure() {
        // With a TIGHT lure radius the initial fill would be near-impossible to
        // place if it respected the lure; it ignores it and still fills the floor.
        let mut s = make_state_with_pool(42);
        crate::skills::apply_buff(&mut s.grid.entities[0], crate::skills::Buff {
            kind: crate::skills::BuffKind::SpawnLure, magnitude: 4, ticks: 36000, source_id: 1,
        });
        spawn_initial_population(&mut s);
        assert_eq!(alive_monsters(&s), 3, "initial wave fills the floor regardless of lure");
    }

    #[test]
    fn monster_accelerator_halves_the_trickle_interval() {
        let mut s = make_state_with_pool(42);
        spawn_initial_population(&mut s); // fills to the floor (3)
        crate::skills::apply_buff(&mut s.grid.entities[0], crate::skills::Buff {
            kind: crate::skills::BuffKind::SpawnHaste, magnitude: 1, ticks: 36000, source_id: 1,
        });
        try_continuous_spawn(&mut s); // trickles one, arms the timer at HALF
        let next = s.zone_spawn.as_ref().unwrap().next_spawn_at_tick;
        assert_eq!(next, s.tick + MONSTER_TRICKLE_TICKS / 2,
            "haste halves the trickle interval");
    }

    #[test]
    fn continuous_spawn_fires_after_kill_and_cooldown() {
        let mut s = make_state_with_pool(42);
        spawn_initial_population(&mut s);
        // Kill one monster.
        let kill_idx = s.grid.entities.iter().position(
            |e| matches!(e.team, Team::Monster) && e.alive,
        ).unwrap();
        s.grid.entities[kill_idx].alive = false;
        let alive_after_kill: usize = s.grid.entities.iter()
            .filter(|e| matches!(e.team, Team::Monster) && e.alive).count();
        assert_eq!(alive_after_kill, 2);

        // Advance to spawn_interval_ticks. First try fires the spawn,
        // because next_spawn_at_tick was already set to 0 + interval
        // by the initial fill loop — wait, actually no, spawn_initial
        // doesn't touch next_spawn_at_tick. So spawn_at = 0 ⇒ tick=0
        // try would fire. Set tick forward to be safe.
        s.tick = 100;
        try_continuous_spawn(&mut s);
        let alive_after_respawn: usize = s.grid.entities.iter()
            .filter(|e| matches!(e.team, Team::Monster) && e.alive).count();
        assert_eq!(alive_after_respawn, 3, "one monster should have spawned");
    }
}
