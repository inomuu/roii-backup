//! build::zone — zone population, player spawn, new-game/zone-change, wipe.

use super::*;

/// A monster's combat stats: the LEVEL's base, shaped by that monster's own multipliers.
///
/// `stat = level_base[the encounter's level] x monster.<stat>_mult_bp`
///
/// Replaces `authored x mult x the ZONE's scale`. Under that arrangement a creature's strength
/// depended on where it happened to stand, and the difficulty curve was spread over the monster's
/// authored values, its role multipliers and the zone's scale with nothing showing the total —
/// which is how two bands ended up getting EASIER as you advanced without anyone noticing.
///
/// `def` IS here as of 0.0.277 — it is the level, with `def_mult_bp` as the identity knob.
/// (MDEF was a quarter of it until 0.0.333 retired the second armour number.) The note that used to sit here said armour has no level
/// formula in RO (correlation 0.51 renewal, 0-127 at every level) and must therefore stay monster
/// identity; the spread was real but the conclusion was not, since our own authored column tracked
/// level at 0.92 already, just far too shallowly to matter. `hit` / `flee` / `gil` are still the
/// monster's own. (`scale_from_base` mobs used to inherit theirs from a placeholder; those values are
/// baked into the monsters themselves now, hit/flee from RO's `175 + Lv + DEX` / `100 + Lv + AGI`.)
///
/// Returns (hp, atk, def, atk_min, atk_max, xp, gil, stage).
fn base_scaled_stats(
    m: &MonsterDef,
    zone: &ZoneDef,
    data: &GameData,
) -> Result<(i32, i32, i32, i32, i32, u32, u32, u16), String> {
    // NO TABLE means "this GameData was hand-built" — a unit fixture, not the shipped catalog. Fall
    // back to the monster's OWN authored stats, which is what such fixtures mean by their numbers.
    // Returning the table's level-1 row instead turned a fixture's 30 HP monster into a 1 HP one
    // without a word, which is exactly the sort of silence this refactor exists to remove.
    // THE LEVEL OF THIS ENCOUNTER, resolved once, and the only level anything below may read.
    //
    // The zone names it. That is not the old zone-scale coupling coming back: a zone does not
    // multiply anything and does not know what a monster is worth. It says "the creatures here are
    // level 8", and the creature turns that into stats through its OWN multipliers. Identity and
    // level are separate axes, which is the whole point — the same baby_pinkling is a level-1 one
    // in the first band and a level-5 one three bands later, and there is exactly one baby_pinkling.
    //
    // A level ON THE MONSTER DEF cannot express that: it pins a creature to one power forever, so
    // three bands sharing a pool collapse into three copies of the same fight. That was tried for
    // one version (0.0.279) and it did exactly that — bands 1-3 became identical, separated only by
    // their boss thresholds. `MonsterDef.level` is now the fallback for a monster nobody placed,
    // and story mode will pass a level explicitly.
    // THE STAGE decides everything (0.0.398-399): hp / atk / def / toughness / xp. A ladder zone
    // names its stage; a dungeon or story zone authors the stage it fights like; a monster nobody
    // placed falls back to its own `stage`. There is no monster level.
    let stage: u16 = if zone.stage > 0 { zone.stage } else { m.stage };
    let base = if data.stage_base.is_empty() {
        crate::data::defs::StageBase {
            stage,
            hp: m.hp,
            atk: m.atk,
            xp: m.xp_reward,
            gil: 0,
            def: m.def,
            toughness: 0,
        }
    } else {
        data.stage_stats(stage)
    };
    // 0 bp -> x1. A monster that names no multiplier for a stat sits exactly on the level's base.
    let mul = |v: i32, bp: u32| -> i32 {
        if bp == 0 { v } else { ((v as i64) * bp as i64 / 10_000) as i32 }
    };
    let mul_u = |v: u32, bp: u32| -> u32 {
        if bp == 0 { v } else { ((v as u64) * bp as u64 / 10_000) as u32 }
    };
    // atk_min/atk_max keep their SHAPE around the scaled atk: a monster that authored a spread
    // gets the same spread relative to its new attack, not its old absolute numbers.
    let atk = mul(base.atk, m.atk_mult_bp);
    let spread = |v: i32| -> i32 {
        if m.atk <= 0 || v <= 0 { atk } else { ((v as i64) * atk as i64 / m.atk as i64) as i32 }
    };
    // ARMOUR IS THE ZONE'S LEVEL (0.0.277), the same input that drives HP and ATK above.
    //
    // It used to be whatever was typed on the monster, and the typing had drifted into a curve so
    // shallow it did nothing: DEF 5 at level 1 rising to 33 at level 96, i.e. 1.1% to 6.9% off a
    // physical hit, against RO renewal's own mobs at 14.7% for the same band. `def = level` puts
    // us on RO's number (13.4% at the launch cap of 70 vs their measured 14.7%) and costs one base
    // level over a 7-day sim-cli run — the response curve was mapped at x1/x2/x3/x5 to confirm
    // that is a real small effect and not an insensitive harness.
    //
    // `def_mult_bp` is the identity knob, defaulting to x1 so every ordinary monster IS its level.
    // The two hand-tuned Duskmourn bosses carry a real multiplier because their armour was
    // authored at RO-MVP scale and is the point of them; without it dullahan would fall from 31.9%
    // to 10.2% as a side effect of a change about ordinary mobs.
    //
    // MDEF IS A QUARTER OF THE LEVEL — of the LEVEL, not of this monster's DEF (0.0.278).
    //
    // A quarter is the ratio at which MDEF's steeper curve (K=1000) and DEF's (K=4000) cancel
    // exactly, so an ordinary monster — one that names no `def_mult_bp` and therefore wears its
    // level as armour — still mitigates magic and steel identically. That property is why the
    // quarter is here at all (0.0.275) and it is untouched for all but two monsters in the catalog.
    //
    // What changes is what an ARMOURED monster means. Riding DEF made plate stop spells: dullahan's
    // 5.5x armour bought it 31.9% against a nuke as well as against a sword, and nothing in the
    // game could answer it. Riding the level instead leaves it at 8.2% magic against 31.9%
    // physical — the armoured boss now has a shape a party can be built against, which is the
    // point of building a party. RO's own renewal DB does correlate MDEF with DEF rather than
    // level inside the levels we ship (r 0.50 vs 0.04, n=1334), so this is a deliberate departure
    // for a design reason, not a correction.
    // A monster's DEF is its LEVEL and its MDEF a quarter of it — both are level
    // numbers, not stat numbers, so they take the x10 stat scale here rather than
    // in data (0.0.84). Without it the monsters stay on the old scale while the
    // party moves to the new one, and the party simply wins.
    let def_ = mul(base.def, m.def_mult_bp);
    Ok((
        mul(base.hp, m.hp_mult_bp),
        atk,
        def_,
        spread(m.atk_min),
        spread(m.atk_max),
        mul_u(base.xp, m.xp_mult_bp),
        mul_u(base.gil, m.xp_mult_bp).saturating_add(m.gil_reward),
        // The SAME stage everything above was built from. It reaches the entity as `mob_stage`,
        // where it drives soft defence and the sigil tier — one number for the whole creature.
        stage,
    ))
}

