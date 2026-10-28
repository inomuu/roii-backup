//! `tick()` — the only way time advances (GDD §4.2 rule 6).
//!
//! Per-tick work (ALPHA-3 scope):
//!   1. Increment `state.tick`.
//!   2. For each entity in spawn order (NOT iter over HashMap):
//!        a. Skip if dead.
//!        b. Pick a target (nearest living enemy). If found, snap
//!           `target` cell to that enemy's position.
//!        c. If a target exists AND it's within `atk_range` →
//!             accumulate `atk_acc`, spend 1000 per attack swing.
//!           Else (no target, or out of range) →
//!             accumulate `move_acc`, spend 1000 per step.
//!        d. If the entity attacked, the target may now be dead;
//!           that's fine — next tick's target select drops it.
//!
//! Why "either move OR attack, never both same tick"?
//!   - Keeps the rule simple: one accumulator advances per tick.
//!   - Prevents a fast unit from "step + swing" each tick which would
//!     out-DPS a stationary unit just by being mobile — that's a
//!     balance lever we want to design later, not get for free.
//!   - When a unit transitions in-range → out-of-range, its other
//!     accumulator starts cleanly from where it left off (we don't
//!     reset). So no "save up energy by running circles" exploit.

// DEBT (70 sites): this file predates `deny(clippy::indexing_slicing)`. An index that
// cannot be proven in range is a panic, and in verify-server (which links this crate and
// replays saves under `panic = "abort"`) a panic is the whole process. Shrink this, never
// add to it — a NEW file has no allow and is rejected on its first bare index.
#![allow(clippy::indexing_slicing)]

use crate::combat;
use crate::drops;
use crate::grid::{Cell, Team};
use crate::spawn;
use crate::state::{Projectile, GameState};

/// Ticks at the start of a freshly-opened zone during which NO entity acts — the deployment grace.
/// Combat and movement are frozen while the party walks into position; kept in step with the
/// renderer's camera pan + march (arena3d_scene.gd CAM_TRAVEL_SEC / GRACE_SEC). 31 ticks = 3.1 s at
/// 10 ticks/s — the party's march lands inside this; the camera keeps its tuned 3.4 s pan and
/// finishes settling just as the fight resumes. Rushing the pan to fit a shorter grace read as
/// "ไวแปลกๆ" (user 2026-09-02), so the grace is sized to the travel instead. Was 20, then 36.
/// 110 = 11.0 s. Was 20 (2.0 s), which was sized for a band whose rooms sat one field width apart:
/// the party's march between them took about a second and landed well inside it.
///
/// Region 1's authored rooms are 52 m apart, and a party crossing that inside 2 s is moving at 26
/// cells per second — a slide, not a walk. The renderer caps the march to this window (combat must
/// not resume mid-stride; that was rejected on 2026-09-03), so the window is what decides whether
/// the crossing looks like walking. 6 s holds a 5.2 s march at 10 cells/s with a settle beat left
/// over. 6 s held a 5.2 s march at 10 cells/s; 10 read as too fast on sight (user 2026-09-09) and
/// the speed came down to 5, which is 10.4 s across the longest room. `ArenaView.GraceSeconds`
/// mirrors this number and `MarchSpeedCps` is the speed — the cost is real and worth naming: this
/// is 11 s of nobody acting on every room entry, three times a band. It is the price of the MARCH
/// and nothing else, which is why an open with no march to cover gets `DEPLOY_GRACE_TICKS_OPEN`
/// instead — the zone records which one it opened with (`ZoneSpawnState::deploy_grace_ticks`).
pub(crate) const DEPLOY_GRACE_TICKS_MARCH: u16 = 110;

/// THE MARCH, SIZED TO THE PARTY (0.0.398, user: "delay ตอนเดินถึงตำแหน่งต่อสู้ลดลง <0.5 วิ หรือเริ่มทันที
/// + เดินข้ามห้องไวขึ้นตาม move speed"). The grace is exactly the walk plus a `MARCH_SETTLE_TICKS`
/// beat: `MARCH_CELLS` at `MARCH_BASE_CPS`, divided by the slowest member's move-speed bonus. The
/// renderer reads the number back (`zone_spawn.deploy_grace_ticks`) and paces the march to land
/// `MARCH_SETTLE_TICKS` before combat resumes, so a Move line shortens the crossing on screen and
/// in the sim by the same amount. Rounds UP so the party never arrives after the fight starts.
pub(crate) const MARCH_CELLS: u32 = 52;
pub(crate) const MARCH_BASE_CPS: u32 = 5;
pub(crate) const MARCH_SETTLE_TICKS: u16 = 3;
pub(crate) fn march_grace_ticks(move_speed_bp: i32) -> u16 {
    let bp = (10_000 + i64::from(move_speed_bp)).max(2_500) as u64; // a slow can at most quadruple it
    let walk = (u64::from(MARCH_CELLS) * 10 * 10_000).div_ceil(u64::from(MARCH_BASE_CPS) * bp);
    (walk as u16).saturating_add(MARCH_SETTLE_TICKS).max(MARCH_SETTLE_TICKS + 1)
}

/// The grace for an open with NO march to cover: the first room, a band swap behind the fade, and
/// a retry. The party is placed where it stands and walks in from just off the left edge — about a
/// second — so holding the fight for eleven leaves ten seconds of nobody doing anything, which is
/// exactly how it played (user 2026-09-09: "ตอนตายเริ่มใหม่ ... ยืนรอเกือบสิบวิ").
///
/// 1.5 s (0.0.398): the entrance walks in from six cells off the edge at the march pace, 1.2 s,
/// plus the same settle beat the march gets — the user asked for under half a second between
/// arriving and the first swing.
pub(crate) const DEPLOY_GRACE_TICKS_OPEN: u16 = 15;

/// The combat + movement half of a tick (skipped during the deployment grace, see
/// DEPLOY_GRACE_TICKS): every living non-pet entity targets/attacks or steps, then pets, traps,
/// arrived projectiles, absorb bursts and pending explosions resolve — the order these must run in.
fn resolve_actions(state: &mut GameState) {
    let count = state.grid.entities.len();
    for i in 0..count {
        if !state.grid.entities[i].alive || state.grid.entities[i].is_pet {
            continue; // pets are driven by `pet_step`, not the player/monster loop
        }
        tick_one_entity(state, i);
    }
    // Ranger pets: spawn (per Pet Master owner) + move + attack.
    pet_step(state);
    // Ranger traps: fuse Claymores + fire any trap an enemy entered THIS tick (after every entity
    // + pet has moved, so triggers see final positions).
    tick_traps(state);
    // Projectiles fired on earlier ticks land at the END of the tick their `arrive_at_tick`
    // matches — after every entity has moved this tick. Order: spawn order in `state.projectiles`,
    // which is also fire order (attack_step pushes onto the back).
    resolve_arrived_projectiles(state);
    // Flame Shield: any absorb pool that emptied from this tick's hits explodes.
    resolve_absorb_bursts(state);
    // Bomb monsters (ChomBombs): fire any scheduled explosion now due — the blast is delayed a few
    // ticks past death so it lands in sync with the explosion anim.
    fire_pending_explosions(state);
    // Sylvie's crit riders read every crit this tick has recorded, whatever path dealt it.
    crit_riders(state);
    // Zolfia: her aura ticks, then every hit and break of the tick is read for Winter Chill.
    winter_aura(state);
    winter_pass(state);
}

/// Zolfia's passive (0.0.476): `(chill ticks, shatter bp, shatter extra ticks)`, or None.
fn winter_passive(e: &crate::grid::Entity) -> Option<(u32, i32, u32)> {
    e.skills
        .iter()
        .filter(|s| s.level > 0)
        .filter_map(|s| crate::skills::skill_def(&s.id))
        .find(|d| d.winter_chill_ticks > 0)
        .map(|d| (d.winter_chill_ticks, d.shatter_bp, d.shatter_extra_ticks))
}

/// Winter Chill and Shattered (0.0.476, Zolfia's passive). Reads the tick's hit stream, so it
/// sees every path that dealt a hit: (1) each hit a Winter owner landed chills its target;
/// (2) each BREAK on a chilled target — whoever broke it — lays Shattered for the break's stun
/// plus the passive's extra time; (3) at S4 a Shattered enemy that died hands its Shattered to
/// the nearest living enemy. Deterministic — no PRNG.
fn winter_pass(state: &mut GameState) {
    use crate::skills::{apply_buff, Buff, BuffKind};
    let events: Vec<(u32, u32, bool)> = state
        .recent_hits
        .iter()
        .chain(state.field_hits.iter())
        .filter(|h| !matches!(h.kind, crate::state::HitKind::Dodge | crate::state::HitKind::Guard))
        .map(|h| (h.attacker_id, h.target_id, matches!(h.kind, crate::state::HitKind::Break)))
        .collect();
    for &(att, tgt, _) in &events {
        let Some(ai) = state.grid.idx_of_id(att) else { continue };
        let Some((chill_ticks, _, _)) = winter_passive(&state.grid.entities[ai]) else { continue };
        let Some(ti) = state.grid.idx_of_id(tgt) else { continue };
        if state.grid.entities[ti].alive && state.grid.entities[ti].team != state.grid.entities[ai].team {
            apply_buff(&mut state.grid.entities[ti], Buff { kind: BuffKind::WinterChill, magnitude: 1, ticks: chill_ticks, source_id: att });
        }
    }
    for &(_, tgt, brk) in &events {
        if !brk {
            continue;
        }
        let Some(ti) = state.grid.idx_of_id(tgt) else { continue };
        let Some(src) = state.grid.entities[ti].buffs.iter().find(|b| b.kind == BuffKind::WinterChill).map(|b| b.source_id) else { continue };
        let Some(si) = state.grid.idx_of_id(src) else { continue };
        let Some((_, bp, extra)) = winter_passive(&state.grid.entities[si]) else { continue };
        let stun = if state.grid.entities[ti].is_boss { crate::grid::BREAK_STUN_TICKS_BOSS } else { crate::grid::BREAK_STUN_TICKS };
        apply_buff(&mut state.grid.entities[ti], Buff { kind: BuffKind::Shattered, magnitude: bp, ticks: stun + extra, source_id: src });
        // S1: the run's shatter count.
        if let Some((_, _, cap)) = crate::unit_rank::shatter_stack(&state.grid.entities[si]) {
            let z = &mut state.grid.entities[si];
            z.shatter_stacks = z.shatter_stacks.saturating_add(1).min(if cap > 0 { cap } else { u16::MAX });
        }
    }
    // S4: a shatter outlives its body. A corpse keeps its buffs until it is pruned later in the
    // tick; its Shattered is taken off either way so it is handed on once.
    for k in 0..state.grid.entities.len() {
        if state.grid.entities[k].alive {
            continue;
        }
        let Some(pos) = state.grid.entities[k].buffs.iter().position(|b| b.kind == BuffKind::Shattered) else { continue };
        let b = state.grid.entities[k].buffs[pos].clone();
        state.grid.entities[k].buffs.retain(|x| x.kind != BuffKind::Shattered);
        let Some(si) = state.grid.idx_of_id(b.source_id) else { continue };
        if !crate::unit_rank::shatter_jump(&state.grid.entities[si]) {
            continue;
        }
        let from = state.grid.entities[k].pos;
        let my_team = state.grid.entities[si].team;
        let next = state
            .grid
            .entities
            .iter()
            .enumerate()
            .filter(|(_, e)| e.alive && e.team != my_team && e.on_board())
            .min_by_key(|(j, e)| (e.pos.chebyshev(from), *j))
            .map(|(j, _)| j);
        if let Some(j) = next {
            apply_buff(&mut state.grid.entities[j], b);
        }
    }
}

