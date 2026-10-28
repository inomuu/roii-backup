//! Exemplar-GRANTED skills — signature skills that ride on a collectible Exemplar
//! (Resonance system), not on any class tree. Registered like class skills so
//! `skill_def` / `skill_display` resolve them, but reached only when an equipped
//! Exemplar materializes them into a member's loadout.

use super::super::{BuffKind, BuffSpec, DebuffSpec, DEFAULT_SKILL, SkillDef, SkillKind};
use crate::grid::{DamageType, Element};

/// Sanctus Terra — Seraphine Luminary Knight's signature. Consecrates the 5×5 ground
/// AROUND THE CASTER as a moving AURA (follows her for the duration) for 15 s, pulsing every 1 s
/// (15 waves). Each pulse is MAGIC damage (MDEF-mitigated) but scales off the caster's
/// ATK — 8%/level → Lv5 40% … Lv10 80% — element Holy. Every enemy standing in it is
/// marked `SanctusTerra` (the game's first ELEMENT-specific vulnerability): +20%…+30%
/// (2%/level) taken from HOLY damage of ANY source. Costs 2 loadout slots. Max Lv10.
pub const SANCTUS_TERRA: SkillDef = SkillDef {
    id: "sanctus_terra",
    kind: SkillKind::Active,
    max_level: 10,
    loadout_slots: 2,        // uses 2 skill slots
    cooldown_ticks: 200,        // 20 s (> the 15 s duration → still one field at a time)
    // No cast time — the consecration lands INSTANTLY (motion delay below is only the
    // brief casting animation lock, not a wind-up bar).
    motion_delay_ticks: 10,
    anim_slot: "special_1",     // cast clip = the overhead-chop slot (universal id)
    field_center_self: true,    // consecrate where the knight STANDS
    field_follows_caster: true, // ...and the consecration FOLLOWS her (an aura, not a drop)
    range: 2,                   // cast when an enemy is close (inside the eventual field)
    ground_field_radius: 2,     // 5×5
    field_hits: 15,             // 15 waves …
    field_duration_ticks: 150,  // … over 15 s → a pulse every 1 s
    field_strike_hits: 1,       // one real number per pulse
    power_bp: 800,              // 8% at Lv1 → Lv5 40%, Lv10 80% (per second)
    power_per_level_bp: 800,
    field_power_from_atk: true, // magic damage, but base = ATK
    cast_element: Some(Element::Light),
    damage_type: Some(DamageType::Magic),
    // Holy-vulnerability mark, applied to every victim each pulse (chance 10000 = guaranteed):
    // magnitude Lv5 = 2000 (+20%) … Lv10 = 3000 (+30%). Duration 45 t outlives the 30-t pulse
    // gap so it persists while standing in the field and lapses ~1.5 s after leaving.
    on_hit_debuff: Some(DebuffSpec {
        kind: BuffKind::SanctusTerra,
        magnitude: 1_200,
        per_level: 200,
        duration: 45,
        chance_bp: 10_000,
        chance_per_level: 0,
    }),
    ..DEFAULT_SKILL
};

/// Fiat Lux — Seraphine's PASSIVE: on a landed basic MELEE hit, 20% chance to smite the
/// struck target with a burst of holy light — ONE magic instance (MDEF-mitigated) of
/// `20% ATK × level` (Lv5 100% … Lv10 200%), drawn as a TWIN flash (2 visual hits of
/// half the total: Lv5 2×50% … Lv10 2×100%). Single
/// target. The proc roll is GATED on knowing the skill, so no other entity ever takes
/// the PRNG draw (goldens safe). Ranged basics join when an archer Exemplar lands.
pub const FIAT_LUX: SkillDef = SkillDef {
    id: "fiat_lux",
    kind: SkillKind::Passive,
    max_level: 10,
    power_bp: 2_000,            // 20% ATK per level: power_at(lv) = 2000·lv
    power_per_level_bp: 2_000,
    ..DEFAULT_SKILL
};

