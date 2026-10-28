//! Session lifecycle shared by every host bridge.
//!
//! The online model (docs/server-plan-phases.md, GDD §4.4) is client-sim + server-VERIFY: a
//! session is `(start checkpoint + recorded input log)`, and a push is a `SaveBundle` the server
//! REPLAYS. Getting any of this subtly different between bridges shows up as "the server rejects
//! every save", never as a visible bug — which is why the whole lifecycle lives here once.
//!
//! Free functions take the pieces (`GameState`, `GameData`, the start checkpoint, the pending
//! anchor) so `SimBridge` can keep its existing fields; [`Session`] bundles the same pieces for
//! hosts that just want one handle (sim-ffi).

use sim_core::data::{self, GameData};
use sim_core::events::{self, InputEvent};
use sim_core::state::GameState;
use sim_core::{hash, replay};

/// S1/S1c: record this session's input log for replay + chain snapshot checkpoints. Applied to
/// every state a session starts from (new game, character creation, load).
pub fn arm_recording(st: &mut GameState) {
    st.record_inputs = true;
    st.snapshot_interval = replay::SNAPSHOT_INTERVAL_TICKS;
}

/// New game in `zone_id` from a catalog JSON. Errors are strings because no host should need to
/// tell them apart — the answer is always "refuse, keep the previous state".
pub fn start_new(seed: u64, game_data_json: &str, zone_id: &str) -> Result<(GameState, GameData), String> {
    let gd = GameData::from_json_str(game_data_json)?;
    let mut st = data::new_game_in_zone(seed, &gd, zone_id)?;
    arm_recording(&mut st);
    Ok((st, gd))
}

/// Character creation genesis (`new_game_as`), deliberately NAME-FREE so the bundle's start stays
/// the server-reproducible state; the caller records the party name as an `InputEvent`
/// afterwards (see `Session::new_as`, `SimBridge::create_character`).
pub fn start_new_as(
    seed: u64,
    game_data_json: &str,
    zone_id: &str,
    unit_def: &str,
) -> Result<(GameState, GameData), String> {
    let gd = GameData::from_json_str(game_data_json)?;
    let mut st = data::new_game_as(seed, &gd, zone_id, unit_def)?;
    arm_recording(&mut st);
    Ok((st, gd))
}

/// Load a saved `GameState` against the CURRENT catalog: migrate held items to today's
/// definitions, re-arm recording (the loaded snapshot is the new start checkpoint), and recover
/// `current_zone_id` for pre-0.0.162 saves from the grid size.
pub fn load(save_json: &str, game_data_json: &str) -> Result<(GameState, GameData), String> {
    let mut st: GameState = serde_json::from_str(save_json).map_err(|e| e.to_string())?;
    let gd = GameData::from_json_str(game_data_json)?;
    data::migrate_save_items(&mut st, &gd);
    arm_recording(&mut st);
    if st.current_zone_id.is_none() {
        let (w, h) = (st.grid.width, st.grid.height);
        if let Some(z) = gd.zones.iter().find(|z| z.width == w && z.height == h) {
            st.current_zone_id = Some(z.id.clone());
        }
    }
    Ok((st, gd))
}

/// ONE driver step, shared with `replay::replay`, so a re-run of (start + log) reproduces the
/// session bit-for-bit. No-op for n == 0.
pub fn tick(st: &mut GameState, gd: &GameData, n: u64) {
    if n == 0 {
        return;
    }
    replay::advance(st, gd, n);
}

/// Apply one `InputEvent` given as serde JSON (the same shape the input log stores). Returns
/// false only when the JSON does not parse — an event the sim rejects is a silent no-op inside
/// `apply_input`, by design (replay must survive catalog skew).
///
/// `gd` IS REQUIRED, and this took a live bug to establish. It used to call the catalog-less
/// `events::apply_input`, which is `apply_input_with(state, None, ..)` — and every data-dependent
/// handler is written `if let Some(d) = data`, so with `None` they do NOTHING. Recruiting, deploying,
/// advancing a job, crafting, learning a recipe and entering a dungeon all parsed perfectly, applied
/// cleanly, and changed nothing at all. The Godot bridge always passed its catalog; this path, which
/// every non-Godot host uses, never did, and the failure is indistinguishable from a dead button.
pub fn apply_input_json(st: &mut GameState, gd: Option<&GameData>, event_json: &str) -> bool {
    let Ok(ev) = serde_json::from_str::<InputEvent>(event_json) else {
        return false;
    };
    events::apply_input_with(st, gd, ev);
    true
}

/// Client-facing view JSON. ALWAYS parseable — `{}` when the view cannot be built — because an
/// empty string once made every pre-load frame log a parse error that masked real ones.
pub fn view_json(st: &GameState, gd: Option<&GameData>) -> String {
    match crate::view_state::build_view_state(st, gd) {
        Some(v) => serde_json::to_string(&v).unwrap_or_else(|_| "{}".to_string()),
        None => "{}".to_string(),
    }
}

/// The item COLLECTIONS, one named query rather than a getter per screen.
///
/// `view_state` deliberately empties these arrays and sends only their counts: at 10 ticks a
/// second the bag alone caused visible stutter, so the screens that need one ask for it when they
/// open. The Godot bridge answers that with eighteen separate `#[func]` getters; a C ABI pays for
/// each of those in symbols and in wrapper code, so this is one entry point with a key. An unknown
/// key returns `[]` rather than an error — a client asking for something this build does not have
/// should render an empty screen, not fail to open one.
///
/// Every item goes through the SAME `item_to_view` the per-tick view uses, so a tooltip in a bag
/// and a tooltip in the fight cannot disagree.
/// DEV CHEATS — DIRECT state edits, and deliberately NOT `InputEvent`s.
///
/// That distinction is the whole safety argument, and it is the Godot bridge's, copied. The sim is
/// replay-verified: a save is `(start + the input log)`, and the server checks it by replaying it.
/// If granting Gil were an input, it would replay cleanly and the server would accept a fabricated
/// fortune. Because these edit the state directly instead, they are invisible to the log — which
/// means the save STOPS verifying, and that is the correct outcome. A cheat should break your
/// ability to push, not your ability to be believed.
///
/// So they are refused while ONLINE (`Session::allow_debug`), with that reason stated, exactly as
/// `debug_blocked` does on the Godot side.
///
/// Returns false for an unknown command or a bad target, never a panic — same rule as `apply_input`.
pub fn debug_json(st: &mut GameState, gd: Option<&GameData>, cmd: &str) -> bool {
    let Ok(v) = serde_json::from_str::<serde_json::Value>(cmd) else { return false };
    let Some(obj) = v.as_object().and_then(|o| o.iter().next()) else { return false };
    let (name, arg) = obj;
    let n = |k: &str| arg.get(k).and_then(|x| x.as_i64()).unwrap_or(0);
    let sid = |k: &str| arg.get(k).and_then(|x| x.as_str()).unwrap_or("");
    match name.as_str() {
        // Gil and Soul carry an EARN REASON in this sim; `Debug` is a real variant so the
        // monetary-policy accounting can tell a cheat from a faucet rather than silently
        // mis-attributing it.
        "give_currency" => {
            st.earn_gil(n("gil").max(0) as u64, sim_core::state::GilEarn::Debug);
            st.earn_soul(n("soul").max(0) as u64, sim_core::state::SoulEarn::Debug);
            true
        }
        "add_levels" => data::debug_add_levels(st, n("entity_id") as u32, n("n").max(0) as u32),
        // APEX POINTS, which nothing else can hand out: they are earned by XP past the level cap,
        // so a fresh party is hours of fighting away from the screen having anything on it. Copied
        // from `bridge_world.rs::debug_grant_apex` — including the half-filled bar, because the
        // cost curve depends on `apex_level` and a bar left at zero hides the one reading the
        // screen exists to show.
        // KILL an entity outright. The story layer's `@ remove_member` needs it — a cast member
        // leaving the field mid-scene is how a sacrifice lands — and there is no INPUT for it,
        // because "this character dies now" is not a thing a player may author. Copied from
        // `bridge_world.rs::debug_kill`.
        "kill" => {
            let Some(i) = st.grid.idx_of_id(n("entity_id") as u32) else { return false };
            st.grid.entities[i].hp = 0;
            st.grid.entities[i].alive = false;
            true
        }
        // MEET THE BOSS THRESHOLD, so the room can be finished without farming it.
        //
        // The room-advance rule stays the sim's: `mark_zone_cleared_if_swept` still wants the boss
        // SPAWNED and nothing left alive, and the zone still swaps on its own. This only skips the
        // grind in front of that — the kills a probe would otherwise have to wait minutes for, at a
        // rate the fight decides.
        //
        // It cannot be reached from a real session: every debug edit is refused once the client is
        // online, because a cheated save stops verifying. `kill` is here for the same reason and was
        // added for the same kind of need.
        // LANE MODE (`sim_core::lane`, a trial — `docs/lane-combat-design.md`): the field becomes
        // 20 × 3 lanes with the party on a formation, everything else the live combat and room
        // flow. A DEV-SLOT demo only, like every debug edit: refused online, and the verify server
        // would refuse a lane save anyway (it replays without lane mode). `lane_off` returns the
        // session to the normal field by re-entering the stage's first room.
        "lane_mode" => {
            let Some(gd) = gd else { return false };
            let cap = u8::try_from(n("cap").clamp(1, 9)).unwrap_or(2);
            sim_core::lane::enable(st, gd, cap).is_ok()
        }
        // FORMATION SCREEN (dev slots, like the rest of lane mode): put a member on a slot.
        // `col` is "back" / "mid" / "front" (x 4 / 6 / 8), `lane` 0..=2; a member already there swaps.
        "lane_slot" => {
            let x = match sid("col") {
                "back" => sim_core::lane::COL_BACK,
                "mid" => sim_core::lane::COL_MID,
                "front" => sim_core::lane::COL_FRONT,
                _ => return false,
            };
            sim_core::lane::set_slot(st, n("entity_id") as u32, x, n("lane") as i32)
        }
        "lane_off" => {
            let Some(gd) = gd else { return false };
            st.lane = None;
            let zone = st.current_zone_id.clone().unwrap_or_default();
            let room1 = sim_core::state::band_of_zone(&zone).map(|b| format!("zone_b{b}_1")).unwrap_or(zone);
            data::change_zone(st, gd, &room1).is_ok()
        }
        "meet_boss_threshold" => {
            let Some(zs) = st.zone_spawn.as_mut() else { return false };
            zs.kill_counter = zs.boss_threshold;
            true
        }
        "grant_apex" => {
            let Some(i) = st.grid.idx_of_id(n("entity_id") as u32) else { return false };
            let stm = &mut st.grid.entities[i].stats;
            stm.level = stm.level.max(sim_core::tick::LEVEL_CAP);
            stm.apex_level =
                (stm.apex_level + n("n").max(0) as u32).min(sim_core::tick::APEX_LEVEL_CAP);
            stm.xp = sim_core::tick::apex_xp_cost(stm.apex_level) / 2;
            true
        }
        "max_skills" => data::debug_max_skills(st, n("entity_id") as u32),
        "give_gear" => gd.is_some_and(|d| data::debug_give_gear(st, d, n("entity_id") as u32)),
        // The ONE cheat that IS an input, because it already was: granting an item is a real
        // server-authored event (`GrantLoot`), so it stays in the log where it belongs.
        "grant_item" => {
            events::apply_input_with(st, gd, InputEvent::GrantLoot {
                def_id: sid("def_id").to_string(),
                foil: arg.get("foil").and_then(|x| x.as_bool()).unwrap_or(false),
            });
            true
        }
        _ => false,
    }
}

