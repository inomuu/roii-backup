//! `--roster-web DIR` — the roster reference, generated FROM THE CODE (user 2026-09-22: "เอกสาร
//! stat + skill ตัวละคร ที่อ้างอิงเลขจากโค้ดตรงๆ … render บนเวป สวยอ่านง่าย แต่อยู่ในเครื่อง").
//!
//! Writes `DIR/roster.js` (`window.ROSTER = {...}`) for the static `docs/roster/index.html` to
//! render from a `file://` URL — a script tag, because `fetch` of a sibling JSON is blocked
//! off-disk. Every number is read from the same tables the sim runs on: the catalog unit row,
//! `SkillDef` fields, `unit_rank::describe`, `regalia_passive::describe`, `REGALIA_LINES`, and
//! the no-gear stat curve `--dump-stats` measures. Text comes from `godot/i18n/strings.csv`
//! (EN + TH), the strings the game itself shows. Nothing here is typed by hand, so it cannot
//! drift; `docs/exemplar-roster.md` §0 is the prose companion, this is the ledger.

use serde::Serialize;
use sim_core::combat;
use sim_core::data::{self, GameData};
use sim_core::grid::Team;
use sim_core::skills::{self, BuffSpec, DebuffSpec, SkillDef, SkillKind};
use std::collections::BTreeMap;

const UNITS: &[&str] = &["seraphine", "lapis", "lucielle", "aveline", "raplus", "nemu", "lumi", "airi", "charon", "suzu", "mirei", "astier", "sylvie", "zolfia"];
const STAT_LEVELS: &[u32] = &[1, 10, 20, 30, 40, 50, 60, 70];

#[derive(Serialize)]
struct Roster {
    sim_version: &'static str,
    catalog_version: String,
    generated_from: String,
    units: Vec<Unit>,
}

#[derive(Serialize)]
struct Unit {
    id: String,
    name: String,
    rarity: &'static str,
    doctrine: String,
    element: String,
    weapon: String,
    offhand: String,
    /// The catalog row's raw base fields.
    base: BTreeMap<String, serde_json::Value>,
    /// No-gear effective stats at `STAT_LEVELS`.
    stats: Vec<StatRow>,
    skills: Vec<Skill>,
    ladder: Vec<Vec<Line>>,
    regalia: Vec<Regalia>,
}

#[derive(Serialize)]
struct StatRow {
    level: u32,
    max_hp: i32,
    atk: i32,
    hard_def: i32,
    soft_def: i32,
    aspd_milli: u32,
    crit_bp: u32,
    crit_dmg_bp: i32,
}

#[derive(Serialize)]
struct Skill {
    id: String,
    name: String,
    blurb: String,
    detail: String,
    kind: String,
    role: &'static str,
    /// Every non-default `SkillDef` field, by its Rust name, as the code holds it.
    fields: BTreeMap<&'static str, serde_json::Value>,
}

#[derive(Serialize)]
struct Line {
    key: String,
    en: String,
    th: String,
    args: Vec<i64>,
    skill: String,
}

#[derive(Serialize)]
struct Regalia {
    id: String,
    name: String,
    rarity: &'static str,
    lump_lv1: i32,
    advanced: Option<String>,
    passive_s0: Vec<Line>,
    passive_s5: Vec<Line>,
}

/// `godot/i18n/strings.csv`: key → (EN, TH). Quoted fields with commas are handled.
fn load_loc(path: &str) -> BTreeMap<String, (String, String)> {
    let mut out = BTreeMap::new();
    let Ok(text) = std::fs::read_to_string(path) else { return out };
    for line in text.lines().skip(1) {
        let cols = split_csv(line);
        if cols.len() >= 4 {
            out.insert(cols[0].clone(), (cols[2].clone(), cols[3].clone()));
        }
    }
    out
}

fn split_csv(line: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    let mut quoted = false;
    let mut chars = line.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '"' if quoted && chars.peek() == Some(&'"') => {
                cur.push('"');
                chars.next();
            }
            '"' => quoted = !quoted,
            ',' if !quoted => out.push(std::mem::take(&mut cur)),
            _ => cur.push(c),
        }
    }
    out.push(cur);
    out
}

/// `{0}` `{1}` `{2}` → the args; `{2}` is the skill NAME when the line names one.
fn fill(template: &str, args: &[i64], skill: &str) -> String {
    let mut s = template.to_string();
    for (i, a) in args.iter().enumerate() {
        s = s.replace(&format!("{{{i}}}"), &a.to_string());
    }
    if !skill.is_empty() {
        let (name, _) = skills::skill_display(skill);
        s = s.replace("{2}", if name.is_empty() { skill } else { name });
    }
    s
}

fn line(loc: &BTreeMap<String, (String, String)>, key: &str, args: &[i64], skill: &str) -> Line {
    let (en, th) = loc.get(key).cloned().unwrap_or_default();
    Line {
        key: key.to_string(),
        en: if en.is_empty() { key.to_string() } else { fill(&en, args, skill) },
        th: if th.is_empty() { String::new() } else { fill(&th, args, skill) },
        args: args.to_vec(),
        skill: skill.to_string(),
    }
}

