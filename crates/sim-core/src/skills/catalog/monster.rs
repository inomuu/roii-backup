//! Monster-only skills. These never appear in a class tree — a monster carries
//! them via `MonsterDef.skills` (wired into `Entity.skills` at spawn), and the
//! sim reads them by id like the player MARKER passives (Steal / Hiding). Params
//! live here in code, same "tune in code, move to data later" stance as the rest
//! of the catalog. A monster may carry SEVERAL passives.

use super::super::{BuffKind, BuffSpec, DebuffSpec, Delivery, SkillDef, SkillKind, DEFAULT_SKILL};
use crate::grid::{DamageType, Element};

/// Self-Destruct — bomb-plant passive (ChomBombs, band 3): on the owner's DEATH,
/// detonate a blast centred on the corpse. CONVENTION: a `Passive` with
/// `self_aoe_radius > 0` IS an on-death blast — `combat_step::xp::schedule_explosion`
/// finds it and reads its params. `power_bp` is read as FLAT HP damage (there is no
/// caster ATK to scale off — the owner is already dead); `knockback` = cells every
/// survivor is shoved; `self_aoe_radius` = blast radius. Hits BOTH teams in range.
/// Deterministic (no PRNG); a bomb killed by another's blast chains its own.
pub const SELF_DESTRUCT: SkillDef = SkillDef {
    id: "self_destruct",
    kind: SkillKind::Passive,
    max_level: 1,
    power_bp: 5,        // FLAT HP damage (not a %) — see note above
    self_aoe_radius: 1, // 3×3 blast
    knockback: 1,       // shove survivors 1 cell
    ..DEFAULT_SKILL
};

/// Burst Blast — Burst Chom passive (band 4, room 2): a BIGGER on-death blast —
/// 5×5 (`self_aoe_radius` 2) for a flat 20 HP, still shoving every survivor. Same
/// on-death convention as `self_destruct`; no poison. A big red boom in the crowd.
pub const BURST_BLAST: SkillDef = SkillDef {
    id: "burst_blast",
    kind: SkillKind::Passive,
    max_level: 1,
    power_bp: 20,       // FLAT 20 HP damage
    self_aoe_radius: 2, // 5×5 blast
    knockback: 1,       // shove every survivor 1 cell
    ..DEFAULT_SKILL
};

/// Mending Burst — Rechum passive (band 4, room 3): on death, instead of harming,
/// it HEALS every ally MONSTER in a 5×5 (`self_aoe_radius` 2) for a flat `power_bp` HP.
/// `heals_ally` flips `resolve_explosion` from a damage blast to an ally heal (no damage,
/// knockback or poison). A support bomb — killing it late lets its pack heal up.
pub const MENDING_BURST: SkillDef = SkillDef {
    id: "mending_burst",
    kind: SkillKind::Passive,
    max_level: 1,
    power_bp: 50,       // FLAT 50 HP healed to allies
    self_aoe_radius: 2, // 5×5 heal pulse
    heals_ally: true,
    ..DEFAULT_SKILL
};

/// Toxic Burst — Toxic Chom passive (band 4): the SAME on-death blast as
/// Self-Destruct, but the cloud also POISONS every survivor caught (classic
/// `Poison` — a DoT that can never land the killing blow). `on_hit_debuff` carries
/// the Poison + its duration; `schedule_explosion` forwards it to the blast so
/// `resolve_explosion` applies it (bosses are immune, like every other Poison).
pub const TOXIC_BURST: SkillDef = SkillDef {
    id: "toxic_burst",
    kind: SkillKind::Passive,
    max_level: 1,
    power_bp: 5,
    self_aoe_radius: 1,
    knockback: 1,
    on_hit_debuff: Some(DebuffSpec {
        kind: BuffKind::Poison,
        magnitude: 1,
        per_level: 0,
        duration: 100, // 10 s of classic Poison
        chance_bp: 10_000,
        chance_per_level: 0,
    }),
    ..DEFAULT_SKILL
};