pub fn query_json(st: &GameState, gd: Option<&GameData>, key: &str) -> String {
    // The NON-item answers first. They are here rather than in the per-tick view for the same
    // reason the item lists are: a screen wants them once when it opens, and the view pays for
    // every field ten times a second whether anything is looking or not.
    match key {
        k if k.starts_with("combat_skills:") => {
            let Some(id) = k.strip_prefix("combat_skills:").and_then(|v| v.parse::<u32>().ok())
            else { return "[]".into() };
            return crate::combat_skills::for_entity(st, id).to_string();
        }
        k if k.starts_with("potential:") => {
            let Some(id) = k.strip_prefix("potential:").and_then(|value| value.parse::<u32>().ok())
            else { return "{}".to_string() };
            return crate::potential_view::for_entity(st, id).to_string();
        }
        // The BENCH. Deployed members ride the view's `grid.entities`; these are the ones who are
        // owned and not on the field, which no per-entity list can carry because they have no
        // entity.
        "roster" => {
            let list: Vec<serde_json::Value> = st
                .roster
                .iter()
                .map(|(id, rc)| {
                    serde_json::json!({
                        "id": id,
                        "unit_def": rc.unit_def,
                        "level": rc.data.stats.level,
                        "apex_level": rc.data.stats.apex_level,
                        // What a refund would hand back (0.0.349) — the sim's own figure, so the
                        // bench can show it before the player commits.
                        "refund_xp": sim_core::tick::cum_base(&rc.data.stats),
                    })
                })
                .collect();
            return serde_json::to_string(&list).unwrap_or_else(|_| "[]".to_string());
        }
        // EXEMPLAR UNITS (0.0.350): every Exemplar the account holds IS a unit — benched or on the
        // field — with its stars, level and the Resonance credits waiting for it.
        "exemplar_units" => {
            return serde_json::to_string(&sim_core::data::exemplar_units(st))
                .unwrap_or_else(|_| "[]".to_string());
        }
        // THE ENGRAM RESERVE + the account's ceiling (0.0.349): the stock per tier, each tier's
        // XP value, and how far a benched unit may be poured. One answer for the bench screen.
        "engrams" => {
            let (lvl, apx) = sim_core::data::refund_ceiling(st);
            return serde_json::to_string(&serde_json::json!({
                "owned": st.owned_engrams,
                "tiers": sim_core::state::ENGRAM_TIERS,
                "ceiling_level": lvl,
                "ceiling_apex": apx,
            }))
            .unwrap_or_else(|_| "{}".to_string());
        }
        // What "fill to max" WOULD do to benched unit N — plan, resulting level, XP wasted —
        // computed by the sim without mutating, so the confirm shows the sim's number.
        k if k.starts_with("engram_preview:") => {
            let Some(id) = k.strip_prefix("engram_preview:").and_then(|v| v.parse::<u32>().ok())
            else { return "null".to_string() };
            return sim_core::data::engram_fill_preview(st, id)
                .and_then(|p| serde_json::to_string(&p).ok())
                .unwrap_or_else(|| "null".to_string());
        }
        // Only TIER-1 starters are recruitable; a 2nd-tier job is reached by advancement, not by
        // being bought. `creatable_units` is the catalog's own answer to that, not a filter the
        // client is trusted to reproduce.
        "units" => {
            let list: Vec<serde_json::Value> = gd
                .map(|d| {
                    d.creatable_units()
                        .iter()
                        .map(|c| {
                            serde_json::json!({
                                "id": c.id,
                            })
                        })
                        .collect()
                })
                .unwrap_or_default();
            return serde_json::to_string(&list).unwrap_or_else(|_| "[]".to_string());
        }
        // An OBJECT, not a list: the party header's numbers. `slot_cost` is derived, not
        // stored — the client must not re-implement the price curve, because a client that
        // disagrees with the sim shows a price the purchase then refuses.
        "party_meta" => {
            let name = if st.party_name.is_empty() { &st.character_name } else { &st.party_name };
            let owned = data::owned_count(st);
            return serde_json::to_string(&serde_json::json!({
                "name": name,
                "gil": st.gil(),
                "owned": owned,
                // COMBAT SCORE (display only, `sim_core::combat_score`): the fielded party's sum.
                "combat_score": sim_core::combat_score::party_score(st),
                "party_size": data::party_size(st),
                // The CAP travels with the number it caps: `party_slots` open, `party_max` the
                // ceiling. Slots open by PROGRESS since 0.0.490 (`data::open_earned_slots`), so
                // `slot_cost` is always -1 (kept for old clients; JsonUtility cannot read null) and
                // `slot_next` is the stage whose arrival opens the next one (-1 = all open).
                // Characters are not bought here at all (gacha).
                "party_slots": st.party_slots(),
                "party_max": data::MAX_PARTY,
                "slot_cost": -1i64,
                "slot_next": data::next_slot_requirement(st).map_or(-1i64, i64::from),
                // What the SERVER pays when a slot opens, slots 2-5 alike: this many Starter pulls
                // (verify-server `slot_tickets`). The sim never grants it.
                "slot_reward_tickets": 10,
            }))
            .unwrap_or_else(|_| "{}".to_string());
        }
        // THE REFINE PLAN for one item: the odds, the ore, the price, and whether the bag can pay.
        //
        // THE DPS WINDOW — damage per class and per skill since the meter was last reset. Copied
        // from the Godot bridge's `dps_json`, including its ordering (biggest first) and its one
        // piece of naming: a skill id becomes its display name, a class id and "Basic Attack" are
        // already names. The client only divides by the elapsed seconds.
        //
        // A RESETTABLE WINDOW, which is what separates it from `record` (cumulative forever) and
        // from a character's stats.
        "dps" => {
            let d = &st.dps;
            let elapsed = st.tick.saturating_sub(d.start_tick);
            let rows = |m: &std::collections::BTreeMap<String, u64>, is_skill: bool| {
                let mut r: Vec<serde_json::Value> = m
                    .iter()
                    .map(|(k, v)| {
                        let name = if is_skill && sim_core::skills::skill_def(k).is_some() {
                            sim_core::skills::skill_display(k).0.to_string()
                        } else {
                            k.clone()
                        };
                        serde_json::json!({ "name": name, "total": v })
                    })
                    .collect();
                r.sort_by(|a, b| b["total"].as_u64().cmp(&a["total"].as_u64()));
                r
            };
            return serde_json::to_string(&serde_json::json!({
                "elapsed_ticks": elapsed,
                "by_class": rows(&d.by_class, false),
                "by_skill": rows(&d.by_skill, true),
            }))
            .unwrap_or_else(|_| "{}".to_string());
        }

        // "orbs" / "orb_plan:" / "orb_fits:" — RETIRED with the Star Stone system (0.0.381).
        // Answer the empty shape an old client expects rather than an error.
        "orbs" => return "[]".to_string(),
        k if k.starts_with("orb_plan:") || k.starts_with("orb_fits:") => return "{}".to_string(),

        // "advances" — RETIRED with class and Job (0.0.352). Answers an empty list so an old
        // client's query is not an error.
        "advances" => return "[]".to_string(),

        // THE SOUL PATHS graph. Copied from the Godot bridge's `soul_paths_json`, including the
        // `allocatable && soul >= cost` gate — that is two different reasons a node cannot be
        // taken (the web does not reach it yet, or you cannot afford it) collapsed into the one
        // answer the screen needs, and re-deriving either half client-side would let the UI offer
        // a node the sim then refuses.
        "soul_paths" => {
            use sim_core::soul_paths::{self, PathEffect};
            let soul = st.soul();
            let owned = &st.soul_paths;
            let effect_name = |e: PathEffect| match e {
                PathEffect::None => "",
                PathEffect::AtkPct => "ATK%", PathEffect::DefPct => "DEF%",
                PathEffect::DefAdd => "DEF",
                PathEffect::HpPct => "HP%", PathEffect::HpAdd => "MaxHP",
                PathEffect::AtkAdd => "ATK", PathEffect::ExpPct => "EXP%",
                PathEffect::GilPct => "Gil%", PathEffect::SoulPct => "Soul%",
                PathEffect::StashCap => "Stash", PathEffect::AutoSellOverflow => "AutoSell%",
                PathEffect::CritBp => "CRIT%", PathEffect::DodgeBp => "Dodge%",
            };
            let arr: Vec<serde_json::Value> = soul_paths::SOUL_PATHS
                .iter()
                .map(|n| serde_json::json!({
                    "id": n.id, "label": n.label,
                    "effect": effect_name(n.effect), "magnitude": n.magnitude,
                    "cost": n.cost, "x": n.x, "y": n.y,
                    "neighbors": n.neighbors,
                    "owned": soul_paths::is_owned(owned, n.id),
                    "allocatable": soul_paths::is_allocatable(owned, n) && soul >= n.cost,
                }))
                .collect();
            return serde_json::to_string(&arr).unwrap_or_else(|_| "[]".to_string());
        }

        // BESTIARY — one entry per monster FAMILY (same-model groups share a kill pool), plus the
        // milestone table. Copied from the Godot bridge's `bestiary_json`, and every judgement in
        // it is the SIM's rather than the client's: which monsters are bosses, what counts as a
        // tanky HP band, where a creature is actually met (its zones, not the vestigial `level` on
        // its def), and which drops are family-distinctive rather than the shared pool.
        "bestiary" => {
            let Some(d) = gd else { return "{}".to_string() };
            let mut entries: Vec<serde_json::Value> = Vec::new();
            let mut seen: Vec<String> = Vec::new();
            // Built once: it walks every zone to work out who is a boss, which is not something to
            // redo per row.
            let band_scale = sim_core::data::bestiary_bands::BandScale::build(d);
            for m in &d.monsters {
                if m.name.is_empty() {
                    continue;
                }
                let key = if m.bestiary_group.is_empty() { m.id.clone() } else { m.bestiary_group.clone() };
                if seen.contains(&key) {
                    continue;
                }
                seen.push(key.clone());
                // Representative def: the one whose id == the group key, else this one.
                let rep = d.monsters.iter().find(|x| x.id == key).unwrap_or(m);
                let variants: Vec<&str> = d
                    .monsters
                    .iter()
                    .filter(|x| {
                        !x.name.is_empty()
                            && (x.id == key
                                || (!x.bestiary_group.is_empty() && x.bestiary_group == key))
                    })
                    .map(|x| x.name.as_str())
                    .collect();
                // Family-distinctive drops with their rate. Skip the SHARED pool
                // (equipment / stones / ore / junk — every monster carries these)
                // and the "nothing" sentinel. Rate = weight / table-total (the
                // total INCLUDES nothing, so it's the true per-kill chance); across
                // variants keep the BEST (max) rate — a player's best farm spot.
                // `[{name, rate_bp}]` where rate_bp is per-100000 (0.001% = 1).
                let mut drops: Vec<(String, u64)> = Vec::new();
                for x in d.monsters.iter().filter(|x| {
                    x.id == key || (!x.bestiary_group.is_empty() && x.bestiary_group == key)
                }) {
                    let total: u64 = x.drops.iter().map(|r| r.weight as u64).sum();
                    if total == 0 { continue; }
                    for row in &x.drops {
                        if row.item_id == "nothing" { continue; }
                        if let Some(i) = d.item(&row.item_id) {
                            if matches!(i.kind, sim_core::items::ItemKind::Weapon
                                | sim_core::items::ItemKind::Armor) { continue; }
                        }
                        // Hide the crafting/upgrade feed from the bestiary's drop list — it is noise
                        // next to the gear a player is hunting. Filtered by `emission_group`, the field
                        // that EXISTS to classify this, not by the id's spelling: the prefixes agreed
                        // with the groups only by convention (ore 4/4, junk 33/33, stone 13/13), and
                        // `contains("stone")` would also have swallowed a future `moonstone_blade`.
                        if d.item(&row.item_id).is_some_and(|i| {
                            matches!(i.emission_group.as_str(), "ore" | "junk" | "star_stone")
                        }) { continue; }
                        let n = d.item(&row.item_id)
                            .map(|i| if i.name.is_empty() { i.id.clone() } else { i.name.clone() })
                            .unwrap_or_else(|| row.item_id.clone());
                        // per-100000 (5-dp %) rounded — enough to render "0.001%".
                        let rate_bp = (row.weight as u64 * 100_000) / total;
                        match drops.iter_mut().find(|(dn, _)| *dn == n) {
                            Some(e) => e.1 = e.1.max(rate_bp),
                            None => drops.push((n, rate_bp)),
                        }
                    }
                }
                drops.sort_by(|a, b| b.1.cmp(&a.1)); // best rate first
                let drop_names: Vec<serde_json::Value> = drops
                    .into_iter()
                    .map(|(name, rate_bp)| serde_json::json!({ "name": name, "rate_bp": rate_bp }))
                    .collect();
                // WHERE it is met, not what its def says. Since 0.0.280 a monster's `level` field is a
                // fallback for one that is placed nowhere; the creature's actual level is whatever the
                // zone it stands in names, and most stand in several. So the bestiary reports the range
                // it is found across — which is also the question a player is asking.
                let mut lv_min = u16::MAX;
                let mut lv_max = 0u16;
                for z in &d.zones {
                    let here = z.boss_monster_id == key
                        || z.spawn_pool.iter().any(|e| e.monster_id == key)
                        || d.monsters.iter().any(|x| {
                            (!x.bestiary_group.is_empty() && x.bestiary_group == key)
                                && (z.boss_monster_id == x.id
                                    || z.spawn_pool.iter().any(|e| e.monster_id == x.id))
                        });
                    if here && z.stage > 0 {
                        lv_min = lv_min.min(z.stage);
                        lv_max = lv_max.max(z.stage);
                    }
                }
                if lv_min == u16::MAX {
                    // Never placed — fall back to whatever the def claims, so an unplaced monster
                    // still reads as something rather than "Lv 0".
                    lv_min = rep.stage;
                    lv_max = rep.stage;
                }
                // Bands, not numbers: `hp` and `atk` on the def are vestigial (the sim reads
                // `level_base[level] x the multiplier`), and printing them showed "HP 1 ATK 1" for 12
                // of the 66 placed monsters. What does not move with the zone is the MULTIPLIER, and
                // the sim classifies it — GDScript is not the place to decide what counts as tanky.
                let is_boss = sim_core::data::bestiary_bands::BandScale::is_boss(d, &key);
                entries.push(serde_json::json!({
                    "key": key,
                    "stage_min": lv_min,
                    "stage_max": lv_max,
                    "name": rep.name,
                    "race": rep.race,
                    "size": rep.size,
                    // A monster has no element (0.0.444): the bestiary shows what it is WEAK to.
                    "weaknesses": rep.weaknesses.clone(),
                    "resists": rep.resists.clone(),
                    "hp_band": band_scale.band(
                        rep, sim_core::data::bestiary_bands::BandStat::Hp, is_boss).key(),
                    "atk_band": band_scale.band(
                        rep, sim_core::data::bestiary_bands::BandStat::Atk, is_boss).key(),
                    "sprite": rep.sprite,
                    "kills": st.monster_kills.get(&key).copied().unwrap_or(0),
                    "claimed": st.bestiary_claimed.get(&key).copied().unwrap_or(0),
                    "variants": variants,
                    "drops": drop_names,
                }));
            }
            let out = serde_json::json!({
                // NAMED FIELDS, where the Godot bridge sends a triple. The one structural change
                // in this copy, and it is forced: Unity's JsonUtility cannot parse a jagged array,
                // so `[[100, 500, 1], ...]` is unreadable on that side. Naming them also makes the
                // payload say what the numbers are.
                "milestones": sim_core::state::BESTIARY_MILESTONES
                    .iter()
                    .map(|(kills, gil, soul)| serde_json::json!({
                        "kills": kills, "gil": gil, "soul": soul,
                    }))
                    .collect::<Vec<_>>(),
                "entries": entries,
            });
            return serde_json::to_string(&out).unwrap_or_else(|_| "{}".to_string());
        }

        // LIFETIME RECORD — everything the party has done since the save began. Copied from the
        // Godot bridge's `lifetime_stats_json`, including its one non-obvious choice: playtime is
        // the SIM's own `tick`, not a wall clock, because in an idle game the time the world
        // advanced (offline catch-up included) is what a player means by "how long have I played".
        //
        // SIM NUMBERS ONLY. Gacha, market and store activity happen server-side and are genuinely
        // absent here — the screen says so, because a player who counts their pulls on this page
        // and finds nothing would reasonably call it a bug.
        "record" => {
            let m = &st.measures;
            return serde_json::to_string(&serde_json::json!({
                "total_ticks": st.tick,
                "kills": m.kills, "boss_kills": m.boss_kills,
                "rooms_cleared": m.rooms_cleared, "bands_cleared": m.bands_cleared,
                "damage_dealt": m.damage_dealt, "damage_taken": m.damage_taken,
                "healing_done": m.healing_done, "skills_cast": m.skills_cast,
                "deaths": m.deaths, "wipes": m.wipes,
                "biggest_hit": st.records.biggest_hit,
                "levels_gained": m.levels_gained,
                "items_crafted": m.items_crafted, "orbs_used": m.orbs_used,
                "cards_fed": m.cards_fed, "exemplars_fed": m.exemplars_fed,
                "gil_earned": m.gil_earned, "gil_spent": m.gil_spent,
                "gil_received": m.gil_received, "gil_sent": m.gil_sent,
                "items_listed": m.items_listed,
            }))
            .unwrap_or_else(|_| "{}".to_string());
        }

        // SHOP — every buyable catalog row (`buy_price > 0`) as a FULL item view plus its price.
        // Copied from the Godot bridge's `shop_items_json`, including the reason it builds the row
        // through `instantiate_item`: the shop's tooltip reads the same fields the inventory's
        // does, and hand-assembling a preview is how a shop ends up describing an item differently
        // from the copy the player receives. The price lives on the DEF, not on the Item, which is
        // why it is inserted afterwards.
        "shop" => {
            let Some(d) = gd else { return "[]".to_string() };
            let arr: Vec<serde_json::Value> = d
                .items
                .iter()
                .filter(|it| data::sells_for_gil(d, it))
                .map(|def| {
                    let mut v = crate::item_view::item_to_view(&data::instantiate_item(def));
                    if let Some(o) = v.as_object_mut() {
                        o.insert("buy_price".into(), serde_json::json!(def.buy_price));
                        // The PREDICATE, not just the value. `orb` is already in the view as a
                        // serialized enum, but the shop only asks "is this a Star Stone?" to file
                        // it under Stones — and Unity's JsonUtility cannot type a tagged enum, so a
                        // client reading the enum would have to re-derive the answer from a shape.
                        o.insert("is_orb".into(), serde_json::json!(def.orb.is_some()));
                    }
                    v
                })
                .collect();
            return serde_json::to_string(&arr).unwrap_or_else(|_| "[]".to_string());
        }

        // FEATURES (0.0.490) — the catalog's release switches, so the client hides a held-back
        // system's screens (cards: album, enchant, card sockets) instead of guessing. No catalog
        // → every feature on, the same default the catalog itself uses.
        "features" => {
            let cards = gd.map_or(true, |d| d.features.cards);
            let craft = gd.map_or(true, |d| d.features.craft);
            return serde_json::json!({ "cards": cards, "craft": craft }).to_string();
        }

        // MATERIALS — the Potential materials LEDGER (0.0.354) as item rows: one row per held def,
        // `quantity` = the count, built through `instantiate_item` for the same reason the shop
        // is, so a Sigil reads the same in the trade window as anywhere else. The ledger is not
        // the bag, so a client that lists sellable stock must read BOTH `inventory` and this.
        // A key the catalog cannot resolve (data skew) is skipped, not invented.
        "materials" => {
            let Some(d) = gd else { return "[]".to_string() };
            let arr: Vec<serde_json::Value> = st
                .materials()
                .filter_map(|(id, n)| {
                    let mut it = data::instantiate_item(d.item(id)?);
                    it.quantity = n;
                    Some(crate::item_view::item_to_view(&it))
                })
                .collect();
            return serde_json::to_string(&arr).unwrap_or_else(|_| "[]".to_string());
        }

        // RECIPES — the catalog rows plus the one thing the catalog cannot know, whether this save
        // has LEARNED each. Copied from the Godot bridge's `recipes_json`, including
        // `server_rolled`: that predicate is the SIM's own, and a second copy is how a recipe ends
        // up craftable in the UI and a silent no-op in the sim.
        "recipes" => {
            let Some(d) = gd else { return "[]".to_string() };
            let name_of = |id: &str| -> String {
                d.item(id).map(|it| it.name.as_str()).filter(|n| !n.is_empty())
                    .unwrap_or(id).to_string()
            };
            let arr: Vec<serde_json::Value> = d
                .recipes
                .iter()
                .map(|r| {
                    let ings: Vec<serde_json::Value> = r
                        .ingredients
                        .iter()
                        .map(|ing| serde_json::json!({
                            "def_id": ing.def_id, "qty": ing.qty, "name": name_of(&ing.def_id),
                        }))
                        .collect();
                    serde_json::json!({
                        "id": r.id,
                        "name": if r.name.is_empty() { r.id.clone() } else { r.name.clone() },
                        "output_def_id": r.output_def_id,
                        "output_name": name_of(&r.output_def_id),
                        "output_qty": r.output_qty,
                        "gil_cost": r.gil_cost,
                        "learn_soul": r.learn_soul,
                        "learn_gil": r.learn_gil,
                        "learned": st.recipes_learned.contains(&r.id),
                        "station": r.station,
                        "ingredients": ings,
                        "server_rolled": sim_core::craft::output_is_rolled_gear(d, &r.output_def_id),
                    })
                })
                .collect();
            return serde_json::to_string(&arr).unwrap_or_else(|_| "[]".to_string());
        }

        // FUSION FORGE (0.0.385) — the two answers the screen needs, from the SIM's own rules so
        // the preview can never disagree with what `FuseGear` does:
        //   `fuse_rules`               — the constants: how many go in, the patch ceiling, the
        //                                grade from which the server must roll.
        //   `fuse_preview:i,i,i,i,i,i` — six bag indices (any order) → what pressing would make:
        //                                the tier the average lands on and the odds of the step
        //                                up, the output grade, the Gil paid, whether the server
        //                                must roll first, and if so the binding (`key`, `attempt`)
        //                                to POST to `/fuse/roll`. `{}` when the six cannot fuse
        //                                (mixed grades, a duplicate index, at the ceiling, a
        //                                bound or unique piece, not gear).
        "fuse_rules" => {
            return serde_json::to_string(&serde_json::json!({
                "inputs": sim_core::fusion::FUSE_INPUTS,
                "ceiling_step": sim_core::fusion::FUSE_GRADE_CEILING.step(),
                "server_roll_from_step": sim_core::fusion::FUSE_SERVER_ROLL_FROM.step(),
                // Blacksmith (0.0.396): the party's level and the LIVE fail odds per INPUT grade —
                // the same rule (`loot_rules.fusion`) `gem_rules` reports for gems.
                "blacksmith_level": gd.map(|d| sim_core::fusion::blacksmith_level(st, d)).unwrap_or(0),
                "crafter_level_cap": gd.map(|d| d.loot_rules.fusion.crafter_level_cap).unwrap_or(0),
                "crafter_fusions_per_level": gd.map(|d| d.loot_rules.fusion.crafter_fusions_per_level).unwrap_or(0),
                "crafter_bp_per_level": gd.map(|d| d.loot_rules.fusion.crafter_bp_per_level).unwrap_or(0),
                "fusions_run": st.measures.fusions.get(),
                "fail_bp_by_grade": gd.map(|d| d.loot_rules.fusion.fail_bp_by_grade.clone()).unwrap_or_default(),
                "fail_bp_now_by_grade": gd.map(|d| sim_core::items::GRADES.iter().map(|g| sim_core::fusion::fail_bp(st, d, *g)).collect::<Vec<u32>>()).unwrap_or_default(),
            }))
            .unwrap_or_else(|_| "{}".to_string());
        }
        k if k.starts_with("fuse_preview:") => {
            let idx: Vec<u32> = k["fuse_preview:".len()..]
                .split(',')
                .filter_map(|p| p.trim().parse::<u32>().ok())
                .collect();
            let Ok(picks) = <[u32; sim_core::fusion::FUSE_INPUTS]>::try_from(idx) else {
                return "{}".to_string();
            };
            let Some((tier_lo, up_bp, grade, payout)) = sim_core::fusion::preview(st, &picks) else {
                return "{}".to_string();
            };
            let input_grade = st.inventory.get(picks[0] as usize).map(|i| i.grade).unwrap_or_default();
            let needs = sim_core::fusion::needs_server_roll(input_grade);
            let (key, attempt) = sim_core::fusion::binding(st, &picks).unwrap_or_default();
            return serde_json::to_string(&serde_json::json!({
                "tier": tier_lo, "tier_up_bp": up_bp,
                // Fail odds from the INPUT grade, after the Blacksmith bonus.
                "fail_bp": gd.map(|d| sim_core::fusion::fail_bp(st, d, input_grade)).unwrap_or(0),
                "grade_step": grade.step(), "grade": format!("{grade:?}"),
                "payout_gil": payout,
                "needs_server_roll": needs,
                "key": key, "attempt": attempt,
            }))
            .unwrap_or_else(|_| "{}".to_string());
        }

        // TRAINING (0.0.406). `training_rules`: the table the Training window is built from —
        // every slot with its stat, per-level value, cap, the party's CURRENT level, the value it
        // is worth now, the NEXT level's Gil (null at cap) and the line as text, plus the curve.
        // The client never prices a level itself; `Train { slot_id }` is the only write.
        "training_rules" => {
            let Some(d) = gd else { return "{}".to_string() };
            let slots: Vec<serde_json::Value> = d.training.slots.iter().map(|sl| {
                let lv = sim_core::training::level(st, &sl.id);
                let m = sim_core::training::slot_mod(sl, lv);
                let next = sim_core::training::cost(&d.training, sl, lv.saturating_add(1));
                serde_json::json!({
                    "id": sl.id, "stat": sl.stat, "element": sl.element,
                    "per_level": sl.per_level, "max_level": sl.max_level,
                    "base_gil": sl.base_gil, "cost_mult_bp": sl.cost_mult_bp,
                    "level": lv,
                    "value_now": sl.per_level.saturating_mul(i32::from(lv)),
                    "next_cost": next,
                    // The breadth gate: the party-wide total this slot needs before it can leave
                    // its current level (null = not on a gate step), and whether it is open.
                    "gate_total_needed": sim_core::training::gate_total_needed(&d.training, lv),
                    "gate_open": sim_core::training::gate_open(st, d, &sl.id),
                    "text": m.map(|m| crate::item_view::stat_mod_text(&m)),
                    "text_per_level": sim_core::training::slot_mod(sl, 1).map(|m| crate::item_view::stat_mod_text(&m)),
                })
            }).collect();
            return serde_json::to_string(&serde_json::json!({
                "cost_curve_bp": d.training.cost_curve_bp,
                "slots": slots,
                "levels_bought": st.measures.training_levels.get(),
                "total_levels": sim_core::training::total_levels(st),
                "gate_step": d.training.gate_step,
                "gate_total_per_step": d.training.gate_total_per_step,
                "stash_slots": st.training_stash_slots,
            }))
            .unwrap_or_else(|_| "{}".to_string());
        }

        // CRESTS (0.0.416). `crest_rules`: every crest with its category, tier, stat, level, value
        // now, the material it eats and how many the party holds vs the next level's price, as
        // text — plus the whole material purse. `disenchant_preview:<inv_idx>` prices one bag
        // item (null when it cannot be disenchanted). `Disenchant` / `UpgradeCrest` write.
        "crest_rules" => {
            let Some(d) = gd else { return "{}".to_string() };
            let crests: Vec<serde_json::Value> = d.crests.slots.iter().map(|sl| {
                let lv = sim_core::crest::level(st, &sl.id);
                let next = sim_core::crest::cost(&d.crests, sl, lv.saturating_add(1));
                let key = sim_core::crest::mat_key(sl.category, sl.tier);
                serde_json::json!({
                    "id": sl.id, "category": sl.category, "tier": sl.tier,
                    "stat": sl.stat, "element": sl.element,
                    "per_level": sl.per_level, "max_level": sl.max_level, "level": lv,
                    "material": key,
                    // SHARDS: `shards_per_material` make one material; `held`/`next_cost` are
                    // shards, the `*_materials` twins whole materials for display.
                    "held": st.crest_mats.get(&key).copied().unwrap_or(0),
                    "held_materials": st.crest_mats.get(&key).copied().unwrap_or(0) / d.crests.shards_per_material.max(1),
                    "next_cost": next.as_ref().map(|(_, n)| *n),
                    "next_cost_materials": next.as_ref().map(|(_, n)| n / d.crests.shards_per_material.max(1)),
                    "text": sim_core::crest::slot_mod(sl, lv).map(|m| crate::item_view::stat_mod_text(&m)),
                    "text_per_level": sim_core::crest::slot_mod(sl, 1).map(|m| crate::item_view::stat_mod_text(&m)),
                })
            }).collect();
            return serde_json::to_string(&serde_json::json!({
                "min_tier": d.crests.min_tier, "max_tier": d.crests.max_tier,
                "yield_by_grade": d.crests.yield_by_grade, "gem_yield": d.crests.gem_yield,
                "shards_per_material": d.crests.shards_per_material,
                "materials": st.crest_mats,
                "crests": crests,
                "items_disenchanted": st.measures.items_disenchanted.get(),
                "crest_levels": st.measures.crest_levels.get(),
            }))
            .unwrap_or_else(|_| "{}".to_string());
        }
        k if k.starts_with("disenchant_preview:") => {
            let Some(d) = gd else { return "null".to_string() };
            let Ok(idx) = k["disenchant_preview:".len()..].trim().parse::<usize>() else { return "null".to_string() };
            let Some(it) = st.inventory.get(idx) else { return "null".to_string() };
            let Some((kind, tier)) = sim_core::crest::material_of(d, it) else { return "null".to_string() };
            return serde_json::to_string(&serde_json::json!({
                "material": sim_core::crest::mat_key(kind, tier),
                "category": kind, "tier": tier,
                "yield": sim_core::crest::yield_of(&d.crests, it, kind),
                "shards_per_material": d.crests.shards_per_material,
            }))
            .unwrap_or_else(|_| "null".to_string());
        }

        // GEMS (0.0.386). `gem_rules`: the table a socket screen is built from — sockets per grade,
        // every set with its group, stats and the MID value at each grade (so the client never
        // scales a base itself), the roll window, the merge count and the per-stat caps.
        "gem_rules" => {
            let Some(d) = gd else { return "{}".to_string() };
            let r = &d.loot_rules.gems;
            let sets: Vec<serde_json::Value> = r
                .sets
                .iter()
                .map(|set| {
                    serde_json::json!({
                        "id": set.id, "group": set.group, "weight": set.weight,
                        "stats": set.stats.iter().map(|st| serde_json::json!({
                            "key": st.key,
                            // Flat stats on the display scale; bp stats as they are.
                            "mid_by_grade": sim_core::items::GRADES.iter()
                                .map(|g| {
                                    let v = sim_core::gems::mid_value(st.base, *g);
                                    if matches!(st.key, sim_core::substats::SubstatKey::Atk | sim_core::substats::SubstatKey::Hp | sim_core::substats::SubstatKey::Def | sim_core::substats::SubstatKey::HpRegen) { crate::disp(v.into()) as i32 } else { v }
                                }).collect::<Vec<i32>>(),
                            "cap": sim_core::gems::cap_of(st.key),
                        })).collect::<Vec<_>>(),
                        "defs": sim_core::items::GRADES.iter()
                            .map(|g| sim_core::gems::def_id(&set.id, *g)).collect::<Vec<String>>(),
                    })
                })
                .collect();
            return serde_json::to_string(&serde_json::json!({
                "gem_slots_by_grade": sim_core::gems::GEM_SLOTS_BY_GRADE,
                "card_slots_by_grade": sim_core::gems::CARD_SLOTS_BY_GRADE,
                "roll_window_bp": r.roll_window_bp, "merge": r.merge,
                "fail_bp_by_grade": d.loot_rules.fusion.fail_bp_by_grade,
                // What THIS party faces after its crafter level — the number a fuse screen shows.
                "fail_bp_now_by_grade": sim_core::items::GRADES.iter().map(|g| sim_core::gems::fail_bp(st, d, *g)).collect::<Vec<u32>>(),
                "crafter_level": sim_core::gems::crafter_level(st, d),
                "crafter_level_cap": d.loot_rules.fusion.crafter_level_cap,
                "crafter_fusions_per_level": d.loot_rules.fusion.crafter_fusions_per_level,
                "crafter_bp_per_level": d.loot_rules.fusion.crafter_bp_per_level,
                "fusions_run": st.measures.gem_fusions.get(),
                "ceiling_step": sim_core::fusion::FUSE_GRADE_CEILING.step(),
                "sets": sets,
            }))
            .unwrap_or_else(|_| "{}".to_string());
        }
        // `gem_plan:inv:<idx>` / `gem_plan:eq:<entity_id>,<equipped_slot>`: the piece's sockets
        // and what is in them, plus every held gem that FITS it with the window each stat would
        // roll in — the sim's `refusal` per candidate, so the screen shows why, not a dead button.
        k if k.starts_with("gem_plan:") => {
            let (Some(d), rest) = (gd, &k["gem_plan:".len()..]) else { return "{}".to_string() };
            let target = if let Some(i) = rest.strip_prefix("inv:") {
                i.trim().parse::<u32>().ok().map(|idx| sim_core::gems::GemTarget::Inventory { idx })
            } else if let Some(pair) = rest.strip_prefix("eq:") {
                let mut it = pair.split(',').filter_map(|p| p.trim().parse::<u32>().ok());
                match (it.next(), it.next()) {
                    (Some(entity_id), Some(equipped_slot)) => Some(sim_core::gems::GemTarget::Equipped { entity_id, equipped_slot }),
                    _ => None,
                }
            } else {
                None
            };
            let Some(target) = target else { return "{}".to_string() };
            let item = match target {
                sim_core::gems::GemTarget::Inventory { idx } => st.inventory.get(idx as usize),
                sim_core::gems::GemTarget::Equipped { entity_id, equipped_slot } => st
                    .grid
                    .idx_of_id(entity_id)
                    .and_then(|i| st.grid.entities.get(i))
                    .and_then(|e| e.equipped.get(equipped_slot as usize)),
            };
            let Some(item) = item else { return "{}".to_string() };
            let next_socket = item.gems.len() as u8;
            // Gems are bag stacks since 0.0.448 (`gems::held` — the ledger holds none).
            let held: Vec<serde_json::Value> = sim_core::gems::held(st, d)
                .filter_map(|(id, n)| {
                    let (set, grade) = sim_core::gems::set_of(d, id)?;
                    let socket = next_socket.min(item.gem_slots().saturating_sub(1));
                    let refusal = sim_core::gems::refusal(st, d, target, socket, id);
                    Some(serde_json::json!({
                        "def_id": id, "count": n, "set": set.id, "grade_step": grade.step(),
                        "name": d.item(id).map(|i| i.name.clone()).unwrap_or_default(),
                        "fits": refusal.is_none(), "refusal": refusal,
                        "lines": set.stats.iter().map(|stt| {
                            let (lo, hi) = sim_core::gems::window(sim_core::gems::mid_value(stt.base, grade), d.loot_rules.gems.roll_window_bp);
                            serde_json::json!({
                                "key": stt.key,
                                "lo": crate::item_view::stat_mod_text(&stt.key.make(lo)),
                                "hi": crate::item_view::stat_mod_text(&stt.key.make(hi)),
                            })
                        }).collect::<Vec<_>>(),
                    }))
                })
                .collect();
            return serde_json::to_string(&serde_json::json!({
                "gem_slots": item.gem_slots(),
                "card_slots": item.socket_count(),
                "next_socket": next_socket,
                "socketed": item.gems.iter().map(|g| serde_json::json!({
                    "def_id": g.def_id,
                    "name": d.item(&g.def_id).map(|i| i.name.clone()).unwrap_or_default(),
                    "grade_step": d.item(&g.def_id).and_then(|i| i.gem.as_ref()).map(|s| s.grade.step()).unwrap_or(0),
                    "lines": g.lines.iter().map(crate::item_view::stat_mod_text).collect::<Vec<String>>(),
                })).collect::<Vec<_>>(),
                "held": held,
            }))
            .unwrap_or_else(|_| "{}".to_string());
        }

        // THE PURSE. Two numbers on three screens, and neither is worth a per-tick field the
        // client then has to regex out of the view.
        "purse" => {
            return serde_json::to_string(&serde_json::json!({
                "gil": st.gil(), "soul": st.soul(), "zone_id": st.current_zone_id,
            }))
            .unwrap_or_else(|_| "{}".to_string());
        }

        // EFFECTIVE STATS for one member — what the character sheet shows. Copied from the Godot
        // bridge's `effective_stats_json` rather than re-derived: every one of these is a function
        // the SIM already owns, and a client that recomputed "atk" would eventually disagree with
        // the number the fight actually uses.
        //
        // On demand, not per tick: none of it moves at a rate a player can watch, and it is eight
        // combat functions plus a buff pass per member.
        k if k.starts_with("stats:") => {
            use sim_core::combat::*;
            let Some(want) = k.strip_prefix("stats:").and_then(|v| v.parse::<u32>().ok())
            else { return "{}".to_string() };
            let Some(i) = st.grid.idx_of_id(want) else { return "{}".to_string() };
            let e = &st.grid.entities[i];
            let atk_equip: i32 = e
                .equipped
                .iter()
                .map(|it| it.worn_atk(e.exemplar.as_ref().map(|x| x.id.as_str()), &e.unit_def))
                .sum();
            let def_equip: i32 = e
                .equipped
                .iter()
                .map(|it| it.worn_def(e.exemplar.as_ref().map(|x| x.id.as_str()), &e.unit_def))
                .sum();
            // ATTACK SPEED as attacks/sec ×100 (0.0.367) — the accumulator value
            // already IS that number, so there is no curve to invert here.
            let atk_speed_x100 = effective_atk_speed_milli(e) as i64;
            // Amounts on the DISPLAY scale (`crate::disp`); the percentages below are not amounts.
            let d = |v: i32| crate::disp(i64::from(v));
            return serde_json::to_string(&serde_json::json!({
                // Party HP is on the sim scale (see `disp_for`); ATK / DEF stay ÷10.
                "max_hp": i64::from(effective_max_hp(e)),
                "atk": d(effective_atk(e)), "atk_equip": d(atk_equip),
                "def": d(effective_def(e)), "def_equip": d(def_equip),
                "dodge_bp": sim_core::combat::stat_mods::dodge_bp(e),
                "crit_bp": effective_crit_chance_bp(e),
                "crit_dmg_bp": sim_core::combat::effective_crit_dmg_bp(e),
                "mitigation_bp": sim_core::combat::def_mitigation_bp(e),
                "combat_score": sim_core::combat_score::member_score(st, e),
                "atk_speed_x100": atk_speed_x100,
                "level": e.stats.level,
                "xp": e.stats.xp,
                "level_cap": sim_core::tick::LEVEL_CAP,
                // APEX rides the SAME answer, exactly as it does in the Godot bridge: at the
                // level cap the `xp` bar above fills toward the next Apex point instead, so the
                // two halves are one reading and splitting them would let a screen show a bar
                // filling against a cost from a different tick.
                "level_points": e.stats.level_points,
                "core_unspent": e.stats.core_unspent(),
                "apex_level": e.stats.apex_level,
                "apex_unspent": e.stats.apex_unspent(),
                "apex_alloc": e.stats.apex.to_vec(),
                "apex_cost": sim_core::tick::apex_xp_cost(e.stats.apex_level),
                "apex_level_cap": sim_core::tick::APEX_LEVEL_CAP,
                "apex_bucket_cap": sim_core::stats::APEX_BUCKET_CAP,
            }))
            .unwrap_or_else(|_| "{}".to_string());
        }

        // WHAT BINDING THIS CARD WOULD DO — the effect it grants, how many binds the party has
        // already done of this type, what the NEXT one costs and whether it is maxed. Copied from
        // `bridge_items.rs::bind_info`: the escalating cost (1, 2, 3) and the cap live in the sim,
        // and a client that recomputed them would grey the wrong button the day either changes.
        //
        // Binding is PARTY-WIDE, so `done` is the max over members rather than the picked one's
        // count — they stay in sync because a bind always applies to all.
        k if k.starts_with("bind_info:") => {
            let Some(idx) = k.strip_prefix("bind_info:").and_then(|v| v.parse::<usize>().ok())
            else { return "null".to_string() };
            let Some(card) = st.card_album.get(idx) else { return "null".to_string() };
            if card.bind_mods.is_empty() {
                return "null".to_string();
            }
            let done: u8 = st
                .grid
                .entities
                .iter()
                .filter(|e| matches!(e.team, sim_core::grid::Team::Player) && !e.is_pet)
                .map(|e| e.bound_card_counts.get(&card.def_id).copied().unwrap_or(0))
                .max()
                .unwrap_or(0);
            let cap: u8 = 3; // mirrors events::handlers::BIND_CARD_CAP
            return serde_json::to_string(&serde_json::json!({
                "effect": card
                    .bind_mods
                    .iter()
                    .map(crate::item_view::stat_mod_text)
                    .collect::<Vec<_>>()
                    .join(", "),
                "done": done,
                "cost": (done + 1) as u32,
                "cap": cap,
                "maxed": done >= cap,
                "stock": card.quantity,
                "foil": card.foil,
            }))
            .unwrap_or_else(|_| "null".to_string());
        }

        // THE STAT-POINT BUCKETS, in index order, from the sim's own `APEX_PER_PT` table — so the
        // screen renders "+N per point" without a second copy. TWO TIERS: indices 0..APEX_CORE are
        // Tier-1 (core, level-up points), the rest Tier-2 (substats, post-cap points). Named by the
        // stat directly (user 2026-09-11), `bp` = the per-pt value is basis points.
        "apex_buckets" => {
            use sim_core::stats as sst;
            let per = |i: usize| sst::APEX_PER_PT[i];
            let tier = |i: usize| if i < sst::APEX_CORE { "core" } else { "substat" };
            let row = |i: usize, name: &str, bp: bool| serde_json::json!(
                {"name": name, "per_pt": per(i), "bp": bp, "tier": tier(i)});
            return serde_json::to_string(&serde_json::json!([
                row(sst::AP_ATK,        "ATK",           false),
                row(sst::AP_HP,         "HP",            false),
                row(sst::AP_DEF,        "DEF",           false),
                row(sst::AP_CRIT,       "Crit",          true),
                row(sst::AP_CRITDMG,    "Crit DMG",      true),
                row(sst::AP_DODGE,      "Dodge",         true),
                row(sst::AP_DEFPEN,     "DEF Pen",       true),
                row(sst::AP_BREAKDMG,   "Break DMG",     true),
                row(sst::AP_BREAKBUILD, "Break Buildup", true),
                row(sst::AP_COOLDOWN,   "Cooldown",      true),
                row(sst::AP_CAST,       "Cast",          true),
                row(sst::AP_CHARGE,     "Charge",        true),
                row(sst::AP_HEAL,       "Heal",          true),
            ]))
            .unwrap_or_else(|_| "[]".to_string());
        }

        // THE CARD PANEL, per class, for the lead character. One answer rather than three,
        // because the three are useless apart: a socket count without its capacity cannot say
        // whether a slot is empty or locked, and a card without its panel index cannot be removed.
        //
        // The capacities come from `card_panel_capacity`, the sim's own function. The Godot client
        // carries a hand-written copy of that table; copying it a second time into a second client
        // is how two clients end up disagreeing with the sim about which panel is full.
        // `card_slots` answers the LEAD; `card_slots:<entity_id>` answers that member. The
        // suffix rather than a second key: it is the same question about a different subject, and
        // a per-character screen needs to ask it about whoever the player picked.
        k if k == "card_slots" || k.starts_with("card_slots:") => {
            use sim_core::items::{card_class_of_slot, card_panel_capacity, CARD_CLASSES};
            let want: Option<u32> = k.strip_prefix("card_slots:").and_then(|v| v.parse().ok());
            let ent = st.grid.entities.iter().find(|e| {
                matches!(e.team, sim_core::grid::Team::Player)
                    && !e.is_pet
                    && want.is_none_or(|id| e.id == id)
            });
            let Some(ent) = ent else { return "[]".to_string() };
            let list: Vec<serde_json::Value> = CARD_CLASSES
                .iter()
                .map(|&class| {
                    // Sockets are summed off the EQUIPPED gear feeding this class — the same rule
                    // the per-tick view uses for its activation caps.
                    let sockets: u32 = ent
                        .equipped
                        .iter()
                        .filter(|it| card_class_of_slot(it.equip_slot) == Some(class))
                        .map(|it| it.socket_count() as u32)
                        .sum();
                    let cards: Vec<serde_json::Value> = ent
                        .card_panel
                        .iter()
                        .enumerate()
                        .filter(|(_, it)| it.card_slot == Some(class))
                        .map(|(i, it)| {
                            serde_json::json!({
                                "panel_idx": i,
                                "item": crate::item_view::item_to_view(it),
                            })
                        })
                        .collect();
                    serde_json::json!({
                        "class": format!("{class:?}"),
                        "sockets": sockets,
                        "cap": card_panel_capacity(class),
                        "cards": cards,
                    })
                })
                .collect();
            return serde_json::to_string(&list).unwrap_or_else(|_| "[]".to_string());
        }
        _ => {}
    }

    let items: &[sim_core::items::Item] = match key {
        "inventory" => &st.inventory,
        "stash" => &st.stash,
        "escrow" => &st.escrow,
        "rare_bag" => &st.rare_bag,
        "card_album" => &st.card_album,

        "owned_regalia" => &st.owned_regalia,
        // A KEY THIS HOST DOES NOT ANSWER IS NOT AN EMPTY LIST.
        //
        // It used to fall through to `&[]`, and `[]` is a lie with a plausible reading: a typo'd
        // or renamed key rendered as "you own nothing", which is exactly what an empty bag looks
        // like on the day the bag really is empty. `null` cannot be mistaken for an answer, and
        // every list screen already treats it as one — `JsonUtility` hands back a null array and
        // the caller substitutes an empty one.
        _ => return "null".to_string(),
    };
    let mut out: Vec<serde_json::Value> = items.iter().map(crate::item_view::item_to_view).collect();
    // PASSIVE ACTIVE (0.0.485): a Regalia's passive works only for a wearer of its DOCTRINE. The
    // host answers it per card, so the client never compares doctrines itself; false when nobody
    // wears the card.
    if key == "owned_regalia" {
        for (it, v) in items.iter().zip(out.iter_mut()) {
            let active = it.equipped_by != 0
                && st.grid.entities.iter().find(|e| e.id == it.equipped_by).is_some_and(|e| {
                    let doctrine = sim_core::potential::doctrine_for_unit(&e.unit_def);
                    let bare = it.def_id.strip_prefix("regalia_").unwrap_or(&it.def_id);
                    doctrine.is_some() && doctrine == sim_core::items::regalia_doctrine(bare)
                        && sim_core::regalia_passive::passive_of(bare).is_some()
                });
            if let Some(o) = v.as_object_mut() {
                o.insert("passive_active".into(), serde_json::json!(active));
            }
        }
        // SALVAGE (0.0.489): the lock, and what salvaging each copy pays — `salvage` is null when
        // the sim would refuse it (worn, locked, an SSR short of a spare past S5).
        for (i, (it, v)) in items.iter().zip(out.iter_mut()).enumerate() {
            let paid = sim_core::items::regalia_salvage_of(items, i);
            if let Some(o) = v.as_object_mut() {
                o.insert("locked".into(), serde_json::json!(it.locked));
                o.insert("bulk_salvage".into(), serde_json::json!(sim_core::items::regalia_bulk_salvageable(items, i)));
                // ENHANCE XP (0.0.490): progress into this level, the step to the next (0 at the
                // cap), and what is still missing to the cap — the bar and "use max" read these.
                if let Some(rung) = sim_core::items::regalia_rung_of(&it.def_id) {
                    use sim_core::items::{regalia_xp_to_level, REGALIA_MAX_LEVEL};
                    let lv = it.enhance_level.min(REGALIA_MAX_LEVEL);
                    let next = if lv >= REGALIA_MAX_LEVEL { 0 } else { regalia_xp_to_level(rung, lv + 1) - regalia_xp_to_level(rung, lv) };
                    let have = regalia_xp_to_level(rung, lv).saturating_add(it.enhance_xp);
                    o.insert("enhance_xp".into(), serde_json::json!(it.enhance_xp));
                    o.insert("enhance_xp_next".into(), serde_json::json!(next));
                    o.insert("enhance_xp_to_max".into(), serde_json::json!(regalia_xp_to_level(rung, REGALIA_MAX_LEVEL).saturating_sub(have)));
                }
                o.insert(
                    "salvage".into(),
                    paid.map_or(serde_json::Value::Null, |pay| {
                        serde_json::Value::Object(pay.tiers().iter().map(|(id, n)| (id.to_string(), serde_json::json!(n))).collect())
                    }),
                );
            }
        }
    }
    serde_json::to_string(&out).unwrap_or_else(|_| "[]".to_string())
}

