//! Non-attack active casts: Acolyte ally heals (`try_cast_support`) and
//! proactive pure-buff casts (`try_cast_buffs`).

// DEBT (154 sites): this file predates `deny(clippy::indexing_slicing)`. An index that
// cannot be proven in range is a panic, and in verify-server (which links this crate and
// replays saves under `panic = "abort"`) a panic is the whole process. Shrink this, never
// add to it — a NEW file has no allow and is rejected on its first bare index.
#![allow(clippy::indexing_slicing)]

use crate::{combat, skills};
use crate::state::GameState;

/// Acolyte support: if `i` has a ready ally-heal skill and an injured
/// ally is in range, heal the MOST-injured one (lowest HP fraction) and
/// return true (healing was this entity's action). Heal restores
/// `power_bp`% of the caster's MATK. Deterministic (no PRNG).
pub(crate) fn try_cast_support(state: &mut GameState, i: usize) -> bool {
    // Find the highest-priority ready heal skill (list order). Cooldown-only.
    let mut chosen: Option<(usize, u32, u32, i32, u8)> = None; // slot,power,cd,range,skill_lv
    for (slot, s) in state.grid.entities[i].skills.iter().enumerate() {
        let Some(def) = skills::skill_def(&s.id) else { continue; };
        if !def.heals_ally || !matches!(def.kind, skills::SkillKind::Active) {
            continue;
        }
        if !skills::loadout_fits(
            &state.grid.entities[i].active_skills,
            combat::loadout_capacity(&state.grid.entities[i]),
            &s.id,
        ) {
            continue; // not slotted / over the loadout budget → doesn't fire
        }
        if s.cooldown > 0 || !has_charge(s, def) {
            continue;
        }
        chosen = Some((slot, crate::unit_rank::power(&state.grid.entities[i], def, s.level), def.cooldown_ticks, def.range, s.level));
        break;
    }
    let Some((slot, power_bp, cooldown, range, skill_level)) = chosen else { return false; };

    // Most-injured living ally (same team) within range, hurt enough to be
    // worth a cast: only heal allies BELOW this fraction of max HP (don't
    // burn SP topping off a scratch).
    const HEAL_THRESHOLD_PCT: i64 = 70;
    let me_pos = state.grid.entities[i].pos;
    let me_team = state.grid.entities[i].team;
    let mut best: Option<(i64, u32, usize)> = None; // (hp_fraction_bp, id, idx)
    let self_only = skills::skill_def(&state.grid.entities[i].skills[slot].id).is_some_and(|d| d.heal_self_only);
    for (j, e) in state.grid.entities.iter().enumerate() {
        if !e.alive || e.team != me_team || (self_only && j != i) {
            continue;
        }
        let maxhp = combat::effective_max_hp(e);
        if (e.hp as i64) * 100 >= (maxhp as i64) * HEAL_THRESHOLD_PCT
            || me_pos.chebyshev(e.pos) > range
        {
            continue;
        }
        let frac = (e.hp as i64) * 10_000 / (maxhp.max(1) as i64);
        let key = (frac, e.id, j);
        match best {
            Some((bf, bid, _)) if (bf, bid) <= (key.0, key.1) => {}
            _ => best = Some(key),
        }
    }
    let Some((_, _, tidx)) = best else { return false; }; // nobody hurt in range

    // Cast: set cooldown, restore HP. RO RENEWAL Heal formula (irowiki):
    //   HP = ⌊(BaseLevel + INT) / 5⌋ × SkillLevel × 3 + MATK
    // (× healing modifiers — we have none yet, so ×1.) The old code used RAW
    // MATK scaled ~×5.7, which a rod + INT cards inflated into an over-heal;
    // Renewal makes BaseLevel + INT the primary driver and MATK only a flat
    // additive term. `power_bp` is a tuning scalar (10000 = the exact Renewal
    // amount; lower softens it for our smaller HP pools). INT/MATK are the
    // BUFFED values, so Blessing / INT cards feed it the RO way.
    {
        // Gear-mod CDR (accessory pool, cap 30%) scales the standard cast cooldown.
        let cd = crate::combat::stat_mods::scale_cooldown(&state.grid.entities[i], cooldown);
        state.grid.entities[i].skills[slot].cooldown = cd;
    }
    if let Some(def) = skills::skill_def(&state.grid.entities[i].skills[slot].id) {
        spend_charge(&mut state.grid.entities[i].skills[slot], def);
    }
    state.grid.entities[i].skills[slot].last_fired = state.tick;
    // Casting-animation lock (Heal's 0.5s motion) — a motion lock, not a cast bar.
    let md = skills::skill_def(&state.grid.entities[i].skills[slot].id)
        .map_or(0, |d| d.motion_delay_ticks);
    if md > 0 {
        state.grid.entities[i].act_locked_until = state.tick + md as u64;
    }
    // Surface the cast so the animator plays Heal's cast clip (no damage → no swing pipeline).
    {
        let fx_sid = state.grid.entities[i].skills[slot].id.clone();
        let fx_caster = state.grid.entities[i].id;
        state.skill_fx.push(crate::state::SkillFx { caster_id: fx_caster, target_id: fx_caster, skill_id: fx_sid });
    }
    // A `heal_maxhp_bp` skill (monster healer, e.g. Dustelle) heals a % of the TARGET's max HP
    // so it scales with a monster's big HP pool; everything else uses the INT/MATK Renewal heal.
    let heal_maxhp_bp = skills::skill_def(&state.grid.entities[i].skills[slot].id)
        .map_or(0, |d| d.heal_maxhp_bp + d.heal_maxhp_per_level_bp * u32::from(skill_level.saturating_sub(1)));
    let heal_scales_atk = skills::skill_def(&state.grid.entities[i].skills[slot].id)
        .is_some_and(|d| d.heal_scales_atk);
    let mut heal = if heal_maxhp_bp > 0 {
        ((combat::effective_max_hp(&state.grid.entities[tidx]) as i64) * heal_maxhp_bp as i64
            / 10_000)
            .max(1) as i32
    } else if heal_scales_atk {
        // Lux Medica (0.0.421): `power_at(level)` of the caster's ATK, with the heal-effect
        // gear line on top — a Support whose heal rides the same number as her damage.
        let atk = combat::effective_atk(&state.grid.entities[i]) as i64;
        let heal_bp = crate::combat::stat_mods::heal_bp(&state.grid.entities[i]) as i64;
        ((atk * power_bp as i64 / 10_000) * (10_000 + heal_bp) / 10_000).max(1) as i32
    } else {
        renewal_heal_amount(&state.grid.entities[i], skill_level, power_bp)
    };
    // Rechum card (legacy set): +25→50% on the HEAL skill's output ONLY — this
    // cast path also serves other heals_ally skills, so gate on the id; Dupla /
    // Subvenio / proc-heals are deliberately untouched.
    if state.grid.entities[i].skills[slot].id == "heal" {
        let bonus = combat::card_specials::heal_skill_bonus_bp(&state.grid.entities[i]);
        if bonus > 0 {
            heal = ((heal as i64) * (10_000 + bonus as i64) / 10_000) as i32;
        }
    }
    super::apply_skill_heal_from(state, i, tidx, heal);
    communio_after_cast(state, i);
    // AoE heal (Mending Dirge, `aoe_radius` > 0): splash the heal to every OTHER ally within
    // radius of the primary target — each mended for its OWN %-max-HP (or the flat renewal amount).
    // Monster healers carry no weapon runes, so the rune HoT/smite below stays on the primary only.
    if let Some(def) = skills::skill_def(&state.grid.entities[i].skills[slot].id) {
        if def.aoe_radius > 0 {
            let center = state.grid.entities[tidx].pos;
            let team = state.grid.entities[i].team;
            let radius = def.aoe_radius;
            let splash: Vec<usize> = state
                .grid
                .entities
                .iter()
                .enumerate()
                .filter(|(j, e)| {
                    // `*j != tidx` excludes the PRIMARY target by index, not by cell — it already
                    // took the full heal. That is a policy of this call site and stays here.
                    *j != tidx && e.alive && e.team == team
                        && crate::grid::Blast::radius(center, radius).contains(e.pos)
                })
                .map(|(j, _)| j)
                .collect();
            for t in splash {
                let h = if heal_maxhp_bp > 0 {
                    ((combat::effective_max_hp(&state.grid.entities[t]) as i64)
                        * heal_maxhp_bp as i64
                        / 10_000)
                        .max(1) as i32
                } else {
                    heal
                };
                super::apply_skill_heal_from(state, i, t, h);
            }
        }
    }
    true
}

