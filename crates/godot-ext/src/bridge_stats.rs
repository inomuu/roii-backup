// SimBridge — derived-stat / skill / buff / soul-path query methods +
// the generic input bridge.
//
// A SECONDARY `#[godot_api]` impl block (see bridge_world.rs for why the
// methods are split across modules this way).

// `use super::*` glob-imports the crate-root items the gdext `(secondary)`
// macro needs in scope: the `SimBridge` struct, its funcs-collection struct,
// and the module-local `__registration_*` statics the primary impl declares.
#[allow(unused_imports)]
use super::*;
use sim_core::events;

#[godot_api(secondary)]
impl SimBridge {
    /// Toggle whether a character's learned ACTIVE skill is enabled (slotted)
    /// for casting — disable a skill that's bad here (e.g. a Fire skill in a
    /// Water zone). Empty loadout = all actives fire.
    #[func]
    fn toggle_active_skill(&mut self, entity_id: i64, skill_id: GString) {
        if let Some(s) = self.state.as_mut() {
            events::apply_input(
                s,
                events::InputEvent::ToggleActiveSkill {
                    entity_id: entity_id as u32,
                    skill_id: skill_id.to_string(),
                },
            );
        }
    }

    /// DPS-meter snapshot as JSON: `{elapsed_ticks, by_class:[{name,total}],
    /// by_skill:[{name,total}]}`, each list sorted by total desc. GDScript
    /// divides total by elapsed seconds for live DPS; skill ids map to names.
    #[func]
    fn dps_json(&self) -> GString {
        let Some(s) = self.state.as_ref() else {
            return GString::from("{}");
        };
        let d = &s.dps;
        let elapsed = s.tick.saturating_sub(d.start_tick);
        let rows = |m: &std::collections::BTreeMap<String, u64>, is_skill: bool| {
            let mut r: Vec<serde_json::Value> = m
                .iter()
                .map(|(k, v)| {
                    let name = if is_skill && sim_core::skills::skill_def(k).is_some() {
                        sim_core::skills::skill_display(k).0.to_string()
                    } else {
                        k.clone()  // "Basic Attack" or a class id — keep as-is
                    };
                    serde_json::json!({ "name": name, "total": v })
                })
                .collect();
            r.sort_by(|a, b| b["total"].as_u64().cmp(&a["total"].as_u64()));
            r
        };
        let out = serde_json::json!({
            "elapsed_ticks": elapsed,
            "by_class": rows(&d.by_class, false),
            "by_skill": rows(&d.by_skill, true),
        });
        GString::from(out.to_string())
    }

    /// LIFETIME account tallies as flat JSON — `Measures` plus `Records`.
    ///
    /// Its own method rather than a `view_state` key: `view_state` is read every frame and none
    /// of this changes at a rate anyone can see, so a screen that opens occasionally has no
    /// business riding the per-frame payload.
    ///
    /// Named `lifetime_` to keep three different "stats" apart — this is cumulative-forever,
    /// `dps_json` is a resettable window, and a character's STR/AGI/VIT is something else again.
    ///
    /// SCOPE, and the screen should say so: these are what the SIM saw. Gacha pulls, market
    /// trades and real-money purchases happen server-side and are not in here.
    #[func]
    fn lifetime_stats_json(&self) -> GString {
        let Some(s) = self.state.as_ref() else {
            return GString::from("{}");
        };
        let m = &s.measures;
        let out = serde_json::json!({
            // ELAPSED time, which is already counted: `tick` is the sim's own clock and needs no
            // new field. It measures time the WORLD advanced, which in an idle game includes any
            // offline catch-up — that is the number a player means by "how long have I played",
            // not wall-clock with the window focused.
            "total_ticks": s.tick,
            // combat
            "kills": m.kills,
            "boss_kills": m.boss_kills,
            "rooms_cleared": m.rooms_cleared,
            "bands_cleared": m.bands_cleared,
            "damage_dealt": m.damage_dealt,
            "damage_taken": m.damage_taken,
            "healing_done": m.healing_done,
            "skills_cast": m.skills_cast,
            "deaths": m.deaths,
            "wipes": m.wipes,
            "biggest_hit": s.records.biggest_hit,
            // building
            "levels_gained": m.levels_gained,
            "items_crafted": m.items_crafted,
            "orbs_used": m.orbs_used,
            "cards_fed": m.cards_fed,
            "exemplars_fed": m.exemplars_fed,
            // economy
            "gil_earned": m.gil_earned,
            "gil_spent": m.gil_spent,
            "gil_received": m.gil_received,
            "gil_sent": m.gil_sent,
            "items_listed": m.items_listed,
        });
        GString::from(out.to_string())
    }

