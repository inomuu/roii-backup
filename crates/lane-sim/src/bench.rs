//! REAL entities for the lane trial: party members and stage monsters built by `sim-core` itself,
//! so HP, ATK, DEF, attack speed, range, crit and the damage formula are the live combat's.
//!
//! The party setup is adapted from `sim-cli`'s `--bench` (`build.rs`: `force_power`,
//! `bench_gear`, `set_stars`, `auto_regalia`), through the same public `sim-core` calls. Each unit
//! is built in its own state (`new_game_as`), because the live party caps at five; party-shared
//! power (Training, Crests, Soul Paths) and allies' auras are therefore absent.

use sim_core::combat;
use sim_core::data::{self, GameData};
use sim_core::grid::{Entity, Team};
use sim_core::items::{self, Item, ItemKind, Rarity};

use crate::{Stats, Traits};

#[derive(Clone, Debug)]
pub struct BenchSpec {
    pub level: u32,
    pub tier: u32,
    /// Index into `sim_core::items::GRADES` (3 = Legendary).
    pub grade: u8,
    pub stage: u32,
    pub stars: u8,
    /// Signature Regalia at this level (0 = none).
    pub regalia_level: u8,
    pub seed: u64,
}

impl Default for BenchSpec {
    /// The tuning workflow's standard read (`--bench 70:6:legendary:168 --regalia sig:60:0`).
    fn default() -> Self {
        BenchSpec { level: 70, tier: 6, grade: 3, stage: 168, stars: 0, regalia_level: 60, seed: 42 }
    }
}

pub fn zone_of(stage: u32) -> String {
    format!("zone_b{stage}_1")
}

/// Lane stats read off a live entity through the combat module's own `effective_*`.
pub fn stats_of(e: &Entity) -> Stats {
    Stats {
        hp: i64::from(combat::effective_max_hp(e)),
        atk: i64::from(combat::effective_atk(e)),
        def: i64::from(combat::effective_def(e)),
        range: combat::effective_atk_range(e).max(1),
        atk_interval: crate::catalog::interval(combat::effective_atk_speed_milli(e)),
        move_interval: crate::catalog::interval(e.speed_milli),
    }
}

/// One party member at the bench's power.
pub fn unit(data: &GameData, unit_id: &str, spec: &BenchSpec) -> Result<(Entity, Traits), String> {
    let mut state = data::new_game_as(spec.seed, data, &zone_of(spec.stage), unit_id)?;
    let id = state.grid.entities.iter().find(|e| matches!(e.team, Team::Player) && !e.is_pet).map(|e| e.id)
        .ok_or_else(|| format!("{unit_id}: no player entity"))?;
    // set_stars: the Exemplar overlay carries the Resonance ranks.
    if let Some(e) = state.grid.entities.iter_mut().find(|e| e.id == id) {
        if e.exemplar.is_none() {
            if let Some(def) = items::exemplar_def_by_id(&e.unit_def) {
                e.exemplar = Some(items::Exemplar { id: def.id.clone(), name: def.name.clone(), skin_id: def.skin_id.clone(), stars: 0 });
            }
        }
        if let Some(x) = e.exemplar.as_mut() {
            x.stars = spec.stars.min(items::EXEMPLAR_MAX_STARS);
            e.apply_exemplar_skill();
        }
    }
    // force_power + bench_gear.
    let cur = state.grid.entities.iter().find(|e| e.id == id).map_or(1, |e| e.stats.level);
    if spec.level > cur {
        data::debug_add_levels(&mut state, id, spec.level - cur);
    }
    let grade = items::GRADES.get(usize::from(spec.grade)).copied().unwrap_or(items::Grade::Legendary);
    data::debug_give_gear_at(&mut state, data, id, spec.tier, grade);
    // auto_regalia ("sig"): the unit's highest existing rung, worn through the real input.
    if spec.regalia_level > 0 {
        if let Some(ids) = items::regalia_line_for_unit(unit_id) {
            let rung = (0..3usize).rev().find_map(|i| ids.get(i).copied().filter(|s| !s.is_empty()).map(|s| (s, i)));
            if let Some((rid, i)) = rung {
                let rarity = [Rarity::Common, Rarity::Mythical, Rarity::Legendary][i];
                let mut item = Item::new(&items::regalia_item_def_id(rid), ItemKind::Regalia, 0, 0, rarity);
                item.enhance_level = spec.regalia_level.min(items::REGALIA_MAX_LEVEL);
                state.owned_regalia.push(item);
                let owned_idx = u32::try_from(state.owned_regalia.len() - 1).unwrap_or(0);
                sim_core::events::apply_input(&mut state, sim_core::events::InputEvent::EquipRegalia { entity_id: id, owned_idx });
            }
        }
    }
    let e = state.grid.entities.iter().find(|e| e.id == id).cloned().ok_or("member vanished")?;
    let traits = Traits::default();
    Ok((e, traits))
}