/// Heal amount: ⌊BaseLevel/5⌋ × skill_level × 3 + ATK, scaled by `power_bp`
/// (10000 = full). The RO formula had an INT term here; INT feeds nothing since
/// 0.0.314, so the level term carries it alone. Shared by Heal, Dupla Medicina,
/// and Subvenio so the formula has one source of truth.
/// VOLLEY (0.0.423 — Nightfall Volley): a `volley_targets` skill fires one instance on each of
/// the N lowest-HP living enemies in range (distinct; ties by spawn order). Counts as the turn.
/// Each instance is a full `resolve_attack_scaled` (its own crit/dodge draws, in target order),
/// so the PRNG stream is fixed by the target list, which is fixed by HP then id.
pub(crate) fn try_cast_volley(state: &mut GameState, i: usize) -> bool {
    // A volley ULTIMATE (0.0.462, Airi's Storm Volley) waits on its gauge like every other
    // ultimate path; a volley skill with no gauge is cooldown-gated as before.
    let gauge = state.grid.entities[i].limit_break;
    let found = state.grid.entities[i]
        .skills
        .iter()
        .enumerate()
        .find(|(_, s)| s.level > 0 && s.cooldown == 0 && skills::skill_def(&s.id).is_some_and(|d| d.volley_targets > 0 && matches!(d.kind, skills::SkillKind::Active) && (d.lb_max == 0 || gauge >= d.lb_max)))
        .map(|(slot, s)| (slot, s.id.clone(), s.level));
    let Some((slot, sid, level)) = found else { return false };
    if !skills::loadout_fits(&state.grid.entities[i].active_skills, combat::loadout_capacity(&state.grid.entities[i]), &sid) {
        return false;
    }
    let Some(def) = skills::skill_def(&sid) else { return false };
    let (me_pos, me_team) = (state.grid.entities[i].pos, state.grid.entities[i].team);
    let mut targets: Vec<(i32, u32, usize)> = state
        .grid
        .entities
        .iter()
        .enumerate()
        .filter(|(_, e)| e.alive && e.team != me_team && e.on_board() && me_pos.chebyshev(e.pos) <= def.range)
        .map(|(j, e)| (e.hp, e.id, j))
        .collect();
    if targets.is_empty() {
        return false;
    }
    targets.sort();
    let take = def.volley_targets.saturating_add(crate::unit_rank::volley_extra_targets(&state.grid.entities[i]));
    let capped_gauge = crate::unit_rank::gauge_on_capped_mark(&state.grid.entities[i]);
    let mark_cap: i32 = {
        let e = &state.grid.entities[i];
        let (step, stacks) = e.skills.iter().filter(|s| s.level > 0).filter_map(|s| skills::skill_def(&s.id))
            .fold((0i32, 0i32), |(a, b), d| (a.max(d.mark_step_bp as i32), b.max(d.mark_max_stacks as i32)));
        if stacks > 0 { step * (stacks + crate::unit_rank::mark_extra_stacks(e)) } else { 0 }
    };
    targets.truncate(take as usize);
    let power = crate::unit_rank::power(&state.grid.entities[i], def, level);
    let kind = def.damage_type.unwrap_or(state.grid.entities[i].atk_type);
    {
        let cd = crate::combat::stat_mods::scale_cooldown(&state.grid.entities[i], def.cooldown_ticks);
        state.grid.entities[i].skills[slot].cooldown = cd;
    }
    state.grid.entities[i].skills[slot].last_fired = state.tick;
    let me = state.grid.entities[i].id;
    for (_, _, j) in targets {
        if !state.grid.entities[j].alive {
            continue;
        }
        let pre_hp = state.grid.entities[j].hp.max(0);
        let tid = state.grid.entities[j].id;
        let dealt = combat::resolve_attack_scaled(
            &mut state.grid.entities, &mut state.rng, i, j, power, 1,
            &mut state.recent_hits, kind, def.toughness_dmg_mult,
        );
        state.record_damage(i, Some(&sid), dealt.min(pre_hp));
        state.skill_fx.push(crate::state::SkillFx { caster_id: me, target_id: tid, skill_id: sid.clone() });
        // Aveline S2: an arrow on a target whose mark is at its cap feeds the gauge.
        if capped_gauge > 0 && state.grid.entities[j].alive {
            let cap = mark_cap;
            if cap > 0 && crate::skills::shadow_vuln_bp(&state.grid.entities[j]) >= cap {
                crate::skills::charge_limit_break(&mut state.grid.entities[i], capped_gauge);
            }
        }
        if !state.grid.entities[j].alive {
            crate::tick::combat_step::kill_monster(state, j, Some(i));
        }
    }
    if def.lb_max > 0 {
        state.grid.entities[i].limit_break = 0;
        crate::unit_rank::after_ult(state, i, &sid);
    } else {
        let gain = crate::skills::lb_gain(&state.grid.entities[i].skills, |d| d.lb_on_skill);
        crate::skills::charge_limit_break(&mut state.grid.entities[i], gain);
    }
    if def.motion_delay_ticks > 0 {
        state.grid.entities[i].act_locked_until = state.tick + def.motion_delay_ticks as u64;
    }
    communio_after_cast(state, i);
    true
}

/// ZONE-WIDE STRIKE (0.0.426, Nyx's Umbral Requiem): one instance of `power_at(level)` on EVERY
/// living enemy on the board, then both on-hit debuffs on each. Gated on the Limit Break gauge
/// when the skill is an ultimate (`lb_max`), spent here. Spawn-order resolution.
pub(crate) fn try_cast_zone_wide(state: &mut GameState, i: usize) -> bool {
    let found = state.grid.entities[i]
        .skills
        .iter()
        .enumerate()
        .find(|(_, s)| s.level > 0 && s.cooldown == 0 && skills::skill_def(&s.id).is_some_and(|d| d.zone_wide && matches!(d.kind, skills::SkillKind::Active)))
        .map(|(slot, s)| (slot, s.id.clone(), s.level));
    let Some((slot, sid, level)) = found else { return false };
    let Some(def) = skills::skill_def(&sid) else { return false };
    if def.lb_max > 0 && state.grid.entities[i].limit_break < def.lb_max {
        return false;
    }
    if !skills::loadout_fits(&state.grid.entities[i].active_skills, combat::loadout_capacity(&state.grid.entities[i]), &sid) {
        return false;
    }
    let me_team = state.grid.entities[i].team;
    let targets: Vec<usize> = state
        .grid
        .entities
        .iter()
        .enumerate()
        .filter(|(_, e)| e.alive && e.team != me_team && e.on_board())
        .map(|(j, _)| j)
        .collect();
    if targets.is_empty() {
        return false;
    }
    let power = crate::unit_rank::power(&state.grid.entities[i], def, level);
    let kind = def.damage_type.unwrap_or(state.grid.entities[i].atk_type);
    if def.lb_max > 0 {
        state.grid.entities[i].limit_break = 0;
    }
    {
        let cd = crate::combat::stat_mods::scale_cooldown(&state.grid.entities[i], def.cooldown_ticks);
        state.grid.entities[i].skills[slot].cooldown = cd;
    }
    state.grid.entities[i].skills[slot].last_fired = state.tick;
    let me = state.grid.entities[i].id;
    let slot_h = skills::SkillSlot::new(slot);
    for j in targets {
        if !state.grid.entities[j].alive {
            continue;
        }
        let pre_hp = state.grid.entities[j].hp.max(0);
        let tid = state.grid.entities[j].id;
        let dealt = combat::resolve_attack_scaled(
            &mut state.grid.entities, &mut state.rng, i, j, power, 1,
            &mut state.recent_hits, kind, def.toughness_dmg_mult,
        );
        state.record_damage(i, Some(&sid), dealt.min(pre_hp));
        state.skill_fx.push(crate::state::SkillFx { caster_id: me, target_id: tid, skill_id: sid.clone() });
        crate::tick::combat_step::pickpocket(state, j, def.pickpocket_gil_bp);
        crate::tick::combat_step::lay_static_spark(state, i, j, def);
        if def.spark_detonate {
            let mut hv = std::mem::take(&mut state.recent_hits);
            combat::detonate_static_spark(&mut state.grid.entities, j, &mut hv);
            state.recent_hits = hv;
        }
        if !state.grid.entities[j].alive {
            crate::tick::combat_step::kill_monster(state, j, Some(i));
            continue;
        }
        if let Some(spec) = def.on_hit_debuff {
            crate::tick::combat_step::apply_on_hit_debuff(state, i, slot_h, j, spec);
        }
        if let Some(spec) = def.on_hit_debuff_2 {
            crate::tick::combat_step::apply_on_hit_debuff(state, i, slot_h, j, spec);
        }
    }
    if def.lb_max == 0 {
        let gain = crate::skills::lb_gain(&state.grid.entities[i].skills, |d| d.lb_on_skill);
        crate::skills::charge_limit_break(&mut state.grid.entities[i], gain);
    } else {
        crate::unit_rank::after_ult(state, i, &sid);
    }
    if def.motion_delay_ticks > 0 {
        state.grid.entities[i].act_locked_until = state.tick + def.motion_delay_ticks as u64;
    }
    communio_after_cast(state, i);
    true
}

/// COMMUNIO (0.0.421): after entity `i` completes any active cast, heal every living ally that
/// shares its element or is a Sentinel-doctrine unit for the passive's share of its ATK. Reads
/// the caster's own `communio_atk_bp` passive; 0 (everyone but Lucielle) is a no-op. Called from
/// every cast site — support, buff, strike — so the heal follows the cast wherever it resolves.
pub(crate) fn communio_after_cast(state: &mut GameState, i: usize) {
    let (bp, pact_bp, pact_ticks) = state.grid.entities[i]
        .skills
        .iter()
        .filter(|s| s.level > 0)
        .filter_map(|s| skills::skill_def(&s.id))
        .fold((0u32, 0u32, 0u32), |(c, p, t), d| (c.max(d.communio_atk_bp), p.max(d.pact_crit_bp), t.max(d.pact_ticks)));
    if bp == 0 && pact_bp == 0 {
        return;
    }
    let (team, element, me) = {
        let c = &state.grid.entities[i];
        (c.team, c.element, c.id)
    };
    // S2 (0.0.441): Communio reaches EVERY ally, at the rank's rate.
    let all = crate::unit_rank::communio_all(&state.grid.entities[i]);
    let bp = if bp > 0 { all.unwrap_or(bp) } else { bp };
    if bp > 0 {
        let heal = ((combat::effective_atk(&state.grid.entities[i]) as i64) * bp as i64 / 10_000).max(1) as i32;
        let targets: Vec<usize> = state
            .grid
            .entities
            .iter()
            .enumerate()
            .filter(|(_, e)| {
                e.alive && e.team == team && e.id != me && !e.is_pet
                    && (all.is_some() || e.element == element
                        || crate::potential::doctrine_for_unit(&e.unit_def) == Some(crate::potential::Doctrine::Sentinel))
            })
            .map(|(j, _)| j)
            .collect();
        for j in targets {
            super::apply_skill_heal_from(state, i, j, heal);
        }
    }
    // TWILIGHT PACT (0.0.423): the damage-side twin — crit to the allies of the caster's element
    // or of the Striker doctrine, one instance per source (refreshed, never stacked).
    if pact_bp > 0 {
        for e in state.grid.entities.iter_mut() {
            if !(e.alive && e.team == team && e.id != me && !e.is_pet) {
                continue;
            }
            if !(e.element == element
                || crate::potential::doctrine_for_unit(&e.unit_def) == Some(crate::potential::Doctrine::Striker))
            {
                continue;
            }
            e.buffs.retain(|b| !(matches!(b.kind, skills::BuffKind::CritBpAdd) && b.source_id == me));
            e.buffs.push(skills::Buff { kind: skills::BuffKind::CritBpAdd, magnitude: pact_bp as i32, ticks: pact_ticks, source_id: me });
        }
    }
}