    /// Reset the live DPS meter (clear totals, restart the window).
    #[func]
    fn reset_dps(&mut self) {
        if let Some(s) = self.state.as_mut() {
            events::apply_input(s, events::InputEvent::ResetDps);
        }
    }

    /// Display card for ONE skill id that lives OUTSIDE every class tree (an
    /// Exemplar-granted signature): one node shape for every skill
    /// (`{id, name, blurb, detail, kind, role, slot_cost, max_level}`), so the skill
    /// page can render it with the identical card path. `{}` for an unknown id.
    #[func]
    fn skill_info_json(&self, skill_id: GString) -> GString {
        let sid = skill_id.to_string();
        let Some(def) = sim_core::skills::skill_def(&sid) else {
            return GString::from("{}");
        };
        let (name, blurb) = sim_core::skills::skill_display(&sid);
        let kind = match def.kind {
            sim_core::skills::SkillKind::Active => "active",
            _ => "passive",
        };
        let role = if !matches!(def.kind, sim_core::skills::SkillKind::Active) {
            "passive"
        } else if def.heals_ally {
            "heal"
        } else if def.power_bp > 0 || def.power_per_level_bp > 0 {
            "damage"
        } else {
            "buff"
        };
        let slot_cost =
            if kind == "active" { sim_core::skills::loadout_slot_cost(&sid) } else { 0 };
        // Field skills surface their area/duration through the panel's existing
        // "special" rows (Area around caster NxN + Duration), so the inspect panel
        // reads complete without a bespoke renderer.
        let special = if def.ground_field_radius > 0 {
            serde_json::json!({
                "radius": def.ground_field_radius,
                "lock_ticks": def.field_duration_ticks,
            })
        } else {
            serde_json::Value::Null
        };
        let v = serde_json::json!({
            "id": sid,
            "name": name,
            "blurb": blurb,
            "detail": sim_core::skills::skill_detail(&sid),
            "kind": kind,
            "role": role,
            "slot_cost": slot_cost,
            "anim_slot": def.anim_slot,
            // Does the caster BLINK to its target rather than walk? The renderer cuts the
            // position instead of gliding it, so a teleport does not render as a fast dash.
            // The SIM's own flag, not a guess from how far the entity moved: a multi-tick
            // catch-up burst also moves several cells at once, and cutting those would make
            // ordinary movement stutter exactly when the client is already behind.
            "teleport": def.teleport,
            "max_level": def.max_level,
            "prereq_id": "",
            "prereq_level": 0,
            // The full numeric payload the skill-detail panel renders (same keys as a
            // one node shape for every skill, so the panel needs no special casing).
            "cooldown_ticks": def.cooldown_ticks,
            "cast_time_ticks": def.cast_time_ticks,
            "range": def.range,
            "aoe_radius": def.aoe_radius,
            "self_aoe_radius": def.self_aoe_radius,
            "knockback": def.knockback,
            "power_bp": def.power_bp,
            "power_per_level_bp": def.power_per_level_bp,
            "hp_regen_flat": def.hp_regen_flat,
            "hp_regen_maxhp_bp": def.hp_regen_maxhp_bp,
            "atk_flat_per_level": def.atk_flat_per_level,
            "dodge_bp_per_level": def.dodge_bp_per_level,
            "double_atk_chance_bp": def.double_atk_chance_bp,
            "element": def.cast_element.map(|e| format!("{e:?}")).unwrap_or_default(),
            "special": special,
        });
        GString::from(v.to_string())
    }

    /// The learnable skill TREE for a class, as JSON: an array of
 
    /// LOADOUT slot capacity for the entity (a fixed budget since 0.0.315).
    #[func]
    fn loadout_capacity(&self, entity_id: i64) -> i64 {
        let Some(s) = self.state.as_ref() else { return 0; };
        let Some(i) = s.grid.idx_of_id(entity_id as u32) else { return 0; };
        sim_core::combat::loadout_capacity(&s.grid.entities[i]) as i64
    }

