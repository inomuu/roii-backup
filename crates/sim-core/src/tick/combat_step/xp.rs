//! Monster death rewards (Gil / Soul / shared XP) and the base + job
//! level/XP progression curves.

// DEBT (66 sites): this file predates `deny(clippy::indexing_slicing)`. An index that
// cannot be proven in range is a panic, and in verify-server (which links this crate and
// replays saves under `panic = "abort"`) a panic is the whole process. Shrink this, never
// add to it — a NEW file has no allow and is rejected on its first bare index.
#![allow(clippy::indexing_slicing)]

use crate::soul_paths::{self, PathEffect};
use crate::grid::Team;
use crate::state::{GameState, GilEarn};

/// An entity just died (the caller already set `alive = false`): roll its drop
/// table and award the kill. `drop_on_death` + `handle_monster_death` were always
/// called as this pair at every death site — this is that pair, with `killer` =
/// the last-hitter index for on-kill effects (Steal …), or `None` (DoT / click /
/// projectile, which carry no attacker index).
/// Per-member share of a kill's XP by party size (RO-ish soft split): 1→100% … 5→40%.
pub const SHARE_PCT: [u64; 5] = [100, 70, 55, 45, 40];

/// OFFLINE PROGRESSION (user 2026-10-02): EXP only, 80% of the online rate at the latest band
/// reached, at most 10 hours, no EXP Elixir / crest / Insight bonus.
///
/// Online, a member earns `stage_base.xp × SHARE_PCT × ~660` an hour at any band — measured with
/// sim-cli (3 seeds × 24 h, real gacha parties, stages 7-23: 629-749): the kill rate is bounded by
/// the spawn rate, not the party's damage, so one constant serves every band. 80% of it is 528.
pub const OFFLINE_KILLS_PER_HOUR: u64 = 528;
/// A gap shorter than this is a reconnect, not time away (the client saves every 10 s).
pub const OFFLINE_MIN_MINUTES: u64 = 10;
pub const OFFLINE_MAX_MINUTES: u64 = 600;

/// EXP each fielded member earns for `minutes` away (capped), at the save's latest band reached.
/// Always the FIVE-member share (user 2026-10-02: "เฉลี่ยของทีม 5 ตัว แม้ตัวละครจะน้อยกว่า 5"), which
/// also keeps the amount independent of who is fielded when the envelope is claimed.
pub fn offline_xp_per_member(state: &GameState, data: &crate::data::GameData, minutes: u64) -> u64 {
    if minutes < OFFLINE_MIN_MINUTES {
        return 0;
    }
    let minutes = minutes.min(OFFLINE_MAX_MINUTES);
    let band = state.band_reached.max(1);
    let Some(base) = data.stage_base.iter().find(|r| u32::from(r.stage) == band).map(|r| u64::from(r.xp)) else {
        return 0;
    };
    let pct = SHARE_PCT[SHARE_PCT.len() - 1];
    base * pct * OFFLINE_KILLS_PER_HOUR * minutes / 60 / 100
}

/// `xp` to every fielded member, through the same level-up path a kill uses.
pub fn grant_party_xp(state: &mut GameState, xp: u64) {
    for pidx in 0..state.grid.entities.len() {
        let e = &state.grid.entities[pidx];
        if !matches!(e.team, Team::Player) || e.is_pet {
            continue;
        }
        state.grid.entities[pidx].stats.xp = state.grid.entities[pidx].stats.xp.saturating_add(xp);
        try_level_up(state, pidx);
    }
}

pub(crate) fn kill_monster(state: &mut GameState, idx: usize, killer: Option<usize>) {
    // A summon is a combat hazard, not a reward: it rolls no drops, and `handle_monster_death`
    // pays nothing for it. Combat hooks (rank on-kill, corpse blasts) still run.
    if !state.grid.entities.get(idx).is_some_and(|e| e.is_summon) {
        crate::drops::drop_on_death(state, idx);
    }
    handle_monster_death(state, idx, killer);
    // S4 (0.0.441, Rapis): the corpse detonates. After the death is booked, so a chain of kills
    // resolves one at a time in spawn order.
    if let Some(k) = killer {
        crate::unit_rank::on_kill(state, k, idx);
    }
}