/// The full `GameState` as JSON (the save-to-disk payload). Empty string on a serialize failure.
pub fn save_json(st: &GameState) -> String {
    serde_json::to_string(st).unwrap_or_default()
}

pub fn state_hash(st: &GameState) -> String {
    hash::state_hash(st)
}

/// S1b: build the replayable `SaveBundle` for a push, plus the RE-ANCHOR point to apply if the
/// server confirms it (`(live state now, input log length now)`). `None` if the bundle does not
/// serialize.
pub fn export_log(
    start: &GameState,
    live: &GameState,
    gd: &GameData,
) -> Option<(String, (GameState, usize))> {
    let bundle = replay::SaveBundle::capture(sim_core::VERSION, &gd.version, start.clone(), live);
    let json = serde_json::to_string(&bundle).ok()?;
    Some((json, (live.clone(), live.input_log.len())))
}

/// After the server VERIFIED the pushed bundle: rebase the live log + snapshot chain onto the
/// confirmed state and return the new start checkpoint (the confirmed state, cleaned to a
/// pristine base). Events + ticks recorded during the async confirm are preserved.
pub fn commit_anchor(live: &mut GameState, pending: (GameState, usize)) -> GameState {
    let (mut base, confirmed_len) = pending;
    replay::reanchor(live, base.tick, confirmed_len);
    base.input_log.clear();
    base.snapshots.clear();
    base.record_inputs = false;
    base
}

