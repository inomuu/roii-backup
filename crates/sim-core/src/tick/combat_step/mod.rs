//! Attack resolution — active-skill selection, the swing/cast loop,
//! monster death + XP/job-level progression.

// DEBT (319 sites): this file predates `deny(clippy::indexing_slicing)`. An index that
// cannot be proven in range is a panic, and in verify-server (which links this crate and
// replays saves under `panic = "abort"`) a panic is the whole process. Shrink this, never
// add to it — a NEW file has no allow and is rejected on its first bare index.
#![allow(clippy::indexing_slicing)]

pub(crate) mod cast;
mod trap;
pub(crate) mod xp;
#[cfg(test)]
mod tests;

pub(crate) use cast::*;
pub(crate) use trap::*;
pub(crate) use xp::*;
pub use xp::{apex_xp_cost, xp_to_next, LEVEL_CAP, APEX_LEVEL_CAP};

use super::*;
use crate::{combat, skills};
use crate::grid::{Cell, DamageType, Element};
use crate::state::GameState;

/// One resolved active-skill cast (which slot fired + its numbers + the
/// chosen target, which may differ from the attacker's nearest enemy when
/// the skill picks the lowest-HP target).
pub(crate) struct ActiveCast {
    slot: crate::skills::SkillSlot,
    power_bp: u32,
    cooldown: u32,
    aoe_radius: i32,
    /// Caster-centred blast radius (Magnum Break). > 0 ⇒ point-blank AoE
    /// replaces the single-target resolve.
    self_aoe_radius: i32,
    self_aoe_max_targets: u32,
    knockback: i32,
    pull: i32,
    toughness_dmg_mult: u32,
    /// Self-buffs applied on cast (Magnum Break Fire element, Energy Coat DEF).
    /// Party buffs never reach here — they're cast via `try_cast_buffs`.
    cast_buff: &'static [skills::BuffSpec],
    on_hit_debuff: Option<skills::DebuffSpec>,
    on_hit_debuff_2: Option<skills::DebuffSpec>,
    /// Damage-packet element override for this cast (mage bolts).
    cast_element: Option<Element>,
    /// Damage-TYPE override (axis separation, 0.0.189): None = caster's atk_type.
    damage_type: Option<crate::grid::DamageType>,
    delivery: crate::skills::Delivery,
    teleport: bool,
    target_idx: usize,
    /// Base cast-time ticks of this skill (RO VCT). > 0 → the swing begins a
    /// channel instead of resolving immediately (mage bolts / Fireball).
    base_cast_time: u32,
    /// Number of HITS this cast delivers (RO bolts = skill level; 1 otherwise).
    /// Each hit is a separate damage application (MDEF/element per hit).
    hits: u32,
    /// Motion delay: ticks to lock the caster (no action) after this resolves.
    motion_delay: u32,
    /// How many numbers the renderer shows for this cast (Sonic Blow = 8).
    display_hits: u32,
    /// Bonus damage = caster HARD DEF · this/10000, added per hit (Shield Blast).
    def_scale_bp: u32,
    /// On hit, taunt the target onto the caster for this many ticks (Shield Boomerang).
    taunt_on_hit_ticks: u32,
    /// Pickpocket (0.0.464): bp of the struck enemy's `gil_reward` paid to the party on the hit.
    pickpocket_gil_bp: u32,
    /// An ULTIMATE riding the strike path (0.0.463, a self-AoE ult like Charon's Passage):
    /// picked only on a full gauge, spends it on resolve instead of charging it.
    ult: bool,
}

/// The level-scalable "bolt" skills: fire ONE bolt per (cast) level, and their
/// CAST TIME scales with the cast level too (RO variable cast level). Single
/// source for both the hit-count and the cast-time helpers below.
fn is_bolt_skill(skill_id: &str) -> bool {
    matches!(skill_id, "fire_bolt" | "lightning_bolt" | "frost_bolt" | "earth_piercer")
}

/// RO bolt skills fire ONE bolt per (cast) level (each ~MATK); everything else is
/// a single hit. Keeps the multi-hit list here instead of widening every SkillDef.
fn bolt_hit_count(skill_id: &str, cast_level: u8) -> u32 {
    if skill_id == "spirit_strike" {
        // Spirit Strike: one bolt per 2 levels (⌈lv/2⌉) — matches the Banshee
        // card's proc resolve so tree-cast and proc-cast stay consistent.
        return ((cast_level as u32) + 1) / 2;
    }
    if is_bolt_skill(skill_id) {
        (cast_level as u32).max(1)
    } else {
        1
    }
}

/// Cast time (ticks) for a cast at `cast_level`: a bolt casts at 5 ticks/level
/// (0.5 s/level — the variable-cast-level trade: higher level = more bolts but a
/// longer channel); every other skill keeps its authored fixed cast time.
fn bolt_cast_time_ticks(skill_id: &str, cast_level: u8, default_ct: u32) -> u32 {
    if is_bolt_skill(skill_id) {
        (5 * cast_level as u32).max(1)
    } else {
        default_ct
    }
}

/// Lowest-HP alive enemy of `i` within `range` (Chebyshev). Tie-break by
/// lowest id so it's deterministic. `None` if no enemy is in range.
/// The NEWEST non-boss enemy within `range` (highest spawn id = most recently
/// spawned; monster ids increment from `next_monster_id`). Drives Dark Ambush's
/// "blink onto a fresh add". Bosses are excluded. `None` if none in range.
pub(crate) fn find_newest_nonboss_enemy_in_range(state: &GameState, i: usize, range: i32) -> Option<usize> {
    let me = &state.grid.entities[i];
    let mut best: Option<(u32, usize)> = None; // (id, idx)
    for (j, e) in state.grid.entities.iter().enumerate() {
        if j == i || !e.alive || e.team == me.team || e.is_boss {
            continue;
        }
        if me.pos.chebyshev(e.pos) > range {
            continue;
        }
        if best.map_or(true, |(bid, _)| e.id > bid) {
            best = Some((e.id, j));
        }
    }
    best.map(|(_, idx)| idx)
}

/// The FURTHEST alive enemy within `range` (Chebyshev), tie-broken by lowest id.
/// Drives Shield Boomerang's long throw. `None` if no enemy is in range.
pub(crate) fn find_furthest_enemy_in_range(state: &GameState, i: usize, range: i32) -> Option<usize> {
    let me = &state.grid.entities[i];
    let mut best: Option<(i32, u32, usize)> = None; // (dist, id, idx)
    for (j, e) in state.grid.entities.iter().enumerate() {
        if j == i || !e.alive || e.team == me.team {
            continue;
        }
        let dist = me.pos.chebyshev(e.pos);
        if dist > range {
            continue;
        }
        // Furthest wins; on a distance tie keep the lowest id (deterministic).
        if best.map_or(true, |(bd, bid, _)| dist > bd || (dist == bd && e.id < bid)) {
            best = Some((dist, e.id, j));
        }
    }
    best.map(|(_, _, idx)| idx)
}

/// An enemy in range still carrying a Static Spark from `i` (0.0.465, Mirei's passive).
pub(crate) fn find_sparked_enemy_in_range(state: &GameState, i: usize, range: i32) -> Option<usize> {
    let me = &state.grid.entities[i];
    state
        .grid
        .entities
        .iter()
        .enumerate()
        .filter(|(j, e)| *j != i && e.alive && e.team != me.team && me.pos.chebyshev(e.pos) <= range)
        .find(|(_, e)| e.buffs.iter().any(|b| matches!(b.kind, skills::BuffKind::StaticSpark) && b.source_id == me.id))
        .map(|(j, _)| j)
}

/// Lay `spark_stacks` Static Sparks from `i` onto `tgt`, each worth `spark_atk_bp` of the
/// layer's ATK at this moment (0.0.465). Replaces what was there from the same layer.
pub(crate) fn lay_static_spark(state: &mut GameState, i: usize, tgt: usize, def: &skills::SkillDef) {
    if def.spark_stacks == 0 || def.spark_atk_bp == 0 {
        return;
    }
    let stacks = def.spark_stacks + crate::unit_rank::spark_extra_stacks(&state.grid.entities[i]);
    let bp = i64::from(def.spark_atk_bp) + i64::from(crate::unit_rank::spark_extra_bp(&state.grid.entities[i]));
    let per = ((combat::effective_atk(&state.grid.entities[i]) as i64) * bp / 10_000).max(1) as i32;
    let src = state.grid.entities[i].id;
    let ticks = 100; // 10 s
    let t = &mut state.grid.entities[tgt];
    t.buffs.retain(|b| !(matches!(b.kind, skills::BuffKind::StaticSpark) && b.source_id == src));
    for _ in 0..stacks {
        t.buffs.push(skills::Buff { kind: skills::BuffKind::StaticSpark, magnitude: per, ticks, source_id: src });
    }
}

/// Pickpocket (0.0.464): pay the party `bp` of `tgt`'s `gil_reward`. 0 bp = nothing.
pub(crate) fn pickpocket(state: &mut GameState, tgt: usize, bp: u32) {
    if bp == 0 {
        return;
    }
    let take = u64::from(state.grid.entities[tgt].gil_reward).saturating_mul(u64::from(bp)) / 10_000;
    if take > 0 {
        state.earn_gil(take, crate::state::GilEarn::Pickpocket);
    }
}

/// A BOSS in range first, else the highest-HP enemy in range (0.0.463, Blackwater Mark).
pub(crate) fn find_boss_or_highest_hp_enemy_in_range(state: &GameState, i: usize, range: i32) -> Option<usize> {
    let me = &state.grid.entities[i];
    let mut best: Option<(bool, i32, u32, usize)> = None; // (is_boss, hp, id, idx)
    for (j, e) in state.grid.entities.iter().enumerate() {
        if j == i || !e.alive || e.team == me.team || me.pos.chebyshev(e.pos) > range {
            continue;
        }
        let key = (e.is_boss, e.hp, e.id, j);
        match best {
            Some((bb, bh, bid, _)) if (bb, bh, std::cmp::Reverse(bid)) >= (key.0, key.1, std::cmp::Reverse(key.2)) => {}
            _ => best = Some(key),
        }
    }
    best.map(|(_, _, _, j)| j)
}

pub(crate) fn find_lowest_hp_enemy_in_range(state: &GameState, i: usize, range: i32) -> Option<usize> {
    let me = &state.grid.entities[i];
    let mut best: Option<(i32, u32, usize)> = None; // (hp, id, idx)
    for (j, e) in state.grid.entities.iter().enumerate() {
        if j == i || !e.alive || e.team == me.team {
            continue;
        }
        if me.pos.chebyshev(e.pos) > range {
            continue;
        }
        let key = (e.hp, e.id, j);
        match best {
            Some((bh, bid, _)) if (bh, bid) <= (key.0, key.1) => {}
            _ => best = Some(key),
        }
    }
    best.map(|(_, _, idx)| idx)
}

/// Nearest enemy in range that does NOT already carry `buff` — for a SPREADING
/// debuff (Aspd Dampening) so a re-cast lands on a fresh target instead of
/// re-applying to one already affected. Nearest wins, then lowest id (deterministic).
/// `None` → every in-range enemy already carries it.
pub(crate) fn find_nearest_enemy_without_buff(
    state: &GameState,
    i: usize,
    range: i32,
    buff: skills::BuffKind,
) -> Option<usize> {
    let me = &state.grid.entities[i];
    let mut best: Option<(i32, u32, usize)> = None; // (dist, id, idx)
    for (j, e) in state.grid.entities.iter().enumerate() {
        if j == i || !e.alive || e.team == me.team {
            continue;
        }
        let dist = me.pos.chebyshev(e.pos);
        if dist > range || skills::buff_sum(e, buff) > 0 {
            continue;
        }
        let key = (dist, e.id, j);
        match best {
            Some((bd, bid, _)) if (bd, bid) <= (key.0, key.1) => {}
            _ => best = Some(key),
        }
    }
    best.map(|(_, _, idx)| idx)
}

/// PREVIEW (display-only, PURE): the skill id the entity's NEXT swing WOULD cast if
/// it resolved right now, or `None` for a basic attack. The renderer uses this during
/// the WIND-UP to show the correct attack animation — the real `skill_fx` only arrives
/// on the resolve tick, too late to animate the swing that cast it. No mutation, no
/// PRNG draw, so it's safe to call every tick from the bridge (goldens untouched).
///
/// Since 0.0.481 the casts outside `attack_step` (heals, buffs, ultimates, …) also wait for the
/// swing and take its place, so they are previewed too: while a party member is mid-swing, the
/// cast chain is run on a CLONE and the skill it would fire is named. It takes priority over the
/// swing's own skill, as it does in `tick_one_entity`.
/// `preview_pending_skill` for every fielded member at once, with ONE probe clone of the state
/// (audit 2026-10-02: a clone per mid-swing member per view — up to five ~1 MB copies at 10 Hz —
/// worked against the companion-mode CPU rule). Between members only the GRID and the PRNG are
/// restored: they are all a cast reads to pick its skill. Same answers as the single form.
pub fn preview_pending_skills(state: &GameState) -> std::collections::BTreeMap<u32, String> {
    let mut out = std::collections::BTreeMap::new();
    let mut probe: Option<GameState> = None;
    for i in 0..state.grid.entities.len() {
        let e = &state.grid.entities[i];
        if !(e.alive && matches!(e.team, crate::grid::Team::Player) && crate::tick::swing_engaged(state, i)) {
            if let Some(id) = preview_off_swing(state, i) {
                out.insert(e.id, id);
            }
            continue;
        }
        if !e.windup_cast {
            continue;
        }
        let p = match probe.as_mut() {
            Some(p) => {
                p.grid = state.grid.clone();
                p.rng = state.rng.clone();
                p
            }
            None => probe.insert(state.clone()),
        };
        let before: Vec<(u64, u32)> = e.skills.iter().map(|s| (s.last_fired, s.cooldown)).collect();
        if crate::tick::out_of_cycle_cast(p, i) {
            if let Some(sk) = p.grid.entities[i].skills.iter().zip(before)
                .find(|(s, was)| (s.last_fired, s.cooldown) != *was)
                .map(|(s, _)| s)
            {
                out.insert(e.id, sk.id.clone());
                continue;
            }
        }
        if let Some(id) = preview_off_swing(state, i) {
            out.insert(e.id, id);
        }
    }
    out
}