pub(crate) fn renewal_heal_amount(healer: &crate::grid::Entity, skill_level: u8, power_bp: u32) -> i32 {
    let base_level = healer.stats.level as i64;
    let matk = combat::effective_atk(healer) as i64;
    let ro_base = (base_level / 5) * (skill_level as i64) * 3 + matk;
    // Gear-mod HEAL EFFECTIVENESS (support pool): +% on the healer's output.
    let heal_bp = crate::combat::stat_mods::heal_bp(healer) as i64;
    ((ro_base * (power_bp as i64) / 10_000) * (10_000 + heal_bp) / 10_000).max(1) as i32
}

/// Dupla Medicina (Bishop): when ≥1 ally is below 75% HP, heal the TWO lowest-HP%
/// allies map-wide (including the priest) for `power_bp` of the Renewal Heal
/// formula at Dupla's level. Counts as this tick's action. Returns true when cast.
pub(crate) fn try_cast_dupla(state: &mut GameState, i: usize) -> bool {
    let found = state.grid.entities[i].ready_skill("dupla_medicina");
    let Some(level) = found else { return false; };
    if !skills::loadout_fits(
        &state.grid.entities[i].active_skills,
        combat::loadout_capacity(&state.grid.entities[i]),
        "dupla_medicina",
    ) {
        return false;
    }
    let Some(def) = skills::skill_def("dupla_medicina") else { return false; };
    let me_team = state.grid.entities[i].team;
    // Living allies sorted by HP%; fire only if the lowest is under 75%.
    let mut allies: Vec<(i64, usize)> = state
        .grid
        .entities
        .iter()
        .enumerate()
        .filter(|(_, e)| e.alive && e.team == me_team)
        .map(|(j, e)| {
            let maxhp = combat::effective_max_hp(e).max(1) as i64;
            ((e.hp as i64) * 10_000 / maxhp, j)
        })
        .collect();
    allies.sort_by_key(|(frac, _)| *frac);
    let Some(&(low_frac, _)) = allies.first() else { return false; };
    if low_frac >= 7_500 {
        return false; // nobody hurt enough
    }
    let heal = renewal_heal_amount(&state.grid.entities[i], level, def.power_bp);
    for &(_, j) in allies.iter().take(2) {
        super::apply_skill_heal_from(state, i, j, heal);
    }
    let cd = crate::combat::stat_mods::scale_cooldown(&state.grid.entities[i], def.cooldown_ticks);
    state.grid.entities[i].skills.iter_mut().for_each(|s| {
        if s.id == "dupla_medicina" {
            s.cooldown = cd;
            s.last_fired = state.tick;
        }
    });
    if def.motion_delay_ticks > 0 {
        state.grid.entities[i].act_locked_until = state.tick + def.motion_delay_ticks as u64;
    }
    true
}

/// Rivus (Astier): a heal that BOUNCES. It lands on the ally in the worst shape by PERCENT,
/// then twice more on the next two worst at `RIVUS_BOUNCE_PCT` of the first number.
///
/// Percent and not missing HP, because missing HP always points at the biggest health bar in
/// the party — the tank would eat every bounce while a 200-HP caster sat at a tenth of its life.
/// Ties break on entity index, which is spawn order, so the pick is deterministic.
///
/// Unlike `dupla_medicina` there is no "is anybody hurt enough" gate: a bouncing heal that only
/// fires under a threshold spends its whole cooldown waiting for the party to get hurt, which is
/// the one thing an idle party cannot ask a player to watch for. It fires when SOMEONE is missing
/// health at all, and the decay keeps the overheal small.
pub(crate) fn try_cast_rivus(state: &mut GameState, i: usize) -> bool {
    let Some(level) = state.grid.entities[i].ready_skill("rivus") else { return false; };
    if !skills::loadout_fits(
        &state.grid.entities[i].active_skills,
        combat::loadout_capacity(&state.grid.entities[i]),
        "rivus",
    ) {
        return false;
    }
    let Some(def) = skills::skill_def("rivus") else { return false; };
    let me_team = state.grid.entities[i].team;
    let mut allies: Vec<(i64, usize)> = state
        .grid
        .entities
        .iter()
        .enumerate()
        .filter(|(_, e)| e.alive && e.team == me_team)
        .map(|(j, e)| {
            let maxhp = combat::effective_max_hp(e).max(1) as i64;
            ((e.hp as i64) * 10_000 / maxhp, j)
        })
        .collect();
    allies.sort_by_key(|&(frac, j)| (frac, j));
    let Some(&(low_frac, _)) = allies.first() else { return false; };
    if low_frac >= 10_000 {
        return false; // nobody is missing anything — the stream has nowhere to go
    }
    let first = renewal_heal_amount(&state.grid.entities[i], level, def.power_bp);
    for (bounce, &(_, j)) in allies.iter().take(skills::RIVUS_BOUNCE_PCT.len()).enumerate() {
        let heal = ((first as i64) * skills::RIVUS_BOUNCE_PCT[bounce] / 100).max(1) as i32;
        super::apply_skill_heal_from(state, i, j, heal);
    }
    let cd = crate::combat::stat_mods::scale_cooldown(&state.grid.entities[i], def.cooldown_ticks);
    state.grid.entities[i].skills.iter_mut().for_each(|s| {
        if s.id == "rivus" {
            s.cooldown = cd;
            s.last_fired = state.tick;
        }
    });
    if def.motion_delay_ticks > 0 {
        state.grid.entities[i].act_locked_until = state.tick + def.motion_delay_ticks as u64;
    }
    true
}

/// Subvenio (Bishop): when an ALLY (not self) drops below 40% HP, blink to them
/// and Heal them for the priest's own Heal value (Renewal formula at the priest's
/// HEAL level). Sharing Heal's cooldown prevents a Subvenio + manual-Heal double
/// burst. Subvenio's own cd shrinks with level (120 → 60 s). Returns true when cast.
pub(crate) fn try_cast_subvenio(state: &mut GameState, i: usize) -> bool {
    let found = state.grid.entities[i].ready_skill("subvenio");
    let Some(level) = found else { return false; };
    if !skills::loadout_fits(
        &state.grid.entities[i].active_skills,
        combat::loadout_capacity(&state.grid.entities[i]),
        "subvenio",
    ) {
        return false;
    }
    let me_id = state.grid.entities[i].id;
    let me_team = state.grid.entities[i].team;
    // Lowest-HP% OTHER ally under 40%.
    let mut best: Option<(i64, usize)> = None;
    for (j, e) in state.grid.entities.iter().enumerate() {
        if !e.alive || e.team != me_team || e.id == me_id {
            continue;
        }
        let maxhp = combat::effective_max_hp(e).max(1) as i64;
        let frac = (e.hp as i64) * 10_000 / maxhp;
        if frac >= 4_000 {
            continue; // not below 40%
        }
        if best.map_or(true, |(bf, _)| frac < bf) {
            best = Some((frac, j));
        }
    }
    let Some((_, tidx)) = best else { return false; };
    // Heal value uses the priest's HEAL skill level (Bishops carry Heal).
    let heal_level = state
        .grid
        .entities[i]
        .skills
        .iter()
        .find(|s| s.id == "heal")
        .map_or(1, |s| s.level.max(1));
    let heal = renewal_heal_amount(&state.grid.entities[i], heal_level, 10_000);
    super::teleport_adjacent(state, i, tidx); // random free cell in the ally's 5×5
    super::apply_skill_heal_from(state, i, tidx, heal);
    // Per-level cooldown: 120/105/90/75/60 s (−15 s/level), in ticks. Subvenio is
    // SELF-CONTAINED — it runs on its OWN cooldown only and does NOT touch the Heal
    // skill's cooldown (a fired Subvenio no longer parks Heal).
    let cd = crate::combat::stat_mods::scale_cooldown(&state.grid.entities[i], (1200 - 150 * (level as u32 - 1)).max(600));
    for s in state.grid.entities[i].skills.iter_mut() {
        if s.id == "subvenio" {
            s.cooldown = cd;
            s.last_fired = state.tick;
            break;
        }
    }
    true
}

