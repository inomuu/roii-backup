//! REGALIA PASSIVES (0.0.435) — the third number on a W-Engine.
//!
//! A Regalia is a lump of ATK, one advanced stat, and a PASSIVE keyed on the wearer's ROLE, not
//! on a skill by name (user 2026-09-18: "การออกแบบประจำตัวต้องไม่อิงสกิลเป๊ะๆ … ดูดีไซน์จริงๆ ของ ZZZ
//! HSR"). The conditions are the ones those games use — an enemy under your taunt, a shield that
//! broke on you, how many enemies are on the field, a Toughness break you caused, an ally you
//! healed, a debuffed target, a shot from range, a status you applied, a target below half —
//! so any wearer in that role gets something, and the character whose kit walks into the
//! condition every fight gets it all.
//!
//! Values are `base + per_star × stars` (Resonance S0..S5). Every effect lands as an ordinary
//! buff or a fold into a damage packet: no PRNG draw anywhere, and a wearer without a passive
//! (every 3★/4★ today, every golden) is byte-identical to before.

use crate::grid::{Entity, Team};
use crate::items::StatMod;
use crate::skills::{apply_buff, Buff, BuffKind};
use crate::state::GameState;

/// `source_id` of every buff a Regalia passive stamps on its OWN wearer, so it never collides
/// with a self-cast skill buff of the same kind. Entity ids are small; this is not one.
pub const REGALIA_SELF_SRC: u32 = u32::MAX - 7;
/// …and the source of the TIMED self-buffs (shield break, break), so a sustained stamp and a
/// timed one on the same wearer stack instead of replacing each other.
pub const REGALIA_EVENT_SRC: u32 = u32::MAX - 8;
/// Source of the `StatusVuln` debuff a wearer lays on ITS TARGET (audit 2026-10-02). It used the
/// wearer's own id — the same key the Toughness break and rank `nuke_vuln` vulnerabilities use —
/// so one overwrote or stacked onto the other. Shared by every wearer: the stacks have one cap.
pub const REGALIA_VULN_SRC: u32 = u32::MAX - 30;
/// Source of the buffs a wearer's heal lays on an ALLY (Litany of Dawn, Tidecaller, the DR
/// trigger). They used the healer's id — the key of the healer's OWN cast buffs on that ally
/// (Benedictio ATK, Tidebless DMG) — so every heal overwrote the longer buff with a shorter one
/// (audit 2026-10-02).
pub const REGALIA_ALLY_SRC: u32 = u32::MAX - 31;

/// One condition → one effect. `base` / `per_star` in basis points unless the field says ticks.
#[derive(Debug, Clone, Copy)]
pub enum Trigger {
    /// While the wearer holds ≥1 enemy under its taunt, every party member takes −`base` damage.
    TauntGuard { base: i32, per_star: i32 },
    /// When a shield ON the wearer breaks, the wearer gains +`base` ATK for `ticks`.
    ShieldBreakAtk { base: i32, per_star: i32, ticks: u32 },
    /// +`base` ATK per living enemy on the board, at most `max_enemies` counted.
    AtkPerEnemy { base: i32, per_star: i32, max_enemies: i32 },
    /// When the wearer breaks a target's Toughness, +`base` ATK for `ticks`.
    OnBreakAtk { base: i32, per_star: i32, ticks: u32 },
    /// An ally the wearer heals with a skill gains +`base` ATK for `ticks` (one instance per healer).
    HealedAllyAtk { base: i32, per_star: i32, ticks: u32 },
    /// Hits on a target carrying ≥1 debuff deal +`base`.
    DebuffedTargetDmg { base: i32, per_star: i32 },
    /// A RANGED attacker's hits deal +`base` — at any distance, point-blank included (0.0.486,
    /// user: "โจมตีระยะไกลก็พอ ธนูยิงติดตัวก็นับ"; was 4 cells or further). `Entity::is_ranged`.
    RangedDmg { base: i32, per_star: i32 },
    /// Each status the wearer applies leaves +`base` damage taken on the target, `max_stacks`
    /// deep, for `ticks` (one instance per wearer, magnitude grown in place).
    StatusVuln { base: i32, per_star: i32, max_stacks: i32, ticks: u32 },
    /// Hits on a target below `threshold_pct`% HP deal +`base`.
    LowHpTargetDmg { threshold_pct: i32, base: i32, per_star: i32 },
    /// Each status the wearer applies to an enemy gives the WEARER +`base` ATK, `max_stacks`
    /// deep, for `ticks` (one instance, magnitude grown in place, timer refreshed).
    DebuffAppliedAtk { base: i32, per_star: i32, max_stacks: i32, ticks: u32 },
    /// An ally the wearer HEALS takes `base` less damage for `ticks` (0.0.468, Tidecaller).
    HealedAllyDr { base: i32, per_star: i32, ticks: u32 },
    /// When the wearer casts its ULTIMATE, every living party member is healed `base` of its
    /// own max HP.
    UltHealParty { base: i32, per_star: i32 },
    // --- the 3★ set (0.0.485) ---
    /// While the wearer holds ≥1 enemy under its taunt, the WEARER alone takes −`base` damage.
    TauntGuardSelf { base: i32, per_star: i32 },
    /// While the wearer's HP is strictly below `threshold_pct`% of max, +`base` DEF.
    LowHpDef { threshold_pct: i32, base: i32, per_star: i32 },
    /// When the wearer casts its ULTIMATE, every living party member (wearer included) gains
    /// +`base` ATK for `ticks` — `BellAtkPct`, so two Bells never add: the strongest counts.
    UltPartyAtk { base: i32, per_star: i32, ticks: u32 },
    /// Toughness damage the wearer deals is +`base` (HP damage untouched).
    ToughnessDmg { base: i32, per_star: i32 },
    // --- SSR two-zone pass (0.0.488, user 2026-09-28): the conditional half lands in a zone the
    // always-on half does not, and every condition is a ROLE event, never one unit's skill. ---
    /// After the wearer breaks a target: the WEARER deals +`base` damage (`DmgDealtPct`) for
    /// `ticks`, on every target — a buff on the breaker, not a window on the broken one.
    OnBreakDmg { base: i32, per_star: i32, ticks: u32 },
    /// After the wearer breaks a target: the wearer's CRIT DMG +`base` for `ticks`.
    OnBreakCritDmg { base: i32, per_star: i32, ticks: u32 },
    /// After the wearer breaks a target: the wearer's CRIT RATE +`base` for `ticks` — paired with
    /// `OnBreakCritDmg` so the window crits often as well as hard (a crit-damage line alone is
    /// worth its rate share, and our crit rates are low).
    OnBreakCritRate { base: i32, per_star: i32, ticks: u32 },
    /// The wearer's crits on a target carrying ≥1 debuff deal +`base` CRIT DMG.
    DebuffedTargetCritDmg { base: i32, per_star: i32 },
    /// The wearer's hits on a target carrying ≥1 debuff roll +`base` CRIT RATE (paired with the
    /// damage line so the window crits often as well as hard).
    DebuffedTargetCritRate { base: i32, per_star: i32 },
    /// Hits on a target carrying at least `min_kinds` DISTINCT debuffs deal +`base`.
    MultiDebuffTargetDmg { min_kinds: i32, base: i32, per_star: i32 },
    /// An ally the wearer heals deals +`base` damage (`DmgDealtPct`) for `ticks`.
    HealedAllyDmg { base: i32, per_star: i32, ticks: u32 },
}

