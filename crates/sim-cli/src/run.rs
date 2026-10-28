// sim-cli run loop + CSV output.
//
// Loads game data, runs the sim for N ticks, auto-plays the party (see
// `build` module), and emits a CSV progression curve row every `sample` ticks.

use std::fs;
use std::io::{self, BufWriter, Write};

use sim_core::data::{self, GameData};
use sim_core::grid::Team;
use sim_core::hash::state_hash;
use sim_core::state::GameState;
use sim_core::tick;

use crate::build::{
    auto_allocate_soul_paths, auto_allocate_stat_points, auto_equip_best, auto_train,
    auto_party, auto_potential, auto_regalia, bench_gear, force_power, set_stars, set_stars_for,
    auto_discard_surplus_gear, auto_disenchant, auto_fuse_surplus, auto_crests,
    auto_fuse_gems, auto_socket_gems,
};
use crate::Opts;

pub(crate) fn run(opts: Opts) -> Result<(), String> {
    let json = fs::read_to_string(&opts.data_path)
        .map_err(|e| format!("read {}: {e}", opts.data_path))?;
    let data = GameData::from_json_str(&json)?;
    // The STARTER — Seraphine alone, the genesis every real client boots from
    // (`Session::new_starter`), so the first hours measure the first session a player has.
    let mut state = match &opts.lead {
        // `--lead <unit>`: field another unit alone at genesis (A/B: "what if Rapis led?").
        Some(unit) => data::new_game_as(opts.seed, &data, &opts.zone, unit)?,
        None => data::new_starter_party(opts.seed, &data, &opts.zone)?,
    };

    let (mut stall_kills, mut stall_since, mut stall_dumped) = (0u64, 0u64, false);
    let mut writer: Box<dyn Write> = match &opts.out {
        Some(p) => Box::new(BufWriter::new(
            fs::File::create(p).map_err(|e| format!("create {p}: {e}"))?,
        )),
        None => Box::new(BufWriter::new(io::stdout())),
    };

    write_header(&mut writer)?;

    // TEST card drop-rate multiplier. sim-cli is a TRUSTED balance tool (never
    // replay-verified), so it sets the field DIRECTLY at tick 0 — bypassing the
    // MAX_CARD_RATE_BONUS_BP cap on the player-facing SetCardRateBonus event — to
    // farm-test cards at any multiplier.
    if opts.card_rate_mult > 1 {
        state.card_rate_bonus_bp = opts.card_rate_mult.saturating_mul(10_000);
    }

    // Toughness breaks, counted from the display hit stream — the measurement the
    // gauge exists to answer ("how often does it actually break?"). Counting here
    // rather than on GameState keeps a tuning number out of the hashed state.
    let mut breaks: u64 = 0;
    // Ultimates fired, counted from the per-tick `ult_fx` stream the sim emits for every
    // cast (the old gauge-falls-to-zero read would miss a cast whose gauge is refilled on
    // the same tick). Kept out of GameState for the same reason as `breaks`: a tuning
    // number has no business in the hashed state.
    let mut ults: u64 = 0;
    let mut ults_by_unit: std::collections::BTreeMap<String, u64> = std::collections::BTreeMap::new();
    let mut cumulative_kills: u64 = 0;
    let mut bosses_killed: u64 = 0;
    let mut wipes: u64 = 0;
    let mut current_zone = opts.zone.clone();
    // Every Potential material the run DROPPED, by def — read off `loot_fx` (per-tick, display
    // only) so the tuning number never enters the hashed state. The purse shows what is LEFT;
    // this shows what came in, which is what a rate is tuned against.
    let mut dropped: std::collections::BTreeMap<String, u64> = std::collections::BTreeMap::new();
    let mut gear_drops: Vec<(u64, String, String)> = Vec::new();
    let mut loot_by_def: std::collections::BTreeMap<String, u64> = std::collections::BTreeMap::new();
    let mut flow = opts.gacha_flow.map(|seed| {
        crate::onboarding::GachaFlow::new(seed, opts.lead.as_deref().unwrap_or("seraphine"), !opts.no_quest_weapon)
    });

    // --level / --kit: stand the party up at the target power BEFORE the first
    // tick, so tick 0 already reflects the party the content is tuned for.
    force_power(&mut state, &data, opts.level, opts.kit);
    if let Some((level, tier, grade, _)) = opts.bench {
        // The whole party stands up at tick 0 in the benchmark wardrobe.
        auto_party(&mut state, &data, &opts.recruit_order, true);
        force_power(&mut state, &data, level, false);
        bench_gear(&mut state, &data, tier, grade);
        if let Some((mode, rl, rs)) = &opts.regalia {
            auto_regalia(&mut state, mode, *rl, *rs);
        }
    }
    if let Some(stars) = opts.stars {
        set_stars(&mut state, stars);
    }
    for (unit, n) in &opts.stars_for {
        set_stars_for(&mut state, unit, *n);
    }

    if let Some(cap) = opts.lane {
        sim_core::lane::enable(&mut state, &data, cap)?;
        if let (Some(l), Some(bp)) = (state.lane.as_mut(), opts.lane_speed_bp) {
            l.speed_bp = bp.max(1);
        }
        if let (Some(l), Some(g)) = (state.lane.as_mut(), opts.lane_groups) {
            l.groups_per_room = g.max(1);
        }
        current_zone = state.current_zone_id.clone().unwrap_or(current_zone);
    }
    snapshot(opts.snapshots.as_deref(), &state, &current_zone)?;
    // Tick 0 — initial state row.
    write_row(&mut writer, &state, &data, &current_zone, cumulative_kills, bosses_killed, wipes, breaks, ults)?;

    // PROGRESS on stderr, every 1% (and at least every sample): a multi-day run is minutes to an
    // hour of wall-clock, and "is it stuck" is otherwise unanswerable. Wall-clock is fine here —
    // this is the runner, not the sim.
    let started = std::time::Instant::now();
    let every = (opts.ticks / 100).max(1);
    for t in 1..=opts.ticks {
        if let Some(n) = opts.dummies {
            // Hold the room at exactly N: the zone stops producing at N (dummies never die, so
            // the quota is the standing count) and any surplus from the opening spawn leaves.
            if let Some(zs) = state.zone_spawn.as_mut() {
                zs.max_spawns = n;
                zs.target_population = n;
            }
            let mut kept = 0u32;
            state.grid.entities.retain(|e| {
                if !(matches!(e.team, Team::Monster) && e.alive) {
                    return true;
                }
                kept += 1;
                kept <= n
            });
        }
        if opts.dummy {
            // Every monster on the board is a dummy: it never dies and never hits back.
            for e in state.grid.entities.iter_mut() {
                if matches!(e.team, Team::Monster) && e.alive {
                    e.max_hp = i32::MAX / 4;
                    e.hp = e.max_hp;
                    e.atk = 0;
                    e.atk_base_milli = 0;
                }
            }
        }
        tick::tick(&mut state, 1);
        if t % every == 0 || t == opts.ticks {
            let secs = started.elapsed().as_secs_f64();
            let eta = if t > 0 { secs * (opts.ticks - t) as f64 / t as f64 } else { 0.0 };
            eprintln!(
                "progress {:3}% tick {}/{} day {:.1} lv{} {} — {:.0}s elapsed, ~{:.0}s left",
                t * 100 / opts.ticks, t, opts.ticks, t as f64 / 864_000.0,
                data::lead_level(&state), current_zone, secs, eta
            );
        }
        for it in &state.loot_fx {
            if sim_core::potential::is_sigil_def(&it.def_id) {
                *dropped.entry(it.def_id.clone()).or_insert(0) += u64::from(it.quantity.max(1));
            }
            // Every drop by def, for the LOOT line (display-only `loot_fx`).
            *loot_by_def.entry(it.def_id.clone()).or_insert(0) += u64::from(it.quantity.max(1));
            // Gear drops (display-only `loot_fx`), for the GEAR DROPS line: slot and grade.
            if matches!(it.kind, sim_core::items::ItemKind::Weapon | sim_core::items::ItemKind::Armor) {
                gear_drops.push((t, format!("{:?}", it.slot()), format!("{:?}", it.grade)));
            }
        }
        breaks += state
            .recent_hits
            .iter()
            .filter(|h| matches!(h.kind, sim_core::state::HitKind::Break))
            .count() as u64;
        for f in &state.ult_fx {
            if let Some(e) = state.grid.entities.iter().find(|e| e.id == f.caster_id && matches!(e.team, Team::Player)) {
                ults += 1;
                *ults_by_unit.entry(e.unit_def.clone()).or_insert(0) += 1;
            }
        }

        // Auto-advance zones the same way godot-ext does: if the sim
        // requested a zone change, apply it before sampling. Counters
        // must be carried over BEFORE change_zone resets the per-zone
        // kill_counter (and the boss kill itself).
        if let Some(mut next_id) = state.pending_zone_change.clone() {
            // A benchmark holds its stage: past room 3 it wraps to room 1 of the same stage.
            if let Some((_, _, _, stage)) = opts.bench {
                let past = next_id
                    .strip_prefix("zone_b")
                    .and_then(|r| r.split('_').next())
                    .and_then(|s| s.parse::<u32>().ok())
                    .is_some_and(|s| s != stage);
                if past {
                    next_id = format!("zone_b{stage}_1");
                }
            }
            let last_kc = state
                .zone_spawn
                .as_ref()
                .map(|z| z.kill_counter as u64)
                .unwrap_or(0);
            cumulative_kills += last_kc;
            bosses_killed += 1;
            data::change_zone(&mut state, &data, &next_id)?;
            // Lane mode moves a stage into its final room (`sim_core::lane::enter_zone`).
            current_zone = if opts.lane.is_some() { state.current_zone_id.clone().unwrap_or(next_id) } else { next_id };
            snapshot(opts.snapshots.as_deref(), &state, &current_zone)?;
        }

        // Party WIPE → restart the instance at room 1, KEEPING xp/job/soul
        // (mirrors godot-ext `restart_instance`). Without this a fresh char
        // dies and the run stalls — the kept-XP wipe-grind is how an idle
        // character actually progresses, so it must be modeled to measure
        // the 24h pace.
        // EXCLUDE pets (mirrors sim-core's own game-over check, tick/mod.rs): a
        // ranger's untargetable pet can't be killed by monsters, so counting it
        // here means a party whose every REAL member is dead never registers a
        // wipe — the run deadlocks (dead lead never revived, no instance restart).
        let party_alive = state
            .grid
            .entities
            .iter()
            .any(|e| matches!(e.team, Team::Player) && e.alive && !e.is_pet);
        if !party_alive {
            // Auto-retreat farming: 5 wipes on a band → grind the previous band
            // 20 clears, then return — models an OPTIMAL player pacing past a wall.
            // sim-cli ONLY (auto_retreat = true); the real game restarts in place.
            data::handle_party_wipe(&mut state, &data)?;
            current_zone = state
                .zone_spawn
                .as_ref()
                .map(|z| z.instance_start_zone_id.clone())
                .filter(|s| !s.is_empty())
                .unwrap_or(current_zone);
            wipes += 1;
        }

        // Model the player allocating earned stat points to a class build —
        // in-game the USER does this manually, but for a headless 24h run we
        // auto-spend on a sensible build so the measured pace reflects a
        // character that actually gets tankier, not the never-allocates worst
        // case that stalls. (sim-cli only; the game stays manual.)
        // Hold the forced power level (tops up later recruits too) BEFORE the
        // auto-play passes, so allocation/skills/gear act on the real level.
        force_power(&mut state, &data, opts.level, opts.kit);
        // Spend looted Potential materials on the board — a real player buys every rank the bag
        // allows, and a kit left at rank 1 under-measures every caster and support. (sim-cli only.)
        auto_potential(&mut state);
        // Spend the stat points those levels granted — since 0.0.347 they are the main thing a
        // level buys, so a run that leaves them unspent measures a weaker character than any
        // real one. (sim-cli only; the game keeps allocation manual.)
        auto_allocate_stat_points(&mut state);
        // Likewise model the player wearing the best looted gear (banked to the
        // stash on each instance clear) — gear is a real part of the power
        // curve, so without it the measured char is too weak past the early
        // bands. (sim-cli only.)
        auto_equip_best(&mut state);
        // Model the player FUSING surplus gear — the only Gil income since vendoring was
        // retired (0.0.411) — then discarding what fusion could not take, so the bag never
        // clogs and higher-tier drops are not lost on the floor. (sim-cli only.)
        auto_fuse_surplus(&mut state, &data);
        auto_discard_surplus_gear(&mut state, &data);
        // Model the party growing: slots bought with the first Gil, each filled at once
        // (the slot's 10-pull ticket, modelled as the next class in `--recruit`), or by the
        // draft onboarding with real pulls under --gacha-flow.
        match flow.as_mut() {
            Some(f) => crate::onboarding::step(&mut state, &data, f),
            None => auto_party(&mut state, &data, &opts.recruit_order, false),
        }
        if let Some((mode, level, stars)) = &opts.regalia {
            auto_regalia(&mut state, mode, *level, *stars);
        }
        if let Some(stars) = opts.stars {
            set_stars(&mut state, stars);
        }
        // Re-applied AFTER `--stars`, which stamps every member each tick: without this a
        // `--stars 0 --stars-for u:6` run measured u at S0.
        for (unit, n) in &opts.stars_for {
            set_stars_for(&mut state, unit, *n);
        }
        // Model the player spending SOUL on the passive web — a party-wide power
        // source the run otherwise never measures. (sim-cli only.)
        auto_allocate_soul_paths(&mut state);
        // Model the player buying Training levels with the Gil that has nowhere else to go
        // (0.0.406). Once a minute: the prices are what make it slow, not the cadence.
        if t % 600 == 0 {
            auto_train(&mut state, &data);
            auto_crests(&mut state, &data);
            // Gems (0.0.448 — they are bag stacks the player fuses six-to-one and sockets into
            // the worn gear's holes): both once a minute, like Training. Neither was ever wired
            // into this loop before — only `--gear-report` modelled them.
            auto_fuse_gems(&mut state, &data);
            auto_socket_gems(&mut state, &data);
        }
        // The "fusion or disenchant" split, once an hour: whatever T3+ fusion could not use in
        // that hour goes to the crests. Every minute starved fusion of its sixes (measured: Gil
        // flat from day 3) — an hour lets the sixes form.
        if t % 36_000 == 0 {
            auto_disenchant(&mut state, &data, true);
        }

        if t % opts.sample == 0 || t == opts.ticks {
            write_row(&mut writer, &state, &data, &current_zone, cumulative_kills, bosses_killed, wipes, breaks, ults)?;
        }
        // STALL DUMP (0.0.416 diagnostics): `ROI_STALL_DUMP=<path>` writes the save the moment no
        // kill has landed for an hour of sim time, so a deadlock a 14-day run hits on day 9 can
        // be loaded and inspected instead of re-run. Once.
        if let Ok(path) = std::env::var("ROI_STALL_DUMP") {
            if cumulative_kills != stall_kills {
                stall_kills = cumulative_kills;
                stall_since = t;
            } else if t - stall_since >= 36_000 && !stall_dumped {
                stall_dumped = true;
                let json = serde_json::to_string(&state).unwrap_or_default();
                let _ = fs::write(&path, json);
                eprintln!("stall: no kill since tick {stall_since}; state written to {path}");
            }
        }
    }

    writer.flush().map_err(|e| e.to_string())?;
    // GEAR DROPS: every weapon/armour piece that dropped, by minute, slot and grade.
    {
        let mut by_slot: std::collections::BTreeMap<&str, u32> = std::collections::BTreeMap::new();
        let mut by_grade: std::collections::BTreeMap<&str, u32> = std::collections::BTreeMap::new();
        for (_, slot, grade) in &gear_drops {
            *by_slot.entry(slot.as_str()).or_insert(0) += 1;
            *by_grade.entry(grade.as_str()).or_insert(0) += 1;
        }
        let at: Vec<String> = gear_drops.iter().map(|(t, slot, _)| format!("{:.1}m {slot}", *t as f64 / 600.0)).collect();
        eprintln!("GEAR DROPS: {} pieces  slots {by_slot:?}  grades {by_grade:?}", gear_drops.len());
        eprintln!("  at: {}", at.join(", "));
    }
    // LOOT: everything that dropped, grouped by item kind, then the ten commonest defs.
    {
        let mut by_kind: std::collections::BTreeMap<String, u64> = std::collections::BTreeMap::new();
        for (def, n) in &loot_by_def {
            let kind = data.item(def).map_or("?".to_string(), |d| format!("{:?}", d.kind));
            *by_kind.entry(kind).or_insert(0) += n;
        }
        let mut top: Vec<(&String, &u64)> = loot_by_def.iter().collect();
        top.sort_by(|a, b| b.1.cmp(a.1));
        let top: Vec<String> = top.iter().take(10).map(|(d, n)| format!("{d}×{n}")).collect();
        eprintln!("LOOT: {by_kind:?}  soul held {}  gil held {}", state.soul(), state.gil());
        eprintln!("  top: {}", top.join(", "));
    }
    report_potential(&state, &data, bosses_killed, &dropped);
    if let Some(f) = &flow {
        eprintln!("GACHA FLOW: {} pulls, {} unit results", f.pulls, f.units_pulled);
        for line in &f.log {
            eprintln!("  {line}");
        }
    }
    report_members(&state, &ults_by_unit);
    if let Some((level, tier, grade, stage)) = opts.bench {
        // One line to compare runs by: the party's damage per sim hour and the kill pace.
        let hours = (state.tick.max(1) as f64) / 36_000.0;
        let dealt: u64 = state.dps.by_class.values().sum();
        let kills = cumulative_kills + state.zone_spawn.as_ref().map_or(0, |z| z.kill_counter as u64);
        eprintln!(
            "BENCH lv{level} tier{tier} grade{grade} stage{stage} regalia {}: party dmg/h {:.0}  kills/h {:.0}  bosses {}  wipes {}  CP {}",
            opts.regalia.as_ref().map_or("none".to_string(), |(m, l, s)| format!("{m}:{l}:{s}")),
            dealt as f64 / hours, kills as f64 / hours, bosses_killed, wipes,
            sim_core::combat_score::party_score(&state)
        );
        // Per member, so a star or a tier step can be read against that member's dmg/alive-h.
        let g = &state.gear_dropped_by_tier;
        eprintln!("  LOOT gear/h by tier: T1 {} T2 {} T3 {} T4 {} T5 {} T6 {} T7 {}  (kills {})", g[1], g[2], g[3], g[4], g[5], g[6], g[7], kills);
        // Memory Dust banked by the run (0.0.490) — the Enhance XP faucet, per sim hour.
        let (d, r, c) = (
            state.material_count("regalia_fodder"),
            state.material_count("regalia_sr_fodder"),
            state.material_count("regalia_crystal_fodder"),
        );
        let xp = u64::from(d) * 1_000 + u64::from(r) * 10_000 + u64::from(c) * 100_000;
        eprintln!("  DUST dust {d} radiant {r} crystal {c}  = {:.0} XP/h", xp as f64 / hours);
        let train: u32 = state.training.values().map(|v| u32::from(*v)).sum();
        let crest: u32 = state.crests.values().map(|v| u32::from(*v)).sum();
        eprintln!("  CP shared train_lv {train} crest_lv {crest}");
        for e in state.grid.entities.iter().filter(|e| matches!(e.team, Team::Player) && !e.is_pet) {
            eprintln!("  CP {:<10} {} (own {})", e.unit_def, sim_core::combat_score::member_score(&state, e), sim_core::combat_score::member_own_score(e));
        }
    }
    Ok(())
}

