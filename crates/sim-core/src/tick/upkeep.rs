//! Per-tick upkeep systems — skill cooldown/regen, party auras, Endure,
//! Taunt. Run before entities act so the buffs are live this tick.

// DEBT (108 sites): this file predates `deny(clippy::indexing_slicing)`. An index that
// cannot be proven in range is a panic, and in verify-server (which links this crate and
// replays saves under `panic = "abort"`) a panic is the whole process. Shrink this, never
// add to it — a NEW file has no allow and is rejected on its first bare index.
#![allow(clippy::indexing_slicing)]

use super::*;
use crate::combat;
use crate::skills;
use crate::grid::{Cell, Team};
use crate::state::{GameState, GroundFieldKind};

/// Reserved `Buff.source_id` for party-aura buffs (cleared + re-stamped each
/// tick by `apply_party_auras`), so they never collide with a real entity id.
const AURA_SOURCE: u32 = u32::MAX;

/// Per-tick skill upkeep for one entity: tick down active cooldowns,
/// regen SP, and apply passive HP regen (HP Recovery). No-op for
/// entities without skills (every monster), so their state is untouched.
pub(crate) fn skill_upkeep(state: &mut GameState, i: usize) {
    if state.grid.entities[i].skills.is_empty() {
        return;
    }
    // Cooldowns count down every tick; HP regen only on the interval.
    for s in state.grid.entities[i].skills.iter_mut() {
        s.cooldown = s.cooldown.saturating_sub(1);
    }
    if !state.grid.entities[i].alive {
        return;
    }
    // HP Recovery on the SLOWER 5-second interval — bigger, less jittery heals. WHILE THE PARTY
    // MARCHES between rooms (the deploy grace) it ticks every second instead (0.0.398, user:
    // "ตอนเดินข้าม trigger regen"): the crossing is a breather, and a regen line is worth five
    // times as much on it — which is what makes the walk, and the line, worth having.
    let marching = state.zone_spawn.as_ref().is_some_and(|z| {
        z.opened_at_tick > 0 && state.tick.saturating_sub(z.opened_at_tick) < u64::from(z.deploy_grace_ticks)
    });
    let interval = if marching { crate::tick::HP_REGEN_MARCH_INTERVAL_TICKS } else { HP_REGEN_INTERVAL_TICKS };
    if state.tick % interval != 0 {
        return;
    }
    // Heal per 5-SECOND WINDOW = flat·Lv + MaxHP·bp/10000·Lv, taken straight
    // from the def (0.0.178 retune: the def values ARE the per-window amount —
    // the old code applied them per SECOND, ~10x RO's rate, and with no
    // out-of-combat gate to check it the tank out-sustained band mob DPS).
    let max_hp = combat::effective_max_hp(&state.grid.entities[i]);
    let mut heal = 0i32;
    for s in &state.grid.entities[i].skills {
        if let Some(def) = skills::skill_def(&s.id) {
            if matches!(def.kind, skills::SkillKind::Passive) {
                let lvl = s.level as i64;
                heal += def.hp_regen_flat * (s.level as i32);
                heal += ((max_hp as i64) * (def.hp_regen_maxhp_bp as i64) * lvl / 10_000) as i32;
            }
        }
    }
    // Flat regen lines (0.0.391, the Fortitude gem) — the same window, on top of any passive.
    heal = heal.saturating_add(combat::stat_mods::hp_regen_flat(&state.grid.entities[i]));
    if heal > 0 {
        let e = &mut state.grid.entities[i];
        let before = e.hp;
        e.hp = (e.hp + heal).min(max_hp);
        let (got, who) = (e.hp - before, e.id);
        crate::state::push_heal(state, who, who, got, crate::state::HealKind::Regen);
    }
}


/// Age every entity's buffs one tick (dropping expired ones) and apply any
/// PoisonDot damage on the regen interval. Called FIRST in the tick loop so a
/// 1-tick aura buff survives the same tick it's (re-)stamped and timed buffs
/// age exactly once. No PRNG — DoT routes lethal ticks through the normal
/// death path (drop + kill counter).
/// What is left of RO's freeze-resist term once MDEF and LUK were removed: a flat 97,
/// i.e. a freeze lands at 97% of the skill's own chance and the target has no say.
pub(crate) const FREEZE_RESIST_BASE: i64 = 97;

