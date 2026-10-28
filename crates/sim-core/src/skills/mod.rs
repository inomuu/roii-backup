//! Skills — catalog + per-entity skill state (BETA-5 Phase 3).
//!
//! Design (see docs/taskbar-arpg-skills-jobs-design.md + the auto-cast
//! discussion): this is an idle auto-battler, so a build runs only ~2
//! ACTIVE skills, auto-cast by priority in place of a basic attack. Depth
//! comes from deepening those few (skill level now; attributes later).
//! PASSIVE skills are always-on and may be numerous (they don't compete
//! for the active "slots").
//!
//! Layout:
//!   - this file: the core types (`SkillKind`, `SkillDef`, `SkillState`).
//!   - `catalog`: the hardcoded `SkillDef` table + `skill_def` lookup.
//!   - `effects`: the passive-folding helpers (ATK/SP/aura/double-attack)
//!     and the Taunt/Endure tuning constants.
//!
//! All values are integer + the selection rule is deterministic
//! (GDD §4.2 — no hidden AI randomness).

use serde::{Deserialize, Serialize};

use crate::grid::{DamageType, Element};

mod buffs;
mod catalog;
mod effects;
mod tree;

// Flat re-export so callers keep using `skills::BASH`, `skills::skill_def`,
// `skills::passive_atk_bonus`, `skills::buffed_stats`, … regardless of which
// submodule owns them.
pub use buffs::*;
pub use catalog::*;
pub use effects::*;
pub use tree::*;

/// DELIVERY axis (0.0.190) — HOW a skill reaches its target, independent of its
/// damage type. `Inherit` = follow the caster's `proj_travel_ticks`. `Instant`
/// / `Projectile` FORCE it on the skill regardless of the caster — a weapon
/// strike stays instant even on a projectile class, a bolt stays a flying
/// projectile even on an instant class. Default `Inherit`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Delivery {
    Inherit,
    Instant,
    Projectile,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SkillKind {
    /// Auto-cast in place of a basic attack, gated by **cooldown + range
    /// only** — cooldown is the single power dial (short = spammy, long =
    /// signature). Reservation auras hold a stance by spending LOADOUT SLOTS
    /// (`loadout_slots > 0`).
    Active,
    /// Always-on effect (HP regen, stat boost, …). No cooldown, no slot;
    /// a build may stack many.
    Passive,
}