/// Ticks the NEXT bundle would replay (live − start checkpoint). The client keeps this under the
/// server's `MAX_REPLAY_SPAN`; past it a bundle is refused for good.
pub fn replay_span(start: &GameState, live: &GameState) -> i64 {
    live.tick.saturating_sub(start.tick) as i64
}

/// One handle over the whole lifecycle, for hosts without their own bridge struct.
pub struct Session {
    pub state: GameState,
    /// The catalog, SHARED between a live session and its preview clones (`roi_clone`): it is
    /// immutable after parse, and a clone that deep-copied 800 KB of defs per Auto Equip
    /// candidate was the whole cost of a preview. `Arc::make_mut` in a test that edits it.
    pub data: std::sync::Arc<GameData>,
    session_start: GameState,
    pending_anchor: Option<(GameState, usize)>,
    /// Direct-edit cheats. Defaults to ALLOWED because a fresh session is offline, and the host
    /// turns it off the moment it logs in — see `set_allow_debug`.
    allow_debug: bool,
    forge_receipt: serde_json::Value,
}

/// A clone is a PREVIEW: the same state and catalog, no anchor in flight (the live session keeps
/// that), so applying inputs to it can never touch what the live one uploads.
impl Clone for Session {
    fn clone(&self) -> Self {
        Session {
            state: self.state.clone(),
            data: std::sync::Arc::clone(&self.data),
            session_start: self.session_start.clone(),
            pending_anchor: None,
            allow_debug: self.allow_debug,
            forge_receipt: self.forge_receipt.clone(),
        }
    }
}

