//! `lane-sim` — run the lane-combat trial against the live catalog.
//!
//!   cargo run -p lane-sim -- [--mode A|B|C] [--cap N] [--party standard6|full3|nine|F0=id,M1=id,...]
//!                            [--bench L:T:grade:stage] [--stars N] [--regalia LV] [--teleport unit_id]
//!                            [--scenario name|all] [--ticks N] [--frames K] [--compare]
//!                            [--simple [--level N] [--monster id]]
//!
//! Default: LIVE entities (`bench.rs`) — the party at `--bench` power (default 70:6:legendary:168,
//! Regalia sig 60, stars 0) against monsters the stage's own zone spawns, every blow through
//! `sim-core`'s damage formula. `--simple` falls back to raw catalog numbers.
//!
//! Scenarios: melee9, lane2, spit, diver, burrow, rear, mixed. `--compare` runs every scenario
//! for modes A/B/C × the standard6 and full3 formations and prints one line each.

use lane_sim::bench::{self, BenchSpec};
use lane_sim::{catalog, Battle, Col, Event, Outcome, Rules, TargetMode, Team, Traits};
use sim_core::data::GameData;

const PRESETS: &[(&str, &str)] = &[
    ("standard6", "F0=lumi,F1=seraphine,M0=raplus,M1=lucielle,B0=lapis,B1=aveline"),
    ("full3", "F0=lumi,F1=seraphine,F2=charon,M1=lucielle,B0=lapis,B1=aveline"),
    ("skills6", "F0=lumi,F1=seraphine,F2=charon,M1=nemu,B0=lapis,B1=aveline"),
    ("nine", "F0=lumi,F1=seraphine,F2=charon,M0=raplus,M1=lucielle,M2=zolfia,B0=lapis,B1=aveline,B2=airi"),
];
const SCENARIOS: &[&str] = &["melee9", "lane2", "spit", "diver", "burrow", "rear", "mixed"];

struct Opts {
    mode: TargetMode,
    cap: usize,
    party: String,
    level: i64,
    monster: String,
    teleport: Option<String>,
    scenario: String,
    ticks: u64,
    frames: u64,
    compare: bool,
    simple: bool,
    bench: BenchSpec,
    party_stack: usize,
    stage_runs: u64,
    ladder: Option<String>,
    reach: bool,
    walkin: bool,
    trace: Option<(u32, String)>,
    speed_bp: u32,
    spawn_near: bool,
}

fn parse() -> Result<Opts, String> {
    let mut o = Opts {
        mode: TargetMode::C, cap: 2, party: "standard6".into(), level: 1, monster: "pinkling".into(),
        teleport: None, scenario: "all".into(), ticks: 3000, frames: 0, compare: false,
        simple: false, bench: BenchSpec::default(), party_stack: 1, stage_runs: 0, ladder: None, reach: false, walkin: false, trace: None, speed_bp: sim_core::lane::LANE_SPEED_BP, spawn_near: false,
    };
    let args: Vec<String> = std::env::args().skip(1).collect();
    let mut it = args.iter();
    while let Some(a) = it.next() {
        let mut val = || it.next().cloned().ok_or(format!("{a} needs a value"));
        match a.as_str() {
            "--mode" => o.mode = match val()?.as_str() { "A" => TargetMode::A, "B" => TargetMode::B, "C" => TargetMode::C, v => return Err(format!("mode {v}")) },
            "--cap" => o.cap = val()?.parse().map_err(|e| format!("--cap: {e}"))?,
            "--party" => o.party = val()?,
            "--level" => o.level = val()?.parse().map_err(|e| format!("--level: {e}"))?,
            "--monster" => o.monster = val()?,
            "--teleport" => o.teleport = Some(val()?),
            "--scenario" => o.scenario = val()?,
            "--ticks" => o.ticks = val()?.parse().map_err(|e| format!("--ticks: {e}"))?,
            "--frames" => o.frames = val()?.parse().map_err(|e| format!("--frames: {e}"))?,
            "--compare" => o.compare = true,
            "--simple" => o.simple = true,
            "--ladder" => o.ladder = Some(val()?),
            "--reach" => o.reach = true,
            "--walkin" => o.walkin = true,
            "--speed" => o.speed_bp = val()?.parse().map_err(|e| format!("--speed: {e}"))?,
            "--spawn-near" => o.spawn_near = true,
            "--trace" => {
                let stage = val()?.parse().map_err(|e| format!("--trace STAGE OUT: {e}"))?;
                o.trace = Some((stage, val()?));
            }
            "--stage" => o.stage_runs = val()?.parse().map_err(|e| format!("--stage: {e}"))?,
            "--party-stack" => o.party_stack = val()?.parse().map_err(|e| format!("--party-stack: {e}"))?,
            "--stars" => o.bench.stars = val()?.parse().map_err(|e| format!("--stars: {e}"))?,
            "--regalia" => o.bench.regalia_level = val()?.parse().map_err(|e| format!("--regalia: {e}"))?,
            "--bench" => {
                let v = val()?;
                let mut p = v.split(':');
                o.bench.level = p.next().and_then(|x| x.parse().ok()).unwrap_or(70);
                o.bench.tier = p.next().and_then(|x| x.parse().ok()).unwrap_or(6);
                let g = p.next().unwrap_or("legendary");
                o.bench.grade = g.parse().ok().unwrap_or_else(|| match g.to_ascii_lowercase().as_str() {
                    "common" => 0, "uncommon" => 1, "rare" => 2, "legendary" => 3, "immortal" => 4,
                    "arcana" => 5, "beyond" => 6, "celestial" => 7, "divine" => 8, "cosmic" => 9, _ => 3,
                });
                o.bench.stage = p.next().and_then(|x| x.parse().ok()).unwrap_or(168);
            }
            "--help" | "-h" => return Err(String::new()),
            v => return Err(format!("unknown flag {v}")),
        }
    }
    Ok(o)
}