/// A Regalia's passive: flat lines that are always on (scaled by stars) plus its triggers.
#[derive(Debug, Clone, Copy)]
pub struct Passive {
    /// `(mod at S0, added per star)` — `StatMod::with_value` carries the kind.
    pub mods: &'static [(StatMod, i32)],
    pub triggers: &'static [Trigger],
}

fn at(base: i32, per_star: i32, stars: u8) -> i32 {
    base.saturating_add(per_star.saturating_mul(i32::from(stars)))
}

/// The five signature pieces — one per unit, named after the SSR rung of its class line.
pub fn passive_of(regalia_id: &str) -> Option<&'static Passive> {
    Some(match regalia_id {
        // Seraphine — the tank: holding the room protects the room; a shield that breaks on her
        // is an insult she answers.
        "aurora_edge" => &Passive {
            mods: &[],
            triggers: &[
                Trigger::TauntGuard { base: 1_000, per_star: 250 },
                Trigger::ShieldBreakAtk { base: 6_000, per_star: 1_500, ticks: 100 },
            ],
        },
        // Rapis — the room-wide mage: more enemies is more fuel; a break is a window.
        "empyrean" => &Passive {
            mods: &[],
            triggers: &[
                Trigger::AtkPerEnemy { base: 800, per_star: 200, max_enemies: 5 },
                Trigger::OnBreakDmg { base: 3_000, per_star: 750, ticks: 80 },
            ],
        },
        // Lucielle — the healer: whoever she mends, hits harder for a while.
        "litany_of_dawn" => &Passive {
            mods: &[(StatMod::HealBp(4_000), 1_000)],
            triggers: &[Trigger::HealedAllyAtk { base: 4_000, per_star: 1_000, ticks: 80 }],
        },
        // Aveline — the sniper: a marked target, from a distance.
        "meridian" => &Passive {
            mods: &[],
            triggers: &[
                Trigger::RangedDmg { base: 1_500, per_star: 375 },
                Trigger::DebuffedTargetCritRate { base: 1_300, per_star: 325 },
                Trigger::DebuffedTargetCritDmg { base: 3_000, per_star: 750 },
            ],
        },
        // Nyx — the status thief: every status opens the target further; finish what is low.
        "silent_verdict" => &Passive {
            mods: &[],
            triggers: &[
                Trigger::StatusVuln { base: 1_200, per_star: 300, max_stacks: 3, ticks: 80 },
                Trigger::LowHpTargetDmg { threshold_pct: 50, base: 5_000, per_star: 1_250 },
            ],
        },
        // Nemu — Lycoris Lullaby: every status laid feeds her; the Striker who debuffs climbs.
        "lycoris_lullaby" => &Passive {
            mods: &[],
            triggers: &[
                Trigger::DebuffAppliedAtk { base: 1_000, per_star: 250, max_stacks: 6, ticks: 80 },
                Trigger::MultiDebuffTargetDmg { min_kinds: 2, base: 4_000, per_star: 1_000 },
            ],
        },
        // Lumi — Hoarfrost Crown (user 2026-09-21: "ผู้สวมใส่ +def 16%, เมื่อใช้อัลติเมทจะฮีลทุกคนใน
        // party 10% ของ max hp"): a Sentinel's ultimate is the moment the party is safest.
        "hoarfrost_crown" => &Passive {
            mods: &[(StatMod::DefBp(1_600), 400)],
            triggers: &[Trigger::UltHealParty { base: 2_000, per_star: 500 }],
        },
        // Airi — Thunderhead: the Striker who calls the last shot — a target under half takes
        // more (her chain and storm already pick the lowest). No break trigger: Striker, not
        // Destroyer (user).
        "thunderhead" => &Passive {
            mods: &[(StatMod::AtkBp(2_400), 600)],
            triggers: &[Trigger::LowHpTargetDmg { threshold_pct: 50, base: 4_000, per_star: 1_000 }],
        },
        // Charon — Ferryman's Lantern: the Destroyer's break is her moment.
        "ferrymans_lantern" => &Passive {
            mods: &[(StatMod::AtkBp(1_400), 350)],
            triggers: &[
                Trigger::OnBreakCritRate { base: 1_800, per_star: 450, ticks: 80 },
                Trigger::OnBreakCritDmg { base: 3_600, per_star: 900, ticks: 80 },
            ],
        },
        // Suzu — Emberbell: the breaker's moment, the SR rung of it.
        "emberbell" => &Passive {
            mods: &[(StatMod::AtkBp(1_300), 325)],
            triggers: &[Trigger::OnBreakAtk { base: 3_300, per_star: 825, ticks: 80 }],
        },
        // Mirei — Stormglass Pin: every status she lays feeds her.
        "stormglass_pin" => &Passive {
            mods: &[(StatMod::AtkBp(1_400), 350)],
            triggers: &[Trigger::DebuffAppliedAtk { base: 600, per_star: 150, max_stacks: 4, ticks: 80 }],
        },
        // Astier — Tidecaller: what she mends, she also shields from the next blows.
        "tidecaller" => &Passive {
            mods: &[(StatMod::MaxHpBp(1_200), 300)],
            triggers: &[
                Trigger::HealedAllyDr { base: 1_500, per_star: 375, ticks: 80 },
                Trigger::HealedAllyDmg { base: 2_000, per_star: 500, ticks: 80 },
            ],
        },
        // Zolfia — Winter's Oath: the Destroyer's break is her moment, as the other SSR piece.
        "winters_oath" => &Passive {
            mods: &[(StatMod::AtkBp(1_800), 450)],
            triggers: &[Trigger::OnBreakAtk { base: 4_500, per_star: 1_125, ticks: 80 }],
        },
        // Sylvie — Static Charm: the wearer's crits hit harder; no trigger (a pure buffer's piece).
        "static_charm" => &Passive {
            mods: &[(StatMod::CritBp(1_600), 400)],
            triggers: &[],
        },
        // THE 3★ SET (0.0.485, docs/regalia-three-star-backend-handoff-2026-09-27.md) — two per
        // Doctrine, every number doubling S0 → S5. Proposed balance, not yet benchmarked there.
        "hearthguard" => &Passive { mods: &[], triggers: &[Trigger::TauntGuardSelf { base: 500, per_star: 125 }] },
        "last_rivet" => &Passive {
            mods: &[],
            triggers: &[Trigger::LowHpDef { threshold_pct: 50, base: 1_200, per_star: 300 }],
        },
        "kestrel" => &Passive { mods: &[], triggers: &[Trigger::RangedDmg { base: 1_200, per_star: 300 }] },
        "emberwick" => &Passive {
            mods: &[],
            triggers: &[Trigger::AtkPerEnemy { base: 600, per_star: 150, max_enemies: 3 }],
        },
        "cutpurse" => &Passive { mods: &[], triggers: &[Trigger::DebuffedTargetDmg { base: 1_200, per_star: 300 }] },
        // One stack: a status applied refreshes the 6 s, it never climbs.
        "rouge_thread" => &Passive {
            mods: &[],
            triggers: &[Trigger::DebuffAppliedAtk { base: 1_600, per_star: 400, max_stacks: 1, ticks: 60 }],
        },
        "vesper" => &Passive { mods: &[(StatMod::HealBp(1_600), 400)], triggers: &[] },
        "wayfarers_bell" => &Passive {
            mods: &[],
            triggers: &[Trigger::UltPartyAtk { base: 1_200, per_star: 300, ticks: 60 }],
        },
        "faultline" => &Passive {
            mods: &[],
            triggers: &[Trigger::OnBreakAtk { base: 2_400, per_star: 600, ticks: 60 }],
        },
        "striking_stone" => &Passive { mods: &[], triggers: &[Trigger::ToughnessDmg { base: 1_600, per_star: 400 }] },
        _ => return None,
    })
}