/// The Potential ladder's pace, on stderr: every member's skill-node ranks and the purse the run
/// ended with. This is the number the rate tuning is measured against ("rank 12 ≈ 10-14 idle
/// days"), and nothing in the CSV carries it.
/// Sigil per grade for one unit's whole board: every skill node to `SOFT_CAP` plus every stat
/// and Major node at rank 1 — straight from the sim's own cost tables.
fn one_unit_need() -> [u64; 3] {
    use sim_core::potential::{node_cost, NodeKind, NODE_DEFS, SOFT_CAP};
    let mut need = [0u64; 3];
    for def in NODE_DEFS.iter() {
        match def.kind {
            NodeKind::Root => {}
            NodeKind::Skill { .. } => {
                for r in 2..=SOFT_CAP {
                    let c = node_cost(def.kind, r);
                    need[c.sigil.0] += u64::from(c.sigil.1);
                }
            }
            _ => {
                let c = node_cost(def.kind, 1);
                need[c.sigil.0] += u64::from(c.sigil.1);
            }
        }
    }
    need
}

/// PER-MEMBER TELEMETRY (0.0.422; user: "ต้องวัด damage done / healing done / damage received
/// แต่ละตัวตอน tune"): what each unit dealt, healed and took over the whole run, from the sim's
/// own `dps` tracker, plus its share of the party's damage. Stderr, so a CSV pipe stays clean.
fn report_members(state: &GameState, ults_by_unit: &std::collections::BTreeMap<String, u64>) {
    let dealt_total: u64 = state.dps.by_class.values().sum::<u64>().max(1);
    let mut units: Vec<String> = state
        .dps
        .by_class
        .keys()
        .chain(state.dps.taken_by_class.keys())
        .chain(state.dps.healed_by_class.keys())
        .chain(state.dps.blocked_by_class.keys())
        .cloned()
        .collect();
    units.sort();
    units.dedup();
    eprintln!("MEMBERS (whole run):");
    eprintln!("  {:<14} {:>14} {:>6} {:>7} {:>14} {:>14} {:>14} {:>6} {:>14} {:>7} {:>7} {:>12} {:>5}", "unit", "damage dealt", "share", "basic%", "healing done", "damage taken", "mitigated", "mit%", "blocked", "deaths", "uptime", "dmg/alive-h", "ults");
    for u in units {
        let d = state.dps.by_class.get(&u).copied().unwrap_or(0);
        let h = state.dps.healed_by_class.get(&u).copied().unwrap_or(0);
        let t = state.dps.taken_by_class.get(&u).copied().unwrap_or(0);
        let k = state.dps.deaths_by_class.get(&u).copied().unwrap_or(0);
        let a = state.dps.alive_ticks_by_class.get(&u).copied().unwrap_or(0);
        let up = if state.tick > 0 { a * 100 / state.tick } else { 0 };
        let per_h = if a > 0 { d * 36_000 / a } else { 0 };
        let b = state.dps.basic_by_class.get(&u).copied().unwrap_or(0);
        let ul = ults_by_unit.get(&u).copied().unwrap_or(0);
        let bl = state.dps.blocked_by_class.get(&u).copied().unwrap_or(0);
        // mit% = mitigated / (mitigated + taken + blocked): the share of what was thrown at
        // this member that its DEF / reductions took off before shields and HP.
        let mit = state.dps.mitigated_by_class.get(&u).copied().unwrap_or(0);
        let thrown = mit + t + bl;
        let mit_pct = if thrown > 0 { mit * 100 / thrown } else { 0 };
        eprintln!("  {:<14} {:>14} {:>5}% {:>6}% {:>14} {:>14} {:>14} {:>5}% {:>14} {:>7} {:>6}% {:>12} {:>5}", u, d, d * 100 / dealt_total, if d > 0 { b * 100 / d } else { 0 }, h, t, mit, mit_pct, bl, k, up, per_h, ul);
    }
    let mut skills: Vec<(&String, &u64)> = state.dps.by_skill.iter().collect();
    skills.sort_by(|a, b| b.1.cmp(a.1));
    eprintln!("  top skills: {}", skills.iter().take(8).map(|(k, v)| format!("{k} {}%", *v * 100 / dealt_total)).collect::<Vec<_>>().join(" · "));
    // Per member: every skill's share of THAT member's damage.
    let mut by_unit: std::collections::BTreeMap<&String, Vec<(&String, u64)>> = std::collections::BTreeMap::new();
    for ((unit, skill), amt) in &state.dps.by_class_skill {
        by_unit.entry(unit).or_default().push((skill, *amt));
    }
    for (unit, mut rows) in by_unit {
        let total: u64 = rows.iter().map(|(_, a)| *a).sum::<u64>().max(1);
        rows.sort_by(|a, b| b.1.cmp(&a.1));
        eprintln!("    {:<10} {}", unit, rows.iter().map(|(k, v)| format!("{k} {}%", v * 100 / total)).collect::<Vec<_>>().join(" · "));
    }
}