/// Immutable skill definition. Active and passive fields coexist; the
/// irrelevant ones are simply ignored per `kind`.
pub struct SkillDef {
    pub id: &'static str,
    pub kind: SkillKind,
    /// Max skill level (skill points raise level 1..max).
    pub max_level: u8,
    // --- active ---
    /// Slot cost in the loadout. EVERY slotted ACTIVE skill costs at least 1
    /// slot of the entity's capacity (`combat::loadout_capacity`); this field
    /// OVERRIDES that to a higher cost for a heavier skill (a party buff like
    /// Blessing = 2). 0 ⇒ the default cost of 1 (or 0 for passives, which never
    /// enter the loadout). See `skills::loadout_slot_cost`.
    pub loadout_slots: i32,
    pub cooldown_ticks: u32,
    /// MONSTER-ONLY spawn gate: chance-in-10000 that a freshly-spawned monster actually
    /// CARRIES this active (0 = always). Resolved per-monster from the entity id at spawn
    /// (NO PRNG draw), so a common pack gets organic variety — some cast it, some never —
    /// while a signature/elite (chance 0) always does. Players ignore it (chosen loadout).
    pub spawn_use_chance_bp: u32,
    /// MONSTER enrage gate: only cast this skill while the caster's HP is BELOW this percent of
    /// max (0 = no HP condition). A "desperation" trigger — e.g. Feral Transformation at <50% HP.
    /// Cheap check, no PRNG; pairs with `cast_chance_bp` for a probabilistic enrage.
    pub cast_hp_below_pct: u32,
    /// USE BUDGET: how many times this skill may fire before it is spent. 0 =
    /// unlimited (the default). Seeded into `SkillState.charges_left` when the skill
    /// is created, so a MONSTER — built fresh every spawn — gets its budget per
    /// FIGHT, which is what "she calls the household once, when wounded" means. A
    /// long cooldown only makes a repeat rare; this makes it impossible.
    pub max_charges: u32,
    /// HP gate, UPPER bound: the skill offers itself only while the caster is ABOVE
    /// this % of max HP. 0 = no upper bound.
    ///
    /// Pairs with `cast_hp_below_pct` to split a fight into phases: give the calm
    /// version an upper bound and the wounded version the matching lower one, and the
    /// two can never be live at the same time. Without it, a "wounded" variant simply
    /// ADDS to the base skill instead of replacing it — both stay live and fire on
    /// independent cooldowns.
    pub cast_hp_above_pct: u32,
    /// Per-ATTEMPT cast chance in basis points (0 = always, no PRNG). When set, each time the
    /// skill is otherwise ready (off cooldown, HP-gate met) it rolls this chance from the sim
    /// PRNG; a FAILED roll still burns the cooldown, so it retries next cycle. A genuine combat
    /// draw (like crit/freeze), so it shifts the stream — bless goldens when a mob using it fights.
    pub cast_chance_bp: u32,
    /// Motion delay (RO animation/motion lock): ticks the caster is LOCKED after
    /// this skill resolves — no move / attack / next cast — so the skill's
    /// animation plays out before basics resume. This is the lock that actually
    /// blocks basic attacks (RO's After-Cast Delay only gates the next SKILL).
    /// 0 = resolves with no lock (like a basic swing).
    pub motion_delay_ticks: u32,
    /// GROUND FIELD radius (Chebyshev cells). > 0 marks a "place a persistent
    /// field" active (Poison Cloud): instead of attacking, it spawns a
    /// `GroundField` centred on the caster. Cast via `try_cast_ground_field`
    /// (NOT the attack path); the field duration is per-skill. 0 = not a field skill.
    pub ground_field_radius: i32,
    /// DAMAGE-field timing: how many times the field ticks (Vermilion = 20) and
    /// over how many TICKS (Vermilion = 40 = 4 s) → hit interval = duration/hits.
    /// `power_bp` is the PER-HIT MATK%. 0 hits = the legacy status-only field
    /// (Poison Cloud uses neither; its duration is 50+50·level).
    pub field_hits: u32,
    /// Extra field hits PER skill level above 1 — total hits = `field_hits +
    /// field_hits_per_level × (level − 1)`. Lets a field scale its meteor / quake
    /// COUNT with level (Earth Drive = `level` hits, Meteor Rain = more meteors)
    /// instead of just per-hit power. 0 = a fixed hit count (Vermilion / Glacial).
    pub field_hits_per_level: u32,
    /// VISUAL hits each field WAVE shows (Sancta Tempestas = 10 per wave): one
    /// damage instance of `power × this`, drawn as this many numbers. 0 ⇒ 1 (a
    /// normal one-number field tick). Like a meteor's per-strike hits but for a
    /// plain (non-random) field.
    pub field_strike_hits: u32,
    pub field_duration_ticks: u32,
    /// DAMAGE-field base stat source. `false` (default) = the per-wave damage snapshots
    /// the caster's MATK (every existing field is magic). `true` = snapshot ATK instead
    /// while STILL delivering the wave as magic (MDEF-mitigated) — a physical-scaling holy
    /// field (Sanctus Terra). Only read by `place_ground_field`.
    pub field_power_from_atk: bool,
    /// SELF-CENTERED field: the field drops centred on the CASTER's cell (consecrate the
    /// ground you stand on — Sanctus Terra), not on the target's. `false` (default) =
    /// every existing field centres on the target. Only read by `place_ground_field`.
    pub field_center_self: bool,
    /// Pick the field's victim at RANDOM among enemies in range instead of the
    /// nearest one. One PRNG draw, taken only when the cast actually fires (so the
    /// draw order stays fixed for every skill that doesn't set this). A boss that
    /// marks a random member forces the whole party to watch the telegraph, where a
    /// nearest-target field only ever threatens the frontline.
    pub cast_target_random: bool,
    /// Plant the field as a FUSED bomb: harmless while it lives, one detonation on
    /// its last tick. `field_duration_ticks` becomes the countdown.
    pub field_fuse: bool,
    /// AURA field: the placed field FOLLOWS the caster (re-centred on their cell each
    /// tick) instead of staying where it was cast — Sanctus Terra moves with the knight.
    /// `false` (default) = every existing field stays put.
    pub field_follows_caster: bool,
    /// UNIVERSAL ANIMATION SLOT (display hint, locale-free): an abstract clip id — e.g.
    /// "special_1" — that every sheet (class or Exemplar skin) maps to its OWN row. ""
    /// (default) = the basic attack clip. The renderer resolves skill → slot via the
    /// bridge (`skill_info_json`), so adding a skill never edits a per-class table.
    pub anim_slot: &'static str,
    /// REUSABLE basic-attack override — a PASSIVE skill (e.g. Sagitta Lucis) re-profiles
    /// the wearer's BASIC attack. `basic_range` (0 = keep) sets its reach; `basic_ranged`
    /// flags the packet ranged; `basic_damage_type`/`basic_element` (None = keep) re-type it;
    /// `basic_power_bp` (0 = 100%) scales it. Read via `skills::basic_override` at
    /// `effective_atk_range` + `build_damage`. All default-off → any class/Exemplar can drive it.
    pub basic_range: i32,
    pub basic_ranged: bool,
    pub basic_damage_type: Option<DamageType>,
    pub basic_element: Option<Element>,
    pub basic_power_bp: u32,
    /// On every Nth field hit a victim takes (`status_at`), the status's BASE
    /// chance is boosted to 150% (RO Storm Gust's 3rd-hit freeze) — still rolled
    /// (and, for Freeze, still cut by MDEF/LUK). 0 = no special hit; the status
    /// kind + duration come from `on_hit_debuff`.
    pub status_at: u32,
    /// METEOR field mode (Meteor Rain). 0 = normal field (every interval hits the
    /// whole `ground_field_radius`). > 0 = each interval ONE meteor strikes a random
    /// cell in the target area and hits only this radius around it (5×5 ⇒ 2) — so
    /// drops can miss; meteor COUNT + hits-per-meteor scale with level in
    /// `place_ground_field`.
    pub impact_radius: i32,
    /// ONCE-PER-TARGET field (Lord of Vermilion): a persistent AoE that hits each
    /// victim ONE instance of `field_hits` numbers for its whole life — but a unit
    /// entering the AoE later gets its own. The instance is a VISUAL multi-hit
    /// (DEF/MDEF once). false = a normal repeating field.
    pub once_per_target: bool,
    /// How many HITS the renderer SHOWS for one cast (a display flourish — the
    /// sim still computes one damage packet). 1 = a single number; Sonic Blow
    /// sets 8 so its burst reads as 8 numbers + a yellow total. 0/1 = normal.
    pub display_hits: u32,
    /// Damage as a basis-point multiple of effective ATK/MATK at level 1.
    pub power_bp: u32,
    /// Added to `power_bp` per skill level above 1.
    pub power_per_level_bp: u32,
    pub range: i32,
    /// AoE explosion radius in cells (Chebyshev). 0 = single-target.
    /// 1 = a 3×3 blast centred on the target, hitting every enemy in it.
    pub aoe_radius: i32,
    /// AoE ASPD-slow driver (basis points). > 0 makes this a "dampener": on cast it slows the
    /// ASPD (`AspdSlowPct`) of EVERY foe within `aoe_radius` of the struck target by this bp,
    /// and targets a foe not already slowed (spread, not re-slow) — the acolyte Aspd Dampening
    /// mechanic, generalized so a monster's Wail reuses it. 0 = not a dampener. `apply_aspd_dampening`.
    pub aoe_aspd_slow_bp: u32,
    /// Caster-centred AoE radius (Chebyshev) for a point-blank blast that hits
    /// every enemy around the CASTER (Magnum Break). 0 = not a self-AoE. When
    /// > 0 the cast skips the single-target resolve and damages everything in
    /// range instead, then knocks survivors back by `knockback`.
    pub self_aoe_radius: i32,
    /// Cells to knock self-AoE survivors away from the caster (Magnum Break).
    /// 0 = no knockback.
    pub knockback: i32,
    /// Cells this skill HAULS every victim toward the caster (Iron Chains). A
    /// separate field from `knockback` on purpose: the two walk the same way but
    /// are different events, and a shove-triggered effect must never read a pull
    /// as a shove. 0 = no pull.
    pub pull: i32,
    /// Buffs applied on cast. Multi-kind (Blessing = STR+INT+DEX), so a slice.
    /// Empty = none. `cast_buff_party` decides self vs whole-party targeting.
    pub cast_buff: &'static [BuffSpec],
    /// If true, `cast_buff` applies to every living ally and the skill is cast
    /// proactively via `tick::try_cast_buffs` (cooldown ≈ duration ⇒ uptime),
    /// dealing no damage. If false, it's a self-buff applied on the attack path
    /// (the swing still lands). Blessing/Increase AGI/Angelus = true;
    /// Magnum Break/Energy Coat/Improve Concentration = false.
    pub cast_buff_party: bool,
    /// Debuff rolled onto the target on a hit (Frost Bolt slow, Envenom poison).
    /// `None` = none; `chance_bp == 0` applies without a PRNG draw.
    pub on_hit_debuff: Option<DebuffSpec>,
    /// Overrides the damage packet's element for this cast (the 3 mage bolts:
    /// Fire/Earth/Water). `None` = use the attacker's own element. Also drives
    /// element-aware auto-cast in `pick_active_skill`.
    pub cast_element: Option<Element>,
    /// DAMAGE-TYPE override for this skill (axis separation, 0.0.189): `Some`
    /// forces Physical / Magic / True regardless of the caster's `atk_type`, so
    /// "magic" is a property of the SKILL, not of the caster's weapon or its
    /// melee/ranged delivery. `None` = inherit the caster's `atk_type` (every
    /// current skill), so the default is byte-identical.
    pub damage_type: Option<DamageType>,
    /// DELIVERY override (axis separation): None-like `Inherit` = caster path.
    pub delivery: Delivery,
    /// Teleport-strike: on cast the attacker blinks to the cell BEHIND
    /// the target (a guaranteed backstab) before resolving, so `range`
    /// acts as the engage/gap-close reach. Shadow Strike-style.
    pub teleport: bool,
    /// Target selection for the cast: when true, pick the LOWEST-HP valid
    /// target within `range` (lowest-HP enemy for a damage skill, lowest-
    /// HP ally for a heal) instead of the attacker's default nearest
    /// target. Opt-in so other skills keep nearest-target behaviour.
    pub target_lowest_hp: bool,
    /// Target selection: when true, pick the NEWEST non-boss enemy within
    /// `range` (highest spawn id = most-recently-spawned) instead of the
    /// nearest. Drives Dark Ambush — the Assassin blinks onto a fresh add.
    /// Opt-in; mutually exclusive with `target_lowest_hp` in practice.
    pub target_newest_nonboss: bool,
    /// Base VARIABLE CAST TIME in ticks (RO VCT) before this active resolves.
    /// 0 = instant (every skill but the mage's damage spells). > 0 makes the
    /// caster channel a windup, standing still, before the spell fires; DEX+INT
    /// shorten it via `combat::effective_cast_time` (DEX·2+INT ≥ 530 ⇒ instant).
    /// Only the attack-path actives (bolts, Fireball) read this — buff/heal casts
    /// go through other paths and ignore it.
    pub cast_time_ticks: u32,
    // --- passive ---
    /// HP regen per 5-SECOND WINDOW (`HP_REGEN_INTERVAL_TICKS`), per skill
    /// level: `heal = flat·Lv + MaxHP·bp/10000·Lv` each window (0.0.178 —
    /// HP Recovery: `10·Lv + MaxHP·0.002·Lv` per 5 s, where 0.002 = 20 bp).
    pub hp_regen_flat: i32,
    pub hp_regen_maxhp_bp: u32,
    /// Flat ATK added per skill level (folded into `effective_atk` for
    /// any entity carrying this passive). Keen Eye-style mastery.
    pub atk_flat_per_level: i32,
    /// DODGE CHANCE in basis points per skill level (0.0.333). Replaces the old
    /// HIT/FLEE pair of fields: accuracy is gone, and evasion is one readable stat
    /// now, so an "Improve Dodge" passive grants the same thing gear does.
    pub dodge_bp_per_level: i32,
    /// Chance (basis points) PER SKILL LEVEL for a basic attack to strike
    /// twice. Double Attack-style passive — rolled only on basic swings
    /// (power 100%), only when > 0, so it never perturbs other entities'
    /// PRNG stream.
    pub double_atk_chance_bp: u32,
    /// DEF-scaling bonus damage (basis points of the caster's HARD DEF) ADDED to
    /// each hit of this skill — Shield Blast scales on armor, not just ATK. 0 =
    /// pure ATK scaling.
    pub def_scale_bp: u32,
    /// Requires a SHIELD equipped (`EquipSlot::Shield`) to cast — the shield-as-
    /// weapon skills (Shield Blast / Boomerang). `pick_active_skill` skips it
    /// otherwise. false = no equipment requirement.
    pub requires_shield: bool,
    /// Target selection: pick the FURTHEST valid enemy within `range` instead of
    /// the nearest (Shield Boomerang's long throw). Opt-in.
    pub target_furthest: bool,
    /// On a hit, lock the target's aggro onto the caster for this many ticks
    /// (single-target taunt — Shield Boomerang). 0 = no taunt.
    pub taunt_on_hit_ticks: u32,
    // --- Limit Break (the gauge lives on the ULTIMATE that spends it) ---
    /// Gauge size. **Non-zero marks this skill as the character's Ultimate**: it
    /// never fires on a cooldown, only when the gauge is full, and firing empties
    /// it. Putting the gauge on the ultimate rather than on the character means a
    /// character's answer to "how do I charge?" is written beside the thing the
    /// charge pays for.
    pub lb_max: u16,
    /// Charge gained per damage instance TAKEN — a tank's clock (Seraphine).
    pub lb_on_hit_taken: u16,
    /// Charge gained per BASIC swing that lands.
    pub lb_on_basic: u16,
    /// Charge gained per SKILL cast that resolves — once per cast, not once per
    /// victim. An AoE that catches eight enemies is one decision, not eight, and
    /// paying per body would make a crowd fill the gauge instantly.
    pub lb_on_skill: u16,
    /// Charge gained per Toughness break anywhere on the field — Rapis's clock,
    /// the same trigger her passive answers.
    pub lb_on_break: u16,
    /// Charge gained per second, so a quiet fight still fills eventually.
    pub lb_per_second: u16,
    /// Bonus multiplier on the Toughness damage this skill deals, in percent of
    /// the baseline (0 = the baseline every damage instance already carries, 300 =
    /// triple). INERT until the Toughness gauge exists
    /// (`docs/element-weakness-system.md` §6.1) — carried on the definitions now so
    /// the skills that are meant to break the gauge say so where they are written.
    pub toughness_dmg_mult: u32,
    /// BREAK TRIGGER: this skill answers a Toughness break — ANY break, by anyone,
    /// on any enemy. The break itself is the same event for every character
    /// (damage, stun, a vulnerability window); this is where characters differ,
    /// and it is authored per skill rather than special-cased in the tick loop.
    ///
    /// The owner queues each broken enemy id and fires this skill's own
    /// `power_at(level)` at them, one every `BREAK_TRIGGER_CD_TICKS`, up to
    /// `BREAK_QUEUE_CAP` waiting. A free answer — it never costs the owner a turn.
    pub break_trigger: bool,
    /// Active support: targets the most-injured ALLY in range and restores
    /// HP (`power_bp` % of the caster's MATK) instead of dealing damage.
    /// Heal-style. Cast in `try_cast_support`, not the attack path.
    pub heals_ally: bool,
    /// ATK-SCALED heal (0.0.421): when true a `heals_ally` active restores `power_at(level)`
    /// basis points of the caster's effective ATK instead of the Renewal formula. Lucielle's Lux
    /// Medica — a Support whose every skill rides ATK (user: "ทุกสกิล scale จาก atk").
    pub heal_scales_atk: bool,
    /// VOLLEY (0.0.423): > 0 makes the active a MULTI-TARGET strike — one instance of
    /// `power_at(level)` on each of the `volley_targets` lowest-HP enemies in range, distinct
    /// targets, spawn-order tie-break. Cast via `try_cast_volley`, not the attack path.
    pub volley_targets: u32,
    /// DENSEST PLACEMENT (0.0.423): a ground field with this set lands on the cell within
    /// `range` whose `ground_field_radius` square covers the MOST enemies (tie: nearer the
    /// caster, then lower cell id) instead of on the nearest enemy.
    pub field_place_densest: bool,
    /// UMBRAL MARK (0.0.424, passive field): every hit this entity LANDS stacks `UmbralMark` on
    /// the target — `mark_step_bp` per hit, `mark_max_stacks` deep, `mark_ticks` long. 0 = none.
    pub mark_step_bp: u32,
    pub mark_max_stacks: u32,
    pub mark_ticks: u32,
    /// SWEET POISON (0.0.451, Nemu): the mark also stacks a MOVE slow of `mark_slow_pct` (whole
    /// percent) and an ASPD slow of `mark_aspd_slow_bp` per stack, both per source like the
    /// mark itself. 0 = the mark is damage-only (Aveline).
    pub mark_slow_pct: i32,
    pub mark_aspd_slow_bp: i32,
    /// LEECH (0.0.451, Heartstring): this skill's `on_hit_debuff` DoT heals its SOURCE for
    /// `leech_bp` of every tick it deals. 0 = none. Carried on the Bleed buff as `leech_bp`.
    pub leech_bp: u32,
    /// BLACK HOLE (0.0.451, Abyssal Embrace): the ground field this skill lays PULLS every
    /// victim `field_pull` cells toward its centre on each hit interval and ROOTS them while it
    /// lives (they arrive and stay). With `field_fuse` the burst lands when the hole closes.
    pub field_pull: i32,
    /// TWILIGHT PACT (0.0.423, passive field): after this entity completes ANY active cast,
    /// every living ally of its element, or of the Striker doctrine, gains `pact_crit_bp` crit
    /// for `pact_ticks`. 0 = no such passive.
    pub pact_crit_bp: u32,
    /// EXPLOIT (0.0.426, passive field): every damage packet this entity deals is amplified by
    /// `debuff_amp_bp` per DISTINCT debuff kind on the target (Darkness, Bleed, Umbral Mark…).
    /// 0 = none. Nyx's Umbral Exploit.
    pub debuff_amp_bp: u32,
    /// ZONE-WIDE STRIKE (0.0.426): the active hits EVERY living enemy on the board for
    /// `power_at(level)`, applying `on_hit_debuff` and `on_hit_debuff_2` to each. Cast via
    /// `try_cast_zone_wide`, gated on the Limit Break gauge when `lb_max` is set.
    pub zone_wide: bool,
    /// A second on-hit debuff rolled after `on_hit_debuff` (same rules).
    pub on_hit_debuff_2: Option<DebuffSpec>,
    /// TARGET CAP for a caster-centred blast (0.0.427): > 0 keeps only the `n` NEAREST enemies
    /// in the ring (Chebyshev, then spawn order) — Iron Chains hauls two, not the room. 0 = all.
    pub self_aoe_max_targets: u32,
    pub pact_ticks: u32,
    /// COMMUNIO (0.0.421, passive field): after this entity completes ANY active cast, every
    /// living ally whose element matches the caster's, or whose unit is a Sentinel, is healed
    /// this many basis points of the caster's effective ATK. 0 = no such passive.
    pub communio_atk_bp: u32,
    /// MONSTER ally-heal scaling: when > 0, a `heals_ally` active restores this basis-point
    /// fraction of the HEALED ally's MAX HP, instead of the INT/MATK Renewal formula (a monster
    /// has no INT/MATK, so the RO heal barely dents its big HP pool). Read in `try_cast_support`.
    pub heal_maxhp_bp: u32,
    /// Added to `heal_maxhp_bp` per skill level above 1 (0.0.456, Ice Mend +1%/lv).
    pub heal_maxhp_per_level_bp: u32,
    /// A `heals_ally` skill that only ever targets the CASTER (0.0.456, Ice Mend).
    pub heal_self_only: bool,
    /// PASSIVE (0.0.462, Airi): +bp on EVERY hit the owner lands — basic and every skill — on a
    /// target that is currently TAUNTED (by anyone). The pair with a tank: Lumi holds them, Airi
    /// hits what is held.
    pub taunted_target_bp: u32,
    /// PASSIVE (0.0.463, Charon): +bp on EVERY hit the owner lands on a target whose Toughness
    /// is BROKEN (in its recovery window).
    pub broken_target_bp: u32,
    /// PASSIVE (0.0.463, Charon): the owner's BASIC swings chip Toughness at this `mult_pct`
    /// (100 = the baseline chip; 200 = double). 0 = baseline.
    pub basic_toughness_mult: u32,
    /// Single-target skill: aim at a BOSS in range first, else the highest-HP enemy in range
    /// (0.0.463, Blackwater Mark — the mark belongs on the one you mean to break).
    pub target_boss_first: bool,
    /// PASSIVE (0.0.464, Laplus): a monster the owner LAST-HITS pays +bp of its Gil.
    pub kill_gil_bonus_bp: u32,
    /// On every enemy this skill touches (0.0.464, Pickpocket / Grand Heist): the party is paid
    /// `bp` of that enemy's own `gil_reward` — the stage's price — at once.
    pub pickpocket_gil_bp: u32,
    /// This skill's cooldown is CLEARED whenever any enemy's Toughness breaks (0.0.465, Suzu).
    pub reset_on_break: bool,
    /// While this skill's self-buff is up, the owner's BASIC chips Toughness at this `mult_pct`
    /// and ignores the weakness gate (0.0.465, Suzu's ultimate — carried by `BuffKind::Sunder`'s
    /// twin `BuffKind::BreakFever`).
    pub fever_toughness_mult: u32,
    /// Stacks of `BuffKind::StaticSpark` this skill lays, and what one spark PAYS when the
    /// victim is struck (bp of the LAYER's ATK) — 0 = no spark (0.0.465, Mirei).
    pub spark_stacks: u32,
    pub spark_atk_bp: u32,
    /// The owner's attacks trigger a spark WITHOUT consuming a stack, and it aims at a sparked
    /// enemy first (0.0.465, Mirei's passive).
    pub spark_keeper: bool,
    /// This skill DETONATES every spark on every enemy it touches (0.0.465, Mirei's ultimate).
    pub spark_detonate: bool,
    /// PASSIVE (0.0.468, Astier): this share of the owner's basic damage heals the most wounded
    /// ally (the old `Unda Vitae` rule, now a field instead of an id check).
    pub heal_on_hit_bp: u32,
    /// PASSIVE (0.0.468, Astier): an ally the owner HEALS carries +bp damage dealt, this many
    /// stacks deep, for `HEALED_ALLY_TICKS`.
    pub healed_ally_dmg_bp: i32,
    pub healed_ally_stacks: i32,
    /// This party buff ALSO rolls one element the party carries and buffs it by `bp`
    /// (0.0.468, Tidebless).
    pub element_roll_bp: i32,
    /// A party HEAL-OVER-TIME of this share of each ally's OWN max HP per second, for the
    /// cast-buff's duration (0.0.468, Fons Vitae).
    pub hot_maxhp_bp: u32,
    pub hot_maxhp_per_level_bp: u32,
    /// A SELF-kind cast buff that lands instead on the `n` allies (not the caster) with the
    /// highest effective ATK, damage-dealing doctrines first — the carry (0.0.473, Sylvie's Arc
    /// Focus). 0 = the caster.
    pub buff_top_atk_allies: u8,
    /// A PARTY cast that also cuts every ally's skill cooldowns by this many ticks (0.0.473,
    /// Sylvie's ultimate).
    pub party_cooldown_cut_ticks: u32,
    /// A PARTY cast's ASPD buff is scaled by `aspd_kin_mult_bp` on allies whose element is this
    /// one (0.0.474, Sylvie's Surge: Thunder allies ×1.5).
    pub aspd_kin_element: Option<crate::grid::Element>,
    pub aspd_kin_mult_bp: u32,
    /// An ULTIMATE that strikes every enemy on the board once (`try_cast_room_nuke`).
    pub room_nuke: bool,
    /// PASSIVE (0.0.476, Zolfia): every hit the owner lands lays `WinterChill` for this long.
    pub winter_chill_ticks: u32,
    /// PASSIVE (0.0.476, Zolfia): a break on a Winter-Chilled target lays `Shattered` of `bp`
    /// for the break's stun plus `shatter_extra_ticks`.
    pub shatter_bp: i32,
    pub shatter_extra_ticks: u32,
    /// AURA (0.0.476, Zolfia's Remorseless Winter): while the skill's `WinterAura` buff lasts,
    /// every second each enemy within `aura_radius` is slowed and takes an Ice DoT of the
    /// owner's ATK.
    pub aura_radius: i32,
    pub aura_dot_atk_bp: u32,
    pub aura_dot_per_level_bp: u32,
    pub aura_aspd_slow_bp: i32,
    pub aura_aspd_slow_per_level_bp: i32,
    /// When this active applies a `Hiding` self-buff, the caster may MOVE while hidden (Twinling's
    /// vanish-and-stalk) instead of standing still. Read in the tick's Hiding guard. Default false
    /// = the thief's panic Hiding, which holds position. A property of the vanish SKILL, not the mob.
    pub move_while_hidden: bool,
    /// MONSTER spawn-stalk: this active fires ONCE, 1-3 s after the mob spawns (a hash-derived
    /// delay in `spawn`, NOT a PRNG draw), then never recasts (paired with a huge `cooldown_ticks`).
    /// Twinling's vanish: wink out shortly after spawn, stalk to the target, reappear on reach.
    /// Default false = the normal cooldown-staggered recast. A property of the SKILL, not the mob.
    pub once_per_spawn: bool,
    /// MONSTER summon: number of familiars to spawn ringing the caster when this active fires
    /// (0 = not a summon). Cast via `try_cast_summon` → `spawn::spawn_familiars`; the familiars are
    /// scaled off the SUMMONER and carry NO drops / XP / Gil (a summoned mob must never mint loot —
    /// else infinite summons = an infinite farm). `summon_def_id` names the familiar's sprite/id.
    pub summon_count: u32,
    pub summon_def_id: &'static str,
    /// Passive party AURA: flat ATK per skill level granted to EVERY living
    /// ally (Blessing). Stamped onto `Entity.aura_atk` each tick by
    /// `tick::apply_party_auras`, then folded into `effective_atk`. This is the
    /// always-on aura path; active timed party buffs use `cast_buff` +
    /// `cast_buff_party` instead. Both coexist.
    pub party_atk_per_level: i32,
}