/// The wearer's passive and Resonance — only when the wearer's DOCTRINE is the piece's (user:
/// "ใส่ได้ทุกตัว แต่สกิลจะทำงานเฉพาะตรงสาย Doctrine"). The lump and advanced stat never ask.
pub fn of_wearer(e: &Entity) -> Option<(&'static Passive, u8)> {
    let r = e.regalia.as_ref()?;
    let p = passive_of(&r.id)?;
    let needed = crate::items::regalia_doctrine(&r.id)?;
    if crate::potential::doctrine_for_unit(&e.unit_def) != Some(needed) {
        return None;
    }
    Some((p, r.stars))
}

/// The always-on lines at `stars` — folded into `RegaliaDef::mods_at` next to the lump.
pub fn flat_mods(regalia_id: &str, stars: u8) -> Vec<StatMod> {
    passive_of(regalia_id)
        .map(|p| p.mods.iter().map(|(m, step)| m.with_value(at(m.value(), *step, stars))).collect())
        .unwrap_or_default()
}

/// Per-tick stamps (after `decay_buffs`, so a 2-tick buff refreshed every tick never lapses):
/// TauntGuard onto the party, AtkPerEnemy onto the wearer.
pub fn tick(state: &mut GameState) {
    let enemies = state
        .grid
        .entities
        .iter()
        .filter(|e| matches!(e.team, Team::Monster) && e.alive && e.on_board())
        .count() as i32;
    // (wearer id, party-wide DR, own DR, own ATK%, own DEF%) — read first, write after, so the
    // party stamp never borrows the entity list twice.
    let stamps: Vec<(u32, i32, i32, i32, i32)> = state
        .grid
        .entities
        .iter()
        .filter(|e| e.alive && matches!(e.team, Team::Player))
        .filter_map(|e| {
            let (p, stars) = of_wearer(e)?;
            let mut dr = 0;
            let mut self_dr = 0;
            let mut atk = 0;
            let mut def = 0;
            for t in p.triggers {
                match *t {
                    Trigger::TauntGuard { base, per_star } if e.taunt_hold > 0 => dr = at(base, per_star, stars),
                    Trigger::TauntGuardSelf { base, per_star } if e.taunt_hold > 0 => self_dr = at(base, per_star, stars),
                    Trigger::AtkPerEnemy { base, per_star, max_enemies } if enemies > 0 => {
                        atk = at(base, per_star, stars).saturating_mul(enemies.min(max_enemies));
                    }
                    // STRICTLY below: exactly half does not qualify.
                    Trigger::LowHpDef { threshold_pct, base, per_star } => {
                        let max = i64::from(crate::combat::effective_max_hp(e).max(1));
                        if i64::from(e.hp.max(0)) * 100 < max * i64::from(threshold_pct) {
                            def = at(base, per_star, stars);
                        }
                    }
                    _ => {}
                }
            }
            (dr > 0 || self_dr > 0 || atk > 0 || def > 0).then_some((e.id, dr, self_dr, atk, def))
        })
        .collect();
    for (me, dr, self_dr, atk, def) in stamps {
        if self_dr > 0 || def > 0 {
            if let Some(e) = state.grid.entities.iter_mut().find(|e| e.id == me) {
                if self_dr > 0 {
                    apply_buff(e, Buff { kind: BuffKind::DmgReducePct, magnitude: self_dr, ticks: 2, source_id: REGALIA_SELF_SRC });
                }
                if def > 0 {
                    apply_buff(e, Buff { kind: BuffKind::DefPct, magnitude: def, ticks: 2, source_id: REGALIA_SELF_SRC });
                }
            }
        }
        if dr > 0 {
            for e in state.grid.entities.iter_mut() {
                if e.alive && matches!(e.team, Team::Player) && !e.is_pet {
                    apply_buff(e, Buff { kind: BuffKind::DmgReducePct, magnitude: dr, ticks: 2, source_id: me });
                }
            }
        }
        if atk > 0 {
            if let Some(e) = state.grid.entities.iter_mut().find(|e| e.id == me) {
                apply_buff(e, Buff { kind: BuffKind::AtkPct, magnitude: atk, ticks: 2, source_id: REGALIA_SELF_SRC });
            }
        }
    }
}

