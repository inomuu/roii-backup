// sim-cli — headless simulation runner.
//
// Loads `data/game_data.json`, runs the sim for N ticks, and emits a
// CSV progression curve. Used to tune balance without ever opening
// Godot.
//
// Phase 0 minimal CLI — positional-ish flag parsing, no clap.
// Defaults are tuned for "run a 10-minute game session" out of the box.

mod build;
mod onboarding;
/// The server's gacha core, compiled in as-is so a run pulls at the live rates, pity and SR
/// guarantee rather than a copy that drifts. Its own tests name `crate::GameData`.
#[path = "../../verify-server/src/gacha.rs"]
#[allow(dead_code)]
mod gacha;
#[cfg(test)]
use sim_core::data::GameData;
mod run;
mod gearreport;
mod roster_export;
mod statdump;

use std::env;
use std::process::ExitCode;

use crate::run::run;

fn main() -> ExitCode {
    match parse_args() {
        Ok(opts) if opts.dump_strings => {
            dump_strings();
            ExitCode::SUCCESS
        }
        Ok(opts) if opts.substat_table => match substat_table(&opts) {
            Ok(()) => ExitCode::SUCCESS,
            Err(e) => {
                eprintln!("error: {e}");
                ExitCode::FAILURE
            }
        },
        Ok(opts) if opts.gear_report => match gearreport::report(&opts) {
            Ok(()) => ExitCode::SUCCESS,
            Err(e) => {
                eprintln!("error: {e}");
                ExitCode::FAILURE
            }
        },
        Ok(opts) if opts.roster_web.is_some() => match roster_web(&opts) {
            Ok(()) => ExitCode::SUCCESS,
            Err(e) => {
                eprintln!("error: {e}");
                ExitCode::FAILURE
            }
        },
        Ok(opts) if opts.dump_stats > 0 => match dump_stats(&opts) {
            Ok(()) => ExitCode::SUCCESS,
            Err(e) => {
                eprintln!("error: {e}");
                ExitCode::FAILURE
            }
        },
        Ok(opts) => match run(opts) {
            Ok(()) => ExitCode::SUCCESS,
            Err(e) => {
                eprintln!("error: {e}");
                ExitCode::FAILURE
            }
        },
        Err(msg) => {
            print_usage();
            if msg != "(help)" {
                eprintln!("\n{msg}");
            }
            ExitCode::from(2)
        }
    }
}