/// Initialize the zone's spawn state on `GameState` from the zone's
/// `spawn_pool`. After this call, `spawn::try_continuous_spawn(state)`
/// is what creates monsters at runtime.
///
/// Idempotency note: this REPLACES any existing `zone_spawn` so calling
/// it twice on the same state hot-swaps the active zone. Existing
/// entities (alive monsters, ground drops) are NOT cleared — that's
/// the caller's responsibility before a true zone change.
/// A monster's own gauge PERCENT resolved against its level's standard bar. 0 stays
/// 0: no gauge at all is a property of the creature, not something a level can grant.
/// Element names from the catalog to `Element`. An unknown name is DROPPED rather
/// than defaulted: a typo that silently became Neutral would make a monster weak to
/// plain weapons, which is the one thing every party carries.
fn parse_elements(names: &[String]) -> Vec<crate::grid::Element> {
    names.iter().filter_map(|n| crate::data::parse_element_opt(n)).collect()
}

fn scaled_toughness(pct: u16, stage: u16, data: &GameData) -> u16 {
    if pct == 0 {
        return 0;
    }
    let standard = if data.stage_base.is_empty() { 20 } else { data.stage_stats(stage).toughness.max(1) };
    ((u32::from(standard) * u32::from(pct) / 100).max(1)).min(u16::MAX as u32) as u16
}