/// A shield on `holder_idx` just broke (`resolve_absorb_bursts`).
pub fn on_shield_break(state: &mut GameState, holder_idx: usize) {
    let Some(holder) = state.grid.entities.get_mut(holder_idx) else { return };
    let Some((p, stars)) = of_wearer(holder) else { return };
    for t in p.triggers {
        if let Trigger::ShieldBreakAtk { base, per_star, ticks } = *t {
            apply_buff(holder, Buff { kind: BuffKind::AtkPct, magnitude: at(base, per_star, stars), ticks, source_id: REGALIA_EVENT_SRC });
        }
    }
}

/// `attacker` just broke a target's Toughness (`combat::damage`, entity slice in hand).
pub fn on_break(attacker: &mut Entity) {
    let Some((p, stars)) = of_wearer(attacker) else { return };
    for t in p.triggers {
        match *t {
            Trigger::OnBreakAtk { base, per_star, ticks } => {
                apply_buff(attacker, Buff { kind: BuffKind::AtkPct, magnitude: at(base, per_star, stars), ticks, source_id: REGALIA_EVENT_SRC });
            }
            Trigger::OnBreakDmg { base, per_star, ticks } => {
                apply_buff(attacker, Buff { kind: BuffKind::DmgDealtPct, magnitude: at(base, per_star, stars), ticks, source_id: REGALIA_EVENT_SRC });
            }
            Trigger::OnBreakCritDmg { base, per_star, ticks } => {
                apply_buff(attacker, Buff { kind: BuffKind::CritDmgBpAdd, magnitude: at(base, per_star, stars), ticks, source_id: REGALIA_EVENT_SRC });
            }
            Trigger::OnBreakCritRate { base, per_star, ticks } => {
                apply_buff(attacker, Buff { kind: BuffKind::CritBpAdd, magnitude: at(base, per_star, stars), ticks, source_id: REGALIA_EVENT_SRC });
            }
            _ => {}
        }
    }
}

/// `healer_idx` restored HP on `target_idx` with a skill.
pub fn on_heal(state: &mut GameState, healer_idx: usize, target_idx: usize) {
    let Some(healer_e) = state.grid.entities.get(healer_idx) else { return };
    let Some((p, stars)) = of_wearer(healer_e) else { return };
    let Some(target) = state.grid.entities.get_mut(target_idx) else { return };
    for t in p.triggers {
        if let Trigger::HealedAllyDr { base, per_star, ticks } = *t {
            apply_buff(target, Buff { kind: BuffKind::DmgReducePct, magnitude: at(base, per_star, stars), ticks, source_id: REGALIA_ALLY_SRC });
        }
        if let Trigger::HealedAllyAtk { base, per_star, ticks } = *t {
            apply_buff(target, Buff { kind: BuffKind::AtkPct, magnitude: at(base, per_star, stars), ticks, source_id: REGALIA_ALLY_SRC });
        }
        if let Trigger::HealedAllyDmg { base, per_star, ticks } = *t {
            apply_buff(target, Buff { kind: BuffKind::DmgDealtPct, magnitude: at(base, per_star, stars), ticks, source_id: REGALIA_ALLY_SRC });
        }
    }
}

/// `source_idx` just applied a status to `target_idx`.
pub fn on_status_applied(state: &mut GameState, source_idx: usize, target_idx: usize) {
    let Some(source) = state.grid.entities.get(source_idx) else { return };
    let Some((p, stars)) = of_wearer(source) else { return };
    for t in p.triggers {
        match *t {
            Trigger::StatusVuln { base, per_star, max_stacks, ticks } => {
                let src = REGALIA_VULN_SRC;
                let Some(tgt) = state.grid.entities.get_mut(target_idx) else { return };
                let step = at(base, per_star, stars);
                let cap = step.saturating_mul(max_stacks);
                if let Some(b) = tgt.buffs.iter_mut().find(|b| matches!(b.kind, BuffKind::DmgTakenPct) && b.source_id == src) {
                    b.magnitude = (b.magnitude + step).min(cap);
                    b.ticks = ticks;
                } else {
                    tgt.buffs.push(Buff { kind: BuffKind::DmgTakenPct, magnitude: step, ticks, source_id: src });
                }
            }
            // Lycoris Lullaby (0.0.451, was Crimson Mirror): the status feeds the WEARER — ATK +step per status,
            // `max_stacks` deep, on the event source so it stacks beside a skill's own ATK buff.
            Trigger::DebuffAppliedAtk { base, per_star, max_stacks, ticks } => {
                let Some(me) = state.grid.entities.get_mut(source_idx) else { return };
                let step = at(base, per_star, stars);
                let cap = step.saturating_mul(max_stacks);
                if let Some(b) = me.buffs.iter_mut().find(|b| matches!(b.kind, BuffKind::AtkPct) && b.source_id == REGALIA_EVENT_SRC) {
                    b.magnitude = (b.magnitude + step).min(cap);
                    b.ticks = ticks;
                } else {
                    me.buffs.push(Buff { kind: BuffKind::AtkPct, magnitude: step, ticks, source_id: REGALIA_EVENT_SRC });
                }
            }
            _ => {}
        }
    }
}

/// `caster_idx` just cast its ULTIMATE (called from `unit_rank::after_ult`, every ult path).
pub fn on_ult_cast(state: &mut GameState, caster_idx: usize) {
    let Some(caster) = state.grid.entities.get(caster_idx) else { return };
    let Some((p, stars)) = of_wearer(caster) else { return };
    let team = caster.team;
    let me = caster.id;
    for t in p.triggers {
        if let Trigger::UltPartyAtk { base, per_star, ticks } = *t {
            let m = at(base, per_star, stars);
            for e in state.grid.entities.iter_mut().filter(|e| e.alive && e.team == team && !e.is_pet) {
                apply_buff(e, Buff { kind: BuffKind::BellAtkPct, magnitude: m, ticks, source_id: me });
            }
        }
        if let Trigger::UltHealParty { base, per_star } = *t {
            let bp = i64::from(at(base, per_star, stars));
            let heals: Vec<(usize, i32)> = state
                .grid
                .entities
                .iter()
                .enumerate()
                .filter(|(_, e)| e.alive && e.team == team && !e.is_pet)
                .map(|(j, e)| (j, ((crate::combat::effective_max_hp(e) as i64) * bp / 10_000).max(1) as i32))
                .collect();
            for (j, heal) in heals {
                crate::tick::combat_step::apply_skill_heal_raw(state, caster_idx, j, heal);
            }
        }
    }
}