fn print_usage() {
    eprintln!(
        "sim-cli v{} — headless balance runner

usage: sim-cli [options]

  --seed N         PRNG seed (default 42)
  --zone ID        starting zone id (default zone_b1_1)
  --ticks N        how many sim ticks to run (default 6000 = 10 min)
  --sample N       emit a CSV row every N ticks (default 50 = 5 sec)
  --gear-report    measure GEAR PACING instead of the progression curve: how many pieces
                   of each GRADE an hour of farming pays, in Common-equivalents, and how
                   long a party's 55 slots take to fill. Pair with --level (a band's real
                   odds) and --data (a tuning copy of the catalog).
  --substat-table  print every gear def × grade with its SUBSTAT lines as CSV (to --out or
                   stdout) — the generated table the catalog's `loot_rules.substats` block
                   resolves to. Diff it after a tuning edit; it is what the player sees.
  --data PATH      path to game_data.json (default data/game_data.json)
  --out PATH       CSV output path (default stdout)
  --level N        START the party at base level N and KEEP it there (tops up every
                   member, including later recruits) + grants Gil so the recruit
                   slots open immediately. The ONLY way to measure late content:
                   without it a run always begins at Lv1, so a Lv46-50 dungeon can
                   never be reached in a headless run. sim-cli-only cheat.
  --kit            max each member's skills and hand them a gear set
                   (the debug helpers the game exposes in its own debug view), so
                   --level measures an EQUIPPED party rather than a naked one.
  --card-rate-mult N  card drop-rate multiplier for testing/events
                   (default 1; e.g. 100 = cards drop 100x — recorded as a
                   SetCardRateBonus input so runs stay replayable)
  --dump-strings   print skill display strings as `key,en` CSV and exit
                   (consumed by tools/loc_extract.py)
  --dump-stats N   print effective combat stats per class for base levels 1..N as
                   CSV and exit — the measurement the new flat stat model is fitted
                   to (docs/skill-system-redesign.md §4.5.1.2). Reads only; no gear.
  --bench L:T:G:S  STANDARD DPS BENCH: party at level L, gear tier ≤ T at grade G
                   (name or 0-9), all five fielded at tick 0, held on stage S; one sim
                   hour by default (--ticks/--sample still apply). Read the MEMBERS
                   table's dmg/alive-h. e.g. --bench 70:6:legendary:100 --regalia sig:60:0
  --dummy          monsters are training dummies (huge HP, 0 ATK): pure damage output,
                   nothing dies, nothing kills. Pair with --bench / --lead for a solo read.
                   Without --dummies the room fills to the 20-monster cap, which flatters AoE.
  --dummies N      --dummy with exactly N dummies standing (1 = boss, 3/5/7 = a room)
  --gacha-flow S   grow the party by the draft onboarding with REAL newcomer-banner pulls
                   (seed S): slot 2 = Lumi at stage 2, slots 3/4/5 = 10 pulls each at stage
                   6/18/24 (Starter rule: an SR is a new SR character); quest 1 pays a Rare
                   weapon at the first boss
  --no-quest-weapon  with --gacha-flow: skip that weapon
  --stars N        every fielded Exemplar unit at Resonance rank N (0-6): S1-S6 mechanics
  --stars-for U:N  ONE unit at rank N (repeatable) — read a ladder as the PARTY's delta
                   (unit_rank) and the S3/S5 level ranks
  --regalia M:L:S  every member wears its doctrine's SSR (`sig`), R (`r`), or ONE named piece (an id, e.g. `aurora_edge`) at
                   level L, Resonance S, from the tick it is fielded
  --help, -h       show this message",
        sim_core::VERSION
    );
}