/// The tail of `preview_pending_skill`: the skill `pick_active_skill` would choose against the
/// nearest enemy (no state change, no clone).
fn preview_off_swing(state: &GameState, i: usize) -> Option<String> {
    let enemy_idx = combat::find_enemy_idx(&state.grid.entities, i)?;
    let cast = pick_active_skill(state, i, enemy_idx)?;
    Some(state.grid.entities[i].skills[cast.slot.get()].id.clone())
}

pub fn preview_pending_skill(state: &GameState, entity_id: u32) -> Option<String> {
    let i = state.grid.idx_of_id(entity_id)?;
    let e = &state.grid.entities[i];
    if e.alive && matches!(e.team, crate::grid::Team::Player) && crate::tick::swing_engaged(state, i) {
        if !e.windup_cast {
            return None; // this wind-up opened with nothing ready: it stays a basic
        }
        let before: Vec<(u64, u32)> = e.skills.iter().map(|s| (s.last_fired, s.cooldown)).collect();
        let mut probe = state.clone();
        if crate::tick::out_of_cycle_cast(&mut probe, i) {
            // The skill that fired is the one whose stamp or cooldown moved.
            if let Some(sk) = probe.grid.entities[i].skills.iter().zip(before)
                .find(|(s, was)| (s.last_fired, s.cooldown) != *was)
                .map(|(s, _)| s)
            {
                return Some(sk.id.clone());
            }
        }
    }
    let enemy_idx = combat::find_enemy_idx(&state.grid.entities, i)?;
    let cast = pick_active_skill(state, i, enemy_idx)?;
    Some(state.grid.entities[i].skills[cast.slot.get()].id.clone())
}

/// Pick the highest-priority usable ACTIVE skill (priority = skill-list
/// order) for `i`: off cooldown, enough SP, and a valid target in the
/// skill's range. Returns the chosen target too — normally the attacker's
/// nearest enemy (`enemy_idx`), but the LOWEST-HP enemy in range for a
/// `target_lowest_hp` skill (e.g. Shadow Strike). `None` → basic attack.
/// Deterministic (GDD §4.2 — published rule, no randomness).
/// Resolve the target index for `slot_id`'s skill using ITS targeting rule
/// (furthest / newest-add / lowest-HP / spread-onto-fresh / default nearest-in-range).
/// `enemy_idx` seeds the default (nearest) case. `None` = no valid target for this
/// skill right now. Single source of truth shared by `pick_active_skill` (skill
/// selection) and `retarget_current_cast` (a mid-cast whose target vanished).
fn resolve_skill_target(
    state: &GameState,
    i: usize,
    def: &skills::SkillDef,
    slot_id: &str,
    enemy_idx: usize,
) -> Option<usize> {
    if def.target_furthest {
        // Shield Boomerang: hurl the shield at the FURTHEST foe in reach.
        find_furthest_enemy_in_range(state, i, def.range)
    } else if def.target_newest_nonboss {
        // Dark Ambush: blink onto the freshest non-boss add in reach.
        find_newest_nonboss_enemy_in_range(state, i, def.range)
    } else if def.target_lowest_hp {
        find_lowest_hp_enemy_in_range(state, i, def.range)
    } else if def.target_boss_first {
        find_boss_or_highest_hp_enemy_in_range(state, i, def.range)
    } else if def.spark_keeper {
        // Mirei: whoever still carries her spark, else the nearest in range.
        find_sparked_enemy_in_range(state, i, def.range).or_else(|| {
            (state.grid.entities[i].pos.chebyshev(state.grid.entities[enemy_idx].pos) <= def.range).then_some(enemy_idx)
        })
    } else if slot_id == "aspd_dampening" || def.aoe_aspd_slow_bp > 0 {
        // Spread the slow: nearest enemy NOT already slowed (fresh, not a re-slow).
        // Both the acolyte's Aspd Dampening and any `aoe_aspd_slow_bp` mob skill (Banshee Wail).
        find_nearest_enemy_without_buff(state, i, def.range, skills::BuffKind::AspdSlowPct)
    } else if slot_id == "lumen_contagio" {
        // Seed the plague on a FRESH target — nearest enemy not yet marked.
        find_nearest_enemy_without_buff(state, i, def.range, skills::BuffKind::LumenVuln)
    } else {
        // Default: the attacker's nearest enemy, must be in range.
        if state.grid.entities[i].pos.chebyshev(state.grid.entities[enemy_idx].pos) > def.range {
            return None;
        }
        Some(enemy_idx)
    }
}

/// A cast whose locked target vanished mid-windup: re-resolve a target for the
/// SAME skill (its own targeting rule) so the windup isn't wasted — "retarget
/// before it whiffs". Returns the new target's entity id, or `None` when no valid
/// target exists anywhere (caller then abandons the channel). Deterministic: reuses
/// the same targeting helpers, no PRNG.
pub(crate) fn retarget_current_cast(state: &GameState, i: usize, slot: crate::skills::SkillSlot) -> Option<u32> {
    let sid = state.grid.entities[i].skills.get(slot.get())?.id.clone();
    let def = skills::skill_def(&sid)?;
    // Seed the default (nearest) targeting case with the attacker's nearest enemy.
    let enemy_idx = combat::find_enemy_idx(&state.grid.entities, i)?;
    let tgt = resolve_skill_target(state, i, def, &sid, enemy_idx)?;
    Some(state.grid.entities[tgt].id)
}

pub(crate) fn pick_active_skill(state: &GameState, i: usize, enemy_idx: usize) -> Option<ActiveCast> {
    let attacker = &state.grid.entities[i];
    // Selection priority among READY damage actives:
    //   1. LONGEST cooldown first — a signature skill (long CD) preempts the
    //      cheap, spammable ones the instant it's off cooldown, so it always
    //      lands on cadence instead of being skipped.
    //   2. then highest element score (the mage's 3 same-CD bolts pick the best
    //      matchup),
    //   3. then list order (first-seen wins ties).
    //   4. then the skill that fired LONGEST ago (fair "ready-time queue"
    //      rotation among otherwise-equal skills).
    // `best_key` = (cooldown_ticks, element_score, Reverse(last_fired)); a
    // candidate wins on a strict `>` so the first-seen keeps exact ties.
    let mut best: Option<((u32, u32, std::cmp::Reverse<u64>), ActiveCast)> = None;
    for (slot, s) in attacker.skills.iter().enumerate() {
        let Some(def) = skills::skill_def(&s.id) else { continue; };
        if !matches!(def.kind, skills::SkillKind::Active) {
            continue;
        }
        // ULTIMATES are not part of the rotation. They have no cooldown — their
        // gate is the Limit Break gauge — so without this the generic picker would
        // see one as "always ready" and spend every turn on it. The one exception
        // (0.0.463): a SELF-AoE ultimate rides this path when its gauge is FULL, and
        // wins outright — nothing else has a path for it.
        let ult = def.lb_max > 0;
        if ult && !(def.self_aoe_radius > 0 && attacker.limit_break >= def.lb_max) {
            continue;
        }
        // VOLLEYS have their own cast path (`try_cast_volley`); as a single strike here they
        // would fire one arrow at whoever is nearest.
        if def.volley_targets > 0 || def.zone_wide {
            continue;
        }
        // Loadout gate: only SLOTTED actives that FIT the SP-reserve slot budget
        // (10 + INT/10) fire — drop a skill from the bar (or run out of slots) and
        // it goes inert. `loadout_fits` subsumes the slotted check.
        // The rune surcharge on a skill's loadout cost retired with the rune system (0.0.369),
        // so this is the plain shared rule again.
        if !crate::skills::loadout_fits(
            &attacker.active_skills, combat::loadout_capacity(attacker), &s.id,
        ) {
            continue;
        }
        // Support heals (Heal) are NOT attacks — they're cast on hurt allies
        // via `try_cast_support`. Their `power_bp` scales the heal amount, so
        // without this guard the caster would "attack" enemies with Heal,
        // dealing MATK-scaled magic damage and burning SP.
        if def.heals_ally {
            continue;
        }
        // Ground-field skills (Poison Cloud) place a field via
        // `try_cast_ground_field`, not the attack path — skip them here.
        if def.ground_field_radius > 0 {
            continue;
        }
        // No-damage skills never use the attack path: pure buff casts (Blessing /
        // Increase AGI / Energy Coat — cast via `try_cast_buffs`) and marker
        // stances (Autocounter). A damage skill that ALSO self-buffs (Magnum Break,
        // power_bp > 0) stays here. EXCEPTION: a no-damage AoE dampener (Banshee's Wail,
        // aoe_aspd_slow_bp > 0) rides the attack path only to PLACE its slow — it deals no
        // damage (resolve_instant skips the strike for power 0); the value is the debuff.
        if def.power_bp == 0 && def.aoe_aspd_slow_bp == 0 {
            continue;
        }
        if s.cooldown > 0 || !cast::has_charge(s, def) {
            continue;
        }
        // Shield-as-weapon skills (Shield Blast / Boomerang) need a shield equipped.
        if def.requires_shield && !combat::has_shield(attacker) {
            continue;
        }
        // Resolve this skill's target by ITS targeting rule (shared with mid-cast
        // retargeting so both use one source of truth). None → no valid target.
        let target_idx = match resolve_skill_target(state, i, def, &s.id, enemy_idx) {
            Some(t) => t,
            None => continue,
        };
        // A DoT strike (Open Wound bleed / Deadly Poison) won't re-apply to a
        // target that already carries THAT DoT — skip it so the attacker uses a
        // basic / other skill while it ticks, re-applying once it lapses.
        if let Some(d) = def.on_hit_debuff {
            if matches!(d.kind, skills::BuffKind::PoisonDot | skills::BuffKind::Bleed | skills::BuffKind::LeechBleed) {
                // Instances of THIS unit's DoT already on the target; a rank may allow more
                // than the one the rule keeps (Nyx S2: Rend stacks twice).
                let me = attacker.id;
                let mine = state.grid.entities[target_idx].buffs.iter().filter(|b| b.kind == d.kind && b.source_id == me).count();
                let allowed = crate::unit_rank::dot_stacks(attacker, &s.id);
                if (allowed <= 1 && skills::buff_sum(&state.grid.entities[target_idx], d.kind) > 0) || (allowed > 1 && mine >= allowed) {
                    continue;
                }
            }
        }
        // Element-aware auto-cast (mage's 3 bolts): score each ready skill by
        // its element matchup vs ITS target, and pick the highest. `cast_element`
        // (Fire/Earth/Water) overrides the attacker's own element for scoring +
        // resolution. Skills without an override score on the attacker element,
        // so a class with a single damaging active is unaffected — and every
        // current golden has ≤ 1 ready active ⇒ byte-identical.
        let cast_el = def.cast_element.unwrap_or(attacker.element);
        let score = combat::element_mult_vs(&state.grid.entities[target_idx], cast_el);
        // (longest cooldown, best element score, longest-ago fired). Reverse on
        // last_fired so a SMALLER value (fired longer ago) sorts higher → equal
        // skills rotate. Strict `>` keeps the first-seen on an exact tie.
        let key = if ult { (u32::MAX, score, std::cmp::Reverse(s.last_fired)) } else { (def.cooldown_ticks, score, std::cmp::Reverse(s.last_fired)) };
        if best.as_ref().map_or(true, |(bkey, _)| key > *bkey) {
            best = Some((
                key,
                ActiveCast {
                    slot: crate::skills::SkillSlot::new(slot),
                    power_bp: crate::unit_rank::power(attacker, def, s.level),
                    cooldown: def.cooldown_ticks,
                    aoe_radius: def.aoe_radius,
                    self_aoe_radius: def.self_aoe_radius.max(crate::unit_rank::skill_aoe(attacker, &s.id)),
                    self_aoe_max_targets: def.self_aoe_max_targets,
                    knockback: def.knockback,
                    pull: def.pull,
                    toughness_dmg_mult: def.toughness_dmg_mult,
                    cast_buff: def.cast_buff,
                    on_hit_debuff: def.on_hit_debuff,
                    on_hit_debuff_2: def.on_hit_debuff_2,
                    cast_element: def.cast_element,
                    damage_type: def.damage_type,
                    delivery: def.delivery,
                    teleport: def.teleport,
                    target_idx,
                    base_cast_time: bolt_cast_time_ticks(&s.id, s.effective_cast_level(), def.cast_time_ticks),
                    hits: bolt_hit_count(&s.id, s.effective_cast_level()),
                    motion_delay: def.motion_delay_ticks,
                    display_hits: def.display_hits,
                    def_scale_bp: def.def_scale_bp,
                    taunt_on_hit_ticks: def.taunt_on_hit_ticks,
                    pickpocket_gil_bp: def.pickpocket_gil_bp,
                    ult,
                },
            ));
        }
    }
    best.map(|(_, c)| c)
}

/// Apply a cast's self-buffs (Magnum Break Fire element, Energy Coat DEF) to the
/// caster. No-op for skills without `cast_buff`. Party buffs never reach here.
fn apply_self_cast_buff(state: &mut GameState, i: usize, cast: &ActiveCast) {
    if cast.cast_buff.is_empty() {
        return;
    }
    let level = state.grid.entities[i].skills[cast.slot.get()].level;
    let source = state.grid.entities[i].id;
    for spec in cast.cast_buff {
        skills::apply_buff(&mut state.grid.entities[i], spec.at_level(level, source));
    }
}