pub(crate) fn handle_monster_death(state: &mut GameState, idx: usize, killer: Option<usize>) {
    if !matches!(state.grid.entities[idx].team, Team::Monster) {
        return;
    }
    // SUMMON (audit 2026-10-02/03): no soul, kill counts, room progress, bestiary, XP or steal —
    // only the corpse effects a kill always has.
    if state.grid.entities[idx].is_summon {
        if let Some(k) = killer {
            maybe_poison_explosion(state, k, idx);
        }
        schedule_explosion(state, idx);
        maybe_schedule_lumen(state, idx);
        return;
    }
    let is_boss = state.grid.entities[idx].is_boss;
    let xp_reward = state.grid.entities[idx].xp_reward;
    // Gil is party-shared (one pool), awarded once per kill — `award_gil` folds
    // the PATHS OF SOUL "Greed" (GilPct) bonus and carries the sub-Gil remainder.
    // (gil_reward is 0 in current data, so this is a no-op until kills pay Gil.)
    let mut gil_reward = u64::from(state.grid.entities[idx].gil_reward);
    // Sticky Fingers (0.0.464, Laplus): the killer's `kill_gil_bonus_bp` passive, on a LAST HIT
    // of its own — the party's other kills pay the plain price.
    if let Some(k) = killer {
        let bonus: u64 = state.grid.entities[k]
            .skills
            .iter()
            .filter(|s| s.level > 0)
            .filter_map(|s| crate::skills::skill_def(&s.id))
            .map(|d| u64::from(d.kill_gil_bonus_bp))
            .sum();
        if bonus > 0 {
            gil_reward = gil_reward.saturating_mul(10_000 + bonus) / 10_000;
        }
    }
    state.earn_gil(gil_reward, GilEarn::MonsterKill);
    // Soul is party-shared and KEPT through a wipe (unlike loot). It scales with
    // the current BAND so the running total keeps growing as you progress (the
    // idle-incremental feel), with bosses ×3. Band is parsed from the instance's
    // "zone_bN_1" start id. The "Avarice" path bonus folds in last.
    let band = state
        .zone_spawn
        .as_ref()
        .map(|z| band_of(&z.instance_start_zone_id))
        .unwrap_or(1);
    // Soul scales with the REGION (legacy band = 3 sub-bands, 0.0.226 split) so
    // the faucet keeps its pre-split curve — sub-band numbers tripling must not
    // triple soul income at the same content depth.
    let region = (band as u64 + 2) / 3;
    let base_soul = region * if is_boss { 3 } else { 1 };
    let mut soul = soul_paths::apply_reward_bonus(base_soul, &state.soul_paths, PathEffect::SoulPct);
    // Apex "Avarice" (0.0.227): the party's HIGHEST soul-gain bucket lifts the
    // shared reward (best farmer carries the party). Max over players, in spawn
    // order → deterministic; 0 for a party with no Avarice ⇒ byte-identical.
    let avarice_bp = state
        .grid
        .entities
        .iter()
        .filter(|e| matches!(e.team, crate::grid::Team::Player))
        .map(|_e| 0u32) // Apex Avarice/soul bucket retired (docs: soul is party-shared)
        .max()
        .unwrap_or(0);
    if avarice_bp > 0 {
        soul = soul.saturating_mul(10_000 + avarice_bp as u64) / 10_000;
    }
    // Soul crest (0.0.433): +% on every kill's soul. 0 without it.
    soul = soul.saturating_mul((10_000 + crate::crest::soul_gain_bp(state).max(0)) as u64) / 10_000;
    state.earn_soul(soul, crate::state::SoulEarn::MonsterKill);
    state.measures.kills.add(1);
    if let Some(zs) = state.zone_spawn.as_mut() {
        if is_boss {
            // The zone is NOT cleared by the boss's death alone any more: the party has to sweep
            // the whole field (boss + minions + any mob still standing), so `mark_zone_cleared`
            // (called each tick after spawns) flips `zone_cleared` when no monster is left alive.
            // Killing the boss only stops the spawner (`boss_spawned` was set when it appeared) and
            // books the boss-kill stat here; `rooms_cleared` moves to the actual clear.
            state.measures.boss_kills.add(1);
        } else {
            zs.kill_counter = zs.kill_counter.saturating_add(1);
        }
    }
    // BESTIARY: tally the kill under the def's bestiary GROUP (same-model
    // family shares one pool; empty group = the def's own id). Milestone
    // rewards are NOT auto-paid — the player claims them in the Bestiary
    // screen via InputEvent::ClaimBestiaryReward (recorded, replay-safe).
    let did = state.grid.entities[idx].def_id.clone();
    if !did.is_empty() {
        let group = state
            .zone_spawn
            .as_ref()
            .and_then(|zs| {
                zs.pool
                    .iter()
                    .map(|e| &e.template)
                    .chain(zs.boss_template.iter())
                    .find(|t| t.def_id == did)
                    .map(|t| t.bestiary_group.clone())
            })
            .unwrap_or_default();
        let key = if group.is_empty() { did } else { group };
        let n = state.monster_kills.entry(key).or_insert(0);
        *n = n.saturating_add(1);
    }
    // Per-character XP: every LIVING party member gets a SHARE of the kill XP
    // by party size (RO-ish soft split, friendlier than 1/N): 1→100% / 2→70% /
    // 3→55% / 4→45% / 5→40%. Solo = 100% (goldens unaffected). Harsher for big
    // parties so a full party (the late game) levels slower — early solo stays
    // fast. Killing-blow attribution is still deferred.
    if xp_reward > 0 {
        // Counted over the WHOLE party, not the survivors: now that the downed are paid, a death
        // must not quietly raise everyone's share by shrinking the divisor.
        let party = state
            .grid
            .entities
            .iter()
            .filter(|e| matches!(e.team, Team::Player) && !e.is_pet)
            .count();
        let pct = SHARE_PCT[party.max(1).min(SHARE_PCT.len()) - 1];
        // Base EXP also takes the "Insight" bonus.
        let xp = soul_paths::apply_reward_bonus(
            (xp_reward as u64) * pct / 100, &state.soul_paths, PathEffect::ExpPct);
        // XP crest (0.0.433): +% on the whole kill. 0 without it.
        let xp = xp.saturating_mul((10_000 + crate::crest::xp_gain_bp(state).max(0)) as u64) / 10_000;
        // EXP Elixir (2026-10-01): the party's highest XpPct buff, never a sum. 0 without one.
        // The single strongest XpPct BUFF, never a sum — per member or across sources (audit
        // 2026-10-02: unloading and reloading the rack left the old buff running on its own
        // source key, so three cycles summed to +60%).
        let elixir_bp = state.grid.entities.iter()
            .filter(|e| matches!(e.team, Team::Player) && e.alive)
            .flat_map(|e| e.buffs.iter())
            .filter(|b| b.kind == crate::skills::BuffKind::XpPct)
            .map(|b| b.magnitude.max(0))
            .max()
            .unwrap_or(0);
        let xp = xp.saturating_mul((10_000 + elixir_bp) as u64) / 10_000;
        // Run tally (dungeon clear summary): base EXP per member, as awarded.
        state.run_tally.xp = state.run_tally.xp.saturating_add(xp);
        let count = state.grid.entities.len();
        // A DOWNED member still earns. Excluding them made the split punish exactly the member
        // who was already behind — die once, miss that band's XP, fall further behind, die more —
        // and it is the party that killed the monster, not the survivors (user 2026-08-04:
        // "ไม่ให้ตัวละครเลเวลห่างกัน"). `alive` gates who FIGHTS, not who is on the payroll.
        // A member removed from the party is a different case and is already off `entities`.
        for pidx in 0..count {
            let e = &state.grid.entities[pidx];
            if !matches!(e.team, Team::Player) || e.is_pet {
                continue;
            }
            state.grid.entities[pidx].stats.xp =
                state.grid.entities[pidx].stats.xp.saturating_add(xp);
            try_level_up(state, pidx);
        }
    }

    // Steal (thief passive): on a player's LAST-HIT kill, a level-scaled chance
    // (10%/level) to snatch an EXTRA roll from the victim's drop table — the roll
    // obeys the table (the NOTHING sentinel = a miss), so it's a modest bonus, not
    // doubled loot. The proc roll + the drop roll are BOTH conditional on the
    // killer carrying Steal, so skill-less golden streams stay byte-identical.
    if let Some(k) = killer {
        let steal_lvl = state.grid.entities[k].skill_level("steal");
        if steal_lvl > 0 && state.rng.gen_range_u32(0, 10_000) < steal_lvl as u32 * 1_000 {
            crate::drops::drop_on_death(state, idx);
        }

        // Arrow Crafter (Ranger passive): on a LAST-HIT kill of a NON-Neutral
        // enemy, 10%/level chance to craft 1 arrow of the victim's element into the
        // killer's ammo pool. The roll is taken ONLY when the killer owns the skill
        // AND the victim is non-Neutral → skill-less golden streams stay identical.
        let ac_lvl = state.grid.entities[k].skill_level("arrow_crafter");
        let victim_el = state.grid.entities[idx].element;
        if ac_lvl > 0
            && victim_el != crate::grid::Element::Physical
            && state.rng.gen_range_u32(0, 10_000) < ac_lvl as u32 * 1_000
        {
            let ammo = &mut state.grid.entities[k].ammo;
            match ammo.iter_mut().find(|a| a.element == victim_el) {
                Some(stack) => stack.shots = stack.shots.saturating_add(1),
                None => ammo.push(crate::items::AmmoStack { element: victim_el, shots: 1 }),
            }
        }

        // Poison Explosion: a poison-afflicted corpse detonates (see below).
        maybe_poison_explosion(state, k, idx);
    }
    // Bomb monsters (ChomBombs): SCHEDULE the blast to fire in sync with the explosion
    // animation (a few ticks after death), not the instant of death.
    schedule_explosion(state, idx);
    // Lumen Contagio: a marked corpse detonates a scheduled holy blast (see below).
    maybe_schedule_lumen(state, idx);
}