/// A zone pool's entries resolved to spawn templates, with the zone's stage scaling folded in.
pub(crate) fn pool_entries(
    entries: &[crate::data::defs::SpawnPoolEntry],
    zone: &ZoneDef,
    data: &GameData,
) -> Result<Vec<MonsterSpawnEntry>, String> {
    let mut pool: Vec<MonsterSpawnEntry> = Vec::with_capacity(entries.len());
    // SUB-BAND scaling (0.0.226): fold the zone's hp/atk scale into the template
    // ONCE here — spawns, bestiary, cards and drops all read the same template,
    // and a 10000-bp zone is byte-identical to the pre-split world.
    for entry in entries {
        let m = data
            .monster(&entry.monster_id)
            .ok_or_else(|| format!("unknown monster: {}", entry.monster_id))?;
        let drops = build_drop_table(m, data, zone.stage);
        let (hp, atk, def_, amin, amax, xp, gil, lvl) =
            base_scaled_stats(m, zone, data)?;
        pool.push(MonsterSpawnEntry {
            template: MonsterTemplate {
                def_id: m.id.clone(),
                bestiary_group: m.bestiary_group.clone(),
                hp,
                atk,
                def: def_,
                atk_min: amin,
                atk_max: amax,
                skills: m.skills.clone(),
                element: crate::grid::Element::Physical, // monsters carry weaknesses, not an element (0.0.444)
                weaknesses: parse_elements(&m.weaknesses),
                resists: parse_elements(&m.resists),
                // PERCENT of the level's standard gauge (0 = no gauge at all).
                toughness_max: scaled_toughness(m.toughness_max, lvl, data),
                speed_milli: m.speed_milli,
                atk_speed_milli: m.atk_speed_milli,
                atk_range: m.atk_range,
                drops,
                gear_grade_odds: crate::data::build::items::grade_odds_for(data, lvl),
                proj_travel_ticks: m.proj_travel_ticks,
                xp_reward: xp,
                gil_reward: gil,
                stage: lvl,
                boss_minions: m.boss_minions,
                race: parse_race(&m.race),
                size: parse_size(&m.size),
            },
            weight: entry.weight,
        });
    }
    Ok(pool)
}