/// Cast a ready ACTIVE pure-buff skill (no damage, `power_bp == 0`): party
/// buffs (Blessing / Increase AGI → every living ally) or self-buffs (Energy
/// Coat / Improve Concentration → just the caster). Fires even with no enemy
/// around (like a heal) and is this entity's action for the tick. With
/// `cooldown_ticks ≈ duration` the buff is re-cast just as it lapses ⇒
/// near-100% uptime. Deterministic — no PRNG.
pub(crate) fn try_cast_buffs(state: &mut GameState, i: usize) -> bool {
    // Highest-priority ready pure-buff active (list order).
    let mut chosen: Option<(skills::SkillSlot, u32, u32, u32)> = None; // slot, cooldown, cast_time, cast_chance_bp
    for (slot, s) in state.grid.entities[i].skills.iter().enumerate() {
        let Some(def) = skills::skill_def(&s.id) else { continue; };
        if !matches!(def.kind, skills::SkillKind::Active)
            || def.cast_buff.is_empty()
            || def.power_bp != 0
        {
            continue;
        }
        // Must be slotted AND fit the loadout's slot budget (a buff skill that
        // overflows the budget is inert). `loadout_fits` subsumes the old
        // skill_slotted check.
        if !skills::loadout_fits(
            &state.grid.entities[i].active_skills,
            combat::loadout_capacity(&state.grid.entities[i]),
            &s.id,
        ) {
            continue;
        }
        if s.cooldown > 0 {
            continue;
        }
        // An ULTIMATE buff (Benedictio Lucis, 0.0.421) is gated by the Limit Break gauge, not a
        // clock — the generic picker would otherwise cast it every turn.
        if def.lb_max > 0 && state.grid.entities[i].limit_break < def.lb_max {
            continue;
        }
        // Health gates (`cast_hp_below_pct` / `cast_hp_above_pct`) and the use budget.
        if !hp_gate_open(&state.grid.entities[i], def) || !has_charge(s, def) {
            continue;
        }
        chosen =
                Some((skills::SkillSlot::new(slot), def.cooldown_ticks, def.cast_time_ticks, def.cast_chance_bp));
        break;
    }
    let Some((slot, cooldown, cast_time, cast_chance_bp)) = chosen else { return false; };
    let source = state.grid.entities[i].id;
    {
        // Gear-mod CDR (accessory pool, cap 30%) scales the standard cast cooldown.
        let cd = crate::combat::stat_mods::scale_cooldown(&state.grid.entities[i], cooldown);
        state.grid.entities[i].skills[slot.get()].cooldown = cd;
    }
    state.grid.entities[i].skills[slot.get()].last_fired = state.tick;
    // Per-attempt enrage chance: the cooldown is already latched above, so a FAILED roll just
    // burns this cycle and retries next time the skill comes off cooldown. A genuine PRNG combat
    // draw — only taken when a `cast_chance_bp` skill is actually ready, so it never perturbs the
    // stream for ordinary fights (default 0 = no draw).
    if cast_chance_bp > 0 && state.rng.gen_range_u32(0, 10_000) >= cast_chance_bp {
        return true;
    }
    // A limited-use buff spends its charge once it really casts (a failed chance roll above does
    // not); an interrupted channel has still spent it.
    if let Some(def) = skills::skill_def(&state.grid.entities[i].skills[slot.get()].id) {
        spend_charge(&mut state.grid.entities[i].skills[slot.get()], def);
    }
    // A cast_time buff (RO Increase AGI) CHANNELS — the buff lands when the windup
    // elapses via `fire_pending_cast`; an instant buff applies now. Cooldown is latched
    // above either way so it never re-triggers mid-channel.
    let windup = combat::effective_cast_time(&state.grid.entities[i], cast_time);
    if windup > 0 {
        state.grid.entities[i].casting = Some(crate::grid::Casting {
            ticks_left: windup,
            ticks_total: windup,
            slot,
            target_id: source,
            ground_cell: None, // a buff channel places no field
        });
        return true;
    }
    resolve_buff_cast(state, i, slot);
    true
}

/// Apply a pure-buff/aura skill's effect (party or self), lock the cast motion, and
/// surface the cast to the renderer (skill_fx → the animator's cast clip + name banner).
/// Shared by the instant path and the windup-complete path (`fire_pending_cast`); the
/// caller latches the cooldown at cast START. Deterministic — no PRNG.
/// Set on the caster id to key an ultimate's party buff apart from the caster's own skill buffs
/// of the same kind (see `resolve_buff_cast`). Entity ids never reach it.
pub(crate) const ULT_SRC_BIT: u32 = 0x4000_0000;

pub(crate) fn resolve_buff_cast(state: &mut GameState, i: usize, slot: crate::skills::SkillSlot) {
    // `slot` can outlive the skill it named — see `clear_exemplar_skill`. Fizzle, never index.
    let Some((sid, level)) =
        state.grid.entities[i].skills.get(slot.get()).map(|s| (s.id.clone(), s.level))
    else {
        return;
    };
    let Some(def) = skills::skill_def(&sid) else { return; };
    let source = state.grid.entities[i].id;
    let me_team = state.grid.entities[i].team;
    // An ultimate buff spends its gauge here, where the buff actually lands.
    if def.lb_max > 0 {
        state.grid.entities[i].limit_break = 0;
        crate::unit_rank::after_ult(state, i, &sid);
    }
    // Casting-animation lock (a motion lock, not a cast bar) — the cast_finish window.
    if def.motion_delay_ticks > 0 {
        state.grid.entities[i].act_locked_until = state.tick + def.motion_delay_ticks as u64;
    }
    // Surface the cast (display-only, serde-skip) so the animator plays the cast clip +
    // name banner — a pure buff never enters the swing pipeline.
    state.skill_fx.push(crate::state::SkillFx { caster_id: source, target_id: source, skill_id: sid.clone() });
    communio_after_cast(state, i);
    // S6 (0.0.466, Mirei): a SELF buff this rank names reaches the whole party.
    let party = def.cast_buff_party || crate::unit_rank::buff_reaches_party(&state.grid.entities[i], &sid);
    // TIDEBLESS (0.0.468): roll ONE element the party carries and buff it on top. The draw is
    // taken whenever the skill names one, even with a single element in the party, so the PRNG
    // stream never depends on who is fielded.
    let picks = 1 + crate::unit_rank::element_extra_picks(&state.grid.entities[i]) as usize;
    let rolled: Vec<crate::grid::Element> = if def.element_roll_bp > 0 {
        let mut elems: Vec<crate::grid::Element> = state
            .grid
            .entities
            .iter()
            .filter(|e| e.alive && e.team == me_team && !e.is_pet)
            .map(|e| e.element)
            .collect();
        elems.sort_by_key(|e| skills::element_to_mag(*e));
        elems.dedup();
        let mut out = Vec::new();
        // One draw PER pick, always taken, so the stream never depends on the party's elements.
        for _ in 0..picks {
            let n = elems.len().max(1) as u32;
            let idx = state.rng.gen_range_u32(0, n) as usize;
            if let Some(el) = elems.get(idx).copied() {
                if !out.contains(&el) {
                    out.push(el);
                }
            }
        }
        out
    } else {
        Vec::new()
    };
    if party {
        let count = state.grid.entities.len();
        // S6 (0.0.441, Lucielle): the blessing carries more ATK and mends the party.
        let signature = if def.lb_max > 0 { crate::unit_rank::benedictio_signature(&state.grid.entities[i]) } else { None };
        for j in 0..count {
            if !state.grid.entities[j].alive || state.grid.entities[j].team != me_team {
                continue;
            }
            let kin = def.aspd_kin_element.is_some_and(|el| state.grid.entities[j].element == el);
            for spec in def.cast_buff {
                // An ULTIMATE's party buff keys on its own source when another of the caster's
                // skills casts the same kind (audit 2026-10-02): Sylvie's Bolt from the Blue and
                // her Surge are both party CritBpAdd, and on the caster's id the shorter Surge
                // (fired soon after, thanks to the ult's cooldown cut) replaced the ult's +25%.
                // Still per caster (the id with the top bit set), so two units never share it.
                let shared_kind = def.lb_max > 0 && state.grid.entities[i].skills.iter()
                    .filter(|s| s.id != sid)
                    .filter_map(|s| skills::skill_def(&s.id))
                    .any(|d| d.cast_buff.iter().any(|x| x.kind == spec.kind));
                let src = if shared_kind { source | ULT_SRC_BIT } else { source };
                let mut b = spec.at_level(level, src);
                b.ticks += crate::unit_rank::buff_extra_ticks(&state.grid.entities[i], &sid);
                b.magnitude += crate::unit_rank::buff_power_add(&state.grid.entities[i], &sid);
                // Surge (0.0.474): an ally of the named element gets more of the ASPD.
                if kin && matches!(b.kind, skills::BuffKind::AspdPct) {
                    b.magnitude = ((b.magnitude as i64) * i64::from(def.aspd_kin_mult_bp) / 10_000) as i32;
                }
                skills::apply_buff(&mut state.grid.entities[j], b);
            }
            // Sylvie (0.0.473): the ultimate cuts every ally's skill cooldowns, and her ranks can
            // put crit riders on the blessing for its duration.
            if def.party_cooldown_cut_ticks > 0 {
                for s in state.grid.entities[j].skills.iter_mut() {
                    s.cooldown = s.cooldown.saturating_sub(def.party_cooldown_cut_ticks);
                }
            }
            let (crit_gauge, crit_spark) = crate::unit_rank::crit_riders(&state.grid.entities[i], &sid);
            if crit_gauge > 0 || crit_spark > 0 {
                let ticks = def.cast_buff.first().map_or(120, |s| s.duration)
                    + crate::unit_rank::buff_extra_ticks(&state.grid.entities[i], &sid);
                for (kind, magnitude) in [(skills::BuffKind::CritGauge, crit_gauge), (skills::BuffKind::CritSpark, crit_spark)] {
                    if magnitude > 0 {
                        skills::apply_buff(&mut state.grid.entities[j], skills::Buff { kind, magnitude, ticks, source_id: crate::unit_rank::RANK_SELF_SRC });
                    }
                }
            }
            let gauge_bp = crate::unit_rank::cast_gauge_rate(&state.grid.entities[i], &sid);
            if gauge_bp > 0 {
                let ticks = def.cast_buff.first().map_or(120, |s| s.duration)
                    + crate::unit_rank::buff_extra_ticks(&state.grid.entities[i], &sid);
                // Own source: Fons Vitae's GaugeRatePct rides the caster's source, and
                // `apply_buff` replaces a buff of the same (kind, source).
                skills::apply_buff(&mut state.grid.entities[j], skills::Buff {
                    kind: skills::BuffKind::GaugeRatePct,
                    magnitude: gauge_bp,
                    ticks,
                    source_id: crate::unit_rank::RANK_SELF_SRC,
                });
            }
            for &el in &rolled {
                let bp = def.element_roll_bp + crate::unit_rank::element_roll_extra_bp(&state.grid.entities[i]);
                let ticks = def.cast_buff.first().map_or(120, |s| s.duration)
                    + crate::unit_rank::buff_extra_ticks(&state.grid.entities[i], &sid);
                // One slot PER ELEMENT (0.0.469): `apply_buff` replaces a buff of the same
                // (kind, source), so two rolls sharing a source made the second erase the first.
                // S6 (Astier): the roll counts for every ally — a plain DMG% on the same slot.
                let (kind, magnitude) = if crate::unit_rank::element_roll_all(&state.grid.entities[i]) {
                    (skills::BuffKind::DmgDealtPct, bp)
                } else {
                    (skills::BuffKind::ElementDmgDealtPct, skills::pack_element_bp(el, bp))
                };
                skills::apply_buff(&mut state.grid.entities[j], skills::Buff {
                    kind,
                    magnitude,
                    ticks,
                    source_id: skills::ELEMENT_BUFF_SRC + skills::element_to_mag(el) as u32,
                });
            }
            // FONS VITAE (0.0.468): the ultimate's tide — a HoT of each ally's OWN max HP.
            if def.hot_maxhp_bp > 0 {
                let bp = i64::from(def.hot_maxhp_bp)
                    + i64::from(def.hot_maxhp_per_level_bp) * i64::from(level.saturating_sub(1));
                let maxhp = combat::effective_max_hp(&state.grid.entities[j]) as i64;
                let per_tick = ((maxhp * bp / 10_000) / 10).max(1).min(i32::MAX as i64) as i32;
                let ticks = def.cast_buff.first().map_or(80, |s| s.duration);
                skills::apply_buff(&mut state.grid.entities[j], skills::Buff {
                    kind: skills::BuffKind::HealHot,
                    magnitude: per_tick,
                    ticks,
                    source_id: source,
                });
            }
            if let Some((extra_atk, heal_bp, dmg_bp)) = signature {
                // A 0% stamp is not applied: it replaced a real buff with nothing.
                if extra_atk > 0 {
                    skills::apply_buff(&mut state.grid.entities[j], skills::Buff { kind: skills::BuffKind::AtkPct, magnitude: extra_atk, ticks: 300, source_id: crate::unit_rank::RANK_SIGNATURE_SRC });
                }
                // S6 (0.0.469): her blessing finally carries a DAMAGE layer of its own, which is
                // what an ATK% buff cannot reach on a unit already buried in gear ATK.
                if dmg_bp > 0 {
                    skills::apply_buff(&mut state.grid.entities[j], skills::Buff { kind: skills::BuffKind::DmgDealtPct, magnitude: dmg_bp, ticks: 300, source_id: crate::unit_rank::RANK_SIGNATURE_SRC });
                }
                let heal = ((combat::effective_max_hp(&state.grid.entities[j]) as i64) * i64::from(heal_bp) / 10_000).max(1) as i32;
                super::apply_skill_heal_raw(state, i, j, heal);
            }
        }
    } else {
        // Arc Focus (0.0.473): the buff goes to the allies with the highest ATK, not the caster.
        let targets: Vec<usize> = if def.buff_top_atk_allies > 0 {
            let n = (def.buff_top_atk_allies + crate::unit_rank::buff_extra_targets(&state.grid.entities[i], &sid)) as usize;
            // Damage dealers first (a healer or a tank can out-ATK a DPS on paper): allies whose
            // doctrine is Striker / Destroyer / Catalyst rank above the rest, then highest ATK,
            // then spawn order.
            let mut allies: Vec<(bool, i32, usize)> = state
                .grid
                .entities
                .iter()
                .enumerate()
                .filter(|(j, e)| *j != i && e.alive && e.team == me_team && !e.is_pet)
                .map(|(j, e)| {
                    let dealer = matches!(
                        crate::potential::doctrine_for_unit(&e.unit_def),
                        Some(crate::potential::Doctrine::Striker | crate::potential::Doctrine::Destroyer | crate::potential::Doctrine::Catalyst)
                    );
                    (dealer, combat::effective_atk(e), j)
                })
                .collect();
            allies.sort_by(|a, b| b.0.cmp(&a.0).then(b.1.cmp(&a.1)).then(a.2.cmp(&b.2)));
            allies.into_iter().take(n).map(|(_, _, j)| j).collect()
        } else {
            vec![i]
        };
        for j in targets {
            for spec in def.cast_buff {
                let mut b = spec.at_level(level, source);
                b.ticks += crate::unit_rank::buff_extra_ticks(&state.grid.entities[i], &sid);
                b.magnitude += crate::unit_rank::buff_power_add(&state.grid.entities[i], &sid);
                skills::apply_buff(&mut state.grid.entities[j], b);
            }
        }
    }
}