/// Aspd Dampening (acolyte): slow the attack speed of every enemy in the 3×3
/// around `target` by the skill's per-level %. A DIRECT `AspdSlowPct` cut (so it
/// bites monsters despite their flat ASPD, and enemy players in PvP). Gated on
/// the skill id, so it's a no-op for any other cast — and, crucially, it's
/// called from BOTH the instant (melee) AND ranged resolution paths, so the
/// effect rides the SKILL, not the caster's animation/class: a future item that
/// grants this skill to another class keeps the 3×3 slow no matter how that
/// class resolves its attacks.
fn apply_aspd_dampening(state: &mut GameState, i: usize, slot: crate::skills::SkillSlot, target: usize) {
    let Some(sid) = state.grid.entities[i].skills.get(slot.get()).map(|s| s.id.clone()) else {
        return;
    };
    let Some(def) = crate::skills::skill_def(&sid) else { return; };
    if !state.grid.entities[target].alive {
        return;
    }
    let lvl = state.grid.entities[i].skills[slot.get()].level;
    // Slow magnitude + reach are DEF-driven so the effect rides the skill, not a hardcoded id:
    // a `aoe_aspd_slow_bp` skill (Banshee's Wail) uses its flat field; the acolyte's Aspd
    // Dampening keeps its level-scaled curve. Anything else isn't a dampener → no-op.
    let slow = if def.aoe_aspd_slow_bp > 0 {
        def.aoe_aspd_slow_bp as i32
    } else if sid == "aspd_dampening" {
        crate::skills::aspd_dampening_slow_bp(lvl)
    } else {
        return;
    };
    let radius = def.aoe_radius.max(1); // aspd_dampening = 1 (3×3); Wail = 2 (5×5)
    let center = state.grid.entities[target].pos;
    let team = state.grid.entities[target].team;
    let src = state.grid.entities[i].id;
    let victims: Vec<usize> = state
        .grid
        .entities
        .iter()
        .enumerate()
        // `radius` already carries this site's `.max(1)` floor — deliberately applied above and
        // NOT folded into the shape, which knows nothing about defaults.
        .filter(|(_, e)| {
            e.on_board() && e.team == team && crate::grid::Blast::radius(center, radius).contains(e.pos)
        })
        .map(|(j, _)| j)
        .collect();
    for v in victims {
        crate::skills::apply_buff(
            &mut state.grid.entities[v],
            crate::skills::Buff {
                kind: crate::skills::BuffKind::AspdSlowPct,
                magnitude: slow,
                ticks: 100, // 10s (was 50) — matches cd so the slow spreads across the pack
                source_id: src,
            },
        );
    }
}

/// Roll a cast's `on_hit_debuff` onto `tgt`. The effective chance is
/// `chance_bp + chance_per_level * skill_level`; a TOTAL of 0 applies with NO
/// PRNG draw, otherwise a single roll positioned AFTER the attack's crit/hit/
/// double draws (so skill-less streams stay byte-identical). No-op on a dead
/// target.
pub(crate) fn apply_on_hit_debuff(state: &mut GameState, i: usize, slot: crate::skills::SkillSlot, tgt: usize, spec: skills::DebuffSpec) {
    if !state.grid.entities[tgt].alive {
        return;
    }
    let level = state.grid.entities[i].skills[slot.get()].level;
    // Level-scaled proc chance (Sand Strike: blind 7%/level); 0 total = always.
    let chance = (spec.chance_bp as i32 + spec.chance_per_level * level as i32).max(0) as u32;
    let applies = chance == 0 || state.rng.gen_range_u32(0, 10_000) < chance;
    if !applies {
        return;
    }
    // Bosses are immune to STUN (RO-style). The roll above still happens (keeps
    // the draw stream consistent for the skill); we just don't apply it.
    if matches!(spec.kind, skills::BuffKind::Stun) && state.grid.entities[tgt].is_boss {
        return;
    }
    let source = state.grid.entities[i].id;
    let mut buff = spec.at_level(level, source);
    // Resonance ranks reshape a status by the skill that applies it (0.0.441).
    let (dur_bp, mag_bp) = crate::unit_rank::status_shape_bp(&state.grid.entities[i], &state.grid.entities[i].skills[slot.get()].id);
    if dur_bp != 10_000 {
        buff.ticks = ((buff.ticks as u64) * u64::from(dur_bp) / 10_000).max(1) as u32;
    }
    if mag_bp != 10_000 {
        buff.magnitude = ((buff.magnitude as i64) * i64::from(mag_bp) / 10_000) as i32;
    }
    // Darkness (0.0.426) lands at HALF strength on a boss (user: 20%, boss 10%). The full
    // strength is kept for the spread below, which halves per NEIGHBOUR (audit 2026-10-02: the
    // boss-halved copy was cloned onto the non-boss neighbours).
    let full_magnitude = buff.magnitude;
    if matches!(spec.kind, skills::BuffKind::Darkness | skills::BuffKind::Dazzle) && state.grid.entities[tgt].is_boss {
        buff.magnitude /= 2;
    }
    // A PoisonDot/Bleed spec magnitude is a POWER (basis points of the attacker's
    // ATK) for the TOTAL DoT over its lifetime; spread it across the DoT ticks
    // (one per regen interval) into a per-tick HP amount.
    if matches!(spec.kind, skills::BuffKind::PoisonDot | skills::BuffKind::Bleed | skills::BuffKind::LeechBleed) {
        let mut total = (combat::effective_atk(&state.grid.entities[i]) as i64) * (buff.magnitude as i64) / 10_000;
        // Bleed mitigates like a PHYSICAL hit — hard DEF (armor, a diminishing %) then soft
        // DEF (VIT/AGI/Level, flat) — applied ONCE to the whole-lifetime TOTAL. Per-tick
        // mitigation would subtract the flat soft-DEF every interval and wildly over-reduce
        // it. Deadly Poison (PoisonDot) stays unmitigated by design.
        if matches!(spec.kind, skills::BuffKind::Bleed | skills::BuffKind::LeechBleed) {
            let mit_bp = combat::def_mitigation_bp(&state.grid.entities[tgt]).max(0) as i64;
            let soft = combat::effective_soft_def(&state.grid.entities[tgt]).max(0) as i64;
            let after_hard = total * (10_000 - mit_bp) / 10_000;
            total = (after_hard - soft).max(1);
        }
        let ticks = (spec.duration / REGEN_INTERVAL_TICKS as u32).max(1) as i64;
        buff.magnitude = (total / ticks).max(1) as i32;
    }
    let skill_id = state.grid.entities[i].skills[slot.get()].id.clone();
    // A ranked DoT keeps several instances from one source (Nyx S2): push a new one while
    // under the allowance instead of refreshing the one `apply_buff` would find.
    let allowed = crate::unit_rank::dot_stacks(&state.grid.entities[i], &skill_id);
    let is_dot = matches!(spec.kind, skills::BuffKind::PoisonDot | skills::BuffKind::Bleed | skills::BuffKind::LeechBleed);
    let mine = state.grid.entities[tgt].buffs.iter().filter(|b| b.kind == buff.kind && b.source_id == source).count();
    if is_dot && allowed > 1 && mine < allowed {
        state.grid.entities[tgt].buffs.push(buff.clone());
    } else {
        skills::apply_buff(&mut state.grid.entities[tgt], buff.clone());
    }
    // A ranked status SPREADS to the neighbours of the target (Nyx S1: Darkness to the 3×3).
    let radius = crate::unit_rank::status_spread_radius(&state.grid.entities[i], &skill_id);
    if radius > 0 {
        let center = state.grid.entities[tgt].pos;
        let tteam = state.grid.entities[tgt].team;
        let near: Vec<usize> = state
            .grid
            .entities
            .iter()
            .enumerate()
            .filter(|(j, e)| *j != tgt && e.alive && e.team == tteam && e.on_board() && e.pos.chebyshev(center) <= radius)
            .map(|(j, _)| j)
            .collect();
        for j in near {
            let mut b = buff.clone();
            if matches!(spec.kind, skills::BuffKind::Darkness | skills::BuffKind::Dazzle) {
                b.magnitude = if state.grid.entities[j].is_boss { full_magnitude / 2 } else { full_magnitude };
            }
            skills::apply_buff(&mut state.grid.entities[j], b);
        }
    }
    crate::regalia_passive::on_status_applied(state, i, tgt);
}

/// Push `victim` `dist` cells directly away from `src`, stopping at the first
/// blocked / out-of-bounds cell. Resets its move accumulator so it re-approaches
/// cleanly. No-op when they share a cell (no direction).
/// Alive entities within Chebyshev `radius` of `center`, in stable entity-index order.
/// `hostile_to` = only the OPPOSITE team (Some) or BOTH teams (None). The entity AT
/// `center` (the blast source / caster) is excluded. Shared by every AoE blast.
pub(super) fn victims_in_radius(
    entities: &[crate::grid::Entity],
    center: crate::grid::Cell,
    radius: i32,
    hostile_to: Option<crate::grid::Team>,
) -> Vec<usize> {
    entities
        .iter()
        .enumerate()
        .filter(|(_, e)| {
            // `e.pos != center` is this helper's own policy — an on-death blast must not re-hit
            // the corpse, a self-AoE must not hit its caster. It is NOT part of the shape, which
            // is why `Blast` does not know about it.
            e.on_board()
                && e.pos != center
                && crate::grid::Blast::radius(center, radius).contains(e.pos)
                && hostile_to.is_none_or(|t| e.team != t)
        })
        .map(|(j, _)| j)
        .collect()
}

/// Shove `victim` `dist` cells AWAY from `src` (its live position). Convenience wrapper
/// over `knockback_from` for the common case where the blast source is a live entity.
pub(crate) fn knockback_entity(state: &mut GameState, src: usize, victim: usize, dist: i32) {
    let from = state.grid.entities[src].pos;
    knockback_from(state, victim, from, dist);
}

/// Which way a displacement goes, and which event it counts as.
///
/// A pull and a shove walk identically — same angle snap, same free-cell walk —
/// but they are NOT the same thing to anything watching. The renderer draws them
/// differently, and a future "when knocked back" trigger must not fire on someone
/// being hauled in. Passing the intent down means the walk stays one
/// implementation while the two events stay separate at every read site.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum Displace {
    Shove,
    Pull,
}

/// Shove `victim` `dist` cells AWAY from a CELL `from` (works for a source that's no
/// longer a live entity, e.g. a scheduled bomb explosion). Angle-snap + cell_is_free.
pub(crate) fn knockback_from(state: &mut GameState, victim: usize, from: Cell, dist: i32) {
    displace_from(state, victim, from, dist, Displace::Shove);
}

/// Haul `victim` `dist` cells TOWARD `from` (Iron Chains). Stops at the first
/// blocked cell, so a pull never lands anyone on top of the puller.
pub(crate) fn pull_toward(state: &mut GameState, victim: usize, from: Cell, dist: i32) {
    displace_from(state, victim, from, dist, Displace::Pull);
}

fn displace_from(state: &mut GameState, victim: usize, from: Cell, dist: i32, how: Displace) {
    let mut pos = state.grid.entities[victim].pos;
    // Knockback snaps to 8 directions by ANGLE, but the DIAGONAL band is narrowed
    // to ~30°–60° (vs the naive 8-way's 22.5°–67.5°): near-45° angles still shove
    // diagonally (natural), while shallow/steep angles go straight along the
    // dominant axis. Per-axis sign alone would snap any shallow angle to a diagonal
    // — barely visible at 1 cell, but obviously wrong for a multi-cell archer shove.
    // integer fixed-point: diagonal when min/max ≥ tan(30°)≈0.577 ⇔ lo·1000 ≥ hi·577.
    let (rx, ry) = (pos.x - from.x, pos.y - from.y);
    let (ax, ay) = (rx.abs(), ry.abs());
    let (lo, hi) = (ax.min(ay), ax.max(ay));
    let (dx, dy) = if lo * 1000 >= hi * 577 {
        (rx.signum(), ry.signum()) // near 45° → diagonal (handles 0,0 → guarded below)
    } else if ax >= ay {
        (rx.signum(), 0) // dominant horizontal
    } else {
        (0, ry.signum()) // dominant vertical
    };
    if dx == 0 && dy == 0 {
        return;
    }
    let (dx, dy) = match how {
        Displace::Shove => (dx, dy),
        Displace::Pull => (-dx, -dy),
    };
    for _ in 0..dist.max(0) {
        let next = Cell::new(pos.x + dx, pos.y + dy);
        if cell_is_free(state, next) {
            pos = next;
        } else {
            break;
        }
    }
    let moved = pos != state.grid.entities[victim].pos;
    let lane_on = state.lane.is_some();
    let vid = state.grid.entities[victim].id;
    let e = &mut state.grid.entities[victim];
    e.pos = pos;
    e.target = pos;
    // Lane mode: a victim that did not move keeps its sub-cell progress (zeroing it drew a rewind).
    if moved || !lane_on {
        e.move_acc = 0;
    }
    // Flag the move so the renderer plays the right arc (display-only; not hashed).
    if moved {
        match how {
            Displace::Shove => state.knockbacks.push(vid),
            Displace::Pull => state.pulls.push(vid),
        }
    }
}

/// Knock Back Shot (Ranger passive): a SURVIVING single-target basic hit can shove
/// the victim 2 cells away from the archer — but only ONCE per enemy lifetime
/// (`kb_shot_hit` marker) to prevent perma-kiting. `5% × level` chance. The PRNG
/// draw is taken ONLY when the attacker owns the skill AND the victim hasn't been
/// shoved yet, so skill-less streams (every golden) stay byte-identical.
pub(crate) fn maybe_knock_back_shot(state: &mut GameState, attacker: Option<usize>, victim: usize) {
    let Some(a) = attacker else { return; };
    let lvl = state.grid.entities[a].skill_level("knock_back_shot");
    if lvl == 0 || state.grid.entities[victim].kb_shot_hit {
        return;
    }
    if state.rng.gen_range_u32(0, 10_000) >= lvl as u32 * 500 {
        return; // missed the proc (the one-time shove is NOT consumed on a miss)
    }
    state.grid.entities[victim].kb_shot_hit = true;
    knockback_entity(state, a, victim, 2);
}

/// Pet attack-speed accumulator increment per tick: 100 ⇒ a swing every 10 ticks
/// (~1 hit/sec at 10 ticks/s).
const PET_ATK_SPEED_MILLI: u32 = 100;