/// Poison Attack — a REUSABLE monster ACTIVE bite (first used by the Zombie). Unlike
/// the on-death blasts above, it fires DURING combat via the shared `pick_active_skill`
/// / `attack_step` path: `build_and_spawn_monster` copies a monster's `Active` skills
/// into `active_skills`, so the same pipeline that casts player actives casts this one.
/// A 150% hit that also lays classic `Poison` (a DoT that can never land the kill).
/// Any poison monster can carry it — tune the values (power / cd / cast) per use.
/// Cooldown-gated (30 s), 1.5 s wind-up. No PRNG.
pub const POISON_ATTACK: SkillDef = SkillDef {
    id: "poison_attack",
    kind: SkillKind::Active,
    max_level: 1,
    cooldown_ticks: 300,        // 30 s
    spawn_use_chance_bp: 5_000, // ~50% of a pack carries it (organic variety)
    cast_time_ticks: 15,        // 1.5 s wind-up (telegraphed bite)
    power_bp: 15_000,    // 150% ATK
    cast_element: Some(Element::Physical), // element-agnostic bite (reusable across mobs)
    on_hit_debuff: Some(DebuffSpec {
        kind: BuffKind::Poison,
        magnitude: 1,
        per_level: 0,
        duration: 100, // 10 s classic Poison
        chance_bp: 0,  // always — a guaranteed on-hit debuff takes NO PRNG draw (keeps
                       // the loot/refine stream byte-identical even as mobs cast)
        chance_per_level: 0,
    }),
    ..DEFAULT_SKILL
};

/// Umbral Roots — a monster ACTIVE ground field (first used by Gnarloak). Drops a small
/// SHADOW field at a target's feet that BOTH ticks damage AND slows anyone standing in
/// it — the same `ground_field` engine as the Elementalist zones (Vermilion = damage,
/// Quicksand = slow; this does both). Placed via `try_cast_ground_field`, which — like
/// every `try_cast_*` — is not team-gated, so it fires for a monster once the skill sits
/// in `active_skills`. `field_power_from_atk` scales the ticks off the caster's ATK
/// (monsters have no MATK). `cast_element` overrides the mob's own element to Shadow. No PRNG.
pub const UMBRAL_ROOTS: SkillDef = SkillDef {
    id: "umbral_roots",
    kind: SkillKind::Active,
    max_level: 1,
    cooldown_ticks: 300,        // 30 s
    spawn_use_chance_bp: 5_000, // ~50% of a pack carries it (same as poison_attack)
    motion_delay_ticks: 10,
    cast_time_ticks: 20,        // 2 s wind-up
    range: 8,                   // dropped at a target's feet (long reach)
    ground_field_radius: 1,     // 3×3
    // 80% ATK per tick, raised from 25% at 0.0.282.
    //
    // The field is TEN separate packets and the magic path subtracts soft MDEF from EACH, so the
    // flat term is charged ten times over. At 25% a tick was 15-21 ATK where this monster stands
    // (levels 21-25). A mage or acolyte there carries 30-42 soft MDEF, so every tick floored to 1
    // and the whole five seconds dealt 10.
    //
    // SCOPE, because the first write-up of this comment overstated it: this is a DoT-versus-INT
    // problem, not a broken magic system. A physical class at that band carries 10-19 soft MDEF
    // and was still taking 5-11 a tick, and single-packet monster magic lands 80-92% on the three
    // classes that never raise INT. Magic mitigation is doing roughly what RO intends.
    //
    // Swept rather than guessed. At 80% a tick is 49-69, comfortably clear of the floor for every
    // class, and the field lands 190-340 on a caster and 390-580 on a physical body across levels
    // 21-25. That is roughly two to three times what it dealt before 0.0.273 (90-140 / 150-210) —
    // a deliberate step up, not a restoration. 50% still floors against an INT class; 25% is what
    // it was.
    //
    // It treats the symptom. The cause is that a flat subtraction is charged per PACKET, so
    // splitting a total into N packets multiplies it by N. RO avoids that by making each tick huge
    // (Storm Gust is 570% ATK a tick) or by routing the effect out of the mitigation path
    // (`IgnoreDefense`, which its own ground fields carry). Neither was taken here.
    power_bp: 8_000,
    field_hits: 10,             // ~10 ticks over the 5 s
    field_duration_ticks: 50,   // 5 s
    field_power_from_atk: true, // scale off ATK — a monster has no MATK
    cast_element: Some(Element::Dark),
    on_hit_debuff: Some(DebuffSpec {
        kind: BuffKind::AgiDexSlowPct,
        magnitude: 25, // −25% AGI/DEX/move while inside
        per_level: 0,
        duration: 30,  // 3 s, re-applied while standing in it
        chance_bp: 10_000, // ground-field convention (Quicksand): always inside the field
        chance_per_level: 0,
    }),
    ..DEFAULT_SKILL
};