/// Ticks between a bomb monster's death and its blast — so the sim damage + knockback
/// land IN SYNC with the explosion animation (blink → boom), not the instant of death.
const EXPLODE_DELAY: u64 = 2;

/// Queue a dying monster's on-death blast, IF it carries an on-death blast passive.
/// CONVENTION: a `Passive` skill with `self_aoe_radius > 0` is an on-death blast
/// (`self_destruct` / `toxic_burst`). Its params: `power_bp` = FLAT HP damage (no
/// caster ATK to scale — the owner is dead), `knockback` = shove cells,
/// `self_aoe_radius` = radius, `on_hit_debuff` Poison = a lingering toxic cloud.
/// No-op for a monster with no such skill.
fn schedule_explosion(state: &mut GameState, idx: usize) {
    use crate::skills::{skill_def, BuffKind, SkillKind};
    let e = &state.grid.entities[idx];
    let Some(def) = e.skills.iter().find_map(|s| {
        if s.level == 0 {
            return None;
        }
        let def = skill_def(&s.id)?;
        (def.kind == SkillKind::Passive && def.self_aoe_radius > 0).then_some(def)
    }) else {
        return;
    };
    let poison_ticks = def
        .on_hit_debuff
        .as_ref()
        .filter(|d| matches!(d.kind, BuffKind::Poison))
        .map(|d| d.duration)
        .unwrap_or(0);
    // A `heals_ally` blast reads power_bp as a flat HEAL to allies; a normal one deals it.
    let heal = if def.heals_ally { def.power_bp as i32 } else { 0 };
    state.pending_explosions.push(crate::state::PendingExplosion {
        x: e.pos.x,
        y: e.pos.y,
        dmg: if heal > 0 { 0 } else { def.power_bp as i32 },
        push: if heal > 0 { 0 } else { def.knockback },
        radius: def.self_aoe_radius,
        poison_ticks: if heal > 0 { 0 } else { poison_ticks },
        heal,
        src_id: e.id,
        fire_tick: state.tick + EXPLODE_DELAY,
        lumen_level: 0,
    });
}

/// Lumen Contagio (Lucielle): when a `LumenVuln`-marked enemy dies (by ANY hand),
/// the Exemplar owner detonates a MAGIC HOLY 5×5 blast at the corpse, SCHEDULED
/// (EXPLODE_DELAY) so the contagion spreads across ticks instead of recursing in one.
/// Owner = the lowest-id living party member who knows `lumen_contagio`; the blast
/// scales off THEIR MATK, snapshot now (they may die before it fires). No owner → the
/// mark simply lapses. Gated on the victim carrying the mark → skill-less streams
/// are byte-identical.
fn maybe_schedule_lumen(state: &mut GameState, victim_idx: usize) {
    use crate::grid::Team;
    if crate::skills::buff_sum(&state.grid.entities[victim_idx], crate::skills::BuffKind::LumenVuln) == 0 {
        return;
    }
    // Find the owner (lowest-id living player who knows the skill) + their level.
    let owner = state.grid.entities.iter().enumerate().find_map(|(i, e)| {
        if e.alive && matches!(e.team, Team::Player) {
            let lv = e.skill_level("lumen_contagio");
            if lv > 0 {
                return Some((i, lv));
            }
        }
        None
    });
    let Some((oi, level)) = owner else { return };
    // Explosion power: 1500·level bp of MATK (Lv5 75% … Lv10 150%), snapshot now.
    // (×3 buff — was 500·level / Lv5 25%…Lv10 50%; the blast felt too light.)
    let amount = (crate::combat::effective_atk(&state.grid.entities[oi]) as i64
        * (1500 * level as i64)
        / 10_000) as i32;
    let pos = state.grid.entities[victim_idx].pos;
    state.pending_explosions.push(crate::state::PendingExplosion {
        x: pos.x,
        y: pos.y,
        dmg: amount,
        push: 0,
        radius: 2, // 5×5
        poison_ticks: 0,
        heal: 0,
        src_id: state.grid.entities[oi].id,
        fire_tick: state.tick + EXPLODE_DELAY,
        lumen_level: level,
    });
}

/// Fire every scheduled explosion whose `fire_tick` has arrived (called once per tick).
pub(crate) fn fire_pending_explosions(state: &mut GameState) {
    if state.pending_explosions.is_empty() {
        return;
    }
    let now = state.tick;
    // Drain the DUE ones in schedule order (stable → deterministic); keep the rest.
    let mut due: Vec<crate::state::PendingExplosion> = Vec::new();
    state.pending_explosions.retain(|p| {
        if p.fire_tick <= now {
            due.push(*p);
            false
        } else {
            true
        }
    });
    for p in due {
        resolve_explosion(state, &p);
    }
}