/// Flame Shield: if the Elementalist has the skill slotted + off cooldown and NO
/// shield up, bank an absorb pool of `INT × level × 2` and snapshot the explosion
/// (100% ATK Fire + a level-tier knockback). Counts as this tick's action.
/// Returns true when cast. The pool soaks damage in `combat::absorb_soak`; it
/// detonates in `tick::resolve_absorb_bursts` when emptied.
pub(crate) fn try_cast_flame_shield(state: &mut GameState, i: usize) -> bool {
    let self_id = state.grid.entities[i].id;
    // Don't re-bank while our own shield is still up — the cooldown only STARTS
    // when the shield breaks (see `resolve_absorb_bursts`), so a live shield blocks
    // re-cast and a broken one waits out the cd.
    if state.grid.entities[i].shields.iter().any(|s| s.src_id == self_id) {
        return false;
    }
    let found = state.grid.entities[i]
        .skills
        .iter()
        .enumerate()
        .find(|(_, s)| s.id == "flame_shield" && s.level > 0 && s.cooldown == 0)
        .map(|(slot, s)| (slot, s.level));
    let Some((slot, level)) = found else { return false; };
    if !skills::loadout_fits(
        &state.grid.entities[i].active_skills,
        combat::loadout_capacity(&state.grid.entities[i]),
        "flame_shield",
    ) {
        return false;
    }
    // The pool is the caster's own DEF, half a copy per skill level: Lv10 = 5x
    // DEF absorbed. It read INT until 0.0.316 — a stat that fed nothing and had
    // stopped growing, so the shield had stopped growing with it.
    let def = combat::effective_def(&state.grid.entities[i]).max(0);
    let pool = def * level as i32 / 2;
    if pool <= 0 {
        return false;
    }
    let burst = combat::effective_atk(&state.grid.entities[i]) * 3 / 2; // 150% ATK snapshot (0.0.437: skills ×3)
    let knock = match level {
        1..=3 => 1,
        4..=6 => 2,
        _ => 3,
    };
    let e = &mut state.grid.entities[i];
    // Newest shield goes to the BACK; damage drains the oldest (front) first.
    // No cooldown is set here — it starts only when the shield BREAKS.
    e.shields.push(crate::grid::AbsorbShield {
        pool,
        source_skill_id: "flame_shield".into(),
        burst_dmg: burst,
        burst_knock: knock,
        src_id: self_id,
        hits_left: 0,    // Flame Shield: pool-only, no hit cap
        persist: false,  // breaks/clears on a transition like other buffs
    });
    e.skills[slot].last_fired = state.tick;
    true
}

/// Praesidium Divinum (Bishop = RO Kyrie Eleison): cast on cooldown onto the
/// LOWEST-HP% ally that does NOT already carry a PD shield (incl. self), a
/// damage-ABSORB shield with a DUAL cap — pool = the CASTER's MaxHP ×
/// `(10 + 2·level)%` (Lv1 12% .. Lv10 30%) AND a hit budget `5 + level/2`
/// (Lv1 5 .. Lv10 10); whichever empties first ends it (see `combat::absorb_soak`).
/// No burst. Targeting the UNSHIELDED weakest spreads protection across the party
/// (one new ally every cd) instead of refreshing one member — the Bishop maintains
/// several PD shields at once, topping each back up as it breaks. No SP; cd-gated
/// (cd set here, unlike Flame Shield). 1 slot.
pub(crate) fn try_cast_praesidium(state: &mut GameState, i: usize) -> bool {
    let self_id = state.grid.entities[i].id;
    let found = state.grid.entities[i]
        .skills
        .iter()
        .enumerate()
        .find(|(_, s)| s.id == "praesidium_divinum" && s.level > 0 && s.cooldown == 0)
        .map(|(slot, s)| (slot, s.level));
    let Some((slot, level)) = found else { return false; };
    if !skills::loadout_fits(
        &state.grid.entities[i].active_skills,
        combat::loadout_capacity(&state.grid.entities[i]),
        "praesidium_divinum",
    ) {
        return false;
    }
    let Some(def) = skills::skill_def("praesidium_divinum") else { return false; };
    let me_team = state.grid.entities[i].team;
    // Only bother while there's a live enemy (don't shield an empty room).
    if !state.grid.entities.iter().any(|e| e.alive && e.team != me_team) {
        return false;
    }
    // Auto-target: the lowest-HP% living ally (self included) that does NOT yet
    // have a PD shield (any hit-capped shield) — so casts spread, never doubling
    // up. All allies already shielded ⇒ no target, hold the cast.
    let target = state
        .grid
        .entities
        .iter()
        .enumerate()
        .filter(|(_, e)| {
            e.alive && e.team == me_team && !e.shields.iter().any(|s| s.hits_left > 0)
        })
        .min_by_key(|(_, e)| {
            let maxhp = combat::effective_max_hp(e).max(1) as i64;
            ((e.hp as i64) * 10_000 / maxhp, e.id)
        })
        .map(|(j, _)| j);
    let Some(tj) = target else { return false; };
    let pool = (combat::effective_max_hp(&state.grid.entities[i]) as i64
        * (10 + 2 * level as i64) / 100) as i32;
    if pool <= 0 {
        return false;
    }
    let hits = 5 + level as i32 / 2;
    state.grid.entities[tj].shields.push(crate::grid::AbsorbShield {
        pool,
        source_skill_id: "praesidium_divinum".into(),
        burst_dmg: 0,
        burst_knock: 0,
        src_id: self_id,
        hits_left: hits,
        persist: true,
    });
    state.grid.entities[i].skills[slot].cooldown = def.cooldown_ticks;
    state.grid.entities[i].skills[slot].last_fired = state.tick;
    if def.motion_delay_ticks > 0 {
        state.grid.entities[i].act_locked_until =
            state.tick + def.motion_delay_ticks as u64;
    }
    true
}