pub(crate) struct Opts {
    pub seed: u64,
    pub zone: String,
    pub ticks: u64,
    pub sample: u64,
    pub data_path: String,
    pub out: Option<String>,
    /// Ids fielded (in order) as the party's slots open: EXEMPLAR ids arrive the way a pull does
    /// (the default), class ids field the old kit-less harness. Override with --recruit.
    pub recruit_order: Vec<String>,
    /// The unit fielded at genesis instead of the real starter (Seraphine) — A/B only.
    pub lead: Option<String>,
    /// `--regalia sig|r[:level[:stars]]`: every member wears its SIGNATURE 5★ piece (`sig`) or
    /// the R rung of its class line (`r`) at that level / Resonance, from the moment it is
    /// fielded. None = no Regalia at all (the measurement before 0.0.435).
    pub regalia: Option<(String, u8, u8)>,
    /// `--bench level:tier:grade:stage` — a STANDARD party to read DPS off, fast: every member at
    /// `level`, wearing the best gear of tier ≤ `tier` at `grade` (a name or a 0-9 step), all
    /// five fielded from tick 0, held on `stage` (room 3 wraps back to room 1). Defaults ticks to
    /// one sim hour (36,000) and sample to a minute. Combine with --regalia.
    pub bench: Option<(u32, u32, u8, u32)>,
    /// `--stars N`: every fielded Exemplar unit at Resonance N (0-6) — the ranks bench.
    pub stars: Option<u8>,
    /// `--dummy`: every monster becomes a training dummy — a huge HP pool and 0 ATK — so the
    /// MEMBERS damage is raw output, never capped by kills, overkill or dying. Pair with --bench.
    pub dummy: bool,
    /// `--dummies N`: dummy mode with exactly N dummies standing (1 = a boss, 3/5/7 = a room).
    pub dummies: Option<u32>,
    /// `--snapshots DIR`: write the whole state as `DIR/stage_<N>.json` the first time the run
    /// enters stage N (lane-sim's ladder sweep reads them). Read-only on the run.
    pub snapshots: Option<String>,
    /// `--lane [CAP]`: run in LANE MODE (`sim_core::lane`), monsters per cell = CAP (default 2).
    pub lane: Option<u8>,
    /// `--lane-speed BP`: lane-mode walking speed, bp of live (default: the lane mode's own).
    pub lane_speed_bp: Option<u32>,
    /// `--lane-groups N`: lane-mode groups per room (default: the lane mode's own).
    pub lane_groups: Option<u8>,
    /// `--gacha-flow SEED`: grow the party by the DRAFT onboarding (`onboarding.rs`) — slots
    /// bought with Gil, each paying its fixed unit or its pulls on the real newcomer banner —
    /// instead of `--recruit`'s "next unit arrives". SEED drives the pulls only.
    pub gacha_flow: Option<u64>,
    /// `--no-quest-weapon`: with --gacha-flow, skip tutorial quest 1's Rare weapon (a player who
    /// never equips it).
    pub no_quest_weapon: bool,
    /// Card drop-rate multiplier (×N) — 1 = off. Applied as a recorded
    /// `SetCardRateBonus` input at tick 0.
    pub card_rate_mult: u32,
    /// Base level to hold every party member at (0 = off — normal Lv1 start).
    /// Lets a headless run measure content the natural curve would take days to
    /// reach (a Lv46-50 dungeon). sim-cli is a trusted balance tool, so this
    /// pokes the debug helpers directly and is never part of a replayed log.
    pub level: u32,
    /// With `--level`: also max skills and equip a gear set per member.
    pub kit: bool,
    /// Dump effective stats per class for base levels 1..=N and exit (0 = off).
    /// Read-only measurement used to fit the new flat stat model to the balance
    /// the game already has.
    pub dump_stats: u32,
    /// `--roster-web DIR`: write DIR/roster.js for docs/roster/index.html (the code-generated
    /// roster reference) and exit.
    pub roster_web: Option<String>,
    /// `--loc PATH`: the strings.csv the roster export reads its EN/TH text from.
    pub loc_path: String,
    /// `--stars-for unit:N`: raise ONE unit's Resonance, so a ladder can be measured as the
    /// PARTY's delta (0.0.466) rather than as that member's own damage.
    pub stars_for: Vec<(String, u8)>,
    /// Measure GEAR PACING instead of the progression curve: per-grade drop counts, CE/hour and
    /// how long 55 party slots take to fill. The report the grade odds are tuned against.
    pub gear_report: bool,
    /// Print the resolved substat table (def × grade → lines) as CSV and exit.
    pub substat_table: bool,
    /// Print every skill display string as `key,en` CSV rows and exit.
    /// Skill names/blurbs live in the Rust catalog (not game_data.json), so
    /// tools/loc_extract.py shells out to us for them (docs/localization-plan.md).
    pub dump_strings: bool,
}

fn dump_strings() {
    fn csv(field: &str) -> String {
        if field.contains(',') || field.contains('"') {
            format!("\"{}\"", field.replace('"', "\"\""))
        } else {
            field.to_string()
        }
    }
    for (id, name, blurb, detail) in sim_core::skills::all_skill_display() {
        if !name.is_empty() {
            println!("SKILL_{id},{}", csv(name));
        }
        if !blurb.is_empty() {
            println!("SKILL_{id}_B,{}", csv(blurb));
        }
        if !detail.is_empty() {
            println!("SKILL_{id}_D,{}", csv(detail));
        }
    }
}

fn roster_web(opts: &Opts) -> Result<(), String> {
    let json = std::fs::read_to_string(&opts.data_path)
        .map_err(|e| format!("{}: {e}", opts.data_path))?;
    let data = sim_core::data::GameData::from_json_str(&json)?;
    let dir = opts.roster_web.as_deref().unwrap_or("docs/roster");
    roster_export::export(&data, &opts.data_path, &opts.zone, opts.seed, &opts.loc_path, dir)
}

fn dump_stats(opts: &Opts) -> Result<(), String> {
    let json = std::fs::read_to_string(&opts.data_path)
        .map_err(|e| format!("{}: {e}", opts.data_path))?;
    let data = sim_core::data::GameData::from_json_str(&json)?;
    statdump::dump(&data, &opts.zone, opts.seed, opts.dump_stats)
}