/// Detonate a bomb blast: flat `dmg` + `push`-cell knockback to EVERY alive entity
/// (BOTH teams) within `radius` of the corpse, plus `poison_ticks` of classic Poison
/// to living survivors (bosses immune). Deterministic (no PRNG). A MONSTER killed by
/// the blast schedules its OWN explosion (bounded chain — each entity dies + fires once).
/// A `heal` blast (Rechum) instead restores HP to nearby ally monsters and returns early.
fn resolve_explosion(state: &mut GameState, p: &crate::state::PendingExplosion) {
    let center = crate::grid::Cell::new(p.x, p.y);
    if p.heal > 0 {
        resolve_heal_blast(state, center, p.radius.max(1), p.heal);
        return;
    }
    if p.lumen_level > 0 {
        resolve_lumen_explosion(state, p);
        return;
    }
    let victims = super::victims_in_radius(&state.grid.entities, center, p.radius.max(1), None);
    for vidx in victims {
        if !state.grid.entities[vidx].alive {
            continue; // a chain / earlier blast already killed it
        }
        let dealt = p.dmg.min(state.grid.entities[vidx].hp.max(1));
        let tgt_id = state.grid.entities[vidx].id;
        let was_monster = matches!(state.grid.entities[vidx].team, Team::Monster);
        state.grid.entities[vidx].take_hit(p.dmg);
        state.recent_hits.push(crate::state::HitEvent {
            target_id: tgt_id,
            attacker_id: p.src_id,
            amount: dealt,
            hits: 1,
            kind: crate::state::HitKind::Normal,
        });
        if state.grid.entities[vidx].alive {
            // Toxic cloud: poison the survivor (classic Poison can never kill; bosses immune).
            if p.poison_ticks > 0 && !state.grid.entities[vidx].is_boss {
                crate::tick::upkeep::apply_or_refresh_poison(
                    &mut state.grid.entities[vidx],
                    p.poison_ticks,
                );
            }
            // Knockback AWAY from the blast cell (the bomb is gone — cell-based helper).
            if p.push > 0 {
                super::knockback_from(state, vidx, center, p.push);
            }
        }
        if was_monster && !state.grid.entities[vidx].alive {
            kill_monster(state, vidx, None); // schedules its own blast → chain
        }
    }
}

/// Rechum's Mending Burst: restore `heal` HP to every living ALLY MONSTER within
/// `radius` of the corpse (the dead Rechum at the centre is excluded). No damage,
/// knockback, poison, or chain. `Some(Team::Player)` selects the non-Player team
/// (the monsters). Deterministic (no PRNG). HP goes up → the renderer pops a heal.
fn resolve_heal_blast(state: &mut GameState, center: crate::grid::Cell, radius: i32, heal: i32) {
    let allies = super::victims_in_radius(&state.grid.entities, center, radius, Some(Team::Player));
    for aidx in allies {
        if !state.grid.entities[aidx].alive {
            continue;
        }
        let maxhp = crate::combat::effective_max_hp(&state.grid.entities[aidx]);
        let e = &mut state.grid.entities[aidx];
        let before = e.hp;
        e.hp = (e.hp + heal).min(maxhp);
        let (got, who) = (e.hp - before, e.id);
        crate::state::push_heal(state, who, who, got, crate::state::HealKind::Skill);
    }
}

/// Poison Explosion (Assassin passive): when the killer `k` owns it (off
/// cooldown) and the just-killed `victim_idx` was poison-afflicted (classic
/// `Poison` OR Deadly `PoisonDot` — NOT Bleed), detonate at the corpse: a 5×5
/// physical, Poison-element blast whose TOTAL power (`500% + 100%/level` ATK) is
/// split EQUALLY among the living enemies caught. The 30 s cooldown is latched
/// BEFORE the blast, so any kill it scores can't chain another explosion.
/// Deterministic (no PRNG); no-op unless the killer carries the ready passive.
/// Lumen Contagio blast: a MAGIC HOLY 5×5 hitting ENEMIES (scaled MATK in `p.dmg`, so the
/// target's MDEF + holy-vuln apply), then RE-MARKS every un-marked survivor with `LumenVuln`
/// (contagion). A marked enemy killed here dies through `kill_monster` → `handle_monster_death`
/// → schedules ITS OWN blast, so the plague spreads ACROSS ticks (EXPLODE_DELAY breaks
/// same-tick recursion).
fn resolve_lumen_explosion(state: &mut GameState, p: &crate::state::PendingExplosion) {
    use crate::combat::{self, Damage};
    use crate::grid::{DamageType, Element, Team};
    let center = crate::grid::Cell::new(p.x, p.y);
    let owner = state.grid.idx_of_id(p.src_id);
    let victims = super::victims_in_radius(&state.grid.entities, center, p.radius.max(1), Some(Team::Player));
    state.poison_bursts.push(crate::state::BurstFx {
        x: center.x,
        y: center.y,
        radius: p.radius,
        element: Element::Light,
    });
    for vidx in victims {
        if !state.grid.entities[vidx].alive {
            continue;
        }
        // 0.0.472: the blast crits off the one who planted the plague.
        let (amount, is_crit) = match state.grid.idx_of_id(p.src_id) {
            Some(si) => combat::roll_source_crit(&mut state.rng, &state.grid.entities[si], p.dmg),
            None => (p.dmg, false),
        };
        let dmg = Damage {
            atk_level: 1, amount,
            element: Element::Light,
            kind: DamageType::Magic,
            is_ranged: false,
            is_crit,
            is_aoe: true,
            mastery_bp: 0,
            shadow_bane_bp: 0, debuff_amp_bp: 0,
            atk_race: crate::grid::Race::Unknown,
            atk_size: crate::grid::Size::Medium,
            ignore_def_bp: 0,
        };
        let mob = combat::attackers_on(&state.grid.entities, vidx);
        let pre_hp = state.grid.entities[vidx].hp.max(0);
        let dealt = combat::apply_damage(&state.grid.entities[vidx], &dmg, mob);
        let was_alive = state.grid.entities[vidx].alive;
        state.grid.entities[vidx].take_hit(dealt);
        state.recent_hits.push(crate::state::HitEvent {
            target_id: state.grid.entities[vidx].id,
            attacker_id: p.src_id,
            amount: dealt.min(pre_hp),
            hits: 1,
            kind: if is_crit { crate::state::HitKind::Crit } else { crate::state::HitKind::Normal },
        });
        if let Some(oi) = owner {
            state.record_damage(oi, Some("lumen_contagio"), dealt.min(pre_hp));
        }
        if was_alive && !state.grid.entities[vidx].alive {
            kill_monster(state, vidx, owner); // marked corpse re-detonates (scheduled)
        } else if state.grid.entities[vidx].alive
            && crate::skills::buff_sum(&state.grid.entities[vidx], crate::skills::BuffKind::LumenVuln) == 0
        {
            // CONTAGION: mark an un-marked survivor (300·level bp holy-vuln, 10 s).
            crate::skills::apply_buff(
                &mut state.grid.entities[vidx],
                crate::skills::Buff {
                    kind: crate::skills::BuffKind::LumenVuln,
                    magnitude: 300 * p.lumen_level as i32,
                    ticks: 100,
                    source_id: p.src_id,
                },
            );
        }
    }
}