/// `n` monsters of the bench stage, spawned by the live zone code (weights, stage scaling and
/// all) and lifted out of its grid.
pub fn monsters(data: &GameData, spec: &BenchSpec, n: usize) -> Result<Vec<Entity>, String> {
    let mut state = data::new_game_as(spec.seed, data, &zone_of(spec.stage), "seraphine")?;
    let mut out: Vec<Entity> = Vec::new();
    let mut guard = 0;
    while out.len() < n && guard < n * 20 + 50 {
        guard += 1;
        let have: Vec<u32> = out.iter().map(|e| e.id).collect();
        let fresh: Vec<Entity> = state.grid.entities.iter()
            .filter(|e| matches!(e.team, Team::Monster) && e.alive && !have.contains(&e.id)).cloned().collect();
        out.extend(fresh);
        if out.len() >= n {
            break;
        }
        state.grid.entities.retain(|e| !matches!(e.team, Team::Monster));
        sim_core::spawn::spawn_one_monster(&mut state, false);
    }
    out.truncate(n);
    if out.len() < n {
        return Err(format!("stage {}: spawned {} of {n} monsters", spec.stage, out.len()));
    }
    Ok(out)
}

/// One blow, the live way: `build_damage` for the attacker, `apply_damage` against the target with
/// `mob_attackers` monsters on it (the mobbed-DEF penalty), and crit folded in as its EXPECTED
/// value — the lane trial draws no random numbers.
pub fn blow(attacker: &Entity, target: &Entity, mob_attackers: u32) -> i64 {
    let plain = i64::from(combat::apply_damage(target, &combat::build_damage(attacker, false), mob_attackers));
    let crit = i64::from(combat::apply_damage(target, &combat::build_damage(attacker, true), mob_attackers));
    let c = i64::from(combat::effective_crit_chance_bp(attacker).min(10_000));
    ((plain * (10_000 - c) + crit * c) / 10_000).max(1)
}

/// The stage's boss and its minions, spawned by the live zone code: the final room's kill quota
/// is met so `tick` raises the boss (pool pick, boss stats, its `boss_minions` ring) exactly as
/// the game does. Returns `(boss, minions)`.
pub fn boss(data: &GameData, spec: &BenchSpec) -> Result<(Entity, Vec<Entity>), String> {
    let zone = [3, 2, 1].iter().map(|r| format!("zone_b{}_{r}", spec.stage))
        .find(|z| data.zones.iter().any(|d| &d.id == z)).ok_or("no zone for the stage")?;
    let mut state = data::new_game_as(spec.seed, data, &zone, "seraphine")?;
    state.grid.entities.retain(|e| !matches!(e.team, Team::Monster));
    {
        let zs = state.zone_spawn.as_mut().ok_or("zone has no spawn state")?;
        if zs.boss_template.is_none() {
            return Err(format!("{zone} has no boss"));
        }
        zs.kill_counter = zs.boss_threshold;
    }
    for _ in 0..50 {
        sim_core::tick::tick(&mut state, 1);
        if let Some(b) = state.grid.entities.iter().find(|e| e.is_boss && e.alive) {
            let id = b.id;
            let minions = state.grid.entities.iter()
                .filter(|e| matches!(e.team, Team::Monster) && e.alive && !e.is_boss && e.id > id).cloned().collect();
            return Ok((b.clone(), minions));
        }
    }
    Err(format!("{zone}: the boss never spawned"))
}