pub fn populate_zone(state: &mut GameState, data: &GameData, zone_id: &str) -> Result<(), String> {
    let zone = data
        .zone(zone_id)
        .ok_or_else(|| format!("unknown zone: {zone_id}"))?;
    // Authoritative current-zone pointer. Every zone load (new-game + change_zone)
    // funnels through here, so this is the one place it needs to be stamped.
    state.current_zone_id = Some(zone_id.to_string());
    // Standing in a band IS reaching it. Raised here because every transition funnels through
    // `change_zone` — the natural `next_zone_id` advance, a manual `SelectBand`, a retreat and a
    // farm-loop all land here, so there is no second place that has to remember to do this.
    // Monotonic: a retreat to band 1 does not give back what was earned.
    if let Some(b) = crate::state::band_of_zone(zone_id) {
        state.band_reached = state.band_reached.max(b as u32);
    }
    // Resolve pool entries (data lookups happen here, never again).
    let pool = pool_entries(&zone.spawn_pool, zone, data)?;
    // Boss template (None if zone has no boss).
    let boss_template: Option<MonsterTemplate> = if zone.boss_monster_id.is_empty() {
        None
    } else {
        let b = data
            .monster(&zone.boss_monster_id)
            .ok_or_else(|| format!("unknown boss monster: {}", zone.boss_monster_id))?;
        let drops = build_drop_table(b, data, zone.stage);
        let (hp, atk, def_, amin, amax, xp, gil, lvl) =
            base_scaled_stats(b, zone, data)?;
        Some(MonsterTemplate {
            def_id: b.id.clone(),
            bestiary_group: b.bestiary_group.clone(),
            hp,
            atk,
            def: def_,
            atk_min: amin,
            atk_max: amax,
            skills: b.skills.clone(),
            element: crate::grid::Element::Physical,
            weaknesses: parse_elements(&b.weaknesses),
            resists: parse_elements(&b.resists),
            // The BOSS gauge, not the creature's own: the same monster is a 60-point
            // bar in the field and a long bar as the zone's boss.
            toughness_max: if b.toughness_max == 0 {
                0 // no gauge at all is a property of the creature, boss or not
            } else if b.boss_toughness_max > 0 {
                scaled_toughness(b.boss_toughness_max, lvl, data)
            } else {
                scaled_toughness(crate::data::defs::BOSS_TOUGHNESS_PCT, lvl, data)
            },
            speed_milli: b.speed_milli,
            atk_speed_milli: b.atk_speed_milli,
            atk_range: b.atk_range,
            drops,
            gear_grade_odds: crate::data::build::items::grade_odds_for(data, lvl),
            proj_travel_ticks: b.proj_travel_ticks,
            xp_reward: xp,
            gil_reward: gil,
            stage: lvl,
            boss_minions: b.boss_minions,
            race: parse_race(&b.race),
            size: parse_size(&b.size),
        })
    };
    let next_zone_id = if zone.next_zone_id.is_empty() {
        None
    } else {
        Some(zone.next_zone_id.clone())
    };

    state.zone_spawn = Some(ZoneSpawnState {
        pool,
        target_population: zone.target_population,
        spawn_interval_ticks: zone.spawn_interval_ticks,
        opened_at_tick: state.tick,
        deploy_grace_ticks: crate::tick::DEPLOY_GRACE_TICKS_OPEN,
        next_spawn_at_tick: 0,
        kill_counter: 0,
        boss_threshold: zone.boss_threshold,
        boss_spawned: false,
        boss_template,
        boss_from_pool: zone.boss_from_pool,
        max_spawns: zone.max_spawns,
        zone_cleared: false,
        cleared_at_tick: None,
        band_clear_counted: false,
        next_zone_id,
        is_instance_end: zone.is_instance_end,
        player_skills: zone.player_skills.clone(),
        instance_start_zone_id: zone.instance_start_zone_id.clone(),
    });
    // Fill the initial population. spawn::spawn_initial_population uses
    // the same PRNG path as continuous spawn, so replays match.
    spawn::spawn_initial_population(state);
    // First continuous-spawn attempt won't fire again until the
    // interval passes; set the timer accordingly so the population is
    // "stable" right after init.
    if let Some(zs) = state.zone_spawn.as_mut() {
        zs.next_spawn_at_tick = state.tick + zs.spawn_interval_ticks as u64;
    }
    Ok(())
}

/// Spawn the zone's declared unit at zone.player_spawn with
/// id `FIRST_PLAYER_ID`. Idempotent guard: panics if a player with id
/// `FIRST_PLAYER_ID` already exists, because that means the caller set up
/// the grid wrong — better to fail at the seam than ship a confused
/// save.
pub fn spawn_player_from_zone(state: &mut GameState, data: &GameData, zone_id: &str) -> Result<(), String> {
    let zone = data.zone(zone_id).ok_or_else(|| format!("unknown zone: {zone_id}"))?;
    if state.grid.idx_of_id(FIRST_PLAYER_ID).is_some() {
        return Err("player already spawned".to_string());
    }
    // Party roster: zone.party if set, else the solo unit.
    let roster: Vec<&str> = if zone.party.is_empty() {
        vec![zone.unit.as_str()]
    } else {
        zone.party.iter().map(|s| s.as_str()).collect()
    };
    for (i, unit_def) in roster.iter().enumerate() {
        let class = data
            .unit(unit_def)
            .ok_or_else(|| format!("unknown unit: {unit_def}"))?;
        let id = FIRST_PLAYER_ID + i as u32;
        let pos = party_cell(zone.player_spawn, i, zone.width, zone.height);
        state.grid.spawn(build_player_entity(data, class, id, pos));
    }
    Ok(())
}