    /// LOADOUT SLOTS the entity's loadout currently REQUESTS (sum of every
    /// slotted active's cost). May exceed capacity — then the loadout tail is inert.
    #[func]
    fn loadout_used(&self, entity_id: i64) -> i64 {
        let Some(s) = self.state.as_ref() else { return 0; };
        let Some(i) = s.grid.idx_of_id(entity_id as u32) else { return 0; };
        s.grid.entities[i]
            .active_skills
            .iter()
            .map(|id| sim_core::skills::loadout_slot_cost(id) as i64)
            .sum()
    }

    /// Base XP required to advance FROM `level` to `level + 1` — the sim's
    /// QUADRATIC curve, exposed so the HUD's EXP bars read the single source of
    /// truth instead of mirroring the formula.
    #[func]
    fn xp_to_next(&self, level: i64) -> i64 {
        sim_core::tick::xp_to_next(level.max(1) as u32) as i64
    }


    /// Every DERIVED combat stat the Character window + party frames display,
    /// computed by the SIM's `effective_*` functions — the single source of
    /// truth so a formula change in Rust can never drift the readout. `atk`/`def`
    /// are the full effective totals (incl. stats, gear, refine, buffs, auras);
    /// `atk_equip`/`def_equip` are the equipment-only portions so the view can
    /// still show RO's "status + equip" split (status = total - equip). `aspd` is
    /// the displayed ASPD value (50..193), inverted from the swing accumulator.
    /// `crit_bp` is basis points (view divides by 100 for a %). `{}` if unknown.
    #[func]
    fn effective_stats_json(&self, entity_id: i64) -> GString {
        use sim_core::combat::*;
        let Some(s) = self.state.as_ref() else { return "{}".into(); };
        let Some(i) = s.grid.idx_of_id(entity_id as u32) else { return "{}".into(); };
        let e = &s.grid.entities[i];
        let atk_equip: i32 = e
            .equipped
            .iter()
            .map(|it| it.graded_atk())
            .sum();
        let def_equip: i32 = e
            .equipped
            .iter()
            .map(|it| it.graded_def())
            .sum();
        // ATTACK SPEED as attacks/sec ×100 (0.0.367): the accumulator gains
        // `milli` per 100 ms tick and spends 1000 per swing, so milli IS
        // hundredths of an attack per second — no inversion, no curve to
        // re-derive. The client divides by 100 and prints "1.06 /s".
        let atk_speed_x100 = effective_atk_speed_milli(e) as i64;
        let obj = serde_json::json!({
            "max_hp": effective_max_hp(e),
            "lb_max": sim_core::skills::ultimate_of(&e.skills).map_or(0, |d| d.lb_max) as i32,
            "atk": effective_atk(e),
            "atk_equip": atk_equip,
            "def": effective_def(e),
            "def_equip": def_equip,
            "dodge_bp": sim_core::combat::stat_mods::dodge_bp(e),
            "crit_bp": effective_crit_chance_bp(e),
            // Crit DAMAGE multiplier (0.0.224 renewal-crit): 15000 = ×1.5 base;
            // card/gear crit-dmg lines raise it. View renders `bp / 100`%.
            "crit_dmg_bp": sim_core::combat::effective_crit_dmg_bp(e),
            // EXTRA defensive/QoL stats (stat-panel block; all bp, all computed by
            // the sim). `mitigation_bp` = the hard-DEF renewal-curve reduction.
            "mitigation_bp": sim_core::combat::def_mitigation_bp(e),
            "basic_dr_bp": sim_core::combat::card_specials::basic_dmg_taken_reduction_bp(e),
            "ignore_def_bp": sim_core::combat::stat_mods::ignore_def_bp(e),
            "cdr_bp": sim_core::combat::stat_mods::cooldown_bp(e),
            "cast_bp": sim_core::combat::stat_mods::cast_bp(e),
            "move_bp": sim_core::combat::stat_mods::move_speed_bp(e),
            "heal_bp": sim_core::combat::stat_mods::heal_bp(e),
            // Non-zero per-element / per-status resists only (usually empty).
            "resists": resists_json(e),
            "status_resists": status_resists_json(e),
            "atk_speed_x100": atk_speed_x100,
            // APEX (0.0.228): earned/unspent points + per-bucket allocation, plus the
            // level/xp the Apex screen needs — at LEVEL_CAP the same `xp` bar fills
            // toward the next Apex point (`apex_cost`). One call feeds the whole panel.
            "level": e.stats.level,
            "xp": e.stats.xp,
            "level_cap": sim_core::tick::LEVEL_CAP,
            "level_points": e.stats.level_points,
            "core_unspent": e.stats.core_unspent(),
            "apex_level": e.stats.apex_level,
            "apex_unspent": e.stats.apex_unspent(),
            "apex_alloc": e.stats.apex.to_vec(),
            "apex_cost": sim_core::tick::apex_xp_cost(e.stats.apex_level),
            "apex_level_cap": sim_core::tick::APEX_LEVEL_CAP,
            "apex_bucket_cap": sim_core::stats::APEX_BUCKET_CAP,
        });
        serde_json::to_string(&obj).unwrap_or_default().into()
    }