impl SkillDef {
    /// Damage multiplier (basis points) at the given skill level. The skill's own curve stops at
    /// `max_level`; every rank past it is an OVERRANK (Potential v2) worth
    /// `OVERRANK_BP_PER_RANK` on top — the one hook every skill shares, so rank 20 of a
    /// level-10 skill is +20%, never a second curve.
    pub fn power_at(&self, level: u8) -> u32 {
        let cap = self.max_level.max(1);
        let base = self.power_bp + self.power_per_level_bp * (level.min(cap).saturating_sub(1) as u32);
        let over = level.saturating_sub(cap) as u64;
        let mult = 10_000 + crate::potential::OVERRANK_BP_PER_RANK as u64 * over;
        ((base as u64) * mult / 10_000) as u32
    }
}

/// All-default `SkillDef` base for functional-record-update: a catalog entry
/// lists only the fields that DIFFER, then `..DEFAULT_SKILL`. Defaults are the
/// "does nothing" value of each field (0 / None / false / empty), with `range`
/// at 1 (the common melee reach). `id` / `kind` / `max_level` carry placeholder
/// defaults but every real skill overrides them.
pub const DEFAULT_SKILL: SkillDef = SkillDef {
    id: "",
    kind: SkillKind::Passive,
    max_level: 1,
    loadout_slots: 0,
    cooldown_ticks: 0,
    spawn_use_chance_bp: 0,
    cast_hp_below_pct: 0,
    cast_hp_above_pct: 0,
    max_charges: 0,
    cast_chance_bp: 0,
    motion_delay_ticks: 0,
    ground_field_radius: 0,
    field_hits: 0,
    field_hits_per_level: 0,
    field_strike_hits: 0,
    field_duration_ticks: 0,
    field_power_from_atk: false,
    field_center_self: false,
    cast_target_random: false,
    field_fuse: false,
    field_follows_caster: false,
    anim_slot: "",
    basic_range: 0,
    basic_ranged: false,
    basic_damage_type: None,
    basic_element: None,
    basic_power_bp: 0,
    status_at: 0,
    impact_radius: 0,
    once_per_target: false,
    display_hits: 1,
    power_bp: 0,
    power_per_level_bp: 0,
    range: 1,
    aoe_radius: 0,
    aoe_aspd_slow_bp: 0,
    self_aoe_radius: 0,
    knockback: 0,
    pull: 0,
    cast_buff: &[],
    cast_buff_party: false,
    on_hit_debuff: None,
    cast_element: None,
    damage_type: None,
    delivery: Delivery::Inherit,
    teleport: false,
    target_lowest_hp: false,
    target_newest_nonboss: false,
    cast_time_ticks: 0,
    hp_regen_flat: 0,
    hp_regen_maxhp_bp: 0,
    atk_flat_per_level: 0,
    dodge_bp_per_level: 0,
    double_atk_chance_bp: 0,
    def_scale_bp: 0,
    requires_shield: false,
    target_furthest: false,
    taunt_on_hit_ticks: 0,
    lb_max: 0,
    lb_on_hit_taken: 0,
    lb_on_basic: 0,
    lb_on_skill: 0,
    lb_on_break: 0,
    lb_per_second: 0,
    toughness_dmg_mult: 0,
    break_trigger: false,
    heals_ally: false,
    heal_scales_atk: false,
    volley_targets: 0,
    field_place_densest: false,
    mark_step_bp: 0,
    mark_max_stacks: 0,
    mark_ticks: 0,
    mark_slow_pct: 0,
    mark_aspd_slow_bp: 0,
    leech_bp: 0,
    field_pull: 0,
    pact_crit_bp: 0,
    pact_ticks: 0,
    debuff_amp_bp: 0,
    zone_wide: false,
    on_hit_debuff_2: None,
    self_aoe_max_targets: 0,
    communio_atk_bp: 0,
    heal_maxhp_bp: 0,
    heal_maxhp_per_level_bp: 0,
    heal_self_only: false,
    taunted_target_bp: 0,
    broken_target_bp: 0,
    basic_toughness_mult: 0,
    target_boss_first: false,
    kill_gil_bonus_bp: 0,
    pickpocket_gil_bp: 0,
    reset_on_break: false,
    fever_toughness_mult: 0,
    spark_stacks: 0,
    spark_atk_bp: 0,
    spark_keeper: false,
    spark_detonate: false,
    heal_on_hit_bp: 0,
    healed_ally_dmg_bp: 0,
    healed_ally_stacks: 0,
    element_roll_bp: 0,
    buff_top_atk_allies: 0,
    party_cooldown_cut_ticks: 0,
    aspd_kin_element: None,
    aspd_kin_mult_bp: 10_000,
    room_nuke: false,
    winter_chill_ticks: 0,
    shatter_bp: 0,
    shatter_extra_ticks: 0,
    aura_radius: 0,
    aura_dot_atk_bp: 0,
    aura_dot_per_level_bp: 0,
    aura_aspd_slow_bp: 0,
    aura_aspd_slow_per_level_bp: 0,
    hot_maxhp_bp: 0,
    hot_maxhp_per_level_bp: 0,
    move_while_hidden: false,
    once_per_spawn: false,
    summon_count: 0,
    summon_def_id: "",
    party_atk_per_level: 0,
};