/// Ranger Pet system (Pet Master). Ensures each living Pet Master owner has a pet,
/// then drives every pet's movement + attack. The pet is an UNTARGETABLE, immortal
/// grid entity (team Player, `is_pet`) with its OWN ATK (4 × Pet Master level), not
/// derived from the owner's stats. A future tank pet flips `untargetable` + gets HP.
/// All gated on owning Pet Master → no golden spawns a pet → byte-identical.
pub(crate) fn pet_step(state: &mut GameState) {
    use crate::grid::{Entity, Team};
    // 1. Spawn a pet for any living Pet Master owner that lacks one.
    let mut spawns: Vec<(u32, usize)> = Vec::new(); // (owner_id, owner_idx)
    for (idx, e) in state.grid.entities.iter().enumerate() {
        if !e.alive || e.is_pet || !matches!(e.team, Team::Player) {
            continue;
        }
        if !e.skills.iter().any(|s| s.id == "pet_master" && s.level > 0) {
            continue;
        }
        let owner_id = e.id;
        if !state.grid.entities.iter().any(|p| p.is_pet && p.alive && p.owner_id == owner_id) {
            spawns.push((owner_id, idx));
        }
    }
    for (owner_id, owner_idx) in spawns {
        let owner_pos = state.grid.entities[owner_idx].pos;
        let cell = pick_surround_cell(&state.grid, owner_idx, owner_pos).unwrap_or(owner_pos);
        let id = state.next_monster_id;
        state.next_monster_id = state.next_monster_id.wrapping_add(1);
        let mut pet = Entity::idle(id, cell).with_team(Team::Player);
        pet.is_pet = true;
        pet.owner_id = owner_id;
        pet.untargetable = true;
        pet.atk_range = 1; // melee auto-attacker
        pet.atk_speed_milli = PET_ATK_SPEED_MILLI; // ~1 hit/sec
        state.grid.spawn(pet);
    }
    // 2. Drive each living pet (move toward nearest enemy, attack on cadence).
    let count = state.grid.entities.len();
    for i in 0..count {
        if state.grid.entities[i].alive && state.grid.entities[i].is_pet {
            // ENFORCE the pet's combat stats every tick. Pets persist across saves
            // and never despawn, so one summoned before these were set (atk_speed 0
            // ⇒ a swing that took 1000 ticks = never bit) would stay broken forever.
            // Re-stamping here self-heals such stale pets on load.
            state.grid.entities[i].atk_speed_milli = PET_ATK_SPEED_MILLI;
            state.grid.entities[i].atk_range = 1;
            state.grid.entities[i].untargetable = true;
            pet_act(state, i);
        }
    }
}

/// One pet's turn: chase the nearest enemy and bite it on cadence; drift toward the
/// owner when no enemy is around.
fn pet_act(state: &mut GameState, i: usize) {
    let Some(tgt) = combat::find_enemy_idx(&state.grid.entities, i) else {
        // No enemy → stay near the owner, face nothing.
        state.grid.entities[i].target_id = None;
        if let Some(oidx) = state.grid.idx_of_id(state.grid.entities[i].owner_id) {
            let opos = state.grid.entities[oidx].pos;
            state.grid.entities[i].target = opos;
            state.grid.step_toward_target(i);
        }
        return;
    };
    let tgt_id = state.grid.entities[tgt].id;
    // Track the target so the renderer faces the pet toward it while it stands
    // and bites (slime facing falls back to target_id when not moving).
    state.grid.entities[i].target_id = Some(tgt_id);
    let pet_pos = state.grid.entities[i].pos;
    let tgt_pos = state.grid.entities[tgt].pos;
    // The swing CHARGES every tick the pet is engaged — while approaching too, not
    // only once adjacent — so a pet chasing a jittery/shuffling target arrives with
    // a ready swing and bites ON CONTACT instead of needing 10 uninterrupted
    // adjacent ticks (the bug: a melee pet whose target keeps shifting never reached
    // 1000). atk_speed 100 ⇒ ~1 hit/sec once engaged. The DROP at 1000 is what the
    // renderer reads to play the chomp.
    let aspd = state.grid.entities[i].atk_speed_milli.max(1) as u32;
    state.grid.entities[i].atk_acc += aspd;
    if pet_pos.chebyshev(tgt_pos) > state.grid.entities[i].atk_range {
        // Out of reach: hold the swing READY (cap, don't bank extra) and close in.
        state.grid.entities[i].atk_acc = state.grid.entities[i].atk_acc.min(1000);
        state.grid.entities[i].target = tgt_pos; // close the distance
        state.grid.step_toward_target(i);
        return;
    }
    if state.grid.entities[i].atk_acc < 1000 {
        return; // in range but still winding up
    }
    state.grid.entities[i].atk_acc -= 1000;
    // Pet ATK = 20 × the owner's Pet Master level (self-contained, not owner gear).
    let Some(oidx) = state.grid.idx_of_id(state.grid.entities[i].owner_id) else { return; };
    let lvl = state.grid.entities[oidx].skill_level("pet_master");
    if lvl == 0 {
        return;
    }
    // Pet ATK = 20 × Pet Master level (Lv1 20 .. Lv10 200). Pet Assault (active,
    // cd-gated) turns THIS bite into a heavy strike — 150% + 50%/level of the pet's
    // ATK (Lv1 150% .. Lv10 600%) — when the owner knows it, it's slotted, off cd.
    let mut amount = 20 * lvl as i32;
    if let Some(adef) = skills::skill_def("pet_assault") {
        let assault = state.grid.entities[oidx]
            .skills
            .iter()
            .position(|s| s.id == "pet_assault" && s.level > 0 && s.cooldown == 0);
        if let Some(slot) = assault {
            if skills::loadout_fits(
                &state.grid.entities[oidx].active_skills,
                combat::loadout_capacity(&state.grid.entities[oidx]),
                "pet_assault",
            ) {
                let alvl = state.grid.entities[oidx].skills[slot].level as i32;
                let power = 150 + 50 * (alvl - 1); // Lv1 150% .. Lv10 600%
                amount = amount * power / 100;
                state.grid.entities[oidx].skills[slot].cooldown = adef.cooldown_ticks;
            }
        }
    }
    let dmg = combat::Damage {
        atk_level: state.grid.entities[oidx].stats.level as i32, amount,
        element: Element::Physical,
        kind: DamageType::Physical,
        is_ranged: false,
        is_crit: false,
        is_aoe: false,
        mastery_bp: 0, shadow_bane_bp: 0, debuff_amp_bp: 0,
            atk_race: crate::grid::Race::Unknown,
            atk_size: crate::grid::Size::Medium,
            ignore_def_bp: 0,
    };
    let pet_id = state.grid.entities[i].id;
    let mob = combat::attackers_on(&state.grid.entities, tgt);
    let dealt = combat::apply_damage(&state.grid.entities[tgt], &dmg, mob);
    state.grid.entities[tgt].take_hit(dealt); // hp − dmg, shatter Freeze, flag death
    state.recent_hits.push(crate::state::HitEvent {
        target_id: tgt_id, attacker_id: pet_id, amount: dealt, hits: 1,
        kind: crate::state::HitKind::Normal,
    });
    if !state.grid.entities[tgt].alive {
        kill_monster(state, tgt, Some(oidx)); // credit the owner (feeds on-kill hooks)
        return;
    }
    // Snare Strike (Ranger active, reserve-gated): the bite may ROOT a surviving
    // target once per lifetime.
    maybe_snare(state, oidx, tgt);
}

/// Snare Strike (Ranger): a pet bite has `2.5% × level` chance to ROOT the target
/// for 2 s — but only ONCE per enemy lifetime (`snare_used`) and only if the owner
/// has the skill SLOTTED (reserve budget). Bosses are immune. The PRNG draw is
/// taken only when the owner owns + slots it AND the target is un-snared → no
/// golden runs it (no pets) → byte-identical.
fn maybe_snare(state: &mut GameState, owner: usize, tgt: usize) {
    if state.grid.entities[tgt].snare_used || state.grid.entities[tgt].is_boss {
        return;
    }
    let lvl = state.grid.entities[owner].skill_level("snare_strike");
    if lvl == 0 {
        return;
    }
    if !skills::loadout_fits(
        &state.grid.entities[owner].active_skills,
        combat::loadout_capacity(&state.grid.entities[owner]),
        "snare_strike",
    ) {
        return;
    }
    if state.rng.gen_range_u32(0, 10_000) >= lvl as u32 * 250 {
        return; // 2.5%/level
    }
    state.grid.entities[tgt].snare_used = true;
    let src = state.grid.entities[owner].id;
    skills::apply_buff(
        &mut state.grid.entities[tgt],
        skills::Buff { kind: skills::BuffKind::Root, magnitude: 1, ticks: 20, source_id: src },
    );
}

/// Caster-centred blast (Magnum Break): damage every enemy within
/// `self_aoe_radius` of the caster in stable entity-index order (deterministic
/// drop rolls), then knock survivors back. Kills route through the normal
/// drop + death path. Replaces the single-target resolve for a self-AoE cast.
fn resolve_self_aoe(state: &mut GameState, i: usize, cast: &ActiveCast) {
    let center = state.grid.entities[i].pos;
    let me_team = state.grid.entities[i].team;
    let r = cast.self_aoe_radius;
    let mut victims = victims_in_radius(&state.grid.entities, center, r, Some(me_team));
    // Target cap (Iron Chains hauls two): the nearest first, spawn order on a tie — except a PULL,
    // which takes the FARTHEST first (0.0.482, user: a haul is wasted on what already stands at
    // her feet). Stable sort keeps the entity-index order inside each distance band, so drop
    // rolls stay deterministic.
    let cap = cast.self_aoe_max_targets.saturating_add(if cast.self_aoe_max_targets > 0 { crate::unit_rank::self_aoe_extra_targets(&state.grid.entities[i]) } else { 0 });
    if cap > 0 && victims.len() > cap as usize {
        let far_first = cast.pull > 0;
        victims.sort_by_key(|&v| {
            let d = center.chebyshev(state.grid.entities[v].pos);
            if far_first { -d } else { d }
        });
        victims.truncate(cap as usize);
        if far_first {
            // Chosen far-first, HAULED near-first: the nearer one steps in and clears the lane
            // the farther one is dragged down.
            victims.sort_by_key(|&v| center.chebyshev(state.grid.entities[v].pos));
        }
    }
    let sid = state.grid.entities[i].skills[cast.slot.get()].id.clone();
    // Resonance ranks on a caster-centred blast (0.0.441): the caught take more damage for a
    // while (Iron Chains S1), and a crowd shortens the sweep's cooldown (Steel Break S2).
    let rank_vuln = crate::unit_rank::self_aoe_vuln(&state.grid.entities[i], &sid);
    let refund = crate::unit_rank::self_aoe_refund(&state.grid.entities[i], &sid, victims.len());
    if refund > 0 {
        let s = &mut state.grid.entities[i].skills[cast.slot.get()];
        s.cooldown = s.cooldown.saturating_sub(refund);
    }
    let caster_id = state.grid.entities[i].id;
    // DEF-scaling bonus (Shield Blast): flat extra per hit = caster HARD DEF ·
    // def_scale_bp/10000, computed ONCE (the caster's DEF doesn't change mid-blast).
    let def_bonus = if cast.def_scale_bp > 0 {
        (combat::effective_hard_def(&state.grid.entities[i]) as i64 * cast.def_scale_bp as i64 / 10_000) as i32
    } else {
        0
    };
    for vidx in victims {
        if !state.grid.entities[vidx].alive {
            continue; // a prior hit this blast already downed it
        }
        let pre_hp = state.grid.entities[vidx].hp.max(0);
        // Axis separation: the SKILL's damage type wins; else the caster's.
        let kind = cast.damage_type.unwrap_or(state.grid.entities[i].atk_type);
        let mut dealt = combat::resolve_attack_scaled(
            &mut state.grid.entities, &mut state.rng, i, vidx, cast.power_bp,
            cast.display_hits, &mut state.recent_hits, kind, cast.toughness_dmg_mult,
        );
        // Add the DEF-scaled flat bonus on top (true add — already-resolved blow).
        if def_bonus > 0 && state.grid.entities[vidx].hp > 0 {
            let extra = def_bonus.min(state.grid.entities[vidx].hp);
            let e = &mut state.grid.entities[vidx];
            e.hp -= extra;
            if e.hp <= 0 {
                e.alive = false;
            }
            dealt += extra;
        }
        // DPS meter records EFFECTIVE damage (HP actually removed), not overkill.
        state.record_damage(i, Some(&sid), dealt.min(pre_hp));
        // Static Spark (0.0.465): a caster-centred blast seeds every victim.
        if let Some(d) = skills::skill_def(&sid) {
            lay_static_spark(state, i, vidx, d);
        }
        if let (Some((vuln, ticks)), true) = (rank_vuln, state.grid.entities[vidx].alive) {
            skills::apply_buff(&mut state.grid.entities[vidx], skills::Buff { kind: skills::BuffKind::DmgTakenPct, magnitude: vuln, ticks, source_id: caster_id });
        }
        if !state.grid.entities[vidx].alive {
            kill_monster(state, vidx, Some(i));
        } else if cast.knockback > 0 {
            knockback_entity(state, i, vidx, cast.knockback);
        } else if cast.pull > 0 {
            if state.lane.is_some() {
                crate::lane::pull_beside(state, vidx, i);
            } else {
                let from = state.grid.entities[i].pos;
                pull_toward(state, vidx, from, cast.pull);
            }
        }
        // A blast may also lock every survivor onto the caster (Iron Chains):
        // the single-target path taunts too, but a gather is the whole point of
        // a pull, so the AoE path needs it as well.
        if cast.taunt_on_hit_ticks > 0 && state.grid.entities[vidx].alive {
            let me_id = state.grid.entities[i].id;
            let t = &mut state.grid.entities[vidx];
            t.taunted_by = Some(me_id);
            t.taunt_ticks = cast.taunt_on_hit_ticks;
        }
    }
}