/// Remorseless Winter (0.0.476, Zolfia's aura): once a second while `WinterAura` lasts, every
/// enemy within the aura is slowed and takes an Ice DoT of her ATK. At S6
/// a Shattered enemy inside keeps its Shattered until it dies. DoT ticks never crit and never
/// chip Toughness (the engine's DoT rule). Deterministic — no PRNG.
fn winter_aura(state: &mut GameState) {
    use crate::skills::{apply_buff, buff_sum, Buff, BuffKind};
    if state.tick % 10 != 0 {
        return;
    }
    for i in 0..state.grid.entities.len() {
        if !state.grid.entities[i].alive || buff_sum(&state.grid.entities[i], BuffKind::WinterAura) <= 0 {
            continue;
        }
        let Some((def, level)) = state.grid.entities[i]
            .skills
            .iter()
            .filter(|s| s.level > 0)
            .find_map(|s| crate::skills::skill_def(&s.id).filter(|d| d.aura_radius > 0).map(|d| (d, s.level)))
        else {
            continue;
        };
        let lv = i64::from(level.saturating_add(crate::unit_rank::level_bonus(&state.grid.entities[i], def.id)).max(1)) - 1;
        let hold = crate::unit_rank::aura_holds_shatter(&state.grid.entities[i]);
        let radius = def.aura_radius;
        let slow = def.aura_aspd_slow_bp + def.aura_aspd_slow_per_level_bp * lv as i32;
        let dot_bp = i64::from(def.aura_dot_atk_bp) + i64::from(def.aura_dot_per_level_bp) * lv;
        let amount = ((crate::combat::effective_atk(&state.grid.entities[i]) as i64) * dot_bp / 10_000).clamp(1, i32::MAX as i64) as i32;
        let me = state.grid.entities[i].id;
        let me_team = state.grid.entities[i].team;
        let center = state.grid.entities[i].pos;
        let victims: Vec<usize> = state
            .grid
            .entities
            .iter()
            .enumerate()
            .filter(|(_, e)| e.alive && e.team != me_team && e.on_board() && e.pos.chebyshev(center) <= radius)
            .map(|(j, _)| j)
            .collect();
        for j in victims {
            apply_buff(&mut state.grid.entities[j], Buff { kind: BuffKind::AspdSlowPct, magnitude: slow, ticks: 15, source_id: me });
            if hold {
                for b in state.grid.entities[j].buffs.iter_mut().filter(|b| b.kind == BuffKind::Shattered) {
                    b.ticks = b.ticks.max(15);
                }
            }
            let dmg = crate::combat::Damage {
                atk_level: 1, amount,
                element: crate::grid::Element::Ice,
                kind: crate::grid::DamageType::Magic,
                is_ranged: false,
                is_crit: false,
                is_aoe: true,
                mastery_bp: 0, shadow_bane_bp: 0, debuff_amp_bp: 0,
                atk_race: crate::grid::Race::Unknown,
                atk_size: crate::grid::Size::Medium,
                ignore_def_bp: 0,
            };
            let mob = crate::combat::attackers_on(&state.grid.entities, j);
            let pre_hp = state.grid.entities[j].hp.max(0);
            let dealt = crate::combat::apply_damage(&state.grid.entities[j], &dmg, mob);
            state.grid.entities[j].take_hit(dealt);
            state.record_damage(i, Some(def.id), dealt.min(pre_hp));
            state.field_hits.push(crate::state::HitEvent {
                target_id: state.grid.entities[j].id,
                attacker_id: me,
                amount: dealt,
                hits: 1,
                kind: crate::state::HitKind::Normal,
            });
            if !state.grid.entities[j].alive {
                kill_monster(state, j, Some(i));
            }
        }
    }
}

/// Crit riders (0.0.473, Sylvie S1/S4): every crit recorded this tick (`HitKind::Crit` in
/// `recent_hits` and `field_hits`) whose attacker carries `CritGauge` charges that attacker's
/// gauge, and one carrying `CritSpark` throws a Thunder spark at the same target. The events
/// are snapshotted first, so a spark (which does not crit) never feeds the pass it came from.
/// Deterministic — no PRNG.
fn crit_riders(state: &mut GameState) {
    use crate::skills::{buff_sum, BuffKind};
    let crits: Vec<(u32, u32)> = state
        .recent_hits
        .iter()
        .chain(state.field_hits.iter())
        .filter(|h| matches!(h.kind, crate::state::HitKind::Crit))
        .map(|h| (h.attacker_id, h.target_id))
        .collect();
    for (att, tgt) in crits {
        let Some(ai) = state.grid.idx_of_id(att) else { continue };
        if !state.grid.entities[ai].alive {
            continue;
        }
        let gauge = buff_sum(&state.grid.entities[ai], BuffKind::CritGauge);
        if gauge > 0 {
            crate::skills::charge_limit_break(&mut state.grid.entities[ai], gauge.min(u16::MAX as i32) as u16);
        }
        let spark_bp = buff_sum(&state.grid.entities[ai], BuffKind::CritSpark);
        if spark_bp <= 0 {
            continue;
        }
        let Some(ti) = state.grid.idx_of_id(tgt) else { continue };
        if !state.grid.entities[ti].alive || state.grid.entities[ti].team == state.grid.entities[ai].team {
            continue;
        }
        let amount = ((crate::combat::effective_atk(&state.grid.entities[ai]) as i64) * i64::from(spark_bp) / 10_000)
            .clamp(1, i32::MAX as i64) as i32;
        let dmg = crate::combat::Damage {
            atk_level: 1, amount,
            element: crate::grid::Element::Thunder,
            kind: crate::grid::DamageType::Magic,
            is_ranged: true,
            is_crit: false,
            is_aoe: false,
            mastery_bp: 0, shadow_bane_bp: 0, debuff_amp_bp: 0,
            atk_race: crate::grid::Race::Unknown,
            atk_size: crate::grid::Size::Medium,
            ignore_def_bp: 0,
        };
        let mob = crate::combat::attackers_on(&state.grid.entities, ti);
        let pre_hp = state.grid.entities[ti].hp.max(0);
        let dealt = crate::combat::apply_damage(&state.grid.entities[ti], &dmg, mob);
        state.grid.entities[ti].take_hit(dealt);
        state.record_damage(ai, Some("surge"), dealt.min(pre_hp));
        state.field_hits.push(crate::state::HitEvent {
            target_id: tgt,
            attacker_id: att,
            amount: dealt,
            hits: 1,
            kind: crate::state::HitKind::Normal,
        });
        if !state.grid.entities[ti].alive {
            kill_monster(state, ti, Some(ai));
        }
    }
}

/// Thief Hiding: a living player carrying the `hiding` skill auto-hides when its
/// HP drops below 30% of max — ONCE per SESSION (`hiding_used`, reset when a new
/// instance run begins at room 1; see `change_zone`). Applies the Hiding marker
/// (10 s: untargetable + no actions) plus a
/// HealHot restoring `(10 + 10*level)%` of max HP over those 10 s (Lv1 20% ..
/// Lv6 70%), then it surfaces. Threshold-driven — no PRNG draw, so skill-less
/// streams stay byte-identical.
fn maybe_enter_hiding(state: &mut GameState, i: usize) {
    let e = &state.grid.entities[i];
    if !matches!(e.team, Team::Player) || !e.alive || e.hiding_used {
        return;
    }
    if crate::skills::buff_sum(e, crate::skills::BuffKind::Hiding) > 0 {
        return; // already hidden
    }
    let lvl = e.skill_level("hiding");
    if lvl == 0 {
        return;
    }
    let maxhp = combat::effective_max_hp(e);
    if maxhp <= 0 || (e.hp as i64) * 100 >= (maxhp as i64) * 30 {
        return; // HP >= 30% → don't hide
    }
    let heal_pct = 10 + 10 * lvl as i64; // Lv1=20 .. Lv6=70
    let per_tick = ((maxhp as i64) * heal_pct / 100 / 10).max(1) as i32; // 10 HoT ticks / 10s
    let id = e.id;
    crate::skills::apply_buff(
        &mut state.grid.entities[i],
        crate::skills::Buff { kind: crate::skills::BuffKind::Hiding, magnitude: 1, ticks: 100, source_id: id },
    );
    crate::skills::apply_buff(
        &mut state.grid.entities[i],
        crate::skills::Buff { kind: crate::skills::BuffKind::HealHot, magnitude: per_tick, ticks: 100, source_id: id },
    );
    state.grid.entities[i].hiding_used = true;
}

pub(crate) mod combat_step;
mod movement;
pub(crate) mod upkeep;

pub(crate) use combat_step::*;
pub(crate) use movement::*;
pub(crate) use upkeep::*;

// Public progression curves — the single source of truth for the HUD's EXP
// bars (party frames + Character window) so GDScript never re-derives them.
pub use combat_step::{apex_xp_cost, preview_pending_skill, preview_pending_skills, xp_to_next, LEVEL_CAP, APEX_LEVEL_CAP};
// The XP-refund primitives (0.0.349): one curve shared with combat, read by `data::` and hosts.
pub use combat_step::xp::{
    cum_at, cum_base, grant_party_xp, offline_xp_per_member, OFFLINE_KILLS_PER_HOUR, OFFLINE_MAX_MINUTES,
    OFFLINE_MIN_MINUTES,
};
pub(crate) use combat_step::xp::apply_base_xp_capped;
pub(crate) use combat_step::click_strike;
// Commander smite kills monsters from OUTSIDE the combat step, like a click-attack does.
pub(crate) use combat_step::xp::kill_monster;

/// SP (and poison DoT / buff aging) tick once every this many sim ticks
/// (10 = 1s), so resources refill in per-second steps, not every tick.
pub(crate) const REGEN_INTERVAL_TICKS: u64 = 10;
/// Passive HP regen (HP Recovery skill) ticks on a SLOWER cadence (50 = 5s)
/// so the HP bar heals in a few bigger, less jittery bounces instead of a
/// twitch every second. The per-tick amount is scaled up by the ratio
/// (`HP_REGEN_INTERVAL_TICKS / REGEN_INTERVAL_TICKS`) so the average HP/s is
/// unchanged — only the cadence is coarser. RO-style interval regen.
pub(crate) const HP_REGEN_INTERVAL_TICKS: u64 = 50;
/// The regen interval while the party marches between rooms (0.0.398) — one second.
pub(crate) const HP_REGEN_MARCH_INTERVAL_TICKS: u64 = 10;

/// Ember Cascade's trigger: every Toughness break this tick becomes one queued
/// Fireball for every living ally who knows the passive.
///
/// Reading the break out of the display hit stream keeps the damage path from
/// needing to know a passive exists — combat reports what happened, and whoever
/// cares reads it afterwards. Ids only; the queue is capped, and a break that
/// arrives with the queue full is dropped rather than growing a save's tail.
/// Refill a broken Toughness gauge ACROSS ITS RECOVERY, so the bar arrives full at
/// the moment it can be broken again. The bar on screen is the break timer — there
/// is no second number to read. The stun ends FIRST and is a separate clock: the
/// target is up and fighting again while its guard is still coming back.
///
/// The step is the gauge divided by that recovery, rounded UP, so a boss's longer
/// bar refills at its own pace and does not finish early. Nothing chips a gauge
/// while it refills (`Entity::chip_toughness`), which is what keeps one break from
/// running straight into the next.
pub(crate) fn refill_toughness(state: &mut GameState) {
    for e in state.grid.entities.iter_mut() {
        if e.break_ticks == 0 {
            continue;
        }
        e.break_ticks -= 1;
        let stun = if e.is_boss {
            crate::grid::BREAK_STUN_TICKS_BOSS
        } else {
            crate::grid::BREAK_STUN_TICKS
        };
        let window = crate::grid::BREAK_RECOVERY_TICKS.max(stun);
        let step = ((e.toughness_max as u32).div_ceil(window)).max(1) as u16;
        e.toughness = e.toughness.saturating_add(step).min(e.toughness_max);
    }
}