/// Convenience: create a fresh `GameState` sized for `zone_id`,
/// spawn the player at `zone.player_spawn` with the zone's declared
/// class, and spawn all monsters. The single entry point used by
/// SimBridge::init_game.
pub fn new_game_in_zone(seed: u64, data: &GameData, zone_id: &str) -> Result<GameState, String> {
    let zone = data
        .zone(zone_id)
        .ok_or_else(|| format!("unknown zone: {zone_id}"))?;
    let mut state = GameState::new(seed, zone.width, zone.height);
    // BETA-5 Phase 1.5: the zone's published class is the starting
    // class for the per-class roster. Stored on GameState so that
    // save/load preserves it without bridge-side recovery heuristics.
    state.current_unit_def = Some(zone.unit.clone());
    // Order: player first, then monsters. The tick loop processes
    // entities in spawn order, so player gets target select / first
    // swing before any monster does at tick 1. This is published
    // behavior — don't reorder casually.
    spawn_player_from_zone(&mut state, data, zone_id)?;
    state.claim_fielded_slots();
    populate_zone(&mut state, data, zone_id)?;
    Ok(state)
}

/// The unit every new account starts with (user 2026-10-12: Suzu; `docs/gacha-economy-design.md`).
pub const STARTER_UNIT: &str = "suzu";

/// Fixed starter party, shared by the client and verifier genesis calculation.
/// Legacy constructors remain available to load/replay historical scenarios.
///
/// ONE UNIT ALONE since 0.0.415 (user 2026-09-17: "ทีมเริ่มด้วย seraphine"; the unit is Suzu
/// since 0.0.493, user 2026-10-12, matching the gacha onboarding): Rapis' AoE kit ran
/// the ladder about twice as fast as the party it was tuned on (stage 138 vs 64 at 24 h),
/// and every other character comes from gacha, so the starter is the one free unit and the
/// party's slots are what Gil buys.
pub fn new_starter_party(seed: u64, data: &GameData, zone_id: &str) -> Result<GameState, String> {
    let zone = data.zone(zone_id).ok_or_else(|| format!("unknown zone: {zone_id}"))?;
    let mut state = GameState::new(seed, zone.width, zone.height);
    state.current_unit_def = Some(STARTER_UNIT.into());
    for (offset, id) in [STARTER_UNIT].into_iter().enumerate() {
        let character = data.unit(id).ok_or_else(|| format!("missing starter: {id}"))?;
        let pos = party_cell(zone.player_spawn, offset, zone.width, zone.height);
        state.grid.spawn(build_player_entity(data, character, FIRST_PLAYER_ID + offset as u32, pos));
    }
    state.claim_fielded_slots();
    populate_zone(&mut state, data, zone_id)?;
    Ok(state)
}

/// Like `new_game_in_zone` but spawns a SOLO player of `unit_def`,
/// overriding the zone's declared class/party. Used by character
/// creation so the player's chosen class is the starting lead. Stats
/// are the class base here; the creation screen overrides them via the
/// bridge before play begins.
pub fn new_game_as(
    seed: u64,
    data: &GameData,
    zone_id: &str,
    unit_def: &str,
) -> Result<GameState, String> {
    let zone = data.zone(zone_id).ok_or_else(|| format!("unknown zone: {zone_id}"))?;
    let class = data
        .unit(unit_def)
        .ok_or_else(|| format!("unknown unit: {unit_def}"))?;
    let mut state = GameState::new(seed, zone.width, zone.height);
    state.current_unit_def = Some(unit_def.to_string());
    let pos = party_cell(zone.player_spawn, 0, zone.width, zone.height);
    state.grid.spawn(build_player_entity(data, class, FIRST_PLAYER_ID, pos));
    state.claim_fielded_slots();
    populate_zone(&mut state, data, zone_id)?;
    Ok(state)
}