fn buff_json(b: &BuffSpec) -> serde_json::Value {
    serde_json::json!({ "kind": format!("{:?}", b.kind), "magnitude": b.magnitude, "per_level": b.per_level, "duration_ticks": b.duration })
}

fn debuff_json(d: &DebuffSpec) -> serde_json::Value {
    serde_json::json!({ "kind": format!("{:?}", d.kind), "magnitude": d.magnitude, "per_level": d.per_level, "duration_ticks": d.duration, "chance_bp": d.chance_bp, "chance_per_level": d.chance_per_level })
}

/// The `SkillDef` fields that carry a number the player feels. Zero / `None` / `false` is the
/// default and is left out, so a card shows only what the skill actually does.
fn skill_fields(d: &SkillDef) -> BTreeMap<&'static str, serde_json::Value> {
    use serde_json::json;
    let mut f: BTreeMap<&'static str, serde_json::Value> = BTreeMap::new();
    macro_rules! num {
        ($($name:ident),*) => { $( if i64::from(d.$name) != 0 { f.insert(stringify!($name), json!(d.$name)); } )* };
    }
    num!(
        max_level, loadout_slots, cooldown_ticks, motion_delay_ticks, cast_time_ticks, range, power_bp, power_per_level_bp,
        aoe_radius, self_aoe_radius, self_aoe_max_targets, impact_radius, knockback, pull, taunt_on_hit_ticks, volley_targets,
        ground_field_radius, field_hits, field_hits_per_level, field_strike_hits, field_duration_ticks, field_pull,
        basic_power_bp, basic_range, mark_step_bp, mark_max_stacks, mark_ticks, mark_slow_pct, mark_aspd_slow_bp, leech_bp,
        lb_max, lb_on_basic, lb_on_skill, lb_on_hit_taken, lb_on_break, lb_per_second, toughness_dmg_mult,
        heal_maxhp_bp, heal_maxhp_per_level_bp, communio_atk_bp, debuff_amp_bp, hp_regen_flat, hp_regen_maxhp_bp,
        aoe_aspd_slow_bp, display_hits, party_atk_per_level, pact_crit_bp, pact_ticks, def_scale_bp, taunted_target_bp, broken_target_bp, basic_toughness_mult, spark_stacks, spark_atk_bp, pickpocket_gil_bp, kill_gil_bonus_bp, heal_on_hit_bp, healed_ally_dmg_bp, healed_ally_stacks, element_roll_bp, hot_maxhp_bp
    );
    macro_rules! flag {
        ($($name:ident),*) => { $( if d.$name { f.insert(stringify!($name), json!(true)); } )* };
    }
    flag!(
        zone_wide, field_place_densest, field_fuse, field_power_from_atk, field_center_self, field_follows_caster, basic_ranged,
        cast_buff_party, heals_ally, heal_self_only, heal_scales_atk, target_lowest_hp, target_furthest, target_boss_first, spark_keeper, spark_detonate, reset_on_break, once_per_target, requires_shield
    );
    if let Some(e) = d.basic_element { f.insert("basic_element", json!(format!("{e:?}"))); }
    if let Some(t) = d.basic_damage_type { f.insert("basic_damage_type", json!(format!("{t:?}"))); }
    if let Some(e) = d.cast_element { f.insert("cast_element", json!(format!("{e:?}"))); }
    if let Some(t) = d.damage_type { f.insert("damage_type", json!(format!("{t:?}"))); }
    if !d.cast_buff.is_empty() { f.insert("cast_buff", json!(d.cast_buff.iter().map(buff_json).collect::<Vec<_>>())); }
    if let Some(x) = d.on_hit_debuff { f.insert("on_hit_debuff", debuff_json(&x)); }
    if let Some(x) = d.on_hit_debuff_2 { f.insert("on_hit_debuff_2", debuff_json(&x)); }
    if !d.anim_slot.is_empty() { f.insert("anim_slot", json!(d.anim_slot)); }
    f
}

fn stat_rows(data: &GameData, zone: &str, seed: u64, unit: &str) -> Result<Vec<StatRow>, String> {
    let mut state = data::new_game_as(seed, data, zone, unit)?;
    let id = state.grid.entities.iter().find(|e| matches!(e.team, Team::Player) && !e.is_pet).map(|e| e.id).ok_or("no lead")?;
    let mut rows = Vec::new();
    for &level in STAT_LEVELS {
        let cur = state.grid.entities.iter().find(|e| e.id == id).map_or(1, |e| e.stats.level);
        if level > cur {
            data::debug_add_levels(&mut state, id, level - cur);
        }
        let e = state.grid.entities.iter().find(|e| e.id == id).ok_or("lead vanished")?;
        rows.push(StatRow {
            level,
            max_hp: combat::effective_max_hp(e),
            atk: combat::effective_atk(e),
            hard_def: combat::effective_hard_def(e),
            soft_def: combat::effective_soft_def(e),
            aspd_milli: combat::effective_atk_speed_milli(e),
            crit_bp: combat::effective_crit_chance_bp(e),
            crit_dmg_bp: combat::effective_crit_dmg_bp(e),
        });
    }
    Ok(rows)
}