/// The entity's BREAK TRIGGER skill and the level it is at, or `None`. First match
/// in skill order; a character carries at most one by authoring, not by rule.
pub(crate) fn break_trigger_of(
    states: &[crate::skills::SkillState],
) -> Option<(&'static crate::skills::SkillDef, u8)> {
    states.iter().filter(|s| s.level > 0).find_map(|s| {
        crate::skills::skill_def(&s.id)
            .filter(|d| d.break_trigger)
            .map(|d| (d, s.level))
    })
}

pub(crate) fn collect_break_triggers(state: &mut GameState) {
    let broken: Vec<u32> = state
        .recent_hits
        .iter()
        .filter(|h| matches!(h.kind, crate::state::HitKind::Break))
        .map(|h| h.target_id)
        .collect();
    if broken.is_empty() {
        return;
    }
    // S6 (0.0.466, Suzu): a break extends a LIVE Break Fever, hers only.
    for e in state.grid.entities.iter_mut() {
        if !e.alive {
            continue;
        }
        let extra = crate::unit_rank::fever_on_break_ticks(e);
        if extra == 0 {
            continue;
        }
        let me = e.id;
        for b in e.buffs.iter_mut() {
            if b.source_id == me && matches!(b.kind, crate::skills::BuffKind::BreakFever | crate::skills::BuffKind::AspdPct) && b.ticks > 0 {
                b.ticks += extra;
            }
        }
    }
    // A skill that RESETS on a break (0.0.465, Suzu's second blow) clears its cooldown here —
    // any enemy, any breaker, once per tick however many broke.
    for e in state.grid.entities.iter_mut() {
        if !e.alive {
            continue;
        }
        for s in e.skills.iter_mut() {
            if s.cooldown > 0 && crate::skills::skill_def(&s.id).is_some_and(|d| d.reset_on_break) {
                s.cooldown = 0;
            }
        }
    }
    // Limit Break gauges that run off breaks (Rapis) charge here too — the same
    // event, read once.
    for e in state.grid.entities.iter_mut() {
        if !e.alive {
            continue;
        }
        let per = crate::skills::lb_gain(&e.skills, |d| d.lb_on_break);
        if per > 0 {
            let gain = per.saturating_mul(broken.len().min(255) as u16);
            crate::skills::charge_limit_break(e, gain);
        }
    }
    for e in state.grid.entities.iter_mut() {
        if !e.alive || break_trigger_of(&e.skills).is_none() {
            continue;
        }
        for id in &broken {
            if e.break_queue.len() >= crate::grid::BREAK_QUEUE_CAP {
                break;
            }
            e.break_queue.push(*id);
        }
    }
}

pub fn tick(state: &mut GameState, n: u64) {
    for _ in 0..n {
        state.tick += 1;
        // Who is standing at the START of the tick. A tick-LOCAL list, deliberately not state:
        // deaths are a transition, and the only thing that can see a transition is something that
        // saw both sides. Party entities are never pruned (only monsters are), so an id missing
        // at the end means the member died rather than despawned.
        let standing: Vec<u32> = state
            .grid
            .entities
            .iter()
            .filter(|e| e.alive && matches!(e.team, crate::grid::Team::Player))
            .map(|e| e.id)
            .collect();
        state.recent_hits.clear(); // display-only; rebuilt by this tick's swings
        state.loot_fx.clear(); // display-only; rebuilt by whatever this tick banks
        state.knockbacks.clear(); // display-only; this tick's knockbacks (bounce arc)
        state.pulls.clear(); // display-only; this tick's pulls (haul-in arc)
        state.poison_bursts.clear(); // display-only; rebuilt by this tick's explosions
        state.field_hits.clear(); // display-only; rebuilt by this tick's field hits
        state.heal_fx.clear(); // display-only; rebuilt by this tick's heals
        state.meteor_impacts.clear(); // display-only; rebuilt by this tick's meteors
        state.trap_fires.clear(); // display-only; rebuilt by this tick's trap activations
        state.skill_fx.clear(); // display-only; rebuilt by this tick's skill casts
        state.ult_fx.clear(); // display-only; rebuilt by this tick's ultimates
        // The player's own resources recover on the tick, independent of any entity:
        // `regent_hand` charges come back one per cooldown.
        crate::events::recharge_pickup(state);
        crate::events::tick_commander(state);
        crate::data::open_earned_slots(state);
        // Age timed buffs / debuffs (and tick DoTs) FIRST, so 1-tick aura buffs
        // survive the same tick they're re-stamped and durations age once.
        decay_buffs(state);
        // Monsters that outlive the grace window ramp their ATK (enrage).
        upkeep::tick_enrage(state);
        // Regalia passives that hold while a condition holds (taunt guard, ATK per enemy).
        crate::regalia_passive::tick(state);
        // Persistent ground fields (Poison Cloud) apply their effect + age out.
        tick_ground_fields(state);
        upkeep::heal_exemplar_grants(state);
        upkeep::expire_rare_candidates(state);
        // Party auras (legacy ATK aura + aura buffs like Angelus) refresh
        // before anyone acts.
        apply_party_auras(state);
        // PATHS OF SOUL Power auras (party-wide ATK/DEF/MATK/HP %) refresh too.
        apply_soul_path_auras(state);
        // TRAINING lines (0.0.406) refresh the same way.
        crate::training::apply(state);
        crate::crest::apply(state);
        // Auto-use slotted consumables (RO auto-pot): heal / refresh self-buffs
        // before anyone acts. No-op for parties with empty consumable slots.
        consumable_upkeep(state);
        // AFTER the per-character pass, in a fixed order — both layers apply every tick and the
        // sequence must never depend on layout.
        upkeep::party_provision_upkeep(state);
        // Phoenix Tail: a living holder revives a downed ally mid-fight (separate
        // pass — it touches another entity than the holder).
        phoenix_revive_upkeep(state);
        // Bishop Resurrection: auto-revive a downed ally if a charge is banked.
        resurrection_upkeep(state);
        // Taunt aggro locks refresh before targeting so this tick's
        // choices see them. (Endure has no upkeep since 0.0.178 — it's a
        // passive VIT + mob-penalty resist read inside apply_damage.)
        apply_taunt(state);
        crate::potential::effects::refresh(&mut state.grid.entities, state.tick, false);
        // Limit Break's slow hand: a quiet fight still fills, once a second.
        if state.tick % 10 == 0 {
            for e in state.grid.entities.iter_mut() {
                if !e.alive {
                    continue;
                }
                let gain = crate::skills::lb_gain(&e.skills, |d| d.lb_per_second);
                crate::skills::charge_limit_break(e, gain);
            }
        }
        // Ember Cascade's pacing clock — only runs while she has something queued.
        for e in state.grid.entities.iter_mut() {
            if e.break_cd > 0 && !e.break_queue.is_empty() {
                e.break_cd -= 1;
            }
        }
        // DEPLOYMENT GRACE: for the first DEPLOY_GRACE_TICKS of a freshly-opened zone NOBODY acts —
        // no target, no step, no swing, no projectile lands, on either team. The map opens with the
        // party walking into position (the renderer strides it in from the left; monsters wait on
        // the right) before the fight starts, instead of combat resolving from tick 0 while the
        // sprites are still sliding into place. Spawns + upkeep still run below, so the field
        // populates during the pause. Anchored on `opened_at_tick`, the opening window's origin.
        // `opened_at_tick > 0` scopes this to a zone CHANGE, never the initial zone: a new game (and
        // every combat unit test) opens its first zone at tick 0 and expects to act at once, and the
        // renderer likewise strides the party in only on a zone change, not the first spawn.
        // LANE MODE (off unless `state.lane` is set; `lane.rs`): stage pacing + wave spawns.
        if state.lane.is_some() {
            crate::lane::upkeep(state);
        }
        let in_grace = state.zone_spawn.as_ref().is_some_and(|z| {
            z.opened_at_tick > 0
                && state.tick.saturating_sub(z.opened_at_tick) < z.deploy_grace_ticks as u64
        });
        if !in_grace {
            resolve_actions(state);
        }
        collect_break_triggers(state);
        refill_toughness(state);
        // Boss appears when the threshold is reached. Continuous spawn
        // keeps going alongside the boss — user spec: "spawn พร้อมของ
        // เดิม รุมหลายตัว". Boss spawn happens BEFORE continuous spawn
        // each tick so the boss claims its random cell first.
        let first_new = state.next_monster_id;
        if crate::lane::boss_allowed(state) {
            try_spawn_boss(state);
        }
        if state.lane.is_none() {
            spawn::try_continuous_spawn(state);
        }
        if state.lane.is_some() {
            crate::lane::to_lane_edges(state, first_new);
        }
        // The zone clears when the field is SWEPT, not when the boss falls: mark it once the boss
        // has appeared (so the spawner has stopped and the wave is finite) and no monster is left
        // alive. AFTER the spawns above, so a boss/minions born this tick are counted.
        mark_zone_cleared_if_swept(state);
        // `zone_cleared` (set just above) becomes a request for the bridge/CLI to switch zones
        // next time it loops.
        request_zone_change_if_cleared(state);
        // A fight nobody can end is a lost fight: fell the survivors so the wipe path runs.
        break_stalemate(state);
        // ...and the other way a dungeon run can end.
        raise_dungeon_fail_if_wiped(state);
        // Stop dead monster slots from piling up over a long session. Dead
        // bodies never act or block cells, but they bloat every O(n) scan
        // (targeting, auras) AND the per-tick state JSON the client parses —
        // the cause of the late-game stutter. Keep a buffer so freshly-killed
        // monsters still play their death animation, then drop the oldest.
        prune_dead_monsters(state);
        // DEBUG god mode (no-op when off): top players up at the END of the tick
        // — full HP/SP, revived, cooldowns cleared, huge ATK/MATK buff stamped —
        // so they're invincible at the tick boundary and one-shot next tick.
        apply_debug_god(state);
        note_damage_records(state);
        note_deaths(state, &standing);
    }
}