fn slots(spec: &str) -> Result<Vec<(Col, i32, String)>, String> {
    let spec = PRESETS.iter().find(|(n, _)| *n == spec).map(|(_, s)| *s).unwrap_or(spec);
    spec.split(',').filter(|s| !s.is_empty()).map(|s| {
        let (slot, id) = s.split_once('=').ok_or(format!("slot '{s}' is not X#=unit"))?;
        let mut ch = slot.chars();
        let col = match ch.next() { Some('F') => Col::Front, Some('M') => Col::Mid, Some('B') => Col::Back, _ => return Err(format!("slot '{slot}'")) };
        let lane: i32 = ch.as_str().parse().map_err(|_| format!("slot '{slot}'"))?;
        Ok((col, lane, id.to_string()))
    }).collect()
}

/// What a scenario spawns: (traits, lane, ranged).
fn waves(scenario: &str) -> Result<Vec<(Traits, i32, bool)>, String> {
    let t = Traits::default();
    let diver = Traits { diver: true, ..t };
    let burrow = Traits { burrow: true, ..t };
    let rear = Traits { rear: true, ..t };
    let mut w = Vec::new();
    match scenario {
        "melee9" => for lane in 0..3 { for _ in 0..3 { w.push((t, lane, false)); } },
        "lane2" => for _ in 0..6 { w.push((t, 2, false)); },
        "spit" => {
            for lane in 0..3 { for _ in 0..2 { w.push((t, lane, false)); } }
            for lane in 0..3 { w.push((t, lane, true)); }
        }
        "diver" => for lane in 0..3 { w.push((diver, lane, false)); },
        "burrow" => for lane in 0..3 { w.push((burrow, lane, false)); },
        "rear" => for lane in [0, 1, 1, 2] { w.push((rear, lane, false)); },
        "mixed" => {
            for lane in 0..3 { w.push((t, lane, false)); w.push((t, lane, false)); }
            w.push((t, 1, true));
            w.push((diver, 2, false));
            w.push((burrow, 1, false));
            w.push((rear, 1, false));
        }
        s => return Err(format!("unknown scenario {s}")),
    }
    Ok(w)
}

fn label(t: &Traits, ranged: bool) -> &'static str {
    if t.diver { "diver" } else if t.burrow { "burrow" } else if t.rear { "rear" } else if ranged { "ranged" } else { "melee" }
}

fn place_party(data: &GameData, o: &Opts, party: &str, mode: TargetMode) -> Result<Battle, String> {
    let slots = slots(party)?;
    let mut b = Battle::new(Rules { mode, stack_cap: o.cap, party_limit: slots.len().max(6), party_stack: o.party_stack });
    for (col, lane, id) in &slots {
        // Display name from the Exemplar line.
        let shown = sim_core::items::exemplar_def_by_id(id).map_or(id.as_str(), |d| d.name.as_str());
        let name = format!("{shown}@{}{lane}", col.tag());
        if o.simple {
            let (stats, mut traits) = catalog::unit(data, id, o.level).ok_or(format!("unknown unit {id}"))?;
            traits.teleport = o.teleport.as_deref() == Some(id.as_str());
            b.place(&name, stats, traits, *col, *lane).map_err(|e| format!("{id}: {e:?}"))?;
        } else {
            let (ent, mut traits) = bench::unit(data, id, &o.bench)?;
            traits.teleport = o.teleport.as_deref() == Some(id.as_str());
            b.place_entity(&name, ent, traits, *col, *lane).map_err(|e| format!("{id}: {e:?}"))?;
        }
    }
    Ok(b)
}