impl Session {
    fn from_parts((state, data): (GameState, GameData)) -> Self {
        Session { session_start: state.clone(), state, data: std::sync::Arc::new(data), pending_anchor: None, allow_debug: true, forge_receipt: serde_json::json!({}) }
    }

    pub fn new(seed: u64, game_data_json: &str, zone_id: &str) -> Result<Self, String> {
        start_new(seed, game_data_json, zone_id).map(Self::from_parts)
    }

    pub fn new_starter(seed: u64, game_data_json: &str, zone_id: &str, party_name: &str) -> Result<Self, String> {
        let gd = GameData::from_json_str(game_data_json)?;
        let mut state = data::new_starter_party(seed, &gd, zone_id)?;
        arm_recording(&mut state);
        let mut session = Self::from_parts((state, gd));
        events::apply_input(&mut session.state, InputEvent::SetPartyName { name: party_name.into() });
        Ok(session)
    }

    pub fn new_as(seed: u64, game_data_json: &str, zone_id: &str, unit_def: &str, party_name: &str) -> Result<Self, String> {
        let mut s = start_new_as(seed, game_data_json, zone_id, unit_def).map(Self::from_parts)?;
        events::apply_input(&mut s.state, InputEvent::SetPartyName { name: party_name.to_string() });
        Ok(s)
    }

    pub fn load(save_json: &str, game_data_json: &str) -> Result<Self, String> {
        load(save_json, game_data_json).map(Self::from_parts)
    }

    pub fn tick(&mut self, n: u64) {
        tick(&mut self.state, &self.data, n);
    }

    pub fn apply_input_json(&mut self, event_json: &str) -> bool {
        // A DEBUG-authority event is refused outright once debug is off (audit 2026-10-02): applied,
        // it went into the log, the server refused every bundle carrying it, and the re-sent
        // pending bundle kept carrying it — the session could never save again.
        if !self.allow_debug
            && serde_json::from_str::<InputEvent>(event_json)
                .is_ok_and(|e| e.authority() == sim_core::events::InputAuthority::Debug)
        {
            return false;
        }
        // The session HAS the catalog, so there is never a reason to withhold it.
        let before = serde_json::from_str::<InputEvent>(event_json).ok()
            .and_then(|event| crate::forge_receipt::Before::capture(&self.state, &self.data, &event));
        self.forge_receipt = serde_json::json!({});
        let parsed = apply_input_json(&mut self.state, Some(&self.data), event_json);
        if let Some(before) = before { self.forge_receipt = before.finish(&self.state, &self.data); }
        parsed
    }

    pub fn query_json(&self, key: &str) -> String {
        if key == "forge_receipt" { return self.forge_receipt.to_string(); }
        query_json(&self.state, Some(&self.data), key)
    }

    /// Whether the DIRECT-edit cheats are permitted. The host sets this false at login: a direct
    /// edit is not in the input log, so the save stops verifying and every push after it is
    /// rejected. Refusing up front is kinder than letting someone cheat and then discovering their
    /// session is unpushable.
    pub fn set_allow_debug(&mut self, allow: bool) {
        self.allow_debug = allow;
    }

    pub fn debug_json(&mut self, cmd: &str) -> bool {
        if !self.allow_debug {
            return false;
        }
        debug_json(&mut self.state, Some(&self.data), cmd)
    }

    pub fn view_json(&self) -> String {
        view_json(&self.state, Some(&self.data))
    }

    pub fn save_json(&self) -> String {
        save_json(&self.state)
    }

    pub fn state_hash(&self) -> String {
        state_hash(&self.state)
    }

    /// Bundle JSON for a push; arms the pending re-anchor. Empty string if it cannot serialize.
    ///
    /// While a pending anchor is armed (a push not yet confirmed) this re-exports THAT bundle
    /// instead of a newer one (audit 2026-10-02). The host drops an export that overlaps a push in
    /// flight, and replacing the pending anchor then made the confirm of the FIRST push re-anchor
    /// onto a state the server never saw — every later save was refused and flagged. Re-sending
    /// the same bundle is safe both ways: unconfirmed, it is simply retried; already stored, the
    /// server answers it as an idempotent re-send.
    pub fn export_log(&mut self) -> String {
        if let Some((pending, _)) = &self.pending_anchor {
            let bundle = sim_core::replay::SaveBundle::capture(sim_core::VERSION, &self.data.version, self.session_start.clone(), pending);
            return serde_json::to_string(&bundle).unwrap_or_default();
        }
        match export_log(&self.session_start, &self.state, &self.data) {
            Some((json, pending)) => {
                self.pending_anchor = Some(pending);
                json
            }
            None => String::new(),
        }
    }

    pub fn commit_anchor(&mut self) {
        if let Some(pending) = self.pending_anchor.take() {
            self.session_start = commit_anchor(&mut self.state, pending);
        }
    }

    pub fn replay_span(&self) -> i64 {
        replay_span(&self.session_start, &self.state)
    }
}