/// Update the party's personal bests from the hits this tick produced.
///
/// Read off `recent_hits` / `field_hits` and NOT off `record_damage`, because the DPS meter is
/// deliberately fed `dealt.min(pre_hp)` — effective damage, overkill clamped. That is correct for
/// DPS and wrong for a best: the hardest hit is precisely the one that overkills, so a 500,000
/// crit on a 12,000-HP monster would be filed as 12,000 while the player watched 500000 come off
/// the screen. `HitEvent.amount` is the number the renderer draws, so the stat and the popup can
/// never disagree.
///
/// Scanning the two vectors once per tick also means no call site can be missed: every hit the
/// player sees is in one of them by construction, which a per-site hook could not promise across
/// fourteen of them. `HitEvent.hits` is `display_hits` — one instance split into N numbers — so
/// `amount` is the whole instance, which is what a biggest-hit figure means.
///
/// The same pass also books `damage_taken` and `skills_cast`, because they are answers to the
/// same question — what happened this tick — and reading the vectors three times would let the
/// three drift apart. Attacker-side and target-side are exclusive: a party member catching
/// friendly splash is not the party damaging itself.
pub(crate) fn note_damage_records(state: &mut GameState) {
    let mut best: i32 = 0;
    let mut taken: u64 = 0;
    for h in state.recent_hits.iter().chain(state.field_hits.iter()) {
        if h.amount <= 0 {
            continue;
        }
        // The two sides are INDEPENDENT, not exclusive. Friendly splash — a party AoE catching
        // an ally — is both the caster's hit and HP the party genuinely lost, and they land in
        // different counters, so there is nothing to double-book. An earlier `else if` here
        // silently dropped that damage; it survived its own sabotage because the test only ever
        // aimed outgoing hits at monsters, where the target-side check excludes them anyway.
        if attacker_is_party(state, h.attacker_id) && h.amount > best {
            best = h.amount;
        }
        if is_party(state, h.target_id) {
            taken = taken.saturating_add(h.amount as u64);
            if let Some(class) = unit_def_of(state, h.target_id) {
                *state.dps.taken_by_class.entry(class).or_insert(0) += h.amount as u64;
            }
        }
    }
    // UNDA VITAE (0.0.468): spend what her blows booked, onto the most wounded ally.
    let debts: Vec<(usize, i32)> = state
        .grid
        .entities
        .iter_mut()
        .enumerate()
        .filter(|(_, e)| e.heal_debt > 0)
        .map(|(i, e)| (i, std::mem::take(&mut e.heal_debt)))
        .collect();
    for (i, amount) in debts {
        let team = state.grid.entities[i].team;
        let worst = state
            .grid
            .entities
            .iter()
            .enumerate()
            .filter(|(_, e)| e.alive && e.team == team && !e.is_pet)
            .min_by_key(|(j, e)| {
                let maxhp = crate::combat::effective_max_hp(e).max(1) as i64;
                ((e.hp as i64) * 10_000 / maxhp, e.id, *j as u32)
            })
            .map(|(j, _)| j);
        if let Some(j) = worst {
            combat_step::apply_skill_heal_from(state, i, j, amount);
            combat_step::healed_ally_buff(state, i, j);
        }
    }
    // Static Spark this tick → the LAYER's damage tally (display-only, 0.0.465).
    let sparks: Vec<(u32, i32)> = state.grid.entities.iter_mut().flat_map(|e| e.spark_fx.drain(..)).collect();
    for (src, amt) in sparks {
        if amt > 0 {
            if let Some(class) = unit_def_of(state, src) {
                *state.dps.by_class.entry(class.clone()).or_insert(0) += amt as u64;
                *state.dps.by_class_skill.entry((class, "Static Spark".to_string())).or_insert(0) += amt as u64;
                *state.dps.by_skill.entry("Static Spark".to_string()).or_insert(0) += amt as u64;
            }
        }
    }
    // Mitigation this tick → the wearer's tally (display-only).
    let mitigated: Vec<(u32, i64)> = state.grid.entities.iter_mut().filter(|e| e.mitigated_fx != 0).map(|e| (e.id, std::mem::take(&mut e.mitigated_fx))).collect();
    for (id, amt) in mitigated {
        if amt > 0 {
            if let Some(class) = unit_def_of(state, id) {
                *state.dps.mitigated_by_class.entry(class).or_insert(0) += amt as u64;
            }
        }
    }
    // Shield soak this tick → the placer's block tally (display-only).
    let soaked: Vec<(u32, i32)> = state.grid.entities.iter_mut().flat_map(|e| e.soaked_fx.drain(..)).collect();
    for (src, amt) in soaked {
        if amt > 0 {
            if let Some(class) = unit_def_of(state, src) {
                *state.dps.blocked_by_class.entry(class).or_insert(0) += amt as u64;
            }
        }
    }
    if best > 0 && best as u64 > state.records.biggest_hit {
        state.records.biggest_hit = best as u64;
    }
    if taken > 0 {
        state.measures.damage_taken.add(taken);
    }
    // Casts ride the list the renderer already uses to pick a skill's VFX, so a skill that plays
    // an effect and a skill that counts are the same set by construction.
    let cast = state
        .skill_fx
        .iter()
        .filter(|f| is_party(state, f.caster_id))
        .count() as u64;
    if cast > 0 {
        state.measures.skills_cast.add(cast);
    }
    // Healing rides the event list for the same reason as the rest: the renderer's green number
    // and this counter should never be able to disagree about whether a heal happened.
    let healed: u64 = state
        .heal_fx
        .iter()
        .filter(|h| h.amount > 0 && is_party(state, h.healer_id))
        .map(|h| h.amount as u64)
        .sum();
    if healed > 0 {
        state.measures.healing_done.add(healed);
        let per: Vec<(String, u64)> = state
            .heal_fx
            .iter()
            .filter(|h| h.amount > 0)
            .filter_map(|h| unit_def_of(state, h.healer_id).map(|c| (c, h.amount as u64)))
            .collect();
        for (class, amt) in per {
            *state.dps.healed_by_class.entry(class).or_insert(0) += amt;
        }
    }
}

/// Party members who were standing at the top of the tick and are not standing now.
pub(crate) fn note_deaths(state: &mut GameState, standing: &[u32]) {
    let fallen: Vec<u32> = standing
        .iter()
        .copied()
        .filter(|id| state.grid.idx_of_id(*id).is_none_or(|i| !state.grid.entities[i].alive))
        .collect();
    if !fallen.is_empty() {
        state.measures.deaths.add(fallen.len() as u64);
        for id in fallen {
            if let Some(class) = unit_def_of(state, id) {
                *state.dps.deaths_by_class.entry(class).or_insert(0) += 1;
            }
        }
    }
    // Uptime: every player standing at the END of the tick banks a tick (display-only).
    let alive: Vec<String> = state
        .grid
        .entities
        .iter()
        .filter(|e| matches!(e.team, Team::Player) && !e.is_pet && e.alive)
        .map(|e| e.unit_def.clone())
        .collect();
    for class in alive {
        *state.dps.alive_ticks_by_class.entry(class).or_insert(0) += 1;
    }
}

/// The unit_def of a PLAYER entity by id — `None` for monsters, pets and the unknown.
fn unit_def_of(state: &GameState, id: u32) -> Option<String> {
    state.grid.idx_of_id(id).and_then(|i| {
        let e = &state.grid.entities[i];
        (matches!(e.team, crate::grid::Team::Player) && !e.is_pet).then(|| e.unit_def.clone())
    })
}

fn is_party(state: &GameState, id: u32) -> bool {
    state
        .grid
        .idx_of_id(id)
        .is_some_and(|i| matches!(state.grid.entities[i].team, crate::grid::Team::Player))
}

fn attacker_is_party(state: &GameState, attacker_id: u32) -> bool {
    state
        .grid
        .idx_of_id(attacker_id)
        .is_some_and(|i| matches!(state.grid.entities[i].team, crate::grid::Team::Player))
}

/// Cap retained dead MONSTER slots at `MAX_DEAD_MONSTERS`, removing the oldest
/// (lowest-index) extras. Dead players are always kept (revive/party). This is
/// gameplay-neutral — dead entities take no actions, never block cells, and are
/// skipped by targeting — so it only trims their serialized slots. Short runs
/// (every golden) kill far fewer than the cap, so they never prune and stay
/// byte-identical.
const MAX_DEAD_MONSTERS: usize = 64;

/// HEARTSTRING leech (0.0.451): the share of every leeching-DoT tick paid back to its source
/// as healing (user: "ดูดเลือด 10% ของความเสียหาย").
pub const LEECH_BP: u32 = 1_000;

/// Ticks a dead monster's corpse lingers before the RENDER view drops it (the
/// bridge's `view_state` filters corpses older than this out of the JSON the
/// client parses, so N accumulated invisible corpses stop bloating the parse +
/// spawning render nodes). ~2s covers any death animation. This is a VIEW-only
/// cut — the canonical `GameState` keeps the corpse (until the count cap), so
/// golden hashes are byte-identical (`down_ticks` is serde-skip).
pub const CORPSE_RENDER_LINGER_TICKS: u32 = 20;

fn prune_dead_monsters(state: &mut GameState) {
    // Age each corpse's linger timer (reuses the serde-skip `down_ticks` — a
    // monster never revives, so the field is free for corpse age; hash-neutral).
    for e in state.grid.entities.iter_mut() {
        if !e.alive && matches!(e.team, Team::Monster) {
            e.down_ticks = e.down_ticks.saturating_add(1);
        }
    }
    let dead = state
        .grid
        .entities
        .iter()
        .filter(|e| !e.alive && matches!(e.team, Team::Monster))
        .count();
    if dead <= MAX_DEAD_MONSTERS {
        return;
    }
    let mut to_remove = dead - MAX_DEAD_MONSTERS;
    state.grid.entities.retain(|e| {
        if to_remove > 0 && !e.alive && matches!(e.team, Team::Monster) {
            to_remove -= 1;
            false
        } else {
            true
        }
    });
}