fn maybe_poison_explosion(state: &mut GameState, k: usize, victim_idx: usize) {
    use crate::combat::{self, Damage};
    use crate::grid::{DamageType, Element};
    use crate::skills::BuffKind;

    // Killer must own Poison Explosion, learned + off cooldown.
    let Some((slot, level)) = state.grid.entities[k]
        .skills
        .iter()
        .enumerate()
        .find(|(_, s)| s.id == "poison_explosion" && s.level > 0 && s.cooldown == 0)
        .map(|(slot, s)| (slot, s.level))
    else {
        return;
    };
    // Victim must carry a POISON (classic or Deadly) — Bleed does NOT count.
    let v = &state.grid.entities[victim_idx];
    if crate::skills::buff_sum(v, BuffKind::Poison) == 0
        && crate::skills::buff_sum(v, BuffKind::PoisonDot) == 0
    {
        return;
    }
    let Some(def) = crate::skills::skill_def("poison_explosion") else { return; };
    // Latch the cooldown FIRST so a kill from this blast can't re-trigger it.
    state.grid.entities[k].skills[slot].cooldown = def.cooldown_ticks;

    let center = state.grid.entities[victim_idx].pos;
    let radius = def.aoe_radius;
    let me_team = state.grid.entities[k].team;
    let victims = super::victims_in_radius(&state.grid.entities, center, radius, Some(me_team));
    if victims.is_empty() {
        return;
    }
    // Surface the blast for the renderer's burst FX (display-only, serde-skip).
    state.poison_bursts.push(crate::state::BurstFx {
        x: center.x,
        y: center.y,
        radius: radius as i32,
        element: crate::grid::Element::Poison,
    });
    // TOTAL power split equally among the caught enemies (1 = full, N = ÷N).
    let per_bp = def.power_at(level) / victims.len() as u32;
    let amount = (combat::effective_atk(&state.grid.entities[k]) as i64 * per_bp as i64 / 10_000) as i32;
    for vidx in victims {
        if !state.grid.entities[vidx].alive {
            continue;
        }
        let dmg = Damage {
            atk_level: 1, amount,
            element: Element::Poison,
            kind: DamageType::Physical,
            is_ranged: false,
            is_crit: false,
            is_aoe: true,
            mastery_bp: 0, shadow_bane_bp: 0, debuff_amp_bp: 0,
            atk_race: crate::grid::Race::Unknown,
            atk_size: crate::grid::Size::Medium,
            ignore_def_bp: 0,
        };
        let mob = combat::attackers_on(&state.grid.entities, vidx);
        let pre_hp = state.grid.entities[vidx].hp.max(0);
        let dealt = combat::apply_damage(&state.grid.entities[vidx], &dmg, mob);
        let was_alive = state.grid.entities[vidx].alive;
        state.grid.entities[vidx].take_hit(dealt); // hp − dmg, shatter Freeze, flag death
        state.record_damage(k, Some("poison_explosion"), dealt.min(pre_hp));
        if was_alive && !state.grid.entities[vidx].alive {
            kill_monster(state, vidx, Some(k)); // cd latched above → no chain
        }
    }
}

// --- Level / XP tuning ---
//
// Hardcoded while tuning — moves to game_data.json (`ProgressionDef`) once
// the curve is locked. The XP curve is QUADRATIC so the per-level cost grows
// faster than the per-zone-band EXP reward (×1..×6), making each new band take
// progressively longer wall-clock — the idle pacing target is ~24h → Lv30.
// `EXP_BASE` is calibrated via the sim-cli headless run. Target re-set to
// ~Lv50 @ 3 days (2.59M ticks) with the auto-retreat farming model: at 185 the
// sim hit Lv56@72h (too fast), so raised ~×1.35 to stretch the curve toward
// Lv50@72h (cumulative XP ∝ L³ ⇒ Lv56→Lv50 ≈ ×1.4, minus the slower-income feedback).
// FIXED PER-LEVEL TABLE (user 2026-07-07: "ตารางเลเวลให้ใช้ fixed value แทนสูตร").
// `XP_TABLE[L]` = XP to advance FROM level L TO L+1. Seeded from the previous blended
// curve (70·L² + 10·L³) so the pacing is byte-identical to the formula era, but each
// level is now an INDEPENDENT designer knob — tune any single level without shifting
// the rest (the whole point of a table over a formula). Index 0 is unused (levels
// start at 1); the table runs to Lv98→99 (the eventual LEVEL_CAP ceiling). Any level
// at/beyond the table length falls back to the old formula (never reached under a
// ≤99 cap, since the cap halts XP before then).
#[rustfmt::skip]
const XP_TABLE: [u64; 99] = [
    0, 80, 360, 900, 1760, 3000,
    4680, 6860, 9600, 12960, 17000, 21780,
    27360, 33800, 41160, 49500, 58880, 69360,
    81000, 93860, 108000, 123480, 140360, 158700,
    178560, 200000, 223080, 247860, 274400, 302760,
    333000, 365180, 399360, 435600, 473960, 514500,
    557280, 602360, 649800, 699660, 752000, 806880,
    864360, 924500, 987360, 1053000, 1121480, 1192860,
    1267200, 1344560, 1425000, 1508580, 1595360, 1685400,
    1778760, 1875500, 1975680, 2079360, 2186600, 2297460,
    2412000, 2530280, 2652360, 2778300, 2908160, 3042000,
    3179880, 3321860, 3468000, 3618360, 3773000, 3931980,
    4095360, 4263200, 4435560, 4612500, 4794080, 4980360,
    5171400, 5367260, 5568000, 5773680, 5984360, 6200100,
    6420960, 6647000, 6878280, 7114860, 7356800, 7604160,
    7857000, 8115380, 8379360, 8649000, 8924360, 9205500,
    9492480, 9785360, 10084200,
];

/// Band number parsed from a `"zone_b{N}_{R}"` id (1 if absent / unparseable —
/// e.g. hand-built test zones with an empty start id).
fn band_of(zone_id: &str) -> u32 {
    zone_id
        .split("_b")
        .nth(1)
        .and_then(|rest| rest.split('_').next())
        .and_then(|n| n.parse::<u32>().ok())
        .unwrap_or(1)
        .max(1)
}