pub(crate) fn decay_buffs(state: &mut GameState) {
    let regen_now = state.tick % REGEN_INTERVAL_TICKS == 0;
    let count = state.grid.entities.len();
    for i in 0..count {
        if state.grid.entities[i].buffs.is_empty() {
            continue;
        }
        if regen_now && state.grid.entities[i].alive {
            let mut dot: i32 = state.grid.entities[i]
                .buffs
                .iter()
                .filter(|b| matches!(b.kind, skills::BuffKind::PoisonDot | skills::BuffKind::Bleed | skills::BuffKind::LeechBleed))
                .map(|b| b.magnitude.max(1))
                .fold(0i32, |a, m| a.saturating_add(m));
            // HEARTSTRING (0.0.451): a leeching DoT heals its source for `LEECH_BP` of each
            // tick. Collected here, paid after the victim's HP moves — the source may be the
            // victim's killer this very tick and must still be healed.
            let leech: Vec<(u32, i32)> = state.grid.entities[i]
                .buffs
                .iter()
                .filter(|b| matches!(b.kind, skills::BuffKind::LeechBleed))
                .map(|b| (b.source_id, (i64::from(b.magnitude.max(1)) * i64::from(crate::tick::LEECH_BP) / 10_000) as i32))
                .collect();
            // Burn (Combustion) is a LINEAR-DECAY DoT: `magnitude` holds the PEAK
            // and the value falls linearly with the time left, so this interval
            // deals peak × ticks_left / BURN_DURATION — hottest right after the hit,
            // fading to 0 at the end of the window (peak 100 reads 50 at the 5 s mark).
            for b in state.grid.entities[i].buffs.iter() {
                if matches!(b.kind, skills::BuffKind::Burn) {
                    let v = b.magnitude as i64 * b.ticks as i64
                        / crate::tick::BURN_DURATION_TICKS as i64;
                    dot = dot.saturating_add(v.max(1) as i32);
                }
            }
            // CREDIT the meter (0.0.451 — DoTs were never counted: Rend, Shadowbind's bleed
            // and Burn read as nothing on the breakdown). Each instance to its source, capped
            // at the HP it actually took, under a per-kind label.
            let credits: Vec<(u32, &'static str, i32)> = state.grid.entities[i]
                .buffs
                .iter()
                .filter_map(|b| match b.kind {
                    skills::BuffKind::Bleed => Some((b.source_id, "Bleed", b.magnitude.max(1))),
                    skills::BuffKind::PoisonDot => Some((b.source_id, "Poison", b.magnitude.max(1))),
                    skills::BuffKind::LeechBleed => Some((b.source_id, "heartstring", b.magnitude.max(1))),
                    skills::BuffKind::Burn => Some((b.source_id, "Burn", (b.magnitude as i64 * b.ticks as i64 / crate::tick::BURN_DURATION_TICKS as i64).max(1) as i32)),
                    _ => None,
                })
                .collect();
            if dot > 0 {
                let pre_hp = state.grid.entities[i].hp.max(0);
                state.grid.entities[i].hp = state.grid.entities[i].hp.saturating_sub(dot);
                let mut left = pre_hp;
                for (src, label, amt) in credits {
                    let taken = amt.min(left);
                    left -= taken;
                    if let Some(si) = state.grid.idx_of_id(src) {
                        state.record_damage(si, Some(label), taken);
                    }
                }
                if state.grid.entities[i].hp <= 0 {
                    state.grid.entities[i].alive = false;
                    kill_monster(state, i, None);
                }
            }
            for (src, heal) in leech {
                if heal <= 0 {
                    continue;
                }
                if let Some(si) = state.grid.idx_of_id(src) {
                    if state.grid.entities[si].alive {
                        let max = combat::effective_max_hp(&state.grid.entities[si]);
                        let e = &mut state.grid.entities[si];
                        let before = e.hp;
                        e.hp = (e.hp + heal).min(max);
                        let got = e.hp - before;
                        if got > 0 {
                            crate::state::push_heal(state, src, src, got, crate::state::HealKind::Regen);
                        }
                    }
                }
            }
            // Classic Poison (Poison Cloud): lose 1.5%·MaxHP + 2 per interval, but
            // ONLY while HP > 25% MaxHP and FLOORED there — it can't kill and never
            // routes the death path (unlike PoisonDot). Marker buff (presence).
            if state.grid.entities[i].alive
                && state.grid.entities[i]
                    .buffs
                    .iter()
                    .any(|b| matches!(b.kind, skills::BuffKind::Poison))
            {
                let maxhp = combat::effective_max_hp(&state.grid.entities[i]);
                let floor = maxhp / 4; // 25% — poison never drops the victim below this
                let e = &mut state.grid.entities[i];
                if e.hp > floor {
                    let dmg = (maxhp as i64 * 150 / 10_000) as i32 + 2; // 1.5% + 2
                    e.hp = (e.hp - dmg).max(floor);
                }
            }
            // Heal-over-time (Rejuvenating Potion) — restore HP per interval,
            // capped at max. Applied AFTER the DoT (a HoT can out-heal a poison).
            if state.grid.entities[i].alive {
                let hot: i32 = state.grid.entities[i]
                    .buffs
                    .iter()
                    .filter(|b| matches!(b.kind, skills::BuffKind::HealHot))
                    .map(|b| b.magnitude.max(0))
                    .fold(0i32, |a, m| a.saturating_add(m));
                // S6 (0.0.469, Astier): the tide's ticks carry her damage buff too.
                let hot_srcs: Vec<u32> = state.grid.entities[i]
                    .buffs
                    .iter()
                    .filter(|b| matches!(b.kind, skills::BuffKind::HealHot) && b.magnitude > 0)
                    .map(|b| b.source_id)
                    .collect();
                for src in hot_srcs {
                    if let Some(hi) = state.grid.idx_of_id(src) {
                        if crate::unit_rank::heal_buff_everywhere(&state.grid.entities[hi]).is_some() {
                            crate::tick::combat_step::healed_ally_buff(state, hi, i);
                        }
                    }
                }
                if hot > 0 {
                    let maxhp = combat::effective_max_hp(&state.grid.entities[i]);
                    let e = &mut state.grid.entities[i];
                    let before = e.hp;
                    e.hp = (e.hp + hot).min(maxhp);
                    let (got, who) = (e.hp - before, e.id);
                    crate::state::push_heal(state, who, who, got, crate::state::HealKind::Hot);
                }
            }
        }
        state.grid.entities[i].buffs.retain_mut(|b| {
            b.ticks = b.ticks.saturating_sub(1);
            b.ticks > 0
        });
    }
}

/// Source id stamped on god-mode buffs so they refresh (not stack) each tick.
///
/// NOT `u32::MAX`: that is `AURA_SOURCE`, and `apply_party_auras` runs BEFORE the entity actions
/// and unconditionally strips everything carrying it. God mode's damage buff was therefore wiped
/// the tick after it was stamped, every tick — god delivered invincibility and no cooldowns but
/// never the damage, while the comment claimed a one-shot next tick. Debug-only, so nothing in
/// play or in a golden ever saw it.
const GOD_BUFF_SOURCE: u32 = 9_300_001;
/// God-mode ATK/MATK bonus (basis points) — +100000% ⇒ one-shots everything.
const GOD_DMG_PCT: i32 = 1_000_000;

/// DEBUG god mode: every tick, top each player up (revive + full HP/SP, all
/// cooldowns cleared) and refresh a huge ATK/MATK buff so the party is
/// invincible, never SP/cd-starved, and one-shots foes. No-op (early return)
/// unless `debug_god` is set → normal play / goldens are byte-identical.
pub(crate) fn apply_debug_god(state: &mut GameState) {
    if !state.debug_god {
        return;
    }
    let count = state.grid.entities.len();
    for i in 0..count {
        if !matches!(state.grid.entities[i].team, Team::Player) {
            continue;
        }
        let maxhp = combat::effective_max_hp(&state.grid.entities[i]);
        let e = &mut state.grid.entities[i];
        e.alive = true;
        e.hp = maxhp;
        for s in e.skills.iter_mut() {
            s.cooldown = 0;
        }
        for kind in [skills::BuffKind::AtkPct] {
            match e.buffs.iter_mut().find(|b| b.kind == kind && b.source_id == GOD_BUFF_SOURCE) {
                Some(b) => b.ticks = 2,
                None => e.buffs.push(skills::Buff {
                    kind,
                    magnitude: GOD_DMG_PCT,
                    ticks: 2,
                    source_id: GOD_BUFF_SOURCE,
                }),
            }
        }
    }
}

/// Apply or refresh a classic `Poison` buff on `e` for `ticks` (staying in a
/// cloud refreshes the timer; leaving lets it lapse). No PRNG. Shared by ground
/// fields (Poison Cloud) and on-death toxic blasts (`toxic_burst`).
pub(crate) fn apply_or_refresh_poison(e: &mut crate::grid::Entity, ticks: u32) {
    // THROUGH `apply_buff`, not a raw push. The status-resist duration cut lives inside it and
    // nowhere else, so pushing directly meant `ResistPoisonBp` never applied to the two sources
    // that actually inflict classic Poison — Poison Cloud fields and toxic_burst. A Field Ration
    // advertises "Poison wears off 30% faster" (item_desc.gd) and it was never true against
    // either of them; only the card proc, which does route through `apply_buff`, was resisted.
    //
    // REFRESH-TO-LONGEST is preserved by hand: `apply_buff` replaces outright, so standing in a
    // cloud must not be able to SHORTEN a longer poison already ticking.
    let live = e
        .buffs
        .iter()
        .find(|b| matches!(b.kind, skills::BuffKind::Poison) && b.source_id == 0)
        .map(|b| b.ticks)
        .unwrap_or(0);
    skills::apply_buff(
        e,
        skills::Buff { kind: skills::BuffKind::Poison, magnitude: 1, ticks, source_id: 0 },
    );
    if let Some(b) = e
        .buffs
        .iter_mut()
        .find(|b| matches!(b.kind, skills::BuffKind::Poison) && b.source_id == 0)
    {
        b.ticks = b.ticks.max(live);
    }
}

/// Age + apply persistent ground fields. On the regen interval a field applies
/// its effect to every victim-team entity within its radius (bosses are immune
/// to Poison); every tick all fields count down and expired ones drop. No PRNG;
/// no-op (early return) for the field-less common case → goldens byte-identical.
/// Self-heal Exemplar DEFINITIONS + skill grants every tick (persistent-entity rule).
/// The equipped Exemplar is SERIALIZED into the save at equip time, so a catalog change
/// (e.g. Seraphine gaining Fiat Lux) would never reach an already-equipped copy: refresh
/// the stored definition from the CODE CATALOG by id (keeping the earned `stars` — the
/// save's only authoritative Exemplar fact), then re-apply grants. `apply_exemplar_skill`
/// is idempotent + re-levels in place; the `is_some` gate keeps every exemplar-less state
/// (all goldens) zero-cost. Living in `tick()` makes it replay-safe by construction —
/// every runtime heals the same way at the same tick. An id no longer in the catalog
/// keeps its stored definition (never brick a save).
/// Expire unresolved (dud) rare candidates past their TTL — the server never sends
/// duds (zero-byte convention), so the client prunes them by age. Deterministic
/// (tick-based), gated on non-empty so candidate-less states cost nothing.
///
/// CRAFT and GEAR candidates never expire. The TTL exists because most DROP candidates are duds
/// that would otherwise accumulate forever; a craft is not a dud, it is an item the
/// player already paid for, and ageing it out would delete that item on any player who
/// crafts and then loses their connection past 8 h. Gear is the same: the piece already
/// dropped, so ageing out its ticket would delete loot the player earned. Both stay bounded by
/// their own caps (`CRAFT_CANDIDATE_CAP`, `GEAR_CANDIDATE_CAP`), and both are refused at the
/// cap rather than accrued and then silently dropped.
pub(crate) fn expire_rare_candidates(state: &mut GameState) {
    if state.rare_candidates.is_empty() {
        return;
    }
    let cutoff = state.tick.saturating_sub(crate::drops::RARE_CANDIDATE_TTL_TICKS);
    state.rare_candidates.retain(|c| c.tick > cutoff || !c.kind.is_drop());
}

pub(crate) fn heal_exemplar_grants(state: &mut GameState) {
    // The overlay is now LEAN (id/stars) with stats recomputed from the def every tick, so there is
    // no stored stat grant to go stale — only the granted SKILLS live on the entity (SkillState).
    // Re-apply them idempotently so a save keeps its Exemplar/Regalia skills after a version skew.
    for e in state.grid.entities.iter_mut() {
        if e.exemplar.is_some() {
            e.apply_exemplar_skill();
        }
        if e.regalia.is_some() {
            e.apply_regalia_skill();
        }
    }
}

pub(crate) fn tick_ground_fields(state: &mut GameState) {
    if state.ground_fields.is_empty() {
        return;
    }
    // Iterate fields by INDEX (kind is Copy, so we read params without holding a
    // borrow, then mutate entities inside). Field count is stable until the prune
    // below, so a once-per-target field can write back its new hit_ids by index.
    // Poison Cloud applies on the slow REGEN interval; a Damage field fires on its
    // OWN `hit_interval` (Vermilion: every tick, once per target).
    let on_regen = state.tick % REGEN_INTERVAL_TICKS == 0;
    let field_count = state.ground_fields.len();
    for idx in 0..field_count {
        // AURA field: re-centre on the carrier's CURRENT cell while it lives (the field
        // freezes where it was if the carrier died — it still burns out its duration).
        let follow = state.ground_fields[idx].follow_id;
        if follow != 0 {
            if let Some(ei) = state.grid.idx_of_id(follow) {
                if state.grid.entities[ei].alive {
                    state.ground_fields[idx].center = state.grid.entities[ei].pos;
                }
            }
        }
        let (center, radius, victim_team, kind, source_id, ticks_left, skill_id) = {
            let f = &state.ground_fields[idx];
            (f.center, f.radius, f.victim_team, f.kind, f.source_id, f.ticks_left, f.skill_id.clone())
        };
        match kind {
            GroundFieldKind::PoisonCloud { poison_ticks } => {
                if !on_regen {
                    continue;
                }
                for e in state.grid.entities.iter_mut() {
                    if !e.on_board() || e.team != victim_team || e.is_boss {
                        continue; // bosses are immune to Poison; a held member is off the board
                    }
                    if e.pos.chebyshev(center) <= radius {
                        apply_or_refresh_poison(e, poison_ticks);
                    }
                }
            }
            GroundFieldKind::Damage { amount, element, status, status_bp, status_dur, status_mag, hit_interval, status_at, knockback, impact_radius, strike_hits, once_per_target, fuse, pull } => {
                // BLACK HOLE (0.0.451): on every hit interval, haul each victim within reach
                // `pull` cells toward the centre and hold it there for the interval (Root) —
                // so the crowd arrives over the hole's life and stays for the burst. The pull
                // reaches `radius + pull` so the ring just outside is drawn in too. Bosses are
                // hauled but not rooted, as with every field CC. No draw.
                if pull > 0 && ticks_left % hit_interval.max(1) == 0 {
                    let reach = radius + pull;
                    let victims: Vec<usize> = state
                        .grid
                        .entities
                        .iter()
                        .enumerate()
                        .filter(|(_, e)| e.alive && e.on_board() && e.team == victim_team && e.pos.chebyshev(center) <= reach)
                        .map(|(j, _)| j)
                        .collect();
                    for j in victims {
                        if state.grid.entities[j].pos != center {
                            if state.lane.is_some() {
                                crate::lane::pull_onto(state, j, center);
                            } else {
                                crate::tick::combat_step::pull_toward(state, j, center, pull);
                            }
                        }
                        if !state.grid.entities[j].is_boss {
                            crate::skills::apply_buff(&mut state.grid.entities[j], crate::skills::Buff {
                                kind: crate::skills::BuffKind::Root, magnitude: 1, ticks: hit_interval.max(1) + 1, source_id: source_id,
                            });
                        }
                    }
                }
                // A FUSED field (a planted bomb) is inert until the tick it expires;
                // everything else burns on its own interval from the moment it lands.
                let fires = if fuse { ticks_left <= 1 } else { ticks_left % hit_interval.max(1) == 0 };
                if fires {
                    // once-per-target: pass the victims this field already hit so they
                    // are skipped, then record the new ones it hits this tick.
                    let already: Vec<u32> = if once_per_target {
                        state.ground_fields[idx].hit_ids.clone()
                    } else {
                        Vec::new()
                    };
                    let newly = tick_damage_field(state, center, radius, victim_team, source_id,
                        amount, element, status, status_bp, status_dur, status_mag, status_at, knockback,
                        impact_radius, strike_hits, once_per_target, &already, &skill_id);
                    if once_per_target {
                        state.ground_fields[idx].hit_ids.extend(newly);
                    }
                }
            }
        }
    }
    for f in state.ground_fields.iter_mut() {
        f.ticks_left = f.ticks_left.saturating_sub(1);
    }
    state.ground_fields.retain(|f| f.ticks_left > 0);
}

/// One interval of an Elementalist DAMAGE field (Glacial Storm / Vermilion /
/// Quicksand): hit every victim-team entity in range for `amount` (snapshot MATK·
/// power, element-adjusted + MDEF via `apply_damage`), credit kills/DPS to the
/// caster, then roll the field's `status` (Freeze/Blind/Slow) at `status_bp`.
/// Bosses take damage but resist the field CC. PRNG only runs for a status field
/// (Elementalist-only) so goldens are unaffected.
#[allow(clippy::too_many_arguments)]
fn tick_damage_field(
    state: &mut GameState, center: Cell, radius: i32, victim_team: Team, source_id: u32,
    amount: i32, element: crate::grid::Element, status: Option<crate::skills::BuffKind>,
    status_bp: u32, status_dur: u32, status_mag: i32, status_at: u32, knockback: i32,
    impact_radius: i32, strike_hits: u32, once_per_target: bool, already: &[u32],
    skill_id: &str,
) -> Vec<u32> {
    use crate::skills::BuffKind;
    // Victims this call hit — the caller records them in the field's `hit_ids`
    // when the field is once-per-target (Lord of Vermilion). Empty for the rest.
    let mut hit_now: Vec<u32> = Vec::new();
    const DIRS: [(i32, i32); 8] =
        [(-1, -1), (0, -1), (1, -1), (-1, 0), (1, 0), (-1, 1), (0, 1), (1, 1)];
    let (gw, gh) = (state.grid.width, state.grid.height);
    let src_idx = state.grid.idx_of_id(source_id);
    // METEOR mode: this interval's ONE meteor lands at a random cell within the
    // target area (`radius`), and only cells within `impact_radius` of it are hit
    // (so the random drop can miss). 2 PRNG draws — gated on a meteor field (which
    // only Elementalist places) so goldens are unaffected. Normal fields keep the
    // whole-field center/radius with no extra draw.
    let (eff_center, eff_radius) = if impact_radius > 0 {
        let dx = state.rng.gen_range_u32(0, (2 * radius + 1) as u32) as i32 - radius;
        let dy = state.rng.gen_range_u32(0, (2 * radius + 1) as u32) as i32 - radius;
        let c = Cell::new((center.x + dx).clamp(0, gw - 1), (center.y + dy).clamp(0, gh - 1));
        (c, impact_radius)
    } else {
        (center, radius)
    };
    let hits = strike_hits.max(1);
    // Surface the meteor's landing cell so the renderer can flash the 5×5 impact
    // (even a miss). Display-only, cleared each tick.
    if impact_radius > 0 {
        state.meteor_impacts.push(crate::state::BurstFx {
            x: eff_center.x,
            y: eff_center.y,
            radius: impact_radius,
            element: crate::grid::Element::Fire,
        });
    }
    let count = state.grid.entities.len();
    for j in 0..count {
        {
            let e = &state.grid.entities[j];
            // A HELD member (`regent_hand`) is off the board — the field sweeps
            // under them. This is the whole point of the pick-up: dodging the
            // telegraph, at the cost of that character's uptime.
            if !e.on_board()
                || e.team != victim_team
                || e.pos.chebyshev(eff_center) > eff_radius
            {
                continue;
            }
            // ONCE-PER-TARGET (Vermilion): skip a victim THIS field already hit —
            // but a NEW victim (or another field's `hit_ids`) is still eligible.
            if once_per_target && already.contains(&e.id) {
                continue;
            }
        }
        if amount > 0 {
            // 0.0.472: a field tick crits off its caster, like every other skill hit.
            let (amount, is_crit) = match src_idx {
                Some(si) => crate::combat::roll_source_crit(&mut state.rng, &state.grid.entities[si], amount),
                None => (amount, false),
            };
            let dmg = crate::combat::Damage {
                atk_level: 1, amount,
                element,
                kind: crate::grid::DamageType::Magic,
                is_ranged: true,
                is_crit,
                is_aoe: true,
                mastery_bp: 0, shadow_bane_bp: 0, debuff_amp_bp: 0,
            atk_race: crate::grid::Race::Unknown,
            atk_size: crate::grid::Size::Medium,
            ignore_def_bp: 0,
            };
            // ONE damage instance per strike (DEF/MDEF subtracted ONCE). `amount`
            // already bundles a meteor's per-hit × hit-count, so a meteor is a
            // VISUAL multi-hit: one packet shown as `hits` numbers + a yellow total
            // (user's call — 1 meteor = 1 instance). Normal fields have hits == 1.
            let was_alive = state.grid.entities[j].alive;
            let pre_hp = state.grid.entities[j].hp.max(0);
            let mob = crate::combat::attackers_on(&state.grid.entities, j);
            let dealt = crate::combat::apply_damage(&state.grid.entities[j], &dmg, mob);
            if matches!(state.grid.entities[j].team, crate::grid::Team::Player) {
                state.grid.entities[j].mitigated_fx += i64::from(dmg.amount) - i64::from(dealt);
            }
            // Field damage now ALSO shatters Freeze (take_hit) — only DoT/poison
            // leaves it intact. (Previously frozen victims were skipped entirely.)
            state.grid.entities[j].take_hit(dealt);
            if let Some(si) = src_idx {
                let attrib = if skill_id.is_empty() { None } else { Some(skill_id) };
                state.record_damage(si, attrib, dealt.min(pre_hp));
                // A field tick is a landed hit (0.0.451): it stacks the caster's mark (Sweet
                // Poison) and, on the hole's close, leaves the S6 vulnerability behind.
                if dealt > 0 {
                    crate::combat::stamp_mark(&mut state.grid.entities, si, j);
                    if state.grid.entities[j].alive {
                        if let Some((vuln, ticks)) = crate::unit_rank::nuke_vuln(&state.grid.entities[si], skill_id) {
                            crate::skills::apply_buff(&mut state.grid.entities[j], crate::skills::Buff {
                                kind: crate::skills::BuffKind::DmgTakenPct, magnitude: vuln, ticks, source_id,
                            });
                        }
                    }
                }
            }
            // Surface the strike for the renderer; `hits` makes a meteor read as a
            // `hits`-number burst + yellow total (a per-interval field tick = 1).
            state.field_hits.push(crate::state::HitEvent {
                target_id: state.grid.entities[j].id,
                attacker_id: source_id,
                amount: dealt,
                hits,
                kind: if is_crit { crate::state::HitKind::Crit } else { crate::state::HitKind::Normal },
            });
            // Mark this victim as struck by the field (used only when once-per-target).
            hit_now.push(state.grid.entities[j].id);
            if was_alive && !state.grid.entities[j].alive {
                kill_monster(state, j, src_idx);
            }
        }
        // Random knockback (Storm Gust scatter): shove the victim 1+ cells in a
        // random of 8 directions (clamped to the grid). PRNG only runs for a
        // knockback field → goldens unaffected.
        if knockback > 0 && state.grid.entities[j].alive {
            let (dx, dy) = DIRS[state.rng.gen_range_u32(0, 8) as usize];
            let mut p = state.grid.entities[j].pos;
            for _ in 0..knockback {
                let (nx, ny) = (p.x + dx, p.y + dy);
                if nx < 0 || ny < 0 || nx >= gw || ny >= gh {
                    break;
                }
                p = crate::grid::Cell::new(nx, ny);
            }
            let e = &mut state.grid.entities[j];
            e.pos = p;
            e.target = p;
            e.move_acc = 0;
        }
        if let Some(kind) = status {
            if state.grid.entities[j].alive && !state.grid.entities[j].is_boss {
                // RO Storm Gust freeze: EVERY hit rolls the field's level-scaled
                // BaseChance; every `status_at`-th hit THIS victim takes, the base is
                // BOOSTED to 150% (still rollable — a huge-MDEF foe can resist even
                // that). `field_hit_count` is per-victim; a FROZEN victim is skipped
                // above so it only counts hits actually taken (≈ "N hits since the
                // last freeze broke"). status_at 0 = no special hit.
                let special = status_at > 0 && {
                    let c = state.grid.entities[j].field_hit_count.saturating_add(1);
                    state.grid.entities[j].field_hit_count = c;
                    c as u32 % status_at == 0
                };
                let base_bp = if special { 15_000 } else { status_bp };
                // FREEZE carries what is left of the RO resist: `BaseChance x
                // FREEZE_RESIST_BASE/100`. The rest of that formula read MDEF and LUK,
                // and both are gone (0.0.85-87 / 0.0.317), so nothing about the TARGET
                // changes its chance any more — Elemental RES% takes this job when it
                // lands. Other field statuses (Blind / Slow) use the raw chance.
                let (eff_bp, eff_dur) = if kind == BuffKind::Freeze {
                    ((base_bp as i64 * FREEZE_RESIST_BASE as i64 / 100) as u32, status_dur)
                } else {
                    (base_bp, status_dur)
                };
                if eff_bp > 0 && state.rng.gen_range_u32(0, 10_000) < eff_bp {
                    crate::skills::apply_buff(
                        &mut state.grid.entities[j],
                        crate::skills::Buff { kind, magnitude: status_mag.max(1), ticks: eff_dur.max(1), source_id },
                    );
                }
            }
        }
    }
    hit_now
}

/// ENRAGE (0.0.430): a monster that has stood on the board for `ENRAGE_GRACE_TICKS` gains
/// `ENRAGE_STEP_BP` ATK every `ENRAGE_STEP_TICKS` after that, without limit — a fight that drags
/// gets harder, never stalls (user: "+20% ทุก 15 วิ หลัง 30 วิ, stack ขึ้นเรื่อยๆ"). One `AtkPct`
/// buff per monster, sourced from itself, magnitude grown in place; it dies with the monster.
/// Bosses and their adds enrage alike. No PRNG draw.
pub const ENRAGE_GRACE_TICKS: u32 = 300;
pub const ENRAGE_STEP_TICKS: u32 = 150;
pub const ENRAGE_STEP_BP: i32 = 2_000;

pub(crate) fn tick_enrage(state: &mut GameState) {
    for e in state.grid.entities.iter_mut() {
        if !matches!(e.team, crate::grid::Team::Monster) || !e.alive || e.is_pet || !e.on_board() {
            continue;
        }
        e.enrage_ticks = e.enrage_ticks.saturating_add(1);
        if e.enrage_ticks < ENRAGE_GRACE_TICKS || (e.enrage_ticks - ENRAGE_GRACE_TICKS) % ENRAGE_STEP_TICKS != 0 {
            continue;
        }
        let me = e.id;
        if let Some(b) = e.buffs.iter_mut().find(|b| matches!(b.kind, crate::skills::BuffKind::AtkPct) && b.source_id == me) {
            b.magnitude = b.magnitude.saturating_add(ENRAGE_STEP_BP);
            b.ticks = u32::MAX / 2;
        } else {
            e.buffs.push(crate::skills::Buff { kind: crate::skills::BuffKind::AtkPct, magnitude: ENRAGE_STEP_BP, ticks: u32::MAX / 2, source_id: me });
        }
    }
}

/// Detonate any Flame Shield whose absorb pool just emptied (`absorb_burst_pending`).
/// A 3×3 (radius 1) 100% ATK Fire blast on surrounding enemies + a knockback,
/// credited to the shield's owner. Gated on the pending flag — only an Elementalist
/// who ran a shield dry triggers it, so non-owners (every golden) are untouched.
pub(crate) fn resolve_absorb_bursts(state: &mut GameState) {
    use crate::grid::{DamageType, Element};
    let count = state.grid.entities.len();
    for k in 0..count {
        if state.grid.entities[k].pending_bursts.is_empty() {
            continue;
        }
        let bursts = std::mem::take(&mut state.grid.entities[k].pending_bursts);
        crate::regalia_passive::on_shield_break(state, k);
        for b in &bursts {
            crate::unit_rank::on_shield_break(state, b.src_id, &b.source_skill_id);
        }
        let center = state.grid.entities[k].pos;
        let me_team = state.grid.entities[k].team;
        for b in bursts {
            let amount = b.burst_dmg;
            let knock = b.burst_knock;
            let src_id = b.src_id;
            // The burst is credited to the skill that PLACED the shield (Aegis bursts were
            // reported as Flame Shield until 0.0.427 and hid 28% of the party's damage).
            let burst_skill: String = if b.source_skill_id.is_empty() { "flame_shield".into() } else { b.source_skill_id.clone() };
            // Credit the blast to the shield's caster (Flame Shield = the holder).
            let src_idx = state.grid.idx_of_id(src_id).unwrap_or(k);
            if amount <= 0 {
                continue;
            }
            // The shield just BROKE → start its Flame Shield cooldown NOW (the cd
            // runs from break, not cast), so there's real downtime before re-shield.
            if let Some(d) = crate::skills::skill_def("flame_shield").filter(|_| burst_skill == "flame_shield") {
                let cd = crate::combat::stat_mods::scale_cooldown(&state.grid.entities[src_idx], d.cooldown_ticks);
                if let Some(s) = state.grid.entities[src_idx]
                    .skills
                    .iter_mut()
                    .find(|s| s.id == "flame_shield")
                {
                    s.cooldown = cd;
                }
            }
            // Orange 3×3 flash for the renderer (reuse the meteor-impact channel).
            state.meteor_impacts.push(crate::state::BurstFx { x: center.x, y: center.y, radius: 1, element: crate::grid::Element::Fire });
            for j in 0..count {
                if j == k {
                    continue;
                }
            {
                let e = &state.grid.entities[j];
                if !e.alive || e.team == me_team || e.pos.chebyshev(center) > 1 {
                    continue;
                }
            }
            // 0.0.472: a burst crits off the shield's caster.
            let (amount, is_crit) = crate::combat::roll_source_crit(&mut state.rng, &state.grid.entities[src_idx], amount);
            let dmg = crate::combat::Damage {
                atk_level: 1, amount,
                element: Element::Fire,
                kind: DamageType::Magic,
                is_ranged: true,
                is_crit,
                is_aoe: true,
                mastery_bp: 0, shadow_bane_bp: 0, debuff_amp_bp: 0,
            atk_race: crate::grid::Race::Unknown,
            atk_size: crate::grid::Size::Medium,
            ignore_def_bp: 0,
            };
            let mob = crate::combat::attackers_on(&state.grid.entities, j);
            let pre_hp = state.grid.entities[j].hp.max(0);
            let was_alive = state.grid.entities[j].alive;
            let dealt = crate::combat::apply_damage(&state.grid.entities[j], &dmg, mob);
            if matches!(state.grid.entities[j].team, crate::grid::Team::Player) {
                state.grid.entities[j].mitigated_fx += i64::from(dmg.amount) - i64::from(dealt);
            }
            state.grid.entities[j].take_hit(dealt); // hp − dmg, shatter Freeze, flag death
            state.record_damage(src_idx, Some(&burst_skill), dealt.min(pre_hp));
            state.field_hits.push(crate::state::HitEvent {
                target_id: state.grid.entities[j].id,
                attacker_id: src_id,
                amount: dealt,
                hits: 1,
                kind: if is_crit { crate::state::HitKind::Crit } else { crate::state::HitKind::Normal },
            });
            // Knock the (surviving) victim away from the blast centre (the owner `k`) —
            // the shared helper (angle-snap + cell_is_free collision).
            if knock > 0 && state.grid.entities[j].alive {
                crate::tick::combat_step::knockback_entity(state, k, j, knock);
            }
            if was_alive && !state.grid.entities[j].alive {
                kill_monster(state, j, Some(src_idx));
            }
            }
        }
    }
}

/// Refresh the party-wide auras each tick (before anyone acts):
///   1. The legacy flat-ATK aura (`party_atk_per_level` → `aura_atk`).
///   2. Aura BUFFS — any PASSIVE skill carrying `cast_buff` specs (e.g. Angelus
///      DefPct+VitAdd) projects them to every living ally as `ticks = 1` buffs,
///      re-stamped each tick (self-expire, like the ATK aura). The strongest
///      magnitude per `BuffKind` across the party covers everyone (no stacking).
///
/// Active timed party buffs (Blessing / Increase AGI) are NOT here — they're
/// cast on cooldown via `try_cast_buffs`. A party with neither an ATK aura nor
/// an aura-buff passive leaves every entity's `buffs` untouched (empty stays
/// empty), so the state hash is unchanged.
pub(crate) fn apply_party_auras(state: &mut GameState) {
    let mut atk_aura = 0i32;
    let mut aura_buffs: Vec<(skills::BuffKind, i32)> = Vec::new();
    for e in &state.grid.entities {
        if !(e.alive && matches!(e.team, Team::Player)) {
            continue;
        }
        atk_aura = atk_aura.max(skills::party_atk_bonus(&e.skills));
        for s in &e.skills {
            let Some(def) = skills::skill_def(&s.id) else { continue; };
            // Only PASSIVE auras (e.g. Guardian Aura) project here. Active
            // reservation buffs (Blessing / Agility Up) are now CAST on cooldown
            // (try_cast_buffs) — a real cast with a timed buff — so they're no
            // longer folded as always-on auras.
            if def.cast_buff.is_empty()
                || def.loadout_slots > 0
                || !matches!(def.kind, skills::SkillKind::Passive)
            {
                continue;
            }
            for spec in def.cast_buff {
                let mag = spec.magnitude + spec.per_level * (s.level.saturating_sub(1) as i32);
                match aura_buffs.iter_mut().find(|(k, _)| *k == spec.kind) {
                    Some(slot) => slot.1 = slot.1.max(mag),
                    None => aura_buffs.push((spec.kind, mag)),
                }
            }
        }
    }
    for e in state.grid.entities.iter_mut() {
        if !matches!(e.team, Team::Player) {
            continue;
        }
        e.aura_atk = atk_aura;
        e.buffs.retain(|b| b.source_id != AURA_SOURCE);
        for (kind, mag) in &aura_buffs {
            if *mag != 0 {
                e.buffs.push(skills::Buff {
                    kind: *kind,
                    magnitude: *mag,
                    ticks: 1,
                    source_id: AURA_SOURCE,
                });
            }
        }
    }
}

/// PATHS OF SOUL Power auras: stamp the party-wide AtkPct / DefPct / MatkPct /
/// HpPct bonuses onto every living player each tick (cleared + re-added like the
/// other auras, so they refresh and never stack). Early-returns when the tree is
/// empty, so old saves + goldens are byte-identical.
pub(crate) fn apply_soul_path_auras(state: &mut GameState) {
    use crate::soul_paths::{self, power_buff_kind, PATH_AURA_SOURCE};
    if state.soul_paths.is_empty() {
        return;
    }
    let mut auras: Vec<(skills::BuffKind, i32)> = Vec::new();
    for eff in soul_paths::AURA_EFFECTS {
        let mag = soul_paths::path_bonus(&state.soul_paths, eff);
        if mag != 0 {
            if let Some(kind) = power_buff_kind(eff) {
                auras.push((kind, mag));
            }
        }
    }
    for e in state.grid.entities.iter_mut() {
        if !matches!(e.team, Team::Player) {
            continue;
        }
        // 0.0.247: stamp the DEAD too, exactly like `apply_party_auras` — the two aura
        // channels used to disagree (a corpse kept its party aura but lost its path aura),
        // which would read differently the moment anything inspects a downed member's stats.
        e.buffs.retain(|b| b.source_id != PATH_AURA_SOURCE);
        for (kind, mag) in &auras {
            e.buffs.push(skills::Buff {
                kind: *kind,
                magnitude: *mag,
                ticks: 1,
                source_id: PATH_AURA_SOURCE,
            });
        }
    }
}

/// Auto-use the party's slotted consumables (RO auto-pot). For every alive
/// player with consumable slots, fire each slotted item whose trigger is met +
/// is off its internal cooldown + has charges left: heal flat HP, or refresh a
/// self-buff. Deterministic (no PRNG). Entities with no consumables are skipped,
/// so old saves + goldens stay byte-identical.
/// The PARTY provision rack: buff EVERY living member from up to three shared slots.
///
/// A second layer over `consumable_upkeep`, not a replacement — each member's own slots keep
/// firing and keep draining. That separation is the whole design
/// (docs/party-buff-slots-design.md); the objection it answers is in `provisions_view.gd`.
///
/// ONE SOURCE PER SLOT (`PARTY_PROVISION_SOURCE + i`), so three different items granting the same
/// BuffKind apply as three buffs. Sharing one id would collapse that loadout to a single buff with
/// nothing to say so.
///
/// Consumption is ONE unit per refresh, not one per member: the rack is a party-level object and
/// the per-character sink is untouched beside it.
pub(crate) fn party_provision_upkeep(state: &mut GameState) {
    use crate::items::{ConsumeEffect, ConsumeTrigger, PARTY_PROVISION_SOURCE};
    if state.party_provisions.is_empty() {
        return;
    }
    let tick = state.tick;
    let mut rack = core::mem::take(&mut state.party_provisions);
    for slot in rack.iter_mut() {
        // From the entry's STABLE key, never its position — see `PartyProvision`.
        let src = PARTY_PROVISION_SOURCE + slot.key as u32;
        let Some(spec) = slot.item.consumable else { continue };
        if slot.item.quantity == 0 || tick < slot.ready_tick {
            continue;
        }
        // Only the self-buff trigger makes sense for a rack: an HP threshold is per-member and a
        // revive belongs to the ally loop. Anything else simply never fires here.
        let ConsumeTrigger::BuffMissing(kind) = spec.trigger else { continue };
        // Fire when ANY living member is missing THIS SLOT's buff — a member who joined or
        // respawned after the last refresh must not wait out the full duration bare.
        let due = state.grid.entities.iter().any(|e| {
            matches!(e.team, Team::Player)
                && e.alive
                && !e.buffs.iter().any(|b| b.kind == kind && b.source_id == src)
        });
        if !due {
            continue;
        }
        for e in state.grid.entities.iter_mut() {
            if !matches!(e.team, Team::Player) || !e.alive {
                continue;
            }
            match spec.effect {
                ConsumeEffect::Buff { kind, magnitude, ticks } => {
                    skills::apply_buff(e, skills::Buff { kind, magnitude, ticks, source_id: src });
                }
                ConsumeEffect::Buffs(list) => {
                    for cb in list.iter() {
                        skills::apply_buff(
                            e,
                            skills::Buff {
                                kind: cb.kind,
                                magnitude: cb.magnitude,
                                ticks: cb.ticks,
                                source_id: src,
                            },
                        );
                    }
                }
                // A rack slot buffs; it does not heal, load ammo or revive. Exhaustive so a new
                // effect has to decide what a party-wide version of it means.
                ConsumeEffect::HealHp(_)
                | ConsumeEffect::LoadAmmo { .. }
                | ConsumeEffect::ReviveAlly { .. } => {}
            }
        }
        // NOTHING APPLIED, NOTHING SPENT. A Party item pairing `BuffMissing(X)` with a non-buff
        // effect would otherwise drain a charge every cooldown forever while `X` was never
        // applied, so the trigger stayed true and the stack emptied for nothing. No catalog item
        // does this today and `GameData::validate` checks no consumable-spec coherence at all, so
        // it would ship silently.
        if !matches!(spec.effect, ConsumeEffect::Buff { .. } | ConsumeEffect::Buffs(_)) {
            continue;
        }
        slot.item.quantity -= 1;
        slot.ready_tick = tick + spec.cooldown as u64;
    }
    rack.retain(|s| s.item.quantity > 0);
    state.party_provisions = rack;
}

pub(crate) fn consumable_upkeep(state: &mut GameState) {
    use crate::items::{ConsumeEffect, ConsumeTrigger, CONSUMABLE_BUFF_SOURCE};
    let tick = state.tick;
    // `state` is mutably borrowed for this whole loop, so heals are collected and recorded after
    // it rather than at the point of application.
    let mut healed: Vec<(u32, i32)> = Vec::new();
    for e in state.grid.entities.iter_mut() {
        if !matches!(e.team, Team::Player) || !e.alive {
            continue;
        }
        if e.consumables.is_empty() && e.provisions.is_empty() {
            continue;
        }
        let maxhp = combat::effective_max_hp(e);
        // The RESERVED provision slots fire on exactly the same rules as the bar, so they run the
        // same body rather than a copy of it. The vec is moved OUT for the pass because the body
        // needs `&mut e` (to read HP and apply buffs) while iterating it. Bar first, then
        // reserved — a fixed order, so the sequence of applications never depends on layout.
        for reserved in [false, true] {
            let mut slots = if reserved {
                core::mem::take(&mut e.provisions)
            } else {
                core::mem::take(&mut e.consumables)
            };
            // ASSIGN ANY MISSING KEY FIRST — a slot written before `key` existed reads 0, and
            // leaving them all at 0 is exactly the shared-source collision this replaces. Done
            // here rather than in a migration because this is the one pass that always runs.
            //
            // A key is free only when it is BOTH absent from the Vec and not still stamped on this
            // entity as a live buff — the same rule `add_party_provision` uses, and for the same
            // reason. Removing a slot leaves its buff running, so the key it carried is not free:
            // hand it to the replacement and the replacement's `BuffMissing` finds the orphan on
            // its own source and never fires. Swapping `aspd_pot_1` for `aspd_pot_3` would leave
            // the upgrade inert, and its stack undrained, for the full 30 minutes; with a meal,
            // two hours.
            let base = if reserved { CONSUMABLE_BUFF_SOURCE + 16 } else { CONSUMABLE_BUFF_SOURCE };
            for i in 0..slots.len() {
                if slots[i].key != 0 {
                    continue;
                }
                // BOUNDED, and that is the whole point of the rewrite.
                //
                // The previous version walked `while free_in_vec || live_on_entity { k += 1 }`,
                // which has no upper bound: orphaned buffs are unbounded (removing a slot leaves
                // its buff running on purpose, and `e.buffs` only shrinks on expiry), so `k`
                // climbs past every one of them. `k` is a `u8` and `overflow-checks = true` in
                // BOTH profiles with `panic = "abort"` in release — so ~255 slot/unslot cycles
                // with a long-duration buff PANIC the process. verify-server links this crate and
                // replays client-authored input logs, which makes a crafted log a way to abort the
                // server. The rack version this was modelled on searches a fixed range and refuses;
                // only half of that shape was copied.
                //
                // Preference, then fallback. A key that is free in the Vec AND carries no live
                // buff is best — that is what stops a replacement inheriting the departed slot's
                // source. If every key in the band is spoken for, take one that is merely free in
                // the Vec: it always exists (at most 3 slots against a 16-wide band), and the
                // worst case is the inheritance this prefers to avoid, not a panic.
                // Bounded to the container's CAPACITY, not to the 16-wide source band and not to
                // how many slots happen to be filled right now.
                //
                // Not the band: removing a slot leaves its buff running, so re-adding the same item
                // mints a fresh key each time and its buff stacks on itself — over 16 keys that is
                // a 16x self-stack for the price of 16 charges. Capping at capacity makes the
                // ceiling the honest one, as many simultaneous copies as there are slots to hold
                // them.
                //
                // Not the current length either: a container holding ONE slot would then have one
                // key, and swapping that slot would hand the replacement the departed item's still
                // -live source every time — the inert-upgrade bug this allocator exists to stop.
                let span = crate::events::CONSUMABLE_SLOTS as u8;
                let free_in_vec = |k: u8| !slots.iter().any(|s| s.key == k);
                let no_live_buff =
                    |k: u8| !e.buffs.iter().any(|b| b.source_id == base + k as u32 - 1);
                let k = (1..=span)
                    .find(|k| free_in_vec(*k) && no_live_buff(*k))
                    .or_else(|| (1..=span).find(|k| free_in_vec(*k)))
                    .unwrap_or(1);
                slots[i].key = k;
            }
            for i in 0..slots.len() {
                // One source per SLOT, from the slot's STABLE KEY and never its position: these
                // Vecs shrink — a depleted stack is swept below, and the player removes slots by
                // hand — and a positional source would slide the survivor onto the departed
                // slot's id, where its `BuffMissing` finds nothing, re-fires, burns a charge and
                // runs two copies of one buff. Reserved slots take a separate block so a bar slot
                // and a provision slot can never collide on the same number.
                // `key - 1`, so slot-key 1 lands on the SAME source the old positional scheme
                // gave index 0. Without that offset every save carrying a live provision would
                // have its buff orphaned once on update — the slot re-fires against an empty
                // source, burns a charge, and two copies of one buff sum. The transition is a
                // no-op instead.
                let src = base + slots[i].key as u32 - 1;
                let slot = &slots[i];
            let Some(spec) = slot.item.consumable else { continue };
            if slot.item.quantity == 0 || tick < slot.ready_tick {
                continue;
            }
            let fire = match spec.trigger {
                ConsumeTrigger::HpBelowPct(p) => {
                    (e.hp as i64) * 100 < (maxhp as i64) * (p as i64)
                }
                // MISSING FROM THIS SOURCE, not missing entirely.
                //
                // A kind-only check makes a consumable stop refreshing the moment ANYTHING else
                // grants the same BuffKind — and the party provision slots
                // (docs/party-buff-slots-design.md) are exactly that: a second layer, on the same
                // kinds, by design. Under a kind-only check a party +ATK item would silently stop
                // every member's personal +ATK food from ever firing again, draining nothing.
                //
                // That is the "cut the craft sink by 80%" outcome `provisions_view.gd` warns
                // about, arriving through a door nobody was watching: the two layers were meant to
                // stack and consume independently, and instead the outer one switches the inner
                // one off.
                //
                // Costs nothing today — no golden moved and no test changed, because nothing else
                // currently grants a consumable's buff kind. It is a prerequisite landed before
                // the thing that needs it, not a behaviour change looking for a reason.
                ConsumeTrigger::BuffMissing(kind) => {
                    !e.buffs.iter().any(|b| b.kind == kind && b.source_id == src)
                }
                // Quivers etc. — loaded by explicit input, never auto-fired.
                ConsumeTrigger::Manual => false,
                // Phoenix Tail — resolved in `phoenix_revive_upkeep` (it revives
                // a DIFFERENT entity), never in this self-use loop.
                ConsumeTrigger::AllyDown => false,
            };
            if !fire {
                continue;
            }
            // NOTHING APPLIED, NOTHING SPENT — and the decision is made BEFORE the effect runs.
            //
            // Two shapes. `LoadAmmo`/`ReviveAlly` are no-ops in this loop whatever fired them. And
            // a `BuffMissing` trigger paired with an effect that grants no buff never stops being
            // true, so it would fire every tick forever.
            //
            // ⚠️ The first version of this guard `continue`d AFTER the match, which inverted it:
            // `HealHp`'s arm really does mutate HP, so `BuffMissing` + `HealHp` healed for free
            // every tick and never depleted — the cooldown write-back was skipped too. Checking
            // first makes the pairing inert instead of a fountain. `GameData::validate` checks no
            // consumable-spec coherence, which is why this lives here and not at author time.
            let applies = !matches!(
                spec.effect,
                ConsumeEffect::LoadAmmo { .. } | ConsumeEffect::ReviveAlly { .. }
            ) && !(matches!(spec.trigger, ConsumeTrigger::BuffMissing(_))
                && !matches!(spec.effect, ConsumeEffect::Buff { .. } | ConsumeEffect::Buffs(_)));
            if !applies {
                continue;
            }
            match spec.effect {
                ConsumeEffect::HealHp(n) => {
                    let before = e.hp;
                    e.hp = (e.hp + n).min(maxhp);
                    healed.push((e.id, e.hp - before));
                }
                ConsumeEffect::Buff { kind, magnitude, ticks } => {
                    skills::apply_buff(
                        e,
                        skills::Buff { kind, magnitude, ticks, source_id: src },
                    );
                }
                // Applied in authored order so the stream is fixed; none of these draw.
                ConsumeEffect::Buffs(list) => {
                    for cb in list.iter() {
                        skills::apply_buff(
                            e,
                            skills::Buff {
                                kind: cb.kind,
                                magnitude: cb.magnitude,
                                ticks: cb.ticks,
                                source_id: src,
                            },
                        );
                    }
                }
                // Loaded via LoadAmmo input, not the auto-use bar — unreachable
                // here (Manual trigger never fires), but the match is exhaustive.
                ConsumeEffect::LoadAmmo { .. } => {}
                // Revives a downed ALLY — handled in `phoenix_revive_upkeep`
                // (AllyDown trigger never fires here), so no-op for exhaustiveness.
                ConsumeEffect::ReviveAlly { .. } => {}
            }
                slots[i].item.quantity -= 1;
                slots[i].ready_tick = tick + spec.cooldown as u64;
            }
            // Drop any slot whose stack is now empty so a depleted consumable
            // disappears from the bar instead of lingering as a 0-charge slot.
            slots.retain(|s| s.item.quantity > 0);
            if reserved {
                e.provisions = slots;
            } else {
                e.consumables = slots;
            }
        }
    }
    for (id, got) in healed {
        state.note_heal(id, id, got, crate::state::HealKind::Potion);
    }
}

/// Phoenix Tail: a living party member with a ready Phoenix slot REVIVES the
/// lowest-id downed ally mid-fight (restoring `hp_pct`% of max HP), instead of
/// waiting for the next zone change to revive the fallen. Separate from
/// `consumable_upkeep` because the effect touches a DIFFERENT entity than the
/// holder. Deterministic — no PRNG.
pub(crate) fn phoenix_revive_upkeep(state: &mut GameState) {
    use crate::items::{ConsumeEffect, ConsumeTrigger};
    let tick = state.tick;
    let count = state.grid.entities.len();
    for i in 0..count {
        if !matches!(state.grid.entities[i].team, Team::Player) || !state.grid.entities[i].alive {
            continue;
        }
        // First ready Phoenix slot on this holder.
        let mut chosen: Option<(usize, u8, u32)> = None; // (slot, hp_pct, cooldown)
        for s in 0..state.grid.entities[i].consumables.len() {
            let slot = &state.grid.entities[i].consumables[s];
            let Some(spec) = slot.item.consumable else { continue };
            if slot.item.quantity == 0 || tick < slot.ready_tick {
                continue;
            }
            if let (ConsumeTrigger::AllyDown, ConsumeEffect::ReviveAlly { hp_pct }) =
                (spec.trigger, spec.effect)
            {
                chosen = Some((s, hp_pct, spec.cooldown));
                break;
            }
        }
        let Some((s, hp_pct, cooldown)) = chosen else { continue };
        // Lowest-id downed party member.
        let me_team = state.grid.entities[i].team;
        let target = state
            .grid
            .entities
            .iter()
            .enumerate()
            .filter(|(j, e)| *j != i && e.team == me_team && !e.alive)
            .min_by_key(|(_, e)| e.id)
            .map(|(j, _)| j);
        let Some(tj) = target else { continue }; // nobody down → hold the charge
        let maxhp = combat::effective_max_hp(&state.grid.entities[tj]);
        let t = &mut state.grid.entities[tj];
        t.alive = true;
        t.hp = ((maxhp as i64) * (hp_pct as i64) / 100).max(1) as i32;
        t.casting = None;
        t.atk_acc = 0;
        t.bloodthirst_stacks = 0; // death loses the Bloodthirst streak
        state.grid.entities[i].consumables[s].item.quantity -= 1;
        state.grid.entities[i].consumables[s].ready_tick = tick + cooldown as u64;
    }
    for e in state.grid.entities.iter_mut() {
        e.consumables.retain(|s| s.item.quantity > 0);
    }
}

/// Grace delay before Bishop Resurrection auto-revives a fallen ally: 5 s (10
/// ticks/s). A member must stay DOWN this long before the rez fires, so they
/// aren't instantly popped back up the moment they fall.
pub(crate) const RESURRECTION_DELAY_TICKS: u32 = 50;

/// Resurrection (Bishop): an ACTIVE (1 reserve slot) — a living member who knows
/// the skill, has it SLOTTED within the SP-reserve budget, and has a CHARGE
/// auto-revives the first-fallen (lowest-id) ally that has been DOWN for at least
/// `RESURRECTION_DELAY_TICKS`, at `10% × level` MaxHP (Lv10 = 100%), spending one
/// charge (refilled per band in `build::zone`). Gated on the skill + a charge → no
/// one else is affected (goldens safe).
pub(crate) fn resurrection_upkeep(state: &mut GameState) {
    // Age each party member's down-timer (transient, serde-skip): alive ⇒ reset,
    // down ⇒ +1. Drives the rez grace delay below. A DOWN member also has its
    // DEBUFFS stripped (Stun/Freeze/Poison/…) so it doesn't come back crippled
    // when revived — beneficial buffs are left alone. (Players carry no debuffs in
    // normal play today, so this is a no-op for the goldens → byte-identical.)
    for e in state.grid.entities.iter_mut() {
        if matches!(e.team, Team::Player) {
            if e.alive {
                e.down_ticks = 0;
            } else {
                e.down_ticks = e.down_ticks.saturating_add(1);
                e.buffs.retain(|b| !b.kind.is_debuff());
            }
        }
    }
    let count = state.grid.entities.len();
    for i in 0..count {
        if !matches!(state.grid.entities[i].team, Team::Player) || !state.grid.entities[i].alive {
            continue;
        }
        let level = state.grid.entities[i]
            .skills
            .iter()
            .find(|s| s.id == "resurrection" && s.level > 0 && s.charges_left > 0)
            .map(|s| s.level);
        let Some(level) = level else { continue };
        // Active: must be slotted within the loadout budget to function.
        if !crate::skills::loadout_fits(
            &state.grid.entities[i].active_skills,
            crate::combat::loadout_capacity(&state.grid.entities[i]),
            "resurrection",
        ) {
            continue;
        }
        let me_team = state.grid.entities[i].team;
        let target = state
            .grid
            .entities
            .iter()
            .enumerate()
            .filter(|(j, e)| {
                *j != i && e.team == me_team && !e.alive
                    && e.down_ticks >= RESURRECTION_DELAY_TICKS
            })
            .min_by_key(|(_, e)| e.id)
            .map(|(j, _)| j);
        let Some(tj) = target else { continue }; // nobody down → hold the charge
        let hp_pct = (level as i64 * 10).min(100); // 10%/level, Lv10 = 100%
        let maxhp = combat::effective_max_hp(&state.grid.entities[tj]);
        {
            let t = &mut state.grid.entities[tj];
            t.alive = true;
            t.hp = ((maxhp as i64) * hp_pct / 100).max(1) as i32;
            t.casting = None;
            t.atk_acc = 0;
            t.bloodthirst_stacks = 0; // death still loses the streak
        }
        for s in state.grid.entities[i].skills.iter_mut() {
            if s.id == "resurrection" {
                s.charges_left -= 1;
                break;
            }
        }
    }
}

// (Endure's old per-tick soft-DEF stamp was removed in 0.0.178 — the skill is
// now a passive VIT bonus + a mobbed-penalty resist read directly inside
// `apply_damage`, so there is no per-tick upkeep for it.)

/// How many living enemies each entity currently holds under its own Taunt.
/// Bulwark reads this from `effective_hard_def`, which only ever sees one
/// `Entity` — so the count has to live ON the entity rather than be gathered
/// at the read site. Recomputed from `taunted_by`, never accumulated, so it
/// cannot drift.
pub(crate) fn recount_taunt_hold(state: &mut GameState) {
    let holders: Vec<(u32, u32)> = {
        let mut counts: std::collections::BTreeMap<u32, u32> = std::collections::BTreeMap::new();
        for e in state.grid.entities.iter() {
            if !e.alive || e.taunt_ticks == 0 {
                continue;
            }
            if let Some(by) = e.taunted_by {
                *counts.entry(by).or_insert(0) += 1;
            }
        }
        counts.into_iter().collect()
    };
    for e in state.grid.entities.iter_mut() {
        e.taunt_hold = holders
            .iter()
            .find(|(id, _)| *id == e.id)
            .map_or(0, |(_, n)| *n);
    }
}

/// Taunt: age out existing aggro locks, then any caster whose Taunt is off
/// cooldown re-locks every enemy within `TAUNT_RADIUS` (5×5) onto itself
/// and restarts its cooldown. Free — no swing, SP, or animation.
pub(crate) fn apply_taunt(state: &mut GameState) {
    // 1. Age existing locks; clear when they lapse.
    for e in state.grid.entities.iter_mut() {
        if e.taunt_ticks > 0 {
            e.taunt_ticks -= 1;
            if e.taunt_ticks == 0 {
                e.taunted_by = None;
            }
        }
    }
    recount_taunt_hold(state);
    // 2. Ready casters re-apply to enemies in range.
    let count = state.grid.entities.len();
    for i in 0..count {
        if !state.grid.entities[i].alive {
            continue;
        }
        let Some(slot) = state.grid.entities[i]
            .skills.iter()
            .position(|s| s.id == "taunt" && s.cooldown == 0)
        else {
            continue;
        };
        let lvl = state.grid.entities[i].skills[slot].level;
        let me_pos = state.grid.entities[i].pos;
        let me_team = state.grid.entities[i].team;
        let me_id = state.grid.entities[i].id;
        // Lock duration is a FIXED 4 s; the cooldown scales down with level, so
        // higher levels re-cast before the lock lapses (less aggro gap).
        let cd = crate::combat::stat_mods::scale_cooldown(&state.grid.entities[i], skills::taunt_cooldown_ticks(lvl));
        let mut taunted_any = false;
        for j in 0..count {
            if j == i {
                continue;
            }
            let (alive, team, pos) = {
                let e = &state.grid.entities[j];
                (e.alive, e.team, e.pos)
            };
            if !alive || team == me_team || me_pos.chebyshev(pos) > skills::TAUNT_RADIUS {
                continue;
            }
            taunted_any = true;
            state.grid.entities[j].taunted_by = Some(me_id);
            state.grid.entities[j].taunt_ticks = skills::TAUNT_LOCK_TICKS;
        }
        if taunted_any {
            state.grid.entities[i].skills[slot].cooldown = cd;
        }
    }
}


#[cfg(test)]
mod field_tests {
    use super::*;
    use crate::grid::{Cell, Element, Entity, Team};
    use crate::skills::{buff_sum, BuffKind};
    use crate::state::{GameState, GroundField, GroundFieldKind};

    #[test]
    fn damage_field_hits_victim_and_freezes() {
        let mut s = GameState::new(1, 16, 8);
        let caster = Entity::idle(1, Cell::new(0, 0)).with_team(Team::Player);
        s.grid.spawn(caster);
        let mut mob = Entity::idle(1000, Cell::new(2, 2)).with_team(Team::Monster);
        mob.hp = 100_000;
        mob.max_hp = 100_000;
        s.grid.spawn(mob);
        s.ground_fields.push(GroundField {
            center: Cell::new(2, 2),
            radius: 3,
            victim_team: Team::Monster,
            ticks_left: 100,
            source_id: 1,
            follow_id: 0,
            skill_id: String::new(), hit_ids: Vec::new(),
            kind: GroundFieldKind::Damage {
                amount: 500,
                element: Element::Ice,
                status: Some(BuffKind::Freeze),
                status_bp: 10_000, // always, for the test
                status_dur: 30,
                status_mag: 1,
                hit_interval: 1, status_at: 0, knockback: 0,
                impact_radius: 0, strike_hits: 1, once_per_target: false, fuse: false, pull: 0,
            },
        });
        // tick 0 is a regen interval → the field fires once.
        tick_ground_fields(&mut s);
        assert!(s.grid.entities[1].hp < 100_000, "damage field hit the victim");
        assert!(buff_sum(&s.grid.entities[1], BuffKind::Freeze) > 0, "victim frozen");
    }

    #[test]
    fn damage_field_status_skips_bosses() {
        let mut s = GameState::new(1, 16, 8);
        s.grid.spawn(Entity::idle(1, Cell::new(0, 0)).with_team(Team::Player));
        let mut boss = Entity::idle(1000, Cell::new(2, 2)).with_team(Team::Monster);
        boss.hp = 100_000; boss.max_hp = 100_000; boss.is_boss = true;
        s.grid.spawn(boss);
        s.ground_fields.push(GroundField {
            center: Cell::new(2, 2), radius: 3, victim_team: Team::Monster,
            ticks_left: 100, source_id: 1, follow_id: 0,
            skill_id: String::new(), hit_ids: Vec::new(),
            kind: GroundFieldKind::Damage {
                amount: 500, element: Element::Ice, status: Some(BuffKind::Freeze),
                status_bp: 10_000, status_dur: 30, status_mag: 1, hit_interval: 1, status_at: 0, knockback: 0,
                impact_radius: 0, strike_hits: 1, once_per_target: false, fuse: false, pull: 0,
            },
        });
        tick_ground_fields(&mut s);
        assert!(s.grid.entities[1].hp < 100_000, "boss still takes field damage");
        assert_eq!(buff_sum(&s.grid.entities[1], BuffKind::Freeze), 0, "boss resists field CC");
    }

    #[test]
    fn meteor_field_random_strikes_register_multihit() {
        // A meteor field (impact_radius > 0) drops one random 5×5 strike per
        // interval; each landing hits a victim `strike_hits` times (one event).
        let mut s = GameState::new(7, 24, 24);
        s.grid.spawn(Entity::idle(1, Cell::new(0, 0)).with_team(Team::Player));
        let mut mob = Entity::idle(1000, Cell::new(12, 12)).with_team(Team::Monster);
        mob.hp = 1_000_000;
        mob.max_hp = 1_000_000;
        s.grid.spawn(mob);
        s.ground_fields.push(GroundField {
            center: Cell::new(12, 12), radius: 3, victim_team: Team::Monster,
            ticks_left: 100, source_id: 1, follow_id: 0,
            skill_id: String::new(), hit_ids: Vec::new(),
            kind: GroundFieldKind::Damage {
                amount: 500, element: Element::Fire, status: None,
                status_bp: 0, status_dur: 0, status_mag: 0, hit_interval: 1, status_at: 0,
                knockback: 0, impact_radius: 2, strike_hits: 5, once_per_target: false, fuse: false, pull: 0,
            },
        });
        let player_hp0 = s.grid.entities[0].hp; // caster's team — never a victim
        let mob_hp0 = s.grid.entities[1].hp;
        for _ in 0..60 {
            tick_ground_fields(&mut s);
        }
        assert_eq!(s.grid.entities[0].hp, player_hp0, "the caster's own team is never hit");
        assert!(s.grid.entities[1].hp < mob_hp0, "random meteors eventually land on the mob");
        assert!(
            s.field_hits.iter().any(|h| h.target_id == 1000 && h.hits == 5),
            "a meteor strike registers as a 5-hit event"
        );
    }

    #[test]
    fn vermilion_once_per_target_hits_each_unit_once() {
        // Lord of Vermilion: a unit takes ONE 20-hit instance for the field's life,
        // but a NEW unit entering the lingering AoE still gets its own.
        let mut s = GameState::new(1, 16, 8);
        s.grid.spawn(Entity::idle(1, Cell::new(0, 0)).with_team(Team::Player));
        let mut a = Entity::idle(1000, Cell::new(2, 2)).with_team(Team::Monster);
        a.hp = 1_000_000;
        a.max_hp = 1_000_000;
        s.grid.spawn(a);
        s.ground_fields.push(GroundField {
            center: Cell::new(2, 2), radius: 3, victim_team: Team::Monster,
            ticks_left: 100, source_id: 1, follow_id: 0,
            skill_id: String::new(), hit_ids: Vec::new(),
            kind: GroundFieldKind::Damage {
                amount: 500, element: Element::Wind, status: None,
                status_bp: 0, status_dur: 0, status_mag: 0, hit_interval: 1, status_at: 0,
                knockback: 0, impact_radius: 0, strike_hits: 20, once_per_target: true, fuse: false, pull: 0,
            },
        });
        tick_ground_fields(&mut s);
        let hp_after_first = s.grid.entities[1].hp;
        assert!(hp_after_first < 1_000_000, "first tick hits the unit");
        assert!(s.ground_fields[0].hit_ids.contains(&1000), "the unit is recorded as hit");
        assert!(
            s.field_hits.iter().any(|h| h.target_id == 1000 && h.hits == 20),
            "a Vermilion instance registers as a 20-hit event"
        );
        // many more ticks: the SAME unit is never hit again
        for _ in 0..10 {
            tick_ground_fields(&mut s);
        }
        assert_eq!(s.grid.entities[1].hp, hp_after_first, "once-per-target: not hit again");
        // a NEW unit entering the lingering field gets its OWN instance
        let mut b = Entity::idle(1001, Cell::new(2, 3)).with_team(Team::Monster);
        b.hp = 1_000_000;
        b.max_hp = 1_000_000;
        s.grid.spawn(b);
        tick_ground_fields(&mut s);
        assert!(s.grid.entities[2].hp < 1_000_000, "a new entrant is still hit");
        assert_eq!(s.grid.entities[1].hp, hp_after_first, "the old unit stays un-re-hit");
    }

    #[test]
    fn flame_shield_absorbs_then_explodes() {
        use crate::grid::AbsorbShield;
        let mut s = GameState::new(1, 16, 8);
        let mut mage = Entity::idle(1, Cell::new(2, 2)).with_team(Team::Player);
        mage.hp = 1000;
        mage.max_hp = 1000;
        mage.shields.push(AbsorbShield { pool: 100, burst_dmg: 200, burst_knock: 1, src_id: 1, ..Default::default() });
        s.grid.spawn(mage);
        let mut mob = Entity::idle(1000, Cell::new(2, 3)).with_team(Team::Monster);
        mob.hp = 1000;
        mob.max_hp = 1000;
        s.grid.spawn(mob);
        // A hit under the pool is fully soaked — no HP loss, no burst.
        let through = crate::combat::absorb_soak(&mut s.grid.entities[0], 30);
        assert_eq!(through, 0, "small hit fully absorbed");
        assert_eq!(s.grid.entities[0].shields[0].pool, 70);
        assert!(s.grid.entities[0].pending_bursts.is_empty());
        // A hit exceeding the pool: 70 soaked, 30 through, pool empties → burst armed.
        let through2 = crate::combat::absorb_soak(&mut s.grid.entities[0], 100);
        assert_eq!(through2, 30, "overflow passes through");
        assert!(s.grid.entities[0].shields.is_empty(), "emptied shield removed");
        assert_eq!(s.grid.entities[0].pending_bursts.len(), 1, "empty pool arms the explosion");
        // Resolve it: the adjacent mob eats the Fire blast; the burst clears.
        let mob_hp0 = s.grid.entities[1].hp;
        resolve_absorb_bursts(&mut s);
        assert!(s.grid.entities[1].hp < mob_hp0, "explosion hits the adjacent enemy");
        assert!(s.grid.entities[0].pending_bursts.is_empty(), "burst consumed");
    }

    #[test]
    fn shields_cascade_oldest_first() {
        use crate::grid::AbsorbShield;
        let mut s = GameState::new(1, 16, 8);
        let mut e = Entity::idle(1, Cell::new(0, 0)).with_team(Team::Player);
        e.hp = 1000;
        e.max_hp = 1000;
        // Two shields from DIFFERENT sources stack; [0] is the oldest.
        e.shields.push(AbsorbShield { pool: 50, burst_dmg: 0, burst_knock: 0, src_id: 10, ..Default::default() });
        e.shields.push(AbsorbShield { pool: 50, burst_dmg: 0, burst_knock: 0, src_id: 20, ..Default::default() });
        s.grid.spawn(e);
        // A 60 hit drains the oldest (50) fully, cascades 10 into the next.
        let through = crate::combat::absorb_soak(&mut s.grid.entities[0], 60);
        assert_eq!(through, 0, "both shields together absorb it");
        assert_eq!(s.grid.entities[0].shields.len(), 1, "the oldest emptied + was removed");
        assert_eq!(s.grid.entities[0].shields[0].src_id, 20, "the newer shield remains");
        assert_eq!(s.grid.entities[0].shields[0].pool, 40);
        assert_eq!(s.grid.entities[0].pending_bursts.len(), 1, "the emptied shield armed a burst");
    }

    #[test]
    fn praesidium_hit_cap_ends_shield_even_with_pool_left() {
        use crate::grid::AbsorbShield;
        let mut s = GameState::new(1, 16, 8);
        let mut e = Entity::idle(1, Cell::new(0, 0)).with_team(Team::Player);
        e.hp = 1000;
        e.max_hp = 1000;
        // Huge pool but only 2 hits (Praesidium dual cap) → ends on the 2nd blow.
        e.shields.push(AbsorbShield {
            pool: 1000, burst_dmg: 0, burst_knock: 0, src_id: 7, hits_left: 2, persist: true,
            ..Default::default()
        });
        s.grid.spawn(e);
        assert_eq!(crate::combat::absorb_soak(&mut s.grid.entities[0], 10), 0, "1st blow soaked");
        assert_eq!(s.grid.entities[0].shields.len(), 1, "1 hit left → still up");
        assert_eq!(s.grid.entities[0].shields[0].hits_left, 1);
        assert_eq!(crate::combat::absorb_soak(&mut s.grid.entities[0], 10), 0, "2nd blow soaked");
        assert!(s.grid.entities[0].shields.is_empty(), "hit cap reached → shield ended");
        // No burst (burst_dmg 0) → resolve is a no-op, pending clears.
        resolve_absorb_bursts(&mut s);
        assert!(s.grid.entities[0].pending_bursts.is_empty(), "inert burst cleaned up");
    }

    #[test]
    fn flame_shield_cooldown_starts_on_break() {
        use crate::grid::AbsorbShield;
        use crate::skills::{skill_def, SkillState};
        let mut s = GameState::new(1, 16, 8);
        let mut mage = Entity::idle(1, Cell::new(2, 2)).with_team(Team::Player);
        mage.hp = 1000;
        mage.max_hp = 1000;
        mage.skills.push(SkillState::new("flame_shield", 5));
        // A shield that just emptied (queued to explode this tick).
        mage.pending_bursts.push(AbsorbShield { pool: 0, burst_dmg: 100, burst_knock: 1, src_id: 1, ..Default::default() });
        s.grid.spawn(mage);
        let mut mob = Entity::idle(1000, Cell::new(2, 3)).with_team(Team::Monster);
        mob.hp = 1000;
        mob.max_hp = 1000;
        s.grid.spawn(mob);
        assert_eq!(s.grid.entities[0].skills[0].cooldown, 0, "no cd before the shield breaks");
        resolve_absorb_bursts(&mut s);
        let cd = skill_def("flame_shield").unwrap().cooldown_ticks;
        assert_eq!(s.grid.entities[0].skills[0].cooldown, cd, "cd starts when the shield breaks");
    }
}

#[cfg(test)]
mod consumable_tests {
    use super::*;
    use crate::grid::{Cell, Entity};
    use crate::items::{
        ConsumableSlot, ConsumableSpec, ConsumeEffect, ConsumeTrigger, Item, ItemKind, Rarity,
    };
    use crate::skills::BuffKind;
    use crate::state::GameState;

    fn player_with(pot: Item, hp: i32, max_hp: i32) -> GameState {
        let mut s = GameState::new(1, 8, 8);
        let mut e = Entity::idle(1, Cell::new(0, 0));
        e.team = Team::Player;
        e.max_hp = max_hp;
        e.hp = hp;
        e.consumables.push(ConsumableSlot { key: 0, item: pot, ready_tick: 0 });
        s.grid.spawn(e);
        s
    }

    fn hp_pot(heal: i32, qty: u32) -> Item {
        let mut p = Item::new("hp_pot", ItemKind::Consumable, 0, 0, Rarity::Common);
        p.quantity = qty;
        p.consumable = Some(ConsumableSpec { provision: None,
            trigger: ConsumeTrigger::HpBelowPct(50),
            effect: ConsumeEffect::HealHp(heal),
            cooldown: 30,
        });
        p
    }

    #[test]
    fn hp_potion_fires_when_low_then_respects_cooldown() {
        let mut s = player_with(hp_pot(100, 3), 50, 200); // 25% < 50%
        consumable_upkeep(&mut s);
        let e = &s.grid.entities[0];
        assert_eq!(e.hp, 150, "healed flat 100");
        assert_eq!(e.consumables[0].item.quantity, 2, "one charge spent");
        assert_eq!(e.consumables[0].ready_tick, 30, "cooldown armed");
        // Same tick again: still on cooldown (tick 0 < ready 30) → no second use.
        consumable_upkeep(&mut s);
        assert_eq!(s.grid.entities[0].consumables[0].item.quantity, 2);
    }

    #[test]
    fn depleted_consumable_slot_is_removed() {
        // A 1-charge potion fires, hits 0, and its slot is dropped from the bar.
        let mut s = player_with(hp_pot(100, 1), 50, 200); // 25% < 50%
        consumable_upkeep(&mut s);
        assert!(
            s.grid.entities[0].consumables.is_empty(),
            "depleted slot removed, not left at quantity 0"
        );
    }

    #[test]
    fn hp_potion_skips_when_healthy() {
        let mut s = player_with(hp_pot(100, 3), 150, 200); // 75% ≥ 50%
        consumable_upkeep(&mut s);
        let e = &s.grid.entities[0];
        assert_eq!(e.hp, 150, "untouched");
        assert_eq!(e.consumables[0].item.quantity, 3, "no charge spent");
    }

    #[test]
    fn aspd_potion_refreshes_self_buff() {
        let mut p = Item::new("aspd_pot", ItemKind::Consumable, 0, 0, Rarity::Common);
        p.quantity = 2;
        p.consumable = Some(ConsumableSpec { provision: None,
            trigger: ConsumeTrigger::BuffMissing(BuffKind::AspdPotionPct),
            effect: ConsumeEffect::Buff { kind: BuffKind::AspdPotionPct, magnitude: 1000, ticks: 600 },
            cooldown: 50,
        });
        let mut s = player_with(p, 200, 200);
        consumable_upkeep(&mut s);
        let e = &s.grid.entities[0];
        assert!(
            e.buffs.iter().any(|b| b.kind == BuffKind::AspdPotionPct && b.magnitude == 1000),
            "ASPD buff applied"
        );
        assert_eq!(e.consumables[0].item.quantity, 1, "one charge spent");
    }

    /// EVERY BUFF STACKS (user 2026-08-18). Two consumables granting the SAME kind must apply as
    /// two separate buffs, and both must keep draining.
    ///
    /// The case has to use ONE kind twice. A test with two DIFFERENT kinds passes on the broken
    /// code, because the collision is on `(kind, source_id)` — and every consumable used to share
    /// one source id, so the second item silently replaced the first: one buff where the player
    /// paid for two, and the loser's stack then never drained again because
    /// `BuffMissing` saw the winner's buff and never re-fired.
    #[test]
    fn two_consumables_of_the_same_kind_stack_instead_of_replacing() {
        let spec = |mag: i32| ConsumableSpec {
            provision: None,
            trigger: ConsumeTrigger::BuffMissing(BuffKind::AspdPotionPct),
            effect: ConsumeEffect::Buff { kind: BuffKind::AspdPotionPct, magnitude: mag, ticks: 600 },
            cooldown: 50,
        };
        let mut s = player_with(cons(spec(100), 2), 200, 200);
        s.grid.entities[0].consumables.push(ConsumableSlot { key: 0, item: cons(spec(300), 2), ready_tick: 0 });
        consumable_upkeep(&mut s);
        let e = &s.grid.entities[0];
        let mine: Vec<_> = e.buffs.iter().filter(|b| b.kind == BuffKind::AspdPotionPct).collect();
        assert_eq!(mine.len(), 2, "the second item REPLACED the first instead of stacking");
        assert_eq!(mine.iter().map(|b| b.magnitude).sum::<i32>(), 400, "magnitudes did not add");
        // And BOTH stacks paid for it — a buff that applies without draining is a free buff.
        assert_eq!(e.consumables[0].item.quantity, 1, "the first stack did not drain");
        assert_eq!(e.consumables[1].item.quantity, 1, "the second stack did not drain");
    }

    /// THE CASE THE WHOLE DESIGN TURNS ON: three DIFFERENT party items granting the SAME buff
    /// kind must apply as three buffs on every member, and the per-character layer must keep
    /// firing underneath them.
    ///
    /// Three different KINDS would pass on a broken implementation — the collision is on
    /// `(kind, source_id)`, so only a repeated kind can show a shared source id.
    #[test]
    fn three_party_items_of_one_kind_stack_on_every_member() {
        use crate::items::{ProvisionKind, PARTY_PROVISION_SOURCE};
        let party = |mag: i32| {
            let mut it = Item::new(
                &format!("rally_{mag}"), ItemKind::Consumable, 0, 0, Rarity::Common);
            it.quantity = 5;
            it.consumable = Some(ConsumableSpec {
                provision: Some(ProvisionKind::Party),
                trigger: ConsumeTrigger::BuffMissing(BuffKind::AtkPct),
                effect: ConsumeEffect::Buff { kind: BuffKind::AtkPct, magnitude: mag, ticks: 600 },
                cooldown: 50,
            });
            it
        };
        // A personal +ATK food on the member at the same time — the layer that must not be
        // switched off by the rack.
        let mut own = Item::new("own_food", ItemKind::Consumable, 0, 0, Rarity::Common);
        own.quantity = 3;
        own.consumable = Some(ConsumableSpec {
            provision: None,
            trigger: ConsumeTrigger::BuffMissing(BuffKind::AtkPct),
            effect: ConsumeEffect::Buff { kind: BuffKind::AtkPct, magnitude: 7, ticks: 600 },
            cooldown: 50,
        });
        let mut s = player_with(own, 200, 200);
        for mag in [100, 300, 500] {
            s.inventory.push(party(mag));
            let idx = s.inventory.len() as u32 - 1;
            crate::events::apply_input(&mut s, crate::events::InputEvent::AddPartyProvision {
                inv_idx: idx, qty: 1,
            });
        }
        assert_eq!(s.party_provisions.len(), 3, "the rack did not take three items");
        consumable_upkeep(&mut s);
        party_provision_upkeep(&mut s);
        let e = &s.grid.entities[0];
        let party_buffs: Vec<_> = e.buffs.iter()
            .filter(|b| b.source_id >= PARTY_PROVISION_SOURCE && b.kind == BuffKind::AtkPct)
            .collect();
        assert_eq!(party_buffs.len(), 3, "the rack collapsed to one buff — shared source id");
        assert_eq!(party_buffs.iter().map(|b| b.magnitude).sum::<i32>(), 900);
        // The personal layer fired too, and paid for it.
        assert_eq!(e.consumables[0].item.quantity, 2, "the rack switched the personal food off");
        assert_eq!(crate::skills::buff_sum(e, BuffKind::AtkPct), 907, "the layers did not add");
        // One unit per refresh, per slot — not one per member.
        assert!(s.party_provisions.iter().all(|sl| sl.item.quantity == 4), "rack drained wrongly");
    }

    /// A RUNNING BUFF SURVIVES ITS ITEM LEAVING (user 2026-08-18) — take the item out fifteen
    /// minutes into a thirty-minute buff and it still ends at thirty.
    ///
    /// And the entry that shifts down must NOT inherit it. That is the whole reason a rack entry
    /// carries a stable key: with a position-derived source, removing slot 0 would move slot 1
    /// onto the departed item's source, so it would stop refreshing while the orphan blocked it
    /// and then double up when the orphan lapsed. This asserts the neighbour keeps its own buff,
    /// separate from the one left behind.
    #[test]
    fn removing_a_rack_item_leaves_its_buff_running_and_does_not_shift_the_neighbour() {
        use crate::items::{ProvisionKind, PARTY_PROVISION_SOURCE};
        let party = |id: &str, mag: i32| {
            let mut it = Item::new(id, ItemKind::Consumable, 0, 0, Rarity::Common);
            it.quantity = 5;
            it.consumable = Some(ConsumableSpec {
                provision: Some(ProvisionKind::Party),
                trigger: ConsumeTrigger::BuffMissing(BuffKind::AtkPct),
                effect: ConsumeEffect::Buff { kind: BuffKind::AtkPct, magnitude: mag, ticks: 600 },
                cooldown: 50,
            });
            it
        };
        let mut s = player_with(party("first", 100), 200, 200);
        s.inventory.clear();
        for (id, mag) in [("first", 100), ("second", 300)] {
            s.inventory.push(party(id, mag));
            let idx = s.inventory.len() as u32 - 1;
            // SEVERAL charges each: a one-charge stack is spent by the first refresh and the entry
            // is swept, which would make this a test about depletion rather than about removal.
            crate::events::apply_input(&mut s, crate::events::InputEvent::AddPartyProvision {
                inv_idx: idx, qty: 4,
            });
        }
        assert_eq!(s.party_provisions.len(), 2, "the rack did not take both items");
        party_provision_upkeep(&mut s);
        assert_eq!(crate::skills::buff_sum(&s.grid.entities[0], BuffKind::AtkPct), 400);

        // Pull the FIRST item out. Its buff is paid for and must run on.
        crate::events::apply_input(&mut s, crate::events::InputEvent::RemovePartyProvision { slot: 0 });
        assert_eq!(s.party_provisions.len(), 1);
        assert_eq!(crate::skills::buff_sum(&s.grid.entities[0], BuffKind::AtkPct), 400,
            "removing the item cancelled a buff that was already paid for");

        let survivor_src = PARTY_PROVISION_SOURCE + s.party_provisions[0].key as u32;
        let orphan_mag = s.grid.entities[0].buffs.iter()
            .find(|b| b.kind == BuffKind::AtkPct && b.source_id != survivor_src)
            .map(|b| b.magnitude);
        assert_eq!(orphan_mag, Some(100), "the departed item's buff vanished, or was inherited");

        // THE CASE THAT SEPARATES A STABLE KEY FROM A POSITIONAL ONE, and it only shows once the
        // SURVIVOR's own buff lapses while the orphan is still running. Drop just the survivor's
        // buff — the orphan stays — and refresh.
        //
        // With a stable key the survivor asks about ITS OWN source, sees nothing, and re-applies.
        // With a positional source it has slid onto the departed item's id, sees the orphan
        // sitting there, and never fires again until the orphan expires: an item still in the rack,
        // with charges left, silently doing nothing.
        let before = s.party_provisions[0].item.quantity;
        s.grid.entities[0].buffs.retain(|b| b.source_id != survivor_src);
        s.tick += 100; // past the cooldown
        party_provision_upkeep(&mut s);
        assert_eq!(s.party_provisions[0].item.quantity, before - 1,
            "the survivor was blocked by the departed item's buff and never refreshed");
        assert_eq!(crate::skills::buff_sum(&s.grid.entities[0], BuffKind::AtkPct), 400,
            "the survivor did not re-apply alongside the still-running orphan");
    }

    /// Removing a personal slot must not make its NEIGHBOUR re-fire.
    ///
    /// The audit's headline: `consumables`/`provisions` shrink (a depleted stack is swept every
    /// tick, and the player removes slots by hand), and a position-derived buff source slides the
    /// survivor onto the departed slot's id. Its `BuffMissing` then finds nothing there, fires
    /// again, burns a charge, and leaves TWO copies of one buff summing in `buff_sum`.
    #[test]
    fn removing_a_bar_slot_does_not_make_its_neighbour_refire() {
        let spec = |mag: i32| ConsumableSpec {
            provision: None,
            trigger: ConsumeTrigger::BuffMissing(BuffKind::AspdPotionPct),
            effect: ConsumeEffect::Buff { kind: BuffKind::AspdPotionPct, magnitude: mag, ticks: 6000 },
            cooldown: 0,
        };
        let mut s = player_with(cons(spec(100), 3), 200, 200);
        s.grid.entities[0].consumables.push(ConsumableSlot { key: 0, item: cons(spec(300), 3), ready_tick: 0 });
        consumable_upkeep(&mut s);
        assert_eq!(crate::skills::buff_sum(&s.grid.entities[0], BuffKind::AspdPotionPct), 400);
        let survivor_qty = s.grid.entities[0].consumables[1].item.quantity;

        // Drop slot 0 — exactly what `RemoveConsumable` and the depletion sweep both do.
        s.grid.entities[0].consumables.remove(0);
        s.grid.entities[0].buffs.retain(|b| b.magnitude != 100); // the departed slot's buff lapses
        s.tick += 10;
        consumable_upkeep(&mut s);
        assert_eq!(s.grid.entities[0].consumables[0].item.quantity, survivor_qty,
            "the survivor slid onto the removed slot's source and burned a charge re-firing");
        assert_eq!(crate::skills::buff_sum(&s.grid.entities[0], BuffKind::AspdPotionPct), 300,
            "one item is applying twice");
    }

    /// A rack key must not be handed out again while its buff is still running.
    ///
    /// Un-racking leaves the buff live BY DESIGN, so the key it was stamped with is not free.
    /// Give it to a new item and the newcomer sees the orphan on its own source, never fires and
    /// never drains — an item in the rack, charges left, doing nothing.
    #[test]
    fn a_rack_key_is_not_reused_while_its_buff_still_runs() {
        use crate::items::ProvisionKind;
        let party = |id: &str, mag: i32| {
            let mut it = Item::new(id, ItemKind::Consumable, 0, 0, Rarity::Common);
            it.quantity = 5;
            it.consumable = Some(ConsumableSpec {
                provision: Some(ProvisionKind::Party),
                trigger: ConsumeTrigger::BuffMissing(BuffKind::AtkPct),
                effect: ConsumeEffect::Buff { kind: BuffKind::AtkPct, magnitude: mag, ticks: 6000 },
                cooldown: 0,
            });
            it
        };
        let mut s = player_with(party("seed", 1), 200, 200);
        s.inventory.clear();
        s.inventory.push(party("feast", 100));
        crate::events::apply_input(&mut s, crate::events::InputEvent::AddPartyProvision { inv_idx: 0, qty: 4 });
        party_provision_upkeep(&mut s);
        assert_eq!(crate::skills::buff_sum(&s.grid.entities[0], BuffKind::AtkPct), 100);

        // Un-rack it. The buff runs on, so its key is NOT free.
        crate::events::apply_input(&mut s, crate::events::InputEvent::RemovePartyProvision { slot: 0 });
        s.inventory.push(party("banquet", 700));
        let idx = s.inventory.len() as u32 - 1;
        crate::events::apply_input(&mut s, crate::events::InputEvent::AddPartyProvision { inv_idx: idx, qty: 4 });
        assert_eq!(s.party_provisions.len(), 1, "the new item was not racked");
        s.tick += 10;
        party_provision_upkeep(&mut s);
        assert_eq!(crate::skills::buff_sum(&s.grid.entities[0], BuffKind::AtkPct), 800,
            "the new item inherited a live orphan's source and never fired");
        assert_eq!(s.party_provisions[0].item.quantity, 3, "the new item never drained");
    }

    /// "Add more": the same item with the same binding refills the slot IN PLACE — key,
    /// `ready_tick` and the running buff stay; a different binding is refused; a full rack still
    /// takes a refill.
    #[test]
    fn adding_more_of_a_racked_item_refills_its_slot_in_place() {
        use crate::items::ProvisionKind;
        let party = |id: &str, qty: u32, bound: bool| {
            let mut it = Item::new(id, ItemKind::Consumable, 0, 0, Rarity::Common);
            it.quantity = qty;
            it.bound = bound;
            it.consumable = Some(ConsumableSpec {
                provision: Some(ProvisionKind::Party),
                trigger: ConsumeTrigger::BuffMissing(BuffKind::AtkPct),
                effect: ConsumeEffect::Buff { kind: BuffKind::AtkPct, magnitude: 100, ticks: 6000 },
                cooldown: 0,
            });
            it
        };
        let mut s = player_with(party("seed", 1, false), 200, 200);
        s.inventory.clear();
        s.inventory.push(party("feast", 2, true));
        crate::events::apply_input(&mut s, crate::events::InputEvent::AddPartyProvision { inv_idx: 0, qty: 2 });
        party_provision_upkeep(&mut s);
        let (key, ready) = (s.party_provisions[0].key, s.party_provisions[0].ready_tick);
        assert_eq!(s.party_provisions[0].item.quantity, 1, "one fired");
        // Fill the rest of the rack, then add more feast: it still refills.
        s.inventory.push(party("a", 1, true));
        s.inventory.push(party("b", 1, true));
        crate::events::apply_input(&mut s, crate::events::InputEvent::AddPartyProvision { inv_idx: 0, qty: 1 });
        crate::events::apply_input(&mut s, crate::events::InputEvent::AddPartyProvision { inv_idx: 0, qty: 1 });
        assert_eq!(s.party_provisions.len(), 3, "rack full");
        s.inventory.push(party("feast", 5, true));
        crate::events::apply_input(&mut s, crate::events::InputEvent::AddPartyProvision { inv_idx: 0, qty: 3 });
        let slot = s.party_provisions.iter().find(|p| p.item.def_id == "feast").unwrap();
        assert_eq!((slot.item.quantity, slot.key, slot.ready_tick), (4, key, ready), "topped up in place");
        assert_eq!(s.inventory[0].quantity, 2, "only what was asked left the bag");
        assert_eq!(crate::skills::buff_sum(&s.grid.entities[0], BuffKind::AtkPct), 100, "the running buff is untouched");
        // A tradeable feast is a different item for the rack: refused, nothing debited.
        s.inventory.push(party("feast", 2, false));
        let idx = s.inventory.len() as u32 - 1;
        crate::events::apply_input(&mut s, crate::events::InputEvent::AddPartyProvision { inv_idx: idx, qty: 2 });
        assert_eq!(s.inventory[idx as usize].quantity, 2);
        assert_eq!(s.party_provisions.iter().find(|p| p.item.def_id == "feast").unwrap().item.quantity, 4);
    }

    /// A full bag must not eat the item. `try_grant_item` banks nothing when it fails, so
    /// unracking first and granting after deletes it on the one press a full-bag player makes.
    #[test]
    fn unracking_into_a_full_bag_is_a_no_op_not_a_deletion() {
        use crate::items::ProvisionKind;
        let mut it = Item::new("feast", ItemKind::Consumable, 0, 0, Rarity::Common);
        it.quantity = 5;
        it.consumable = Some(ConsumableSpec {
            provision: Some(ProvisionKind::Party),
            trigger: ConsumeTrigger::BuffMissing(BuffKind::AtkPct),
            effect: ConsumeEffect::Buff { kind: BuffKind::AtkPct, magnitude: 100, ticks: 600 },
            cooldown: 0,
        });
        let mut s = player_with(it.clone(), 200, 200);
        s.inventory.clear();
        s.inventory.push(it);
        crate::events::apply_input(&mut s, crate::events::InputEvent::AddPartyProvision { inv_idx: 0, qty: 5 });
        assert_eq!(s.party_provisions.len(), 1);
        // Fill the bag to its cap with items that cannot merge with the coupon.
        s.inventory.clear();
        let cap = s.inventory_cap();
        for i in 0..cap {
            s.inventory.push(Item::new(&format!("filler_{i}"), ItemKind::Junk, 0, 0, Rarity::Common));
        }
        crate::events::apply_input(&mut s, crate::events::InputEvent::RemovePartyProvision { slot: 0 });
        assert_eq!(s.party_provisions.len(), 1, "the item was unracked into a bag that refused it");
        assert!(!s.inventory.iter().any(|i| i.def_id == "feast"), "it was banked past the cap");
    }

    /// A PROVISION MAY NOT SIT IN THE EMERGENCY BAR, and the sim is what says so.
    ///
    /// The reserved slots exist so one Meal and one Tonic are the limit. `provisions_view.gd`
    /// filtered meals out of its own picker; `character_view_bag.gd` routed any Consumable
    /// straight to `add_consumable`, so the rule held on one of the two screens that reach it —
    /// which is the same as not holding. Four meals at once sum in `buff_sum`.
    #[test]
    fn a_meal_cannot_be_parked_in_the_emergency_bar() {
        use crate::items::ProvisionKind;
        let mut meal = Item::new("meal", ItemKind::Consumable, 0, 0, Rarity::Common);
        meal.quantity = 3;
        meal.consumable = Some(ConsumableSpec {
            provision: Some(ProvisionKind::Meal),
            trigger: ConsumeTrigger::BuffMissing(BuffKind::AtkPct),
            effect: ConsumeEffect::Buff { kind: BuffKind::AtkPct, magnitude: 600, ticks: 600 },
            cooldown: 0,
        });
        let mut s = player_with(meal.clone(), 200, 200);
        s.grid.entities[0].consumables.clear();
        s.inventory.clear();
        s.inventory.push(meal);
        let eid = s.grid.entities[0].id;
        crate::events::apply_input(&mut s, crate::events::InputEvent::AddConsumable {
            entity_id: eid, inv_idx: 0,
        });
        assert!(s.grid.entities[0].consumables.is_empty(),
            "a Meal took an emergency slot — the one-per-kind rule is client-side only");
        assert_eq!(s.inventory.len(), 1, "and the bag was debited for it");
    }

    /// A slot-holder must be a party MEMBER — not a monster, not a pet. `idx_of_id` resolves any
    /// entity, so an InputEvent naming one would debit the bag and push the item onto something
    /// `consumable_upkeep` skips and the pruner deletes: the item simply gone. Not reachable
    /// through the UI, which is not the bar — a replay-verify server accepts what the log holds.
    #[test]
    fn a_pet_cannot_hold_a_consumable_slot() {
        let mut pot = Item::new("pot", ItemKind::Consumable, 0, 0, Rarity::Common);
        pot.quantity = 2;
        pot.consumable = Some(ConsumableSpec {
            provision: None,
            trigger: ConsumeTrigger::HpBelowPct(50),
            effect: ConsumeEffect::HealHp(10),
            cooldown: 0,
        });
        let mut s = player_with(pot.clone(), 200, 200);
        s.inventory.clear();
        s.inventory.push(pot);
        // A PET, built from the member beside it, so the fixture always has one to aim at — an
        // earlier version looked for a monster, found none, and returned before asserting
        // anything at all.
        let mut pet = s.grid.entities[0].clone();
        pet.id = 4242;
        pet.is_pet = true;
        pet.consumables.clear();
        s.grid.entities.push(pet);
        crate::events::apply_input(&mut s, crate::events::InputEvent::AddConsumable {
            entity_id: 4242, inv_idx: 0,
        });
        assert_eq!(s.inventory.len(), 1, "the bag was debited to arm a pet");
        assert!(s.grid.entities.iter().all(|e| !e.is_pet || e.consumables.is_empty()),
            "a pet took a consumable slot");
    }

    /// Unslotting BANKS: it merges with the stack already in the bag instead of making a second
    /// one, and it refuses rather than pushing past the cap.
    #[test]
    fn unslotting_merges_with_the_bag_stack_instead_of_making_a_second() {
        let mut pot = Item::new("pot", ItemKind::Consumable, 0, 0, Rarity::Common);
        pot.quantity = 10;
        pot.consumable = Some(ConsumableSpec {
            provision: None,
            trigger: ConsumeTrigger::HpBelowPct(1),
            effect: ConsumeEffect::HealHp(10),
            cooldown: 0,
        });
        let mut s = player_with(pot.clone(), 200, 200);
        s.grid.entities[0].consumables.clear();
        s.grid.entities[0].consumables.push(ConsumableSlot { key: 0, item: pot.clone(), ready_tick: 0 });
        s.inventory.clear();
        let mut more = pot.clone();
        more.quantity = 3;
        s.inventory.push(more);
        let eid = s.grid.entities[0].id;
        crate::events::apply_input(&mut s, crate::events::InputEvent::RemoveConsumable {
            entity_id: eid, slot: 0,
        });
        let stacks = s.inventory.iter().filter(|i| i.def_id == "pot").count();
        assert_eq!(stacks, 1, "unslotting made a SECOND stack of one consumable");
        assert_eq!(s.inventory.iter().find(|i| i.def_id == "pot").map(|i| i.quantity), Some(13));
    }

    /// SWAPPING A PROVISION MUST WORK. The replacement gets a key that is free in the Vec but
    /// still stamped on the entity by the departed item's running buff, so it saw "already
    /// buffed", never fired and never drained — a paid-for upgrade inert for up to two hours.
    #[test]
    fn swapping_a_provision_does_not_leave_the_replacement_inert() {
        use crate::items::ProvisionKind;
        let tonic = |id: &str, mag: i32| {
            let mut it = Item::new(id, ItemKind::Consumable, 0, 0, Rarity::Common);
            it.quantity = 5;
            it.consumable = Some(ConsumableSpec {
                provision: Some(ProvisionKind::Tonic),
                trigger: ConsumeTrigger::BuffMissing(BuffKind::AspdPotionPct),
                effect: ConsumeEffect::Buff { kind: BuffKind::AspdPotionPct, magnitude: mag, ticks: 18000 },
                cooldown: 0,
            });
            it
        };
        let mut s = player_with(tonic("weak", 500), 200, 200);
        s.grid.entities[0].consumables.clear();
        s.grid.entities[0].provisions.clear();
        s.inventory.clear();
        s.inventory.push(tonic("weak", 500));
        let eid = s.grid.entities[0].id;
        crate::events::apply_input(&mut s, crate::events::InputEvent::AddProvision {
            entity_id: eid, inv_idx: 0, qty: 1,
        });
        consumable_upkeep(&mut s);
        assert_eq!(crate::skills::buff_sum(&s.grid.entities[0], BuffKind::AspdPotionPct), 500);

        // UPGRADE. The old tonic's buff keeps running — nothing strips it — so its key is not free.
        s.inventory.push(tonic("strong", 1500));
        let idx = s.inventory.len() as u32 - 1;
        // Several charges: a one-charge slot is emptied by its first fire and swept, which would
        // make this a test about depletion instead of about the swap.
        crate::events::apply_input(&mut s, crate::events::InputEvent::AddProvision {
            entity_id: eid, inv_idx: idx, qty: 4,
        });
        assert_eq!(s.grid.entities[0].provisions.len(), 1, "the swap did not happen");
        assert_eq!(s.grid.entities[0].provisions[0].item.def_id, "strong");
        s.tick += 10;
        consumable_upkeep(&mut s);
        assert_eq!(s.grid.entities[0].provisions[0].item.quantity, 3,
            "the replacement never fired — it inherited the old tonic's live source");
        assert_eq!(crate::skills::buff_sum(&s.grid.entities[0], BuffKind::AspdPotionPct), 2000,
            "the upgrade did not apply alongside the buff already paid for");
    }

    /// A `BuffMissing` trigger paired with an effect that grants no buff must not drain. The
    /// trigger never stops being true, so it would empty the stack a charge per cooldown while
    /// applying nothing.
    #[test]
    fn a_trigger_that_can_never_be_satisfied_does_not_drain_the_stack() {
        let spec = ConsumableSpec {
            provision: None,
            trigger: ConsumeTrigger::BuffMissing(BuffKind::AspdPotionPct),
            effect: ConsumeEffect::HealHp(10),
            cooldown: 0,
        };
        // WOUNDED, not at full health. At full HP `HealHp` is a no-op whatever the guard does, so
        // an earlier version of this test passed against a build where the guard ran AFTER the
        // effect — the pairing healed for free every tick and never depleted, and the assertion on
        // `quantity` was satisfied by exactly that bug.
        let mut s = player_with(cons(spec, 5), 50, 200);
        let hp0 = s.grid.entities[0].hp;
        for _ in 0..4 {
            s.tick += 10;
            consumable_upkeep(&mut s);
        }
        assert_eq!(s.grid.entities[0].consumables[0].item.quantity, 5,
            "charges were spent on an effect that grants the buff the trigger waits for");
        assert_eq!(s.grid.entities[0].hp, hp0,
            "it healed for free — the guard ran after the effect instead of before it");
    }

    /// The key search must be BOUNDED. Orphaned buffs are unbounded — removing a slot leaves its
    /// buff running on purpose — so a search that skips every live source climbs without limit,
    /// and `k` is a `u8` under `overflow-checks = true` in both profiles.
    ///
    /// That is not a wrap, it is a PANIC, and verify-server links this crate and replays
    /// client-authored logs: a crafted log of slot/unslot pairs aborts the server process.
    #[test]
    fn the_key_search_cannot_be_walked_off_the_end_of_a_u8() {
        let spec = ConsumableSpec {
            provision: None,
            trigger: ConsumeTrigger::BuffMissing(BuffKind::AspdPotionPct),
            effect: ConsumeEffect::Buff { kind: BuffKind::AspdPotionPct, magnitude: 1, ticks: 100_000 },
            cooldown: 0,
        };
        let mut s = player_with(cons(spec, 1), 200, 200);
        s.grid.entities[0].consumables.clear();
        // Slot, fire, unslot — 300 times. Each cycle strands one more buff on a source no slot
        // holds, which is exactly what the old search walked past one key at a time.
        for _ in 0..300 {
            s.grid.entities[0]
                .consumables
                .push(ConsumableSlot { key: 0, item: cons(spec, 1), ready_tick: 0 });
            s.tick += 1;
            consumable_upkeep(&mut s);
            s.grid.entities[0].consumables.clear();
        }
        // REACHING THIS LINE IS THE ASSERTION — the old unbounded search panics on `k += 1`
        // partway through, and `overflow-checks` is on in the test profile too.
        //
        // And the count proves the search SATURATES rather than escaping: 300 cycles leave exactly
        // one live buff per key in the band, because keys are reused once every candidate carries
        // one. An unbounded search would instead have stranded 300 distinct sources.
        // SATURATES AT THE CONTAINER'S CAPACITY, not at the source band. 300 slot/unslot cycles
        // leave at most one live buff per key, and the key space is as wide as the bar can hold —
        // so re-adding one item can self-stack three times, matching the honest ceiling of three
        // slots. Searching the whole 16-wide band would have made it a 16x self-buff for the price
        // of the charges.
        assert_eq!(
            s.grid.entities[0].buffs.len(),
            crate::events::CONSUMABLE_SLOTS,
            "keys did not saturate at the slot capacity — the search escaped it or never reused one"
        );
    }

    /// The rack must not go DEAD after a removal, and must never eat the units on a refusal.
    ///
    /// Its key space is exactly as wide as the rack (3), and un-racking deliberately leaves the
    /// buff running — so with a long buff, taking one item out could make every later add fail for
    /// the buff's whole duration. The add also debited the bag BEFORE the key search could decline,
    /// so each of those failures destroyed the units it had just taken.
    #[test]
    fn the_rack_still_accepts_after_a_removal_and_never_eats_the_units() {
        use crate::items::ProvisionKind;
        let party = |id: &str| {
            let mut it = Item::new(id, ItemKind::Consumable, 0, 0, Rarity::Common);
            it.quantity = 4;
            it.consumable = Some(ConsumableSpec {
                provision: Some(ProvisionKind::Party),
                trigger: ConsumeTrigger::BuffMissing(BuffKind::AtkPct),
                effect: ConsumeEffect::Buff { kind: BuffKind::AtkPct, magnitude: 10, ticks: 100_000 },
                cooldown: 0,
            });
            it
        };
        let mut s = player_with(party("seed"), 200, 200);
        s.inventory.clear();
        // Fill all three slots, let them stamp their buffs, then empty the rack. Every key is now
        // free in the Vec and still radiating.
        for id in ["a", "b", "c"] {
            s.inventory.push(party(id));
            let idx = s.inventory.len() as u32 - 1;
            crate::events::apply_input(&mut s, crate::events::InputEvent::AddPartyProvision { inv_idx: idx, qty: 4 });
        }
        party_provision_upkeep(&mut s);
        for _ in 0..3 {
            crate::events::apply_input(&mut s, crate::events::InputEvent::RemovePartyProvision { slot: 0 });
        }
        assert!(s.party_provisions.is_empty());
        assert!(s.grid.entities[0].buffs.iter().any(|b| b.kind == BuffKind::AtkPct),
            "the fixture never left a live buff — it cannot exercise the blocked-key case");

        // A fresh item must still go in. Every key is blocked by an orphan, and refusing here is
        // what made the rack dead.
        s.inventory.push(party("d"));
        let idx = s.inventory.len() as u32 - 1;
        let before = s.inventory.len();
        crate::events::apply_input(&mut s, crate::events::InputEvent::AddPartyProvision { inv_idx: idx, qty: 4 });
        assert_eq!(s.party_provisions.len(), 1,
            "the rack refused every add because each key still carried an orphaned buff");
        // And the units are accounted for: either racked or still in the bag, never neither.
        let in_bag = s.inventory.iter().filter(|i| i.def_id == "d").count();
        assert_eq!(in_bag + s.party_provisions.len(), 1, "the units were destroyed on the way in");
        assert_eq!(s.inventory.len(), before - 1, "the bag was charged for something that vanished");
    }

    /// THE ONE RULE: the same item cannot occupy two rack slots. A different item granting the
    /// same buff kind is legal and must still be accepted.
    #[test]
    fn the_rack_refuses_a_duplicate_item_but_not_a_duplicate_buff() {
        use crate::items::ProvisionKind;
        let make = |id: &str| {
            let mut it = Item::new(id, ItemKind::Consumable, 0, 0, Rarity::Common);
            it.quantity = 4;
            it.consumable = Some(ConsumableSpec {
                provision: Some(ProvisionKind::Party),
                trigger: ConsumeTrigger::BuffMissing(BuffKind::AtkPct),
                effect: ConsumeEffect::Buff { kind: BuffKind::AtkPct, magnitude: 10, ticks: 600 },
                cooldown: 50,
            });
            it
        };
        let mut s = player_with(make("a"), 200, 200);
        s.inventory.clear();
        for id in ["a", "a", "b"] {
            s.inventory.push(make(id));
            let idx = s.inventory.len() as u32 - 1;
            crate::events::apply_input(&mut s, crate::events::InputEvent::AddPartyProvision {
                inv_idx: idx, qty: 1,
            });
        }
        let ids: Vec<&str> = s.party_provisions.iter().map(|sl| sl.item.def_id.as_str()).collect();
        assert_eq!(ids, vec!["a", "b"], "a duplicate ITEM entered the rack, or `b` was refused");
    }

    fn cons(spec: ConsumableSpec, qty: u32) -> Item {
        let mut p = Item::new("loot", ItemKind::Consumable, 0, 0, Rarity::Common);
        p.quantity = qty;
        p.consumable = Some(spec);
        p
    }


    #[test]
    fn rejuvenating_heals_over_time_and_caps() {
        let spec = ConsumableSpec { provision: None,
            trigger: ConsumeTrigger::BuffMissing(BuffKind::HealHot),
            effect: ConsumeEffect::Buff { kind: BuffKind::HealHot, magnitude: 5, ticks: 36000 },
            cooldown: 100,
        };
        let mut s = player_with(cons(spec, 1), 100, 200);
        consumable_upkeep(&mut s); // applies the HoT buff
        assert!(s.grid.entities[0].buffs.iter().any(|b| matches!(b.kind, BuffKind::HealHot)));
        decay_buffs(&mut s); // tick 0 == regen interval → +5
        assert_eq!(s.grid.entities[0].hp, 105, "HoT healed 5/interval");
        s.grid.entities[0].hp = 199;
        decay_buffs(&mut s);
        assert_eq!(s.grid.entities[0].hp, 200, "HoT caps at max HP");
    }

    #[test]
    fn phoenix_tail_revives_downed_ally() {
        let spec = ConsumableSpec { provision: None,
            trigger: ConsumeTrigger::AllyDown,
            effect: ConsumeEffect::ReviveAlly { hp_pct: 50 },
            cooldown: 600,
        };
        let mut s = player_with(cons(spec, 1), 100, 100); // holder id1, alive
        let mut ally = Entity::idle(2, Cell::new(1, 0));
        ally.team = Team::Player;
        ally.max_hp = 200;
        ally.hp = 0;
        ally.alive = false;
        s.grid.spawn(ally);
        phoenix_revive_upkeep(&mut s);
        let a = &s.grid.entities[1];
        assert!(a.alive, "downed ally revived");
        assert_eq!(a.hp, 100, "revived at 50% of 200 max HP");
        assert!(s.grid.entities[0].consumables.is_empty(), "Phoenix charge spent");
    }
}

#[cfg(test)]
mod freeze_resist_binding_tests {
    /// GLACIAL STORM'S FREEZE RATE USED TO BE BOUND TO THE MDEF COLUMN, AND NOTHING SAID SO.
    ///
    /// The roll read `effective_mdef` as PERCENTAGE POINTS — `chance x (97 - MDEF - LUK/3)/100` —
    /// so two unrelated edits to the mdef column silently made Glacial Storm half again as likely
    /// to freeze a level-98 mob, with no test to notice. MDEF is gone entirely now (0.0.85-87 took
    /// it out of the damage path, 0.0.332 out of the code), and the resist term is the constant
    /// 97, so the binding this test existed to pin no longer exists.
    ///
    /// What is left worth asserting is that it is a CONSTANT: a freeze lands at the skill's own
    /// chance and nothing about the target changes it. Elemental RES% is what will take that job.
    #[test]
    fn nothing_about_a_target_changes_its_freeze_chance() {
        assert_eq!(super::FREEZE_RESIST_BASE, 97, "the resist term is a constant, not a stat");
    }
}
