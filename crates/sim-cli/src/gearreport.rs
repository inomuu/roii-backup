// GEAR PACING REPORT — the measurement the grade tables are tuned against.
//
// The CSV curve answers "how fast does the party level"; it says nothing about the question the
// drop tables actually pose: at this band, how many pieces of each GRADE does an hour of farming
// pay, and how long does filling a party's 55 slots take? Tuning those numbers by reading the
// table is guesswork — the odds interact with kill rate, the gear rule's bp and the level-gap
// penalty, and only a run knows all three.
//
// Pair it with `--data <copy of game_data.json>`: the odds live in the catalog (loot_rules.gear
// .grade_odds), so a sweep is "edit a copy, re-run, compare" with no rebuild and no digest bump
// on the real file.
//
// MODEL: the report VENDORS every piece of gear the moment it lands (counting it first). A real
// player does not, but the 20-slot gear cap otherwise clogs within a band and the run stops
// measuring the drop rate and starts measuring the cap instead — which is a different question.

use sim_core::data::{self, GameData};
use sim_core::items::{Grade, ItemKind, GRADES};
use sim_core::state::GameState;
use sim_core::tick;

use crate::build::{auto_allocate_soul_paths, auto_allocate_stat_points, auto_equip_best, auto_fuse_gems, auto_fuse_surplus, auto_party, auto_potential, auto_socket_gems, force_power};
use crate::Opts;

/// Ticks per simulated hour: the sim runs at 10 ticks/second.
const TICKS_PER_HOUR: u64 = 36_000;
/// Slots a full party wears: 5 members × 11 pieces (design §D11 — pacing is computed against the
/// PARTY, not one character, which is what makes six-into-one fusion survivable).
const PARTY_SLOTS: u64 = 55;
/// Stamp marking a piece this report has already counted. `instance_id` is the market-identity
/// field, unused on un-listed gear, and this tool never lists anything — borrowing it keeps the
/// bookkeeping out of the sim.
const COUNTED: &str = "roi-gear-report";

/// The report's label for a line, and its value; `None` for a line the tally does not show.
fn tally_key(m: &sim_core::items::StatMod) -> Option<(&'static str, i32)> {
    use sim_core::items::StatMod as S;
    Some(match *m {
        S::AtkFlat(v) => ("ATK flat", v), S::AtkBp(v) => ("ATK%", v),
        S::CritBp(v) => ("Crit%", v), S::CritDmgBp(v) => ("Crit DMG%", v),
        S::CooldownBp(v) => ("CDR%", v), S::MaxHp(v) => ("HP flat", v),
        S::MaxHpBp(v) => ("HP%", v), S::DefFlat(v) => ("DEF flat", v),
        S::MoveSpeedBp(v) => ("Move%", v), S::DodgeBp(v) => ("Dodge", v),
        S::BreakBuildupBp(v) => ("Break build-up%", v), S::BreakDmgBp(v) => ("Break DMG%", v),
        S::ChargeRateBp(v) => ("Charge%", v),
        S::DmgVsBossBp(v) => ("Boss DMG%", v),
        S::ElementDmgBp(_, v) => ("Element DMG (all)%", v),
        S::HpRegenFlat(v) => ("HP regen flat", v), S::DefBp(v) => ("DEF%", v), S::HealBp(v) => ("Heal%", v),
        S::IgnoreDefBp(v) => ("Ignore DEF%", v),
        _ => return None,
    })
}