fn report_potential(
    state: &GameState,
    data: &GameData,
    bosses_killed: u64,
    dropped: &std::collections::BTreeMap<String, u64>,
) {
    use sim_core::potential::{actions, NodeKind, NODE_DEFS};
    let purse = actions::purse(state);
    for e in state.grid.entities.iter().filter(|e| matches!(e.team, sim_core::grid::Team::Player) && !e.is_pet) {
        let ranks: Vec<String> = NODE_DEFS
            .iter()
            .filter(|d| matches!(d.kind, NodeKind::Skill { .. }))
            .map(|d| format!("{}={}", d.id, e.potential.ranks.get(d.id).copied().unwrap_or(0)))
            .collect();
        eprintln!(
            "potential {} ({}) skills [{}] spent={}",
            e.unit_def, e.id, ranks.join(" "), e.potential.spent_points,
        );
    }
    let sig = purse.sigil;
    eprintln!("potential purse sigil I×{} II×{} III×{} core={} track={} gil={}", sig[0], sig[1], sig[2], purse.core, purse.track, purse.gil);
    // Boss Core / Track are SERVER-rolled (crowd-scaled), so this run never holds any; the
    // expected value at the catalog's base odds is what the rate is tuned against.
    // What came in per grade, as grade-I equivalents (3:1 per grade) against ONE unit's whole
    // board (rank 12 on four skill nodes + every stat / Major node). Sigils have no element since
    // 2026-09-30, so every drop counts toward any unit.
    use sim_core::potential::{SIGIL_GRADES, SIGIL_MERGE};
    let mut by_grade = [0u64; SIGIL_GRADES];
    for (g, n) in by_grade.iter_mut().enumerate() {
        *n = dropped.get(&sim_core::potential::sigil_def(g)).copied().unwrap_or(0);
    }
    let m = u64::from(SIGIL_MERGE);
    let got_eq = by_grade[0] + by_grade[1] * m + by_grade[2] * m * m;
    let need = one_unit_need();
    let need_eq = need[0] + need[1] * m + need[2] * m * m;
    eprintln!(
        "potential dropped I×{} II×{} III×{} = {} I-eq · one unit to 12 + every node needs I×{} II×{} III×{} = {} I-eq → {}%",
        by_grade[0], by_grade[1], by_grade[2], got_eq, need[0], need[1], need[2], need_eq,
        got_eq * 100 / need_eq.max(1)
    );
    let bp = |id: &str| -> u64 {
        data.monsters
            .iter()
            .flat_map(|m| m.drops.iter())
            .filter(|r| r.item_id == id)
            .map(|r| r.server_bp as u64)
            .max()
            .unwrap_or(0)
    };
    let (core_bp, track_bp) = (bp("boss_core"), bp("track"));
    eprintln!(
        "potential expected (solo, no crowd scale): core≈{} track≈{} from {} boss kills @ {}/{} bp",
        bosses_killed * core_bp / 10_000, bosses_killed * track_bp / 10_000, bosses_killed, core_bp, track_bp
    );
}