fn parse_args() -> Result<Opts, String> {
    let mut opts = Opts {
        seed: 42,
        zone: "zone_b1_1".to_string(),
        ticks: 6000,
        sample: 50,
        data_path: "data/game_data.json".to_string(),
        out: None,
        // The party a real player holds (0.0.415): Seraphine from genesis, then pulls — a
        // Fire caster, the Holy healer, a Shadow striker and an archer. Every one is a fixed
        // kit; a plain class never enters a real party any more.
        recruit_order: ["lapis", "lucielle", "raplus", "aveline"]
            .iter()
            .map(|s| s.to_string())
            .collect(),
        lead: None,
        regalia: None,
        bench: None,
        stars: None,
        dummy: false,
        dummies: None,
        snapshots: None,
        lane: None,
        lane_speed_bp: None,
        lane_groups: None,
        gacha_flow: None,
        no_quest_weapon: false,
        card_rate_mult: 1,
        level: 0,
        kit: false,
        dump_stats: 0,
        roster_web: None,
        loc_path: "godot/i18n/strings.csv".to_string(),
        stars_for: Vec::new(),
        gear_report: false,
        substat_table: false,
        dump_strings: false,
    };
    let args: Vec<String> = env::args().skip(1).collect();
    let mut i = 0;
    while i < args.len() {
        let a = &args[i];
        match a.as_str() {
            "--seed" => {
                opts.seed = require_arg(&args, i, "--seed")?
                    .parse()
                    .map_err(|_| "--seed: not a u64")?;
                i += 2;
            }
            "--zone" => {
                opts.zone = require_arg(&args, i, "--zone")?.to_string();
                i += 2;
            }
            "--ticks" => {
                opts.ticks = require_arg(&args, i, "--ticks")?
                    .parse()
                    .map_err(|_| "--ticks: not a u64")?;
                i += 2;
            }
            "--sample" => {
                opts.sample = require_arg(&args, i, "--sample")?
                    .parse()
                    .map_err(|_| "--sample: not a u64")?;
                if opts.sample == 0 {
                    return Err("--sample must be > 0".to_string());
                }
                i += 2;
            }
            "--level" => {
                opts.level = require_arg(&args, i, "--level")?
                    .parse()
                    .map_err(|_| "--level: not a u32")?;
                i += 2;
            }
            "--kit" => {
                opts.kit = true;
                i += 1;
            }
            "--data" => {
                opts.data_path = require_arg(&args, i, "--data")?.to_string();
                i += 2;
            }
            "--lead" => {
                opts.lead = Some(require_arg(&args, i, "--lead")?.to_string());
                i += 2;
            }
            "--bench" => {
                let spec = require_arg(&args, i, "--bench")?;
                let mut parts = spec.split(':');
                let level: u32 = parts.next().and_then(|v| v.parse().ok()).unwrap_or(70);
                let tier: u32 = parts.next().and_then(|v| v.parse().ok()).unwrap_or(6);
                let grade_s = parts.next().unwrap_or("legendary");
                let grade: u8 = grade_s.parse().ok().unwrap_or_else(|| match grade_s.to_ascii_lowercase().as_str() {
                    "common" => 0, "uncommon" => 1, "rare" => 2, "legendary" => 3, "immortal" => 4,
                    "arcana" => 5, "beyond" => 6, "celestial" => 7, "divine" => 8, "cosmic" => 9, _ => 3,
                });
                let stage: u32 = parts.next().and_then(|v| v.parse().ok()).unwrap_or(100);
                opts.bench = Some((level, tier, grade, stage));
                opts.level = level;
                opts.zone = format!("zone_b{stage}_1");
                if !args.iter().any(|a| a == "--ticks") { opts.ticks = 36_000; }
                if !args.iter().any(|a| a == "--sample") { opts.sample = 600; }
                i += 2;
            }
            "--dummy" => {
                opts.dummy = true;
                i += 1;
            }
            "--no-quest-weapon" => {
                opts.no_quest_weapon = true;
                i += 1;
            }
            "--gacha-flow" => {
                opts.gacha_flow = Some(require_arg(&args, i, "--gacha-flow")?.parse().map_err(|e| format!("--gacha-flow: {e}"))?);
                i += 2;
            }
            "--lane" => {
                let cap = args.get(i + 1).and_then(|v| v.parse::<u8>().ok());
                opts.lane = Some(cap.unwrap_or(2));
                i += if cap.is_some() { 2 } else { 1 };
            }
            "--lane-groups" => {
                opts.lane_groups = Some(require_arg(&args, i, "--lane-groups")?.parse().map_err(|e| format!("--lane-groups: {e}"))?);
                i += 2;
            }
            "--lane-speed" => {
                opts.lane_speed_bp = Some(require_arg(&args, i, "--lane-speed")?.parse().map_err(|e| format!("--lane-speed: {e}"))?);
                i += 2;
            }
            "--snapshots" => {
                opts.snapshots = Some(require_arg(&args, i, "--snapshots")?.to_string());
                i += 2;
            }
            "--dummies" => {
                opts.dummy = true;
                opts.dummies = Some(require_arg(&args, i, "--dummies")?.parse().map_err(|e| format!("--dummies: {e}"))?);
                i += 2;
            }
            "--stars" => {
                opts.stars = Some(require_arg(&args, i, "--stars")?.parse().map_err(|e| format!("--stars: {e}"))?);
                i += 2;
            }
            "--regalia" => {
                let spec = require_arg(&args, i, "--regalia")?;
                let mut parts = spec.split(':');
                let mode = parts.next().unwrap_or("sig").to_string();
                let level: u8 = parts.next().and_then(|v| v.parse().ok()).unwrap_or(1);
                let stars: u8 = parts.next().and_then(|v| v.parse().ok()).unwrap_or(0);
                opts.regalia = Some((mode, level, stars));
                i += 2;
            }
            "--recruit" => {
                opts.recruit_order = require_arg(&args, i, "--recruit")?
                    .split(',')
                    .map(|s| s.trim().to_string())
                    .collect();
                i += 2;
            }
            "--card-rate-mult" => {
                opts.card_rate_mult = require_arg(&args, i, "--card-rate-mult")?
                    .parse()
                    .map_err(|_| "--card-rate-mult: not a u32")?;
                i += 2;
            }
            "--dump-strings" => {
                opts.dump_strings = true;
                i += 1;
            }
            "--gear-report" => {
                opts.gear_report = true;
                i += 1;
            }
            "--substat-table" => {
                opts.substat_table = true;
                i += 1;
            }
            "--roster-web" => {
                opts.roster_web = Some(require_arg(&args, i, "--roster-web")?.to_string());
                i += 2;
            }
            "--stars-for" => {
                let v = require_arg(&args, i, "--stars-for")?;
                let (u, n) = v.split_once(':').ok_or("--stars-for wants unit:N")?;
                let n: u8 = n.parse().map_err(|_| format!("bad --stars-for: {v}"))?;
                opts.stars_for.push((u.to_string(), n));
                i += 2;
            }
            "--loc" => {
                opts.loc_path = require_arg(&args, i, "--loc")?.to_string();
                i += 2;
            }
            "--dump-stats" => {
                let v = args.get(i + 1).ok_or("--dump-stats needs a max level")?;
                opts.dump_stats = v.parse().map_err(|_| format!("bad --dump-stats: {v}"))?;
                if opts.dump_stats == 0 {
                    return Err("--dump-stats needs a max level above 0".to_string());
                }
                i += 2;
            }
            "--out" => {
                opts.out = Some(require_arg(&args, i, "--out")?.to_string());
                i += 2;
            }
            "--help" | "-h" => return Err("(help)".to_string()),
            other => return Err(format!("unknown arg: {other}")),
        }
    }
    // --bench fields all five members at tick 0, so --gacha-flow's slot timing would be noise.
    if opts.bench.is_some() && opts.gacha_flow.is_some() {
        return Err("--gacha-flow cannot be combined with --bench (the bench opens every slot at tick 0)".to_string());
    }
    Ok(opts)
}