/// Base CHARACTER LEVEL soft cap. Launch caps the level at 70 (user 2026-07-07:
/// "level cap 70, band ไม่ cap" — ONLY the level is capped; every band stays open,
/// so a Lv70 player can still push into high bands, just can't level past the cap).
/// Raised per expansion patch (80 → 90 → 99). At the cap, base XP is parked at 0 so
/// a capped character stops accruing — the same "halt" the job cap uses.
pub const LEVEL_CAP: u32 = 70;

/// Tier-1 stat points granted per base level-up (docs/apex-from-level-1-design.md D3). Flat: the
/// per-bucket cap paces the build, and a flat grant is exactly reversible for respec/XP-refund.
pub const POINTS_PER_LEVEL: u32 = 1;

/// APEX level cap (0.0.228, user: "ใช้แค่ 1-50 apex ก่อน, 51+ เป็นอนาคต"). Post-cap
/// XP earns Apex Points up to this; a future tier unlocks 51+. Calibrated with
/// `apex_xp_cost` so a 24/7 idler reaches ~50 around the 90-day patch window. At the
/// apex cap, XP parks at 0 (same halt as the level cap).
pub const APEX_LEVEL_CAP: u32 = 50;

/// Scales every `XP_TABLE` row: ×2 halves leveling speed (target: 24 h/day reaches stage ~170 in
/// ~60 days).
pub const XP_CURVE_MULT: u64 = 2;

/// XP required to advance FROM `level` TO `level + 1`. FIXED TABLE lookup
/// (`XP_TABLE`) — each level is an independent designer value. Out-of-table
/// levels (≥ 99) fall back to the seed formula (70·L² + 10·L³); a ≤99 cap
/// never reaches them.
pub fn xp_to_next(level: u32) -> u64 {
    match XP_TABLE.get(level as usize) {
        Some(&v) => v * XP_CURVE_MULT,
        None => {
            let l = level as u64;
            70 * l * l + 10 * l * l * l
        }
    }
}

/// Loop because a giant single XP gain (e.g. boss kill) can push the
/// player up multiple levels in one tick. Each iteration:
///   - reads the threshold for the CURRENT level
///   - if xp covers it, deducts and bumps level (the overflow carries
///     into the next bar — no XP waste)
///   - awards `stat_points_for_level(leaving_level)` to the unspent pool
/// Drain the EXISTING `stats.xp` bar into whole levels while `level < level_cap`, then, once at
/// `LEVEL_CAP`, into Apex points while `apex_level < apex_cap`. PURE on `stats`: no `Measures`, no
/// high-water marks, and it never discards — a remainder that cannot afford the next step stays in
/// the bar. Two callers share it so the curve cannot drift between them: combat (`try_level_up`,
/// caps = the game's) and the XP-refund apply path (caps = the account's proven ceiling).
/// Returns how many BASE levels were gained, for the caller that counts them.
pub(crate) fn drain_base_xp(stats: &mut crate::stats::Stats, level_cap: u32, apex_cap: u32) -> u32 {
    let mut gained = 0;
    loop {
        if stats.level >= LEVEL_CAP {
            while stats.apex_level < apex_cap {
                let cost = apex_xp_cost(stats.apex_level);
                if stats.xp < cost {
                    break;
                }
                stats.xp -= cost;
                stats.apex_level += 1;
            }
            break;
        }
        if stats.level >= level_cap {
            break;
        }
        let needed = xp_to_next(stats.level);
        if stats.xp < needed {
            break;
        }
        stats.xp -= needed;
        stats.level += 1;
        // The six attributes no longer grow (0.0.86); what a level BUYS is a Tier-1 stat point
        // (0.0.347), spent into the core Apex buckets. Flat one per level, because the per-bucket
        // cap does the pacing and a flat grant reverses exactly — which is what a free respec and
        // an XP refund both need. At LEVEL_CAP 70 that is 69 points against the 120 it would take
        // to max all six core buckets, so the allocation is a real choice rather than a formality.
        stats.level_points = stats.level_points.saturating_add(POINTS_PER_LEVEL);
        gained += 1;
    }
    gained
}

/// Cumulative BASE XP a reconciled `stats` represents: every level climbed, every Apex point
/// bought, plus the partial bar — one stream, integer-exact. This is what an XP refund hands back.
/// Only meaningful for a RECONCILED state (bar below its next requirement), which combat
/// guarantees after every kill.
pub fn cum_base(stats: &crate::stats::Stats) -> u64 {
    let mut total = stats.xp;
    for l in 1..stats.level {
        total = total.saturating_add(xp_to_next(l));
    }
    for a in 0..stats.apex_level {
        total = total.saturating_add(apex_xp_cost(a));
    }
    total
}

/// Grant `amount` XP to a unit and drain it against a CEILING (`level_cap` / `apex_cap` — the
/// account's proven marks, not the game's caps). At the ceiling the bar is DISCARDED: a parked
/// 99% bar would become a free level the day the cap rises, which is XP smuggled past the proof.
/// Below the ceiling a partial bar is legitimate and kept. The apply path's only entry.
pub(crate) fn apply_base_xp_capped(
    stats: &mut crate::stats::Stats,
    amount: u64,
    level_cap: u32,
    apex_cap: u32,
) {
    stats.xp = stats.xp.saturating_add(amount);
    drain_base_xp(stats, level_cap, apex_cap);
    // Below LEVEL_CAP the ceiling is a base level; at LEVEL_CAP it is the Apex cap (a partial
    // Apex bar below `apex_cap` is legitimate and kept).
    let at_ceiling =
        stats.level >= level_cap && (level_cap < LEVEL_CAP || stats.apex_level >= apex_cap);
    if at_ceiling {
        stats.xp = 0;
    }
}

/// Cumulative XP at exactly `level` / `apex_level` with an empty bar — the target `cum_base` of a
/// unit poured to the ceiling.
pub fn cum_at(level: u32, apex_level: u32) -> u64 {
    let mut total = 0u64;
    for l in 1..level {
        total = total.saturating_add(xp_to_next(l));
    }
    for a in 0..apex_level {
        total = total.saturating_add(apex_xp_cost(a));
    }
    total
}