/// `--snapshots`: the whole state, the first time the run stands in room 1 of a stage.
fn snapshot(dir: Option<&str>, state: &GameState, zone: &str) -> Result<(), String> {
    let (Some(dir), Some(stage)) = (dir, sim_core::state::band_of_zone(zone)) else { return Ok(()) };
    let path = format!("{dir}/stage_{stage}.json");
    if zone != format!("zone_b{stage}_1") || std::path::Path::new(&path).exists() {
        return Ok(());
    }
    let json = serde_json::to_string(state).map_err(|e| format!("snapshot: {e}"))?;
    fs::create_dir_all(dir).map_err(|e| format!("{dir}: {e}"))?;
    fs::write(&path, json).map_err(|e| format!("{path}: {e}"))
}

fn write_header(w: &mut Box<dyn Write>) -> Result<(), String> {
    writeln!(
        w,
        "tick,zone,player_hp,player_alive,equipped,ground,kill_counter,boss_threshold,boss_spawned,zone_cleared,cum_kills,boss_kills,wipes,level,xp,apex_level,gil,party,breaks,ults,state_hash12,party_atk,party_hp,train_lv,gil_spent,fusions,gil_earned,crest_lv,disenchanted,gems_held,gems_socketed,gem_fusions,combat_score"
    )
    .map_err(|e| e.to_string())
}