pub(crate) fn attack_step(state: &mut GameState, i: usize, enemy_idx: usize) {
    let atk_speed = combat::effective_atk_speed_milli(&state.grid.entities[i]);
    state.grid.entities[i].atk_acc =
        state.grid.entities[i].atk_acc.saturating_add(atk_speed);
    let travel = proj_travel_to(state, i, enemy_idx);
    // Multiple swings per tick possible if atk_speed_milli > 1000.
    // Target may die mid-loop — re-check `alive` each swing.
    while state.grid.entities[i].atk_acc >= 1000 {
        state.grid.entities[i].atk_acc -= 1000;
        // The ATTACKER too (audit 2026-10-02): one killed by a reflect mid-loop swung again.
        if !state.grid.entities[enemy_idx].alive || !state.grid.entities[i].alive {
            break;
        }
        // Pick the auto-cast skill ONCE per swing (pure, no PRNG) — for both
        // melee and ranged, so the cast-time intercept can fire before any
        // crit/hit roll. Reordering the (rng-free) pick ahead of the ranged
        // rolls keeps every skill-less stream byte-identical.
        // 0.0.483: only a wind-up that opened with a skill ready may end in one. An entity here
        // with no enemy in basic reach came for a skill's reach (a blink), and keeps it.
        let may_cast = state.grid.entities[i].windup_cast || !crate::tick::swing_engaged(state, i);
        let cast_opt = if may_cast { pick_active_skill(state, i, enemy_idx) } else { None };
        // CAST-TIME intercept: a chosen spell with a windup (mage bolts /
        // Fireball) begins a CHANNEL instead of resolving now — the caster
        // stands still until the windup elapses (handled in `tick_one_entity`),
        // then `fire_pending_cast` resolves it. Skills with cast_time 0 (every
        // other active) skip this entirely and resolve immediately as before.
        if let Some(cast) = &cast_opt {
            if cast.base_cast_time > 0 {
                let windup =
                    combat::effective_cast_time(&state.grid.entities[i], cast.base_cast_time);
                // A BLINK's tick is structural, not a cast bar: it is the gap the caster stands
                // in after arriving. DEX shortens spell wind-ups, and at 1 tick it would round
                // this one to nothing, so a high-DEX thief would silently lose the split while a
                // low-DEX one kept it. Floor it.
                let windup = if cast.teleport { windup.max(1) } else { windup };
                if windup > 0 {
                    begin_channel(state, i, cast, windup);
                    break; // channel takes over; no swing this tick
                }
            }
        }
        let md = cast_opt.as_ref().map_or(0, |c| c.motion_delay);
        // Delivery axis: a skill can FORCE instant/projectile; else inherit the
        // caster's flight time. All skills are Inherit today → byte-identical.
        let effective_travel = effective_delivery_travel(cast_opt.as_ref(), travel);
        if effective_travel == 0 {
            resolve_instant(state, i, enemy_idx, cast_opt);
        } else {
            resolve_projectile(state, i, enemy_idx, cast_opt, effective_travel);
        }
        // Motion delay: a skill that resolved locks the caster for its
        // animation — stop swinging this tick; `tick_one_entity` skips the
        // entity until the lock elapses. 0 (basics / no-lock skills) = continue.
        if md > 0 {
            state.grid.entities[i].act_locked_until = state.tick + md as u64;
            break;
        }
    }
}

/// Projectile flight time (ticks) from `i` to `target_idx`. 0 = melee (no
/// `proj_travel_ticks`). `proj_travel_ticks` is the flight time at MAX range;
/// scaling it by distance/range makes bolts fly at a CONSTANT visual speed
/// (an adjacent target resolves almost instantly), floored at 1 for a shooter.
fn proj_travel_to(state: &GameState, i: usize, target_idx: usize) -> u32 {
    let e = &state.grid.entities[i];
    // A RANGED basic-attack override (Sagitta Lucis) turns an otherwise-melee class
    // into a shooter — give it a default bolt flight so the basic routes to the
    // projectile path (a flying Holy bolt), even though the class's own
    // proj_travel_ticks is 0. `effective_atk_range` already reports the override reach.
    const BASIC_OVERRIDE_TRAVEL: u32 = 2;
    let base_ticks = if e.proj_travel_ticks > 0 {
        e.proj_travel_ticks
    } else if crate::skills::basic_override(&e.skills).map_or(false, |o| o.ranged) {
        BASIC_OVERRIDE_TRAVEL
    } else {
        return 0;
    };
    let base = base_ticks as i64;
    let range = combat::effective_atk_range(&state.grid.entities[i]).max(1) as i64;
    let dist = state.grid.entities[i].pos
        .chebyshev(state.grid.entities[target_idx].pos)
        .clamp(1, range as i32) as i64;
    (((base * dist) + range / 2) / range).max(1) as u32
}

/// Begin a spell CHANNEL (mage Variable Cast Time): stand still and remember
/// the spell + locked target so it resolves the SAME way when the windup ends.
/// Cooldown is deliberately NOT set here — it's set at FIRE, so the windup and
/// the cooldown ADD UP (RO-style throughput cost that DEX/INT shorten).
fn begin_channel(state: &mut GameState, i: usize, cast: &ActiveCast, windup: u32) {
    let target_id = state.grid.entities[cast.target_idx].id;
    // A field skill aims at GROUND: lock the cell now (see `Casting.ground_cell`).
    let sid = state.grid.entities[i].skills[cast.slot.get()].id.clone();
    let ground_cell = skills::skill_def(&sid)
        .filter(|d| d.ground_field_radius > 0)
        .map(|_| state.grid.entities[cast.target_idx].pos);
    // A BLINK happens at the START of its wind-up, not at the strike: the caster is meant to be
    // standing behind its target for the tick before the hit lands. Doing it here rather than in
    // `resolve_instant` is what puts the tick BETWEEN the two.
    if cast.teleport {
        crate::tick::movement::teleport_behind(state, i, cast.target_idx);
    }
    state.grid.entities[i].casting = Some(crate::grid::Casting {
        ticks_left: windup,
        ticks_total: windup,
        slot: cast.slot,
        target_id,
        ground_cell,
    });
}

/// Resolve a finished channel: rebuild the stored spell for its slot and fire
/// it at the locked target. Fizzles (no cooldown, re-picks next tick) if the
/// target is gone. Cooldown is set here, after the windup. Caller clears
/// `casting` first / guarantees `ticks_left == 0`.
pub(crate) fn fire_pending_cast(state: &mut GameState, i: usize) {
    let Some(c) = state.grid.entities[i].casting.take() else { return; };
    // A ground-field skill PLACES its field when the channel completes (it never
    // resolves an attack) — Glacial Storm / Vermilion / Quicksand after their cast.
    // Centre on the target's CURRENT cell (fall back to the caster if it's gone).
    // `c.slot` was bound when the windup STARTED and `skills` can shrink under it
    // (`clear_exemplar_skill`), so it may now point past the end. It also arrives from the save
    // file — `Casting` is serialized — so a state that predates the fix still reaches here.
    // Fizzle: the caster re-engages next tick, which is what every other lost cast does.
    let Some(sid) = state.grid.entities[i].skills.get(c.slot.get()).map(|s| s.id.clone()) else {
        return;
    };
    if skills::skill_def(&sid).map_or(false, |d| d.ground_field_radius > 0) {
        // The cell locked at cast START (the telegraph's promise). Only a cast that
        // began without one — an instant field, or a save from before the lock —
        // falls back to reading the target's live position.
        let center = c.ground_cell.unwrap_or_else(|| {
            state.grid.entities.iter()
                .find(|e| e.id == c.target_id && e.alive)
                .map_or(state.grid.entities[i].pos, |e| e.pos)
        });
        cast::place_ground_field(state, i, c.slot, center);
        return;
    }
    // A pure-buff cast (Increase AGI with a windup) grants its buff on channel-complete.
    if skills::skill_def(&sid).map_or(false, |d| !d.cast_buff.is_empty()) {
        cast::resolve_buff_cast(state, i, c.slot);
        return;
    }
    let me_team = state.grid.entities[i].team;
    // Re-find the locked target — must still be a living enemy.
    let Some(tgt) = state.grid.entities.iter().position(|e| {
        e.id == c.target_id && e.on_board() && e.team != me_team
    }) else {
        return; // target gone → cast fizzles, mage re-engages next tick
    };
    let Some(s) = state.grid.entities[i].skills.get(c.slot.get()) else { return; };
    let Some(def) = skills::skill_def(&s.id) else { return; };
    let cast = ActiveCast {
        slot: c.slot,
        power_bp: crate::unit_rank::power(&state.grid.entities[i], def, s.level),
        cooldown: def.cooldown_ticks,
        pull: def.pull,
        toughness_dmg_mult: def.toughness_dmg_mult,
        aoe_radius: def.aoe_radius,
        self_aoe_radius: def.self_aoe_radius.max(crate::unit_rank::skill_aoe(&state.grid.entities[i], &s.id)),
        self_aoe_max_targets: def.self_aoe_max_targets,
        knockback: def.knockback,
        cast_buff: def.cast_buff,
        on_hit_debuff: def.on_hit_debuff,
        on_hit_debuff_2: def.on_hit_debuff_2,
        cast_element: def.cast_element,
        damage_type: def.damage_type,
        delivery: def.delivery,
        teleport: def.teleport,
        target_idx: tgt,
        base_cast_time: bolt_cast_time_ticks(&s.id, s.effective_cast_level(), def.cast_time_ticks),
        hits: bolt_hit_count(&s.id, s.effective_cast_level()),
        motion_delay: def.motion_delay_ticks,
        display_hits: def.display_hits,
        def_scale_bp: def.def_scale_bp,
        taunt_on_hit_ticks: def.taunt_on_hit_ticks,
        pickpocket_gil_bp: def.pickpocket_gil_bp,
        ult: def.lb_max > 0,
    };
    let md = cast.motion_delay;
    // Surface the offensive cast so the animator plays its cast_finish clip. A bolt/nuke
    // resolves WITHOUT a swing, so — exactly like Heal / Increase AGI / field casts — this
    // skill_fx is the animator's ONLY cast signal. Display-only (serde-skip → no hash impact).
    let (fx_caster, fx_target) = (state.grid.entities[i].id, state.grid.entities[tgt].id);
    state.skill_fx.push(crate::state::SkillFx { caster_id: fx_caster, target_id: fx_target, skill_id: sid.clone() });
    let travel = proj_travel_to(state, i, tgt);
    let effective_travel = effective_delivery_travel(Some(&cast), travel);
    if effective_travel == 0 {
        resolve_instant(state, i, tgt, Some(cast));
    } else {
        resolve_projectile(state, i, tgt, Some(cast), effective_travel);
    }
    // Motion delay: lock the caster so the skill animation plays out.
    if md > 0 {
        state.grid.entities[i].act_locked_until = state.tick + md as u64;
    }
}

/// Resolve one MELEE swing: an active skill auto-casts in place of the basic
/// swing when ready (`cast_opt`); both go through the same crit/hit rolls, so
/// the PRNG stream doesn't depend on which fires. `target_idx` is the nearest
/// enemy (basic) or the channel's locked target (fire path).
/// Resolve the DELIVERY axis into an effective flight time: a skill can FORCE
/// instant (0) or projectile (>=1 tick) regardless of the caster; `Inherit` (or
/// a basic, no cast) keeps the caster's own `travel`. All skills are `Inherit`
/// today, so this returns `travel` unchanged → goldens byte-identical.
fn effective_delivery_travel(cast: Option<&ActiveCast>, travel: u32) -> u32 {
    match cast.map(|c| c.delivery) {
        Some(crate::skills::Delivery::Instant) => 0,
        Some(crate::skills::Delivery::Projectile) => travel.max(1),
        _ => travel,
    }
}