/// Blood Slash — Crimsonne's ACTIVE (Duskmourn cast). A quick Shadow slash that opens a
/// BLEED (`BuffKind::Bleed`, ATK-scaled DoT — the thief's Open Wound uses the same kind,
/// so it will NOT double up with Poison). Same shape as `poison_attack` (an Active hit via
/// `pick_active_skill`) but a faster, aggressive-bruiser cadence. A signature named mob →
/// no spawn gate (always carries it). `cast_element` forces Shadow; `chance_bp: 0` applies
/// the bleed deterministically with NO PRNG draw. No PRNG.
pub const BLOOD_SLASH: SkillDef = SkillDef {
    id: "blood_slash",
    kind: SkillKind::Active,
    max_level: 1,
    cooldown_ticks: 60, // 6 s
    cast_time_ticks: 8, // 0.8 s — a quick slash, not a full spell wind-up
    power_bp: 8_000,    // 80% ATK direct hit (the bleed carries the rest)
    cast_element: Some(Element::Dark),
    on_hit_debuff: Some(DebuffSpec {
        kind: BuffKind::Bleed,
        magnitude: 20_000, // 200% ATK total bleed over the duration
        per_level: 0,
        duration: 70, // 7 s
        chance_bp: 0, // always — no PRNG draw
        chance_per_level: 0,
    }),
    ..DEFAULT_SKILL
};

/// Pyre Mark — Dulahaness's INNATE hazard (user 2026-07-30). Every 8 s a mark is
/// planted under a RANDOM party member; 3 s later it erupts in a 3×3 Fire blast. She
/// does NOT stop to cast it — the room is simply always about to be on fire somewhere,
/// and the countdown lives on the mark, not on her.
///
/// The mark burns the GROUND it was set on and never follows its victim, so both
/// answers work: walk clear, or lift the marked member off the board (`regent_hand`).
/// An idle party does neither and eats every blast.
///
/// PHASED: this is the CALM half only (`cast_hp_above_pct: 50`). Below half health it
/// switches off and `PYRE_MARK_WOUNDED` takes over — a hard swap, not a second skill
/// layered on top. The upper gate is the whole point: without it both versions stay
/// live below the threshold and fire on independent cooldowns, which is exactly the
/// doubled-up AoE seen in the first play test.
///
/// Ground fields already resolve as MAGIC against MDEF, so "damage as magic" needed no
/// new damage path. Two hooks were added for this skill: `cast_target_random` (a
/// nearest-target field can only ever threaten the frontline, never the healer behind
/// it) and the FUSE — a field inert until its final tick, which is what makes a mark a
/// warning rather than damage that has already landed.
///
/// Damage starts at 1× ATK (user: "1x ก่อน ทดลอง"); `field_power_from_atk` because a
/// monster carries no MATK. `field_hits: 1` over the fuse = ONE detonation.
pub const PYRE_MARK: SkillDef = SkillDef {
    id: "pyre_mark",
    kind: SkillKind::Active,
    max_level: 1,
    cooldown_ticks: 80,        // every 8 s
    cast_hp_above_pct: 50,     // calm half only — hands over at 50% (see PYRE_MARK_WOUNDED)
    range: 9,
    ground_field_radius: 1,    // 3×3 blast
    field_duration_ticks: 30,  // a 3 s FUSE — the window to walk clear or be lifted out
    field_fuse: true,          // inert until that last tick (see GroundFieldKind::Damage)
    field_hits: 1,             // one detonation
    power_bp: 10_000,          // 1× ATK, as magic (vs MDEF)
    field_power_from_atk: true,
    cast_target_random: true,
    cast_element: Some(Element::Fire),
    ..DEFAULT_SKILL
};