/// Conditional +damage (basis points) for a packet from `attacker` onto `target`, read at resolve
/// time: debuffed target, range, low HP. 0 for a wearer without such a trigger.
pub fn hit_bonus_bp(attacker: &Entity, target: &Entity) -> i64 {
    let Some((p, stars)) = of_wearer(attacker) else { return 0 };
    let mut bonus: i64 = 0;
    for t in p.triggers {
        match *t {
            Trigger::DebuffedTargetDmg { base, per_star } if crate::skills::distinct_debuff_kinds(target) > 0 => {
                bonus += i64::from(at(base, per_star, stars));
            }
            Trigger::MultiDebuffTargetDmg { min_kinds, base, per_star }
                if crate::skills::distinct_debuff_kinds(target) as i32 >= min_kinds =>
            {
                bonus += i64::from(at(base, per_star, stars));
            }
            Trigger::RangedDmg { base, per_star } if attacker.is_ranged => {
                bonus += i64::from(at(base, per_star, stars));
            }
            Trigger::LowHpTargetDmg { threshold_pct, base, per_star } => {
                let max = crate::combat::effective_max_hp(target).max(1) as i64;
                if (target.hp.max(0) as i64) * 100 < max * i64::from(threshold_pct) {
                    bonus += i64::from(at(base, per_star, stars));
                }
            }
            _ => {}
        }
    }
    bonus
}

/// Extra CRIT DMG (bp) the attacker's passive adds against `target` (Meridian: a debuffed
/// target). Folded into the crit multiplier in `combat::damage`; 0 without the piece.
pub fn crit_dmg_bonus_bp(attacker: &Entity, target: &Entity) -> i64 {
    let Some((p, stars)) = of_wearer(attacker) else { return 0 };
    p.triggers
        .iter()
        .map(|t| match *t {
            Trigger::DebuffedTargetCritDmg { base, per_star } if crate::skills::distinct_debuff_kinds(target) > 0 => {
                i64::from(at(base, per_star, stars))
            }
            _ => 0,
        })
        .sum()
}

/// Extra CRIT RATE (bp) the attacker's passive adds against `target` (Meridian: a debuffed target),
/// read where the crit is rolled in `combat::damage`. 0 without the piece.
pub fn crit_rate_bonus_bp(attacker: &Entity, target: &Entity) -> u32 {
    let Some((p, stars)) = of_wearer(attacker) else { return 0 };
    p.triggers
        .iter()
        .map(|t| match *t {
            Trigger::DebuffedTargetCritRate { base, per_star } if crate::skills::distinct_debuff_kinds(target) > 0 => {
                at(base, per_star, stars).max(0) as u32
            }
            _ => 0,
        })
        .sum()
}

/// Toughness damage bonus (bp) the attacker's passive adds (Striking Stone). Folded into the chip's
/// build-up in `combat::damage`, so it scales only the bar, never HP. 0 without the piece.
pub fn toughness_bonus_bp(attacker: &Entity) -> i32 {
    let Some((p, stars)) = of_wearer(attacker) else { return 0 };
    p.triggers
        .iter()
        .map(|t| match *t {
            Trigger::ToughnessDmg { base, per_star } => at(base, per_star, stars),
            _ => 0,
        })
        .sum()
}

/// One line of a passive for a client: a loc key (`REGALIA_PASSIVE_<kind>`) and the numbers it
/// formats — `{0}` the value at the carrier's stars, `{1}` the value at S5, both in whole percent
/// (or the plain count for `max_enemies`, carried in `{2}`). The client owns the
/// wording; the sim owns the numbers.
#[derive(Debug, Clone, serde::Serialize, PartialEq, Eq)]
pub struct PassiveLine {
    pub key: &'static str,
    pub args: Vec<i64>,
}

fn pct(bp: i32) -> i64 {
    i64::from(bp) / 100
}

/// The passive of `regalia_id` described at `stars`; empty for a piece without one.
pub fn describe(regalia_id: &str, stars: u8) -> Vec<PassiveLine> {
    let Some(p) = passive_of(regalia_id) else { return Vec::new() };
    // A carrier saved before 0.0.486 may hold more stars than S5 now has; it reads as S5.
    let stars = stars.min(crate::items::REGALIA_MAX_STARS);
    let mut out = Vec::new();
    for (m, step) in p.mods {
        let key = match m {
            StatMod::HealBp(_) => "REGALIA_PASSIVE_heal",
            StatMod::DefBp(_) => "REGALIA_PASSIVE_def",
            StatMod::AtkBp(_) => "REGALIA_PASSIVE_atk",
            StatMod::CritBp(_) => "REGALIA_PASSIVE_crit",
            _ => continue,
        };
        out.push(PassiveLine { key, args: vec![pct(at(m.value(), *step, stars)), pct(at(m.value(), *step, crate::items::REGALIA_MAX_STARS))] });
    }
    for t in p.triggers {
        let (key, base, per_star, extra) = match *t {
            Trigger::TauntGuard { base, per_star } => ("REGALIA_PASSIVE_taunt_guard", base, per_star, 0),
            Trigger::ShieldBreakAtk { base, per_star, ticks } => ("REGALIA_PASSIVE_shield_break_atk", base, per_star, i64::from(ticks) / 10),
            Trigger::AtkPerEnemy { base, per_star, max_enemies } => ("REGALIA_PASSIVE_atk_per_enemy", base, per_star, i64::from(max_enemies)),
            Trigger::OnBreakAtk { base, per_star, ticks } => ("REGALIA_PASSIVE_on_break_atk", base, per_star, i64::from(ticks) / 10),
            Trigger::HealedAllyAtk { base, per_star, ticks } => ("REGALIA_PASSIVE_healed_ally_atk", base, per_star, i64::from(ticks) / 10),
            Trigger::HealedAllyDr { base, per_star, ticks } => ("REGALIA_PASSIVE_healed_ally_dr", base, per_star, i64::from(ticks) / 10),
            Trigger::DebuffedTargetDmg { base, per_star } => ("REGALIA_PASSIVE_debuffed_target_dmg", base, per_star, 0),
            Trigger::RangedDmg { base, per_star } => ("REGALIA_PASSIVE_ranged_dmg", base, per_star, 0),
            Trigger::StatusVuln { base, per_star, max_stacks, .. } => ("REGALIA_PASSIVE_status_vuln", base, per_star, i64::from(max_stacks)),
            Trigger::LowHpTargetDmg { threshold_pct, base, per_star } => ("REGALIA_PASSIVE_low_hp_target_dmg", base, per_star, i64::from(threshold_pct)),
            // One stack is a timed refresh, not a ladder (Rouge Thread, 0.0.485): `{2}` = seconds.
            Trigger::DebuffAppliedAtk { base, per_star, max_stacks: 1, ticks } => ("REGALIA_PASSIVE_debuff_applied_atk_once", base, per_star, i64::from(ticks) / 10),
            Trigger::DebuffAppliedAtk { base, per_star, max_stacks, .. } => ("REGALIA_PASSIVE_debuff_applied_atk", base, per_star, i64::from(max_stacks)),
            Trigger::UltHealParty { base, per_star } => ("REGALIA_PASSIVE_ult_heal_party", base, per_star, 0),
            Trigger::TauntGuardSelf { base, per_star } => ("REGALIA_PASSIVE_taunt_guard_self", base, per_star, 0),
            Trigger::LowHpDef { threshold_pct, base, per_star } => ("REGALIA_PASSIVE_low_hp_def", base, per_star, i64::from(threshold_pct)),
            Trigger::UltPartyAtk { base, per_star, ticks } => ("REGALIA_PASSIVE_ult_party_atk", base, per_star, i64::from(ticks) / 10),
            Trigger::ToughnessDmg { base, per_star } => ("REGALIA_PASSIVE_toughness_dmg", base, per_star, 0),
            Trigger::OnBreakDmg { base, per_star, ticks } => ("REGALIA_PASSIVE_on_break_dmg", base, per_star, i64::from(ticks) / 10),
            Trigger::OnBreakCritDmg { base, per_star, ticks } => ("REGALIA_PASSIVE_on_break_crit_dmg", base, per_star, i64::from(ticks) / 10),
            Trigger::OnBreakCritRate { base, per_star, ticks } => ("REGALIA_PASSIVE_on_break_crit_rate", base, per_star, i64::from(ticks) / 10),
            Trigger::DebuffedTargetCritDmg { base, per_star } => ("REGALIA_PASSIVE_debuffed_target_crit_dmg", base, per_star, 0),
            Trigger::DebuffedTargetCritRate { base, per_star } => ("REGALIA_PASSIVE_debuffed_target_crit_rate", base, per_star, 0),
            Trigger::MultiDebuffTargetDmg { min_kinds, base, per_star } => ("REGALIA_PASSIVE_multi_debuff_target_dmg", base, per_star, i64::from(min_kinds)),
            Trigger::HealedAllyDmg { base, per_star, ticks } => ("REGALIA_PASSIVE_healed_ally_dmg", base, per_star, i64::from(ticks) / 10),
        };
        out.push(PassiveLine { key, args: vec![pct(at(base, per_star, stars)), pct(at(base, per_star, crate::items::REGALIA_MAX_STARS)), extra] });
    }
    out
}