/// A position in `Entity.skills`.
///
/// A newtype because this codebase has at least four other `usize` "slots" — provision slots,
/// consumable slots, equipped slots, inventory indices — and to the compiler they are all the
/// same type. `remove_provision(entity_id, slot)` and `resolve_buff_cast(state, i, slot)` take
/// interchangeable arguments today; they should not.
///
/// `#[serde(transparent)]` so the wire shape stays the bare integer it always was: saves written
/// before this type read back unchanged, no golden moves, no VERSION bump.
///
/// This does NOT make a slot safe to HOLD while `skills` changes — no type can express that. That
/// is what dropping the cast in `clear_exemplar_skill` and reading with `.get()` are for
/// (0.0.294). This stops the other mistake: handing an inventory index to something that wants a
/// skill.
#[derive(
    Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize,
)]
#[serde(transparent)]
pub struct SkillSlot(usize);

impl SkillSlot {
    pub const fn new(i: usize) -> Self {
        Self(i)
    }
    /// The raw position, for the one thing a slot is for: `skills.get(slot.get())`.
    pub const fn get(self) -> usize {
        self.0
    }
}

/// Per-entity runtime state for a known skill (part of the canonical
/// serialization → adding fields needs a VERSION bump).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SkillState {
    pub id: String,
    pub level: u8,
    /// Ticks until an active skill is usable again. Always 0 for passives.
    pub cooldown: u32,
    /// Tick this skill last fired. The auto-cast tie-breaker prefers the skill
    /// that fired LONGEST ago, so equal-priority skills rotate fairly (the
    /// "ready-time queue"). 0 = never fired. Skip-if-0 keeps old saves/goldens
    /// byte-identical until a skill actually casts.
    #[serde(default, skip_serializing_if = "is_zero_u64")]
    pub last_fired: u64,
    /// Remaining CHARGES (Pokémon-PP style) for a charge skill (Soul Crush). Each
    /// use spends one; refills to max on a zone change. 0 = empty / not a charge
    /// skill. Skip-if-0 keeps non-charge skills (every golden) byte-identical.
    #[serde(default, skip_serializing_if = "is_zero_u32")]
    pub charges_left: u32,
    /// PLAYER-CHOSEN cast level for a level-scalable skill (the mage bolts): the
    /// skill fires at THIS level instead of the learned `level`, trading cast time
    /// for hits (RO variable cast level). 0 = unset → cast at the learned level.
    /// Skip-if-0 keeps every existing save/golden byte-identical.
    #[serde(default, skip_serializing_if = "is_zero_u8")]
    pub cast_level: u8,
    /// True when this skill was GRANTED by an equipped Exemplar (not learned from the
    /// class tree). Materialized into the live loadout at `deploy` and stripped at `bench`
    /// so it never persists into the character's learned skills. Skip-if-false keeps every
    /// learned/golden skill byte-identical.
    #[serde(default, skip_serializing_if = "is_false")]
    pub from_exemplar: bool,
    /// True when this skill was GRANTED by an equipped Regalia (the weapon-overlay twin of
    /// `from_exemplar`). Same materialize-at-deploy / strip-at-bench lifecycle, tracked
    /// separately so the two overlays never strip each other's grants.
    #[serde(default, skip_serializing_if = "is_false")]
    pub from_regalia: bool,
}