pub(crate) fn try_level_up(state: &mut GameState, idx: usize) {
    // At the LEVEL_CAP, XP no longer parks — it drains into APEX (0.0.227): the same `xp` bar
    // fills toward the next Apex Point at a scaling cost. When an expansion raises the cap the
    // loop resumes real levelling and the earned Apex is kept. Only players who reach the cap
    // ever take that branch, so every golden (the party never hits LEVEL_CAP) is byte-identical.
    let stm = &mut state.grid.entities[idx].stats;
    let gained = drain_base_xp(stm, LEVEL_CAP, APEX_LEVEL_CAP);
    // At the Apex cap the bar must be EMPTY, not parked: a 99% bar would become a free point the
    // day the cap rises. Combat discards it here; the drain itself never does.
    if stm.level >= LEVEL_CAP && stm.apex_level >= APEX_LEVEL_CAP {
        stm.xp = 0;
    }
    // The account's proven ceiling, raised ONLY here — never by a refund or an apply, so pooling
    // many units' XP into one can never fabricate a unit past what the account has actually done.
    state.level_reached = state.level_reached.max(stm.level);
    state.apex_reached = state.apex_reached.max(stm.apex_level);
    for _ in 0..gained {
        state.measures.levels_gained.add(1);
    }
}

/// APEX XP TABLE — cost of the NEXT Apex Point, indexed by the `apex_level` you are
/// leaving (0.0.228 tune). A FIXED per-point designer table like `XP_TABLE` for
/// levels (consistency + hand-tunable per entry). Seeded from a linear 4M + 0.85M×n
/// ramp calibrated to the sim-measured cap income (~15.4M XP/day for a 24/7 idler):
/// early points quick (dopamine), later a chase; cumulative to 50 ≈ 81 days at cap,
/// so a hardcore player reaches ~50 (`APEX_LEVEL_CAP`) right at the 90-day patch
/// window. Length = the cap; 51+ is a future tier (the fallback below never runs at
/// launch since the earn loop stops at the cap).
const APEX_XP_TABLE: [u64; 50] = [
    4000000, 4850000, 5700000, 6550000, 7400000, 8250000,
    9100000, 9950000, 10800000, 11650000, 12500000, 13350000,
    14200000, 15050000, 15900000, 16750000, 17600000, 18450000,
    19300000, 20150000, 21000000, 21850000, 22700000, 23550000,
    24400000, 25250000, 26100000, 26950000, 27800000, 28650000,
    29500000, 30350000, 31200000, 32050000, 32900000, 33750000,
    34600000, 35450000, 36300000, 37150000, 38000000, 38850000,
    39700000, 40550000, 41400000, 42250000, 43100000, 43950000,
    44800000, 45650000,
];
pub fn apex_xp_cost(apex_level: u32) -> u64 {
    match APEX_XP_TABLE.get(apex_level as usize) {
        Some(&v) => v,
        None => 4_000_000 + 850_000 * apex_level as u64, // 51+ future: extend the ramp
    }
}


#[cfg(test)]
mod drain_tests {
    use super::*;
    use crate::stats::Stats;

    #[test]
    fn cum_base_inverts_drain_base_xp_exactly() {
        // The refund's no-mint spine: pour N XP into a fresh unit, drain it into levels/Apex, and
        // the cumulative reading must give N back — every XP accounted for, none created. Checked
        // below the cap, across the cap into Apex, and at the top of Apex (bar left in place).
        for &n in &[0u64, 999, 1_000, 123_456, 5_000_000, 400_000_000, 2_000_000_000] {
            let mut s = Stats::new();
            s.xp = n;
            let gained = drain_base_xp(&mut s, LEVEL_CAP, APEX_LEVEL_CAP);
            assert_eq!(cum_base(&s), n, "n={n} level={} apex={} bar={}", s.level, s.apex_level, s.xp);
            assert_eq!(s.level_points, gained, "one point per level gained");
            assert!(s.level <= LEVEL_CAP && s.apex_level <= APEX_LEVEL_CAP);
            if s.level < LEVEL_CAP {
                assert!(s.xp < xp_to_next(s.level), "reconciled: bar below the next level");
            }
        }
    }

    #[test]
    fn drain_respects_a_lower_ceiling_and_parks_the_rest() {
        // The apply path drains against the ACCOUNT's ceiling, not the game's: a unit may not be
        // poured past `level_reached`. The remainder stays in the bar for the caller to decide.
        let mut s = Stats::new();
        s.xp = 50_000_000;
        drain_base_xp(&mut s, 10, 0);
        assert_eq!(s.level, 10, "stops at the ceiling");
        assert!(s.xp > 0, "the drain never discards; the caller does");
    }
}

#[cfg(test)]
mod steal_tests {
    use super::*;
    use crate::grid::{Cell, Entity, Team};
    use crate::items::{DropEntry, Item, ItemKind, Rarity};
    use crate::skills::SkillState;
    use crate::state::GameState;

    // A dead monster carrying a guaranteed (single-entry, non-NOTHING) drop.
    /// Loot banked + gear tickets still awaiting the server's roll. Since 0.0.257 a weapon
    /// drop lands as a pending ticket rather than in the bag, so "did a drop happen?" has to
    /// count both — otherwise these tests measure the routing, not the drop.
    fn loot_produced(s: &GameState) -> usize {
        s.inventory.len() + crate::drops::gear_candidate_count(s)
    }

    fn mob_with_loot(s: &mut GameState) -> usize {
        let mut mob = Entity::idle(1000, Cell::new(1, 0)).with_team(Team::Monster);
        mob.drops = vec![DropEntry {
            template: Item::new("steal_loot", ItemKind::Weapon, 5, 0, Rarity::Common),
            weight: 1,
            server_bp: 0, overrides: Vec::new(), lines_by_grade: Vec::new() }];
        mob.alive = false;
        s.grid.spawn(mob);
        s.grid.entities.len() - 1
    }

    #[test]
    fn xp_at_cap_drains_into_apex_not_wasted() {
        let mut s = GameState::new(1, 16, 8);
        s.grid.spawn(Entity::idle(1, Cell::new(0, 0)).with_team(Team::Player));
        // SCALING cost: point 0 costs apex_xp_cost(0), point 1 costs apex_xp_cost(1).
        let c0 = apex_xp_cost(0);
        let c1 = apex_xp_cost(1);
        assert!(c1 > c0, "cost scales up per apex point");
        s.grid.entities[0].stats.level = LEVEL_CAP;
        s.grid.entities[0].stats.xp = c0 + c1 + c0 / 2; // 2 full points + a partial 3rd
        try_level_up(&mut s, 0);
        assert_eq!(s.grid.entities[0].stats.level, LEVEL_CAP, "cap not exceeded");
        assert_eq!(s.grid.entities[0].stats.apex_level, 2, "2 apex points earned");
        assert_eq!(s.grid.entities[0].stats.xp, c0 / 2, "remainder carries in the bar");

        // A later cap raise: the entity resumes real leveling, apex retained.
        s.grid.entities[0].stats.level = LEVEL_CAP - 1;
        s.grid.entities[0].stats.xp = xp_to_next(LEVEL_CAP - 1);
        try_level_up(&mut s, 0);
        assert_eq!(s.grid.entities[0].stats.level, LEVEL_CAP, "resumed leveling into the cap");
        assert_eq!(s.grid.entities[0].stats.apex_level, 2, "apex kept through the raise");
    }

