//! LADDER SWEEP (plan A, user 2026-10-04): replay the live game's own party, stage by stage, on
//! the lane rules. `sim-cli --snapshots DIR` writes the whole state the first time a live run
//! enters each stage. Each snapshot's fielded party is placed on a lane formation and runs that
//! stage (`stage::run`). The question it answers: where do lanes make the live ladder harder or
//! easier?

use sim_core::grid::{Entity, Team};
use sim_core::potential::{doctrine_for_unit, Doctrine};
use sim_core::state::GameState;

use crate::bench::BenchSpec;
use crate::{Battle, Col, Rules};

/// One snapshot: the stage it was taken on, the live tick, the fielded members.
pub struct Snapshot {
    pub stage: u32,
    pub tick: u64,
    pub party: Vec<Entity>,
}

pub fn load(dir: &str) -> Result<Vec<Snapshot>, String> {
    let mut out = Vec::new();
    for entry in std::fs::read_dir(dir).map_err(|e| format!("{dir}: {e}"))? {
        let path = entry.map_err(|e| e.to_string())?.path();
        let Some(stage) = path.file_stem().and_then(|s| s.to_str()).and_then(|s| s.strip_prefix("stage_")).and_then(|s| s.parse().ok()) else { continue };
        let raw = std::fs::read_to_string(&path).map_err(|e| format!("{}: {e}", path.display()))?;
        let state: GameState = serde_json::from_str(&raw).map_err(|e| format!("{}: {e}", path.display()))?;
        let party = state.grid.entities.iter().filter(|e| matches!(e.team, Team::Player) && !e.is_pet).cloned().collect();
        out.push(Snapshot { stage, tick: state.tick, party });
    }
    out.sort_by_key(|s| s.stage);
    Ok(out)
}

/// Where a member stands: Sentinels and other melee in front (middle lane first), Supports in the
/// mid row, everything ranged at the back; overflow spills one row back.
pub fn formation(party: &[Entity]) -> Vec<(Col, i32, usize)> {
    let order = [1, 0, 2];
    let mut used: Vec<(Col, i32)> = Vec::new();
    let mut out = Vec::new();
    let mut idx: Vec<usize> = (0..party.len()).collect();
    let rank = |e: &Entity| match (doctrine_for_unit(&e.unit_def), e.atk_range <= 1) {
        (Some(Doctrine::Sentinel), _) => 0,
        (_, true) => 1,
        (Some(Doctrine::Support), _) => 2,
        _ => 3,
    };
    idx.sort_by_key(|&i| (rank(&party[i]), party[i].id));
    for i in idx {
        let prefer: &[Col] = match rank(&party[i]) {
            0 | 1 => &[Col::Front, Col::Mid, Col::Back],
            2 => &[Col::Mid, Col::Back, Col::Front],
            _ => &[Col::Back, Col::Mid, Col::Front],
        };
        let slot = prefer.iter().flat_map(|&c| order.iter().map(move |&l| (c, l))).find(|s| !used.contains(s));
        if let Some((c, l)) = slot {
            used.push((c, l));
            out.push((c, l, i));
        }
    }
    out
}

/// A battle with the snapshot's party on its lane formation, every member at full HP.
pub fn place(snap: &Snapshot, rules: Rules) -> Battle {
    let mut b = Battle::new(Rules { party_limit: 9, ..rules });
    for (col, lane, i) in formation(&snap.party) {
        let mut e = snap.party[i].clone();
        e.alive = true;
        e.hp = sim_core::combat::effective_max_hp(&e);
        let name = sim_core::items::exemplar_def_by_id(&e.unit_def).map_or(e.unit_def.clone(), |d| d.name.clone());
        let traits = crate::Traits::default();
        let _ = b.place_entity(&format!("{name}@{}{lane}", col.tag()), e, traits, col, lane);
    }
    b
}

pub fn spec_for(stage: u32, seed: u64) -> BenchSpec {
    BenchSpec { stage, seed, ..BenchSpec::default() }
}

pub fn party_label(snap: &Snapshot) -> String {
    snap.party.iter().map(|e| {
        let name = sim_core::items::exemplar_def_by_id(&e.unit_def).map_or(e.unit_def.clone(), |d| d.name.clone());
        format!("{name} {}", e.stats.level)
    }).collect::<Vec<_>>().join(", ")
}

/// How a stage went on the LIVE sim in lane mode (`sim_core::lane`).
pub struct LiveRun {
    pub cleared: bool,
    pub ticks: u64,
    pub dead: usize,
}