/// Resolve an INSTANT strike (no projectile in flight) — a weapon swing or a
/// range-6 magic hit that lands immediately. The name is the DELIVERY axis, not
/// a claim about melee weapons: an acolyte's Holy Light resolves here too.
fn resolve_instant(state: &mut GameState, i: usize, target_idx: usize, cast_opt: Option<ActiveCast>) {
    match cast_opt {
        Some(cast) => {
            state.grid.entities[i].skills[cast.slot.get()].cooldown = cast.cooldown;
            state.grid.entities[i].skills[cast.slot.get()].last_fired = state.tick;
            if let Some(def) = skills::skill_def(&state.grid.entities[i].skills[cast.slot.get()].id) {
                cast::spend_charge(&mut state.grid.entities[i].skills[cast.slot.get()], def);
            }
            cast::communio_after_cast(state, i);
            // A cast is ONE event on the Limit Break clock however many it catches — and an
            // ultimate on this path SPENDS the gauge instead (0.0.463).
            if cast.ult {
                state.grid.entities[i].limit_break = 0;
                let sid = state.grid.entities[i].skills[cast.slot.get()].id.clone();
                crate::unit_rank::after_ult(state, i, &sid);
            } else {
                let gain = crate::skills::lb_gain(
                    &state.grid.entities[i].skills, |d| d.lb_on_skill);
                crate::skills::charge_limit_break(&mut state.grid.entities[i], gain);
            }
            // Self cast-buff (Magnum Break Fire element, Energy Coat DEF)
            // applies first so a self-AoE blast inherits the new element.
            apply_self_cast_buff(state, i, &cast);
            if cast.self_aoe_radius > 0 {
                // Point-blank AoE (Magnum Break) replaces the single-target
                // resolve and hits everything around the caster.
                resolve_self_aoe(state, i, &cast);
            } else {
                // Teleport-strike blinks behind the chosen target first — but ONLY when it has
                // no wind-up. A skill with `cast_time_ticks` already blinked in `begin_channel`;
                // blinking again here would collapse the gap that wind-up exists to create, and
                // would re-position the caster a second time if the target had turned.
                if cast.teleport && cast.base_cast_time == 0 {
                    teleport_behind(state, i, cast.target_idx);
                }
                let tgt = cast.target_idx;
                let was_alive = state.grid.entities[tgt].alive;
                let pre_hp = state.grid.entities[tgt].hp.max(0);
                let sid = state.grid.entities[i].skills[cast.slot.get()].id.clone();
                let power_bp = cast.power_bp;
                let kind = cast.damage_type.unwrap_or(state.grid.entities[i].atk_type);
                // Brutus card: Bash gains +5→10% ATK of power. The card's bonus IS
                // basis points of ATK now (0.0.316), so it joins `power_bp` directly.
                // Gated on the card, so card-less streams are untouched.
                let power_bp = if sid == "bash" {
                    power_bp + combat::card_specials::bash_atk_bonus_bp(&state.grid.entities[i])
                } else {
                    power_bp
                };
                // A no-damage utility cast (Wail: power_bp 0) skips the strike ENTIRELY — the
                // combat floor is min-1, so there is no 0-damage hit through the normal path.
                // Skipping it means zero damage AND no crit/hit PRNG draws; the cast still runs
                // its effect (apply_aspd_dampening) and animation below.
                let dealt = if power_bp == 0 {
                    0
                } else {
                    combat::resolve_attack_scaled(
                        &mut state.grid.entities, &mut state.rng, i, tgt, power_bp,
                        cast.display_hits, &mut state.recent_hits, kind,
                        cast.toughness_dmg_mult,
                    )
                };
                // Per-skill VFX hook (display-only): record this cast so the renderer
                // can play a skill-specific effect (e.g. Bash's impact) instead of the
                // generic weapon slash on the struck target.
                state.skill_fx.push(crate::state::SkillFx {
                    caster_id: state.grid.entities[i].id,
                    target_id: state.grid.entities[tgt].id,
                    skill_id: sid.clone(),
                });
                // EFFECTIVE damage only (clamp overkill to the target's HP).
                state.record_damage(i, Some(&sid), dealt.min(pre_hp));
                if was_alive && !state.grid.entities[tgt].alive {
                    kill_monster(state, tgt, Some(i));
                }
                // Pickpocket (0.0.464): the touched enemy's own price, a share of it, now.
                pickpocket(state, tgt, cast.pickpocket_gil_bp);
                // Static Spark (0.0.465): lay, or detonate what is there.
                if let Some(d) = skills::skill_def(&sid) {
                    lay_static_spark(state, i, tgt, d);
                    if d.spark_detonate {
                        let mut hits = std::mem::take(&mut state.recent_hits);
                        combat::detonate_static_spark(&mut state.grid.entities, tgt, &mut hits);
                        state.recent_hits = hits;
                    }
                }
                // On-hit debuff (Envenom poison) — one roll iff chance>0, AFTER
                // the resolve's crit/hit/double draws.
                if let Some(spec) = cast.on_hit_debuff {
                    apply_on_hit_debuff(state, i, cast.slot, tgt, spec);
                }
                if let Some(spec) = cast.on_hit_debuff_2 {
                    apply_on_hit_debuff(state, i, cast.slot, tgt, spec);
                }
                // Aspd Dampening (acolyte): slow the 3×3 around the struck target.
                // Called via a shared helper (also from the ranged path) so the
                // EFFECT rides the SKILL, not the caster's resolution path —
                // a future item granting this to another class keeps the slow.
                apply_aspd_dampening(state, i, cast.slot, tgt);
                // Cleave (Swordmaster): a targeted skill strike (Double Edge, Bash)
                // splashes just like a basic when the caster knows Cleave. Gated.
                let splash_bp = crate::skills::cleave_splash_bp(&state.grid.entities[i].skills);
                if splash_bp > 0 && dealt > 0 {
                    apply_cleave_splash(state, i, tgt, dealt, splash_bp);
                }
                // Single-target taunt (Shield Boomerang): yank the struck foe's
                // aggro onto the caster for the buff's duration.
                if cast.taunt_on_hit_ticks > 0 && state.grid.entities[tgt].alive {
                    let me_id = state.grid.entities[i].id;
                    let t = &mut state.grid.entities[tgt];
                    t.taunted_by = Some(me_id);
                    t.taunt_ticks = cast.taunt_on_hit_ticks;
                }
            }
        }
        None => {
            // Basic attack — only if the nearest enemy is actually in melee
            // range. We may have entered attack_step purely because a longer-
            // range skill could reach; without this guard that would become a
            // free out-of-range basic hit.
            let in_melee = state.grid.entities[i].pos
                .chebyshev(state.grid.entities[target_idx].pos)
                <= combat::effective_atk_range(&state.grid.entities[i]);
            if in_melee {
                let was_alive = state.grid.entities[target_idx].alive;
                let pre_hp = state.grid.entities[target_idx].hp.max(0);
                let dealt = combat::resolve_attack(
                    &mut state.grid.entities, &mut state.rng, i, target_idx,
                    &mut state.recent_hits,
                );
                // EFFECTIVE damage only (clamp overkill to the target's HP).
                state.record_damage(i, None, dealt.min(pre_hp));
                if dealt > 0 {
                    let gain = crate::skills::lb_gain(
                        &state.grid.entities[i].skills, |d| d.lb_on_basic);
                    crate::skills::charge_limit_break(&mut state.grid.entities[i], gain);
                }
                if was_alive && !state.grid.entities[target_idx].alive {
                    kill_monster(state, target_idx, Some(i));
                }
                // Cleave (Swordmaster): the basic hit splashes a fraction of its
                // damage to enemies beside the target. Gated by the passive so no
                // one else does extra work / takes PRNG draws (goldens safe).
                let splash_bp = crate::skills::cleave_splash_bp(&state.grid.entities[i].skills);
                if splash_bp > 0 && dealt > 0 {
                    apply_cleave_splash(state, i, target_idx, dealt, splash_bp);
                }
                // Fiat Lux (Exemplar passive): 20% on a landed basic hit to smite the
                // struck target with holy light. Gated on KNOWING the skill (no PRNG
                // draw for anyone else — goldens safe).
                if dealt > 0 {
                    try_fiat_lux(state, i, target_idx);
                }
                // CARD SPECIAL procs (0.0.212) on a landed basic hit — FIXED order,
                // each roll gated on the attacker actually holding the card (no PRNG
                // draw otherwise — goldens safe). Then Gnarloak's reflect for the
                // DEFENDER (deterministic, no draw).
                if dealt > 0 {
                    try_card_procs(state, i, target_idx, dealt);
                    apply_card_reflect(state, i, target_idx, dealt);
                    try_thorn_burn(state, i, target_idx, dealt);
                }
                // Bloodfrenzy (Gloomtooth passive): a landed basic hit ramps the biter's OWN
                // ASPD +5%, capped at +100%. Gated on knowing the passive → no work / no PRNG
                // draw for anyone else (goldens safe).
                if dealt > 0 {
                    try_bloodfrenzy(state, i);
                }
                // Autocounter (Swordmaster): the STRUCK defender ripostes with a
                // basic strike. Gated by the DEFENDER knowing Autocounter, holding a
                // shield, AND the autocounter SP RESERVE fitting the budget (the
                // reserve stance must be ON) — so nobody else rolls (goldens safe).
                // The riposte is a direct resolve (not a swing) so it can't re-counter.
                let cc_bp = crate::skills::autocounter_chance_bp(&state.grid.entities[target_idx].skills);
                if cc_bp > 0
                    && state.grid.entities[target_idx].alive
                    && state.grid.entities[i].alive
                    && combat::has_shield(&state.grid.entities[target_idx])
                    && crate::skills::loadout_fits(
                        &state.grid.entities[target_idx].active_skills,
                        combat::loadout_capacity(&state.grid.entities[target_idx]),
                        "autocounter",
                    )
                    && state.rng.gen_range_u32(0, 10_000) < cc_bp
                {
                    let atk_pre = state.grid.entities[i].hp.max(0);
                    let was_alive = state.grid.entities[i].alive;
                    let cdealt = combat::resolve_attack(
                        &mut state.grid.entities, &mut state.rng, target_idx, i,
                        &mut state.recent_hits,
                    );
                    state.record_damage(target_idx, Some("autocounter"), cdealt.min(atk_pre));
                    if was_alive && !state.grid.entities[i].alive {
                        kill_monster(state, i, Some(target_idx));
                    }
                }
            }
        }
    }
}

/// Fiat Lux (Exemplar passive) proc: 20% on a landed basic MELEE hit — ONE magic HOLY
/// packet of `20% ATK × level` at the struck target, drawn as a TWIN flash (2 visual
/// hits of half the total: Lv5 = 2×50% ATK … Lv10 = 2×100%).
/// The roll happens ONLY when the attacker knows the skill (exemplar-granted), so no
/// other entity's PRNG stream moves. Kills route through `kill_monster` (drops/XP);
/// damage is attributed to "fiat_lux" in the DPS meter.
/// CARD SPECIAL procs on a landed basic hit (0.0.212, user-designed gothic set).
/// FIXED roll order — Dustelle, Banshee, Crimsonne, Marionette — each roll taken
/// ONLY when the attacker has that card in an ACTIVE socket (state-keyed
/// conditional draws; card-less streams stay byte-identical).
pub(crate) fn try_card_procs(state: &mut GameState, i: usize, target_idx: usize, dealt: i32) {
    use crate::combat::card_specials::active_card_star;
    // Dustelle: 5→10% to cast Heal Lv1 on SELF (a SKILL heal — it can feed
    // Ashelle's on-healed trigger; the maid sisters combo is intentional).
    if let Some(s) = active_card_star(&state.grid.entities[i], "card_dustelle") {
        let chance = 500 + 100 * s as u32;
        if state.rng.gen_range_u32(0, 10_000) < chance {
            let heal = cast::renewal_heal_amount(&state.grid.entities[i], 1, 10_000);
            apply_skill_heal(state, i, heal);
            state.skill_fx.push(crate::state::SkillFx {
                caster_id: state.grid.entities[i].id,
                target_id: state.grid.entities[i].id,
                skill_id: "heal".to_string(),
            });
        }
    }
    // Banshee: 3% to cast Spirit Strike Lv (5+star) at the struck target, with
    // the card's +50→100% damage bonus.
    if state.grid.entities[target_idx].alive {
        if let Some(s) = active_card_star(&state.grid.entities[i], "card_banshee") {
            if state.rng.gen_range_u32(0, 10_000) < 300 {
                let bonus_bp = 5_000 + 1_000 * s as u32;
                cast_spirit_strike(state, i, target_idx, 5 + s, bonus_bp);
            }
        }
    }
    // Crimsonne: 5% to drain 5→10% of the damage dealt as HP (a direct drain,
    // NOT a skill heal — it does not feed Ashelle).
    if let Some(s) = active_card_star(&state.grid.entities[i], "card_crimsonne") {
        if state.rng.gen_range_u32(0, 10_000) < 500 {
            let drain = ((dealt as i64) * (500 + 100 * s as i64) / 10_000).max(1) as i32;
            let maxhp = combat::effective_max_hp(&state.grid.entities[i]);
            let e = &mut state.grid.entities[i];
            let before = e.hp;
            e.hp = (e.hp + drain).min(maxhp);
            let (got, who) = (e.hp - before, e.id);
            crate::state::push_heal(state, who, who, got, crate::state::HealKind::Lifesteal);
        }
    }
    // Marionette: 10→20% to inflict BLEED — 100% of the attacker's ATK spread
    // over 5 s (per-source DoT; re-proc REFRESHES this attacker's instance).
    if state.grid.entities[target_idx].alive {
        if let Some(s) = active_card_star(&state.grid.entities[i], "card_marionette") {
            let chance = 1_000 + 200 * s as u32;
            if state.rng.gen_range_u32(0, 10_000) < chance {
                let total = combat::effective_atk(&state.grid.entities[i]).max(1) as i64;
                let magnitude = (total / 5).max(1) as i32; // 5 one-second intervals
                let src = state.grid.entities[i].id;
                skills::apply_buff(
                    &mut state.grid.entities[target_idx],
                    skills::Buff { kind: skills::BuffKind::Bleed, magnitude, ticks: 50, source_id: src },
                );
            }
        }
    }
    // Toxic Chom (legacy set): 5→10% to inflict classic POISON (10 s, soft DEF
    // −25% + the never-killing DoT). The roll is taken whenever the card is held
    // (draw-stream stable); bosses just don't receive it (Stun-immunity rule).
    if state.grid.entities[target_idx].alive {
        if let Some(s) = active_card_star(&state.grid.entities[i], "card_toxic_chom") {
            let chance = 500 + 100 * s as u32;
            if state.rng.gen_range_u32(0, 10_000) < chance
                && !state.grid.entities[target_idx].is_boss
            {
                let src = state.grid.entities[i].id;
                skills::apply_buff(
                    &mut state.grid.entities[target_idx],
                    skills::Buff { kind: skills::BuffKind::Poison, magnitude: 1, ticks: 100, source_id: src },
                );
            }
        }
    }
}

/// Gnarloak: the DEFENDER reflects 5→10% of a landed basic hit back at the
/// attacker. Deterministic (no PRNG); the reflection can kill.
pub(crate) fn apply_card_reflect(state: &mut GameState, attacker_idx: usize, target_idx: usize, dealt: i32) {
    use crate::combat::card_specials::active_card_star;
    let Some(s) = active_card_star(&state.grid.entities[target_idx], "card_gnarloak") else {
        return;
    };
    let refl = ((dealt as i64) * (500 + 100 * s as i64) / 10_000).max(1) as i32;
    if !state.grid.entities[attacker_idx].alive {
        return;
    }
    let was_alive = state.grid.entities[attacker_idx].alive;
    state.grid.entities[attacker_idx].take_hit(refl);
    state.recent_hits.push(crate::state::HitEvent {
        target_id: state.grid.entities[attacker_idx].id,
        attacker_id: state.grid.entities[target_idx].id,
        amount: refl,
        hits: 1,
        kind: crate::state::HitKind::Normal,
    });
    if was_alive && !state.grid.entities[attacker_idx].alive {
        kill_monster(state, attacker_idx, Some(target_idx));
    }
}

/// Yellowling (legacy set): the DEFENDER has a 5→10% chance to IGNITE an attacker
/// whose basic hit lands — a tier-1 Combustion-style Burn (peak 20% of the hit
/// per second, linear decay over 10 s, ≈1.1× the hit total). The roll is taken
/// ONLY when the defender holds the card (state-keyed conditional draw), placed
/// AFTER Gnarloak's deterministic reflect at both basic-hit sites.
pub(crate) fn try_thorn_burn(state: &mut GameState, attacker_idx: usize, target_idx: usize, dealt: i32) {
    use crate::combat::card_specials::active_card_star;
    let Some(s) = active_card_star(&state.grid.entities[target_idx], "card_yellowling") else {
        return;
    };
    let chance = 500 + 100 * s as u32;
    if state.rng.gen_range_u32(0, 10_000) < chance && state.grid.entities[attacker_idx].alive {
        let src = state.grid.entities[target_idx].id;
        crate::tick::apply_combustion_burn(state, attacker_idx, src, dealt, 1);
    }
}

/// Apply a SKILL heal to `tidx` (clamped to max HP) and fire on-healed hooks:
/// Ashelle's card grants +2→4% ASPD for 5 s, stacking to 3 instances. EVERY
/// skill-heal resolve path must route through here (Heal / Dupla / Subvenio /
/// card procs) so the trigger can't be missed by a new heal source.
pub(crate) fn apply_skill_heal(state: &mut GameState, tidx: usize, heal: i32) {
    apply_skill_heal_from(state, tidx, tidx, heal);
}

/// `apply_skill_heal` with the HEALER named (0.0.422): the heal event credits `healer_idx`, so
/// per-member "healing done" is the healer's and not the patient's — every cast site knows its
/// caster, and a self-heal passes itself.
pub(crate) fn apply_skill_heal_from(state: &mut GameState, healer_idx: usize, tidx: usize, heal: i32) {
    let got = apply_skill_heal_raw(state, healer_idx, tidx, heal);
    // S6 (0.0.469, Astier): her SKILL heals carry the passive's buff too. Without the rank only
    // the basic's heal does (`heal_debt`, spent in the tick).
    if got > 0 && crate::unit_rank::heal_buff_everywhere(&state.grid.entities[healer_idx]).is_some() {
        healed_ally_buff(state, healer_idx, tidx);
    }
    if got > 0 && healer_idx != tidx {
        crate::regalia_passive::on_heal(state, healer_idx, tidx);
        crate::unit_rank::on_skill_heal(state, healer_idx, tidx, got);
    }
}