/// Pyre Mark (wounded) — the SAME mark, twice as often, once Dulahaness drops below
/// half health (`cast_hp_below_pct: 50`, the mirror of the base skill's upper gate, so
/// exactly one of the pair is ever live).
///
/// It is a separate def rather than a cadence field on the base skill because the
/// wounded version is expected to diverge further than timing — a bigger blast, harder
/// damage, a debuff on hit. Everything below is an independent dial; only `id` and the
/// gate have to stay paired with the base.
pub const PYRE_MARK_WOUNDED: SkillDef = SkillDef {
    id: "pyre_mark_wounded",
    kind: SkillKind::Active,
    max_level: 1,
    cooldown_ticks: 40,        // every 4 s — the wounded cadence
    cast_hp_below_pct: 50,     // wounded half only
    range: 9,
    ground_field_radius: 1,    // (dial: widen for a nastier phase)
    field_duration_ticks: 30,
    field_fuse: true,
    field_hits: 1,
    power_bp: 10_000,          // (dial: raise for a nastier phase)
    field_power_from_atk: true,
    cast_target_random: true,
    cast_element: Some(Element::Fire),
    ..DEFAULT_SKILL
};

/// Handmaid's Grasp — the BOSS-ROOM handmaids' ACTIVE (Duskmourn arenas). A weak clutch
/// that TAUNTS its victim onto the maid for 4 s (`taunt_on_hit_ticks` — the same lock the
/// Swordmaster's provoke uses; `find_enemy_idx` honours it over nearest-target).
///
/// This is the answer to a measured problem, not flavour: with the escort ring and the boss
/// side by side, the party's nearest-target rule made it kill the BOSS through the ring, so
/// eight minions changed nothing. A taunt makes the ring genuinely absorb attention — you
/// fight your way in rather than past.
///
/// Carried by `maid_warden` / `maid_horn_warden` ONLY (user 2026-07-30: taunt belongs to the
/// boss rooms). A monster's skills live on its DEF, which is game-wide, so the boss-room
/// handmaids are their own defs — the same trick `brutus_lord` uses to be "a werewolf with
/// boss stats". The field maids keep their own kit and never taunt.
pub const HANDMAIDS_GRASP: SkillDef = SkillDef {
    id: "handmaids_grasp",
    kind: SkillKind::Active,
    max_level: 1,
    cooldown_ticks: 80,      // 8 s per maid — with a ring of 8 the lock is near-continuous
    power_bp: 4_000,         // 40% ATK — the taunt is the payload, not the damage
    taunt_on_hit_ticks: 40,  // 4 s glued to whoever grabbed you
    ..DEFAULT_SKILL
};

/// Rite of the Severed Head — Dulahaness's WOUNDED-phase ACTIVE (user 2026-07-30: "another
/// wave of minions at 50% HP"). Below half health she raises her own head and the household
/// answers ONCE: `summon_count` handmaids ring her via the existing summon path.
///
/// `max_charges: 1` is what makes it once — a monster is built fresh at every spawn, so the
/// budget is per FIGHT. A long cooldown would only have made the repeat rare (it fired every
/// 40 s through the whole wounded phase, which a play test caught).
///
/// Two consequences of that path worth knowing (they are the engine's rules, not choices
/// made here): a summon's stats scale off the SUMMONER (20% max HP / 40% ATK, so these are
/// far tankier than a field maid) and it carries NO skills — so the summoned wave does not
/// taunt. It also drops nothing and grants no XP, which is what keeps a summoning boss from
/// being an XP farm.
pub const RITE_OF_THE_SEVERED_HEAD: SkillDef = SkillDef {
    id: "rite_of_the_severed_head",
    kind: SkillKind::Active,
    max_level: 1,
    cooldown_ticks: 400,      // (irrelevant while max_charges is 1 — kept as the floor
                              //  if the budget is ever raised)
    max_charges: 1,           // ONE wave per fight (user 2026-07-30: it was repeating
                              //  every 40 s — "another set at 50%" means one set)
    cast_hp_below_pct: 50,    // the phase gate: never before she is wounded
    motion_delay_ticks: 15,   // 1.5 s rite anim lock — a visible window
    summon_count: 4,
    summon_def_id: "maid",
    ..DEFAULT_SKILL
};