fn build(data: &GameData, o: &Opts, party: &str, mode: TargetMode, scenario: &str) -> Result<Battle, String> {
    let waves = waves(scenario)?;
    let mut b = place_party(data, o, party, mode)?;
    if o.simple {
        let mob = catalog::monster(data, &o.monster).ok_or(format!("unknown monster {}", o.monster))?;
        for (traits, lane, ranged) in waves {
            let mut st = mob.clone();
            if ranged { st.range = st.range.max(3); }
            b.spawn(label(&traits, ranged), st, traits, lane);
        }
    } else {
        let mobs = bench::monsters(data, &o.bench, waves.len())?;
        for ((traits, lane, ranged), mut ent) in waves.into_iter().zip(mobs) {
            if ranged { ent.atk_range = ent.atk_range.max(3); }
            b.spawn_entity(label(&traits, ranged), ent, traits, lane);
        }
    }
    Ok(b)
}

fn describe(o: &Opts) -> String {
    if o.simple {
        format!("catalog Lv{} vs {}", o.level, o.monster)
    } else {
        let b = &o.bench;
        format!("bench {}:{}:{}:{} · stars {} · regalia {}", b.level, b.tier, b.grade, b.stage, b.stars, b.regalia_level)
    }
}

struct Summary {
    outcome: Outcome,
    ticks: u64,
    dead: usize,
    party: usize,
    worst_engaged: (u32, String),
    taken: i64,
    teleports: usize,
    fizzles: usize,
    surfaced: usize,
}

fn summarize(b: &Battle, outcome: Outcome) -> Summary {
    let party: Vec<_> = b.units.iter().filter(|u| u.team == Team::Party).collect();
    let worst = party.iter().max_by_key(|u| (u.max_engaged, std::cmp::Reverse(u.id))).map(|u| (u.max_engaged, u.name.clone())).unwrap_or((0, String::new()));
    let count = |f: fn(&Event) -> bool| b.events.iter().filter(|(_, e)| f(e)).count();
    Summary {
        outcome, ticks: b.tick,
        dead: party.iter().filter(|u| !u.alive).count(), party: party.len(),
        worst_engaged: worst, taken: party.iter().map(|u| u.taken).sum(),
        teleports: count(|e| matches!(e, Event::Teleport { .. })),
        fizzles: count(|e| matches!(e, Event::TeleportFizzled { .. })),
        surfaced: count(|e| matches!(e, Event::Surfaced { .. })),
    }
}

fn run(b: &mut Battle, ticks: u64, frames: u64) -> Outcome {
    while b.tick < ticks {
        b.step();
        if frames > 0 && b.tick % frames == 0 {
            println!("-- tick {} --\n{}", b.tick, b.render());
        }
        if let Some(o) = b.outcome() {
            return o;
        }
    }
    Outcome::Timeout
}