    #[test]
    fn apex_level_halts_at_the_cap() {
        let mut s = GameState::new(1, 16, 8);
        s.grid.spawn(Entity::idle(1, Cell::new(0, 0)).with_team(Team::Player));
        s.grid.entities[0].stats.level = LEVEL_CAP;
        s.grid.entities[0].stats.apex_level = APEX_LEVEL_CAP - 1;
        // A huge XP dump can't push apex past the cap; excess XP is parked at 0.
        s.grid.entities[0].stats.xp = u64::MAX / 2;
        try_level_up(&mut s, 0);
        assert_eq!(s.grid.entities[0].stats.apex_level, APEX_LEVEL_CAP, "halts at the apex cap");
        assert_eq!(s.grid.entities[0].stats.xp, 0, "excess XP parked at the apex cap");
    }

    /// Offline EXP: the band's base XP × the party share × 528 an hour, nothing under 10 minutes,
    /// nothing past 10 hours; the envelope pays every fielded member through the level-up path.
    #[test]
    fn offline_xp_follows_the_band_the_party_and_the_cap() {
        let d = crate::data::GameData::from_json_str(include_str!("../../../../../data/game_data.json")).unwrap();
        let mut s = crate::data::new_game_in_zone(1, &d, "zone_b1_1").unwrap();
        s.band_reached = 20;
        let base = d.stage_base.iter().find(|r| r.stage == 20).unwrap().xp as u64;
        assert_eq!(offline_xp_per_member(&s, &d, 60), base * 40 * 528 / 100, "one hour, the five-member share even solo");
        assert_eq!(offline_xp_per_member(&s, &d, 9), 0, "a reconnect is not time away");
        assert_eq!(offline_xp_per_member(&s, &d, 24 * 60), offline_xp_per_member(&s, &d, 600), "capped at 10 h");
        let lumi = crate::data::grant_exemplar_unit(&mut s, &d, "lumi").unwrap();
        s.party_slots = 2;
        crate::data::deploy(&mut s, &d, lumi).unwrap();
        assert_eq!(offline_xp_per_member(&s, &d, 60), base * 40 * 528 / 100, "two members: the same share");

        let before: Vec<u64> = s.grid.entities.iter().filter(|e| e.team == Team::Player).map(|e| crate::tick::cum_base(&e.stats)).collect();
        let mut env = crate::data::instantiate_item(d.item("offline_exp").unwrap());
        env.quantity = 5_000;
        let bag = s.inventory.len();
        assert!(crate::data::try_grant_item(&mut s, &d, env));
        let after: Vec<u64> = s.grid.entities.iter().filter(|e| e.team == Team::Player).map(|e| crate::tick::cum_base(&e.stats)).collect();
        assert!(before.iter().zip(&after).all(|(b, a)| a - b == 5_000), "{before:?} -> {after:?}");
        assert_eq!(s.inventory.len(), bag, "never in the bag");
    }

    /// EXP Elixir: +20% on the kill, read once from the party's highest holder — two members
    /// carrying it (the rack buffs everyone) must not make it +40%.
    #[test]
    fn elixir_adds_its_bp_once_for_the_party() {
        use crate::skills::{Buff, BuffKind};
        let earned = |elixir: bool| {
            let mut s = GameState::new(1, 16, 8);
            for id in [1, 2] {
                let mut p = Entity::idle(id, Cell::new(0, id as i32)).with_team(Team::Player);
                if elixir {
                    p.buffs.push(Buff { kind: BuffKind::XpPct, magnitude: 2000, ticks: 100, source_id: 9 });
                }
                s.grid.spawn(p);
            }
            let mut mob = Entity::idle(1000, Cell::new(1, 0)).with_team(Team::Monster);
            mob.xp_reward = 1000;
            mob.alive = false;
            s.grid.spawn(mob);
            handle_monster_death(&mut s, 2, Some(0));
            s.run_tally.xp
        };
        assert_eq!(earned(false), 700);
        assert_eq!(earned(true), 840);
        // Three elixir buffs on different sources (a rack cycled three times) are still +20%.
        let mut s = GameState::new(1, 16, 8);
        let mut p = Entity::idle(1, Cell::new(0, 1)).with_team(Team::Player);
        for src in 9..12 {
            p.buffs.push(Buff { kind: BuffKind::XpPct, magnitude: 2000, ticks: 100, source_id: src });
        }
        s.grid.spawn(p);
        let mut mob = Entity::idle(1000, Cell::new(1, 0)).with_team(Team::Monster);
        mob.xp_reward = 1000;
        mob.alive = false;
        s.grid.spawn(mob);
        handle_monster_death(&mut s, 1, Some(0));
        assert_eq!(s.run_tally.xp, 1200, "never a sum of elixirs");
    }

    #[test]
    fn steal_banks_an_extra_drop_on_a_thief_kill() {
        let mut s = GameState::new(1, 16, 8);
        let mut thief = Entity::idle(1, Cell::new(0, 0)).with_team(Team::Player);
        thief.skills = vec![SkillState::new("steal", 10)]; // Lv10 => 100% proc
        s.grid.spawn(thief);
        let mob = mob_with_loot(&mut s);
        let before = loot_produced(&s);
        handle_monster_death(&mut s, mob, Some(0));
        assert!(loot_produced(&s) > before, "Steal banks an extra drop on kill");
    }

    #[test]
    fn no_steal_means_no_extra_drop() {
        let mut s = GameState::new(1, 16, 8);
        s.grid.spawn(Entity::idle(1, Cell::new(0, 0)).with_team(Team::Player)); // no Steal
        let mob = mob_with_loot(&mut s);
        let before = loot_produced(&s);
        handle_monster_death(&mut s, mob, Some(0));
        assert_eq!(loot_produced(&s), before, "no Steal => no extra drop (handle_monster_death alone)");
    }


}