#[cfg(test)]
mod tests {
    /// THE STAR RULE (user 2026-09-22, re-cut 2026-09-27 to S1..S5): every number on every passive
    /// doubles from S1 to S5 — `per_star × 4 == base`, no exceptions.
    #[test]
    fn every_passive_number_doubles_by_five_stars() {
        use super::Trigger;
        for id in ["aurora_edge", "empyrean", "litany_of_dawn", "meridian", "silent_verdict", "lycoris_lullaby", "hoarfrost_crown", "thunderhead", "ferrymans_lantern", "emberbell", "stormglass_pin", "tidecaller", "static_charm", "winters_oath"].into_iter().chain(crate::items::REGALIA_THREE_STAR) {
            let p = super::passive_of(id).unwrap_or_else(|| panic!("{id}"));
            for (m, step) in p.mods {
                assert_eq!(step * 4, m.value(), "{id}: flat mod {m:?} must double S1 → S5");
            }
            for t in p.triggers {
                let (base, per_star) = match *t {
                    Trigger::TauntGuard { base, per_star }
                    | Trigger::ShieldBreakAtk { base, per_star, .. }
                    | Trigger::AtkPerEnemy { base, per_star, .. }
                    | Trigger::OnBreakAtk { base, per_star, .. }
                    | Trigger::HealedAllyAtk { base, per_star, .. }
                    | Trigger::DebuffedTargetDmg { base, per_star }
                    | Trigger::RangedDmg { base, per_star, .. }
                    | Trigger::StatusVuln { base, per_star, .. }
                    | Trigger::LowHpTargetDmg { base, per_star, .. }
                    | Trigger::DebuffAppliedAtk { base, per_star, .. }
                    | Trigger::HealedAllyDr { base, per_star, .. }
                    | Trigger::UltHealParty { base, per_star }
                    | Trigger::TauntGuardSelf { base, per_star }
                    | Trigger::LowHpDef { base, per_star, .. }
                    | Trigger::UltPartyAtk { base, per_star, .. }
                    | Trigger::ToughnessDmg { base, per_star }
                    | Trigger::OnBreakDmg { base, per_star, .. }
                    | Trigger::OnBreakCritDmg { base, per_star, .. }
                    | Trigger::OnBreakCritRate { base, per_star, .. }
                    | Trigger::DebuffedTargetCritDmg { base, per_star }
                    | Trigger::DebuffedTargetCritRate { base, per_star }
                    | Trigger::MultiDebuffTargetDmg { base, per_star, .. }
                    | Trigger::HealedAllyDmg { base, per_star, .. } => (base, per_star),
                };
                assert_eq!(per_star * 4, base, "{id}: {t:?} must double S1 → S5");
            }
        }
    }

    #[test]
    fn every_signature_passive_describes_itself_and_the_others_say_nothing() {
        for id in ["aurora_edge", "empyrean", "litany_of_dawn", "meridian", "silent_verdict", "lycoris_lullaby", "hoarfrost_crown", "thunderhead", "ferrymans_lantern", "emberbell", "stormglass_pin", "tidecaller", "static_charm", "winters_oath"].into_iter().chain(crate::items::REGALIA_THREE_STAR) {
            let s0 = super::describe(id, 0);
            let s5 = super::describe(id, crate::items::REGALIA_MAX_STARS);
            assert!(!s0.is_empty(), "{id} has a passive to describe");
            for (a, b) in s0.iter().zip(&s5) {
                assert_eq!(a.key, b.key);
                assert!(b.args[0] >= a.args[0], "{id}: S5 is never weaker");
                assert_eq!(a.args[1], b.args[1], "{id}: the S5 column is the same whoever asks");
            }
        }
        assert!(super::describe("no_such_card", 3).is_empty(), "an unknown id has no passive");
        let hg = super::describe("hearthguard", 0);
        assert_eq!((hg[0].key, hg[0].args.clone()), ("REGALIA_PASSIVE_taunt_guard_self", vec![5, 10, 0]));
        let sera = super::describe("aurora_edge", 0);
        assert_eq!(sera[0].key, "REGALIA_PASSIVE_taunt_guard");
        assert_eq!(sera[0].args, vec![10, 20, 0]);
    }