/// Apply damage from any projectile whose `arrive_at_tick` has come due,
/// then drop those projectiles. Order is preserved: projectiles that survive
/// (still in flight) stay in their original `state.projectiles` order so
/// renderer interpolation doesn't jitter.
///
/// Whiff rule: if the target was killed before arrival (by something
/// else — melee, another projectile), the in-flight projectile lands harmlessly
/// and is removed. The pre-computed `projectile.damage` is otherwise
/// applied as-is — the attacker's stats at FIRE time are what matter,
/// not their stats now (GDD §3.2: ranged attacks commit on fire).
fn resolve_arrived_projectiles(state: &mut GameState) {
    // Two-pass split so retain() can run without holding a borrow on
    // state.grid / state.rng while we mutate them in the apply step.
    let mut fired: Vec<Projectile> = Vec::new();
    state.projectiles.retain(|a| {
        if a.arrive_at_tick <= state.tick {
            fired.push(a.clone());
            false
        } else {
            true
        }
    });
    for projectile in fired {
        // The blast centres on the target's CURRENT cell if it's still
        // alive; otherwise on the cell it occupied at fire time (so an
        // AoE still detonates where it was aimed even if the primary
        // target died in flight).
        let target_idx = state.grid.idx_of_id(projectile.target_id);
        let center = match target_idx {
            Some(ti) if state.grid.entities[ti].alive => state.grid.entities[ti].pos,
            _ => projectile.target_pos,
        };
        // Collect every entity that takes this hit, in stable index order
        // (so drop-roll PRNG draws stay deterministic for multi-kills).
        let mut victims: Vec<usize> = Vec::new();
        if projectile.aoe_radius > 0 {
            for (idx, e) in state.grid.entities.iter().enumerate() {
                // Shape via `Blast`; the CENTRE IS INCLUDED here, unlike `victims_in_radius` —
                // a projectile blast must damage the target it landed on.
                let box_ = crate::grid::Blast::radius(center, projectile.aoe_radius);
                if e.on_board() && e.team == projectile.victim_team && box_.contains(e.pos) {
                    victims.push(idx);
                }
            }
        } else {
            // Single target: only the homing target, and only if alive and still
            // on the board (a `regent_hand` lift makes an in-flight shot whiff).
            match target_idx {
                Some(ti) if state.grid.entities[ti].on_board() => victims.push(ti),
                _ => {} // whiff — target died or was lifted before the projectile landed
            }
        }
        // Attribute the hit to the firer (for ranged on-kill effects + Knock Back
        // Shot). `None` if the archer despawned mid-flight.
        // A firer that died while the shot was in flight is no longer an attacker (audit
        // 2026-10-03): the shot lands plainly instead of charging, healing or breaking for a corpse.
        let attacker_idx = if projectile.attacker_id != 0 {
            state.grid.idx_of_id(projectile.attacker_id).filter(|&ai| state.grid.entities[ai].alive)
        } else {
            None
        };
        // Focus rune (archer arrow_shower): the shot's TOTAL damage is multiplied
        // then SPLIT evenly among the victims — a lone target eats the whole
        // multiplier, a crowd thins it. `hit_damage` is the per-victim amount.
        let hit_damage = if projectile.focus_mult_bp > 0 {
            let n = (victims.len().max(1)) as i64;
            ((projectile.damage as i64) * projectile.focus_mult_bp as i64 / 10_000 / n) as i32
        } else {
            projectile.damage
        };
        // S2 (0.0.441, Rapis): a blast that catches a crowd shortens its own cooldown.
        if projectile.aoe_radius > 0 && !projectile.skill_id.is_empty() {
            if let Some(ai) = attacker_idx {
                let slot = state.grid.entities.get(ai).and_then(|e| e.skills.iter().position(|s| s.id == projectile.skill_id));
                if let Some(slot) = slot {
                    crate::unit_rank::on_aoe_hits(state, ai, slot, &projectile.skill_id, victims.len());
                }
            }
        }
        for vidx in victims {
            if !state.grid.entities[vidx].alive {
                continue; // a prior hit this tick already downed it
            }
            // Greenling (defender-side): shave 2.5→5% off a BASIC projectile —
            // the ranged mirror of the melee fold in `resolve_attack_scaled`.
            // Per-victim (each wears its own armor card); 0 without the card.
            let hit_damage = if projectile.is_basic {
                let red = crate::combat::card_specials::basic_dmg_taken_reduction_bp(
                    &state.grid.entities[vidx]);
                if red > 0 {
                    (((hit_damage as i64) * (10_000 - red as i64)) / 10_000).max(1) as i32
                } else {
                    hit_damage
                }
            } else {
                hit_damage
            };
            // THE FULL LANDING (audit 2026-10-02): shields, on-hit-taken Limit Break, marks,
            // sparks, Frost Aura and the Toughness chip + break — a bare `take_hit` skipped all
            // of it. The packet carries what the impact needs; its amount is already mitigated.
            match attacker_idx {
                Some(ai) => {
                    let dmg = crate::combat::Damage {
                        amount: hit_damage,
                        element: projectile.element,
                        is_ranged: true,
                        is_crit: projectile.crit,
                        is_aoe: projectile.aoe_radius > 0,
                        ..crate::combat::build_damage(&state.grid.entities[ai], projectile.crit)
                    };
                    let mob = crate::combat::attackers_on(&state.grid.entities, vidx);
                    let tough = crate::skills::skill_def(&projectile.skill_id).map_or(0, |d| d.toughness_dmg_mult);
                    let through = crate::combat::land_blow(
                        &mut state.grid.entities, ai, vidx, hit_damage, &dmg, mob, tough, &mut state.recent_hits,
                    );
                    // D2 (audit 2026-10-02): a landed BASIC charges the firer's Limit Break, as a
                    // melee basic does — ranged units' `lb_on_basic` was dead.
                    if projectile.is_basic && through > 0 {
                        let gain = crate::skills::lb_gain(&state.grid.entities[ai].skills, |d| d.lb_on_basic);
                        crate::skills::charge_limit_break(&mut state.grid.entities[ai], gain);
                    }
                }
                // The firer is gone: the shot still lands, through the victim's shields (audit
                // 2026-10-03: the plain hit here skipped them), without the firer-side riders.
                None => {
                    let t = &mut state.grid.entities[vidx];
                    let thr = crate::combat::absorb_soak(t, hit_damage);
                    t.take_hit(thr);
                    if hit_damage > 0 && thr == 0 {
                        state.recent_hits.push(crate::state::HitEvent {
                            target_id: t.id,
                            attacker_id: projectile.attacker_id,
                            amount: 0,
                            hits: 0,
                            kind: crate::state::HitKind::Guard,
                        });
                    }
                }
            }
            // VISUAL multi-hit (Twin Strafe): record ONE display event with N sub-hits
            // so the renderer splits the single instance into N numbers (like Double
            // Attack). Damage was applied once above; this is display only.
            // A crit single shot records its event too, so the renderer can label the number.
            if projectile.display_hits > 1 || projectile.crit {
                state.recent_hits.push(crate::state::HitEvent {
                    target_id: state.grid.entities[vidx].id,
                    attacker_id: projectile.attacker_id,
                    amount: hit_damage,
                    hits: projectile.display_hits.max(1),
                    kind: if projectile.crit { crate::state::HitKind::Crit } else { crate::state::HitKind::Normal },
                });
            }
            // Static Spark (0.0.465, Mirei): a magic blast seeds every victim it lands on —
            // the projectile path applies no on-hit debuffs, so the seeding rides here.
            if let (Some(ai), Some(d)) = (attacker_idx, crate::skills::skill_def(&projectile.skill_id)) {
                if d.spark_stacks > 0 {
                    combat_step::lay_static_spark(state, ai, vidx, d);
                }
            }
            if !state.grid.entities[vidx].alive {
                kill_monster(state, vidx, attacker_idx);
            } else {
                if projectile.aoe_radius == 0 {
                    // Survived a single-target shot → Ranger Knock Back Shot may shove it.
                    maybe_knock_back_shot(state, attacker_idx, vidx);
                }
                // A surviving blast victim burns for tier× the hit damage over 8 s. The only
                // author of `burn_tier` was the Combustion rune, retired in 0.0.369 — the field
                // and this branch stay because the Projectile shape is anchored (see
                // `replay.rs`) and a future skill can set it; `apply_combustion_burn` itself is
                // still live through the Yellowling thorn burn.
                if projectile.burn_tier > 0 {
                    apply_combustion_burn(
                        state, vidx, projectile.attacker_id,
                        hit_damage, projectile.burn_tier,
                    );
                }
                // The shot SLOWS survivors. Same story as `burn_tier`: its only author was the
                // Pinning rune (retired 0.0.369), so this is always zero today.
                if projectile.pin_slow_pct > 0 {
                    crate::skills::apply_buff(
                        &mut state.grid.entities[vidx],
                        crate::skills::Buff {
                            kind: crate::skills::BuffKind::AgiDexSlowPct,
                            magnitude: projectile.pin_slow_pct,
                            ticks: 50, // 5 s
                            source_id: projectile.attacker_id,
                        },
                    );
                }
            }
            // CARD SPECIAL procs + Gnarloak reflect for RANGED BASIC hits
            // (0.0.213 — the melee-only gap the user flagged): same fixed
            // order + card gating as the melee site; skill shots excluded.
            if projectile.is_basic && hit_damage > 0 {
                if let Some(ai) = attacker_idx {
                    if state.grid.entities[ai].alive {
                        combat_step::try_card_procs(state, ai, vidx, hit_damage);
                        combat_step::apply_card_reflect(state, ai, vidx, hit_damage);
                        combat_step::try_thorn_burn(state, ai, vidx, hit_damage);
                    }
                }
            }
        }
    }
}

/// Ticks of a Combustion burn (10 s at 10 ticks/s). A LINEAR-DECAY DoT: peak on
/// the first second, −10% of peak each second, to 0 at 10 s. `magnitude` stores
/// the PEAK per-second value; the per-interval damage is scaled by the seconds
/// left in `upkeep`, so the total ≈ peak × 5.5.
pub(crate) const BURN_DURATION_TICKS: u32 = 100;

/// Burn: ignite `vidx` at a peak of `20% × tier` of the hit damage
/// per second, decaying linearly to 0 over 10 s (total ≈ 1.1×/2.2×/3.3× the hit at
/// tier 1/2/3). A Burn-kind DoT (aged by `decay_buffs`; does NOT feed Poison
/// Explosion). Re-applying from the SAME caster STACKS the peak and resets the
/// duration to a full 10 s (user 2026-07-06: "add value + reset duration") — so
/// sustained fireballs ramp a bigger, longer fire.
pub(crate) fn apply_combustion_burn(state: &mut GameState, vidx: usize, source_id: u32, hit_damage: i32, tier: u8) {
    // Peak per-second = 20% × tier of the hit (single-sourced in items).
    let peak = ((hit_damage as i64) * crate::items::combustion_peak_bp(tier) as i64 / 10_000)
        .max(1) as i32;
    let buffs = &mut state.grid.entities[vidx].buffs;
    if let Some(b) = buffs
        .iter_mut()
        .find(|b| matches!(b.kind, crate::skills::BuffKind::Burn) && b.source_id == source_id)
    {
        // Stack onto the CURRENT (already-decayed) value, NOT the stored peak
        // (user 2026-07-06: peak 100 → after 5 s it is 50 → +100 = 150, then it
        // decays 15/s over the reset 10 s). current = peak × ticks_left / duration.
        let current = (b.magnitude as i64 * b.ticks as i64 / BURN_DURATION_TICKS as i64) as i32;
        b.magnitude = current.saturating_add(peak); // new peak = current + incoming
        b.ticks = BURN_DURATION_TICKS; // reset to a full 10 s
    } else {
        buffs.push(crate::skills::Buff {
            kind: crate::skills::BuffKind::Burn,
            magnitude: peak,
            ticks: BURN_DURATION_TICKS,
            source_id,
        });
    }
}

/// If the active zone has hit `boss_threshold` and hasn't spawned a
/// boss yet, spawn it now. One boss per zone load.
fn try_spawn_boss(state: &mut GameState) {
    let (should, boss_stats, from_pool) = match state.zone_spawn.as_ref() {
        Some(zs) if !zs.boss_spawned
            && zs.kill_counter >= zs.boss_threshold
            && zs.boss_template.is_some() =>
        {
            (true, zs.boss_template.clone().unwrap(), zs.boss_from_pool)
        }
        _ => return,
    };
    if !should {
        return;
    }
    let player_pos = state
        .grid
        .entities
        .iter()
        .find(|e| matches!(e.team, Team::Player) && e.alive && !e.is_pet)
        .map(|e| e.pos);
    let Some(cell) = spawn::pick_random_cell_for_boss(state, player_pos) else {
        // Couldn't find a cell — try again next tick.
        return;
    };
    // The boss is EITHER a random eligible pool mob wearing the boss stats (gothic bands 5-10)
    // or the fixed boss_template. The random pick is one PRNG draw, AFTER the cell pick.
    let template = if from_pool {
        spawn::pick_pool_boss_template(state, &boss_stats)
    } else {
        boss_stats
    };
    // Build through the SAME constructor as regular spawns so the boss gets its
    // FULL template — mdef/hit/flee, atk min-max variance and catalog skills
    // included (0.0.128 fix: the old hand-rolled copy silently dropped those, so
    // every boss fought with 0 mdef, fallback hit/flee, flat ATK and no skills).
    let boss_minions = template.boss_minions;
    let bi = spawn::build_and_spawn_monster(state, template, cell);
    state.grid.entities[bi].is_boss = true;
    state.zone_spawn.as_mut().unwrap().boss_spawned = true;
    // The boss arrives WITH its entourage: spawn its minions ringing it, then
    // continuous spawn halts (gated on `boss_spawned`) — the room is sealed.
    spawn::spawn_minions_around(state, cell, boss_minions);
}