fn require_arg<'a>(args: &'a [String], i: usize, flag: &str) -> Result<&'a str, String> {
    args.get(i + 1)
        .map(|s| s.as_str())
        .ok_or_else(|| format!("{flag} requires a value"))
}

/// `--substat-table`: the catalog's `loot_rules.substats` block, RESOLVED — one row per gear def
/// per grade with the lines a copy of that product carries. This is the table TBH ships as a
/// datamine (`docs/references/tbh-datamine/gear_lines.csv`) and the one to diff after an edit to
/// the JSON block: the rules are compact, the table is what a player reads on the item.
fn substat_table(opts: &Opts) -> Result<(), String> {
    use std::io::Write;
    let json = std::fs::read_to_string(&opts.data_path).map_err(|e| format!("{}: {e}", opts.data_path))?;
    let data = sim_core::data::GameData::from_json_str(&json)?;
    let rules = &data.loot_rules.substats;
    let mut out = String::from("def_id,num,slot,tier,grade_step,grade,line1,line2,line3,line4\n");
    for def in data.items.iter().filter(|i| {
        matches!(i.kind, sim_core::items::ItemKind::Weapon | sim_core::items::ItemKind::Armor) && !i.unique
    }) {
        let it = sim_core::data::instantiate_item(def);
        let slot = it.slot();
        for g in sim_core::items::GRADES {
            let lines = sim_core::substats::lines_for_product(rules, &def.id, slot, g);
            let mut cells: Vec<String> = lines.iter().map(line_text).collect();
            cells.resize(4, String::new());
            out.push_str(&format!(
                "{},{},{:?},{},{},{:?},{}\n",
                def.id, def.num, slot, sim_core::gear_roll::def_tier(&def.id), g.step(), g, cells.join(",")
            ));
        }
    }
    match opts.out.as_deref() {
        Some(p) => std::fs::write(p, out).map_err(|e| format!("write {p}: {e}")),
        None => std::io::stdout().write_all(out.as_bytes()).map_err(|e| e.to_string()),
    }
}