fn is_false(b: &bool) -> bool {
    !*b
}

fn is_zero_u32(v: &u32) -> bool {
    *v == 0
}

fn is_zero_u64(v: &u64) -> bool {
    *v == 0
}

fn is_zero_u8(v: &u8) -> bool {
    *v == 0
}

impl SkillState {
    pub fn new(id: &str, level: u8) -> Self {
        Self {
            id: id.to_string(),
            level,
            cooldown: 0,
            last_fired: 0,
            // A limited-use skill starts full (`max_charges`); everything else is 0,
            // which the spend/refill paths already read as "not charge-limited".
            charges_left: skill_def(id).map_or(0, |d| d.max_charges),
            cast_level: 0,
            from_exemplar: false,
            from_regalia: false,
        }
    }

    /// The level this skill actually CASTS at: the player's `cast_level` when set
    /// and valid (1..=learned), else the learned `level`. Never exceeds what's
    /// learned and never returns 0 for a learned skill.
    pub fn effective_cast_level(&self) -> u8 {
        if self.cast_level >= 1 && self.cast_level <= self.level {
            self.cast_level
        } else {
            self.level
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn effective_cast_level_defaults_and_clamps() {
        let mut s = SkillState::new("fire_bolt", 5); // learned Lv5
        assert_eq!(s.effective_cast_level(), 5, "cast_level 0 → learned level");
        s.cast_level = 2;
        assert_eq!(s.effective_cast_level(), 2, "player-chosen level in range");
        s.cast_level = 9; // above learned → invalid
        assert_eq!(s.effective_cast_level(), 5, "clamps back to learned");
        s.cast_level = 0;
        assert_eq!(s.effective_cast_level(), 5, "0 resets to learned");
    }

    #[test]
    fn skill_power_scales_with_level() {
        assert_eq!(IRON_CHAINS.power_at(1), 10_000);
        assert_eq!(IRON_CHAINS.power_at(5), 10_000 + 4 * 1_000);
        // Past max_level the curve holds and each OVERRANK adds +2%: rank 12 of a lv-10 skill.
        let at_max = IRON_CHAINS.power_at(IRON_CHAINS.max_level);
        assert_eq!(IRON_CHAINS.power_at(IRON_CHAINS.max_level + 2), at_max * 10_400 / 10_000);
        assert_eq!(IRON_CHAINS.power_at(IRON_CHAINS.max_level + 10), at_max * 12_000 / 10_000);
    }

    #[test]
    fn unknown_skill_is_none() {
        assert!(skill_def("nope").is_none());
        assert!(skill_def("iron_chains").is_some());
        assert!(matches!(skill_def("bulwark").unwrap().kind, SkillKind::Passive));
    }

}