/// Ticks the zone change is HELD after the boss dies, so its death animation
/// (humanoid die clip ≈ 1.25s at 8fps + a beat) finishes on the client before
/// the room swaps, and leaves room for the client's zone-change transition (a ~1 s
/// beat + a ~1 s diamond wipe) to cover the swap. 10 ticks/s ⇒ 2.5s.
pub(crate) const ZONE_CLEAR_DELAY_TICKS: u64 = 25;

/// A party wipe INSIDE A DUNGEON ends the run — raise the same summary a clear does, flagged
/// `failed`, so the client shows ONE screen with two verdicts instead of silently restarting.
///
/// DUNGEONS ONLY. A normal band wipe keeps the respawn-and-retry loop: that is the core
/// progression grind, not a run with a beginning and an end, and interrupting it with a modal
/// every death would be miserable. A dungeon is a chain whose start zone is not a band zone —
/// `band_of_zone("zone_b4_1")` is Some(4), `band_of_zone("zone_dungeon_duskmourn_1")` is None.
/// That distinction needs no new data field, which matters because adding one to ZoneDef moves
/// every golden.
/// STALEMATE (0.0.416, re-keyed 0.0.418): a boss is up, at least one member is down, and the
/// boss's HP has not fallen below its lowest for `STALEMATE_TICKS` — the survivors cannot finish
/// it and it cannot finish them (a self-healing maid boss against a shielded Seraphine stood for
/// five sim days). Fell them, which is what a lost boss fight is, so the host's wipe path
/// (fall-back, revive) runs. The FLOOR is the signal, not kills or damage: a healing boss takes
/// damage all day, and a boss room offers nothing else to kill. No PRNG draw.
fn break_stalemate(state: &mut GameState) {
    let boss_up = state.zone_spawn.as_ref().is_some_and(|z| z.boss_spawned && !z.zone_cleared);
    let boss_hp = state
        .grid
        .entities
        .iter()
        .filter(|e| matches!(e.team, Team::Monster) && e.alive && e.is_boss)
        .map(|e| e.hp)
        .min();
    let (alive, down) = state.grid.entities.iter().fold((0u32, 0u32), |(a, d), e| {
        if !matches!(e.team, Team::Player) || e.is_pet || e.owner_id != 0 {
            (a, d)
        } else if e.alive {
            (a + 1, d)
        } else {
            (a, d + 1)
        }
    });
    // No enemy standing = nothing to be stuck on (a lull between spawns, an empty room, a
    // scenario with no spawner) — the clock only runs against something alive.
    let enemy_up = state.grid.entities.iter().any(|e| matches!(e.team, Team::Monster) && e.alive && e.on_board());
    if alive == 0 || !enemy_up {
        state.stalemate_ticks = 0;
        state.stalemate_floor = 0;
        state.stalemate_kills = 0;
        return;
    }
    // Nobody down: the clock still runs, on the LONG window (0.0.443) — an S6 party on stage 240
    // stood a whole hour killing nothing and losing no one, which no rule could end.
    let window = if down > 0 { crate::state::STALEMATE_TICKS } else { crate::state::STALEMATE_TICKS_HEALTHY };
    if let (Some(hp), true) = (boss_hp, boss_up) {
        state.stalemate_kills = 0;
        // A new floor (or a fight just begun) resets the clock; a floor that holds runs it.
        if state.stalemate_floor == 0 || hp < state.stalemate_floor {
            state.stalemate_floor = hp.max(1);
            state.stalemate_ticks = 0;
            return;
        }
    } else if down == 0 {
        // FULL STRENGTH, trash (0.0.443): the signal is the ROOM's HP floor — the lowest total
        // enemy HP seen — not the kill count: a slow party that is still grinding a tough room
        // down keeps moving the floor (a kill drops it too), and only a party that dents nothing
        // for the whole window is stuck. A fresh spawn lifts the total, never the floor.
        state.stalemate_kills = 0;
        let total: i64 = state.grid.entities.iter().filter(|e| matches!(e.team, Team::Monster) && e.alive && e.on_board()).map(|e| i64::from(e.hp.max(0))).sum();
        let total = total.min(i64::from(i32::MAX)) as i32;
        if state.stalemate_floor == 0 || total < state.stalemate_floor {
            state.stalemate_floor = total.max(1);
            state.stalemate_ticks = 0;
            return;
        }
    } else {
        // TRASH ROOM (0.0.429): the kill counter is the signal — a room offers plenty to kill,
        // so a counter that holds while someone is down is a fight nobody can end.
        state.stalemate_floor = 0;
        let kills = state.zone_spawn.as_ref().map_or(0, |z| z.kill_counter).saturating_add(1);
        if state.stalemate_kills != kills {
            state.stalemate_kills = kills;
            state.stalemate_ticks = 0;
            return;
        }
    }
    state.stalemate_ticks = state.stalemate_ticks.saturating_add(1);
    if state.stalemate_ticks < window {
        return;
    }
    state.stalemate_ticks = 0;
    state.stalemate_floor = 0;
    state.stalemate_kills = 0;
    for e in state.grid.entities.iter_mut() {
        if matches!(e.team, Team::Player) && !e.is_pet && e.owner_id == 0 && e.alive {
            e.take_hit(e.hp.max(1));
        }
    }
}

fn raise_dungeon_fail_if_wiped(state: &mut GameState) {
    if state.last_clear.is_some() {
        return; // a verdict is already on screen
    }
    let inst_start = match state.zone_spawn.as_ref() {
        Some(zs) => zs.instance_start_zone_id.clone(),
        None => return,
    };
    if inst_start.is_empty() || crate::state::band_of_zone(&inst_start).is_some() {
        return; // not inside a dungeon
    }
    let any_alive = state
        .grid
        .entities
        .iter()
        .any(|e| matches!(e.team, Team::Player) && e.alive && !e.is_pet);
    if any_alive {
        return;
    }
    state.last_clear = Some(crate::state::ClearSummary {
        instance_id: inst_start,
        ticks: state.tick.saturating_sub(state.run_tally.started_tick),
        kills: state.measures.kills.saturating_sub(state.run_tally.start_kills),
        xp: state.run_tally.xp,
        gil: state.run_tally.gil,
        loot: state.run_tally.loot.clone(),
        failed: true,
    });
}

/// Flip `zone_cleared` when the party has SWEPT the field: the boss has appeared (so the spawner
/// has halted and the surviving monsters are a finite, draining set — see `try_continuous_spawn`'s
/// `boss_spawned` gate) and none is left alive. This replaces the old "boss dies → cleared" so
/// leftover mobs must be finished too, the rule the continuous walk-through world needs. No-op for a
/// bossless farm zone (`boss_spawned` never set) and once the zone is already cleared.
fn mark_zone_cleared_if_swept(state: &mut GameState) {
    let ready = matches!(
        state.zone_spawn.as_ref(),
        Some(zs) if zs.boss_spawned && !zs.zone_cleared
    );
    if !ready {
        return;
    }
    let any_monster_alive = state
        .grid
        .entities
        .iter()
        .any(|e| e.alive && matches!(e.team, Team::Monster));
    if any_monster_alive {
        return;
    }
    if let Some(zs) = state.zone_spawn.as_mut() {
        zs.zone_cleared = true;
    }
    state.measures.rooms_cleared.add(1);
}

fn request_zone_change_if_cleared(state: &mut GameState) {
    if state.pending_zone_change.is_some() {
        return; // already queued
    }
    if !matches!(state.zone_spawn.as_ref(), Some(zs) if zs.zone_cleared) {
        return;
    }
    // Stamp the clear tick on first sight, then hold the swap until the death
    // animation has had time to play (deterministic — a fixed tick delay, no
    // wall-clock). The boss corpse is retained by the dead-monster buffer.
    let now = state.tick;
    let cleared_at = {
        let zs = state.zone_spawn.as_mut().unwrap();
        *zs.cleared_at_tick.get_or_insert(now)
    };
    if now < cleared_at + ZONE_CLEAR_DELAY_TICKS {
        return;
    }
    // If the WHOLE party got finished off during the hold (leftover minions
    // after the boss died), the clear does NOT count — don't advance. The party
    // is dead, so the harness's wipe path restarts the instance instead. (A
    // partial death still advances: change_zone revives the fallen on entry.)
    let any_player_alive = state
        .grid
        .entities
        .iter()
        .any(|e| matches!(e.team, Team::Player) && e.alive && !e.is_pet);
    if !any_player_alive {
        return;
    }
    let (is_end, next, inst_start) = match state.zone_spawn.as_ref() {
        Some(zs) => (
            zs.is_instance_end,
            zs.next_zone_id.clone(),
            zs.instance_start_zone_id.clone(),
        ),
        None => return,
    };
    // Does anything move the party after this clear? A band's third zone advances to the next
    // band; FARM mode loops back to room 1. Only when NOTHING moves them does the run truly END
    // — the party is left standing in the room, and that is the one case the clear screen is for.
    //
    // ⚠️ `is_instance_end` alone is NOT that test. It is set on all 60 `zone_bN_3` zones, where it
    // has always meant "bank the escrow, count the band clear"; the dungeon kit reused the flag
    // for the clear screen and so popped a modal on every ordinary band boss (play-test
    // 2026-07-31). A dungeon's final room is distinguished by having no `next_zone_id`.
    let farm_loop = is_end && state.farm_current_instance && !inst_start.is_empty();
    let run_ends_here = is_end && !farm_loop && next.is_none();
    // A run that ENDS here leaves the party standing in the room, so this block is reached every
    // tick after the clear. Handle it ONCE (audit 2026-10-02): it re-wrote the clear summary,
    // re-banked the escrow and counted the fall-back countdown down every tick.
    if run_ends_here && state.zone_spawn.as_ref().is_some_and(|zs| zs.band_clear_counted) {
        return;
    }
    // Clearing the instance's FINAL room (big boss) banks the loot escrow to the
    // stash AND counts one run of this band into the back-log (latched so the
    // per-tick check can't tally the same clear twice). A mid-room clear does
    // neither.
    if is_end {
        // Snapshot what this run produced BEFORE the escrow drains into the stash —
        // afterwards the loot is indistinguishable from everything else banked. Only for a run
        // that ENDS here: see `run_ends_here`.
        if run_ends_here {
            state.last_clear = Some(crate::state::ClearSummary {
                instance_id: inst_start.clone(),
                ticks: state.tick.saturating_sub(state.run_tally.started_tick),
                kills: state.measures.kills.saturating_sub(state.run_tally.start_kills),
                xp: state.run_tally.xp,
                gil: state.run_tally.gil,
                loot: state.run_tally.loot.clone(),
                failed: false,
            });
        }
        drops::grant_escrow_to_stash(state);
        // A successful instance clear means the party is NOT walled here → reset the wipe streak.
        state.consecutive_wipes = 0;
        // THE FALL-BACK COUNTDOWN (0.0.403): while farming the stage below after a wipe, each
        // clear counts one down; at zero the party goes back up to the stage that walled it, farm
        // mode off (`data::handle_party_wipe` set this up).
        if state.retreat_return_band > 0 && state.retreat_farm_remaining > 0 {
            state.retreat_farm_remaining -= 1;
            if state.retreat_farm_remaining == 0 {
                let ret = state.retreat_return_band;
                state.retreat_return_band = 0;
                state.farm_current_instance = false;
                state.pending_zone_change = Some(format!("zone_b{ret}_1"));
                return;
            }
        }
        let already = state
            .zone_spawn
            .as_ref()
            .map(|zs| zs.band_clear_counted)
            .unwrap_or(true);
        if !already {
            if let Some(band) = crate::state::band_of_zone(&inst_start) {
                state.record_band_clear(band);
            }
            // (Per-session resets — Hiding charge + Bloodthirst streak — live in
            // `change_zone`, which all transitions funnel through.)
            if let Some(zs) = state.zone_spawn.as_mut() {
                zs.band_clear_counted = true;
            }
        }
    }
    // On instance end, FARM mode loops back to room 1 instead of advancing.
    let target = if farm_loop { Some(inst_start) } else { next };
    if let Some(id) = target {
        state.pending_zone_change = Some(id);
    }
}