/// Benedictio Lucis — Lucielle's party buff (Active, 3 s variable cast). Grants every
/// living ally +flat ATK & MATK and +% Holy-damage-dealt for 30 s, re-cast on cooldown
/// (duration > cd ⇒ ~100% uptime). Scales via ★ (0★ = Lv5 → 5★ = Lv10). Costs 2 slots.
pub const BENEDICTIO_LUCIS: SkillDef = SkillDef {
    id: "benedictio_lucis",
    kind: SkillKind::Active,
    max_level: 10,
    loadout_slots: 2,
    // Her ULTIMATE since 0.0.421 (user: "Ultimate เปลี่ยนเป็นบัฟ ... เป็น fix"): no cooldown and
    // no windup — the Limit Break gauge is the gate — and FIXED magnitudes: the buff does not
    // scale with her ATK or her level, only the star ranks re-level it.
    lb_max: 1_000,
    lb_on_skill: 15,
    // 0.0.491: 5 → 2 once a ranged basic began to charge the gauge; 25 casts an hour → ~17.
    lb_on_basic: 2,
    lb_on_hit_taken: 0,
    lb_per_second: 2,
    motion_delay_ticks: 5,
    range: 0,
    cast_buff: BENEDICTIO_BUFFS,
    cast_buff_party: true,
    anim_slot: "special_1",
    ..DEFAULT_SKILL
};
/// +5·lvl flat ATK & MATK (Lv5 +25 … Lv10 +50) and +500·lvl bp Holy-dmg-dealt
/// (Lv5 +25% … Lv10 +50%), party-wide. Duration slightly over the cooldown for uptime.
/// +20% ATK to the whole party for 30 s, +2% per ultimate level (the board's ranks, not her
/// ATK — "fixed" means it never reads the caster's numbers; user: "ulti +atk 20% +2%/lv").
/// 0.0.469: the blessing is ATK% (in THIS engine `AtkPct` is a clean multiplier on total ATK, so
/// it is worth as much as a damage layer and keeps her distinct from Astier's `DmgDealtPct`), and
/// what was wrong is UPTIME: 13 casts an hour × 30 s covered 11% of the fight and her seat
/// measured −7% against a plain DPS. The fix is DURATION, not frequency — a cheaper gauge read
/// 39 casts an hour, and every ultimate is a cut-in nobody wants to watch that often (user
/// 2026-09-23: "39 ครั้งต่อชม เยอะไป … 10-12 ครั้งก็พอ"). ONE long blessing instead: 120 s, so the
/// same ~13 casts cover ~43% of the fight.
const BENEDICTIO_BUFFS: &[BuffSpec] = &[
    BuffSpec { kind: BuffKind::AtkPct, magnitude: 3_000, per_level: 300, duration: 1_200 },
];

/// Sagitta Lucis — Lucielle's PASSIVE basic-attack override: her basic becomes a RANGED
/// (range 3) Holy MAGIC beam dealing 50% ATK, so the atk=0 acolyte pokes from the
/// backline and her basics feed the Holy loop. Flat (no ★ scaling). Drives the reusable
/// `SkillDef.basic_*` fields via `skills::basic_override`.
pub const SAGITTA_LUCIS: SkillDef = SkillDef {
    id: "sagitta_lucis",
    kind: SkillKind::Passive,
    max_level: 1,
    basic_range: 3,
    basic_ranged: true,
    basic_damage_type: Some(DamageType::Magic),
    basic_element: Some(Element::Light),
    basic_power_bp: 5_000, // 50% ATK
    ..DEFAULT_SKILL
};

/// Lux Medica — Lucielle's HEAL (Skill 1 since 0.0.421): the lowest-HP ally in range 6, every
/// 8 s, for `power_at(level)` of her ATK (Lv1 50% → Lv10 95%; user: "50% atk +5%"). Rides
/// `try_cast_support` through `heal_scales_atk`; the acolyte Heal's Renewal formula is not hers.
pub const LUX_MEDICA: SkillDef = SkillDef {
    id: "lux_medica",
    kind: SkillKind::Active,
    max_level: 10,
    loadout_slots: 2,
    cooldown_ticks: 80,
    motion_delay_ticks: 5,
    power_bp: 5_000,
    power_per_level_bp: 500,
    range: 6,
    target_lowest_hp: true,
    heals_ally: true,
    heal_scales_atk: true,
    anim_slot: "cast_finish",
    ..DEFAULT_SKILL
};