/// Tear down the current zone's arena (everything but the player) and
/// load a new zone. Triggered by the gdext bridge (or sim-cli) after
/// the sim sets `state.pending_zone_change`.
pub fn change_zone(state: &mut GameState, data: &GameData, zone_id: &str) -> Result<(), String> {
    let zone = data
        .zone(zone_id)
        .ok_or_else(|| format!("unknown zone: {zone_id}"))?;
    // Per-SESSION resets (Hiding charge + Bloodthirst streak) key off whether
    // the destination is an instance's room 1 — computed BEFORE populate_zone
    // overwrites zone_spawn. Robust across advance / retreat / farm-loop / any
    // zone change, since every transition funnels through here.
    // Entering room 1 of an instance = a NEW SESSION/RUN (band advance,
    // auto-retreat, OR a farm-mode loop back to room 1 of the same band). One
    // instance = 3 rooms = one session; landing on room 1 begins a fresh one.
    // Drives the per-session resets below (Hiding charge + Bloodthirst streak) —
    // a mid-instance room hop (room 1→2→3) is the SAME session, so neither resets.
    let is_instance_start = zone.instance_start_zone_id == zone_id;
    // A new RUN starts here — reset what the clear screen will report.
    if is_instance_start {
        state.run_tally = crate::state::RunTally {
            started_tick: state.tick,
            start_kills: state.measures.kills.get(),
            xp: 0,
            gil: 0,
            loot: Vec::new(),
        };
    }
    // Keep player(s); drop monsters + corpses. Player inventory carries
    // over across zones (auto-loot model — items are character-bound).
    state.grid.entities.retain(|e| matches!(e.team, Team::Player));
    state.projectiles.clear();
    state.ground_fields.clear();
    state.traps.clear(); // armed traps don't follow the party to a new zone
    state.grid.width = zone.width;
    state.grid.height = zone.height;
    // Move the party to the new zone's spawn + full heal. Spread them
    // around the spawn cell (party_cell) so members don't pile up.
    let (w, h) = (zone.width, zone.height);
    let mut party_i = 0usize;
    for e in state.grid.entities.iter_mut() {
        if !matches!(e.team, Team::Player) {
            continue;
        }
        let pos = party_cell(zone.player_spawn, party_i, w, h);
        party_i += 1;
        e.pos = pos;
        e.target = pos;
        e.target_id = None;
        e.move_acc = 0;
        e.atk_acc = 0;
        e.casting = None; // drop any in-progress spell windup on warp/revive
        // Absorb shields (Flame Shield, Praesidium) follow the buff/Bloodthirst
        // lifetime: they CARRY across a mid-instance room hop (1→2→3) and clear
        // only on a NEW RUN (`is_instance_start` — band advance / retreat /
        // farm-loop) or DEATH. (The `persist` flag is reserved for a future
        // survives-a-new-run effect and is intentionally not consulted yet.)
        if is_instance_start || !e.alive {
            e.shields.clear();
            e.pending_bursts.clear();
        }
        // Zolfia's S1 stacks count one run.
        if is_instance_start {
            e.shatter_stacks = 0;
        }
        // (Full-heal happens AFTER this loop — the Soul-path auras that lift max
        // HP/SP are only stamped on ALIVE players, and revive is below at `e.alive
        // = true`, so healing here would use the base max on a respawn.)
        // Bloodthirst streak resets at the START of a new instance run
        // (`is_instance_start` — band advance, retreat, OR a farm-mode loop back
        // to room 1) OR on death (`!e.alive` before the revive below). A
        // mid-instance room hop (1→2→3) keeps the streak alive.
        if is_instance_start || !e.alive {
            e.bloodthirst_stacks = 0;
        }
        e.alive = true;
        // Starting a new session (room 1) refreshes the thief's once-per-session
        // Hiding charge — including a farm-mode loop back to room 1.
        if is_instance_start {
            e.hiding_used = false;
            // Bishop Resurrection refills its single charge per band (like Hiding).
            for s in e.skills.iter_mut() {
                if s.id == "resurrection" {
                    s.charges_left = 1;
                }
            }
        }
        // Soul Crush refills its charges in full on every zone change (5 × level).
        for s in e.skills.iter_mut() {
            if s.id == "soul_crush" {
                s.charges_left = 5 * s.level as u32;
            }
        }
    }
    // NOW that every player is revived (alive), stamp the always-on Soul-path auras
    // and heal to the real effective max (base + VIT + HpPct + flat MaxHpAdd).
    // Order matters: the aura pass skips DEAD entities, so it must run after the
    // revive loop above — otherwise a respawn heals only to the base max.
    //
    // HOW MUCH (0.0.398, `stage-ladder-renovation.md` §4): room 1 of a stage heals in FULL — a new
    // run. A hop to room 2 or 3 heals `ROOM_HOP_HEAL_BP` of max (+ the party's stage-recovery
    // lines, when those exist), capped at max; a member who fell in the room behind stands up at
    // that same amount. So a stage is one three-room run the party has to last through, and HP,
    // regen and sustain are worth carrying — the difficulty no longer has to live in stat cliffs.
    crate::tick::apply_soul_path_auras(state);
    for e in state.grid.entities.iter_mut() {
        if !matches!(e.team, Team::Player) {
            continue;
        }
        let max = crate::combat::effective_max_hp(e);
        e.hp = if is_instance_start {
            max
        } else {
            let heal = ((max as i64) * i64::from(ROOM_HOP_HEAL_BP) / 10_000).max(1) as i32;
            e.hp.max(0).saturating_add(heal).min(max)
        };
    }
    populate_zone(state, data, zone_id)?;
    // WHICH GRACE THIS OPEN GETS. A hop WITHIN an instance (room 1 -> 2 -> 3) is the walk-through
    // march — the party crosses 52 m of authored ground and the fight must not resume mid-stride.
    // Landing on an instance's room 1 is not: a band advance, a farm loop and a retry all put the
    // party down where it stands, and the renderer only walks them in from just off the left edge.
    // `is_instance_start` is already computed above for the per-session resets and it splits these
    // exactly the way the renderer does, with no zone-id parsing of its own.
    // The march's length follows the party's MOVE SPEED (0.0.398, user: "เดินข้ามห้องไวขึ้นตาม move
    // speed"): the slowest living member sets the pace so nobody is left behind.
    let slowest_bp = state
        .grid
        .entities
        .iter()
        .filter(|e| matches!(e.team, Team::Player) && !e.is_pet)
        .map(crate::combat::stat_mods::move_speed_bp)
        .min()
        .unwrap_or(0);
    if let Some(z) = state.zone_spawn.as_mut() {
        z.deploy_grace_ticks = if is_instance_start {
            crate::tick::DEPLOY_GRACE_TICKS_OPEN
        } else {
            crate::tick::march_grace_ticks(slowest_bp)
        };
    }
    crate::lane::enter_zone(state, data)?;
    state.pending_zone_change = None;
    Ok(())
}

