//! UNIT RESONANCE RANKS S1..S6 (0.0.441) — Genshin's constellations, one row per unit.
//!
//! A duplicate pull raises the unit's `stars` (0..=6). Each rank is a fixed effect: S1 a nudge to
//! the opener, S2 a small mechanic, S3 the two skills +3 levels, S4 a bigger mechanic, S5 the
//! ultimate +3 levels, S6 the signature. The level ranks live in `items::authored_skills` (the
//! same overlay ladder every star used to re-level through); everything else is a `Rank` here,
//! read at the site it changes through the small `*_bp` / `on_*` helpers below.
//!
//! Every helper is the identity for a unit below the rank or without a row, so a golden party
//! (no stars, class units) is byte-identical. No PRNG draw anywhere.

use crate::grid::{Entity, Team};
use crate::skills::{apply_buff, Buff, BuffKind};
use crate::state::GameState;

/// `source_id` of the self-buffs a rank stamps, distinct from every skill's and every Regalia's.
pub const RANK_SELF_SRC: u32 = u32::MAX - 9;
/// Lucielle S6's signature blessing on each ally — its own key, so it no longer replaces another
/// rank's party ATK stamp (Seraphine S4 Aegis) on the same ally (audit 2026-10-02).
pub const RANK_SIGNATURE_SRC: u32 = u32::MAX - 33;
/// Seraphine S4's Aegis ATK on each ally, likewise.
pub const RANK_AEGIS_SRC: u32 = u32::MAX - 34;