/// Communio — Lucielle's second PASSIVE (0.0.421; user: "เมื่อใช้สกิลแล้วจะฮีล unit จาก faction
/// เดียวกัน หรือ Sentinel เพิ่มเติม"): every active she completes also heals each living ally that
/// shares her element (Holy) or stands as a Sentinel (Seraphine) for 10% of her ATK. The
/// party-building hook: a Holy or tank-led team gets a second heal out of every cast.
pub const COMMUNIO: SkillDef = SkillDef {
    id: "communio",
    kind: SkillKind::Passive,
    max_level: 1,
    communio_atk_bp: 1_000,
    ..DEFAULT_SKILL
};

// ==================== AVELINE STARFLETCHER (Archer · Shadow · Destroyer) — 0.0.423 ====================
// Every skill scales from ATK except the ultimate (the user's rule for every kit since Lucielle).

/// Umbra Fletch — her ONE passive (user: "Passive 1 อันพอ"): every arrow is Shadow, and every
/// hit she lands leaves an UMBRAL MARK — +5% Shadow damage taken per hit, three deep (15%),
/// 8 s, refreshed by each hit. The mark feeds her own volley and every Shadow ally.
pub const UMBRA_FLETCH: SkillDef = SkillDef {
    id: "umbra_fletch",
    kind: SkillKind::Passive,
    max_level: 1,
    basic_element: Some(Element::Dark),
    basic_power_bp: 8_000, // 0.0.481: 5,500 → 8,000 — each cast now takes a swing
    mark_step_bp: 500,
    mark_max_stacks: 3,
    mark_ticks: 80,
    ..DEFAULT_SKILL
};

/// Nightfall Volley — Skill 1: three arrows, one each on the three lowest-HP enemies in range
/// 6, each for 70% ATK +5% per level. The finisher of a crowd, not the opener.
pub const NIGHTFALL_VOLLEY: SkillDef = SkillDef {
    id: "nightfall_volley",
    kind: SkillKind::Active,
    max_level: 10,
    loadout_slots: 2,
    cooldown_ticks: 60,
    motion_delay_ticks: 5,
    power_bp: 5_800,            // 84% ATK +6%/lv (0.0.440: −20%)
    power_per_level_bp: 420,
    range: 6,
    volley_targets: 3,
    damage_type: Some(DamageType::Physical),
    anim_slot: "special_1",
    ..DEFAULT_SKILL
};

/// Shadowbind — Skill 2: one arrow at 60% ATK that opens a BLEED for 120% ATK +10% per level
/// over 5 s. The boss tool: a single target, damage that keeps landing while she moves on.
pub const SHADOWBIND: SkillDef = SkillDef {
    id: "shadowbind",
    kind: SkillKind::Active,
    max_level: 10,
    loadout_slots: 2,
    cooldown_ticks: 100,
    motion_delay_ticks: 5,
    power_bp: 6_750,
    range: 6,
    damage_type: Some(DamageType::Physical),
    on_hit_debuff: Some(DebuffSpec {
        kind: BuffKind::Bleed,
        magnitude: 13_500,
        per_level: 1_125,
        duration: 50,
        chance_bp: 0,
        chance_per_level: 0,
    }),
    anim_slot: "special_1",
    ..DEFAULT_SKILL
};