/// What a hop to the next room of a stage heals, in bp of max HP (0.0.398). Room 1 heals in full.
pub const ROOM_HOP_HEAL_BP: u32 = 2000;

/// Handle a PARTY WIPE (every member down): count it, clear the loot escrow (the risked held
/// loot) and FALL BACK one stage to farm it (or restart room 1 on stage 1 / a player farm).
/// Always KEEPS xp/job/soul. Used by BOTH the bridges and sim-cli so the game and the headless
/// pace measurement share one behavior.
pub fn handle_party_wipe(state: &mut GameState, data: &GameData) -> Result<(), String> {
    crate::drops::clear_escrow(state);
    state.consecutive_wipes += 1;
    // `consecutive_wipes` is a STREAK, zeroed by a clear; the stat wants the history.
    state.measures.wipes.add(1);
    let inst_start = state
        .zone_spawn
        .as_ref()
        .map(|zs| zs.instance_start_zone_id.clone())
        .filter(|s| !s.is_empty());
    let cur_band = inst_start
        .as_deref()
        .and_then(crate::state::band_of_zone)
        .unwrap_or(1);
    // FALL BACK (0.0.403, WoG's rule): a wipe on stage N+1 sends the party to stage N in farm
    // mode for `WIPE_FALLBACK_FARM_CLEARS` clears, after which `tick` sends it back up — the
    // auto-advance no longer throws an idle party at the same wall until it happens to win. A
    // wipe on stage 1, or while the PLAYER has farm mode on (they chose this stage), just restarts
    // the stage at room 1 (a full heal). Wiping on the fall-back stage falls back again, and the
    // return target stays the highest stage that walled the party.
    let player_farming = state.farm_current_instance && state.retreat_return_band == 0;
    if cur_band > 1 && !player_farming {
        state.retreat_return_band = state.retreat_return_band.max(cur_band);
        state.retreat_farm_remaining = crate::state::wipe_fallback_clears(cur_band as u16);
        state.farm_current_instance = true;
        return change_zone(state, data, &format!("zone_b{}_1", cur_band - 1));
    }
    let start = inst_start.unwrap_or_else(|| format!("zone_b{cur_band}_1"));
    change_zone(state, data, &start)
}