    /// Spend `count` unspent Apex Points into `bucket` (0..9) on `entity_id`.
    /// EVENTED via `InputEvent::AllocateApex` (replay-safe; clamps to the pool).
    #[func]
    fn allocate_apex(&mut self, entity_id: i64, bucket: i64, count: i64) -> bool {
        let Some(s) = self.state.as_mut() else { return false; };
        events::apply_input(
            s,
            events::InputEvent::AllocateApex {
                entity_id: entity_id as u32,
                bucket: bucket.clamp(0, 255) as u8,
                count: count.max(0) as u32,
            },
        );
        true
    }

    /// Refund all spent Apex Points on `entity_id` (every bucket → 0).
    #[func]
    fn reset_apex(&mut self, entity_id: i64) -> bool {
        let Some(s) = self.state.as_mut() else { return false; };
        events::apply_input(s, events::InputEvent::ResetApex { entity_id: entity_id as u32 });
        true
    }

    // --- XP REFUND (0.0.349) — the Engram reserve and the three roster-only inputs. ---

    /// `{owned: [4], tiers: [4], ceiling_level, ceiling_apex}` — the Engram stock per tier, each
    /// tier's XP value, and how far a benched unit may be poured (the account's proven ceiling).
    #[func]
    fn engrams_json(&self) -> GString {
        let Some(s) = self.state.as_ref() else { return GString::default(); };
        let (lvl, apx) = sim_core::data::refund_ceiling(s);
        serde_json::to_string(&serde_json::json!({
            "owned": s.owned_engrams,
            "tiers": sim_core::state::ENGRAM_TIERS,
            "ceiling_level": lvl,
            "ceiling_apex": apx,
        }))
        .unwrap_or_default()
        .into()
    }

    /// What "fill to max" WOULD do to benched `unit_id`: `{plan: [4], resulting_level,
    /// resulting_apex, xp_wasted}`, computed by the sim without mutating. "null" if not benched.
    #[func]
    fn engram_preview_json(&self, unit_id: i64) -> GString {
        let Some(s) = self.state.as_ref() else { return "null".into(); };
        sim_core::data::engram_fill_preview(s, unit_id as u32)
            .and_then(|p| serde_json::to_string(&p).ok())
            .unwrap_or_else(|| "null".to_string())
            .into()
    }

    /// Drain benched `unit_id`'s base progression into Engrams and return it to Lv1.
    /// EVENTED via `InputEvent::RefundUnitXp` (replay-safe; no-op unless benched).
    #[func]
    fn refund_unit_xp(&mut self, unit_id: i64) -> bool {
        let Some(s) = self.state.as_mut() else { return false; };
        events::apply_input(s, events::InputEvent::RefundUnitXp { unit_id: unit_id as u32 });
        true
    }

    /// Spend `count` Engrams of `tier` (0..=3) on benched `unit_id`. EVENTED via `ApplyEngram`.
    #[func]
    fn apply_engram(&mut self, unit_id: i64, tier: i64, count: i64) -> bool {
        let Some(s) = self.state.as_mut() else { return false; };
        events::apply_input(
            s,
            events::InputEvent::ApplyEngram {
                unit_id: unit_id as u32,
                tier: tier.clamp(0, 255) as u8,
                count: count.max(0) as u32,
            },
        );
        true
    }