    // ---- the 3★ set (0.0.485): the acceptance checks of the handoff ----

    use crate::grid::{Cell, Entity, Team};
    use crate::skills::BuffKind;
    use crate::state::GameState;

    fn wearer(id: u32, unit: &str, regalia: &str, stars: u8) -> Entity {
        let mut e = Entity::idle(id, Cell::new(0, 0)).with_team(Team::Player);
        e.unit_def = unit.into();
        e.hp = 1_000;
        e.max_hp = 1_000;
        e.regalia = Some(crate::items::Regalia { id: regalia.into(), stars, level: 1, ..Default::default() });
        e
    }

    fn mob(id: u32, x: i32) -> Entity {
        let mut m = Entity::idle(id, Cell::new(x, 0)).with_team(Team::Monster);
        m.hp = 1_000_000;
        m.max_hp = 1_000_000;
        m
    }

    fn buff(e: &Entity, kind: BuffKind, src: u32) -> Option<(i32, u32)> {
        e.buffs.iter().find(|b| b.kind == kind && b.source_id == src).map(|b| (b.magnitude, b.ticks))
    }

    #[test]
    fn three_star_primary_and_secondary_mods_climb_on_the_integer_curve() {
        use crate::items::{regalia_def_by_id, StatMod};
        let at = |id: &str, lv: u8| regalia_def_by_id(id).unwrap_or_else(|| panic!("{id}")).mods_at(lv, 0);
        for id in crate::items::REGALIA_THREE_STAR {
            assert!(at(id, 1).contains(&StatMod::AtkFlat(100)), "{id}: Primary Mod 100 raw at Lv1");
            assert!(at(id, 60).contains(&StatMod::AtkFlat(1_503)), "{id}: Primary Mod 1,503 raw at Lv60");
        }
        assert!(at("hearthguard", 1).contains(&StatMod::DefBp(360)) && at("hearthguard", 60).contains(&StatMod::DefBp(899)));
        assert!(at("rouge_thread", 1).contains(&StatMod::AtkBp(150)) && at("rouge_thread", 60).contains(&StatMod::AtkBp(374)));
        assert!(at("last_rivet", 60).contains(&StatMod::MaxHpBp(899)));
        assert!(at("striking_stone", 60).contains(&StatMod::CritBp(374)));
        assert!(at("emberwick", 60).contains(&StatMod::CooldownBp(187)), "CDR is half the small size (0.0.490)");
        assert_eq!(crate::items::regalia_doctrine("rouge_thread"), Some(crate::potential::Doctrine::Catalyst));
        assert_eq!(crate::items::regalia_doctrine("lycoris_lullaby"), Some(crate::potential::Doctrine::Striker));
    }

    #[test]
    fn hearthguard_guards_only_its_wearer_and_only_while_it_holds_a_taunt() {
        let mut s = GameState::new(1, 16, 8);
        s.grid.spawn(wearer(1, "seraphine", "hearthguard", 0));
        s.grid.spawn(wearer(2, "lucielle", "", 0));
        s.grid.spawn(mob(1000, 3));
        super::tick(&mut s);
        assert_eq!(buff(&s.grid.entities[0], BuffKind::DmgReducePct, super::REGALIA_SELF_SRC), None, "no taunt, no guard");
        s.grid.entities[0].taunt_hold = 1;
        super::tick(&mut s);
        assert_eq!(buff(&s.grid.entities[0], BuffKind::DmgReducePct, super::REGALIA_SELF_SRC).map(|b| b.0), Some(500));
        assert!(s.grid.entities[1].buffs.iter().all(|b| b.kind != BuffKind::DmgReducePct), "never party-wide");
        // Off-Doctrine: Primary / Secondary Mods still apply, the passive does not.
        let mut s = GameState::new(1, 16, 8);
        let mut n = wearer(1, "raplus", "hearthguard", 4);
        n.taunt_hold = 1;
        s.grid.spawn(n);
        super::tick(&mut s);
        assert!(s.grid.entities[0].buffs.is_empty());
    }

    #[test]
    fn last_rivet_hardens_strictly_below_half() {
        let mut s = GameState::new(1, 16, 8);
        s.grid.spawn(wearer(1, "seraphine", "last_rivet", 4));
        let m = crate::combat::effective_max_hp(&s.grid.entities[0]);
        s.grid.entities[0].hp = (m + 1) / 2; // at or just over half
        super::tick(&mut s);
        assert_eq!(buff(&s.grid.entities[0], BuffKind::DefPct, super::REGALIA_SELF_SRC), None, "half does not qualify");
        s.grid.entities[0].hp = (m - 1) / 2;
        super::tick(&mut s);
        assert_eq!(buff(&s.grid.entities[0], BuffKind::DefPct, super::REGALIA_SELF_SRC).map(|b| b.0), Some(2_400), "S5: +24%");
    }

    #[test]
    fn emberwick_counts_living_enemies_up_to_three() {
        for (n, want) in [(0, None), (1, Some(600)), (3, Some(1_800)), (4, Some(1_800))] {
            let mut s = GameState::new(1, 16, 8);
            s.grid.spawn(wearer(1, "lapis", "emberwick", 0));
            for k in 0..n {
                s.grid.spawn(mob(1000 + k, 3 + k as i32));
            }
            super::tick(&mut s);
            assert_eq!(buff(&s.grid.entities[0], BuffKind::AtkPct, super::REGALIA_SELF_SRC).map(|b| b.0), want, "{n} enemies");
        }
    }

    #[test]
    fn kestrel_pays_every_ranged_attack_point_blank_included() {
        let mut a = wearer(1, "aveline", "kestrel", 0);
        a.is_ranged = true;
        assert_eq!(super::hit_bonus_bp(&a, &mob(1000, 1)), 1_200, "an arrow at point-blank counts");
        assert_eq!(super::hit_bonus_bp(&a, &mob(1001, 6)), 1_200);
        a.is_ranged = false;
        assert_eq!(super::hit_bonus_bp(&a, &mob(1001, 6)), 0, "a melee attacker never does");
    }

    #[test]
    fn rouge_thread_refreshes_one_buff_and_never_stacks() {
        let mut s = GameState::new(1, 16, 8);
        s.grid.spawn(wearer(1, "raplus", "rouge_thread", 0));
        s.grid.spawn(mob(1000, 1));
        super::on_status_applied(&mut s, 0, 1);
        assert_eq!(buff(&s.grid.entities[0], BuffKind::AtkPct, super::REGALIA_EVENT_SRC), Some((1_600, 60)));
        s.grid.entities[0].buffs[0].ticks = 5;
        super::on_status_applied(&mut s, 0, 1);
        assert_eq!(buff(&s.grid.entities[0], BuffKind::AtkPct, super::REGALIA_EVENT_SRC), Some((1_600, 60)), "refreshed, not stacked");
        // A Striker wearing it gets the Mods, not the passive.
        let mut s = GameState::new(1, 16, 8);
        s.grid.spawn(wearer(1, "lapis", "rouge_thread", 0));
        s.grid.spawn(mob(1000, 1));
        super::on_status_applied(&mut s, 0, 1);
        assert!(s.grid.entities[0].buffs.is_empty());
    }