pub(crate) fn export(data: &GameData, data_path: &str, zone: &str, seed: u64, loc_path: &str, dir: &str) -> Result<(), String> {
    let loc = load_loc(loc_path);
    let catalog: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(data_path).map_err(|e| e.to_string())?).map_err(|e| e.to_string())?;
    let catalog_units: BTreeMap<String, serde_json::Value> = catalog["units"]
        .as_array()
        .map(|a| a.iter().filter_map(|u| Some((u["id"].as_str()?.to_string(), u.clone()))).collect())
        .unwrap_or_default();
    let mut units = Vec::new();
    for &uid in UNITS {
        let def = sim_core::items::exemplar_def_by_id(uid).ok_or_else(|| format!("{uid}: no Exemplar def"))?;
        let rarity = match sim_core::items::exemplar_rarity_index(uid) { Some(0) => "SR", Some(_) => "SSR", None => "?" };
        let doctrine = sim_core::potential::doctrine_for_unit(uid).map(|d| format!("{d:?}")).unwrap_or_default();
        let weapon = sim_core::items::weapon_types_for_unit(uid).first().copied();
        let offhand = weapon.map(|w| format!("{:?}", sim_core::items::offhand_for_weapon(w))).unwrap_or_default();
        let row = catalog_units.get(uid).cloned().unwrap_or(serde_json::Value::Null);
        let element = row["element"].as_str().unwrap_or("").to_string();
        let mut base = BTreeMap::new();
        if let Some(obj) = row.as_object() {
            for (k, v) in obj {
                if k != "skills" && k != "id" {
                    base.insert(k.clone(), v.clone());
                }
            }
        }
        let kit = skills::unit_kit_tree(uid);
        let mut skill_rows = Vec::new();
        for (i, node) in kit.iter().enumerate() {
            let Some(d) = skills::skill_def(node.skill_id) else { continue };
            let (name, blurb) = skills::skill_display(node.skill_id);
            let role = match (i, d.kind) {
                (_, SkillKind::Passive) => "passive",
                _ if d.lb_max > 0 => "ultimate",
                _ => "skill",
            };
            skill_rows.push(Skill {
                id: node.skill_id.to_string(),
                name: name.to_string(),
                blurb: blurb.to_string(),
                detail: skills::skill_detail(node.skill_id).to_string(),
                kind: format!("{:?}", d.kind),
                role,
                fields: skill_fields(d),
            });
        }
        let ladder = sim_core::unit_rank::describe(uid)
            .into_iter()
            .map(|rank| rank.into_iter().map(|l| line(&loc, l.key, &l.args, l.skill)).collect())
            .collect();
        let mut regalia = Vec::new();
        if let Some(ids) = sim_core::items::regalia_line_for_unit(uid) {
            for (rung, rid) in ids.iter().enumerate().filter(|(_, id)| !id.is_empty()) {
                let Some(rd) = sim_core::items::regalia_def(&sim_core::items::regalia_item_def_id(rid)) else { continue };
                regalia.push(Regalia {
                    id: rid.to_string(),
                    name: rd.name.clone(),
                    rarity: ["R", "SR", "SSR"][rung.min(2)],
                    lump_lv1: rd.lump_base,
                    advanced: rd.advanced.map(|m| format!("{m:?}")),
                    passive_s0: sim_core::regalia_passive::describe(rid, 0).into_iter().map(|l| line(&loc, l.key, &l.args, "")).collect(),
                    passive_s5: sim_core::regalia_passive::describe(rid, 5).into_iter().map(|l| line(&loc, l.key, &l.args, "")).collect(),
                });
            }
        }
        units.push(Unit {
            id: uid.to_string(),
            name: def.name.clone(),
            rarity,
            doctrine,
            element,
            weapon: weapon.map(|w| format!("{w:?}")).unwrap_or_default(),
            offhand,
            base,
            stats: stat_rows(data, zone, seed, uid)?,
            skills: skill_rows,
            ladder,
            regalia,
        });
    }
    let roster = Roster {
        sim_version: sim_core::VERSION,
        catalog_version: catalog["version"].as_str().unwrap_or("").to_string(),
        generated_from: format!("sim-cli --roster-web ({data_path}, {loc_path})"),
        units,
    };
    std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    let js = format!("window.ROSTER = {};\n", serde_json::to_string_pretty(&roster).map_err(|e| e.to_string())?);
    let path = format!("{dir}/roster.js");
    std::fs::write(&path, js).map_err(|e| format!("{path}: {e}"))?;
    eprintln!("wrote {path} ({} units, sim {}, catalog {})", roster.units.len(), roster.sim_version, roster.catalog_version);
    Ok(())
}