/// HELD by the player's hand (`regent_hand`): the character is off the board this
/// tick — no action of its own, and the combat scans skip it, so nothing reaches
/// it either. Returns true while the hold consumes the turn. The hold DROPS the
/// character in place once `held_until` arrives (or the moment they die), paying

/// A monster with nothing to fight WANDERS instead of standing on the spot.
///
/// NO PRNG. The direction is a hash of `(entity id, tick / WANDER_PERIOD)` — a draw here would
/// advance the one shared stream and change every downstream roll (drops, refine, cards) for what
/// is only an idle animation. Same input, same walk, on every machine.
///
/// Only re-aims once it has ARRIVED, so a wander is a short stroll and not a per-tick jitter.
///
/// BOTH teams, and the player half is the more visible fix: with no enemy the old code left
/// `target` on whatever cell was last set, which is the same cell for everyone, so the party
/// walked into one stack and stood there. The offset is keyed by entity id, so members spread
/// instead of converging without anyone needing to know about the others.
/// Pets never reach here — they run on `pet_step` and already drift to their owner.
/// Clamped inside the board with a 1-cell margin so nobody walks into the wall and sticks.
fn wander_step(state: &mut GameState, i: usize) {
    let e = &state.grid.entities[i];
    if !e.alive {
        return;
    }
    let id = e.id as u64;
    match e.team {
        // MONSTERS stroll: walk 2-5 cells, stand 3-5 s, repeat (user, 2026-08-06).
        Team::Monster => {
            // A leg the WANDER owns is marked with WANDER_WALKING. Anything else moving here is
            // a PURSUIT target left over from the enemy that just vanished — drop it on the spot
            // rather than finishing the walk, or a monster keeps marching to where the thief was
            // standing and looks like it can still see through the smoke (user, 2026-08-06).
            if e.wander_until != WANDER_WALKING && e.pos != e.target {
                let here = e.pos;
                state.grid.entities[i].target = here;
                state.grid.entities[i].wander_until =
                    state.tick + 30 + (wander_hash(id, state.tick) % 21);
                return;
            }
            if e.pos != e.target {
                return; // mid-stroll — let it finish the leg
            }
            if e.wander_until == WANDER_WALKING {
                // Arrived → stand a while. The PAUSE is the point: re-aiming on arrival makes it
                // walk continuously, which reads as patrolling rather than idling.
                state.grid.entities[i].wander_until =
                    state.tick + 30 + (wander_hash(id, state.tick) % 21); // 3.0-5.0 s
                return;
            }
            if state.tick < e.wander_until {
                return; // standing still, pause not up
            }
            // Pause over — pick the next leg. Seeded off `wander_until` so each leg differs (the
            // tick alone gives every monster that woke together one heading).
            let h = wander_hash(id, e.wander_until);
            let dist = 2 + (h % 4) as i32; // 2-5 cells
            let (dx, dy) = WANDER_DIRS[((h >> 8) % 8) as usize];
            let (w, hgt) = (state.grid.width, state.grid.height);
            let dest = Cell::new(
                (e.pos.x + dx * dist).clamp(1, (w - 2).max(1)),
                (e.pos.y + dy * dist).clamp(1, (hgt - 2).max(1)),
            );
            state.grid.entities[i].target = dest;
            state.grid.entities[i].wander_until = WANDER_WALKING;
        }
        // PLAYERS re-form instead of strolling. They pile onto one cell because they all chase
        // the SAME nearest enemy, converge on it, and stay where the last one died — so the fix
        // is to send them back into formation, not to make them wander (a member that drifts is
        // out of position when the next pack spawns).
        //
        // The anchor is the LEAD's cell, not the zone's spawn point: `tick()` takes no `GameData`
        // by design, so `zone.player_spawn` is not reachable from here. Using the lead also makes
        // this safe by construction — the lead's own offset is (0,0), so a party of one never
        // moves, which is exactly the walk-to-fixed-target contract golden_alpha2 pins.
        //
        // `party_cell` is the SAME formation used when a zone places the party, so idle spacing
        // and arrival spacing cannot drift apart.
        Team::Player => {
            if e.pos != e.target {
                return; // still walking into position
            }
            let mut lead = Cell::new(0, 0);
            let mut slot = usize::MAX;
            let mut seen = 0usize;
            for (k, o) in state.grid.entities.iter().enumerate() {
                if !matches!(o.team, Team::Player) || !o.alive || o.is_pet {
                    continue;
                }
                if seen == 0 {
                    lead = o.pos; // spawn order == the party's own order; first living = lead
                }
                if k == i {
                    slot = seen;
                }
                seen += 1;
            }
            if slot == usize::MAX {
                return;
            }
            let dest = crate::data::party_cell(lead, slot, state.grid.width, state.grid.height);
            state.grid.entities[i].target = dest;
        }
    }
}

/// Sentinel in `wander_until` meaning "the current `target` is a leg THIS wander chose".
/// Anything else moving with no enemy is chasing a target the vanished enemy left behind.
const WANDER_WALKING: u64 = u64::MAX;

/// The 8 compass steps a stroll can take.
const WANDER_DIRS: [(i32, i32); 8] =
    [(1, 0), (1, 1), (0, 1), (-1, 1), (-1, 0), (-1, -1), (0, -1), (1, -1)];

/// Spread `(id, seed)` into something that does not march adjacent ids in formation.
/// A hash, NEVER a PRNG draw: a draw would advance the one shared stream and change every
/// downstream roll (drops, refine, cards) to pay for an idle animation.
fn wander_hash(id: u64, seed: u64) -> u64 {
    id.wrapping_mul(0x9E37_79B9_7F4A_7C15)
        .rotate_left(23)
        ^ seed.wrapping_mul(0xBF58_476D_1CE4_E5B9)
}

/// the same landing cost a placed drop pays.
fn held_step(state: &mut GameState, i: usize) -> bool {
    if !state.grid.entities[i].is_held() {
        return false;
    }
    let tick = state.tick;
    if tick >= state.grid.entities[i].held_until || !state.grid.entities[i].alive {
        crate::events::drop_held_and_arm_cooldown(state, i, None);
        return false;
    }
    true
}