fn write_row(
    w: &mut Box<dyn Write>,
    state: &GameState,
    data: &GameData,
    zone: &str,
    cum_kills: u64,
    bosses_killed: u64,
    wipes: u64,
    breaks: u64,
    ults: u64,
) -> Result<(), String> {
    let player = state
        .grid
        .entities
        .iter()
        .find(|e| matches!(e.team, Team::Player));
    let hp = player.map(|e| e.hp).unwrap_or(0);
    let alive = player.map(|e| e.alive).unwrap_or(false);
    let equipped = player.map(|e| e.equipped.len()).unwrap_or(0);
    // Auto-loot model: drops land in the party-shared `state.inventory`
    // (replaced per-entity `Entity.inventory`). We report the stash row
    // count as the "loot count" so the column keeps its meaning.
    let drops = state.inventory.len();
    let (kc, bt, bs, zc) = state.zone_spawn.as_ref().map_or(
        (0u32, 0u32, false, false),
        |z| (z.kill_counter, z.boss_threshold, z.boss_spawned, z.zone_cleared),
    );
    let total_kills = cum_kills + kc as u64;
    let level = player.map(|e| e.stats.level).unwrap_or(0);
    let xp = player.map(|e| e.stats.xp).unwrap_or(0);
    let apex_level = player.map(|e| e.stats.apex_level).unwrap_or(0);
    let hash = state_hash(state);
    let party = state
        .grid
        .entities
        .iter()
        .filter(|e| matches!(e.team, Team::Player))
        .count();
    // PARTY POWER (0.0.401): the sum of every member's effective ATK and max HP — what
    // `tools/fit_stage_ladder.py` fits the stage table against.
    let (party_atk, party_hp) = state
        .grid
        .entities
        .iter()
        .filter(|e| matches!(e.team, Team::Player) && !e.is_pet)
        .fold((0i64, 0i64), |(a, h), e| {
            (a + i64::from(sim_core::combat::effective_atk(e)), h + i64::from(sim_core::combat::effective_max_hp(e)))
        });
    writeln!(
        w,
        "{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{}",
        state.tick,
        zone,
        hp,
        alive,
        equipped,
        drops,
        kc,
        bt,
        bs,
        zc,
        total_kills,
        bosses_killed,
        wipes,
        level,
        xp,
        apex_level,
        state.gil(),
        party,
        breaks,
        ults,
        &hash[..12],
        party_atk,
        party_hp,
        state.training.values().map(|v| u32::from(*v)).sum::<u32>(),
        state.measures.gil_spent.get(),
        state.measures.fusions.get(),
        state.measures.gil_earned.get(),
        state.crests.values().map(|v| u32::from(*v)).sum::<u32>(),
        state.measures.items_disenchanted.get(),
        sim_core::gems::held(state, data).map(|(_, n)| n).sum::<u32>(),
        state.measures.gems_socketed.get(),
        state.measures.gem_fusions.get(),
        sim_core::combat_score::party_score(state)
    )
    .map_err(|e| e.to_string())?;
    // A sample row is progress: flush it so a multi-day run can be watched (`--out` or a pipe)
    // instead of landing all at once at the end.
    w.flush().map_err(|e| e.to_string())
}