/// Load a snapshot as a whole live state.
pub fn load_state(dir: &str, stage: u32) -> Result<GameState, String> {
    let path = format!("{dir}/stage_{stage}.json");
    let raw = std::fs::read_to_string(&path).map_err(|e| format!("{path}: {e}"))?;
    serde_json::from_str(&raw).map_err(|e| format!("{path}: {e}"))
}

/// Run `stage` with the snapshot's party on the live sim in lane mode (everything live: skills,
/// heals, breaks, Limit Break, Regalia, monster skills, the 3-room flow), from a full-HP entry
/// until the third room's boss falls (cleared), the party wipes, or `limit` ticks pass.
pub fn live_stage(data: &sim_core::data::GameData, mut state: GameState, stage: u32, cap: u8, limit: u64) -> Result<LiveRun, String> {
    sim_core::data::change_zone(&mut state, data, &format!("zone_b{stage}_1"))?;
    sim_core::lane::enable(&mut state, data, cap)?;
    let start = state.tick;
    while state.tick - start < limit {
        sim_core::tick::tick(&mut state, 1);
        let alive = state.grid.entities.iter().any(|e| matches!(e.team, Team::Player) && !e.is_pet && e.alive);
        if !alive {
            return Ok(LiveRun { cleared: false, ticks: state.tick - start, dead: count_dead(&state) });
        }
        if let Some(next) = state.pending_zone_change.clone() {
            // The live room flow: rooms 1 → 2 → 3 of this stage, then the next stage = cleared.
            if sim_core::state::band_of_zone(&next).map(u32::from) != Some(stage) {
                return Ok(LiveRun { cleared: true, ticks: state.tick - start, dead: count_dead(&state) });
            }
            sim_core::data::change_zone(&mut state, data, &next)?;
        }
    }
    Ok(LiveRun { cleared: false, ticks: state.tick - start, dead: count_dead(&state) })
}

fn count_dead(state: &GameState) -> usize {
    state.grid.entities.iter().filter(|e| matches!(e.team, Team::Player) && !e.is_pet && !e.alive).count()
}

/// Walk-in time: for every monster that entered during one room-1 → room-3 run, the ticks from its
/// spawn to the first tick a party member or it had the other in range; plus the share of ticks
/// the whole party stood with nothing in range. `lane` = lane mode, else the live 20×10 field.
pub fn walk_in(data: &sim_core::data::GameData, mut state: GameState, stage: u32, lane: bool, limit: u64, speed_bp: u32) -> Result<(u64, u64, u64, u64), String> {
    use std::collections::BTreeMap;
    sim_core::data::change_zone(&mut state, data, &format!("zone_b{stage}_1"))?;
    if lane {
        sim_core::lane::enable(&mut state, data, 2)?;
        if let Some(l) = state.lane.as_mut() {
            l.speed_bp = speed_bp;
        }
    }
    let mut born: BTreeMap<u32, u64> = BTreeMap::new();
    let mut waits: Vec<u64> = Vec::new();
    let (mut idle, mut ticks) = (0u64, 0u64);
    let start = state.tick;
    while state.tick - start < limit {
        sim_core::tick::tick(&mut state, 1);
        ticks += 1;
        let party: Vec<&Entity> = state.grid.entities.iter().filter(|e| matches!(e.team, Team::Player) && !e.is_pet && e.alive).collect();
        if party.is_empty() {
            break;
        }
        let mut any = false;
        for m in state.grid.entities.iter().filter(|e| matches!(e.team, Team::Monster) && e.alive) {
            let t0 = *born.entry(m.id).or_insert(state.tick);
            let reach = party.iter().any(|p| {
                let d = p.pos.chebyshev(m.pos);
                d <= sim_core::combat::effective_atk_range(p) || d <= sim_core::combat::effective_atk_range(m)
            });
            if reach {
                any = true;
                if t0 != u64::MAX {
                    waits.push(state.tick - t0);
                    born.insert(m.id, u64::MAX);
                }
            }
        }
        if !any {
            idle += 1;
        }
        if let Some(next) = state.pending_zone_change.clone() {
            if sim_core::state::band_of_zone(&next).map(u32::from) != Some(stage) {
                break;
            }
            sim_core::data::change_zone(&mut state, data, &next)?;
        }
    }
    let n = waits.len() as u64;
    let avg = if n == 0 { 0 } else { waits.iter().sum::<u64>() / n };
    let mut sorted = waits.clone();
    sorted.sort_unstable();
    let p90 = sorted.get(sorted.len().saturating_sub(1) * 9 / 10).copied().unwrap_or(0);
    Ok((avg, p90, idle, ticks))
}