/// What a rank does. Numbers in basis points unless the field says otherwise.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Rank {
    /// A caster-centred blast (Iron Chains) takes `n` more targets.
    SelfAoeTargets { n: u32 },
    /// Every blow taken while the unit holds ≥1 enemy under taunt: DEF +`pct` (percent units),
    /// `max_stacks` deep, `ticks` long.
    DefStackWhenHitTaunting { pct: i32, max_stacks: i32, ticks: u32 },
    /// A shield placed by `skill` breaking on anyone refunds `gauge` to the placer.
    ShieldBreakRefund { skill: &'static str, gauge: u16 },
    /// `skill`'s damage ×`mult_bp`/10000.
    SkillPower { skill: &'static str, mult_bp: u32 },
    /// Aegis: burst ×`burst_mult_bp`, every enemy on the board taunted onto her for `taunt_ticks`,
    /// and each of its shields breaking refunds `refund_gauge`.
    AegisSignature { burst_mult_bp: u32, taunt_ticks: u32, refund_gauge: u16 },
    /// A blast of `skill` that catches ≥`hits` enemies shortens its own cooldown by `ticks`.
    AoeCooldownRefund { skill: &'static str, hits: usize, ticks: u32 },
    /// An enemy killed by this unit detonates for `atk_bp` of its ATK in a 3×3.
    KillExplosion { atk_bp: u32 },
    /// A skill heal also heals the next most-wounded ally for `share_bp` of the amount.
    HealSplash { share_bp: u32 },
    /// Communio reaches every ally and pays `atk_bp` (replaces the passive's own number).
    CommunioAll { atk_bp: u32 },
    /// An ally healed by a skill gains DEF +`pct` (percent units) for `ticks`.
    HealedAllyDef { pct: i32, ticks: u32 },
    /// Benedictio also pays `extra_atk_bp` on top and heals the party `heal_maxhp_bp` of max HP.
    BenedictioSignature { extra_atk_bp: i32, heal_maxhp_bp: u32, dmg_dealt_bp: i32 },
    /// Umbral Mark stacks `n` deeper.
    MarkStacks { n: u32 },
    /// The on-hit status `skill` applies runs at `duration_bp`/10000 of its duration and
    /// `magnitude_bp`/10000 of its magnitude (a DoT's magnitude is its total damage).
    StatusShape { skill: &'static str, duration_bp: u32, magnitude_bp: u32 },
    /// A volley takes `n` more targets.
    VolleyTargets { n: u32 },
    /// Umbral Exploit pays `bp` more per distinct debuff.
    ExploitBonus { bp: u32 },
    /// Enemies a caster-centred blast of `skill` catches take +`vuln_bp` damage for `ticks`.
    SelfAoeVuln { skill: &'static str, vuln_bp: i32, ticks: u32 },
    /// A caster-centred blast of `skill` that catches ≥`hits` shortens its cooldown by `ticks`.
    SelfAoeCooldownRefund { skill: &'static str, hits: usize, ticks: u32 },
    /// The party-shield cast `skill` also gives every member +`atk_bp` ATK for `ticks`.
    ShieldCastAtk { skill: &'static str, atk_bp: i32, ticks: u32 },
    /// A kill by this unit charges its gauge by `gauge`.
    KillGauge { gauge: u16 },
    /// Enemies the room nuke `skill` hits take +`vuln_bp` damage for `ticks`.
    NukeVuln { skill: &'static str, vuln_bp: i32, ticks: u32 },
    /// Umbral Mark lasts ×`mult_bp`/10000 longer.
    MarkDuration { mult_bp: u32 },
    /// Each Umbral Mark stack is worth `bp` more Shadow vulnerability.
    MarkStep { bp: i32 },
    /// Each Static Spark pays `bp` more of the layer's ATK (0.0.465, Mirei).
    SparkStep { bp: i32 },
    /// `skill`'s cast-buff magnitudes are `add` stronger (0.0.469, Astier S2 / S4).
    BuffPower { skill: &'static str, add: i32 },
    /// Unda Vitae's healed-ally buff also fires from her SKILL heals and her ultimate's tide,
    /// and stacks `stacks` deep (0.0.469, Astier S6).
    HealBuffEverywhere { stacks: i32 },
    /// A heal that would overflow leaves `share_bp` of the excess as a shield (0.0.468).
    OverhealShield { share_bp: u32, ticks: u32 },
    /// Tidebless' rolled element is worth `bp` more (0.0.468, Astier).
    ElementRoll { bp: i32 },
    /// Tidebless rolls `n` MORE elements (0.0.469, Astier S1).
    ElementPicks { n: u32 },
    /// Storm Field lays `n` more Static Sparks (0.0.466, Mirei S1).
    SparkStacks { n: u32 },
    /// A spark fired on a target under `pct`% HP does not spend its stack (0.0.466, Mirei S4).
    SparkKeepBelowPct { pct: i32 },
    /// `skill`'s SELF cast-buff reaches the whole party (0.0.466, Mirei S6 — Quicken).
    BuffParty { skill: &'static str },
    /// `skill`'s cast-buff lasts `ticks` longer (0.0.466, Suzu S4 — Break Fever).
    BuffDuration { skill: &'static str, ticks: u32 },
    /// While `skill`'s party buff lasts, every ally's Limit Break gauge fills `bp` faster
    /// (a `GaugeRatePct` rider on the same duration — it scales with the skill's uptime, so
    /// cooldown reduction keeps its value).
    CastGaugeRate { skill: &'static str, bp: i32 },
    /// A rolled-element blessing counts for EVERY ally, whatever element they deal.
    ElementRollAll,
    /// `skill` (a top-ATK-allies buff) reaches `plus` more allies (0.0.473, Sylvie S2).
    BuffTargets { skill: &'static str, plus: u8 },
    /// While `skill`'s party buff lasts, each crit an ally lands charges its gauge by `gauge`.
    CritGaugeRider { skill: &'static str, gauge: i32 },
    /// While `skill`'s party buff lasts, each crit an ally lands throws a Thunder spark of
    /// `atk_bp` of its ATK at the same target.
    CritSparkRider { skill: &'static str, atk_bp: i32 },
    /// Every enemy Shattered this run adds `bp` to `skill`'s damage, at most `cap` stacks
    /// (0 = no cap); a run is one instance, room 1 to room 3 (0.0.476, Zolfia S1).
    ShatterStack { skill: &'static str, bp: u32, cap: u16 },
    /// `skill` strikes everything within `radius` of the caster instead of one target (S2).
    SkillAoe { skill: &'static str, radius: i32 },
    /// When a Shattered enemy dies, its Shattered jumps to the nearest living enemy (S4).
    ShatterJump,
    /// A Shattered enemy inside the aura stays Shattered until it dies (S6).
    AuraHoldsShatter,
    /// While Break Fever is up, every Toughness break extends it by `ticks` (0.0.466, Suzu S6).
    FeverOnBreak { ticks: u32 },
    /// A volley arrow on a target whose mark is at its cap charges the gauge by `gauge`.
    GaugeOnCappedMark { gauge: u16 },
    /// After the ultimate `ult` resolves, the listed skills' cooldowns reset.
    CooldownResetAfterUlt { ult: &'static str, skills: &'static [&'static str] },
    /// The status `skill` applies also lands on every enemy within `radius` of the target.
    StatusSpread { skill: &'static str, radius: i32 },
    /// The DoT `skill` applies may stack `instances` deep from this unit (the generic rule is one).
    DotStacks { skill: &'static str, instances: usize },
    /// +`bp` DAMAGE on `skill` as an ADD-ON bucket: every such bonus on the skill ADDS (two
    /// +20%s are +40%, never ×1.44) and the bucket is applied once, apart from the skill's own
    /// power — the "+X% DMG" line, not a change to what the skill is (user).
    DmgAdd { skill: &'static str, bp: u32 },
    /// The named skills read `plus` levels higher than they are (over the cap, Genshin's +3 —
    /// `power_at` pays over-rank) — the S3 / S5 level ranks.
    SkillLevels { skills: &'static [&'static str], plus: u8 },
}

/// The six ranks of `unit_def`, S1 first — the RESONANCE LADDER. Empty = no ladder (an S3/S5 level rank alone still
/// lives in `authored_skills`).
pub fn ranks_of(unit_def: &str) -> &'static [&'static [Rank]] {
    // MECHANICS, not multipliers (user: "ปรับเพิ่ม effect ไม่ใช่การดัน damage ตรงๆ"): only S3 and S5
    // touch numbers, and only through skill levels.
    match unit_def {
        "seraphine" => &[
            &[Rank::SelfAoeTargets { n: 2 }, Rank::SelfAoeVuln { skill: "iron_chains", vuln_bp: 1_000, ticks: 60 }, Rank::DmgAdd { skill: "iron_chains", bp: 6_000 }],
            &[Rank::DefStackWhenHitTaunting { pct: 2, max_stacks: 10, ticks: 100 }, Rank::SelfAoeCooldownRefund { skill: "steel_break", hits: 3, ticks: 20 }],
            &[Rank::SkillLevels { skills: &["iron_chains", "steel_break"], plus: 3 }],
            &[Rank::ShieldCastAtk { skill: "aegis", atk_bp: 2_000, ticks: 300 }, Rank::DmgAdd { skill: "steel_break", bp: 6_000 }],
            &[Rank::SkillLevels { skills: &["aegis"], plus: 3 }],
            &[Rank::AegisSignature { burst_mult_bp: 20_000, taunt_ticks: 30, refund_gauge: 100 }, Rank::DmgAdd { skill: "steel_break", bp: 10_000 }, Rank::DmgAdd { skill: "iron_chains", bp: 4_000 }],
        ],
        "lapis" => &[
            &[Rank::AoeCooldownRefund { skill: "fireball", hits: 3, ticks: 20 }, Rank::DmgAdd { skill: "fireball", bp: 10_000 }],
            &[Rank::KillGauge { gauge: 30 }],
            &[Rank::SkillLevels { skills: &["fireball", "ember_cascade"], plus: 3 }],
            &[Rank::KillExplosion { atk_bp: 5_000 }, Rank::DmgAdd { skill: "fireball", bp: 10_000 }],
            &[Rank::SkillLevels { skills: &["fire_cat_meteor"], plus: 3 }],
            &[Rank::NukeVuln { skill: "fire_cat_meteor", vuln_bp: 2_000, ticks: 100 }, Rank::DmgAdd { skill: "fire_cat_meteor", bp: 20_000 }, Rank::DmgAdd { skill: "ember_cascade", bp: 10_000 }],
        ],
        // Lucielle is an SR since 0.0.469 (user) and her ladder was the flattest in the game
        // (+3% measured) — rewritten to the SR band (+40-50%) out of MECHANICS: the plague
        // spreads, Communio reaches everyone, her blessing is stronger and finally lands on
        // every ally as a damage buff rather than one more ATK number.
        "lucielle" => &[
            &[Rank::StatusSpread { skill: "lumen_contagio", radius: 1 }, Rank::HealSplash { share_bp: 5_000 }],
            &[Rank::CommunioAll { atk_bp: 1_500 }],
            &[Rank::SkillLevels { skills: &["lux_medica", "lumen_contagio"], plus: 3 }],
            &[Rank::BuffPower { skill: "benedictio_lucis", add: 800 }, Rank::HealedAllyDef { pct: 10, ticks: 80 }],
            &[Rank::SkillLevels { skills: &["benedictio_lucis"], plus: 3 }],
            &[Rank::BenedictioSignature { extra_atk_bp: 0, heal_maxhp_bp: 3_000, dmg_dealt_bp: 1_300 }],
        ],
        // SR ladder (0.0.452): the same six effects, the add-ons cut so S6 lands at ≈ +45%.
        "aveline" => &[
            &[Rank::MarkStacks { n: 1 }, Rank::MarkDuration { mult_bp: 20_000 }, Rank::DmgAdd { skill: "nightfall_volley", bp: 2_000 }],
            &[Rank::StatusShape { skill: "shadowbind", duration_bp: 5_000, magnitude_bp: 10_000 }, Rank::GaugeOnCappedMark { gauge: 6 }],
            &[Rank::SkillLevels { skills: &["nightfall_volley", "shadowbind"], plus: 3 }],
            &[Rank::VolleyTargets { n: 2 }],
            &[Rank::SkillLevels { skills: &["eclipse_rain"], plus: 3 }],
            &[Rank::MarkStep { bp: 500 }, Rank::DmgAdd { skill: "eclipse_rain", bp: 6_000 }, Rank::DmgAdd { skill: "nightfall_volley", bp: 1_500 }],
        ],
        "raplus" => &[
            // 0.0.464: Pickpocket deals nothing, so its +DMG moved to Rend; the dazzle lasts
            // longer and spreads.
            &[Rank::StatusShape { skill: "veil_slash", duration_bp: 15_000, magnitude_bp: 10_000 }, Rank::StatusSpread { skill: "veil_slash", radius: 1 }, Rank::DmgAdd { skill: "rend", bp: 8_000 }],
            &[Rank::DotStacks { skill: "rend", instances: 3 }],
            &[Rank::SkillLevels { skills: &["veil_slash", "rend"], plus: 3 }],
            &[Rank::ExploitBonus { bp: 5_000 }, Rank::DmgAdd { skill: "rend", bp: 8_000 }],
            &[Rank::SkillLevels { skills: &["umbral_requiem"], plus: 3 }],
            &[Rank::CooldownResetAfterUlt { ult: "umbral_requiem", skills: &["veil_slash", "rend"] }, Rank::DmgAdd { skill: "umbral_requiem", bp: 22_000 }, Rank::DmgAdd { skill: "rend", bp: 10_000 }],
        ],
        // Nemu (0.0.451): the poison runs deeper (S1), bites harder per stack (S2), the tether
        // doubles (S4), the hole leaves everything it held open (S6).
        "nemu" => &[
            &[Rank::MarkStacks { n: 5 }, Rank::DmgAdd { skill: "velvet_bog", bp: 18_000 }],
            &[Rank::MarkStep { bp: 400 }, Rank::DmgAdd { skill: "heartstring", bp: 6_000 }],
            &[Rank::SkillLevels { skills: &["velvet_bog", "heartstring"], plus: 3 }],
            &[Rank::DotStacks { skill: "heartstring", instances: 2 }, Rank::DmgAdd { skill: "heartstring", bp: 15_000 }],
            &[Rank::SkillLevels { skills: &["abyssal_embrace"], plus: 3 }],
            &[Rank::NukeVuln { skill: "abyssal_embrace", vuln_bp: 2_000, ticks: 100 }, Rank::DmgAdd { skill: "abyssal_embrace", bp: 35_000 }, Rank::DmgAdd { skill: "velvet_bog", bp: 22_000 }],
        ],
        // Lumi (0.0.456, SR ladder ≈ +45%): the chill runs deeper (S1), the flurry refunds on a
        // crowd (S2), opens what it stuns (S4), the whiteout leaves the room open (S6).
        "lumi" => &[
            &[Rank::MarkStacks { n: 5 }, Rank::DmgAdd { skill: "frost_taunt", bp: 2_500 }],
            &[Rank::SelfAoeCooldownRefund { skill: "frost_taunt", hits: 3, ticks: 20 }],
            &[Rank::SkillLevels { skills: &["frost_taunt", "ice_mend"], plus: 3 }],
            &[Rank::SelfAoeVuln { skill: "frost_taunt", vuln_bp: 500, ticks: 60 }, Rank::DmgAdd { skill: "frost_taunt", bp: 2_000 }],
            &[Rank::SkillLevels { skills: &["frost_aura"], plus: 3 }],
            &[Rank::MarkStep { bp: 50 }, Rank::DmgAdd { skill: "frost_taunt", bp: 3_000 }],
        ],
        // Airi (0.0.462, SR ladder ≈ +45%): the chain reaches further (S1), a kill charges the
        // storm (S2), the storm and the chain hit harder (S4, S6), Static runs deeper (S6).
        "airi" => &[
            &[Rank::VolleyTargets { n: 1 }, Rank::DmgAdd { skill: "chain_bolt", bp: 2_000 }],
            &[Rank::KillGauge { gauge: 25 }],
            &[Rank::SkillLevels { skills: &["chain_bolt", "overload"], plus: 3 }],
            &[Rank::DmgAdd { skill: "storm_volley", bp: 2_000 }, Rank::DmgAdd { skill: "chain_bolt", bp: 1_500 }],
            &[Rank::SkillLevels { skills: &["storm_volley"], plus: 3 }],
            &[Rank::MarkStacks { n: 2 }, Rank::DmgAdd { skill: "storm_volley", bp: 2_000 }, Rank::DmgAdd { skill: "chain_bolt", bp: 1_000 }],
        ],
        // Charon (0.0.463, SSR ladder +100-150%): the blow widens (S1), a break charges more
        // (S2), the broken pay more (S4), the passage lands harder (S6).
        "charon" => &[
            &[Rank::DmgAdd { skill: "tolling_blow", bp: 8_000 }],
            &[Rank::KillGauge { gauge: 30 }],
            &[Rank::SkillLevels { skills: &["tolling_blow", "blackwater_mark"], plus: 3 }],
            &[Rank::DmgAdd { skill: "tolling_blow", bp: 10_000 }, Rank::DmgAdd { skill: "blackwater_mark", bp: 12_000 }],
            &[Rank::SkillLevels { skills: &["passage_of_no_return"], plus: 3 }],
            &[Rank::NukeVuln { skill: "passage_of_no_return", vuln_bp: 2_000, ticks: 100 }, Rank::DmgAdd { skill: "passage_of_no_return", bp: 55_000 }, Rank::DmgAdd { skill: "tolling_blow", bp: 20_000 }],
        ],
        // Suzu (0.0.465, SR ladder ≈ +45%): the sweep widens (S1), the bell hits harder (S4),
        // the fever runs longer and the sweep harder (S6).
        // Suzu (0.0.466, user: "+dmg ตรงๆ มันสิ้นคิด ควรสร้างผลพิเศษ"): the sweep pays for itself,
        // the sundered take more from EVERYONE, and the fever runs long and refreshes on a break.
        "suzu" => &[
            &[Rank::SelfAoeCooldownRefund { skill: "sunder", hits: 3, ticks: 20 }],
            &[Rank::SelfAoeVuln { skill: "sunder", vuln_bp: 1_500, ticks: 60 }, Rank::KillGauge { gauge: 25 }],
            &[Rank::SkillLevels { skills: &["sunder", "bell_strike"], plus: 3 }],
            &[Rank::BuffDuration { skill: "break_fever", ticks: 50 }],
            &[Rank::SkillLevels { skills: &["break_fever"], plus: 3 }],
            &[Rank::FeverOnBreak { ticks: 20 }, Rank::DmgAdd { skill: "sunder", bp: 3_000 }],
        ],
        // Mirei (0.0.465, SR ladder ≈ +45%): the field bites (S1), the sparks feed the gauge
        // (S2), the fall lands harder (S4, S6).
        // Mirei (0.0.466): more sparks, sparks that outlive a finishing blow, and a Quicken the
        // whole party rides (user: "S6 เป็น skill 2 + aspd ให้ทุกคน 30%").
        "mirei" => &[
            &[Rank::SparkStacks { n: 3 }],
            &[Rank::SparkKeepBelowPct { pct: 50 }],
            &[Rank::SkillLevels { skills: &["storm_field", "quicken"], plus: 3 }],
            &[Rank::SparkStep { bp: 4_000 }],
            &[Rank::SkillLevels { skills: &["thunderfall"], plus: 3 }],
            &[Rank::BuffParty { skill: "quicken" }, Rank::KillGauge { gauge: 25 }],
        ],
        // Astier (0.0.468): the stream reaches further, an overheal becomes a shield, and the
        // blessing's rolled element is worth more.
        // Astier (0.0.469, user: each star should land): the blessing covers two elements, the
        // tide charges three times as fast, the blessing hits harder, and at S6 every heal of
        // hers — the stream and the tide included — carries the damage buff, five stacks deep.
        "astier" => &[
            &[Rank::ElementPicks { n: 1 }, Rank::BuffDuration { skill: "tidebless", ticks: 10 }],
            &[Rank::BuffPower { skill: "fons_vitae", add: 10_000 }, Rank::CooldownResetAfterUlt { ult: "fons_vitae", skills: &["tidebless"] }, Rank::CastGaugeRate { skill: "tidebless", bp: 5_000 }],
            &[Rank::SkillLevels { skills: &["rivus", "tidebless"], plus: 3 }],
            &[Rank::BuffPower { skill: "tidebless", add: 1_000 }],
            &[Rank::SkillLevels { skills: &["fons_vitae"], plus: 3 }],
            &[Rank::HealBuffEverywhere { stacks: 5 }, Rank::ElementRollAll],
        ],
        // Sylvie (0.0.473): every crit under Surge charges the gauge (S1) and throws a spark (S4);
        // Arc Focus reaches a second carry (S2) and then the whole party (S6).
        // Zolfia (0.0.476): every shatter of the run sharpens the split (S1), the split sweeps (S2), a shatter outlives its
        // body (S4), and at S6 a shatter inside the winter lasts until the enemy dies.
        "zolfia" => &[
            &[Rank::ShatterStack { skill: "coldflame_split", bp: 1_000, cap: 0 }],
            &[Rank::SkillAoe { skill: "coldflame_split", radius: 1 }],
            &[Rank::SkillLevels { skills: &["coldflame_split", "remorseless_winter"], plus: 3 }],
            &[Rank::ShatterJump],
            &[Rank::SkillLevels { skills: &["winters_verdict"], plus: 3 }],
            &[Rank::AuraHoldsShatter],
        ],
        "sylvie" => &[
            &[Rank::CritGaugeRider { skill: "surge", gauge: 5 }],
            &[Rank::BuffTargets { skill: "arc_focus", plus: 1 }],
            &[Rank::SkillLevels { skills: &["surge", "arc_focus"], plus: 3 }],
            &[Rank::CritSparkRider { skill: "surge", atk_bp: 3_000 }],
            &[Rank::SkillLevels { skills: &["bolt_from_the_blue"], plus: 3 }],
            &[Rank::BuffParty { skill: "arc_focus" }],
        ],
        _ => &[],
    }
}

/// The ranks `e` has reached, in order — its Exemplar's stars against its own unit's ladder. A class
/// unit, a benched overlay of another unit, or S0 → empty.
pub fn active(e: &Entity) -> impl Iterator<Item = &'static Rank> {
    let stars = e.exemplar.as_ref().filter(|x| x.id == e.unit_def).map_or(0, |x| x.stars) as usize;
    let all = ranks_of(&e.unit_def);
    all.iter().take(stars.min(all.len())).flat_map(|r| r.iter())
}

/// Basic-attack multiplier from the ranks — none of today's webs touches the basic (user: ranks
/// are effects, not multipliers); the fold stays so the damage path has one place to ask.
pub fn basic_power_mult_bp(_e: &Entity) -> u32 {
    10_000
}

/// `skill`'s damage multiplier from every reached rank (10000 = none).
pub fn skill_power_mult_bp(e: &Entity, skill: &str) -> u32 {
    active(e).fold(10_000u64, |m, r| match r {
        Rank::SkillPower { skill: s, mult_bp } if *s == skill => m * u64::from(*mult_bp) / 10_000,
        _ => m,
    }) as u32
}

/// Levels the ranks add to `skill` (read-time, over the cap).
pub fn level_bonus(e: &Entity, skill: &str) -> u8 {
    active(e).map(|r| match r {
        Rank::SkillLevels { skills, plus } if skills.contains(&skill) => *plus,
        _ => 0,
    }).sum()
}

/// +DMG add-on bucket for `skill` (basis points, additive across ranks).
pub fn dmg_add_bp(e: &Entity, skill: &str) -> u32 {
    active(e).map(|r| match r { Rank::DmgAdd { skill: s, bp } if *s == skill => *bp, _ => 0 }).sum()
}

/// `def`'s power at `level` for this wearer: the rank levels on top, the (legacy) multiplier,
/// then the +DMG add-on bucket applied once.
pub fn power(e: &Entity, def: &crate::skills::SkillDef, level: u8) -> u32 {
    let lv = level.saturating_add(level_bonus(e, def.id));
    let base = u64::from(def.power_at(lv)) * u64::from(skill_power_mult_bp(e, def.id)) / 10_000;
    let stacks = match shatter_stack(e) {
        Some((skill, bp, _)) if skill == def.id => u64::from(bp) * u64::from(e.shatter_stacks),
        _ => 0,
    };
    (base * (10_000 + u64::from(dmg_add_bp(e, def.id)) + stacks) / 10_000).min(u32::MAX as u64) as u32
}

pub fn self_aoe_extra_targets(e: &Entity) -> u32 {
    active(e).map(|r| if let Rank::SelfAoeTargets { n } = r { *n } else { 0 }).sum()
}

pub fn volley_extra_targets(e: &Entity) -> u32 {
    active(e).map(|r| if let Rank::VolleyTargets { n } = r { *n } else { 0 }).sum()
}

/// Extra Static Spark stacks the ranks lay (0.0.466).
/// Extra bp the ranks add to Tidebless' rolled element (0.0.468).
/// The overheal-to-shield rank (0.0.468): `(share bp, ticks)`.
/// What the ranks ADD to `skill`'s cast-buff magnitudes (0.0.469).
pub fn buff_power_add(e: &Entity, skill: &str) -> i32 {
    active(e).map(|r| if let Rank::BuffPower { skill: s, add } = r { if *s == skill { *add } else { 0 } } else { 0 }).sum()
}

/// S6 (0.0.469): the healed-ally buff fires from every heal of hers, this many stacks deep.
pub fn heal_buff_everywhere(e: &Entity) -> Option<i32> {
    active(e).find_map(|r| if let Rank::HealBuffEverywhere { stacks } = r { Some(*stacks) } else { None })
}

pub fn overheal_shield(e: &Entity) -> Option<(u32, u32)> {
    active(e).find_map(|r| if let Rank::OverhealShield { share_bp, ticks } = r { Some((*share_bp, *ticks)) } else { None })
}

/// Extra elements Tidebless rolls (0.0.469).
pub fn element_extra_picks(e: &Entity) -> u32 {
    active(e).map(|r| if let Rank::ElementPicks { n } = r { *n } else { 0 }).sum()
}

/// Extra allies the ranks add to a top-ATK-allies buff (0.0.473).
pub fn buff_extra_targets(e: &Entity, skill: &str) -> u8 {
    active(e).map(|r| if let Rank::BuffTargets { skill: s, plus } = r { if *s == skill { *plus } else { 0 } } else { 0 }).sum()
}

/// The crit riders the ranks put on `skill`'s party buff: (gauge per crit, spark ATK bp).
pub fn crit_riders(e: &Entity, skill: &str) -> (i32, i32) {
    active(e).fold((0, 0), |(g, s), r| match r {
        Rank::CritGaugeRider { skill: k, gauge } if *k == skill => (g + gauge, s),
        Rank::CritSparkRider { skill: k, atk_bp } if *k == skill => (g, s + atk_bp),
        _ => (g, s),
    })
}

/// Zolfia's S1: `(bp per stack, cap)` for `skill`, if she has it.
pub fn shatter_stack(e: &Entity) -> Option<(&'static str, u32, u16)> {
    active(e).find_map(|r| if let Rank::ShatterStack { skill, bp, cap } = r { Some((*skill, *bp, *cap)) } else { None })
}

/// The caster-centred radius the ranks give `skill` (0 = none) (0.0.476).
pub fn skill_aoe(e: &Entity, skill: &str) -> i32 {
    active(e).map(|r| if let Rank::SkillAoe { skill: s, radius } = r { if *s == skill { *radius } else { 0 } } else { 0 }).max().unwrap_or(0)
}

pub fn shatter_jump(e: &Entity) -> bool {
    active(e).any(|r| matches!(r, Rank::ShatterJump))
}

pub fn aura_holds_shatter(e: &Entity) -> bool {
    active(e).any(|r| matches!(r, Rank::AuraHoldsShatter))
}

pub fn element_roll_all(e: &Entity) -> bool {
    active(e).any(|r| matches!(r, Rank::ElementRollAll))
}

pub fn element_roll_extra_bp(e: &Entity) -> i32 {
    active(e).map(|r| if let Rank::ElementRoll { bp } = r { *bp } else { 0 }).sum()
}

pub fn spark_extra_stacks(e: &Entity) -> u32 {
    active(e).map(|r| if let Rank::SparkStacks { n } = r { *n } else { 0 }).sum()
}

/// The HP% under which a spark of `e`'s survives being fired by someone else (0 = never).
pub fn spark_keep_below_pct(e: &Entity) -> i32 {
    active(e).map(|r| if let Rank::SparkKeepBelowPct { pct } = r { *pct } else { 0 }).max().unwrap_or(0)
}

/// `skill`'s self cast-buff reaches the whole party at this rank (0.0.466).
pub fn buff_reaches_party(e: &Entity, skill: &str) -> bool {
    active(e).any(|r| matches!(r, Rank::BuffParty { skill: s } if *s == skill))
}

/// The gauge-rate rider the ranks put on `skill`'s party cast-buff, in bp (0 = none).
pub fn cast_gauge_rate(e: &Entity, skill: &str) -> i32 {
    active(e).map(|r| if let Rank::CastGaugeRate { skill: s, bp } = r { if *s == skill { *bp } else { 0 } } else { 0 }).sum()
}

/// Extra ticks the ranks add to `skill`'s cast-buff (0.0.466).
pub fn buff_extra_ticks(e: &Entity, skill: &str) -> u32 {
    active(e).map(|r| if let Rank::BuffDuration { skill: s, ticks } = r { if *s == skill { *ticks } else { 0 } } else { 0 }).sum()
}

/// Ticks a break adds to a live Break Fever (0.0.466).
pub fn fever_on_break_ticks(e: &Entity) -> u32 {
    active(e).map(|r| if let Rank::FeverOnBreak { ticks } = r { *ticks } else { 0 }).max().unwrap_or(0)
}

pub fn spark_extra_bp(e: &Entity) -> i32 {
    active(e).map(|r| if let Rank::SparkStep { bp } = r { *bp } else { 0 }).sum()
}

pub fn mark_extra_step_bp(e: &Entity) -> i32 {
    active(e).map(|r| if let Rank::MarkStep { bp } = r { *bp } else { 0 }).sum()
}

pub fn mark_extra_stacks(e: &Entity) -> i32 {
    active(e).map(|r| if let Rank::MarkStacks { n } = r { *n as i32 } else { 0 }).sum()
}

pub fn exploit_bonus_bp(e: &Entity) -> u32 {
    active(e).map(|r| if let Rank::ExploitBonus { bp } = r { *bp } else { 0 }).sum()
}

/// `(duration_bp, magnitude_bp)` for the status `skill` applies (10000 / 10000 = as authored).
pub fn status_shape_bp(e: &Entity, skill: &str) -> (u32, u32) {
    active(e).fold((10_000u32, 10_000u32), |(d, m), r| match r {
        Rank::StatusShape { skill: s, duration_bp, magnitude_bp } if *s == skill => {
            ((u64::from(d) * u64::from(*duration_bp) / 10_000) as u32, (u64::from(m) * u64::from(*magnitude_bp) / 10_000) as u32)
        }
        _ => (d, m),
    })
}

/// Communio's reach and rate when a rank rewrites it: `Some(atk_bp)` = every ally at that rate.
pub fn communio_all(e: &Entity) -> Option<u32> {
    active(e).find_map(|r| if let Rank::CommunioAll { atk_bp } = r { Some(*atk_bp) } else { None })
}

/// Aegis' burst multiplier and room taunt, if the signature rank is reached.
pub fn aegis_signature(e: &Entity) -> Option<(u32, u32)> {
    active(e).find_map(|r| if let Rank::AegisSignature { burst_mult_bp, taunt_ticks, .. } = r { Some((*burst_mult_bp, *taunt_ticks)) } else { None })
}

pub fn benedictio_signature(e: &Entity) -> Option<(i32, u32, i32)> {
    active(e).find_map(|r| if let Rank::BenedictioSignature { extra_atk_bp, heal_maxhp_bp, dmg_dealt_bp } = r { Some((*extra_atk_bp, *heal_maxhp_bp, *dmg_dealt_bp)) } else { None })
}

/// The unit took a blow (`through` > 0) — the taunting-tank DEF stack.
pub fn on_hit_taken(e: &mut Entity) {
    if e.taunt_hold == 0 {
        return;
    }
    let stack = active(e).find_map(|r| if let Rank::DefStackWhenHitTaunting { pct, max_stacks, ticks } = r { Some((*pct, *max_stacks, *ticks)) } else { None });
    let Some((pct, max_stacks, ticks)) = stack else { return };
    // `pct` is authored in whole percent; `DefPct` magnitudes are basis points (0.0.457).
    let step = pct.saturating_mul(100);
    if let Some(b) = e.buffs.iter_mut().find(|b| matches!(b.kind, BuffKind::DefPct) && b.source_id == RANK_SELF_SRC) {
        b.magnitude = (b.magnitude + step).min(step.saturating_mul(max_stacks));
        b.ticks = ticks;
    } else {
        e.buffs.push(Buff { kind: BuffKind::DefPct, magnitude: step, ticks, source_id: RANK_SELF_SRC });
    }
}

/// A shield placed by `skill` (source entity `src_id`) just broke: refund the placer's gauge.
pub fn on_shield_break(state: &mut GameState, src_id: u32, skill: &str) {
    let Some(placer) = state.grid.entities.iter_mut().find(|e| e.id == src_id) else { return };
    let gauge: u16 = active(placer).map(|r| match r {
        Rank::ShieldBreakRefund { skill: s, gauge } if *s == skill => *gauge,
        Rank::AegisSignature { refund_gauge, .. } if skill == "aegis" => *refund_gauge,
        _ => 0,
    }).sum();
    if gauge > 0 {
        crate::skills::charge_limit_break(placer, gauge);
    }
}

/// A blast of `skill` from `caster_idx` (slot `slot`) caught `hits` enemies.
pub fn on_aoe_hits(state: &mut GameState, caster_idx: usize, slot: usize, skill: &str, hits: usize) {
    let Some(c) = state.grid.entities.get_mut(caster_idx) else { return };
    let refund: u32 = active(c).map(|r| match r {
        Rank::AoeCooldownRefund { skill: s, hits: h, ticks } if *s == skill && hits >= *h => *ticks,
        _ => 0,
    }).sum();
    if refund > 0 {
        if let Some(s) = c.skills.get_mut(slot) {
            s.cooldown = s.cooldown.saturating_sub(refund);
        }
    }
}

/// `killer_idx` just killed the entity at `dead_idx`: the kill explosion, if ranked. Flat packets
/// (no crit, no dodge → no draw) on every enemy in the 3×3 around the corpse.
pub fn on_kill(state: &mut GameState, killer_idx: usize, dead_idx: usize) {
    // A kill feeds the gauge (Rapis S2).
    if let Some(k) = state.grid.entities.get_mut(killer_idx) {
        let g = kill_gauge(k);
        if g > 0 {
            crate::skills::charge_limit_break(k, g);
        }
    }
    let (atk_bp, killer_id, kteam) = {
        let Some(k) = state.grid.entities.get(killer_idx) else { return };
        let bp: u32 = active(k).map(|r| if let Rank::KillExplosion { atk_bp } = r { *atk_bp } else { 0 }).sum();
        if bp == 0 || !matches!(k.team, Team::Player) {
            return;
        }
        (bp, k.id, k.team)
    };
    let Some(dead) = state.grid.entities.get(dead_idx) else { return };
    let center = dead.pos;
    let amount = {
        let Some(k) = state.grid.entities.get(killer_idx) else { return };
        ((crate::combat::effective_atk(k) as i64) * i64::from(atk_bp) / 10_000).max(1) as i32
    };
    let victims: Vec<usize> = state
        .grid
        .entities
        .iter()
        .enumerate()
        .filter(|(j, e)| *j != dead_idx && e.alive && e.team != kteam && e.on_board() && e.pos.chebyshev(center) <= 1)
        .map(|(j, _)| j)
        .collect();
    for j in victims {
        let dmg = crate::combat::Damage {
            atk_level: 1, amount,
            element: crate::grid::Element::Fire,
            kind: crate::grid::DamageType::Magic,
            is_ranged: true, is_crit: false, is_aoe: true,
            mastery_bp: 0, shadow_bane_bp: 0, debuff_amp_bp: 0,
            atk_race: crate::grid::Race::Unknown, atk_size: crate::grid::Size::Medium, ignore_def_bp: 0,
        };
        let Some(t) = state.grid.entities.get(j) else { continue };
        let mob = crate::combat::attackers_on(&state.grid.entities, j);
        let dealt = crate::combat::apply_damage(t, &dmg, mob);
        let pre_hp = t.hp.max(0);
        let tid = t.id;
        if let Some(t) = state.grid.entities.get_mut(j) {
            t.take_hit(dealt);
        }
        state.record_damage(killer_idx, Some("kill_explosion"), dealt.min(pre_hp));
        state.field_hits.push(crate::state::HitEvent { target_id: tid, attacker_id: killer_id, amount: dealt, hits: 1, kind: crate::state::HitKind::Normal });
        if state.grid.entities.get(j).is_some_and(|t| !t.alive) {
            crate::tick::combat_step::kill_monster(state, j, Some(killer_idx));
        }
    }
}

/// `healer_idx` healed `target_idx` for `amount` with a skill: the splash and the DEF gift.
pub fn on_skill_heal(state: &mut GameState, healer_idx: usize, target_idx: usize, amount: i32) {
    let (splash_bp, def) = {
        let Some(h) = state.grid.entities.get(healer_idx) else { return };
        let splash: u32 = active(h).map(|r| if let Rank::HealSplash { share_bp } = r { *share_bp } else { 0 }).sum();
        let def = active(h).find_map(|r| if let Rank::HealedAllyDef { pct, ticks } = r { Some((*pct, *ticks)) } else { None });
        (splash, def)
    };
    let healer_id = state.grid.entities.get(healer_idx).map_or(0, |h| h.id);
    if let Some((pct, ticks)) = def {
        if let Some(t) = state.grid.entities.get_mut(target_idx) {
            apply_buff(t, Buff { kind: BuffKind::DefPct, magnitude: pct.saturating_mul(100), ticks, source_id: healer_id });
        }
    }
    if splash_bp > 0 && amount > 0 {
        let team = state.grid.entities.get(healer_idx).map(|h| h.team);
        // The next most-wounded living ally that is not the patient: lowest HP share, spawn order.
        let next = state
            .grid
            .entities
            .iter()
            .enumerate()
            .filter(|(j, e)| *j != target_idx && e.alive && !e.is_pet && Some(e.team) == team && e.on_board())
            .map(|(j, e)| ((e.hp.max(0) as i64) * 10_000 / (crate::combat::effective_max_hp(e).max(1) as i64), j))
            .min();
        if let Some((share, j)) = next {
            if share < 10_000 {
                let heal = ((amount as i64) * i64::from(splash_bp) / 10_000).max(1) as i32;
                crate::tick::combat_step::apply_skill_heal_raw(state, healer_idx, j, heal);
            }
        }
    }
}


pub fn self_aoe_vuln(e: &Entity, skill: &str) -> Option<(i32, u32)> {
    active(e).find_map(|r| match r { Rank::SelfAoeVuln { skill: s, vuln_bp, ticks } if *s == skill => Some((*vuln_bp, *ticks)), _ => None })
}

pub fn self_aoe_refund(e: &Entity, skill: &str, hits: usize) -> u32 {
    active(e).map(|r| match r { Rank::SelfAoeCooldownRefund { skill: s, hits: h, ticks } if *s == skill && hits >= *h => *ticks, _ => 0 }).sum()
}

pub fn shield_cast_atk(e: &Entity, skill: &str) -> Option<(i32, u32)> {
    active(e).find_map(|r| match r { Rank::ShieldCastAtk { skill: s, atk_bp, ticks } if *s == skill => Some((*atk_bp, *ticks)), _ => None })
}

pub fn kill_gauge(e: &Entity) -> u16 {
    active(e).map(|r| if let Rank::KillGauge { gauge } = r { *gauge } else { 0 }).sum()
}

pub fn nuke_vuln(e: &Entity, skill: &str) -> Option<(i32, u32)> {
    active(e).find_map(|r| match r { Rank::NukeVuln { skill: s, vuln_bp, ticks } if *s == skill => Some((*vuln_bp, *ticks)), _ => None })
}

pub fn mark_duration_mult_bp(e: &Entity) -> u32 {
    active(e).fold(10_000u64, |m, r| match r { Rank::MarkDuration { mult_bp } => m * u64::from(*mult_bp) / 10_000, _ => m }) as u32
}

pub fn gauge_on_capped_mark(e: &Entity) -> u16 {
    active(e).map(|r| if let Rank::GaugeOnCappedMark { gauge } = r { *gauge } else { 0 }).sum()
}

/// Skills whose cooldown resets after `ult` fires.
pub fn cooldown_resets_after(e: &Entity, ult: &str) -> &'static [&'static str] {
    active(e).find_map(|r| match r { Rank::CooldownResetAfterUlt { ult: u, skills } if *u == ult => Some(*skills), _ => None }).unwrap_or(&[])
}

pub fn status_spread_radius(e: &Entity, skill: &str) -> i32 {
    active(e).map(|r| match r { Rank::StatusSpread { skill: s, radius } if *s == skill => *radius, _ => 0 }).max().unwrap_or(0)
}

/// How many instances of `skill`'s DoT this unit may keep on one target (1 = the generic rule).
pub fn dot_stacks(e: &Entity, skill: &str) -> usize {
    active(e).map(|r| match r { Rank::DotStacks { skill: s, instances } if *s == skill => *instances, _ => 1 }).max().unwrap_or(1)
}

/// `ult` just resolved for `caster_idx`: reset the ranked cooldowns.
pub fn after_ult(state: &mut GameState, caster_idx: usize, ult: &str) {
    if let Some(c) = state.grid.entities.get(caster_idx) {
        state.ult_fx.push(crate::state::SkillFx { caster_id: c.id, target_id: c.id, skill_id: ult.to_string() });
    }
    crate::regalia_passive::on_ult_cast(state, caster_idx);
    let Some(c) = state.grid.entities.get_mut(caster_idx) else { return };
    let resets = cooldown_resets_after(c, ult);
    if resets.is_empty() {
        return;
    }
    for s in c.skills.iter_mut() {
        if resets.contains(&s.id.as_str()) {
            s.cooldown = 0;
        }
    }
}



/// One line of a rank for a client: a loc key (`RANK_<kind>`) and the numbers it formats — the
/// client owns the wording, the sim owns the numbers. `{0}` is the main number in whole percent
/// (or a count), `{1}` a second number (seconds / stacks / count), `{2}` the skill id when the
/// line names one (the client resolves it through its skill names).
#[derive(Debug, Clone, serde::Serialize, PartialEq, Eq)]
pub struct RankLine {
    pub key: &'static str,
    pub args: Vec<i64>,
    pub skill: &'static str,
}

fn line(r: &Rank) -> RankLine {
    let p = |bp: i32| i64::from(bp) / 100;
    let pu = |bp: u32| i64::from(bp) / 100;
    let sec = |t: u32| i64::from(t) / 10;
    match *r {
        Rank::SelfAoeTargets { n } => RankLine { key: "RANK_self_aoe_targets", args: vec![i64::from(n)], skill: "iron_chains" },
        Rank::DefStackWhenHitTaunting { pct, max_stacks, ticks } => RankLine { key: "RANK_def_stack_when_hit_taunting", args: vec![i64::from(pct), i64::from(max_stacks), sec(ticks)], skill: "" },
        Rank::ShieldBreakRefund { skill, gauge } => RankLine { key: "RANK_shield_break_refund", args: vec![i64::from(gauge)], skill },
        Rank::SkillPower { skill, mult_bp } => RankLine { key: "RANK_skill_power", args: vec![pu(mult_bp) - 100], skill },
        Rank::AegisSignature { burst_mult_bp, taunt_ticks, refund_gauge } => RankLine { key: "RANK_aegis_signature", args: vec![pu(burst_mult_bp), sec(taunt_ticks), i64::from(refund_gauge)], skill: "aegis" },
        Rank::AoeCooldownRefund { skill, hits, ticks } => RankLine { key: "RANK_aoe_cooldown_refund", args: vec![hits as i64, sec(ticks)], skill },
        Rank::KillExplosion { atk_bp } => RankLine { key: "RANK_kill_explosion", args: vec![pu(atk_bp)], skill: "" },
        Rank::HealSplash { share_bp } => RankLine { key: "RANK_heal_splash", args: vec![pu(share_bp)], skill: "" },
        Rank::CommunioAll { atk_bp } => RankLine { key: "RANK_communio_all", args: vec![pu(atk_bp)], skill: "communio" },
        Rank::HealedAllyDef { pct, ticks } => RankLine { key: "RANK_healed_ally_def", args: vec![i64::from(pct), sec(ticks)], skill: "" },
        Rank::BenedictioSignature { extra_atk_bp, heal_maxhp_bp, dmg_dealt_bp } => RankLine { key: "RANK_benedictio_signature", args: vec![p(extra_atk_bp), pu(heal_maxhp_bp), p(dmg_dealt_bp)], skill: "benedictio_lucis" },
        Rank::MarkStacks { n } => RankLine { key: "RANK_mark_stacks", args: vec![i64::from(n)], skill: "" },
        Rank::StatusShape { skill, duration_bp, magnitude_bp } => RankLine { key: "RANK_status_shape", args: vec![pu(duration_bp), pu(magnitude_bp)], skill },
        Rank::VolleyTargets { n } => RankLine { key: "RANK_volley_targets", args: vec![i64::from(n)], skill: "" },
        Rank::ExploitBonus { bp } => RankLine { key: "RANK_exploit_bonus", args: vec![pu(bp)], skill: "" },
        Rank::SkillLevels { skills, plus } => RankLine { key: "RANK_skill_levels", args: vec![i64::from(plus), skills.len() as i64], skill: skills.first().copied().unwrap_or("") },
        Rank::DmgAdd { skill, bp } => RankLine { key: "RANK_dmg_add", args: vec![pu(bp)], skill },
        Rank::SelfAoeVuln { skill, vuln_bp, ticks } => RankLine { key: "RANK_self_aoe_vuln", args: vec![p(vuln_bp), sec(ticks)], skill },
        Rank::SelfAoeCooldownRefund { skill, hits, ticks } => RankLine { key: "RANK_aoe_cooldown_refund", args: vec![hits as i64, sec(ticks)], skill },
        Rank::ShieldCastAtk { skill, atk_bp, ticks } => RankLine { key: "RANK_shield_cast_atk", args: vec![p(atk_bp), sec(ticks)], skill },
        Rank::KillGauge { gauge } => RankLine { key: "RANK_kill_gauge", args: vec![i64::from(gauge)], skill: "" },
        Rank::NukeVuln { skill, vuln_bp, ticks } => RankLine { key: "RANK_nuke_vuln", args: vec![p(vuln_bp), sec(ticks)], skill },
        Rank::MarkDuration { mult_bp } => RankLine { key: "RANK_mark_duration", args: vec![pu(mult_bp)], skill: "" },
        Rank::MarkStep { bp } => RankLine { key: "RANK_mark_step", args: vec![p(bp)], skill: "" },
        Rank::SparkStep { bp } => RankLine { key: "RANK_spark_step", args: vec![p(bp)], skill: "" },
        Rank::BuffPower { skill, add } => RankLine { key: "RANK_buff_power", args: vec![p(add)], skill },
        Rank::HealBuffEverywhere { stacks } => RankLine { key: "RANK_heal_buff_everywhere", args: vec![i64::from(stacks)], skill: "" },
        Rank::OverhealShield { share_bp, ticks } => RankLine { key: "RANK_overheal_shield", args: vec![pu(share_bp), sec(ticks)], skill: "" },
        Rank::ElementPicks { n } => RankLine { key: "RANK_element_picks", args: vec![i64::from(n)], skill: "" },
        Rank::ElementRoll { bp } => RankLine { key: "RANK_element_roll", args: vec![p(bp)], skill: "" },
        Rank::SparkStacks { n } => RankLine { key: "RANK_spark_stacks", args: vec![i64::from(n)], skill: "" },
        Rank::SparkKeepBelowPct { pct } => RankLine { key: "RANK_spark_keep_below", args: vec![i64::from(pct)], skill: "" },
        Rank::BuffParty { skill } => RankLine { key: "RANK_buff_party", args: vec![], skill },
        Rank::BuffDuration { skill, ticks } => RankLine { key: "RANK_buff_duration", args: vec![sec(ticks)], skill },
        Rank::CastGaugeRate { skill, bp } => RankLine { key: "RANK_cast_gauge_rate", args: vec![p(bp)], skill },
        Rank::ElementRollAll => RankLine { key: "RANK_element_roll_all", args: vec![], skill: "tidebless" },
        Rank::BuffTargets { skill, plus } => RankLine { key: "RANK_buff_targets", args: vec![i64::from(plus)], skill },
        Rank::CritGaugeRider { skill, gauge } => RankLine { key: "RANK_crit_gauge_rider", args: vec![i64::from(gauge)], skill },
        Rank::CritSparkRider { skill, atk_bp } => RankLine { key: "RANK_crit_spark_rider", args: vec![p(atk_bp)], skill },
        Rank::ShatterStack { skill, bp, cap } => RankLine { key: if cap > 0 { "RANK_shatter_stack_cap" } else { "RANK_shatter_stack" }, args: vec![pu(bp), i64::from(cap)], skill },
        Rank::SkillAoe { skill, radius } => RankLine { key: "RANK_skill_aoe", args: vec![i64::from(radius) * 2 + 1], skill },
        Rank::ShatterJump => RankLine { key: "RANK_shatter_jump", args: vec![], skill: "" },
        Rank::AuraHoldsShatter => RankLine { key: "RANK_aura_holds_shatter", args: vec![], skill: "remorseless_winter" },
        Rank::FeverOnBreak { ticks } => RankLine { key: "RANK_fever_on_break", args: vec![sec(ticks)], skill: "break_fever" },
        Rank::GaugeOnCappedMark { gauge } => RankLine { key: "RANK_gauge_on_capped_mark", args: vec![i64::from(gauge)], skill: "" },
        Rank::CooldownResetAfterUlt { ult, skills } => RankLine { key: "RANK_cooldown_reset_after_ult", args: vec![skills.len() as i64], skill: ult },
        Rank::StatusSpread { skill, radius } => RankLine { key: "RANK_status_spread", args: vec![i64::from(radius) * 2 + 1], skill },
        Rank::DotStacks { skill, instances } => RankLine { key: "RANK_dot_stacks", args: vec![instances as i64], skill },
    }
}

/// The six ranks of `unit_def` described, S1 first — each a list of lines.
pub fn describe(unit_def: &str) -> Vec<Vec<RankLine>> {
    ranks_of(unit_def).iter().map(|r| r.iter().map(line).collect()).collect()
}

#[cfg(test)]
mod tests {
    use crate::grid::{Cell, Element, Entity, Team};
    use crate::items::Exemplar;

    fn unit(def: &str, stars: u8) -> Entity {
        let mut e = Entity::idle(1, Cell::new(0, 0)).with_team(Team::Player).with_combat(1_000, 100, 0, 0, Element::Fire);
        e.unit_def = def.into();
        e.exemplar = Some(Exemplar { id: def.into(), stars, ..Default::default() });
        e
    }

    #[test]
    fn zolfia_s2_sweeps_the_split_s4_jumps_and_s6_holds_the_shatter() {
        assert_eq!(super::skill_aoe(&unit("zolfia", 1), "coldflame_split"), 0);
        assert_eq!(super::skill_aoe(&unit("zolfia", 2), "coldflame_split"), 1);
        assert_eq!(super::shatter_stack(&unit("zolfia", 1)).map(|s| s.0), Some("coldflame_split"));
        assert!(!super::shatter_jump(&unit("zolfia", 3)) && super::shatter_jump(&unit("zolfia", 4)));
        assert!(!super::aura_holds_shatter(&unit("zolfia", 5)) && super::aura_holds_shatter(&unit("zolfia", 6)));
    }

    #[test]
    fn astier_s2_rides_the_gauge_on_tidebless_and_s6_blesses_every_element() {
        assert_eq!(super::cast_gauge_rate(&unit("astier", 1), "tidebless"), 0);
        assert_eq!(super::cast_gauge_rate(&unit("astier", 2), "tidebless"), 5_000);
        assert!(!super::element_roll_all(&unit("astier", 5)));
        assert!(super::element_roll_all(&unit("astier", 6)));
        assert_eq!(super::buff_extra_ticks(&unit("astier", 6), "tidebless"), 10, "only S1's +1 s");
    }

    #[test]
    fn ranks_open_in_order_and_the_level_ranks_read_above_the_cap() {
        let fb = crate::skills::skill_def("fireball").unwrap();
        let s0 = super::power(&unit("lapis", 0), fb, 10);
        let s3 = super::power(&unit("lapis", 3), fb, 10);
        assert!(s3 > s0, "S3 lifts Fireball above Lv10: {s0} → {s3}");
        let meteor = crate::skills::skill_def("fire_cat_meteor").unwrap();
        assert_eq!(super::power(&unit("lapis", 5), meteor, 5), meteor.power_at(8), "S5 = ult +3");
        assert_eq!(super::self_aoe_extra_targets(&unit("seraphine", 1)), 2);
        assert_eq!(super::self_aoe_extra_targets(&unit("seraphine", 0)), 0);
        assert_eq!(super::volley_extra_targets(&unit("aveline", 4)), 2);
        assert_eq!(super::dot_stacks(&unit("raplus", 2), "rend"), 3);
        assert_eq!(super::dot_stacks(&unit("raplus", 1), "rend"), 1);
        assert_eq!(super::communio_all(&unit("lucielle", 2)), Some(1_500));
        assert_eq!(super::active(&unit("swordsman_t1", 6)).count(), 0, "a class unit has no ladder");
        let mut other = unit("raplus", 6);
        other.exemplar.as_mut().unwrap().id = "aveline".into();
        assert_eq!(super::active(&other).count(), 0, "another unit's overlay is not this unit's ladder");
    }
}

#[cfg(test)]
mod fielded_tests {
    #[test]
    fn a_pulled_lapis_at_six_stars_has_her_web() {
        let d = crate::data::GameData::from_json_str(include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/../../data/game_data.json"))).unwrap();
        let mut s = crate::data::new_starter_party(42, &d, "zone_b1_1").unwrap();
        s.party_slots = 2;
        let id = crate::data::grant_exemplar_unit(&mut s, &d, "lapis").expect("granted");
        crate::data::deploy(&mut s, &d, id).expect("deployed");
        let e = s.grid.entity_of_id_mut(id).unwrap();
        e.exemplar.as_mut().unwrap().stars = 6;
        let e = s.grid.entity_of_id(id).unwrap();
        eprintln!("unit_def={} exemplar={:?}", e.unit_def, e.exemplar.as_ref().map(|x| (&x.id, x.stars)));
        assert!(super::active(e).count() >= 6);
        let fb = crate::skills::skill_def("fireball").unwrap();
        assert!(super::power(e, fb, 5) > fb.power_at(5));
    }
}