/// PLACEHOLDER (user 2026-07-29): stands in for Dulahaness's real signature skill,
/// still to be designed. It exists to give the `regent_hand` pick-up something to
/// dodge, so treat its numbers as scaffolding, not balance. What is NOT placeholder
/// is the engine contract it proved out: a wind-up LOCKS its aim cell
/// (`Casting.ground_cell`), so any telegraphed field — this one or its replacement —
/// stays where it was aimed instead of chasing its target.
///
/// Deathknell — Dulahaness's ACTIVE (Duskmourn Manor end-boss). A 3 s TELEGRAPHED
/// Shadow barrage: five grave-bells fall one per second onto random cells in a 7×7
/// area, each blasting 5×5 where it lands (`impact_radius` — a drop can miss).
/// Reuses the Meteor Rain field engine; the count/hits are authored flat here
/// (a monster is always skill level 1, so the player level curve can't shape it).
/// The 3 s wind-up is the POINT: it is the window the player answers with the
/// `regent_hand` pick-up (`ZoneDef.player_skills`), the first content-gated player
/// skill. Eating the whole barrage should nearly kill a Lv50 member; catching two
/// or three drops hurts. `field_power_from_atk` scales off ATK (monsters have no
/// MATK). No PRNG beyond the field's own random drop cells.
pub const DEATHKNELL: SkillDef = SkillDef {
    id: "deathknell",
    kind: SkillKind::Active,
    max_level: 1,
    cooldown_ticks: 400,       // 40 s — one barrage per phase, not a rhythm
    cast_time_ticks: 30,       // 3 s telegraph: the dodge window
    motion_delay_ticks: 15,
    range: 9,
    ground_field_radius: 3,    // 7×7 area the bells land in
    impact_radius: 2,          // each bell's 5×5 blast (meteor mode)
    field_duration_ticks: 50,  // 5 s
    field_hits: 5,             // 5 bells ≈ one per second
    field_strike_hits: 3,      // 3 numbers per blast (ONE damage packet)
    power_bp: 2_500,           // 25% ATK per number → 75% ATK a blast
    field_power_from_atk: true,
    cast_element: Some(Element::Dark),
    ..DEFAULT_SKILL
};

/// Bloodfrenzy — Gloomtooth's PASSIVE (Duskmourn cast). Every LANDED basic hit ramps its
/// OWN attack speed +5%, capped at +100% — a fast rabbit that only gets faster the longer
/// it's left alive (and decays if it stops attacking). A marker passive:
/// `combat_step::try_bloodfrenzy` reads it on each landed hit and grows a self ASPD buff.
/// No target debuff, no cast — the threat is the ramp. No PRNG.
pub const BLOODFRENZY: SkillDef = SkillDef {
    id: "bloodfrenzy",
    kind: SkillKind::Passive,
    max_level: 1,
    ..DEFAULT_SKILL
};

/// Puppet Lunge — Marionette's ACTIVE (Duskmourn cast). Yanks itself on its strings to the
/// lowest-HP party member and backstabs — the SAME `teleport` + `target_lowest_hp` mechanic
/// as the thief's Shadow Strike, reused wholesale. A signature dive that punishes a weakened
/// member from across the field. `cast_element` forces Shadow. No PRNG.
pub const PUPPET_LUNGE: SkillDef = SkillDef {
    id: "puppet_lunge",
    kind: SkillKind::Active,
    max_level: 1,
    cooldown_ticks: 60, // 6 s
    power_bp: 25_000,   // 250% backstab burst
    range: 5,           // teleport reach — dives from up to 5 cells away
    teleport: true,
    target_lowest_hp: true,
    cast_element: Some(Element::Dark),
    ..DEFAULT_SKILL
};