/// Map a monster def's race STRING to the enum — unknown/empty = `Unknown`
/// (matches no conditional), so stale data can never panic a load.
fn parse_race(s: &str) -> crate::grid::Race {
    use crate::grid::Race::*;
    match s {
        "Formless" => Formless,
        "Undead" => Undead,
        "Brute" => Brute,
        "Plant" => Plant,
        "Insect" => Insect,
        "Fish" => Fish,
        "Demon" => Demon,
        "DemiHuman" => DemiHuman,
        "Angel" => Angel,
        "Dragon" => Dragon,
        _ => Unknown,
    }
}

/// Map a monster def's size STRING — unknown/empty defaults to `Medium`.
fn parse_size(s: &str) -> crate::grid::Size {
    use crate::grid::Size::*;
    match s {
        "Small" => Small,
        "Large" => Large,
        _ => Medium,
    }
}

#[cfg(test)]
mod armour_from_level_tests {
    /// One level describes an encounter, and it is the ZONE's. The same creature is a level-1 one
    /// in the first band and a level-5 one three bands later — three bands share a pool precisely
    /// so they can differ by level, and pinning the level to the MonsterDef collapses them into
    /// three copies of the same fight. That was shipped for one version (0.0.279) and this test is
    /// what would have caught it: it asserts a monster in two different zones is two different
    /// strengths.
    #[test]
    fn the_same_monster_is_a_different_strength_in_a_different_band() {
        let d = crate::data::GameData::from_json_str(include_str!(
            "../../../../../data/game_data.json"
        ))
        .expect("game_data.json must parse");

        // Stages 1-5 share one pool; each is a step up the stage table, and the level it derives
        // (`level_of_stage`) is what DEF reads.
        let mut seen = Vec::new();
        for zone_id in ["zone_b1_1", "zone_b2_1", "zone_b3_1", "zone_b4_1", "zone_b5_1"] {
            let zone = d.zone(zone_id).expect("zone exists");
            let s = crate::data::new_game_in_zone(42, &d, zone_id).expect("zone loads");
            let t = s
                .zone_spawn
                .as_ref()
                .expect("zone spawn state")
                .pool
                .iter()
                .map(|p| &p.template)
                .find(|t| t.def_id == "baby_pinkling")
                .unwrap_or_else(|| panic!("baby_pinkling is not in {zone_id}'s pool"));
            assert_eq!(
                (t.stage, t.def),
                // DEF is the stage table's, carrying the x10 stat scale (0.0.84).
                (zone.stage, d.stage_stats(zone.stage).def),
                "{zone_id}: the encounter's stage is the ZONE's {}",
                zone.stage
            );
            seen.push((zone.stage, t.hp, t.atk));
        }
        let st: Vec<_> = seen.iter().map(|s| s.0).collect();
        assert_eq!(st, vec![1, 2, 3, 4, 5], "zone N is stage N");
        assert!(
            seen.windows(2).all(|w| w[0].1 < w[1].1),
            "the SAME monster must get stronger band to band, got hp {:?} — if any two are equal, \
             strength has been pinned to the MonsterDef again and a whole pool group is one fight",
            seen.iter().map(|s| s.1).collect::<Vec<_>>()
        );

        // The identity knob still moves PHYSICAL armour only, and off the zone's level.
        let s = crate::data::new_game_in_zone(42, &d, "zone_dungeon_duskmourn_4").expect("dungeon");
        let zone = d.zone("zone_dungeon_duskmourn_4").unwrap();
        let b = s.zone_spawn.as_ref().unwrap().boss_template.as_ref().expect("boss");
        let want = d.monster("dullahan").unwrap();
        assert_eq!(b.def_id, "dullahan");
        assert_eq!(
            b.def,
            // carries the x10 stat scale (0.0.84)
            (d.stage_stats(zone.stage).def as i64 * want.def_mult_bp as i64 / 10_000) as i32,
            "def_mult_bp scales boss armour off the zone's level"
        );
    }
}