    #[test]
    fn wayfarers_bells_never_add_the_strongest_counts_and_each_keeps_its_expiry() {
        let mut s = GameState::new(1, 16, 8);
        s.grid.spawn(wearer(1, "lucielle", "wayfarers_bell", 0));
        s.grid.spawn(wearer(2, "sylvie", "wayfarers_bell", 4));
        s.grid.spawn(wearer(3, "lapis", "", 0));
        super::on_ult_cast(&mut s, 0);
        super::on_ult_cast(&mut s, 1);
        let ally = &s.grid.entities[2];
        assert_eq!(buff(ally, BuffKind::BellAtkPct, 1), Some((1_200, 60)));
        assert_eq!(buff(ally, BuffKind::BellAtkPct, 2), Some((2_400, 60)));
        assert_eq!(crate::skills::effective_bp(ally, BuffKind::BellAtkPct), 2_400, "strongest, not 3,600");
        assert_eq!(buff(&s.grid.entities[0], BuffKind::BellAtkPct, 1).map(|b| b.0), Some(1_200), "the wearer too");
        s.grid.entities[2].buffs.retain(|b| b.source_id != 2); // the stronger one lapses first
        assert_eq!(crate::skills::effective_bp(&s.grid.entities[2], BuffKind::BellAtkPct), 1_200);
    }

    #[test]
    fn striking_stone_adds_to_the_toughness_bar_and_never_to_hp_damage() {
        use crate::grid::{DamageType, Element};
        assert_eq!(super::toughness_bonus_bp(&wearer(1, "zolfia", "striking_stone", 0)), 1_600, "S1");
        assert_eq!(super::toughness_bonus_bp(&wearer(1, "zolfia", "striking_stone", 4)), 3_200, "S5 = 2 × S1");
        assert_eq!(super::toughness_bonus_bp(&wearer(1, "lapis", "striking_stone", 4)), 0, "off-Doctrine");
        // Same stats and Mods both sides; only the Doctrine (the passive) differs.
        let run = |unit: &str| {
            let mut a = wearer(1, unit, "striking_stone", 0);
            a.atk = 500;
            let mut t = mob(1000, 1);
            t.weaknesses = vec![Element::Fire]; // `Entity::idle` attacks Fire
            t.toughness_max = 10_000;
            t.toughness = 10_000;
            let mut es = vec![a, t];
            let mut rng = crate::rng::Pcg32::new(7);
            let mut hits = Vec::new();
            let dealt = crate::combat::resolve_attack_scaled(&mut es, &mut rng, 0, 1, 10_000, 1, &mut hits, DamageType::Physical, 0);
            (dealt, 10_000 - i32::from(es[1].toughness))
        };
        let (hp_on, bar_on) = run("zolfia");
        let (hp_off, bar_off) = run("no_doctrine");
        assert_eq!(hp_on, hp_off, "HP damage untouched");
        assert!(bar_on > bar_off && bar_off > 0, "the bar takes more: {bar_on} vs {bar_off}");
    }


    #[test]
    fn faultline_refreshes_on_the_wearers_break_and_never_stacks() {
        let mut z = wearer(1, "zolfia", "faultline", 0);
        super::on_break(&mut z);
        assert_eq!(buff(&z, BuffKind::AtkPct, super::REGALIA_EVENT_SRC), Some((2_400, 60)));
        z.buffs[0].ticks = 3;
        super::on_break(&mut z);
        assert_eq!(buff(&z, BuffKind::AtkPct, super::REGALIA_EVENT_SRC), Some((2_400, 60)), "refreshed, not 4,800");
        let mut r = wearer(1, "lapis", "faultline", 4);
        super::on_break(&mut r);
        assert!(r.buffs.is_empty(), "off-Doctrine");
    }


    // ---- the SSR two-zone pass (0.0.488) ----

    #[test]
    fn lycoris_lullaby_pays_on_two_distinct_statuses_not_one() {
        use crate::skills::Buff;
        let n = wearer(1, "nemu", "lycoris_lullaby", 0);
        let mut t = mob(1000, 2);
        t.buffs.push(Buff { kind: BuffKind::Bleed, magnitude: 1, ticks: 50, source_id: 9 });
        assert_eq!(super::hit_bonus_bp(&n, &t), 0, "one status");
        t.buffs.push(Buff { kind: BuffKind::Burn, magnitude: 1, ticks: 50, source_id: 9 });
        assert_eq!(super::hit_bonus_bp(&n, &t), 4_000, "two different statuses: +40%");
        assert_eq!(super::hit_bonus_bp(&wearer(1, "lapis", "lycoris_lullaby", 0), &t), 4_000,
            "a role condition: any Striker who lays statuses gets it, not only Nemu");
    }

    #[test]
    fn tidecaller_gives_the_healed_ally_damage_as_well_as_guard() {
        let mut s = GameState::new(1, 16, 8);
        s.grid.spawn(wearer(1, "astier", "tidecaller", 0));
        s.grid.spawn(wearer(2, "lapis", "", 0));
        super::on_heal(&mut s, 0, 1);
        assert_eq!(buff(&s.grid.entities[1], BuffKind::DmgDealtPct, super::REGALIA_ALLY_SRC), Some((2_000, 80)));
        assert_eq!(buff(&s.grid.entities[1], BuffKind::DmgReducePct, super::REGALIA_ALLY_SRC), Some((1_500, 80)));
    }

    #[test]
    fn ferrymans_lantern_breaks_into_crit_damage_not_more_atk() {
        let mut c = wearer(1, "charon", "ferrymans_lantern", 0);
        super::on_break(&mut c);
        assert_eq!(buff(&c, BuffKind::CritDmgBpAdd, super::REGALIA_EVENT_SRC), Some((3_600, 80)));
        assert_eq!(buff(&c, BuffKind::CritBpAdd, super::REGALIA_EVENT_SRC), Some((1_800, 80)), "and crits more often");
        assert_eq!(buff(&c, BuffKind::AtkPct, super::REGALIA_EVENT_SRC), None);
    }

}
