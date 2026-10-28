//! `view_state` — the ONE read interface the client has, and the widest untyped surface in the
//! project. `serde_json::to_value(GameState)` gives the base shape; everything below INJECTS
//! derived fields the sim does not store (`eff_max_hp`, `mon_name`, `sprite`, `stash_count`, the
//! per-tick FX arrays …) and TRIMS the big lists to counts.
//!
//! WHY THIS IS ITS OWN MODULE: it was 356 lines inside `lib.rs`, and GDScript reads its output
//! with `.get(key, default)` in **1,439 places**. A renamed or dropped key therefore does not
//! error anywhere — the panel just renders empty, and no test noticed (CLAUDE.md lists this as
//! coverage gap #1). Splitting it out is what makes `contract_keys_are_stable` below possible:
//! that test pins the emitted key set, so removing or renaming a key fails CI instead of
//! silently blanking a UI panel months later.
//!
//! Adding a key is free — the test only guards against LOSS. Renaming or removing one is a
//! deliberate act that must be paired with the GDScript reader and a re-pin of the list.

use sim_core::data;
use sim_core::state::GameState;

use crate::item_view::item_to_view;

/// Build the client-facing view JSON. `None` only if `GameState` itself fails to serialize.
pub fn build_view_state(s: &GameState, data: Option<&data::GameData>) -> Option<serde_json::Value> {
    // The stash can grow into the thousands over a long session; sending the whole list every
    // tick (this is called ~10x/s) is what made the late game stutter. Stripped to a
    // `stash_count` below — the inventory UI fetches the full list via `stash_json()`.
    let mut v = serde_json::to_value(s).ok()?;
    if let Some(obj) = v.as_object_mut() {
        // ZONE_SPAWN → only the five fields the client reads. Measured 2026-07-31: the full
        // struct is 45 KB of the payload's 74 KB — 61% — because `pool` holds a whole
        // MonsterTemplate per entry, drop tables included, and every DropEntry carries a FULL
        // `Item`. None of it changes until the party changes zone, yet it was re-sent and
        // re-parsed by Godot ~10x a second.
        //
        // Trimming here fixes the two costs that dominate: bytes across the FFI and
        // `JSON.parse_string` on the far side. It does NOT save the Rust-side serialization —
        // `to_value` above already built the whole thing before we throw it away — same
        // limitation the stash trim has.
        //
        // It also stops shipping the drop table (with weights) to the client, which was free
        // information disclosure. Adding a field the client needs means adding it HERE, and the
        // `.get(key, default)` reader on the far side will silently see nothing if you forget.
        if let Some(zs) = obj.get("zone_spawn").cloned() {
            if let Some(z) = zs.as_object() {
                let keep = |k: &str| z.get(k).cloned().unwrap_or(serde_json::Value::Null);
                obj.insert(
                    "zone_spawn".into(),
                    serde_json::json!({
                        "zone_cleared": keep("zone_cleared"),
                        "kill_counter": keep("kill_counter"),
                        "boss_threshold": keep("boss_threshold"),
                        "boss_spawned": keep("boss_spawned"),
                        "player_skills": keep("player_skills"),
                        // The band's final room: the walk-through renderer stands the party down
                        // and fades there instead of marching + panning onto bare ground.
                        "is_instance_end": keep("is_instance_end"),
                        // WHEN THIS ROOM OPENED, and how long the fight is held for while it does.
                        // The zone ID is not enough to see a room begin — a retry reopens the SAME
                        // zone — so this is what the renderer watches to play an entrance, and the
                        // grace is what it caps that entrance to. Trimming them out is why a retry
                        // showed the party sliding into formation instead of walking in (0.0.343).
                        "opened_at_tick": keep("opened_at_tick"),
                        "deploy_grace_ticks": keep("deploy_grace_ticks"),
                    }),
                );
            }
        }
        // NOTE: long-settled MONSTER CORPSES are trimmed from the client
        // JSON further down — AFTER the per-entity field injections
        // (eff_max_hp / mon_name / sprite …). Trimming here would shorten
        // the entities array before those injections zip it against the
        // full `s.grid.entities`, misaligning every entity past the first
        // corpse (the boss bar then read a mob's max HP). See the retain
        // block after the injection loop.
        // Strip the big item lists (stash + escrow + rare bag) to
        // counts — the inventory / loot / rare panels fetch them on
        // demand (stash_json / escrow_json / rare_bag_json).
        // How far a HELD member (`regent_hand`) may be dropped. A sim
        // constant, sent so the renderer can outline the legal cells
        // without hardcoding a rule the sim owns.
        obj.insert(
            "pickup_range".into(),
            serde_json::json!(sim_core::grid::PICKUP_RANGE),
        );
        // COMMANDER SKILLS. Sent explicitly rather than left to the state dump for two reasons:
        // `commander_charges` is `skip_serializing_if` FULL, so the renderer would see the field
        // vanish exactly when the pool is healthy; and the footprint SIDE is a sim constant the
        // targeting preview must not hardcode — the preview and the hit test disagreeing is the
        // one failure here that no compiler can catch, so the size at least comes from one place.
        obj.insert(
            "commander_charges".into(),
            serde_json::json!(s.commander_charges),
        );
        obj.insert(
            "commander_charge_max".into(),
            serde_json::json!(sim_core::state::COMMANDER_CHARGE_MAX),
        );
        obj.insert(
            "commander_charge_ticks".into(),
            serde_json::json!(sim_core::state::COMMANDER_CHARGE_TICKS),
        );
        // The skill BAR, in bar order — one entry per castable skill. A LIST rather than a key per
        // skill (`commander_smite_cd`, `commander_smite_side`, …) so that shipping the second
        // Commander skill costs a row in `COMMANDER_SKILLS` and nothing here or in GDScript; and
        // so the bar cannot draw a button for a skill `apply_input` would refuse, because both
        // read the same catalog.
        obj.insert(
            "commander_skills".into(),
            serde_json::Value::Array(
                sim_core::events::COMMANDER_SKILLS
                    .iter()
                    // Only skills the player OWNS — the bar must not draw a button `apply_input`
                    // would refuse, and the smite handler gates on this same set. Empty = locked.
                    .filter(|d| s.commander_skills_owned.contains(d.id))
                    .map(|d| {
                        serde_json::json!({
                            "id": d.id,
                            // TICKS left, 0 = ready. Ticks not seconds: the client already
                            // divides by 10 everywhere and the sim owns no clock.
                            "cd": s.commander_cd.get(d.id).copied().unwrap_or(0),
                            "cd_max": d.cooldown,
                            // How to ask for a target. The renderer switches its aim mode on
                            // THIS, never on the id — see `CommanderTarget`.
                            "target": d.target.as_str(),
                            // Footprint SIDE. A sim constant the targeting preview must not
                            // hardcode: preview and hit test disagreeing is the one failure
                            // here no compiler can catch, so the size comes from one place.
                            "side": d.side,
                        })
                    })
                    .collect(),
            ),
        );
        // The RAW owned set (every unlocked id, NOT filtered by the bar catalog above). The client's
        // grant-echo guard reads THIS to decide "already owned": checking the catalog-filtered
        // `commander_skills` would miss a skill whose catalog row this build lacks (version skew),
        // re-appending its GrantCommanderSkill to the save log on every refresh.
        obj.insert(
            "commander_skills_owned".into(),
            serde_json::Value::Array(
                s.commander_skills_owned
                    .iter()
                    .map(|id| serde_json::Value::String(id.clone()))
                    .collect(),
            ),
        );
        // The highest band the picker may select, so the client can lock buttons past the
        // frontier instead of offering every band and letting the sim silently refuse the ones
        // out of reach — which reads as "the picker is broken" rather than "that band is locked".
        //
        // The SAME gate `InputEvent::SelectBand` / `EnterInstance` apply (`band_travel_max`), so the
        // button the client enables and the input the sim accepts cannot drift.
        {
            obj.insert("band_select_max".into(), serde_json::json!(s.band_travel_max()));
        }
        for key in ["inventory", "stash", "escrow", "rare_bag", "card_album"] {
            let count = obj
                .get(key)
                .and_then(|x| x.as_array())
                .map(|a| a.len())
                .unwrap_or(0);
            obj.insert(format!("{key}_count"), serde_json::json!(count));
            obj.insert(key.into(), serde_json::json!([]));
        }
        // Always surface the EFFECTIVE rare-bag cap + the current
        // threshold (both serde-skip at default, so inject them so the
        // UI always has them).
        obj.insert("rare_bag_cap".into(), serde_json::json!(s.rare_bag_cap()));
        // (The card album is unbounded as of 0.0.217 — no cap to surface.)
        // Stash-warehouse cap (serde-skips at default) so the stash UI can
        // show "used / cap"; the full list comes via stash_json().
        obj.insert("stash_cap".into(), serde_json::json!(s.stash_cap()));
        // INVENTORY (main bag) cap — separate from the warehouse cap above.
        // The sidebar + Stash screen show the bag's "used / cap" from this.
        obj.insert("inventory_cap".into(), serde_json::json!(s.inventory_cap()));
        // RED DOTS held by the save (the server-held ones are `GET /badges`): fielded members with
        // unspent core / Apex points, bag fullness, and units a banked Resonance credit can raise.
        let stat_points: Vec<serde_json::Value> = s.grid.entities.iter()
            .filter(|e| matches!(e.team, sim_core::grid::Team::Player) && !e.is_pet)
            .filter(|e| e.stats.core_unspent() > 0 || e.stats.apex_unspent() > 0)
            .map(|e| serde_json::json!({
                "id": e.id, "core": e.stats.core_unspent(), "apex": e.stats.apex_unspent(),
            }))
            .collect();
        let bag_free = s.inventory_cap().saturating_sub(s.inventory.len());
        obj.insert("badges".into(), serde_json::json!({
            "stat_points": stat_points,
            "bag_free": bag_free,
            "bag_full": bag_free == 0,
            "resonance": data::feedable_units(s),
        }));
        obj.insert(
            "rare_threshold".into(),
            serde_json::json!(s.rare_threshold),
        );
        // Crafted GEAR awaiting the server's star roll. Without this the craft screen goes
        // silent for a save cycle — the player spends 30 materials, nothing appears in the
        // bag, and the only honest reading is that the craft failed. Counted here rather
        // than sending the candidate list: the UI needs "how many are being appraised", and
        // `rare_candidates` also carries the DROP lottery, which is deliberately invisible.
        let (pending_drops, pending_crafts) = sim_core::drops::candidate_counts(s);
        obj.insert("pending_crafts".into(), serde_json::json!(pending_crafts));
        // The DROP half is here purely so a refused craft is diagnosable from the client log
        // — the two budgets are separate now, but "the button did nothing" was invisible once
        // and should never be invisible again.
        obj.insert("pending_drops".into(), serde_json::json!(pending_drops));
        // Gear waiting to be VALUED by the server (stage 3). Its own key because it is the
        // only one that represents loot the player has earned but cannot see yet.
        obj.insert(
            "pending_gear".into(),
            serde_json::json!(sim_core::drops::gear_candidate_count(s)),
        );
        // Loot LEVEL-GAP penalty for the CURRENT zone (bp; 10000 = none) —
        // the HUD shows a "Loot −X%" badge when the party over-levels the
        // zone. Computed in sim-core (view-only; per-kill combat uses each
        // dead mob's own level).
        // The unified stash SLOT cap (gear + material + junk) so the
        // inventory grid draws exactly that many slots (empties when
        // not full). Upgrades raise it via the StashCap soul node.
        obj.insert("inventory_cap".into(), serde_json::json!(s.inventory_cap()));
        // Per-tick hit events (display-only, serde-skip) for the
        // renderer's multi-hit popups: Double Attack / dual-wield show
        // 2 numbers, Sonic Blow 8, each with a yellow running total.
        // Ids of the party's own entities: amounts landing on them are NOT display-scaled.
        let party_ids: std::collections::BTreeSet<u32> = s
            .grid
            .entities
            .iter()
            .filter(|e| matches!(e.team, sim_core::grid::Team::Player))
            .map(|e| e.id)
            .collect();
        let hits: Vec<serde_json::Value> = s
            .recent_hits
            .iter()
            .map(|h| {
                serde_json::json!({
                    "target_id": h.target_id,
                    "attacker_id": h.attacker_id,
                    "amount": crate::disp_for(party_ids.contains(&h.target_id), i64::from(h.amount)),
                    "hits": h.hits,
                    "kind": match h.kind {
                        sim_core::state::HitKind::Normal => "normal",
                        sim_core::state::HitKind::Dodge => "dodge",
                        sim_core::state::HitKind::Guard => "guard",
                        sim_core::state::HitKind::Break => "break",
                        sim_core::state::HitKind::Crit => "crit",
                    },
                })
            })
            .collect();
        obj.insert("recent_hits".into(), serde_json::json!(hits));
        // Items banked THIS tick, for the loot banner. Serialised with the same
        // `item_to_view` the bag and stash screens use, so the client names and colours it
        // through its one existing path rather than a second one invented here. Tiny —
        // usually zero or one entry — which is why it can ride the per-tick state at all,
        // unlike the bag itself (pulled on demand because 10x/s of it caused stutter).
        let loot: Vec<serde_json::Value> =
            s.loot_fx.iter().map(crate::item_view::item_to_view).collect();
        obj.insert("loot_fx".into(), serde_json::json!(loot));
        // Heals that LANDED this tick. The client used to infer these from `hp_now - hp_was`,
        // which cannot name the healer, merges two heals into one number, and shows NOTHING when
        // a heal and a hit of similar size land on the same tick because net HP never moves.
        // `amount` is the effective restore, so a heal onto a full bar is not in this list at all.
        let heals: Vec<serde_json::Value> = s
            .heal_fx
            .iter()
            .map(|h| {
                serde_json::json!({
                    "target_id": h.target_id,
                    "healer_id": h.healer_id,
                    "amount": crate::disp_for(party_ids.contains(&h.target_id), i64::from(h.amount)),
                    "kind": match h.kind {
                        sim_core::state::HealKind::Skill => "skill",
                        sim_core::state::HealKind::Regen => "regen",
                        sim_core::state::HealKind::Hot => "hot",
                        sim_core::state::HealKind::Potion => "potion",
                        sim_core::state::HealKind::Lifesteal => "lifesteal",
                    },
                })
            })
            .collect();
        obj.insert("heal_fx".into(), serde_json::json!(heals));
        // Entity ids knocked back this tick → the renderer plays a bounce arc.
        obj.insert("knockbacks".into(), serde_json::json!(s.knockbacks));
        // Per-tick SKILL activations (display-only) so the renderer can
        // play a per-skill effect (Bash impact) instead of the weapon slash.
        let sfx: Vec<serde_json::Value> = s
            .skill_fx
            .iter()
            .map(|f| {
                serde_json::json!({
                    "caster_id": f.caster_id,
                    "target_id": f.target_id,
                    "skill_id": f.skill_id,
                })
            })
            .collect();
        obj.insert("skill_fx".into(), serde_json::json!(sfx));
        // Ultimates that resolved this tick — the renderer's cut-in trigger.
        let ufx: Vec<serde_json::Value> = s
            .ult_fx
            .iter()
            .map(|f| serde_json::json!({ "caster_id": f.caster_id, "skill_id": f.skill_id }))
            .collect();
        obj.insert("ult_fx".into(), serde_json::json!(ufx));
        // Per-tick DAMAGE-field hits (Vermilion/Glacial) for the renderer's
        // per-victim running yellow total (real multi-hit over time).
        let fhits: Vec<serde_json::Value> = s
            .field_hits
            .iter()
            .map(|h| serde_json::json!({"target_id": h.target_id, "amount": crate::disp_for(party_ids.contains(&h.target_id), i64::from(h.amount)), "hits": h.hits, "crit": matches!(h.kind, sim_core::state::HitKind::Crit)}))
            .collect();
        obj.insert("field_hits".into(), serde_json::json!(fhits));
        // Poison Explosions that fired this tick (display-only, serde-skip)
        // so the renderer can spawn a poison-burst FX at each corpse.
        let bursts: Vec<serde_json::Value> = s
            .poison_bursts
            .iter()
            .map(|b| serde_json::json!({"x": b.x, "y": b.y, "radius": b.radius, "element": format!("{:?}", b.element)}))
            .collect();
        obj.insert("poison_bursts".into(), serde_json::json!(bursts));
        // Meteor strikes this tick (display-only, serde-skip): each is a
        // 5×5 impact cell the renderer flashes where a meteor landed.
        let meteors: Vec<serde_json::Value> = s
            .meteor_impacts
            .iter()
            .map(|b| serde_json::json!({"x": b.x, "y": b.y, "radius": b.radius}))
            .collect();
        obj.insert("meteor_impacts".into(), serde_json::json!(meteors));
        // Traps that fired this tick (display-only, serde-skip): the
        // renderer plays a snap/blast anim at each cell, even a trap
        // consumed the same tick it triggered.
        let trap_fires: Vec<serde_json::Value> = s
            .trap_fires
            .iter()
            .map(|t| serde_json::json!({"x": t.x, "y": t.y, "kind": t.kind}))
            .collect();
        obj.insert("trap_fires".into(), serde_json::json!(trap_fires));
        // Projectiles carry a transient (serde-skip) `sky_fall` hint —
        // inject it so the renderer flies a basic magic attack from the
        // staff but rains skill bolts from the sky. Zip by index.
        if let Some(projs) = obj
            .get_mut("projectiles")
            .and_then(|p| p.as_array_mut())
        {
            for (pv, p) in projs.iter_mut().zip(s.projectiles.iter()) {
                if let Some(po) = pv.as_object_mut() {
                    po.insert("sky_fall".into(), serde_json::json!(p.sky_fall));
                    // Same reason as `sky_fall`: serde-skips, so it has to be added by hand or
                    // the renderer cannot tell one element's bolt from another's.
                    po.insert("skill_id".into(), serde_json::json!(p.skill_id));
                    // Bolt element → renderer tint (Holy bolt = gold, etc.).
                    po.insert("element".into(), serde_json::json!(format!("{:?}", p.element)));
                    // true = a basic attack; false = a SKILL cast (renderer sizes a
                    // per-source bolt sprite by this — witch basic 1×1 vs fireball 2×2).
                    po.insert("is_basic".into(), serde_json::json!(p.is_basic));
                }
            }
        }
        // FUSED ground fields (planted AoE marks) carry no record of how
        // long their fuse WAS — only what's left. The renderer needs the
        // total to sweep its countdown fill, so resolve it from the skill
        // that planted the field: the number stays owned by the sim's
        // catalog instead of being duplicated as a magic constant in the
        // client. Absent for fields whose skill is unknown.
        if let Some(fields) = obj.get_mut("ground_fields").and_then(|f| f.as_array_mut())
        {
            for (fv, f) in fields.iter_mut().zip(s.ground_fields.iter()) {
                if let Some(fo) = fv.as_object_mut() {
                    if let Some(def) = sim_core::skills::skill_def(&f.skill_id) {
                        if def.field_fuse {
                            fo.insert(
                                "fuse_total".into(),
                                serde_json::json!(def.field_duration_ticks),
                            );
                        }
                    }
                }
            }
        }
        // Inject the SIM's effective max HP/SP onto every entity so
        // each HP/SP bar (party frames + floating monster bars) reads
        // the sim's value directly — single source of truth, no
        // per-entity FFI call. The serialized array is in the same
        // order as `s.grid.entities`, so zip by index.
        if let Some(ents) = obj
            .get_mut("grid")
            .and_then(|g| g.get_mut("entities"))
            .and_then(|e| e.as_array_mut())
        {
            // Every member's wind-up preview from ONE probe clone, not one per member.
            let pending_skills = sim_core::tick::preview_pending_skills(s);
            for (ent_v, e) in ents.iter_mut().zip(s.grid.entities.iter()) {
                if let Some(eo) = ent_v.as_object_mut() {
                    // An equipped OVERLAY carries only id/level/stars in state — its name and
                    // its numbers are recomputed from the def. The crown widgets were reading
                    // `name`, `effect`, `per_level` and `star_ranks` off it, none of which are
                    // serialized, so the Regalia hover card showed a fallback title and no stats
                    // at all. Resolve here, AT the worn level and stars, and hand over finished
                    // strings — same contract the item tooltip already uses.
                    let ov = |mods: Vec<sim_core::items::StatMod>| -> Vec<String> {
                        sim_core::items::StatMod::merged(&mods)
                            .iter()
                            .map(crate::item_view::stat_mod_text)
                            .collect()
                    };
                    if let (Some(x), Some(o2)) =
                        (e.exemplar.as_ref(), eo.get_mut("exemplar").and_then(|v| v.as_object_mut()))
                    {
                        if let Some(def) = sim_core::items::exemplar_def_by_id(&x.id) {
                            o2.insert("name".into(), serde_json::json!(def.name));
                        }
                        o2.insert("lines".into(), serde_json::json!(ov(x.mods())));
                    }
                    if let (Some(r), Some(o2)) =
                        (e.regalia.as_ref(), eo.get_mut("regalia").and_then(|v| v.as_object_mut()))
                    {
                        if let Some(def) = sim_core::items::regalia_def_by_id(&r.id) {
                            o2.insert("name".into(), serde_json::json!(def.name));
                        }
                        o2.insert("lines".into(), serde_json::json!(ov(r.mods())));
                    }
                    // HP AMOUNTS on the display scale (`crate::disp`): `hp`, `max_hp` and the
                    // injected `eff_max_hp` move together, so every bar ratio and every printed
                    // number a client builds from them is the same one the sim has, ÷10.
                    // A PARTY member's HP stays on the sim scale (`disp_for`); a monster's is ÷10.
                    let mine = matches!(e.team, sim_core::grid::Team::Player);
                    eo.insert("hp".into(), serde_json::json!(crate::disp_for(mine, i64::from(e.hp))));
                    eo.insert("max_hp".into(), serde_json::json!(crate::disp_for(mine, i64::from(e.max_hp))));
                    let shield_hp: i64 = e.shields.iter().map(|s| i64::from(s.pool.max(0))).sum();
                    eo.insert("shield_hp".into(), serde_json::json!(crate::disp_for(mine, shield_hp)));
                    eo.insert(
                        "eff_max_hp".into(),
                        serde_json::json!(crate::disp_for(mine, i64::from(sim_core::combat::effective_max_hp(e)))),
                    );
                    // The gauge under HP is the Limit Break gauge now (SP is
                    // gone, 0.0.315). 0 = this entity owns no Ultimate → no bar.
                    eo.insert(
                        "eff_lb_max".into(),
                        serde_json::json!(sim_core::skills::ultimate_of(&e.skills)
                            .map_or(0, |d| d.lb_max)),
                    );
                    // EFFECTIVE attack range (honours a basic-attack override
                    // like Sagitta's range 3), overriding the raw melee `atk_range`
                    // field. The renderer reads this to skip the melee lunge on a
                    // ranged attacker (atk_range > 1).
                    eo.insert(
                        "atk_range".into(),
                        serde_json::json!(sim_core::combat::effective_atk_range(e)),
                    );
                    // Effective attack-speed accumulator gain per tick
                    // (players derive this from job + AGI/DEX, so the raw
                    // `atk_speed_milli` field is 0). The renderer uses it
                    // to pace multi-hit popups to the real swing rhythm.
                    eo.insert(
                        "eff_atk_speed_milli".into(),
                        serde_json::json!(sim_core::combat::effective_atk_speed_milli(e)),
                    );
                    // WEAPON FAMILY this character may wield (0.0.368) — the types, resolved
                    // through the worn Exemplar exactly as `equip_item` resolves them. The
                    // client tests `item.weapon_type` against this ONE list instead of
                    // re-deriving the exemplar-beats-unit precedence in two languages, which
                    // is how a greyed-out slot and the sim's answer start disagreeing.
                    // Empty = unrestricted.
                    eo.insert(
                        "weapon_family".into(),
                        serde_json::json!(sim_core::items::weapon_types_for_character(
                            e.exemplar.as_ref().map(|x| x.id.as_str()),
                            &e.unit_def,
                        )),
                    );
                    // OFF-HAND kinds this character may equip (0.0.446) — the same precedence,
                    // one kind per family; the client greys a Tome on Seraphine the way it greys
                    // a staff.
                    eo.insert(
                        "offhand_family".into(),
                        serde_json::json!(sim_core::items::offhand_types_for_character(
                            e.exemplar.as_ref().map(|x| x.id.as_str()),
                            &e.unit_def,
                        )),
                    );
                    // Main-hand weapon type ("Dagger"/"Sword1H"/… /"None")
                    // so the renderer keys the basic-attack slash VFX by
                    // WEAPON, not class (future: per-weapon effects).
                    eo.insert(
                        "weapon_type".into(),
                        serde_json::json!(sim_core::combat::main_hand_weapon_type(e)),
                    );
                    // TELEGRAPH for a ground-field cast in progress (the
                    // boss AoE the `regent_hand` pick-up answers): the
                    // FOOTPRINT the field will cover, resolved from the
                    // sim's skill def so the renderer only draws it —
                    // GDScript never derives a radius. Absent unless this
                    // entity is mid-cast of a field skill.
                    if let Some(c) = e.casting.as_ref() {
                        if let Some(def) = e
                            .skills
                            .get(c.slot.get())
                            .and_then(|sk| sim_core::skills::skill_def(&sk.id))
                        {
                            if def.ground_field_radius > 0 {
                                eo.insert(
                                    "cast_field_radius".into(),
                                    serde_json::json!(def.ground_field_radius),
                                );
                                // The LOCKED aim cell — where the field
                                // will actually land. Drawing the warning
                                // on the target entity instead would let
                                // the marker chase whoever it's aimed at.
                                if let Some(cell) = c.ground_cell {
                                    eo.insert(
                                        "cast_cell_x".into(),
                                        serde_json::json!(cell.x),
                                    );
                                    eo.insert(
                                        "cast_cell_y".into(),
                                        serde_json::json!(cell.y),
                                    );
                                }
                            }
                        }
                    }
                    // Skill the NEXT swing would cast (preview) so the
                    // renderer can play the right attack animation during
                    // the wind-up — skill_fx only arrives at resolve.
                    if let Some(pending) = pending_skills.get(&e.id) {
                        eo.insert("pending_skill".into(), serde_json::json!(pending));
                    }
                    // Ticks left in a skill's MOTION LOCK (0.0.481): the swing accumulator is
                    // frozen through it, so the renderer holds the skill pose instead of starting
                    // a basic wind-up. Absent when not locked.
                    let lock = e.act_locked_until.saturating_sub(s.tick);
                    if e.alive && lock > 0 {
                        eo.insert("motion_lock".into(), serde_json::json!(lock));
                    }
                    // VISUAL sprite key from the monster DB (decoupled from
                    // element) so the renderer picks the look per-monster,
                    // not from the combat element. Empty for players / unset.
                    if let Some(md) =
                        data.as_ref().and_then(|d| d.monster(&e.def_id))
                    {
                        if !md.sprite.is_empty() {
                            eo.insert("sprite".into(), serde_json::json!(md.sprite));
                        }
                        // Display name (hover tooltip). Fall back to the
                        // def_id if the DB name is empty.
                        let nm = if md.name.is_empty() {
                            md.id.clone()
                        } else {
                            md.name.clone()
                        };
                        eo.insert("mon_name".into(), serde_json::json!(nm));
                        // BOSS NAME (2026-09-29): from stage 49 the boss is a random pool mob
                        // wearing boss stats (`spawn::pick_pool_boss_template`), so its title
                        // follows the mob that was picked. A loc KEY, never text; the client
                        // falls back to `BOSS_NAME_DEFAULT` ("Alpha {0}") for a mob without a row.
                        if e.is_boss && s.zone_spawn.as_ref().is_some_and(|z| z.boss_from_pool) {
                            eo.insert("boss_name".into(), serde_json::json!(format!("BOSS_NAME_{}", md.id)));
                        }
                    }
                    // Re-enrich each equipped item through the SAME
                    // `item_to_view` the stash uses (refine bonuses,
                    // slot, refine_ore, socket_count, wieldable) so equip
                    // tooltips match the inventory — single source, no
                    // duplicated injection.
                    if let Some(eq) = eo
                        .get_mut("equipped")
                        .and_then(|x| x.as_array_mut())
                    {
                        let ex = e.exemplar.as_ref().map(|x| x.id.as_str());
                        for (item_v, it) in eq.iter_mut().zip(e.equipped.iter()) {
                            *item_v = item_to_view(it);
                            // OFF-FAMILY (2026-09-29): worn outside this character's weapon
                            // family, so it gives `OFF_FAMILY_GEAR_BP` of its own ATK/DEF.
                            if let Some(o) = item_v.as_object_mut() {
                                o.insert("off_family".into(), serde_json::json!(!it.is_preferred_by(ex, &e.unit_def)));
                            }
                        }
                    }
                    // The weapon types this character prefers and the off-hand kind they carry —
                    // what an off-family tooltip names.
                    {
                        let fam = sim_core::items::weapon_types_for_character(e.exemplar.as_ref().map(|x| x.id.as_str()), &e.unit_def);
                        eo.insert("preferred_weapons".into(), serde_json::json!(fam));
                        let offhands: Vec<&str> = fam.iter()
                            .map(|w| sim_core::items::offhand_for_weapon(*w))
                            .filter(|o| !o.is_none())
                            .map(|o| o.key())
                            .collect();
                        eo.insert("preferred_offhands".into(), serde_json::json!(offhands));
                    }
                    // Enrich the CARD PANEL the same way (name/effect/star/foil/
                    // card_slot for the card-panel UI). Skipped when empty.
                    if let Some(cp) = eo
                        .get_mut("card_panel")
                        .and_then(|x| x.as_array_mut())
                    {
                        for (item_v, it) in cp.iter_mut().zip(e.card_panel.iter()) {
                            *item_v = item_to_view(it);
                        }
                    }
                    // Per-class ACTIVATION caps = summed sockets of the equipped
                    // gear feeding each card class, so the UI draws active vs
                    // locked slots without recomputing. Always present.
                    let mut socks = serde_json::Map::new();
                    for class in sim_core::items::CARD_CLASSES {
                        let n: u8 = e
                            .equipped
                            .iter()
                            .filter(|it| {
                                sim_core::items::card_class_of_slot(it.equip_slot)
                                    == Some(class)
                            })
                            .map(|it| it.socket_count())
                            .sum();
                        socks.insert(format!("{class:?}"), serde_json::json!(n));
                    }
                    eo.insert(
                        "card_sockets".into(),
                        serde_json::Value::Object(socks),
                    );
                }
            }
        }
        // Drop long-settled MONSTER CORPSES from the client JSON: dead
        // bodies linger in GameState (O(n)-cheap + the count cap) but the
        // renderer only needs them during the death animation. Past
        // CORPSE_RENDER_LINGER_TICKS they're invisible dead weight that
        // bloats this per-tick parse and keeps render nodes alive — trim
        // them here. This runs AFTER the per-entity field injections above
        // so those zips stay aligned with the full `s.grid.entities`;
        // shortening the array earlier misaligned them (the boss bar then
        // read a mob's max HP). `down_ticks` is serde-skip, so read it from
        // the live state in the SAME order serde emitted.
        if let Some(ents) = obj
            .get_mut("grid")
            .and_then(|g| g.as_object_mut())
            .and_then(|g| g.get_mut("entities"))
            .and_then(|e| e.as_array_mut())
        {
            let live = &s.grid.entities;
            let mut idx = 0usize;
            ents.retain(|_| {
                let keep = live.get(idx).map_or(true, |e| {
                    !(!e.alive
                        && matches!(e.team, sim_core::grid::Team::Monster)
                        && e.down_ticks
                            > sim_core::tick::CORPSE_RENDER_LINGER_TICKS)
                });
                idx += 1;
                keep
            });
        }
    }
    Some(v)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every key this payload always carries at the TOP level. Pinned as a REQUIRED SUBSET:
    /// adding a key is free, losing one fails. Most are serde-derived from `GameState` (renaming
    /// those is at least a visible Rust change), but the `*_cap` / `*_count` entries are
    /// hand-written strings this module substitutes for trimmed lists — delete the line that
    /// writes one and NOTHING in Rust notices.
    const TOP_KEYS: &[&str] = &[
        "badges", "card_album", "card_album_count", "current_unit_def",
        "current_zone_id", "escrow", "escrow_count", "field_hits", "gil", "grid",
        "inventory", "inventory_cap", "inventory_count", "knockbacks", "loot_fx",
        "meteor_impacts", "next_monster_id", "pending_crafts", "pending_drops", "pending_gear",
        "pending_zone_change", "pickup_range",
        "poison_bursts", "projectiles", "rare_bag", "rare_bag_cap", "rare_bag_count",
        "rare_threshold", "recent_hits", "rng", "skill_fx", "stash", "stash_cap",
        "stash_count", "tick", "trap_fires", "zone_spawn",
    ];

    /// Derived per-entity values the sim does NOT store — pure injections, so pure string risk.
    const ENTITY_INJECTED: &[&str] =
        &["eff_max_hp", "shield_hp", "eff_lb_max", "eff_atk_speed_milli", "element", "weapon_family"];

    /// Per-PARTY-MEMBER injections (the off-family tooltip), and the `badges` block's keys.
    const PLAYER_INJECTED: &[&str] = &["preferred_weapons", "preferred_offhands"];
    const BADGE_KEYS: &[&str] = &["stat_points", "bag_free", "bag_full", "resonance"];

    /// `zone_spawn` is TRIMMED to exactly these — a hand-written list replacing a 45 KB struct,
    /// so dropping one is invisible in Rust and shows up as a dead HUD element months later.
    const ZONE_SPAWN_KEPT: &[&str] =
        &["zone_cleared", "kill_counter", "boss_threshold", "boss_spawned", "player_skills",
            "is_instance_end"];

    /// The save-held red dots: unspent points per fielded member, bag room, feedable units.
    #[test]
    fn badges_report_points_bag_room_and_resonance() {
        let (mut st, gd) = realistic();
        let read = |st: &GameState| build_view_state(st, Some(&gd)).unwrap()["badges"].clone();
        let b = read(&st);
        assert_eq!(b["bag_free"].as_u64(), Some((st.inventory_cap() - st.inventory.len()) as u64));
        assert_eq!(b["bag_full"], false);
        assert_eq!(b["resonance"].as_array().map(Vec::len), Some(0));

        let idx = st.grid.entities.iter().position(|e| e.team == sim_core::grid::Team::Player).unwrap();
        st.grid.entities[idx].stats.level_points = 3;
        let id = st.grid.entities[idx].id;
        let pts = read(&st)["stat_points"].clone();
        assert!(pts.as_array().is_some_and(|a| a.iter().any(|p| p["id"] == id && p["core"].as_u64() > Some(0))), "{pts}");

        // A second copy of an owned unit banks a credit; that unit is then feedable.
        let lumi = data::grant_exemplar_unit(&mut st, &gd, "lumi").expect("lumi joins");
        assert_eq!(read(&st)["resonance"], serde_json::json!([]));
        let _ = data::grant_exemplar_unit(&mut st, &gd, "lumi");
        assert_eq!(read(&st)["resonance"], serde_json::json!([lumi]));
    }

    /// A REAL state from the real catalog. An empty `GameState::new` has no monsters, so the
    /// per-entity injections would never run and the test would pass while covering nothing.
    fn realistic() -> (GameState, data::GameData) {
        let json = std::fs::read_to_string(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../data/game_data.json"
        ))
        .expect("data/game_data.json — resolved from CARGO_MANIFEST_DIR, not the CWD");
        let gd = data::GameData::from_json_str(&json).expect("catalog parses");
        let mut st = data::new_game_in_zone(7, &gd, "zone_b1_1").expect("zone_b1_1 exists");
        for _ in 0..400 {
            if st.grid.entities.len() > 1 {
                break;
            }
            sim_core::tick::tick(&mut st, 1);
        }
        (st, gd)
    }

    /// THE FFI CONTRACT TEST — the only thing in the workspace that covers it.
    ///
    /// GDScript reads this payload with `.get(key, default)` in ~1,400 places, so a renamed or
    /// deleted key raises NOTHING: the panel silently renders empty (CLAUDE.md coverage gap #1).
    ///
    /// Guards LOSS only. Adding a key needs no change here. Removing or renaming one is
    /// deliberate — update the GDScript reader and re-pin the list above in the SAME commit.
    #[test]
    fn contract_keys_are_stable() {
        let (st, gd) = realistic();
        let v = build_view_state(&st, Some(&gd)).expect("state serializes");
        let obj = v.as_object().expect("top level is an object");

        let missing: Vec<_> = TOP_KEYS.iter().filter(|k| !obj.contains_key(**k)).collect();
        assert!(missing.is_empty(), "top-level keys vanished from view_state: {missing:?}");

        let zs = obj["zone_spawn"].as_object().expect("zone_spawn is an object");
        let missing: Vec<_> = ZONE_SPAWN_KEPT.iter().filter(|k| !zs.contains_key(**k)).collect();
        assert!(missing.is_empty(), "zone_spawn lost keys the client reads: {missing:?}");
        assert!(
            !zs.contains_key("pool"),
            "the spawn pool is back in the per-tick payload — that is 45 KB, 10x a second"
        );

        let ents = obj["grid"]["entities"].as_array().expect("grid.entities is an array");
        assert!(ents.len() > 1, "need a spawned monster to reach the per-entity injections");

        for (i, e) in ents.iter().enumerate() {
            let eo = e.as_object().unwrap_or_else(|| panic!("entity {i} is not an object"));
            let missing: Vec<_> = ENTITY_INJECTED.iter().filter(|k| !eo.contains_key(**k)).collect();
            assert!(missing.is_empty(), "entity {i}: injected keys vanished: {missing:?}");
        }

        // Party members carry the off-family tooltip's keys (audit 2026-10-02: unpinned until now).
        let players: Vec<_> = ents.iter().filter(|e| e.get("team").and_then(|t| t.as_str()) == Some("Player")).collect();
        assert!(!players.is_empty(), "no party member in the view — the check below covers nothing");
        for e in players {
            for k in PLAYER_INJECTED {
                assert!(e.get(*k).is_some(), "a party member lost '{k}'");
            }
        }
        let badges = obj["badges"].as_object().expect("badges is an object");
        let missing: Vec<_> = BADGE_KEYS.iter().filter(|k| !badges.contains_key(**k)).collect();
        assert!(missing.is_empty(), "badges lost keys the client reads: {missing:?}");

        // Monster-only, and the ONE place the catalog is threaded through — so this doubles as the
        // guard against a future caller passing `None` for `data` and quietly losing every name.
        let named = ents.iter().filter(|e| e.get("mon_name").is_some()).count();
        assert!(named > 0, "no entity carried 'mon_name' — the catalog lookup silently no-opped");
        let sprited = ents.iter().filter(|e| e.get("sprite").is_some()).count();
        assert!(sprited > 0, "no entity carried 'sprite'");
    }

    /// Without a catalog the payload must still be well-formed. The client renders a live game
    /// off this: a missing catalog may cost names, never the whole screen.
    #[test]
    fn a_missing_catalog_still_produces_a_usable_payload() {
        let (st, _) = realistic();
        let v = build_view_state(&st, None).expect("serializes without a catalog");
        let obj = v.as_object().expect("top level is an object");
        let missing: Vec<_> = TOP_KEYS.iter().filter(|k| !obj.contains_key(**k)).collect();
        assert!(missing.is_empty(), "catalog-less payload lost keys: {missing:?}");
        assert!(obj["grid"]["entities"].is_array());
    }

    #[test]
    fn shield_hp_tracks_remaining_pools_on_the_hp_display_scale() {
        use sim_core::grid::{AbsorbShield, Team};
        let (mut st, gd) = realistic();
        let idx = st.grid.entities.iter().position(|e| e.team == Team::Player).unwrap();
        let read = |st: &GameState| {
            build_view_state(st, Some(&gd)).unwrap()["grid"]["entities"][idx]["shield_hp"]
                .as_i64().expect("shield_hp is always emitted")
        };
        assert_eq!(read(&st), 0);
        st.grid.entities[idx].shields = vec![
            AbsorbShield { pool: 1200, ..Default::default() },
            AbsorbShield { pool: 350, ..Default::default() },
        ];
        assert_eq!(read(&st), 1550);
        st.grid.entities[idx].shields[0].pool = 200;
        assert_eq!(read(&st), 550);
        st.grid.entities[idx].shields[0].pool = -10;
        assert_eq!(read(&st), 350);
        st.grid.entities[idx].team = Team::Monster;
        assert_eq!(read(&st), crate::disp(350));
        st.grid.entities[idx].shields.clear();
        assert_eq!(read(&st), 0);
    }
}