/// Eclipse Rain — her ULTIMATE: a 5×5 on the BOSS if one stands within 8 (user: "priority
/// boss"), else on the densest knot of enemies; one strike of 300% ATK +20% per level on
/// everything under it (user: "อัลติ scale ได้"). The Limit Break gauge is the gate.
pub const ECLIPSE_RAIN: SkillDef = SkillDef {
    id: "eclipse_rain",
    kind: SkillKind::Active,
    max_level: 10,
    loadout_slots: 2,
    // 0.0.469: 26 casts an hour was a cut-in every two minutes; halving the per-cast gauge puts
    // her near 15 and the ultimate pays 60% more to hold the same damage.
    lb_max: 1_000,
    lb_on_skill: 6,
    lb_on_basic: 2,
    lb_on_hit_taken: 0,
    lb_per_second: 2,
    motion_delay_ticks: 5,
    range: 8,
    ground_field_radius: 2,
    field_hits: 1,
    field_duration_ticks: 5,
    field_power_from_atk: true,
    field_place_densest: true,
    power_bp: 160_000,          // 2,250% ATK +150%/lv (0.0.440: Striker ult ×3)
    power_per_level_bp: 11_000,
    anim_slot: "special_1",
    ..DEFAULT_SKILL
};

/// Umbral Exploit — Nyx's one passive: every blade is Shadow, and every packet she deals is
/// +10% per DISTINCT debuff on the target (Darkness, Bleed, Umbral Mark, Burn…). Flat.
pub const UMBRAL_EXPLOIT: SkillDef = SkillDef {
    id: "umbral_exploit",
    kind: SkillKind::Passive,
    max_level: 1,
    basic_element: Some(Element::Dark),
    debuff_amp_bp: 1_000,
    // Sticky Fingers (0.0.464, Laplus): a monster SHE last-hits pays +20% Gil (user: 50% read
    // ≈ +18% party Gil on the bench; 20% keeps the whole kit inside +5-10%).
    kill_gil_bonus_bp: 2_000,
    ..DEFAULT_SKILL
};

/// Pickpocket — Skill 1 (0.0.464, Laplus; the id stays `veil_slash` so saves, ranks and the
/// art keys hold): NO damage — a touch that DAZZLES the target (it whiffs 20% of its blows,
/// 10% on a boss, for 8 s) and lifts 10% of its own price (`gil_reward`) into the party's purse
/// at once. Every 8 s.
pub const VEIL_SLASH: SkillDef = SkillDef {
    id: "veil_slash",
    kind: SkillKind::Active,
    max_level: 10,
    loadout_slots: 2,
    cooldown_ticks: 80,
    motion_delay_ticks: 5,
    power_bp: 1,
    range: 1,
    damage_type: Some(DamageType::Physical),
    pickpocket_gil_bp: 1_000,
    on_hit_debuff: Some(DebuffSpec {
        kind: BuffKind::Dazzle,
        magnitude: 2_000,
        per_level: 0,
        duration: 80,
        chance_bp: 0,
        chance_per_level: 0,
    }),
    anim_slot: "special_2",
    ..DEFAULT_SKILL
};

/// Rend — Skill 2: 100% ATK +10%/lv and a REND (Bleed) of 150% ATK +15%/lv over 5 s. Only cast
/// on a target not already bleeding (the generic DoT rule). Every 10 s.
pub const REND: SkillDef = SkillDef {
    id: "rend",
    kind: SkillKind::Active,
    max_level: 10,
    loadout_slots: 2,
    cooldown_ticks: 100,
    motion_delay_ticks: 5,
    power_bp: 15_000,
    power_per_level_bp: 1_500,
    range: 1,
    damage_type: Some(DamageType::Physical),
    on_hit_debuff: Some(DebuffSpec {
        kind: BuffKind::Bleed,
        magnitude: 22_500,
        per_level: 2_250,
        duration: 50,
        chance_bp: 0,
        chance_per_level: 0,
    }),
    anim_slot: "special_1",
    ..DEFAULT_SKILL
};