/// UNDA VITAE (0.0.468, Astier): every ally she heals carries +`healed_ally_dmg_bp` damage
/// dealt, `healed_ally_stacks` deep, for 8 s. One instance per healer, grown in place.
pub(crate) fn healed_ally_buff(state: &mut GameState, healer_idx: usize, tidx: usize) {
    let bonus_stacks = crate::unit_rank::heal_buff_everywhere(&state.grid.entities[healer_idx]);
    let (step, max_stacks) = state.grid.entities[healer_idx]
        .skills
        .iter()
        .filter(|s| s.level > 0)
        .filter_map(|s| skills::skill_def(&s.id))
        .fold((0i32, 0i32), |(a, b), d| (a.max(d.healed_ally_dmg_bp), b.max(d.healed_ally_stacks)));
    let max_stacks = bonus_stacks.unwrap_or(max_stacks);
    if step <= 0 || max_stacks <= 0 {
        return;
    }
    let src = HEAL_BUFF_SRC;
    let cap = step.saturating_mul(max_stacks);
    let t = &mut state.grid.entities[tidx];
    match t.buffs.iter_mut().find(|b| matches!(b.kind, skills::BuffKind::DmgDealtPct) && b.source_id == src) {
        Some(b) => {
            b.magnitude = (b.magnitude + step).min(cap);
            b.ticks = HEALED_ALLY_TICKS;
        }
        None => t.buffs.push(skills::Buff { kind: skills::BuffKind::DmgDealtPct, magnitude: step, ticks: HEALED_ALLY_TICKS, source_id: src }),
    }
}

/// How long the healed-ally damage buff lasts (0.0.468).
pub(crate) const HEALED_ALLY_TICKS: u32 = 80;

/// `source_id` of the healed-ally buff — its OWN, not the healer's entity id (0.0.469). Her
/// Tidebless carries the same `DmgDealtPct` kind under her real id, and `apply_buff` replaces a
/// buff of the same (kind, source): sharing the id made every heal overwrite her own blessing,
/// which measured as a 2.5% LOSS at the rank that heals most often.
/// Its own value (audit 2026-10-02): it shared `u32::MAX - 9` with `unit_rank::RANK_SELF_SRC`, so a
/// heal's buff and a rank stamp of the same kind overwrote each other.
pub(crate) const HEAL_BUFF_SRC: u32 = u32::MAX - 32;

/// The heal itself, without the rank / Regalia follow-ups (a rank's SPLASH heal lands here so it
/// cannot splash again). Returns the HP actually restored.
pub(crate) fn apply_skill_heal_raw(state: &mut GameState, healer_idx: usize, tidx: usize, heal: i32) -> i32 {
    let maxhp = combat::effective_max_hp(&state.grid.entities[tidx]);
    let healer = state.grid.entities[healer_idx].id;
    // S2 (0.0.468, Astier): what the heal could not fit becomes a shield.
    let overheal = crate::unit_rank::overheal_shield(&state.grid.entities[healer_idx]);
    if let Some((share_bp, ticks)) = overheal {
        let e = &state.grid.entities[tidx];
        let excess = (i64::from(e.hp) + i64::from(heal) - i64::from(maxhp)).max(0);
        let pool = (excess * i64::from(share_bp) / 10_000).min(i32::MAX as i64) as i32;
        if pool > 0 {
            let t = &mut state.grid.entities[tidx];
            t.shields.retain(|s| s.source_skill_id != "overheal");
            t.shields.push(crate::grid::AbsorbShield {
                pool,
                source_skill_id: "overheal".into(),
                burst_dmg: 0,
                burst_knock: 0,
                src_id: healer,
                hits_left: 0,
                persist: false,
            });
            let _ = ticks;
        }
    }
    let e = &mut state.grid.entities[tidx];
    let before = e.hp;
    e.hp = (e.hp + heal).min(maxhp);
    let (got, who) = (e.hp - before, e.id);
    crate::state::push_heal(state, who, healer, got, crate::state::HealKind::Skill);
    if let Some(s) =
        crate::combat::card_specials::active_card_star(&state.grid.entities[tidx], "card_ashelle")
    {
        let mag = 200 + 40 * s as i32; // bp: 2.0% → 4.0%
        let me = state.grid.entities[tidx].id;
        let stacks = state.grid.entities[tidx]
            .buffs
            .iter()
            .filter(|b| matches!(b.kind, skills::BuffKind::CardAspdStack))
            .count();
        if stacks < 3 {
            state.grid.entities[tidx].buffs.push(skills::Buff {
                kind: skills::BuffKind::CardAspdStack,
                magnitude: mag,
                ticks: 50,
                source_id: me,
            });
        } else if let Some(b) = state.grid.entities[tidx]
            .buffs
            .iter_mut()
            .filter(|b| matches!(b.kind, skills::BuffKind::CardAspdStack))
            .min_by_key(|b| b.ticks)
        {
            b.ticks = 50; // at cap — refresh the stack closest to expiring
            b.magnitude = mag;
        }
    }
    got
}

/// Spirit Strike (0.0.212, user-designed with the Banshee card; future mage
/// skill): fire ⌈level/2⌉ Spirit-element magic bolts at the target, each 100%
/// MATK — REAL multi-hit (each bolt is its own damage instance and popup).
/// `bonus_bp` = extra % damage (the Banshee card's +50→100%).
pub(crate) fn cast_spirit_strike(
    state: &mut GameState,
    i: usize,
    target_idx: usize,
    level: u8,
    bonus_bp: u32,
) {
    let bolts = (level.max(1) as usize + 1) / 2;
    let matk = combat::effective_atk(&state.grid.entities[i]).max(1) as i64;
    let amount = (matk * (10_000 + bonus_bp as i64) / 10_000).max(1) as i32;
    let pre_hp = state.grid.entities[target_idx].hp.max(0);
    let mut total = 0i32;
    let was_alive = state.grid.entities[target_idx].alive;
    for _ in 0..bolts {
        if !state.grid.entities[target_idx].alive {
            break;
        }
        let pkt = combat::Damage {
            atk_level: state.grid.entities[i].stats.level as i32, amount,
            element: crate::grid::Element::Spirit,
            kind: crate::grid::DamageType::Magic,
            is_ranged: true,
            is_crit: false,
            is_aoe: false,
            mastery_bp: 0,
            shadow_bane_bp: 0, debuff_amp_bp: 0,
            atk_race: state.grid.entities[i].race,
            atk_size: state.grid.entities[i].size,
            ignore_def_bp: 0,
        };
        let bolt_dealt = combat::apply_damage(&state.grid.entities[target_idx], &pkt, 1);
        state.grid.entities[target_idx].take_hit(bolt_dealt);
        total = total.saturating_add(bolt_dealt);
        state.recent_hits.push(crate::state::HitEvent {
            target_id: state.grid.entities[target_idx].id,
            attacker_id: state.grid.entities[i].id,
            amount: bolt_dealt,
            hits: 1,
            kind: crate::state::HitKind::Normal,
        });
    }
    state.record_damage(i, Some("spirit_strike"), total.min(pre_hp));
    state.skill_fx.push(crate::state::SkillFx {
        caster_id: state.grid.entities[i].id,
        target_id: state.grid.entities[target_idx].id,
        skill_id: "spirit_strike".to_string(),
    });
    if was_alive && !state.grid.entities[target_idx].alive {
        kill_monster(state, target_idx, Some(i));
    }
}

/// Bloodfrenzy (Gloomtooth passive): each landed basic hit grows the biter's OWN ASPD by
/// +5% (500 bp), capped at +100% (10_000 bp). A single self-sourced `AspdPct` buff, its
/// duration refreshed each hit — so the ramp DECAYS if the biter stops attacking. No PRNG,
/// no target effect; gated on knowing the passive so nobody else does any work.
fn try_bloodfrenzy(state: &mut GameState, i: usize) {
    if !crate::skills::has_bloodfrenzy(&state.grid.entities[i].skills) {
        return;
    }
    const STEP_BP: i32 = 500; // +5% per hit
    const CAP_BP: i32 = 10_000; // +100% max
    const REFRESH_TICKS: u32 = 30; // 3 s — decays if it stops attacking
    let src = state.grid.entities[i].id;
    let e = &mut state.grid.entities[i];
    if let Some(b) = e
        .buffs
        .iter_mut()
        .find(|b| matches!(b.kind, skills::BuffKind::AspdPct) && b.source_id == src)
    {
        b.magnitude = (b.magnitude + STEP_BP).min(CAP_BP);
        b.ticks = REFRESH_TICKS;
    } else {
        e.buffs.push(skills::Buff {
            kind: skills::BuffKind::AspdPct,
            magnitude: STEP_BP,
            ticks: REFRESH_TICKS,
            source_id: src,
        });
    }
}

fn try_fiat_lux(state: &mut GameState, i: usize, target_idx: usize) {
    let flv = state.grid.entities[i].skill_level("fiat_lux");
    if flv == 0 || !state.grid.entities[target_idx].alive {
        return;
    }
    if state.rng.gen_range_u32(0, 10_000) >= 2_000 {
        return; // 20%
    }
    let power = skills::skill_def("fiat_lux").map(|d| d.power_at(flv)).unwrap_or(0);
    let amount =
        ((combat::effective_atk(&state.grid.entities[i]) as i64) * power as i64 / 10_000).max(1) as i32;
    let pkt = combat::Damage {
        atk_level: state.grid.entities[i].stats.level as i32, amount,
        element: crate::grid::Element::Light,
        kind: crate::grid::DamageType::Magic,
        is_ranged: false,
        is_crit: false,
        is_aoe: false,
        mastery_bp: 0,
        shadow_bane_bp: 0, debuff_amp_bp: 0,
        atk_race: state.grid.entities[i].race,
        atk_size: state.grid.entities[i].size,
        ignore_def_bp: 0,
    };
    let pre_hp = state.grid.entities[target_idx].hp.max(0);
    let dealt = combat::apply_damage(&state.grid.entities[target_idx], &pkt, 1);
    let was_alive = state.grid.entities[target_idx].alive;
    state.grid.entities[target_idx].take_hit(dealt);
    state.recent_hits.push(crate::state::HitEvent {
        target_id: state.grid.entities[target_idx].id,
        attacker_id: state.grid.entities[i].id,
        // FULL mitigated damage, even on overkill — display convention matches the
        // basic-attack event (only the DPS meter clamps to effective damage below).
        amount: dealt,
        hits: 2, // VISUAL: one instance drawn as a TWIN flash (50%->100% ATK each)
        kind: crate::state::HitKind::Normal,
    });
    state.record_damage(i, Some("fiat_lux"), dealt.min(pre_hp));
    state.skill_fx.push(crate::state::SkillFx {
        caster_id: state.grid.entities[i].id,
        target_id: state.grid.entities[target_idx].id,
        skill_id: "fiat_lux".to_string(),
    });
    if was_alive && !state.grid.entities[target_idx].alive {
        kill_monster(state, target_idx, Some(i));
    }
}

/// Cleave splash: deal `base_dealt · splash_bp/10000` to every enemy ORTHOGONALLY
/// adjacent (the 4 N/S/E/W cells) to `target_idx`. A fraction of an ALREADY-
/// resolved blow, so no DEF/crit re-roll and no PRNG — deterministic. Records as
/// "cleave" and can kill (routes through `kill_monster` for drops/rewards).
fn apply_cleave_splash(state: &mut GameState, attacker: usize, target_idx: usize, base_dealt: i32, splash_bp: u32) {
    let splash = (base_dealt as i64 * splash_bp as i64 / 10_000) as i32;
    if splash <= 0 {
        return;
    }
    let center = state.grid.entities[target_idx].pos;
    let me_team = state.grid.entities[attacker].team;
    let victims: Vec<usize> = state
        .grid
        .entities
        .iter()
        .enumerate()
        .filter(|(j, e)| {
            *j != target_idx
                && e.on_board()
                && e.team != me_team
                && (e.pos.x - center.x).abs() + (e.pos.y - center.y).abs() == 1
        })
        .map(|(j, _)| j)
        .collect();
    for vidx in victims {
        let pre_hp = state.grid.entities[vidx].hp.max(0);
        let e = &mut state.grid.entities[vidx];
        let was_alive = e.alive;
        e.hp = e.hp.saturating_sub(splash);
        if e.hp <= 0 {
            e.alive = false;
        }
        state.record_damage(attacker, Some("cleave"), splash.min(pre_hp));
        if was_alive && !state.grid.entities[vidx].alive {
            kill_monster(state, vidx, Some(attacker));
        }
    }
}

/// Player CLICK-ATTACK (PvE, single-player early-game aid): a flat PURE
/// (true-damage) hit on the monster `target_id`. Pure damage means it's strong
/// vs low early-game HP and trivial vs late-game HP (which outscales the flat
/// amount) — so it never accelerates late-game farming. A click-kill routes
/// through the SAME drop + reward path as any other kill. Idempotent: unknown /
/// dead / non-monster id = no-op.
pub(crate) fn click_strike(state: &mut GameState, target_id: u32, dmg: i32) {
    // Rate-limit: at most ONE click per tick (100ms) — deterministic, replay-safe.
    // `last_click_tick` holds tick + 1 (0 = never clicked), so tick 0 is limited too (audit
    // 2026-10-02: the old `tick != 0` exemption allowed any number of clicks there).
    if state.last_click_tick == state.tick.saturating_add(1) {
        return;
    }
    let Some(idx) = state.grid.entities.iter().position(|e| {
        e.id == target_id && e.alive && e.team != crate::grid::Team::Player
    }) else {
        return;
    };
    state.last_click_tick = state.tick.saturating_add(1);
    let pkt = combat::Damage {
        atk_level: 1, amount: dmg,
        element: crate::grid::Element::Physical,
        kind: crate::grid::DamageType::True,
        is_ranged: false,
        is_crit: false,
        is_aoe: false,
        mastery_bp: 0, shadow_bane_bp: 0, debuff_amp_bp: 0,
            atk_race: crate::grid::Race::Unknown,
            atk_size: crate::grid::Size::Medium,
            ignore_def_bp: 0,
    };
    let dealt = combat::apply_damage(&state.grid.entities[idx], &pkt, 1);
    let e = &mut state.grid.entities[idx];
    e.hp = (e.hp - dealt).max(0);
    if e.hp == 0 {
        e.alive = false;
        kill_monster(state, idx, None);
    }
}