    /// Fill benched `unit_id` to the ceiling with the plan `engram_preview_json` showed.
    /// EVENTED via `ApplyEngramsToMax`.
    #[func]
    fn apply_engrams_to_max(&mut self, unit_id: i64) -> bool {
        let Some(s) = self.state.as_mut() else { return false; };
        events::apply_input(s, events::InputEvent::ApplyEngramsToMax { unit_id: unit_id as u32 });
        true
    }

    /// Static metadata for the stat-point buckets, in index order — so the screen
    /// renders per-point effects from the SIM's own `APEX_PER_PT` table (no GDScript
    /// drift). Each: {loc: name key, per_pt: magnitude/point, bp: true if the value
    /// is basis points (render /100 as %), tier: "core" | "substat"}. TIERS: indices
    /// 0..APEX_CORE are bought with level-up points, the rest with post-cap Apex
    /// points (docs/apex-from-level-1-design.md).
    #[func]
    fn apex_buckets(&self) -> GString {
        use sim_core::stats as st;
        let row = |i: usize, loc: &str, bp: bool| {
            serde_json::json!({
                "loc": loc,
                "per_pt": st::APEX_PER_PT[i],
                "bp": bp,
                "tier": if i < st::APEX_CORE { "core" } else { "substat" },
            })
        };
        let b = serde_json::json!([
            row(st::AP_ATK,        "UI_APEX_ATK",        false),
            row(st::AP_HP,         "UI_APEX_HP",         false),
            row(st::AP_DEF,        "UI_APEX_DEF",        false),
            row(st::AP_CRIT,       "UI_APEX_CRIT",       true),
            row(st::AP_CRITDMG,    "UI_APEX_CRITDMG",    true),
            row(st::AP_DODGE,      "UI_APEX_DODGE",      true),
            row(st::AP_DEFPEN,     "UI_APEX_DEFPEN",     true),
            row(st::AP_BREAKDMG,   "UI_APEX_BREAKDMG",   true),
            row(st::AP_BREAKBUILD, "UI_APEX_BREAKBUILD", true),
            row(st::AP_COOLDOWN,   "UI_APEX_COOLDOWN",   true),
            row(st::AP_CAST,       "UI_APEX_CAST",       true),
            row(st::AP_CHARGE,     "UI_APEX_CHARGE",     true),
            row(st::AP_HEAL,       "UI_APEX_HEAL",       true),
        ]);
        serde_json::to_string(&b).unwrap_or_default().into()
    }

    /// The entity's ACTIVE-skill loadout as JSON: `[{id, name, cd, cd_max}]` in
    /// slot order, where `cd` is the ticks of cooldown remaining and `cd_max`
    /// the skill's full cooldown. Drives the character-window skill bar (icon +
    /// cooldown overlay). Empty array for an unknown entity / empty loadout.
    #[func]
    fn loadout_json(&self, entity_id: i64) -> GString {
        let Some(s) = self.state.as_ref() else { return "[]".into(); };
        let Some(i) = s.grid.idx_of_id(entity_id as u32) else { return "[]".into(); };
        let e = &s.grid.entities[i];
        let arr: Vec<serde_json::Value> = e
            .active_skills
            .iter()
            .map(|id| {
                let st = e.skills.iter().find(|sk| &sk.id == id);
                let cd = st.map_or(0, |sk| sk.cooldown);
                let lvl = st.map_or(0, |sk| sk.level);
                let cd_max = sim_core::skills::skill_def(id).map_or(0, |d| d.cooldown_ticks);
                let (name, blurb) = sim_core::skills::skill_display(id);
                serde_json::json!({
                    "id": id, "name": name, "blurb": blurb,
                    "level": lvl, "cd": cd, "cd_max": cd_max,
                })
            })
            .collect();
        serde_json::to_string(&arr).unwrap_or_default().into()
    }