pub(crate) fn report(opts: &Opts) -> Result<(), String> {
    let json = std::fs::read_to_string(&opts.data_path)
        .map_err(|e| format!("read {}: {e}", opts.data_path))?;
    let data = GameData::from_json_str(&json)?;
    let mut state = data::new_game_in_zone(opts.seed, &data, &opts.zone)?;
    // KIT IS FORCED ON. The report strips gear out of the bag every tick to measure the drop
    // RATE, which means `auto_equip_best` would never find anything to wear — a bare-handed party
    // at Lv70 kills ~17 monsters an hour instead of ~1,250, and the "measurement" would be of
    // nothing. Wearing a starting set is what makes the farm loop real.
    if opts.level > 0 {
        force_power(&mut state, &data, opts.level, true);
    }

    let mut by_grade = [0u64; 10];
    let mut fused = 0u32;
    // What FUSION produced, by grade — the other half of "what does the player end up holding".
    let mut made_by_grade = [0u64; 10];
    let mut kills_before = state.measures.kills.get();
    let mut kills = 0u64;
    for t in 0..opts.ticks {
        tick::tick(&mut state, 1);
        // COUNT → WEAR → CLEAR, once a second (every 10 ticks). Counting is what the report is
        // for; wearing has to happen BEFORE the clear or a natural Lv1 run never equips anything
        // and stalls bare-handed; clearing keeps the 20-slot gear cap from turning the
        // measurement into a measurement of the cap. A second of drops never fills 20 slots.
        if t % 10 == 9 {
            if std::env::var("ROI_GEAR_DEBUG").is_ok() {
                let n = state.inventory.iter()
                    .filter(|it| matches!(it.kind, ItemKind::Weapon | ItemKind::Armor) && !it.unique)
                    .count();
                if n > 3 {
                    eprintln!("[debug] tick {t}: {n} gear in bag at once");
                }
            }
            // COUNT ONCE PER PIECE. The bag now HOLDS fodder between cycles (fusion needs six in
            // hand), so counting its contents every cycle counts the same sword 3,600 times an
            // hour — the first version of this reported 1.9 million drops in 48 hours. Each piece
            // gets stamped the first time it is seen; fusion OUTPUTS are stamped too, silently,
            // because a fused item is not a drop and must not inflate the drop rate.
            for it in state.inventory.iter_mut() {
                if matches!(it.kind, ItemKind::Weapon | ItemKind::Armor)
                    && !it.unique
                    && it.instance_id.is_none()
                {
                    by_grade[it.grade.step()] += 1;
                    it.instance_id = Some(COUNTED.to_string());
                }
            }
            auto_equip_best(&mut state);
            // FUSE the surplus instead of vendoring it: six-into-one is the road the design
            // expects every spare piece to take, and a report that throws them away measures a
            // player who never uses the system the numbers were written for.
            fused += auto_fuse_surplus(&mut state, &data);
            // GEMS: fuse the held spares up, then socket what fits — after the equip pass, so a
            // gem lands on the piece the party will keep wearing.
            auto_fuse_gems(&mut state, &data);
            auto_socket_gems(&mut state, &data);
            for it in state.inventory.iter() {
                if it.instance_id.is_none()
                    && matches!(it.kind, ItemKind::Weapon | ItemKind::Armor)
                {
                    made_by_grade[it.grade.step()] += 1;
                }
            }
            for it in state.inventory.iter_mut() {
                if it.instance_id.is_none() {
                    it.instance_id = Some(COUNTED.to_string()); // a fusion output, not a drop
                }
            }
            auto_equip_best(&mut state);
            // KEEP THE FODDER, drop the rest. Fusion needs six of a grade in hand at once, so a
            // bag emptied every cycle can never assemble one — the first version of this loop
            // reported "0 fusions" for exactly that reason. Up to five per grade stay (the
            // remainder a fusion could not use), everything past that leaves, which keeps the
            // 20-slot cap from becoming the thing being measured.
            // Keep fodder up to the BAG, not to five-per-grade. Five was the first attempt and it
            // capped the ladder by accident: a grade can never assemble its six, so the party
            // could only ever climb one step per grade per cycle. The bag is the real constraint
            // a player has, so the model uses that — and drops the OLDEST surplus first, which is
            // what a player clearing space does.
            let cap = state.inventory_cap();
            while state.inventory.len() > cap {
                let Some(pos) = state.inventory.iter().position(|it| {
                    matches!(it.kind, ItemKind::Weapon | ItemKind::Armor) && !it.unique
                }) else { break };
                state.inventory.remove(pos);
            }
        }
        // ADVANCE + REVIVE. Both are the CSV run's own loop, and both are load-bearing here: a
        // report that skips them measures a party standing in room 1 of one zone, dead. (It was
        // 17 kills an hour before these two were added, against 1,182 for the same parameters.)
        if let Some(next_id) = state.pending_zone_change.clone() {
            data::change_zone(&mut state, &data, &next_id)?;
        }
        let alive = state.grid.entities.iter()
            .any(|e| matches!(e.team, sim_core::grid::Team::Player) && e.alive && !e.is_pet);
        if !alive {
            data::handle_party_wipe(&mut state, &data)?;
        }
        // Keep the party growing, or the measured rate decays as the band outpaces it.
        if t % 600 == 0 {
            auto_party(&mut state, &data, &opts.recruit_order, false);
            auto_allocate_stat_points(&mut state);
            auto_allocate_soul_paths(&mut state);
            auto_potential(&mut state);
            if opts.level > 0 {
                force_power(&mut state, &data, opts.level, true);
            }
        }
        let now = state.measures.kills.get();
        kills += now.saturating_sub(kills_before);
        kills_before = now;
    }

    let hours = (opts.ticks as f64) / (TICKS_PER_HOUR as f64);
    let total: u64 = by_grade.iter().sum();
    // COMMON-EQUIVALENT: fusion is six-into-one, so a grade is worth 6^step Commons. This is the
    // only unit in which "7 pieces an hour" and "55 slots of Legendary" are the same currency.
    let ce = |step: usize| -> u64 { 6u64.pow(step as u32) };
    let ce_total: u64 = GRADES.iter().enumerate().map(|(i, _)| by_grade[i] * ce(i)).sum();

    // WHERE THE RUN ACTUALLY ENDED. On a natural run (`--level` off) the party climbs, so the
    // band at the end is the one the odds should be read against — quoting the STARTING band
    // would describe a table the run spent almost none of its time in.
    let end_zone = state.current_zone_id.clone().unwrap_or_default();
    let end_level = state.grid.entities.iter()
        .filter(|e| matches!(e.team, sim_core::grid::Team::Player) && !e.is_pet)
        .map(|e| e.stats.level as u32)
        .max()
        .unwrap_or(1);
    println!("start {} → end {end_zone} (Lv{end_level})  seed {}  {:.2} sim-hours",
        opts.zone, opts.seed, hours);
    // BOSS SHARE, because it is the other half of the gear rate: the rule gives a boss ~2× a
    // field monster's chance (38 rows at 3 bp against 27 at 2), so two runs with the same kills
    // per hour can still differ by a third on gear per hour purely on what they were killing.
    let bosses = state.measures.boss_kills.get();
    let boss_share = if kills > 0 { bosses as f64 * 100.0 / kills as f64 } else { 0.0 };
    println!("kills {kills} ({:.0}/h)   gear {total} ({:.1}/h)   CE {ce_total} ({:.0}/h)",
        kills as f64 / hours, total as f64 / hours, ce_total as f64 / hours);
    println!("bosses {bosses} ({:.1}/h, {boss_share:.1}% of kills)   rooms {}",
        bosses as f64 / hours, state.measures.rooms_cleared.get());
    if std::env::var("ROI_GEAR_DEBUG").is_ok() {
        let mut bag = [0u32; 10];
        for it in &state.inventory {
            if matches!(it.kind, ItemKind::Weapon | ItemKind::Armor) {
                bag[it.grade.step()] += 1;
            }
        }
        eprintln!("[debug] gil {}  bag gear by grade {:?}  bag len {}", state.gil(), bag, state.inventory.len());
    }
    println!();
    println!("{:<10} {:>8} {:>9} {:>12} {:>14}", "grade", "count", "share", "CE each", "55 slots in");
    for (i, g) in GRADES.iter().enumerate() {
        if by_grade[i] == 0 && i > 0 && by_grade[..i].iter().sum::<u64>() == total {
            continue; // nothing at or above this grade dropped — stop the table where the data does
        }
        let share = if total > 0 { by_grade[i] as f64 * 100.0 / total as f64 } else { 0.0 };
        // How long a party takes to wear this grade in every slot, counting EVERY drop as fusion
        // fodder (the road the design expects — farming the grade directly is the lottery).
        let need = PARTY_SLOTS * ce(i);
        let per_day = ce_total as f64 / hours * 12.0; // a 12-hour play day
        let days = if per_day > 0.0 { need as f64 / per_day } else { f64::INFINITY };
        let when = if days < 1.0 { format!("{:.1} h", days * 24.0) }
                   else if days < 400.0 { format!("{days:.1} d") }
                   else { format!("{:.1} y", days / 365.0) };
        println!("{:<10} {:>8} {:>8.2}% {:>12} {:>14}", format!("{g:?}"), by_grade[i], share, ce(i), when);
    }
    // EXPECTED, from the authored table × the measured drop rate. The sampled columns above
    // cannot see the tail: Immortal is 0.25% of drops, one per ~45 hours of farming, so a
    // six-hour run reports it as zero and a reader would conclude the ladder is unreachable.
    // The odds are KNOWN — only the rate has to be measured — so the honest report states both.
    // The odds are by STAGE (0.0.399): the stage the run ended on.
    let end_stage = sim_core::state::band_of_zone(&end_zone).map(u16::from).unwrap_or(1);
    let odds = data::items_grade_odds_for(&data, end_stage);
    if !odds.is_empty() {
        let per_drop_ce: f64 = odds.iter().enumerate()
            .map(|(i, bp)| *bp as f64 / 10_000.0 * ce(i) as f64)
            .sum();
        let gear_per_h = total as f64 / hours;
        let ce_per_h = per_drop_ce * gear_per_h;
        println!();
        println!("EXPECTED from the authored odds × the measured {gear_per_h:.1} gear/h:");
        println!("  {per_drop_ce:.1} CE per drop → {ce_per_h:.0} CE/h → {:.0} CE/day (12 h)", ce_per_h * 12.0);
        println!("{:<10} {:>9} {:>16} {:>14}", "grade", "odds", "one every", "55 slots in");
        for (i, g) in GRADES.iter().enumerate() {
            if odds[i] == 0 {
                continue;
            }
            let per_h = gear_per_h * odds[i] as f64 / 10_000.0;
            let every = if per_h >= 1.0 { format!("{:.1} h", 1.0 / per_h) }
                        else if per_h > 0.0 { format!("{:.1} d", 1.0 / per_h / 24.0) }
                        else { "never".into() };
            let days = (PARTY_SLOTS * ce(i)) as f64 / (ce_per_h * 12.0);
            let when = if days < 1.0 { format!("{:.1} h", days * 24.0) }
                       else if days < 400.0 { format!("{days:.1} d") }
                       else { format!("{:.1} y", days / 365.0) };
            println!("{:<10} {:>8.2}% {:>16} {:>14}", format!("{g:?}"), odds[i] as f64 / 100.0, every, when);
        }
    }
    // WHAT THE PARTY IS ACTUALLY WEARING. Every number above is about supply; this is the only
    // one about POWER, and it is the one that says whether the ladder's multipliers are inflated.
    let mut worn = [0u32; 10];
    let mut atk_from_gear = 0i64;
    let mut members = 0;
    for e in state.grid.entities.iter()
        .filter(|e| matches!(e.team, sim_core::grid::Team::Player) && !e.is_pet)
    {
        members += 1;
        for it in &e.equipped {
            worn[it.grade.step()] += 1;
            atk_from_gear += it.graded_atk() as i64;
        }
    }
    let slots: u32 = worn.iter().sum();
    // SUBSTAT TOTALS as the fight sees them, per member, from the pieces actually worn — the
    // number the catalog's `substats` windows are tuned against ("a full set at the launch cap
    // must not reach Crit 40%"). Dodge is folded the way `stat_mods::dodge_bp` folds it, not
    // summed; everything else is additive in the sim. Averaged over the members so a party of
    // five reads like one character.
    let mut sub: std::collections::BTreeMap<&'static str, i64> = Default::default();
    let mut dodge_remaining: i64 = 10_000 * members.max(1) as i64;
    let mut gem_sub: std::collections::BTreeMap<&'static str, i64> = Default::default();
    let mut gem_dodge_remaining: i64 = 10_000 * members.max(1) as i64;
    for e in state
        .grid
        .entities
        .iter()
        .filter(|e| matches!(e.team, sim_core::grid::Team::Player) && !e.is_pet)
    {
        let mut member_dodge: i64 = 10_000;
        for it in &e.equipped {
            for m in &it.lines {
                let Some((k, v)) = tally_key(m) else { continue };
                if k == "Dodge" {
                    member_dodge = member_dodge * (10_000 - v.clamp(0, 10_000) as i64) / 10_000;
                    continue;
                }
                *sub.entry(k).or_default() += v as i64;
            }
        }
        dodge_remaining -= 10_000 - member_dodge;
        // GEMS: the fight's own fold (summed per stat, capped), once per member.
        let mut member_gem_dodge: i64 = 10_000;
        sim_core::gems::for_each_gem_mod(e, &mut |m| {
            let Some((k, v)) = tally_key(m) else { return };
            if k == "Dodge" {
                member_gem_dodge = member_gem_dodge * (10_000 - v.clamp(0, 10_000) as i64) / 10_000;
                return;
            }
            *gem_sub.entry(k).or_default() += v as i64;
        });
        gem_dodge_remaining -= 10_000 - member_gem_dodge;
    }
    println!();
    println!("SUBSTATS worn, average per member (from the lines on the pieces actually equipped):");
    let m = members.max(1) as i64;
    for (k, v) in &sub {
        let per = v / m;
        if k.ends_with('%') {
            println!("  {:<16} +{}.{:02}%", k, per / 100, (per % 100).abs());
        } else {
            println!("  {:<16} +{per}", k);
        }
    }
    let dodge_avg_bp = 10_000 - dodge_remaining / m;
    if dodge_avg_bp > 0 {
        println!("  {:<16} {}.{:02}% (folded)", "Dodge", dodge_avg_bp / 100, dodge_avg_bp % 100);
    }
    println!();
    // GEMS: what the party holds, what it socketed, and what the socketed gems add per member.
    let mut held_by_grade = [0u32; 10];
    for (id, n) in state.materials() {
        if let Some((_, g)) = sim_core::gems::set_of(&data, id) {
            held_by_grade[g.step()] += n;
        }
    }
    let mut socketed_by_grade = [0u32; 10];
    let mut sockets_open = 0u32;
    for e in state.grid.entities.iter().filter(|e| matches!(e.team, sim_core::grid::Team::Player) && !e.is_pet) {
        for it in &e.equipped {
            sockets_open += u32::from(it.gem_slots()) - it.gems.len() as u32;
            for g in &it.gems {
                if let Some((_, gr)) = sim_core::gems::set_of(&data, &g.def_id) {
                    socketed_by_grade[gr.step()] += 1;
                }
            }
        }
    }
    let fmt_grades = |row: &[u32; 10]| -> String {
        let s: Vec<String> = GRADES.iter().enumerate().filter(|(i, _)| row[*i] > 0).map(|(i, g)| format!("{g:?} ×{}", row[i])).collect();
        if s.is_empty() { "nothing".into() } else { s.join("  ") }
    };
    println!("GEMS ({} socketed over the run, {} gem fusions ({} failed), {sockets_open} sockets still empty on worn gear):",
        state.measures.gems_socketed.get(), state.measures.gem_fusions.get(), state.measures.gem_fusions_failed.get());
    println!("  held in the ledger: {}", fmt_grades(&held_by_grade));
    println!("  socketed on worn gear: {}", fmt_grades(&socketed_by_grade));
    if !gem_sub.is_empty() || gem_dodge_remaining < 10_000 * m {
        println!("  gem lines, average per member (the fight's fold — summed per stat, capped):");
        for (k, v) in &gem_sub {
            let per = v / m;
            if k.ends_with('%') {
                println!("    {:<16} +{}.{:02}%", k, per / 100, (per % 100).abs());
            } else {
                println!("    {:<16} +{per}", k);
            }
        }
        let gd = 10_000 - gem_dodge_remaining / m;
        if gd > 0 {
            println!("    {:<16} {}.{:02}% (folded)", "Dodge", gd / 100, gd % 100);
        }
    }
    println!();
    println!("WORN by the party at the end ({members} members, {slots} filled slots, {fused} fusions run, {} failed):", state.measures.fusions_failed.get());
    let mut line = String::new();
    for (i, g) in GRADES.iter().enumerate() {
        if worn[i] > 0 {
            line.push_str(&format!("{g:?} ×{}  ", worn[i]));
        }
    }
    println!("  {}", if line.is_empty() { "nothing".into() } else { line });
    println!("  gear ATK across the party: {atk_from_gear}");
    let mut line2 = String::new();
    for (i, g) in GRADES.iter().enumerate() {
        if made_by_grade[i] > 0 {
            line2.push_str(&format!("{g:?} ×{}  ", made_by_grade[i]));
        }
    }
    println!("  FUSED into existence: {}", if line2.is_empty() { "nothing".into() } else { line2 });
    // TOTAL OWNED per grade = dropped + fused. This is the number a player would describe as
    // "how much good gear do I have", and the one a drop-rate tune is aimed at.
    println!("  owned (dropped + fused), by grade:");
    for (i, g) in GRADES.iter().enumerate() {
        let n = by_grade[i] + made_by_grade[i];
        if n > 0 {
            println!("    {:<10} {}", format!("{g:?}"), n);
        }
    }
    println!();
    println!("55 slots = a party of 5 wearing 11 pieces each; days assume a 12-hour play day and");
    println!("that every drop is fused upward. Sweep with --data <copy>, then bump the catalog once.");
    let _ = Grade::Common; // keeps the import honest if the table above is ever reshaped
    let _: &GameState = &state;
    Ok(())
}