/// A tick-by-tick TRACE of one stage on the live sim in lane mode, for the replay page: every
/// on-board entity's cell and HP each tick, and the tick's hits, skill casts, ultimates and heals
/// exactly as the sim recorded them (`recent_hits`, `skill_fx`, `ult_fx`, `heal_fx`).
pub fn trace(data: &sim_core::data::GameData, mut state: GameState, stage: u32, limit: u64, speed_bp: u32, spawn_near: bool) -> Result<String, String> {
    use std::fmt::Write as _;
    sim_core::data::change_zone(&mut state, data, &format!("zone_b{stage}_1"))?;
    sim_core::lane::enable(&mut state, data, 2)?;
    if let Some(l) = state.lane.as_mut() {
        l.speed_bp = speed_bp;
        l.spawn_near = spawn_near;
    }
    let mut ents = String::new();
    let mut known: std::collections::BTreeSet<u32> = std::collections::BTreeSet::new();
    let mut frames = String::new();
    let start = state.tick;
    let mut outcome = "timeout";
    let mut room = 1u32;
    while state.tick - start < limit {
        sim_core::tick::tick(&mut state, 1);
        for e in state.grid.entities.iter().filter(|e| e.on_board()) {
            if known.insert(e.id) {
                let player = matches!(e.team, Team::Player);
                let name = if player {
                    sim_core::items::exemplar_def_by_id(&e.unit_def).map_or(e.unit_def.clone(), |d| d.name.clone())
                } else {
                    data.monster(&e.def_id).map_or(e.def_id.clone(), |m| m.name.clone())
                };
                let _ = write!(ents, "{}\"{}\":{{\"n\":{:?},\"t\":\"{}\",\"u\":{:?},\"r\":{},\"lv\":{}}}",
                    if ents.is_empty() { "" } else { "," }, e.id, name, if player { "P" } else if e.is_boss { "B" } else { "M" },
                    if player { e.unit_def.clone() } else { e.def_id.clone() }, sim_core::combat::effective_atk_range(e), e.stats.level);
            }
        }
        let units: Vec<String> = state.grid.entities.iter().filter(|e| e.on_board() && e.alive)
            .map(|e| {
                let max = sim_core::combat::effective_max_hp(e).max(1);
                format!("[{},{},{},{}]", e.id, e.pos.x, e.pos.y, i64::from(e.hp.max(0)) * 1000 / i64::from(max))
            }).collect();
        let hits: Vec<String> = state.recent_hits.iter().map(|h| format!("[{},{},{},{}]", h.attacker_id, h.target_id, h.amount,
            match h.kind { sim_core::state::HitKind::Crit => 1, sim_core::state::HitKind::Break => 2, sim_core::state::HitKind::Dodge => 3, sim_core::state::HitKind::Guard => 4, _ => 0 })).collect();
        let skills: Vec<String> = state.skill_fx.iter().map(|f| format!("[{},{:?}]", f.caster_id, f.skill_id)).collect();
        let ults: Vec<String> = state.ult_fx.iter().map(|f| format!("[{},{:?}]", f.caster_id, f.skill_id)).collect();
        let heals: Vec<String> = state.heal_fx.iter().map(|h| format!("[{},{},{}]", h.healer_id, h.target_id, h.amount)).collect();
        let marching = state.lane.as_ref().is_some_and(|l| l.marching);
        let _ = write!(frames, "{}[{},{},[{}],[{}],[{}],[{}],[{}],{}]", if frames.is_empty() { "" } else { ",\n" },
            state.tick - start, room, units.join(","), hits.join(","), skills.join(","), ults.join(","), heals.join(","), u8::from(marching));
        if !state.grid.entities.iter().any(|e| matches!(e.team, Team::Player) && !e.is_pet && e.alive) {
            outcome = "wiped";
            break;
        }
        if let Some(next) = state.pending_zone_change.clone() {
            if sim_core::state::band_of_zone(&next).map(u32::from) != Some(stage) {
                outcome = "cleared";
                break;
            }
            sim_core::data::change_zone(&mut state, data, &next)?;
            room += 1;
        }
    }
    Ok(format!("{{\"stage\":{stage},\"outcome\":\"{outcome}\",\"lanes\":{},\"width\":{},\"ents\":{{{ents}}},\"frames\":[\n{frames}]}}",
        sim_core::lane::LANES, sim_core::lane::WIDTH))
}