/// Test-only: put `n` gems of `id` in the bag the way a drop lands them (0.0.448 — gems are
/// bag stacks).
#[cfg(test)]
pub(crate) fn give_gems(state: &mut sim_core::state::GameState, data: &sim_core::data::GameData, id: &str, n: u32) {
    let mut it = sim_core::data::instantiate_item(data.item(id).expect(id));
    it.quantity = n;
    sim_core::drops::bank_purchase_to_inventory(state, it);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn catalog() -> String {
        std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/../../data/game_data.json")).expect("catalog")
    }

    /// `query_json` answers the lists the per-tick view deliberately empties, and answers an
    /// unknown key with NULL rather than an empty array — an empty array is a lie a screen reads
    /// as "you own nothing", which is indistinguishable from the day the bag really is empty. The view's own
    /// counts and the query have to agree, or a bag draws a different number of items than the
    /// count beside it.
    fn view_gil(s: &Session) -> i64 {
        serde_json::from_str::<serde_json::Value>(&s.view_json())
            .unwrap()["gil"].as_i64().unwrap_or(-1)
    }

    /// LANE MODE as a dev-slot demo: `lane_mode` turns the field into 3 lanes (the view carries
    /// the `lane` block), `lane_off` brings back the normal field, and online (debug off) both are
    /// refused.
    #[test]
    fn lane_mode_is_a_debug_toggle_that_the_view_shows() {
        let gd = catalog();
        let mut s = Session::new(7, &gd, "zone_b3_1").expect("new");
        assert!(s.debug_json(r#"{"lane_mode":{"cap":2}}"#));
        s.tick(300);
        let v: serde_json::Value = serde_json::from_str(&s.view_json()).unwrap();
        assert_eq!(v["grid"]["height"].as_i64(), Some(3));
        assert!(v["lane"]["slots"].is_object(), "the formation reaches the client");
        assert!(s.debug_json(r#"{"lane_off":{}}"#));
        let v: serde_json::Value = serde_json::from_str(&s.view_json()).unwrap();
        assert_eq!(v["grid"]["height"].as_i64(), Some(10));
        assert!(v.get("lane").is_none_or(|l| l.is_null()));
        assert!(s.debug_json(r#"{"lane_mode":{"cap":2}}"#));
        let v: serde_json::Value = serde_json::from_str(&s.view_json()).unwrap();
        let id = v["lane"]["slots"].as_object().unwrap().keys().next().unwrap().clone();
        assert!(s.debug_json(&format!(r#"{{"lane_slot":{{"entity_id":{id},"col":"back","lane":2}}}}"#)));
        let v: serde_json::Value = serde_json::from_str(&s.view_json()).unwrap();
        assert_eq!(v["lane"]["slots"][&id], serde_json::json!([4, 2]), "the formation screen's move shows in the view");
        assert!(!s.debug_json(&format!(r#"{{"lane_slot":{{"entity_id":{id},"col":"gap","lane":2}}}}"#)), "a bad column is refused");
        s.set_allow_debug(false);
        assert!(!s.debug_json(r#"{"lane_mode":{"cap":2}}"#), "never online");
    }

    /// AUDIT 2026-10-02: with debug off, a Debug-authority event is refused at the door and never
    /// reaches the log (the server refuses every bundle that carries one).
    #[test]
    fn a_debug_event_is_refused_when_debug_is_off() {
        let gd = catalog();
        let mut s = Session::new(7, &gd, "zone_b1_1").expect("new");
        s.set_allow_debug(false);
        let ev = serde_json::json!({"GrantLoot": {"def_id": "hp_pot_1", "foil": false}}).to_string();
        assert!(serde_json::from_str::<InputEvent>(&ev).is_ok(), "the fixture must be a real event");
        assert!(!s.apply_input_json(&ev), "refused");
        let b: sim_core::replay::SaveBundle = serde_json::from_str(&s.export_log()).unwrap();
        assert!(b.log.is_empty(), "and not logged");
    }

    /// AUDIT 2026-10-02: an export while a push is unconfirmed re-sends THAT bundle, so the
    /// confirm of the first push anchors on the state the server verified; after the confirm the
    /// next export moves on.
    #[test]
    fn an_export_during_an_unconfirmed_push_resends_the_same_bundle() {
        let gd = catalog();
        let mut s = Session::new(7, &gd, "zone_b1_1").expect("new");
        s.tick(200);
        let first = s.export_log();
        s.tick(200);
        let again = s.export_log();
        assert_eq!(first, again, "the unconfirmed bundle is re-sent, not replaced");
        let b: sim_core::replay::SaveBundle = serde_json::from_str(&first).unwrap();
        s.commit_anchor();
        assert_eq!(s.session_start.tick, b.final_tick, "the anchor is the state that was sent");
        let next = s.export_log();
        assert_ne!(next, first, "after the confirm the next export carries the new play");
    }

    #[test]
    fn query_answers_the_lists_the_view_trims() {
        let gd = catalog();
        let mut s = Session::new(7, &gd, "zone_b1_1").expect("new");
        s.tick(3_000);   // long enough to bank some drops
        for key in ["inventory", "stash", "escrow", "rare_bag", "card_album", "owned_regalia"] {
            let json = s.query_json(key);
            let v: serde_json::Value = serde_json::from_str(&json).expect("query must parse");
            assert!(v.is_array(), "{key} must answer an array, got {json}");
        }
        // An unknown key answers NULL, not `[]`. It used to answer `[]`, and that is a lie with a
        // plausible reading — a renamed key rendered as "you own nothing", which is exactly what an
        // empty bag looks like on the day the bag really is empty. This assertion moved with the
        // behaviour on purpose: `null` is the answer that cannot be mistaken for an answer.
        let unknown: serde_json::Value = serde_json::from_str(&s.query_json("no_such_list")).unwrap();
        assert!(unknown.is_null(), "an unknown key answers null, got {unknown}");

        // The party screen's three reads. `roster` and `classes` are arrays like the lists above;
        // `party_meta` is the one OBJECT answer, and its buy price must be the sim's own — a
        // client that re-derived the curve would show a price the purchase then refused.
        // The card panel answers one entry per class, each carrying the capacity the SIM
        // enforces — a client that copied that table could show a slot the equip then refuses.
        let slots_value: serde_json::Value = serde_json::from_str(&s.query_json("card_slots")).unwrap();
        let slots = slots_value.as_array().expect("card_slots answers an array");
        assert_eq!(slots.len(), sim_core::items::CARD_CLASSES.len(),
                   "one entry per card class");
        for entry in slots {
            for field in ["class", "sockets", "cap", "cards"] {
                assert!(entry.get(field).is_some(), "card_slots entry must carry `{field}`");
            }
            let class = entry["class"].as_str().unwrap();
            let cap = entry["cap"].as_u64().unwrap();
            let expected = sim_core::items::CARD_CLASSES
                .iter()
                .find(|c| format!("{c:?}") == class)
                .map(|&c| sim_core::items::card_panel_capacity(c) as u64);
            assert_eq!(Some(cap), expected, "{class} must report the sim's own capacity");
        }

        for key in ["roster", "units"] {
            let v: serde_json::Value = serde_json::from_str(&s.query_json(key)).expect("must parse");
            assert!(v.is_array(), "{key} must answer an array");
        }
        assert!(!serde_json::from_str::<serde_json::Value>(&s.query_json("units"))
            .unwrap().as_array().unwrap().is_empty(),
            "the catalog has creatable classes, so `classes` must not be empty");
        // The per-member form answers the SAME shape for a named entity, and an unknown id
        // answers empty rather than quietly falling back to the lead — a screen showing the wrong
        // character's cards is worse than one showing none.
        let lead_id = s.state.grid.entities.iter()
            .find(|e| matches!(e.team, sim_core::grid::Team::Player) && !e.is_pet)
            .map(|e| e.id).expect("a new game has a player");
        let by_id: serde_json::Value =
            serde_json::from_str(&s.query_json(&format!("card_slots:{lead_id}"))).unwrap();
        assert_eq!(by_id, slots_value, "card_slots:<lead> must equal card_slots");
        let missing: serde_json::Value =
            serde_json::from_str(&s.query_json("card_slots:999999")).unwrap();
        assert_eq!(missing.as_array().map(|a| a.len()), Some(0), "an unknown entity answers []");

        // The character sheet's numbers. Pinned as a required subset for the same reason the view
        // pins its top-level keys: these are hand-written strings, and deleting the line that
        // writes one is invisible in Rust and shows up as a blank row on a screen months later.
        let es: serde_json::Value =
            serde_json::from_str(&s.query_json(&format!("stats:{lead_id}"))).unwrap();
        for field in ["max_hp", "atk", "atk_equip", "def", "def_equip", "dodge_bp",
                      "crit_bp", "crit_dmg_bp", "mitigation_bp", "atk_speed_x100",
                      "level", "xp", "level_cap"] {
            assert!(es.get(field).is_some(), "stats:<id> must carry `{field}`");
        }
        assert_eq!(es["level"].as_u64(),
                   s.state.grid.entities.iter().find(|e| e.id == lead_id).map(|e| e.stats.level as u64),
                   "the sheet's level and the entity's level are one number");
        let no_ent: serde_json::Value =
            serde_json::from_str(&s.query_json("stats:999999")).unwrap();
        assert_eq!(no_ent.as_object().map(|o| o.len()), Some(0), "an unknown entity answers {{}}");

        // Recipes carry the catalog row AND the save's learned flag; a station name is what the
        // craft screens tab on, so an empty one would silently drop a recipe off every tab.
        let recipes: serde_json::Value = serde_json::from_str(&s.query_json("recipes")).unwrap();
        let rows = recipes.as_array().expect("recipes answers an array");
        assert!(!rows.is_empty(), "the catalog has recipes");
        for r in rows {
            for field in ["id", "name", "output_def_id", "output_qty", "gil_cost",
                          "learn_soul", "learn_gil", "learned", "station", "ingredients",
                          "server_rolled"] {
                assert!(r.get(field).is_some(), "a recipe must carry `{field}`");
            }
            assert!(!r["station"].as_str().unwrap_or("").is_empty(), "every recipe names a station");
        }

        // The soul web and the advancement table: both are catalogs the CLIENT could half-derive
        // and must not. `allocatable` folds two different refusals into one answer, and `job_cap`
        // is a sim constant the Godot client hand-copies with a comment admitting the drift.
        let web: serde_json::Value = serde_json::from_str(&s.query_json("soul_paths")).unwrap();
        let nodes = web.as_array().expect("soul_paths answers an array");
        assert!(!nodes.is_empty(), "the soul web has nodes");
        for n in nodes {
            for field in ["id", "label", "effect", "magnitude", "cost", "x", "y",
                          "neighbors", "owned", "allocatable"] {
                assert!(n.get(field).is_some(), "a soul node must carry `{field}`");
            }
        }
        // Class advancement is retired (0.0.352): the query still answers, and answers nothing.
        let adv: serde_json::Value = serde_json::from_str(&s.query_json("advances")).unwrap();
        assert_eq!(adv, serde_json::json!([]));

        // Refine is retired (0.0.370): the key answers nothing, like any unknown key.
        let gone: serde_json::Value =
            serde_json::from_str(&s.query_json(&format!("refine_plan:{lead_id}:0"))).unwrap();
        assert!(gone.is_null(), "a retired query must not answer");

        let purse: serde_json::Value = serde_json::from_str(&s.query_json("purse")).unwrap();
        assert_eq!(purse["gil"].as_i64(), Some(view_gil(&s)), "one gil, two readers");
        assert!(purse.get("soul").is_some() && purse.get("zone_id").is_some());

        let meta: serde_json::Value = serde_json::from_str(&s.query_json("party_meta")).unwrap();
        assert!(meta.is_object(), "party_meta answers an object");
        for field in ["name", "gil", "owned", "party_size", "party_slots", "party_max", "slot_cost"] {
            assert!(meta.get(field).is_some(), "party_meta must carry `{field}`");
        }
        assert_eq!(meta["gil"].as_i64(), Some(view_gil(&s)),
                   "the gil the header shows and the gil the view shows are one number");
        assert_eq!(meta["slot_cost"].as_i64(), Some(-1), "slots are not bought any more");
        assert_eq!(meta["slot_next"].as_i64(),
                   Some(sim_core::data::next_slot_requirement(&s.state).map_or(-1, i64::from)),
                   "slot_next is the sim's rule, not a client-side re-derivation");

        // The view trims the list and keeps the count; the query is where the items come from.
        let view: serde_json::Value = serde_json::from_str(&s.view_json()).unwrap();
        let count = view.get("inventory_count").and_then(|c| c.as_u64()).unwrap_or(0);
        let items: serde_json::Value = serde_json::from_str(&s.query_json("inventory")).unwrap();
        assert_eq!(items.as_array().unwrap().len() as u64, count,
                   "the bag the view counts and the bag the query returns must be the same bag");
        assert!(view.get("inventory").unwrap().as_array().unwrap().is_empty(),
                "the per-tick view must still send the list EMPTY");
    }

    /// EVERY QUERY KEY THE UNITY CLIENT ASKS FOR must be ANSWERED.
    ///
    /// The twin of the event test below, closing the other direction of the same FFI. An event the
    /// host cannot parse is a button that does nothing; a query the host does not answer is a
    /// SCREEN THAT LIES — it draws an empty bag, a card with no effect, a bench with no recipes,
    /// and every one of those is a sentence the player reads and believes.
    ///
    /// The keys below are copied from the C# that asks for them, with concrete values substituted
    /// for the interpolations. A key that has been renamed in Rust fails HERE rather than on a
    /// screen months later, which is the whole reason the list is written out by hand.
    ///
    /// The `stats:` / `card_slots:` / `orb_plan:` / `orb_fits:` / `bind_info:`
    /// families are asked with a PARAMETER, so both halves are checked: the prefix is answered at
    /// all, and a miss inside it answers empty rather than something from another subject.
    #[test]
    fn every_query_the_unity_client_asks_is_answered() {
        let gd = catalog();
        let mut s = Session::new(9, &gd, "zone_b1_1").expect("new");
        s.tick(3_000);   // long enough for the bag to hold something
        let lead = s.state.grid.entities.iter()
            .find(|e| matches!(e.team, sim_core::grid::Team::Player) && !e.is_pet)
            .map(|e| e.id).expect("a new game has a player");
        // The two collections a gacha fills, and the album, are empty in a fresh save — seed one
        // of each so "answered" and "empty" cannot be confused for the lists that matter.
        let _ = sim_core::data::grant_exemplar_unit(&mut s.state, &s.data, "seraphine");
        s.state.owned_regalia.push(sim_core::items::Item::new(
            "regalia_hearthguard", sim_core::items::ItemKind::Regalia, 0, 0,
            sim_core::items::Rarity::Legendary));
        // The album card is INSTANTIATED FROM THE CATALOG, not hand-built: `bind_info` answers
        // from the card's own `bind_mods`, and a card without them answers null for the honest
        // reason (nothing to grant) rather than the one under test (nobody answers this key).
        let card = s.data.item("card_greenling").map(sim_core::data::instantiate_item)
            .expect("the catalog carries a bindable card");
        s.state.card_album.push(card);

        // (the screen that asks, the key exactly as the client builds it)
        let keys: &[(&str, String)] = &[
            ("Bag", "inventory".into()),
            ("Warehouse", "stash".into()),
            ("Market escrow", "escrow".into()),
            ("Rare net", "rare_bag".into()),
            ("Album", "card_album".into()),
            ("Exemplar units", "exemplar_units".into()),
            ("Regalia list", "owned_regalia".into()),
            ("Party bench", "roster".into()),
            ("Party recruit", "units".into()),
            ("Party header", "party_meta".into()),
            ("Wallet", "purse".into()),
            ("Bench recipes", "recipes".into()),
            ("Paths", "soul_paths".into()),
            ("Advancement", "advances".into()),
            ("Bestiary", "bestiary".into()),
            ("Shop", "shop".into()),
            ("Record", "record".into()),
            ("Damage meter", "dps".into()),
            ("Apex buckets", "apex_buckets".into()),
            ("Character sheet", format!("stats:{lead}")),
            ("Cards, the lead", "card_slots".into()),
            ("Cards, a member", format!("card_slots:{lead}")),
            ("Album, what binding grants", "bind_info:0".into()),
            ("Fusion, the rules", "fuse_rules".into()),
        ];
        for (screen, key) in keys {
            let json = s.query_json(key);
            let v: serde_json::Value = serde_json::from_str(&json).unwrap_or_else(|e| {
                panic!("{screen}: `{key}` answered something that is not JSON.\n  {json}\n  {e}")
            });
            assert!(!v.is_null(),
                    "{screen}: the host does not answer `{key}` — the screen would draw nothing \
                     and say nothing was there");
        }

        // A MISS INSIDE a parameterised key is a different thing from an unknown key, and it must
        // stay empty rather than answer about someone else. Each of these names a subject that
        // does not exist.
        for (what, key) in [
            ("an entity that is not there", format!("stats:{}", u32::MAX)),
            ("a member that is not there", format!("card_slots:{}", u32::MAX)),
            ("an album row that is not there", "bind_info:9999".to_string()),
        ] {
            let json = s.query_json(&key);
            let v: serde_json::Value = serde_json::from_str(&json)
                .unwrap_or_else(|e| panic!("{what}: `{key}` answered non-JSON: {json} ({e})"));
            let empty = v.is_null()
                || v.as_object().is_some_and(|o| o.is_empty())
                || v.as_array().is_some_and(|a| a.is_empty());
            assert!(empty, "{what}: `{key}` must answer empty, got {json}");
        }
    }

    /// EVERY EVENT THE UNITY CLIENT SENDS, byte for byte, must PARSE.
    ///
    /// This is the one test the UI port cannot do without, and the reason is a deliberate property
    /// of the sim: `apply_input` treats a malformed or unknown event as a SILENT NO-OP, because
    /// replay must survive a save recorded by a build that knew a different event set. That safety
    /// has a cost on the way in — a typo'd variant, a renamed field, an integer sent as a string
    /// all fail exactly the way a correctly-wired button that has nothing to do fails. Nothing on
    /// screen distinguishes them, and no screenshot ever will.
    ///
    /// So the JSON literals below are copied from the C# that emits them, with concrete values
    /// substituted for the interpolations. If a field is renamed in Rust, THIS breaks — which is
    /// the point, because the alternative is finding out when a player taps Refine and the anvil
    /// does nothing.
    #[test]
    fn every_event_the_unity_client_sends_parses() {
        use sim_core::events::InputEvent as E;
        // (what the screen is, the exact JSON, the variant it must become)
        let cases: &[(&str, &str, fn(&E) -> bool)] = &[
            ("Party bench", r#"{"BenchCharacter":{"char_id":3}}"#,
             |e| matches!(e, E::BenchCharacter { char_id: 3 })),
            ("Party deploy", r#"{"DeployCharacter":{"char_id":3}}"#,
             |e| matches!(e, E::DeployCharacter { char_id: 3 })),
            ("Party recruit", r#"{"BuyCharacter":{"unit_def":"seraphine"}}"#,
             |e| matches!(e, E::BuyCharacter { unit_def } if unit_def == "seraphine")),
            ("Provisions add emergency", r#"{"AddConsumable":{"entity_id":1,"inv_idx":0}}"#,
             |e| matches!(e, E::AddConsumable { entity_id: 1, inv_idx: 0 })),
            ("Provisions remove emergency", r#"{"RemoveConsumable":{"entity_id":1,"slot":0}}"#,
             |e| matches!(e, E::RemoveConsumable { entity_id: 1, slot: 0 })),
            ("Provisions add reserved", r#"{"AddProvision":{"entity_id":1,"inv_idx":0,"qty":10}}"#,
             |e| matches!(e, E::AddProvision { entity_id: 1, inv_idx: 0, qty: 10 })),
            ("Provisions remove reserved", r#"{"RemoveProvision":{"entity_id":1,"slot":0}}"#,
             |e| matches!(e, E::RemoveProvision { entity_id: 1, slot: 0 })),
            // `target` is an Option<CardClass> and the client sends the bare variant NAME. An
            // accessory card may go in either accessory panel, so the field is not decorative.
            ("Cards equip", r#"{"EquipCard":{"entity_id":1,"album_idx":0,"target":"Ring"}}"#,
             |e| matches!(e, E::EquipCard { entity_id: 1, album_idx: 0, target: Some(t) }
                          if *t == sim_core::items::CardClass::Ring)),
            ("Cards unequip", r#"{"UnequipCard":{"entity_id":1,"panel_idx":0}}"#,
             |e| matches!(e, E::UnequipCard { entity_id: 1, panel_idx: 0 })),
            ("Bench craft", r#"{"CraftItem":{"recipe_id":"brew_red_phial"}}"#,
             |e| matches!(e, E::CraftItem { recipe_id } if recipe_id == "brew_red_phial")),
            ("Bench learn", r#"{"LearnRecipe":{"recipe_id":"brew_red_phial"}}"#,
             |e| matches!(e, E::LearnRecipe { recipe_id } if recipe_id == "brew_red_phial")),
            ("Dungeon enter", r#"{"EnterInstance":{"start_zone_id":"zone_b10_1"}}"#,
             |e| matches!(e, E::EnterInstance { start_zone_id } if start_zone_id == "zone_b10_1")),
            ("Map select", r#"{"SelectBand":{"band":6}}"#, |e| matches!(e, E::SelectBand { band: 6 })),
            ("Paths allocate", r#"{"LevelSoulPath":{"skill_id":"aegis"}}"#,
             |e| matches!(e, E::LevelSoulPath { skill_id } if skill_id == "aegis")),
            ("Advance", r#"{"AdvanceClass":{"entity_id":1,"target_class_id":"knight_t2"}}"#,
             |e| matches!(e, E::AdvanceClass { entity_id: 1, target_class_id } if target_class_id == "knight_t2")),

            // --- everything the client learned to send AFTER this test was written -------------
            // The list above covered the screens that existed then. Each of these is a button that
            // now exists, and every one of them would fail silently on a rename.
            ("Bag sell", r#"{"SellInventoryItem":{"inv_idx":0}}"#,
             |e| matches!(e, E::SellInventoryItem { inv_idx: 0 })),
            ("Bag sell all junk", r#""SellAllJunk""#, |e| matches!(e, E::SellAllJunk)),
            ("Bag discard", r#"{"DiscardItem":{"inv_idx":0}}"#,
             |e| matches!(e, E::DiscardItem { inv_idx: 0 })),
            // The two halves of the warehouse take DIFFERENT field names, because each indexes the
            // list it reads from. Sending `inv_idx` to the withdraw is the exact mistake this pins.
            ("Bag deposit to stash", r#"{"DepositToStash":{"inv_idx":0}}"#,
             |e| matches!(e, E::DepositToStash { inv_idx: 0 })),
            ("Stash withdraw", r#"{"WithdrawFromStash":{"stash_idx":0}}"#,
             |e| matches!(e, E::WithdrawFromStash { stash_idx: 0 })),
            ("Rare net take", r#"{"MoveRareToInventory":{"rare_idx":0}}"#,
             |e| matches!(e, E::MoveRareToInventory { rare_idx: 0 })),
            ("Rare net threshold", r#"{"SetRareThreshold":{"rarity":"Mythical"}}"#,
             |e| matches!(e, E::SetRareThreshold { rarity } if *rarity == sim_core::items::Rarity::Mythical)),
            // FUSION: six bag indices, and the server roll the forge got from `/fuse/roll` when the
            // preview said `needs_server_roll` (0 = client roll, below the line).
            ("Fusion, client-rolled", r#"{"FuseGear":{"picks":[0,1,2,3,4,5]}}"#,
             |e| matches!(e, E::FuseGear { picks: [0, 1, 2, 3, 4, 5], server_roll: 0 })),
            ("Fusion, server-rolled", r#"{"FuseGear":{"picks":[5,4,3,2,1,0],"server_roll":123456789}}"#,
             |e| matches!(e, E::FuseGear { picks: [5, 4, 3, 2, 1, 0], server_roll: 123_456_789 })),

            // CARRIERS. The client builds these two names by CONCATENATION — "Equip" + the list's
            // kind — so a rename in Rust does not even change a string literal in the C#.
            ("Exemplar equip", r#"{"EquipExemplar":{"entity_id":1,"owned_idx":0}}"#,
             |e| matches!(e, E::EquipExemplar { entity_id: 1, owned_idx: 0 })),
            ("Exemplar unequip", r#"{"UnequipExemplar":{"entity_id":1}}"#,
             |e| matches!(e, E::UnequipExemplar { entity_id: 1 })),
            ("Regalia equip", r#"{"EquipRegalia":{"entity_id":1,"owned_idx":0}}"#,
             |e| matches!(e, E::EquipRegalia { entity_id: 1, owned_idx: 0 })),
            ("Regalia unequip", r#"{"UnequipRegalia":{"entity_id":1}}"#,
             |e| matches!(e, E::UnequipRegalia { entity_id: 1 })),

            ("Album file", r#"{"DepositCardToAlbum":{"inv_idx":0}}"#,
             |e| matches!(e, E::DepositCardToAlbum { inv_idx: 0 })),
            ("Album withdraw", r#"{"WithdrawCardFromAlbum":{"album_idx":0}}"#,
             |e| matches!(e, E::WithdrawCardFromAlbum { album_idx: 0 })),
            ("Album level up", r#"{"FeedCard":{"target_idx":0,"feeder_idx":1}}"#,
             |e| matches!(e, E::FeedCard { target_idx: 0, feeder_idx: 1 })),
            ("Album bind", r#"{"BindCard":{"entity_id":1,"album_idx":0}}"#,
             |e| matches!(e, E::BindCard { entity_id: 1, album_idx: 0 })),

            ("Apex spend", r#"{"AllocateApex":{"entity_id":1,"bucket":0,"count":5}}"#,
             |e| matches!(e, E::AllocateApex { entity_id: 1, bucket: 0, count: 5 })),
            ("Apex refund", r#"{"ResetApex":{"entity_id":1}}"#,
             |e| matches!(e, E::ResetApex { entity_id: 1 })),

            ("Shop buy", r#"{"BuyItem":{"def_id":"hp_pot_1","qty":1}}"#,
             |e| matches!(e, E::BuyItem { def_id, qty: 1 } if def_id == "hp_pot_1")),
            ("Story recruit", r#"{"RecruitMember":{"unit_def":"lucielle"}}"#,
             |e| matches!(e, E::RecruitMember { unit_def } if unit_def == "lucielle")),
            ("Bestiary claim", r#"{"ClaimBestiaryReward":{"group":"slime","idx":0}}"#,
             |e| matches!(e, E::ClaimBestiaryReward { group, idx: 0 } if group == "slime")),
            ("Commander grant", r#"{"GrantCommanderSkill":{"skill_id":"regent_hand","nonce":0}}"#,
             |e| matches!(e, E::GrantCommanderSkill { skill_id, .. } if skill_id == "regent_hand")),

            // MAIL is built field by field from an inbox row, so the SHAPE varies by what the mail
            // carries. Both ends of that range are pinned: a bare Gil grant and a full item.
            ("Mail claim, Gil", r#"{"ClaimMail":{"mail_id":7,"gil":500}}"#,
             |e| matches!(e, E::ClaimMail { mail_id: 7, gil, .. } if gil.get() == 500)),
            ("Mail claim, def+qty", r#"{"ClaimMail":{"mail_id":7,"def_id":"hp_pot_1","qty":3}}"#,
             |e| matches!(e, E::ClaimMail { mail_id: 7, qty: 3, def_id, .. } if def_id == "hp_pot_1")),

            // The RETIRED market events (0.0.490 tombstones): old logs still carry them, so they
            // must keep deserializing.
            ("Market list an item", r#"{"DepositItem":{"inv_idx":0,"price":1000}}"#,
             |e| matches!(e, E::DepositItem { inv_idx: 0, price: 1000 })),
            ("Market buy a listing", r#"{"BuyItemListing":{"listing_id":42,"max_price":1000}}"#,
             |e| matches!(e, E::BuyItemListing { listing_id: 42, max_price } if max_price.get() == 1000)),
            ("Market list a commodity",
             r#"{"DepositCommodity":{"def_id":"junk_moss_gel","qty":10,"unit_price":5}}"#,
             |e| matches!(e, E::DepositCommodity { def_id, qty: 10, unit_price: 5 }
                          if def_id == "junk_moss_gel")),
            ("Market buy a commodity",
             r#"{"BuyCommodity":{"def_id":"junk_moss_gel","qty":10,"max_price":5}}"#,
             |e| matches!(e, E::BuyCommodity { def_id, qty: 10, max_price: 5 }
                          if def_id == "junk_moss_gel")),

            ("Dungeon leave", r#""LeaveInstance""#, |e| matches!(e, E::LeaveInstance)),
            ("Dungeon restart", r#""RestartInstance""#, |e| matches!(e, E::RestartInstance)),
            ("Meter reset", r#""ResetDps""#, |e| matches!(e, E::ResetDps)),
            ("Login names the party", r#"{"SetPartyName":{"name":"Trial"}}"#,
             |e| matches!(e, E::SetPartyName { name } if name == "Trial")),
        ];
        for (screen, json, want) in cases {
            let parsed: E = serde_json::from_str(json)
                .unwrap_or_else(|e| panic!("{screen}: the client sends JSON the sim cannot parse.\n  {json}\n  {e}"));
            assert!(want(&parsed), "{screen}: parsed, but into the wrong shape: {json}");
        }
    }

    /// A DATA-DEPENDENT event must actually DO something through this host.
    ///
    /// The regression this pins was invisible in every way that matters. `apply_input_json` called
    /// the catalog-less `events::apply_input`, every data-dependent handler is written
    /// `if let Some(d) = data`, and so recruiting, deploying, advancing, crafting, learning a
    /// recipe and entering a dungeon PARSED, APPLIED, and changed nothing. The Godot bridge always
    /// passed its catalog, so only the hosts on this path were affected — which is every host that
    /// is not Godot. It was found by pressing a button in the Unity client and asking the state
    /// whether anything had happened.
    ///
    /// Parsing is not applying, and the contract test that checks the JSON cannot see this.
    #[test]
    fn a_data_dependent_event_actually_applies() {
        let gd = catalog();
        let mut s = Session::new(7, &gd, "zone_b1_1").expect("new");
        // BuyPartySlot is a tombstone (0.0.490): it parses and buys nothing, even with the Gil.
        s.state.earn_gil(50_000, sim_core::state::GilEarn::Debug);
        let before = s.state.party_slots();
        assert!(s.apply_input_json(r#""BuyPartySlot""#), "parses");
        assert_eq!(s.state.party_slots(), before, "slots open by progress, not Gil");
        // Open a slot the way the game does, so the deploy below has room.
        s.state.band_reached = 2;
        sim_core::data::open_earned_slots(&mut s.state);
        assert_eq!(s.state.party_slots(), before + 1, "reaching stage 2 opens slot 2");

        // The retired purchase is a tombstone: it parses and benches nobody.
        assert!(s.apply_input_json(r#"{"BuyCharacter":{"unit_def":"lucielle"}}"#), "parses");
        assert!(s.state.roster.is_empty(), "BuyCharacter is retired (0.0.413): nobody benched");

        // A unit that arrives the gacha way deploys through the same data-dependent path.
        let id = sim_core::data::grant_exemplar_unit(&mut s.state, &s.data, "lucielle").expect("granted");
        assert!(s.apply_input_json(&format!(r#"{{"DeployCharacter":{{"char_id":{id}}}}}"#)));
        assert!(s.state.roster.is_empty(), "and DeployCharacter, which needs the catalog too, moved them");
    }

    /// The FUSION FORGE's two reads answer from the sim's own rules, and the preview says exactly
    /// what pressing does — including whether the server must roll first and the binding to ask
    /// it with. A preview the client could not trust would have to be re-derived in C#, which is
    /// how a forge ends up promising a tier the sim then refuses.
    #[test]
    fn gem_rules_and_gem_plan_answer_from_the_sims_table() {
        use sim_core::items::Grade;
        let gd = catalog();
        let mut s = Session::new(7, &gd, "zone_b1_1").expect("new");
        let gd = sim_core::data::GameData::from_json_str(&gd).expect("parse");
        let rules: serde_json::Value = serde_json::from_str(&s.query_json("gem_rules")).unwrap();
        assert_eq!(rules["merge"], 6);
        assert_eq!(rules["gem_slots_by_grade"].as_array().unwrap().len(), 10);
        assert_eq!(rules["card_slots_by_grade"].as_array().unwrap().len(), 10);
        let sets = rules["sets"].as_array().unwrap();
        assert_eq!(sets.len(), gd.loot_rules.gems.sets.len());
        assert_eq!(sets[0]["stats"][0]["mid_by_grade"].as_array().unwrap().len(), 10);

        // A Grade::Legendary (step 3 — two gem sockets) sword in the bag, one Rare Strife + one Rare Life held.
        let mut sword = sim_core::data::instantiate_item(gd.item("sword_s3").unwrap());
        sword.grade = Grade::Legendary;
        s.state.inventory.push(sword);
        let idx = s.state.inventory.len() - 1;
        let strife = sim_core::gems::def_id("strife", Grade::Rare);
        let life = sim_core::gems::def_id("life", Grade::Rare);
        give_gems(&mut s.state, &s.data, &strife, 1);
        give_gems(&mut s.state, &s.data, &life, 1);
        let p: serde_json::Value = serde_json::from_str(&s.query_json(&format!("gem_plan:inv:{idx}"))).unwrap();
        assert_eq!(p["gem_slots"], 2);
        assert_eq!(p["next_socket"], 0);
        let held = p["held"].as_array().unwrap();
        let by_id = |id: &str| held.iter().find(|h| h["def_id"] == id).unwrap_or_else(|| panic!("{id} listed"));
        assert_eq!(by_id(&strife)["fits"], true, "a weapon set fits a sword");
        assert_eq!(by_id(&life)["fits"], false, "an armour set does not");
        assert!(by_id(&life)["refusal"].as_str().unwrap().contains("slot"));
        assert!(by_id(&strife)["lines"][0]["lo"].as_str().unwrap().contains('%'), "ATK% reads as a percent");

        // Socket it through the event path; the plan then shows it and the bag row carries it.
        let ev = serde_json::to_string(&sim_core::events::InputEvent::SocketGem {
            target: sim_core::gems::GemTarget::Inventory { idx: idx as u32 }, socket: 0, gem: strife.clone(),
        }).unwrap();
        assert!(s.apply_input_json(&ev), "the wire form round-trips: {ev}");
        let p: serde_json::Value = serde_json::from_str(&s.query_json(&format!("gem_plan:inv:{idx}"))).unwrap();
        assert_eq!(p["next_socket"], 1);
        assert_eq!(p["socketed"][0]["def_id"], strife);
        assert_eq!(p["socketed"][0]["lines"].as_array().unwrap().len(), 2);
        let bag: serde_json::Value = serde_json::from_str(&s.query_json("inventory")).unwrap();
        let row = &bag[idx];
        assert_eq!(row["gem_slots"], 2);
        assert_eq!(row["gem_lines"].as_array().unwrap().len(), 1);
        assert_eq!(row["market_clean"], false);
        assert_eq!(s.query_json(&format!("gem_plan:eq:999,0")), "{}", "an unknown target answers {{}}");
    }

    #[test]
    fn fusion_preview_answers_from_the_sims_rules_and_names_the_server_roll() {
        use sim_core::items::{Grade, Item, ItemKind, Rarity};
        let gd = catalog();
        let mut s = Session::new(7, &gd, "zone_b1_1").expect("new");
        let rules: serde_json::Value = serde_json::from_str(&s.query_json("fuse_rules")).unwrap();
        assert_eq!(rules["inputs"], 6);
        assert_eq!(rules["ceiling_step"], sim_core::fusion::FUSE_GRADE_CEILING.step());
        assert_eq!(rules["server_roll_from_step"], sim_core::fusion::FUSE_SERVER_ROLL_FROM.step());

        // Six Uncommon swords at the front of the bag: Uncommon → Rare needs no server roll.
        let n0 = s.state.inventory.len() as u32;
        for def in ["sword_s1", "sword_s1", "sword_s1", "sword_s3", "sword_s3", "sword_s3"] {
            let mut it = Item::new(def, ItemKind::Weapon, 10, 0, Rarity::Common);
            it.grade = Grade::Uncommon;
            s.state.inventory.push(it);
        }
        let key = format!("fuse_preview:{}", (n0..n0 + 6).map(|i| i.to_string()).collect::<Vec<_>>().join(","));
        let p: serde_json::Value = serde_json::from_str(&s.query_json(&key)).unwrap();
        assert_eq!(p["tier"], 2, "three s1 + three s3 average to 2.0");
        assert_eq!(p["tier_up_bp"], 0);
        assert_eq!(p["fail_bp"], 0, "Uncommon inputs are free");
        assert_eq!(rules["blacksmith_level"], 0);
        assert_eq!(rules["fail_bp_now_by_grade"][9], 5_000);
        assert_eq!(p["grade"], "Rare");
        assert_eq!(p["needs_server_roll"], false);
        assert!(p["key"].as_str().unwrap().contains("sword_s1#Uncommon"), "the binding is still named");

        // Six Rare: Rare → Legendary is over the line, so the forge must ask `/fuse/roll` first.
        for it in s.state.inventory.iter_mut().skip(n0 as usize) {
            it.grade = Grade::Rare;
        }
        let p: serde_json::Value = serde_json::from_str(&s.query_json(&key)).unwrap();
        assert_eq!(p["needs_server_roll"], true);
        assert_eq!(p["attempt"], 0, "no fusion run yet");

        // A bad pick set is `{}`, never a guess.
        let bad: serde_json::Value = serde_json::from_str(&s.query_json("fuse_preview:0,0,0,0,0,0")).unwrap();
        assert!(bad.as_object().is_some_and(|o| o.is_empty()));
        let short: serde_json::Value = serde_json::from_str(&s.query_json("fuse_preview:1,2")).unwrap();
        assert!(short.as_object().is_some_and(|o| o.is_empty()));
    }

    /// The cheats must EDIT rather than be INPUTS, and must stop when the session goes online.
    ///
    /// Both halves matter. If a cheat were an `InputEvent` it would replay cleanly and the server
    /// would accept a fabricated fortune as legitimate play — the input-authority question is
    /// exactly "is every event safe to author". Because these edit the state directly they are
    /// absent from the log, so a cheated save simply stops verifying, which is the right outcome.
    /// And the online gate refuses them up front rather than letting someone cheat and then find
    /// their whole session unpushable.
    #[test]
    fn cheats_edit_the_state_and_stop_at_the_door() {
        let gd = catalog();
        let mut s = Session::new(7, &gd, "zone_b1_1").expect("new");
        let before = s.state.gil();
        let log_before = s.state.input_log.len();

        assert!(s.debug_json(r#"{"give_currency":{"gil":5000,"soul":10}}"#), "offline: allowed");
        assert_eq!(s.state.gil(), before + 5000, "the edit landed");
        assert_eq!(s.state.input_log.len(), log_before,
                   "a direct edit must NOT enter the input log — that is what makes a cheated \
                    save stop verifying instead of replaying as legitimate play");

        assert!(!s.debug_json(r#"{"no_such_cheat":{}}"#), "an unknown command is refused, not a panic");

        s.set_allow_debug(false);
        let gil = s.state.gil();
        assert!(!s.debug_json(r#"{"give_currency":{"gil":5000,"soul":0}}"#), "online: refused");
        assert_eq!(s.state.gil(), gil, "and refused means nothing happened");
    }

    /// The whole online contract through the shared lifecycle: a session that ticks, takes a
    /// JSON input, exports a bundle — and that bundle VERIFIES by replay, exactly what the server
    /// does. If this ever fails, every host's saves would be rejected.
    #[test]
    fn exported_bundle_verifies_by_replay() {
        let gd = catalog();
        let mut s = Session::new(7, &gd, "zone_b1_1").expect("new");
        s.tick(300);
        assert!(s.apply_input_json(r#"{"ResetDps":null}"#), "a serde-shaped event must parse");
        s.tick(300);
        let json = s.export_log();
        assert!(!json.is_empty());
        let bundle: replay::SaveBundle = serde_json::from_str(&json).unwrap();
        assert!(bundle.verify(&s.data), "bundle from the shared lifecycle failed server-side verify");
        // Re-anchoring after a confirmed push shrinks the span the NEXT bundle would replay.
        assert_eq!(s.replay_span(), 600);
        s.commit_anchor();
        assert_eq!(s.replay_span(), 0);
        s.tick(50);
        assert_eq!(s.replay_span(), 50);
        let again = s.export_log();
        let b2: replay::SaveBundle = serde_json::from_str(&again).unwrap();
        assert!(b2.verify(&s.data), "post-anchor bundle must still verify");
    }

    /// Save → load round-trips to the same hash, and the loaded session is armed for replay.
    #[test]
    fn load_round_trip_keeps_hash_and_arms_recording() {
        let gd = catalog();
        let mut s = Session::new(11, &gd, "zone_b1_1").unwrap();
        s.tick(120);
        let h = s.state_hash();
        let l = Session::load(&s.save_json(), &gd).expect("load");
        assert_eq!(l.state_hash(), h);
        assert!(l.state.record_inputs);
        assert_eq!(l.state.snapshot_interval, replay::SNAPSHOT_INTERVAL_TICKS);
        assert_eq!(l.replay_span(), 0);
    }

    /// Garbage input is refused, not a panic; the view is always parseable JSON.
    #[test]
    fn bad_input_is_refused_and_view_is_json() {
        let gd = catalog();
        let mut s = Session::new(3, &gd, "zone_b1_1").unwrap();
        assert!(!s.apply_input_json("{ nope"));
        let v: serde_json::Value = serde_json::from_str(&s.view_json()).expect("view is JSON");
        assert!(v.is_object());
        assert!(Session::new(3, "{ not a catalog", "zone_b1_1").is_err());
        assert!(Session::new(3, &gd, "no_such_zone").is_err());
    }
}