/// Feral Transformation's ONE effect: -50% incoming damage (5000 bp) — the thick hide of the
/// transformed beast. `DmgReducePct` is the same flat cut the mage's Arcane Ward uses (folded
/// in `combat::apply_damage`). The bigger SPRITE that sells the transform is a renderer job
/// (Godot scales it while this buff is up).
const FERAL_BUFFS: &[BuffSpec] = &[
    BuffSpec { kind: BuffKind::DmgReducePct, magnitude: 5_000, per_level: 0, duration: 120 },
];

/// Feral Transformation — Brutus's ENRAGE (Duskmourn cast, werewolf). A DESPERATION stance: only
/// once the wolf is wounded below 50% HP (`cast_hp_below_pct`) does it try to transform, and even
/// then only a 50% chance per attempt (`cast_chance_bp`). On success it thickens its hide — a
/// single -50% incoming-damage self-buff, nothing more. A pure self-buff (`cast_buff`, no attack).
/// A failed roll burns the 10 s cooldown and it re-rolls next cycle; the 12 s buff outlasts the cd
/// so once it lands it holds while it keeps fighting. Its bleeding bite is a SEPARATE skill
/// (Brutus carries `blood_slash` too) — deliberately not folded in here.
pub const FERAL_TRANSFORMATION: SkillDef = SkillDef {
    id: "feral_transformation",
    kind: SkillKind::Active,
    max_level: 1,
    cooldown_ticks: 100,      // 10 s — the retry cadence while wounded
    cast_hp_below_pct: 30,    // only enrages below 30% HP (user 2026-10-19; 50% until then)
    cast_chance_bp: 5_000,    // 50% chance per attempt (a real combat PRNG draw)
    cast_buff: FERAL_BUFFS,   // self -50% incoming (the transformation, its only effect)
    ..DEFAULT_SKILL
};

/// Wail — Banshee's ACTIVE (Duskmourn cast). A demoralizing Shadow scream: the VALUE is the AoE
/// ASPD slow — every party member within a 5×5 of the struck target is slowed -30% ASPD via
/// `aoe_aspd_slow_bp` (the generalized acolyte Aspd Dampening path — the effect rides the skill,
/// not a hardcoded id). It targets a member NOT already slowed, so repeated wails SPREAD the slow
/// across the party rather than re-slowing one. A pure-buff (power 0) can't route through the
/// attack path (`pick_active_skill` skips power_bp==0), so it carries a minimal token chip; the
/// damage is incidental. `cast_element` forces Shadow. No PRNG beyond the ordinary attack draws.
pub const WAIL: SkillDef = SkillDef {
    id: "wail",
    kind: SkillKind::Active,
    max_level: 1,
    cooldown_ticks: 150,     // 15 s
    cast_time_ticks: 15,     // 1.5 s wailing wind-up
    power_bp: 0,             // ZERO damage — a pure debuff. pick_active_skill lets a dampener through
                             // at power 0, and resolve_instant skips the strike (which would else floor
                             // to min-1). The wail's only value is the slow.
    range: 6,
    aoe_radius: 2,           // 5×5 scream
    aoe_aspd_slow_bp: 1_500, // -15% ASPD to every party member caught (-30% until 0.0.419: with the maid's
                             // heal it held the real party on stages 84-86 for ten days)
    max_charges: 1,          // ONCE per spawn, same -15% (user 2026-10-19); was every 15 s
    cast_element: Some(Element::Dark),
    ..DEFAULT_SKILL
};