fn main() {
    let o = match parse() {
        Ok(o) => o,
        Err(e) => {
            if !e.is_empty() { eprintln!("{e}"); }
            eprintln!("usage: lane-sim [--mode A|B|C] [--cap N] [--party standard6|full3|nine|F0=id,...] [--bench L:T:grade:stage] [--stars N] [--regalia LV] [--teleport unit] [--party-stack N] [--stage RUNS] [--ladder DIR] [--scenario {}|all] [--ticks N] [--frames K] [--compare] [--simple [--level N] [--monster id]]", SCENARIOS.join("|"));
            std::process::exit(2);
        }
    };
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../../data/game_data.json");
    let data = match std::fs::read_to_string(path).map_err(|e| e.to_string()).and_then(|s| GameData::from_json_str(&s)) {
        Ok(d) => d,
        Err(e) => { eprintln!("catalog: {e}"); std::process::exit(1); }
    };
    if let (Some(dir), Some((stage, out))) = (o.ladder.clone(), o.trace.clone()) {
        let r = lane_sim::ladder::load_state(&dir, stage).and_then(|st| lane_sim::ladder::trace(&data, st, stage, 36_000, o.speed_bp, o.spawn_near));
        match r.and_then(|json| std::fs::write(&out, json).map_err(|e| e.to_string())) {
            Ok(()) => println!("wrote {out}"),
            Err(e) => { eprintln!("{e}"); std::process::exit(2); }
        }
        return;
    }
    if let (Some(dir), true) = (o.ladder.clone(), o.walkin) {
        println!("stage  mode   walk-in avg  p90   party idle");
        for stage in [11u32, 41, 71, 101] {
            for (lane, bp) in [(false, 10_000u32), (true, 10_000), (true, 5_000)] {
                let r = lane_sim::ladder::load_state(&dir, stage).and_then(|st| lane_sim::ladder::walk_in(&data, st, stage, lane, 36_000, bp));
                match r {
                    Ok((avg, p90, idle, ticks)) => println!("{:>5}  {:<8}  {:>9.1}s  {:>4.1}s  {:>4.1}% of {:.0}s", stage, if !lane { "live".to_string() } else { format!("lane{}%", bp / 100) },
                        avg as f64 / 10.0, p90 as f64 / 10.0, idle as f64 * 100.0 / ticks.max(1) as f64, ticks as f64 / 10.0),
                    Err(e) => println!("{stage}: {e}"),
                }
            }
        }
        return;
    }
    if let (Some(dir), true) = (o.ladder.clone(), o.reach) {
        run_reach(&data, &o, &dir);
        return;
    }
    if let Some(dir) = o.ladder.clone() {
        run_ladder(&data, &o, &dir);
        return;
    }
    if o.stage_runs > 0 {
        run_stages(&data, &o);
        return;
    }
    let scenarios: Vec<&str> = if o.scenario == "all" { SCENARIOS.to_vec() } else { vec![o.scenario.as_str()] };

    if o.compare {
        println!("{}", describe(&o));
        println!("{:<9} {:<10} {:<4} {:<8} {:>6} {:>5} {:>9} {:>22}", "scenario", "party", "mode", "outcome", "sec", "dead", "dmg_taken", "max_swarm (unit)");
        for s in &scenarios {
            for party in ["standard6", "full3"] {
                for mode in [TargetMode::A, TargetMode::B, TargetMode::C] {
                    let mut b = match build(&data, &o, party, mode, s) { Ok(b) => b, Err(e) => { eprintln!("{e}"); std::process::exit(2); } };
                    let out = run(&mut b, o.ticks, 0);
                    let r = summarize(&b, out);
                    println!("{:<9} {:<10} {:<4} {:<8} {:>6} {:>2}/{:<2} {:>9} {:>22}", s, party, format!("{mode:?}"), format!("{:?}", r.outcome),
                        r.ticks / 10, r.dead, r.party, r.taken, format!("{} ({})", r.worst_engaged.0, r.worst_engaged.1));
                }
            }
        }
        return;
    }

    for s in scenarios {
        let mut b = match build(&data, &o, &o.party, o.mode, s) { Ok(b) => b, Err(e) => { eprintln!("{e}"); std::process::exit(2); } };
        println!("== {s} · party {} · mode {:?} · cap {} · {} ==", o.party, o.mode, o.cap, describe(&o));
        print!("{}", b.render());
        let out = run(&mut b, o.ticks, o.frames);
        let r = summarize(&b, out);
        println!("{:?} after {}.{} s · party down {}/{} · teleports {} (no-fire {}) · surfaced {}",
            r.outcome, r.ticks / 10, r.ticks % 10, r.dead, r.party, r.teleports, r.fizzles, r.surfaced);
        println!("  {:<18} {:>11} {:>9} {:>9} {:>10}", "unit", "hp", "dealt", "taken", "max_swarm");
        for u in b.units.iter().filter(|u| u.team == Team::Party) {
            println!("  {:<18} {:>5}/{:<5} {:>9} {:>9} {:>10}", u.name, u.hp.max(0), u.stats.hp, u.dealt, u.taken, u.max_engaged);
        }
        let casts = |skill: &str| b.events.iter().filter(|(_, e)| matches!(e, Event::Cast { skill: s, .. } if *s == skill)).count();
        if !b.skill_dmg.is_empty() {
            println!("  lane skills: {}", b.skill_dmg.iter().map(|(k, v)| format!("{k} ×{} {v}", casts(k))).collect::<Vec<_>>().join(" · "));
        }
        print!("{}", b.render());
        println!();
    }
}