/// Resolve one RANGED swing: roll crit/hit (skipped for magic — bolts always
/// land, never crit, take no draws), apply the active skill's power/element,
/// lock damage at fire time, and queue the projectile. `enemy_idx` is the
/// nearest enemy (basic) or the channel's locked target (fire path).
fn resolve_projectile(
    state: &mut GameState, i: usize, enemy_idx: usize,
    cast_opt: Option<ActiveCast>, travel: u32,
) {
    // Damage type follows the SKILL's override (axis separation) then the caster
    // — so a skill declared Magic (Holy Light) is a real magic bolt (MATK
    // variance, sky-fall visual) even on a physical caster.
    // Basic-attack re-profile (Sagitta Lucis: ranged Holy MAGIC @ 50% ATK). Only the
    // no-cast path honours it — a real cast declares its own damage type. Without this
    // the ranged basic fell through to `atk_type` (the acolyte's Physical), so Sagitta
    // scaled ATK, not MATK, and stayed Neutral instead of Holy (the melee basic path in
    // combat::build_damage already applied it; the projectile path did not).
    let basic_ov = if cast_opt.is_none() {
        crate::skills::basic_override(&state.grid.entities[i].skills)
    } else {
        None
    };
    let kind = cast_opt
        .as_ref()
        .and_then(|c| c.damage_type)
        .or_else(|| basic_ov.as_ref().and_then(|o| o.damage_type))
        .unwrap_or(state.grid.entities[i].atk_type);
    let is_magic = matches!(kind, DamageType::Magic);
    // A magic skill that scales off ATK (a monster has no MATK) — Broomhilda's fireball is
    // MAGIC (MDEF-mitigated, undodgeable) but its base stat is ATK, like Sanctus Terra's field.
    let power_from_atk = cast_opt.as_ref().map_or(false, |c| {
        crate::skills::skill_def(&state.grid.entities[i].skills[c.slot.get()].id)
            .map_or(false, |d| d.field_power_from_atk)
    });
    let (is_crit, landed) = if is_magic {
        // 0.0.472: a magic bolt crits too; it still cannot be dodged.
        let crit_chance = combat::effective_crit_chance_bp(&state.grid.entities[i])
            + crate::regalia_passive::crit_rate_bonus_bp(&state.grid.entities[i], &state.grid.entities[enemy_idx]);
        (combat::roll_crit(&mut state.rng, crit_chance), true)
    } else {
        // + a Regalia passive's crit rate vs this target — the same fold as `resolve_attack_scaled`.
        let crit_chance = combat::effective_crit_chance_bp(&state.grid.entities[i])
            + crate::regalia_passive::crit_rate_bonus_bp(&state.grid.entities[i], &state.grid.entities[enemy_idx]);
        let crit = combat::roll_crit(&mut state.rng, crit_chance);
        // A ranged blow can be DODGED by its target (0.0.333); nothing else avoids it.
        let hit = crit
            || !combat::roll_dodge(
                &mut state.rng,
                &state.grid.entities[i],
                &state.grid.entities[enemy_idx],
            );
        (crit, hit)
    };
    let mut soul_crush_basic = false;
    let (power_bp, aoe_radius) = match &cast_opt {
        Some(cast) => {
            state.grid.entities[i].skills[cast.slot.get()].cooldown = cast.cooldown;
            state.grid.entities[i].skills[cast.slot.get()].last_fired = state.tick;
            if let Some(def) = skills::skill_def(&state.grid.entities[i].skills[cast.slot.get()].id) {
                cast::spend_charge(&mut state.grid.entities[i].skills[cast.slot.get()], def);
            }
            cast::communio_after_cast(state, i);
            // A cast is ONE event on the Limit Break clock however many it catches.
            {
                let gain = crate::skills::lb_gain(
                    &state.grid.entities[i].skills, |d| d.lb_on_skill);
                crate::skills::charge_limit_break(&mut state.grid.entities[i], gain);
            }
            apply_self_cast_buff(state, i, cast); // Energy Coat, etc.
            // Per-skill VFX hook (display-only) — same as the melee path, so a RANGED
            // skill cast also surfaces its name above the caster (+ any target FX).
            state.skill_fx.push(crate::state::SkillFx {
                caster_id: state.grid.entities[i].id,
                target_id: state.grid.entities[cast.target_idx].id,
                skill_id: state.grid.entities[i].skills[cast.slot.get()].id.clone(),
            });
            (cast.power_bp, cast.aoe_radius)
        }
        None if is_magic => {
            // Soul Crush (Elementalist passive): while charges remain, the mage's
            // basic becomes a 200% ATK NEUTRAL hit that spends one charge; with no
            // charges (or no Soul Crush) the mage basic is a weak 50% ATK poke.
            let sc = state.grid.entities[i].skills.iter_mut()
                .find(|s| s.id == "soul_crush" && s.charges_left > 0);
            if let Some(s) = sc {
                s.charges_left -= 1;
                soul_crush_basic = true;
                (20_000, 0)
            } else {
                (5_000, 0)
            }
        }
        None => (10_000, 0), // physical basic unchanged
    };
    // A missed projectile still flies (visual) but lands for 0 — the resolver's
    // saturating_sub makes that a harmless no-op.
    // Render tint: the bolt's FINAL element, captured from the damage packet below
    // (after every override). A miss builds no packet → stays Neutral (untinted),
    // but magic always lands so a Holy bolt like Sagitta always gets its gold.
    let mut proj_element = crate::grid::Element::Physical;
    let damage = if landed {
        // `kind` computed at the top of the fn (skill override → caster).
        // A re-profiled BASIC (Sagitta) runs through build_damage so the override's
        // element (Holy), power (50% ATK), and Holy-dmg buff all apply — identical to
        // the melee basic path; a cast/plain basic keeps the scaled builder.
        let mut packet = if basic_ov.is_some() {
            combat::build_damage(&state.grid.entities[i], is_crit)
        } else {
            combat::build_damage_scaled(&state.grid.entities[i], is_crit, power_bp, kind, power_from_atk)
        };
        // CRIT DAMAGE (0.0.224 renewal-crit): the ranged path builds its packet
        // here (not in resolve_attack_scaled), so the crit multiply layer is
        // applied here too — ×150% base + card/gear crit-dmg lines.
        if is_crit {
            let cd = i64::from(combat::effective_crit_dmg_bp(&state.grid.entities[i]))
                + crate::regalia_passive::crit_dmg_bonus_bp(&state.grid.entities[i], &state.grid.entities[enemy_idx]);
            packet.amount = ((packet.amount as i64) * cd / 10_000) as i32;
        }
        packet.is_aoe = aoe_radius > 0;
        // Soul Crush forces its 200% ATK basic to NEUTRAL.
        if soul_crush_basic {
            packet.element = crate::grid::Element::Physical;
        }
        // Per-skill element override (the 3 mage bolts: Fire/Earth/Water).
        if let Some(el) = cast_opt.as_ref().and_then(|c| c.cast_element) {
            packet.element = el;
        } else if !is_magic {
            // Elemental-arrow autoswap: a physical archer with loaded quivers
            // fires the element that best beats THIS target, spending one round.
            let target_el = state.grid.entities[enemy_idx].element;
            if let Some(ai) = combat::pick_ammo_idx(&state.grid.entities[i].ammo, target_el) {
                packet.element = state.grid.entities[i].ammo[ai].element;
                let st = &mut state.grid.entities[i].ammo[ai];
                st.shots -= 1;
                if st.shots == 0 {
                    state.grid.entities[i].ammo.remove(ai);
                }
            }
        }
        // dmg-vs folds (0.0.225): the ranged path mirrors resolve_attack_scaled —
        // gear Hydra-lineage (element × race × size layers, combined in
        // dmg_vs_bp) then the card channel bucket. Resolved vs the PRIMARY
        // target (splash victims inherit the folded packet, like cleave).
        // Placed AFTER every element override above so dmg_with_atk_element
        // (Broomhilda/Burst Chom) sees the shot's FINAL element. Deterministic
        // (no draws) and 0 without the gear/cards — mod-less streams identical.
        {
            let atk_e = &state.grid.entities[i];
            let def_e = &state.grid.entities[enemy_idx];
            // CONDITIONAL DAMAGE (0.0.488 fix): the Regalia passive, taunted-target and
            // broken-target bonuses were folded only on the melee path — a bow or a bolt never
            // got Kestrel / Meridian / Cutpurse. Same fold, same place in the order.
            let rb = crate::regalia_passive::hit_bonus_bp(atk_e, def_e)
                + crate::skills::taunted_target_bp(atk_e, def_e)
                + crate::skills::broken_target_bp(atk_e, def_e);
            if rb > 0 {
                packet.amount = ((packet.amount as i64) * (10_000 + rb) / 10_000).min(i32::MAX as i64) as i32;
            }
            let vs = combat::stat_mods::dmg_vs_bp(
                atk_e, &def_e.weaknesses,
                combat::stat_mods::race_of(def_e), combat::stat_mods::size_of(def_e),
            );
            if vs > 0 {
                packet.amount = ((packet.amount as i64) * (10_000 + vs as i64) / 10_000) as i32;
            }
            let card_bp = combat::card_specials::dmg_with_atk_element_bp(atk_e, packet.element)
                + combat::stat_mods::element_dmg_bp(atk_e, packet.element);
            if card_bp > 0 {
                packet.amount = ((packet.amount as i64) * (10_000 + card_bp as i64) / 10_000) as i32;
            }
            // BOSS DAMAGE — also missing here until 0.0.488.
            if def_e.is_boss {
                let boss_bp = combat::stat_mods::dmg_vs_boss_bp(atk_e);
                if boss_bp > 0 {
                    packet.amount = ((packet.amount as i64) * (10_000 + boss_bp as i64) / 10_000) as i32;
                }
            }
        }
        // PHYSICAL bolts (arrows) get RO weapon variance; MAGIC gets the RO
        // Renewal MATK range (weapon-MATK variance). One draw either way, after
        // the crit/hit rolls.
        if !is_magic {
            combat::apply_weapon_variance(
                &mut state.rng, &mut packet.amount, &state.grid.entities[i],
            );
        } else {
            combat::apply_matk_variance(
                &mut state.rng, &mut packet.amount, &state.grid.entities[i],
            );
        }
        proj_element = packet.element; // render tint (final element, all overrides applied)
        let mob = combat::attackers_on(&state.grid.entities, enemy_idx);
        combat::apply_damage(&state.grid.entities[enemy_idx], &packet, mob)
    } else {
        0
    };
    // Multi-hit (RO bolts): this cast delivers `hits` separate hits.
    let hits = cast_opt.as_ref().map_or(1, |c| c.hits.max(1));
    let damage = damage;
    // The mage-rune fields below retired with the rune system (0.0.369); the
    // projectile still carries them so its shape is unchanged, always zero.
    let burn_tier: u8 = 0;
    let focus_mult_bp: u32 = 0;
    let pin_slow_pct: i32 = 0;
    // DPS meter: ranged damage is locked at FIRE (applied on arrival) — record the
    // multi-hit total, but clamp to the target's HP AT FIRE so OVERKILL isn't
    // counted (effective damage). Approximate for the ranged path: hits land later
    // and other attackers may chip in between, but it cuts the gross overkill.
    let r_sid = cast_opt.as_ref().map(|c| state.grid.entities[i].skills[c.slot.get()].id.clone());
    let volley = damage.saturating_mul(hits as i32);
    let pre_hp = state.grid.entities[enemy_idx].hp.max(0);
    state.record_damage(i, r_sid.as_deref(), volley.min(pre_hp));
    // On-hit debuff (Frost Bolt slow) applies at FIRE time, on a landed hit
    // only — one roll iff chance>0, after the crit/hit draws above.
    if landed {
        if let Some(spec) = cast_opt.as_ref().and_then(|c| c.on_hit_debuff) {
            apply_on_hit_debuff(state, i, cast_opt.as_ref().unwrap().slot, enemy_idx, spec);
        }
        if let Some(spec) = cast_opt.as_ref().and_then(|c| c.on_hit_debuff_2) {
            apply_on_hit_debuff(state, i, cast_opt.as_ref().unwrap().slot, enemy_idx, spec);
        }
        // Aspd Dampening from a RANGED caster (a future cross-class item) — same
        // 3×3 slow as the melee path, so the skill isn't coupled to how the
        // caster resolves attacks.
        if let Some(cast) = cast_opt.as_ref() {
            apply_aspd_dampening(state, i, cast.slot, enemy_idx);
        }
    }
    // RO bolts RAIN `hits` bolts onto the target, staggered ~2 ticks apart (each
    // a separate damage instance). Single-hit attacks push exactly one.
    const BOLT_STAGGER: u64 = 2;
    let base_arrive = state.tick + travel as u64;
    // A magic SKILL bolt (cast) rains from the sky; a BASIC magic attack (no cast)
    // flies from the staff. AoE casts (Fireball) fly regardless (renderer handles).
    let sky_fall = is_magic && cast_opt.is_some() && aoe_radius == 0;
    // The skill that fired this, for the renderer. "" on a basic attack.
    let fx_skill_id: String = cast_opt
        .as_ref()
        .and_then(|c| state.grid.entities[i].skills.get(c.slot.get()))
        .map(|s| s.id.clone())
        .unwrap_or_default();
    // VISUAL multi-hit for a SINGLE-projectile skill (Twin Strafe → 2 numbers). For a
    // multi-BOLT skill (hits > 1, already N real projectiles) each bolt shows one.
    let display_hits = if hits == 1 {
        cast_opt.as_ref().map_or(1, |c| c.display_hits.max(1))
    } else {
        1
    };
    for k in 0..hits {
        state.projectiles.push(Projectile {
            is_basic: cast_opt.is_none(),
            target_id: state.grid.entities[enemy_idx].id,
            spawn_pos: state.grid.entities[i].pos,
            target_pos: state.grid.entities[enemy_idx].pos,
            spawn_tick: state.tick,
            arrive_at_tick: base_arrive + (k as u64) * BOLT_STAGGER,
            damage,
            is_magic,
            skill_id: fx_skill_id.clone(),
            aoe_radius,
            victim_team: state.grid.entities[enemy_idx].team,
            sky_fall,
            attacker_id: state.grid.entities[i].id,
            display_hits,
            crit: is_crit,
            burn_tier,
            focus_mult_bp,
            pin_slow_pct,
            element: proj_element,
        });
    }
}