fn tick_one_entity(state: &mut GameState, i: usize) {
    // --- 0. Skill upkeep: cooldowns tick down, SP + passive HP regen ---
    // (Runs while HELD too — being in the player's hand costs actions, not time.)
    skill_upkeep(state, i);

    // --- 0.3. HELD by the player's hand (`regent_hand`): off the board, no
    // action. Checked before the cast channel so a lift can't be out-raced by a
    // windup, and before every try_cast_* branch below.
    if held_step(state, i) {
        return;
    }

    // --- 0.4. Cast-time channel (mage Variable Cast Time): a mid-cast entity
    // stands still — the spell windup IS its whole action — until the windup
    // elapses, then the stored spell resolves at the locked target. v1 has NO
    // damage interrupt (getting hit doesn't break the cast), but a dead caster
    // or a vanished target drops the channel.
    if state.grid.entities[i].casting.is_some() {
        if !state.grid.entities[i].alive {
            state.grid.entities[i].casting = None; // died mid-cast → drop it
            return;
        }
        let me_team = state.grid.entities[i].team;
        let (tid, slot) = {
            let c = state.grid.entities[i].casting.as_ref().unwrap();
            (c.target_id, c.slot)
        };
        // A pure-buff channel (Increase AGI) targets the CASTER — it stays valid while the
        // caster lives (checked above); a damage/field cast needs a living ENEMY target.
        let sid =
        state.grid.entities[i].skills.get(slot.get()).map(|s| s.id.clone()).unwrap_or_default();
        let is_buff = crate::skills::skill_def(&sid).map_or(false, |d| !d.cast_buff.is_empty());
        let target_alive = is_buff
            || state.grid.entities.iter().any(|e| e.id == tid && e.alive && e.team != me_team);
        // The target vanished (died / despawned). Rather than waste the windup, try to
        // RETARGET this same cast onto another valid foe (the skill's own targeting rule)
        // and keep channelling — "retarget before it whiffs". A pure buff never reaches
        // here (its target is the still-living caster). Only abandon if NOTHING is left.
        let retarget = if target_alive {
            Some(tid) // current target still valid — keep it
        } else {
            combat_step::retarget_current_cast(state, i, slot)
        };
        if let Some(new_tid) = retarget {
            // A STUNNED or FROZEN caster's channel holds (audit 2026-10-02): it counted down and
            // fired straight through a break stun or a Freeze.
            let held = crate::skills::buff_sum(&state.grid.entities[i], crate::skills::BuffKind::Stun) > 0
                || crate::skills::buff_sum(&state.grid.entities[i], crate::skills::BuffKind::Freeze) > 0;
            let c = state.grid.entities[i].casting.as_mut().unwrap();
            c.target_id = new_tid; // no-op when unchanged; swaps on a retarget (progress kept)
            if held {
                return;
            }
            c.ticks_left = c.ticks_left.saturating_sub(1);
            if c.ticks_left > 0 {
                return; // still channelling — stand still
            }
            fire_pending_cast(state, i); // windup elapsed → resolve now
            return;
        }
        state.grid.entities[i].casting = None; // no valid target anywhere → abandon, re-engage
    }

    // --- 0.4. Hiding (thief panic): auto-enter at the HP threshold, then take
    // NO action while hidden (the heal-over-time ticks in upkeep regardless).
    maybe_enter_hiding(state, i);
    if crate::skills::buff_sum(&state.grid.entities[i], crate::skills::BuffKind::Hiding) > 0 {
        // A hider whose vanish skill sets `move_while_hidden` keeps repositioning (walks toward its
        // last target) while untargetable — Twinling's stalk. The thief's panic Hiding holds still.
        if crate::skills::allows_move_while_hidden(&state.grid.entities[i].skills) {
            // Reappear on REACH: once we've closed to attack range of the stalk target, drop the
            // Hiding buff (become visible/targetable) and resume the normal attack flow next tick.
            let reached = {
                let e = &state.grid.entities[i];
                match e.target_id {
                    Some(tid) => {
                        let pos = e.pos;
                        let rng = combat::effective_atk_range(e);
                        state.grid.entities.iter()
                            .any(|t| t.id == tid && t.alive && t.pos.chebyshev(pos) <= rng)
                    }
                    None => false,
                }
            };
            if reached {
                state.grid.entities[i]
                    .buffs
                    .retain(|b| b.kind != crate::skills::BuffKind::Hiding);
                return;
            }
            move_step(state, i);
        }
        return;
    }

    // --- 0.45. Stun (Sonic Blow): immobilized — take NO action this turn (no
    // move, no attack). Still targetable + still takes damage; the buff just
    // decays (doesn't break on hit). Ages out in `decay_buffs` like any buff.
    if crate::skills::buff_sum(&state.grid.entities[i], crate::skills::BuffKind::Stun) > 0
        || crate::skills::buff_sum(&state.grid.entities[i], crate::skills::BuffKind::Freeze) > 0
    {
        return;
    }

    // --- 0.47. Motion delay (RO animation/motion lock): a skill that just
    // resolved locks the caster — no move / attack / next cast — until its
    // animation window elapses, so basics resume only AFTER the skill finishes.
    // (This is the lock that blocks basics; RO's ACD only gates the next skill.)
    if state.tick < state.grid.entities[i].act_locked_until {
        // The attack accumulator does NOT charge during the lock (0.0.481): the next swing winds
        // up after the skill's motion ends, so the renderer never draws a basic wind-up over a
        // skill (user 2026-09-26). Before, it charged to a full swing here and a basic fired the
        // tick the lock ended.
        return;
    }

    // --- 0.49. LANE MODE hold-fire (`lane.rs`; off unless `state.lane` is set): until the two sides
    // CRASH mid-field nobody attacks or casts, shooters included; they only walk.
    if state.lane.is_some() && !crate::lane::clashed(state) {
        let enemy = combat::find_enemy_idx(&state.grid.entities, i);
        if matches!(state.grid.entities[i].team, Team::Player) {
            crate::lane::party_move(state, i);
        } else {
            crate::lane::monster_move(state, i, enemy);
        }
        return;
    }

    // --- 0.5. SWING-SLOT GATE (0.0.481, user 2026-09-26: a skill cancelled the attack mid-swing).
    // Every cast below used to fire the tick its cooldown ended, whatever the swing was doing.
    // While engaged (`swing_engaged`) a cast now waits for the tick this swing would land and
    // takes that swing's place, the way `attack_step`'s skills always have. Off-target (walking,
    // no enemy) it fires at once, as before. Ember Cascade's queued Fireball is a reaction, not
    // an action, and is never gated.
    if state.grid.entities[i].alive {
        let engaged = swing_engaged(state, i);
        let aspd = combat::effective_atk_speed_milli(&state.grid.entities[i]);
        if engaged {
            open_windup(state, i, aspd);
        }
        let e = &state.grid.entities[i];
        if !engaged || (e.windup_cast && e.atk_acc.saturating_add(aspd) >= 1000) {
            if out_of_cycle_cast(state, i) {
                if engaged {
                    let e = &mut state.grid.entities[i];
                    e.atk_acc = e.atk_acc.saturating_add(aspd).saturating_sub(1000);
                }
                return;
            }
        } else {
            try_cast_break_trigger(state, i);
        }
    }

    // --- 1. Target selection ---
    let enemy_idx = combat::find_enemy_idx(&state.grid.entities, i);
    match enemy_idx {
        Some(eidx) => {
            let epos = state.grid.entities[eidx].pos;
            let eid = state.grid.entities[eidx].id;
            // Melee monsters pick a SURROUND CELL (free neighbor of
            // the enemy) instead of routing to the enemy's own tile.
            // Tick order is the determinism anchor: lower-id monsters
            // tick first and claim closer surround cells; later ones
            // see those positions and pick the next-best free
            // neighbor. When all 8 neighbors are taken, fall back to
            // the enemy's cell (stacking is allowed there).
            //
            // Ranged units don't need to surround — they want to stop
            // at `atk_range`, not glue themselves to a single tile,
            // and current BFS already gives them that.
            let is_melee_monster = matches!(state.grid.entities[i].team, Team::Monster)
                && state.grid.entities[i].atk_range <= 1;
            let move_target = if is_melee_monster && state.lane.is_none() {
                pick_surround_cell(&state.grid, i, epos).unwrap_or(epos)
            } else {
                epos
            };
            state.grid.entities[i].target = move_target;
            state.grid.entities[i].target_id = Some(eid);
        }
        None => {
            // No enemy: fall back to whatever `target` Cell was set
            // (ALPHA-2 walk-to-fixed-target behavior). Clear target_id.
            state.grid.entities[i].target_id = None;
            if state.lane.is_none() {
                wander_step(state, i);
            }
        }
    }

    // --- 2. Attack or move ---
    // Engage if the nearest enemy is in melee/attack range OR a ready
    // active skill can reach a valid target (e.g. Shadow Strike's
    // teleport reach lets the thief blink in from outside melee range).
    let in_combat = match enemy_idx {
        Some(eidx) => {
            let dist = state.grid.entities[i].pos
                .chebyshev(state.grid.entities[eidx].pos);
            dist <= combat::effective_atk_range(&state.grid.entities[i])
                || pick_active_skill(state, i, eidx).is_some()
        }
        None => false,
    };

    if in_combat {
        attack_step(state, i, enemy_idx.unwrap());
    } else if crate::skills::buff_sum(&state.grid.entities[i], crate::skills::BuffKind::Root) == 0 {
        // ROOT (Ranger Snare): movement locked — can't chase. (An in-range rooted
        // enemy still attacks via the branch above; root only blocks the step.)
        if state.lane.is_none() {
            move_step(state, i);
        } else if matches!(state.grid.entities[i].team, Team::Player) {
            crate::lane::party_move(state, i);
        } else {
            crate::lane::monster_move(state, i, enemy_idx);
        }
    }
}


/// A WIND-UP BEGINS on the tick the accumulator is below one tick's charge (just after a swing,
/// or on the step into reach): decide there, once, whether this wind-up may end in a skill —
/// only if one is ready NOW (0.0.483). One that comes ready mid-wind-up waits for the next swing,
/// so `pending_skill` never appears halfway through a basic and swaps the pose.
pub(crate) fn open_windup(state: &mut GameState, i: usize, aspd: u32) {
    let e = &state.grid.entities[i];
    if e.atk_acc >= aspd {
        return; // mid-wind-up: the decision stands
    }
    let ready = e.skills.iter().any(|s| {
        s.cooldown == 0
            && crate::skills::skill_def(&s.id).is_some_and(|d| {
                matches!(d.kind, crate::skills::SkillKind::Active) && (d.lb_max == 0 || e.limit_break >= d.lb_max)
            })
    });
    state.grid.entities[i].windup_cast = ready;
}

/// Is this entity mid-swing — the gate's "engaged"? A charge on the accumulator, OR an enemy in
/// reach (0.0.482). `atk_acc > 0` alone missed two cases: an ASPD that divides 1000 leaves it at
/// exactly 0 after every swing, and `move_step` zeroes it on the step into range — both let a
/// cast fire on the next tick and cut the strike's follow-through.
pub(crate) fn swing_engaged(state: &GameState, i: usize) -> bool {
    let e = &state.grid.entities[i];
    e.atk_acc > 0
        || combat::find_enemy_idx(&state.grid.entities, i).is_some_and(|j| {
            e.pos.chebyshev(state.grid.entities[j].pos) <= combat::effective_atk_range(e)
        })
}

/// The casts that are not a swing's skill (heals, party buffs, shields, ultimates, fields,
/// summons, traps), in priority order. True = one was cast and it is this entity's action.
/// Called behind the swing-slot gate in `tick_one_entity`, and on a CLONE by
/// `combat_step::preview_pending_skill` to name the cast the wind-up leads into.
pub(crate) fn out_of_cycle_cast(state: &mut GameState, i: usize) -> bool {
    // --- 0.5. Support cast (Acolyte Heal): if a ready heal skill finds an
    // injured ally in range, healing IS this entity's action — done before
    // (and instead of) attacking, and even when no enemy is around.
    // --- 0.48. Subvenio (Bishop emergency blink-heal): an ally under 40% HP gets
    // a teleport + Heal. Highest priority — fires before the normal heal.
    if state.grid.entities[i].alive && try_cast_subvenio(state, i) {
        return true;
    }
    if state.grid.entities[i].alive && try_cast_support(state, i) {
        return true;
    }
    // --- 0.52. Dupla Medicina (Bishop 2-target heal): when an ally is below 75%,
    // heal the 2 lowest map-wide. After single-target Heal so Heal leads.
    if state.grid.entities[i].alive && try_cast_dupla(state, i) {
        return true;
    }

    // --- 0.53. Fons Vitae is a PARTY BUFF + HoT since 0.0.468: the generic buff-cast path
    // owns it (gauge-gated there like every other ultimate), so there is no bespoke step here.

    // --- 0.54. Rivus (Astier): a 3-bounce heal down the party's worst-off, by percent.
    if state.grid.entities[i].alive && try_cast_rivus(state, i) {
        return true;
    }

    // --- 0.55. Purificatio (Bishop cleanse): strip level-gated debuffs off the
    // ally with the worst ailment, when off cooldown + a valid target exists.
    if state.grid.entities[i].alive && try_cast_purificatio(state, i) {
        return true;
    }

    // --- 0.6. Party-buff cast (Blessing / Increase AGI): an active party buff
    // that's off cooldown is cast proactively, even with no enemy present.
    if state.grid.entities[i].alive && try_cast_buffs(state, i) {
        return true;
    }

    // --- 0.62. Flame Shield (Elementalist self absorb-shield): bank the pool when
    // it's off cooldown and no shield is up. Counts as this turn.
    if state.grid.entities[i].alive && try_cast_flame_shield(state, i) {
        return true;
    }

    // --- 0.63. Praesidium Divinum (Bishop ally absorb-shield): shield the
    // lowest-HP% ally when off cooldown + an enemy is present. Counts as this turn.
    if state.grid.entities[i].alive && try_cast_praesidium(state, i) {
        return true;
    }

    // --- 0.635. Ember Cascade (Rapis passive): release one queued Fireball if the
    // pacing cooldown has run out. Never consumes the turn — it is an answer to
    // someone else's break, not her action.
    if state.grid.entities[i].alive {
        try_cast_break_trigger(state, i);
    }

    // --- 0.638. Fire Cat Meteor (Rapis ultimate): one instance on every living
    // enemy in the room. Counts as this turn.
    if state.grid.entities[i].alive && try_cast_room_nuke(state, i) {
        return true;
    }

    // --- 0.638. Zone-wide strike (Umbral Requiem, 0.0.426): every enemy on the board at
    // once, gauge-gated. Counts as this turn.
    if state.grid.entities[i].alive && try_cast_zone_wide(state, i) {
        return true;
    }

    // --- 0.639. Volley (Nightfall Volley, 0.0.423): one arrow on each of the N lowest-HP
    // enemies in range. Counts as this turn.
    if state.grid.entities[i].alive && try_cast_volley(state, i) {
        return true;
    }

    // --- 0.64. Aegis (Seraphine ultimate): shield the WHOLE party at once when
    // off cooldown + an enemy is present. Counts as this turn.
    if state.grid.entities[i].alive && try_cast_aegis(state, i) {
        return true;
    }

    // --- 0.65. Ground-field cast (Poison Cloud): place a persistent field when
    // it's off cooldown AND an enemy is already in range. Counts as this turn.
    if state.grid.entities[i].alive && try_cast_ground_field(state, i) {
        return true;
    }

    // --- 0.655. Summon cast (Broomhilda's Summon Bats): spawn familiars ringing the caster
    // when a summon skill is off cooldown and an enemy is present. Counts as this turn.
    if state.grid.entities[i].alive && try_cast_summon(state, i) {
        return true;
    }

    // --- 0.66. Trap placement (Ranger Land Mine / Claymore / Freezing): drop a
    // dormant trap on the 5×5 perimeter when a slotted trap skill is off cooldown,
    // under its cap, and a threat exists. Counts as this turn (a DPS tradeoff).
    if state.grid.entities[i].alive && try_place_trap(state, i) {
        return true;
    }
    false
}

#[cfg(test)]
mod tests;