    /// Every active buff/debuff on the entity, aggregated by KIND (so the same
    /// kind from several sources — a path aura + a potion + a skill — shows as
    /// ONE entry): `[{kind, magnitude, ticks}]`, ticks = the longest remaining.
    /// Combines ALL sources (consumables, skills, path/party auras). Re-stamped
    /// auras read as ticks <= 2 (the UI treats those as persistent).
    #[func]
    fn buffs_json(&self, entity_id: i64) -> GString {
        let Some(s) = self.state.as_ref() else { return "[]".into(); };
        let Some(i) = s.grid.idx_of_id(entity_id as u32) else { return "[]".into(); };
        let mut map: std::collections::BTreeMap<String, (i32, u32)> =
            std::collections::BTreeMap::new();
        for b in &s.grid.entities[i].buffs {
            // Skip PATH-of-Soul auras (always-on, party-wide) — they'd be noise.
            // Skill auras (Blessing/Agility Up) + potions + timed casts stay.
            if b.source_id == sim_core::soul_paths::PATH_AURA_SOURCE {
                continue;
            }
            // AGGREGATE THE WAY THE SIM DOES, which is not one rule but two. `effective_bp` sends
            // every non-DoT DEBUFF through `StackPolicy::Strongest` (two monsters landing a 30%
            // slow is a 30% slow) and everything else through `PerSource`, which adds. Summing
            // blindly over-reported every debuff by the number of sources; taking `max` blindly
            // under-reported two identical potions. Branch on the kind's own policy and the
            // readout matches the damage being dealt in both directions.
            let entry = map.entry(format!("{:?}", b.kind)).or_insert((0, 0));
            entry.0 = match b.kind.stack_policy() {
                sim_core::skills::StackPolicy::Strongest => entry.0.max(b.magnitude),
                sim_core::skills::StackPolicy::PerSource => entry.0.saturating_add(b.magnitude),
            };
            entry.1 = entry.1.max(b.ticks);
        }
        let mut arr: Vec<serde_json::Value> = map
            .iter()
            .map(|(k, (mag, ticks))| serde_json::json!({ "kind": k, "magnitude": mag, "ticks": ticks }))
            .collect();
        // Bloodthirst stacks aren't a Buff entry (they live on the entity) — surface
        // them as a synthetic buff so the buff bar shows the icon + current stacks.
        let bt = s.grid.entities[i].bloodthirst_stacks;
        if bt > 0 {
            arr.push(serde_json::json!({
                "kind": "Bloodthirst", "magnitude": bt as i32, "ticks": 0, "stacks": bt as i32,
            }));
        }
        // Absorb shields are pools (not Buffs) — surface them as synthetic buffs so
        // the icon + remaining pool show. The two types are told apart by the hit
        // cap: Flame Shield = the holder's own pool-only shield (hits_left 0);
        // Praesidium = a hit-capped shield (hits_left > 0), cast onto this ally by
        // a Bishop (so its src_id is the caster, not this entity).
        let self_id = entity_id as u32;
        if let Some(sh) = s.grid.entities[i]
            .shields
            .iter()
            .find(|sh| sh.src_id == self_id && sh.hits_left == 0)
        {
            arr.push(serde_json::json!({
                "kind": "FlameShield", "magnitude": sh.pool, "ticks": 0,
            }));
        }
        if let Some(sh) = s.grid.entities[i].shields.iter().find(|sh| sh.hits_left > 0) {
            arr.push(serde_json::json!({
                "kind": "Praesidium", "magnitude": sh.pool, "ticks": 0, "stacks": sh.hits_left,
            }));
        }
        serde_json::to_string(&arr).unwrap_or_default().into()
    }

    /// PATHS OF SOUL web as JSON for the UI: an array of
    /// `{id, label, effect, magnitude, cost, x, y, neighbors, owned,
    /// allocatable}`. The UI draws the graph (positions + edges) and unlock
    /// state straight from this; no node table lives in GDScript.
    #[func]
    fn soul_paths_json(&self) -> GString {
        use sim_core::soul_paths::{self, PathEffect};
        let Some(s) = self.state.as_ref() else { return GString::from("[]"); };
        let soul = s.soul();
        let owned = &s.soul_paths;
        let effect_name = |e: PathEffect| match e {
            PathEffect::None => "",
            PathEffect::AtkPct => "ATK%",
            PathEffect::DefPct => "DEF%",
            PathEffect::DefAdd => "DEF",
            PathEffect::HpPct => "HP%",
            PathEffect::HpAdd => "MaxHP",
            PathEffect::AtkAdd => "ATK",
            PathEffect::ExpPct => "EXP%",
            PathEffect::GilPct => "Gil%",
            PathEffect::SoulPct => "Soul%",
            PathEffect::StashCap => "Stash",
            PathEffect::AutoSellOverflow => "AutoSell%",
            PathEffect::CritBp => "CRIT%",
            PathEffect::DodgeBp => "Dodge%",
        };
        let arr: Vec<serde_json::Value> = soul_paths::SOUL_PATHS
            .iter()
            .map(|n| {
                let is_owned = soul_paths::is_owned(owned, n.id);
                let allocatable = soul_paths::is_allocatable(owned, n);
                serde_json::json!({
                    "id": n.id,
                    "label": n.label,
                    "effect": effect_name(n.effect),
                    "magnitude": n.magnitude,
                    "cost": n.cost,
                    "x": n.x,
                    "y": n.y,
                    "neighbors": n.neighbors,
                    "owned": is_owned,
                    "allocatable": allocatable && soul >= n.cost,
                })
            })
            .collect();
        serde_json::to_string(&arr).unwrap_or_default().into()
    }