/// Mending Dirge — Dustelle's ACTIVE (Duskmourn maid, the healer sister — her card casts Heal).
/// A monster AoE ally-heal via the shared `try_cast_support` path (not team-gated, so it fires for
/// a mob). It centres on the most-injured ally in range, then mends EVERY ally within a 5×5
/// (`aoe_radius` 2) for 15% of THAT ally's MAX HP (`heal_maxhp_bp`, so it scales with a monster's
/// big pool — the INT/MATK Renewal heal barely moves it). No PRNG.
pub const MENDING_DIRGE: SkillDef = SkillDef {
    id: "mending_dirge",
    kind: SkillKind::Active,
    max_level: 1,
    cooldown_ticks: 100,   // 10 s
    motion_delay_ticks: 5, // 0.5 s heal-cast anim lock (support path applies the heal at once)
    range: 6,
    aoe_radius: 2,        // 5×5 — heals the whole huddle, not just one
    heals_ally: true,
    heal_maxhp_bp: 1_000, // heal 10% of each ally's MAX HP (15% until 0.0.419, see `wail`)
    max_charges: 2,       // TWO casts per maid per spawn, same strength (user 2026-10-19); was every 10 s
    ..DEFAULT_SKILL
};

/// Ashelle's pack-buff: +30% ASPD to every ally (`AspdPct` is bp — 3000 = 30%). Matches her
/// card theme (ASPD-on-heal). Near-permanent while she lives (duration ≈ cd).
const FRENZIED_RITE_BUFF: &[BuffSpec] = &[
    BuffSpec { kind: BuffKind::AspdPct, magnitude: 3_000, per_level: 0, duration: 150 },
];

/// Frenzied Rite — Ashelle's ACTIVE (Duskmourn maid_horn, the horned elder sister who bargained
/// in the cellar). She goads the whole pack into a frenzy: a party-wide +30% ASPD buff via
/// `cast_buff_party` (the same pure-buff cast path as Blessing — power 0 → routes through
/// `try_cast_buffs`, not team-gated). Buffs every monster on her team, herself included. No PRNG.
pub const FRENZIED_RITE: SkillDef = SkillDef {
    id: "frenzied_rite",
    kind: SkillKind::Active,
    max_level: 1,
    cooldown_ticks: 150,          // 15 s
    cast_time_ticks: 15,          // 1.5 s rite wind-up
    cast_buff: FRENZIED_RITE_BUFF, // +30% ASPD to the pack
    cast_buff_party: true,
    max_charges: 1,               // ONCE per spawn, same +30% (user 2026-10-19); was every 15 s
    ..DEFAULT_SKILL
};

/// Hiding self-buff for Twinling's Vanish: untargetable for 3 s. Same `Hiding` marker the thief's
/// panic hide uses (targeting.rs skips a hidden entity) — but paired with `move_while_hidden`.
const VANISH_BUFF: &[BuffSpec] = &[
    // Long duration = a SAFETY CAP only. The stalk normally reappears on REACH (the tick's
    // Hiding guard drops it once in attack range of the target); this cap just forces a
    // reappear if it somehow never closes the gap (target unreachable), so it can't stay
    // invisible forever.
    BuffSpec { kind: BuffKind::Hiding, magnitude: 1, per_level: 0, duration: 100 },
];

/// Vanish — Twinling's ACTIVE (Duskmourn mini_ghost). A ONE-TIME spawn stalk: 1-3 s after it
/// spawns (a hash-derived delay in `spawn`, `once_per_spawn`) it winks out of sight (a `Hiding`
/// self-buff, untargetable) and — unlike the thief's stand-still panic hide — STALKS while hidden:
/// the tick's Hiding guard sees `move_while_hidden` and keeps stepping it toward its target, then
/// REAPPEARS once it reaches attack range and fights normally (never re-hides — the huge
/// `cooldown_ticks` blocks a recast). A pure self-buff (power 0 → routes through `try_cast_buffs`).
/// Not a teleport — it walks. No PRNG.
pub const VANISH: SkillDef = SkillDef {
    id: "vanish",
    kind: SkillKind::Active,
    max_level: 1,
    cooldown_ticks: 100_000, // effectively one-time (never recasts in a fight)
    once_per_spawn: true,    // fires 1-3 s after spawn (hash delay), not on a recast cadence
    cast_buff: VANISH_BUFF,  // self Hiding (untargetable) until it reaches the target
    move_while_hidden: true, // ...and it STALKS while hidden (walks toward its target)
    ..DEFAULT_SKILL
};