/// `--stage N`: N full stages (7–10 waves + boss), one seed each; the first is printed wave by wave.
fn run_stages(data: &GameData, o: &Opts) {
    println!("== stage {} · party {} · mode {:?} · {} · {} runs ==", o.bench.stage, o.party, o.mode, describe(o), o.stage_runs);
    let (mut cleared, mut total, mut boss) = (0u64, 0u64, 0u64);
    for seed in 0..o.stage_runs {
        let mut b = match place_party(data, o, &o.party, o.mode) { Ok(b) => b, Err(e) => { eprintln!("{e}"); std::process::exit(2); } };
        let mut rolls = lane_sim::stage::Rolls::new(seed);
        let r = match lane_sim::stage::run(data, &o.bench, &mut b, &mut rolls) { Ok(r) => r, Err(e) => { eprintln!("{e}"); std::process::exit(2); } };
        if seed == 0 {
            for (k, w) in r.waves.iter().enumerate() {
                println!("  wave {:>2}: {} mobs · walk {:>4.1}s · fight {:>5.1}s · party HP {:>3}%",
                    k + 1, w.monsters, w.walk_ticks as f64 / 10.0, w.fight_ticks as f64 / 10.0, w.party_hp_bp / 100);
            }
            if r.boss_ticks > 0 {
                println!("  boss + {} minions: {:.1}s", r.boss_minions, r.boss_ticks as f64 / 10.0);
            }
        }
        println!("  run {seed}: {} · {} waves · {:.1}s total (boss {:.1}s) · party down {}",
            if r.cleared { "CLEARED" } else { "FAILED" }, r.waves.len(), r.total_ticks as f64 / 10.0, r.boss_ticks as f64 / 10.0, r.dead);
        cleared += u64::from(r.cleared);
        total += r.total_ticks;
        boss += r.boss_ticks;
    }
    println!("cleared {cleared}/{} · avg stage {:.1}s · avg boss {:.1}s", o.stage_runs,
        total as f64 / 10.0 / o.stage_runs as f64, boss as f64 / 10.0 / o.stage_runs as f64);
}

/// `--ladder DIR`: every `stage_<N>.json` a live `sim-cli --snapshots` run wrote, played on the
/// LIVE sim in lane mode (`sim_core::lane`). "live h" = when the live run first reached it.
fn run_ladder(data: &GameData, o: &Opts, dir: &str) {
    let snaps = match lane_sim::ladder::load(dir) { Ok(s) => s, Err(e) => { eprintln!("{e}"); std::process::exit(2); } };
    println!("stage  live_h  lane: result   secs  down   party");
    for snap in &snaps {
        let r = lane_sim::ladder::load_state(dir, snap.stage)
            .and_then(|st| lane_sim::ladder::live_stage(data, st, snap.stage, o.cap as u8, 36_000));
        match r {
            Ok(r) => println!("{:>5}  {:>6.1}  {:>12}  {:>5.0}  {:>4}   {}", snap.stage, snap.tick as f64 / 36_000.0,
                if r.cleared { "CLEARED" } else if r.dead > 0 { "WIPED" } else { "TIMEOUT" }, r.ticks as f64 / 10.0, r.dead, lane_sim::ladder::party_label(snap)),
            Err(e) => println!("{:>5}  error: {e}", snap.stage),
        }
    }
}

/// `--ladder DIR --reach`: for every 10th snapshot, the highest stage its party clears in lane mode
/// on the live sim, climbing 5 stages at a time.
fn run_reach(data: &GameData, o: &Opts, dir: &str) {
    let snaps = match lane_sim::ladder::load(dir) { Ok(s) => s, Err(e) => { eprintln!("{e}"); std::process::exit(2); } };
    println!("live_stage  live_h  lane_reach   party");
    for snap in snaps.iter().filter(|s| s.stage % 10 == 1) {
        let clears = |stage: u32| lane_sim::ladder::load_state(dir, snap.stage)
            .and_then(|st| lane_sim::ladder::live_stage(data, st, stage, o.cap as u8, 36_000))
            .is_ok_and(|r| r.cleared);
        let mut best = 0;
        let mut st = snap.stage;
        while st <= 240 && clears(st) {
            best = st;
            st += 5;
        }
        println!("{:>10}  {:>6.1}  {:>10}   {}", snap.stage, snap.tick as f64 / 36_000.0, best, lane_sim::ladder::party_label(snap));
    }
}