    /// Apply a player input. `event_json` is the JSON form of
    /// `sim_core::events::InputEvent`. Returns `true` if the event
    /// parsed and applied; `false` on parse error or no state.
    ///
    /// Invalid (but well-formed) events are themselves silent no-ops
    /// inside the sim — replay safety (GDD §4.4).
    #[func]
    fn apply_input_json(&mut self, event_json: GString) -> bool {
        let json = event_json.to_string();
        let Ok(ev) = serde_json::from_str::<events::InputEvent>(&json) else {
            return false;
        };
        let Some(s) = self.state.as_mut() else { return false; };
        // Pass the catalog so DATA-DEPENDENT events (ChangeClass, future
        // ChangeZone/recruit/buy) resolve their defs; data-free events ignore it.
        events::apply_input_with(s, self.data.as_ref(), ev);
        true
    }

    /// BESTIARY: one entry per group (same-model families share a pool).
    /// `{milestones: [[kills, gil, soul],..], entries: [{key, name, level,
    /// race, size, element, hp, atk, def, kills, claimed, variants}]}` —
    /// entries only for monsters with a display name ("official" content).
    #[func]
    fn bestiary_json(&self) -> GString {
        let (Some(d), Some(st)) = (self.data.as_ref(), self.state.as_ref()) else {
            return GString::from("{}");
        };
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
                // Monsters have no element since 0.0.444 — weaknesses only. The Godot
                // bestiary still reads "element" for its label, so it gets the first weakness
                // (or Neutral), the same rule the Unity bestiary payload uses.
                "element": rep.weaknesses.first().cloned().unwrap_or_else(|| "Neutral".into()),
                "weaknesses": rep.weaknesses,
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
            "milestones": sim_core::state::BESTIARY_MILESTONES
                .iter().map(|(k, g, s)| serde_json::json!([k, g, s]))
                .collect::<Vec<_>>(),
            "entries": entries,
        });
        serde_json::to_string(&out).unwrap_or_default().into()
    }
}

/// Non-zero gear elemental resists as `{ "Fire": bp, ... }` (empty for most
/// entities — the panel shows one row per entry).
fn resists_json(e: &sim_core::grid::Entity) -> serde_json::Value {
    use sim_core::grid::Element as El;
    let mut m = serde_json::Map::new();
    for el in [El::Fire, El::Earth, El::Ice, El::Wind, El::Light,
               El::Dark, El::Poison, El::Thunder, El::Spirit] {
        let bp = sim_core::combat::stat_mods::resist_element_bp(e, el);
        if bp != 0 {
            m.insert(format!("{el:?}"), serde_json::json!(bp));
        }
    }
    serde_json::Value::Object(m)
}

/// Non-zero gear status resists as `{ "Freeze": bp, ... }`.
fn status_resists_json(e: &sim_core::grid::Entity) -> serde_json::Value {
    use sim_core::combat::stat_mods as gm;
    let mut m = serde_json::Map::new();
    for (code, name) in [(gm::STATUS_FREEZE, "Freeze"), (gm::STATUS_POISON, "Poison"),
                         (gm::STATUS_BLIND, "Blind"), (gm::STATUS_STUN, "Stun")] {
        let bp = gm::resist_status_bp(e, code);
        if bp != 0 {
            m.insert(name.to_string(), serde_json::json!(bp));
        }
    }
    serde_json::Value::Object(m)
}