/// Aegis (Seraphine's ultimate): one absorb shield on EVERY living party member
/// at once, sized from HER MaxHP — `15% + 2%/level` each — with no hit cap, and a
/// burst when a shield breaks. Praesidium spreads one shield per cast across the
/// party over time; this is the same engine spending the whole party at once,
/// which is what makes it an ultimate rather than a maintenance cast.
///
/// The burst is what keeps a defensive ultimate legible in a game the player is
/// not watching: a shield that quietly absorbs shows nothing, a shield that ends
/// in an explosion shows a number.
///
/// Holds the cast while no enemy is alive (nothing to shield against) and while a
/// shield of its own is still up on the caster, so it never overwrites itself.
pub(crate) fn try_cast_aegis(state: &mut GameState, i: usize) -> bool {
    let self_id = state.grid.entities[i].id;
    let found = state.grid.entities[i]
        .skills
        .iter()
        .enumerate()
        .find(|(_, s)| s.id == "aegis" && s.level > 0)
        .map(|(slot, s)| (slot, s.level));
    let Some((slot, level)) = found else { return false; };
    if !skills::loadout_fits(
        &state.grid.entities[i].active_skills,
        combat::loadout_capacity(&state.grid.entities[i]),
        "aegis",
    ) {
        return false;
    }
    let Some(def) = skills::skill_def("aegis") else { return false; };
    if state.grid.entities[i].limit_break < def.lb_max {
        return false; // the gauge is the gate, not a clock
    }
    let me_team = state.grid.entities[i].team;
    if !state.grid.entities.iter().any(|e| e.alive && e.team != me_team) {
        return false;
    }
    // Don't re-cast while EVERY living member still carries one of these shields (0.0.460: it
    // used to wait on her OWN shield alone, and a full gauge sat idle while the DPS stood bare).
    if state
        .grid
        .entities
        .iter()
        .filter(|e| e.alive && e.team == me_team && !e.is_pet)
        .all(|e| e.shields.iter().any(|s| s.src_id == self_id))
    {
        return false;
    }
    // 0.0.460: 30% +4%/lv of her max HP per member (was 15% +2%) — the tank blocked a third of
    // what the healer healed back per hour and lost her slot to a fourth DPS at every stage.
    let pool = (combat::effective_max_hp(&state.grid.entities[i]) as i64
        * (30 + 4 * level as i64)
        / 100) as i32;
    if pool <= 0 {
        return false;
    }
    // Burst on break: 40% of her ATK (0.0.428; 200% until 0.0.427 — five shields, five bursts,
    // 28% of the party's damage; at 100% still 22%), thrown off wherever that member stood.
    let mut burst = combat::effective_atk(&state.grid.entities[i]) * 2; // 200% (0.0.437: ults ×5)
    // S6 (0.0.441): the burst doubles and the whole room turns on her.
    if let Some((mult_bp, taunt_ticks)) = crate::unit_rank::aegis_signature(&state.grid.entities[i]) {
        burst = ((burst as i64) * i64::from(mult_bp) / 10_000).min(i32::MAX as i64) as i32;
        for e in state.grid.entities.iter_mut() {
            if e.alive && e.team != me_team && e.on_board() {
                e.taunted_by = Some(self_id);
                e.taunt_ticks = e.taunt_ticks.max(taunt_ticks);
            }
        }
    }
    let targets: Vec<usize> = state
        .grid
        .entities
        .iter()
        .enumerate()
        .filter(|(_, e)| e.alive && e.team == me_team && !e.is_pet)
        .map(|(j, _)| j)
        .collect();
    let shield_atk = crate::unit_rank::shield_cast_atk(&state.grid.entities[i], "aegis");
    for j in targets {
        if let Some((atk_bp, ticks)) = shield_atk {
            skills::apply_buff(&mut state.grid.entities[j], skills::Buff { kind: skills::BuffKind::AtkPct, magnitude: atk_bp, ticks, source_id: crate::unit_rank::RANK_AEGIS_SRC });
        }
        // A member still holding an older Aegis shield gets it REPLACED, not stacked.
        state.grid.entities[j].shields.retain(|s| s.src_id != self_id);
        state.grid.entities[j].shields.push(crate::grid::AbsorbShield {
            pool,
            source_skill_id: "aegis".into(),
            burst_dmg: burst,
            burst_knock: 0,
            src_id: self_id,
            hits_left: 0, // pool-only, like Flame Shield
            persist: false,
        });
    }
    state.grid.entities[i].limit_break = 0; // the gauge IS the cost
    crate::unit_rank::after_ult(state, i, "aegis");
    state.grid.entities[i].skills[slot].last_fired = state.tick;
    if def.motion_delay_ticks > 0 {
        state.grid.entities[i].act_locked_until = state.tick + def.motion_delay_ticks as u64;
    }
    true
}

/// Fire Cat Meteor (Rapis's ultimate): ONE instance on every living enemy in the
/// room at the same moment. Not a rain of separate meteors — a single number on
/// every enemy reads better than a shower landing out of step, and it costs the
/// sim one pass instead of a queue of projectiles.
///
/// Iterates in grid order, which is spawn order, so the damage stream is fixed.
pub(crate) fn try_cast_room_nuke(state: &mut GameState, i: usize) -> bool {
    let found = state.grid.entities[i]
        .skills
        .iter()
        .enumerate()
        .find(|(_, s)| s.level > 0 && skills::skill_def(&s.id).is_some_and(|d| d.room_nuke))
        .map(|(slot, s)| (slot, s.level, s.id.clone()));
    let Some((slot, level, sid)) = found else { return false; };
    if !skills::loadout_fits(
        &state.grid.entities[i].active_skills,
        combat::loadout_capacity(&state.grid.entities[i]),
        &sid,
    ) {
        return false;
    }
    let Some(def) = skills::skill_def(&sid) else { return false; };
    if state.grid.entities[i].limit_break < def.lb_max {
        return false; // the gauge is the gate, not a clock
    }
    let me_team = state.grid.entities[i].team;
    let targets: Vec<usize> = state
        .grid
        .entities
        .iter()
        .enumerate()
        .filter(|(_, e)| e.alive && e.team != me_team && e.on_board())
        .map(|(j, _)| j)
        .collect();
    if targets.is_empty() {
        return false;
    }
    let power = crate::unit_rank::power(&state.grid.entities[i], def, level);
    let nuke_vuln = crate::unit_rank::nuke_vuln(&state.grid.entities[i], &sid);
    let me_id = state.grid.entities[i].id;
    let kind = def.damage_type.unwrap_or(state.grid.entities[i].atk_type);
    for j in targets {
        if !state.grid.entities[j].alive {
            continue;
        }
        let pre_hp = state.grid.entities[j].hp.max(0);
        let dealt = combat::resolve_attack_scaled(
            &mut state.grid.entities, &mut state.rng, i, j, power, 1,
            &mut state.recent_hits, kind, def.toughness_dmg_mult,
        );
        state.record_damage(i, Some(&sid), dealt.min(pre_hp));
        if let (Some((vuln, ticks)), true) = (nuke_vuln, state.grid.entities[j].alive) {
            skills::apply_buff(&mut state.grid.entities[j], skills::Buff { kind: skills::BuffKind::DmgTakenPct, magnitude: vuln, ticks, source_id: me_id });
        }
        if !state.grid.entities[j].alive {
            crate::tick::combat_step::kill_monster(state, j, Some(i));
        }
    }
    state.grid.entities[i].limit_break = 0;
    state.grid.entities[i].skills[slot].last_fired = state.tick;
    crate::unit_rank::after_ult(state, i, &sid);
    if def.motion_delay_ticks > 0 {
        state.grid.entities[i].act_locked_until = state.tick + def.motion_delay_ticks as u64;
    }
    true
}

/// Ember Cascade (Rapis's passive): release ONE queued Fireball, if the pacing
/// cooldown has run out and the queue still holds a living target. Dead targets
/// are dropped as they are reached rather than swept, so a queue full of corpses
/// costs one pass and no shot.
pub(crate) fn try_cast_break_trigger(state: &mut GameState, i: usize) -> bool {
    if state.grid.entities[i].break_queue.is_empty() || state.grid.entities[i].break_cd > 0 {
        return false;
    }
    let Some((def, level)) = crate::tick::break_trigger_of(&state.grid.entities[i].skills)
    else {
        state.grid.entities[i].break_queue.clear();
        return false;
    };
    let me_team = state.grid.entities[i].team;
    // Pop until a living enemy turns up.
    let target = loop {
        let Some(id) = state.grid.entities[i].break_queue.first().copied() else {
            return false;
        };
        state.grid.entities[i].break_queue.remove(0);
        match state.grid.entities.iter().position(|e| {
            e.id == id && e.alive && e.team != me_team && e.on_board()
        }) {
            Some(j) => break j,
            None => continue,
        }
    };
    let power = crate::unit_rank::power(&state.grid.entities[i], def, level);
    let kind = def.damage_type.unwrap_or(state.grid.entities[i].atk_type);
    let pre_hp = state.grid.entities[target].hp.max(0);
    let dealt = combat::resolve_attack_scaled(
        &mut state.grid.entities, &mut state.rng, i, target, power, 1,
        &mut state.recent_hits, kind, def.toughness_dmg_mult,
    );
    state.record_damage(i, Some(def.id), dealt.min(pre_hp));
    if !state.grid.entities[target].alive {
        crate::tick::combat_step::kill_monster(state, target, Some(i));
    }
    state.grid.entities[i].break_cd = crate::grid::BREAK_TRIGGER_CD_TICKS;
    // A free answer, not this turn's action — she keeps casting on her own clock.
    false
}

/// Purificatio (Bishop cleanse): the priority RANK (lower = worse ailment) at which
/// Purificatio level `lvl` can remove buff `kind` — `None` if this level can't clear
/// it (level-gated) OR it's never cleansable. Gates: Lv1 Stun, Lv2 +Freeze, Lv3
/// +Blind, Lv4 +Poison (classic), Lv5 +Slow. NEVER clears `PoisonDot` (Deadly
/// Poison / EDP) or `Bleed` — heavy DoTs are cleanse-immune by design; beneficial
/// buffs are obviously untouched.
fn purificatio_rank(kind: crate::skills::BuffKind, lvl: u8) -> Option<u8> {
    use crate::skills::BuffKind;
    let (needed, rank) = match kind {
        BuffKind::Stun => (1, 0),
        BuffKind::Freeze => (2, 1),
        BuffKind::Poison => (4, 2),
        BuffKind::Blind => (3, 3),
        BuffKind::AgiDexSlowPct => (5, 4),
        _ => return None,
    };
    if lvl >= needed { Some(rank) } else { None }
}