/// Self-buffs for Scarlette's Quick Step: nimble footwork — +15% DODGE (the first skill to grant
/// `DodgeBpAdd`, which folds into `stat_mods::dodge_bp` as one more source) and +30% ASPD
/// (`AspdPct` bp). Duration > cd so she keeps it up while dancing.
const QUICK_STEP_BUFF: &[BuffSpec] = &[
    BuffSpec { kind: BuffKind::DodgeBpAdd, magnitude: 1_500, per_level: 0, duration: 150 },
    BuffSpec { kind: BuffKind::AspdPct, magnitude: 3_000, per_level: 0, duration: 150 },
];

/// Quick Step — Scarlette's ACTIVE (Duskmourn red_hood). A dancer's nimble self-buff: she becomes
/// hard to pin (+80 Flee = more dodges) and fast (+30% ASPD) for the window. A pure self-buff
/// (`cast_buff`, no party, no attack — power 0 → routes through `try_cast_buffs`), recast on a cd
/// shorter than the buff so it stays up. No PRNG (the evasion draws happen on incoming attacks).
pub const QUICK_STEP: SkillDef = SkillDef {
    id: "quick_step",
    kind: SkillKind::Active,
    max_level: 1,
    cooldown_ticks: 120,        // 12 s
    cast_time_ticks: 8,         // 0.8 s
    cast_buff: QUICK_STEP_BUFF, // self +Flee +ASPD
    ..DEFAULT_SKILL
};

/// Witch's Fireball — Broomhilda's ACTIVE nuke (Duskmourn witch). A monster Fireball: a 200% ATK
/// blast in a 3×3 (`aoe_radius` 1), `cast_element` forced to Fire (she is a Shadow mob but her
/// magic is fire — matches `card_broomhilda`). Scales the caster's ATK like any monster damage cast
/// (a monster has no MATK). Routes through the normal attack/cast path. No PRNG beyond the blast.
pub const WITCH_FIREBALL: SkillDef = SkillDef {
    id: "witch_fireball",
    kind: SkillKind::Active,
    max_level: 1,
    cooldown_ticks: 100,  // 10 s
    cast_time_ticks: 20,  // 2 s wind-up
    power_bp: 20_000,     // 200% ATK
    range: 6,
    aoe_radius: 1,        // 3×3 blast
    cast_element: Some(Element::Fire),
    // A MAGIC nuke: MDEF-mitigated + undodgeable (magic skips the flee roll), but base = ATK
    // via field_power_from_atk (a monster has no MATK), like Sanctus Terra's ATK-scaled field.
    damage_type: Some(DamageType::Magic),
    field_power_from_atk: true,
    // FORCE the projectile path: the 3×3 splash lives in `resolve_projectile`. A monster's
    // proj_travel_ticks is 0 (would Inherit → instant → single-target), so declare it flies.
    delivery: Delivery::Projectile,
    ..DEFAULT_SKILL
};

/// Summon Bats — Broomhilda's ACTIVE (Duskmourn witch). She calls a flock of bats: `summon_count`
/// familiars ring her via `try_cast_summon` → `spawn::spawn_familiars`. The bats scale off HER
/// (20% HP / 40% ATK) and drop NOTHING (no loot/XP — a summoned mob must never be farmable). A pure
/// utility cast (power 0, `summon_count` > 0), so `pick_active_skill` skips it and the summon path
/// takes it. `motion_delay` plays the summon anim. No PRNG (fixed ring placement, skill-less bats).
pub const SUMMON_BATS: SkillDef = SkillDef {
    id: "summon_bats",
    kind: SkillKind::Active,
    max_level: 1,
    cooldown_ticks: 200,    // 20 s
    motion_delay_ticks: 10, // 1 s summon anim lock
    summon_count: 2,        // a pair of bats
    summon_def_id: "bat",
    ..DEFAULT_SKILL
};