/// Umbral Requiem — her ULTIMATE: every enemy on the board takes 200% ATK +15%/lv Shadow and is
/// left in Darkness AND Rended. Gated by the Limit Break gauge (`try_cast_zone_wide`).
pub const UMBRAL_REQUIEM: SkillDef = SkillDef {
    id: "umbral_requiem",
    kind: SkillKind::Active,
    max_level: 10,
    loadout_slots: 2,
    lb_max: 1_000,
    // 0.0.469: 26 casts an hour is a cut-in every two minutes; half the gauge per cast puts her
    // near 15, and the strike pays 70% more so her damage holds.
    lb_on_skill: 6,
    lb_on_basic: 2,
    lb_on_hit_taken: 0,
    lb_per_second: 2,
    motion_delay_ticks: 8,
    range: 1,
    zone_wide: true,
    power_bp: 85_000,
    power_per_level_bp: 6_375,
    damage_type: Some(DamageType::Physical),
    // Grand Heist (0.0.464): the storm is a pickpocket of the whole room — every enemy is
    // Dazzled and pays 10% of its price, then bleeds.
    pickpocket_gil_bp: 1_000,
    on_hit_debuff: Some(DebuffSpec {
        kind: BuffKind::Dazzle,
        magnitude: 2_000,
        per_level: 0,
        duration: 80,
        chance_bp: 0,
        chance_per_level: 0,
    }),
    on_hit_debuff_2: Some(DebuffSpec {
        kind: BuffKind::Bleed,
        magnitude: 22_500,
        per_level: 2_250,
        duration: 50,
        chance_bp: 0,
        chance_per_level: 0,
    }),
    anim_slot: "special_1",
    ..DEFAULT_SKILL
};

/// Rivus — Astier's heal, and a stream rather than a beam: it lands on the ally in the worst
/// shape BY PERCENT, then BOUNCES twice more to the next two worst, at 70% and 50% of the first
/// number. Percent, not missing HP, because flat HP always feeds the tank. Three touches, not a
/// party-wide wash, so it still matters who is hurt. cd 8 s. Max Lv10. `try_cast_rivus`.
pub const RIVUS: SkillDef = SkillDef {
    id: "rivus",
    kind: SkillKind::Active,
    max_level: 10,
    loadout_slots: 2,
    cooldown_ticks: 80,    // 8 s
    cast_time_ticks: 10,   // 1 s
    motion_delay_ticks: 5,
    range: 0,              // map-wide, like the other smart heals
    heals_ally: true,
    power_bp: 6_000,       // scales the renewal heal formula, as `heal`/`dupla_medicina` do
    anim_slot: "cast_finish",
    ..DEFAULT_SKILL
};

/// How each bounce of `RIVUS` decays, in percent of the FIRST heal. Three entries = three
/// touches; the decay is what stops it being a party heal with extra steps.
pub const RIVUS_BOUNCE_PCT: [i64; 3] = [100, 70, 50];

/// Per-level growth of `FONS_VITAE`'s share, in basis points of max HP.
pub const FONS_VITAE_PER_LEVEL_BP: u32 = 200;

/// Lumen Contagio — Lucielle's active plague-seeder. A light Holy magic poke (range 6) on the
/// nearest enemy NOT yet carrying LumenVuln, applying +15…30% holy-vuln (300·lvl bp) for 10 s.
/// When a marked enemy dies, `xp::maybe_schedule_lumen` detonates a SCHEDULED 5×5 Holy blast that
/// re-marks the unmarked around it — so the plague spreads across the pack. cd 8 s. Max Lv10.
pub const LUMEN_CONTAGIO: SkillDef = SkillDef {
    id: "lumen_contagio",
    kind: SkillKind::Active,
    max_level: 10,
    loadout_slots: 2,
    cooldown_ticks: 80, // 8 s — just seeds the chain
    motion_delay_ticks: 5,
    range: 6,
    power_bp: 9_000, // 90% ATK poke (0.0.437: skills ×3)
    damage_type: Some(DamageType::Magic),
    cast_element: Some(Element::Light),
    on_hit_debuff: Some(DebuffSpec {
        kind: BuffKind::LumenVuln,
        magnitude: 300,
        per_level: 300,
        duration: 100,     // 10 s
        chance_bp: 10_000, // guaranteed, no extra PRNG draw
        chance_per_level: 0,
    }),
    anim_slot: "cast_finish",
    ..DEFAULT_SKILL
};