/// Purificatio (Bishop = RO clearance cleanse): on cooldown, auto-target the ally
/// carrying the WORST clearable ailment (highest priority: Stun > Freeze > Poison >
/// Blind > Slow; tie-break lowest HP% then id) and strip ALL of that ally's
/// level-unlocked debuffs at once. Deadly Poison (`PoisonDot`) / Bleed are immune.
/// No SP, 1 reserve slot, cd 5 s, 0.5 s cast motion. Lv5. Mostly forward-looking
/// (today statuses run player→enemy), so it usually finds no target and holds.
pub(crate) fn try_cast_purificatio(state: &mut GameState, i: usize) -> bool {
    let found = state.grid.entities[i]
        .skills
        .iter()
        .enumerate()
        .find(|(_, s)| s.id == "purificatio" && s.level > 0 && s.cooldown == 0)
        .map(|(slot, s)| (slot, s.level));
    let Some((slot, level)) = found else { return false; };
    if !skills::loadout_fits(
        &state.grid.entities[i].active_skills,
        combat::loadout_capacity(&state.grid.entities[i]),
        "purificatio",
    ) {
        return false;
    }
    let Some(def) = skills::skill_def("purificatio") else { return false; };
    let me_team = state.grid.entities[i].team;
    // Target: ally with the worst clearable ailment (rank), then lowest HP%, then id.
    let target = state
        .grid
        .entities
        .iter()
        .enumerate()
        .filter(|(_, e)| e.alive && e.team == me_team)
        .filter_map(|(j, e)| {
            let worst = e.buffs.iter().filter_map(|b| purificatio_rank(b.kind, level)).min();
            let maxhp = combat::effective_max_hp(e).max(1) as i64;
            worst.map(|r| (r, (e.hp as i64) * 10_000 / maxhp, e.id, j))
        })
        .min()
        .map(|(_, _, _, j)| j);
    let Some(tj) = target else { return false; };
    // Strip every debuff this level can clear (Deadly Poison / Bleed survive).
    state.grid.entities[tj].buffs.retain(|b| purificatio_rank(b.kind, level).is_none());
    state.grid.entities[i].skills[slot].cooldown = def.cooldown_ticks;
    state.grid.entities[i].skills[slot].last_fired = state.tick;
    if def.motion_delay_ticks > 0 {
        state.grid.entities[i].act_locked_until = state.tick + def.motion_delay_ticks as u64;
    }
    true
}