/// A line as text, in the same shape the client prints (`sim-host::stat_mod_text`) — a flat
/// number as `+N STAT`, a basis-point one as `+N.NN% STAT`.
fn line_text(m: &sim_core::items::StatMod) -> String {
    use sim_core::items::StatMod as S;
    let pct = |v: i32| format!("{}{}.{:02}%", if v < 0 { "-" } else { "+" }, v.abs() / 100, v.abs() % 100);
    match *m {
        S::AtkFlat(v) => format!("{v:+} ATK"),
        S::DefFlat(v) => format!("{v:+} DEF"),
        S::MaxHp(v) => format!("{v:+} HP"),
        S::AtkBp(v) => format!("{} ATK", pct(v)),
        S::MaxHpBp(v) => format!("{} HP", pct(v)),
        S::CritBp(v) => format!("{} Crit", pct(v)),
        S::CritDmgBp(v) => format!("{} Crit DMG", pct(v)),
        S::CooldownBp(v) => format!("{} CDR", pct(v)),
        S::MoveSpeedBp(v) => format!("{} Move Speed", pct(v)),
        S::DodgeBp(v) => format!("{} Dodge", pct(v)),
        S::BreakBuildupBp(v) => format!("{} Break Build-up", pct(v)),
        S::BreakDmgBp(v) => format!("{} Break DMG", pct(v)),
        S::ChargeRateBp(v) => format!("{} Charge Rate", pct(v)),
        S::ElementDmgBp(el, v) => format!("{} {el:?} DMG", pct(v)),
        S::DmgVsBossBp(v) => format!("{} Boss DMG", pct(v)),
        S::HpRegenFlat(v) => format!("{v:+} HP Regen"),
        S::DefBp(v) => format!("{} DEF", pct(v)),
        S::HealBp(v) => format!("{} Heal", pct(v)),
        S::IgnoreDefBp(v) => format!("{} Ignore DEF", pct(v)),
        ref other => format!("{other:?}"),
    }
}