/// Place a persistent GROUND FIELD (Poison Cloud) if a slotted field skill is
/// off cooldown AND an enemy is ALREADY within its radius (don't waste a 60s
/// field on empty ground). The field is fixed at the caster's current cell.
/// Returns true when placed (= this entity's action this tick). Deterministic.
/// Monster summon (Broomhilda's Summon Bats): if a ready summon skill (`summon_count > 0`) is
/// slotted AND an enemy is present, spawn its familiars ringing the caster and count it as this
/// turn's action. Instant — a `motion_delay_ticks` lock plays the summon anim. Returns true when cast.
pub(crate) fn try_cast_summon(state: &mut GameState, i: usize) -> bool {
    let me_team = state.grid.entities[i].team;
    let mut chosen: Option<(usize, u32, u32, u32, &'static str)> = None; // slot, cd, motion, count, def_id
    for (slot, s) in state.grid.entities[i].skills.iter().enumerate() {
        let Some(def) = skills::skill_def(&s.id) else { continue; };
        if def.summon_count == 0 || !matches!(def.kind, skills::SkillKind::Active) {
            continue;
        }
        if !hp_gate_open(&state.grid.entities[i], def) || !has_charge(s, def) {
            continue;
        }
        if s.cooldown > 0
            || !skills::loadout_fits(
                &state.grid.entities[i].active_skills,
                combat::loadout_capacity(&state.grid.entities[i]),
                &s.id,
            )
        {
            continue;
        }
        chosen = Some((
            slot,
            def.cooldown_ticks,
            def.motion_delay_ticks,
            def.summon_count,
            def.summon_def_id,
        ));
        break;
    }
    let Some((slot, cooldown, motion, count, def_id)) = chosen else { return false; };
    // Only summon mid-fight — an enemy must exist (else the familiars have nothing to do).
    if !state.grid.entities.iter().any(|e| e.alive && e.team != me_team) {
        return false;
    }
    let cd = combat::stat_mods::scale_cooldown(&state.grid.entities[i], cooldown);
    state.grid.entities[i].skills[slot].cooldown = cd;
    // Spend the use budget (no-op unless the skill is charge-limited).
    if let Some(def) = skills::skill_def(&state.grid.entities[i].skills[slot].id) {
        let sk = &mut state.grid.entities[i].skills[slot];
        spend_charge(sk, def);
    }
    state.grid.entities[i].skills[slot].last_fired = state.tick;
    if motion > 0 {
        state.grid.entities[i].act_locked_until = state.tick + motion as u64;
    }
    // Surface the cast (anim + name banner).
    let src = state.grid.entities[i].id;
    let sid = state.grid.entities[i].skills[slot].id.clone();
    state.skill_fx.push(crate::state::SkillFx { caster_id: src, target_id: src, skill_id: sid });
    crate::spawn::spawn_familiars(state, i, count, def_id);
    true
}

/// USE BUDGET (`max_charges`): a limited-use skill may fire only while it has a charge
/// left. Unlimited skills (the default, `max_charges` 0) always pass. Kept beside the
/// health gate so every "may this skill offer itself right now" rule lives in one place.
pub(crate) fn has_charge(s: &skills::SkillState, def: &skills::SkillDef) -> bool {
    def.max_charges == 0 || s.charges_left > 0
}

/// Spend one charge (no-op for an unlimited skill). Call wherever the cooldown is latched.
pub(crate) fn spend_charge(s: &mut skills::SkillState, def: &skills::SkillDef) {
    if def.max_charges > 0 {
        s.charges_left = s.charges_left.saturating_sub(1);
    }
}

/// HP GATE (`cast_hp_below_pct`): an enrage/desperation skill offers itself only while
/// the caster is at or below that % of max HP. 0 = always available.
///
/// This lives in ONE function because it did not: the gate was implemented inline on the
/// buff-cast path alone, so a ground-field / summon / trap skill carrying it fired from
/// full health. Dulahaness's 5%-HP frenzy mark ran the whole fight that way — the bug
/// read as "the AoE spawns too often and sometimes doubles up", which is exactly what two
/// independent cooldowns firing at once look like.
pub(crate) fn hp_gate_open(e: &crate::grid::Entity, def: &skills::SkillDef) -> bool {
    if def.cast_hp_below_pct == 0 && def.cast_hp_above_pct == 0 {
        return true;
    }
    let maxhp = combat::effective_max_hp(e).max(1) as i64;
    let hp100 = (e.hp as i64) * 100;
    if def.cast_hp_below_pct > 0 && hp100 >= maxhp * def.cast_hp_below_pct as i64 {
        return false;
    }
    if def.cast_hp_above_pct > 0 && hp100 < maxhp * def.cast_hp_above_pct as i64 {
        return false;
    }
    true
}

pub(crate) fn try_cast_ground_field(state: &mut GameState, i: usize) -> bool {
    let me_pos = state.grid.entities[i].pos;
    let me_team = state.grid.entities[i].team;

    let mut chosen: Option<(skills::SkillSlot, i32, u32)> = None; // (slot, range, base_cast)
    for (slot, s) in state.grid.entities[i].skills.iter().enumerate() {
        let Some(def) = skills::skill_def(&s.id) else { continue; };
        if def.ground_field_radius <= 0 || !matches!(def.kind, skills::SkillKind::Active) {
            continue;
        }
        if !hp_gate_open(&state.grid.entities[i], def) || !has_charge(s, def) {
            continue;
        }
        // An ULTIMATE field (Eclipse Rain, 0.0.423) waits on the Limit Break gauge.
        if def.lb_max > 0 && state.grid.entities[i].limit_break < def.lb_max {
            continue;
        }
        if s.cooldown > 0
            || !skills::loadout_fits(
                &state.grid.entities[i].active_skills,
                combat::loadout_capacity(&state.grid.entities[i]),
                &s.id,
            )
        {
            continue;
        }
        chosen = Some((skills::SkillSlot::new(slot), def.range, def.cast_time_ticks));
        break;
    }
    let Some((slot, range, base_cast)) = chosen else { return false; };
    // DENSEST PLACEMENT (0.0.423): land on the cell whose square covers the most enemies.
    if let Some(def) = skills::skill_def(&state.grid.entities[i].skills[slot.get()].id).filter(|d| d.field_place_densest) {
        let Some(center) = densest_cell(state, i, range, def.ground_field_radius) else { return false };
        let windup = combat::effective_cast_time(&state.grid.entities[i], base_cast);
        if windup > 0 {
            let me = state.grid.entities[i].id;
            state.grid.entities[i].casting = Some(crate::grid::Casting {
                ticks_left: windup, ticks_total: windup, slot, target_id: me, ground_cell: Some(center),
            });
            return true;
        }
        place_ground_field(state, i, slot, center);
        return true;
    }
    let random_target = skills::skill_def(&state.grid.entities[i].skills[slot.get()].id)
        .is_some_and(|d| d.cast_target_random);

    // Target the NEAREST enemy within CAST RANGE — the field is centred on it
    // (a thrown spell lands ON the foes, not on the caster). Tie-break by id.
    let mut best: Option<(i32, u32)> = None; // (dist, id)
    for e in &state.grid.entities {
        if !e.alive || e.team == me_team {
            continue;
        }
        let d = me_pos.chebyshev(e.pos);
        if d > range {
            continue;
        }
        if best.map_or(true, |(bd, bid)| d < bd || (d == bd && e.id < bid)) {
            best = Some((d, e.id));
        }
    }
    let Some((_, nearest_id)) = best else { return false; };
    // RANDOM-MARK skills (a boss branding one member) draw among every enemy in
    // range instead. The draw happens HERE — after the skill is known ready and a
    // target is known to exist — so a skill that doesn't set the flag never shifts
    // the PRNG stream.
    let target_id = if random_target {
        let ids: Vec<u32> = state
            .grid
            .entities
            .iter()
            .filter(|e| e.on_board() && e.team != me_team && me_pos.chebyshev(e.pos) <= range)
            .map(|e| e.id)
            .collect();
        if ids.is_empty() {
            return false;
        }
        ids[state.rng.gen_range_u32(0, ids.len() as u32) as usize]
    } else {
        nearest_id
    };

    // A windup → CHANNEL (shows a cast bar; the field is placed by
    // `fire_pending_cast` when it elapses). Instant skills place the field now.
    let windup = combat::effective_cast_time(&state.grid.entities[i], base_cast);
    if windup > 0 {
        // LOCK the aim cell at cast START. The wind-up is a promise about WHERE the
        // field lands; re-reading the target's position at fire time would drag the
        // danger zone along with whoever it is aimed at, making the telegraph — and
        // every dodge answering it — meaningless.
        let aim = state.grid.entities.iter()
            .find(|e| e.id == target_id)
            .map_or(me_pos, |e| e.pos);
        state.grid.entities[i].casting = Some(crate::grid::Casting {
            ticks_left: windup,
            ticks_total: windup,
            slot,
            target_id,
            ground_cell: Some(aim),
        });
        return true;
    }
    // Centre on the target's CURRENT cell.
    let center = state.grid.entities.iter()
        .find(|e| e.id == target_id)
        .map_or(me_pos, |e| e.pos);
    place_ground_field(state, i, slot, center);
    true
}

/// Place the ground field for the skill in `slot` at the caster's cell, latch its
/// cooldown + motion lock. A damaging field (power/debuff) snapshots the caster's
/// MATK·power and a per-field hit interval (`field_duration / field_hits`); else
/// the classic status-only Poison Cloud. Shared by the instant cast and the
/// channel-complete (`fire_pending_cast`) path.
/// The cell within `range` of the caster whose `(2r+1)²` square covers the most living
/// enemies — candidates are the enemies' own cells (the densest square always contains one).
/// Ties: nearer the caster, then the lower (y, x). Deterministic; `None` with no enemy in range.
fn densest_cell(state: &GameState, i: usize, range: i32, radius: i32) -> Option<crate::grid::Cell> {
    let me = &state.grid.entities[i];
    let (me_pos, me_team) = (me.pos, me.team);
    // A BOSS in range takes priority over any crowd (user: "priority boss") — the ultimate is
    // the boss tool first. Lowest id if two stand.
    if let Some(b) = state
        .grid
        .entities
        .iter()
        .filter(|e| e.alive && e.team != me_team && e.is_boss && e.on_board() && me_pos.chebyshev(e.pos) <= range)
        .min_by_key(|e| e.id)
    {
        return Some(b.pos);
    }
    let foes: Vec<crate::grid::Cell> = state
        .grid
        .entities
        .iter()
        .filter(|e| e.alive && e.team != me_team && e.on_board())
        .map(|e| e.pos)
        .collect();
    let mut best: Option<(usize, i32, i32, i32, crate::grid::Cell)> = None;
    for c in foes.iter().filter(|c| me_pos.chebyshev(**c) <= range) {
        let covered = foes.iter().filter(|f| c.chebyshev(**f) <= radius).count();
        let key = (covered, -me_pos.chebyshev(*c), -c.y, -c.x);
        if best.map_or(true, |(bc, bd, by, bx, _)| (covered, -me_pos.chebyshev(*c), -c.y, -c.x) > (bc, bd, by, bx)) {
            best = Some((key.0, key.1, key.2, key.3, *c));
        }
    }
    best.map(|b| b.4)
}

pub(crate) fn place_ground_field(
    state: &mut GameState,
    i: usize,
    slot: crate::skills::SkillSlot,
    center: crate::grid::Cell,
) {
    use crate::grid::Team;
    let me_team = state.grid.entities[i].team;
    // An ultimate field spends its gauge here, where the field actually lands.
    if state.grid.entities[i].skills.get(slot.get()).and_then(|s| skills::skill_def(&s.id)).is_some_and(|d| d.lb_max > 0) {
        state.grid.entities[i].limit_break = 0;
        if let Some(ult) = state.grid.entities[i].skills.get(slot.get()).map(|s| s.id.clone()) {
            crate::unit_rank::after_ult(state, i, &ult);
        }
    }
    let victim_team = if me_team == Team::Player { Team::Monster } else { Team::Player };
    // Same stale-slot hazard as `resolve_buff_cast` — a windup outlives its skill list.
    let Some((sid, level)) =
        state.grid.entities[i].skills.get(slot.get()).map(|s| (s.id.clone(), s.level as u32))
    else {
        return;
    };
    let Some(d) = skills::skill_def(&sid) else { return; };
    // Self-centred field (Sanctus Terra): consecrate the ground the CASTER stands on,
    // ignoring the passed target-centre. Authoritative here so every cast path agrees.
    let center = if d.field_center_self {
        state.grid.entities[i].pos
    } else if state.lane.is_some() && d.field_pull > 0 {
        crate::lane::abyss_point(state)
    } else {
        center
    };
    let (kind, ticks_left) = if d.power_bp > 0 || d.on_hit_debuff.is_some() {
        // METEOR mode (Meteor Rain): the field schedules `meteor_count` drops
        // (2 + lvl/2 → Lv1 2 .. Lv10 7), each meteor a `strike_hits`-hit blast
        // (Lv1 1 .. Lv10 5) at a random cell. Normal fields: fixed/scaling hits,
        // one packet each.
        let meteor = d.impact_radius > 0;
        let once = d.once_per_target;
        let duration = if d.field_duration_ticks > 0 { d.field_duration_ticks } else { 40 };
        // `hits` = how many TIMES the field fires over its life; `strike_hits` = the
        // VISUAL numbers per instance. METEOR: N drops of (lvl+1)/2 hits each.
        // ONCE-PER-TARGET (Vermilion): scans EVERY tick for new entrants, one
        // `field_hits`-hit instance per unit. Normal field: field_hits ticks ×1 hit.
        let (hits, strike_hits, hit_interval) = if meteor {
            // Meteor COUNT / hits-per-meteor: an EXPLICIT `field_hits` /
            // `field_strike_hits` wins (a boss barrage is authored flat — a monster
            // always sits at skill level 1, so the level curve can't shape it);
            // otherwise the player curve (Meteor Rain: 2 + lvl/2 drops of
            // (lvl+1)/2 hits). Meteor Rain leaves both 0 ⇒ unchanged.
            let cnt = if d.field_hits > 0 { d.field_hits } else { 2 + level / 2 }.max(1);
            let per = if d.field_strike_hits > 0 { d.field_strike_hits } else { (level + 1) / 2 };
            (cnt, per.max(1), (duration / cnt).max(1))
        } else if once {
            (1, d.field_hits.max(1), 1)
        } else {
            // Normal field: `field_hits` waves over the duration; each wave shows
            // `field_strike_hits` VISUAL numbers (Sancta = 10; default 1).
            let cnt = (d.field_hits + d.field_hits_per_level * level.saturating_sub(1)).max(1);
            (cnt, d.field_strike_hits.max(1), (duration / cnt).max(1))
        };
        let _ = hits; // (only meteor/normal use a schedule; kept for clarity)
        // Base stat: MATK for every normal (magic) field; ATK for a physical-scaling holy
        // field (Sanctus Terra) that still delivers as magic. `power_at` unchanged.
        let base_stat = if d.field_power_from_atk {
            combat::effective_atk(&state.grid.entities[i])
        } else {
            combat::effective_atk(&state.grid.entities[i])
        };
        let per_hit = (base_stat as i64 * crate::unit_rank::power(&state.grid.entities[i], d, level as u8) as i64 / 10_000) as i32;
        // A VISUAL multi-hit (meteor / Vermilion) is ONE damage instance (DEF once)
        // shown as `strike_hits` numbers — total = per_hit × strike_hits in ONE packet.
        // A normal repeating field has strike_hits == 1 ⇒ one packet of per_hit.
        let amount = per_hit.saturating_mul(strike_hits as i32);
        let element = d.cast_element.unwrap_or(state.grid.entities[i].element);
        let (status, status_bp, status_dur, status_mag) = match d.on_hit_debuff {
            Some(spec) => (
                Some(spec.kind),
                (spec.chance_bp as i32 + spec.chance_per_level * level as i32).max(0) as u32,
                spec.duration,
                spec.magnitude + spec.per_level * level.saturating_sub(1) as i32,
            ),
            None => (None, 0, 0, 0),
        };
        (
            crate::state::GroundFieldKind::Damage {
                amount, element, status, status_bp, status_dur, status_mag, hit_interval,
                status_at: d.status_at, knockback: d.knockback,
                impact_radius: d.impact_radius, strike_hits, once_per_target: once,
                fuse: d.field_fuse,
                pull: d.field_pull,
            },
            duration,
        )
    } else {
        (crate::state::GroundFieldKind::PoisonCloud { poison_ticks: 30 }, 50 + 50 * level)
    };
    // Surface the cast for the renderer (display-only, serde-skip — no hash impact):
    // field skills never enter the swing pipeline, so this is the animator's only
    // signal to play the skill's anim-slot clip at resolve.
    state.skill_fx.push(crate::state::SkillFx {
        caster_id: state.grid.entities[i].id,
        target_id: state.grid.entities[i].id,
        skill_id: sid.clone(),
    });
    state.ground_fields.push(crate::state::GroundField {
        center,
        radius: d.ground_field_radius,
        victim_team,
        ticks_left,
        kind,
        source_id: state.grid.entities[i].id,
        follow_id: if d.field_follows_caster { state.grid.entities[i].id } else { 0 },
        skill_id: sid,
        hit_ids: Vec::new(),
    });
    state.grid.entities[i].skills[slot.get()].cooldown = d.cooldown_ticks;
    spend_charge(&mut state.grid.entities[i].skills[slot.get()], d);
    state.grid.entities[i].skills[slot.get()].last_fired = state.tick;
    if d.motion_delay_ticks > 0 {
        state.grid.entities[i].act_locked_until = state.tick + d.motion_delay_ticks as u64;
    }
}
